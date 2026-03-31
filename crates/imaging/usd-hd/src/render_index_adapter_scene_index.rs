
//! HdRenderIndexAdapterSceneIndex - Scene index backed by render index.
//!
//! Corresponds to pxr/imaging/hd/renderIndexAdapterSceneIndex.h.

use super::render_delegate_info::HdRenderDelegateInfo;
use super::scene_index::base::HdSceneIndexBaseImpl;
use super::scene_index::observer::HdSceneIndexObserverHandle;
use super::scene_index::{HdSceneIndexBase, HdSceneIndexPrim};
use crate::data_source::HdContainerDataSourceHandle;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;

/// Scene index that adapts HdRenderIndex (legacy) for scene index API.
///
/// Populated from HdSceneDelegate through the owned render index.
/// Corresponds to C++ `HdRenderIndexAdapterSceneIndex`.
pub struct HdRenderIndexAdapterSceneIndex {
    base: HdSceneIndexBaseImpl,
}

impl HdRenderIndexAdapterSceneIndex {
    /// Create from input args (container data source).
    pub fn new_from_args(_input_args: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self {
            base: HdSceneIndexBaseImpl::new(),
        })
    }

    /// Create from render delegate info.
    pub fn new_from_info(_info: &HdRenderDelegateInfo) -> Arc<Self> {
        Arc::new(Self {
            base: HdSceneIndexBaseImpl::new(),
        })
    }

    /// Get render index (opaque - for integration). Placeholder for now.
    pub fn get_render_index(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

impl HdSceneIndexBase for HdRenderIndexAdapterSceneIndex {
    fn get_prim(&self, _prim_path: &SdfPath) -> HdSceneIndexPrim {
        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, _prim_path: &SdfPath) -> super::scene_index::SdfPathVector {
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.remove_observer(observer);
    }
}
