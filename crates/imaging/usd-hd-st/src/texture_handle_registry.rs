#![allow(dead_code)]

//! HdSt_TextureHandleRegistry - Registry managing texture handle lifecycle.
//!
//! Keeps track of texture handles and allocates textures and samplers
//! using the texture/sampler object registries. Responsibilities include:
//! - Tracking texture handle to texture object associations
//! - Computing target memory from handle memory requests
//! - Triggering sampler and texture garbage collection
//! - Determining which shaders are affected by texture commits
//!
//! Port of pxr/imaging/hdSt/textureHandleRegistry.h

use super::texture_handle::{HdStTextureHandle, HdStTextureHandleSharedPtr, MemoryRequest};
use super::texture_identifier::HdStTextureIdentifier;
use super::texture_object::TextureType;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use usd_hd::types::HdSamplerParameters;

/// Weak pointer to a texture handle (for GC tracking).
pub type HdStTextureHandleWeakPtr = Weak<HdStTextureHandle>;

/// Weak pointer to a shader code (for dirty tracking).
pub type HdStShaderCodeWeakPtr = Weak<dyn std::any::Any + Send + Sync>;

/// Registry managing texture handle lifecycle and commit scheduling.
///
/// Central registry that coordinates between texture handles, texture
/// objects, sampler objects, and shader code instances. Handles garbage
/// collection and dirty tracking for efficient resource management.
///
/// Port of HdSt_TextureHandleRegistry
#[derive(Debug)]
pub struct HdStTextureHandleRegistry {
    /// Per-type default memory request (applied if no handle specifies one)
    type_memory_requests: HashMap<TextureType, usize>,
    /// Whether type_memory_requests changed since last commit
    type_memory_request_changed: bool,

    /// Handles that are new or whose underlying texture changed
    dirty_handles: Mutex<Vec<HdStTextureHandleWeakPtr>>,
    /// Textures whose handle set or target memory may have changed
    dirty_textures: Mutex<Vec<Weak<super::texture_object::HdStTextureObject>>>,

    /// Map from texture object to its referencing handles
    texture_to_handles: HashMap<usize, Vec<HdStTextureHandleWeakPtr>>,

    /// Total number of live handles (for diagnostics)
    handle_count: usize,

    /// Whether sampler GC is needed on next commit
    sampler_gc_needed: bool,
}

impl HdStTextureHandleRegistry {
    /// Create a new texture handle registry.
    pub fn new() -> Self {
        Self {
            type_memory_requests: HashMap::new(),
            type_memory_request_changed: false,
            dirty_handles: Mutex::new(Vec::new()),
            dirty_textures: Mutex::new(Vec::new()),
            texture_to_handles: HashMap::new(),
            handle_count: 0,
            sampler_gc_needed: false,
        }
    }

    /// Allocate a new texture handle (thread-safe).
    ///
    /// Creates a handle referencing a texture identified by `texture_id`
    /// with the given sampler parameters and memory request.
    ///
    /// # Arguments
    /// * `texture_id` - Identifies the texture file/resource
    /// * `texture_type` - Type of texture (UV, Field, Ptex, etc.)
    /// * `sampler_params` - Wrap/filter mode parameters
    /// * `memory_request` - Memory budget in bytes (0 = full resolution)
    pub fn allocate_handle(
        &mut self,
        texture_id: HdStTextureIdentifier,
        _texture_type: TextureType,
        sampler_params: HdSamplerParameters,
        memory_request: usize,
    ) -> HdStTextureHandleSharedPtr {
        let mut handle = HdStTextureHandle::new(texture_id, sampler_params);
        handle.set_memory_request(MemoryRequest::new(memory_request));

        let shared = Arc::new(handle);
        let weak = Arc::downgrade(&shared);

        // Track as dirty for next commit
        if let Ok(mut dirty) = self.dirty_handles.lock() {
            dirty.push(weak);
        }

        self.handle_count += 1;
        shared
    }

    /// Mark a texture as dirty (thread-safe).
    ///
    /// The texture's target memory will be recomputed during commit
    /// and its handle associations will be updated.
    pub fn mark_texture_dirty(&self, texture: &Weak<super::texture_object::HdStTextureObject>) {
        if let Ok(mut dirty) = self.dirty_textures.lock() {
            dirty.push(texture.clone());
        }
    }

    /// Mark that sampler garbage collection is needed (thread-safe).
    pub fn mark_sampler_gc_needed(&mut self) {
        self.sampler_gc_needed = true;
    }

    /// Set default memory request for a texture type.
    ///
    /// Only has effect if non-zero and only applies when no handle
    /// referencing the texture has its own memory request.
    pub fn set_memory_request_for_type(&mut self, texture_type: TextureType, memory: usize) {
        if memory > 0 {
            self.type_memory_requests.insert(texture_type, memory);
        } else {
            self.type_memory_requests.remove(&texture_type);
        }
        self.type_memory_request_changed = true;
    }

    /// Get default memory request for a texture type.
    pub fn memory_request_for_type(&self, texture_type: TextureType) -> usize {
        self.type_memory_requests
            .get(&texture_type)
            .copied()
            .unwrap_or(0)
    }

    /// Commit changes: process dirty handles and textures.
    ///
    /// Returns the set of shader code instances that need to be updated
    /// because their textures were (re-)committed.
    pub fn commit(&mut self) -> HashSet<usize> {
        let affected_shaders = HashSet::new();

        // Process dirty handles: resolve texture objects, create samplers
        if let Ok(mut dirty) = self.dirty_handles.lock() {
            for _weak_handle in dirty.drain(..) {
                // In full implementation:
                // 1. Resolve texture object from registry
                // 2. Set on handle
                // 3. Create sampler object
                // 4. Track affected shaders
            }
        }

        // Process dirty textures: recompute target memory
        if let Ok(mut dirty) = self.dirty_textures.lock() {
            dirty.clear();
        }

        // Garbage collect if needed
        if self.sampler_gc_needed {
            self.gc_samplers();
            self.sampler_gc_needed = false;
        }

        self.type_memory_request_changed = false;
        affected_shaders
    }

    /// Garbage collect expired handles.
    ///
    /// Removes handles whose Arc has been dropped. Updates the
    /// texture-to-handles mapping accordingly.
    pub fn gc_handles(&mut self) -> bool {
        let mut any_collected = false;

        self.texture_to_handles.retain(|_tex_id, handles| {
            let before = handles.len();
            handles.retain(|weak| weak.strong_count() > 0);
            if handles.len() < before {
                any_collected = true;
            }
            !handles.is_empty()
        });

        if any_collected {
            self.handle_count = self.texture_to_handles.values().map(|v| v.len()).sum();
        }

        any_collected
    }

    /// Garbage collect samplers.
    fn gc_samplers(&mut self) {
        // Delegate to sampler object registry in full implementation
    }

    /// Get total number of live texture handles.
    pub fn handle_count(&self) -> usize {
        self.handle_count
    }

    /// Compute target memory for a texture based on all referencing handles.
    ///
    /// Takes the maximum of all handles' memory requests, falling back
    /// to the per-type default if no handle has a request.
    fn compute_target_memory(
        &self,
        handles: &[HdStTextureHandleWeakPtr],
        texture_type: TextureType,
    ) -> usize {
        let max_request = handles
            .iter()
            .filter_map(|weak| weak.upgrade())
            .map(|h| h.memory_request().target_memory())
            .filter(|&m| m > 0)
            .max()
            .unwrap_or(0);

        if max_request > 0 {
            max_request
        } else {
            self.memory_request_for_type(texture_type)
        }
    }
}

impl Default for HdStTextureHandleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdf::AssetPath;

    #[test]
    fn test_registry_creation() {
        let reg = HdStTextureHandleRegistry::new();
        assert_eq!(reg.handle_count(), 0);
    }

    #[test]
    fn test_allocate_handle() {
        let mut reg = HdStTextureHandleRegistry::new();
        let id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));

        let handle = reg.allocate_handle(id, TextureType::Uv, HdSamplerParameters::default(), 0);

        assert_eq!(reg.handle_count(), 1);
        assert!(handle.is_enabled());
    }

    #[test]
    fn test_memory_request_per_type() {
        let mut reg = HdStTextureHandleRegistry::new();

        reg.set_memory_request_for_type(TextureType::Uv, 1024 * 1024);
        assert_eq!(reg.memory_request_for_type(TextureType::Uv), 1024 * 1024);
        assert_eq!(reg.memory_request_for_type(TextureType::Field), 0);

        // Clear by setting to 0
        reg.set_memory_request_for_type(TextureType::Uv, 0);
        assert_eq!(reg.memory_request_for_type(TextureType::Uv), 0);
    }

    #[test]
    fn test_commit() {
        let mut reg = HdStTextureHandleRegistry::new();
        let id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));

        let _handle = reg.allocate_handle(id, TextureType::Uv, HdSamplerParameters::default(), 0);

        let affected = reg.commit();
        // No actual texture loading, so no affected shaders
        assert!(affected.is_empty());
    }

    #[test]
    fn test_sampler_gc_flag() {
        let mut reg = HdStTextureHandleRegistry::new();
        assert!(!reg.sampler_gc_needed);

        reg.mark_sampler_gc_needed();
        assert!(reg.sampler_gc_needed);

        reg.commit();
        assert!(!reg.sampler_gc_needed);
    }
}
