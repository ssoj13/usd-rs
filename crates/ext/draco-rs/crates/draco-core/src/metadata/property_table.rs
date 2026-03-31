//! Property table metadata (EXT_structural_metadata).
//!
//! What: Stores columnar property data for structured metadata tables.
//! Why: Mirrors Draco C++ PropertyTable to preserve EXT_structural_metadata
//! semantics across encode/decode and mesh copies.
//! How: Keeps a name, schema class, row count, and a list of properties with
//! raw data and optional offsets for arrays/strings.
//! Where used: Referenced by `StructuralMetadata` and (later) glTF IO.

use crate::core::status::{Status, StatusCode};
use crate::core::status_or::StatusOr;

/// Property table descriptor for EXT_structural_metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PropertyTable {
    name: String,
    class_name: String,
    count: i32,
    properties: Vec<Property>,
}

impl PropertyTable {
    /// Creates an empty property table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `src` into this table.
    pub fn copy_from(&mut self, src: &PropertyTable) {
        self.name = src.name.clone();
        self.class_name = src.class_name.clone();
        self.count = src.count;
        self.properties.clear();
        self.properties.extend(src.properties.iter().cloned());
    }

    /// Sets the display name of the table.
    pub fn set_name(&mut self, value: &str) {
        self.name = value.to_string();
    }

    /// Returns the display name of the table.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets the schema class name for this table.
    pub fn set_class(&mut self, value: &str) {
        self.class_name = value.to_string();
    }

    /// Returns the schema class name for this table.
    pub fn class(&self) -> &str {
        &self.class_name
    }

    /// Sets the row count for the table.
    pub fn set_count(&mut self, count: i32) {
        self.count = count;
    }

    /// Returns the row count for the table.
    pub fn count(&self) -> i32 {
        self.count
    }

    /// Adds a property (column) and returns its index.
    pub fn add_property(&mut self, property: Property) -> i32 {
        self.properties.push(property);
        (self.properties.len() - 1) as i32
    }

    /// Returns the number of properties (columns).
    pub fn num_properties(&self) -> i32 {
        self.properties.len() as i32
    }

    /// Returns a property by index.
    pub fn property(&self, index: i32) -> &Property {
        &self.properties[index as usize]
    }

    /// Returns a mutable property by index.
    pub fn property_mut(&mut self, index: i32) -> &mut Property {
        &mut self.properties[index as usize]
    }

    /// Removes a property by index.
    pub fn remove_property(&mut self, index: i32) {
        self.properties.remove(index as usize);
    }
}

/// Describes a property (column) within a property table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Property {
    name: String,
    data: Data,
    array_offsets: Offsets,
    string_offsets: Offsets,
}

impl Property {
    /// Creates an empty property.
    pub fn new() -> Self {
        Self::default()
    }

    /// Copies all data from `src` into this property.
    pub fn copy_from(&mut self, src: &Property) {
        self.name = src.name.clone();
        self.data = src.data.clone();
        self.array_offsets = src.array_offsets.clone();
        self.string_offsets = src.string_offsets.clone();
    }

    /// Sets the property name.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Returns the property name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns mutable property data (raw buffer view payload).
    pub fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }

    /// Returns property data (raw buffer view payload).
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// Returns array offsets for variable-length numeric arrays.
    pub fn array_offsets(&self) -> &Offsets {
        &self.array_offsets
    }

    /// Returns mutable array offsets for variable-length numeric arrays.
    pub fn array_offsets_mut(&mut self) -> &mut Offsets {
        &mut self.array_offsets
    }

    /// Returns string offsets for string-valued properties.
    pub fn string_offsets(&self) -> &Offsets {
        &self.string_offsets
    }

    /// Returns mutable string offsets for string-valued properties.
    pub fn string_offsets_mut(&mut self) -> &mut Offsets {
        &mut self.string_offsets
    }
}

/// Raw buffer view data associated with a property.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Data {
    /// Raw byte payload.
    pub data: Vec<u8>,
    /// BufferView target (glTF semantics), defaults to 0.
    pub target: i32,
}

/// Offsets describing arrays or strings inside property data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Offsets {
    /// Offset payload buffer.
    pub data: Data,
    /// Offset element type name (UINT8/UINT16/UINT32/UINT64).
    pub type_name: String,
}

impl Offsets {
    /// Builds offsets from integer values, choosing the smallest valid type.
    pub fn make_from_ints(ints: &[u64]) -> Offsets {
        let mut max_value = 0u64;
        for &value in ints {
            if value > max_value {
                max_value = value;
            }
        }

        let (type_name, bytes_per_int) = if max_value <= u8::MAX as u64 {
            ("UINT8".to_string(), 1usize)
        } else if max_value <= u16::MAX as u64 {
            ("UINT16".to_string(), 2usize)
        } else if max_value <= u32::MAX as u64 {
            ("UINT32".to_string(), 4usize)
        } else {
            ("UINT64".to_string(), 8usize)
        };

        let mut data = vec![0u8; ints.len() * bytes_per_int];
        for (i, &value) in ints.iter().enumerate() {
            let bytes = value.to_le_bytes();
            let start = i * bytes_per_int;
            data[start..start + bytes_per_int].copy_from_slice(&bytes[..bytes_per_int]);
        }

        Offsets {
            data: Data { data, target: 0 },
            type_name,
        }
    }

    /// Parses offsets back into integer values.
    pub fn parse_to_ints(&self) -> StatusOr<Vec<u64>> {
        if self.data.data.is_empty() {
            return StatusOr::new_value(Vec::new());
        }

        let bytes_per_int = match self.type_name.as_str() {
            "UINT8" => 1usize,
            "UINT16" => 2usize,
            "UINT32" => 4usize,
            "UINT64" => 8usize,
            _ => {
                return StatusOr::new_status(Status::new(
                    StatusCode::DracoError,
                    "Offsets data type invalid",
                ))
            }
        };

        let count = self.data.data.len() / bytes_per_int;
        let mut result = vec![0u64; count];
        for i in 0..count {
            let start = i * bytes_per_int;
            let mut tmp = [0u8; 8];
            tmp[..bytes_per_int].copy_from_slice(&self.data.data[start..start + bytes_per_int]);
            result[i] = u64::from_le_bytes(tmp);
        }
        StatusOr::new_value(result)
    }
}
