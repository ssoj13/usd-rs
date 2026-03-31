
//! HioFieldTextureData - interface for reading volume files with transformations.

use super::types::HioFormat;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use usd_gf::BBox3d;

/// Shared pointer type for field texture data
pub type HioFieldTextureDataSharedPtr = Arc<dyn HioFieldTextureData>;

/// Factory function type for creating field texture data handlers
pub type FieldTextureDataFactory = fn(
    file_path: &str,
    field_name: &str,
    field_index: i32,
    field_purpose: &str,
    target_memory: usize,
) -> Option<HioFieldTextureDataSharedPtr>;

/// An interface class for reading volume files having a transformation.
///
/// This is used for loading volumetric data from formats like OpenVDB and Field3D.
pub trait HioFieldTextureData: Send + Sync {
    /// Bounding box describing how 3d texture maps into world space.
    fn bounding_box(&self) -> &BBox3d;

    /// Width of the resized texture
    fn resized_width(&self) -> i32;

    /// Height of the resized texture
    fn resized_height(&self) -> i32;

    /// Depth of the resized texture
    fn resized_depth(&self) -> i32;

    /// Get the format of the texture data
    fn format(&self) -> HioFormat;

    /// Read the texture data
    fn read(&mut self) -> bool;

    /// Check if raw buffer is available
    fn has_raw_buffer(&self) -> bool;

    /// Get the raw buffer data
    fn raw_buffer(&self) -> Option<&[u8]>;
}

/// Registry for field texture data factories
///
/// Singleton registry that manages file format handlers for volumetric texture data.
/// Corresponds to `HioFieldTextureDataFactoryRegistry` in OpenUSD.
///
/// # See Also
/// - `pxr/imaging/hio/fieldTextureData.h` in OpenUSD
pub struct HioFieldTextureDataRegistry {
    /// Map of file extensions to factory functions for creating texture data handlers
    factories: RwLock<HashMap<String, FieldTextureDataFactory>>,
}

impl HioFieldTextureDataRegistry {
    /// Get the global registry instance
    pub fn instance() -> &'static HioFieldTextureDataRegistry {
        static INSTANCE: std::sync::OnceLock<HioFieldTextureDataRegistry> =
            std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| HioFieldTextureDataRegistry {
            factories: RwLock::new(HashMap::new()),
        })
    }

    /// Register a factory for a file extension
    pub fn register(&self, extension: &str, factory: FieldTextureDataFactory) {
        let ext = extension.to_lowercase();
        let mut factories = self.factories.write().expect("lock poisoned");
        factories.insert(ext, factory);
    }

    /// Check if an extension is supported
    pub fn is_supported_extension(&self, extension: &str) -> bool {
        let ext = extension.to_lowercase();
        let factories = self.factories.read().expect("lock poisoned");
        factories.contains_key(&ext)
    }

    /// Create a new field texture data handler
    ///
    /// # Arguments
    /// * `file_path` - Path to the volume file
    /// * `field_name` - Field/grid name (e.g., gridName in OpenVDB or layer/attribute in Field3D)
    /// * `field_index` - Partition index
    /// * `field_purpose` - Partition name/grouping
    /// * `target_memory` - Target memory usage in bytes
    pub fn create(
        &self,
        file_path: &str,
        _field_name: &str,
        _field_index: i32,
        _field_purpose: &str,
        _target_memory: usize,
    ) -> Option<HioFieldTextureDataSharedPtr> {
        let path = Path::new(file_path);
        let ext = path.extension()?.to_str()?.to_lowercase();

        let factories = self.factories.read().expect("lock poisoned");
        let factory = factories.get(&ext)?;

        factory(
            file_path,
            _field_name,
            _field_index,
            _field_purpose,
            _target_memory,
        )
    }

    /// Get list of all registered extensions
    pub fn registered_extensions(&self) -> Vec<String> {
        let factories = self.factories.read().expect("lock poisoned");
        factories.keys().cloned().collect()
    }

    /// Clear all registered factories (mainly for testing)
    pub fn clear(&self) {
        let mut factories = self.factories.write().expect("lock poisoned");
        factories.clear();
    }
}

/// Base implementation for field texture data
///
/// Provides storage and accessors for volumetric texture data loaded from files.
/// This is a helper struct that can be used to implement the [`HioFieldTextureData`] trait.
///
/// # See Also
/// - `pxr/imaging/hio/fieldTextureData.h` in OpenUSD
pub struct HioFieldTextureDataBase {
    /// 3D bounding box defining how the texture maps to world space coordinates
    bounding_box: BBox3d,
    /// Width of the loaded/resized texture in texels
    width: i32,
    /// Height of the loaded/resized texture in texels
    height: i32,
    /// Depth of the loaded/resized texture in texels
    depth: i32,
    /// Pixel format of the texture data (e.g., float32, uint8)
    format: HioFormat,
    /// Raw texture data buffer in the specified format
    raw_buffer: Vec<u8>,
}

impl HioFieldTextureDataBase {
    /// Creates a new empty field texture data with default values.
    ///
    /// Initializes with an empty bounding box, zero dimensions,
    /// invalid format, and empty buffer.
    pub fn new() -> Self {
        Self {
            bounding_box: BBox3d::default(),
            width: 0,
            height: 0,
            depth: 0,
            format: HioFormat::Invalid,
            raw_buffer: Vec::new(),
        }
    }

    /// Sets the bounding box that maps the 3D texture to world space.
    ///
    /// # Arguments
    /// * `bbox` - 3D bounding box in world coordinates
    pub fn set_bounding_box(&mut self, bbox: BBox3d) {
        self.bounding_box = bbox;
    }

    /// Sets the dimensions of the resized/loaded texture.
    ///
    /// # Arguments
    /// * `width` - Width in texels
    /// * `height` - Height in texels
    /// * `depth` - Depth in texels (Z dimension for 3D textures)
    pub fn set_dimensions(&mut self, width: i32, height: i32, depth: i32) {
        self.width = width;
        self.height = height;
        self.depth = depth;
    }

    /// Sets the pixel format of the texture data.
    ///
    /// # Arguments
    /// * `format` - Pixel format (e.g., `HioFormat::Float32`, `HioFormat::UInt8`)
    pub fn set_format(&mut self, format: HioFormat) {
        self.format = format;
    }

    /// Sets the raw texture data buffer.
    ///
    /// The buffer should contain pixel data in the format specified by [`set_format`](Self::set_format),
    /// with dimensions matching those set by [`set_dimensions`](Self::set_dimensions).
    ///
    /// # Arguments
    /// * `buffer` - Raw pixel data as bytes
    pub fn set_raw_buffer(&mut self, buffer: Vec<u8>) {
        self.raw_buffer = buffer;
    }

    /// Returns the bounding box defining the texture's world space mapping.
    ///
    /// # Returns
    /// Reference to the 3D bounding box
    pub fn bounding_box(&self) -> &BBox3d {
        &self.bounding_box
    }

    /// Returns the width of the texture in texels.
    ///
    /// # Returns
    /// Width of the resized/loaded texture
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Returns the height of the texture in texels.
    ///
    /// # Returns
    /// Height of the resized/loaded texture
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Returns the depth of the texture in texels.
    ///
    /// # Returns
    /// Depth (Z dimension) of the 3D texture
    pub fn depth(&self) -> i32 {
        self.depth
    }

    /// Returns the pixel format of the texture data.
    ///
    /// # Returns
    /// Pixel format enum value
    pub fn format(&self) -> HioFormat {
        self.format
    }

    /// Checks if texture data has been loaded into the raw buffer.
    ///
    /// # Returns
    /// `true` if buffer contains data, `false` if empty
    pub fn has_raw_buffer(&self) -> bool {
        !self.raw_buffer.is_empty()
    }

    /// Returns a slice of the raw texture data buffer.
    ///
    /// # Returns
    /// - `Some(&[u8])` - Slice of raw pixel data if loaded
    /// - `None` - If no data has been loaded yet
    pub fn raw_buffer(&self) -> Option<&[u8]> {
        if self.raw_buffer.is_empty() {
            None
        } else {
            Some(&self.raw_buffer)
        }
    }
}

impl Default for HioFieldTextureDataBase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_singleton() {
        let registry1 = HioFieldTextureDataRegistry::instance();
        let registry2 = HioFieldTextureDataRegistry::instance();

        assert!(std::ptr::eq(registry1, registry2));
    }

    #[test]
    fn test_register_extension() {
        let registry = HioFieldTextureDataRegistry::instance();

        fn mock_factory(
            _file_path: &str,
            _field_name: &str,
            _field_index: i32,
            _field_purpose: &str,
            _target_memory: usize,
        ) -> Option<HioFieldTextureDataSharedPtr> {
            None
        }

        // Use unique extension name to avoid conflicts with other tests
        registry.register("test_vdb", mock_factory);

        assert!(registry.is_supported_extension("test_vdb"));
        assert!(registry.is_supported_extension("TEST_VDB"));
        assert!(!registry.is_supported_extension("test_unknown"));
    }

    #[test]
    fn test_base_implementation() {
        let mut base = HioFieldTextureDataBase::new();

        base.set_dimensions(128, 128, 128);
        base.set_format(HioFormat::Float32);
        base.set_raw_buffer(vec![0u8; 1024]);

        assert_eq!(base.width(), 128);
        assert_eq!(base.height(), 128);
        assert_eq!(base.depth(), 128);
        assert_eq!(base.format(), HioFormat::Float32);
        assert!(base.has_raw_buffer());
        assert_eq!(base.raw_buffer().unwrap().len(), 1024);
    }

    #[test]
    fn test_registered_extensions() {
        let registry = HioFieldTextureDataRegistry::instance();

        fn mock_factory(
            _file_path: &str,
            _field_name: &str,
            _field_index: i32,
            _field_purpose: &str,
            _target_memory: usize,
        ) -> Option<HioFieldTextureDataSharedPtr> {
            None
        }

        // Use unique extension names to avoid conflicts with other tests
        registry.register("test_ext1", mock_factory);
        registry.register("test_ext2", mock_factory);

        let extensions = registry.registered_extensions();
        // Check that our extensions are present (may have more from other tests)
        assert!(extensions.contains(&"test_ext1".to_string()));
        assert!(extensions.contains(&"test_ext2".to_string()));
    }
}
