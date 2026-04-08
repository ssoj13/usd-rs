//! CoordSysAdapter - Adapter for coordinate system prims.
//!
//! Port of pxr/usdImaging/usdImaging/coordSysAdapter.h/cpp
//!
//! Provides imaging support for coordinate system prims used in shading.

use super::data_source_prim::DataSourcePrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static COORD_SYS: LazyLock<Token> = LazyLock::new(|| Token::new("coordSys"));
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    pub static MATRIX: LazyLock<Token> = LazyLock::new(|| Token::new("matrix"));
    pub static NAME: LazyLock<Token> = LazyLock::new(|| Token::new("name"));
}

// ============================================================================
// DataSourceCoordSys
// ============================================================================

/// Data source for coordinate system.
#[derive(Clone)]
pub struct DataSourceCoordSys {
    #[allow(dead_code)]
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceCoordSys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCoordSys").finish()
    }
}

impl DataSourceCoordSys {
    /// Create new coord sys data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourceCoordSys {
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

impl HdContainerDataSource for DataSourceCoordSys {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::NAME.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::NAME {
            return Some(HdRetainedTypedSampledDataSource::new(Token::new(
                self.prim.path().get_name(),
            )) as HdDataSourceBaseHandle);
        }
        None
    }
}

// ============================================================================
// DataSourceCoordSysPrim
// ============================================================================

/// Prim data source for coordinate system prims.
#[derive(Clone)]
pub struct DataSourceCoordSysPrim {
    #[allow(dead_code)]
    scene_index_path: Path,
    coord_sys_ds: Arc<DataSourceCoordSys>,
    prim_ds: Arc<DataSourcePrim>,
}

impl std::fmt::Debug for DataSourceCoordSysPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceCoordSysPrim").finish()
    }
}

impl DataSourceCoordSysPrim {
    /// Create new coord sys prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let coord_sys_ds = DataSourceCoordSys::new(prim.clone(), stage_globals.clone());
        let prim_ds = Arc::new(DataSourcePrim::new(
            prim,
            scene_index_path.clone(),
            stage_globals,
        ));
        Arc::new(Self {
            scene_index_path,
            coord_sys_ds,
            prim_ds,
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
            let prop_str = prop.as_str();
            // Transform changes affect coord sys
            if prop_str.starts_with("xformOp") || prop_str == "xformOpOrder" {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    tokens::XFORM.clone(),
                    tokens::MATRIX.clone(),
                ));
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceCoordSysPrim {
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

impl HdContainerDataSource for DataSourceCoordSysPrim {
    fn get_names(&self) -> Vec<Token> {
        vec![tokens::COORD_SYS.clone(), tokens::XFORM.clone()]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::COORD_SYS {
            return Some(Arc::clone(&self.coord_sys_ds) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::XFORM {
            return self.prim_ds.get(name);
        }
        None
    }
}

// ============================================================================
// CoordSysAdapter
// ============================================================================

/// Adapter for coordinate system prims.
///
/// Coordinate systems provide named transforms for shader coordinate spaces.
#[derive(Debug, Clone)]
pub struct CoordSysAdapter;

impl Default for CoordSysAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CoordSysAdapter {
    /// Create a new coord sys adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for CoordSysAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::COORD_SYS.clone()
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
            Some(DataSourceCoordSysPrim::new(
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
        DataSourceCoordSysPrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Handle type for CoordSysAdapter.
pub type CoordSysAdapterHandle = Arc<CoordSysAdapter>;

/// Factory for creating coord sys adapters.
pub fn create_coord_sys_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(CoordSysAdapter::new())
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
    fn test_coord_sys_adapter() {
        let adapter = CoordSysAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "coordSys");
    }

    #[test]
    fn test_coord_sys_subprims() {
        let adapter = CoordSysAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_coord_sys_data_source() {
        let adapter = CoordSysAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_coord_sys_invalidation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("xformOpOrder")];

        let locators = DataSourceCoordSysPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_factory() {
        let _ = create_coord_sys_adapter();
    }
}
