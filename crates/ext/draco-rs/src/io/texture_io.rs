//! Texture IO helpers.
//! Reference: `_ref/draco/src/draco/io/texture_io.h` + `.cc`.

use crate::core::status::{ok_status, Status, StatusCode};
use crate::core::status_or::StatusOr;
use crate::io::file_utils;
use crate::texture::{SourceImage, Texture, TextureUtils};
use draco_core::io::image_compression_options::ImageFormat;

fn create_draco_texture_internal(
    image_data: &[u8],
    out_source_image: &mut SourceImage,
) -> StatusOr<Box<Texture>> {
    let draco_texture = Box::new(Texture::new());
    let format = image_format_from_buffer(image_data);
    out_source_image.encoded_data_mut().clear();
    out_source_image
        .encoded_data_mut()
        .extend_from_slice(image_data);
    out_source_image.set_mime_type(&TextureUtils::get_mime_type(format));
    StatusOr::new_value(draco_texture)
}

/// Detects image format for an encoded buffer.
pub fn image_format_from_buffer(buffer: &[u8]) -> ImageFormat {
    if buffer.len() > 4 {
        let jpeg_soi = [0xFFu8, 0xD8u8];
        let jpeg_eoi = [0xFFu8, 0xD9u8];
        if buffer.starts_with(&jpeg_soi) {
            if find_end(buffer, &jpeg_eoi).is_some() {
                return ImageFormat::Jpeg;
            }
        }
    }

    if buffer.len() > 2 {
        let basis_signature = [0x42u8, 0x73u8];
        if buffer.starts_with(&basis_signature) {
            return ImageFormat::Basis;
        }
    }

    if buffer.len() > 4 {
        let ktx2_signature = [0xabu8, 0x4bu8, 0x54u8, 0x58u8];
        if buffer.starts_with(&ktx2_signature) {
            return ImageFormat::Basis;
        }
    }

    if buffer.len() > 8 {
        let png_signature = [
            0x89u8, 0x50u8, 0x4eu8, 0x47u8, 0x0du8, 0x0au8, 0x1au8, 0x0au8,
        ];
        if buffer.starts_with(&png_signature) {
            return ImageFormat::Png;
        }
    }

    if buffer.len() > 12 {
        let riff = [0x52u8, 0x49u8, 0x46u8, 0x46u8];
        let webp = [0x57u8, 0x45u8, 0x42u8, 0x50u8];
        if buffer.starts_with(&riff) && buffer[8..].starts_with(&webp) {
            return ImageFormat::Webp;
        }
    }

    ImageFormat::None
}

/// Reads a texture from a file.
pub fn read_texture_from_file(file_name: &str) -> StatusOr<Box<Texture>> {
    let mut image_data: Vec<u8> = Vec::new();
    if !file_utils::read_file_to_buffer(file_name, &mut image_data) {
        return StatusOr::new_status(Status::new(
            StatusCode::IoError,
            "Unable to read input texture file.",
        ));
    }

    let mut source_image = SourceImage::new();
    let texture_or = create_draco_texture_internal(&image_data, &mut source_image);
    if !texture_or.is_ok() {
        return texture_or;
    }
    let mut texture = texture_or.into_value();
    source_image.set_filename(file_name);
    if source_image.mime_type().is_empty() {
        let extension = file_utils::lowercase_file_extension(file_name);
        if !extension.is_empty() {
            let mime_type = if extension == "jpg" {
                "image/jpeg".to_string()
            } else {
                format!("image/{}", extension)
            };
            source_image.set_mime_type(&mime_type);
        }
    }
    texture.set_source_image(&source_image);
    StatusOr::new_value(texture)
}

/// Reads a texture from a buffer.
pub fn read_texture_from_buffer(buffer: &[u8]) -> StatusOr<Box<Texture>> {
    let mut source_image = SourceImage::new();
    let texture_or = create_draco_texture_internal(buffer, &mut source_image);
    if !texture_or.is_ok() {
        return texture_or;
    }
    let mut texture = texture_or.into_value();
    texture.set_source_image(&source_image);
    StatusOr::new_value(texture)
}

/// Deprecated: mime type is ignored, detected from buffer.
pub fn read_texture_from_buffer_with_mime(
    buffer: &[u8],
    _mime_type: &str,
) -> StatusOr<Box<Texture>> {
    read_texture_from_buffer(buffer)
}

/// Writes a texture to a file.
pub fn write_texture_to_file(file_name: &str, texture: &Texture) -> Status {
    let mut buffer: Vec<u8> = Vec::new();
    let status = write_texture_to_buffer(texture, &mut buffer);
    if !status.is_ok() {
        return status;
    }
    if !file_utils::write_buffer_to_file(&buffer, file_name) {
        return Status::new(StatusCode::DracoError, "Failed to write image.");
    }
    ok_status()
}

/// Writes a texture to a buffer.
pub fn write_texture_to_buffer(texture: &Texture, buffer: &mut Vec<u8>) -> Status {
    if !texture.source_image().encoded_data().is_empty() {
        buffer.clear();
        buffer.extend_from_slice(texture.source_image().encoded_data());
        return ok_status();
    }
    if !texture.source_image().filename().is_empty() {
        if !file_utils::read_file_to_buffer(texture.source_image().filename(), buffer) {
            return Status::new(StatusCode::IoError, "Unable to read input texture file.");
        }
        return ok_status();
    }
    Status::new(
        StatusCode::DracoError,
        "Invalid source data for the texture.",
    )
}

fn find_end(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let mut i = haystack.len() - needle.len();
    loop {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i);
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}
