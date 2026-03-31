#![allow(dead_code)]

//! HdStDynamicUvTextureImplementation - Interface for dynamic texture generators.
//!
//! Allows external clients to specify how a UV texture is loaded from,
//! e.g., a file and how it is committed to the GPU. Clients implement
//! this trait to provide custom load and commit behavior for dynamic
//! textures (procedural textures, render targets, AOVs, etc.).
//!
//! Port of pxr/imaging/hdSt/dynamicUvTextureImplementation.h

use super::dynamic_uv_texture_object::HdStDynamicUvTextureObject;

/// Trait for dynamic UV texture implementations.
///
/// Provides custom load and commit behavior for textures managed
/// by external clients rather than by the Storm texture system.
///
/// # Thread Safety
/// - `load()` must be thread-safe (called from worker threads)
/// - `commit()` is called on the main thread only
///
/// # Example
/// ```ignore
/// struct MyProceduralTexture;
///
/// impl HdStDynamicUvTextureImpl for MyProceduralTexture {
///     fn load(&self, texture: &mut HdStDynamicUvTextureObject) {
///         // Generate procedural data on CPU
///         let data = generate_noise(512, 512);
///         texture.set_cpu_data(data);
///     }
///
///     fn commit(&self, texture: &mut HdStDynamicUvTextureObject) {
///         // Upload CPU data to GPU
///         if let Some(cpu_data) = texture.cpu_data() {
///             texture.create_texture_from_desc(cpu_data.texture_desc());
///         }
///     }
///
///     fn is_valid(&self, _texture: &HdStDynamicUvTextureObject) -> bool {
///         true
///     }
/// }
/// ```
///
/// Port of HdStDynamicUvTextureImplementation
pub trait HdStDynamicUvTextureImpl: std::fmt::Debug + Send + Sync {
    /// Called during load phase to populate CPU data.
    ///
    /// Must be thread-safe. Typically reads from a file or generates
    /// procedural data and stores it via `texture.set_cpu_data()`.
    fn load(&self, texture: &mut HdStDynamicUvTextureObject);

    /// Called during commit phase to upload CPU data to GPU.
    ///
    /// Called on the main thread only. Typically creates the GPU
    /// texture from the CPU data set during load.
    fn commit(&self, texture: &mut HdStDynamicUvTextureObject);

    /// Query whether the texture is valid.
    ///
    /// Used by the material system to determine whether to use
    /// fallback values instead of this texture.
    fn is_valid(&self, texture: &HdStDynamicUvTextureObject) -> bool;
}

/// Shared pointer to a dynamic UV texture implementation.
pub type HdStDynamicUvTextureImplSharedPtr = std::sync::Arc<dyn HdStDynamicUvTextureImpl>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texture_identifier::HdStTextureIdentifier;
    use usd_sdf::AssetPath;

    #[derive(Debug)]
    struct TestImpl;

    impl HdStDynamicUvTextureImpl for TestImpl {
        fn load(&self, _texture: &mut HdStDynamicUvTextureObject) {
            // No-op for test
        }

        fn commit(&self, _texture: &mut HdStDynamicUvTextureObject) {
            // No-op for test
        }

        fn is_valid(&self, _texture: &HdStDynamicUvTextureObject) -> bool {
            true
        }
    }

    #[test]
    fn test_impl_trait() {
        let id = HdStTextureIdentifier::from_path(AssetPath::new("dynamic.png"));
        let mut obj = HdStDynamicUvTextureObject::new(id);
        let imp = TestImpl;

        imp.load(&mut obj);
        imp.commit(&mut obj);
        assert!(imp.is_valid(&obj));
    }
}
