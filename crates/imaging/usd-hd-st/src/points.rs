
//! HdStPoints - Storm points prim implementation.
//!
//! This file intentionally follows the same three-phase contract as the live
//! mesh path:
//! 1. read scene delegate data,
//! 2. prepare CPU-side draw payload,
//! 3. upload BAR-backed buffers and refresh draw items in place.
//!
//! The old Rust port kept `HdStPoints` as a placeholder object that only
//! cleared dirty flags. That made Alembic/USD files with authored `Points`
//! appear "loaded" while never reaching the actual Storm draw path.

use crate::buffer_resource::{HdStBufferArrayRange, HdStBufferResourceSharedPtr};
use crate::draw_item::{
    DrawPrimitiveKind, HdBufferArrayRangeSharedPtr, HdStDrawItem, HdStDrawItemSharedPtr,
};
use crate::mesh_shader_key::DrawTopology;
use crate::resource_registry::{
    BufferArrayUsageHint, BufferSource, BufferSourceSharedPtr, BufferSpec, HdStResourceRegistry,
    ManagedBarSharedPtr,
};
use crate::wgsl_code_gen::MaterialParams;
use std::sync::Arc;
use usd_gf::Vec3f;
use usd_hd::HdSceneDelegate;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Vertex-domain data for point rendering.
#[derive(Debug, Clone, Default)]
pub struct HdStPointsVertexData {
    /// Packed xyz positions.
    pub positions: Vec<f32>,
    /// Optional per-point widths.
    pub widths: Vec<f32>,
    /// Optional displayColor primvar.
    pub colors: Vec<f32>,
}

impl HdStPointsVertexData {
    /// Return the logical point count.
    pub fn get_point_count(&self) -> usize {
        self.positions.len() / 3
    }

    /// Byte size of packed positions.
    pub fn get_positions_byte_size(&self) -> usize {
        self.positions.len() * std::mem::size_of::<f32>()
    }

    /// Byte size of packed colors.
    pub fn get_colors_byte_size(&self) -> usize {
        self.colors.len() * std::mem::size_of::<f32>()
    }

    /// Byte size of packed widths.
    pub fn get_widths_byte_size(&self) -> usize {
        self.widths.len() * std::mem::size_of::<f32>()
    }
}

/// Storm points rprim.
#[derive(Debug, Clone)]
pub struct HdStPoints {
    path: SdfPath,
    vertex_data: HdStPointsVertexData,
    point_indices: Vec<u32>,
    vertex_buffer: Option<HdStBufferResourceSharedPtr>,
    index_buffer: Option<HdStBufferResourceSharedPtr>,
    vertex_bar: Option<ManagedBarSharedPtr>,
    element_bar: Option<ManagedBarSharedPtr>,
    constant_bar: Option<ManagedBarSharedPtr>,
    draw_items: Vec<HdStDrawItemSharedPtr>,
    visible: bool,
    world_transform: [[f64; 4]; 4],
    material_params: MaterialParams,
    vertex_dirty: bool,
    constant_dirty: bool,
}

impl HdStPoints {
    /// Create a new points prim.
    pub fn new(path: SdfPath) -> Self {
        Self {
            path,
            vertex_data: HdStPointsVertexData::default(),
            point_indices: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
            vertex_bar: None,
            element_bar: None,
            constant_bar: None,
            draw_items: Vec::new(),
            visible: true,
            world_transform: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            material_params: MaterialParams::default(),
            vertex_dirty: true,
            constant_dirty: true,
        }
    }

    /// Sync delegate-owned point data into the retained Storm prim.
    ///
    /// This is the `_ref`-equivalent "delegate read" phase. We keep authored
    /// widths/colors even if the current wgpu point shader does not consume all
    /// of them yet, so follow-up shader work is driven by real retained data
    /// rather than by another placeholder path.
    pub fn sync_from_delegate(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        dirty_bits: &mut HdDirtyBits,
    ) {
        self.visible = delegate.get_visible(&self.path);

        if *dirty_bits != 0 {
            self.world_transform = matrix4d_to_rows(delegate.get_transform(&self.path));
            self.constant_dirty = true;
        }

        if *dirty_bits != 0 {
            let points_value = delegate.get(&self.path, &Token::new("points"));
            let widths_value = delegate.get(&self.path, &Token::new("widths"));
            let (display_color_value, display_color_indices) =
                delegate.get_indexed_primvar(&self.path, &Token::new("displayColor"));

            if let Some(points) = points_value.as_vec_clone::<Vec3f>() {
                self.vertex_data.positions = flatten_vec3f(&points);
                self.vertex_dirty = true;
            } else {
                self.vertex_data.positions.clear();
                self.vertex_dirty = true;
            }

            self.vertex_data.widths = widths_value.as_vec_clone::<f32>().unwrap_or_default();

            if let Some(colors) = display_color_value.as_vec_clone::<Vec3f>() {
                let expanded = expand_indexed_vec3f(&colors, display_color_indices.as_deref());
                self.vertex_data.colors = flatten_vec3f(&expanded);
            } else {
                self.vertex_data.colors.clear();
            }
        }

        *dirty_bits = 0;
    }

    /// CPU-side preparation for point-list rendering.
    pub fn process_cpu(&mut self) {
        if self.vertex_dirty {
            self.point_indices = (0..self.vertex_data.get_point_count() as u32).collect();
        }
    }

    /// Upload points payload to the Storm resource registry.
    pub fn upload_to_registry(&mut self, resource_registry: &HdStResourceRegistry, repr: &Token) {
        self.upload_vertices(resource_registry);
        self.upload_topology(resource_registry);
        self.sync_constant_primvars(resource_registry);
        self.update_draw_items(repr);
        self.vertex_dirty = false;
        self.constant_dirty = false;
    }

    /// Return draw items matching the requested repr.
    pub fn get_draw_items(&self, repr: &Token) -> Vec<HdStDrawItemSharedPtr> {
        self.draw_items
            .iter()
            .filter(|item| item.get_repr() == *repr)
            .cloned()
            .collect()
    }

    fn ensure_repr(&mut self, repr: &Token) {
        if self.draw_items.iter().any(|item| item.get_repr() == *repr) {
            return;
        }

        let item = HdStDrawItem::new(self.path.clone());
        item.set_repr(repr.clone());
        item.set_primitive_topology(DrawTopology::PointList);
        item.set_primitive_kind(DrawPrimitiveKind::Points);
        item.set_material_network_shader(self.material_params.clone());
        self.draw_items.push(Arc::new(item));
    }

    fn upload_vertices(&mut self, resource_registry: &HdStResourceRegistry) {
        if self.vertex_data.positions.is_empty() {
            return;
        }

        let vertex_count = self.vertex_data.get_point_count();
        let float_size = std::mem::size_of::<f32>();
        let vec3_size = 3 * float_size;
        let has_colors = !self.vertex_data.colors.is_empty();
        let color_count = self.vertex_data.colors.len() / 3;

        let mut specs = vec![BufferSpec {
            name: Token::new("points"),
            num_elements: vertex_count,
            element_size: vec3_size,
        }];
        if !self.vertex_data.widths.is_empty() {
            specs.push(BufferSpec {
                name: Token::new("widths"),
                num_elements: self.vertex_data.widths.len(),
                element_size: float_size,
            });
        }
        if has_colors {
            specs.push(BufferSpec {
                name: Token::new("displayColor"),
                num_elements: color_count,
                element_size: vec3_size,
            });
        }

        let usage = BufferArrayUsageHint {
            vertex: true,
            ..Default::default()
        };
        let vertex_bar = resource_registry.update_non_uniform_bar(
            &Token::new("pointsVertex"),
            self.vertex_bar.as_ref(),
            &specs,
            &[],
            usage,
        );

        let mut sources: Vec<BufferSourceSharedPtr> = vec![Arc::new(BufferSource::new(
            Token::new("points"),
            encode_f32_slice_le(&self.vertex_data.positions),
            vertex_count,
            vec3_size,
        ))];
        if !self.vertex_data.widths.is_empty() {
            sources.push(Arc::new(BufferSource::new(
                Token::new("widths"),
                encode_f32_slice_le(&self.vertex_data.widths),
                self.vertex_data.widths.len(),
                float_size,
            )));
        }
        if has_colors {
            sources.push(Arc::new(BufferSource::new(
                Token::new("displayColor"),
                encode_f32_slice_le(&self.vertex_data.colors),
                color_count,
                vec3_size,
            )));
        }
        resource_registry.add_sources(&vertex_bar, sources);

        let raw_buf = {
            let locked = vertex_bar.lock().expect("points vertex_bar lock");
            locked.buffer.clone()
        };
        self.vertex_buffer = Some(raw_buf);
        self.vertex_bar = Some(vertex_bar);
    }

    fn upload_topology(&mut self, resource_registry: &HdStResourceRegistry) {
        let index_count = self.point_indices.len();
        if index_count == 0 {
            return;
        }

        let element_size = std::mem::size_of::<u32>();
        let specs = vec![BufferSpec {
            name: Token::new("indices"),
            num_elements: index_count,
            element_size,
        }];
        let usage = BufferArrayUsageHint {
            index: true,
            ..Default::default()
        };
        let element_bar = resource_registry.update_non_uniform_bar(
            &Token::new("pointsTopology"),
            self.element_bar.as_ref(),
            &specs,
            &[],
            usage,
        );
        resource_registry.add_source(
            &element_bar,
            Arc::new(BufferSource::new(
                Token::new("indices"),
                encode_u32_slice_le(&self.point_indices),
                index_count,
                element_size,
            )),
        );

        let raw_buf = {
            let locked = element_bar.lock().expect("points element_bar lock");
            locked.buffer.clone()
        };
        self.index_buffer = Some(raw_buf);
        self.element_bar = Some(element_bar);
    }

    fn sync_constant_primvars(&mut self, resource_registry: &HdStResourceRegistry) {
        let mat_f32: Vec<f32> = self
            .world_transform
            .iter()
            .flat_map(|row| row.iter().map(|&v| v as f32))
            .collect();
        let element_size = std::mem::size_of::<f32>();
        let specs = vec![BufferSpec {
            name: Token::new("transform"),
            num_elements: 16,
            element_size,
        }];
        let usage = BufferArrayUsageHint {
            uniform: true,
            ..Default::default()
        };
        let constant_bar = resource_registry.update_uniform_bar(
            &Token::new("pointsConstant"),
            self.constant_bar.as_ref(),
            &specs,
            &[],
            usage,
        );
        resource_registry.add_source(
            &constant_bar,
            Arc::new(BufferSource::new(
                Token::new("transform"),
                encode_f32_slice_le(&mat_f32),
                16,
                element_size,
            )),
        );
        self.constant_bar = Some(constant_bar);
    }

    fn update_draw_items(&mut self, repr: &Token) {
        self.ensure_repr(repr);

        let (vbuf, vbuf_size, vbuf_offset) = if let Some(ref vbar) = self.vertex_bar {
            let locked = vbar.lock().expect("points vertex_bar lock");
            if !locked.is_valid() {
                return;
            }
            (locked.buffer.clone(), locked.byte_size(), locked.offset)
        } else {
            return;
        };
        let (ibuf, ibuf_size, ibuf_offset) = if let Some(ref ebar) = self.element_bar {
            let locked = ebar.lock().expect("points element_bar lock");
            if !locked.is_valid() {
                return;
            }
            (locked.buffer.clone(), locked.byte_size(), locked.offset)
        } else {
            return;
        };
        let constant_bar: Option<HdBufferArrayRangeSharedPtr> =
            self.constant_bar.as_ref().and_then(|cbar| {
                let locked = cbar.lock().expect("points constant_bar lock");
                if !locked.is_valid() {
                    return None;
                }
                Some(Arc::new(HdStBufferArrayRange::new(
                    locked.buffer.clone(),
                    locked.offset,
                    locked.byte_size(),
                )) as HdBufferArrayRangeSharedPtr)
            });

        let vertex_bar: HdBufferArrayRangeSharedPtr = Arc::new(HdStBufferArrayRange::with_stream_sizes(
            vbuf,
            vbuf_offset,
            vbuf_size,
            self.vertex_data.get_positions_byte_size(),
            self.vertex_data.get_widths_byte_size(),
            0,
            self.vertex_data.get_colors_byte_size(),
        ));
        let element_bar: HdBufferArrayRangeSharedPtr =
            Arc::new(HdStBufferArrayRange::new(ibuf, ibuf_offset, ibuf_size));

        let bbox = compute_aabb(&self.vertex_data.positions, &self.vertex_data.widths);
        for item in &self.draw_items {
            if item.get_repr() != *repr {
                continue;
            }
            item.set_vertex_bar(vertex_bar.clone());
            item.set_element_bar(element_bar.clone());
            item.clear_constant_bar();
            if let Some(ref constant_bar) = constant_bar {
                item.set_constant_bar(constant_bar.clone());
            }
            item.set_primitive_topology(DrawTopology::PointList);
            item.set_primitive_kind(DrawPrimitiveKind::Points);
            item.set_geometric_shader_key(points_geometric_shader_key());
            item.set_visible(self.visible);
            item.set_material_network_shader(self.material_params.clone());
            item.set_bbox(bbox.0, bbox.1);
        }
    }
}

/// Shared pointer to Storm points.
pub type HdStPointsSharedPtr = Arc<HdStPoints>;

fn points_geometric_shader_key() -> u64 {
    0x5054_5354_504F_494Eu64
}

fn flatten_vec3f(values: &[Vec3f]) -> Vec<f32> {
    values.iter().flat_map(|v| [v.x, v.y, v.z]).collect()
}

fn expand_indexed_vec3f(values: &[Vec3f], indices: Option<&[i32]>) -> Vec<Vec3f> {
    if let Some(indices) = indices {
        indices
            .iter()
            .map(|&i| values.get(i.max(0) as usize).copied().unwrap_or_default())
            .collect()
    } else {
        values.to_vec()
    }
}

fn encode_f32_slice_le(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn encode_u32_slice_le(values: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn compute_aabb(positions: &[f32], widths: &[f32]) -> ([f32; 3], [f32; 3]) {
    if positions.len() < 3 {
        return ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
    }
    let max_half_width = widths.iter().fold(0.0f32, |a, &b| a.max(b * 0.5));
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for xyz in positions.chunks_exact(3) {
        min[0] = min[0].min(xyz[0] - max_half_width);
        min[1] = min[1].min(xyz[1] - max_half_width);
        min[2] = min[2].min(xyz[2] - max_half_width);
        max[0] = max[0].max(xyz[0] + max_half_width);
        max[1] = max[1].max(xyz[1] + max_half_width);
        max[2] = max[2].max(xyz[2] + max_half_width);
    }
    (min, max)
}

fn matrix4d_to_rows(matrix: usd_gf::Matrix4d) -> [[f64; 4]; 4] {
    [
        [matrix[0][0], matrix[0][1], matrix[0][2], matrix[0][3]],
        [matrix[1][0], matrix[1][1], matrix[1][2], matrix[1][3]],
        [matrix[2][0], matrix[2][1], matrix[2][2], matrix[2][3]],
        [matrix[3][0], matrix[3][1], matrix[3][2], matrix[3][3]],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_points_creation() {
        let path = SdfPath::from_string("/points/cloud1").unwrap();
        let points = HdStPoints::new(path.clone());

        assert_eq!(points.path, path);
        assert_eq!(points.vertex_data.get_point_count(), 0);
    }

    #[test]
    fn test_expand_indexed_vec3f() {
        let values = vec![Vec3f::new(1.0, 0.0, 0.0), Vec3f::new(0.0, 1.0, 0.0)];
        let expanded = expand_indexed_vec3f(&values, Some(&[1, 0]));
        assert_eq!(expanded[0], values[1]);
        assert_eq!(expanded[1], values[0]);
    }

    #[test]
    fn test_points_aabb_includes_widths() {
        let positions = vec![0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let widths = vec![2.0f32, 2.0];
        let (min, max) = compute_aabb(&positions, &widths);
        assert!(min[1] <= -1.0);
        assert!(max[1] >= 1.0);
    }
}
