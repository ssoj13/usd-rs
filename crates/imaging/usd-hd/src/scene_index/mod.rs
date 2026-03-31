
//! Scene Index infrastructure for Hydra 2.0.
//!
//! Scene indices are the core abstraction for representing scene data
//! in Hydra 2.0. They provide a query interface for accessing prims
//! and support observer notification for change tracking.
//!
//! # Architecture
//!
//! - **HdSceneIndexBase**: Base trait for all scene indices
//! - **HdSceneIndexObserver**: Observer pattern for tracking changes
//! - **HdSceneIndexPrim**: Prim representation with type and data source
//! - **Filtering Indices**: Chain scene indices for processing
//! - **Retained Index**: Mutable scene container
//!
//! # Example
//!
//! ```ignore
//! use usd_hd::scene_index::*;
//!
//! // Create a retained scene index
//! let scene = HdRetainedSceneIndex::new();
//!
//! // Add a prim
//! scene.add_prims(&[AddedPrimEntry {
//!     prim_path: SdfPath::from_string("/World").unwrap(),
//!     prim_type: TfToken::new("Mesh"),
//!     data_source: data_source_handle,
//! }]);
//!
//! // Query prims
//! let prim = scene.get_prim(&SdfPath::from_string("/World").unwrap());
//! ```

pub mod base;
pub mod caching;
pub mod dependency_forwarding;
pub mod encapsulating;
pub mod filtering;
pub mod flattening;
pub mod legacy_geom_subset;
pub mod legacy_prim;
pub mod material_filtering_scene_index_base;
pub mod merging;
pub mod name_registry;
pub mod notice_batching;
pub mod observer;
pub mod plugin;
pub mod plugin_registry;
pub mod prefixing;
pub mod prim;
pub mod prim_view;
pub mod retained;

// Re-export core types
pub use base::SceneRwLock;
pub use base::si_ref;
pub use base::{
    HdContainerDataSourceHandle, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexWeakHandle,
    SdfPathVector, arc_scene_index_to_handle, scene_index_to_handle,
};
pub use caching::HdCachingSceneIndex;
pub use dependency_forwarding::HdDependencyForwardingSceneIndex;
pub use encapsulating::{
    HdEncapsulatingSceneIndex, hd_make_encapsulating_scene_index,
    hd_use_encapsulating_scene_indices,
};
pub use filtering::{
    FilteringObserverTarget, HdEncapsulatingSceneIndexBaseTrait, HdFilteringSceneIndexBase,
    HdSingleInputFilteringSceneIndexBase, wire_filter_to_input,
};
pub use flattening::HdFlatteningSceneIndex;
pub use legacy_geom_subset::HdLegacyGeomSubsetSceneIndex;
pub use legacy_prim::HdLegacyPrimSceneIndex;
pub use material_filtering_scene_index_base::{
    HdMaterialFilteringSceneIndexBase, MaterialFilteringFn,
};
pub use merging::HdMergingSceneIndex;
pub use name_registry::HdSceneIndexNameRegistry;
pub use notice_batching::HdNoticeBatchingSceneIndex;
pub use observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserver, HdSceneIndexObserverHandle,
    RemovedPrimEntry, RenamedPrimEntry,
};
pub use plugin::{HdSceneIndexPlugin, HdSceneIndexPluginHandle};
pub use plugin_registry::HdSceneIndexPluginRegistry;
pub use prefixing::HdPrefixingSceneIndex;
pub use prim::HdSceneIndexPrim;
pub use prim_view::HdSceneIndexPrimView;
pub use retained::{HdRetainedSceneIndex, RetainedAddedPrimEntry};

/// Minimal no-op scene index used as dummy sender in lock-free observer dispatch.
pub struct NullSceneIndex;
impl base::HdSceneIndexBase for NullSceneIndex {
    fn get_prim(&self, _: &usd_sdf::Path) -> prim::HdSceneIndexPrim {
        prim::HdSceneIndexPrim::empty()
    }
    fn get_child_prim_paths(&self, _: &usd_sdf::Path) -> Vec<usd_sdf::Path> {
        Vec::new()
    }
    fn add_observer(&self, _: HdSceneIndexObserverHandle) {}
    fn remove_observer(&self, _: &HdSceneIndexObserverHandle) {}
}
