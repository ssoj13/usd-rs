//! Texture module.
//! Reference: `_ref/draco/src/draco/texture/*`.

pub mod source_image;
pub mod texture;
pub mod texture_library;
pub mod texture_map;
pub mod texture_transform;
pub mod texture_utils;

pub use source_image::SourceImage;
pub use texture::Texture;
pub use texture_library::TextureLibrary;
pub use texture_map::{
    TextureMap, TextureMapAxisWrappingMode, TextureMapFilterType, TextureMapType,
    TextureMapWrappingMode,
};
pub use texture_transform::TextureTransform;
pub use texture_utils::TextureUtils;
