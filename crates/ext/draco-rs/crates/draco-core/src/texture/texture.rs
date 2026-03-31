//! Texture container.
//!
//! What: Wraps a source image and compression preferences.
//! Why: Mirrors Draco `Texture` used by materials and texture IO.
//! How: Stores `SourceImage` plus optional compression settings.
//! Where used: `TextureMap`, `TextureLibrary`, and texture utilities.

use crate::texture::source_image::SourceImage;

/// Texture storing encoded source image data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Texture {
    source_image: SourceImage,
}

impl Texture {
    /// Creates an empty texture.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `other` into this texture.
    pub fn copy_from(&mut self, other: &Texture) {
        self.source_image.copy_from(&other.source_image);
    }

    /// Sets the source image for this texture.
    pub fn set_source_image(&mut self, image: &SourceImage) {
        self.source_image.copy_from(image);
    }

    /// Returns the source image (immutable).
    pub fn source_image(&self) -> &SourceImage {
        &self.source_image
    }

    /// Returns the source image (mutable).
    pub fn source_image_mut(&mut self) -> &mut SourceImage {
        &mut self.source_image
    }
}
