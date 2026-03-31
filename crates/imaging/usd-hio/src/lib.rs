
//! Hio (Hydra Image I/O) - Image and texture loading/writing library.
//!
//! This module provides image I/O functionality for USD, including:
//! - Image format types and utilities
//! - Image loading and writing interface
//! - Plugin-based image format handlers
//! - Volume/field texture data support

pub mod field_texture_data;
pub mod glslfx;
pub mod glslfx_config;
pub mod glslfx_resource_layout;
pub mod image;
pub mod image_reader;
pub mod image_registry;
pub mod ranked_type_map;
pub mod types;

// Re-export commonly used types
pub use types::{
    HioAddressDimension, HioAddressMode, HioFormat, HioType, get_component_count, get_data_size,
    get_data_size_of_format, get_data_size_of_type, get_data_size_of_type_from_format, get_format,
    get_hio_type, is_compressed,
};

pub use image::{
    HioImage, HioImageBase, HioImageSharedPtr, ImageOriginLocation, SourceColorSpace, StorageSpec,
};

pub use image_reader::{
    ExrImage, ImageReadResult, StdImage, open_image, open_image_shared, read_image_data,
    register_standard_formats,
};
pub use image_registry::{HioImageRegistry, ImageFactory, is_supported_image_file};

pub use glslfx::HioGlslfx;
pub use glslfx_config::{
    Attribute as GlslfxAttribute, Attributes as GlslfxAttributes, HioGlslfxConfig,
    MetadataDictionary as GlslfxMetadataDictionary, Parameter as GlslfxParameter,
    Parameters as GlslfxParameters, Role as GlslfxRole, Texture as GlslfxTexture,
    Textures as GlslfxTextures,
};
pub use glslfx_resource_layout::{
    Element as GlslfxElement, ElementVector as GlslfxElementVector, HioGlslfxResourceLayout,
    InOut as GlslfxInOut, Kind as GlslfxKind, Member as GlslfxMember,
    TextureElement as GlslfxTextureElement, TextureType as GlslfxTextureType,
};

pub use ranked_type_map::HioRankedTypeMap;

pub use field_texture_data::{
    FieldTextureDataFactory, HioFieldTextureData, HioFieldTextureDataBase,
    HioFieldTextureDataRegistry, HioFieldTextureDataSharedPtr,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Test that all types are accessible
        let _format = HioFormat::UNorm8Vec4;
        let _type = HioType::Float;
        let _dim = HioAddressDimension::U;
        let _mode = HioAddressMode::Repeat;
        let _origin = ImageOriginLocation::UpperLeft;
        let _color_space = SourceColorSpace::Auto;
    }

    #[test]
    fn test_format_utilities() {
        // Test format utilities are accessible
        let format = get_format(4, HioType::Float, false);
        assert_eq!(format, HioFormat::Float32Vec4);

        let hio_type = get_hio_type(format);
        assert_eq!(hio_type, HioType::Float);

        let count = get_component_count(format);
        assert_eq!(count, 4);

        let size = get_data_size_of_type(hio_type);
        assert_eq!(size, 4);
    }

    #[test]
    fn test_registry_access() {
        // Test that registries are accessible
        let _image_registry = HioImageRegistry::instance();
        let _field_registry = HioFieldTextureDataRegistry::instance();
    }
}
