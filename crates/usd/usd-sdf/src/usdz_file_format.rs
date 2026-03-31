//! USD ZIP package file format (.usdz) implementation.
//!
//! This module provides the file format handler for USD package files.
//! The `.usdz` format is a ZIP archive containing a USD layer and its
//! dependencies (textures, other layers, etc.) in a single file.
//!
//! # File Structure
//!
//! A `.usdz` file is a standard ZIP archive with specific requirements:
//!
//! - Uncompressed storage (for memory mapping)
//! - 64-byte aligned file entries (for GPU texture access)
//! - Root layer must be first entry
//! - Only certain file types allowed (usd, usda, usdc, png, jpg, etc.)
//!
//! # String Operations
//!
//! `ReadFromString`, `WriteToString`, and `WriteToStream` delegate to the
//! usda format, as the package format itself cannot be meaningfully
//! represented as a string.
//!
//! # Writing
//!
//! Direct writing via `WriteToFile` is not supported. Use the UsdUtils
//! CreateNewUsdzPackage API instead.

use std::path::Path as StdPath;
use std::sync::Arc;

use usd_ar::{Asset, InMemoryAsset, PackageResolver, ResolvedPath};
use usd_tf::Token;
use usd_vt::Value;

use super::Layer;
use super::file_format::{
    FileFormat, FileFormatArguments, FileFormatError, find_format_by_extension, find_format_by_id,
    register_file_format,
};
use super::usda_reader;
use super::zip_file::ZipFile;

// ============================================================================
// Tokens
// ============================================================================

/// Tokens for usdz file format.
pub mod tokens {
    use std::sync::OnceLock;
    use usd_tf::Token;

    /// Format ID token
    pub fn id() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("usdz")).clone()
    }

    /// Version token
    pub fn version() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("1.0")).clone()
    }

    /// Target token
    pub fn target() -> Token {
        static TOKEN: OnceLock<Token> = OnceLock::new();
        TOKEN.get_or_init(|| Token::new("usd")).clone()
    }
}

// ============================================================================
// Constants
// ============================================================================

/// ZIP local file header signature
pub const ZIP_LOCAL_HEADER_SIG: &[u8; 4] = b"PK\x03\x04";

/// Required alignment for usdz entries (64 bytes)
pub const USDZ_ALIGNMENT: usize = 64;

// ============================================================================
// UsdzResolverCache
// ============================================================================

/// Cache for opened usdz files.
///
/// Ensures we only open a usdz file once when reading its contents.
mod resolver_cache {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::{Mutex, OnceLock};

    /// Max cached USDZ files before LRU eviction kicks in.
    const MAX_CACHE_ENTRIES: usize = 64;

    struct Cache {
        files: HashMap<String, ZipFile>,
        /// LRU order: front = oldest, back = most recent
        order: VecDeque<String>,
    }

    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

    fn get_cache() -> &'static Mutex<Cache> {
        CACHE.get_or_init(|| {
            Mutex::new(Cache {
                files: HashMap::new(),
                order: VecDeque::new(),
            })
        })
    }

    /// Finds or opens a ZIP file from the cache.
    /// Uses LRU eviction when cache exceeds MAX_CACHE_ENTRIES (P1-5).
    pub fn find_or_open_zip_file(path: &str) -> Result<ZipFile, FileFormatError> {
        let mut cache = get_cache().lock().expect("lock poisoned");

        if let Some(zip) = cache.files.get(path).cloned() {
            // Move to back of LRU (most recently used)
            cache.order.retain(|k| k != path);
            cache.order.push_back(path.to_string());
            return Ok(zip);
        }

        // Load the file
        let zip =
            ZipFile::open(path).map_err(|e| FileFormatError::io_error(path, e.to_string()))?;

        // Evict oldest entries if at capacity
        while cache.files.len() >= MAX_CACHE_ENTRIES {
            if let Some(oldest) = cache.order.pop_front() {
                cache.files.remove(&oldest);
            } else {
                break;
            }
        }

        // Cache it
        cache.files.insert(path.to_string(), zip.clone());
        cache.order.push_back(path.to_string());

        Ok(zip)
    }

    /// Gets raw data for a file inside the ZIP archive.
    pub fn get_file_data(package_path: &str, inner_path: &str) -> Result<Vec<u8>, FileFormatError> {
        let zip = find_or_open_zip_file(package_path)?;

        zip.get_file_data(inner_path)
            .map(|data| data.to_vec())
            .ok_or_else(|| {
                FileFormatError::corrupt_file(
                    package_path,
                    format!("File not found in archive: {}", inner_path),
                )
            })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Gets the first file in a ZIP archive.
fn get_first_file_in_zip_file(path: &str) -> Result<String, FileFormatError> {
    let zip = resolver_cache::find_or_open_zip_file(path)?;

    zip.first_file()
        .map(|s| s.to_string())
        .ok_or_else(|| FileFormatError::corrupt_file(path, "Empty ZIP archive"))
}

/// Joins a package path with an inner path.
///
/// Corresponds to ArJoinPackageRelativePath.
fn join_package_relative_path(package_path: &str, inner_path: &str) -> String {
    format!("{}[{}]", package_path, inner_path)
}

/// Checks if an extension is a valid USD format.
fn is_usd_extension(ext: &str) -> bool {
    matches!(ext.to_lowercase().as_str(), "usd" | "usda" | "usdc")
}

// ============================================================================
// UsdzFileFormat
// ============================================================================

/// File format handler for USD package files (.usdz).
///
/// This format handles ZIP archives containing USD layers and assets.
/// It provides a self-contained package format for distribution.
///
/// # Writing
///
/// Direct writing via `write_to_file` is not supported. Use the UsdUtils
/// CreateNewUsdzPackage API instead.
///
/// # String Operations
///
/// String-based operations delegate to the usda format.
#[derive(Debug, Clone)]
pub struct UsdzFileFormat {
    /// Format identifier
    format_id: Token,
    /// Version string
    version_string: Token,
    /// Target schema
    target: Token,
    /// File extensions
    extensions: Vec<String>,
}

impl Default for UsdzFileFormat {
    fn default() -> Self {
        Self::new()
    }
}

impl UsdzFileFormat {
    /// Creates a new usdz file format handler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format_id: tokens::id(),
            version_string: tokens::version(),
            target: tokens::target(),
            extensions: vec!["usdz".to_string()],
        }
    }

    /// Internal read helper.
    fn read_helper(
        &self,
        layer: &mut Layer,
        resolved_path: &str,
        metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        // Get the first file in the package
        let first_file = get_first_file_in_zip_file(resolved_path)?;

        if first_file.is_empty() {
            return Err(FileFormatError::corrupt_file(
                resolved_path,
                "Empty ZIP archive",
            ));
        }

        // Check if it's a USD file
        let ext = StdPath::new(&first_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if !is_usd_extension(ext) {
            return Err(FileFormatError::unsupported_format(
                &first_file,
                format!(
                    "First file in usdz package is not a USD file: {}",
                    first_file
                ),
            ));
        }

        // Get the file format for the packaged file (pass extension, not full filename)
        let packaged_format = find_format_by_extension(ext, None).ok_or_else(|| {
            FileFormatError::unsupported_format(
                &first_file,
                format!("No format handler for extension '.{}'", ext),
            )
        })?;

        // Get the file data from the archive
        let file_data = resolver_cache::get_file_data(resolved_path, &first_file)?;

        // Read using the appropriate format
        // For usda, we can read from string
        // For usdc, we need to read from bytes
        if packaged_format.format_id() == usda_reader::tokens::id() {
            // Parse as USDA text
            let content = String::from_utf8(file_data)
                .map_err(|e| FileFormatError::corrupt_file(resolved_path, e.to_string()))?;

            // Use UsdaFileFormat's read_from_string
            let usda_format = usda_reader::UsdaFileFormat::new();
            usda_format.read_from_string(layer, &content)?;
        } else if packaged_format.format_id() == super::usdc_reader::tokens::id() {
            // Parse as USDC binary
            super::usdc_reader::read_from_bytes(layer, &file_data, metadata_only)?;
        } else {
            // Generic USD - try to delegate
            return Err(FileFormatError::unsupported_format(
                &first_file,
                format!(
                    "Unsupported format inside usdz: {}",
                    packaged_format.format_id().get_text()
                ),
            ));
        }

        // Note: Layer identifier is managed by the caller (find_or_open sets it)
        // The package-relative path would be: join_package_relative_path(resolved_path, &first_file)

        Ok(())
    }

    /// Gets the usda format for string operations.
    fn get_usda_format() -> Option<Arc<dyn FileFormat>> {
        find_format_by_id(&usda_reader::tokens::id())
    }

    /// P2-5: Reads a USDZ archive from raw bytes in memory.
    ///
    /// Treats `bytes` as a ZIP archive in memory, finds the first USD layer
    /// inside, and parses it into `layer`. Rarely used in practice but
    /// required for API completeness.
    pub fn read_from_bytes(
        &self,
        layer: &mut Layer,
        bytes: Vec<u8>,
    ) -> Result<(), FileFormatError> {
        let zip = ZipFile::from_bytes(bytes).map_err(|e| {
            FileFormatError::corrupt_file("<in-memory>", format!("Invalid ZIP: {}", e))
        })?;

        let first_file = zip
            .first_file()
            .map(|s| s.to_string())
            .ok_or_else(|| FileFormatError::corrupt_file("<in-memory>", "Empty ZIP archive"))?;

        let ext = StdPath::new(&first_file)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if !is_usd_extension(ext) {
            return Err(FileFormatError::unsupported_format(
                &first_file,
                format!(
                    "First file in usdz package is not a USD file: {}",
                    first_file
                ),
            ));
        }

        let file_data = zip
            .get_file_data(&first_file)
            .ok_or_else(|| {
                FileFormatError::corrupt_file(
                    "<in-memory>",
                    format!("Cannot read '{}' from ZIP", first_file),
                )
            })?
            .to_vec();

        match ext {
            "usda" | "usd" => {
                let text = String::from_utf8(file_data)
                    .map_err(|e| FileFormatError::corrupt_file("<in-memory>", e.to_string()))?;
                let usda_format = usda_reader::UsdaFileFormat::new();
                usda_format.read_from_string(layer, &text)
            }
            "usdc" => super::usdc_reader::read_from_bytes(layer, &file_data, false),
            other => Err(FileFormatError::unsupported_format(
                &first_file,
                format!("Unsupported format inside usdz: .{}", other),
            )),
        }
    }
}

impl FileFormat for UsdzFileFormat {
    fn format_id(&self) -> Token {
        self.format_id.clone()
    }

    fn target(&self) -> Token {
        self.target.clone()
    }

    fn file_extensions(&self) -> Vec<String> {
        self.extensions.clone()
    }

    fn is_package(&self) -> bool {
        true
    }

    fn get_package_root_layer_path(&self, resolved_path: &ResolvedPath) -> String {
        get_first_file_in_zip_file(resolved_path.as_str()).unwrap_or_default()
    }

    fn can_read(&self, path: &str) -> bool {
        // Check if we can read the first file in the package
        match get_first_file_in_zip_file(path) {
            Ok(first_file) => {
                if first_file.is_empty() {
                    return false;
                }

                // Extract extension from filename (C++ FindByExtension does this internally)
                let ext = StdPath::new(&first_file)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");

                // Check if we have a format for the packaged file
                if let Some(packaged_format) = find_format_by_extension(ext, None) {
                    let package_relative_path = join_package_relative_path(path, &first_file);
                    packaged_format.can_read(&package_relative_path)
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }

    fn read(
        &self,
        layer: &mut Layer,
        resolved_path: &ResolvedPath,
        metadata_only: bool,
    ) -> Result<(), FileFormatError> {
        self.read_helper(layer, resolved_path.as_str(), metadata_only)
    }

    fn write_to_file(
        &self,
        _layer: &Layer,
        _file_path: &str,
        _comment: Option<&str>,
        _args: &FileFormatArguments,
    ) -> Result<(), FileFormatError> {
        // Writing usdz layers is not allowed via this API
        // Use UsdUtils::CreateNewUsdzPackage instead
        Err(FileFormatError::other(
            "Writing usdz layers is not allowed via this API. Use UsdUtils::CreateNewUsdzPackage instead",
        ))
    }

    /// Writes to string by delegating to usda format.
    fn write_to_string(
        &self,
        layer: &Layer,
        comment: Option<&str>,
    ) -> Result<String, FileFormatError> {
        if let Some(usda) = Self::get_usda_format() {
            usda.write_to_string(layer, comment)
        } else {
            Err(FileFormatError::other("usda format not available"))
        }
    }

    fn get_file_cookie(&self) -> String {
        String::from_utf8_lossy(ZIP_LOCAL_HEADER_SIG).to_string()
    }

    fn get_version_string(&self) -> Token {
        self.version_string.clone()
    }

    fn supports_reading(&self) -> bool {
        true
    }

    fn supports_writing(&self) -> bool {
        false // Writing via FileFormat API is not supported
    }

    fn supports_editing(&self) -> bool {
        false
    }
}

// ============================================================================
// UsdzPackageResolver — ArPackageResolver implementation for .usdz
// ============================================================================

/// Package resolver for USDZ archives.
///
/// Implements `ArPackageResolver` for `.usdz`/`.zip` files. Given a resolved
/// package path (absolute path to the .usdz file) and a packaged path (path
/// to an asset inside the archive), this resolver locates and reads the asset.
///
/// Registered globally at startup via `register_usdz_format()`.
pub struct UsdzPackageResolver;

impl PackageResolver for UsdzPackageResolver {
    /// Resolve a packaged path within the USDZ archive.
    ///
    /// Returns a non-empty string if the file exists inside the archive.
    /// The returned string is the packaged path itself (existence check only).
    fn resolve(&self, resolved_package_path: &str, packaged_path: &str) -> String {
        let zip = match ZipFile::open(resolved_package_path) {
            Ok(z) => z,
            Err(_) => return String::new(),
        };

        if zip.find(packaged_path).is_some() {
            packaged_path.to_string()
        } else {
            String::new()
        }
    }

    /// Open an asset from within the USDZ archive.
    ///
    /// `resolved_package_path` is the absolute path to the .usdz file.
    /// `resolved_packaged_path` is the inner path inside the archive.
    /// Rejects compressed or encrypted files per USDZ spec (matches C++ behavior).
    fn open_asset(
        &self,
        resolved_package_path: &str,
        resolved_packaged_path: &str,
    ) -> Option<Arc<dyn Asset>> {
        let zip = ZipFile::open(resolved_package_path).ok()?;

        // Check file info for compression/encryption (per C++ usdzResolver.cpp)
        let info = zip.find(resolved_packaged_path)?;
        if info.compression_method != 0 {
            eprintln!(
                "Cannot open {} in {}: compressed files are not supported",
                resolved_packaged_path, resolved_package_path
            );
            return None;
        }
        if info.encrypted {
            eprintln!(
                "Cannot open {} in {}: encrypted files are not supported",
                resolved_packaged_path, resolved_package_path
            );
            return None;
        }

        let data = zip.get_file_data(resolved_packaged_path)?.to_vec();
        Some(Arc::new(InMemoryAsset::from_vec(data)) as Arc<dyn Asset>)
    }

    fn begin_cache_scope(&self, _cache_scope_data: &mut Value) {}
    fn end_cache_scope(&self, _cache_scope_data: &mut Value) {}
}

// ============================================================================
// Registration
// ============================================================================

/// Registers the usdz file format and USDZ package resolver globally.
pub fn register_usdz_format() {
    register_file_format(Arc::new(UsdzFileFormat::new()));

    // Register the USDZ package resolver so that asset paths like
    // `archive.usdz[texture.png]` can be resolved and opened via usd-ar.
    usd_ar::register_package_resolver("usdz", || Box::new(UsdzPackageResolver));
    // Also register for plain "zip" in case someone uses it
    usd_ar::register_package_resolver("zip", || Box::new(UsdzPackageResolver));
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usdz_format_id() {
        let format = UsdzFileFormat::new();
        assert_eq!(format.format_id(), Token::new("usdz"));
        assert_eq!(format.target(), Token::new("usd"));
    }

    #[test]
    fn test_usdz_extensions() {
        let format = UsdzFileFormat::new();
        assert_eq!(format.file_extensions(), vec!["usdz".to_string()]);
    }

    #[test]
    fn test_usdz_is_package() {
        let format = UsdzFileFormat::new();
        assert!(format.is_package());
    }

    #[test]
    fn test_usdz_capabilities() {
        let format = UsdzFileFormat::new();
        assert!(format.supports_reading());
        assert!(!format.supports_writing()); // Writing not allowed via this API
        assert!(!format.supports_editing());
    }

    #[test]
    fn test_write_to_file_fails() {
        let format = UsdzFileFormat::new();
        let layer = Layer::create_anonymous(Some("test"));
        let result = format.write_to_file(&layer, "test.usdz", None, &FileFormatArguments::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_version_string() {
        let format = UsdzFileFormat::new();
        assert_eq!(format.get_version_string(), Token::new("1.0"));
    }

    #[test]
    fn test_join_package_relative_path() {
        assert_eq!(
            join_package_relative_path("/path/to/file.usdz", "model.usdc"),
            "/path/to/file.usdz[model.usdc]"
        );
    }

    #[test]
    fn test_is_usd_extension() {
        assert!(is_usd_extension("usd"));
        assert!(is_usd_extension("usda"));
        assert!(is_usd_extension("usdc"));
        assert!(is_usd_extension("USD"));
        assert!(is_usd_extension("USDA"));
        assert!(!is_usd_extension("png"));
        assert!(!is_usd_extension("jpg"));
    }

    #[test]
    fn test_usdz_package_resolver_roundtrip() {
        use super::super::zip_file::ZipFileWriter;
        use usd_ar::open_packaged_asset;

        // Register the USDZ package resolver so usd-ar can open packaged assets
        register_usdz_format();

        // Create a small USDZ with a fake PNG inside
        let temp_path = std::env::temp_dir().join("test_pkg_resolver.usdz");
        let temp_str = temp_path.to_str().unwrap();
        let png_data = b"FAKEPNG\x00\x01\x02\x03";

        {
            let mut writer = ZipFileWriter::create_new(temp_str);
            writer
                .add_file_data("textures/albedo.png", png_data)
                .unwrap();
            writer.save().unwrap();
        }

        // Resolve inner path: archive.usdz[textures/albedo.png]
        let resolver = UsdzPackageResolver;
        let resolved = resolver.resolve(temp_str, "textures/albedo.png");
        assert_eq!(
            resolved, "textures/albedo.png",
            "should find file in archive"
        );

        // Open asset via the global registry
        let packaged = format!("{}[textures/albedo.png]", temp_str);
        let asset = open_packaged_asset(&packaged).expect("should open packaged asset");
        assert_eq!(asset.size(), png_data.len());
        let buf = asset.get_buffer().unwrap();
        assert_eq!(&*buf, png_data);

        std::fs::remove_file(temp_str).ok();
    }

    #[test]
    fn test_usdz_package_resolver_missing_file() {
        use super::super::zip_file::ZipFileWriter;
        use usd_ar::open_packaged_asset;

        register_usdz_format();

        let temp_path = std::env::temp_dir().join("test_pkg_resolver_missing.usdz");
        let temp_str = temp_path.to_str().unwrap();

        {
            let mut writer = ZipFileWriter::create_new(temp_str);
            writer.add_file_data("model.usda", b"#usda 1.0").unwrap();
            writer.save().unwrap();
        }

        // Non-existent file inside archive → should return None
        let packaged = format!("{}[nonexistent.png]", temp_str);
        assert!(open_packaged_asset(&packaged).is_none());

        std::fs::remove_file(temp_str).ok();
    }

    /// P2-5: read_from_bytes parses an in-memory USDZ containing a USDA layer
    #[test]
    fn test_read_from_bytes_usda() {
        use super::super::zip_file::ZipFileWriter;

        // Build a minimal USDZ in a temp file then read its bytes back
        let temp_path = std::env::temp_dir().join("test_read_from_bytes.usdz");
        let temp_str = temp_path.to_str().unwrap();
        let usda_content = b"#usda 1.0\n";

        {
            let mut writer = ZipFileWriter::create_new(temp_str);
            writer.add_file_data("root.usda", usda_content).unwrap();
            writer.save().unwrap();
        }

        let bytes = std::fs::read(temp_str).unwrap();
        std::fs::remove_file(temp_str).ok();

        let format = UsdzFileFormat::new();
        // Create layer without registry (direct, so &mut works)
        let mut layer = Layer::new_internal(
            "anon:p2-5-test".to_string(),
            None,
            Box::new(super::super::data::Data::new()),
            true,
        );
        let result = format.read_from_bytes(&mut layer, bytes);
        // Should succeed: valid ZIP with a USDA first entry
        assert!(result.is_ok(), "read_from_bytes failed: {:?}", result.err());
    }

    /// P2-5: read_from_bytes returns Err for invalid ZIP bytes
    #[test]
    fn test_read_from_bytes_invalid() {
        let format = UsdzFileFormat::new();
        let mut layer = Layer::new_internal(
            "anon:p2-5-invalid".to_string(),
            None,
            Box::new(super::super::data::Data::new()),
            true,
        );
        let result = format.read_from_bytes(&mut layer, b"not a zip".to_vec());
        assert!(result.is_err(), "should fail on garbage input");
    }
}
