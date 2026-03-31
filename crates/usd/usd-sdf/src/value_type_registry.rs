//! SdfValueTypeRegistry - registry of value type names.
//!
//! Port of pxr/usd/sdf/valueTypeRegistry.h
//!
//! A registry of value type names used by a schema. Provides lookup by name,
//! type, or value, and supports registering new types with aliases.

use crate::value_type_name::{TupleDimensions, ValueTypeImpl, ValueTypeName};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use usd_tf::Token;
use usd_vt::Value;

/// Builder for specifying a value type to register.
pub struct ValueTypeBuilder {
    /// Primary name for the type.
    name: Token,
    /// Default value.
    default_value: Value,
    /// Default array value.
    default_array_value: Option<Value>,
    /// C++ type name.
    cpp_type_name: String,
    /// Array C++ type name.
    array_cpp_type_name: String,
    /// Role (e.g., "Point", "Color").
    role: Token,
    /// Dimensions.
    dimensions: TupleDimensions,
    /// TypeId for the Rust type.
    type_id: Option<TypeId>,
    /// TypeId for the array type.
    array_type_id: Option<TypeId>,
}

impl ValueTypeBuilder {
    /// Creates a new builder with the given name and default value.
    ///
    /// This uses `from_no_hash` internally for float types that don't implement Hash.
    pub fn new<T: Clone + Send + Sync + std::fmt::Debug + PartialEq + 'static>(
        name: impl Into<String>,
        default_value: T,
    ) -> Self {
        let name_str = name.into();
        let cpp_name = std::any::type_name::<T>().to_string();
        Self {
            name: Token::new(&name_str),
            default_value: Value::from_no_hash(default_value),
            default_array_value: Some(Value::from_no_hash(Vec::<T>::new())),
            cpp_type_name: cpp_name.clone(),
            array_cpp_type_name: format!("VtArray<{}>", cpp_name),
            role: Token::default(),
            dimensions: TupleDimensions::scalar(),
            type_id: Some(TypeId::of::<T>()),
            array_type_id: Some(TypeId::of::<Vec<T>>()),
        }
    }

    /// Creates a builder for a type without a default value.
    pub fn from_name(name: impl Into<String>) -> Self {
        Self {
            name: Token::new(&name.into()),
            default_value: Value::default(),
            default_array_value: None,
            cpp_type_name: String::new(),
            array_cpp_type_name: String::new(),
            role: Token::default(),
            dimensions: TupleDimensions::scalar(),
            type_id: None,
            array_type_id: None,
        }
    }

    /// Sets the C++ type name.
    pub fn cpp_type_name(mut self, name: impl Into<String>) -> Self {
        let name_str = name.into();
        self.array_cpp_type_name = format!("VtArray<{}>", name_str);
        self.cpp_type_name = name_str;
        self
    }

    /// Sets the dimensions (shape) of the type.
    pub fn dimensions(mut self, dims: TupleDimensions) -> Self {
        self.dimensions = dims;
        self
    }

    /// Sets the role for this type.
    pub fn role(mut self, role: impl Into<String>) -> Self {
        self.role = Token::new(&role.into());
        self
    }

    /// Indicates that arrays of this type are not supported.
    pub fn no_arrays(mut self) -> Self {
        self.default_array_value = None;
        self.array_cpp_type_name = String::new();
        self.array_type_id = None;
        self
    }
}

/// A registry of value type names.
///
/// This registry maps type names to their implementations and provides
/// lookup by name, Rust type, or value. It also supports registering
/// aliases for types.
pub struct ValueTypeRegistry {
    /// All registered types by name.
    types_by_name: RwLock<HashMap<Token, Arc<ValueTypeImpl>>>,
    /// All registered types by TypeId.
    types_by_type_id: RwLock<HashMap<TypeId, Arc<ValueTypeImpl>>>,
    /// Temporary types for unknown names.
    temp_types: RwLock<HashMap<Token, Arc<ValueTypeImpl>>>,
}

impl Default for ValueTypeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ValueTypeRegistry {
    /// Creates a new empty registry.
    pub fn new() -> Self {
        Self {
            types_by_name: RwLock::new(HashMap::new()),
            types_by_type_id: RwLock::new(HashMap::new()),
            temp_types: RwLock::new(HashMap::new()),
        }
    }

    /// Creates a registry with standard USD types pre-registered.
    pub fn with_standard_types() -> Self {
        let registry = Self::new();
        registry.register_standard_types();
        registry
    }

    /// Registers all standard USD value types.
    fn register_standard_types(&self) {
        // Primitives
        self.add_type(ValueTypeBuilder::new("bool", false));
        self.add_type(ValueTypeBuilder::new("uchar", 0u8).cpp_type_name("unsigned char"));
        self.add_type(ValueTypeBuilder::new("int", 0i32));
        self.add_type(ValueTypeBuilder::new("uint", 0u32).cpp_type_name("unsigned int"));
        self.add_type(ValueTypeBuilder::new("int64", 0i64));
        self.add_type(ValueTypeBuilder::new("uint64", 0u64));
        // Use GfHalf (16-bit) as distinct type from f32, matching C++ GfHalf identity.
        self.add_type(
            ValueTypeBuilder::new("half", usd_gf::half::Half::from_f32(0.0))
                .cpp_type_name("GfHalf"),
        );
        self.add_type(ValueTypeBuilder::new("float", 0.0f32));
        self.add_type(ValueTypeBuilder::new("double", 0.0f64));
        self.add_type(ValueTypeBuilder::new("string", String::new()).cpp_type_name("std::string"));
        self.add_type(ValueTypeBuilder::new("token", Token::default()).cpp_type_name("TfToken"));

        // Asset path
        self.add_type(ValueTypeBuilder::from_name("asset").cpp_type_name("SdfAssetPath"));

        // Vectors (using f32 for simplicity; real impl would have gf types)
        self.add_type(
            ValueTypeBuilder::new("float2", [0.0f32; 2])
                .cpp_type_name("GfVec2f")
                .dimensions(TupleDimensions::one_d(2)),
        );
        self.add_type(
            ValueTypeBuilder::new("float3", [0.0f32; 3])
                .cpp_type_name("GfVec3f")
                .dimensions(TupleDimensions::one_d(3)),
        );
        self.add_type(
            ValueTypeBuilder::new("float4", [0.0f32; 4])
                .cpp_type_name("GfVec4f")
                .dimensions(TupleDimensions::one_d(4)),
        );

        self.add_type(
            ValueTypeBuilder::new("double2", [0.0f64; 2])
                .cpp_type_name("GfVec2d")
                .dimensions(TupleDimensions::one_d(2)),
        );
        self.add_type(
            ValueTypeBuilder::new("double3", [0.0f64; 3])
                .cpp_type_name("GfVec3d")
                .dimensions(TupleDimensions::one_d(3)),
        );
        self.add_type(
            ValueTypeBuilder::new("double4", [0.0f64; 4])
                .cpp_type_name("GfVec4d")
                .dimensions(TupleDimensions::one_d(4)),
        );

        self.add_type(
            ValueTypeBuilder::new("int2", [0i32; 2])
                .cpp_type_name("GfVec2i")
                .dimensions(TupleDimensions::one_d(2)),
        );
        self.add_type(
            ValueTypeBuilder::new("int3", [0i32; 3])
                .cpp_type_name("GfVec3i")
                .dimensions(TupleDimensions::one_d(3)),
        );
        self.add_type(
            ValueTypeBuilder::new("int4", [0i32; 4])
                .cpp_type_name("GfVec4i")
                .dimensions(TupleDimensions::one_d(4)),
        );

        // Half-precision vectors (matching C++ GfVec2h/3h/4h)
        let h0 = usd_gf::half::Half::from_f32(0.0);
        self.add_type(
            ValueTypeBuilder::new("half2", usd_gf::Vec2h::new(h0, h0))
                .cpp_type_name("GfVec2h")
                .dimensions(TupleDimensions::one_d(2)),
        );
        self.add_type(
            ValueTypeBuilder::new("half3", usd_gf::Vec3h::new(h0, h0, h0))
                .cpp_type_name("GfVec3h")
                .dimensions(TupleDimensions::one_d(3)),
        );
        self.add_type(
            ValueTypeBuilder::new("half4", usd_gf::Vec4h::new(h0, h0, h0, h0))
                .cpp_type_name("GfVec4h")
                .dimensions(TupleDimensions::one_d(4)),
        );

        // Roles (point, vector, normal, color)
        self.add_type(
            ValueTypeBuilder::new("point3f", [0.0f32; 3])
                .cpp_type_name("GfVec3f")
                .dimensions(TupleDimensions::one_d(3))
                .role("Point"),
        );
        self.add_type(
            ValueTypeBuilder::new("point3d", [0.0f64; 3])
                .cpp_type_name("GfVec3d")
                .dimensions(TupleDimensions::one_d(3))
                .role("Point"),
        );

        self.add_type(
            ValueTypeBuilder::new("vector3f", [0.0f32; 3])
                .cpp_type_name("GfVec3f")
                .dimensions(TupleDimensions::one_d(3))
                .role("Vector"),
        );
        self.add_type(
            ValueTypeBuilder::new("vector3d", [0.0f64; 3])
                .cpp_type_name("GfVec3d")
                .dimensions(TupleDimensions::one_d(3))
                .role("Vector"),
        );

        self.add_type(
            ValueTypeBuilder::new("normal3f", [0.0f32; 3])
                .cpp_type_name("GfVec3f")
                .dimensions(TupleDimensions::one_d(3))
                .role("Normal"),
        );
        self.add_type(
            ValueTypeBuilder::new("normal3d", [0.0f64; 3])
                .cpp_type_name("GfVec3d")
                .dimensions(TupleDimensions::one_d(3))
                .role("Normal"),
        );

        self.add_type(
            ValueTypeBuilder::new("color3f", [0.0f32; 3])
                .cpp_type_name("GfVec3f")
                .dimensions(TupleDimensions::one_d(3))
                .role("Color"),
        );
        self.add_type(
            ValueTypeBuilder::new("color4f", [0.0f32; 4])
                .cpp_type_name("GfVec4f")
                .dimensions(TupleDimensions::one_d(4))
                .role("Color"),
        );

        // Quaternions
        self.add_type(
            ValueTypeBuilder::new("quath", [h0; 4])
                .cpp_type_name("GfQuath")
                .dimensions(TupleDimensions::one_d(4)),
        );
        self.add_type(
            ValueTypeBuilder::new("quatf", [0.0f32; 4])
                .cpp_type_name("GfQuatf")
                .dimensions(TupleDimensions::one_d(4)),
        );
        self.add_type(
            ValueTypeBuilder::new("quatd", [0.0f64; 4])
                .cpp_type_name("GfQuatd")
                .dimensions(TupleDimensions::one_d(4)),
        );

        // Matrices
        self.add_type(
            ValueTypeBuilder::new("matrix2d", [[0.0f64; 2]; 2])
                .cpp_type_name("GfMatrix2d")
                .dimensions(TupleDimensions::two_d(2, 2)),
        );
        self.add_type(
            ValueTypeBuilder::new("matrix3d", [[0.0f64; 3]; 3])
                .cpp_type_name("GfMatrix3d")
                .dimensions(TupleDimensions::two_d(3, 3)),
        );
        self.add_type(
            ValueTypeBuilder::new("matrix4d", [[0.0f64; 4]; 4])
                .cpp_type_name("GfMatrix4d")
                .dimensions(TupleDimensions::two_d(4, 4)),
        );

        // Frame (4x4 matrix with transform role)
        self.add_type(
            ValueTypeBuilder::new("frame4d", [[0.0f64; 4]; 4])
                .cpp_type_name("GfMatrix4d")
                .dimensions(TupleDimensions::two_d(4, 4))
                .role("Frame"),
        );

        // TexCoord
        self.add_type(
            ValueTypeBuilder::new("texCoord2f", [0.0f32; 2])
                .cpp_type_name("GfVec2f")
                .dimensions(TupleDimensions::one_d(2))
                .role("TextureCoordinate"),
        );
        self.add_type(
            ValueTypeBuilder::new("texCoord3f", [0.0f32; 3])
                .cpp_type_name("GfVec3f")
                .dimensions(TupleDimensions::one_d(3))
                .role("TextureCoordinate"),
        );
    }

    /// Adds a type to the registry.
    pub fn add_type(&self, builder: ValueTypeBuilder) {
        // Create scalar type impl
        let scalar_impl = Arc::new(ValueTypeImpl {
            name: builder.name.clone(),
            aliases: vec![builder.name.clone()],
            cpp_type_name: builder.cpp_type_name.clone(),
            role: builder.role.clone(),
            default_value: builder.default_value.clone(),
            dimensions: builder.dimensions,
            is_array: false,
            scalar_type: None,
            array_type: None,
        });

        // Register scalar type
        {
            let mut by_name = self.types_by_name.write().expect("rwlock poisoned");
            by_name.insert(builder.name.clone(), scalar_impl.clone());
        }

        if let Some(type_id) = builder.type_id {
            let mut by_type = self.types_by_type_id.write().expect("rwlock poisoned");
            by_type.insert(type_id, scalar_impl.clone());
        }

        // Create and register array type if supported
        if let Some(array_value) = builder.default_array_value {
            let array_name = Token::new(&format!("{}[]", builder.name.as_str()));
            let array_impl = Arc::new(ValueTypeImpl {
                name: array_name.clone(),
                aliases: vec![array_name.clone()],
                cpp_type_name: builder.array_cpp_type_name,
                role: builder.role,
                default_value: array_value,
                dimensions: builder.dimensions,
                is_array: true,
                scalar_type: Some(scalar_impl.clone()),
                array_type: None,
            });

            {
                let mut by_name = self.types_by_name.write().expect("rwlock poisoned");
                by_name.insert(array_name, array_impl.clone());
            }

            if let Some(array_type_id) = builder.array_type_id {
                let mut by_type = self.types_by_type_id.write().expect("rwlock poisoned");
                by_type.insert(array_type_id, array_impl);
            }
        }
    }

    /// Returns all registered value type names.
    pub fn get_all_types(&self) -> Vec<ValueTypeName> {
        let by_name = self.types_by_name.read().expect("rwlock poisoned");
        by_name
            .values()
            .map(|i| ValueTypeName::new(i.clone()))
            .collect()
    }

    /// Returns a value type name by name.
    pub fn find_type(&self, name: &str) -> ValueTypeName {
        self.find_type_by_token(&Token::new(name))
    }

    /// Returns a value type name by token.
    pub fn find_type_by_token(&self, name: &Token) -> ValueTypeName {
        let by_name = self.types_by_name.read().expect("rwlock poisoned");
        by_name
            .get(name)
            .map(|i| ValueTypeName::new(i.clone()))
            .unwrap_or_default()
    }

    /// Returns the value type name for the given Rust TypeId and role.
    pub fn find_type_by_type_id(&self, type_id: TypeId, role: Option<&Token>) -> ValueTypeName {
        let by_type = self.types_by_type_id.read().expect("rwlock poisoned");
        if let Some(impl_) = by_type.get(&type_id) {
            // Check role if specified
            if let Some(r) = role {
                if &impl_.role != r {
                    // Try to find a matching role
                    let by_name = self.types_by_name.read().expect("rwlock poisoned");
                    for (_, other) in by_name.iter() {
                        if other.dimensions == impl_.dimensions
                            && &other.role == r
                            && other.is_array == impl_.is_array
                        {
                            return ValueTypeName::new(other.clone());
                        }
                    }
                }
            }
            ValueTypeName::new(impl_.clone())
        } else {
            ValueTypeName::invalid()
        }
    }

    /// Returns a value type name by name, creating a temporary one if not found.
    ///
    /// Use this when you need to ensure the name isn't lost even if the type
    /// isn't registered, typically when writing the name to a file.
    pub fn find_or_create_type_name(&self, name: &Token) -> ValueTypeName {
        // First check registered types
        {
            let by_name = self.types_by_name.read().expect("rwlock poisoned");
            if let Some(impl_) = by_name.get(name) {
                return ValueTypeName::new(impl_.clone());
            }
        }

        // Check temp types
        {
            let temp = self.temp_types.read().expect("rwlock poisoned");
            if let Some(impl_) = temp.get(name) {
                return ValueTypeName::new(impl_.clone());
            }
        }

        // Create temporary type
        let impl_ = Arc::new(ValueTypeImpl {
            name: name.clone(),
            aliases: vec![name.clone()],
            cpp_type_name: name.as_str().to_string(),
            role: Token::default(),
            default_value: Value::default(),
            dimensions: TupleDimensions::scalar(),
            is_array: false,
            scalar_type: None,
            array_type: None,
        });

        let mut temp = self.temp_types.write().expect("rwlock poisoned");
        temp.insert(name.clone(), impl_.clone());
        ValueTypeName::new(impl_)
    }

    /// Empties out the registry.
    pub fn clear(&self) {
        self.types_by_name.write().expect("rwlock poisoned").clear();
        self.types_by_type_id
            .write()
            .expect("rwlock poisoned")
            .clear();
        self.temp_types.write().expect("rwlock poisoned").clear();
    }

    /// Returns the number of registered types.
    pub fn type_count(&self) -> usize {
        self.types_by_name.read().expect("rwlock poisoned").len()
    }
}

impl std::fmt::Debug for ValueTypeRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValueTypeRegistry")
            .field("type_count", &self.type_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = ValueTypeRegistry::new();
        assert_eq!(registry.type_count(), 0);

        let t = registry.find_type("float");
        assert!(!t.is_valid());
    }

    #[test]
    fn test_standard_types() {
        let registry = ValueTypeRegistry::with_standard_types();
        assert!(registry.type_count() > 0);

        let float_type = registry.find_type("float");
        assert!(float_type.is_valid());
        assert!(float_type.is_scalar());
        assert_eq!(float_type.as_token().as_str(), "float");

        let double3 = registry.find_type("double3");
        assert!(double3.is_valid());
        assert_eq!(double3.dimensions().size, 1);
        assert_eq!(double3.dimensions().d[0], 3);

        let point3f = registry.find_type("point3f");
        assert!(point3f.is_valid());
        assert_eq!(point3f.get_role().as_str(), "Point");

        let matrix4d = registry.find_type("matrix4d");
        assert!(matrix4d.is_valid());
        assert_eq!(matrix4d.dimensions().size, 2);
        assert_eq!(matrix4d.dimensions().d, [4, 4]);
    }

    #[test]
    fn test_find_or_create() {
        let registry = ValueTypeRegistry::new();

        // Unknown type gets created as temporary
        let custom = registry.find_or_create_type_name(&Token::new("customType"));
        assert!(custom.is_valid());
        assert_eq!(custom.as_token().as_str(), "customType");

        // Same name returns same temporary
        let custom2 = registry.find_or_create_type_name(&Token::new("customType"));
        assert_eq!(custom, custom2);
    }

    #[test]
    fn test_add_type() {
        let registry = ValueTypeRegistry::new();

        registry.add_type(
            ValueTypeBuilder::new("myFloat", 0.0f32)
                .cpp_type_name("MyFloat")
                .role("Custom"),
        );

        let my_type = registry.find_type("myFloat");
        assert!(my_type.is_valid());
        assert_eq!(my_type.get_role().as_str(), "Custom");
        assert_eq!(my_type.cpp_type_name(), "MyFloat");
    }
}

// ============================================================================
// Global Instance
// ============================================================================

impl ValueTypeRegistry {
    /// Returns the global value type registry instance with standard types.
    ///
    /// Matches C++ `SdfSchema::GetInstance().FindType()` pattern.
    /// This provides a singleton registry with all standard USD types
    /// pre-registered.
    pub fn instance() -> &'static ValueTypeRegistry {
        static INSTANCE: OnceLock<ValueTypeRegistry> = OnceLock::new();
        INSTANCE.get_or_init(ValueTypeRegistry::with_standard_types)
    }
}
