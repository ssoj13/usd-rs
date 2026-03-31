//! Source image container.
//!
//! What: Holds encoded image data and metadata for textures.
//! Why: Mirrors Draco `SourceImage` used by textures and texture IO.
//! How: Stores filename, mime type, and encoded bytes.
//! Where used: `Texture`, `texture_io`, and texture utilities.

/// Encoded image data and metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceImage {
    filename: String,
    mime_type: String,
    encoded_data: Vec<u8>,
}

impl SourceImage {
    /// Creates an empty source image.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `src` into this image.
    pub fn copy_from(&mut self, src: &SourceImage) {
        self.filename = src.filename.clone();
        self.mime_type = src.mime_type.clone();
        self.encoded_data = src.encoded_data.clone();
    }

    /// Sets the filename of the source image.
    pub fn set_filename(&mut self, filename: &str) {
        self.filename = filename.to_string();
    }

    /// Returns the filename of the source image.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Sets the mime type of the encoded data.
    pub fn set_mime_type(&mut self, mime_type: &str) {
        self.mime_type = mime_type.to_string();
    }

    /// Returns the mime type of the encoded data.
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Returns a mutable reference to the encoded image data.
    pub fn encoded_data_mut(&mut self) -> &mut Vec<u8> {
        &mut self.encoded_data
    }

    /// Returns the encoded image data.
    pub fn encoded_data(&self) -> &[u8] {
        &self.encoded_data
    }
}
