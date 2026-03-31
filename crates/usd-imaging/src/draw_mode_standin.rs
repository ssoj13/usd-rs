#![allow(dead_code)]
//! DrawModeStandin - Stand-in geometry for prims with non-default draw modes.
//!
//! Port of pxr/usdImaging/usdImaging/drawModeStandin.cpp (2244 lines).
//!
//! Provides three standin families:
//! - **Bounds**: wireframe bounding box (basisCurves) from extent
//! - **Origin**: 3 perpendicular axis lines from origin (basisCurves)
//! - **Cards**: textured card mesh with up to 6 faces, geom subsets, and materials

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use usd_gf::{Vec2f, Vec3f};
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::observer::{AddedPrimEntry, DirtiedPrimEntry};
use usd_hd::scene_index::{HdSceneIndexPrim, SdfPathVector};
use usd_sdf::Path;
use usd_tf::Token;

// =============================================================================
// Tokens
// =============================================================================

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Draw modes
    pub static BOUNDS: LazyLock<Token> = LazyLock::new(|| Token::new("bounds"));
    pub static ORIGIN: LazyLock<Token> = LazyLock::new(|| Token::new("origin"));
    pub static CARDS: LazyLock<Token> = LazyLock::new(|| Token::new("cards"));

    // Sub-prim names
    pub static BOUNDS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("boundsCurves"));
    pub static ORIGIN_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("originCurves"));
    pub static CARDS_MESH: LazyLock<Token> = LazyLock::new(|| Token::new("cardsMesh"));

    // Schema keys
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new("purpose"));
    pub static VISIBILITY: LazyLock<Token> = LazyLock::new(|| Token::new("visibility"));
    pub static INSTANCED_BY: LazyLock<Token> = LazyLock::new(|| Token::new("instancedBy"));
    pub static DISPLAY_STYLE: LazyLock<Token> = LazyLock::new(|| Token::new("displayStyle"));
    pub static PRIM_ORIGIN: LazyLock<Token> = LazyLock::new(|| Token::new("primOrigin"));
    pub static EXTENT: LazyLock<Token> = LazyLock::new(|| Token::new("extent"));
    pub static MIN: LazyLock<Token> = LazyLock::new(|| Token::new("min"));
    pub static MAX: LazyLock<Token> = LazyLock::new(|| Token::new("max"));

    // Primvar schema
    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static PRIMVAR_VALUE: LazyLock<Token> = LazyLock::new(|| Token::new("primvarValue"));
    pub static INTERPOLATION: LazyLock<Token> = LazyLock::new(|| Token::new("interpolation"));
    pub static ROLE: LazyLock<Token> = LazyLock::new(|| Token::new("role"));
    pub static POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("points"));
    pub static WIDTHS: LazyLock<Token> = LazyLock::new(|| Token::new("widths"));
    pub static DISPLAY_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("displayColor"));
    pub static DISPLAY_OPACITY: LazyLock<Token> = LazyLock::new(|| Token::new("displayOpacity"));
    pub static CARDS_UV: LazyLock<Token> = LazyLock::new(|| Token::new("cardsUv"));
    pub static DISPLAY_ROUGHNESS: LazyLock<Token> =
        LazyLock::new(|| Token::new("displayRoughness"));

    // Interpolation values
    pub static CONSTANT: LazyLock<Token> = LazyLock::new(|| Token::new("constant"));
    pub static VERTEX: LazyLock<Token> = LazyLock::new(|| Token::new("vertex"));

    // Role values
    pub static COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("color"));
    pub static POINT: LazyLock<Token> = LazyLock::new(|| Token::new("point"));

    // BasisCurves schema
    pub static BASIS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("basisCurves"));
    pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
    pub static CURVE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("curveVertexCounts"));
    pub static CURVE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("curveIndices"));
    pub static BASIS: LazyLock<Token> = LazyLock::new(|| Token::new("basis"));
    pub static TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("type"));
    pub static WRAP: LazyLock<Token> = LazyLock::new(|| Token::new("wrap"));
    pub static BEZIER: LazyLock<Token> = LazyLock::new(|| Token::new("bezier"));
    pub static LINEAR: LazyLock<Token> = LazyLock::new(|| Token::new("linear"));
    pub static SEGMENTED: LazyLock<Token> = LazyLock::new(|| Token::new("segmented"));

    // Mesh schema
    pub static MESH: LazyLock<Token> = LazyLock::new(|| Token::new("mesh"));
    pub static FACE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("faceVertexCounts"));
    pub static FACE_VERTEX_INDICES: LazyLock<Token> =
        LazyLock::new(|| Token::new("faceVertexIndices"));
    pub static ORIENTATION: LazyLock<Token> = LazyLock::new(|| Token::new("orientation"));
    pub static RIGHT_HANDED: LazyLock<Token> = LazyLock::new(|| Token::new("rightHanded"));
    pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));

    // DisplayStyle
    pub static CULL_STYLE: LazyLock<Token> = LazyLock::new(|| Token::new("cullStyle"));
    pub static BACK: LazyLock<Token> = LazyLock::new(|| Token::new("back"));
    pub static MATERIAL_IS_FINAL: LazyLock<Token> = LazyLock::new(|| Token::new("materialIsFinal"));

    // GeomModel schema
    pub static GEOM_MODEL: LazyLock<Token> = LazyLock::new(|| Token::new("geomModel"));
    pub static DRAW_MODE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("drawModeColor"));
    pub static CARD_GEOMETRY: LazyLock<Token> = LazyLock::new(|| Token::new("cardGeometry"));
    pub static CARD_TEXTURE_X_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureXPos"));
    pub static CARD_TEXTURE_Y_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureYPos"));
    pub static CARD_TEXTURE_Z_POS: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureZPos"));
    pub static CARD_TEXTURE_X_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureXNeg"));
    pub static CARD_TEXTURE_Y_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureYNeg"));
    pub static CARD_TEXTURE_Z_NEG: LazyLock<Token> =
        LazyLock::new(|| Token::new("cardTextureZNeg"));

    // Card geometry values
    pub static BOX: LazyLock<Token> = LazyLock::new(|| Token::new("box"));
    pub static CROSS: LazyLock<Token> = LazyLock::new(|| Token::new("cross"));
    pub static FROM_TEXTURE: LazyLock<Token> = LazyLock::new(|| Token::new("fromTexture"));

    // Material schema tokens
    pub static MATERIAL: LazyLock<Token> = LazyLock::new(|| Token::new("material"));
    pub static MATERIAL_BINDINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("materialBindings"));
    pub static ALL_PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new(""));
    pub static UNIVERSAL_RENDER_CONTEXT: LazyLock<Token> =
        LazyLock::new(|| Token::new("universalRenderContext"));
    pub static NODES: LazyLock<Token> = LazyLock::new(|| Token::new("nodes"));
    pub static TERMINALS: LazyLock<Token> = LazyLock::new(|| Token::new("terminals"));
    pub static SURFACE: LazyLock<Token> = LazyLock::new(|| Token::new("surface"));
    pub static NODE_IDENTIFIER: LazyLock<Token> = LazyLock::new(|| Token::new("nodeIdentifier"));
    pub static PARAMETERS: LazyLock<Token> = LazyLock::new(|| Token::new("parameters"));
    pub static INPUT_CONNECTIONS: LazyLock<Token> =
        LazyLock::new(|| Token::new("inputConnections"));
    pub static UPSTREAM_NODE_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("upstreamNodePath"));
    pub static UPSTREAM_NODE_OUTPUT_NAME: LazyLock<Token> =
        LazyLock::new(|| Token::new("upstreamNodeOutputName"));
    pub static VALUE: LazyLock<Token> = LazyLock::new(|| Token::new("value"));

    // Material node names
    pub static CARD_SURFACE: LazyLock<Token> = LazyLock::new(|| Token::new("cardSurface"));
    pub static CARD_TEXTURE: LazyLock<Token> = LazyLock::new(|| Token::new("cardTexture"));
    pub static CARD_UV_COORDS: LazyLock<Token> = LazyLock::new(|| Token::new("cardUvCoords"));

    // UsdPreviewSurface params
    pub static USD_PREVIEW_SURFACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("UsdPreviewSurface"));
    pub static USD_UV_TEXTURE: LazyLock<Token> = LazyLock::new(|| Token::new("UsdUVTexture"));
    pub static USD_PRIMVAR_READER_FLOAT2: LazyLock<Token> =
        LazyLock::new(|| Token::new("UsdPrimvarReader_float2"));
    pub static DIFFUSE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("diffuseColor"));
    pub static OPACITY: LazyLock<Token> = LazyLock::new(|| Token::new("opacity"));
    pub static OPACITY_THRESHOLD: LazyLock<Token> =
        LazyLock::new(|| Token::new("opacityThreshold"));
    pub static FALLBACK: LazyLock<Token> = LazyLock::new(|| Token::new("fallback"));
    pub static FILE: LazyLock<Token> = LazyLock::new(|| Token::new("file"));
    pub static ST: LazyLock<Token> = LazyLock::new(|| Token::new("st"));
    pub static WRAP_S: LazyLock<Token> = LazyLock::new(|| Token::new("wrapS"));
    pub static WRAP_T: LazyLock<Token> = LazyLock::new(|| Token::new("wrapT"));
    pub static RGB: LazyLock<Token> = LazyLock::new(|| Token::new("rgb"));
    pub static A: LazyLock<Token> = LazyLock::new(|| Token::new("a"));
    pub static CLAMP: LazyLock<Token> = LazyLock::new(|| Token::new("clamp"));
    pub static VARNAME: LazyLock<Token> = LazyLock::new(|| Token::new("varname"));
    pub static RESULT: LazyLock<Token> = LazyLock::new(|| Token::new("result"));
    pub static PATH: LazyLock<Token> = LazyLock::new(|| Token::new("path"));

    // GeomSubset
    pub static GEOM_SUBSET: LazyLock<Token> = LazyLock::new(|| Token::new("geomSubset"));
    pub static TYPE_FACE_SET: LazyLock<Token> = LazyLock::new(|| Token::new("typeFaceSet"));
    pub static INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("indices"));

    // Prim types
    pub static BASIS_CURVES_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("basisCurves"));
    pub static MESH_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("mesh"));
    pub static MATERIAL_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("material"));
    pub static GEOM_SUBSET_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("geomSubset"));

    // BuiltinMaterial
    pub static BUILTIN_MATERIAL: LazyLock<Token> = LazyLock::new(|| Token::new("builtinMaterial"));
}

// =============================================================================
// Trait: DrawModeStandin
// =============================================================================

/// Trait for draw mode stand-in geometry.
///
/// Port of C++ `UsdImaging_DrawModeStandin` virtual class.
/// Each standin replaces a prim and its descendants with stand-in geometry.
pub trait DrawModeStandin: Send + Sync + std::fmt::Debug {
    /// The draw mode this standin represents.
    fn get_draw_mode(&self) -> Token;

    /// Relative prim paths (including ".").
    fn get_relative_paths(&self) -> Vec<Path>;

    /// Prim type for a relative path.
    fn get_prim_type(&self, rel_path: &Path) -> Token;

    /// Container data source for a relative path.
    fn get_prim_source(&self, rel_path: &Path) -> Option<HdContainerDataSourceHandle>;

    /// Process dirty locators, emitting dirty entries.
    fn process_dirty_locators(
        &self,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: &mut Vec<DirtiedPrimEntry>,
        needs_refresh: &mut bool,
    );

    /// Base path of the prim this standin replaces.
    fn path(&self) -> &Path;

    /// Original prim data source.
    fn prim_source(&self) -> &Option<HdContainerDataSourceHandle>;

    // --- Default implementations ---

    /// Get absolute prim paths for all standin geometry.
    fn get_prim_paths(&self) -> SdfPathVector {
        self.get_relative_paths()
            .iter()
            .filter_map(|rel| {
                let rel_text = rel.get_text();
                if rel_text == "." {
                    Some(self.path().clone())
                } else {
                    self.path().append_path(rel)
                }
            })
            .collect()
    }

    /// Get a prim for a given absolute path.
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        let rel_path = make_relative_path(self.path(), prim_path);
        HdSceneIndexPrim {
            prim_type: self.get_prim_type(&rel_path),
            data_source: self.get_prim_source(&rel_path),
        }
    }

    /// Compute PrimsAdded entries.
    fn compute_prim_added_entries(&self, entries: &mut Vec<AddedPrimEntry>) {
        for rel_path in self.get_relative_paths() {
            let abs_path = if rel_path.get_text() == "." {
                self.path().clone()
            } else {
                match self.path().append_path(&rel_path) {
                    Some(p) => p,
                    None => continue,
                }
            };
            entries.push(AddedPrimEntry::new(abs_path, self.get_prim_type(&rel_path)));
        }
    }
}

/// Make a path relative to a base path.
/// Port of C++ `path.MakeRelativePath(_path)`.
fn make_relative_path(base: &Path, path: &Path) -> Path {
    if base == path {
        return Path::from_string(".").unwrap_or_else(|| Path::empty());
    }
    // Strip base prefix and create relative path
    let base_text = base.get_text();
    let path_text = path.get_text();
    if let Some(suffix) = path_text.strip_prefix(base_text) {
        let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
        if let Some(p) = Path::from_string(suffix) {
            return p;
        }
    }
    Path::empty()
}

pub type DrawModeStandinHandle = Arc<dyn DrawModeStandin>;

// =============================================================================
// Helper: build common data sources
// =============================================================================

/// Build a primvar data source from value, interpolation and role.
/// Port of C++ `_PrimvarDataSource`.
fn build_primvar_ds(
    value: HdDataSourceBaseHandle,
    interpolation: &Token,
    role: &Token,
) -> HdContainerDataSourceHandle {
    let mut children = HashMap::new();
    children.insert(tokens::PRIMVAR_VALUE.clone(), value);
    children.insert(
        tokens::INTERPOLATION.clone(),
        HdRetainedTypedSampledDataSource::new(interpolation.clone()) as HdDataSourceBaseHandle,
    );
    if !role.is_empty() {
        children.insert(
            tokens::ROLE.clone(),
            HdRetainedTypedSampledDataSource::new(role.clone()) as HdDataSourceBaseHandle,
        );
    }
    HdRetainedContainerDataSource::new(children)
}

/// Get draw mode color from prim source's geomModel container.
/// Returns (0.18, 0.18, 0.18) if not found (C++ default).
fn get_draw_mode_color(prim_source: &Option<HdContainerDataSourceHandle>) -> Vec3f {
    use usd_hd::data_source::cast_to_container;
    if let Some(ps) = prim_source {
        if let Some(gm_base) = ps.get(&tokens::GEOM_MODEL) {
            if let Some(gm) = cast_to_container(&gm_base) {
                if let Some(color_base) = gm.get(&tokens::DRAW_MODE_COLOR) {
                    if let Some(sampled) = color_base.as_sampled() {
                        let val = sampled.get_value(0.0);
                        if let Some(c) = val.get::<Vec3f>() {
                            return *c;
                        }
                    }
                }
            }
        }
    }
    Vec3f::new(0.18, 0.18, 0.18)
}

/// Get extent (min, max) from prim source.
fn get_extent(prim_source: &Option<HdContainerDataSourceHandle>) -> (Vec3f, Vec3f) {
    use usd_hd::data_source::cast_to_container;
    let mut ext_min = Vec3f::new(0.0, 0.0, 0.0);
    let mut ext_max = Vec3f::new(0.0, 0.0, 0.0);
    if let Some(ps) = prim_source {
        if let Some(extent_base) = ps.get(&tokens::EXTENT) {
            if let Some(extent_container) = cast_to_container(&extent_base) {
                if let Some(min_base) = extent_container.get(&tokens::MIN) {
                    if let Some(sampled) = min_base.as_sampled() {
                        let val = sampled.get_value(0.0);
                        if let Some(v) = val.get::<usd_gf::Vec3d>() {
                            ext_min = Vec3f::new(v[0] as f32, v[1] as f32, v[2] as f32);
                        }
                    }
                }
                if let Some(max_base) = extent_container.get(&tokens::MAX) {
                    if let Some(sampled) = max_base.as_sampled() {
                        let val = sampled.get_value(0.0);
                        if let Some(v) = val.get::<usd_gf::Vec3d>() {
                            ext_max = Vec3f::new(v[0] as f32, v[1] as f32, v[2] as f32);
                        }
                    }
                }
            }
        }
    }
    (ext_min, ext_max)
}

/// Build common "base" prim data source entries (xform, purpose, visibility, etc.).
/// Port of C++ `_PrimDataSource`.
fn build_base_prim_ds(
    prim_source: &Option<HdContainerDataSourceHandle>,
) -> HashMap<Token, HdDataSourceBaseHandle> {
    let mut children = HashMap::new();

    // Forward xform, purpose, visibility, instancedBy, primOrigin from original prim
    let forward_keys = [
        &*tokens::XFORM,
        &*tokens::PURPOSE,
        &*tokens::VISIBILITY,
        &*tokens::INSTANCED_BY,
        &*tokens::PRIM_ORIGIN,
    ];
    if let Some(ps) = prim_source {
        for key in &forward_keys {
            if let Some(val) = ps.get(key) {
                children.insert((*key).clone(), val);
            }
        }
    }

    // displayStyle: cullStyle=back, materialIsFinal=true
    static DISPLAY_STYLE_DS: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
        let mut ds = HashMap::new();
        ds.insert(
            tokens::CULL_STYLE.clone(),
            HdRetainedTypedSampledDataSource::new(tokens::BACK.clone()) as HdDataSourceBaseHandle,
        );
        ds.insert(
            tokens::MATERIAL_IS_FINAL.clone(),
            HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle,
        );
        HdRetainedContainerDataSource::new(ds)
    });
    children.insert(
        tokens::DISPLAY_STYLE.clone(),
        DISPLAY_STYLE_DS.clone() as HdDataSourceBaseHandle,
    );

    children
}

/// Build common primvar entries: widths, displayColor, displayOpacity.
/// Port of C++ `_PrimvarsDataSource`.
fn build_base_primvars(
    prim_source: &Option<HdContainerDataSourceHandle>,
) -> HashMap<Token, HdDataSourceBaseHandle> {
    let mut primvars = HashMap::new();

    // widths: constant [1.0]
    static WIDTHS_DS: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
        build_primvar_ds(
            HdRetainedTypedSampledDataSource::new(vec![1.0f32]) as HdDataSourceBaseHandle,
            &tokens::CONSTANT,
            &Token::empty(),
        )
    });
    primvars.insert(
        tokens::WIDTHS.clone(),
        WIDTHS_DS.clone() as HdDataSourceBaseHandle,
    );

    // displayColor: from drawModeColor
    let color = get_draw_mode_color(prim_source);
    let color_ds = build_primvar_ds(
        HdRetainedTypedSampledDataSource::new(vec![color]) as HdDataSourceBaseHandle,
        &tokens::CONSTANT,
        &tokens::COLOR,
    );
    primvars.insert(
        tokens::DISPLAY_COLOR.clone(),
        color_ds as HdDataSourceBaseHandle,
    );

    // displayOpacity: constant [1.0]
    static OPACITY_DS: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
        build_primvar_ds(
            HdRetainedTypedSampledDataSource::new(vec![1.0f32]) as HdDataSourceBaseHandle,
            &tokens::CONSTANT,
            &Token::empty(),
        )
    });
    primvars.insert(
        tokens::DISPLAY_OPACITY.clone(),
        OPACITY_DS.clone() as HdDataSourceBaseHandle,
    );

    primvars
}

// =============================================================================
// BoundsStandin
// =============================================================================

/// Stand-in for bounds draw mode.
///
/// Draws the edges of a bounding box using basis curves determined by extent.
/// Port of C++ `_BoundsDrawMode::_BoundsStandin`.
#[derive(Debug)]
pub struct BoundsStandin {
    path: Path,
    prim_source: Option<HdContainerDataSourceHandle>,
}

impl BoundsStandin {
    pub fn new(path: Path, prim_source: Option<HdContainerDataSourceHandle>) -> Self {
        Self { path, prim_source }
    }

    /// Relative path to the bounds curves child prim.
    fn descendant_path() -> &'static Path {
        static P: LazyLock<Path> = LazyLock::new(|| Path::from_string("boundsCurves").unwrap());
        &P
    }

    /// Compute bounds wireframe topology.
    /// Port of C++ `_ComputeBoundsTopology`.
    fn compute_topology() -> HdContainerDataSourceHandle {
        static TOPO: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
            // 12 edges of a box: bottom face, top face, 4 vertical edges
            let curve_indices: Vec<i32> = vec![
                0, 4, 4, 6, 6, 2, 2, 0, // bottom face
                1, 5, 5, 7, 7, 3, 3, 1, // top face
                0, 1, 4, 5, 6, 7, 2, 3, // vertical edges
            ];
            let curve_vertex_counts: Vec<i32> = vec![curve_indices.len() as i32];

            let mut topo = HashMap::new();
            topo.insert(
                tokens::CURVE_VERTEX_COUNTS.clone(),
                HdRetainedTypedSampledDataSource::new(curve_vertex_counts)
                    as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::CURVE_INDICES.clone(),
                HdRetainedTypedSampledDataSource::new(curve_indices) as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::BASIS.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::BEZIER.clone())
                    as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::TYPE.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::LINEAR.clone())
                    as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::WRAP.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::SEGMENTED.clone())
                    as HdDataSourceBaseHandle,
            );
            HdRetainedContainerDataSource::new(topo)
        });
        TOPO.clone()
    }

    /// Compute 8 box vertices from extent.
    /// Port of C++ `_BoundsPointsPrimvarValueDataSource`.
    fn compute_points(prim_source: &Option<HdContainerDataSourceHandle>) -> Vec<Vec3f> {
        let (ext_min, ext_max) = get_extent(prim_source);
        let exts = [ext_min, ext_max];
        let mut pts = Vec::with_capacity(8);
        for j0 in 0..2 {
            for j1 in 0..2 {
                for j2 in 0..2 {
                    pts.push(Vec3f::new(exts[j0][0], exts[j1][1], exts[j2][2]));
                }
            }
        }
        pts
    }

    /// Build the full data source for the bounds curves prim.
    fn build_prim_data_source(&self) -> HdContainerDataSourceHandle {
        let mut ds = build_base_prim_ds(&self.prim_source);

        // basisCurves: topology
        let mut bc = HashMap::new();
        bc.insert(
            tokens::TOPOLOGY.clone(),
            Self::compute_topology() as HdDataSourceBaseHandle,
        );
        ds.insert(
            tokens::BASIS_CURVES.clone(),
            HdRetainedContainerDataSource::new(bc) as HdDataSourceBaseHandle,
        );

        // primvars: base + points
        let mut primvars = build_base_primvars(&self.prim_source);
        let points = Self::compute_points(&self.prim_source);
        let points_ds = build_primvar_ds(
            HdRetainedTypedSampledDataSource::new(points) as HdDataSourceBaseHandle,
            &tokens::VERTEX,
            &tokens::POINT,
        );
        primvars.insert(tokens::POINTS.clone(), points_ds as HdDataSourceBaseHandle);
        ds.insert(
            tokens::PRIMVARS.clone(),
            HdRetainedContainerDataSource::new(primvars) as HdDataSourceBaseHandle,
        );

        // extent: forward from original
        if let Some(ps) = &self.prim_source {
            if let Some(ext) = ps.get(&tokens::EXTENT) {
                ds.insert(tokens::EXTENT.clone(), ext);
            }
        }

        HdRetainedContainerDataSource::new(ds)
    }
}

impl DrawModeStandin for BoundsStandin {
    fn get_draw_mode(&self) -> Token {
        tokens::BOUNDS.clone()
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn prim_source(&self) -> &Option<HdContainerDataSourceHandle> {
        &self.prim_source
    }

    fn get_relative_paths(&self) -> Vec<Path> {
        vec![
            Path::from_string(".").unwrap_or_else(|| Path::empty()),
            Self::descendant_path().clone(),
        ]
    }

    fn get_prim_type(&self, rel_path: &Path) -> Token {
        if rel_path == Self::descendant_path() {
            tokens::BASIS_CURVES_TYPE.clone()
        } else {
            Token::empty()
        }
    }

    fn get_prim_source(&self, rel_path: &Path) -> Option<HdContainerDataSourceHandle> {
        if rel_path == Self::descendant_path() {
            Some(self.build_prim_data_source())
        } else {
            None
        }
    }

    fn process_dirty_locators(
        &self,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: &mut Vec<DirtiedPrimEntry>,
        needs_refresh: &mut bool,
    ) {
        *needs_refresh = false;

        let extent_locator = HdDataSourceLocator::from_token(tokens::EXTENT.clone());
        let color_locator = HdDataSourceLocator::from_tokens_2(
            tokens::GEOM_MODEL.clone(),
            tokens::DRAW_MODE_COLOR.clone(),
        );

        let dirty_extent = dirty_locators.contains(&extent_locator);
        let dirty_color = dirty_locators.contains(&color_locator);

        if dirty_extent || dirty_color {
            let mut prim_dirty = dirty_locators.clone();
            if dirty_extent {
                // Points depend on extent
                prim_dirty.insert(HdDataSourceLocator::from_tokens_3(
                    tokens::PRIMVARS.clone(),
                    tokens::POINTS.clone(),
                    tokens::PRIMVAR_VALUE.clone(),
                ));
            }
            if dirty_color {
                // displayColor depends on drawModeColor
                prim_dirty.insert(HdDataSourceLocator::from_tokens_2(
                    tokens::PRIMVARS.clone(),
                    tokens::DISPLAY_COLOR.clone(),
                ));
            }
            for path in self.get_prim_paths() {
                entries.push(DirtiedPrimEntry {
                    prim_path: path,
                    dirty_locators: prim_dirty.clone(),
                });
            }
        } else {
            for path in self.get_prim_paths() {
                entries.push(DirtiedPrimEntry {
                    prim_path: path,
                    dirty_locators: dirty_locators.clone(),
                });
            }
        }
    }
}

// =============================================================================
// OriginStandin
// =============================================================================

/// Stand-in for origin draw mode.
///
/// Draws 3 perpendicular lines of unit length from the origin.
/// Port of C++ `_OriginDrawMode::_OriginStandin`.
#[derive(Debug)]
pub struct OriginStandin {
    path: Path,
    prim_source: Option<HdContainerDataSourceHandle>,
}

impl OriginStandin {
    pub fn new(path: Path, prim_source: Option<HdContainerDataSourceHandle>) -> Self {
        Self { path, prim_source }
    }

    fn descendant_path() -> &'static Path {
        static P: LazyLock<Path> = LazyLock::new(|| Path::from_string("originCurves").unwrap());
        &P
    }

    /// Compute origin topology: 3 line segments from origin.
    /// Port of C++ `_ComputeOriginTopology`.
    fn compute_topology() -> HdContainerDataSourceHandle {
        static TOPO: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
            // 3 segments: (0,0,0)->(1,0,0), (0,0,0)->(0,1,0), (0,0,0)->(0,0,1)
            let curve_indices: Vec<i32> = vec![0, 1, 0, 2, 0, 3];
            let curve_vertex_counts: Vec<i32> = vec![curve_indices.len() as i32];

            let mut topo = HashMap::new();
            topo.insert(
                tokens::CURVE_VERTEX_COUNTS.clone(),
                HdRetainedTypedSampledDataSource::new(curve_vertex_counts)
                    as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::CURVE_INDICES.clone(),
                HdRetainedTypedSampledDataSource::new(curve_indices) as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::BASIS.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::BEZIER.clone())
                    as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::TYPE.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::LINEAR.clone())
                    as HdDataSourceBaseHandle,
            );
            topo.insert(
                tokens::WRAP.clone(),
                HdRetainedTypedSampledDataSource::new(tokens::SEGMENTED.clone())
                    as HdDataSourceBaseHandle,
            );
            HdRetainedContainerDataSource::new(topo)
        });
        TOPO.clone()
    }

    /// Build the full data source for the origin curves prim.
    fn build_prim_data_source(&self) -> HdContainerDataSourceHandle {
        let mut ds = build_base_prim_ds(&self.prim_source);

        // basisCurves: topology
        let mut bc = HashMap::new();
        bc.insert(
            tokens::TOPOLOGY.clone(),
            Self::compute_topology() as HdDataSourceBaseHandle,
        );
        ds.insert(
            tokens::BASIS_CURVES.clone(),
            HdRetainedContainerDataSource::new(bc) as HdDataSourceBaseHandle,
        );

        // primvars: base + static origin points
        let mut primvars = build_base_primvars(&self.prim_source);
        static ORIGIN_POINTS: LazyLock<Vec<Vec3f>> = LazyLock::new(|| {
            vec![
                Vec3f::new(0.0, 0.0, 0.0),
                Vec3f::new(1.0, 0.0, 0.0),
                Vec3f::new(0.0, 1.0, 0.0),
                Vec3f::new(0.0, 0.0, 1.0),
            ]
        });
        let points_ds = build_primvar_ds(
            HdRetainedTypedSampledDataSource::new(ORIGIN_POINTS.clone()) as HdDataSourceBaseHandle,
            &tokens::VERTEX,
            &tokens::POINT,
        );
        primvars.insert(tokens::POINTS.clone(), points_ds as HdDataSourceBaseHandle);
        ds.insert(
            tokens::PRIMVARS.clone(),
            HdRetainedContainerDataSource::new(primvars) as HdDataSourceBaseHandle,
        );

        // extent: forward from original
        if let Some(ps) = &self.prim_source {
            if let Some(ext) = ps.get(&tokens::EXTENT) {
                ds.insert(tokens::EXTENT.clone(), ext);
            }
        }

        HdRetainedContainerDataSource::new(ds)
    }
}

impl DrawModeStandin for OriginStandin {
    fn get_draw_mode(&self) -> Token {
        tokens::ORIGIN.clone()
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn prim_source(&self) -> &Option<HdContainerDataSourceHandle> {
        &self.prim_source
    }

    fn get_relative_paths(&self) -> Vec<Path> {
        vec![
            Path::from_string(".").unwrap_or_else(|| Path::empty()),
            Self::descendant_path().clone(),
        ]
    }

    fn get_prim_type(&self, rel_path: &Path) -> Token {
        if rel_path == Self::descendant_path() {
            tokens::BASIS_CURVES_TYPE.clone()
        } else {
            Token::empty()
        }
    }

    fn get_prim_source(&self, rel_path: &Path) -> Option<HdContainerDataSourceHandle> {
        if rel_path == Self::descendant_path() {
            Some(self.build_prim_data_source())
        } else {
            None
        }
    }

    fn process_dirty_locators(
        &self,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: &mut Vec<DirtiedPrimEntry>,
        needs_refresh: &mut bool,
    ) {
        *needs_refresh = false;

        let color_locator = HdDataSourceLocator::from_tokens_2(
            tokens::GEOM_MODEL.clone(),
            tokens::DRAW_MODE_COLOR.clone(),
        );

        if dirty_locators.contains(&color_locator) {
            let mut prim_dirty = dirty_locators.clone();
            prim_dirty.insert(HdDataSourceLocator::from_tokens_2(
                tokens::PRIMVARS.clone(),
                tokens::DISPLAY_COLOR.clone(),
            ));
            for path in self.get_prim_paths() {
                entries.push(DirtiedPrimEntry {
                    prim_path: path,
                    dirty_locators: prim_dirty.clone(),
                });
            }
        } else {
            for path in self.get_prim_paths() {
                entries.push(DirtiedPrimEntry {
                    prim_path: path,
                    dirty_locators: dirty_locators.clone(),
                });
            }
        }
    }
}

// =============================================================================
// CardsStandin
// =============================================================================

/// Axis face indices: [XPos, YPos, ZPos, XNeg, YNeg, ZNeg]
const NUM_FACES: usize = 6;

/// Cached cards data computed from prim source.
/// Port of C++ `_CardsDataCache::_CardsData`.
#[derive(Debug)]
struct CardsData {
    card_geometry: Token,
    points: Vec<Vec3f>,
    uvs: Vec<Vec2f>,
    has_face: [bool; NUM_FACES],
    has_texture: [bool; NUM_FACES],
    face_count: usize,
    /// GeomSubset names for faces that exist
    geom_subset_names: Vec<Token>,
    /// Material names for faces that have textures
    material_names: Vec<Token>,
    /// Mapping from face index to material name (for subset material binding)
    face_to_material: Vec<Option<Token>>,
}

/// Subset name tokens: subsetXPos, subsetYPos, etc.
fn subset_name(axis: usize) -> Token {
    static NAMES: LazyLock<[Token; NUM_FACES]> = LazyLock::new(|| {
        [
            Token::new("subsetXPos"),
            Token::new("subsetYPos"),
            Token::new("subsetZPos"),
            Token::new("subsetXNeg"),
            Token::new("subsetYNeg"),
            Token::new("subsetZNeg"),
        ]
    });
    NAMES[axis].clone()
}

/// Material name tokens: subsetMaterialXPos, etc.
fn material_name(axis: usize) -> Token {
    static NAMES: LazyLock<[Token; NUM_FACES]> = LazyLock::new(|| {
        [
            Token::new("subsetMaterialXPos"),
            Token::new("subsetMaterialYPos"),
            Token::new("subsetMaterialZPos"),
            Token::new("subsetMaterialXNeg"),
            Token::new("subsetMaterialYNeg"),
            Token::new("subsetMaterialZNeg"),
        ]
    });
    NAMES[axis].clone()
}

impl CardsData {
    /// Build cards data from prim source.
    fn from_prim_source(
        _prim_path: &Path,
        prim_source: &Option<HdContainerDataSourceHandle>,
    ) -> Self {
        let (card_geometry, has_face, has_texture) = Self::resolve_card_faces(prim_source);
        let face_count = has_face.iter().filter(|&&v| v).count();
        let points = Self::compute_points(&card_geometry, &has_face);
        let uvs = Self::compute_uvs(&card_geometry, &has_face, &has_texture);

        // Build subset and material name lists
        let mut geom_subset_names = Vec::new();
        let mut material_names = Vec::new();
        let mut face_to_material = Vec::new();

        // Track which faces are present, in insertion order: [x+,x-,y+,y-,z+,z-]
        // Values order: [XPos,YPos,ZPos,XNeg,YNeg,ZNeg]
        if has_texture.iter().any(|&t| t) {
            for i in 0..6 {
                // Face insertion order maps i -> values index vi
                let vi = (i % 2) * 3 + i / 2;
                if has_face[vi] {
                    geom_subset_names.push(subset_name(vi));
                    // Use opposite face's material if no texture for this face
                    let mi = if has_texture[vi] { vi } else { (vi + 3) % 6 };
                    face_to_material.push(Some(material_name(mi)));
                }
            }
            for i in 0..NUM_FACES {
                if has_texture[i] {
                    material_names.push(material_name(i));
                }
            }
        }

        Self {
            card_geometry,
            points,
            uvs,
            has_face,
            has_texture,
            face_count,
            geom_subset_names,
            material_names,
            face_to_material,
        }
    }

    /// Resolve which faces to draw and whether they have textures.
    /// Returns (cardGeometry, hasFace[6], hasTexture[6]).
    fn resolve_card_faces(
        prim_source: &Option<HdContainerDataSourceHandle>,
    ) -> (Token, [bool; NUM_FACES], [bool; NUM_FACES]) {
        use usd_hd::data_source::cast_to_container;

        let mut card_geometry = tokens::CROSS.clone();
        let mut has_face = [false; NUM_FACES];
        let mut has_texture = [false; NUM_FACES];

        if let Some(ps) = prim_source {
            if let Some(gm_base) = ps.get(&tokens::GEOM_MODEL) {
                if let Some(gm) = cast_to_container(&gm_base) {
                    // Get card geometry
                    if let Some(cg_base) = gm.get(&tokens::CARD_GEOMETRY) {
                        if let Some(sampled) = cg_base.as_sampled() {
                            let val = sampled.get_value(0.0);
                            if let Some(t) = val.get::<Token>() {
                                card_geometry = t.clone();
                            }
                        }
                    }

                    // Check texture paths
                    let tex_tokens = [
                        &*tokens::CARD_TEXTURE_X_POS,
                        &*tokens::CARD_TEXTURE_Y_POS,
                        &*tokens::CARD_TEXTURE_Z_POS,
                        &*tokens::CARD_TEXTURE_X_NEG,
                        &*tokens::CARD_TEXTURE_Y_NEG,
                        &*tokens::CARD_TEXTURE_Z_NEG,
                    ];

                    if card_geometry == *tokens::FROM_TEXTURE {
                        // fromTexture: each face is independent
                        for (k, tex_tok) in tex_tokens.iter().enumerate() {
                            if let Some(tex_base) = gm.get(tex_tok) {
                                if let Some(sampled) = tex_base.as_sampled() {
                                    let val = sampled.get_value(0.0);
                                    if let Some(s) = val.get::<String>() {
                                        if !s.is_empty() {
                                            has_texture[k] = true;
                                            has_face[k] = true;
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // box/cross: opposite faces share textures
                        for i in 0..3 {
                            for j in 0..2 {
                                let k = i + 3 * j;
                                let l = i + 3 * (1 - j);
                                if let Some(tex_base) = gm.get(tex_tokens[k]) {
                                    if let Some(sampled) = tex_base.as_sampled() {
                                        let val = sampled.get_value(0.0);
                                        if let Some(s) = val.get::<String>() {
                                            if !s.is_empty() {
                                                has_texture[k] = true;
                                                has_face[k] = true;
                                                has_face[l] = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // If no textures at all, draw all faces
                        if !has_face.iter().any(|&f| f) {
                            has_face = [true; NUM_FACES];
                        }
                    }
                }
            }
        }

        // Default: all faces if no geomModel at all
        if !has_face.iter().any(|&f| f) {
            has_face = [true; NUM_FACES];
        }

        (card_geometry, has_face, has_texture)
    }

    /// Compute card mesh points.
    /// Port of C++ `_CardsDataCache::_CardsData::_ComputePoints`.
    fn compute_points(card_geometry: &Token, has_face: &[bool; NUM_FACES]) -> Vec<Vec3f> {
        let face_count = has_face.iter().filter(|&&v| v).count();
        let mut points = Vec::with_capacity(4 * face_count);

        if card_geometry == &*tokens::FROM_TEXTURE {
            // fromTexture: points computed from worldToScreen matrices (not supported
            // without image metadata reading). Use unit cube fallback.
            for i in 0..3 {
                for j in 0..2 {
                    let k = i + 3 * j;
                    if has_face[k] {
                        // Fallback: unit-cube face points
                        let x = 1.0f32;
                        let pts = [
                            Vec3f::new(x, 1.0, 1.0),
                            Vec3f::new(x, 0.0, 1.0),
                            Vec3f::new(x, 0.0, 0.0),
                            Vec3f::new(x, 1.0, 0.0),
                        ];
                        let one = Vec3f::new(1.0, 1.0, 1.0);
                        if j == 0 {
                            for l in 0..4 {
                                points.push(transform_card_point(&pts[l], i));
                            }
                        } else {
                            for l in (0..4).rev() {
                                points.push(one - transform_card_point(&pts[l], i));
                            }
                        }
                    }
                }
            }
        } else {
            // box or cross
            let x = if card_geometry == &*tokens::BOX {
                1.0f32
            } else {
                0.5f32
            };

            let eps = if card_geometry == &*tokens::CROSS {
                f32::from_bits(0x3400_0000) // 2^-23
            } else {
                0.0f32
            };

            let pts = [
                Vec3f::new(x, 1.0, 1.0),
                Vec3f::new(x, 0.0, 1.0),
                Vec3f::new(x, 0.0, 0.0),
                Vec3f::new(x, 1.0, 0.0),
            ];
            let one = Vec3f::new(1.0, 1.0, 1.0);

            for i in 0..3 {
                let axis_offset = Vec3f::new(
                    if i == 0 { eps } else { 0.0 },
                    if i == 1 { eps } else { 0.0 },
                    if i == 2 { eps } else { 0.0 },
                );
                if has_face[i] {
                    for k in 0..4 {
                        points.push(transform_card_point(&(pts[k] + axis_offset), i));
                    }
                }
                if has_face[i + 3] {
                    for k in (0..4).rev() {
                        points.push(one - transform_card_point(&(pts[k] + axis_offset), i));
                    }
                }
            }
        }

        points
    }

    /// Compute UV coordinates for card faces.
    /// Port of C++ `_CardsDataCache::_CardsData::_ComputeUVs`.
    fn compute_uvs(
        card_geometry: &Token,
        has_face: &[bool; NUM_FACES],
        has_texture: &[bool; NUM_FACES],
    ) -> Vec<Vec2f> {
        let face_count = has_face.iter().filter(|&&v| v).count();
        let mut uvs = Vec::with_capacity(4 * face_count);

        if card_geometry == &*tokens::FROM_TEXTURE {
            for i in 0..3 {
                for j in 0..2 {
                    let k = i + 3 * j;
                    if has_face[k] {
                        fill_uvs(false, false, &mut uvs);
                    }
                }
            }
        } else {
            // X and Y axes
            for i in 0..2 {
                for j in 0..2 {
                    let k = i + 3 * j;
                    if has_face[k] {
                        fill_uvs(!has_texture[k], false, &mut uvs);
                    }
                }
            }
            // Z axis: special handling
            if has_face[2] {
                fill_uvs(false, !has_texture[2], &mut uvs);
            }
            if has_face[5] {
                fill_uvs(true, has_texture[5], &mut uvs);
            }
        }

        uvs
    }
}

/// Transform a card point for the given axis.
/// Port of C++ `_Transform`.
fn transform_card_point(v: &Vec3f, axis: usize) -> Vec3f {
    match axis {
        0 => *v,                                 // x-axis: identity
        1 => Vec3f::new(1.0 - v[1], v[0], v[2]), // y-axis: 90 deg about z
        2 => Vec3f::new(v[1], v[2], v[0]),       // z-axis: 120 deg about space diagonal
        _ => *v,
    }
}

/// Get UV with optional flipping.
fn get_uv(u: f32, v: f32, flip_u: bool, flip_v: bool) -> Vec2f {
    Vec2f::new(
        if flip_u { 1.0 - u } else { u },
        if flip_v { 1.0 - v } else { v },
    )
}

/// Fill 4 UVs for a quad face.
fn fill_uvs(flip_u: bool, flip_v: bool, uvs: &mut Vec<Vec2f>) {
    uvs.push(get_uv(1.0, 1.0, flip_u, flip_v));
    uvs.push(get_uv(0.0, 1.0, flip_u, flip_v));
    uvs.push(get_uv(0.0, 0.0, flip_u, flip_v));
    uvs.push(get_uv(1.0, 0.0, flip_u, flip_v));
}

/// Build disjoint quad mesh topology for n quads.
/// Port of C++ `_DisjointQuadTopology`.
fn build_quad_topology(n: usize) -> HdContainerDataSourceHandle {
    let face_vertex_counts: Vec<i32> = vec![4i32; n];
    // Use saturating_mul to avoid overflow for large n before casting to i32.
    let count = (4usize).saturating_mul(n).min(i32::MAX as usize) as i32;
    let face_vertex_indices: Vec<i32> = (0..count).collect();

    let mut topo = HashMap::new();
    topo.insert(
        tokens::FACE_VERTEX_COUNTS.clone(),
        HdRetainedTypedSampledDataSource::new(face_vertex_counts) as HdDataSourceBaseHandle,
    );
    topo.insert(
        tokens::FACE_VERTEX_INDICES.clone(),
        HdRetainedTypedSampledDataSource::new(face_vertex_indices) as HdDataSourceBaseHandle,
    );
    topo.insert(
        tokens::ORIENTATION.clone(),
        HdRetainedTypedSampledDataSource::new(tokens::RIGHT_HANDED.clone())
            as HdDataSourceBaseHandle,
    );
    HdRetainedContainerDataSource::new(topo)
}

/// Apply extent to card points (box/cross only, not fromTexture).
fn apply_extent_to_points(
    points: &[Vec3f],
    prim_source: &Option<HdContainerDataSourceHandle>,
) -> Vec<Vec3f> {
    let (ext_min, ext_max) = get_extent(prim_source);
    points
        .iter()
        .map(|pt| {
            Vec3f::new(
                ext_min[0] * (1.0 - pt[0]) + ext_max[0] * pt[0],
                ext_min[1] * (1.0 - pt[1]) + ext_max[1] * pt[1],
                ext_min[2] * (1.0 - pt[2]) + ext_max[2] * pt[2],
            )
        })
        .collect()
}

/// Stand-in for cards draw mode.
///
/// Provides a mesh with up to 6 quad faces, each with geom subsets and
/// per-face materials for textured cards.
/// Port of C++ `_CardsDrawMode::_CardsStandin`.
#[derive(Debug)]
pub struct CardsStandin {
    path: Path,
    prim_source: Option<HdContainerDataSourceHandle>,
    data: CardsData,
}

impl CardsStandin {
    pub fn new(path: Path, prim_source: Option<HdContainerDataSourceHandle>) -> Self {
        let data = CardsData::from_prim_source(&path, &prim_source);
        Self {
            path,
            prim_source,
            data,
        }
    }

    fn cards_mesh_path() -> &'static Path {
        static P: LazyLock<Path> = LazyLock::new(|| Path::from_string("cardsMesh").unwrap());
        &P
    }

    /// Build mesh prim data source.
    fn build_mesh_data_source(&self) -> HdContainerDataSourceHandle {
        let mut ds = build_base_prim_ds(&self.prim_source);

        // mesh topology
        let topo = build_quad_topology(self.data.face_count);
        let mut mesh_ds = HashMap::new();
        mesh_ds.insert(tokens::TOPOLOGY.clone(), topo as HdDataSourceBaseHandle);
        mesh_ds.insert(
            tokens::DOUBLE_SIDED.clone(),
            HdRetainedTypedSampledDataSource::new(false) as HdDataSourceBaseHandle,
        );
        ds.insert(
            tokens::MESH.clone(),
            HdRetainedContainerDataSource::new(mesh_ds) as HdDataSourceBaseHandle,
        );

        // primvars: base + points + cardsUv + displayRoughness
        let mut primvars = build_base_primvars(&self.prim_source);

        // Points: apply extent if not fromTexture
        let pts = if self.data.card_geometry != *tokens::FROM_TEXTURE {
            apply_extent_to_points(&self.data.points, &self.prim_source)
        } else {
            self.data.points.clone()
        };
        let points_ds = build_primvar_ds(
            HdRetainedTypedSampledDataSource::new(pts) as HdDataSourceBaseHandle,
            &tokens::VERTEX,
            &tokens::POINT,
        );
        primvars.insert(tokens::POINTS.clone(), points_ds as HdDataSourceBaseHandle);

        // cardsUv primvar
        let uvs_ds = build_primvar_ds(
            HdRetainedTypedSampledDataSource::new(self.data.uvs.clone()) as HdDataSourceBaseHandle,
            &tokens::VERTEX,
            &Token::empty(),
        );
        primvars.insert(tokens::CARDS_UV.clone(), uvs_ds as HdDataSourceBaseHandle);

        // displayRoughness: constant [1.0]
        static ROUGHNESS_DS: LazyLock<HdContainerDataSourceHandle> = LazyLock::new(|| {
            build_primvar_ds(
                HdRetainedTypedSampledDataSource::new(vec![1.0f32]) as HdDataSourceBaseHandle,
                &tokens::CONSTANT,
                &Token::empty(),
            )
        });
        primvars.insert(
            tokens::DISPLAY_ROUGHNESS.clone(),
            ROUGHNESS_DS.clone() as HdDataSourceBaseHandle,
        );

        ds.insert(
            tokens::PRIMVARS.clone(),
            HdRetainedContainerDataSource::new(primvars) as HdDataSourceBaseHandle,
        );

        // extent: from original or computed
        if let Some(ps) = &self.prim_source {
            if let Some(ext) = ps.get(&tokens::EXTENT) {
                ds.insert(tokens::EXTENT.clone(), ext);
            }
        }

        HdRetainedContainerDataSource::new(ds)
    }

    /// Build a material data source for a face with a texture.
    /// Port of C++ `_ComputeMaterials`.
    fn build_material_ds(&self, face_idx: usize) -> HdContainerDataSourceHandle {
        let color = get_draw_mode_color(&self.prim_source);
        let has_tex = self.data.has_texture[face_idx];

        // Build surface node
        let surface_node = self.build_surface_node(has_tex, &color);

        let mut node_names = vec![tokens::CARD_SURFACE.clone()];
        let mut node_values: Vec<HdDataSourceBaseHandle> = vec![surface_node];

        if has_tex {
            // Texture node
            let texture_node = self.build_texture_node(face_idx, &color);
            node_names.push(tokens::CARD_TEXTURE.clone());
            node_values.push(texture_node);

            // UV reader node
            let uv_node = Self::build_uv_reader_node();
            node_names.push(tokens::CARD_UV_COORDS.clone());
            node_values.push(uv_node);
        }

        // Nodes container
        let nodes_ds = HdRetainedContainerDataSource::from_arrays(&node_names, &node_values);

        // Terminal: surface -> cardSurface.surface
        let terminal_ds = build_connection(&tokens::CARD_SURFACE, &tokens::SURFACE);
        let terminals_ds =
            HdRetainedContainerDataSource::new_1(tokens::SURFACE.clone(), terminal_ds);

        // Network
        let mut network = HashMap::new();
        network.insert(tokens::NODES.clone(), nodes_ds as HdDataSourceBaseHandle);
        network.insert(
            tokens::TERMINALS.clone(),
            terminals_ds as HdDataSourceBaseHandle,
        );
        let network_ds = HdRetainedContainerDataSource::new(network);

        // Material schema: universalRenderContext -> network
        let material_ds = HdRetainedContainerDataSource::new_1(
            tokens::UNIVERSAL_RENDER_CONTEXT.clone(),
            network_ds as HdDataSourceBaseHandle,
        );

        // Wrap in material + builtinMaterial
        let mut result = HashMap::new();
        result.insert(
            tokens::MATERIAL.clone(),
            material_ds as HdDataSourceBaseHandle,
        );
        let builtin = HdRetainedContainerDataSource::new_1(
            tokens::BUILTIN_MATERIAL.clone(),
            HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle,
        );
        result.insert(
            tokens::BUILTIN_MATERIAL.clone(),
            builtin as HdDataSourceBaseHandle,
        );
        HdRetainedContainerDataSource::new(result)
    }

    /// Build UsdPreviewSurface node.
    fn build_surface_node(&self, has_texture: bool, color: &Vec3f) -> HdDataSourceBaseHandle {
        let mut params = HashMap::new();
        let mut input_conns = HashMap::new();

        if has_texture {
            // Connect diffuseColor and opacity from texture
            input_conns.insert(
                tokens::DIFFUSE_COLOR.clone(),
                build_connection_vec(&tokens::CARD_TEXTURE, &tokens::RGB),
            );
            input_conns.insert(
                tokens::OPACITY.clone(),
                build_connection_vec(&tokens::CARD_TEXTURE, &tokens::A),
            );
            // opacityThreshold = 0.1 for cutouts
            params.insert(
                tokens::OPACITY_THRESHOLD.clone(),
                build_param_value(
                    HdRetainedTypedSampledDataSource::new(0.1f32) as HdDataSourceBaseHandle
                ),
            );
        } else {
            // Use draw mode color directly
            params.insert(
                tokens::DIFFUSE_COLOR.clone(),
                build_param_value(
                    HdRetainedTypedSampledDataSource::new(*color) as HdDataSourceBaseHandle
                ),
            );
            params.insert(
                tokens::OPACITY.clone(),
                build_param_value(
                    HdRetainedTypedSampledDataSource::new(1.0f32) as HdDataSourceBaseHandle
                ),
            );
        }

        let mut node = HashMap::new();
        node.insert(
            tokens::NODE_IDENTIFIER.clone(),
            HdRetainedTypedSampledDataSource::new(tokens::USD_PREVIEW_SURFACE.clone())
                as HdDataSourceBaseHandle,
        );
        node.insert(
            tokens::PARAMETERS.clone(),
            HdRetainedContainerDataSource::new(params) as HdDataSourceBaseHandle,
        );
        if !input_conns.is_empty() {
            node.insert(
                tokens::INPUT_CONNECTIONS.clone(),
                HdRetainedContainerDataSource::new(input_conns) as HdDataSourceBaseHandle,
            );
        }
        HdRetainedContainerDataSource::new(node) as HdDataSourceBaseHandle
    }

    /// Build UsdUVTexture node.
    fn build_texture_node(&self, _face_idx: usize, color: &Vec3f) -> HdDataSourceBaseHandle {
        let mut params = HashMap::new();
        params.insert(
            tokens::WRAP_S.clone(),
            build_param_value(HdRetainedTypedSampledDataSource::new(tokens::CLAMP.clone())
                as HdDataSourceBaseHandle),
        );
        params.insert(
            tokens::WRAP_T.clone(),
            build_param_value(HdRetainedTypedSampledDataSource::new(tokens::CLAMP.clone())
                as HdDataSourceBaseHandle),
        );
        // Fallback: vec4f from drawModeColor
        let fallback_val = usd_gf::Vec4f::new(color[0], color[1], color[2], 1.0);
        params.insert(
            tokens::FALLBACK.clone(),
            build_param_value(
                HdRetainedTypedSampledDataSource::new(fallback_val) as HdDataSourceBaseHandle
            ),
        );
        params.insert(
            tokens::ST.clone(),
            build_param_value(
                HdRetainedTypedSampledDataSource::new(tokens::CARDS_UV.clone())
                    as HdDataSourceBaseHandle,
            ),
        );

        // Input connection: st -> cardUvCoords.result
        let mut input_conns = HashMap::new();
        input_conns.insert(
            tokens::ST.clone(),
            build_connection_vec(&tokens::CARD_UV_COORDS, &tokens::RESULT),
        );

        let mut node = HashMap::new();
        node.insert(
            tokens::NODE_IDENTIFIER.clone(),
            HdRetainedTypedSampledDataSource::new(tokens::USD_UV_TEXTURE.clone())
                as HdDataSourceBaseHandle,
        );
        node.insert(
            tokens::PARAMETERS.clone(),
            HdRetainedContainerDataSource::new(params) as HdDataSourceBaseHandle,
        );
        node.insert(
            tokens::INPUT_CONNECTIONS.clone(),
            HdRetainedContainerDataSource::new(input_conns) as HdDataSourceBaseHandle,
        );
        HdRetainedContainerDataSource::new(node) as HdDataSourceBaseHandle
    }

    /// Build UsdPrimvarReader_float2 node for cardsUv.
    fn build_uv_reader_node() -> HdDataSourceBaseHandle {
        let mut params = HashMap::new();
        params.insert(
            tokens::VARNAME.clone(),
            build_param_value(
                HdRetainedTypedSampledDataSource::new(tokens::CARDS_UV.clone())
                    as HdDataSourceBaseHandle,
            ),
        );

        let mut node = HashMap::new();
        node.insert(
            tokens::NODE_IDENTIFIER.clone(),
            HdRetainedTypedSampledDataSource::new(tokens::USD_PRIMVAR_READER_FLOAT2.clone())
                as HdDataSourceBaseHandle,
        );
        node.insert(
            tokens::PARAMETERS.clone(),
            HdRetainedContainerDataSource::new(params) as HdDataSourceBaseHandle,
        );
        HdRetainedContainerDataSource::new(node) as HdDataSourceBaseHandle
    }

    /// Build a geom subset data source.
    fn build_geom_subset_ds(
        &self,
        face_index: i32,
        material_path: &Path,
    ) -> HdContainerDataSourceHandle {
        let mut children = HashMap::new();

        // geomSubset: type=typeFaceSet, indices=[face_index]
        let mut subset_ds = HashMap::new();
        subset_ds.insert(
            tokens::TYPE.clone(),
            HdRetainedTypedSampledDataSource::new(tokens::TYPE_FACE_SET.clone())
                as HdDataSourceBaseHandle,
        );
        subset_ds.insert(
            tokens::INDICES.clone(),
            HdRetainedTypedSampledDataSource::new(vec![face_index]) as HdDataSourceBaseHandle,
        );
        children.insert(
            tokens::GEOM_SUBSET.clone(),
            HdRetainedContainerDataSource::new(subset_ds) as HdDataSourceBaseHandle,
        );

        // visibility: true
        let vis_ds = HdRetainedContainerDataSource::new_1(
            tokens::VISIBILITY.clone(),
            HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle,
        );
        children.insert(tokens::VISIBILITY.clone(), vis_ds as HdDataSourceBaseHandle);

        // materialBindings: allPurpose -> path
        let binding_ds = HdRetainedContainerDataSource::new_1(
            tokens::PATH.clone(),
            HdRetainedTypedSampledDataSource::new(material_path.clone()) as HdDataSourceBaseHandle,
        );
        let bindings_ds = HdRetainedContainerDataSource::new_1(
            Token::empty(), // allPurpose
            binding_ds as HdDataSourceBaseHandle,
        );
        children.insert(
            tokens::MATERIAL_BINDINGS.clone(),
            bindings_ds as HdDataSourceBaseHandle,
        );

        // displayStyle: materialIsFinal=true
        let style_ds = HdRetainedContainerDataSource::new_1(
            tokens::MATERIAL_IS_FINAL.clone(),
            HdRetainedTypedSampledDataSource::new(true) as HdDataSourceBaseHandle,
        );
        children.insert(
            tokens::DISPLAY_STYLE.clone(),
            style_ds as HdDataSourceBaseHandle,
        );

        HdRetainedContainerDataSource::new(children)
    }
}

/// Build a material connection data source.
fn build_connection(node_name: &Token, output_name: &Token) -> HdDataSourceBaseHandle {
    let mut conn = HashMap::new();
    conn.insert(
        tokens::UPSTREAM_NODE_PATH.clone(),
        HdRetainedTypedSampledDataSource::new(node_name.clone()) as HdDataSourceBaseHandle,
    );
    conn.insert(
        tokens::UPSTREAM_NODE_OUTPUT_NAME.clone(),
        HdRetainedTypedSampledDataSource::new(output_name.clone()) as HdDataSourceBaseHandle,
    );
    HdRetainedContainerDataSource::new(conn) as HdDataSourceBaseHandle
}

/// Build a connection vector (array of 1 connection) for input connections.
fn build_connection_vec(node_name: &Token, output_name: &Token) -> HdDataSourceBaseHandle {
    // In C++ this is HdRetainedSmallVectorDataSource with 1 element.
    // We use a container with "0" key.
    let conn = build_connection(node_name, output_name);
    HdRetainedContainerDataSource::new_1(Token::new("0"), conn) as HdDataSourceBaseHandle
}

/// Wrap a value as a material node parameter (value container).
fn build_param_value(value: HdDataSourceBaseHandle) -> HdDataSourceBaseHandle {
    HdRetainedContainerDataSource::new_1(tokens::VALUE.clone(), value) as HdDataSourceBaseHandle
}

impl DrawModeStandin for CardsStandin {
    fn get_draw_mode(&self) -> Token {
        tokens::CARDS.clone()
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn prim_source(&self) -> &Option<HdContainerDataSourceHandle> {
        &self.prim_source
    }

    fn get_relative_paths(&self) -> Vec<Path> {
        let mut paths = vec![
            Path::from_string(".").unwrap_or_else(|| Path::empty()),
            Self::cards_mesh_path().clone(),
        ];

        // Material prims are siblings of cardsMesh
        for mat_name in &self.data.material_names {
            if let Some(p) = Path::from_string(mat_name.as_str()) {
                paths.push(p);
            }
        }

        // GeomSubset prims are children of cardsMesh
        for subset_name in &self.data.geom_subset_names {
            let child_path = format!("cardsMesh/{}", subset_name.as_str());
            if let Some(p) = Path::from_string(&child_path) {
                paths.push(p);
            }
        }

        paths
    }

    fn get_prim_type(&self, rel_path: &Path) -> Token {
        let element_count = rel_path.get_path_element_count();

        if element_count == 1 {
            let name = rel_path.get_name_token();
            if name == *tokens::CARDS_MESH {
                return tokens::MESH_TYPE.clone();
            }
            // Check if it's a material name
            if self.data.material_names.iter().any(|m| *m == name) {
                return tokens::MATERIAL_TYPE.clone();
            }
            return Token::empty();
        }
        if element_count == 2 {
            // Check if parent is cardsMesh and child is a geomSubset
            let parent = rel_path.get_parent_path();
            if parent.get_name_token() == *tokens::CARDS_MESH {
                let name = rel_path.get_name_token();
                if self.data.geom_subset_names.iter().any(|s| *s == name) {
                    return tokens::GEOM_SUBSET_TYPE.clone();
                }
            }
            return Token::empty();
        }
        Token::empty()
    }

    fn get_prim_source(&self, rel_path: &Path) -> Option<HdContainerDataSourceHandle> {
        let element_count = rel_path.get_path_element_count();

        if element_count == 1 {
            let name = rel_path.get_name_token();
            if name == *tokens::CARDS_MESH {
                return Some(self.build_mesh_data_source());
            }
            // Material lookup
            for mat_name in self.data.material_names.iter() {
                if *mat_name == name {
                    // Find which face index this material belongs to
                    for i in 0..NUM_FACES {
                        if self.data.has_texture[i] && material_name(i) == *mat_name {
                            return Some(self.build_material_ds(i));
                        }
                    }
                }
            }
            return None;
        }
        if element_count == 2 {
            let parent = rel_path.get_parent_path();
            if parent.get_name_token() == *tokens::CARDS_MESH {
                let name = rel_path.get_name_token();
                // Find subset index and material binding
                for (idx, subset) in self.data.geom_subset_names.iter().enumerate() {
                    if *subset == name {
                        let material_tok = &self.data.face_to_material[idx];
                        if let Some(mat_tok) = material_tok {
                            let mat_path = self
                                .path
                                .append_child(mat_tok.as_str())
                                .unwrap_or_else(|| self.path.clone());
                            return Some(self.build_geom_subset_ds(idx as i32, &mat_path));
                        }
                    }
                }
            }
            return None;
        }
        None
    }

    fn process_dirty_locators(
        &self,
        dirty_locators: &HdDataSourceLocatorSet,
        entries: &mut Vec<DirtiedPrimEntry>,
        needs_refresh: &mut bool,
    ) {
        *needs_refresh = false;

        // Card locators that trigger full refresh
        let card_geometry_locator = HdDataSourceLocator::from_tokens_2(
            tokens::GEOM_MODEL.clone(),
            tokens::CARD_GEOMETRY.clone(),
        );
        let card_texture_locators = [
            HdDataSourceLocator::from_tokens_2(
                tokens::GEOM_MODEL.clone(),
                tokens::CARD_TEXTURE_X_POS.clone(),
            ),
            HdDataSourceLocator::from_tokens_2(
                tokens::GEOM_MODEL.clone(),
                tokens::CARD_TEXTURE_Y_POS.clone(),
            ),
            HdDataSourceLocator::from_tokens_2(
                tokens::GEOM_MODEL.clone(),
                tokens::CARD_TEXTURE_Z_POS.clone(),
            ),
            HdDataSourceLocator::from_tokens_2(
                tokens::GEOM_MODEL.clone(),
                tokens::CARD_TEXTURE_X_NEG.clone(),
            ),
            HdDataSourceLocator::from_tokens_2(
                tokens::GEOM_MODEL.clone(),
                tokens::CARD_TEXTURE_Y_NEG.clone(),
            ),
            HdDataSourceLocator::from_tokens_2(
                tokens::GEOM_MODEL.clone(),
                tokens::CARD_TEXTURE_Z_NEG.clone(),
            ),
        ];

        // Check if cards need full rebuild
        let mut cards_needs_refresh = dirty_locators.contains(&card_geometry_locator);
        if !cards_needs_refresh {
            cards_needs_refresh = card_texture_locators
                .iter()
                .any(|loc| dirty_locators.contains(loc));
        }
        if cards_needs_refresh {
            *needs_refresh = true;
            for path in self.get_prim_paths() {
                entries.push(DirtiedPrimEntry {
                    prim_path: path,
                    dirty_locators: HdDataSourceLocatorSet::empty(),
                });
            }
            return;
        }

        // Check drawModeColor
        let color_locator = HdDataSourceLocator::from_tokens_2(
            tokens::GEOM_MODEL.clone(),
            tokens::DRAW_MODE_COLOR.clone(),
        );
        if dirty_locators.contains(&color_locator) {
            let mut prim_dirty = dirty_locators.clone();
            prim_dirty.insert(HdDataSourceLocator::from_tokens_3(
                tokens::PRIMVARS.clone(),
                tokens::DISPLAY_COLOR.clone(),
                tokens::PRIMVAR_VALUE.clone(),
            ));
            // Dirty the mesh
            if let Some(mesh_path) = self.path.append_child("cardsMesh") {
                entries.push(DirtiedPrimEntry {
                    prim_path: mesh_path,
                    dirty_locators: prim_dirty,
                });
            }
            // Dirty all material prims (color input locators)
            for mat_name in &self.data.material_names {
                if let Some(mat_path) = self.path.append_child(mat_name.as_str()) {
                    entries.push(DirtiedPrimEntry {
                        prim_path: mat_path,
                        dirty_locators: HdDataSourceLocatorSet::empty(),
                    });
                }
            }
            return;
        }

        // Forward dirty to mesh
        if let Some(mesh_path) = self.path.append_child("cardsMesh") {
            entries.push(DirtiedPrimEntry {
                prim_path: mesh_path,
                dirty_locators: dirty_locators.clone(),
            });
        }
    }
}

// =============================================================================
// Factory function
// =============================================================================

/// Create a draw mode standin for the given mode.
/// Port of C++ `UsdImaging_GetDrawModeStandin`.
pub fn create_standin(
    draw_mode: &Token,
    path: &Path,
    prim_source: &Option<HdContainerDataSourceHandle>,
) -> Option<DrawModeStandinHandle> {
    if draw_mode.is_empty() {
        return None;
    }
    if draw_mode == &*tokens::BOUNDS {
        return Some(Arc::new(BoundsStandin::new(
            path.clone(),
            prim_source.clone(),
        )));
    }
    if draw_mode == &*tokens::ORIGIN {
        return Some(Arc::new(OriginStandin::new(
            path.clone(),
            prim_source.clone(),
        )));
    }
    if draw_mode == &*tokens::CARDS {
        return Some(Arc::new(CardsStandin::new(
            path.clone(),
            prim_source.clone(),
        )));
    }
    None
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounds_standin_prim_paths() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = BoundsStandin::new(path.clone(), None);
        let paths = standin.get_prim_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].get_text(), "/World/Model");
        assert_eq!(paths[1].get_text(), "/World/Model/boundsCurves");
    }

    #[test]
    fn test_bounds_standin_prim_types() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = BoundsStandin::new(path.clone(), None);

        // Root has empty type
        let root = standin.get_prim(&path);
        assert!(root.prim_type.is_empty());

        // Child is basisCurves
        let child_path = Path::from_string("/World/Model/boundsCurves").unwrap();
        let child = standin.get_prim(&child_path);
        assert_eq!(child.prim_type.as_str(), "basisCurves");
        assert!(child.data_source.is_some());
    }

    #[test]
    fn test_bounds_points_computation() {
        // With zero extent, all points should be at origin
        let points = BoundsStandin::compute_points(&None);
        assert_eq!(points.len(), 8);
        for pt in &points {
            assert_eq!(*pt, Vec3f::new(0.0, 0.0, 0.0));
        }
    }

    #[test]
    fn test_origin_standin_prim_paths() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = OriginStandin::new(path.clone(), None);
        let paths = standin.get_prim_paths();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0].get_text(), "/World/Model");
        assert_eq!(paths[1].get_text(), "/World/Model/originCurves");
    }

    #[test]
    fn test_origin_standin_prim_types() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = OriginStandin::new(path.clone(), None);

        let child_path = Path::from_string("/World/Model/originCurves").unwrap();
        let child = standin.get_prim(&child_path);
        assert_eq!(child.prim_type.as_str(), "basisCurves");
        assert!(child.data_source.is_some());
    }

    #[test]
    fn test_cards_standin_basic() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = CardsStandin::new(path.clone(), None);

        // Should have at least root + cardsMesh
        let paths = standin.get_prim_paths();
        assert!(paths.len() >= 2);
        assert_eq!(paths[0].get_text(), "/World/Model");
        assert_eq!(paths[1].get_text(), "/World/Model/cardsMesh");
    }

    #[test]
    fn test_cards_standin_mesh_type() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = CardsStandin::new(path.clone(), None);

        let mesh_path = Path::from_string("/World/Model/cardsMesh").unwrap();
        let prim = standin.get_prim(&mesh_path);
        assert_eq!(prim.prim_type.as_str(), "mesh");
        assert!(prim.data_source.is_some());
    }

    #[test]
    fn test_transform_card_point() {
        let v = Vec3f::new(1.0, 0.5, 0.3);

        // X axis: identity
        assert_eq!(transform_card_point(&v, 0), v);

        // Y axis: 90 deg about z
        let ty = transform_card_point(&v, 1);
        assert!((ty[0] - 0.5).abs() < 1e-6);
        assert!((ty[1] - 1.0).abs() < 1e-6);
        assert!((ty[2] - 0.3).abs() < 1e-6);

        // Z axis: 120 deg about space diagonal
        let tz = transform_card_point(&v, 2);
        assert!((tz[0] - 0.5).abs() < 1e-6);
        assert!((tz[1] - 0.3).abs() < 1e-6);
        assert!((tz[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_fill_uvs() {
        let mut uvs = Vec::new();
        fill_uvs(false, false, &mut uvs);
        assert_eq!(uvs.len(), 4);
        assert_eq!(uvs[0], Vec2f::new(1.0, 1.0));
        assert_eq!(uvs[1], Vec2f::new(0.0, 1.0));
        assert_eq!(uvs[2], Vec2f::new(0.0, 0.0));
        assert_eq!(uvs[3], Vec2f::new(1.0, 0.0));
    }

    #[test]
    fn test_fill_uvs_flipped() {
        let mut uvs = Vec::new();
        fill_uvs(true, true, &mut uvs);
        assert_eq!(uvs.len(), 4);
        assert_eq!(uvs[0], Vec2f::new(0.0, 0.0));
        assert_eq!(uvs[1], Vec2f::new(1.0, 0.0));
        assert_eq!(uvs[2], Vec2f::new(1.0, 1.0));
        assert_eq!(uvs[3], Vec2f::new(0.0, 1.0));
    }

    #[test]
    fn test_create_standin_factory() {
        let path = Path::from_string("/Foo").unwrap();

        assert!(create_standin(&Token::empty(), &path, &None).is_none());
        assert!(create_standin(&Token::new("default"), &path, &None).is_none());

        let bounds = create_standin(&Token::new("bounds"), &path, &None);
        assert!(bounds.is_some());
        assert_eq!(bounds.unwrap().get_draw_mode().as_str(), "bounds");

        let origin = create_standin(&Token::new("origin"), &path, &None);
        assert!(origin.is_some());
        assert_eq!(origin.unwrap().get_draw_mode().as_str(), "origin");

        let cards = create_standin(&Token::new("cards"), &path, &None);
        assert!(cards.is_some());
        assert_eq!(cards.unwrap().get_draw_mode().as_str(), "cards");
    }

    #[test]
    fn test_make_relative_path() {
        let base = Path::from_string("/World/Model").unwrap();

        let same = make_relative_path(&base, &base);
        assert_eq!(same.get_text(), ".");

        let child = Path::from_string("/World/Model/boundsCurves").unwrap();
        let rel = make_relative_path(&base, &child);
        assert_eq!(rel.get_text(), "boundsCurves");

        let grandchild = Path::from_string("/World/Model/cardsMesh/subsetXPos").unwrap();
        let rel2 = make_relative_path(&base, &grandchild);
        assert_eq!(rel2.get_text(), "cardsMesh/subsetXPos");
    }

    #[test]
    fn test_cards_default_all_faces() {
        // With no prim source, all 6 faces should be present
        let (_, has_face, _) = CardsData::resolve_card_faces(&None);
        assert_eq!(has_face, [true; 6]);
    }

    #[test]
    fn test_quad_topology() {
        let topo = build_quad_topology(3);
        let names = topo.get_names();
        assert!(names.contains(&tokens::FACE_VERTEX_COUNTS.clone()));
        assert!(names.contains(&tokens::FACE_VERTEX_INDICES.clone()));
        assert!(names.contains(&tokens::ORIENTATION.clone()));
    }

    #[test]
    fn test_apply_extent_to_points() {
        // Unit cube points [0,1]^3 with extent [-1,-1,-1] to [1,1,1]
        let unit_pts = vec![
            Vec3f::new(0.0, 0.0, 0.0),
            Vec3f::new(1.0, 1.0, 1.0),
            Vec3f::new(0.5, 0.5, 0.5),
        ];

        // Build a minimal prim source with extent
        let mut extent_children = HashMap::new();
        extent_children.insert(
            tokens::MIN.clone(),
            HdRetainedTypedSampledDataSource::new(usd_gf::Vec3d::new(-1.0, -1.0, -1.0))
                as HdDataSourceBaseHandle,
        );
        extent_children.insert(
            tokens::MAX.clone(),
            HdRetainedTypedSampledDataSource::new(usd_gf::Vec3d::new(1.0, 1.0, 1.0))
                as HdDataSourceBaseHandle,
        );
        let extent_ds = HdRetainedContainerDataSource::new(extent_children);

        let mut root_children = HashMap::new();
        root_children.insert(tokens::EXTENT.clone(), extent_ds as HdDataSourceBaseHandle);
        let prim_source: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::new(root_children);

        let result = apply_extent_to_points(&unit_pts, &Some(prim_source));
        assert_eq!(result.len(), 3);
        // (0,0,0) -> min=(-1,-1,-1)
        assert!((result[0][0] - (-1.0)).abs() < 1e-6);
        // (1,1,1) -> max=(1,1,1)
        assert!((result[1][0] - 1.0).abs() < 1e-6);
        // (0.5,0.5,0.5) -> midpoint=(0,0,0)
        assert!((result[2][0]).abs() < 1e-6);
    }

    #[test]
    fn test_bounds_added_entries() {
        let path = Path::from_string("/World/Model").unwrap();
        let standin = BoundsStandin::new(path.clone(), None);
        let mut entries = Vec::new();
        standin.compute_prim_added_entries(&mut entries);
        assert_eq!(entries.len(), 2);
        // Root: empty prim type
        assert!(entries[0].prim_type.is_empty());
        assert_eq!(entries[0].prim_path.get_text(), "/World/Model");
        // Child: basisCurves
        assert_eq!(entries[1].prim_type.as_str(), "basisCurves");
        assert_eq!(entries[1].prim_path.get_text(), "/World/Model/boundsCurves");
    }
}
