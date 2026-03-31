
//! HdRenderDelegate - Main extension point for rendering backends.
//!
//! The render delegate is the primary interface that rendering backends implement
//! to integrate with Hydra. It handles:
//! - Creating and destroying scene prims (rprim, sprim, bprim)
//! - Reporting supported prim types
//! - Creating render passes and resource registries
//! - Managing render settings
//! - Providing render capabilities
//!
//! # Architecture
//!
//! The render delegate acts as a factory for all rendering objects.
//! Each backend (Storm, Embree, Arnold, etc.) provides its own implementation.

use super::driver::HdDriverVector;
use crate::aov::HdAovDescriptor;
use crate::change_tracker::HdChangeTracker;
use crate::command::{HdCommandArgs, HdCommandDescriptors};
use crate::data_source::HdContainerDataSourceHandle;
use crate::scene_index::HdSceneIndexHandle;
use crate::types::HdDirtyBits;
use std::collections::HashMap;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

// Forward declarations
use super::render_index::{
    HdBprimHandle, HdPrimHandle, HdRenderIndex, HdRprimHandle, HdSprimHandle,
};
use crate::prim::HdSceneDelegate;

/// Shared pointer to render param.
pub type HdRenderParamSharedPtr = Arc<dyn HdRenderParam>;

/// Shared pointer to render pass.
pub type HdRenderPassSharedPtr = Arc<dyn HdRenderPass>;

/// Shared pointer to resource registry.
pub type HdResourceRegistrySharedPtr = Arc<dyn HdResourceRegistry>;

/// Token vector for supported prim types.
pub type TfTokenVector = Vec<Token>;

/// Opaque render parameter passed to prims during sync.
///
/// Backends can use this to pass per-delegate state to prims without
/// requiring global state. The render param is obtained from the render
/// delegate and passed to each prim during Sync processing.
pub trait HdRenderParam: Send + Sync {
    /// Set an arbitrary custom value.
    ///
    /// Must be thread-safe. Returns true if successful.
    /// C++ name: SetArbitraryValue.
    fn set_arbitrary_value(&mut self, _key: &Token, _value: &Value) -> bool {
        false
    }

    /// Get an arbitrary custom value.
    ///
    /// Must be thread-safe. Returns empty Value if not found.
    /// C++ name: GetArbitraryValue.
    fn get_arbitrary_value(&self, _key: &Token) -> Option<Value> {
        None
    }

    /// Check if a custom value exists.
    ///
    /// Must be thread-safe.
    /// C++ name: HasArbitraryValue.
    fn has_arbitrary_value(&self, _key: &Token) -> bool {
        false
    }

    /// Check if render param is valid. C++ default: false.
    fn is_valid(&self) -> bool {
        false
    }
}

/// Descriptor for a render setting exposed to UI/application.
#[derive(Debug, Clone)]
pub struct HdRenderSettingDescriptor {
    /// Human-readable name
    pub name: String,

    /// Key for get/set operations
    pub key: Token,

    /// Default value
    pub default_value: Value,
}

impl HdRenderSettingDescriptor {
    /// Create a new render setting descriptor.
    pub fn new(name: impl Into<String>, key: Token, default_value: Value) -> Self {
        Self {
            name: name.into(),
            key,
            default_value,
        }
    }
}

/// List of render setting descriptors.
pub type HdRenderSettingDescriptorList = Vec<HdRenderSettingDescriptor>;

/// Map of render settings (key -> value).
pub type HdRenderSettingsMap = HashMap<Token, Value>;

// Import the full HdRenderPass trait from render_pass module
pub use super::render_pass::{HdRenderPass, HdRenderPassStateSharedPtr};

/// Placeholder trait for resource registry.
///
/// Full definition in resource_registry.rs (future)
pub trait HdResourceRegistry: Send + Sync {
    // Placeholder - will be implemented in future
}

/// Placeholder trait for instancer.
///
/// Full definition in prim/instancer.rs
pub trait HdInstancer: Send + Sync {
    // Placeholder
}

// Re-export HdRprimCollection from rprim_collection
pub use super::rprim_collection::HdRprimCollection;

/// Main extension point for rendering backends.
///
/// Render delegates must implement this trait to integrate with Hydra.
///
/// # Example
/// ```ignore
/// use usd_hd::render::*;
///
/// struct MyRenderDelegate {
///     resource_registry: HdResourceRegistrySharedPtr,
///     settings: HdRenderSettingsMap,
/// }
///
/// impl HdRenderDelegate for MyRenderDelegate {
///     fn get_supported_rprim_types(&self) -> &TfTokenVector {
///         static TYPES: &[Token] = &[
///             Token::new("mesh"),
///             Token::new("basisCurves"),
///         ];
///         TYPES
///     }
///     
///     fn create_rprim(&mut self, type_id: &Token, id: SdfPath)
///         -> Option<Box<dyn HdRprim>> {
///         match type_id.as_str() {
///             "mesh" => Some(Box::new(MyMesh::new(id))),
///             _ => None,
///         }
///     }
///     
///     // ... implement other required methods
/// }
/// ```
pub trait HdRenderDelegate: Send + Sync {
    //--------------------------------------------------------------------------
    // Prim Type Support
    //--------------------------------------------------------------------------

    /// Get list of supported Rprim (renderable) types.
    fn get_supported_rprim_types(&self) -> &TfTokenVector;

    /// Get list of supported Sprim (state) types.
    fn get_supported_sprim_types(&self) -> &TfTokenVector;

    /// Get list of supported Bprim (buffer) types.
    fn get_supported_bprim_types(&self) -> &TfTokenVector;

    //--------------------------------------------------------------------------
    // Prim Creation/Destruction
    //--------------------------------------------------------------------------

    /// Create a renderable prim (mesh, curves, etc.).
    fn create_rprim(&mut self, type_id: &Token, id: SdfPath) -> Option<HdPrimHandle>;

    /// Create a sync-capable rprim handle alongside the opaque handle.
    ///
    /// Backends that store prims via typed Rust structs return Some(Box<dyn HdRprimSync>)
    /// here. render_index will call sync_dyn() on it directly, bypassing the
    /// type-erased dispatch through sync_rprim(). Default: None (falls back to sync_rprim).
    fn create_rprim_sync(&mut self, _type_id: &Token, _id: &SdfPath) -> Option<HdRprimHandle> {
        None
    }

    /// Pre-sync opaque rprim handles before `sceneDelegate.sync(...)`.
    ///
    /// `_ref` runs a dedicated pre-sync phase that lets each rprim initialize
    /// reprs, request additional dirty dependencies, and skip fully ignorable
    /// work before the scene delegate sees the aggregate request vector.
    ///
    /// Backends that expose typed `HdRprimSync` handles do this in
    /// `render_index.rs` directly. Backends that only keep opaque `HdPrimHandle`
    /// objects can override this hook to restore the same contract.
    fn pre_sync_rprims_batch(
        &self,
        _handles: &mut [(&SdfPath, &mut HdPrimHandle, &mut HdDirtyBits)],
        _delegate: &dyn crate::prim::HdSceneDelegate,
        _repr_token: &Token,
    ) {
    }

    /// Destroy a renderable prim.
    fn destroy_rprim(&mut self, rprim: HdPrimHandle) {
        drop(rprim);
    }

    /// Create a state prim (camera, light, material).
    fn create_sprim(&mut self, type_id: &Token, id: SdfPath) -> Option<HdPrimHandle>;

    /// Create a sync-capable sprim handle alongside the opaque handle. Default: None.
    fn create_sprim_sync(&mut self, _type_id: &Token, _id: &SdfPath) -> Option<HdSprimHandle> {
        None
    }

    /// Create a fallback sprim for when a referenced sprim doesn't exist.
    ///
    /// Pure virtual in C++ - all backends must implement.
    fn create_fallback_sprim(&mut self, type_id: &Token) -> Option<HdPrimHandle>;

    /// Destroy a state prim.
    fn destroy_sprim(&mut self, sprim: HdPrimHandle) {
        drop(sprim);
    }

    /// Create a buffer prim (render buffer, texture).
    fn create_bprim(&mut self, type_id: &Token, id: SdfPath) -> Option<HdPrimHandle>;

    /// Create a sync-capable bprim handle alongside the opaque handle. Default: None.
    fn create_bprim_sync(&mut self, _type_id: &Token, _id: &SdfPath) -> Option<HdBprimHandle> {
        None
    }

    /// Create a fallback bprim for when a referenced bprim doesn't exist.
    ///
    /// Pure virtual in C++ - all backends must implement.
    fn create_fallback_bprim(&mut self, type_id: &Token) -> Option<HdPrimHandle>;

    /// Destroy a buffer prim.
    fn destroy_bprim(&mut self, bprim: HdPrimHandle) {
        drop(bprim);
    }

    /// Create an instancer.
    ///
    /// Pure virtual in C++ - all backends must implement.
    /// C++ signature: `CreateInstancer(HdSceneDelegate *delegate, SdfPath const& id)`
    fn create_instancer(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        id: SdfPath,
    ) -> Option<Box<dyn HdInstancer>>;

    /// Destroy an instancer.
    ///
    /// Pure virtual in C++ - all backends must implement.
    fn destroy_instancer(&mut self, instancer: Box<dyn HdInstancer>);

    //--------------------------------------------------------------------------
    // Render Resources
    //--------------------------------------------------------------------------

    /// Create a render pass for the given collection.
    ///
    /// C++ signature: `CreateRenderPass(HdRenderIndex *index, HdRprimCollection const& collection)`
    fn create_render_pass(
        &mut self,
        index: &HdRenderIndex,
        collection: &HdRprimCollection,
    ) -> Option<HdRenderPassSharedPtr>;

    /// Create render pass state (camera, viewport, AOV bindings).
    /// Default: creates HdRenderPassStateBase.
    fn create_render_pass_state(&self) -> HdRenderPassStateSharedPtr {
        Arc::new(crate::render_pass_state::HdRenderPassStateBase::default())
    }

    /// Get the resource registry for managing GPU resources.
    fn get_resource_registry(&self) -> HdResourceRegistrySharedPtr;

    /// Sync an rprim handle with the scene delegate.
    ///
    /// The render delegate dispatches to the concrete rprim's Sync() method.
    /// Since prim handles are type-erased (Box<dyn Any>), the delegate must
    /// downcast to the concrete type internally.
    ///
    /// Returns the cleaned dirty bits (bits that remain set after sync).
    fn sync_rprim(
        &self,
        _handle: &mut HdPrimHandle,
        _prim_id: &SdfPath,
        _delegate: &dyn HdSceneDelegate,
        _dirty_bits: &mut crate::types::HdDirtyBits,
        _repr_token: &Token,
    ) {
        // Default: no-op. Backends override to call concrete rprim.sync().
    }

    /// Batch-parallel rprim sync. Backends with parallel processing override this.
    /// Default: falls back to sequential sync_rprim per handle.
    fn sync_rprims_batch(
        &self,
        handles: &mut [(&SdfPath, &mut HdPrimHandle, &mut crate::types::HdDirtyBits)],
        delegate: &dyn HdSceneDelegate,
        repr_token: &Token,
    ) {
        for (prim_id, handle, dirty_bits) in handles.iter_mut() {
            self.sync_rprim(handle, prim_id, delegate, dirty_bits, repr_token);
        }
    }

    /// Sync a sprim handle with the scene delegate.
    ///
    /// The render delegate dispatches to the concrete sprim's Sync() method.
    fn sync_sprim(
        &self,
        _handle: &mut HdPrimHandle,
        _prim_id: &SdfPath,
        _delegate: &dyn HdSceneDelegate,
        _dirty_bits: &mut crate::types::HdDirtyBits,
    ) {
        // Default: no-op. Backends override to call concrete sprim.sync().
    }

    /// Sync a bprim handle with the scene delegate.
    ///
    /// The render delegate dispatches to the concrete bprim's Sync() method.
    fn sync_bprim(
        &self,
        _handle: &mut HdPrimHandle,
        _prim_id: &SdfPath,
        _delegate: &dyn HdSceneDelegate,
        _dirty_bits: &mut crate::types::HdDirtyBits,
    ) {
        // Default: no-op. Backends override to call concrete bprim.sync().
    }

    /// Get draw items for an rprim handle.
    ///
    /// Storm and other backends override this to return backend-specific draw
    /// items (e.g. HdStDrawItem). The default returns an empty list.
    /// Callers downcast the returned `Arc<dyn Any>` to the concrete type.
    ///
    /// `sync_handle` is `Some` when the entry has a typed rprim sync object
    /// (created via `create_rprim_sync`). Backends that route sync through
    /// `HdRprimSync` should prefer reading draw items from it, since `handle`
    /// is the opaque version and may not have been synced.
    fn get_draw_items_for_rprim(
        &self,
        _handle: &HdPrimHandle,
        _sync_handle: Option<&dyn std::any::Any>,
        _prim_id: &SdfPath,
        _collection: &HdRprimCollection,
        _render_tags: &[Token],
    ) -> Vec<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
        Vec::new()
    }

    /// Get the render param (opaque state passed to prims).
    fn get_render_param(&self) -> Option<HdRenderParamSharedPtr> {
        None
    }

    //--------------------------------------------------------------------------
    // Configuration
    //--------------------------------------------------------------------------

    /// Set list of driver objects (GPU devices, contexts).
    fn set_drivers(&mut self, _drivers: &HdDriverVector) {
        // Default: no-op
    }

    /// Set a render setting value.
    fn set_render_setting(&mut self, _key: &Token, _value: &Value) {
        // Default: no-op
    }

    /// Get a render setting value.
    fn get_render_setting(&self, _key: &Token) -> Option<Value> {
        None
    }

    /// Get render setting descriptors for UI.
    fn get_render_setting_descriptors(&self) -> HdRenderSettingDescriptorList {
        Vec::new()
    }

    /// Get current render settings version.
    /// C++ default: 1 (initialized in ctor, incremented on each setting change).
    fn get_render_settings_version(&self) -> u32 {
        1
    }

    /// Get render statistics as dictionary.
    fn get_render_stats(&self) -> HashMap<String, Value> {
        HashMap::new()
    }

    /// Get render capabilities as data source.
    fn get_capabilities(&self) -> Option<HdContainerDataSourceHandle> {
        None
    }

    //--------------------------------------------------------------------------
    // Background Rendering Control
    //--------------------------------------------------------------------------

    /// Check if pause is supported.
    fn is_pause_supported(&self) -> bool {
        false
    }

    /// Check if currently paused.
    fn is_paused(&self) -> bool {
        false
    }

    /// Pause background rendering threads.
    fn pause(&mut self) -> bool {
        false
    }

    /// Resume background rendering threads.
    fn resume(&mut self) -> bool {
        false
    }

    /// Check if stop is supported.
    fn is_stop_supported(&self) -> bool {
        false
    }

    /// Check if currently stopped.
    /// C++ default: true (renderDelegate.cpp:276).
    fn is_stopped(&self) -> bool {
        true
    }

    /// Stop background rendering threads.
    ///
    /// If `blocking` is true, waits until threads exit.
    /// C++ signature: `Stop(bool blocking = true)`
    fn stop(&mut self, blocking: bool) -> bool {
        let _ = blocking;
        false
    }

    /// Restart background rendering threads.
    fn restart(&mut self) -> bool {
        false
    }

    //--------------------------------------------------------------------------
    // Materials (C++ GetMaterialBindingPurpose, GetMaterialNetworkSelector, etc.)
    //--------------------------------------------------------------------------

    /// Material binding purpose (e.g. "preview"). Default: "preview".
    fn get_material_binding_purpose(&self) -> Token {
        Token::new("preview")
    }

    /// Deprecated material network selector. Default: empty.
    fn get_material_network_selector(&self) -> Token {
        Token::new("")
    }

    /// Material render contexts in descending preference order. Default: empty.
    /// C++ default: returns `{GetMaterialNetworkSelector()}` (renderDelegate.cpp:117).
    fn get_material_render_contexts(&self) -> TfTokenVector {
        vec![self.get_material_network_selector()]
    }

    /// Namespace prefixes for render settings. Default: empty (all custom attrs).
    fn get_render_settings_namespaces(&self) -> TfTokenVector {
        Vec::new()
    }

    /// Whether primvar filtering is needed. Default: false.
    fn is_primvar_filtering_needed(&self) -> bool {
        false
    }

    /// Shader source types supported (e.g. "glslfx", "mtlx"). Default: empty.
    fn get_shader_source_types(&self) -> TfTokenVector {
        Vec::new()
    }

    /// Whether parallel sync is enabled for the given prim type. Default: true.
    fn is_parallel_sync_enabled(&self, _prim_type: &Token) -> bool {
        true
    }

    /// Whether the application task graph should include Storm-specific tasks.
    /// Default: false.
    fn requires_storm_tasks(&self) -> bool {
        false
    }

    //--------------------------------------------------------------------------
    // AOVs
    //--------------------------------------------------------------------------

    /// Returns default AOV descriptor for the given named AOV.
    fn get_default_aov_descriptor(&self, _name: &Token) -> HdAovDescriptor {
        HdAovDescriptor::default()
    }

    //--------------------------------------------------------------------------
    // Commands API
    //--------------------------------------------------------------------------

    /// Get descriptors for commands supported by this render delegate.
    fn get_command_descriptors(&self) -> HdCommandDescriptors {
        Vec::new()
    }

    /// Invoke a command with optional arguments. Returns true if successful.
    fn invoke_command(&mut self, _command: &Token, _args: &HdCommandArgs) -> bool {
        false
    }

    /// Renderer display name (e.g. from plugin registry).
    fn get_renderer_display_name(&self) -> String {
        String::new()
    }

    //--------------------------------------------------------------------------
    // Hydra 2.0 API
    //--------------------------------------------------------------------------

    /// Called after scene index graph is created. Hook to register observer.
    fn set_terminal_scene_index(&mut self, _scene_index: HdSceneIndexHandle) {}

    /// Called at start of SyncAll. Process change notices from terminal scene index.
    fn update(&mut self) {}

    //--------------------------------------------------------------------------
    // Lifecycle
    //--------------------------------------------------------------------------

    /// Commit resources after scene changes.
    ///
    /// Called after Sync phase completes. Pure virtual in C++.
    /// C++ signature: `CommitResources(HdChangeTracker *tracker)`
    fn commit_resources(&mut self, tracker: &mut HdChangeTracker);
}

/// Base implementation for render delegates providing built-in settings storage.
///
/// Matches C++ `HdRenderDelegate` protected members:
/// - `_settingsMap` - render settings key-value storage
/// - `_settingsVersion` - incremented on each setting change
/// - `_PopulateDefaultSettings` - populate from descriptor defaults
/// - `_displayName` - renderer display name (set by HdRendererPlugin)
///
/// Concrete delegates can embed this and delegate settings calls.
pub struct HdRenderDelegateBase {
    /// Render settings storage (matches C++ `_settingsMap`).
    pub settings_map: HdRenderSettingsMap,

    /// Settings version, incremented on each change (matches C++ `_settingsVersion`).
    pub settings_version: u32,

    /// Renderer display name, set by HdRendererPlugin (matches C++ `_displayName`).
    display_name: String,
}

impl Default for HdRenderDelegateBase {
    fn default() -> Self {
        Self {
            settings_map: HashMap::new(),
            settings_version: 1,
            display_name: String::new(),
        }
    }
}

impl HdRenderDelegateBase {
    /// Create with initial settings (matches C++ `HdRenderDelegate(settingsMap)` ctor).
    pub fn with_settings(settings_map: HdRenderSettingsMap) -> Self {
        Self {
            settings_map,
            settings_version: 1,
            display_name: String::new(),
        }
    }

    /// Populate settings map from descriptor defaults.
    ///
    /// Matches C++ `HdRenderDelegate::_PopulateDefaultSettings` (renderDelegate.cpp:211-220).
    /// Only inserts defaults for keys not already present.
    pub fn populate_default_settings(&mut self, default_settings: &HdRenderSettingDescriptorList) {
        for desc in default_settings {
            self.settings_map
                .entry(desc.key.clone())
                .or_insert_with(|| desc.default_value.clone());
        }
    }

    /// Set a render setting, incrementing the version.
    pub fn set_render_setting(&mut self, key: &Token, value: &Value) {
        self.settings_map.insert(key.clone(), value.clone());
        self.settings_version = self.settings_version.wrapping_add(1);
    }

    /// Get a render setting value.
    pub fn get_render_setting(&self, key: &Token) -> Option<&Value> {
        self.settings_map.get(key)
    }

    /// Get a render setting with a default fallback.
    ///
    /// Matches C++ template `HdRenderDelegate::GetRenderSetting<T>(key, defValue)`.
    /// Returns the stored value or the provided default.
    pub fn get_render_setting_or<'a>(&'a self, key: &Token, def_value: &'a Value) -> &'a Value {
        self.settings_map.get(key).unwrap_or(def_value)
    }

    /// Get current settings version.
    pub fn get_render_settings_version(&self) -> u32 {
        self.settings_version
    }

    /// Set the renderer display name (called by HdRendererPlugin).
    ///
    /// Matches C++ `HdRenderDelegate::_SetRendererDisplayName` (friend of HdRendererPlugin).
    pub fn set_renderer_display_name(&mut self, name: impl Into<String>) {
        self.display_name = name.into();
    }

    /// Get the renderer display name.
    pub fn get_renderer_display_name(&self) -> &str {
        &self.display_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestRenderParam;

    impl HdRenderParam for TestRenderParam {}

    #[test]
    fn test_render_param_default() {
        let param = TestRenderParam;
        // C++ default is_valid() returns false
        assert!(!param.is_valid());
        assert!(!param.has_arbitrary_value(&Token::new("test")));
    }

    #[test]
    fn test_render_setting_descriptor() {
        let desc = HdRenderSettingDescriptor::new(
            "Sample Count",
            Token::new("sampleCount"),
            Value::from(16i32),
        );

        assert_eq!(desc.name, "Sample Count");
        assert_eq!(desc.key.as_str(), "sampleCount");
    }

    #[test]
    fn test_rprim_collection() {
        let collection = HdRprimCollection::new(Token::new("geometry"));
        assert_eq!(collection.name.as_str(), "geometry");
    }
}
