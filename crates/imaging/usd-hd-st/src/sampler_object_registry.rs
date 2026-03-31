#![allow(dead_code)]

//! Sampler object registry for Storm.
//!
//! Simple registry for GPU samplers. Construction dispatches by texture
//! type to return a matching sampler. Keeps shared pointers alive until
//! garbage collection.
//!
//! Port of pxr/imaging/hdSt/samplerObjectRegistry.h

use super::sampler_object::HdStSamplerObjectSharedPtr;
use super::texture_object::HdStTextureObjectSharedPtr;
use std::sync::Arc;
use usd_hd::types::HdSamplerParameters;

/// GPU sampler object registry.
///
/// No deduplication - each allocation creates a new sampler.
/// Garbage collection removes samplers no longer referenced by clients.
///
/// Port of HdSt_SamplerObjectRegistry
#[derive(Debug)]
pub struct SamplerObjectRegistry {
    /// All allocated sampler objects
    sampler_objects: Vec<HdStSamplerObjectSharedPtr>,
    /// Whether GC is needed
    gc_needed: bool,
}

impl SamplerObjectRegistry {
    /// Create a new sampler object registry.
    pub fn new() -> Self {
        Self {
            sampler_objects: Vec::new(),
            gc_needed: false,
        }
    }

    /// Allocate a new sampler matching the given texture object.
    ///
    /// Creates the GPU resource immediately. Not thread-safe.
    pub fn alloc_sampler(
        &mut self,
        texture: &HdStTextureObjectSharedPtr,
        params: &HdSamplerParameters,
    ) -> HdStSamplerObjectSharedPtr {
        use super::sampler_object::*;
        use super::texture_object::TextureType;

        let sampler = match texture.texture_type() {
            TextureType::Uv => HdStSamplerObject::Uv(HdStUvSamplerObject::new(params.clone())),
            TextureType::Field => {
                HdStSamplerObject::Field(HdStFieldSamplerObject::new(params.clone()))
            }
            TextureType::Ptex => {
                HdStSamplerObject::Ptex(HdStPtexSamplerObject::new(params.clone()))
            }
            TextureType::Udim => {
                HdStSamplerObject::Udim(HdStUdimSamplerObject::new(params.clone()))
            }
            TextureType::Cubemap => {
                HdStSamplerObject::Cubemap(HdStCubemapSamplerObject::new(params.clone()))
            }
        };

        let shared: HdStSamplerObjectSharedPtr = Arc::new(sampler);
        self.sampler_objects.push(shared.clone());
        shared
    }

    /// Delete samplers no longer used by any client.
    pub fn garbage_collect(&mut self) {
        if !self.gc_needed {
            return;
        }
        // Remove entries where our Arc is the only remaining reference
        self.sampler_objects.retain(|s| Arc::strong_count(s) > 1);
        self.gc_needed = false;
    }

    /// Mark that garbage collection is needed.
    pub fn mark_gc_needed(&mut self) {
        self.gc_needed = true;
    }

    /// Get the number of tracked sampler objects.
    pub fn len(&self) -> usize {
        self.sampler_objects.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.sampler_objects.is_empty()
    }
}

impl Default for SamplerObjectRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture_identifier::HdStTextureIdentifier;
    use crate::texture_object::HdStTextureObject;
    use usd_sdf::AssetPath;

    #[test]
    fn test_alloc_and_gc() {
        let mut registry = SamplerObjectRegistry::new();
        let tex_id = HdStTextureIdentifier::from_path(AssetPath::new("test.png"));
        let tex: HdStTextureObjectSharedPtr = Arc::new(HdStTextureObject::new_2d(tex_id));
        let params = HdSamplerParameters::default();

        let s1 = registry.alloc_sampler(&tex, &params);
        let _s2 = registry.alloc_sampler(&tex, &params);
        assert_eq!(registry.len(), 2);

        // Drop s1, mark GC, collect
        drop(s1);
        registry.mark_gc_needed();
        registry.garbage_collect();
        // s1 dropped but _s2 still alive
        assert_eq!(registry.len(), 1);
    }
}
