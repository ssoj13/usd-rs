//! Screenshot capture for viewport exports.

use std::path::Path;

use image::{ImageBuffer, ImageFormat, Rgba};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotFormat {
    Png,
    Jpeg,
    Exr,
}

/// Resolve export format from the file extension.
pub fn detect_format(path: &Path) -> Result<ScreenshotFormat, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => Ok(ScreenshotFormat::Png),
        Some("jpg" | "jpeg") => Ok(ScreenshotFormat::Jpeg),
        Some("exr") => Ok(ScreenshotFormat::Exr),
        Some(ext) => Err(format!(
            "unsupported format: .{ext} (use .png, .jpg, or .exr)"
        )),
        None => Err("no file extension — use .png, .jpg, or .exr".to_string()),
    }
}

/// Save display-corrected RGBA8 pixels to PNG or JPEG.
pub fn save_ldr(pixels: &[u8], width: usize, height: usize, path: &Path) -> Result<(), String> {
    let w = width as u32;
    let h = height as u32;
    let expected = width * height * 4;
    if pixels.len() < expected {
        return Err(format!(
            "pixel buffer too small: {} < {} ({}x{}x4)",
            pixels.len(),
            expected,
            width,
            height
        ));
    }

    let format = match detect_format(path)? {
        ScreenshotFormat::Png => ImageFormat::Png,
        ScreenshotFormat::Jpeg => ImageFormat::Jpeg,
        ScreenshotFormat::Exr => {
            return Err("EXR export requires linear float pixels; use save_exr".to_string());
        }
    };

    let img: ImageBuffer<Rgba<u8>, _> = ImageBuffer::from_raw(w, h, &pixels[..expected])
        .ok_or_else(|| "failed to create image buffer".to_string())?;

    img.save_with_format(path, format)
        .map_err(|e| format!("failed to save {}: {e}", path.display()))
}

/// Save linear RGBA32F pixels to OpenEXR.
pub fn save_exr(pixels: &[f32], width: usize, height: usize, path: &Path) -> Result<(), String> {
    let expected = width * height * 4;
    if pixels.len() < expected {
        return Err(format!(
            "pixel buffer too small: {} < {} ({}x{}x4)",
            pixels.len(),
            expected,
            width,
            height
        ));
    }
    if detect_format(path)? != ScreenshotFormat::Exr {
        return Err("save_exr requires a .exr path".to_string());
    }

    vfx_exr::prelude::write_rgba_file(path, width, height, |x, y| {
        let idx = (y * width + x) * 4;
        (
            pixels[idx],
            pixels[idx + 1],
            pixels[idx + 2],
            pixels[idx + 3],
        )
    })
    .map_err(|e| format!("failed to save {}: {e}", path.display()))
}
