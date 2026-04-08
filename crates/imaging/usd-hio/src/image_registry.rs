//! HioImageRegistry - manages plugin registration and loading for HioImage implementations.

use super::image::{HioImageSharedPtr, SourceColorSpace};
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

/// Type alias for image factory function
pub type ImageFactory = fn() -> Option<HioImageSharedPtr>;

/// Singleton registry for image format handlers
pub struct HioImageRegistry {
    factories: RwLock<HashMap<String, ImageFactory>>,
}

impl HioImageRegistry {
    /// Get the global registry instance
    pub fn instance() -> &'static HioImageRegistry {
        static INSTANCE: std::sync::OnceLock<HioImageRegistry> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| HioImageRegistry {
            factories: RwLock::new(HashMap::new()),
        })
    }

    /// Register an image factory for a file extension
    pub fn register(&self, extension: &str, factory: ImageFactory) {
        let ext = extension.to_lowercase();
        let mut factories = self.factories.write().expect("lock poisoned");
        factories.insert(ext, factory);
    }

    /// Check if a file extension is supported
    pub fn is_supported_extension(&self, extension: &str) -> bool {
        let ext = extension.to_lowercase();
        let factories = self.factories.read().expect("lock poisoned");
        factories.contains_key(&ext)
    }

    /// Check if a filename is supported
    pub fn is_supported_image_file(&self, filename: &str) -> bool {
        if let Some(ext) = Path::new(filename).extension() {
            if let Some(ext_str) = ext.to_str() {
                return self.is_supported_extension(ext_str);
            }
        }
        false
    }

    /// Construct an image handler for the given filename
    pub fn construct_image(&self, filename: &str) -> Option<HioImageSharedPtr> {
        let path = Path::new(filename);
        let ext = path.extension()?.to_str()?.to_lowercase();

        let factories = self.factories.read().expect("lock poisoned");
        let factory = factories.get(&ext)?;

        factory()
    }

    /// Open an image file for reading.
    ///
    /// Dispatches to the concrete StdImage or ExrImage reader.
    /// Mirrors `HioImage::OpenForReading()`.
    pub fn open_for_reading(
        &self,
        filename: &str,
        _subimage: i32,
        _mip: i32,
        source_color_space: SourceColorSpace,
        suppress_errors: bool,
    ) -> Option<HioImageSharedPtr> {
        use crate::image_reader::open_image_shared;

        // Verify the extension is registered before attempting open
        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if !self.is_supported_extension(&ext) {
            if !suppress_errors {
                log::warn!(
                    "HioImageRegistry::open_for_reading: unsupported extension \"{}\" for \"{}\"",
                    ext,
                    filename
                );
            }
            return None;
        }

        // Delegate to real reader (StdImage / ExrImage)
        match open_image_shared(filename, source_color_space) {
            Some(img) => Some(img),
            None => {
                if !suppress_errors {
                    log::warn!(
                        "HioImageRegistry::open_for_reading: failed to open \"{}\"",
                        filename
                    );
                }
                None
            }
        }
    }

    /// Open an image file for writing.
    ///
    /// Returns a write-capable handle.  For EXR the file must not necessarily
    /// exist yet; for LDR formats we use `StdImage::for_writing`.  Mirrors
    /// `HioImage::OpenForWriting()`.
    pub fn open_for_writing(&self, filename: &str) -> Option<HioImageSharedPtr> {
        use crate::image_reader::StdImage;
        use std::sync::Arc;

        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if !self.is_supported_extension(&ext) {
            log::warn!(
                "HioImageRegistry::open_for_writing: unsupported extension \"{}\" for \"{}\"",
                ext,
                filename
            );
            return None;
        }

        // StdImage::for_writing creates a write-only handle without pre-loading.
        // ExrImage write path also uses for_writing style (the write() method
        // does not require a prior read, so StdImage::for_writing covers EXR too
        // in this registry-level abstraction).
        Some(Arc::new(StdImage::for_writing(filename)) as HioImageSharedPtr)
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

/// Return whether `filename` has a registered image extension.
///
/// Convenience wrapper around `HioImageRegistry::is_supported_image_file()`.
/// Mirrors the C++ static `HioImage::IsSupportedImageFile()`.
pub fn is_supported_image_file(filename: &str) -> bool {
    HioImageRegistry::instance().is_supported_image_file(filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_singleton() {
        let registry1 = HioImageRegistry::instance();
        let registry2 = HioImageRegistry::instance();

        // Both should point to the same instance
        assert!(std::ptr::eq(registry1, registry2));
    }

    #[test]
    fn test_register_and_check_extension() {
        let registry = HioImageRegistry::instance();
        // Don't clear - use unique extension to avoid race conditions

        // Mock factory function
        fn mock_factory() -> Option<HioImageSharedPtr> {
            None
        }

        registry.register("testext", mock_factory);

        assert!(registry.is_supported_extension("testext"));
        assert!(registry.is_supported_extension("TESTEXT")); // Case insensitive
        assert!(!registry.is_supported_extension("unknown"));
    }

    #[test]
    fn test_is_supported_image_file() {
        let registry = HioImageRegistry::instance();
        // Don't clear - use unique extension to avoid race conditions

        fn mock_factory() -> Option<HioImageSharedPtr> {
            None
        }

        registry.register("testpng", mock_factory);

        assert!(registry.is_supported_image_file("test.testpng"));
        assert!(registry.is_supported_image_file("test.TESTPNG"));
        assert!(registry.is_supported_image_file("/path/to/image.testpng"));
        assert!(!registry.is_supported_image_file("test.jpg"));
    }

    #[test]
    fn test_registered_extensions() {
        let registry = HioImageRegistry::instance();

        fn mock_factory() -> Option<HioImageSharedPtr> {
            None
        }

        registry.register("test_png", mock_factory);
        registry.register("test_jpg", mock_factory);
        registry.register("test_exr", mock_factory);

        let extensions = registry.registered_extensions();
        // Check that our test extensions are registered (other tests may add more)
        assert!(extensions.len() >= 3);
        assert!(extensions.contains(&"test_png".to_string()));
        assert!(extensions.contains(&"test_jpg".to_string()));
        assert!(extensions.contains(&"test_exr".to_string()));
    }
}
