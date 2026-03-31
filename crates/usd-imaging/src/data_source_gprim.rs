//! Gprim data source for USD imaging.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceGprim.h/cpp
//!
//! Provides data source for geometric primitives (Gprim) which extends
//! DataSourcePrim with custom primvar mappings for PointBased attributes
//! (points, velocities, accelerations, normals) and Curves (widths).
//!
//! Get("primvars") overlays custom primvar mappings onto the base primvars
//! data source, so "primvars:normals" takes priority over "normals".

use super::data_source_prim::DataSourcePrim;
use super::data_source_primvars::{DataSourceCustomPrimvars, DataSourcePrimvars, PrimvarMapping};
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

/// Data source representing a USD gprim.
///
/// Extends DataSourcePrim with geometry-specific behavior:
/// - Custom primvar mappings for PointBased attributes (points, velocities,
///   accelerations, normals) and Curves (widths)
/// - Get("primvars") overlays custom mappings (weaker) under the base
///   primvars data source (stronger), so "primvars:normals" wins over "normals"
///
/// # Hydra Data Structure
///
/// A gprim data source contains:
/// - `primvars` - Primvar data (displayColor, displayOpacity, etc.)
/// - `materialBindings` - Material binding relationships
/// - `xform` - Transform matrix
/// - `visibility` - Visibility state
/// - `purpose` - Render purpose (default, render, proxy, guide)
/// - `extent` - Bounding box extent
/// - `doubleSided` - Whether to render both sides
#[derive(Clone)]
pub struct DataSourceGprim {
    /// Base prim data source
    base: DataSourcePrim,
    /// Scene index path for constructing child data sources
    scene_index_path: Path,
    /// USD prim reference for type checks and attribute access
    prim: Prim,
    /// Stage globals for time context
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceGprim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceGprim")
            .field("base", &self.base)
            .finish()
    }
}

// Token constants for gprim data source names
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static MATERIAL_BINDINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("materialBindings"));
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static VISIBILITY: LazyLock<Token> = LazyLock::new(|| Token::new("visibility"));
    pub static PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new("purpose"));
    pub static EXTENT: LazyLock<Token> = LazyLock::new(|| Token::new("extent"));
    pub static DOUBLE_SIDED: LazyLock<Token> = LazyLock::new(|| Token::new("doubleSided"));
    pub static MODEL: LazyLock<Token> = LazyLock::new(|| Token::new("model"));

    // PointBased attribute names
    pub static POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("points"));
    pub static VELOCITIES: LazyLock<Token> = LazyLock::new(|| Token::new("velocities"));
    pub static ACCELERATIONS: LazyLock<Token> = LazyLock::new(|| Token::new("accelerations"));
    pub static NORMALS: LazyLock<Token> = LazyLock::new(|| Token::new("normals"));
    pub static WIDTHS: LazyLock<Token> = LazyLock::new(|| Token::new("widths"));

    // Motion blur tokens
    pub static NON_LINEAR_SAMPLE_COUNT: LazyLock<Token> =
        LazyLock::new(|| Token::new("nonlinearSampleCount"));
    pub static BLUR_SCALE: LazyLock<Token> = LazyLock::new(|| Token::new("blurScale"));
    pub static MOTION_NONLINEAR_SAMPLE_COUNT: LazyLock<Token> =
        LazyLock::new(|| Token::new("motionNonlinearSampleCount"));
    pub static MOTION_BLUR_SCALE: LazyLock<Token> = LazyLock::new(|| Token::new("motionBlurScale"));

    // Schema type names for IsA checks
    pub static POINT_BASED: LazyLock<Token> = LazyLock::new(|| Token::new("PointBased"));
    pub static CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("BasisCurves"));
    pub static NURBS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("NurbsCurves"));
}

/// Returns custom primvar mappings for the given prim based on its type.
///
/// PointBased prims get mappings for points, velocities, accelerations,
/// nonlinearSampleCount, blurScale, and normals.
/// Curves prims additionally get widths.
///
/// Matches C++ `_GetCustomPrimvarMappings()`.
fn get_custom_primvar_mappings(prim: &Prim) -> Vec<PrimvarMapping> {
    if !prim.is_valid() {
        return vec![];
    }

    // Check if prim IsA PointBased
    if prim.is_a(&tokens::POINT_BASED) {
        let mut mappings = vec![
            PrimvarMapping::new(tokens::POINTS.clone(), tokens::POINTS.clone()),
            PrimvarMapping::new(tokens::VELOCITIES.clone(), tokens::VELOCITIES.clone()),
            PrimvarMapping::new(tokens::ACCELERATIONS.clone(), tokens::ACCELERATIONS.clone()),
            PrimvarMapping::new(
                tokens::NON_LINEAR_SAMPLE_COUNT.clone(),
                tokens::MOTION_NONLINEAR_SAMPLE_COUNT.clone(),
            ),
            PrimvarMapping::new(
                tokens::BLUR_SCALE.clone(),
                tokens::MOTION_BLUR_SCALE.clone(),
            ),
            PrimvarMapping::new(tokens::NORMALS.clone(), tokens::NORMALS.clone()),
        ];

        // Curves get additional "widths" mapping
        if prim.is_a(&tokens::CURVES) || prim.is_a(&tokens::NURBS_CURVES) {
            mappings.push(PrimvarMapping::new(
                tokens::WIDTHS.clone(),
                tokens::WIDTHS.clone(),
            ));
        }

        return mappings;
    }

    vec![]
}

impl DataSourceGprim {
    /// Create new gprim data source.
    ///
    /// Stores the prim and stage globals for lazy custom primvar construction
    /// in Get("primvars").
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            base: DataSourcePrim::new(
                prim.clone(),
                scene_index_path.clone(),
                stage_globals.clone(),
            ),
            scene_index_path,
            prim,
            stage_globals,
        })
    }

    /// Get the wrapped prim.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Get the scene index path.
    pub fn scene_index_path(&self) -> &Path {
        &self.scene_index_path
    }

    /// Get the stage globals.
    pub fn stage_globals(&self) -> &DataSourceStageGlobalsHandle {
        &self.stage_globals
    }

    /// Compute invalidation locators for property changes.
    ///
    /// Combines base DataSourcePrim invalidation with custom primvar
    /// mapping invalidation for PointBased attributes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Start with base prim invalidation (xform, visibility, purpose, extent)
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);

        // Add primvar-specific invalidation
        locators.insert_set(&DataSourcePrimvars::invalidate(properties));

        // Add custom primvar mapping invalidation for PointBased attributes
        if subprim.is_empty() {
            let mappings = get_custom_primvar_mappings(prim);
            if !mappings.is_empty() {
                locators.insert_set(&DataSourceCustomPrimvars::invalidate(properties, &mappings));
            }
        }

        // Gprim-specific property mappings
        for prop in properties {
            let prop_str = prop.as_str();
            match prop_str {
                "doubleSided" => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new("doubleSided")));
                }
                name if name.starts_with("material:binding") => {
                    locators.insert(HdDataSourceLocator::from_token(Token::new(
                        "materialBindings",
                    )));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceGprim {
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

impl HdContainerDataSource for DataSourceGprim {
    fn get_names(&self) -> Vec<Token> {
        // Combine base prim names with gprim-specific names
        let mut names = self.base.get_names();

        // Add gprim-specific names if not already present
        let gprim_names = [
            &*tokens::PRIMVARS,
            &*tokens::MATERIAL_BINDINGS,
            &*tokens::XFORM,
            &*tokens::VISIBILITY,
            &*tokens::PURPOSE,
            &*tokens::EXTENT,
            &*tokens::DOUBLE_SIDED,
            &*tokens::MODEL,
        ];
        for name in gprim_names {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }

        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if !self.prim.is_valid() {
            return None;
        }

        // First get base result (handles xform, visibility, purpose, etc.)
        let base_result = self.base.get(name);

        // For "primvars", overlay custom mappings onto base primvars
        if name == &*tokens::PRIMVARS {
            let mappings = get_custom_primvar_mappings(&self.prim);

            // If no custom mappings, return base primvars (or create default)
            if mappings.is_empty() {
                return base_result.or_else(|| {
                    Some(Arc::new(DataSourcePrimvars::new(
                        self.scene_index_path.clone(),
                        self.prim.clone(),
                        self.stage_globals.clone(),
                    )) as HdDataSourceBaseHandle)
                });
            }

            // Build the base primvars container
            let base_primvars: HdContainerDataSourceHandle = match base_result {
                Some(ref ds) => {
                    // Try to downcast to container
                    if let Some(container) = ds.as_any().downcast_ref::<DataSourcePrimvars>() {
                        Arc::new(container.clone()) as HdContainerDataSourceHandle
                    } else {
                        Arc::new(DataSourcePrimvars::new(
                            self.scene_index_path.clone(),
                            self.prim.clone(),
                            self.stage_globals.clone(),
                        )) as HdContainerDataSourceHandle
                    }
                }
                None => Arc::new(DataSourcePrimvars::new(
                    self.scene_index_path.clone(),
                    self.prim.clone(),
                    self.stage_globals.clone(),
                )) as HdContainerDataSourceHandle,
            };

            // Build custom primvars container (weaker priority)
            let custom_primvars: HdContainerDataSourceHandle =
                Arc::new(DataSourceCustomPrimvars::new(
                    self.scene_index_path.clone(),
                    self.prim.clone(),
                    mappings,
                    self.stage_globals.clone(),
                ));

            // Overlay: base primvars (stronger) over custom (weaker).
            // "primvars:normals" takes priority over "normals" attribute.
            return Some(
                HdOverlayContainerDataSource::new_2(base_primvars, custom_primvars)
                    as HdDataSourceBaseHandle,
            );
        }

        base_result
    }
}

/// Handle type for DataSourceGprim
pub type DataSourceGprimHandle = Arc<DataSourceGprim>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_data_source_gprim_creation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestGprim").unwrap();
        let globals = create_test_globals();

        let ds = DataSourceGprim::new(path.clone(), prim, globals);
        assert_eq!(ds.scene_index_path(), &path);
    }

    #[test]
    fn test_data_source_gprim_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let path = Path::from_string("/TestGprim").unwrap();
        let globals = create_test_globals();

        let ds = DataSourceGprim::new(path, prim, globals);
        let names = ds.get_names();

        assert!(names.iter().any(|n| n == "primvars"));
        assert!(names.iter().any(|n| n == "xform"));
        assert!(names.iter().any(|n| n == "visibility"));
        assert!(names.iter().any(|n| n == "doubleSided"));
        assert!(names.iter().any(|n| n == "materialBindings"));
    }

    #[test]
    fn test_invalidation_points() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("points")];

        let locators = DataSourceGprim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        // Pseudo root is not PointBased, so no custom primvar mapping
        // but the base DataSourcePrim doesn't handle "points" either
        // (it only handles xform, visibility, purpose, extent, proxyPrim)
        assert!(locators.is_empty());
    }

    #[test]
    fn test_invalidation_primvar() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("primvars:displayColor")];

        let locators = DataSourceGprim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_invalidation_double_sided() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("doubleSided")];

        let locators = DataSourceGprim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_invalidation_material_binding() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("material:binding")];

        let locators = DataSourceGprim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_custom_primvar_mappings_empty() {
        // Non-PointBased prim should have no mappings
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let mappings = get_custom_primvar_mappings(&prim);
        assert!(mappings.is_empty());
    }

    #[test]
    fn test_get_primvars_returns_container() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();
        let path = Path::from_string("/TestGprim").unwrap();

        let ds = DataSourceGprim::new(path, prim, globals);
        // Should return a primvars container even for pseudo root
        let primvars = ds.get(&Token::new("primvars"));
        assert!(primvars.is_some());
    }
}
