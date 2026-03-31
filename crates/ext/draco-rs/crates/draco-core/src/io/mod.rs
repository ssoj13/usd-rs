//! IO helpers required by core transcoder types.
//!
//! What: Minimal IO-facing types needed by texture/material utilities.
//! Why: Mirrors `draco/io` headers referenced by texture and material modules.
//! How: Exposes image compression options and formats.
//! Where used: `texture` and `texture_utils` modules.

pub mod image_compression_options;

pub use image_compression_options::ImageFormat;
