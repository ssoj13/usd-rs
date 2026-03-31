
//! Hydra Image I/O (HIO) - Base abstractions for texture image reading and writing.
//!
//! This module provides the foundational types and traits for reading and writing
//! texture image data in the Hydra rendering system. HIO uses a plugin-based
//! architecture where different image formats are handled by format-specific
//! implementations.
//!
//! # Overview
//!
//! The main components are:
//! - [`HioImage`] - Core trait for image I/O operations
//! - [`StorageSpec`] - Memory layout specification for texture data
//! - [`ImageOriginLocation`] - Image coordinate system origin
//! - [`SourceColorSpace`] - Color space encoding specification
//!
//! # Examples
//!
//! ```ignore
//! use usd_hio::{HioImage, StorageSpec, HioFormat};
//!
//! fn load_texture(image: &mut dyn HioImage) -> Option<StorageSpec> {
//!     let mut storage = StorageSpec::new(
//!         image.width(),
//!         image.height(),
//!         1,
//!         image.format()
//!     );
//!     
//!     if image.read(&mut storage) {
//!         Some(storage)
//!     } else {
//!         None
//!     }
//! }
//! ```

use super::types::{HioAddressDimension, HioAddressMode, HioFormat};
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Dictionary;

/// Specifies whether to treat the image origin as the upper-left corner
/// or the lower left.
///
/// Different image formats and APIs use different coordinate systems:
/// - OpenGL and USD typically use lower-left origin
/// - DirectX and most image file formats use upper-left origin
///
/// # Examples
///
/// ```
/// # use usd_hio::ImageOriginLocation;
/// let origin = ImageOriginLocation::LowerLeft;
/// assert_eq!(origin, ImageOriginLocation::LowerLeft);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageOriginLocation {
    /// Origin at upper-left corner (DirectX, most file formats)
    UpperLeft,
    /// Origin at lower-left corner (OpenGL, USD)
    LowerLeft,
}

/// Specifies the source color space in which the texture is encoded.
///
/// Color space information is critical for correct rendering. The `Auto` variant
/// allows the texture reader to determine the color space based on hints from
/// the image file (e.g., file type, number of channels, embedded metadata).
///
/// # Variants
///
/// - `Raw` - Linear color space, no gamma correction
/// - `SRGB` - sRGB color space with gamma correction
/// - `Auto` - Automatically detect from image metadata
///
/// # Examples
///
/// ```
/// # use usd_hio::SourceColorSpace;
/// let color_space = SourceColorSpace::Auto;
/// match color_space {
///     SourceColorSpace::SRGB => println!("sRGB texture"),
///     SourceColorSpace::Raw => println!("Linear texture"),
///     SourceColorSpace::Auto => println!("Auto-detect color space"),
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceColorSpace {
    /// Raw linear color space, no gamma correction applied
    Raw,
    /// sRGB color space with standard 2.2 gamma curve
    SRGB,
    /// Automatically determine color space from image hints
    Auto,
}

/// Describes the memory layout and storage of a texture image.
///
/// This structure specifies how texture data is organized in memory, including
/// dimensions, pixel format, orientation, and a pointer to the actual pixel data.
///
/// # Memory Management
///
/// The `data` field is a raw pointer that requires manual memory management.
/// Users are responsible for:
/// - Allocating sufficient memory for `width * height * depth * bytes_per_pixel`
/// - Ensuring proper alignment for the pixel format
/// - Freeing the memory when no longer needed
///
/// # Safety
///
/// The `data` pointer must be valid for the lifetime of the `StorageSpec` and
/// point to a buffer large enough to hold the image data. Incorrect usage can
/// lead to undefined behavior.
///
/// # Fields
///
/// - `width` - Image width in pixels
/// - `height` - Image height in pixels  
/// - `depth` - Image depth (1 for 2D textures, >1 for 3D textures)
/// - `format` - Pixel format specification
/// - `flipped` - Whether the image is vertically flipped
/// - `data` - Raw pointer to pixel data buffer
///
/// # Examples
///
/// ```
/// # use usd_hio::{StorageSpec, HioFormat};
/// // Create empty storage spec
/// let spec = StorageSpec::new(1024, 768, 1, HioFormat::UNorm8Vec4);
/// assert_eq!(spec.width, 1024);
/// assert_eq!(spec.height, 768);
/// ```
#[derive(Debug)]
pub struct StorageSpec {
    /// Image width in pixels
    pub width: i32,
    /// Image height in pixels
    pub height: i32,
    /// Image depth (1 for 2D, >1 for 3D textures)
    pub depth: i32,
    /// Pixel format specification
    pub format: HioFormat,
    /// Whether the image is vertically flipped
    pub flipped: bool,
    /// Raw pointer to pixel data buffer (requires manual memory management)
    pub data: *mut u8,
}

impl Default for StorageSpec {
    /// Creates a default `StorageSpec` with zero dimensions and null data pointer.
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            depth: 0,
            format: HioFormat::Invalid,
            flipped: false,
            data: std::ptr::null_mut(),
        }
    }
}

impl StorageSpec {
    /// Creates a new `StorageSpec` with the given dimensions and format.
    ///
    /// The data pointer is initialized to null and must be set separately.
    ///
    /// # Arguments
    ///
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `depth` - Image depth (use 1 for 2D textures)
    /// * `format` - Pixel format specification
    ///
    /// # Examples
    ///
    /// ```
    /// # use usd_hio::{StorageSpec, HioFormat};
    /// let spec = StorageSpec::new(512, 512, 1, HioFormat::Float32Vec4);
    /// assert_eq!(spec.width, 512);
    /// assert!(spec.data.is_null());
    /// ```
    pub fn new(width: i32, height: i32, depth: i32, format: HioFormat) -> Self {
        Self {
            width,
            height,
            depth,
            format,
            flipped: false,
            data: std::ptr::null_mut(),
        }
    }

    /// Creates a `StorageSpec` with an allocated data buffer.
    ///
    /// # Arguments
    ///
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `depth` - Image depth (use 1 for 2D textures)
    /// * `format` - Pixel format specification
    /// * `data` - Pointer to allocated pixel data buffer
    ///
    /// # Safety
    ///
    /// The caller must ensure that `data` points to a valid buffer with sufficient
    /// capacity for `width * height * depth * bytes_per_pixel` bytes.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_hio::{StorageSpec, HioFormat};
    /// let buffer = vec![0u8; 512 * 512 * 4].into_boxed_slice();
    /// let data_ptr = Box::into_raw(buffer) as *mut u8;
    /// let spec = StorageSpec::with_data(512, 512, 1, HioFormat::UNorm8Vec4, data_ptr);
    /// ```
    pub fn with_data(
        width: i32,
        height: i32,
        depth: i32,
        format: HioFormat,
        data: *mut u8,
    ) -> Self {
        Self {
            width,
            height,
            depth,
            format,
            flipped: false,
            data,
        }
    }
}

// SAFETY: StorageSpec raw pointer requires manual memory management.
// Send/Sync is safe as long as the pointed-to buffer is exclusively owned.
// Caller ensures proper synchronization when sharing across threads.
#[allow(unsafe_code)]
unsafe impl Send for StorageSpec {}
#[allow(unsafe_code)]
unsafe impl Sync for StorageSpec {}

/// A base trait for reading and writing texture image data.
///
/// `HioImage` provides a plugin-based abstraction for texture I/O operations
/// in the Hydra rendering system. Different image formats (PNG, JPG, EXR, etc.)
/// are handled by format-specific implementations of this trait.
///
/// # Image File Resolution
///
/// Texture paths are UTF-8 strings that are resolvable through the Asset
/// Resolution (AR) system. The texture system dispatches to the appropriate
/// plugin based on file extension, with ASCII letters [A-Z] case-folded
/// (other characters are case-sensitive).
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent texture loading
/// in multi-threaded rendering contexts.
///
/// # Examples
///
/// ```ignore
/// use usd_hio::{HioImage, StorageSpec};
///
/// fn read_image_metadata(image: &dyn HioImage) {
///     println!("Filename: {}", image.filename());
///     println!("Dimensions: {}x{}", image.width(), image.height());
///     println!("Format: {:?}", image.format());
///     println!("Mip levels: {}", image.num_mip_levels());
///     println!("sRGB: {}", image.is_color_space_srgb());
/// }
/// ```
pub trait HioImage: Send + Sync {
    /// Returns the image filename.
    ///
    /// This is typically the path used to open the image, which may be
    /// an Asset Resolution (AR) resolvable path.
    fn filename(&self) -> &str;

    /// Returns the image width in pixels.
    ///
    /// For images with multiple mip levels, this returns the width of the
    /// base (level 0) image.
    fn width(&self) -> i32;

    /// Returns the image height in pixels.
    ///
    /// For images with multiple mip levels, this returns the height of the
    /// base (level 0) image.
    fn height(&self) -> i32;

    /// Returns the destination pixel format.
    ///
    /// This is the format that will be used when reading image data into
    /// a `StorageSpec`. The format may differ from the source file format
    /// if conversion is performed during loading.
    fn format(&self) -> HioFormat;

    /// Returns the number of bytes per pixel for the current format.
    ///
    /// This value is calculated based on the pixel format returned by
    /// `format()` and is useful for memory allocation calculations.
    fn bytes_per_pixel(&self) -> i32;

    /// Returns the number of mipmap levels available in the image.
    ///
    /// Returns 1 for images without mipmaps. Mipmap levels are numbered
    /// from 0 (base level) to `num_mip_levels() - 1`.
    fn num_mip_levels(&self) -> i32;

    /// Returns whether the image uses sRGB color space.
    ///
    /// This information is important for correct color rendering. sRGB
    /// textures require gamma correction when used in linear rendering
    /// pipelines.
    ///
    /// # Returns
    ///
    /// `true` if the image is in sRGB color space, `false` for linear color space.
    fn is_color_space_srgb(&self) -> bool;

    /// Reads the complete image file into the provided storage.
    ///
    /// The storage must be pre-allocated with dimensions and format matching
    /// the image. The `data` pointer in `storage` must point to a buffer
    /// large enough to hold the image data.
    ///
    /// # Arguments
    ///
    /// * `storage` - Storage specification with pre-allocated data buffer
    ///
    /// # Returns
    ///
    /// `true` if the read operation succeeded, `false` on error.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_hio::{HioImage, StorageSpec};
    /// fn load_full_image(image: &mut dyn HioImage) -> bool {
    ///     let mut storage = StorageSpec::new(
    ///         image.width(),
    ///         image.height(),
    ///         1,
    ///         image.format()
    ///     );
    ///     // Allocate buffer and set storage.data...
    ///     image.read(&mut storage)
    /// }
    /// ```
    fn read(&mut self, storage: &mut StorageSpec) -> bool;

    /// Reads a cropped sub-region of the image into storage.
    ///
    /// This method allows reading only a portion of the image, which can be
    /// more memory-efficient than reading the full image and then cropping.
    ///
    /// # Arguments
    ///
    /// * `crop_top` - Top boundary of crop region (pixels from top)
    /// * `crop_bottom` - Bottom boundary of crop region (pixels from top)
    /// * `crop_left` - Left boundary of crop region (pixels from left)
    /// * `crop_right` - Right boundary of crop region (pixels from left)
    /// * `storage` - Storage specification for the cropped data
    ///
    /// # Returns
    ///
    /// `true` if the read operation succeeded, `false` on error.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_hio::{HioImage, StorageSpec};
    /// fn load_thumbnail(image: &mut dyn HioImage) -> bool {
    ///     let mut storage = StorageSpec::new(128, 128, 1, image.format());
    ///     // Read 128x128 region from center
    ///     let w = image.width();
    ///     let h = image.height();
    ///     let top = (h - 128) / 2;
    ///     let left = (w - 128) / 2;
    ///     image.read_cropped(top, top + 128, left, left + 128, &mut storage)
    /// }
    /// ```
    fn read_cropped(
        &mut self,
        crop_top: i32,
        crop_bottom: i32,
        crop_left: i32,
        crop_right: i32,
        storage: &mut StorageSpec,
    ) -> bool;

    /// Writes image data to file with optional metadata.
    ///
    /// This method writes the image data from `storage` to the file specified
    /// by `filename()`. The file format is determined by the file extension.
    ///
    /// # Arguments
    ///
    /// * `storage` - Storage specification containing image data to write
    /// * `metadata` - Optional metadata dictionary to embed in the image file
    ///
    /// # Returns
    ///
    /// `true` if the write operation succeeded, `false` on error.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_hio::{HioImage, StorageSpec};
    /// # use usd_vt::Dictionary;
    /// fn save_image(image: &mut dyn HioImage, storage: &StorageSpec) -> bool {
    ///     let mut metadata = Dictionary::new();
    ///     // Add metadata...
    ///     image.write(storage, Some(&metadata))
    /// }
    /// ```
    fn write(&mut self, storage: &StorageSpec, metadata: Option<&Dictionary>) -> bool;

    /// Retrieves metadata value for the given key.
    ///
    /// Image files can contain embedded metadata (EXIF data, color profiles,
    /// creation info, etc.). This method provides access to that metadata.
    ///
    /// # Arguments
    ///
    /// * `key` - Token identifying the metadata key to retrieve
    ///
    /// # Returns
    ///
    /// `Some(value)` if the metadata key exists, `None` otherwise. The value
    /// is returned as a type-erased `Box<dyn Any>` and must be downcast to
    /// the appropriate type.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_hio::HioImage;
    /// # use usd_tf::Token;
    /// fn get_creation_time(image: &dyn HioImage) -> Option<String> {
    ///     let key = Token::new("creation_time");
    ///     image.get_metadata(&key)
    ///         .and_then(|v| v.downcast_ref::<String>().cloned())
    /// }
    /// ```
    fn get_metadata(&self, key: &Token) -> Option<Box<dyn std::any::Any>>;

    /// Retrieves sampler metadata for the given addressing dimension.
    ///
    /// This method returns texture sampler parameters (wrap mode, filtering, etc.)
    /// that may be embedded in the image file or inferred from the image format.
    ///
    /// # Arguments
    ///
    /// * `dim` - The addressing dimension (U, V, or W for 3D textures)
    ///
    /// # Returns
    ///
    /// `Some(mode)` if sampler metadata is available for the dimension,
    /// `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use usd_hio::{HioImage, HioAddressDimension};
    /// fn get_wrap_mode(image: &dyn HioImage) -> String {
    ///     match image.get_sampler_metadata(HioAddressDimension::U) {
    ///         Some(mode) => format!("{:?}", mode),
    ///         None => "default".to_string(),
    ///     }
    /// }
    /// ```
    fn get_sampler_metadata(&self, dim: HioAddressDimension) -> Option<HioAddressMode>;
}

/// Type alias for a thread-safe shared pointer to an `HioImage` implementation.
///
/// This type is used throughout the HIO system for managing image objects
/// that may be shared across multiple threads or components.
///
/// # Examples
///
/// ```ignore
/// # use usd_hio::{HioImageSharedPtr, HioImage};
/// # use std::sync::Arc;
/// fn share_image(image: HioImageSharedPtr) -> HioImageSharedPtr {
///     Arc::clone(&image)
/// }
/// ```
pub type HioImageSharedPtr = Arc<dyn HioImage>;

/// Base implementation for `HioImage` that provides common functionality.
///
/// `HioImageBase` is a concrete implementation that stores basic image properties
/// and provides default behavior. Format-specific image loaders can use this as
/// a foundation and extend it with actual I/O operations.
///
/// # Fields
///
/// - `filename` - Path to the image file
/// - `width` - Image width in pixels
/// - `height` - Image height in pixels
/// - `format` - Pixel format specification
/// - `num_mip_levels` - Number of mipmap levels
/// - `is_srgb` - Whether the image uses sRGB color space
///
/// # Examples
///
/// ```
/// # use usd_hio::{HioImageBase, HioFormat};
/// let mut base = HioImageBase::new();
/// base.set_filename("texture.png".to_string());
/// base.set_dimensions(1024, 1024);
/// base.set_format(HioFormat::UNorm8Vec4);
/// base.set_num_mip_levels(11);
/// base.set_is_srgb(true);
///
/// assert_eq!(base.filename(), "texture.png");
/// assert_eq!(base.width(), 1024);
/// assert!(base.is_color_space_srgb());
/// ```
pub struct HioImageBase {
    filename: String,
    width: i32,
    height: i32,
    format: HioFormat,
    num_mip_levels: i32,
    is_srgb: bool,
}

impl HioImageBase {
    /// Creates a new `HioImageBase` with default values.
    ///
    /// All dimensions are initialized to 0, format to `Invalid`, and sRGB to `false`.
    pub fn new() -> Self {
        Self {
            filename: String::new(),
            width: 0,
            height: 0,
            format: HioFormat::Invalid,
            num_mip_levels: 1,
            is_srgb: false,
        }
    }

    /// Sets the image filename.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the image file
    pub fn set_filename(&mut self, filename: String) {
        self.filename = filename;
    }

    /// Sets the image dimensions.
    ///
    /// # Arguments
    ///
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    pub fn set_dimensions(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
    }

    /// Sets the pixel format.
    ///
    /// # Arguments
    ///
    /// * `format` - Pixel format specification
    pub fn set_format(&mut self, format: HioFormat) {
        self.format = format;
    }

    /// Sets the number of mipmap levels.
    ///
    /// # Arguments
    ///
    /// * `num_levels` - Number of mipmap levels (1 for no mipmaps)
    pub fn set_num_mip_levels(&mut self, num_levels: i32) {
        self.num_mip_levels = num_levels;
    }

    /// Sets the sRGB color space flag.
    ///
    /// # Arguments
    ///
    /// * `is_srgb` - `true` if the image uses sRGB color space
    pub fn set_is_srgb(&mut self, is_srgb: bool) {
        self.is_srgb = is_srgb;
    }

    /// Returns the image filename.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Returns the image width in pixels.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Returns the image height in pixels.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Returns the pixel format.
    pub fn format(&self) -> HioFormat {
        self.format
    }

    /// Returns the number of mipmap levels.
    pub fn num_mip_levels(&self) -> i32 {
        self.num_mip_levels
    }

    /// Returns whether the image uses sRGB color space.
    pub fn is_color_space_srgb(&self) -> bool {
        self.is_srgb
    }

    /// Calculates and returns the number of bytes per pixel.
    ///
    /// This value is computed from the current pixel format using the
    /// format size lookup table.
    pub fn bytes_per_pixel(&self) -> i32 {
        use super::types::get_data_size_of_format;
        let (size, _) = get_data_size_of_format(self.format);
        size as i32
    }
}

impl Default for HioImageBase {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_spec_default() {
        let spec = StorageSpec::default();
        assert_eq!(spec.width, 0);
        assert_eq!(spec.height, 0);
        assert_eq!(spec.depth, 0);
        assert_eq!(spec.format, HioFormat::Invalid);
        assert!(!spec.flipped);
        assert!(spec.data.is_null());
    }

    #[test]
    fn test_storage_spec_new() {
        let spec = StorageSpec::new(1024, 768, 1, HioFormat::UNorm8Vec4);
        assert_eq!(spec.width, 1024);
        assert_eq!(spec.height, 768);
        assert_eq!(spec.depth, 1);
        assert_eq!(spec.format, HioFormat::UNorm8Vec4);
    }

    #[test]
    fn test_image_base() {
        let mut base = HioImageBase::new();
        base.set_filename("test.png".to_string());
        base.set_dimensions(256, 256);
        base.set_format(HioFormat::Float32Vec4);
        base.set_num_mip_levels(8);
        base.set_is_srgb(false);

        assert_eq!(base.filename(), "test.png");
        assert_eq!(base.width(), 256);
        assert_eq!(base.height(), 256);
        assert_eq!(base.format(), HioFormat::Float32Vec4);
        assert_eq!(base.num_mip_levels(), 8);
        assert!(!base.is_color_space_srgb());
        assert_eq!(base.bytes_per_pixel(), 16);
    }

    #[test]
    fn test_origin_location() {
        assert_ne!(
            ImageOriginLocation::UpperLeft,
            ImageOriginLocation::LowerLeft
        );
    }

    #[test]
    fn test_source_color_space() {
        let auto = SourceColorSpace::Auto;
        let srgb = SourceColorSpace::SRGB;
        let raw = SourceColorSpace::Raw;

        assert_ne!(auto, srgb);
        assert_ne!(auto, raw);
        assert_ne!(srgb, raw);
    }
}
