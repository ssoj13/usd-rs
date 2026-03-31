//! Spec type registration and runtime casting.
//!
//! Port of pxr/usd/sdf/specType.h
//!
//! Provides functions to register spec types with the runtime typing system
//! used to cast between Rust spec types. This allows dynamic dispatch and
//! type-safe conversion between different spec representations.

use crate::SpecType;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::RwLock;

use once_cell::sync::Lazy;

/// Registration entry for a concrete spec type.
#[derive(Debug, Clone)]
struct SpecTypeEntry {
    /// The Rust TypeId of the spec implementation.
    spec_type_id: TypeId,
    /// The SDF spec type enum value.
    spec_enum: SpecType,
    /// The Rust TypeId of the schema that owns this spec type.
    schema_type_id: TypeId,
}

/// Global registry mapping TypeId -> SpecTypeEntry.
static SPEC_TYPE_REGISTRY: Lazy<RwLock<SpecTypeRegistry>> =
    Lazy::new(|| RwLock::new(SpecTypeRegistry::new()));

/// Registry for spec type information.
struct SpecTypeRegistry {
    /// Map from spec implementation TypeId to registration entry.
    by_type: HashMap<TypeId, SpecTypeEntry>,
    /// Map from SpecType enum to list of registered type IDs.
    by_spec_type: HashMap<SpecType, Vec<TypeId>>,
}

impl SpecTypeRegistry {
    fn new() -> Self {
        Self {
            by_type: HashMap::new(),
            by_spec_type: HashMap::new(),
        }
    }
}

/// Provides functions to register spec types with the runtime typing system.
///
/// For a concrete spec type that corresponds to a specific SpecType:
/// ```ignore
/// register_spec_type::<MySchema, MyPrimSpec>(SpecType::Prim);
/// ```
///
/// For an abstract spec type with no corresponding SpecType:
/// ```ignore
/// register_abstract_spec_type::<MySchema, MyPropertySpec>();
/// ```
pub struct SpecTypeRegistration;

impl SpecTypeRegistration {
    /// Registers a concrete spec type associated with a specific `SpecType` enum.
    pub fn register_spec_type<Schema: 'static, Spec: 'static>(spec_type_enum: SpecType) {
        let entry = SpecTypeEntry {
            spec_type_id: TypeId::of::<Spec>(),
            spec_enum: spec_type_enum,
            schema_type_id: TypeId::of::<Schema>(),
        };
        let mut registry = SPEC_TYPE_REGISTRY.write();
        let type_id = TypeId::of::<Spec>();
        registry
            .by_spec_type
            .entry(spec_type_enum)
            .or_default()
            .push(type_id);
        registry.by_type.insert(type_id, entry);
    }

    /// Registers an abstract spec type (no corresponding SpecType enum).
    pub fn register_abstract_spec_type<Schema: 'static, Spec: 'static>() {
        let entry = SpecTypeEntry {
            spec_type_id: TypeId::of::<Spec>(),
            spec_enum: SpecType::Unknown,
            schema_type_id: TypeId::of::<Schema>(),
        };
        let mut registry = SPEC_TYPE_REGISTRY.write();
        registry.by_type.insert(TypeId::of::<Spec>(), entry);
    }
}

/// Runtime spec type information for casting.
pub struct SpecTypeCast;

impl SpecTypeCast {
    /// Returns true if a spec with the given SpecType can be represented
    /// by the Rust type identified by `target_type`.
    pub fn can_cast_from_spec_type(from: SpecType, target_type: TypeId) -> bool {
        let registry = SPEC_TYPE_REGISTRY.read();
        if let Some(entry) = registry.by_type.get(&target_type) {
            // Abstract types (Unknown) can represent anything.
            if entry.spec_enum == SpecType::Unknown {
                return true;
            }
            entry.spec_enum == from
        } else {
            false
        }
    }

    /// Returns true if the given spec type enum has a registered Rust type.
    pub fn is_registered(spec_type: SpecType) -> bool {
        let registry = SPEC_TYPE_REGISTRY.read();
        registry.by_spec_type.contains_key(&spec_type)
    }

    /// Returns the SpecType enum for a registered Rust type, or Unknown.
    pub fn get_spec_type_for<T: 'static>() -> SpecType {
        let registry = SPEC_TYPE_REGISTRY.read();
        registry
            .by_type
            .get(&TypeId::of::<T>())
            .map(|e| e.spec_enum)
            .unwrap_or(SpecType::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSchema;
    struct TestPrimSpec;

    #[test]
    fn test_register_and_lookup() {
        SpecTypeRegistration::register_spec_type::<TestSchema, TestPrimSpec>(SpecType::Prim);

        assert_eq!(
            SpecTypeCast::get_spec_type_for::<TestPrimSpec>(),
            SpecType::Prim
        );
        assert!(SpecTypeCast::can_cast_from_spec_type(
            SpecType::Prim,
            TypeId::of::<TestPrimSpec>()
        ));
        assert!(!SpecTypeCast::can_cast_from_spec_type(
            SpecType::Attribute,
            TypeId::of::<TestPrimSpec>()
        ));
    }
}
