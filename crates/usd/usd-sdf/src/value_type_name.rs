//! SdfValueTypeName - attribute type names.
//!
//! Port of pxr/usd/sdf/valueTypeName.h
//!
//! Represents a value type name, i.e. an attribute's type name. A value type
//! name associates a string with a Rust type and an optional role, along with
//! additional metadata. A schema registers all known value type names and may
//! register multiple names for the same type and role pair (aliases).

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_tf::Token;
use usd_vt::Value;

/// Represents the shape of a value type (or that of an element in an array).
///
/// For scalars, size is 0.
/// For 1D vectors (Vec3), size is 1 and d[0] = 3.
/// For 2D arrays (Matrix3d), size is 2 and d = [3, 3].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TupleDimensions {
    /// Dimension values (up to 2).
    pub d: [usize; 2],
    /// Number of dimensions (0, 1, or 2).
    pub size: usize,
}

impl TupleDimensions {
    /// Creates a scalar (no dimensions).
    pub fn scalar() -> Self {
        Self { d: [0, 0], size: 0 }
    }

    /// Creates a 1D shape (e.g., for Vec3 size would be 3).
    pub fn one_d(m: usize) -> Self {
        Self { d: [m, 0], size: 1 }
    }

    /// Creates a 2D shape (e.g., for Matrix3d size would be (3, 3)).
    pub fn two_d(m: usize, n: usize) -> Self {
        Self { d: [m, n], size: 2 }
    }

    /// Returns true if this is a scalar (no dimensions).
    pub fn is_scalar(&self) -> bool {
        self.size == 0
    }
}

/// Internal implementation data for a value type name.
#[derive(Debug)]
pub struct ValueTypeImpl {
    /// Primary name token.
    pub name: Token,
    /// All aliases for this type.
    pub aliases: Vec<Token>,
    /// C++ type name (e.g., "GfVec3f").
    pub cpp_type_name: String,
    /// Role (e.g., "Point", "Color", "Normal").
    pub role: Token,
    /// Default value.
    pub default_value: Value,
    /// Dimensions of the scalar value.
    pub dimensions: TupleDimensions,
    /// Is this an array type?
    pub is_array: bool,
    /// Scalar version (for array types).
    pub scalar_type: Option<Arc<ValueTypeImpl>>,
    /// Array version (for scalar types).
    pub array_type: Option<Arc<ValueTypeImpl>>,
}

/// Handle to a value type implementation.
pub type ValueTypeImplHandle = Arc<ValueTypeImpl>;

/// Represents a value type name, i.e. an attribute's type name.
///
/// Value type names associate a string name with a Rust type and optional
/// role, along with additional metadata like default values and dimensions.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::ValueTypeName;
///
/// // Get a value type name from the registry
/// let float3 = schema.find_type("float3");
/// assert!(float3.is_valid());
/// assert_eq!(float3.get_role().as_str(), "");
///
/// let point3f = schema.find_type("point3f");
/// assert_eq!(point3f.get_role().as_str(), "Point");
///
/// // Array vs scalar
/// assert!(float3.is_scalar());
/// let float3_array = float3.get_array_type();
/// assert!(float3_array.is_array());
/// ```
#[derive(Clone)]
pub struct ValueTypeName {
    /// Implementation pointer (None for invalid type names).
    pub(crate) impl_: Option<ValueTypeImplHandle>,
}

impl Default for ValueTypeName {
    fn default() -> Self {
        Self::invalid()
    }
}

impl ValueTypeName {
    /// Creates an invalid type name.
    pub fn invalid() -> Self {
        Self { impl_: None }
    }

    /// Creates a value type name from an implementation handle.
    pub(crate) fn new(impl_: ValueTypeImplHandle) -> Self {
        Self { impl_: Some(impl_) }
    }

    /// Returns true if this is a valid (non-empty) type name.
    pub fn is_valid(&self) -> bool {
        self.impl_.is_some()
    }

    /// Returns the type name as a token.
    ///
    /// This should not be used for comparison purposes since aliases may
    /// return different tokens that still represent the same type.
    pub fn as_token(&self) -> Token {
        self.impl_
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_default()
    }

    /// Returns the type name as a token (alias for `as_token()`).
    ///
    /// Matches C++ `GetName()`.
    pub fn name(&self) -> Token {
        self.as_token()
    }

    /// Returns the C++ type name for this type.
    pub fn cpp_type_name(&self) -> &str {
        self.impl_
            .as_ref()
            .map(|i| i.cpp_type_name.as_str())
            .unwrap_or("")
    }

    /// Returns the type's role (e.g., "Point", "Color", "Normal").
    pub fn get_role(&self) -> Token {
        self.impl_
            .as_ref()
            .map(|i| i.role.clone())
            .unwrap_or_default()
    }

    /// Returns the default value for the type.
    pub fn default_value(&self) -> Option<&Value> {
        self.impl_.as_ref().map(|i| &i.default_value)
    }

    /// Returns the scalar version of this type name if it's an array type,
    /// otherwise returns self. If there is no scalar type, returns invalid.
    pub fn scalar_type(&self) -> ValueTypeName {
        self.impl_
            .as_ref()
            .and_then(|i| {
                if i.is_array {
                    i.scalar_type.clone().map(ValueTypeName::new)
                } else {
                    Some(self.clone())
                }
            })
            .unwrap_or_default()
    }

    /// Returns the array version of this type name if it's a scalar type,
    /// otherwise returns self. If there is no array type, returns invalid.
    pub fn array_type(&self) -> ValueTypeName {
        self.impl_
            .as_ref()
            .and_then(|i| {
                if !i.is_array {
                    i.array_type.clone().map(ValueTypeName::new)
                } else {
                    Some(self.clone())
                }
            })
            .unwrap_or_default()
    }

    /// Returns true if this type is a scalar.
    ///
    /// The invalid type is considered neither scalar nor array.
    pub fn is_scalar(&self) -> bool {
        self.impl_.as_ref().is_some_and(|i| !i.is_array)
    }

    /// Returns true if this type is an array.
    ///
    /// The invalid type is considered neither scalar nor array.
    pub fn is_array(&self) -> bool {
        self.impl_.as_ref().is_some_and(|i| i.is_array)
    }

    /// Returns the dimensions of the scalar value (e.g., 3 for Vec3).
    pub fn dimensions(&self) -> TupleDimensions {
        self.impl_
            .as_ref()
            .map(|i| i.dimensions)
            .unwrap_or_default()
    }

    /// Returns the primary alias for this type name as a string.
    ///
    /// Returns the first alias if available, otherwise the type name itself.
    /// Returns None for invalid type names.
    pub fn get_alias(&self) -> Option<String> {
        self.impl_
            .as_ref()
            .map(|i| i.aliases.first().unwrap_or(&i.name).as_str().to_string())
    }

    /// Returns all aliases of the type name as tokens.
    pub fn aliases(&self) -> Vec<Token> {
        self.impl_
            .as_ref()
            .map(|i| i.aliases.clone())
            .unwrap_or_default()
    }

    /// Returns a hash value for this type name.
    pub fn get_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl fmt::Debug for ValueTypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(impl_) = &self.impl_ {
            write!(f, "ValueTypeName({})", impl_.name.as_str())
        } else {
            write!(f, "ValueTypeName(invalid)")
        }
    }
}

impl fmt::Display for ValueTypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_token().as_str())
    }
}

impl PartialEq for ValueTypeName {
    /// C++: operator== compares core type (name) and role, NOT pointer identity.
    /// This ensures equivalent type names from different registries compare equal.
    fn eq(&self, other: &Self) -> bool {
        match (&self.impl_, &other.impl_) {
            (Some(a), Some(b)) => a.name == b.name && a.role == b.role,
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for ValueTypeName {}

impl Hash for ValueTypeName {
    /// C++: hash combines type and role (see GetHash / operator==).
    fn hash<H: Hasher>(&self, state: &mut H) {
        if let Some(impl_) = &self.impl_ {
            Hash::hash(&impl_.name, state);
            Hash::hash(&impl_.role, state);
        } else {
            0usize.hash(state);
        }
    }
}

impl PartialEq<str> for ValueTypeName {
    /// C++: checks all aliases, not just primary name (IsValueIn(aliases, rhs))
    fn eq(&self, other: &str) -> bool {
        match &self.impl_ {
            Some(imp) => imp.aliases.iter().any(|a| a == other),
            None => false,
        }
    }
}

impl PartialEq<Token> for ValueTypeName {
    /// C++: checks all aliases, not just primary name (IsValueIn(aliases, rhs))
    fn eq(&self, other: &Token) -> bool {
        match &self.impl_ {
            Some(imp) => imp.aliases.iter().any(|a| a == other),
            None => false,
        }
    }
}

/// Functor for hashing a ValueTypeName.
pub struct ValueTypeNameHash;

impl ValueTypeNameHash {
    /// Hashes a value type name.
    pub fn hash(x: &ValueTypeName) -> u64 {
        x.get_hash()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid() {
        let invalid = ValueTypeName::invalid();
        assert!(!invalid.is_valid());
        assert!(!invalid.is_scalar());
        assert!(!invalid.is_array());
        assert!(invalid.as_token().is_empty());
    }

    #[test]
    fn test_tuple_dimensions() {
        let scalar = TupleDimensions::scalar();
        assert!(scalar.is_scalar());
        assert_eq!(scalar.size, 0);

        let vec3 = TupleDimensions::one_d(3);
        assert!(!vec3.is_scalar());
        assert_eq!(vec3.size, 1);
        assert_eq!(vec3.d[0], 3);

        let matrix3 = TupleDimensions::two_d(3, 3);
        assert_eq!(matrix3.size, 2);
        assert_eq!(matrix3.d, [3, 3]);
    }

    #[test]
    fn test_equality() {
        let a = ValueTypeName::invalid();
        let b = ValueTypeName::invalid();
        assert_eq!(a, b);

        // Create two type names pointing to same impl
        let impl_ = Arc::new(ValueTypeImpl {
            name: Token::new("float3"),
            aliases: vec![Token::new("float3")],
            cpp_type_name: "GfVec3f".to_string(),
            role: Token::default(),
            default_value: Value::default(),
            dimensions: TupleDimensions::one_d(3),
            is_array: false,
            scalar_type: None,
            array_type: None,
        });
        let c = ValueTypeName::new(impl_.clone());
        let d = ValueTypeName::new(impl_);
        assert_eq!(c, d);
    }
}
