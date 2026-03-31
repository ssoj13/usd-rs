//! Render index backend for IndexProxy.
//!
//! Bridges UsdImagingDelegate population to HdRenderIndex.
//! Port of the UsdImagingIndexProxy -> HdRenderIndex flow.

use crate::index_proxy::IndexProxyBackend;
use std::sync::{Arc, Mutex};
use usd_hd::render::HdRenderIndex;
use usd_sdf::Path;
use usd_tf::Token;

/// Backend that inserts prims into HdRenderIndex.
///
/// Wraps the render index in Mutex for shared mutable access during
/// delegate population.
pub struct RenderIndexIndexProxyBackend {
    /// Render index (Mutex for insert during populate)
    index: Arc<Mutex<HdRenderIndex>>,
}

impl RenderIndexIndexProxyBackend {
    /// Create backend for the given render index.
    pub fn new(index: Arc<Mutex<HdRenderIndex>>) -> Arc<Self> {
        Arc::new(Self { index })
    }
}

impl IndexProxyBackend for RenderIndexIndexProxyBackend {
    fn insert_rprim(&self, prim_type: &Token, scene_delegate_id: &Path, prim_id: &Path) -> bool {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.insert_rprim(prim_type, scene_delegate_id, prim_id)
    }

    fn insert_sprim(&self, prim_type: &Token, scene_delegate_id: &Path, prim_id: &Path) -> bool {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.insert_sprim(prim_type, scene_delegate_id, prim_id)
    }

    fn insert_bprim(&self, prim_type: &Token, scene_delegate_id: &Path, prim_id: &Path) -> bool {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.insert_bprim(prim_type, scene_delegate_id, prim_id)
    }

    fn is_rprim_type_supported(&self, type_id: &Token) -> bool {
        let index = self.index.lock().expect("Mutex poisoned");
        let delegate = index.get_render_delegate();
        let delegate = delegate.read();
        delegate
            .get_supported_rprim_types()
            .iter()
            .any(|t| t == type_id)
    }

    fn is_sprim_type_supported(&self, type_id: &Token) -> bool {
        let index = self.index.lock().expect("Mutex poisoned");
        let delegate = index.get_render_delegate();
        let delegate = delegate.read();
        delegate
            .get_supported_sprim_types()
            .iter()
            .any(|t| t == type_id)
    }

    fn is_bprim_type_supported(&self, type_id: &Token) -> bool {
        let index = self.index.lock().expect("Mutex poisoned");
        let delegate = index.get_render_delegate();
        let delegate = delegate.read();
        delegate
            .get_supported_bprim_types()
            .iter()
            .any(|t| t == type_id)
    }

    fn mark_rprim_dirty(&self, prim_id: &Path, dirty_bits: u32) {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.mark_rprim_dirty(prim_id, dirty_bits);
    }

    fn mark_sprim_dirty(&self, prim_id: &Path, dirty_bits: u32) {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.mark_sprim_dirty(prim_id, dirty_bits);
    }

    fn mark_bprim_dirty(&self, prim_id: &Path, dirty_bits: u32) {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.mark_bprim_dirty(prim_id, dirty_bits);
    }

    fn mark_instancer_dirty(&self, prim_id: &Path, dirty_bits: u32) {
        let mut index = self.index.lock().expect("Mutex poisoned");
        index.mark_instancer_dirty(prim_id, dirty_bits);
    }
}
