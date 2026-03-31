//! Structural metadata container (EXT_structural_metadata).
//!
//! What: Groups schema, property tables, and property attributes.
//! Why: Mirrors Draco C++ `StructuralMetadata` so meshes can carry structured
//! metadata for glTF round-tripping.
//! How: Stores owned vectors of property tables/attributes and a schema object.
//! Where used: Intended to be attached to meshes and copied alongside them.

use crate::metadata::property_attribute::PropertyAttribute;
use crate::metadata::property_table::PropertyTable;
use crate::metadata::structural_metadata_schema::StructuralMetadataSchema;

/// Holds EXT_structural_metadata data for a mesh.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralMetadata {
    schema: StructuralMetadataSchema,
    property_tables: Vec<PropertyTable>,
    property_attributes: Vec<PropertyAttribute>,
}

impl StructuralMetadata {
    /// Creates an empty structural metadata object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `src` into this object.
    pub fn copy_from(&mut self, src: &StructuralMetadata) {
        self.schema = src.schema.clone();
        self.property_tables.clear();
        self.property_tables
            .extend(src.property_tables.iter().cloned());
        self.property_attributes.clear();
        self.property_attributes
            .extend(src.property_attributes.iter().cloned());
    }

    /// Sets the schema for this metadata.
    pub fn set_schema(&mut self, schema: StructuralMetadataSchema) {
        self.schema = schema;
    }

    /// Returns the schema for this metadata.
    pub fn schema(&self) -> &StructuralMetadataSchema {
        &self.schema
    }

    /// Adds a property table and returns its index.
    pub fn add_property_table(&mut self, table: PropertyTable) -> i32 {
        self.property_tables.push(table);
        (self.property_tables.len() - 1) as i32
    }

    /// Returns the number of property tables.
    pub fn num_property_tables(&self) -> i32 {
        self.property_tables.len() as i32
    }

    /// Returns a property table by index.
    pub fn property_table(&self, index: i32) -> &PropertyTable {
        &self.property_tables[index as usize]
    }

    /// Returns a mutable property table by index.
    pub fn property_table_mut(&mut self, index: i32) -> &mut PropertyTable {
        &mut self.property_tables[index as usize]
    }

    /// Removes a property table by index.
    pub fn remove_property_table(&mut self, index: i32) {
        self.property_tables.remove(index as usize);
    }

    /// Adds a property attribute and returns its index.
    pub fn add_property_attribute(&mut self, attribute: PropertyAttribute) -> i32 {
        self.property_attributes.push(attribute);
        (self.property_attributes.len() - 1) as i32
    }

    /// Returns the number of property attributes.
    pub fn num_property_attributes(&self) -> i32 {
        self.property_attributes.len() as i32
    }

    /// Returns a property attribute by index.
    pub fn property_attribute(&self, index: i32) -> &PropertyAttribute {
        &self.property_attributes[index as usize]
    }

    /// Returns a mutable property attribute by index.
    pub fn property_attribute_mut(&mut self, index: i32) -> &mut PropertyAttribute {
        &mut self.property_attributes[index as usize]
    }

    /// Removes a property attribute by index.
    pub fn remove_property_attribute(&mut self, index: i32) {
        self.property_attributes.remove(index as usize);
    }
}
