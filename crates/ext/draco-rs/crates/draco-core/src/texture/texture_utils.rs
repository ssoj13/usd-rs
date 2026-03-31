//! Texture utilities.
//!
//! What: Helper functions for naming and format decisions.
//! Why: Mirrors Draco `TextureUtils` for glTF transcoding.
//! How: Derives stems, extensions, mime types, and channel requirements.
//! Where used: Texture IO and material processing.

use std::collections::HashSet;

use crate::io::image_compression_options::ImageFormat;
use crate::material::material_library::MaterialLibrary;
use crate::texture::texture::Texture;
use crate::texture::texture_map::TextureMapType;

/// Helper utilities for working with textures.
pub struct TextureUtils;

impl TextureUtils {
    /// Returns the image stem from source filename or empty string.
    pub fn get_target_stem(texture: &Texture) -> String {
        let filename = texture.source_image().filename();
        if !filename.is_empty() {
            let (_folder, file) = split_path(filename);
            return remove_file_extension(&file);
        }
        String::new()
    }

    /// Returns stem or generates one from index and suffix.
    pub fn get_or_generate_target_stem(texture: &Texture, index: i32, suffix: &str) -> String {
        let name = Self::get_target_stem(texture);
        if !name.is_empty() {
            return name;
        }
        format!("Texture{}{}", index, suffix)
    }

    /// Returns target format (delegates to source format, matching C++).
    pub fn get_target_format(texture: &Texture) -> ImageFormat {
        Self::get_source_format(texture)
    }

    /// Returns target extension derived from target format.
    pub fn get_target_extension(texture: &Texture) -> String {
        Self::get_extension(Self::get_target_format(texture))
    }

    /// Returns target mime type for the texture.
    pub fn get_target_mime_type(texture: &Texture) -> String {
        let format = Self::get_target_format(texture);
        if format == ImageFormat::None {
            if !texture.source_image().mime_type().is_empty() {
                return texture.source_image().mime_type().to_string();
            } else if !texture.source_image().filename().is_empty() {
                let extension = lowercase_file_extension(texture.source_image().filename());
                if !extension.is_empty() {
                    return format!("image/{}", extension);
                }
            }
        }
        Self::get_mime_type(format)
    }

    /// Returns mime type for a given format.
    pub fn get_mime_type(image_format: ImageFormat) -> String {
        match image_format {
            ImageFormat::Png => "image/png".to_string(),
            ImageFormat::Jpeg => "image/jpeg".to_string(),
            ImageFormat::Basis => "image/ktx2".to_string(),
            ImageFormat::Webp => "image/webp".to_string(),
            ImageFormat::None => String::new(),
        }
    }

    /// Returns format derived from mime type or filename extension.
    pub fn get_source_format(texture: &Texture) -> ImageFormat {
        let mut extension = lowercase_mime_type_extension(texture.source_image().mime_type());
        if extension.is_empty() && !texture.source_image().filename().is_empty() {
            extension = lowercase_file_extension(texture.source_image().filename());
        }
        if extension.is_empty() {
            extension = "png".to_string();
        }
        Self::get_format(&extension)
    }

    /// Returns format for a file extension.
    pub fn get_format(extension: &str) -> ImageFormat {
        match extension {
            "png" => ImageFormat::Png,
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "basis" | "ktx2" => ImageFormat::Basis,
            "webp" => ImageFormat::Webp,
            _ => ImageFormat::None,
        }
    }

    /// Returns extension for a format.
    pub fn get_extension(format: ImageFormat) -> String {
        match format {
            ImageFormat::Png => "png".to_string(),
            ImageFormat::Jpeg => "jpg".to_string(),
            ImageFormat::Basis => "ktx2".to_string(),
            ImageFormat::Webp => "webp".to_string(),
            ImageFormat::None => String::new(),
        }
    }

    /// Computes required channels for a texture used by given material library.
    /// Matches C++ logic: MetallicRoughness=3, otherwise=1.
    pub fn compute_required_num_channels(
        texture: &Texture,
        material_library: &MaterialLibrary,
    ) -> i32 {
        let texture_ptr = texture as *const Texture;
        let mr_textures = Self::find_textures(TextureMapType::MetallicRoughness, material_library);
        if mr_textures.iter().any(|ptr| *ptr == texture_ptr) {
            return 3;
        }
        1
    }

    /// Finds all textures of a given type (unique).
    pub fn find_textures(
        texture_type: TextureMapType,
        material_library: &MaterialLibrary,
    ) -> Vec<*const Texture> {
        let mut textures: HashSet<*const Texture> = HashSet::new();
        for i in 0..material_library.num_materials() {
            if let Some(material) = material_library.material(i as i32) {
                if let Some(texture_map) = material.texture_map_by_type(texture_type) {
                    if let Some(texture) = texture_map.texture() {
                        textures.insert(texture as *const Texture);
                    }
                }
            }
        }
        textures.into_iter().collect()
    }
}

fn split_path(full_path: &str) -> (String, String) {
    if let Some(pos) = full_path.rfind(['/', '\\']) {
        (
            full_path[..pos + 1].to_string(),
            full_path[pos + 1..].to_string(),
        )
    } else {
        (String::new(), full_path.to_string())
    }
}

fn remove_file_extension(filename: &str) -> String {
    if let Some(pos) = filename.rfind('.') {
        if pos == 0 || pos == filename.len() - 1 {
            return filename.to_string();
        }
        return filename[..pos].to_string();
    }
    filename.to_string()
}

fn lowercase_file_extension(filename: &str) -> String {
    if let Some(pos) = filename.rfind('.') {
        if pos == 0 || pos == filename.len() - 1 {
            return String::new();
        }
        return filename[pos + 1..].to_ascii_lowercase();
    }
    String::new()
}

fn lowercase_mime_type_extension(mime_type: &str) -> String {
    if let Some(pos) = mime_type.rfind('/') {
        if pos == 0 || pos == mime_type.len() - 1 {
            return String::new();
        }
        return mime_type[pos + 1..].to_ascii_lowercase();
    }
    String::new()
}
