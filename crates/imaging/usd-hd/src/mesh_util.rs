
//! HdMeshUtil - Triangulation and quadrangulation utilities.
//!
//! Corresponds to pxr/imaging/hd/meshUtil.h.
//! Produces triangle/quad indices from mesh topology.
//! Also provides HdMeshEdgeIndexTable for wireframe/edge rendering.

use super::mesh_topology::HdMeshTopology;
use super::types::HdType;
use crate::tokens;
use usd_gf::{Vec2i, Vec3i};
use usd_sdf::Path;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// HdQuadInfo
// ---------------------------------------------------------------------------

/// Helper for quadrangulation computation.
///
/// Corresponds to C++ `HdQuadInfo`.
#[derive(Debug, Default, Clone)]
pub struct HdQuadInfo {
    /// Offset into points array for additional quad center/edge points.
    pub points_offset: i32,

    /// Number of additional points for non-quad faces.
    pub num_additional_points: i32,

    /// Max vertices in a face.
    pub max_num_vert: i32,

    /// Vertex counts for non-quad faces.
    pub num_verts: Vec<i32>,

    /// Vertex indices for non-quad faces.
    pub verts: Vec<i32>,
}

impl HdQuadInfo {
    /// Returns true if mesh is all quads (no additional points needed).
    pub fn is_all_quads(&self) -> bool {
        self.num_additional_points == 0
    }
}

// ---------------------------------------------------------------------------
// HdMeshComputationResult
// ---------------------------------------------------------------------------

/// Result of mesh computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdMeshComputationResult {
    /// Computation failed.
    Error,
    /// Success, result produced.
    Success,
    /// Success but unchanged (same as input).
    Unchanged,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Count triangles from face vertex counts, skipping holes and degenerate faces.
/// Returns (numTris, allTriangles).
fn count_triangles(face_vertex_counts: &[i32], hole_indices: &[i32]) -> (usize, bool) {
    // Binary search in the loop below requires sorted hole_indices.
    debug_assert!(
        hole_indices.windows(2).all(|w| w[0] <= w[1]),
        "hole_indices must be sorted"
    );
    let num_faces = face_vertex_counts.len();
    let num_hole_faces = hole_indices.len();
    let mut num_tris = 0usize;
    let mut all_triangles = true;
    let mut hole_index = 0usize;

    for i in 0..num_faces {
        let nv = face_vertex_counts[i];
        if nv < 3 {
            all_triangles = false;
        } else if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
            hole_index += 1;
            all_triangles = false;
        } else {
            num_tris += (nv - 2) as usize;
            all_triangles = all_triangles && (nv == 3);
        }
    }
    (num_tris, all_triangles)
}

/// Fan triangulate face-varying data for a single triangle within a face.
/// Returns false on overrun.
fn fan_triangulate_fv<T: Copy + Default>(
    dst: &mut [T; 3],
    source: &[T],
    offset: usize,
    index: usize,
    size: usize,
    flip: bool,
) -> bool {
    if offset + index + 2 >= size {
        *dst = [T::default(); 3];
        return false;
    }
    if flip {
        dst[0] = source[offset];
        dst[1] = source[offset + index + 2];
        dst[2] = source[offset + index + 1];
    } else {
        dst[0] = source[offset];
        dst[1] = source[offset + index + 1];
        dst[2] = source[offset + index + 2];
    }
    true
}

/// Generic face-varying triangulation. Matches C++ _TriangulateFaceVarying.
fn triangulate_face_varying<T: Copy + Default>(
    face_vertex_counts: &[i32],
    hole_indices: &[i32],
    flip: bool,
    source: &[T],
    num_elements: usize,
) -> Option<Vec<T>> {
    let (num_tris, all_triangles) = count_triangles(face_vertex_counts, hole_indices);

    // Already triangulated and not flipped
    if all_triangles && !flip {
        return None; // Unchanged
    }

    let num_faces = face_vertex_counts.len();
    let num_hole_faces = hole_indices.len();
    let mut results = vec![T::default(); num_tris * 3];
    let mut v = 0usize;
    let mut hole_index = 0usize;
    let mut dst_index = 0usize;

    for i in 0..num_faces {
        let nv = face_vertex_counts[i] as usize;
        if nv < 3 {
            // skip degenerate
        } else if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
            hole_index += 1;
        } else {
            for j in 0..nv - 2 {
                let mut tri = [T::default(); 3];
                fan_triangulate_fv(&mut tri, source, v, j, num_elements, flip);

                // Rotate first/last triangle for edge flag consistency when flipped
                if nv > 3 && flip {
                    if j == 0 {
                        // rotate: [0,1,2] -> [1,2,0]
                        let tmp = tri[0];
                        tri[0] = tri[1];
                        tri[1] = tri[2];
                        tri[2] = tmp;
                    } else if j == nv - 3 {
                        // rotate: [0,1,2] -> [2,0,1]
                        let tmp = tri[2];
                        tri[2] = tri[1];
                        tri[1] = tri[0];
                        tri[0] = tmp;
                    }
                }

                results[dst_index] = tri[0];
                results[dst_index + 1] = tri[1];
                results[dst_index + 2] = tri[2];
                dst_index += 3;
            }
        }
        v += nv;
    }

    Some(results)
}

/// Trait for types that support arithmetic needed for quadrangulation.
trait PrimvarArith: Copy + Default {
    fn add(self, other: Self) -> Self;
    fn div_scalar(self, s: f64) -> Self;
    fn mul_scalar(self, s: f64) -> Self;
}

macro_rules! impl_primvar_arith_float {
    ($t:ty) => {
        impl PrimvarArith for $t {
            fn add(self, other: Self) -> Self {
                self + other
            }
            fn div_scalar(self, s: f64) -> Self {
                (self as f64 / s) as $t
            }
            fn mul_scalar(self, s: f64) -> Self {
                (self as f64 * s) as $t
            }
        }
    };
}

impl_primvar_arith_float!(f32);
impl_primvar_arith_float!(f64);

macro_rules! impl_primvar_arith_vec {
    ($t:ty, $n:expr, $new:expr) => {
        impl PrimvarArith for $t {
            fn add(self, other: Self) -> Self {
                let mut r = self;
                for i in 0..$n {
                    r[i] += other[i];
                }
                r
            }
            fn div_scalar(self, s: f64) -> Self {
                let mut r = self;
                for i in 0..$n {
                    r[i] = (r[i] as f64 / s) as _;
                }
                r
            }
            fn mul_scalar(self, s: f64) -> Self {
                let mut r = self;
                for i in 0..$n {
                    r[i] = (r[i] as f64 * s) as _;
                }
                r
            }
        }
    };
}

impl_primvar_arith_vec!([f32; 2], 2, |a: [f32; 2]| a);
impl_primvar_arith_vec!([f32; 3], 3, |a: [f32; 3]| a);
impl_primvar_arith_vec!([f32; 4], 4, |a: [f32; 4]| a);
impl_primvar_arith_vec!([f64; 2], 2, |a: [f64; 2]| a);
impl_primvar_arith_vec!([f64; 3], 3, |a: [f64; 3]| a);
impl_primvar_arith_vec!([f64; 4], 4, |a: [f64; 4]| a);

/// Generic face-varying quadrangulation. Matches C++ _QuadrangulateFaceVarying.
fn quadrangulate_face_varying<T: PrimvarArith>(
    face_vertex_counts: &[i32],
    hole_indices: &[i32],
    flip: bool,
    source: &[T],
    num_elements: usize,
) -> Vec<T> {
    let num_faces = face_vertex_counts.len();
    let num_hole_faces = hole_indices.len();

    // Count output fvar values
    let mut num_fvar_values = 0usize;
    let mut hole_index = 0usize;
    for i in 0..num_faces {
        let nv = face_vertex_counts[i] as usize;
        if nv < 3 {
            // skip degenerate
        } else if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
            hole_index += 1;
        } else if nv == 4 {
            num_fvar_values += 4;
        } else {
            num_fvar_values += 4 * nv;
        }
    }

    let mut results = vec![T::default(); num_fvar_values];
    hole_index = 0;
    let mut dst = 0usize;

    // P1-10: precompute prefix-sum of face vertex offsets to avoid O(n^2) inner sum.
    let mut face_vertex_offsets = Vec::with_capacity(num_faces);
    {
        let mut offset = 0usize;
        for &count in face_vertex_counts {
            face_vertex_offsets.push(offset);
            offset += count as usize;
        }
    }

    for i in 0..num_faces {
        let nv = face_vertex_counts[i] as usize;
        let v = face_vertex_offsets[i];

        if nv < 3 {
            continue;
        }
        if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
            hole_index += 1;
            continue;
        }

        if nv == 4 {
            if v + nv > num_elements {
                for _ in 0..4 {
                    results[dst] = T::default();
                    dst += 1;
                }
            } else {
                results[dst] = source[v];
                dst += 1;
                if flip {
                    results[dst] = source[v + 3];
                    dst += 1;
                    results[dst] = source[v + 2];
                    dst += 1;
                    results[dst] = source[v + 1];
                    dst += 1;
                } else {
                    results[dst] = source[v + 1];
                    dst += 1;
                    results[dst] = source[v + 2];
                    dst += 1;
                    results[dst] = source[v + 3];
                    dst += 1;
                }
            }
        } else {
            // Quadrangulate n-gon face-varying
            if v + nv > num_elements {
                for _ in 0..nv * 4 {
                    results[dst] = T::default();
                    dst += 1;
                }
                continue;
            }

            // Compute center
            let mut center = T::default();
            for j in 0..nv {
                center = center.add(source[v + j]);
            }
            center = center.div_scalar(nv as f64);

            // First vertex's edges
            let e0 = source[v].add(source[v + 1]).mul_scalar(0.5);
            let e1 = source[v].add(source[v + (nv - 1) % nv]).mul_scalar(0.5);

            results[dst] = source[v];
            dst += 1;
            if flip {
                results[dst] = e1;
                dst += 1;
                results[dst] = center;
                dst += 1;
                results[dst] = e0;
                dst += 1;

                for j in (1..nv).rev() {
                    let ej0 = source[v + j].add(source[v + (j + 1) % nv]).mul_scalar(0.5);
                    let ej1 = source[v + j]
                        .add(source[v + (j + nv - 1) % nv])
                        .mul_scalar(0.5);
                    results[dst] = source[v + j];
                    dst += 1;
                    results[dst] = ej1;
                    dst += 1;
                    results[dst] = center;
                    dst += 1;
                    results[dst] = ej0;
                    dst += 1;
                }
            } else {
                results[dst] = e0;
                dst += 1;
                results[dst] = center;
                dst += 1;
                results[dst] = e1;
                dst += 1;

                for j in 1..nv {
                    let ej0 = source[v + j].add(source[v + (j + 1) % nv]).mul_scalar(0.5);
                    let ej1 = source[v + j]
                        .add(source[v + (j + nv - 1) % nv])
                        .mul_scalar(0.5);
                    results[dst] = source[v + j];
                    dst += 1;
                    results[dst] = ej0;
                    dst += 1;
                    results[dst] = center;
                    dst += 1;
                    results[dst] = ej1;
                    dst += 1;
                }
            }
        }
    }

    results
}

// ---------------------------------------------------------------------------
// HdMeshUtil
// ---------------------------------------------------------------------------

/// Mesh triangulation and quadrangulation utilities.
///
/// Corresponds to C++ `HdMeshUtil`.
pub struct HdMeshUtil<'a> {
    topology: &'a HdMeshTopology,
    #[allow(dead_code)] // C++ uses for error/warning messages, not yet wired
    id: Path,
}

/// Primitive param encoding helpers.
impl HdMeshUtil<'_> {
    /// Encode face index and edge flag into primitive param.
    pub fn encode_coarse_face_param(face_index: i32, edge_flag: i32) -> i32 {
        (face_index << 2) | (edge_flag & 3)
    }

    /// Decode face index from coarse face param.
    pub fn decode_face_index_from_coarse_face_param(coarse_face_param: i32) -> i32 {
        coarse_face_param >> 2
    }

    /// Decode edge flag from coarse face param.
    pub fn decode_edge_flag_from_coarse_face_param(coarse_face_param: i32) -> i32 {
        coarse_face_param & 3
    }
}

impl<'a> HdMeshUtil<'a> {
    /// Create mesh util for topology.
    pub fn new(topology: &'a HdMeshTopology, id: Path) -> Self {
        Self { topology, id }
    }

    // -----------------------------------------------------------------------
    // Triangulation
    // -----------------------------------------------------------------------

    /// Compute triangle indices via fan triangulation.
    /// Matches C++ HdMeshUtil::ComputeTriangleIndices.
    pub fn compute_triangle_indices(
        &self,
        indices: &mut Vec<Vec3i>,
        primitive_params: &mut Vec<i32>,
        mut edge_indices: Option<&mut Vec<i32>>,
    ) {
        let face_vertex_counts = self.topology.get_face_vertex_counts();
        let face_vertex_indices = self.topology.get_face_vertex_indices();
        let hole_indices = self.topology.get_hole_indices();
        let flip = *self.topology.get_orientation() != *tokens::RIGHT_HANDED;

        let (num_tris, all_triangles) = count_triangles(face_vertex_counts, hole_indices);

        indices.resize(num_tris, Vec3i::new(0, 0, 0));
        primitive_params.resize(num_tris, 0);
        if let Some(ref mut ei) = edge_indices {
            ei.resize(num_tris, 0);
        }

        let verts = face_vertex_indices;
        let num_vert_indices = verts.len();

        // Fast path: already all triangles and not flipped
        if all_triangles && !flip {
            for i in 0..num_tris {
                let base = i * 3;
                indices[i] = Vec3i::new(verts[base], verts[base + 1], verts[base + 2]);
                primitive_params[i] = Self::encode_coarse_face_param(i as i32, 0);
                if let Some(ref mut ei) = edge_indices {
                    ei[i] = (3 * i) as i32;
                }
            }
            return;
        }

        let num_faces = face_vertex_counts.len();
        let num_hole_faces = hole_indices.len();
        let mut tv = 0usize;
        let mut v = 0usize;
        let mut ev = 0usize;
        let mut hole_index = 0usize;

        for i in 0..num_faces {
            let nv = face_vertex_counts[i] as usize;
            if nv < 3 {
                // skip degenerate
            } else if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
                hole_index += 1;
            } else {
                let mut edge_flag;
                let edge_index = ev;
                for j in 0..nv - 2 {
                    // Fan triangulate
                    let (a, b, c) = if v + j + 2 < num_vert_indices {
                        if flip {
                            (verts[v], verts[v + j + 2], verts[v + j + 1])
                        } else {
                            (verts[v], verts[v + j + 1], verts[v + j + 2])
                        }
                    } else {
                        (0, 0, 0)
                    };

                    let mut idx = Vec3i::new(a, b, c);

                    if nv > 3 {
                        if j == 0 {
                            if flip {
                                // Rotate 012 -> 210
                                idx = Vec3i::new(idx[1], idx[2], idx[0]);
                            }
                            edge_flag = 1;
                        } else if j == nv - 3 {
                            if flip {
                                // Rotate 012 -> 201
                                idx = Vec3i::new(idx[2], idx[0], idx[1]);
                            }
                            edge_flag = 2;
                        } else {
                            edge_flag = 3;
                        }
                    } else {
                        edge_flag = 0;
                    }

                    indices[tv] = idx;
                    primitive_params[tv] = Self::encode_coarse_face_param(i as i32, edge_flag);
                    if let Some(ref mut ei) = edge_indices {
                        ei[tv] = if nv > 3 {
                            (edge_index + if j == 0 { 0 } else { j }) as i32
                        } else {
                            edge_index as i32
                        };
                    }
                    tv += 1;
                }
            }
            v += nv;
            ev += nv;
        }
    }

    /// Triangulate face-varying primvar. Matches C++ ComputeTriangulatedFaceVaryingPrimvar.
    ///
    /// source_data is typed slice (f32, [f32;2], [f32;3], [f32;4] or f64 variants).
    /// Returns Success with triangulated data, Unchanged if already triangulated, or Error.
    pub fn compute_triangulated_face_varying_primvar(
        &self,
        source: &[u8],
        num_elements: usize,
        data_type: HdType,
        triangulated: &mut Value,
    ) -> HdMeshComputationResult {
        let fvc = self.topology.get_face_vertex_counts();
        // Skip holes only when not refined
        let hole_indices = if self.topology.get_refine_level() > 0 {
            &[] as &[i32]
        } else {
            self.topology.get_hole_indices()
        };
        let flip = *self.topology.get_orientation() != *tokens::RIGHT_HANDED;

        macro_rules! do_tri {
            ($t:ty) => {{
                // P0-4: Use read_unaligned to safely read T values from a byte buffer.
                // This avoids undefined behavior from misaligned raw pointer casts.
                let elem_size = std::mem::size_of::<$t>();
                if source.len() < num_elements * elem_size {
                    return HdMeshComputationResult::Error;
                }
                let src: Vec<$t> =
                    (0..num_elements)
                        .map(|i| {
                            // SAFETY: bounds checked above; read_unaligned handles any alignment.
                            #[allow(unsafe_code)]
                            unsafe {
                                std::ptr::read_unaligned(
                                    source.as_ptr().add(i * elem_size) as *const $t
                                )
                            }
                        })
                        .collect();
                match triangulate_face_varying(fvc, hole_indices, flip, &src, num_elements) {
                    None => return HdMeshComputationResult::Unchanged,
                    Some(result) => {
                        *triangulated = Value::from_no_hash(result);
                        return HdMeshComputationResult::Success;
                    }
                }
            }};
        }

        match data_type {
            HdType::Float => do_tri!(f32),
            HdType::FloatVec2 => do_tri!([f32; 2]),
            HdType::FloatVec3 => do_tri!([f32; 3]),
            HdType::FloatVec4 => do_tri!([f32; 4]),
            HdType::Double => do_tri!(f64),
            HdType::DoubleVec2 => do_tri!([f64; 2]),
            HdType::DoubleVec3 => do_tri!([f64; 3]),
            HdType::DoubleVec4 => do_tri!([f64; 4]),
            _ => HdMeshComputationResult::Error,
        }
    }

    // -----------------------------------------------------------------------
    // Quadrangulation
    // -----------------------------------------------------------------------

    /// Compute number of quads. Matches C++ _ComputeNumQuads.
    fn compute_num_quads(face_vertex_counts: &[i32], hole_indices: &[i32]) -> (usize, bool) {
        let num_faces = face_vertex_counts.len();
        let num_hole_faces = hole_indices.len();
        let mut num_quads = 0usize;
        let mut hole_index = 0usize;
        let mut invalid = false;

        for i in 0..num_faces {
            let nv = face_vertex_counts[i];
            if nv < 3 {
                invalid = true;
            } else if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
                hole_index += 1;
            } else {
                num_quads += if nv == 4 { 1 } else { nv as usize };
            }
        }
        (num_quads, invalid)
    }

    /// Generate quad info. Matches C++ HdMeshUtil::ComputeQuadInfo.
    pub fn compute_quad_info(&self, quad_info: &mut HdQuadInfo) {
        let face_vertex_counts = self.topology.get_face_vertex_counts();
        let face_vertex_indices = self.topology.get_face_vertex_indices();
        let hole_indices = self.topology.get_hole_indices();
        let num_points = self.topology.get_num_points();
        let num_faces = face_vertex_counts.len();
        let num_vert_indices = face_vertex_indices.len();
        let num_hole_faces = hole_indices.len();

        quad_info.points_offset = num_points;
        quad_info.num_verts.clear();
        quad_info.verts.clear();

        let mut vert_index = 0usize;
        let mut num_additional_points = 0i32;
        let mut max_num_vert = 0i32;
        let mut hole_index = 0usize;

        for i in 0..num_faces {
            let nv = face_vertex_counts[i] as usize;

            if nv < 3 {
                vert_index += nv;
                continue;
            }
            if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
                vert_index += nv;
                hole_index += 1;
                continue;
            }
            if nv == 4 {
                vert_index += nv;
                continue;
            }

            // Non-quad face
            quad_info.num_verts.push(nv as i32);
            for _j in 0..nv {
                let index = if vert_index < num_vert_indices {
                    let idx = face_vertex_indices[vert_index];
                    vert_index += 1;
                    idx
                } else {
                    vert_index += 1;
                    0
                };
                quad_info.verts.push(index);
            }
            num_additional_points += (nv as i32) + 1; // nv edges + 1 center
            max_num_vert = max_num_vert.max(nv as i32);
        }

        quad_info.num_additional_points = num_additional_points;
        quad_info.max_num_vert = max_num_vert;
    }

    /// Compute quad indices. Matches C++ HdMeshUtil::ComputeQuadIndices.
    pub fn compute_quad_indices(
        &self,
        indices: &mut Vec<i32>,
        primitive_params: &mut Vec<i32>,
        edge_indices: Option<&mut Vec<Vec2i>>,
    ) {
        self.compute_quad_indices_inner(indices, primitive_params, edge_indices, false);
    }

    /// Compute tri-quad indices. Matches C++ HdMeshUtil::ComputeTriQuadIndices.
    pub fn compute_tri_quad_indices(
        &self,
        indices: &mut Vec<i32>,
        primitive_params: &mut Vec<i32>,
        edge_indices: Option<&mut Vec<Vec2i>>,
    ) {
        self.compute_quad_indices_inner(indices, primitive_params, edge_indices, true);
    }

    /// Internal quad index computation. Matches C++ HdMeshUtil::_ComputeQuadIndices.
    fn compute_quad_indices_inner(
        &self,
        indices: &mut Vec<i32>,
        primitive_params: &mut Vec<i32>,
        mut edge_indices: Option<&mut Vec<Vec2i>>,
        triangulate: bool,
    ) {
        let face_vertex_counts = self.topology.get_face_vertex_counts();
        let face_vertex_indices = self.topology.get_face_vertex_indices();
        let hole_indices = self.topology.get_hole_indices();
        let num_faces = face_vertex_counts.len();
        let num_vert_indices = face_vertex_indices.len();
        let num_hole_faces = hole_indices.len();
        let num_points = self.topology.get_num_points();
        let flip = *self.topology.get_orientation() != *tokens::RIGHT_HANDED;

        let (num_quads, _invalid) = Self::compute_num_quads(face_vertex_counts, hole_indices);

        let indices_per_quad = if triangulate { 6 } else { 4 };
        indices.resize(num_quads * indices_per_quad, 0);
        primitive_params.resize(num_quads, 0);
        if let Some(ref mut ei) = edge_indices {
            ei.resize(num_quads, Vec2i::new(0, 0));
        }

        let mut hole_index = 0usize;
        // Additional vertices start after original points
        let mut vert_index = num_points as usize;
        let mut qv = 0usize; // output quad index
        let mut v = 0usize; // input vertex offset
        let mut ev = 0usize; // edge visit counter

        for i in 0..num_faces {
            let nv = face_vertex_counts[i] as usize;
            if nv < 3 {
                v += nv;
                ev += nv;
                continue;
            }
            if hole_index < num_hole_faces && hole_indices[hole_index] == i as i32 {
                hole_index += 1;
                v += nv;
                ev += nv;
                continue;
            }

            let edge_index = ev;

            if v + nv > num_vert_indices {
                // Invalid topology, emit zeros
                if nv == 4 {
                    emit_quad_face(
                        &mut indices[qv * indices_per_quad..],
                        [0, 0, 0, 0],
                        triangulate,
                    );
                    qv += 1;
                } else {
                    for _ in 0..nv {
                        emit_quad_face(
                            &mut indices[qv * indices_per_quad..],
                            [0, 0, 0, 0],
                            triangulate,
                        );
                        qv += 1;
                    }
                }
                v += nv;
                ev += nv;
                continue;
            }

            if nv == 4 {
                let quad = if flip {
                    [
                        face_vertex_indices[v],
                        face_vertex_indices[v + 3],
                        face_vertex_indices[v + 2],
                        face_vertex_indices[v + 1],
                    ]
                } else {
                    [
                        face_vertex_indices[v],
                        face_vertex_indices[v + 1],
                        face_vertex_indices[v + 2],
                        face_vertex_indices[v + 3],
                    ]
                };
                emit_quad_face(&mut indices[qv * indices_per_quad..], quad, triangulate);
                primitive_params[qv] = Self::encode_coarse_face_param(i as i32, 0);
                if let Some(ref mut ei) = edge_indices {
                    ei[qv] = Vec2i::new(edge_index as i32, (edge_index + 3) as i32);
                }
                qv += 1;
            } else {
                // Quadrangulate non-quad face
                for j in 0..nv {
                    let quad = if flip {
                        [
                            face_vertex_indices[v + j],
                            (vert_index + (j + nv - 1) % nv) as i32, // edge prev
                            (vert_index + nv) as i32,                // center
                            (vert_index + j) as i32,                 // edge next
                        ]
                    } else {
                        [
                            face_vertex_indices[v + j],
                            (vert_index + j) as i32,  // edge next
                            (vert_index + nv) as i32, // center
                            (vert_index + (j + nv - 1) % nv) as i32, // edge prev
                        ]
                    };
                    emit_quad_face(&mut indices[qv * indices_per_quad..], quad, triangulate);

                    let edge_flag = if j == 0 {
                        1
                    } else if j == nv - 1 {
                        2
                    } else {
                        3
                    };
                    primitive_params[qv] =
                        Self::encode_coarse_face_param(i as i32, edge_flag as i32);

                    if let Some(ref mut ei) = edge_indices {
                        if flip {
                            ei[qv] = Vec2i::new(
                                (edge_index + (j + nv - 1) % nv) as i32,
                                (edge_index + j) as i32,
                            );
                        } else {
                            ei[qv] = Vec2i::new(
                                (edge_index + j) as i32,
                                (edge_index + (j + nv - 1) % nv) as i32,
                            );
                        }
                    }

                    qv += 1;
                }
                vert_index += nv + 1;
            }
            v += nv;
            ev += nv;
        }
    }

    /// Quadrangulate vertex primvar. Matches C++ ComputeQuadrangulatedPrimvar.
    pub fn compute_quadrangulated_primvar(
        &self,
        qi: &HdQuadInfo,
        source: &[u8],
        num_elements: usize,
        data_type: HdType,
        quadrangulated: &mut Value,
    ) -> bool {
        macro_rules! do_quad {
            ($t:ty) => {{
                // SAFETY: Reinterpreting &[u8] as &[T] for primvar data
                #[allow(unsafe_code)]
                let src = unsafe {
                    std::slice::from_raw_parts(source.as_ptr() as *const $t, num_elements)
                };
                let total = (qi.points_offset + qi.num_additional_points) as usize;
                let mut results = vec![<$t>::default(); total];

                // Copy original points
                let copy_count = num_elements.min(qi.points_offset as usize);
                results[..copy_count].copy_from_slice(&src[..copy_count]);

                // Compute additional quad points
                let mut idx = 0usize;
                let mut dst = qi.points_offset as usize;
                for &nv in &qi.num_verts {
                    let nv = nv as usize;
                    let mut center = <$t>::default();
                    for k in 0..nv {
                        let i0 = qi.verts[idx + k] as usize;
                        let i1 = qi.verts[idx + (k + 1) % nv] as usize;
                        // Midpoint
                        let edge = results[i0].add(results[i1]).mul_scalar(0.5);
                        results[dst] = edge;
                        dst += 1;
                        center = center.add(results[i0]);
                    }
                    center = center.div_scalar(nv as f64);
                    results[dst] = center;
                    dst += 1;
                    idx += nv;
                }

                *quadrangulated = Value::from_no_hash(results);
                return true;
            }};
        }

        match data_type {
            HdType::Float => do_quad!(f32),
            HdType::FloatVec2 => do_quad!([f32; 2]),
            HdType::FloatVec3 => do_quad!([f32; 3]),
            HdType::FloatVec4 => do_quad!([f32; 4]),
            HdType::Double => do_quad!(f64),
            HdType::DoubleVec2 => do_quad!([f64; 2]),
            HdType::DoubleVec3 => do_quad!([f64; 3]),
            HdType::DoubleVec4 => do_quad!([f64; 4]),
            _ => false,
        }
    }

    /// Quadrangulate face-varying primvar. Matches C++ ComputeQuadrangulatedFaceVaryingPrimvar.
    pub fn compute_quadrangulated_face_varying_primvar(
        &self,
        source: &[u8],
        num_elements: usize,
        data_type: HdType,
        quadrangulated: &mut Value,
    ) -> bool {
        let fvc = self.topology.get_face_vertex_counts();
        let hole_indices = if self.topology.get_refine_level() > 0 {
            &[] as &[i32]
        } else {
            self.topology.get_hole_indices()
        };
        let flip = *self.topology.get_orientation() != *tokens::RIGHT_HANDED;

        macro_rules! do_quad_fv {
            ($t:ty) => {{
                // SAFETY: Reinterpreting &[u8] as &[T] for primvar data
                #[allow(unsafe_code)]
                let src = unsafe {
                    std::slice::from_raw_parts(source.as_ptr() as *const $t, num_elements)
                };
                let result = quadrangulate_face_varying(fvc, hole_indices, flip, src, num_elements);
                *quadrangulated = Value::from_no_hash(result);
                return true;
            }};
        }

        match data_type {
            HdType::Float => do_quad_fv!(f32),
            HdType::FloatVec2 => do_quad_fv!([f32; 2]),
            HdType::FloatVec3 => do_quad_fv!([f32; 3]),
            HdType::FloatVec4 => do_quad_fv!([f32; 4]),
            HdType::Double => do_quad_fv!(f64),
            HdType::DoubleVec2 => do_quad_fv!([f64; 2]),
            HdType::DoubleVec3 => do_quad_fv!([f64; 3]),
            HdType::DoubleVec4 => do_quad_fv!([f64; 4]),
            _ => false,
        }
    }

    // -----------------------------------------------------------------------
    // Edge enumeration
    // -----------------------------------------------------------------------

    /// Enumerate edges. Matches C++ HdMeshUtil::EnumerateEdges.
    ///
    /// Produces edge vertex pairs for each face edge. Optionally records first
    /// edge index per face.
    pub fn enumerate_edges(
        &self,
        edge_vertices_out: &mut Vec<Vec2i>,
        mut first_edge_index_for_faces_out: Option<&mut Vec<i32>>,
    ) {
        let face_vertex_counts = self.topology.get_face_vertex_counts();
        let face_vertex_indices = self.topology.get_face_vertex_indices();
        let num_faces = face_vertex_counts.len();
        let flip = *self.topology.get_orientation() != *tokens::RIGHT_HANDED;

        // Validate that face_vertex_counts sum does not exceed face_vertex_indices length.
        // Accessing beyond fvi would be OOB; return empty on corrupt topology.
        let total_verts: usize = face_vertex_counts
            .iter()
            .map(|&nv| nv.max(0) as usize)
            .sum();
        if total_verts > face_vertex_indices.len() {
            return;
        }

        // Count total edges
        let num_edges: usize = face_vertex_counts.iter().map(|&nv| nv as usize).sum();
        edge_vertices_out.resize(num_edges, Vec2i::new(0, 0));

        if let Some(ref mut first) = first_edge_index_for_faces_out {
            first.resize(num_faces, 0);
        }

        let mut v = 0usize;
        let mut ev = 0usize;

        for i in 0..num_faces {
            let nv = face_vertex_counts[i] as usize;
            if let Some(ref mut first) = first_edge_index_for_faces_out {
                first[i] = ev as i32;
            }

            if flip {
                for j in (1..=nv).rev() {
                    let mut v0 = face_vertex_indices[v + j % nv];
                    let mut v1 = face_vertex_indices[v + (j + nv - 1) % nv];
                    if v0 < v1 {
                        std::mem::swap(&mut v0, &mut v1);
                    }
                    edge_vertices_out[ev] = Vec2i::new(v0, v1);
                    ev += 1;
                }
            } else {
                for j in 0..nv {
                    let mut v0 = face_vertex_indices[v + j];
                    let mut v1 = face_vertex_indices[v + (j + 1) % nv];
                    if v0 < v1 {
                        std::mem::swap(&mut v0, &mut v1);
                    }
                    edge_vertices_out[ev] = Vec2i::new(v0, v1);
                    ev += 1;
                }
            }
            v += nv;
        }
    }
}

/// Emit a quad (or tri-quad pair) into the output index buffer.
fn emit_quad_face(out: &mut [i32], quad: [i32; 4], triangulate: bool) {
    if triangulate {
        out[0] = quad[0];
        out[1] = quad[1];
        out[2] = quad[2];
        out[3] = quad[2];
        out[4] = quad[3];
        out[5] = quad[0];
    } else {
        out[0] = quad[0];
        out[1] = quad[1];
        out[2] = quad[2];
        out[3] = quad[3];
    }
}

// ---------------------------------------------------------------------------
// HdMeshEdgeIndexTable
// ---------------------------------------------------------------------------

/// Edge lookup for wireframe/edge rendering.
///
/// Corresponds to C++ `HdMeshEdgeIndexTable`. Provides forward and reverse
/// lookup between edge indices and vertex pairs.
pub struct HdMeshEdgeIndexTable {
    /// Topology reference for face vertex counts
    face_vertex_counts: Vec<i32>,

    /// First edge index for each face
    first_edge_index_for_faces: Vec<i32>,

    /// Edge vertices in enumeration order
    edge_vertices: Vec<Vec2i>,

    /// Sorted edges for binary search (sorted by vertex pair)
    edges_by_index: Vec<Edge>,
}

/// Internal edge representation for sorted lookup.
#[derive(Clone)]
struct Edge {
    /// Sorted vertex pair (v[0] <= v[1])
    verts: Vec2i,
    /// Original edge index
    index: i32,
}

impl Edge {
    fn new(verts: Vec2i, index: i32) -> Self {
        let mut v = verts;
        if v[0] > v[1] {
            let tmp = v[0];
            v[0] = v[1];
            v[1] = tmp;
        }
        Self { verts: v, index }
    }
}

impl HdMeshEdgeIndexTable {
    /// Build edge index table from topology. Matches C++ constructor.
    pub fn new(topology: &HdMeshTopology) -> Self {
        let mesh_util = HdMeshUtil::new(topology, Path::empty());

        let mut edge_vertices = Vec::new();
        let mut first_edge_index_for_faces = Vec::new();
        mesh_util.enumerate_edges(&mut edge_vertices, Some(&mut first_edge_index_for_faces));

        let mut edges_by_index: Vec<Edge> = edge_vertices
            .iter()
            .enumerate()
            .map(|(i, &v)| Edge::new(v, i as i32))
            .collect();

        // Sort by vertex pair for binary search
        edges_by_index.sort_by(|a, b| {
            a.verts[0]
                .cmp(&b.verts[0])
                .then(a.verts[1].cmp(&b.verts[1]))
        });

        Self {
            face_vertex_counts: topology.get_face_vertex_counts().to_vec(),
            first_edge_index_for_faces,
            edge_vertices,
            edges_by_index,
        }
    }

    /// Get vertex pair for a given edge index.
    pub fn get_vertices_for_edge_index(&self, edge_id: i32) -> Option<Vec2i> {
        if edge_id < 0 || edge_id as usize >= self.edge_vertices.len() {
            return None;
        }
        Some(self.edge_vertices[edge_id as usize])
    }

    /// Get unique vertex pairs for a set of edge indices.
    pub fn get_vertices_for_edge_indices(&self, edge_indices: &[i32]) -> Vec<Vec2i> {
        use std::collections::HashSet;
        let mut result = HashSet::new();
        for &edge_id in edge_indices {
            if let Some(v) = self.get_vertices_for_edge_index(edge_id) {
                result.insert((v[0], v[1]));
            }
        }
        result.into_iter().map(|(a, b)| Vec2i::new(a, b)).collect()
    }

    /// Get all edge indices for a given vertex pair (reverse lookup).
    pub fn get_edge_indices(&self, edge_vertices: Vec2i) -> Vec<i32> {
        let target = Edge::new(edge_vertices, -1);

        // Find range via binary search
        let start = self.edges_by_index.partition_point(|e| {
            e.verts[0] < target.verts[0]
                || (e.verts[0] == target.verts[0] && e.verts[1] < target.verts[1])
        });
        let end = self.edges_by_index.partition_point(|e| {
            e.verts[0] < target.verts[0]
                || (e.verts[0] == target.verts[0] && e.verts[1] <= target.verts[1])
        });

        self.edges_by_index[start..end]
            .iter()
            .map(|e| e.index)
            .collect()
    }

    /// Collect all edge indices for the given face indices.
    /// Matches C++ HdMeshEdgeIndexTable::CollectFaceEdgeIndices.
    pub fn collect_face_edge_indices(&self, face_indices: &[i32]) -> Vec<i32> {
        let num_mesh_faces = self.face_vertex_counts.len();
        let mut result = Vec::new();

        for &face in face_indices {
            if face < 0 || face as usize >= num_mesh_faces {
                continue;
            }
            let face_idx = face as usize;
            let first_edge = self.first_edge_index_for_faces[face_idx];
            let num_edges = self.face_vertex_counts[face_idx];

            for e in 0..num_edges {
                let ev_idx = (first_edge + e) as usize;
                if ev_idx < self.edge_vertices.len() {
                    let edge_verts = self.edge_vertices[ev_idx];
                    let edge_ids = self.get_edge_indices(edge_verts);
                    result.extend_from_slice(&edge_ids);
                }
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// HdMeshTriQuadBuilder
// ---------------------------------------------------------------------------

/// Helper for emitting quad/tri-quad index buffers.
/// Matches C++ HdMeshTriQuadBuilder.
pub struct HdMeshTriQuadBuilder {
    /// Whether to split quads into triangle pairs.
    pub triangulate: bool,
}

impl HdMeshTriQuadBuilder {
    /// Number of indices per quad (non-triangulated).
    pub const NUM_INDICES_PER_QUAD: usize = 4;
    /// Number of indices per tri-quad (triangulated).
    pub const NUM_INDICES_PER_TRI_QUAD: usize = 6;

    /// Create builder; if triangulate is true, quads emit as triangle pairs.
    pub fn new(triangulate: bool) -> Self {
        Self { triangulate }
    }

    /// Emit a quad face into the output buffer at the given offset.
    pub fn emit_quad_face(&self, output: &mut [i32], quad: [i32; 4]) {
        emit_quad_face(output, quad, self.triangulate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_px_osd::{MeshTopology, tokens as osd_tokens};

    fn make_quad_topology() -> HdMeshTopology {
        let osd = MeshTopology::new(
            osd_tokens::CATMULL_CLARK.clone(),
            osd_tokens::RIGHT_HANDED.clone(),
            vec![4, 4],
            vec![0, 1, 4, 3, 1, 2, 5, 4],
        );
        HdMeshTopology::new(osd, 0)
    }

    fn make_tri_topology() -> HdMeshTopology {
        let osd = MeshTopology::new(
            osd_tokens::CATMULL_CLARK.clone(),
            osd_tokens::RIGHT_HANDED.clone(),
            vec![3, 3],
            vec![0, 1, 2, 0, 2, 3],
        );
        HdMeshTopology::new(osd, 0)
    }

    fn make_ngon_topology() -> HdMeshTopology {
        // Pentagon + quad
        let osd = MeshTopology::new(
            osd_tokens::CATMULL_CLARK.clone(),
            osd_tokens::RIGHT_HANDED.clone(),
            vec![5, 4],
            vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
        );
        HdMeshTopology::new(osd, 0)
    }

    #[test]
    fn test_triangle_indices_quads() {
        let topo = make_quad_topology();
        let util = HdMeshUtil::new(&topo, Path::empty());
        let mut indices = Vec::new();
        let mut params = Vec::new();
        util.compute_triangle_indices(&mut indices, &mut params, None);
        // 2 quads -> 4 triangles
        assert_eq!(indices.len(), 4);
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn test_triangle_indices_tris() {
        let topo = make_tri_topology();
        let util = HdMeshUtil::new(&topo, Path::empty());
        let mut indices = Vec::new();
        let mut params = Vec::new();
        util.compute_triangle_indices(&mut indices, &mut params, None);
        assert_eq!(indices.len(), 2);
        // Fast path: face index encoded directly
        assert_eq!(
            HdMeshUtil::decode_face_index_from_coarse_face_param(params[0]),
            0
        );
        assert_eq!(
            HdMeshUtil::decode_face_index_from_coarse_face_param(params[1]),
            1
        );
    }

    #[test]
    fn test_encode_decode_face_param() {
        for face in 0..100 {
            for flag in 0..4 {
                let param = HdMeshUtil::encode_coarse_face_param(face, flag);
                assert_eq!(
                    HdMeshUtil::decode_face_index_from_coarse_face_param(param),
                    face
                );
                assert_eq!(
                    HdMeshUtil::decode_edge_flag_from_coarse_face_param(param),
                    flag
                );
            }
        }
    }

    #[test]
    fn test_quad_info() {
        let topo = make_ngon_topology();
        let util = HdMeshUtil::new(&topo, Path::empty());
        let mut qi = HdQuadInfo::default();
        util.compute_quad_info(&mut qi);
        // Pentagon contributes 5+1=6 additional points
        assert_eq!(qi.num_additional_points, 6);
        assert_eq!(qi.num_verts.len(), 1);
        assert_eq!(qi.num_verts[0], 5);
    }

    #[test]
    fn test_quad_indices() {
        let topo = make_quad_topology();
        let util = HdMeshUtil::new(&topo, Path::empty());
        let mut indices = Vec::new();
        let mut params = Vec::new();
        util.compute_quad_indices(&mut indices, &mut params, None);
        // 2 quad faces -> 2 quads, 4 indices each = 8
        assert_eq!(indices.len(), 8);
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn test_tri_quad_indices() {
        let topo = make_quad_topology();
        let util = HdMeshUtil::new(&topo, Path::empty());
        let mut indices = Vec::new();
        let mut params = Vec::new();
        util.compute_tri_quad_indices(&mut indices, &mut params, None);
        // 2 quads -> 2 quads * 6 indices (tri-quad) = 12
        assert_eq!(indices.len(), 12);
    }

    #[test]
    fn test_enumerate_edges() {
        let topo = make_quad_topology();
        let util = HdMeshUtil::new(&topo, Path::empty());
        let mut edges = Vec::new();
        let mut first = Vec::new();
        util.enumerate_edges(&mut edges, Some(&mut first));
        // 2 quads * 4 edges each = 8 edges
        assert_eq!(edges.len(), 8);
        assert_eq!(first.len(), 2);
        assert_eq!(first[0], 0);
        assert_eq!(first[1], 4);
    }

    #[test]
    fn test_edge_index_table() {
        let topo = make_quad_topology();
        let table = HdMeshEdgeIndexTable::new(&topo);

        // Should have 8 edge entries
        assert_eq!(table.edge_vertices.len(), 8);

        // Forward lookup
        let v = table.get_vertices_for_edge_index(0);
        assert!(v.is_some());

        // Reverse lookup: edge between vertex 1 and 4 should have 2 entries
        let indices = table.get_edge_indices(Vec2i::new(1, 4));
        assert_eq!(indices.len(), 2, "shared edge should appear twice");
    }

    #[test]
    fn test_collect_face_edge_indices() {
        let topo = make_quad_topology();
        let table = HdMeshEdgeIndexTable::new(&topo);
        let result = table.collect_face_edge_indices(&[0]);
        // Face 0 has 4 edges, some may be shared
        assert!(!result.is_empty());
    }
}
