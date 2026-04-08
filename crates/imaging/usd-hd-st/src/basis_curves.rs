//! HdStBasisCurves - Storm basis curves prim implementation.
//!
//! This module deliberately mirrors the retained three-phase Storm contract
//! used by meshes and points:
//! 1. read delegate state,
//! 2. build CPU-side topology/primvar payload,
//! 3. upload BAR-backed resources and refresh draw items.
//!
//! The previous Rust port only stored authored curve metadata and cleared dirty
//! bits, which meant valid `BasisCurves` prims from USD/Alembic loaded into the
//! stage but never reached the live Storm draw path.

use crate::basis_curves_computations::{CurveInterpolation, interpolate_primvar};
use crate::basis_curves_topology::{CurveIndexResult, HdStBasisCurvesTopology};
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
use usd_gf::{Matrix4d, Vec3f};
use usd_hd::HdSceneDelegate;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Cubic curve basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveBasis {
    /// Cubic Bezier basis.
    Bezier,
    /// Cubic B-spline basis.
    #[default]
    BSpline,
    /// Cubic Catmull-Rom basis.
    CatmullRom,
    /// Cubic Hermite basis.
    Hermite,
}

/// Curve segment type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveType {
    /// Straight polyline segments.
    #[default]
    Linear,
    /// Cubic segments.
    Cubic,
}

/// Curve wrap mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CurveWrap {
    /// Open curve.
    #[default]
    NonPeriodic,
    /// Closed curve.
    Periodic,
    /// Pinned endpoints.
    Pinned,
}

/// Retained curve primvar payload stored on the Storm rprim.
///
/// Widths and normals are kept even when the current interactive draw path
/// does not yet consume every authored channel. This keeps the rprim state
/// faithful to the delegate and allows future shader work to attach to real
/// retained data rather than to another compatibility stub.
#[derive(Debug, Clone, Default)]
pub struct HdStBasisCurvesVertexData {
    /// Packed xyz control points.
    pub positions: Vec<f32>,
    /// Widths expanded to one value per control point where possible.
    pub widths: Vec<f32>,
    /// Optional packed normals.
    pub normals: Vec<f32>,
    /// Optional packed displayColor.
    pub colors: Vec<f32>,
}

impl HdStBasisCurvesVertexData {
    /// Number of logical control points.
    pub fn get_vertex_count(&self) -> usize {
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

    /// Byte size of packed normals.
    pub fn get_normals_byte_size(&self) -> usize {
        self.normals.len() * std::mem::size_of::<f32>()
    }

    /// Byte size of packed widths.
    pub fn get_widths_byte_size(&self) -> usize {
        self.widths.len() * std::mem::size_of::<f32>()
    }
}

/// Storm basis-curves rprim.
#[derive(Debug, Clone)]
pub struct HdStBasisCurves {
    path: SdfPath,
    topology: HdStBasisCurvesTopology,
    vertex_data: HdStBasisCurvesVertexData,
    active_index_result: CurveIndexResult,
    active_draw_topology: DrawTopology,
    vertex_buffer: Option<HdStBufferResourceSharedPtr>,
    index_buffer: Option<HdStBufferResourceSharedPtr>,
    vertex_bar: Option<ManagedBarSharedPtr>,
    element_bar: Option<ManagedBarSharedPtr>,
    constant_bar: Option<ManagedBarSharedPtr>,
    draw_items: Vec<HdStDrawItemSharedPtr>,
    visible: bool,
    world_transform: [[f64; 4]; 4],
    material_params: MaterialParams,
    topology_dirty: bool,
    vertex_dirty: bool,
    constant_dirty: bool,
}

impl HdStBasisCurves {
    /// Create a new Storm basis-curves prim.
    pub fn new(path: SdfPath) -> Self {
        Self {
            path,
            topology: HdStBasisCurvesTopology::new(
                Vec::new(),
                Vec::new(),
                CurveBasis::BSpline,
                CurveType::Linear,
                CurveWrap::NonPeriodic,
            ),
            vertex_data: HdStBasisCurvesVertexData::default(),
            active_index_result: CurveIndexResult::default(),
            active_draw_topology: DrawTopology::LineList,
            vertex_buffer: None,
            index_buffer: None,
            vertex_bar: None,
            element_bar: None,
            constant_bar: None,
            draw_items: Vec::new(),
            visible: true,
            world_transform: identity_rows(),
            material_params: MaterialParams::default(),
            topology_dirty: true,
            vertex_dirty: true,
            constant_dirty: true,
        }
    }

    /// Number of control vertices in the current vertex data.
    pub fn get_vertex_count(&self) -> usize {
        self.vertex_data.get_vertex_count()
    }

    /// Number of indices in the active topology result.
    pub fn get_index_count(&self) -> usize {
        self.active_index_result.indices.len()
    }

    /// Number of retained draw items.
    pub fn get_draw_item_count(&self) -> usize {
        self.draw_items.len()
    }

    /// Synchronize delegate-owned curve data into the retained Storm prim.
    ///
    /// This is the `_ref`-equivalent delegate read phase. It pulls real Hydra
    /// curve topology and primvars instead of leaving `BasisCurves` as a
    /// metadata-only placeholder.
    pub fn sync_from_delegate(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        dirty_bits: &mut HdDirtyBits,
        repr: &Token,
    ) {
        let started = std::time::Instant::now();
        self.visible = delegate.get_visible(&self.path);

        if *dirty_bits != 0 {
            self.world_transform = matrix4d_to_rows(delegate.get_transform(&self.path));
            self.constant_dirty = true;
        }

        if *dirty_bits != 0 {
            let topology = delegate.get_basis_curves_topology(&self.path);
            self.topology = HdStBasisCurvesTopology::from_hd_topology(&topology);
            self.topology_dirty = true;
        }

        if *dirty_bits != 0 {
            let points_value = delegate.get(&self.path, &Token::new("points"));
            let widths_value = delegate.get(&self.path, &Token::new("widths"));
            let (display_color_value, display_color_indices) =
                delegate.get_indexed_primvar(&self.path, &Token::new("displayColor"));
            let (normals_value, normals_indices) =
                delegate.get_indexed_primvar(&self.path, &Token::new("normals"));

            if let Some(points) = points_value.as_vec_clone::<Vec3f>() {
                self.vertex_data.positions = flatten_vec3f(&points);
            } else {
                self.vertex_data.positions.clear();
            }

            self.vertex_data.widths =
                expand_curve_f32_primvar(&self.topology, widths_value.as_vec_clone::<f32>());

            let expanded_colors = display_color_value
                .as_vec_clone::<Vec3f>()
                .map(|colors| {
                    expand_curve_vec3f_primvar(
                        &self.topology,
                        &colors,
                        display_color_indices.as_deref(),
                    )
                })
                .unwrap_or_default();
            self.vertex_data.colors = flatten_vec3f(&expanded_colors);

            let expanded_normals = normals_value
                .as_vec_clone::<Vec3f>()
                .map(|normals| {
                    expand_curve_vec3f_primvar(&self.topology, &normals, normals_indices.as_deref())
                })
                .unwrap_or_default();
            self.vertex_data.normals = flatten_vec3f(&expanded_normals);

            let _ = repr;
            self.vertex_dirty = true;
        }

        *dirty_bits = 0;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if elapsed_ms > 1.0 {
            log::debug!(
                "[storm] basis_curves_sync: path={} repr={} ms={:.2} cvs={} indices={} widths={} colors={} normals={}",
                self.path,
                repr,
                elapsed_ms,
                self.vertex_data.get_vertex_count(),
                self.active_index_result.indices.len(),
                self.vertex_data.widths.len(),
                self.vertex_data.colors.len() / 3,
                self.vertex_data.normals.len() / 3
            );
        }
    }

    /// Build CPU-side topology payload for the currently requested repr.
    pub fn process_cpu(&mut self, repr: &Token) {
        let started = std::time::Instant::now();
        if self.topology_dirty || self.vertex_dirty {
            let repr_name = repr.as_str();
            let is_points_repr = repr_name == "points";
            if is_points_repr {
                self.active_index_result = self.topology.build_points_index();
                self.active_draw_topology = DrawTopology::PointList;
            } else {
                self.active_index_result = self.topology.build_index(true);
                self.active_draw_topology = DrawTopology::LineList;
            }
        }
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        if elapsed_ms > 1.0 {
            log::debug!(
                "[storm] basis_curves_process_cpu: path={} repr={} ms={:.2} topology={:?} prims={}",
                self.path,
                repr,
                elapsed_ms,
                self.active_draw_topology,
                self.active_index_result.num_segments
            );
        }
    }

    /// Upload retained curve payload to the Storm resource registry.
    pub fn upload_to_registry(&mut self, resource_registry: &HdStResourceRegistry, repr: &Token) {
        self.upload_vertices(resource_registry);
        self.upload_topology(resource_registry);
        self.sync_constant_primvars(resource_registry);
        self.update_draw_items(repr);
        self.topology_dirty = false;
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
        item.set_primitive_topology(self.active_draw_topology);
        item.set_primitive_kind(DrawPrimitiveKind::BasisCurves);
        item.set_material_network_shader(self.material_params.clone());
        self.draw_items.push(Arc::new(item));
    }

    fn upload_vertices(&mut self, resource_registry: &HdStResourceRegistry) {
        if self.vertex_data.positions.is_empty() {
            return;
        }

        let vertex_count = self.vertex_data.get_vertex_count();
        let float_size = std::mem::size_of::<f32>();
        let vec3_size = 3 * float_size;

        // Buffer layout must match bind order in bind_basis_curves_vertex_buffers:
        // positions (loc 0), widths (loc 1), normals (loc 2), colors (loc 3)
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
        if !self.vertex_data.normals.is_empty() {
            specs.push(BufferSpec {
                name: Token::new("normals"),
                num_elements: self.vertex_data.normals.len() / 3,
                element_size: vec3_size,
            });
        }
        if !self.vertex_data.colors.is_empty() {
            specs.push(BufferSpec {
                name: Token::new("displayColor"),
                num_elements: self.vertex_data.colors.len() / 3,
                element_size: vec3_size,
            });
        }

        let usage = BufferArrayUsageHint {
            vertex: true,
            ..Default::default()
        };
        let vertex_bar = resource_registry.update_non_uniform_bar(
            &Token::new("basisCurvesVertex"),
            self.vertex_bar.as_ref(),
            &specs,
            &[],
            usage,
        );

        // Sources must match specs order: positions, widths, normals, colors
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
        if !self.vertex_data.normals.is_empty() {
            sources.push(Arc::new(BufferSource::new(
                Token::new("normals"),
                encode_f32_slice_le(&self.vertex_data.normals),
                self.vertex_data.normals.len() / 3,
                vec3_size,
            )));
        }
        if !self.vertex_data.colors.is_empty() {
            sources.push(Arc::new(BufferSource::new(
                Token::new("displayColor"),
                encode_f32_slice_le(&self.vertex_data.colors),
                self.vertex_data.colors.len() / 3,
                vec3_size,
            )));
        }
        resource_registry.add_sources(&vertex_bar, sources);

        let raw_buf = {
            let locked = vertex_bar.lock().expect("basisCurves vertex_bar lock");
            locked.buffer.clone()
        };
        self.vertex_buffer = Some(raw_buf);
        self.vertex_bar = Some(vertex_bar);
    }

    fn upload_topology(&mut self, resource_registry: &HdStResourceRegistry) {
        if self.active_index_result.indices.is_empty() {
            return;
        }

        let index_count = self.active_index_result.indices.len();
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
            &Token::new("basisCurvesTopology"),
            self.element_bar.as_ref(),
            &specs,
            &[],
            usage,
        );
        resource_registry.add_source(
            &element_bar,
            Arc::new(BufferSource::new(
                Token::new("indices"),
                encode_u32_slice_le(&self.active_index_result.indices),
                index_count,
                element_size,
            )),
        );

        let raw_buf = {
            let locked = element_bar.lock().expect("basisCurves element_bar lock");
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
            &Token::new("basisCurvesConstant"),
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
            let locked = vbar.lock().expect("basisCurves vertex_bar lock");
            if !locked.is_valid() {
                return;
            }
            (locked.buffer.clone(), locked.byte_size(), locked.offset)
        } else {
            return;
        };

        let (ibuf, ibuf_size, ibuf_offset) = if let Some(ref ebar) = self.element_bar {
            let locked = ebar.lock().expect("basisCurves element_bar lock");
            if !locked.is_valid() {
                return;
            }
            (locked.buffer.clone(), locked.byte_size(), locked.offset)
        } else {
            return;
        };

        let constant_bar: Option<HdBufferArrayRangeSharedPtr> =
            self.constant_bar.as_ref().and_then(|cbar| {
                let locked = cbar.lock().expect("basisCurves constant_bar lock");
                if !locked.is_valid() {
                    return None;
                }
                Some(Arc::new(HdStBufferArrayRange::new(
                    locked.buffer.clone(),
                    locked.offset,
                    locked.byte_size(),
                )) as HdBufferArrayRangeSharedPtr)
            });

        // BAR stream sizes map: positions→positions, widths→normals slot,
        // normals→uvs slot, colors→colors. The binder reads normals_byte_size
        // for widths offset and uvs_byte_size for normals offset.
        let vertex_bar: HdBufferArrayRangeSharedPtr =
            Arc::new(HdStBufferArrayRange::with_stream_sizes(
                vbuf,
                vbuf_offset,
                vbuf_size,
                self.vertex_data.get_positions_byte_size(),
                self.vertex_data.get_widths_byte_size(), // → normals slot (binder reads as widths)
                self.vertex_data.get_normals_byte_size(), // → uvs slot (binder reads as normals)
                self.vertex_data.get_colors_byte_size(),
            ));
        let element_bar: HdBufferArrayRangeSharedPtr =
            Arc::new(HdStBufferArrayRange::new(ibuf, ibuf_offset, ibuf_size));

        let bbox = compute_curve_aabb(&self.vertex_data.positions, &self.vertex_data.widths);
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
            item.set_primitive_topology(self.active_draw_topology);
            item.set_primitive_kind(DrawPrimitiveKind::BasisCurves);
            item.set_geometric_shader_key(basis_curves_geometric_shader_key(
                self.active_draw_topology,
            ));
            item.set_visible(self.visible);
            item.set_material_network_shader(self.material_params.clone());
            item.set_bbox(bbox.0, bbox.1);
        }
    }
}

/// Shared pointer to Storm basis curves.
pub type HdStBasisCurvesSharedPtr = Arc<HdStBasisCurves>;

fn identity_rows() -> [[f64; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn flatten_vec3f(values: &[Vec3f]) -> Vec<f32> {
    values.iter().flat_map(|v| [v.x, v.y, v.z]).collect()
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

fn matrix4d_to_rows(matrix: Matrix4d) -> [[f64; 4]; 4] {
    [
        [matrix[0][0], matrix[0][1], matrix[0][2], matrix[0][3]],
        [matrix[1][0], matrix[1][1], matrix[1][2], matrix[1][3]],
        [matrix[2][0], matrix[2][1], matrix[2][2], matrix[2][3]],
        [matrix[3][0], matrix[3][1], matrix[3][2], matrix[3][3]],
    ]
}

fn basis_curves_geometric_shader_key(topology: DrawTopology) -> u64 {
    match topology {
        DrawTopology::PointList => 0x4355_5256_4553_5054u64,
        DrawTopology::LineList => 0x4355_5256_4553_4C4Eu64,
        DrawTopology::TriangleList => 0x4355_5256_4553_5452u64,
    }
}

fn compute_curve_aabb(positions: &[f32], widths: &[f32]) -> ([f32; 3], [f32; 3]) {
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

fn expand_curve_vec3f_primvar(
    topology: &HdStBasisCurvesTopology,
    authored: &[Vec3f],
    indices: Option<&[i32]>,
) -> Vec<Vec3f> {
    let authored = if let Some(indices) = indices {
        indices
            .iter()
            .map(|&i| authored.get(i.max(0) as usize).copied().unwrap_or_default())
            .collect::<Vec<_>>()
    } else {
        authored.to_vec()
    };
    let fallback = Vec3f::new(0.0, 0.0, 0.0);
    let interpolation = infer_curve_interpolation(topology, authored.len());
    interpolate_primvar(topology, &authored, interpolation, &fallback)
}

fn expand_curve_f32_primvar(
    topology: &HdStBasisCurvesTopology,
    authored: Option<Vec<f32>>,
) -> Vec<f32> {
    let authored = authored.unwrap_or_default();
    if authored.is_empty() {
        return Vec::new();
    }
    let interpolation = infer_curve_interpolation(topology, authored.len());
    interpolate_primvar(topology, &authored, interpolation, &1.0f32)
}

fn infer_curve_interpolation(
    topology: &HdStBasisCurvesTopology,
    authored_len: usize,
) -> CurveInterpolation {
    if authored_len == 1 {
        return CurveInterpolation::Constant;
    }
    if authored_len == topology.get_curve_count() {
        return CurveInterpolation::Uniform;
    }
    if authored_len == topology.calc_needed_varying_control_points() {
        return CurveInterpolation::Varying;
    }
    CurveInterpolation::Vertex
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basis_curves_creation() {
        let path = SdfPath::from_string("/curves/main").unwrap();
        let curves = HdStBasisCurves::new(path.clone());
        assert_eq!(curves.path, path);
        assert_eq!(curves.vertex_data.get_vertex_count(), 0);
        assert_eq!(curves.active_draw_topology, DrawTopology::LineList);
    }

    #[test]
    fn test_curve_aabb_includes_widths() {
        let positions = vec![0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0];
        let widths = vec![2.0f32, 2.0];
        let (min, max) = compute_curve_aabb(&positions, &widths);
        assert!(min[1] <= -1.0);
        assert!(max[1] >= 1.0);
    }
}
