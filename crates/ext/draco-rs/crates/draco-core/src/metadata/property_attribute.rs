//! Property attribute metadata (EXT_structural_metadata).
//!
//! What: Describes where per-point properties live as named attributes.
//! Why: Mirrors Draco C++ structural metadata so glTF property attributes can be
//! serialized/deserialized and attached to meshes.
//! How: Stores a name, schema class, and a list of property descriptors that
//! map property names to glTF attribute names.
//! Where used: Referenced by `StructuralMetadata` and (later) mesh metadata IO.

/// Property attribute descriptor for EXT_structural_metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyAttribute {
    name: String,
    class_name: String,
    properties: Vec<Property>,
}

impl PropertyAttribute {
    /// Creates an empty property attribute.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `src` into this attribute.
    pub fn copy_from(&mut self, src: &PropertyAttribute) {
        self.name = src.name.clone();
        self.class_name = src.class_name.clone();
        self.properties.clear();
        self.properties.extend(src.properties.iter().cloned());
    }

    /// Sets the display name for this property attribute.
    pub fn set_name(&mut self, value: &str) {
        self.name = value.to_string();
    }

    /// Returns the display name for this property attribute.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets the schema class name that this attribute conforms to.
    pub fn set_class(&mut self, value: &str) {
        self.class_name = value.to_string();
    }

    /// Returns the schema class name for this attribute.
    pub fn class(&self) -> &str {
        &self.class_name
    }

    /// Adds a property descriptor and returns its index.
    pub fn add_property(&mut self, property: Property) -> i32 {
        self.properties.push(property);
        (self.properties.len() - 1) as i32
    }

    /// Returns the number of properties attached to this attribute.
    pub fn num_properties(&self) -> i32 {
        self.properties.len() as i32
    }

    /// Returns a property descriptor by index.
    pub fn property(&self, index: i32) -> &Property {
        &self.properties[index as usize]
    }

    /// Returns a mutable property descriptor by index.
    pub fn property_mut(&mut self, index: i32) -> &mut Property {
        &mut self.properties[index as usize]
    }

    /// Removes a property descriptor by index.
    pub fn remove_property(&mut self, index: i32) {
        self.properties.remove(index as usize);
    }
}

/// Describes a single property and its backing attribute name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Property {
    name: String,
    attribute_name: String,
}

impl Property {
    /// Creates an empty property descriptor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `src` into this property.
    pub fn copy_from(&mut self, src: &Property) {
        self.name = src.name.clone();
        self.attribute_name = src.attribute_name.clone();
    }

    /// Sets the property name as defined in the schema class.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Returns the schema property name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets the name of the glTF attribute containing property values.
    pub fn set_attribute_name(&mut self, name: &str) {
        self.attribute_name = name.to_string();
    }

    /// Returns the name of the glTF attribute containing property values.
    pub fn attribute_name(&self) -> &str {
        &self.attribute_name
    }
}
