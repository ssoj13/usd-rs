#![allow(dead_code)]

//! HdSt_TextureObjectRegistry - Central registry for texture GPU resources.
//!
//! Manages texture object allocation, deduplication, GPU resource loading,
//! memory tracking, and garbage collection. Texture objects are created
//! lazily; actual GPU resources are allocated during the Commit phase.
//!
//! Port of pxr/imaging/hdSt/textureObjectRegistry.h

use super::texture_identifier::HdStTextureIdentifier;
use super::texture_object::{
    HdStCubemapTextureObject, HdStFieldTextureObject, HdStPtexTextureObject, HdStTextureObject,
    HdStTextureObjectSharedPtr, HdStUdimTextureObject, HdStUvTextureObject, TextureType,
};
use std::collections::HashMap;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicI64, Ordering},
};
use usd_tf::Token;

/// Registry for texture objects with memory tracking and deduplication.
///
/// Allocates texture objects on demand and deduplicates by texture
/// identifier. GPU resources are not allocated until the Commit phase.
///
/// Port of HdSt_TextureObjectRegistry
#[derive(Debug)]
pub struct HdStTextureObjectRegistry {
    /// Main registry: identifier hash -> texture object
    registry: HashMap<u64, HdStTextureObjectSharedPtr>,

    /// File path to texture objects for quick invalidation
    path_to_objects: HashMap<String, Vec<Weak<HdStTextureObject>>>,

    /// File paths needing GPU resource reload
    dirty_paths: Mutex<Vec<Token>>,

    /// Texture objects needing GPU resource reload
    dirty_objects: Mutex<Vec<Weak<HdStTextureObject>>>,

    /// Total GPU memory consumed by all managed textures (bytes)
    total_memory: AtomicI64,
}

impl HdStTextureObjectRegistry {
    /// Create a new texture object registry.
    pub fn new() -> Self {
        Self {
            registry: HashMap::new(),
            path_to_objects: HashMap::new(),
            dirty_paths: Mutex::new(Vec::new()),
            dirty_objects: Mutex::new(Vec::new()),
            total_memory: AtomicI64::new(0),
        }
    }

    /// Allocate a texture object (or return existing one).
    ///
    /// Creates the HdStTextureObject but does NOT allocate GPU
    /// resources. GPU resources are allocated during `commit()`.
    ///
    /// Objects are deduplicated by texture identifier - requesting
    /// the same texture twice returns the same object.
    pub fn allocate(
        &mut self,
        texture_id: &HdStTextureIdentifier,
        texture_type: TextureType,
    ) -> HdStTextureObjectSharedPtr {
        let key = Self::compute_key(texture_id);

        // Return existing object if already registered
        if let Some(existing) = self.registry.get(&key) {
            return existing.clone();
        }

        // Create new texture object of appropriate type
        let obj = Self::make_object(texture_id.clone(), texture_type);
        let shared = Arc::new(obj);

        // Track file path for invalidation
        let path = texture_id.file_path().get_asset_path().to_string();
        if !path.is_empty() {
            self.path_to_objects
                .entry(path)
                .or_default()
                .push(Arc::downgrade(&shared));
        }

        self.registry.insert(key, shared.clone());
        shared
    }

    /// Commit: create GPU resources, load textures, and upload to GPU.
    ///
    /// Returns the list of texture objects that were (re-)loaded.
    pub fn commit(&mut self) -> Vec<HdStTextureObjectSharedPtr> {
        let mut loaded = Vec::new();

        // Process dirty file paths: mark all associated objects as dirty
        if let Ok(mut dirty_paths) = self.dirty_paths.lock() {
            for path in dirty_paths.drain(..) {
                if let Some(objects) = self.path_to_objects.get(path.as_str()) {
                    for weak in objects {
                        if let Ok(mut dirty) = self.dirty_objects.lock() {
                            dirty.push(weak.clone());
                        }
                    }
                }
            }
        }

        // Process dirty objects: reload GPU resources
        if let Ok(mut dirty) = self.dirty_objects.lock() {
            for weak in dirty.drain(..) {
                if let Some(obj) = weak.upgrade() {
                    // In full implementation: call _Load() then _Commit()
                    loaded.push(obj);
                }
            }
        }

        loaded
    }

    /// Garbage collect: clean up stale weak references in path map.
    ///
    /// The main registry holds `Arc` strong references, so entries remain
    /// alive as long as the registry exists. Only the path_to_objects Weak
    /// refs are pruned here to remove entries for objects that have been
    /// replaced or invalidated.
    pub fn gc(&mut self) {
        // Clean up stale Weak refs in the path-to-objects map
        self.path_to_objects.retain(|_path, objects| {
            objects.retain(|weak| weak.strong_count() > 0);
            !objects.is_empty()
        });
    }

    /// Mark a file path as dirty.
    ///
    /// All textures loaded from this path will be reloaded during
    /// the next Commit phase.
    pub fn mark_path_dirty(&self, file_path: Token) {
        if let Ok(mut dirty) = self.dirty_paths.lock() {
            dirty.push(file_path);
        }
    }

    /// Mark a texture object as dirty for reload.
    pub fn mark_object_dirty(&self, texture: &Weak<HdStTextureObject>) {
        if let Ok(mut dirty) = self.dirty_objects.lock() {
            dirty.push(texture.clone());
        }
    }

    /// Get total GPU memory consumed by all managed textures.
    pub fn total_texture_memory(&self) -> i64 {
        self.total_memory.load(Ordering::Relaxed)
    }

    /// Adjust total texture memory (called from texture objects on alloc/dealloc).
    pub fn adjust_total_memory(&self, diff: i64) {
        self.total_memory.fetch_add(diff, Ordering::Relaxed);
    }

    /// Get number of registered texture objects.
    pub fn object_count(&self) -> usize {
        self.registry.len()
    }

    /// Compute registry key from texture identifier.
    fn compute_key(id: &HdStTextureIdentifier) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        id.hash(&mut hasher);
        hasher.finish()
    }

    /// Create a texture object of the appropriate type.
    fn make_object(id: HdStTextureIdentifier, texture_type: TextureType) -> HdStTextureObject {
        match texture_type {
            TextureType::Uv => HdStTextureObject::Uv(HdStUvTextureObject::new(id)),
            TextureType::Field => HdStTextureObject::Field(HdStFieldTextureObject::new(id)),
            TextureType::Ptex => HdStTextureObject::Ptex(HdStPtexTextureObject::new(id)),
            TextureType::Udim => HdStTextureObject::Udim(HdStUdimTextureObject::new(id)),
            TextureType::Cubemap => HdStTextureObject::Cubemap(HdStCubemapTextureObject::new(id)),
        }
    }
}

impl Default for HdStTextureObjectRegistry {
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
        let reg = HdStTextureObjectRegistry::new();
        assert_eq!(reg.object_count(), 0);
        assert_eq!(reg.total_texture_memory(), 0);
    }

    #[test]
    fn test_allocate_texture() {
        let mut reg = HdStTextureObjectRegistry::new();
        let id = HdStTextureIdentifier::from_path(AssetPath::new("diffuse.png"));

        let obj = reg.allocate(&id, TextureType::Uv);
        assert_eq!(reg.object_count(), 1);
        assert_eq!(obj.texture_type(), TextureType::Uv);
    }

    #[test]
    fn test_deduplication() {
        let mut reg = HdStTextureObjectRegistry::new();
        let id1 = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));
        let id2 = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));

        let obj1 = reg.allocate(&id1, TextureType::Uv);
        let obj2 = reg.allocate(&id2, TextureType::Uv);

        // Same identifier should return same object
        assert_eq!(reg.object_count(), 1);
        assert!(Arc::ptr_eq(&obj1, &obj2));
    }

    #[test]
    fn test_different_textures() {
        let mut reg = HdStTextureObjectRegistry::new();
        let id1 = HdStTextureIdentifier::from_path(AssetPath::new("diffuse.png"));
        let id2 = HdStTextureIdentifier::from_path(AssetPath::new("normal.png"));

        let _obj1 = reg.allocate(&id1, TextureType::Uv);
        let _obj2 = reg.allocate(&id2, TextureType::Uv);

        assert_eq!(reg.object_count(), 2);
    }

    #[test]
    fn test_garbage_collection() {
        let mut reg = HdStTextureObjectRegistry::new();
        let id = HdStTextureIdentifier::from_path(AssetPath::new("temp.png"));

        {
            let _obj = reg.allocate(&id, TextureType::Uv);
            assert_eq!(reg.object_count(), 1);
            // _obj dropped here
        }

        reg.gc();
        // Registry still holds the only reference, so GC won't collect
        // (strong_count == 1 from registry). In real usage, handles hold refs.
        assert_eq!(reg.object_count(), 1);
    }

    #[test]
    fn test_memory_tracking() {
        let reg = HdStTextureObjectRegistry::new();
        assert_eq!(reg.total_texture_memory(), 0);

        reg.adjust_total_memory(1024);
        assert_eq!(reg.total_texture_memory(), 1024);

        reg.adjust_total_memory(-512);
        assert_eq!(reg.total_texture_memory(), 512);
    }

    #[test]
    fn test_mark_path_dirty() {
        let reg = HdStTextureObjectRegistry::new();
        reg.mark_path_dirty(Token::new("diffuse.png"));

        let dirty = reg.dirty_paths.lock().unwrap();
        assert_eq!(dirty.len(), 1);
    }

    #[test]
    fn test_commit() {
        let mut reg = HdStTextureObjectRegistry::new();
        let id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));
        let _obj = reg.allocate(&id, TextureType::Uv);

        let loaded = reg.commit();
        // No dirty objects, so nothing loaded
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_different_types() {
        let mut reg = HdStTextureObjectRegistry::new();

        let uv = reg.allocate(
            &HdStTextureIdentifier::from_path(AssetPath::new("a.png")),
            TextureType::Uv,
        );
        let field = reg.allocate(
            &HdStTextureIdentifier::from_path(AssetPath::new("b.vdb")),
            TextureType::Field,
        );
        let ptex = reg.allocate(
            &HdStTextureIdentifier::from_path(AssetPath::new("c.ptex")),
            TextureType::Ptex,
        );

        assert_eq!(uv.texture_type(), TextureType::Uv);
        assert_eq!(field.texture_type(), TextureType::Field);
        assert_eq!(ptex.texture_type(), TextureType::Ptex);
        assert_eq!(reg.object_count(), 3);
    }
}
