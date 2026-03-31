//! Rust port of Draco metadata module.
//! Reference: `_ref/draco/src/draco/metadata`.

pub mod geometry_metadata;
pub mod metadata;
pub mod metadata_decoder;
pub mod metadata_encoder;
pub mod property_attribute;
pub mod property_table;
pub mod structural_metadata;
pub mod structural_metadata_schema;

pub use geometry_metadata::{
    AttributeMetadata, AttributeMetadataHasher, GeometryMetadata, GeometryMetadataHasher,
};
pub use metadata::{EntryValue, EntryValueHasher, Metadata, MetadataHasher};
pub use metadata_decoder::MetadataDecoder;
pub use metadata_encoder::MetadataEncoder;
pub use property_attribute::{Property, PropertyAttribute};
pub use property_table::{
    Data as PropertyTableData, Offsets as PropertyTableOffsets, Property as PropertyTableProperty,
    PropertyTable,
};
pub use structural_metadata::StructuralMetadata;
pub use structural_metadata_schema::{
    Object as StructuralMetadataObject, ObjectType as StructuralMetadataObjectType,
    StructuralMetadataSchema,
};

#[cfg(test)]
mod tests;
