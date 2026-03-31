//! DataSourcePointInstancer - PointInstancer data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourcePointInstancer.h
//!
//! Provides data sources for UsdGeomPointInstancer prims.

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_prim::DataSourcePrim;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use std::sync::Arc;
use usd_core::Prim;
use usd_geom::point_instancer::PointInstancer;
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Array;

// Token constants
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("instancer"));
    pub static INSTANCER_TOPOLOGY: LazyLock<Token> =
        LazyLock::new(|| Token::new("instancerTopology"));
    pub static PROTOTYPES: LazyLock<Token> = LazyLock::new(|| Token::new("prototypes"));
    pub static PROTO_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("protoIndices"));
    pub static MASK: LazyLock<Token> = LazyLock::new(|| Token::new("mask"));
    pub static POSITIONS: LazyLock<Token> = LazyLock::new(|| Token::new("positions"));
    pub static ORIENTATIONS: LazyLock<Token> = LazyLock::new(|| Token::new("orientations"));
    pub static SCALES: LazyLock<Token> = LazyLock::new(|| Token::new("scales"));
    pub static VELOCITIES: LazyLock<Token> = LazyLock::new(|| Token::new("velocities"));
    pub static ANGULAR_VELOCITIES: LazyLock<Token> =
        LazyLock::new(|| Token::new("angularVelocities"));
    pub static INVISIBLE_IDS: LazyLock<Token> = LazyLock::new(|| Token::new("invisibleIds"));
}

// ============================================================================
// DataSourcePointInstancerMask
// ============================================================================

/// Data source for point instancer instance mask.
///
/// Stores per-instance visibility. Empty array means all visible.
#[derive(Clone)]
pub struct DataSourcePointInstancerMask {
    #[allow(dead_code)] // Part of data source infrastructure
    scene_index_path: Path,
    #[allow(dead_code)]
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourcePointInstancerMask {
    /// Create a new mask data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
    }
}

impl std::fmt::Debug for DataSourcePointInstancerMask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourcePointInstancerMask")
    }
}

impl usd_hd::HdDataSourceBase for DataSourcePointInstancerMask {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Handle type for DataSourcePointInstancerMask.
pub type DataSourcePointInstancerMaskHandle = Arc<DataSourcePointInstancerMask>;

// ============================================================================
// DataSourcePointInstancerTopology
// ============================================================================

/// Container data source for point instancer topology.
///
/// Contains prototypes, protoIndices, and mask.
#[derive(Clone)]
pub struct DataSourcePointInstancerTopology {
    #[allow(dead_code)] // Part of data source infrastructure
    scene_index_path: Path,
    #[allow(dead_code)]
    prim: Prim,
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourcePointInstancerTopology {
    /// Create a new topology data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
        }
    }
}

impl std::fmt::Debug for DataSourcePointInstancerTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourcePointInstancerTopology")
    }
}

impl usd_hd::HdDataSourceBase for DataSourcePointInstancerTopology {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourcePointInstancerTopology {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::PROTOTYPES.clone(),
            tokens::PROTO_INDICES.clone(),
            tokens::MASK.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::MASK {
            let mask = DataSourcePointInstancerMask::new(
                self.scene_index_path.clone(),
                self.prim.clone(),
                self.stage_globals.clone(),
            );
            return Some(Arc::new(mask) as HdDataSourceBaseHandle);
        }

        if name == &*tokens::PROTO_INDICES {
            // Read protoIndices attribute from UsdGeomPointInstancer
            let pi = PointInstancer::new(self.prim.clone());
            let attr = pi.get_proto_indices_attr();
            if attr.is_valid() {
                return Some(DataSourceAttribute::<Array<i32>>::new(
                    attr,
                    self.stage_globals.clone(),
                    self.scene_index_path.clone(),
                ) as HdDataSourceBaseHandle);
            }
            return None;
        }

        if name == &*tokens::PROTOTYPES {
            // Read prototypes relationship targets as path array
            let pi = PointInstancer::new(self.prim.clone());
            let rel = pi.get_prototypes_rel();
            let targets = rel.get_targets();
            if !targets.is_empty() {
                return Some(
                    HdRetainedTypedSampledDataSource::new(targets) as HdDataSourceBaseHandle
                );
            }
            return None;
        }

        None
    }
}

/// Handle type for DataSourcePointInstancerTopology.
pub type DataSourcePointInstancerTopologyHandle = Arc<DataSourcePointInstancerTopology>;

// ============================================================================
// DataSourcePointInstancerPrim
// ============================================================================

/// Prim data source for UsdGeomPointInstancer.
///
/// Extends DataSourcePrim with instancer-specific data.
#[derive(Clone)]
pub struct DataSourcePointInstancerPrim {
    /// Base prim data source
    base: DataSourcePrim,
    /// The USD prim
    prim: Prim,
    /// Stage globals
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourcePointInstancerPrim {
    /// Create a new point instancer prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            base: DataSourcePrim::new(
                prim.clone(),
                scene_index_path.clone(),
                stage_globals.clone(),
            ),
            prim,
            stage_globals,
        }
    }

    /// Get the list of data source names.
    pub fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(tokens::INSTANCER.clone());
        names.push(tokens::INSTANCER_TOPOLOGY.clone());
        names
    }

    /// Get a data source by name.
    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &*tokens::INSTANCER_TOPOLOGY {
            let topology = DataSourcePointInstancerTopology::new(
                (*self.base.hydra_path()).clone(),
                self.prim.clone(),
                self.stage_globals.clone(),
            );
            return Some(Arc::new(topology) as HdDataSourceBaseHandle);
        }
        self.base.get(name)
    }

    /// Invalidate data source for property changes.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);

        for prop in properties {
            let prop_str = prop.as_str();
            // Instancer-specific properties
            if prop_str == "prototypes"
                || prop_str == "protoIndices"
                || prop_str == "positions"
                || prop_str == "orientations"
                || prop_str == "scales"
                || prop_str == "velocities"
                || prop_str == "angularVelocities"
                || prop_str == "invisibleIds"
            {
                locators.insert(HdDataSourceLocator::from_token(tokens::INSTANCER.clone()));
                break;
            }
        }

        locators
    }
}

impl std::fmt::Debug for DataSourcePointInstancerPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourcePointInstancerPrim")
    }
}

/// Handle type for DataSourcePointInstancerPrim.
pub type DataSourcePointInstancerPrimHandle = Arc<DataSourcePointInstancerPrim>;

/// Factory function for creating point instancer prim data sources.
pub fn create_data_source_point_instancer_prim(
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
) -> DataSourcePointInstancerPrimHandle {
    Arc::new(DataSourcePointInstancerPrim::new(
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

        let ds = DataSourcePointInstancerTopology::new(Path::absolute_root(), prim, globals);

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "prototypes"));
        assert!(names.iter().any(|n| n == "protoIndices"));
    }

    #[test]
    fn test_prim_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourcePointInstancerPrim::new(Path::absolute_root(), prim, globals);

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "instancer"));
        assert!(names.iter().any(|n| n == "instancerTopology"));
    }
}
