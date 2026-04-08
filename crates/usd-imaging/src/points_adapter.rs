//! PointsAdapter - Adapter for UsdGeomPoints.
//!
//! Port of pxr/usdImaging/usdImaging/pointsAdapter.h/cpp
//!
//! Provides imaging support for UsdGeomPoints prims (point clouds).

use crate::data_source_points as canonical_points_data_source;

use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static POINTS: LazyLock<Token> = LazyLock::new(|| Token::new("points"));
    pub static WIDTHS: LazyLock<Token> = LazyLock::new(|| Token::new("widths"));
    pub static IDS: LazyLock<Token> = LazyLock::new(|| Token::new("ids"));
}

// ============================================================================
// DataSourcePoints
// ============================================================================

/// Data source for points-specific parameters.
#[derive(Clone)]
pub struct DataSourcePoints {
    #[allow(dead_code)] // For future points attribute reading
    prim: Prim,
    #[allow(dead_code)] // For future time-sampled reading
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourcePoints {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePoints").finish()
    }
}

impl DataSourcePoints {
    /// Create new points data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourcePoints {
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

impl HdContainerDataSource for DataSourcePoints {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::WIDTHS.clone(), tokens::IDS.clone()]
    }

    fn get(&self, _name: &Token) -> Option<HdDataSourceBaseHandle> {
        None
    }
}

// ============================================================================
// DataSourcePointsPrim
// ============================================================================

/// Prim data source for UsdGeomPoints.
#[derive(Clone)]
pub struct DataSourcePointsPrim {
    #[allow(dead_code)] // For future path-based operations
    scene_index_path: Path,
    gprim_ds: Arc<DataSourceGprim>,
    points_ds: Arc<DataSourcePoints>,
}

impl std::fmt::Debug for DataSourcePointsPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePointsPrim").finish()
    }
}

impl DataSourcePointsPrim {
    /// Create new points prim data source.
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
        let points_ds = DataSourcePoints::new(prim, stage_globals);

        Arc::new(Self {
            scene_index_path,
            gprim_ds,
            points_ds,
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
            match prop_str {
                "widths" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::POINTS.clone(),
                        tokens::WIDTHS.clone(),
                    ));
                }
                "ids" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::POINTS.clone(),
                        tokens::IDS.clone(),
                    ));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourcePointsPrim {
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

impl HdContainerDataSource for DataSourcePointsPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.gprim_ds.get_names();
        names.push(tokens::POINTS.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::POINTS {
            return Some(Arc::clone(&self.points_ds) as HdDataSourceBaseHandle);
        }
        self.gprim_ds.get(name)
    }
}

// ============================================================================
// PointsAdapter
// ============================================================================

/// Adapter for UsdGeomPoints prims.
///
/// Converts UsdGeomPoints (point clouds) to Hydra points primitives.
#[derive(Debug, Clone, Default)]
pub struct PointsAdapter;

impl PointsAdapter {
    /// Create a new points adapter.
    pub fn new() -> Self {
        Self
    }

    /// Check if a primvar is built-in for points.
    pub fn is_builtin_primvar(primvar_name: &Token) -> bool {
        matches!(
            primvar_name.as_str(),
            "points"
                | "widths"
                | "ids"
                | "velocities"
                | "accelerations"
                | "displayColor"
                | "displayOpacity"
        )
    }
}

impl PrimAdapter for PointsAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::POINTS.clone()
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
            // Keep the adapter on the canonical `_ref`-style points datasource.
            // The older adapter-local datasource tree was a forked
            // compatibility path and had already drifted from
            // `UsdImagingDataSourcePointsPrim`.
            Some(Arc::new(
                canonical_points_data_source::DataSourcePointsPrim::new(
                    prim.path().clone(),
                    prim.clone(),
                    stage_globals.clone(),
                ),
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
        canonical_points_data_source::DataSourcePointsPrim::invalidate(
            prim,
            subprim,
            properties,
            invalidation_type,
        )
    }
}

/// Arc-wrapped PointsAdapter for sharing
pub type PointsAdapterHandle = Arc<PointsAdapter>;

/// Factory function for creating points adapters.
pub fn create_points_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PointsAdapter::new())
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
    fn test_points_adapter_creation() {
        let adapter = PointsAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let subprim = Token::new("");

        let prim_type = adapter.get_imaging_subprim_type(&prim, &subprim);
        assert_eq!(prim_type.as_str(), "points");
    }

    #[test]
    fn test_points_adapter_subprims() {
        let adapter = PointsAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_points_adapter_data_source() {
        let adapter = PointsAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_builtin_primvars() {
        assert!(PointsAdapter::is_builtin_primvar(&Token::new("points")));
        assert!(PointsAdapter::is_builtin_primvar(&Token::new("widths")));
        assert!(PointsAdapter::is_builtin_primvar(&Token::new("ids")));
        assert!(!PointsAdapter::is_builtin_primvar(&Token::new("custom")));
    }

    #[test]
    fn test_points_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("widths"), Token::new("points")];

        let locators = DataSourcePointsPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }
}
