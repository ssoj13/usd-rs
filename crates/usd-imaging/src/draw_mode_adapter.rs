//! DrawModeAdapter - Adapter for draw mode visualization.
//!
//! Port of pxr/usdImaging/usdImaging/drawModeAdapter.cpp
//!
//! Provides imaging support for the drawMode attribute on UsdGeomModelAPI.
//! Draw modes include: default, bounds, cards, origin.
//!
//! Geometry generation:
//! - **bounds**: Wireframe bounding box (8 verts, 12 edges = 24 segment indices)
//! - **origin**: 3 colored axis lines from origin (4 verts, 3 segments = 6 indices)
//! - **cards**: 6 quads from extent (front/back/left/right/top/bottom)

use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::{PopulationMode, PropertyInvalidationType};
use std::sync::Arc;
use usd_core::Prim;
use usd_gf::Vec3f;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static MESH: LazyLock<Token> = LazyLock::new(|| Token::new("mesh"));
    pub static DRAW_MODE: LazyLock<Token> = LazyLock::new(|| Token::new("drawMode"));
    pub static DEFAULT: LazyLock<Token> = LazyLock::new(|| Token::new("default"));
    pub static BOUNDS: LazyLock<Token> = LazyLock::new(|| Token::new("bounds"));
    pub static CARDS: LazyLock<Token> = LazyLock::new(|| Token::new("cards"));
    pub static ORIGIN: LazyLock<Token> = LazyLock::new(|| Token::new("origin"));
    pub static CROSS: LazyLock<Token> = LazyLock::new(|| Token::new("cross"));
    pub static BOX: LazyLock<Token> = LazyLock::new(|| Token::new("box"));

    // Cards attributes
    pub static CARD_GEOMETRY: LazyLock<Token> = LazyLock::new(|| Token::new("model:cardGeometry"));
    pub static CARD_TEXTURE_X_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:cardTextureXPos"));
    pub static CARD_TEXTURE_X_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:cardTextureXNeg"));
    pub static CARD_TEXTURE_Y_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:cardTextureYPos"));
    pub static CARD_TEXTURE_Y_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:cardTextureYNeg"));
    pub static CARD_TEXTURE_Z_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:cardTextureZPos"));
    pub static CARD_TEXTURE_Z_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:cardTextureZNeg"));
    pub static DRAW_MODE_COLOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("model:drawModeColor"));
}

// ============================================================================
// Draw mode geometry types
// ============================================================================

/// Axes bitmask for cards geometry generation. Matches C++ AxesMask.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxesMask {
    /// Positive X axis face.
    XPos = 1 << 0,
    /// Positive Y axis face.
    YPos = 1 << 1,
    /// Positive Z axis face.
    ZPos = 1 << 2,
    /// Negative X axis face.
    XNeg = 1 << 3,
    /// Negative Y axis face.
    YNeg = 1 << 4,
    /// Negative Z axis face.
    ZNeg = 1 << 5,
}

#[allow(dead_code)] // C++ card texture axis masks, used in full draw mode impl
const X_AXIS: u8 = AxesMask::XPos as u8 | AxesMask::XNeg as u8;
#[allow(dead_code)]
const Y_AXIS: u8 = AxesMask::YPos as u8 | AxesMask::YNeg as u8;
#[allow(dead_code)]
const Z_AXIS: u8 = AxesMask::ZPos as u8 | AxesMask::ZNeg as u8;

/// Result of draw mode geometry generation.
#[derive(Debug, Clone)]
pub struct DrawModeGeometry {
    /// Vertex positions
    pub points: Vec<Vec3f>,
    /// Topology: for bounds/origin = segment indices, for cards = face vertex indices
    pub indices: Vec<i32>,
    /// For mesh topology: vertex count per face (cards only)
    pub face_vertex_counts: Vec<i32>,
    /// UV coordinates (cards only)
    pub uvs: Vec<[f32; 2]>,
    /// Per-vertex colors (origin only)
    pub colors: Vec<Vec3f>,
}

/// Generate origin geometry: 3 axis lines from (0,0,0).
///
/// Matches C++ `_GenerateOriginGeometry`. Produces 4 vertices and 6 segment
/// indices for X/Y/Z axis lines.
pub fn gen_origin(_extent: &([f32; 3], [f32; 3])) -> DrawModeGeometry {
    // 4 vertices: origin + one per axis endpoint
    let points = vec![
        Vec3f::new(0.0, 0.0, 0.0),
        Vec3f::new(1.0, 0.0, 0.0),
        Vec3f::new(0.0, 1.0, 0.0),
        Vec3f::new(0.0, 0.0, 1.0),
    ];

    // 3 line segments: origin->X, origin->Y, origin->Z
    let indices = vec![0, 1, 0, 2, 0, 3];

    // Per-vertex colors: origin=white, X=red, Y=green, Z=blue
    let colors = vec![
        Vec3f::new(1.0, 1.0, 1.0), // origin (shared, render picks per-segment)
        Vec3f::new(1.0, 0.0, 0.0), // +X red
        Vec3f::new(0.0, 1.0, 0.0), // +Y green
        Vec3f::new(0.0, 0.0, 1.0), // +Z blue
    ];

    DrawModeGeometry {
        points,
        indices,
        face_vertex_counts: vec![],
        uvs: vec![],
        colors,
    }
}

/// Generate bounds geometry: wireframe bounding box.
///
/// Matches C++ `_GenerateBoundsGeometry`. Produces 8 vertices (box corners)
/// and 24 segment indices for 12 edges.
///
/// Vertex encoding: bit 0 = Z, bit 1 = Y, bit 2 = X
/// (i & 4) ? max.x : min.x, (i & 2) ? max.y : min.y, (i & 1) ? max.z : min.z
pub fn gen_bounds(extent: &([f32; 3], [f32; 3])) -> DrawModeGeometry {
    let min = extent.0;
    let max = extent.1;

    // 8 corner vertices
    let mut points = Vec::with_capacity(8);
    for i in 0..8u32 {
        let x = if i & 4 != 0 { max[0] } else { min[0] };
        let y = if i & 2 != 0 { max[1] } else { min[1] };
        let z = if i & 1 != 0 { max[2] } else { min[2] };
        points.push(Vec3f::new(x, y, z));
    }

    // 12 edges = 24 indices (pairs)
    // Bottom face (z=min): 0-4, 4-6, 6-2, 2-0
    // Top face (z=max):    1-5, 5-7, 7-3, 3-1
    // Vertical edges:      0-1, 4-5, 6-7, 2-3
    let indices = vec![
        // bottom face
        0, 4, 4, 6, 6, 2, 2, 0, // top face
        1, 5, 5, 7, 7, 3, 3, 1, // vertical edges
        0, 1, 4, 5, 6, 7, 2, 3,
    ];

    DrawModeGeometry {
        points,
        indices,
        face_vertex_counts: vec![],
        uvs: vec![],
        colors: vec![],
    }
}

/// Generate cards geometry: 6 quads from extent (box mode).
///
/// Matches C++ `_GenerateCardsGeometry` with box cardGeometry and full axes_mask.
/// Produces 24 vertices (4 per face) and face topology for 6 quads.
pub fn gen_cards_box(extent: &([f32; 3], [f32; 3])) -> DrawModeGeometry {
    let min = extent.0;
    let max = extent.1;

    let mut points = Vec::with_capacity(24);
    let mut face_vertex_counts = Vec::with_capacity(6);
    let mut face_vertex_indices = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);

    let mut face_idx = 0i32;

    // Helper: add a quad with 4 vertices, winding CCW from outside
    let mut add_face = |pts: &[Vec3f; 4], face_uvs: &[[f32; 2]; 4]| {
        for p in pts {
            points.push(*p);
        }
        for uv in face_uvs {
            uvs.push(*uv);
        }
        face_vertex_counts.push(4);
        for j in 0..4 {
            face_vertex_indices.push(face_idx * 4 + j);
        }
        face_idx += 1;
    };

    // Standard UV layout per face
    let uv_normal = [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]];

    // +X face
    add_face(
        &[
            Vec3f::new(max[0], max[1], max[2]),
            Vec3f::new(max[0], min[1], max[2]),
            Vec3f::new(max[0], min[1], min[2]),
            Vec3f::new(max[0], max[1], min[2]),
        ],
        &uv_normal,
    );

    // -X face
    add_face(
        &[
            Vec3f::new(min[0], min[1], max[2]),
            Vec3f::new(min[0], max[1], max[2]),
            Vec3f::new(min[0], max[1], min[2]),
            Vec3f::new(min[0], min[1], min[2]),
        ],
        &uv_normal,
    );

    // +Y face
    add_face(
        &[
            Vec3f::new(min[0], max[1], max[2]),
            Vec3f::new(max[0], max[1], max[2]),
            Vec3f::new(max[0], max[1], min[2]),
            Vec3f::new(min[0], max[1], min[2]),
        ],
        &uv_normal,
    );

    // -Y face
    add_face(
        &[
            Vec3f::new(max[0], min[1], max[2]),
            Vec3f::new(min[0], min[1], max[2]),
            Vec3f::new(min[0], min[1], min[2]),
            Vec3f::new(max[0], min[1], min[2]),
        ],
        &uv_normal,
    );

    // +Z face
    add_face(
        &[
            Vec3f::new(max[0], max[1], max[2]),
            Vec3f::new(min[0], max[1], max[2]),
            Vec3f::new(min[0], min[1], max[2]),
            Vec3f::new(max[0], min[1], max[2]),
        ],
        &uv_normal,
    );

    // -Z face
    add_face(
        &[
            Vec3f::new(min[0], max[1], min[2]),
            Vec3f::new(max[0], max[1], min[2]),
            Vec3f::new(max[0], min[1], min[2]),
            Vec3f::new(min[0], min[1], min[2]),
        ],
        &uv_normal,
    );

    DrawModeGeometry {
        points,
        indices: face_vertex_indices,
        face_vertex_counts,
        uvs,
        colors: vec![],
    }
}

/// Generate cross-style cards geometry.
///
/// Matches C++ `_GenerateCardsGeometry` with cross cardGeometry.
/// 3 cross-planes through the extent center, each having 2 faces (front+back).
pub fn gen_cards_cross(extent: &([f32; 3], [f32; 3])) -> DrawModeGeometry {
    let min = extent.0;
    let max = extent.1;
    let mid = [
        (min[0] + max[0]) * 0.5,
        (min[1] + max[1]) * 0.5,
        (min[2] + max[2]) * 0.5,
    ];

    // Small epsilon to prevent coplanarity between +/- sides
    let eps: f32 = f32::from_bits(0x3400_0000); // 2^-23

    let mut points = Vec::with_capacity(24);
    let mut face_vertex_counts = Vec::with_capacity(6);
    let mut face_vertex_indices = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);

    let mut face_idx = 0i32;

    let uv_normal = [[1.0, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]];

    let mut add_face = |pts: &[Vec3f; 4], face_uvs: &[[f32; 2]; 4]| {
        for p in pts {
            points.push(*p);
        }
        for uv in face_uvs {
            uvs.push(*uv);
        }
        face_vertex_counts.push(4);
        for j in 0..4 {
            face_vertex_indices.push(face_idx * 4 + j);
        }
        face_idx += 1;
    };

    // X-axis cross planes (+X at mid+eps, -X at mid-eps)
    let x_pos = mid[0] + eps;
    add_face(
        &[
            Vec3f::new(x_pos, max[1], max[2]),
            Vec3f::new(x_pos, min[1], max[2]),
            Vec3f::new(x_pos, min[1], min[2]),
            Vec3f::new(x_pos, max[1], min[2]),
        ],
        &uv_normal,
    );

    let x_neg = mid[0] - eps;
    add_face(
        &[
            Vec3f::new(x_neg, min[1], max[2]),
            Vec3f::new(x_neg, max[1], max[2]),
            Vec3f::new(x_neg, max[1], min[2]),
            Vec3f::new(x_neg, min[1], min[2]),
        ],
        &uv_normal,
    );

    // Y-axis cross planes
    let y_pos = mid[1] + eps;
    add_face(
        &[
            Vec3f::new(min[0], y_pos, max[2]),
            Vec3f::new(max[0], y_pos, max[2]),
            Vec3f::new(max[0], y_pos, min[2]),
            Vec3f::new(min[0], y_pos, min[2]),
        ],
        &uv_normal,
    );

    let y_neg = mid[1] - eps;
    add_face(
        &[
            Vec3f::new(max[0], y_neg, max[2]),
            Vec3f::new(min[0], y_neg, max[2]),
            Vec3f::new(min[0], y_neg, min[2]),
            Vec3f::new(max[0], y_neg, min[2]),
        ],
        &uv_normal,
    );

    // Z-axis cross planes
    let z_pos = mid[2] + eps;
    add_face(
        &[
            Vec3f::new(max[0], max[1], z_pos),
            Vec3f::new(min[0], max[1], z_pos),
            Vec3f::new(min[0], min[1], z_pos),
            Vec3f::new(max[0], min[1], z_pos),
        ],
        &uv_normal,
    );

    let z_neg = mid[2] - eps;
    add_face(
        &[
            Vec3f::new(min[0], max[1], z_neg),
            Vec3f::new(max[0], max[1], z_neg),
            Vec3f::new(max[0], min[1], z_neg),
            Vec3f::new(min[0], min[1], z_neg),
        ],
        &uv_normal,
    );

    DrawModeGeometry {
        points,
        indices: face_vertex_indices,
        face_vertex_counts,
        uvs,
        colors: vec![],
    }
}

/// Determine draw mode from prim attribute, defaulting to "default".
pub fn get_draw_mode(prim: &Prim) -> Token {
    if let Some(attr) = prim.get_attribute("model:drawMode") {
        if attr.is_valid() {
            if let Some(val) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(s) = val.downcast_clone::<String>() {
                    return Token::new(&s);
                }
            }
        }
    }
    tokens::DEFAULT.clone()
}

/// Compute extent from a UsdGeomBoundable prim. Returns ((min_x, min_y, min_z), (max_x, ...)).
///
/// Tries the `extent` attribute first, falls back to a unit cube.
pub fn compute_extent(prim: &Prim) -> ([f32; 3], [f32; 3]) {
    if let Some(attr) = prim.get_attribute("extent") {
        if attr.is_valid() {
            if let Some(val) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(ext) = val.as_vec_clone::<Vec3f>() {
                    if ext.len() >= 2 {
                        return (
                            [ext[0].x, ext[0].y, ext[0].z],
                            [ext[1].x, ext[1].y, ext[1].z],
                        );
                    }
                }
            }
        }
    }
    // Default unit cube
    ([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
}

// ============================================================================
// DataSourceDrawMode
// ============================================================================

/// Data source for draw mode visualization.
///
/// Returns the geometry data for the active draw mode (bounds/origin/cards).
#[derive(Clone)]
pub struct DataSourceDrawMode {
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceDrawMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceDrawMode").finish()
    }
}

impl DataSourceDrawMode {
    /// Create new draw mode data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }

    /// Compute geometry for the prim's draw mode.
    pub fn compute_geometry(&self) -> Option<DrawModeGeometry> {
        let mode = get_draw_mode(&self.prim);
        let extent = compute_extent(&self.prim);

        if mode == *tokens::BOUNDS {
            Some(gen_bounds(&extent))
        } else if mode == *tokens::ORIGIN {
            Some(gen_origin(&extent))
        } else if mode == *tokens::CARDS {
            Some(gen_cards_box(&extent))
        } else {
            None
        }
    }
}

impl HdDataSourceBase for DataSourceDrawMode {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceDrawMode {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::DRAW_MODE.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::DRAW_MODE {
            // Return the draw mode token as a sampled data source
            let mode = get_draw_mode(&self.prim);
            let ds = usd_hd::HdRetainedTypedSampledDataSource::new(mode.as_str().to_string());
            return Some(ds as HdDataSourceBaseHandle);
        }
        None
    }
}

// ============================================================================
// DataSourceDrawModePrim
// ============================================================================

/// Prim data source for draw mode prims.
#[derive(Clone)]
pub struct DataSourceDrawModePrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    gprim_ds: Arc<DataSourceGprim>,
    draw_mode_ds: Arc<DataSourceDrawMode>,
}

impl std::fmt::Debug for DataSourceDrawModePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceDrawModePrim").finish()
    }
}

impl DataSourceDrawModePrim {
    /// Create new draw mode prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let gprim_ds = DataSourceGprim::new(
            scene_index_path.clone(),
            prim.clone(),
            stage_globals.clone(),
        );
        let draw_mode_ds = DataSourceDrawMode::new(prim, stage_globals);
        Arc::new(Self {
            scene_index_path,
            gprim_ds,
            draw_mode_ds,
        })
    }

    /// Compute invalidation for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        for prop in properties {
            let prop_str = prop.as_str();
            // Draw mode related properties
            if prop_str.starts_with("model:") || prop_str == "extentsHint" {
                locators.insert(HdDataSourceLocator::from_token(tokens::DRAW_MODE.clone()));
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceDrawModePrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceDrawModePrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.gprim_ds.get_names();
        names.push(tokens::DRAW_MODE.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::DRAW_MODE {
            return Some(Arc::clone(&self.draw_mode_ds) as HdDataSourceBaseHandle);
        }
        self.gprim_ds.get(name)
    }
}

// ============================================================================
// DrawModeAdapter
// ============================================================================

/// Adapter for draw mode visualization.
///
/// Draw mode allows simplified rendering of prims using:
/// - bounds: Bounding box wireframe (8 verts, 12 edges)
/// - cards: Textured quads (6 faces from extent)
/// - origin: Axis visualization (3 colored lines)
///
/// This adapter culls children (the draw mode replaces the subtree)
/// and can populate USD instances.
#[derive(Debug, Clone)]
pub struct DrawModeAdapter;

impl Default for DrawModeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DrawModeAdapter {
    /// Create a new draw mode adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for DrawModeAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            // Draw mode renders as mesh
            tokens::MESH.clone()
        } else {
            Token::new("")
        }
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            Some(DataSourceDrawModePrim::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
            ))
        } else {
            None
        }
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceDrawModePrim::invalidate(prim, subprim, properties, invalidation_type)
    }

    /// Draw mode adapter culls children - it replaces the entire subtree.
    /// Matches C++ `ShouldCullChildren() { return true; }`.
    fn should_cull_children(&self) -> bool {
        true
    }

    /// Draw mode adapter's population mode: represents self and all descendants.
    fn get_population_mode(&self) -> PopulationMode {
        PopulationMode::RepresentsSelfAndDescendents
    }
}

/// Handle type for DrawModeAdapter.
pub type DrawModeAdapterHandle = Arc<DrawModeAdapter>;

/// Factory for creating draw mode adapters.
pub fn create_draw_mode_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(DrawModeAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_draw_mode_adapter() {
        let adapter = DrawModeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_draw_mode_subprims() {
        let adapter = DrawModeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
    }

    #[test]
    fn test_draw_mode_data_source() {
        let adapter = DrawModeAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_draw_mode_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("model:drawMode")];

        let locators = DataSourceDrawModePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_should_cull_children_via_trait() {
        let adapter = DrawModeAdapter::new();
        // Test via PrimAdapter trait method
        assert!(adapter.should_cull_children());
    }

    #[test]
    fn test_population_mode() {
        let adapter = DrawModeAdapter::new();
        assert_eq!(
            adapter.get_population_mode(),
            PopulationMode::RepresentsSelfAndDescendents
        );
    }

    #[test]
    fn test_factory() {
        let adapter = create_draw_mode_adapter();
        // Verify trait methods work through dyn dispatch
        assert!(adapter.should_cull_children());
    }

    // ========================================================================
    // Geometry generation tests
    // ========================================================================

    #[test]
    fn test_gen_origin() {
        let extent = ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let geo = gen_origin(&extent);

        assert_eq!(geo.points.len(), 4, "origin should have 4 vertices");
        assert_eq!(geo.indices.len(), 6, "origin should have 6 segment indices");
        assert_eq!(geo.colors.len(), 4, "origin should have 4 vertex colors");

        // Origin vertex at (0,0,0)
        assert_eq!(geo.points[0], Vec3f::new(0.0, 0.0, 0.0));
        // +X endpoint
        assert_eq!(geo.points[1], Vec3f::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn test_gen_bounds() {
        let extent = ([-1.0, -2.0, -3.0], [1.0, 2.0, 3.0]);
        let geo = gen_bounds(&extent);

        assert_eq!(geo.points.len(), 8, "bounds should have 8 vertices");
        assert_eq!(
            geo.indices.len(),
            24,
            "bounds should have 24 segment indices (12 edges)"
        );

        // Vertex 0 = (min.x, min.y, min.z)
        assert_eq!(geo.points[0], Vec3f::new(-1.0, -2.0, -3.0));
        // Vertex 7 = (max.x, max.y, max.z) - all bits set
        assert_eq!(geo.points[7], Vec3f::new(1.0, 2.0, 3.0));

        // All indices should be valid
        for &idx in &geo.indices {
            assert!((idx as usize) < geo.points.len());
        }
    }

    #[test]
    fn test_gen_cards_box() {
        let extent = ([-1.0, -1.0, -1.0], [1.0, 1.0, 1.0]);
        let geo = gen_cards_box(&extent);

        assert_eq!(
            geo.points.len(),
            24,
            "cards box should have 24 vertices (4 per face x 6)"
        );
        assert_eq!(
            geo.face_vertex_counts.len(),
            6,
            "cards box should have 6 faces"
        );
        assert_eq!(
            geo.indices.len(),
            24,
            "cards box should have 24 face vertex indices"
        );
        assert_eq!(geo.uvs.len(), 24, "cards box should have 24 UV coords");

        // Each face should be a quad
        for &count in &geo.face_vertex_counts {
            assert_eq!(count, 4);
        }

        // All indices should be valid
        for &idx in &geo.indices {
            assert!((idx as usize) < geo.points.len());
        }
    }

    #[test]
    fn test_gen_cards_cross() {
        let extent = ([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]);
        let geo = gen_cards_cross(&extent);

        assert_eq!(geo.points.len(), 24, "cards cross should have 24 vertices");
        assert_eq!(
            geo.face_vertex_counts.len(),
            6,
            "cards cross should have 6 faces"
        );

        // Cross planes should be at midpoint +/- epsilon
        let mid_x = 1.0f32;
        // +X face first vertex should be near mid_x
        assert!((geo.points[0].x - mid_x).abs() < 0.001);
    }

    #[test]
    fn test_gen_bounds_degenerate_extent() {
        // Zero-size extent (flat box)
        let extent = ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let geo = gen_bounds(&extent);

        // Should still produce valid geometry (all verts at origin)
        assert_eq!(geo.points.len(), 8);
        for pt in &geo.points {
            assert_eq!(*pt, Vec3f::new(0.0, 0.0, 0.0));
        }
    }

    #[test]
    fn test_gen_bounds_asymmetric() {
        // Non-centered extent
        let extent = ([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        let geo = gen_bounds(&extent);

        // Vertex 0 should be min corner
        assert_eq!(geo.points[0], Vec3f::new(1.0, 2.0, 3.0));
        // Vertex 7 should be max corner
        assert_eq!(geo.points[7], Vec3f::new(4.0, 5.0, 6.0));
    }
}
