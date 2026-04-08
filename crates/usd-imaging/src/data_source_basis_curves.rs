//! DataSourceBasisCurves - BasisCurves data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceBasisCurves.h/cpp
//!
//! Provides data sources for UsdGeomBasisCurves prims including
//! topology (curveVertexCounts, basis, type, wrap).

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_gprim::DataSourceGprim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::basis_curves::BasisCurves;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBase, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Array;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static BASIS_CURVES: LazyLock<Token> = LazyLock::new(|| Token::new("basisCurves"));
    pub static TOPOLOGY: LazyLock<Token> = LazyLock::new(|| Token::new("topology"));
    pub static CURVE_VERTEX_COUNTS: LazyLock<Token> =
        LazyLock::new(|| Token::new("curveVertexCounts"));
    pub static TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("type"));
    pub static BASIS: LazyLock<Token> = LazyLock::new(|| Token::new("basis"));
    pub static WRAP: LazyLock<Token> = LazyLock::new(|| Token::new("wrap"));
}

// ============================================================================
// DataSourceBasisCurvesTopology
// ============================================================================

/// Container data source representing basis curves topology.
///
/// Reads curveVertexCounts, basis, type, and wrap from UsdGeomBasisCurves.
#[derive(Clone)]
pub struct DataSourceBasisCurvesTopology {
    scene_index_path: Path,
    curves: BasisCurves,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceBasisCurvesTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceBasisCurvesTopology").finish()
    }
}

impl DataSourceBasisCurvesTopology {
    /// Creates a new basis curves topology data source.
    pub fn new(
        scene_index_path: Path,
        curves: BasisCurves,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            curves,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceBasisCurvesTopology {
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

impl HdContainerDataSource for DataSourceBasisCurvesTopology {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::CURVE_VERTEX_COUNTS.clone(),
            tokens::BASIS.clone(),
            tokens::TYPE.clone(),
            tokens::WRAP.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Read topology attributes from UsdGeomBasisCurves.
        // curveVertexCounts gets scene path for locator tracking.
        // basis, type, wrap are simpler token attributes.
        let attr = if *name == *tokens::CURVE_VERTEX_COUNTS {
            self.curves.curves().get_curve_vertex_counts_attr()
        } else if *name == *tokens::BASIS {
            self.curves.get_basis_attr()
        } else if *name == *tokens::TYPE {
            self.curves.get_type_attr()
        } else if *name == *tokens::WRAP {
            self.curves.get_wrap_attr()
        } else {
            return None;
        };

        if !attr.is_valid() {
            log::warn!(
                "DataSourceBasisCurvesTopology::get: attr '{}' invalid for prim at {} (prim_valid={}, attr_names={:?})",
                name,
                self.scene_index_path,
                self.curves.is_valid(),
                self.curves
                    .prim()
                    .get_attribute_names()
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<_>>()
            );
            return None;
        }

        if *name == *tokens::CURVE_VERTEX_COUNTS {
            Some(DataSourceAttribute::<Array<i32>>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ) as HdDataSourceBaseHandle)
        } else {
            Some(DataSourceAttribute::<Token>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ) as HdDataSourceBaseHandle)
        }
    }
}

/// Handle type for DataSourceBasisCurvesTopology.
pub type DataSourceBasisCurvesTopologyHandle = Arc<DataSourceBasisCurvesTopology>;

// ============================================================================
// DataSourceBasisCurves
// ============================================================================

/// Container data source for basis curves specific data.
///
/// Contains only topology sub-container.
/// Matches C++ UsdImagingDataSourceBasisCurves.
#[derive(Clone)]
pub struct DataSourceBasisCurves {
    scene_index_path: Path,
    curves: BasisCurves,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceBasisCurves {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceBasisCurves").finish()
    }
}

impl DataSourceBasisCurves {
    /// Creates a new basis curves data source.
    pub fn new(
        scene_index_path: Path,
        curves: BasisCurves,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            curves,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceBasisCurves {
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

impl HdContainerDataSource for DataSourceBasisCurves {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::TOPOLOGY.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::TOPOLOGY {
            return Some(DataSourceBasisCurvesTopology::new(
                self.scene_index_path.clone(),
                self.curves.clone(),
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }
        None
    }
}

/// Handle type for DataSourceBasisCurves.
pub type DataSourceBasisCurvesHandle = Arc<DataSourceBasisCurves>;

// ============================================================================
// DataSourceBasisCurvesPrim
// ============================================================================

/// Prim data source for UsdGeomBasisCurves.
///
/// Extends DataSourceGprim with "basisCurves" container.
/// Matches C++ UsdImagingDataSourceBasisCurvesPrim.
pub struct DataSourceBasisCurvesPrim {
    base: Arc<DataSourceGprim>,
    prim: Prim,
    scene_index_path: Path,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl HdDataSourceBase for DataSourceBasisCurvesPrim {
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

impl HdContainerDataSource for DataSourceBasisCurvesPrim {
    fn get_names(&self) -> Vec<Token> {
        Self::get_names(self)
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        Self::get(self, name)
    }
}

impl std::fmt::Debug for DataSourceBasisCurvesPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceBasisCurvesPrim").finish()
    }
}

impl DataSourceBasisCurvesPrim {
    /// Creates a new basis curves prim data source.
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
    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(tokens::BASIS_CURVES.clone());
        names
    }

    /// Gets a data source by name.
    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::BASIS_CURVES {
            let curves = BasisCurves::new(self.prim.clone());
            return Some(DataSourceBasisCurves::new(
                self.scene_index_path.clone(),
                curves,
                self.stage_globals.clone(),
            ) as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }

    /// Computes invalidation locators for property changes.
    ///
    /// Maps curve-specific USD property names to topology sub-locators
    /// following the C++ pattern (per-property granularity).
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();

        if subprim.is_empty() {
            // Base gprim invalidation
            let base = DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);
            locators.insert_set(&base);

            // Per-property topology invalidation (C++ uses per-property locators)
            for prop in properties {
                let prop_str = prop.as_str();
                match prop_str {
                    "curveVertexCounts" => {
                        locators.insert(HdDataSourceLocator::from_tokens_3(
                            Token::new("basisCurves"),
                            Token::new("topology"),
                            Token::new("curveVertexCounts"),
                        ));
                    }
                    "type" => {
                        locators.insert(HdDataSourceLocator::from_tokens_3(
                            Token::new("basisCurves"),
                            Token::new("topology"),
                            Token::new("type"),
                        ));
                    }
                    "basis" => {
                        locators.insert(HdDataSourceLocator::from_tokens_3(
                            Token::new("basisCurves"),
                            Token::new("topology"),
                            Token::new("basis"),
                        ));
                    }
                    "wrap" => {
                        locators.insert(HdDataSourceLocator::from_tokens_3(
                            Token::new("basisCurves"),
                            Token::new("topology"),
                            Token::new("wrap"),
                        ));
                    }
                    _ => {}
                }
            }
        }

        locators
    }
}

/// Handle type for DataSourceBasisCurvesPrim.
pub type DataSourceBasisCurvesPrimHandle = Arc<DataSourceBasisCurvesPrim>;

/// Creates a new basis curves prim data source.
pub fn create_data_source_basis_curves_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourceBasisCurvesPrimHandle {
    Arc::new(DataSourceBasisCurvesPrim::new(
        scene_index_path,
        prim,
        stage_globals,
    ))
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
    fn test_topology_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();
        let curves = BasisCurves::new(prim);

        let ds = DataSourceBasisCurvesTopology::new(Path::absolute_root(), curves, globals);
        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "curveVertexCounts"));
        assert!(names.iter().any(|n| n == "basis"));
        assert!(names.iter().any(|n| n == "type"));
        assert!(names.iter().any(|n| n == "wrap"));
    }

    #[test]
    fn test_basis_curves_container() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();
        let curves = BasisCurves::new(prim);

        let ds = DataSourceBasisCurves::new(Path::absolute_root(), curves, globals);
        let names = ds.get_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0].as_str(), "topology");

        // Get topology should return a container
        let topology = ds.get(&tokens::TOPOLOGY);
        assert!(topology.is_some());
    }

    #[test]
    fn test_prim_data_source() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourceBasisCurvesPrim::new(Path::absolute_root(), prim, globals);
        let names = ds.get_names();

        // Should have basisCurves + gprim names
        assert!(names.iter().any(|n| n == "basisCurves"));
        assert!(names.iter().any(|n| n == "primvars"));

        // basisCurves should return container
        let bc = ds.get(&tokens::BASIS_CURVES);
        assert!(bc.is_some());
    }

    #[test]
    fn test_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("curveVertexCounts"), Token::new("basis")];

        let locators = DataSourceBasisCurvesPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );
        assert!(!locators.is_empty());
    }
}
