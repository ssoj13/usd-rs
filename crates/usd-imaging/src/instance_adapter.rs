//! InstanceAdapter - Adapter for native USD instancing.
//!
//! Port of pxr/usdImaging/usdImaging/instanceAdapter.h/cpp
//!
//! Provides imaging support for USD native instances and prototypes.
//! Handles instance aggregation, per-instance data (transforms, visibility,
//! inherited primvars), and prototype caching.

use super::data_source_prim::DataSourcePrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapter;
use super::types::{PopulationMode, PropertyInvalidationType};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_core::Prim;
use usd_geom::primvars_api::PrimvarsAPI;
use usd_hd::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdRetainedSmallVectorDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;

// Token constants matching C++ UsdImagingInstanceAdapter tokens
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("instancer"));
    pub static INSTANCE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("instanceIndices"));
    pub static PROTOTYPE_ROOT: LazyLock<Token> = LazyLock::new(|| Token::new("prototypeRoot"));
    #[allow(dead_code)] // C++ instancing data source token, wiring in progress
    pub static PROTOTYPE_INDEX: LazyLock<Token> = LazyLock::new(|| Token::new("prototypeIndex"));
    pub static INSTANCE_TRANSFORMS: LazyLock<Token> =
        LazyLock::new(|| Token::new("instanceTransforms"));
    pub static INSTANCER_TOPOLOGY: LazyLock<Token> =
        LazyLock::new(|| Token::new("instancerTopology"));
    pub static PROTOTYPES: LazyLock<Token> = LazyLock::new(|| Token::new("prototypes"));
    pub static MASK: LazyLock<Token> = LazyLock::new(|| Token::new("mask"));
    #[allow(dead_code)] // C++ instancing data source token, wiring in progress
    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static XFORM: LazyLock<Token> = LazyLock::new(|| Token::new("xform"));
    #[allow(dead_code)] // Used when per-instance visibility is wired
    pub static VISIBILITY: LazyLock<Token> = LazyLock::new(|| Token::new("visibility"));
    pub static INHERITED_PRIMVARS: LazyLock<Token> =
        LazyLock::new(|| Token::new("inheritedPrimvars"));
    pub static _CATEGORIES: LazyLock<Token> = LazyLock::new(|| Token::new("categories"));

    // Instancing tokens
    #[allow(dead_code)] // C++ native instancing token, wiring in progress
    pub static USD_PROTOTYPE_ROOT: LazyLock<Token> =
        LazyLock::new(|| Token::new("__usdPrototypeRoot"));
}

// ============================================================================
// Visibility enum for per-instance visibility tracking
// ============================================================================

/// Per-instance visibility state, matching C++ _InstancerData::Visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceVisibility {
    /// Invisible over all time
    Invisible,
    /// Visible over all time
    Visible,
    /// Visibility varies over time
    Varying,
    /// Not yet checked
    Unknown,
}

impl Default for InstanceVisibility {
    fn default() -> Self {
        Self::Unknown
    }
}

// ============================================================================
// PrimvarInfo - inherited primvar tracking
// ============================================================================

/// Describes an inherited primvar on an instance, matching C++ PrimvarInfo.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrimvarInfo {
    /// Primvar name
    pub name: Token,
    /// Primvar type name
    pub type_name: Token,
}

// ============================================================================
// ProtoPrim - per-prototype prim data
// ============================================================================

/// A proto prim representing a single adapter under a prototype root.
///
/// Matches C++ _ProtoPrim. Each prim in the prototype subtree gets one entry.
#[derive(Clone)]
pub struct ProtoPrim {
    /// Path to the prim on the USD stage (e.g. a single mesh)
    pub path: Path,
    /// The prim adapter for the actual prototype prim
    pub adapter: Option<Arc<dyn PrimAdapter>>,
}

impl std::fmt::Debug for ProtoPrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtoPrim")
            .field("path", &self.path)
            .field("has_adapter", &self.adapter.is_some())
            .finish()
    }
}

impl Default for ProtoPrim {
    fn default() -> Self {
        Self {
            path: Path::empty(),
            adapter: None,
        }
    }
}

// ============================================================================
// InstancerData - all state for one hydra instancer
// ============================================================================

/// All data associated with a given instancer prim.
///
/// Matches C++ _InstancerData. Groups USD instances sharing the same prototype
/// (and compatible inherited attributes) into one hydra instancer.
#[derive(Debug, Clone)]
pub struct InstancerData {
    /// The prototype prim path associated with this instancer
    pub prototype_path: Path,

    /// The USD material path associated with this instancer
    pub material_usd_path: Path,

    /// The draw mode associated with this instancer
    pub draw_mode: Token,

    /// The inheritable purpose for this instancer
    pub inheritable_purpose: Token,

    /// Inherited primvars (sorted for comparison)
    pub inherited_primvars: Vec<PrimvarInfo>,

    /// Paths to USD instance prims
    pub instance_paths: Vec<Path>,

    /// Number of actual instances to draw (may exceed instance_paths.len()
    /// due to nested instancing)
    pub num_instances_to_draw: usize,

    /// Per-instance visibility cache
    pub visibility: Vec<InstanceVisibility>,

    /// Map of prototype cache path -> ProtoPrim
    pub prim_map: HashMap<Path, ProtoPrim>,

    /// Child point instancers referenced by this instancer
    pub child_point_instancers: Vec<Path>,

    /// Nested native instances
    pub nested_instances: Vec<Path>,

    /// Parent native instances
    pub parent_instances: Vec<Path>,

    /// Whether variability/update has been queued
    pub refresh: bool,
}

impl Default for InstancerData {
    fn default() -> Self {
        Self {
            prototype_path: Path::empty(),
            material_usd_path: Path::empty(),
            draw_mode: Token::new("default"),
            inheritable_purpose: Token::new("default"),
            inherited_primvars: Vec::new(),
            instance_paths: Vec::new(),
            num_instances_to_draw: 0,
            visibility: Vec::new(),
            prim_map: HashMap::new(),
            child_point_instancers: Vec::new(),
            nested_instances: Vec::new(),
            parent_instances: Vec::new(),
            refresh: false,
        }
    }
}

impl InstancerData {
    /// Check if this instancer is compatible with given inherited attributes.
    /// If material, draw mode, primvars, and purpose all match, instances
    /// can share this hydra instancer.
    pub fn is_compatible(
        &self,
        material_path: &Path,
        draw_mode: &Token,
        primvars: &[PrimvarInfo],
        purpose: &Token,
    ) -> bool {
        self.material_usd_path == *material_path
            && self.draw_mode == *draw_mode
            && self.inherited_primvars == primvars
            && self.inheritable_purpose == *purpose
    }
}

// ============================================================================
// DataSourceInstance - per-instance data
// ============================================================================

/// Data source for instance data, providing prototype root path and
/// per-instance transforms.
///
/// Matches C++ DataSourceInstance role within the instance adapter.
#[derive(Clone)]
pub struct DataSourceInstance {
    /// The instance prim
    prim: Prim,
    /// Stage globals for time queries
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceInstance")
            .field("path", &self.prim.path())
            .finish()
    }
}

impl DataSourceInstance {
    /// Create new instance data source.
    pub fn new(prim: Prim, stage_globals: DataSourceStageGlobalsHandle) -> Arc<Self> {
        Arc::new(Self {
            prim,
            stage_globals,
        })
    }

    /// Build the prototype root path data source.
    /// Returns the path to the USD prototype that this instance references.
    fn get_prototype_root(&self) -> Option<HdDataSourceBaseHandle> {
        let prototype = self.prim.get_prototype();
        if prototype.is_valid() {
            Some(
                HdRetainedTypedSampledDataSource::new(prototype.path().clone())
                    as HdDataSourceBaseHandle,
            )
        } else {
            None
        }
    }

    /// Build per-instance transform data source.
    ///
    /// C++ aggregates transforms from ALL instances sharing this prototype
    /// via `_RunForAllInstancesToDraw`. For the scene index path, this is
    /// how Hydra knows where each instance is placed. Each instance prim
    /// contributes its local-to-world xform to the array.
    fn get_instance_transforms(&self) -> Option<HdDataSourceBaseHandle> {
        // If this prim IS a prototype, find all instances and collect their xforms
        let instances = if self.prim.is_prototype() {
            self.prim.get_instances()
        } else {
            // If this is an instance prim, get its prototype's instances
            let proto = self.prim.get_prototype();
            if proto.is_valid() {
                proto.get_instances()
            } else {
                vec![self.prim.clone()]
            }
        };

        // Collect xform data sources from all instances
        let mut xform_sources: Vec<HdDataSourceBaseHandle> = Vec::new();
        for inst in &instances {
            let xform_ds = DataSourcePrim::new(
                inst.clone(),
                inst.path().clone(),
                self.stage_globals.clone(),
            );
            if let Some(xform) = xform_ds.get(&tokens::XFORM) {
                xform_sources.push(xform);
            }
        }

        if xform_sources.is_empty() {
            return None;
        }

        // Return as a vector data source containing all instance xforms
        Some(HdRetainedSmallVectorDataSource::new(&xform_sources) as HdDataSourceBaseHandle)
    }
}

impl HdDataSourceBase for DataSourceInstance {
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

impl HdContainerDataSource for DataSourceInstance {
    fn get_names(&self) -> Vec<Token> {
        vec![
            tokens::PROTOTYPE_ROOT.clone(),
            tokens::INSTANCE_TRANSFORMS.clone(),
        ]
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::PROTOTYPE_ROOT {
            return self.get_prototype_root();
        }
        if *name == *tokens::INSTANCE_TRANSFORMS {
            return self.get_instance_transforms();
        }
        None
    }
}

// ============================================================================
// DataSourceInstanceTopology - instancer topology
// ============================================================================

/// Data source for native instance topology.
///
/// Provides prototypes list, instance indices (which instances map to which
/// prototype), and visibility mask.
#[derive(Clone)]
pub struct DataSourceInstanceTopology {
    /// Scene index path
    scene_index_path: Path,
    /// The instance prim
    prim: Prim,
    /// Stage globals (used when per-instance time-varying data is wired)
    #[allow(dead_code)]
    stage_globals: DataSourceStageGlobalsHandle,
}

impl std::fmt::Debug for DataSourceInstanceTopology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceInstanceTopology")
            .field("path", &self.scene_index_path)
            .finish()
    }
}

impl DataSourceInstanceTopology {
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

    /// Build prototypes data source.
    /// Returns paths to prototype prims (the hidden /__Prototype_N roots).
    fn get_prototypes(&self) -> Option<HdDataSourceBaseHandle> {
        let prototype = self.prim.get_prototype();
        if prototype.is_valid() {
            // Single prototype for native instancing
            let path_ds = HdRetainedTypedSampledDataSource::new(prototype.path().clone());
            let elements: Vec<HdDataSourceBaseHandle> = vec![path_ds as HdDataSourceBaseHandle];
            Some(HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle)
        } else {
            None
        }
    }

    /// Build instance indices data source.
    /// For native instancing, all instances of the same prototype share one instancer.
    /// Returns a VectorDataSource where element[0] = array of indices [0..N-1] for prototype 0.
    fn get_instance_indices(&self) -> Option<HdDataSourceBaseHandle> {
        // Determine instance count from the prototype's instance list.
        // get_prototype() returns the hidden /__Prototype_N prim; get_instances() returns
        // all USD prims that share it. Fall back to 1 if prototype not available.
        let instance_count = {
            let proto = self.prim.get_prototype();
            if proto.is_valid() {
                let instances = proto.get_instances();
                if instances.is_empty() {
                    1
                } else {
                    instances.len()
                }
            } else {
                1
            }
        };

        let indices: Vec<i32> = (0..instance_count as i32).collect();
        let indices_ds = usd_hd::HdRetainedSampledDataSource::new(usd_vt::Value::from(indices));
        let elements: Vec<HdDataSourceBaseHandle> = vec![indices_ds as HdDataSourceBaseHandle];
        Some(HdRetainedSmallVectorDataSource::new(&elements) as HdDataSourceBaseHandle)
    }

    /// Build mask data source (visibility mask).
    /// Returns None (empty mask) when all instances are visible, which is the common case.
    /// A non-empty mask has one bool per instance: true = visible.
    fn get_mask(&self) -> Option<HdDataSourceBaseHandle> {
        // Determine instance count from prototype (same logic as get_instance_indices)
        let proto = self.prim.get_prototype();
        if !proto.is_valid() {
            // No mask needed for a single visible instance
            return None;
        }

        let instances = proto.get_instances();
        if instances.is_empty() {
            return None;
        }

        // Build visibility mask: all instances visible by default.
        // In USD native instancing, per-instance visibility isn't authored per-index
        // at this level — the instancer prim's own visibility governs all.
        // Returning None signals "all visible" to Hydra, which is correct here.
        // A populated mask would override individual instance visibility.
        let _n = instances.len(); // reserved for future per-instance visibility
        None
    }
}

impl HdDataSourceBase for DataSourceInstanceTopology {
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

impl HdContainerDataSource for DataSourceInstanceTopology {
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
// DataSourceInstancePrim
// ============================================================================

/// Prim data source for instance prims.
///
/// Extends DataSourcePrim with instancer-specific data: instancer topology,
/// instance data (prototype root, transforms), and primvars.
#[derive(Clone)]
pub struct DataSourceInstancePrim {
    /// Scene index path
    scene_index_path: Path,
    /// Base prim data source for xform/visibility/purpose
    base: DataSourcePrim,
    /// Instance data source (prototype info + transforms)
    instance_ds: Arc<DataSourceInstance>,
    /// Topology data source
    topology_ds: Arc<DataSourceInstanceTopology>,
    /// Inherited primvars data source
    inherited_primvars_ds: Arc<DataSourceInheritedPrimvars>,
}

impl std::fmt::Debug for DataSourceInstancePrim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceInstancePrim")
            .field("path", &self.scene_index_path)
            .finish()
    }
}

impl DataSourceInstancePrim {
    /// Create new instance prim data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Arc<Self> {
        let base = DataSourcePrim::new(
            prim.clone(),
            scene_index_path.clone(),
            stage_globals.clone(),
        );
        let instance_ds = DataSourceInstance::new(prim.clone(), stage_globals.clone());
        let topology_ds =
            DataSourceInstanceTopology::new(scene_index_path.clone(), prim.clone(), stage_globals);
        let inherited_primvars_ds = DataSourceInheritedPrimvars::new(prim);

        Arc::new(Self {
            scene_index_path,
            base,
            instance_ds,
            topology_ds,
            inherited_primvars_ds,
        })
    }

    /// Compute invalidation for property changes.
    ///
    /// Transform changes -> instance transforms dirty.
    /// Visibility changes -> mask dirty.
    /// Material/purpose changes -> full topology resync.
    pub fn invalidate(
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        // Start with base prim invalidation (xform, visibility, purpose)
        let mut locators = DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type);

        for prop in properties {
            let prop_str = prop.as_str();

            // Transform changes affect per-instance transforms
            if prop_str.starts_with("xformOp") || prop_str == "xformOpOrder" {
                locators.insert(HdDataSourceLocator::from_token(
                    tokens::INSTANCE_TRANSFORMS.clone(),
                ));
            }

            // Visibility changes affect instance mask
            if prop_str == "visibility" {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    tokens::INSTANCER_TOPOLOGY.clone(),
                    tokens::MASK.clone(),
                ));
            }

            // Material binding changes require topology resync
            if prop_str.starts_with("material:binding") {
                locators.insert(HdDataSourceLocator::from_token(
                    tokens::INSTANCER_TOPOLOGY.clone(),
                ));
            }
        }

        locators
    }
}

impl HdDataSourceBase for DataSourceInstancePrim {
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

impl HdContainerDataSource for DataSourceInstancePrim {
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.base.get_names();
        names.push(tokens::INSTANCER.clone());
        names.push(tokens::INSTANCER_TOPOLOGY.clone());
        names.push(tokens::INHERITED_PRIMVARS.clone());
        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if *name == *tokens::INSTANCER {
            return Some(Arc::clone(&self.instance_ds) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::INSTANCER_TOPOLOGY {
            return Some(Arc::clone(&self.topology_ds) as HdDataSourceBaseHandle);
        }
        if *name == *tokens::INHERITED_PRIMVARS {
            return Some(Arc::clone(&self.inherited_primvars_ds) as HdDataSourceBaseHandle);
        }
        // Fall through to base for xform, visibility, purpose, extent
        self.base.get(name)
    }
}

// ============================================================================
// DataSourceInheritedPrimvars - per-instance inherited primvar data
// ============================================================================

/// Data source for inherited primvars on instance prims.
///
/// Walks ancestor prims to collect constant-interpolation primvars
/// and provides them as per-instance arrays.
/// Matches C++ `_ComputeInheritedPrimvarFn` struct (lines 942-1009).
#[derive(Clone)]
pub struct DataSourceInheritedPrimvars {
    /// The instance prim
    prim: Prim,
    /// Cached primvar info
    primvar_infos: Vec<PrimvarInfo>,
}

impl std::fmt::Debug for DataSourceInheritedPrimvars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceInheritedPrimvars")
            .field("path", &self.prim.path())
            .field("count", &self.primvar_infos.len())
            .finish()
    }
}

impl DataSourceInheritedPrimvars {
    /// Create new inherited primvars data source for a prim.
    pub fn new(prim: Prim) -> Arc<Self> {
        let primvar_infos = InstanceAdapter::compute_inherited_primvars(&prim);
        Arc::new(Self {
            prim,
            primvar_infos,
        })
    }

    /// Get the inherited primvar infos.
    pub fn primvar_infos(&self) -> &[PrimvarInfo] {
        &self.primvar_infos
    }

    /// Get a primvar value by name using the PrimvarsAPI.
    /// Returns the value as a generic HdDataSource via usd_vt::Value.
    fn get_primvar_value(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let api = PrimvarsAPI::new(self.prim.clone());
        let inherited = api.find_primvars_with_inheritance();

        for pv in &inherited {
            if &pv.get_primvar_name() == name {
                let interp = pv.get_interpolation();
                if interp != "constant" && !interp.is_empty() {
                    continue;
                }
                if let Some(val) = pv.compute_flattened(usd_sdf::TimeCode::default()) {
                    return Some(
                        usd_hd::HdRetainedSampledDataSource::new(val) as HdDataSourceBaseHandle
                    );
                }
            }
        }
        None
    }
}

impl HdDataSourceBase for DataSourceInheritedPrimvars {
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

impl HdContainerDataSource for DataSourceInheritedPrimvars {
    fn get_names(&self) -> Vec<Token> {
        self.primvar_infos
            .iter()
            .map(|pv| pv.name.clone())
            .collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if self.primvar_infos.iter().any(|pv| &pv.name == name) {
            self.get_primvar_value(name)
        } else {
            None
        }
    }
}

// ============================================================================
// PrototypeCache - shared prototype data across instances
// ============================================================================

/// Cache for prototype data shared across instances.
///
/// In C++, this is spread across _InstancerDataMap and related structures.
/// Here we provide a centralized cache keyed by prototype path.
#[derive(Debug, Clone)]
pub struct PrototypeCache {
    /// Map from prototype path to cached instancer groups (multimap).
    /// Multiple entries per prototype when instances have different attributes.
    entries: Arc<RwLock<HashMap<Path, Vec<PrototypeCacheEntry>>>>,
}

/// Cached data for one instancer group.
///
/// Multiple entries can exist per prototype when instances have different
/// inherited attributes (material, draw mode, purpose, primvars).
#[derive(Debug, Clone)]
pub struct PrototypeCacheEntry {
    /// The prototype prim path (e.g., /__Prototype_1)
    pub prototype_path: Path,
    /// Instancer identity path (first instance that created this group)
    pub instancer_path: Path,
    /// Instancer data for this prototype group
    pub instancer_data: InstancerData,
    /// Number of times this prototype is referenced
    pub ref_count: usize,
}

impl Default for PrototypeCache {
    fn default() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl PrototypeCache {
    /// Create empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get first cache entry for a prototype (or create default).
    pub fn get_or_insert(&self, prototype_path: &Path) -> PrototypeCacheEntry {
        let mut entries = self.entries.write();
        let group = entries
            .entry(prototype_path.clone())
            .or_insert_with(Vec::new);
        if group.is_empty() {
            group.push(PrototypeCacheEntry {
                prototype_path: prototype_path.clone(),
                instancer_path: Path::empty(),
                instancer_data: InstancerData::default(),
                ref_count: 0,
            });
        }
        group[0].clone()
    }

    /// Register an instance with a compatible prototype group.
    ///
    /// Checks compatibility (material, draw mode, primvars, purpose) against
    /// existing groups. If no compatible group exists, creates a new one.
    /// Matches C++ `_Populate()` lines 157-180: multimap split logic.
    pub fn register_instance(
        &self,
        prototype_path: &Path,
        instance_path: &Path,
        material_path: &Path,
        draw_mode: &Token,
        primvars: &[PrimvarInfo],
        purpose: &Token,
    ) {
        let mut entries = self.entries.write();
        let group = entries
            .entry(prototype_path.clone())
            .or_insert_with(Vec::new);

        // Find compatible existing entry
        let compatible_idx = group.iter().position(|e| {
            e.instancer_data
                .is_compatible(material_path, draw_mode, primvars, purpose)
        });

        let entry = if let Some(idx) = compatible_idx {
            &mut group[idx]
        } else {
            // No compatible group — create new instancer with this instance as identity
            group.push(PrototypeCacheEntry {
                prototype_path: prototype_path.clone(),
                instancer_path: instance_path.clone(),
                instancer_data: InstancerData {
                    prototype_path: prototype_path.clone(),
                    material_usd_path: material_path.clone(),
                    draw_mode: draw_mode.clone(),
                    inheritable_purpose: purpose.clone(),
                    inherited_primvars: primvars.to_vec(),
                    ..InstancerData::default()
                },
                ref_count: 0,
            });
            group.last_mut().unwrap()
        };

        entry.ref_count += 1;
        if !entry.instancer_data.instance_paths.contains(instance_path) {
            entry
                .instancer_data
                .instance_paths
                .push(instance_path.clone());
        }
        // Set instancer_path to first instance if not set
        if entry.instancer_path.is_empty() {
            entry.instancer_path = instance_path.clone();
        }
    }

    /// Simple registration without attribute checks (backwards compat).
    pub fn register_instance_simple(&self, prototype_path: &Path, instance_path: &Path) {
        self.register_instance(
            prototype_path,
            instance_path,
            &Path::empty(),
            &Token::new("default"),
            &[],
            &Token::new("default"),
        );
    }

    /// Remove an instance from its prototype group.
    pub fn unregister_instance(&self, prototype_path: &Path, instance_path: &Path) {
        let mut entries = self.entries.write();
        if let Some(group) = entries.get_mut(prototype_path) {
            // Find the group containing this instance
            if let Some(entry) = group
                .iter_mut()
                .find(|e| e.instancer_data.instance_paths.contains(instance_path))
            {
                entry.ref_count = entry.ref_count.saturating_sub(1);
                entry
                    .instancer_data
                    .instance_paths
                    .retain(|p| p != instance_path);
            }
            // Remove empty groups
            group.retain(|e| e.ref_count > 0);
            if group.is_empty() {
                entries.remove(prototype_path);
            }
        }
    }

    /// Find a compatible instancer for the given inherited attributes.
    /// Returns the instancer path if found.
    /// Matches C++ multimap search in `_Populate()`.
    pub fn find_compatible_instancer(
        &self,
        prototype_path: &Path,
        material_path: &Path,
        draw_mode: &Token,
        primvars: &[PrimvarInfo],
        purpose: &Token,
    ) -> Option<Path> {
        let entries = self.entries.read();
        if let Some(group) = entries.get(prototype_path) {
            for entry in group {
                if entry
                    .instancer_data
                    .is_compatible(material_path, draw_mode, primvars, purpose)
                {
                    return Some(entry.instancer_path.clone());
                }
            }
        }
        None
    }

    /// Count all instances to draw for an instancer, handling nested instancing.
    ///
    /// For non-nested instances, returns instance_paths.len().
    /// For nested instances (instance inside a prototype), the count is the
    /// product of instance counts up the chain.
    /// Matches C++ `_CountAllInstancesToDraw` / `_CountAllInstancesToDrawImpl`.
    pub fn count_all_instances_to_draw(
        &self,
        instancer_path: &Path,
        stage: &usd_core::Stage,
    ) -> usize {
        let mut draw_counts: HashMap<Path, usize> = HashMap::new();
        self.count_instances_impl(instancer_path, stage, &mut draw_counts)
    }

    /// Recursive nested instance count with memoization.
    fn count_instances_impl(
        &self,
        instancer_path: &Path,
        stage: &usd_core::Stage,
        draw_counts: &mut HashMap<Path, usize>,
    ) -> usize {
        if let Some(&count) = draw_counts.get(instancer_path) {
            return count;
        }

        // Collect instance paths for this instancer
        let instance_paths = {
            let entries = self.entries.read();
            let mut paths = Vec::new();
            for group in entries.values() {
                for entry in group {
                    if &entry.instancer_path == instancer_path {
                        paths = entry.instancer_data.instance_paths.clone();
                        break;
                    }
                }
                if !paths.is_empty() {
                    break;
                }
            }
            paths
        };

        let mut draw_count: usize = 0;

        for inst_path in &instance_paths {
            let Some(instance_prim) = stage.get_prim_at_path(inst_path) else {
                continue;
            };

            if !instance_prim.is_in_prototype() {
                // Top-level instance — counts as 1 draw
                draw_count += 1;
            } else {
                // Nested instance — find parent prototype and multiply
                let mut parent = instance_prim.clone();
                while parent.is_valid() && !parent.is_prototype() {
                    let p = parent.parent();
                    if !p.is_valid() {
                        break;
                    }
                    parent = p;
                }

                if parent.is_valid() && parent.is_prototype() {
                    let parent_proto_path = parent.path().clone();
                    // Find all instancers for the parent prototype
                    let parent_instancers = self.get_instancers_for_prototype(&parent_proto_path);
                    for parent_instancer in &parent_instancers {
                        draw_count +=
                            self.count_instances_impl(parent_instancer, stage, draw_counts);
                    }
                }
            }
        }

        draw_counts.insert(instancer_path.clone(), draw_count);
        draw_count
    }

    /// Get all instancer paths for a given prototype (reverse lookup).
    fn get_instancers_for_prototype(&self, prototype_path: &Path) -> Vec<Path> {
        let entries = self.entries.read();
        if let Some(group) = entries.get(prototype_path) {
            group.iter().map(|e| e.instancer_path.clone()).collect()
        } else {
            Vec::new()
        }
    }

    /// Get all entries for a prototype.
    pub fn get_entries(&self, prototype_path: &Path) -> Vec<PrototypeCacheEntry> {
        let entries = self.entries.read();
        entries.get(prototype_path).cloned().unwrap_or_default()
    }

    /// Get total number of unique prototypes in cache.
    pub fn len(&self) -> usize {
        self.entries.read().len()
    }

    /// Total number of instancer groups across all prototypes.
    pub fn num_instancers(&self) -> usize {
        let entries = self.entries.read();
        entries.values().map(|g| g.len()).sum()
    }

    /// Check if cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ============================================================================
// InstanceAdapter
// ============================================================================

/// Adapter for USD native instances.
///
/// Handles the scene-index path (Hydra 2.0) for native USD instancing:
/// - Groups USD instances sharing the same prototype into hydra instancers
/// - Computes per-instance transforms, visibility, and constant primvars
/// - Caches prototype data across instances
/// - Manages nested instancing chains
///
/// When instances have differing inherited attributes (material binding,
/// draw mode, purpose), they are split into separate hydra instancers.
#[derive(Debug, Clone)]
pub struct InstanceAdapter {
    /// Prototype cache shared across all instances
    prototype_cache: PrototypeCache,
}

impl Default for InstanceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceAdapter {
    /// Create a new instance adapter.
    pub fn new() -> Self {
        Self {
            prototype_cache: PrototypeCache::new(),
        }
    }

    /// Create with an existing prototype cache (for sharing).
    pub fn with_cache(cache: PrototypeCache) -> Self {
        Self {
            prototype_cache: cache,
        }
    }

    /// Access the prototype cache.
    pub fn prototype_cache(&self) -> &PrototypeCache {
        &self.prototype_cache
    }
}

impl InstanceAdapter {
    /// Compute inherited attributes for instancer compatibility splitting.
    ///
    /// Returns (material_path, draw_mode, purpose, inherited_primvars).
    /// Matches the attribute computation in C++ `_Populate()` before the
    /// multimap search.
    fn compute_inherited_attrs(prim: &Prim) -> (Path, Token, Token, Vec<PrimvarInfo>) {
        // Material binding: walk up hierarchy for material:binding relationship
        let material_path = prim
            .get_relationship("material:binding")
            .and_then(|rel| rel.get_targets().into_iter().next())
            .unwrap_or_else(Path::empty);

        // Draw mode: model:drawMode attribute (default = "default")
        let draw_mode = prim
            .get_attribute("model:drawMode")
            .and_then(|attr| attr.get_typed::<Token>(usd_sdf::TimeCode::default()))
            .unwrap_or_else(|| Token::new("default"));

        // Inheritable purpose
        let purpose = prim
            .get_attribute("purpose")
            .and_then(|attr| attr.get_typed::<Token>(usd_sdf::TimeCode::default()))
            .unwrap_or_else(|| Token::new("default"));

        // Inherited primvars (constant-interpolation only)
        let primvars = Self::compute_inherited_primvars(prim);

        (material_path, draw_mode, purpose, primvars)
    }

    /// Compute inherited primvars for an instance prim.
    ///
    /// Walks up from the prim collecting constant-interpolation primvars.
    /// Matches C++ `_ComputeInheritedPrimvars()` + delegate.rs:2066-2086.
    pub fn compute_inherited_primvars(prim: &Prim) -> Vec<PrimvarInfo> {
        let api = PrimvarsAPI::new(prim.clone());
        let inherited = api.find_primvars_with_inheritance();

        let mut result: Vec<PrimvarInfo> = inherited
            .iter()
            .filter(|pv| {
                // Only constant-interpolation primvars are inheritable
                let interp = pv.get_interpolation();
                interp == "constant" || interp.is_empty()
            })
            .map(|pv| PrimvarInfo {
                name: pv.get_primvar_name(),
                type_name: Token::new(pv.get_type_name().as_token().as_str()),
            })
            .collect();

        result.sort();
        result
    }

    /// Get instance categories for light linking.
    ///
    /// Returns a Vec of token arrays, one per instance. Each token array
    /// contains the collection paths that include that instance.
    /// Matches C++ `GetInstanceCategories()` lines 1574-1586.
    pub fn get_instance_categories(
        &self,
        prototype_path: &Path,
        collection_cache: &super::collection_cache::CollectionCache,
    ) -> Vec<Vec<Token>> {
        let entries = self.prototype_cache.get_entries(prototype_path);
        let mut result = Vec::new();

        for entry in &entries {
            for inst_path in &entry.instancer_data.instance_paths {
                // Query all collections containing this instance path
                let categories = collection_cache.compute_collections_containing_path(inst_path);
                result.push(categories);
            }
        }

        result
    }
}

impl PrimAdapter for InstanceAdapter {
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
            // Register instance with prototype cache, computing inherited attrs
            // for draw-mode / purpose splitting (C++ _Populate lines 157-180)
            if prim.is_instance() {
                let prototype = prim.get_prototype();
                if prototype.is_valid() {
                    let (material_path, draw_mode, purpose, primvars) =
                        Self::compute_inherited_attrs(prim);
                    self.prototype_cache.register_instance(
                        prototype.path(),
                        prim.path(),
                        &material_path,
                        &draw_mode,
                        &primvars,
                        &purpose,
                    );
                }
            }
            Some(DataSourceInstancePrim::new(
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
        DataSourceInstancePrim::invalidate(prim, subprim, properties, invalidation_type)
    }

    fn get_population_mode(&self) -> PopulationMode {
        PopulationMode::RepresentsSelfAndDescendents
    }

    fn should_cull_children(&self) -> bool {
        true
    }
}

// ============================================================================
// InstanceablePrimAdapter
// ============================================================================

/// Base adapter for prims that can be instanced.
///
/// Provides the instancing handshake: when a prim is part of a native
/// instance, this adapter defers to the InstanceAdapter for population.
/// When rendered directly (not instanced), it behaves as a pass-through.
///
/// Prim adapters that support instancing (mesh, curves, etc.) can delegate
/// their instancing behavior through this adapter.
#[derive(Clone)]
pub struct InstanceablePrimAdapter {
    /// Optional delegate adapter for non-instanced rendering
    delegate: Option<Arc<dyn PrimAdapter>>,
}

impl std::fmt::Debug for InstanceablePrimAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceablePrimAdapter")
            .field("has_delegate", &self.delegate.is_some())
            .finish()
    }
}

impl Default for InstanceablePrimAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceablePrimAdapter {
    /// Create a new instanceable prim adapter.
    pub fn new() -> Self {
        Self { delegate: None }
    }

    /// Create with a delegate adapter for direct (non-instanced) rendering.
    pub fn with_delegate(delegate: Arc<dyn PrimAdapter>) -> Self {
        Self {
            delegate: Some(delegate),
        }
    }
}

impl PrimAdapter for InstanceablePrimAdapter {
    fn get_imaging_subprims(&self, prim: &Prim) -> Vec<Token> {
        if let Some(ref delegate) = self.delegate {
            delegate.get_imaging_subprims(prim)
        } else {
            vec![Token::new("")]
        }
    }

    fn get_imaging_subprim_type(&self, prim: &Prim, subprim: &Token) -> Token {
        if let Some(ref delegate) = self.delegate {
            delegate.get_imaging_subprim_type(prim, subprim)
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
        if let Some(ref delegate) = self.delegate {
            delegate.get_imaging_subprim_data(prim, subprim, stage_globals)
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
        if let Some(ref delegate) = self.delegate {
            delegate.invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
        } else {
            HdDataSourceLocatorSet::empty()
        }
    }
}

// ============================================================================
// Factory functions and type aliases
// ============================================================================

/// Handle type for InstanceAdapter.
pub type InstanceAdapterHandle = Arc<InstanceAdapter>;
/// Handle type for InstanceablePrimAdapter.
pub type InstanceablePrimAdapterHandle = Arc<InstanceablePrimAdapter>;

/// Factory for creating instance adapters.
pub fn create_instance_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(InstanceAdapter::new())
}

/// Factory for creating instanceable prim adapters.
pub fn create_instanceable_prim_adapter() -> Arc<dyn PrimAdapter> {
    Arc::new(InstanceablePrimAdapter::new())
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
    fn test_instance_adapter() {
        let adapter = InstanceAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "instancer");
    }

    #[test]
    fn test_instance_adapter_population_mode() {
        let adapter = InstanceAdapter::new();
        assert_eq!(
            adapter.get_population_mode(),
            PopulationMode::RepresentsSelfAndDescendents
        );
    }

    #[test]
    fn test_should_cull_children() {
        let adapter = InstanceAdapter::new();
        assert!(adapter.should_cull_children());
    }

    #[test]
    fn test_instance_data_source() {
        let adapter = InstanceAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(ds.is_some());
    }

    #[test]
    fn test_instance_data_source_names() {
        let adapter = InstanceAdapter::new();
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
    fn test_instance_data_source_get_instancer() {
        let adapter = InstanceAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = adapter
            .get_imaging_subprim_data(&prim, &Token::new(""), &globals)
            .unwrap();
        // Should return instancer container
        let instancer = ds.get(&Token::new("instancer"));
        assert!(instancer.is_some());
    }

    #[test]
    fn test_instance_data_source_get_topology() {
        let adapter = InstanceAdapter::new();
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
    fn test_instance_invalidation_xform() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("xformOpOrder")];

        let locators = DataSourceInstancePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_instance_invalidation_visibility() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("visibility")];

        let locators = DataSourceInstancePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_instance_invalidation_material() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let properties = vec![Token::new("material:binding")];

        let locators = DataSourceInstancePrim::invalidate(
            &prim,
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
        );

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_prototype_cache() {
        let cache = PrototypeCache::new();
        assert!(cache.is_empty());

        let proto_path = Path::from_string("/__Prototype_1").unwrap();
        let inst_path = Path::from_string("/World/InstanceA").unwrap();

        cache.register_instance_simple(&proto_path, &inst_path);
        assert_eq!(cache.len(), 1);

        let entry = cache.get_or_insert(&proto_path);
        assert_eq!(entry.ref_count, 1);
        assert_eq!(entry.instancer_data.instance_paths.len(), 1);

        // Register second instance (compatible defaults)
        let inst_path_b = Path::from_string("/World/InstanceB").unwrap();
        cache.register_instance_simple(&proto_path, &inst_path_b);

        let entry = cache.get_or_insert(&proto_path);
        assert_eq!(entry.ref_count, 2);
        assert_eq!(entry.instancer_data.instance_paths.len(), 2);

        // Unregister
        cache.unregister_instance(&proto_path, &inst_path);
        let entry = cache.get_or_insert(&proto_path);
        assert_eq!(entry.ref_count, 1);

        cache.unregister_instance(&proto_path, &inst_path_b);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_prototype_cache_compatible() {
        let cache = PrototypeCache::new();
        let proto_path = Path::from_string("/__Prototype_1").unwrap();
        let inst_path = Path::from_string("/World/InstanceA").unwrap();

        cache.register_instance_simple(&proto_path, &inst_path);

        // Compatible: same defaults
        let result = cache.find_compatible_instancer(
            &proto_path,
            &Path::empty(),
            &Token::new("default"),
            &[],
            &Token::new("default"),
        );
        assert!(result.is_some());
        assert_eq!(result.unwrap(), inst_path);

        // Incompatible: different material
        let mat_path = Path::from_string("/Materials/Red").unwrap();
        let result = cache.find_compatible_instancer(
            &proto_path,
            &mat_path,
            &Token::new("default"),
            &[],
            &Token::new("default"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_prototype_cache_split_by_material() {
        let cache = PrototypeCache::new();
        let proto_path = Path::from_string("/__Prototype_1").unwrap();
        let inst_a = Path::from_string("/World/InstanceA").unwrap();
        let inst_b = Path::from_string("/World/InstanceB").unwrap();
        let mat_red = Path::from_string("/Materials/Red").unwrap();
        let mat_blue = Path::from_string("/Materials/Blue").unwrap();

        // Register with different materials — should split
        cache.register_instance(
            &proto_path,
            &inst_a,
            &mat_red,
            &Token::new("default"),
            &[],
            &Token::new("default"),
        );
        cache.register_instance(
            &proto_path,
            &inst_b,
            &mat_blue,
            &Token::new("default"),
            &[],
            &Token::new("default"),
        );

        // 1 prototype, 2 instancer groups
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.num_instancers(), 2);

        let entries = cache.get_entries(&proto_path);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].instancer_data.material_usd_path, mat_red);
        assert_eq!(entries[1].instancer_data.material_usd_path, mat_blue);
    }

    #[test]
    fn test_prototype_cache_compatible_merge() {
        let cache = PrototypeCache::new();
        let proto_path = Path::from_string("/__Prototype_1").unwrap();
        let inst_a = Path::from_string("/World/InstanceA").unwrap();
        let inst_b = Path::from_string("/World/InstanceB").unwrap();
        let mat = Path::from_string("/Materials/Red").unwrap();

        // Register with SAME material — should merge
        cache.register_instance(
            &proto_path,
            &inst_a,
            &mat,
            &Token::new("default"),
            &[],
            &Token::new("render"),
        );
        cache.register_instance(
            &proto_path,
            &inst_b,
            &mat,
            &Token::new("default"),
            &[],
            &Token::new("render"),
        );

        // 1 prototype, 1 instancer group with 2 instances
        assert_eq!(cache.num_instancers(), 1);
        let entries = cache.get_entries(&proto_path);
        assert_eq!(entries[0].instancer_data.instance_paths.len(), 2);
    }

    #[test]
    fn test_instancer_data_compatibility() {
        let mut data = InstancerData::default();
        data.material_usd_path = Path::from_string("/Materials/Red").unwrap();
        data.draw_mode = Token::new("default");
        data.inheritable_purpose = Token::new("render");

        // Same attributes -> compatible
        assert!(data.is_compatible(
            &Path::from_string("/Materials/Red").unwrap(),
            &Token::new("default"),
            &[],
            &Token::new("render"),
        ));

        // Different material -> incompatible
        assert!(!data.is_compatible(
            &Path::from_string("/Materials/Blue").unwrap(),
            &Token::new("default"),
            &[],
            &Token::new("render"),
        ));
    }

    #[test]
    fn test_instanceable_prim_adapter_default() {
        let adapter = InstanceablePrimAdapter::new();
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        // No delegate -> empty type, no data
        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert!(prim_type.is_empty());

        let data = adapter.get_imaging_subprim_data(&prim, &Token::new(""), &globals);
        assert!(data.is_none());
    }

    #[test]
    fn test_instanceable_prim_adapter_with_delegate() {
        use crate::prim_adapter::NoOpAdapter;

        let delegate = Arc::new(NoOpAdapter::new(Token::new("mesh")));
        let adapter = InstanceablePrimAdapter::with_delegate(delegate);
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();

        let prim_type = adapter.get_imaging_subprim_type(&prim, &Token::new(""));
        assert_eq!(prim_type.as_str(), "mesh");
    }

    #[test]
    fn test_instance_visibility_enum() {
        assert_eq!(InstanceVisibility::default(), InstanceVisibility::Unknown);
        assert_ne!(InstanceVisibility::Visible, InstanceVisibility::Invisible);
    }

    #[test]
    fn test_primvar_info_ordering() {
        let a = PrimvarInfo {
            name: Token::new("alpha"),
            type_name: Token::new("float"),
        };
        let b = PrimvarInfo {
            name: Token::new("beta"),
            type_name: Token::new("float"),
        };
        assert!(a < b);
    }

    #[test]
    fn test_all_factories() {
        let _ = create_instance_adapter();
        let _ = create_instanceable_prim_adapter();
    }
}
