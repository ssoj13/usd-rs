//! LightAdapter - Base adapter for all USD lights.
//!
//! Port of pxr/usdImaging/usdImaging/lightAdapter.h/cpp
//!
//! Provides base imaging support for UsdLux light prims including:
//! - Light parameters (intensity, color, exposure, etc.)
//! - Transform handling
//! - Visibility
//! - Light linking collections

use super::data_source_attribute::DataSourceAttribute;
use super::data_source_prim::DataSourcePrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use usd_core::{Prim, collection_api::CollectionAPI, prim_flags::PrimFlagsPredicate};
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedSampledDataSource,
    HdRetainedSmallVectorDataSource, HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

// Token constants for lights
#[allow(dead_code)] // Tokens defined for future use
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Light types
    pub static LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("light"));
    pub static SPHERE_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("sphereLight"));
    pub static DOME_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("domeLight"));
    pub static RECT_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("rectLight"));
    pub static DISTANT_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("distantLight"));
    pub static DISK_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("diskLight"));
    pub static CYLINDER_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("cylinderLight"));
    pub static GEOMETRY_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("geometryLight"));
    pub static PLUGIN_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("pluginLight"));
    pub static LIGHT_FILTER: LazyLock<Token> = LazyLock::new(|| Token::new("lightFilter"));
    pub static PLUGIN_LIGHT_FILTER: LazyLock<Token> =
        LazyLock::new(|| Token::new("pluginLightFilter"));

    // Light parameters
    pub static INTENSITY: LazyLock<Token> = LazyLock::new(|| Token::new("intensity"));
    pub static EXPOSURE: LazyLock<Token> = LazyLock::new(|| Token::new("exposure"));
    pub static COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("color"));
    pub static DIFFUSE: LazyLock<Token> = LazyLock::new(|| Token::new("diffuse"));
    pub static SPECULAR: LazyLock<Token> = LazyLock::new(|| Token::new("specular"));
    pub static NORMALIZE: LazyLock<Token> = LazyLock::new(|| Token::new("normalize"));
    pub static ENABLE_COLOR_TEMPERATURE: LazyLock<Token> =
        LazyLock::new(|| Token::new("enableColorTemperature"));
    pub static COLOR_TEMPERATURE: LazyLock<Token> =
        LazyLock::new(|| Token::new("colorTemperature"));

    // Light-specific parameters
    pub static RADIUS: LazyLock<Token> = LazyLock::new(|| Token::new("radius"));
    pub static WIDTH: LazyLock<Token> = LazyLock::new(|| Token::new("width"));
    pub static HEIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("height"));
    pub static LENGTH: LazyLock<Token> = LazyLock::new(|| Token::new("length"));
    pub static ANGLE: LazyLock<Token> = LazyLock::new(|| Token::new("angle"));
    pub static TEXTURE_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("texture:file"));
    pub static TEXTURE_FORMAT: LazyLock<Token> = LazyLock::new(|| Token::new("textureFormat"));

    // Shadow parameters
    pub static SHADOW_ENABLE: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:enable"));
    pub static SHADOW_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:color"));
    pub static SHADOW_DISTANCE: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:distance"));
    pub static SHADOW_FALLOFF: LazyLock<Token> = LazyLock::new(|| Token::new("shadow:falloff"));
    pub static SHADOW_FALLOFF_GAMMA: LazyLock<Token> =
        LazyLock::new(|| Token::new("shadow:falloffGamma"));

    // Shaping parameters
    pub static SHAPING_FOCUS: LazyLock<Token> = LazyLock::new(|| Token::new("shaping:focus"));
    pub static SHAPING_FOCUS_TINT: LazyLock<Token> =
        LazyLock::new(|| Token::new("shaping:focusTint"));
    pub static SHAPING_CONE_ANGLE: LazyLock<Token> =
        LazyLock::new(|| Token::new("shaping:cone:angle"));
    pub static SHAPING_CONE_SOFTNESS: LazyLock<Token> =
        LazyLock::new(|| Token::new("shaping:cone:softness"));
    pub static SHAPING_IES_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("shaping:ies:file"));
    pub static SHAPING_IES_ANGLE_SCALE: LazyLock<Token> =
        LazyLock::new(|| Token::new("shaping:ies:angleScale"));
    pub static SHAPING_IES_NORMALIZE: LazyLock<Token> =
        LazyLock::new(|| Token::new("shaping:ies:normalize"));

    // Collections
    pub static COLLECTION_LIGHT_LINK: LazyLock<Token> =
        LazyLock::new(|| Token::new("collection:lightLink"));
    pub static COLLECTION_SHADOW_LINK: LazyLock<Token> =
        LazyLock::new(|| Token::new("collection:shadowLink"));

    // Data source locators
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static VISIBILITY: LazyLock<Token> = LazyLock::new(|| Token::new("visibility"));
}

/// Global setting for scene lights enabled state.
static SCENE_LIGHTS_ENABLED: AtomicBool = AtomicBool::new(true);

/// Check if scene lights are enabled.
pub fn is_scene_lights_enabled() -> bool {
    SCENE_LIGHTS_ENABLED.load(Ordering::Relaxed)
}

/// Set whether scene lights are enabled.
pub fn set_scene_lights_enabled(enabled: bool) {
    SCENE_LIGHTS_ENABLED.store(enabled, Ordering::Relaxed);
}

// ============================================================================
// DataSourceLightCollection
// ============================================================================

/// Data source for light linking collections (lightLink / shadowLink).
///
/// Resolves UsdCollectionAPI collections on a light prim and returns the
/// set of included prim paths as a sampled data source for Hydra.
///
/// C++ counterpart: UsdImaging uses HdCollectionSchema for light linking.
#[derive(Clone)]
pub struct DataSourceLightCollection {
    prim: Prim,
    /// "lightLink" or "shadowLink"
    collection_name: Token,
}

impl std::fmt::Debug for DataSourceLightCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceLightCollection")
            .field("collection", &self.collection_name)
            .finish()
    }
}

impl DataSourceLightCollection {
    /// Create a new collection data source.
    pub fn new(prim: Prim, collection_name: Token) -> Arc<Self> {
        Arc::new(Self {
            prim,
            collection_name,
        })
    }

    /// Resolve the collection and return included paths.
    ///
    /// Uses UsdCollectionAPI to compute the membership query and then
    /// retrieves all included object paths from the stage.
    fn resolve_included_paths(&self) -> Vec<Path> {
        let api = CollectionAPI::get_from_prim(&self.prim, &self.collection_name);
        if !api.is_valid() {
            // Collection not applied — light links all prims by default
            return Vec::new();
        }

        // Compute membership query (resolves includes/excludes relationships)
        let query = api.compute_membership_query();

        // Get the stage to enumerate paths
        let Some(stage) = self.prim.stage() else {
            return Vec::new();
        };

        // Use default predicate (active, defined prims)
        let pred = PrimFlagsPredicate::default();
        CollectionAPI::compute_included_paths(&query, &stage, pred)
            .into_iter()
            .collect()
    }
}

impl HdDataSourceBase for DataSourceLightCollection {
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

impl HdContainerDataSource for DataSourceLightCollection {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("includedPaths"), Token::new("name")]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        match name.as_str() {
            "includedPaths" => {
                let paths = self.resolve_included_paths();
                let elements: Vec<HdDataSourceBaseHandle> = paths
                    .iter()
                    .map(|p| {
                        HdRetainedTypedSampledDataSource::new(p.clone()) as HdDataSourceBaseHandle
                    })
                    .collect();
                Some(HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle)
            }
            "name" => Some(HdRetainedSampledDataSource::new(usd_vt::Value::from(
                self.collection_name.as_str().to_string(),
            )) as HdDataSourceBaseHandle),
            _ => None,
        }
    }
}

// ============================================================================
// DataSourceLight
// ============================================================================

/// Data source for light parameters.
///
/// Reads UsdLux light attributes (inputs:intensity, inputs:color, etc.)
/// and exposes them to Hydra with the "inputs:" prefix stripped.
#[derive(Clone)]
pub struct DataSourceLight {
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: Path,
    light_type: Token,
}

impl std::fmt::Debug for DataSourceLight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceLight")
            .field("light_type", &self.light_type)
            .finish()
    }
}

impl DataSourceLight {
    /// Create new light data source.
    pub fn new(
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        scene_index_path: Path,
        light_type: Token,
    ) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
            scene_index_path,
            light_type,
        })
    }

    /// Map Hydra token name to USD attribute name.
    ///
    /// UsdLux stores light params as "inputs:X" but Hydra expects bare "X".
    /// Shadow/shaping params like "shadow:enable" map to "inputs:shadow:enable".
    fn usd_attr_name(hydra_name: &str) -> String {
        format!("inputs:{}", hydra_name)
    }

    /// Get light-specific parameter names based on light type.
    fn get_type_specific_params(&self) -> Vec<Token> {
        match self.light_type.as_str() {
            "sphereLight" => vec![tokens::RADIUS.clone()],
            "rectLight" => vec![tokens::WIDTH.clone(), tokens::HEIGHT.clone()],
            "diskLight" => vec![tokens::RADIUS.clone()],
            "cylinderLight" => vec![tokens::RADIUS.clone(), tokens::LENGTH.clone()],
            "distantLight" => vec![tokens::ANGLE.clone()],
            "domeLight" => vec![tokens::TEXTURE_FILE.clone(), tokens::TEXTURE_FORMAT.clone()],
            _ => vec![],
        }
    }
}

impl HdDataSourceBase for DataSourceLight {
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

impl HdContainerDataSource for DataSourceLight {
    fn get_names(&self) -> Vec<Token> {
        let mut names = vec![
            // Common light parameters
            tokens::INTENSITY.clone(),
            tokens::EXPOSURE.clone(),
            tokens::COLOR.clone(),
            tokens::DIFFUSE.clone(),
            tokens::SPECULAR.clone(),
            tokens::NORMALIZE.clone(),
            tokens::ENABLE_COLOR_TEMPERATURE.clone(),
            tokens::COLOR_TEMPERATURE.clone(),
            // Shadow parameters
            tokens::SHADOW_ENABLE.clone(),
            tokens::SHADOW_COLOR.clone(),
            // Shaping parameters
            tokens::SHAPING_FOCUS.clone(),
            tokens::SHAPING_CONE_ANGLE.clone(),
            tokens::SHAPING_CONE_SOFTNESS.clone(),
        ];
        // Add type-specific parameters
        names.extend(self.get_type_specific_params());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Map Hydra token -> USD attribute name (prepend "inputs:")
        let usd_name = Self::usd_attr_name(name.as_str());
        let attr = self.prim.get_attribute(&usd_name)?;

        if !attr.is_valid() {
            return None;
        }

        Some(DataSourceAttribute::<Value>::new(
            attr,
            self.stage_globals.clone(),
            self.scene_index_path.clone(),
        ) as HdDataSourceBaseHandle)
    }
}

// ============================================================================
// DataSourceLightPrim
// ============================================================================

/// Prim data source for light prims.
#[derive(Clone)]
pub struct DataSourceLightPrim {
    #[allow(dead_code)] // For future path-based operations
    scene_index_path: Path,
    prim_ds: Arc<DataSourcePrim>,
    light_ds: Arc<DataSourceLight>,
    /// lightLink collection data source (linked geometry)
    light_link_ds: Arc<DataSourceLightCollection>,
    /// shadowLink collection data source (shadow-casting geometry)
    shadow_link_ds: Arc<DataSourceLightCollection>,
    #[allow(dead_code)] // For future type-specific handling
    light_type: Token,
}

impl std::fmt::Debug for DataSourceLightPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceLightPrim")
            .field("light_type", &self.light_type)
            .finish()
    }
}

impl DataSourceLightPrim {
    /// Create new light prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
        light_type: Token,
    ) -> Arc<Self> {
        // Note: DataSourcePrim uses (prim, hydra_path, stage_globals) order
        let prim_ds = Arc::new(DataSourcePrim::new(
            prim.clone(),
            scene_index_path.clone(),
            stage_globals.clone(),
        ));
        let light_ds = DataSourceLight::new(
            prim.clone(),
            stage_globals,
            scene_index_path.clone(),
            light_type.clone(),
        );
        // Build collection data sources for light/shadow linking.
        let light_link_ds = DataSourceLightCollection::new(prim.clone(), Token::new("lightLink"));
        let shadow_link_ds = DataSourceLightCollection::new(prim, Token::new("shadowLink"));

        Arc::new(Self {
            scene_index_path,
            prim_ds,
            light_ds,
            light_link_ds,
            shadow_link_ds,
            light_type,
        })
    }

    /// Compute invalidation for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Start with base prim invalidation
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);

        for prop in properties {
            let prop_str = prop.as_str();

            // Check for xform changes
            if prop_str == "xformOpOrder" || prop_str.starts_with("xformOp:") {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    tokens::XFORM.clone(),
                    Token::new("matrix"),
                ));
                continue;
            }

            // Light parameter changes
            match prop_str {
                "intensity"
                | "exposure"
                | "color"
                | "diffuse"
                | "specular"
                | "normalize"
                | "enableColorTemperature"
                | "colorTemperature"
                | "radius"
                | "width"
                | "height"
                | "length"
                | "angle"
                | "textureFile"
                | "textureFormat" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::LIGHT.clone(),
                        prop.clone(),
                    ));
                }
                // Shadow parameters
                s if s.starts_with("shadow:") => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::LIGHT.clone(),
                        prop.clone(),
                    ));
                }
                // Shaping parameters
                s if s.starts_with("shaping:") => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::LIGHT.clone(),
                        prop.clone(),
                    ));
                }
                // Collection changes
                s if s.starts_with("collection:lightLink")
                    || s.starts_with("collection:shadowLink") =>
                {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::LIGHT.clone(),
                        Token::new("collection"),
                    ));
                }
                "visibility" => {
                    locators.insert(HdDataSourceLocator::from_token(tokens::VISIBILITY.clone()));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceLightPrim {
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

impl HdContainerDataSource for DataSourceLightPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.prim_ds.get_names();
        names.push(tokens::LIGHT.clone());
        // Expose light-linking collections as top-level data source keys
        names.push(tokens::COLLECTION_LIGHT_LINK.clone());
        names.push(tokens::COLLECTION_SHADOW_LINK.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::LIGHT {
            return Some(Arc::clone(&self.light_ds) as HdDataSourceBaseHandle);
        }
        // Return resolved collection data sources for light linking
        if *name == *tokens::COLLECTION_LIGHT_LINK {
            return Some(Arc::clone(&self.light_link_ds) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::COLLECTION_SHADOW_LINK {
            return Some(Arc::clone(&self.shadow_link_ds) as HdDataSourceBaseHandle);
        }
        self.prim_ds.get(name)
    }
}

// ============================================================================
// LightAdapter - Base adapter for all lights
// ============================================================================

/// Base adapter for all USD light prims.
///
/// Converts UsdLux light types to Hydra light primitives with:
/// - Light parameters (intensity, color, exposure)
/// - Shadow parameters
/// - Shaping parameters (for spot/area lights)
/// - Light linking collections
#[derive(Debug, Clone)]
pub struct LightAdapter {
    /// The Hydra prim type for this light
    light_type: Token,
}

impl Default for LightAdapter {
    fn default() -> Self {
        Self::new(tokens::LIGHT.clone())
    }
}

impl LightAdapter {
    /// Create a new light adapter with specified light type.
    pub fn new(light_type: Token) -> Self {
        Self { light_type }
    }

    /// Check if scene lights are enabled.
    pub fn is_enabled_scene_lights() -> bool {
        is_scene_lights_enabled()
    }
}

impl PrimAdapter for LightAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            self.light_type.clone()
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
            Some(DataSourceLightPrim::new(
                prim.path().clone(),
                prim.clone(),
                stage_globals.clone(),
                self.light_type.clone(),
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
        DataSourceLightPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Specific Light Adapters
// ============================================================================

/// Adapter for UsdLuxSphereLight prims.
#[derive(Debug, Clone, Default)]
pub struct SphereLightAdapter {
    base: LightAdapter,
}

impl SphereLightAdapter {
    /// Create a new sphere light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::SPHERE_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for SphereLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdLuxDomeLight prims.
#[derive(Debug, Clone, Default)]
pub struct DomeLightAdapter {
    base: LightAdapter,
}

impl DomeLightAdapter {
    /// Create a new dome light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::DOME_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for DomeLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdLuxRectLight prims.
#[derive(Debug, Clone, Default)]
pub struct RectLightAdapter {
    base: LightAdapter,
}

impl RectLightAdapter {
    /// Create a new rect light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::RECT_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for RectLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdLuxDistantLight prims.
#[derive(Debug, Clone, Default)]
pub struct DistantLightAdapter {
    base: LightAdapter,
}

impl DistantLightAdapter {
    /// Create a new distant light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::DISTANT_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for DistantLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdLuxDiskLight prims.
#[derive(Debug, Clone, Default)]
pub struct DiskLightAdapter {
    base: LightAdapter,
}

impl DiskLightAdapter {
    /// Create a new disk light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::DISK_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for DiskLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdLuxCylinderLight prims.
#[derive(Debug, Clone, Default)]
pub struct CylinderLightAdapter {
    base: LightAdapter,
}

impl CylinderLightAdapter {
    /// Create a new cylinder light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::CYLINDER_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for CylinderLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for UsdLuxGeometryLight prims.
#[derive(Debug, Clone, Default)]
pub struct GeometryLightAdapter {
    base: LightAdapter,
}

impl GeometryLightAdapter {
    /// Create a new geometry light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::GEOMETRY_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for GeometryLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for plugin-defined lights.
#[derive(Debug, Clone, Default)]
pub struct PluginLightAdapter {
    base: LightAdapter,
}

impl PluginLightAdapter {
    /// Create a new plugin light adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::PLUGIN_LIGHT.clone()),
        }
    }
}

impl PrimAdapter for PluginLightAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Light Filter Adapters
// ============================================================================

/// Adapter for UsdLux light filter prims.
#[derive(Debug, Clone, Default)]
pub struct LightFilterAdapter {
    base: LightAdapter,
}

impl LightFilterAdapter {
    /// Create a new light filter adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::LIGHT_FILTER.clone()),
        }
    }
}

impl PrimAdapter for LightFilterAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Adapter for plugin-defined light filters.
#[derive(Debug, Clone, Default)]
pub struct PluginLightFilterAdapter {
    base: LightAdapter,
}

impl PluginLightFilterAdapter {
    /// Create a new plugin light filter adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(tokens::PLUGIN_LIGHT_FILTER.clone()),
        }
    }
}

impl PrimAdapter for PluginLightFilterAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

// ============================================================================
// Factory functions
// ============================================================================

/// Factory for creating light adapters.
pub fn create_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(LightAdapter::default())
}

/// Factory for creating sphere light adapters.
pub fn create_sphere_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(SphereLightAdapter::new())
}

/// Factory for creating dome light adapters.
pub fn create_dome_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(DomeLightAdapter::new())
}

/// Factory for creating rect light adapters.
pub fn create_rect_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(RectLightAdapter::new())
}

/// Factory for creating distant light adapters.
pub fn create_distant_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(DistantLightAdapter::new())
}

/// Factory for creating disk light adapters.
pub fn create_disk_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(DiskLightAdapter::new())
}

/// Factory for creating cylinder light adapters.
pub fn create_cylinder_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(CylinderLightAdapter::new())
}

/// Factory for creating geometry light adapters.
pub fn create_geometry_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(GeometryLightAdapter::new())
}

/// Factory for creating plugin light adapters.
pub fn create_plugin_light_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PluginLightAdapter::new())
}

/// Factory for creating light filter adapters.
pub fn create_light_filter_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(LightFilterAdapter::new())
}

/// Factory for creating plugin light filter adapters.
pub fn create_plugin_light_filter_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PluginLightFilterAdapter::new())
}

// ============================================================================
// DomeLight_1Adapter - Modern dome light with improved portals
// ============================================================================

/// Adapter for UsdLuxDomeLight_1 (modern dome light variant).
///
/// DomeLight_1 is an updated version of DomeLight with improved
/// portal support and better texture handling.
#[derive(Debug, Clone)]
pub struct DomeLight1Adapter {
    base: LightAdapter,
}

impl Default for DomeLight1Adapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DomeLight1Adapter {
    /// Create a new dome light 1 adapter.
    pub fn new() -> Self {
        Self {
            base: LightAdapter::new(Token::new("domeLight_1")),
        }
    }
}

impl PrimAdapter for DomeLight1Adapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        self.base.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        self.base.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.base
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.base
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Factory for creating dome light 1 adapters.
pub fn create_dome_light_1_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(DomeLight1Adapter::new())
}

// ============================================================================
// Type aliases
// ============================================================================

/// Handle for LightAdapter.
pub type LightAdapterHandle = Arc<LightAdapter>;
/// Handle for SphereLightAdapter.
pub type SphereLightAdapterHandle = Arc<SphereLightAdapter>;
/// Handle for DomeLightAdapter.
pub type DomeLightAdapterHandle = Arc<DomeLightAdapter>;
/// Handle for RectLightAdapter.
pub type RectLightAdapterHandle = Arc<RectLightAdapter>;
/// Handle for DistantLightAdapter.
pub type DistantLightAdapterHandle = Arc<DistantLightAdapter>;
/// Handle for DiskLightAdapter.
pub type DiskLightAdapterHandle = Arc<DiskLightAdapter>;
/// Handle for CylinderLightAdapter.
pub type CylinderLightAdapterHandle = Arc<CylinderLightAdapter>;
/// Handle for GeometryLightAdapter.
pub type GeometryLightAdapterHandle = Arc<GeometryLightAdapter>;
/// Handle for PluginLightAdapter.
pub type PluginLightAdapterHandle = Arc<PluginLightAdapter>;
/// Handle for LightFilterAdapter.
pub type LightFilterAdapterHandle = Arc<LightFilterAdapter>;
/// Handle for PluginLightFilterAdapter.
pub type PluginLightFilterAdapterHandle = Arc<PluginLightFilterAdapter>;
/// Handle for DomeLight1Adapter.
pub type DomeLight1AdapterHandle = Arc<DomeLight1Adapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_light_adapter_creation() {
        let adapter = LightAdapter::default();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "light");
    }

    #[test]
    fn test_sphere_light_adapter() {
        let adapter = SphereLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "sphereLight");
    }

    #[test]
    fn test_dome_light_adapter() {
        let adapter = DomeLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "domeLight");
    }

    #[test]
    fn test_rect_light_adapter() {
        let adapter = RectLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "rectLight");
    }

    #[test]
    fn test_distant_light_adapter() {
        let adapter = DistantLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "distantLight");
    }

    #[test]
    fn test_disk_light_adapter() {
        let adapter = DiskLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "diskLight");
    }

    #[test]
    fn test_cylinder_light_adapter() {
        let adapter = CylinderLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "cylinderLight");
    }

    #[test]
    fn test_light_adapter_subprims() {
        let adapter = SphereLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_light_adapter_data_source() {
        let adapter = SphereLightAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_light_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![
            Token::new("intensity"),
            Token::new("color"),
            Token::new("xformOp:translate"),
        ];

        let locators = DataSourceLightPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_scene_lights_enabled() {
        // Default should be true
        assert!(is_scene_lights_enabled());

        // Toggle
        set_scene_lights_enabled(false);
        assert!(!is_scene_lights_enabled());

        // Reset
        set_scene_lights_enabled(true);
        assert!(is_scene_lights_enabled());
    }

    #[test]
    fn test_light_data_source_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceLight::new(
            prim,
            globals,
            Path::absolute_root(),
            tokens::SPHERE_LIGHT.clone(),
        );
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "intensity"));
        assert!(names.iter().any(|n| n == "color"));
        assert!(names.iter().any(|n| n == "exposure"));
        assert!(names.iter().any(|n| n == "radius")); // sphere-specific
    }

    #[test]
    fn test_all_light_factories() {
        let _ = create_light_adapter();
        let _ = create_sphere_light_adapter();
        let _ = create_dome_light_adapter();
        let _ = create_rect_light_adapter();
        let _ = create_distant_light_adapter();
        let _ = create_disk_light_adapter();
        let _ = create_cylinder_light_adapter();
        let _ = create_geometry_light_adapter();
        let _ = create_plugin_light_adapter();
        let _ = create_light_filter_adapter();
        let _ = create_plugin_light_filter_adapter();
    }

    #[test]
    fn test_light_filter_adapter() {
        let adapter = LightFilterAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "lightFilter");
    }
}
