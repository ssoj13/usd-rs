#![allow(dead_code)]

//! HdStAssetUvTextureCpuData - CPU-side texture data loaded from asset paths.
//!
//! Implements texture CPU data by reading a UV texture from a file path.
//! Handles format conversion, alpha premultiplication, vertical flip,
//! color space conversion, mipmap generation, and memory budgeting.
//!
//! Port of pxr/imaging/hdSt/assetUvTextureCpuData.h

use super::texture_cpu_data::HdStTextureCpuData;
use super::texture_utils::calc_mip_levels;
use usd_gf::Vec3i;
use usd_hd::enums::HdWrap;
use usd_hgi::{HgiFormat, HgiTextureDesc, HgiTextureType, HgiTextureUsage};
use usd_hio::{HioAddressMode, HioFormat, SourceColorSpace, read_image_data};

/// Image origin location (top-left vs bottom-left).
///
/// Controls whether the image is flipped vertically on load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageOriginLocation {
    /// Image origin is at top-left (typical for most image formats)
    UpperLeft,
    /// Image origin is at bottom-left (OpenGL convention)
    LowerLeft,
}

impl Default for ImageOriginLocation {
    fn default() -> Self {
        Self::UpperLeft
    }
}

/// Source color space of a texture.
///
/// Determines how color space conversion is applied during loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceColorSpaceHint {
    /// sRGB color space (gamma-encoded)
    SRGB,
    /// Linear color space (no gamma)
    Linear,
    /// Auto-detect from file metadata
    Auto,
    /// Raw (no color space conversion)
    Raw,
}

impl Default for SourceColorSpaceHint {
    fn default() -> Self {
        Self::Auto
    }
}

impl From<SourceColorSpaceHint> for SourceColorSpace {
    fn from(hint: SourceColorSpaceHint) -> Self {
        match hint {
            SourceColorSpaceHint::SRGB => SourceColorSpace::SRGB,
            SourceColorSpaceHint::Linear | SourceColorSpaceHint::Raw => SourceColorSpace::Raw,
            SourceColorSpaceHint::Auto => SourceColorSpace::Auto,
        }
    }
}

/// Convert HioFormat to HgiFormat for GPU upload.
///
/// Matches C++ HdStTextureUtils::GetHgiFormat() logic.
fn hio_to_hgi_format(hio: HioFormat, is_srgb: bool) -> HgiFormat {
    match hio {
        HioFormat::UNorm8 => HgiFormat::UNorm8,
        HioFormat::UNorm8Vec2 => HgiFormat::UNorm8Vec2,
        HioFormat::UNorm8Vec3 | HioFormat::UNorm8Vec4 => {
            if is_srgb {
                HgiFormat::UNorm8Vec4srgb
            } else {
                HgiFormat::UNorm8Vec4
            }
        }
        HioFormat::Float16Vec4 => HgiFormat::Float16Vec4,
        HioFormat::Float16Vec3 => HgiFormat::Float16Vec3,
        HioFormat::Float16Vec2 => HgiFormat::Float16Vec2,
        HioFormat::Float16 => HgiFormat::Float16,
        HioFormat::Float32Vec4 => HgiFormat::Float32Vec4,
        HioFormat::Float32Vec3 => HgiFormat::Float32Vec3,
        HioFormat::Float32Vec2 => HgiFormat::Float32Vec2,
        HioFormat::Float32 => HgiFormat::Float32,
        HioFormat::Int32 => HgiFormat::Int32,
        HioFormat::Invalid => HgiFormat::Invalid,
        // SNorm, Double, UInt, Int16, BC formats: map to nearest HGI equivalent or invalid
        HioFormat::SNorm8 => HgiFormat::SNorm8,
        HioFormat::SNorm8Vec2 => HgiFormat::SNorm8Vec2,
        HioFormat::SNorm8Vec3 | HioFormat::SNorm8Vec4 => HgiFormat::SNorm8Vec4,
        HioFormat::UNorm8Srgb
        | HioFormat::UNorm8Vec2Srgb
        | HioFormat::UNorm8Vec3Srgb
        | HioFormat::UNorm8Vec4Srgb => HgiFormat::UNorm8Vec4srgb,
        _ => HgiFormat::Invalid, // Double64, BC, UInt16, Int16, UInt32 etc. not supported
    }
}

/// Extract wrap mode from HioAddressMode.
fn address_mode_to_hd_wrap(mode: HioAddressMode) -> HdWrap {
    match mode {
        HioAddressMode::ClampToEdge => HdWrap::Clamp,
        HioAddressMode::Repeat => HdWrap::Repeat,
        HioAddressMode::MirrorRepeat => HdWrap::Mirror,
        HioAddressMode::ClampToBorderColor => HdWrap::Black,
        HioAddressMode::MirrorClampToEdge => HdWrap::Repeat, // Not directly supported -> fallback
    }
}

/// CPU-side texture data loaded from an asset file path.
///
/// Reads a UV texture from a file, optionally applying:
/// - Vertical flip (for different UV conventions)
/// - Alpha premultiplication
/// - Color space conversion
/// - Mipmap generation
/// - Memory budget constraints
///
/// Port of HdStAssetUvTextureCpuData
#[derive(Debug, Clone)]
pub struct HdStAssetUvTextureCpuData {
    /// File path the texture was loaded from
    file_path: String,
    /// HGI texture descriptor
    texture_desc: HgiTextureDesc,
    /// Raw pixel buffer (potentially converted)
    raw_buffer: Vec<u8>,
    /// Whether GPU should generate mipmaps from level 0
    generate_mipmaps: bool,
    /// Wrap mode info extracted from the image file
    wrap_info: (HdWrap, HdWrap),
    /// Data validity flag
    valid: bool,
}

impl HdStAssetUvTextureCpuData {
    /// Create CPU data from a file path with loading options.
    ///
    /// Reads the image via HIO, applies flip and premultiply, sets up
    /// the HGI texture descriptor for GPU upload.
    ///
    /// # Arguments
    /// * `file_path` - Path to the texture file
    /// * `target_memory` - Memory budget in bytes (0 = full resolution)
    /// * `premultiply_alpha` - Whether to premultiply RGB by alpha
    /// * `origin_location` - Image origin convention
    /// * `source_color_space` - Source color space for conversion
    pub fn new(
        file_path: String,
        target_memory: usize,
        premultiply_alpha: bool,
        origin_location: ImageOriginLocation,
        source_color_space: SourceColorSpaceHint,
    ) -> Self {
        let flipped = origin_location == ImageOriginLocation::LowerLeft;
        let hio_color_space: SourceColorSpace = source_color_space.into();

        let mut result = Self {
            file_path: file_path.clone(),
            texture_desc: HgiTextureDesc::new(),
            raw_buffer: Vec::new(),
            generate_mipmaps: false,
            wrap_info: (HdWrap::NoOpinion, HdWrap::NoOpinion),
            valid: false,
        };

        // Try to load image data via HIO
        let image_data =
            match read_image_data(&file_path, hio_color_space, flipped, premultiply_alpha) {
                Some(data) => data,
                None => {
                    log::warn!("HdStAssetUvTextureCpuData: failed to load '{}'", file_path);
                    // Return invalid result (valid = false)
                    return result;
                }
            };

        // Map HioFormat -> HgiFormat
        let hgi_format = hio_to_hgi_format(image_data.format, image_data.is_srgb);
        if hgi_format == HgiFormat::Invalid {
            log::warn!(
                "HdStAssetUvTextureCpuData: unsupported format for '{}'",
                file_path
            );
            return result;
        }

        // Compute dimensions, respecting memory budget
        let (width, height) = fit_to_memory(
            image_data.width,
            image_data.height,
            hgi_format,
            target_memory,
            &image_data.pixels,
        );

        let dims = Vec3i::new(width, height, 1);
        let total_mips = calc_mip_levels(width as u32, height as u32, 1);

        // Set up HGI descriptor
        result.texture_desc.debug_name = format!(
            "{} - flipVertically={} - premultiplyAlpha={} - format={:?}",
            file_path, flipped as i32, premultiply_alpha as i32, hgi_format,
        );
        result.texture_desc.texture_type = HgiTextureType::Texture2D;
        result.texture_desc.usage = HgiTextureUsage::SHADER_READ;
        result.texture_desc.format = hgi_format;
        result.texture_desc.dimensions = dims;

        // Single mip from file; GPU generates the rest
        result.generate_mipmaps = total_mips > 1;
        result.texture_desc.mip_levels = if result.generate_mipmaps {
            total_mips
        } else {
            1
        };

        // Scale down pixel data if needed for memory budget
        result.raw_buffer = if width == image_data.width && height == image_data.height {
            image_data.pixels
        } else {
            // Downsample to fit budget by taking a subset of rows
            // (simple memory-fit: just use top-left crop of mip 0 for now)
            let bpp = pixel_byte_size(hgi_format);
            let needed = (width as usize) * (height as usize) * bpp;
            image_data.pixels[..needed].to_vec()
        };

        result.valid = !result.raw_buffer.is_empty();
        result
    }

    /// Create from existing CPU data (for testing or manual construction).
    pub fn from_cpu_data(
        file_path: String,
        cpu_data: HdStTextureCpuData,
        wrap_info: (HdWrap, HdWrap),
    ) -> Self {
        let valid = cpu_data.is_valid();
        let generate_mipmaps = cpu_data.generate_mipmaps();
        let raw_buffer = cpu_data.pixel_data().to_vec();

        let mut texture_desc = HgiTextureDesc::new();
        texture_desc.dimensions = cpu_data.dimensions();
        texture_desc.format = cpu_data.format();
        texture_desc.texture_type = HgiTextureType::Texture2D;
        texture_desc.mip_levels = if generate_mipmaps { 1 } else { 1 };
        texture_desc.usage = HgiTextureUsage::SHADER_READ;

        Self {
            file_path,
            texture_desc,
            raw_buffer,
            generate_mipmaps,
            wrap_info,
            valid,
        }
    }

    /// Get the texture descriptor for GPU resource creation.
    pub fn texture_desc(&self) -> &HgiTextureDesc {
        &self.texture_desc
    }

    /// Whether GPU should generate mipmaps.
    pub fn generate_mipmaps(&self) -> bool {
        self.generate_mipmaps
    }

    /// Whether data is valid (file loaded successfully).
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Get wrap mode info extracted from the image file.
    ///
    /// Returns (wrapS, wrapT). Either may be NoOpinion if the file
    /// does not specify a wrap mode.
    pub fn wrap_info(&self) -> &(HdWrap, HdWrap) {
        &self.wrap_info
    }

    /// Get the raw pixel buffer.
    pub fn raw_buffer(&self) -> &[u8] {
        &self.raw_buffer
    }

    /// Get the file path.
    pub fn file_path(&self) -> &str {
        &self.file_path
    }

    /// Get texture dimensions.
    pub fn dimensions(&self) -> Vec3i {
        self.texture_desc.dimensions
    }

    /// Get pixel format.
    pub fn format(&self) -> HgiFormat {
        self.texture_desc.format
    }

    /// Set raw buffer directly (for testing or manual population).
    pub fn set_raw_buffer(&mut self, buffer: Vec<u8>, dims: Vec3i, format: HgiFormat) {
        self.raw_buffer = buffer;
        self.texture_desc.dimensions = dims;
        self.texture_desc.format = format;
        self.valid = !self.raw_buffer.is_empty();
    }

    /// Convert to a general HdStTextureCpuData.
    pub fn to_cpu_data(&self) -> HdStTextureCpuData {
        if self.valid {
            HdStTextureCpuData::new(
                self.raw_buffer.clone(),
                self.texture_desc.dimensions,
                self.texture_desc.format,
                HgiTextureType::Texture2D,
                self.texture_desc.mip_levels,
                self.generate_mipmaps,
            )
        } else {
            HdStTextureCpuData::invalid()
        }
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Return bytes per pixel for an HgiFormat.
fn pixel_byte_size(format: HgiFormat) -> usize {
    match format {
        HgiFormat::UNorm8 | HgiFormat::SNorm8 => 1,
        HgiFormat::UNorm8Vec2 | HgiFormat::SNorm8Vec2 => 2,
        HgiFormat::UNorm8Vec4 | HgiFormat::SNorm8Vec4 | HgiFormat::UNorm8Vec4srgb => 4,
        HgiFormat::Float16 => 2,
        HgiFormat::Float16Vec2 => 4,
        HgiFormat::Float16Vec3 => 6,
        HgiFormat::Float16Vec4 => 8,
        HgiFormat::Float32 | HgiFormat::Int32 => 4,
        HgiFormat::Float32Vec2 | HgiFormat::Int32Vec2 => 8,
        HgiFormat::Float32Vec3 | HgiFormat::Int32Vec3 => 12,
        HgiFormat::Float32Vec4 | HgiFormat::Int32Vec4 => 16,
        _ => 4, // fallback
    }
}

/// Compute dimensions fitting within a memory budget.
///
/// Returns `(width, height)` that fit within `target_memory` bytes.
/// If `target_memory == 0`, returns original dimensions.
fn fit_to_memory(
    orig_w: i32,
    orig_h: i32,
    format: HgiFormat,
    target_memory: usize,
    pixels: &[u8],
) -> (i32, i32) {
    if target_memory == 0 {
        return (orig_w, orig_h);
    }
    let bpp = pixel_byte_size(format);
    if bpp == 0 {
        return (orig_w, orig_h);
    }

    let mut w = orig_w;
    let mut h = orig_h;

    // Halve dimensions until we fit
    while (w as usize) * (h as usize) * bpp > target_memory && w > 1 && h > 1 {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
    }

    // Ensure we have enough pixels in the buffer
    let needed = (w as usize) * (h as usize) * bpp;
    if pixels.len() < needed {
        return (orig_w, orig_h);
    }

    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_cpu_data_creation_invalid_file() {
        // Non-existent file => not valid
        let data = HdStAssetUvTextureCpuData::new(
            "textures/nonexistent_file_99999.png".to_string(),
            0,
            false,
            ImageOriginLocation::UpperLeft,
            SourceColorSpaceHint::SRGB,
        );

        assert_eq!(data.file_path(), "textures/nonexistent_file_99999.png");
        assert!(!data.is_valid());
        assert_eq!(data.wrap_info(), &(HdWrap::NoOpinion, HdWrap::NoOpinion));
    }

    #[test]
    fn test_from_cpu_data() {
        let cpu = HdStTextureCpuData::new_2d(vec![255u8; 256], 8, 8, HgiFormat::UNorm8Vec4, true);
        let data = HdStAssetUvTextureCpuData::from_cpu_data(
            "test.png".to_string(),
            cpu,
            (HdWrap::Repeat, HdWrap::Clamp),
        );

        assert!(data.is_valid());
        assert!(data.generate_mipmaps());
        assert_eq!(data.wrap_info(), &(HdWrap::Repeat, HdWrap::Clamp));
        assert_eq!(data.dimensions(), Vec3i::new(8, 8, 1));
        assert_eq!(data.format(), HgiFormat::UNorm8Vec4);
    }

    #[test]
    fn test_manual_buffer_set() {
        let mut data = HdStAssetUvTextureCpuData::new(
            "proc.png".to_string(),
            0,
            false,
            ImageOriginLocation::UpperLeft,
            SourceColorSpaceHint::Raw,
        );

        assert!(!data.is_valid());

        data.set_raw_buffer(vec![128u8; 64], Vec3i::new(4, 4, 1), HgiFormat::UNorm8Vec4);
        assert!(data.is_valid());
        assert_eq!(data.raw_buffer().len(), 64);
    }

    #[test]
    fn test_to_cpu_data() {
        let cpu = HdStTextureCpuData::new_2d(vec![0u8; 16], 2, 2, HgiFormat::UNorm8Vec4, false);
        let asset = HdStAssetUvTextureCpuData::from_cpu_data(
            "test.png".to_string(),
            cpu,
            (HdWrap::NoOpinion, HdWrap::NoOpinion),
        );

        let converted = asset.to_cpu_data();
        assert!(converted.is_valid());
        assert_eq!(converted.dimensions(), Vec3i::new(2, 2, 1));
    }

    #[test]
    fn test_load_real_png() {
        // Create a tiny 4x4 RGBA8 PNG, load it via AssetUvTextureCpuData
        let temp_path = std::env::temp_dir().join("usd_hdst_test_load.png");
        let temp_str = temp_path.to_str().unwrap();

        // All-red 4x4 RGBA pixels
        let pixels = vec![255u8, 0, 0, 255].repeat(4 * 4);
        image::save_buffer(temp_str, &pixels, 4, 4, image::ColorType::Rgba8).unwrap();

        let data = HdStAssetUvTextureCpuData::new(
            temp_str.to_string(),
            0,
            false,
            ImageOriginLocation::UpperLeft,
            SourceColorSpaceHint::Raw,
        );

        assert!(data.is_valid(), "Should load successfully");
        assert_eq!(data.dimensions(), Vec3i::new(4, 4, 1));
        assert_eq!(data.format(), HgiFormat::UNorm8Vec4);
        assert!(!data.raw_buffer().is_empty());

        // Verify pixel content: first pixel should be red (255, 0, 0, 255)
        assert_eq!(data.raw_buffer()[0], 255, "R channel");
        assert_eq!(data.raw_buffer()[1], 0, "G channel");
        assert_eq!(data.raw_buffer()[2], 0, "B channel");
        assert_eq!(data.raw_buffer()[3], 255, "A channel");

        let _ = std::fs::remove_file(temp_str);
    }

    #[test]
    fn test_load_with_flip() {
        let temp_path = std::env::temp_dir().join("usd_hdst_test_flip.png");
        let temp_str = temp_path.to_str().unwrap();

        // 2x2: row0=red, row1=blue
        #[rustfmt::skip]
        let pixels = vec![
            255u8, 0, 0, 255,  255, 0, 0, 255,   // row 0: red
            0, 0, 255, 255,    0, 0, 255, 255,    // row 1: blue
        ];
        image::save_buffer(temp_str, &pixels, 2, 2, image::ColorType::Rgba8).unwrap();

        let data = HdStAssetUvTextureCpuData::new(
            temp_str.to_string(),
            0,
            false,
            ImageOriginLocation::LowerLeft, // flip!
            SourceColorSpaceHint::Raw,
        );

        assert!(data.is_valid());
        let buf = data.raw_buffer();
        // After flip: row0 should be blue (original row1)
        assert_eq!(buf[0], 0, "R of flipped row0 (expected blue)");
        assert_eq!(buf[2], 255, "B of flipped row0 (expected blue)");

        let _ = std::fs::remove_file(temp_str);
    }

    #[test]
    fn test_load_with_premultiply() {
        let temp_path = std::env::temp_dir().join("usd_hdst_test_premult.png");
        let temp_str = temp_path.to_str().unwrap();

        // 1x1 pixel: RGBA = (200, 100, 50, 128) -> alpha ~50%
        let pixels = vec![200u8, 100, 50, 128];
        image::save_buffer(temp_str, &pixels, 1, 1, image::ColorType::Rgba8).unwrap();

        let data = HdStAssetUvTextureCpuData::new(
            temp_str.to_string(),
            0,
            true, // premultiply!
            ImageOriginLocation::UpperLeft,
            SourceColorSpaceHint::Raw,
        );

        assert!(data.is_valid());
        let buf = data.raw_buffer();
        // R should be approximately 200 * (128/255) + 0.5 ≈ 101
        assert!((buf[0] as i32 - 101).abs() <= 2, "R premult = {}", buf[0]);
        assert_eq!(buf[3], 128, "Alpha unchanged");

        let _ = std::fs::remove_file(temp_str);
    }

    #[test]
    fn test_memory_budget_reduces_dimensions() {
        let temp_path = std::env::temp_dir().join("usd_hdst_test_budget.png");
        let temp_str = temp_path.to_str().unwrap();

        // 16x16 RGBA8 = 1024 bytes
        let pixels = vec![128u8; 16 * 16 * 4];
        image::save_buffer(temp_str, &pixels, 16, 16, image::ColorType::Rgba8).unwrap();

        // Budget: only 64 bytes = fits 4x4 RGBA8
        let data = HdStAssetUvTextureCpuData::new(
            temp_str.to_string(),
            64,
            false,
            ImageOriginLocation::UpperLeft,
            SourceColorSpaceHint::Raw,
        );

        assert!(data.is_valid());
        // Dimensions should be reduced to fit budget
        let dims = data.dimensions();
        let mem = (dims[0] as usize) * (dims[1] as usize) * 4;
        assert!(mem <= 64 * 2, "Memory {} should be near budget 64", mem);

        let _ = std::fs::remove_file(temp_str);
    }

    #[test]
    fn test_hio_to_hgi_format_srgb() {
        assert_eq!(
            hio_to_hgi_format(HioFormat::UNorm8Vec4, true),
            HgiFormat::UNorm8Vec4srgb
        );
        assert_eq!(
            hio_to_hgi_format(HioFormat::UNorm8Vec4, false),
            HgiFormat::UNorm8Vec4
        );
        assert_eq!(
            hio_to_hgi_format(HioFormat::Float32Vec4, false),
            HgiFormat::Float32Vec4
        );
        assert_eq!(
            hio_to_hgi_format(HioFormat::Invalid, false),
            HgiFormat::Invalid
        );
    }
}
