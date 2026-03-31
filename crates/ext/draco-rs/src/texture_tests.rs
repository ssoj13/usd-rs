//! Texture test ports from the Draco C++ reference.
//!
//! What: Re-implements `texture_utils_test.cc` for Rust.
//! Why: Confirms parity for naming, format, and mime type helpers.
//! Where used: Runs under `draco-rs` tests; relies on `crates/draco-rs/test`.

use std::env;
use std::path::PathBuf;

use crate::io::texture_io;
use crate::io::ImageFormat;
use crate::texture::{Texture, TextureUtils};

fn test_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_RS_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(dir) = env::var("DRACO_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test")
}

fn read_texture_from_test_file(file_name: &str) -> Box<Texture> {
    let path = test_data_dir()
        .join(file_name)
        .to_string_lossy()
        .into_owned();
    let status_or = texture_io::read_texture_from_file(&path);
    assert!(
        status_or.is_ok(),
        "{}",
        status_or.status().error_msg_string()
    );
    status_or.into_value()
}

#[test]
fn texture_utils_target_name_for_loaded_texture() {
    let texture = read_texture_from_test_file("fast.jpg");
    assert_eq!(TextureUtils::get_target_stem(&texture), "fast");
    assert_eq!(TextureUtils::get_target_extension(&texture), "jpg");
    assert_eq!(TextureUtils::get_target_format(&texture), ImageFormat::Jpeg);
    assert_eq!(
        TextureUtils::get_or_generate_target_stem(&texture, 5, "_Color"),
        "fast"
    );
}

#[test]
fn texture_utils_target_name_for_new_texture() {
    let texture = Texture::new();
    assert_eq!(TextureUtils::get_target_stem(&texture), "");
    assert_eq!(
        TextureUtils::get_or_generate_target_stem(&texture, 5, "_Color"),
        "Texture5_Color"
    );
    assert_eq!(TextureUtils::get_target_extension(&texture), "png");
    assert_eq!(TextureUtils::get_target_format(&texture), ImageFormat::Png);
}

#[test]
fn texture_utils_source_format() {
    let new_texture = Texture::new();
    let png_texture = read_texture_from_test_file("test.png");
    let mut jpg_texture = read_texture_from_test_file("fast.jpg");

    assert_eq!(
        TextureUtils::get_source_format(&new_texture),
        ImageFormat::Png
    );
    assert_eq!(
        TextureUtils::get_source_format(&png_texture),
        ImageFormat::Png
    );
    assert_eq!(
        TextureUtils::get_source_format(&jpg_texture),
        ImageFormat::Jpeg
    );

    jpg_texture.source_image_mut().set_mime_type("");
    assert_eq!(
        TextureUtils::get_source_format(&jpg_texture),
        ImageFormat::Jpeg
    );
}

#[test]
fn texture_utils_get_format() {
    assert_eq!(TextureUtils::get_format("png"), ImageFormat::Png);
    assert_eq!(TextureUtils::get_format("jpg"), ImageFormat::Jpeg);
    assert_eq!(TextureUtils::get_format("jpeg"), ImageFormat::Jpeg);
    assert_eq!(TextureUtils::get_format("basis"), ImageFormat::Basis);
    assert_eq!(TextureUtils::get_format("ktx2"), ImageFormat::Basis);
    assert_eq!(TextureUtils::get_format("webp"), ImageFormat::Webp);
    assert_eq!(TextureUtils::get_format(""), ImageFormat::None);
    assert_eq!(TextureUtils::get_format("bmp"), ImageFormat::None);
}

#[test]
fn texture_utils_get_target_mime_type() {
    let mut texture = Texture::new();
    texture.source_image_mut().set_mime_type("image/jpeg");
    assert_eq!(TextureUtils::get_target_mime_type(&texture), "image/jpeg");

    let mut unknown_format = Texture::new();
    unknown_format
        .source_image_mut()
        .set_mime_type("image/custom");
    assert_eq!(
        TextureUtils::get_target_mime_type(&unknown_format),
        "image/custom"
    );

    let mut unknown_format_file_name = Texture::new();
    unknown_format_file_name
        .source_image_mut()
        .set_filename("test.extension");
    assert_eq!(
        TextureUtils::get_target_mime_type(&unknown_format_file_name),
        "image/extension"
    );
}

#[test]
fn texture_utils_get_mime_type() {
    assert_eq!(TextureUtils::get_mime_type(ImageFormat::Png), "image/png");
    assert_eq!(TextureUtils::get_mime_type(ImageFormat::Jpeg), "image/jpeg");
    assert_eq!(
        TextureUtils::get_mime_type(ImageFormat::Basis),
        "image/ktx2"
    );
    assert_eq!(TextureUtils::get_mime_type(ImageFormat::Webp), "image/webp");
    assert_eq!(TextureUtils::get_mime_type(ImageFormat::None), "");
}

#[test]
fn texture_utils_get_extension() {
    assert_eq!(TextureUtils::get_extension(ImageFormat::Png), "png");
    assert_eq!(TextureUtils::get_extension(ImageFormat::Jpeg), "jpg");
    assert_eq!(TextureUtils::get_extension(ImageFormat::Basis), "ktx2");
    assert_eq!(TextureUtils::get_extension(ImageFormat::Webp), "webp");
    assert_eq!(TextureUtils::get_extension(ImageFormat::None), "");
}
