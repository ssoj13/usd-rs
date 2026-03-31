#![allow(dead_code)]

//! HdStDynamicCubemapTextureImplementation - Interface for cubemap generators.
//!
//! Allows external clients to specify how a cubemap texture is loaded
//! and committed to the GPU. Used by HdStDynamicCubemapTextureObject.
//!
//! Port of pxr/imaging/hdSt/dynamicCubemapTextureImplementation.h

use super::dynamic_cubemap_texture_object::HdStDynamicCubemapTextureObject;

/// Trait for dynamic cubemap texture implementations.
///
/// External clients implement this to control how cubemap textures are
/// loaded from files (or generated procedurally) and committed to the GPU.
///
/// Port of HdStDynamicCubemapTextureImplementation
pub trait HdStDynamicCubemapTextureImpl: std::fmt::Debug + Send + Sync {
    /// Called during the load phase of the Storm texture system
    /// when a texture file is supposed to be loaded to the CPU.
    ///
    /// This method must be thread-safe.
    fn load(&self, texture_object: &mut HdStDynamicCubemapTextureObject);

    /// Called during the commit phase of the Storm texture system
    /// when the CPU texture is committed to the GPU.
    fn commit(&self, texture_object: &mut HdStDynamicCubemapTextureObject);

    /// Queried by the material system to determine whether to use
    /// the fallback value of a texture node.
    fn is_valid(&self, texture_object: &HdStDynamicCubemapTextureObject) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestImpl;

    impl HdStDynamicCubemapTextureImpl for TestImpl {
        fn load(&self, _obj: &mut HdStDynamicCubemapTextureObject) {}
        fn commit(&self, _obj: &mut HdStDynamicCubemapTextureObject) {}
        fn is_valid(&self, _obj: &HdStDynamicCubemapTextureObject) -> bool {
            true
        }
    }

    #[test]
    fn test_impl_trait() {
        let imp = TestImpl;
        assert!(imp.is_valid(&HdStDynamicCubemapTextureObject::default()));
    }
}
