//! Base task class for Hydra extensions.
//!
//! `HdxTask` provides a non-virtual interface (NVI) Sync pattern:
//! - `sync()` is final: initializes HGI from task context, then calls `_sync()`
//! - Derived tasks override `_sync()` instead of `sync()`
//!
//! Also provides:
//! - `toggle_render_target` / `toggle_depth_target` for ping-pong texture swap
//! - `are_tasks_converged` for progressive rendering queries

use parking_lot::RwLock;
use std::sync::Arc;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext};
use usd_hgi::Hgi;
use usd_sdf::Path;
use usd_tf::Token;

/// Base trait for HdxTask — adds HGI access and NVI Sync pattern.
///
/// C++ `HdxTask::Sync()` is `final` and initialises `_hgi` from task context,
/// then calls the virtual `_Sync()`. The trait mirrors this NVI convention.
pub trait HdxTask: HdTask {
    /// Internal sync implementation.
    ///
    /// Override this in derived tasks instead of `sync()`.
    /// At call time `get_hgi()` is already initialised (if HGI is present).
    fn _sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    );

    /// NVI sync — initialises HGI then delegates to `_sync()`.
    ///
    /// This mirrors C++ `HdxTask::Sync()` which is declared `final`.
    fn sync_hdx(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // Lazily initialise HGI from task context ("renderDriver" key).
        // C++: if (!_hgi) { _hgi = _GetDriver<Hgi*>(ctx, HgiTokens->renderDriver); }
        self.init_hgi_from_context(ctx);
        self._sync(delegate, ctx, dirty_bits);
    }

    /// Lazily set HGI from task context if not yet initialised.
    fn init_hgi_from_context(&mut self, ctx: &HdTaskContext);

    /// Get pointer to Hydra Graphics Interface (may be None).
    fn get_hgi(&self) -> Option<Arc<RwLock<dyn Hgi>>>;

    /// Swap color target with colorIntermediate.
    ///
    /// Used when a task reads from and writes to the color buffer.
    /// Matches C++ `_ToggleRenderTarget`.
    fn toggle_render_target(&mut self, ctx: &mut HdTaskContext) {
        let color = Token::new("color");
        let intermediate = Token::new("colorIntermediate");
        if ctx.contains_key(&color) && ctx.contains_key(&intermediate) {
            self.swap_textures(ctx, &color, &intermediate);
        }
    }

    /// Swap depth target with depthIntermediate.
    ///
    /// Matches C++ `_ToggleDepthTarget`.
    fn toggle_depth_target(&mut self, ctx: &mut HdTaskContext) {
        let depth = Token::new("depth");
        let intermediate = Token::new("depthIntermediate");
        if ctx.contains_key(&depth) && ctx.contains_key(&intermediate) {
            self.swap_textures(ctx, &depth, &intermediate);
        }
    }

    /// Swap two texture entries in the task context.
    fn swap_textures(
        &mut self,
        ctx: &mut HdTaskContext,
        texture_token: &Token,
        texture_intermediate_token: &Token,
    );
}

/// Base implementation struct for `HdxTask`.
///
/// Stores the task id and an optional HGI handle (initialised lazily during Sync).
pub struct HdxTaskBase {
    /// Task identifier.
    id: Path,

    /// Hydra Graphics Interface handle (initialised on first `sync_hdx` call).
    hgi: Option<Arc<RwLock<dyn Hgi + Send>>>,
}

impl HdxTaskBase {
    /// Create a new HDX task base with the given path.
    pub fn new(id: Path) -> Self {
        Self { id, hgi: None }
    }

    /// Get task path.
    pub fn id(&self) -> &Path {
        &self.id
    }

    /// Set HGI instance.
    pub fn set_hgi(&mut self, hgi: Arc<RwLock<dyn Hgi + Send>>) {
        self.hgi = Some(hgi);
    }

    /// Get HGI instance.
    pub fn hgi(&self) -> Option<Arc<RwLock<dyn Hgi + Send>>> {
        self.hgi.clone()
    }

    /// Check if all tasks at given paths are converged.
    ///
    /// Used for progressive rendering — returns `true` when every queried task
    /// has finished rendering its current frame.
    pub fn are_tasks_converged(render_index: &dyn HdRenderIndexTrait, task_paths: &[Path]) -> bool {
        for task_path in task_paths {
            if let Some(task) = render_index.get_task(task_path) {
                let guard = task.read();
                if !guard.is_converged() {
                    return false;
                }
            }
        }
        true
    }

    /// Swap two values in the task context.
    ///
    /// Matches C++ `_SwapTextures` — does a `std::swap` on the two map entries.
    pub fn swap_context_data(ctx: &mut HdTaskContext, token_a: &Token, token_b: &Token) {
        let value_a = ctx.get(token_a).cloned();
        let value_b = ctx.get(token_b).cloned();

        if let Some(val_b) = value_b {
            ctx.insert(token_a.clone(), val_b);
        } else {
            ctx.remove(token_a);
        }

        if let Some(val_a) = value_a {
            ctx.insert(token_b.clone(), val_a);
        } else {
            ctx.remove(token_b);
        }
    }
}

impl HdTask for HdxTaskBase {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        ctx: &mut HdTaskContext,
        _dirty_bits: &mut u32,
    ) {
        // NVI: initialise HGI from task context ("renderDriver" key).
        // C++: if (!_hgi) { _hgi = _GetDriver<Hgi*>(ctx, HgiTokens->renderDriver); }
        // In Rust the driver entry stores an HgiDriverHandle under the
        // standard HdTokens->drivers vector.
        if self.hgi.is_none() {
            if let Some(v) =
                usd_hd::render::task::HdTaskBase::get_driver(ctx, &Token::new("renderDriver"))
            {
                if let Some(handle) = v.get::<usd_hgi::HgiDriverHandle>() {
                    self.hgi = Some(handle.get().clone());
                }
            }
        }
        // Base implementation does nothing further — derived tasks use HdxTask trait.
    }

    fn prepare(&mut self, _ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {}

    fn execute(&mut self, _ctx: &mut HdTaskContext) {}

    fn get_render_tags(&self) -> &[Token] {
        &[]
    }

    fn is_converged(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdx_task_base_creation() {
        let path = Path::from_string("/test/task").unwrap();
        let task = HdxTaskBase::new(path.clone());
        assert_eq!(task.id(), &path);
        assert!(task.hgi().is_none());
    }

    #[test]
    fn test_hdx_task_default_convergence() {
        let task = HdxTaskBase::new(Path::from_string("/test").unwrap());
        assert!(task.is_converged());
    }

    #[test]
    fn test_swap_context_data() {
        use usd_vt::Value;

        let mut ctx = HdTaskContext::new();
        let token_a = Token::new("tokenA");
        let token_b = Token::new("tokenB");

        ctx.insert(token_a.clone(), Value::from(100));
        ctx.insert(token_b.clone(), Value::from(200));

        HdxTaskBase::swap_context_data(&mut ctx, &token_a, &token_b);

        assert_eq!(ctx.get(&token_a).and_then(|v| v.get::<i32>()), Some(&200));
        assert_eq!(ctx.get(&token_b).and_then(|v| v.get::<i32>()), Some(&100));
    }

    #[test]
    fn test_swap_context_data_missing() {
        use usd_vt::Value;

        let mut ctx = HdTaskContext::new();
        let token_a = Token::new("tokenA");
        let token_b = Token::new("tokenB");

        ctx.insert(token_a.clone(), Value::from(42));

        HdxTaskBase::swap_context_data(&mut ctx, &token_a, &token_b);

        assert_eq!(ctx.get(&token_b).and_then(|v| v.get::<i32>()), Some(&42));
        assert!(ctx.get(&token_a).is_none());
    }

    #[test]
    fn test_hgi_init_from_context_nvi() {
        // Verify that base sync() reads "renderDriver" from context.
        let mut task = HdxTaskBase::new(Path::from_string("/test").unwrap());
        assert!(task.hgi().is_none());

        // If no renderDriver key in context, hgi stays None.
        let mut ctx = HdTaskContext::new();
        let mut dirty = 0u32;
        let delegate = MockDelegate;
        let _ri = MockRenderIndex;
        task.sync(&delegate, &mut ctx, &mut dirty);
        assert!(task.hgi().is_none(), "no HGI key -> hgi stays None");
    }

    /// Minimal mock delegate for testing.
    struct MockDelegate;
    impl HdSceneDelegate for MockDelegate {
        fn get_dirty_bits(&self, _id: &Path) -> usd_hd::types::HdDirtyBits {
            0
        }
        fn mark_clean(&mut self, _id: &Path, _bits: usd_hd::types::HdDirtyBits) {}
        fn get_instancer_id(&self, _prim_id: &Path) -> Path {
            Path::default()
        }
        fn get_delegate_id(&self) -> Path {
            Path::default()
        }
        fn get_transform(&self, _id: &Path) -> usd_gf::Matrix4d {
            usd_gf::Matrix4d::identity()
        }
        fn get(&self, _id: &Path, _key: &Token) -> usd_vt::Value {
            usd_vt::Value::default()
        }
        fn get_visible(&self, _id: &Path) -> bool {
            true
        }
    }

    /// Minimal mock render index for testing.
    struct MockRenderIndex;
    impl HdRenderIndexTrait for MockRenderIndex {
        fn get_task(&self, _path: &Path) -> Option<&usd_hd::render::HdTaskSharedPtr> {
            None
        }
        fn has_task(&self, _path: &Path) -> bool {
            false
        }
        fn get_rprim(&self, _id: &Path) -> Option<&usd_hd::render::HdPrimHandle> {
            None
        }
        fn get_sprim(
            &self,
            _type_id: &usd_tf::Token,
            _id: &Path,
        ) -> Option<&usd_hd::render::HdPrimHandle> {
            None
        }
        fn get_bprim(
            &self,
            _type_id: &usd_tf::Token,
            _id: &Path,
        ) -> Option<&usd_hd::render::HdPrimHandle> {
            None
        }
        fn get_rprim_ids(&self) -> Vec<Path> {
            Vec::new()
        }
        fn get_prim_id_for_rprim_path(&self, _rprim_path: &Path) -> Option<i32> {
            None
        }
        fn get_render_delegate(&self) -> &usd_hd::render::render_index::HdRenderDelegateSharedPtr {
            unimplemented!("not needed in task test") // intentional: test mock stub
        }
        fn get_change_tracker(&self) -> &usd_hd::change_tracker::HdChangeTracker {
            unimplemented!("not needed in task test") // intentional: test mock stub
        }
    }
}
