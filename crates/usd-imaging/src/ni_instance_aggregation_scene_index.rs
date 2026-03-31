
//! Native instance aggregation scene index.
//!
//! Aggregates multiple instances of the same prototype for efficient batch rendering.
//! This is the final optimization step in native instancing - collecting all instances
//! that reference the same prototype so render backends can process them together.
//!
//! # Batch Rendering
//!
//! Modern renderers can efficiently draw many instances of the same geometry
//! in a single draw call. This scene index groups instances by their prototype
//! to enable this optimization.
//!
//! # Data Structure
//!
//! Creates aggregated data structures like:
//! - List of all instances per prototype
//! - Transform matrices for each instance
//! - Instance-specific attributes (visibility, etc.)
//!
//! # Pipeline Position
//!
//! Typically the final step in native instancing chain:
//! 1. Prototype scene index (identifies prototypes)
//! 2. Prototype propagating scene index (copies data to instances)
//! 3. Prototype pruning scene index (removes prototype prims)
//! 4. This aggregation index (groups instances for batch rendering)
//!
//! # References
//!
//! OpenUSD: `pxr/usdImaging/usdImaging/niInstanceAggregationSceneIndex.h`

use crate::data_source_relocating_scene_index::UsdImagingDataSourceRelocatingSceneIndex;
use crate::ni_instance_observer::InstanceObserver;
use std::sync::{Arc, Weak};
use parking_lot::RwLock;
use usd_hd::HdDataSourceBaseHandle;
use usd_hd::data_source::HdDataSourceLocator;
use usd_hd::scene_index::base::HdSceneIndexBaseImpl;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
    convert_prims_renamed_to_removed_and_added,
};
use usd_hd::scene_index::{
    HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexObserver,
    HdSceneIndexObserverHandle, HdSceneIndexPrim, SdfPathVector, scene_index_to_handle,
};
use usd_hd::schema::HdPrimvarsSchema;
use usd_hd::skinning_settings::is_skinning_deferred;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Token names for instance aggregation.
#[allow(dead_code)]
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Token for instance indices attribute.
    pub static INSTANCE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("instanceIndices"));

    /// Token for instance transforms attribute.
    pub static INSTANCE_TRANSFORMS: LazyLock<Token> =
        LazyLock::new(|| Token::new("instanceTransforms"));

    /// Token for aggregation enabled configuration.
    pub static AGGREGATION_ENABLED: LazyLock<Token> =
        LazyLock::new(|| Token::new("aggregationEnabled"));

    /// Token for prototype path attribute.
    pub static PROTOTYPE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("prototypePath"));
}

/// Information about aggregated instances of a prototype.
#[derive(Debug, Clone)]
pub struct InstanceAggregationInfo {
    /// Path of the prototype these instances reference
    pub prototype_path: SdfPath,
    /// Paths of all instances referencing this prototype
    pub instance_paths: Vec<SdfPath>,
}

impl InstanceAggregationInfo {
    /// Creates new aggregation info for a prototype.
    pub fn new(prototype_path: SdfPath) -> Self {
        Self {
            prototype_path,
            instance_paths: Vec::new(),
        }
    }

    /// Returns the number of instances referencing this prototype.
    #[inline]
    pub fn instance_count(&self) -> usize {
        self.instance_paths.len()
    }

    /// Adds an instance to this aggregation.
    pub fn add_instance(&mut self, instance_path: SdfPath) {
        self.instance_paths.push(instance_path);
    }

    /// Removes an instance from this aggregation.
    pub fn remove_instance(&mut self, instance_path: &SdfPath) -> bool {
        if let Some(pos) = self.instance_paths.iter().position(|p| p == instance_path) {
            self.instance_paths.remove(pos);
            true
        } else {
            false
        }
    }
}

/// Wrapper to expose InstanceObserver as HdSceneIndexObserverHandle.
/// Needed because Arc<RwLock<InstanceObserver>> does not coerce to
/// Arc<RwLock<dyn HdSceneIndexObserver>>.
struct InstanceObserverForwarder(Arc<RwLock<InstanceObserver>>);

impl HdSceneIndexObserver for InstanceObserverForwarder {
    fn prims_added(&self, sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.0.write().prims_added(sender, entries);
    }
    fn prims_removed(&self, sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.0.write().prims_removed(sender, entries);
    }
    fn prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.0.write().prims_dirtied(sender, entries);
    }
    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.0.write().prims_renamed(sender, entries);
    }
}

/// Observer that forwards events from the retained scene index to the
/// owning aggregation scene index's observers.
///
/// Port of _RetainedSceneIndexObserver in niInstanceAggregationSceneIndex.cpp.
struct RetainedSceneIndexObserver {
    owner: Weak<RwLock<UsdImagingNiInstanceAggregationSceneIndex>>,
}

impl RetainedSceneIndexObserver {
    fn new(owner: Weak<RwLock<UsdImagingNiInstanceAggregationSceneIndex>>) -> Self {
        Self { owner }
    }
}

impl HdSceneIndexObserver for RetainedSceneIndexObserver {
    fn prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = unsafe { &*owner_arc.data_ptr() };
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_added(owner_sender, entries);
        }
    }

    fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = unsafe { &*owner_arc.data_ptr() };
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_removed(owner_sender, entries);
        }
    }

    fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = unsafe { &*owner_arc.data_ptr() };
            let owner_sender: &dyn HdSceneIndexBase = owner;
            owner.base.send_prims_dirtied(owner_sender, entries);
        }
    }

    fn prims_renamed(&self, sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        let (removed, added) = convert_prims_renamed_to_removed_and_added(sender, entries);
        if let Some(owner_arc) = self.owner.upgrade() {
            let owner = unsafe { &*owner_arc.data_ptr() };
            let owner_sender: &dyn HdSceneIndexBase = owner;
            if !removed.is_empty() {
                owner.base.send_prims_removed(owner_sender, &removed);
            }
            if !added.is_empty() {
                owner.base.send_prims_added(owner_sender, &added);
            }
        }
    }
}

/// Native instance aggregation scene index.
///
/// Groups instances by prototype for efficient batch rendering.
/// Delegates to an InstanceObserver that populates a retained scene index
/// with aggregated instancers. GetPrim/GetChildPrimPaths come from the retained
/// scene. Observers receive notifications from the retained scene.
///
/// Port of UsdImaging_NiInstanceAggregationSceneIndex.
pub struct UsdImagingNiInstanceAggregationSceneIndex {
    /// Base for observer management and notifications
    pub(crate) base: HdSceneIndexBaseImpl,
    /// Observer that aggregates instances into instancers
    instance_observer: Arc<RwLock<InstanceObserver>>,
}

impl UsdImagingNiInstanceAggregationSceneIndex {
    /// Extracts the prototype name from an instancer path.
    ///
    /// Instancers have paths like
    /// `/UsdNiPropagatedPrototypes/NoBindings/__Prototype_1/UsdNiInstancer`.
    /// Returns the second-to-last element (e.g. `__Prototype_1`), or empty token
    /// if the path does not match the convention.
    ///
    /// Port of UsdImaging_NiInstanceAggregationSceneIndex::GetPrototypeNameFromInstancerPath.
    pub fn get_prototype_name_from_instancer_path(prim_path: &SdfPath) -> TfToken {
        if prim_path.get_path_element_count() < 4 {
            return TfToken::default();
        }
        // UsdImaging_NiPrototypeSceneIndexTokens->instancer
        if prim_path.get_name_token() != "UsdNiInstancer" {
            return TfToken::default();
        }
        prim_path.get_parent_path().get_name_token()
    }

    /// If the given path is for an instancer in this scene index, returns
    /// the binding scope path (parent of parent of instancer).
    ///
    /// E.g. `/X/Y/__Prototype_1/UsdNiInstancer` -> `/X/Y`.
    ///
    /// Port of UsdImaging_NiInstanceAggregationSceneIndex::GetBindingScopeFromInstancerPath.
    pub fn get_binding_scope_from_instancer_path(prim_path: &SdfPath) -> SdfPath {
        prim_path.get_parent_path().get_parent_path()
    }

    /// Creates a new instance aggregation scene index (legacy).
    ///
    /// Equivalent to `new_with_params(input_scene, false, vec![])`.
    pub fn new(
        input_scene: HdSceneIndexHandle,
        _input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        Self::new_with_params(input_scene, false, vec![])
    }

    /// Creates a new instance aggregation scene index with full C++ parity.
    ///
    /// Port of `UsdImaging_NiInstanceAggregationSceneIndex::New`.
    ///
    /// Creates InstanceObserver, populates from input, adds observer to input,
    /// and registers retained observer to forward retained events.
    ///
    /// # Arguments
    ///
    /// * `input_scene` - The input scene index
    /// * `for_native_prototype` - If true, for a USD prototype (nested instancing);
    ///   instancers need instancedBy populated for point instancer compatibility.
    /// * `instance_data_source_names` - Names of data sources that must match
    ///   for instances to aggregate (e.g. materialBindings, purpose, model).
    ///
    /// # Returns
    ///
    /// Thread-safe shared reference to the new scene index
    pub fn new_with_params(
        input_scene: HdSceneIndexHandle,
        for_native_prototype: bool,
        instance_data_source_names: Vec<TfToken>,
    ) -> Arc<RwLock<Self>> {
        let instance_observer = Arc::new(RwLock::new(InstanceObserver::new(
            input_scene.clone(),
            for_native_prototype,
            instance_data_source_names,
        )));

        instance_observer
            .write()
            .set_self_weak(Arc::downgrade(&instance_observer));
        instance_observer.write().populate();

        let retained = { instance_observer.read().get_retained_scene_index() };

        let scene_index = Arc::new(RwLock::new(Self {
            base: HdSceneIndexBaseImpl::new(),
            instance_observer: Arc::clone(&instance_observer),
        }));

        let retained_observer: HdSceneIndexObserverHandle = Arc::new(
            RetainedSceneIndexObserver::new(Arc::downgrade(&scene_index)),
        );
        retained.read().add_observer(retained_observer);

        let observer_handle: HdSceneIndexObserverHandle = Arc::new(
            InstanceObserverForwarder(Arc::clone(&instance_observer)),
        );
        input_scene.read().add_observer(observer_handle);

        scene_index
    }

    /// Creates input for instance aggregation, wrapping with DataSourceRelocatingSceneIndex
    /// when deferred skinning is enabled (for root/non-prototype case).
    ///
    /// Port of _ComputeInstanceAggregationSceneIndex logic from niPrototypePropagatingSceneIndex.cpp.
    /// When HD_ENABLE_DEFERRED_SKINNING is set, relocates skelBinding:animationSource to
    /// primvars:skel:animationSource so skel instances with different animationSource can aggregate.
    pub fn prepare_aggregation_input(
        prototype_scene_index: HdSceneIndexHandle,
        for_prototype: bool,
    ) -> HdSceneIndexHandle {
        if is_skinning_deferred() && !for_prototype {
            let src_locator = HdDataSourceLocator::from_tokens_2(
                TfToken::new("skelBinding"),
                TfToken::new("animationSource"),
            );
            let dst_locator = HdDataSourceLocator::from_tokens_2(
                (*HdPrimvarsSchema::get_schema_token()).clone(),
                TfToken::new("skel:animationSource"),
            );
            let relocating = UsdImagingDataSourceRelocatingSceneIndex::new(
                prototype_scene_index,
                src_locator,
                dst_locator,
                true, // forNativeInstance
            );
            usd_hd::scene_index::scene_index_to_handle(relocating)
        } else {
            prototype_scene_index
        }
    }

    /// Get input scene indices (C++ GetInputScenes).
    pub fn get_input_scenes(&self) -> Vec<HdSceneIndexHandle> {
        vec![
            self.instance_observer
                .read()
                .get_input_scene()
                .clone(),
        ]
    }

    /// Get encapsulated scene indices (C++ GetEncapsulatedScenes).
    pub fn get_encapsulated_scenes(&self) -> Vec<HdSceneIndexHandle> {
        let retained = self
            .instance_observer
            .read()
            .get_retained_scene_index();
        vec![scene_index_to_handle(Arc::clone(&retained))]
    }
}

impl HdSceneIndexBase for UsdImagingNiInstanceAggregationSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let retained = self
            .instance_observer
            .read()
            .get_retained_scene_index();
        let inner = unsafe { &*retained.data_ptr() };
        inner.get_prim(prim_path)
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let retained = self
            .instance_observer
            .read()
            .get_retained_scene_index();
        let inner = unsafe { &*retained.data_ptr() };
        inner.get_child_prim_paths(prim_path)
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "UsdImagingNiInstanceAggregationSceneIndex".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::scene_index::retained::HdRetainedSceneIndex;

    #[test]
    fn test_prototype_name_from_instancer_path() {
        let path = SdfPath::from_string(
            "/UsdNiPropagatedPrototypes/NoBindings/__Prototype_1/UsdNiInstancer",
        )
        .unwrap();
        let name =
            UsdImagingNiInstanceAggregationSceneIndex::get_prototype_name_from_instancer_path(
                &path,
            );
        assert_eq!(name.as_str(), "__Prototype_1");
    }

    #[test]
    fn test_prototype_name_too_short() {
        let path = SdfPath::from_string("/X/Y/Z").unwrap();
        let name =
            UsdImagingNiInstanceAggregationSceneIndex::get_prototype_name_from_instancer_path(
                &path,
            );
        assert!(name.is_empty());
    }

    #[test]
    fn test_binding_scope_from_instancer_path() {
        let path = SdfPath::from_string(
            "/UsdNiPropagatedPrototypes/Binding123/__Prototype_1/UsdNiInstancer",
        )
        .unwrap();
        let scope =
            UsdImagingNiInstanceAggregationSceneIndex::get_binding_scope_from_instancer_path(&path);
        assert_eq!(scope.to_string(), "/UsdNiPropagatedPrototypes/Binding123");
    }

    #[test]
    fn test_new_with_params_and_delegation() {
        let input = scene_index_to_handle(HdRetainedSceneIndex::new());
        let aggregation =
            UsdImagingNiInstanceAggregationSceneIndex::new_with_params(input, false, vec![]);

        let guard = aggregation.read();
        let inputs = guard.get_input_scenes();
        assert_eq!(inputs.len(), 1);

        let encapsulated = guard.get_encapsulated_scenes();
        assert_eq!(encapsulated.len(), 1);

        let prim = guard.get_prim(&SdfPath::absolute_root());
        assert!(prim.prim_type.is_empty());
    }

    #[test]
    fn test_aggregation_info_operations() {
        let mut info =
            InstanceAggregationInfo::new(SdfPath::from_string("/__Prototype_1").unwrap());

        assert_eq!(info.instance_count(), 0);

        let instance1 = SdfPath::from_string("/World/Instance1").unwrap();
        let instance2 = SdfPath::from_string("/World/Instance2").unwrap();

        info.add_instance(instance1.clone());
        info.add_instance(instance2.clone());
        assert_eq!(info.instance_count(), 2);

        assert!(info.remove_instance(&instance1));
        assert_eq!(info.instance_count(), 1);

        assert!(!info.remove_instance(&instance1));
        assert_eq!(info.instance_count(), 1);
    }
}
