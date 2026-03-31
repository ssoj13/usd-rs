//! DataSourcePoints - Points prim data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourcePoints.h/cpp
//!
//! Points prims extend Gprim with a custom primvar overlay for "widths".
//! Unlike mesh or curves, there is no separate schema container - just
//! the gprim with widths exposed as a primvar via overlay.

use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_primvars::{DataSourceCustomPrimvars, PrimvarMapping};
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocatorSet,
    HdOverlayContainerDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static WIDTHS: LazyLock<Token> = LazyLock::new(|| Token::new("widths"));
}

/// Returns the custom primvar mappings for Points prims.
///
/// Maps "widths" USD attribute to "widths" primvar, matching the C++ static mapping.
fn get_custom_primvar_mappings() -> Vec<PrimvarMapping> {
    vec![PrimvarMapping::new(
        tokens::WIDTHS.clone(),
        tokens::WIDTHS.clone(),
    )]
}

// ============================================================================
// DataSourcePointsPrim
// ============================================================================

/// Prim data source for UsdGeomPoints.
///
/// Extends DataSourceGprim by overlaying "widths" as a custom primvar
/// on top of the base gprim's primvars container. This matches the C++
/// architecture which uses HdOverlayContainerDataSource.
pub struct DataSourcePointsPrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    scene_index_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl usd_hd::HdDataSourceBase for DataSourcePointsPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            base: Arc::clone(&self.base),
            prim: self.prim.clone(),
            scene_index_path: self.scene_index_path.clone(),
            stage_globals: self.stage_globals.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(Self {
            base: Arc::clone(&self.base),
            prim: self.prim.clone(),
            scene_index_path: self.scene_index_path.clone(),
            stage_globals: self.stage_globals.clone(),
        }))
    }
}

impl HdContainerDataSource for DataSourcePointsPrim {
    fn get_names(&self) -> Vec<Token> {
        Self::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        Self::get(self, name)
    }
}

impl std::fmt::Debug for DataSourcePointsPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePointsPrim").finish()
    }
}

impl DataSourcePointsPrim {
    /// Creates a new points prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourceGprim::new(
                scene_index_path.clone(),
                prim.clone(),
                stage_globals.clone(),
            ),
            prim,
            scene_index_path,
            stage_globals,
        }
    }

    /// Returns the list of data source names.
    ///
    /// Same as gprim - widths is surfaced through the primvars overlay,
    /// not as a separate top-level container.
    pub fn get_names(&self) -> Vec<Token> {
        self.base.get_names()
    }

    /// Gets a data source by name.
    ///
    /// For "primvars", overlays the custom widths primvar on top of
    /// the base gprim primvars. For everything else, delegates to gprim.
    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let result = self.base.get(name);

        // Overlay widths as custom primvar on the primvars container
        if *name == *tokens::PRIMVARS {
            let custom = DataSourceCustomPrimvars::new(
                self.scene_index_path.clone(),
                self.prim.clone(),
                get_custom_primvar_mappings(),
                self.stage_globals.clone(),
            );
            let custom_container = Arc::new(custom) as usd_hd::HdContainerDataSourceHandle;
            let base_container = result
                .as_ref()
                .and_then(usd_hd::data_source::cast_to_container);

            return Some(match base_container {
                Some(base) => HdOverlayContainerDataSource::new_2(base, custom_container)
                    as HdDataSourceBaseHandle,
                None => custom_container as HdDataSourceBaseHandle,
            });
        }

        result
    }

    /// Computes invalidation locators for property changes.
    ///
    /// Delegates to gprim base + custom primvar invalidation for "widths".
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        if subprim.is_empty() {
            // Check if "widths" changed and add primvars invalidation
            let custom_locators =
                DataSourceCustomPrimvars::invalidate(properties, &get_custom_primvar_mappings());
            locators.insert_set(&custom_locators);
        }

        locators
    }
}

/// Handle type for DataSourcePointsPrim.
pub type DataSourcePointsPrimHandle = Arc<DataSourcePointsPrim>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_points_prim_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourcePointsPrim::new(Path::absolute_root(), prim, globals);
        let names = ds.get_names();

        // Should have gprim names (primvars, xform, etc.) but NO separate "points" container
        assert!(names.iter().any(|n| n == "primvars"));
        // The old incorrect architecture had a "points" container - verify it's gone
        assert!(!names.iter().any(|n| n == "points"));
    }

    #[test]
    fn test_points_primvar_overlay() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourcePointsPrim::new(Path::absolute_root(), prim, globals);
        let primvars_token = Token::new("primvars");

        // Requesting "primvars" should return a container that includes widths
        let result = ds.get(&primvars_token);
        assert!(result.is_some());
    }

    #[test]
    fn test_widths_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        // "widths" should trigger primvars invalidation
        let properties = vec![Token::new("widths")];
        let locators = DataSourcePointsPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());
    }

    #[test]
    fn test_points_invalidation_includes_gprim() {
        usd_core::schema_registry::register_builtin_schemas();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        // Use a Mesh prim (PointBased) — "points" invalidation requires
        // custom primvar mappings which only exist for PointBased prims.
        stage.define_prim("/TestMesh", "Mesh").expect("define prim");
        let prim = stage
            .get_prim_at_path(&usd_sdf::Path::from_string("/TestMesh").unwrap())
            .expect("get prim");

        // "points" attribute should invalidate through gprim custom primvar mappings
        let properties = vec![Token::new("points")];
        let locators = DataSourcePointsPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());
    }
}
