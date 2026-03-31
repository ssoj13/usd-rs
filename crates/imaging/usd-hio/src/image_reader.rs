//! Concrete HioImage readers using the `image` and `exr` crates.
//!
//! Supports PNG, JPEG, BMP, TGA, HDR natively via the `image` crate,
//! and OpenEXR via the `exr` crate.
//! Package-relative paths (e.g. `archive.usdz[texture.png]`) are resolved
//! via the global package resolver registry in `usd-ar`.

use crate::StorageSpec;
use crate::image::{HioImage, HioImageBase, HioImageSharedPtr, SourceColorSpace};
use crate::image_registry::HioImageRegistry;
use crate::types::{
    HioAddressDimension, HioAddressMode, HioFormat, get_component_count, get_data_size_of_format,
};
use std::path::Path;
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Dictionary;

// ---------------------------------------------------------------------------
// Vertical flip helper
// ---------------------------------------------------------------------------

/// Flip a pixel buffer vertically in-place.
///
/// Swaps rows from top and bottom working toward the center.
/// `stride` = bytes per row.
fn flip_vertically(data: &mut [u8], height: usize, stride: usize) {
    if height < 2 {
        return;
    }
    let (mut top, mut bottom) = (0usize, height - 1);
    while top < bottom {
        // Swap row `top` and row `bottom`
        let t_off = top * stride;
        let b_off = bottom * stride;
        // Can't borrow two mutable slices from the same slice directly,
        // so we use split_at_mut.
        let (first, rest) = data.split_at_mut(b_off);
        first[t_off..t_off + stride].swap_with_slice(&mut rest[..stride]);
        top += 1;
        bottom -= 1;
    }
}

// ---------------------------------------------------------------------------
// Premultiply alpha helpers
// ---------------------------------------------------------------------------

/// Premultiply RGB channels by alpha in-place for u8 RGBA pixels.
///
/// Matches C++ `_PremultiplyAlpha<unsigned char, isSRGB=false>`.
fn premultiply_alpha_u8(data: &mut [u8]) {
    debug_assert_eq!(data.len() % 4, 0);
    for chunk in data.chunks_exact_mut(4) {
        let alpha = chunk[3] as f32 / 255.0;
        chunk[0] = (chunk[0] as f32 * alpha + 0.5) as u8;
        chunk[1] = (chunk[1] as f32 * alpha + 0.5) as u8;
        chunk[2] = (chunk[2] as f32 * alpha + 0.5) as u8;
        // alpha channel unchanged
    }
}

/// Premultiply RGB channels by alpha in-place for f32 RGBA pixels.
///
/// Matches C++ `_PremultiplyAlphaFloat<float>`.
fn premultiply_alpha_f32(data: &mut [f32]) {
    debug_assert_eq!(data.len() % 4, 0);
    for chunk in data.chunks_exact_mut(4) {
        let alpha = chunk[3];
        chunk[0] *= alpha;
        chunk[1] *= alpha;
        chunk[2] *= alpha;
        // alpha channel unchanged
    }
}

// ---------------------------------------------------------------------------
// StdImage - handles PNG / JPEG / BMP / TGA / HDR via the `image` crate
// ---------------------------------------------------------------------------

/// Image reader backed by the `image` crate (PNG, JPEG, BMP, TGA, GIF, HDR).
pub struct StdImage {
    base: HioImageBase,
    /// Decoded pixel data (RGBA u8 or RGBA f32 for HDR).
    pixels: Vec<u8>,
    /// Source color space hint (retained for potential re-open).
    _color_space: SourceColorSpace,
}

impl StdImage {
    /// Open an image file for reading.
    ///
    /// Preserves the original channel count from the file (1, 3, or 4),
    /// matching C++ stb_image behavior where _nchannels comes from the file.
    pub fn open(path: &str, color_space: SourceColorSpace) -> Option<Self> {
        let img = image::open(path).ok()?;

        // Determine if sRGB based on color space hint or extension
        let is_hdr = path.ends_with(".hdr") || path.ends_with(".HDR");
        let is_srgb = match color_space {
            SourceColorSpace::SRGB => true,
            SourceColorSpace::Raw => false,
            SourceColorSpace::Auto => !is_hdr,
        };

        // Get dimensions and color type before consuming the image
        let (img_w, img_h) = (img.width(), img.height());
        let color_type = img.color();

        let (width, height, format, pixels) = if is_hdr {
            // HDR: decode to Rgba32F (always 4 channels)
            let rgba = img.into_rgba32f();
            let raw = rgba.into_raw();
            let bytes: Vec<u8> = raw.iter().flat_map(|f| f.to_le_bytes()).collect();
            (img_w as i32, img_h as i32, HioFormat::Float32Vec4, bytes)
        } else {
            // LDR: preserve original channel count like C++ stb_image.
            let nchannels: u32 = match color_type {
                image::ColorType::L8 | image::ColorType::L16 => 1,
                image::ColorType::La8 | image::ColorType::La16 => 2,
                image::ColorType::Rgb8 | image::ColorType::Rgb16 | image::ColorType::Rgb32F => 3,
                _ => 4,
            };
            // C++ stb_image: sRGB only for 3 or 4 channel UNorm8 images
            let effective_srgb = is_srgb && (nchannels == 3 || nchannels == 4);
            let fmt = crate::types::get_format(
                nchannels,
                crate::types::HioType::UnsignedByte,
                effective_srgb,
            );

            let raw = match nchannels {
                1 => img.into_luma8().into_raw(),
                3 => img.into_rgb8().into_raw(),
                _ => img.into_rgba8().into_raw(),
            };
            (img_w as i32, img_h as i32, fmt, raw)
        };

        let mut base = HioImageBase::new();
        base.set_filename(path.to_string());
        base.set_dimensions(width, height);
        base.set_format(format);
        base.set_is_srgb(is_srgb);
        base.set_num_mip_levels(1);

        Some(Self {
            base,
            pixels,
            _color_space: color_space,
        })
    }

    /// Create an empty StdImage for writing.
    pub fn for_writing(path: &str) -> Self {
        let mut base = HioImageBase::new();
        base.set_filename(path.to_string());
        Self {
            base,
            pixels: Vec::new(),
            _color_space: SourceColorSpace::Auto,
        }
    }
}

impl HioImage for StdImage {
    fn filename(&self) -> &str {
        self.base.filename()
    }
    fn width(&self) -> i32 {
        self.base.width()
    }
    fn height(&self) -> i32 {
        self.base.height()
    }
    fn format(&self) -> HioFormat {
        self.base.format()
    }
    fn bytes_per_pixel(&self) -> i32 {
        self.base.bytes_per_pixel()
    }
    fn num_mip_levels(&self) -> i32 {
        self.base.num_mip_levels()
    }
    fn is_color_space_srgb(&self) -> bool {
        self.base.is_color_space_srgb()
    }

    fn read(&mut self, storage: &mut StorageSpec) -> bool {
        if self.pixels.is_empty() || storage.data.is_null() {
            return false;
        }

        // C++ parity: format must match what the file contains
        if storage.format != self.base.format() {
            log::error!(
                "Image format mismatch: requested {:?} but file is {:?}",
                storage.format,
                self.base.format()
            );
            return false;
        }

        let bpp = self.base.bytes_per_pixel() as usize;
        let w = self.base.width() as usize;
        let h = self.base.height() as usize;
        let total = w * h * bpp;

        if self.pixels.len() < total {
            return false;
        }

        // Copy pixel data into the caller's buffer
        let dst = unsafe { std::slice::from_raw_parts_mut(storage.data, total) };
        dst.copy_from_slice(&self.pixels[..total]);

        // Apply vertical flip in-place (C++ stbi__vertical_flip equivalent)
        if storage.flipped {
            let stride = w * bpp;
            flip_vertically(dst, h, stride);
        }

        // Fill storage metadata
        storage.width = self.base.width();
        storage.height = self.base.height();
        storage.depth = 1;
        storage.format = self.base.format();

        true
    }

    fn read_cropped(
        &mut self,
        crop_top: i32,
        crop_bottom: i32,
        crop_left: i32,
        crop_right: i32,
        storage: &mut StorageSpec,
    ) -> bool {
        if self.pixels.is_empty() || storage.data.is_null() {
            return false;
        }

        let bpp = self.base.bytes_per_pixel() as usize;
        let src_w = self.base.width() as usize;
        let src_h = self.base.height() as usize;

        // BUG-1 fix: params are MARGINS (edges to strip), not absolute coords.
        // C++ _IsValidCrop: cropImageWidth = _width - (cropLeft + cropRight)
        let cl = crop_left as usize;
        let cr = crop_right as usize;
        let ct = crop_top as usize;
        let cb = crop_bottom as usize;

        if cl + cr >= src_w || ct + cb >= src_h {
            return false;
        }

        let crop_w = src_w - cl - cr;
        let crop_h = src_h - ct - cb;

        let dst = unsafe { std::slice::from_raw_parts_mut(storage.data, crop_w * crop_h * bpp) };

        // Copy rows starting from `ct` offset, skipping `cl` pixels per row
        for row in 0..crop_h {
            let src_row = ct + row;
            let src_off = (src_row * src_w + cl) * bpp;
            let dst_off = row * crop_w * bpp;
            dst[dst_off..dst_off + crop_w * bpp]
                .copy_from_slice(&self.pixels[src_off..src_off + crop_w * bpp]);
        }

        // Apply vertical flip in-place
        if storage.flipped {
            let stride = crop_w * bpp;
            flip_vertically(dst, crop_h, stride);
        }

        storage.width = crop_w as i32;
        storage.height = crop_h as i32;
        storage.depth = 1;
        storage.format = self.base.format();

        let _ = src_h; // used in bounds check above

        true
    }

    fn write(&mut self, storage: &StorageSpec, _metadata: Option<&Dictionary>) -> bool {
        if storage.data.is_null() || storage.width <= 0 || storage.height <= 0 {
            return false;
        }

        let w = storage.width as u32;
        let h = storage.height as u32;
        let bpp = {
            let (size, _) = get_data_size_of_format(storage.format);
            size
        };
        let total = w as usize * h as usize * bpp;
        let pixels = unsafe { std::slice::from_raw_parts(storage.data, total) };

        let nchannels = get_component_count(storage.format);
        let color_type = match nchannels {
            1 => image::ColorType::L8,
            2 => image::ColorType::La8,
            3 => image::ColorType::Rgb8,
            4 => image::ColorType::Rgba8,
            _ => return false,
        };

        let path = self.base.filename();
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // JPEG: use quality=100 to match C++ stb_image_write behavior
        if matches!(ext.as_str(), "jpg" | "jpeg" | "jpe" | "jfif") {
            let file = match std::fs::File::create(path) {
                Ok(f) => f,
                Err(_) => return false,
            };
            let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
                std::io::BufWriter::new(file),
                100,
            );
            return encoder.encode(pixels, w, h, color_type.into()).is_ok();
        }

        image::save_buffer(path, pixels, w, h, color_type).is_ok()
    }

    fn get_metadata(&self, _key: &Token) -> Option<Box<dyn std::any::Any>> {
        None
    }

    fn get_sampler_metadata(&self, _dim: HioAddressDimension) -> Option<HioAddressMode> {
        None
    }
}

// ---------------------------------------------------------------------------
// ExrImage - OpenEXR reader via the `exr` crate
// ---------------------------------------------------------------------------

/// OpenEXR image reader using the pure-Rust `exr` crate.
pub struct ExrImage {
    base: HioImageBase,
    /// Decoded float32 pixel data (RGBA).
    pixels: Vec<f32>,
}

impl ExrImage {
    /// Create an empty ExrImage for writing.
    pub fn for_writing(path: &str) -> Self {
        let mut base = HioImageBase::new();
        base.set_filename(path.to_string());
        Self {
            base,
            pixels: Vec::new(),
        }
    }

    /// Open an EXR file for reading.
    pub fn open(path: &str) -> Option<Self> {
        use vfx_exr::prelude::*;

        // Store (image_width, pixel_vec) so the set_pixel callback can index correctly.
        // Note: Vec2::width()/height() are coordinate accessors (x/y), NOT image dimensions.
        let img = read_first_rgba_layer_from_file(
            path,
            |resolution, _| {
                let w = resolution.width();
                let count = w * resolution.height();
                (w, vec![(0.0f32, 0.0f32, 0.0f32, 1.0f32); count])
            },
            |storage, pos, (r, g, b, a): (f32, f32, f32, f32)| {
                let (img_w, ref mut pixels) = *storage;
                pixels[pos.y() * img_w + pos.x()] = (r, g, b, a);
            },
        )
        .ok()?;

        let layer = &img.layer_data;
        let w = layer.size.width() as i32;
        let h = layer.size.height() as i32;

        let (_, ref pixel_tuples) = layer.channel_data.pixels;
        let pixels: Vec<f32> = pixel_tuples
            .iter()
            .flat_map(|&(r, g, b, a)| [r, g, b, a])
            .collect();

        let mut base = HioImageBase::new();
        base.set_filename(path.to_string());
        base.set_dimensions(w, h);
        base.set_format(HioFormat::Float32Vec4);
        base.set_is_srgb(false); // EXR is always linear
        base.set_num_mip_levels(1);

        Some(Self { base, pixels })
    }
}

impl HioImage for ExrImage {
    fn filename(&self) -> &str {
        self.base.filename()
    }
    fn width(&self) -> i32 {
        self.base.width()
    }
    fn height(&self) -> i32 {
        self.base.height()
    }
    fn format(&self) -> HioFormat {
        self.base.format()
    }
    fn bytes_per_pixel(&self) -> i32 {
        self.base.bytes_per_pixel()
    }
    fn num_mip_levels(&self) -> i32 {
        self.base.num_mip_levels()
    }
    fn is_color_space_srgb(&self) -> bool {
        false
    }

    fn read(&mut self, storage: &mut StorageSpec) -> bool {
        if self.pixels.is_empty() || storage.data.is_null() {
            return false;
        }

        let w = self.base.width() as usize;
        let h = self.base.height() as usize;
        let byte_len = w * h * 4 * std::mem::size_of::<f32>();

        let src_bytes =
            unsafe { std::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, byte_len) };
        let dst = unsafe { std::slice::from_raw_parts_mut(storage.data, byte_len) };
        dst.copy_from_slice(src_bytes);

        // Apply vertical flip
        if storage.flipped {
            let stride = w * 4 * std::mem::size_of::<f32>();
            flip_vertically(dst, h, stride);
        }

        storage.width = self.base.width();
        storage.height = self.base.height();
        storage.depth = 1;
        storage.format = HioFormat::Float32Vec4;

        true
    }

    fn read_cropped(
        &mut self,
        crop_top: i32,
        crop_bottom: i32,
        crop_left: i32,
        crop_right: i32,
        storage: &mut StorageSpec,
    ) -> bool {
        if self.pixels.is_empty() || storage.data.is_null() {
            return false;
        }

        let src_w = self.base.width() as usize;
        let src_h = self.base.height() as usize;
        let channels = 4usize;

        // BUG-1 fix: params are MARGINS (edges to strip), not absolute coords.
        let cl = crop_left as usize;
        let cr = crop_right as usize;
        let ct = crop_top as usize;
        let cb = crop_bottom as usize;

        if cl + cr >= src_w || ct + cb >= src_h {
            return false;
        }

        let crop_w = src_w - cl - cr;
        let crop_h = src_h - ct - cb;

        let dst = unsafe {
            std::slice::from_raw_parts_mut(storage.data as *mut f32, crop_w * crop_h * channels)
        };

        for row in 0..crop_h {
            let src_row = ct + row;
            let src_off = (src_row * src_w + cl) * channels;
            let dst_off = row * crop_w * channels;
            dst[dst_off..dst_off + crop_w * channels]
                .copy_from_slice(&self.pixels[src_off..src_off + crop_w * channels]);
        }

        // Apply vertical flip (operates on byte view)
        if storage.flipped {
            let dst_bytes = unsafe {
                std::slice::from_raw_parts_mut(
                    dst.as_mut_ptr() as *mut u8,
                    crop_w * crop_h * channels * std::mem::size_of::<f32>(),
                )
            };
            let stride = crop_w * channels * std::mem::size_of::<f32>();
            flip_vertically(dst_bytes, crop_h, stride);
        }

        storage.width = crop_w as i32;
        storage.height = crop_h as i32;
        storage.depth = 1;
        storage.format = HioFormat::Float32Vec4;

        true
    }

    fn write(&mut self, storage: &StorageSpec, _metadata: Option<&Dictionary>) -> bool {
        if storage.data.is_null() || storage.width <= 0 || storage.height <= 0 {
            return false;
        }

        use vfx_exr::prelude::*;

        let w = storage.width as usize;
        let h = storage.height as usize;
        let pixels = unsafe { std::slice::from_raw_parts(storage.data as *const f32, w * h * 4) };

        // Build RGBA channels from flat f32 data
        let r: Vec<f32> = pixels.chunks(4).map(|c| c[0]).collect();
        let g: Vec<f32> = pixels.chunks(4).map(|c| c[1]).collect();
        let b: Vec<f32> = pixels.chunks(4).map(|c| c[2]).collect();
        let a: Vec<f32> = pixels.chunks(4).map(|c| c[3]).collect();

        let channels = SpecificChannels::rgba(|pos: Vec2<usize>| {
            let idx = pos.y() * w + pos.x();
            (r[idx], g[idx], b[idx], a[idx])
        });

        let image = Image::from_channels((w, h), channels);

        image.write().to_file(self.base.filename()).is_ok()
    }

    fn get_metadata(&self, _key: &Token) -> Option<Box<dyn std::any::Any>> {
        None
    }

    fn get_sampler_metadata(&self, _dim: HioAddressDimension) -> Option<HioAddressMode> {
        None
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all standard image format readers with the HioImageRegistry.
///
/// Registers: png, jpg, jpeg, bmp, tga, gif, hdr, exr
pub fn register_standard_formats() {
    let reg = HioImageRegistry::instance();

    // LDR formats via image crate
    for ext in &["png", "jpg", "jpeg", "bmp", "tga", "gif", "hdr"] {
        reg.register(ext, || None); // factory returns None; use open_for_reading
    }

    // EXR via exr crate
    reg.register("exr", || None);

    log::debug!("HIO: registered standard image formats (png/jpg/bmp/tga/gif/hdr/exr)");
}

/// Open an image file, returning a boxed HioImage reader.
///
/// Dispatches to StdImage or ExrImage based on extension.
pub fn open_image(path: &str, color_space: SourceColorSpace) -> Option<Box<dyn HioImage>> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();

    match ext.as_str() {
        "exr" => ExrImage::open(path).map(|img| Box::new(img) as Box<dyn HioImage>),
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "gif" | "hdr" => {
            StdImage::open(path, color_space).map(|img| Box::new(img) as Box<dyn HioImage>)
        }
        _ => None,
    }
}

/// Open an image and return as Arc<dyn HioImage> for the registry API.
pub fn open_image_shared(path: &str, color_space: SourceColorSpace) -> Option<HioImageSharedPtr> {
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();

    match ext.as_str() {
        "exr" => ExrImage::open(path).map(|img| Arc::new(img) as HioImageSharedPtr),
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "gif" | "hdr" => {
            StdImage::open(path, color_space).map(|img| Arc::new(img) as HioImageSharedPtr)
        }
        _ => None,
    }
}

/// Read image data into an owned Vec<u8>, applying flip and premultiply.
///
/// This is the main entry point for texture loading (mirrors C++
/// `HdStTextureUtils::ReadAndConvertImage`). Returns `None` on failure.
///
/// Supports both plain filesystem paths and package-relative paths such as
/// `archive.usdz[textures/diffuse.png]`. Package-relative paths are resolved
/// using the global package resolver registry (registered by crates like usd-sdf).
///
/// # Arguments
/// * `path` - File path or package-relative path to load
/// * `color_space` - Source color space
/// * `flipped` - Flip vertically (OpenGL vs DirectX convention)
/// * `premultiply_alpha` - Premultiply RGB by alpha channel
pub fn read_image_data(
    path: &str,
    color_space: SourceColorSpace,
    flipped: bool,
    premultiply_alpha: bool,
) -> Option<ImageReadResult> {
    // For package-relative paths (e.g. `/path/to/file.usdz[textures/albedo.png]`),
    // extract the image bytes from the archive via the global package resolver,
    // then decode from memory. This avoids writing a temp file to disk.
    if usd_ar::package_utils::is_package_relative_path(path) {
        let asset = usd_ar::open_packaged_asset(path)?;
        let bytes = asset.get_buffer()?;
        return read_image_data_from_bytes(&bytes, path, color_space, flipped, premultiply_alpha);
    }

    // Determine extension from the path (strip any package fragment first).
    let ext = Path::new(path).extension()?.to_str()?.to_lowercase();

    match ext.as_str() {
        "exr" => {
            let mut img = ExrImage::open(path)?;
            let w = img.width() as usize;
            let h = img.height() as usize;
            let total_bytes = w * h * 4 * std::mem::size_of::<f32>();
            let mut buffer = vec![0u8; total_bytes];

            let mut storage = StorageSpec::with_data(
                w as i32,
                h as i32,
                1,
                HioFormat::Float32Vec4,
                buffer.as_mut_ptr(),
            );
            storage.flipped = flipped;

            if !img.read(&mut storage) {
                return None;
            }

            if premultiply_alpha {
                let floats = unsafe {
                    std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut f32, w * h * 4)
                };
                premultiply_alpha_f32(floats);
            }

            Some(ImageReadResult {
                pixels: buffer,
                width: w as i32,
                height: h as i32,
                format: HioFormat::Float32Vec4,
                is_srgb: false,
            })
        }

        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "gif" | "hdr" => {
            let mut img = StdImage::open(path, color_space)?;
            let w = img.width() as usize;
            let h = img.height() as usize;
            let bpp = img.bytes_per_pixel() as usize;
            let total_bytes = w * h * bpp;
            let format = img.format();
            let is_srgb = img.is_color_space_srgb();
            let mut buffer = vec![0u8; total_bytes];

            let mut storage =
                StorageSpec::with_data(w as i32, h as i32, 1, format, buffer.as_mut_ptr());
            storage.flipped = flipped;

            if !img.read(&mut storage) {
                return None;
            }

            // Premultiply alpha for LDR (u8) images
            if premultiply_alpha && format == HioFormat::UNorm8Vec4 {
                premultiply_alpha_u8(&mut buffer);
            } else if premultiply_alpha && format == HioFormat::Float32Vec4 {
                let floats = unsafe {
                    std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut f32, w * h * 4)
                };
                premultiply_alpha_f32(floats);
            }

            Some(ImageReadResult {
                pixels: buffer,
                width: w as i32,
                height: h as i32,
                format,
                is_srgb,
            })
        }

        _ => None,
    }
}

/// Decode image from in-memory bytes, with the same flip/premultiply logic.
///
/// `hint_path` is used only to determine the file extension for format selection.
fn read_image_data_from_bytes(
    bytes: &[u8],
    hint_path: &str,
    color_space: SourceColorSpace,
    flipped: bool,
    premultiply_alpha: bool,
) -> Option<ImageReadResult> {
    use usd_ar::package_utils::split_package_relative_path_inner;

    // Extract the innermost packaged filename to determine extension.
    // e.g. `/abs/file.usdz[textures/albedo.png]` → innermost = `textures/albedo.png`
    let inner_path = if usd_ar::package_utils::is_package_relative_path(hint_path) {
        split_package_relative_path_inner(hint_path).1
    } else {
        hint_path.to_string()
    };

    let ext = Path::new(&inner_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "exr" => {
            // EXR in-memory: vfx-exr crate only supports file paths, so write to
            // a uniquely-named temp file, decode via ExrImage::open(), then delete.
            let tmp_path = {
                let pid = std::process::id();
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0);
                std::env::temp_dir().join(format!("usd_hio_exr_{}_{}.exr", pid, nanos))
            };

            if let Err(e) = std::fs::write(&tmp_path, bytes) {
                log::warn!(
                    "read_image_data_from_bytes: failed to write EXR temp file '{}': {}",
                    tmp_path.display(),
                    e
                );
                return None;
            }

            let tmp_str = tmp_path.to_str().unwrap_or("");
            let result = (|| -> Option<ImageReadResult> {
                let mut img = ExrImage::open(tmp_str)?;
                let w = img.width() as usize;
                let h = img.height() as usize;
                let total_bytes = w * h * 4 * std::mem::size_of::<f32>();
                let mut buffer = vec![0u8; total_bytes];

                let mut storage = StorageSpec::with_data(
                    w as i32,
                    h as i32,
                    1,
                    HioFormat::Float32Vec4,
                    buffer.as_mut_ptr(),
                );
                storage.flipped = flipped;

                if !img.read(&mut storage) {
                    return None;
                }

                if premultiply_alpha {
                    let floats = unsafe {
                        std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut f32, w * h * 4)
                    };
                    premultiply_alpha_f32(floats);
                }

                Some(ImageReadResult {
                    pixels: buffer,
                    width: w as i32,
                    height: h as i32,
                    format: HioFormat::Float32Vec4,
                    is_srgb: false,
                })
            })();

            // Always clean up the temp file regardless of success/failure.
            let _ = std::fs::remove_file(&tmp_path);

            if result.is_none() {
                log::warn!(
                    "read_image_data_from_bytes: EXR decode failed for '{}'",
                    hint_path
                );
            }
            result
        }

        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "gif" | "hdr" => {
            // Decode via the `image` crate's in-memory reader.
            // Auto => sRGB for non-HDR formats (matches C++ stbImage.cpp)
            let is_hdr_ext = ext == "hdr";
            let is_srgb = match color_space {
                SourceColorSpace::SRGB => true,
                SourceColorSpace::Raw => false,
                SourceColorSpace::Auto => !is_hdr_ext,
            };
            let dyn_img = image::load_from_memory(bytes).ok()?;

            if is_hdr_ext {
                // HDR: decode to Rgba32F, store as raw bytes (matches StdImage::open file path).
                // Using to_rgba8() here would destroy the HDR floating-point data.
                let rgba = dyn_img.into_rgba32f();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                let raw = rgba.into_raw();
                let mut buffer: Vec<u8> = raw.iter().flat_map(|f| f.to_le_bytes()).collect();

                if flipped {
                    // Stride = w * 4 channels * 4 bytes per f32
                    flip_vertically(&mut buffer, h, w * 4 * 4);
                }

                if premultiply_alpha {
                    let floats = unsafe {
                        std::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut f32, w * h * 4)
                    };
                    premultiply_alpha_f32(floats);
                }

                Some(ImageReadResult {
                    pixels: buffer,
                    width: w as i32,
                    height: h as i32,
                    format: HioFormat::Float32Vec4,
                    is_srgb: false,
                })
            } else {
                // LDR: decode to Rgba8
                let rgba = dyn_img.into_rgba8();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                let mut buffer = rgba.into_raw();

                if flipped {
                    flip_vertically(&mut buffer, h, w * 4);
                }

                if premultiply_alpha {
                    premultiply_alpha_u8(&mut buffer);
                }

                Some(ImageReadResult {
                    pixels: buffer,
                    width: w as i32,
                    height: h as i32,
                    format: if is_srgb {
                        HioFormat::UNorm8Vec4Srgb
                    } else {
                        HioFormat::UNorm8Vec4
                    },
                    is_srgb,
                })
            }
        }

        _ => {
            log::warn!(
                "read_image_data_from_bytes: unsupported format '{}' in '{}'",
                ext,
                hint_path
            );
            None
        }
    }
}

/// Result of a successful image read operation.
#[derive(Debug)]
pub struct ImageReadResult {
    /// Raw pixel bytes
    pub pixels: Vec<u8>,
    /// Image width in pixels
    pub width: i32,
    /// Image height in pixels
    pub height: i32,
    /// Pixel format
    pub format: HioFormat,
    /// Whether the image uses sRGB color space
    pub is_srgb: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_ext_returns_none() {
        let result = open_image("nonexistent.xyz", SourceColorSpace::Auto);
        assert!(result.is_none());
    }

    #[test]
    fn test_missing_file_returns_none() {
        let result = open_image("nonexistent.png", SourceColorSpace::Auto);
        assert!(result.is_none());
    }

    #[test]
    fn test_exr_missing_returns_none() {
        let result = open_image("nonexistent.exr", SourceColorSpace::Auto);
        assert!(result.is_none());
    }

    #[test]
    fn test_std_image_for_writing() {
        let img = StdImage::for_writing("output.png");
        assert_eq!(img.filename(), "output.png");
        assert_eq!(img.width(), 0);
        assert_eq!(img.height(), 0);
    }

    #[test]
    fn test_read_image_data_missing() {
        let result = read_image_data("nonexistent.png", SourceColorSpace::Auto, false, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_read_image_data_bad_ext() {
        let result = read_image_data("file.xyz", SourceColorSpace::Auto, false, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_flip_vertically_2x2() {
        // 2 rows of 2 pixels each (u8 RGBA = 8 bytes per row = 16 total)
        let mut data: Vec<u8> = vec![
            1, 0, 0, 255, 2, 0, 0, 255, // row 0: red, red
            3, 0, 0, 255, 4, 0, 0, 255, // row 1: different values
        ];
        flip_vertically(&mut data, 2, 8);
        // After flip: row0 should be old row1
        assert_eq!(&data[0..8], &[3u8, 0, 0, 255, 4, 0, 0, 255]);
        assert_eq!(&data[8..16], &[1u8, 0, 0, 255, 2, 0, 0, 255]);
    }

    #[test]
    fn test_flip_vertically_single_row() {
        let mut data = vec![1u8, 2, 3, 4];
        flip_vertically(&mut data, 1, 4); // no-op
        assert_eq!(data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_premultiply_alpha_u8() {
        // Pixel: RGBA = (200, 100, 50, 128) => alpha = 128/255 ≈ 0.502
        let mut data = vec![200u8, 100, 50, 128];
        premultiply_alpha_u8(&mut data);
        // alpha stays = 128
        assert_eq!(data[3], 128);
        // R: 200 * 0.502 + 0.5 ≈ 101
        assert!((data[0] as i32 - 101).abs() <= 2, "R was {}", data[0]);
        // A unchanged
    }

    #[test]
    fn test_premultiply_alpha_f32_opaque() {
        let mut data = vec![0.8f32, 0.5, 0.2, 1.0]; // alpha = 1.0 -> no change
        premultiply_alpha_f32(&mut data);
        assert!((data[0] - 0.8).abs() < 1e-6);
        assert!((data[1] - 0.5).abs() < 1e-6);
        assert!((data[2] - 0.2).abs() < 1e-6);
        assert!((data[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_premultiply_alpha_f32_half() {
        let mut data = vec![0.8f32, 0.4, 0.2, 0.5]; // alpha = 0.5
        premultiply_alpha_f32(&mut data);
        assert!((data[0] - 0.4).abs() < 1e-6, "R={}", data[0]);
        assert!((data[1] - 0.2).abs() < 1e-6, "G={}", data[1]);
        assert!((data[2] - 0.1).abs() < 1e-6, "B={}", data[2]);
        assert!((data[3] - 0.5).abs() < 1e-6); // alpha unchanged
    }

    #[test]
    fn test_write_then_read_png() {
        
        // Create a tiny 2x2 RGBA PNG in memory via image crate, save it temporarily,
        // then load it back and verify dimensions.
        let temp_path = std::env::temp_dir().join("usd_hio_test_2x2.png");
        let temp_str = temp_path.to_str().unwrap();

        // 2x2 RGBA8: all white
        let pixels = vec![255u8; 2 * 2 * 4];
        image::save_buffer(temp_str, &pixels, 2, 2, image::ColorType::Rgba8).unwrap();

        let img = open_image(temp_str, SourceColorSpace::Auto);
        assert!(img.is_some());
        let img = img.unwrap();
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);

        // Clean up
        let _ = std::fs::remove_file(temp_str);
    }

    #[test]
    fn test_read_png_with_flip() {
        // Write a 2x2 test image with distinct rows, read with flip, verify order
        let temp_path = std::env::temp_dir().join("usd_hio_test_flip.png");
        let temp_str = temp_path.to_str().unwrap();

        // Row 0: red (255,0,0,255), Row 1: blue (0,0,255,255)
        #[rustfmt::skip]
        let pixels = vec![
            255u8, 0, 0, 255,  255, 0, 0, 255,  // row 0: red
            0, 0, 255, 255,    0, 0, 255, 255,   // row 1: blue
        ];
        image::save_buffer(temp_str, &pixels, 2, 2, image::ColorType::Rgba8).unwrap();

        let result = read_image_data(temp_str, SourceColorSpace::Auto, true, false);
        assert!(result.is_some());
        let result = result.unwrap();

        // After flip: row 0 should be original row 1 (blue)
        assert_eq!(result.pixels[0], 0, "R of flipped row0 should be 0 (blue)");
        assert_eq!(
            result.pixels[2], 255,
            "B of flipped row0 should be 255 (blue)"
        );
        // row 1 should be original row 0 (red)
        let row1_start = 2 * 4;
        assert_eq!(
            result.pixels[row1_start], 255,
            "R of flipped row1 should be 255 (red)"
        );

        let _ = std::fs::remove_file(temp_str);
    }

    #[test]
    fn test_read_png_with_premultiply() {
        let temp_path = std::env::temp_dir().join("usd_hio_test_premult.png");
        let temp_str = temp_path.to_str().unwrap();

        // 1x1 pixel: RGBA = (200, 100, 50, 128)
        let pixels = vec![200u8, 100, 50, 128];
        image::save_buffer(temp_str, &pixels, 1, 1, image::ColorType::Rgba8).unwrap();

        let result = read_image_data(temp_str, SourceColorSpace::Raw, false, true);
        assert!(result.is_some());
        let result = result.unwrap();

        // Alpha = 128/255 ≈ 0.502; R = 200 * 0.502 + 0.5 ≈ 101
        assert!(
            (result.pixels[0] as i32 - 101).abs() <= 2,
            "R={}",
            result.pixels[0]
        );
        assert_eq!(result.pixels[3], 128); // alpha unchanged

        let _ = std::fs::remove_file(temp_str);
    }

    /// BUG-1 regression: zero margins = full image (no crop)
    #[test]
    fn test_read_cropped_zero_margins_equals_read() {
        let temp_path = std::env::temp_dir().join("usd_hio_crop_zero.png");
        let temp_str = temp_path.to_str().unwrap();
        // 4x4 green RGBA image
        let pixels: Vec<u8> = (0..4 * 4).flat_map(|_| [0u8, 255u8, 0u8, 255u8]).collect();
        image::save_buffer(temp_str, &pixels, 4, 4, image::ColorType::Rgba8).unwrap();

        let mut img = StdImage::open(temp_str, SourceColorSpace::Raw).unwrap();
        let total = 4 * 4 * 4;
        let mut buf = vec![0u8; total];
        let mut storage = StorageSpec::with_data(4, 4, 1, HioFormat::UNorm8Vec4, buf.as_mut_ptr());
        // margins all zero => full image
        assert!(img.read_cropped(0, 0, 0, 0, &mut storage));
        assert_eq!(storage.width, 4);
        assert_eq!(storage.height, 4);
        assert_eq!(
            buf[1], 255,
            "G channel of first pixel should be 255 (green)"
        );
        let _ = std::fs::remove_file(temp_str);
    }

    /// BUG-1 regression: margin semantics -- strip 1px from each edge of 4x4 => 2x2 center
    #[test]
    fn test_read_cropped_margin_semantics() {
        let temp_path = std::env::temp_dir().join("usd_hio_crop_margin.png");
        let temp_str = temp_path.to_str().unwrap();

        // 4x4 image: R = x, G = y, B = 0, A = 255 for identification
        let mut pixels = vec![0u8; 4 * 4 * 4];
        for y in 0..4usize {
            for x in 0..4usize {
                let off = (y * 4 + x) * 4;
                pixels[off] = x as u8; // R = x
                pixels[off + 1] = y as u8; // G = y
                pixels[off + 3] = 255;
            }
        }
        image::save_buffer(temp_str, &pixels, 4, 4, image::ColorType::Rgba8).unwrap();

        let mut img = StdImage::open(temp_str, SourceColorSpace::Raw).unwrap();

        // Strip 1 pixel from each edge: result = 2x2 center (x=1..2, y=1..2)
        let total = 2 * 2 * 4;
        let mut buf = vec![0u8; total];
        let mut storage = StorageSpec::with_data(2, 2, 1, HioFormat::UNorm8Vec4, buf.as_mut_ptr());
        // crop_top=1, crop_bottom=1, crop_left=1, crop_right=1
        assert!(img.read_cropped(1, 1, 1, 1, &mut storage));
        assert_eq!(storage.width, 2);
        assert_eq!(storage.height, 2);
        // First pixel in result: x=1, y=1 => R=1, G=1
        assert_eq!(buf[0], 1, "R should be 1 (x=1)");
        assert_eq!(buf[1], 1, "G should be 1 (y=1)");
        let _ = std::fs::remove_file(temp_str);
    }

    /// EXR in-memory loading via temp-file fallback
    #[test]
    fn test_exr_from_bytes_via_tempfile() {
        use vfx_exr::prelude::*;

        // Write a 2x2 EXR to a buffer using the exr crate, then decode it
        // from memory via read_image_data_from_bytes to exercise the temp-file path.
        let w = 2usize;
        let h = 2usize;
        // Known pixel values: alternating (1.0, 0.0, 0.0, 1.0) and (0.0, 1.0, 0.0, 1.0)
        let src: Vec<(f32, f32, f32, f32)> = vec![
            (1.0, 0.0, 0.0, 1.0),
            (0.0, 1.0, 0.0, 1.0),
            (0.0, 0.0, 1.0, 1.0),
            (1.0, 1.0, 0.0, 1.0),
        ];

        let tmp_exr = std::env::temp_dir().join("usd_hio_test_roundtrip.exr");
        let tmp_str = tmp_exr.to_str().unwrap();

        {
            let channels = SpecificChannels::rgba(|pos: Vec2<usize>| {
                let (r, g, b, a) = src[pos.y() * w + pos.x()];
                (r, g, b, a)
            });
            let image = Image::from_channels((w, h), channels);
            image.write().to_file(tmp_str).expect("write test EXR");
        }

        // Read bytes from disk, then decode from memory
        let bytes = std::fs::read(&tmp_exr).expect("read test EXR");
        let _ = std::fs::remove_file(&tmp_exr);

        let result =
            read_image_data_from_bytes(&bytes, "test.exr", SourceColorSpace::Raw, false, false);
        assert!(
            result.is_some(),
            "EXR from memory should decode successfully"
        );
        let result = result.unwrap();
        assert_eq!(result.width, 2);
        assert_eq!(result.height, 2);
        assert_eq!(result.format, HioFormat::Float32Vec4);
        assert!(!result.is_srgb, "EXR should be linear");

        // Verify pixel buffer has the right byte count and all alpha values == 1.0.
        // (Channel order within ExrImage follows vfx-exr RGBA layout.)
        assert_eq!(result.pixels.len(), w * h * 4 * std::mem::size_of::<f32>());
        let floats =
            unsafe { std::slice::from_raw_parts(result.pixels.as_ptr() as *const f32, w * h * 4) };
        // All source pixels have alpha == 1.0 — verify all decoded alpha channels.
        for (i, chunk) in floats.chunks(4).enumerate() {
            assert!(
                (chunk[3] - 1.0).abs() < 0.01,
                "alpha of px[{}] = {}",
                i,
                chunk[3]
            );
        }
        // At least one pixel should have a non-zero R value (red or yellow pixel present).
        let any_red = floats.chunks(4).any(|c| c[0] > 0.5);
        assert!(any_red, "Expected at least one pixel with R > 0.5");
    }

    /// BUG-4 regression: Auto color space => sRGB for non-HDR formats
    #[test]
    fn test_from_bytes_auto_is_srgb_for_png() {
        // We test open() which also applies Auto->sRGB for LDR
        let temp_path = std::env::temp_dir().join("usd_hio_auto_srgb.png");
        let temp_str = temp_path.to_str().unwrap();
        let pixels = vec![128u8; 4]; // 1x1 RGBA
        image::save_buffer(temp_str, &pixels, 1, 1, image::ColorType::Rgba8).unwrap();

        let img = StdImage::open(temp_str, SourceColorSpace::Auto).unwrap();
        assert!(img.is_color_space_srgb(), "Auto should be sRGB for LDR PNG");

        let img_raw = StdImage::open(temp_str, SourceColorSpace::Raw).unwrap();
        assert!(!img_raw.is_color_space_srgb(), "Raw should NOT be sRGB");

        let _ = std::fs::remove_file(temp_str);
    }
}
