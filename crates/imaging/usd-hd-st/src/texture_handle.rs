#![allow(dead_code)]

//! HdStTextureHandle - Reference-counted handle to texture objects.
//!
//! Provides a handle for referencing texture objects with associated
//! sampler parameters. Multiple handles can reference the same texture
//! object but with different sampler parameters (e.g. different wrap modes).
//!
//! Port of pxr/imaging/hdSt/textureHandle.h

use super::texture_identifier::HdStTextureIdentifier;
use super::texture_object::HdStTextureObjectSharedPtr;
use std::sync::Arc;
use usd_hd::types::HdSamplerParameters;
use usd_hgi::HgiTextureHandle;

/// Sampler parameters for texture handles.
///
/// Re-exports HdSamplerParameters from usd-hd with Storm-specific additions.
pub type SamplerParameters = HdSamplerParameters;

/// Memory request for texture loading.
///
/// Controls how much GPU memory a texture should target. Used for
/// texture streaming and LOD selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRequest {
    /// Target memory size in bytes (0 = full resolution)
    target_memory: usize,
}

impl MemoryRequest {
    /// Create a new memory request with specific byte budget.
    pub fn new(target_memory: usize) -> Self {
        Self { target_memory }
    }

    /// Request full resolution (no memory limit).
    pub fn full_resolution() -> Self {
        Self { target_memory: 0 }
    }

    /// Get target memory size.
    pub fn target_memory(&self) -> usize {
        self.target_memory
    }

    /// Check if this is a full resolution request.
    pub fn is_full_resolution(&self) -> bool {
        self.target_memory == 0
    }
}

impl Default for MemoryRequest {
    fn default() -> Self {
        Self::full_resolution()
    }
}

/// Handle to a texture object with associated sampler parameters.
///
/// Multiple handles can reference the same texture object but with
/// different sampler parameters. The texture handle does not own the
/// texture object - it's a reference into the texture registry.
///
/// # Lifecycle
/// 1. Created with identifier and sampler parameters
/// 2. Texture object resolved from registry
/// 3. Bound to shader via resource binder
/// 4. Dropped when shader/material no longer needs it
///
/// Port of HdStTextureHandle from pxr/imaging/hdSt/textureHandle.h
#[derive(Debug, Clone)]
pub struct HdStTextureHandle {
    /// Texture identifier
    identifier: HdStTextureIdentifier,

    /// Reference to texture object (resolved from registry)
    texture_object: Option<HdStTextureObjectSharedPtr>,

    /// Sampler parameters for this handle
    sampler_params: SamplerParameters,

    /// Memory request for loading
    memory_request: MemoryRequest,

    /// Handle is enabled for use
    enabled: bool,
}

impl HdStTextureHandle {
    /// Create a new texture handle.
    pub fn new(identifier: HdStTextureIdentifier, sampler_params: SamplerParameters) -> Self {
        Self {
            identifier,
            texture_object: None,
            sampler_params,
            memory_request: MemoryRequest::default(),
            enabled: true,
        }
    }

    /// Create handle with default sampler parameters.
    pub fn with_defaults(identifier: HdStTextureIdentifier) -> Self {
        Self::new(identifier, SamplerParameters::default())
    }

    /// Get the texture identifier.
    pub fn identifier(&self) -> &HdStTextureIdentifier {
        &self.identifier
    }

    /// Get the texture object if resolved.
    pub fn texture_object(&self) -> Option<&HdStTextureObjectSharedPtr> {
        self.texture_object.as_ref()
    }

    /// Set the texture object (from registry resolution).
    pub fn set_texture_object(&mut self, texture_object: HdStTextureObjectSharedPtr) {
        self.texture_object = Some(texture_object);
    }

    /// Clear the texture object reference.
    pub fn clear_texture_object(&mut self) {
        self.texture_object = None;
    }

    /// Get sampler parameters.
    pub fn sampler_params(&self) -> &SamplerParameters {
        &self.sampler_params
    }

    /// Get mutable sampler parameters.
    pub fn sampler_params_mut(&mut self) -> &mut SamplerParameters {
        &mut self.sampler_params
    }

    /// Set sampler parameters.
    pub fn set_sampler_params(&mut self, params: SamplerParameters) {
        self.sampler_params = params;
    }

    /// Get memory request.
    pub fn memory_request(&self) -> MemoryRequest {
        self.memory_request
    }

    /// Set memory request.
    pub fn set_memory_request(&mut self, request: MemoryRequest) {
        self.memory_request = request;
    }

    /// Check if handle is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set enabled state.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if texture is loaded/resolved.
    pub fn is_loaded(&self) -> bool {
        self.texture_object.is_some()
    }

    /// Check if handle is valid (enabled, loaded, and texture is valid).
    pub fn is_valid(&self) -> bool {
        self.enabled
            && self
                .texture_object
                .as_ref()
                .map_or(false, |obj| obj.is_valid())
    }

    /// Get the HGI texture handle if loaded and valid.
    pub fn hgi_texture(&self) -> Option<&HgiTextureHandle> {
        self.texture_object
            .as_ref()
            .filter(|obj| obj.is_valid())
            .map(|obj| obj.texture_handle())
    }
}

/// Shared pointer to texture handle.
pub type HdStTextureHandleSharedPtr = Arc<HdStTextureHandle>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture_object::HdStTextureObject;
    use usd_sdf::AssetPath;

    #[test]
    fn test_sampler_parameters_default() {
        let params = SamplerParameters::default();
        assert!(!params.enable_compare);
    }

    #[test]
    fn test_memory_request() {
        let full = MemoryRequest::full_resolution();
        assert!(full.is_full_resolution());
        assert_eq!(full.target_memory(), 0);

        let limited = MemoryRequest::new(1024 * 1024);
        assert!(!limited.is_full_resolution());
        assert_eq!(limited.target_memory(), 1024 * 1024);
    }

    #[test]
    fn test_handle_creation() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("diffuse.png"));
        let handle = HdStTextureHandle::with_defaults(id);

        assert!(!handle.is_loaded());
        assert!(handle.is_enabled());
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_handle_loading() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));
        let mut handle = HdStTextureHandle::with_defaults(id.clone());

        assert!(!handle.is_loaded());

        let tex = HdStTextureObject::new_2d(id);
        handle.set_texture_object(Arc::new(tex));

        assert!(handle.is_loaded());
        assert!(handle.texture_object().is_some());

        handle.clear_texture_object();
        assert!(!handle.is_loaded());
    }

    #[test]
    fn test_handle_enable_disable() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));
        let mut handle = HdStTextureHandle::with_defaults(id);

        assert!(handle.is_enabled());
        handle.set_enabled(false);
        assert!(!handle.is_enabled());
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_shared_handle() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("shared.png"));
        let handle = HdStTextureHandle::with_defaults(id);
        let shared: HdStTextureHandleSharedPtr = Arc::new(handle);
        let _clone = shared.clone();

        assert_eq!(Arc::strong_count(&shared), 2);
    }
}
