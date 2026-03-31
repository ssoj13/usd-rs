//! Image compression options.
//! Reference: `_ref/draco/src/draco/io/image_compression_options.h`.

/// Supported image compression formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    None,
    Png,
    Jpeg,
    Basis,
    Webp,
}

impl Default for ImageFormat {
    fn default() -> Self {
        ImageFormat::None
    }
}
