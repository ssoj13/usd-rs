//! GeomSubsetAdapter - Adapter for UsdGeomSubset prims.
//!
//! Port of pxr/usdImaging/usdImaging/geomSubsetAdapter.h/cpp
//!
//! Provides imaging support for geometry subsets used for material binding.

use super::data_source_attribute::DataSourceAttribute;
use super::data_source_prim::DataSourcePrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::{PopulationMode, PropertyInvalidationType};
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::subset::Subset;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdOverlayContainerDataSource, HdSampledDataSource,
    HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{Array, Value};

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static GEOM_SUBSET: LazyLock<Token> = LazyLock::new(|| Token::new("geomSubset"));
    pub static ELEMENT_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("elementType"));
    pub static INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("indices"));
    // Element types
    pub static FACE: LazyLock<Token> = LazyLock::new(|| Token::new("face"));
    pub static POINT: LazyLock<Token> = LazyLock::new(|| Token::new("point"));
}

// ============================================================================
// ElementTypeConversionDataSource
// ============================================================================

#[derive(Clone)]
struct ElementTypeConversionDataSource {
    source: Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>,
}

impl std::fmt::Debug for ElementTypeConversionDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ElementTypeConversionDataSource").finish()
    }
}

impl ElementTypeConversionDataSource {
    fn new(source: Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>) -> Arc<Self> {
        Arc::new(Self { source })
    }

    fn convert(token: Token) -> Token {
        if token == *tokens::FACE {
            return Token::new("typeFaceSet");
        }
        if token == *tokens::POINT {
            return Token::new("typePointSet");
        }
        Token::default()
    }
}

impl HdDataSourceBase for ElementTypeConversionDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn sample_at_zero(&self) -> Option<Value> {
        Some(Value::from(self.get_typed_value(0.0)))
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for ElementTypeConversionDataSource {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        Value::from(self.get_typed_value(shutter_offset))
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.source
            .get_contributing_sample_times(start_time, end_time, out_sample_times)
    }
}

impl HdTypedSampledDataSource<Token> for ElementTypeConversionDataSource {
    fn get_typed_value(&self, shutter_offset: HdSampledDataSourceTime) -> Token {
        Self::convert(self.source.get_typed_value(shutter_offset))
    }
}

// ============================================================================
// DataSourceGeomSubset
// ============================================================================

/// Data source for geometry subset.
#[derive(Clone)]
pub struct DataSourceGeomSubset {
    scene_index_path: Path,
    subset: Subset,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceGeomSubset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceGeomSubset").finish()
    }
}

impl DataSourceGeomSubset {
    /// Create new geom subset data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            subset: Subset::new(prim),
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceGeomSubset {
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

impl HdContainerDataSource for DataSourceGeomSubset {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::ELEMENT_TYPE.clone(), tokens::INDICES.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::INDICES {
            let attr = self.subset.get_indices_attr();
            return Some(DataSourceAttribute::<Array<i32>>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            ) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::ELEMENT_TYPE {
            let attr = self.subset.get_element_type_attr();
            let ds = DataSourceAttribute::<Token>::new(
                attr,
                self.stage_globals.clone(),
                self.scene_index_path.clone(),
            );
            return Some(ElementTypeConversionDataSource::new(ds) as HdDataSourceBaseHandle);
        }
        None
    }
}

// ============================================================================
// DataSourceGeomSubsetPrim
// ============================================================================

/// Prim data source for geometry subset prims.
#[derive(Clone)]
pub struct DataSourceGeomSubsetPrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    subset_ds: Arc<DataSourceGeomSubset>,
}

impl std::fmt::Debug for DataSourceGeomSubsetPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceGeomSubsetPrim").finish()
    }
}

impl DataSourceGeomSubsetPrim {
    /// Create new geom subset prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let subset_ds = DataSourceGeomSubset::new(scene_index_path.clone(), prim, stage_globals);
        Arc::new(Self {
            scene_index_path,
            subset_ds,
        })
    }

    /// Compute invalidation for property changes.
    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();

        for prop in properties {
            if prop == &*tokens::INDICES {
                locators.insert(HdDataSourceLocator::from_token(Token::new("indices")));
            } else if prop == &*tokens::ELEMENT_TYPE {
                locators.insert(HdDataSourceLocator::from_token(Token::new("type")));
            }
        }

        locators.insert_set(&DataSourcePrim::invalidate(
            _prim,
            _subprim,
            properties,
            _invalidation_type,
        ));

        locators
    }
}

impl HdDataSourceBase for DataSourceGeomSubsetPrim {
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

impl HdContainerDataSource for DataSourceGeomSubsetPrim {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("geomSubset")]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == Token::new("geomSubset") {
            return Some(Arc::clone(&self.subset_ds) as HdDataSourceBaseHandle);
        }
        None
    }
}

// ============================================================================
// GeomSubsetAdapter
// ============================================================================

/// Adapter for UsdGeomSubset prims.
///
/// Geometry subsets partition mesh faces for per-face material binding.
#[derive(Debug, Clone)]
pub struct GeomSubsetAdapter;

impl Default for GeomSubsetAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeomSubsetAdapter {
    /// Create a new geom subset adapter.
    pub fn new() -> Self {
        Self
    }

    /// Geom subsets are represented by their parent mesh.
    pub fn get_population_mode() -> PopulationMode {
        PopulationMode::RepresentedByAncestor
    }
}

impl PrimAdapter for GeomSubsetAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // C++: returns { TfToken() } — one empty subprim
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        // C++: HdPrimTypeTokens->geomSubset when subprim is empty
        if subprim.is_empty() {
            return tokens::GEOM_SUBSET.clone();
        }
        Token::new("")
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if !subprim.is_empty() {
            return None;
        }
        let subset_container = DataSourceGeomSubsetPrim::new(
            prim.get_path().clone(),
            prim.clone(),
            stage_globals.clone(),
        ) as HdContainerDataSourceHandle;
        let prim_container = Arc::new(DataSourcePrim::new(
            prim.clone(),
            prim.get_path().clone(),
            stage_globals.clone(),
        )) as HdContainerDataSourceHandle;
        Some(
            HdOverlayContainerDataSource::new_2(subset_container, prim_container)
                as HdContainerDataSourceHandle,
        )
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourceGeomSubsetPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Handle type for GeomSubsetAdapter.
pub type GeomSubsetAdapterHandle = Arc<GeomSubsetAdapter>;

/// Factory for creating geom subset adapters.
pub fn create_geom_subset_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(GeomSubsetAdapter::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;

    #[test]
    fn test_geom_subset_adapter() {
        let adapter = GeomSubsetAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        // C++: one empty subprim
        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_geom_subset_population_mode() {
        assert_eq!(
            GeomSubsetAdapter::get_population_mode(),
            PopulationMode::RepresentedByAncestor
        );
    }

    #[test]
    fn test_factory() {
        let _ = create_geom_subset_adapter();
    }
}
