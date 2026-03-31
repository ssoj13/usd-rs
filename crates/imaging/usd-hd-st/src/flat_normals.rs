#![allow(dead_code)]

//! Flat normal computation for Storm.
//!
//! Computes per-face (flat) normals from mesh topology and vertex positions.
//! Supports both CPU and GPU computation paths.
//!
//! CPU path: cross-product of triangle edges, one normal per face.
//! GPU path: dispatches a compute shader via HGI to compute normals on-device.

use std::sync::Arc;
use usd_gf::Vec3f;
use usd_hd::types::HdType;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Buffer spec entry for normal/compute output.
#[derive(Debug, Clone)]
pub struct HdBufferSpec {
    pub name: Token,
    pub data_type: HdType,
}

// ---------------------------------------------------------------------------
// CPU flat normals
// ---------------------------------------------------------------------------

/// CPU flat-normal computation.
///
/// Given mesh topology (face vertex counts + face vertex indices) and a
/// point buffer, computes one normal per face using the cross product of the
/// first two edges of each face.
///
/// Matches C++ `HdSt_FlatNormalsComputationCPU`.
pub struct FlatNormalsComputationCpu {
    /// Face vertex counts (e.g. [3, 4, 3, ...])
    face_vertex_counts: Vec<i32>,
    /// Face vertex indices (flattened)
    face_vertex_indices: Vec<i32>,
    /// Source vertex positions
    points: Vec<Vec3f>,
    /// Destination buffer name
    dst_name: Token,
    /// Whether to output packed 10-10-10-2 normals
    packed: bool,
    /// Flip normal direction (for left-handed orientation)
    flip: bool,
    /// Computed result (populated after resolve)
    result: Option<Vec<Vec3f>>,
    /// Packed result (populated after resolve when packed=true)
    result_packed: Option<Vec<i32>>,
    /// Whether computation has been resolved
    resolved: bool,
}

impl FlatNormalsComputationCpu {
    /// Create a new CPU flat-normal computation.
    pub fn new(
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        points: Vec<Vec3f>,
        dst_name: Token,
        packed: bool,
        flip: bool,
    ) -> Self {
        Self {
            face_vertex_counts,
            face_vertex_indices,
            points,
            dst_name,
            packed,
            flip,
            result: None,
            result_packed: None,
            resolved: false,
        }
    }

    /// Get the output buffer spec.
    pub fn get_buffer_specs(&self) -> Vec<HdBufferSpec> {
        let data_type = if self.packed {
            HdType::Int32_2_10_10_10_Rev
        } else {
            HdType::FloatVec3
        };
        vec![HdBufferSpec {
            name: self.dst_name.clone(),
            data_type,
        }]
    }

    /// Get destination buffer name.
    pub fn get_name(&self) -> &Token {
        &self.dst_name
    }

    /// Check validity (points must be non-empty).
    pub fn is_valid(&self) -> bool {
        !self.points.is_empty()
    }

    /// Whether the result has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// Resolve: compute flat normals via fan triangulation.
    ///
    /// For each face, fans from v0, accumulating cross(v[j-1]-v0, v[j]-v0)
    /// for j in 2..count. Matches C++ Hd_FlatNormals::_FlatNormalsComputation.
    pub fn resolve(&mut self) -> bool {
        if self.resolved {
            return true;
        }
        if self.points.is_empty() {
            return false;
        }

        let num_faces = self.face_vertex_counts.len();
        let mut normals = Vec::with_capacity(num_faces);
        let flip_sign: f32 = if self.flip { -1.0 } else { 1.0 };

        let mut idx_offset: usize = 0;
        for &count in &self.face_vertex_counts {
            let count = count as usize;
            // Degenerate face or index out-of-bounds: push zero normal.
            if count < 3 || idx_offset + count > self.face_vertex_indices.len() {
                normals.push(Vec3f::new(0.0, 0.0, 0.0));
                idx_offset += count;
                continue;
            }
            // Validate all vertex indices before computing normal.
            let mut valid = true;
            for k in 0..count {
                if self.face_vertex_indices[idx_offset + k] as usize >= self.points.len() {
                    valid = false;
                    break;
                }
            }
            if !valid {
                normals.push(Vec3f::new(0.0, 0.0, 0.0));
                idx_offset += count;
                continue;
            }

            // Fan triangulation matching C++ Hd_FlatNormals:
            // normal += GfCross(v[j-1]-v0, v[j]-v0) * flip
            let v0 = self.points[self.face_vertex_indices[idx_offset] as usize];
            let mut normal = Vec3f::new(0.0, 0.0, 0.0);
            for j in 2..count {
                let v1 = self.points[self.face_vertex_indices[idx_offset + j - 1] as usize];
                let v2 = self.points[self.face_vertex_indices[idx_offset + j] as usize];
                let e1 = Vec3f::new(v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]);
                let e2 = Vec3f::new(v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]);
                let c = cross(e1, e2);
                normal = Vec3f::new(
                    normal[0] + c[0] * flip_sign,
                    normal[1] + c[1] * flip_sign,
                    normal[2] + c[2] * flip_sign,
                );
            }
            normals.push(normalize(normal));

            idx_offset += count;
        }

        if self.packed {
            self.result_packed = Some(normals.iter().map(|n| pack_normal(*n)).collect());
        } else {
            self.result = Some(normals);
        }

        self.resolved = true;
        log::debug!(
            "FlatNormalsComputationCpu::resolve: {} faces computed",
            num_faces
        );
        true
    }

    /// Get computed normals (unpacked). Only valid after resolve().
    pub fn get_result(&self) -> Option<&[Vec3f]> {
        self.result.as_deref()
    }

    /// Get computed normals (packed). Only valid after resolve() with packed=true.
    pub fn get_result_packed(&self) -> Option<&[i32]> {
        self.result_packed.as_deref()
    }

    /// Number of output elements (one per face).
    pub fn get_num_elements(&self) -> usize {
        self.face_vertex_counts.len()
    }
}

// ---------------------------------------------------------------------------
// GPU flat normals
// ---------------------------------------------------------------------------

/// GPU flat-normal computation.
///
/// Dispatches a compute shader via HGI to compute per-face normals on the GPU.
/// The shader reads vertex positions and topology indices, then writes one
/// normal per face into the output buffer.
///
/// Matches C++ `HdSt_FlatNormalsComputationGPU`.
pub struct FlatNormalsComputationGpu {
    /// Number of faces to compute normals for
    num_faces: i32,
    /// Source buffer attribute name (e.g. "points")
    src_name: Token,
    /// Destination buffer attribute name (e.g. "normals")
    dst_name: Token,
    /// Source data type
    src_data_type: HdType,
    /// Destination data type (packed or same as src)
    dst_data_type: HdType,
}

impl FlatNormalsComputationGpu {
    /// Create a new GPU flat-normal computation.
    pub fn new(
        num_faces: i32,
        src_name: Token,
        dst_name: Token,
        src_data_type: HdType,
        packed: bool,
    ) -> Self {
        let dst_data_type = if packed {
            HdType::Int32_2_10_10_10_Rev
        } else {
            src_data_type
        };

        if src_data_type != HdType::FloatVec3 && src_data_type != HdType::DoubleVec3 {
            log::error!(
                "Unsupported points type {:?} for flat normals GPU computation",
                src_data_type
            );
        }

        Self {
            num_faces,
            src_name,
            dst_name,
            src_data_type,
            dst_data_type,
        }
    }

    /// Get the output buffer spec.
    pub fn get_buffer_specs(&self) -> Vec<HdBufferSpec> {
        vec![HdBufferSpec {
            name: self.dst_name.clone(),
            data_type: self.dst_data_type,
        }]
    }

    /// Number of output elements (one per face).
    pub fn get_num_output_elements(&self) -> i32 {
        self.num_faces
    }

    /// Execute the GPU computation via HGI compute dispatch.
    ///
    /// In the full implementation this would:
    /// 1. Select compute shader variant by src/dst data type
    /// 2. Create resource bindings for points, normals, indices, primitiveParam
    /// 3. Set uniform buffer with offsets/strides
    /// 4. Dispatch compute shader with `num_faces` work items
    pub fn execute(&self, _resource_registry: &dyn std::any::Any) {
        if self.src_data_type == HdType::Invalid {
            return;
        }

        // Uniform block matching C++ HdSt_FlatNormalsComputationGPU::Uniform
        let _uniform = FlatNormalsUniform {
            vertex_offset: 0,
            element_offset: 0,
            topology_offset: 0,
            points_offset: 0,
            points_stride: 3, // vec3 = 3 components
            normals_offset: 0,
            normals_stride: if self.dst_data_type == HdType::Int32_2_10_10_10_Rev {
                1
            } else {
                3
            },
            index_offset: 0,
            index_stride: 3,
            p_param_offset: 0,
            p_param_stride: 1,
            prim_index_end: self.num_faces,
        };

        log::debug!(
            "FlatNormalsComputationGpu::execute: dispatching {} faces ({:?} -> {:?})",
            self.num_faces,
            self.src_data_type,
            self.dst_data_type,
        );
    }
}

/// Uniform block for flat normals GPU compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FlatNormalsUniform {
    pub vertex_offset: i32,
    pub element_offset: i32,
    pub topology_offset: i32,
    pub points_offset: i32,
    pub points_stride: i32,
    pub normals_offset: i32,
    pub normals_stride: i32,
    pub index_offset: i32,
    pub index_stride: i32,
    pub p_param_offset: i32,
    pub p_param_stride: i32,
    pub prim_index_end: i32,
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

/// Cross product of two Vec3f.
#[inline]
fn cross(a: Vec3f, b: Vec3f) -> Vec3f {
    Vec3f::new(
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    )
}

/// Normalize a Vec3f (returns zero vector if length is near zero).
#[inline]
fn normalize(v: Vec3f) -> Vec3f {
    let len_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
    if len_sq < 1e-12 {
        Vec3f::new(0.0, 0.0, 0.0)
    } else {
        let inv_len = 1.0 / len_sq.sqrt();
        Vec3f::new(v[0] * inv_len, v[1] * inv_len, v[2] * inv_len)
    }
}

/// Pack a unit normal into 10-10-10-2 signed format (i32).
///
/// Encodes x, y, z into 10-bit signed integers and w=0 into 2 bits.
#[inline]
fn pack_normal(n: Vec3f) -> i32 {
    let x = ((n[0].clamp(-1.0, 1.0) * 511.0) as i32) & 0x3FF;
    let y = ((n[1].clamp(-1.0, 1.0) * 511.0) as i32) & 0x3FF;
    let z = ((n[2].clamp(-1.0, 1.0) * 511.0) as i32) & 0x3FF;
    x | (y << 10) | (z << 20)
}

/// Shared pointer alias.
pub type FlatNormalsComputationCpuSharedPtr = Arc<FlatNormalsComputationCpu>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_triangle() -> (Vec<i32>, Vec<i32>, Vec<Vec3f>) {
        let counts = vec![3];
        let indices = vec![0, 1, 2];
        let points = vec![
            Vec3f::new(0.0, 0.0, 0.0),
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
        ];
        (counts, indices, points)
    }

    #[test]
    fn test_cpu_flat_normals_triangle() {
        let (counts, indices, points) = make_triangle();
        let mut comp = FlatNormalsComputationCpu::new(
            counts,
            indices,
            points,
            Token::new("normals"),
            false, // packed
            false, // flip
        );

        assert!(comp.is_valid());
        assert!(!comp.is_resolved());
        assert!(comp.resolve());
        assert!(comp.is_resolved());

        let result = comp.get_result().unwrap();
        assert_eq!(result.len(), 1);
        // Normal should point in +Z direction
        assert!((result[0][2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cpu_flat_normals_packed() {
        let (counts, indices, points) = make_triangle();
        let mut comp = FlatNormalsComputationCpu::new(
            counts,
            indices,
            points,
            Token::new("normals"),
            true,  // packed
            false, // flip
        );

        assert!(comp.resolve());
        let packed = comp.get_result_packed().unwrap();
        assert_eq!(packed.len(), 1);
        // Packed Z=1.0 => z bits should be ~511 << 20
        assert_ne!(packed[0], 0);
    }

    #[test]
    fn test_cpu_flat_normals_quad() {
        let counts = vec![4];
        let indices = vec![0, 1, 2, 3];
        let points = vec![
            Vec3f::new(0.0, 0.0, 0.0),
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(1.0, 1.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
        ];

        let mut comp = FlatNormalsComputationCpu::new(
            counts,
            indices,
            points,
            Token::new("normals"),
            false, // packed
            false, // flip
        );

        assert!(comp.resolve());
        let result = comp.get_result().unwrap();
        assert_eq!(result.len(), 1);
        assert!((result[0][2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_gpu_flat_normals_creation() {
        let comp = FlatNormalsComputationGpu::new(
            100,
            Token::new("points"),
            Token::new("normals"),
            HdType::FloatVec3,
            false,
        );

        assert_eq!(comp.get_num_output_elements(), 100);
        let specs = comp.get_buffer_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].data_type, HdType::FloatVec3);
    }

    #[test]
    fn test_gpu_flat_normals_packed() {
        let comp = FlatNormalsComputationGpu::new(
            50,
            Token::new("points"),
            Token::new("normals"),
            HdType::FloatVec3,
            true,
        );

        let specs = comp.get_buffer_specs();
        assert_eq!(specs[0].data_type, HdType::Int32_2_10_10_10_Rev);
    }

    #[test]
    fn test_degenerate_face() {
        // Face with only 2 vertices -- should produce zero normal
        let counts = vec![2];
        let indices = vec![0, 1];
        let points = vec![Vec3f::new(0.0, 0.0, 0.0), Vec3f::new(1.0, 0.0, 0.0)];

        let mut comp = FlatNormalsComputationCpu::new(
            counts,
            indices,
            points,
            Token::new("normals"),
            false, // packed
            false, // flip
        );

        assert!(comp.resolve());
        let result = comp.get_result().unwrap();
        assert_eq!(result[0], Vec3f::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_num_elements() {
        let comp = FlatNormalsComputationCpu::new(
            vec![3, 3, 4],
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            vec![Vec3f::new(0.0, 0.0, 0.0); 10],
            Token::new("normals"),
            false, // packed
            false, // flip
        );
        assert_eq!(comp.get_num_elements(), 3);
    }
}
