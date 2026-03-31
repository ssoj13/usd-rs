//! PointInstancerAdapter - Adapter for UsdGeomPointInstancer.
//!
//! Port of pxr/usdImaging/usdImaging/pointInstancerAdapter.h/cpp
//!
//! Provides imaging support for point instancers, which efficiently
//! instance geometry at multiple positions/orientations/scales.
//!
//! Core functionality:
//! - Read prototypes relationship to discover prototype prims
//! - Compute instance transforms from positions+orientations+scales
//! - Build per-prototype instance indices (protoIndices -> instanceIndices)
//! - Handle invisibleIds mask
//! - Provide instancer topology data sources for Hydra

use super::data_source_gprim::DataSourceGprim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::PropertyInvalidationType;
use std::collections::HashMap;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedSampledDataSource,
    HdRetainedSmallVectorDataSource, HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

// Token constants matching C++ UsdGeomTokens + HdInstancerTokens
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    // Prim type
    #[allow(dead_code)] // C++ prim type token, wiring in progress
    pub static POINT_INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("pointInstancer"));
    pub static INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("instancer"));

    // Hydra schema tokens
    pub static INSTANCER_TOPOLOGY: LazyLock<Token> =
        LazyLock::new(|| Token::new("instancerTopology"));
    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));

    // PointInstancer USD attributes
    pub static PROTO_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("protoIndices"));
    pub static POSITIONS: LazyLock<Token> = LazyLock::new(|| Token::new("positions"));
    pub static ORIENTATIONS: LazyLock<Token> = LazyLock::new(|| Token::new("orientations"));
    #[allow(dead_code)] // C++ float quaternion orientations attr, wiring in progress
    pub static ORIENTATIONSF: LazyLock<Token> = LazyLock::new(|| Token::new("orientationsf"));
    pub static SCALES: LazyLock<Token> = LazyLock::new(|| Token::new("scales"));
    pub static VELOCITIES: LazyLock<Token> = LazyLock::new(|| Token::new("velocities"));
    pub static ANGULAR_VELOCITIES: LazyLock<Token> =
        LazyLock::new(|| Token::new("angularVelocities"));
    pub static ACCELERATIONS: LazyLock<Token> = LazyLock::new(|| Token::new("accelerations"));
    pub static INVISIBLE_IDS: LazyLock<Token> = LazyLock::new(|| Token::new("invisibleIds"));
    pub static IDS: LazyLock<Token> = LazyLock::new(|| Token::new("ids"));
    pub static PROTOTYPES: LazyLock<Token> = LazyLock::new(|| Token::new("prototypes"));

    // Hydra instancer tokens (mapped from USD attributes)
    pub static INSTANCE_TRANSLATIONS: LazyLock<Token> =
        LazyLock::new(|| Token::new("instanceTranslations"));
    pub static INSTANCE_ROTATIONS: LazyLock<Token> =
        LazyLock::new(|| Token::new("instanceRotations"));
    pub static INSTANCE_SCALES: LazyLock<Token> = LazyLock::new(|| Token::new("instanceScales"));
    pub static INSTANCE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("instanceIndices"));
    pub static MASK: LazyLock<Token> = LazyLock::new(|| Token::new("mask"));

    // Interpolation tokens
    #[allow(dead_code)] // C++ interpolation token, wiring in progress
    pub static INSTANCE: LazyLock<Token> = LazyLock::new(|| Token::new("instance"));
    #[allow(dead_code)] // C++ interpolation token, wiring in progress
    pub static CONSTANT: LazyLock<Token> = LazyLock::new(|| Token::new("constant"));
}

// ============================================================================
// ProtoPrim - per-prototype prim in point instancer
// ============================================================================

/// A proto prim under a point instancer prototype root.
///
/// Matches C++ _ProtoPrim. Each imageable prim in the prototype subtree
/// gets one entry with its path chain and adapter.
#[derive(Clone)]
pub struct ProtoPrim {
    /// Path chain through native instances (back to front: prototype->instance)
    pub paths: Vec<Path>,
    /// The prim adapter
    pub adapter: Option<Arc<dyn PrimAdapter>>,
    /// Root prototype path (typically model root)
    pub proto_root_path: Path,
    /// Cached variability bits
    pub variability_bits: u32,
    /// Cached visibility (when not time-varying)
    pub visible: bool,
}

impl std::fmt::Debug for ProtoPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtoPrim")
            .field("paths", &self.paths)
            .field("has_adapter", &self.adapter.is_some())
            .field("proto_root_path", &self.proto_root_path)
            .field("visible", &self.visible)
            .finish()
    }
}

impl Default for ProtoPrim {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            adapter: None,
            proto_root_path: Path::empty(),
            variability_bits: 0,
            visible: true,
        }
    }
}

// ============================================================================
// InstancerData - all state for one point instancer
// ============================================================================

/// All data associated with a point instancer prim.
///
/// Matches C++ _InstancerData. Stores prototype paths, proto prim map,
/// and visibility state.
#[derive(Debug, Clone)]
pub struct PointInstancerData {
    /// Parent instancer cache path (for nested instancers)
    pub parent_instancer_cache_path: Path,
    /// Map from cache path to ProtoPrim
    pub proto_prim_map: HashMap<Path, ProtoPrim>,
    /// Prototype paths (from prototypes relationship)
    pub prototype_paths: Vec<Path>,
    /// Map from prototype path to its index
    pub prototype_path_indices: HashMap<Path, usize>,
    /// Whether visibility varies over time
    pub variable_visibility: bool,
    /// Current visibility state
    pub visible: bool,
    /// Whether this instancer has been initialized
    pub initialized: bool,
}

impl Default for PointInstancerData {
    fn default() -> Self {
        Self {
            parent_instancer_cache_path: Path::empty(),
            proto_prim_map: HashMap::new(),
            prototype_paths: Vec::new(),
            prototype_path_indices: HashMap::new(),
            variable_visibility: true,
            visible: true,
            initialized: false,
        }
    }
}

// ============================================================================
// DataSourcePointInstancerTopology
// ============================================================================

/// Container data source for point instancer topology.
///
/// Provides prototypes (relationship targets), instanceIndices (flipped
/// protoIndices: per-prototype list of instance indices), and mask
/// (from invisibleIds).
///
/// Matches C++ UsdImagingDataSourcePointInstancerTopology.
#[derive(Clone)]
pub struct DataSourcePointInstancerTopology {
    /// Scene index path
    scene_index_path: Path,
    /// The USD prim (UsdGeomPointInstancer)
    prim: Prim,
    /// Stage globals
    #[allow(dead_code)] // C++ uses for time-sampled attribute evaluation
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourcePointInstancerTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePointInstancerTopology")
            .field("path", &self.scene_index_path)
            .finish()
    }
}

impl DataSourcePointInstancerTopology {
    /// Create new topology data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            scene_index_path,
            prim,
            stage_globals,
        })
    }

    /// Read prototype paths from the "prototypes" relationship.
    ///
    /// Returns paths to prototype root prims targeted by this instancer.
    fn get_prototypes(&self) -> Option<HdDataSourceBaseHandle> {
        let rel = self.prim.get_relationship("prototypes")?;
        let targets = rel.get_forwarded_targets();
        if targets.is_empty() {
            return None;
        }

        // Build a vector data source of path data sources
        let elements: Vec<HdDataSourceBaseHandle> = targets
            .iter()
            .map(|path| {
                HdRetainedTypedSampledDataSource::new(path.clone()) as HdDataSourceBaseHandle
            })
            .collect();

        Some(HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle)
    }

    /// Compute per-prototype instance indices from protoIndices attribute.
    ///
    /// Flips protoIndices [0,1,0,2] into per-prototype lists:
    ///   proto 0: [0, 2]
    ///   proto 1: [1]
    ///   proto 2: [3]
    fn get_instance_indices(&self) -> Option<HdDataSourceBaseHandle> {
        let attr = self.prim.get_attribute("protoIndices")?;
        if !attr.has_value() {
            return None;
        }

        // C++ reads at _stageGlobals.GetTime(), not default time.
        // Using default time freezes animated point instancers.
        let value = attr.get(self.stage_globals.get_time())?;
        let proto_indices: Vec<i32> = if let Some(arr) = value.get::<Vec<i32>>() {
            arr.clone()
        } else if let Some(&single) = value.get::<i32>() {
            vec![single]
        } else {
            return None;
        };

        // Flip: protoIndices[i] = protoIdx -> per-prototype instance lists
        let mut instance_indices: Vec<Vec<i32>> = Vec::new();
        for (i, &proto_idx) in proto_indices.iter().enumerate() {
            let idx = proto_idx as usize;
            if idx >= instance_indices.len() {
                instance_indices.resize(idx + 1, Vec::new());
            }
            instance_indices[idx].push(i as i32);
        }

        // Build vector of per-prototype index arrays using Value::from(Vec<i32>)
        let elements: Vec<HdDataSourceBaseHandle> = instance_indices
            .into_iter()
            .map(|indices| {
                HdRetainedSampledDataSource::new(usd_vt::Value::from(indices))
                    as HdDataSourceBaseHandle
            })
            .collect();

        Some(HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle)
    }

    /// Compute instance mask from invisibleIds.
    ///
    /// Reads invisibleIds + protoIndices to determine total instance count,
    /// then builds a bool array where invisible instances are true.
    fn get_mask(&self) -> Option<HdDataSourceBaseHandle> {
        // Read invisibleIds
        let invis_attr = self.prim.get_attribute("invisibleIds")?;
        if !invis_attr.has_value() {
            return None;
        }

        let invis_value = invis_attr.get(self.stage_globals.get_time())?;
        // invisibleIds is VtInt64Array in USD, but we try i32 as fallback
        let invisible_ids: Vec<i64> = if let Some(arr) = invis_value.get::<Vec<i32>>() {
            arr.iter().map(|&x| x as i64).collect()
        } else if let Some(&single) = invis_value.get::<i32>() {
            vec![single as i64]
        } else {
            return None;
        };

        if invisible_ids.is_empty() {
            // No invisible instances -> empty mask (all visible)
            return None;
        }

        // We need the total instance count from protoIndices
        let proto_attr = self.prim.get_attribute("protoIndices")?;
        let proto_value = proto_attr.get(self.stage_globals.get_time())?;
        let num_instances = if let Some(arr) = proto_value.get::<Vec<i32>>() {
            arr.len()
        } else {
            return None;
        };

        // C++ UsdGeomPointInstancer::ComputeMaskAtTime returns std::vector<bool>.
        // Hydra expects VtBoolArray, not VtIntArray.
        // true = visible, false = masked (invisible).
        let mut mask = vec![true; num_instances];
        for &id in &invisible_ids {
            if (id as usize) < num_instances {
                mask[id as usize] = false;
            }
        }

        Some(HdRetainedSampledDataSource::new(usd_vt::Value::from(mask)) as HdDataSourceBaseHandle)
    }
}

impl HdDataSourceBase for DataSourcePointInstancerTopology {
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

impl HdContainerDataSource for DataSourcePointInstancerTopology {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::PROTOTYPES.clone(),
            tokens::INSTANCE_INDICES.clone(),
            tokens::MASK.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::PROTOTYPES {
            return self.get_prototypes();
        }
        if *name == *tokens::INSTANCE_INDICES {
            return self.get_instance_indices();
        }
        if *name == *tokens::MASK {
            return self.get_mask();
        }
        None
    }
}

// ============================================================================
// DataSourcePointInstancer
// ============================================================================

/// Data source for point instancer per-instance primvar parameters.
///
/// Maps USD attributes (positions, orientations, scales) to Hydra primvars
/// with "instance" interpolation.
#[derive(Clone)]
pub struct DataSourcePointInstancer {
    /// The prim
    prim: Prim,
    /// Stage globals
    #[allow(dead_code)] // C++ uses for time-sampled attribute evaluation
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourcePointInstancer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePointInstancer")
            .field("path", &self.prim.path())
            .finish()
    }
}

impl DataSourcePointInstancer {
    /// Create new point instancer data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }
}

impl HdDataSourceBase for DataSourcePointInstancer {
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

impl HdContainerDataSource for DataSourcePointInstancer {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::PROTO_INDICES.clone(),
            tokens::POSITIONS.clone(),
            tokens::ORIENTATIONS.clone(),
            tokens::SCALES.clone(),
            tokens::VELOCITIES.clone(),
            tokens::ANGULAR_VELOCITIES.clone(),
            tokens::ACCELERATIONS.clone(),
            tokens::INVISIBLE_IDS.clone(),
            tokens::IDS.clone(),
            tokens::PROTOTYPES.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        // Read the corresponding USD attribute and return as a sampled DS.
        // The attribute name matches the token in most cases.
        let attr_name = name.as_str();
        if let Some(attr) = self.prim.get_attribute(attr_name) {
            if attr.has_value() {
                if let Some(value) = attr.get(self.stage_globals.get_time()) {
                    return Some(
                        usd_hd::HdRetainedSampledDataSource::new(value) as HdDataSourceBaseHandle
                    );
                }
            }
        }
        None
    }
}

// ============================================================================
// DataSourcePointInstancerPrim
// ============================================================================

/// Prim data source for point instancer prims.
///
/// Extends DataSourceGprim with:
/// - instancerTopology (prototypes, instanceIndices, mask)
/// - primvars with instance interpolation for positions/orientations/scales
///
/// Matches C++ UsdImagingDataSourcePointInstancerPrim.
#[derive(Clone)]
pub struct DataSourcePointInstancerPrim {
    /// Scene index path
    scene_index_path: Path,
    /// Base gprim data source (xform, visibility, purpose, primvars)
    gprim_ds: Arc<DataSourceGprim>,
    /// Instancer-specific data source (positions, orientations, etc.)
    instancer_ds: Arc<DataSourcePointInstancer>,
    /// Topology data source (prototypes, instanceIndices, mask)
    topology_ds: Arc<DataSourcePointInstancerTopology>,
}

impl std::fmt::Debug for DataSourcePointInstancerPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourcePointInstancerPrim")
            .field("path", &self.scene_index_path)
            .finish()
    }
}

impl DataSourcePointInstancerPrim {
    /// Create new point instancer prim data source.
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
        let instancer_ds = DataSourcePointInstancer::new(prim.clone(), stage_globals.clone());
        let topology_ds =
            DataSourcePointInstancerTopology::new(scene_index_path.clone(), prim, stage_globals);

        Arc::new(Self {
            scene_index_path,
            gprim_ds,
            instancer_ds,
            topology_ds,
        })
    }

    /// Compute invalidation for property changes.
    ///
    /// Combines gprim invalidation with instancer-specific:
    /// - prototypes -> full instancerTopology
    /// - protoIndices -> instancerTopology.instanceIndices
    /// - invisibleIds -> instancerTopology.mask
    /// - orientations/orientationsf -> primvars.instanceRotations
    /// - positions/scales/velocities/etc -> via custom primvar mappings
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Start with base gprim invalidation
        let mut locators =
            DataSourceGprim::invalidate(prim, subprim, properties, invalidation_type);

        if !subprim.is_empty() {
            return locators;
        }

        for prop in properties {
            let prop_str = prop.as_str();
            match prop_str {
                // Prototypes relationship change -> full topology resync
                "prototypes" => {
                    locators.insert(HdDataSourceLocator::from_token(
                        tokens::INSTANCER_TOPOLOGY.clone(),
                    ));
                }
                // ProtoIndices change -> instanceIndices in topology
                "protoIndices" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::INSTANCER_TOPOLOGY.clone(),
                        tokens::INSTANCE_INDICES.clone(),
                    ));
                }
                // InvisibleIds change -> mask in topology
                "invisibleIds" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::INSTANCER_TOPOLOGY.clone(),
                        tokens::MASK.clone(),
                    ));
                }
                // Orientations (both variants) -> instanceRotations primvar
                "orientations" | "orientationsf" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::INSTANCE_ROTATIONS.clone(),
                    ));
                }
                // Positions -> instanceTranslations primvar
                "positions" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::INSTANCE_TRANSLATIONS.clone(),
                    ));
                }
                // Scales -> instanceScales primvar
                "scales" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        tokens::INSTANCE_SCALES.clone(),
                    ));
                }
                // Velocities, accelerations, angularVelocities -> primvars
                "velocities" | "accelerations" | "angularVelocities" => {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        prop.clone(),
                    ));
                }
                _ => {}
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourcePointInstancerPrim {
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

impl HdContainerDataSource for DataSourcePointInstancerPrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.gprim_ds.get_names();
        names.push(tokens::INSTANCER.clone());
        names.push(tokens::INSTANCER_TOPOLOGY.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::INSTANCER {
            return Some(Arc::clone(&self.instancer_ds) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::INSTANCER_TOPOLOGY {
            return Some(Arc::clone(&self.topology_ds) as HdDataSourceBaseHandle);
        }
        // Fall through to gprim for primvars, xform, visibility, etc.
        self.gprim_ds.get(name)
    }
}

// ============================================================================
// PointInstancerAdapter
// ============================================================================

/// Adapter for UsdGeomPointInstancer prims.
///
/// Point instancers efficiently render many instances of prototype
/// geometry with per-instance transforms, visibility, and primvars.
///
/// Scene Index (Hydra 2.0) data flow:
/// 1. get_imaging_subprim_type -> "instancer"
/// 2. get_imaging_subprim_data -> DataSourcePointInstancerPrim containing:
///    - instancerTopology: prototypes, instanceIndices, mask
///    - primvars: instanceTranslations, instanceRotations, instanceScales
///    - xform, visibility, purpose (from base DataSourcePrim)
/// 3. invalidate_imaging_subprim maps USD property changes to Hydra locators
#[derive(Debug, Clone)]
pub struct PointInstancerAdapter;

impl Default for PointInstancerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PointInstancerAdapter {
    /// Create a new point instancer adapter.
    pub fn new() -> Self {
        Self
    }
}

impl PrimAdapter for PointInstancerAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Token {
        if subprim.is_empty() {
            tokens::INSTANCER.clone()
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
            Some(DataSourcePointInstancerPrim::new(
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
        DataSourcePointInstancerPrim::invalidate(prim, subprim, properties, invalidation_type)
    }

    fn should_cull_children(&self) -> bool {
        true
    }
}

// ============================================================================
// Factory
// ============================================================================

/// Factory for creating point instancer adapters.
pub fn create_point_instancer_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(PointInstancerAdapter::new())
}

/// Handle for PointInstancerAdapter.
pub type PointInstancerAdapterHandle = Arc<PointInstancerAdapter>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_point_instancer_adapter() {
        let adapter = PointInstancerAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "instancer");
    }

    #[test]
    fn test_point_instancer_subprims() {
        let adapter = PointInstancerAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let subprims = adapter.get_imaging_subprims(&prim);
        assert_eq!(subprims.len(), 1);
        assert!(subprims[0].is_empty());
    }

    #[test]
    fn test_point_instancer_cull_children() {
        let adapter = PointInstancerAdapter::new();
        assert!(adapter.should_cull_children());
    }

    #[test]
    fn test_point_instancer_data_source() {
        let adapter = PointInstancerAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_point_instancer_data_source_names() {
        let adapter = PointInstancerAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter
            .get_imaging_subprim_data(&prim, &Token::new(""), &globals)
            .unwrap();

        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "instancer"));
        assert!(names.iter().any(|n| n == "instancerTopology"));
    }

    #[test]
    fn test_point_instancer_get_instancer() {
        let adapter = PointInstancerAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter
            .get_imaging_subprim_data(&prim, &Token::new(""), &globals)
            .unwrap();

        // Should return instancer data source
        let instancer = ds.get(&Token::new("instancer"));
        assert!(instancer.is_some());
    }

    #[test]
    fn test_point_instancer_get_topology() {
        let adapter = PointInstancerAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter
            .get_imaging_subprim_data(&prim, &Token::new(""), &globals)
            .unwrap();

        let topology = ds.get(&Token::new("instancerTopology"));
        assert!(topology.is_some());
    }

    #[test]
    fn test_point_instancer_invalidation_positions() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("positions")];

        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_point_instancer_invalidation_orientations() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("orientations"), Token::new("orientationsf")];

        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_point_instancer_invalidation_proto_indices() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("protoIndices")];

        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_point_instancer_invalidation_invisible_ids() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("invisibleIds")];

        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_point_instancer_invalidation_prototypes() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("prototypes")];

        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_point_instancer_invalidation_scales() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("scales")];

        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_point_instancer_invalidation_non_empty_subprim() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("positions")];

        // Non-empty subprim should skip instancer-specific invalidation
        let locators = DataSourcePointInstancerPrim::invalidate(
            &prim,
            &Token::new("someSubprim"),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        // Only gprim base invalidation (which won't match "positions" for pseudo root)
        // The instancer-specific positions->primvars mapping is skipped
        assert!(locators.is_empty());
    }

    #[test]
    fn test_point_instancer_data() {
        let data = PointInstancerData::default();
        assert!(!data.initialized);
        assert!(data.visible);
        assert!(data.prototype_paths.is_empty());
        assert!(data.proto_prim_map.is_empty());
    }

    #[test]
    fn test_proto_prim_default() {
        let proto = ProtoPrim::default();
        assert!(proto.paths.is_empty());
        assert!(proto.adapter.is_none());
        assert!(proto.visible);
        assert_eq!(proto.variability_bits, 0);
    }

    #[test]
    fn test_factory() {
        let _ = create_point_instancer_adapter();
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
        assert!(names.iter().any(|n| n == "instanceIndices"));
        assert!(names.iter().any(|n| n == "mask"));
    }

    #[test]
    fn test_topology_no_prototypes_on_pseudo_root() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourcePointInstancerTopology::new(Path::absolute_root(), prim, globals);

        // Pseudo root has no prototypes relationship
        let prototypes = ds.get(&Token::new("prototypes"));
        assert!(prototypes.is_none());
    }
}
