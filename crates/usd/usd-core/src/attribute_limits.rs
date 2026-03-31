//! UsdAttributeLimits - Attribute value constraints.
//!
//! Provides API for retrieving and authoring values within a particular
//! sub-dictionary of the `limits` dictionary metadata field on a UsdAttribute.

use super::attribute::Attribute;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

// ============================================================================
// Limits Keys
// ============================================================================

/// Well-known keys used in limits dictionaries.
pub mod limits_keys {
    use usd_tf::Token;

    /// Key for soft limits sub-dictionary.
    pub fn soft() -> Token {
        Token::new("soft")
    }

    /// Key for hard limits sub-dictionary.
    pub fn hard() -> Token {
        Token::new("hard")
    }

    /// Key for minimum value within a limits sub-dictionary.
    pub fn minimum() -> Token {
        Token::new("minimum")
    }

    /// Key for maximum value within a limits sub-dictionary.
    pub fn maximum() -> Token {
        Token::new("maximum")
    }
}

// ============================================================================
// ValidationResult
// ============================================================================

/// Validation information for a limits sub-dictionary.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// Whether validation succeeded.
    success: bool,
    /// Values that did not match the attribute's type.
    invalid_values: Dictionary,
    /// The conformed sub-dictionary with casted values.
    conformed_sub_dict: Dictionary,
    /// Path of the attribute being validated.
    attr_path: String,
    /// Type name of the attribute.
    attr_type_name: String,
}

impl ValidationResult {
    /// Returns whether validation was successful.
    pub fn success(&self) -> bool {
        self.success
    }

    /// Returns dictionary containing invalid values.
    pub fn invalid_values_dict(&self) -> &Dictionary {
        &self.invalid_values
    }

    /// Returns the conformed limits sub-dictionary.
    pub fn conformed_sub_dict(&self) -> &Dictionary {
        &self.conformed_sub_dict
    }

    /// Returns formatted error string describing invalid values.
    ///
    /// Matches C++ `ValidationResult::GetErrorString()`.
    pub fn error_string(&self) -> String {
        if self.invalid_values.is_empty() {
            return String::new();
        }

        let mut keys_and_types = String::new();
        for (key, value) in self.invalid_values.iter() {
            if !keys_and_types.is_empty() {
                keys_and_types.push_str(", ");
            }
            keys_and_types.push_str(&format!(
                "{} ({})",
                key,
                value.type_name().unwrap_or("unknown")
            ));
        }

        format!(
            "{} limits value key(s) have an unexpected type for attribute <{}> \
             (expected {}): {}",
            self.invalid_values.len(),
            self.attr_path,
            self.attr_type_name,
            keys_and_types
        )
    }
}

// ============================================================================
// Type matching/casting helpers for validation
// ============================================================================

/// Checks if a Value's Rust type name matches an SDF type name.
fn value_type_matches(rust_type: &str, sdf_type: &str) -> bool {
    match sdf_type {
        "int" => rust_type == "i32",
        "uint" => rust_type == "u32",
        "int64" => rust_type == "i64",
        "uint64" => rust_type == "u64",
        "half" | "float" => rust_type == "f32",
        "double" => rust_type == "f64",
        "bool" => rust_type == "bool",
        "string" => rust_type == "String" || rust_type == "alloc::string::String",
        "token" => rust_type == "Token",
        _ => rust_type == sdf_type,
    }
}

/// Tries to cast a Value to match the expected SDF type (e.g. f64->i32, bool->i32).
fn try_cast_value(value: &Value, sdf_type: &str) -> Option<Value> {
    match sdf_type {
        "int" => {
            if let Some(&v) = value.get::<f64>() {
                return Some(Value::from(v as i32));
            }
            if let Some(&v) = value.get::<f32>() {
                return Some(Value::from(v as i32));
            }
            if let Some(&v) = value.get::<bool>() {
                return Some(Value::from(v as i32));
            }
            if let Some(&v) = value.get::<i64>() {
                return Some(Value::from(v as i32));
            }
            None
        }
        "double" => {
            if let Some(&v) = value.get::<f32>() {
                return Some(Value::from(v as f64));
            }
            if let Some(&v) = value.get::<i32>() {
                return Some(Value::from(v as f64));
            }
            if let Some(&v) = value.get::<bool>() {
                return Some(Value::from(if v { 1.0f64 } else { 0.0f64 }));
            }
            None
        }
        "float" | "half" => {
            if let Some(&v) = value.get::<f64>() {
                return Some(Value::from(v as f32));
            }
            if let Some(&v) = value.get::<i32>() {
                return Some(Value::from(v as f32));
            }
            if let Some(&v) = value.get::<bool>() {
                return Some(Value::from(if v { 1.0f32 } else { 0.0f32 }));
            }
            None
        }
        "int64" => {
            if let Some(&v) = value.get::<i32>() {
                return Some(Value::from(v as i64));
            }
            if let Some(&v) = value.get::<bool>() {
                return Some(Value::from(v as i64));
            }
            None
        }
        _ => None,
    }
}

// ============================================================================
// UsdAttributeLimits
// ============================================================================

/// Provides API for attribute value constraints (limits metadata).
///
/// Within a given sub-dictionary, minimum and maximum values are encoded under
/// the `minimum` and `maximum` keys. Typical use involves soft limits
/// (suggested value range) and hard limits (enforced value range).
///
/// # Example
///
/// ```ignore
/// // In USD:
/// // int attr = 7 (
/// //     limits = {
/// //         dictionary soft = { int minimum = 5, int maximum = 10 }
/// //         dictionary hard = { int minimum = 0, int maximum = 15 }
/// //     }
/// // )
///
/// let soft = attr.get_soft_limits();
/// let min: Option<i32> = soft.get_minimum();
/// let max: Option<i32> = soft.get_maximum();
/// ```
#[derive(Clone)]
pub struct AttributeLimits {
    /// The attribute this limits object refers to.
    attr: Attribute,
    /// The sub-dictionary key (e.g., "soft", "hard").
    sub_dict_key: Token,
}

impl std::fmt::Debug for AttributeLimits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttributeLimits")
            .field("attr", &self.attr.path())
            .field("sub_dict_key", &self.sub_dict_key)
            .finish()
    }
}

impl AttributeLimits {
    /// Creates an invalid limits object.
    pub fn invalid() -> Self {
        Self {
            attr: Attribute::invalid(),
            sub_dict_key: Token::new(""),
        }
    }

    /// Creates a limits object for the given attribute and sub-dictionary key.
    pub fn new(attr: &Attribute, sub_dict_key: &Token) -> Self {
        Self {
            attr: attr.clone(),
            sub_dict_key: sub_dict_key.clone(),
        }
    }

    /// Returns whether the limits object is valid.
    pub fn is_valid(&self) -> bool {
        self.attr.is_valid() && !self.sub_dict_key.is_empty()
    }

    /// Returns the attribute this limits object refers to.
    pub fn attribute(&self) -> &Attribute {
        &self.attr
    }

    /// Returns the sub-dictionary key.
    pub fn sub_dict_key(&self) -> &Token {
        &self.sub_dict_key
    }

    // =========================================================================
    // Nested dict helpers
    // =========================================================================

    /// Gets the limits metadata dictionary.
    fn get_limits_dict(&self) -> Option<Dictionary> {
        let limits_key = Token::new("limits");
        self.attr
            .get_metadata(&limits_key)
            .and_then(|v| v.get::<Dictionary>().cloned())
    }

    /// Gets the sub-dictionary for this limits object.
    fn get_sub_dict(&self) -> Option<Dictionary> {
        self.get_limits_dict().and_then(|limits| {
            limits
                .get(self.sub_dict_key.get_text())
                .and_then(|v| v.get::<Dictionary>().cloned())
        })
    }

    /// Writes the limits dict back to metadata.
    fn set_limits_dict(&self, limits: Dictionary) -> bool {
        let limits_key = Token::new("limits");
        self.attr.set_metadata(&limits_key, limits)
    }

    // =========================================================================
    // Authored opinions API
    // =========================================================================

    /// Returns whether any authored opinions exist for this limits sub-dictionary.
    pub fn has_authored(&self) -> bool {
        if !self.is_valid() {
            return false;
        }
        self.get_limits_dict()
            .map(|d| d.contains_key(self.sub_dict_key.get_text()))
            .unwrap_or(false)
    }

    /// Clears all authored opinions for this limits sub-dictionary.
    pub fn clear(&self) -> bool {
        if !self.is_valid() {
            return false;
        }

        if let Some(mut limits) = self.get_limits_dict() {
            limits.remove(self.sub_dict_key.get_text());
            // If limits dict is now empty, clear the whole "limits" metadata
            if limits.is_empty() {
                let limits_key = Token::new("limits");
                return self.attr.clear_metadata(&limits_key);
            }
            return self.set_limits_dict(limits);
        }
        true
    }

    /// Returns whether an authored opinion exists for the given key.
    pub fn has_authored_key(&self, key: &Token) -> bool {
        if !self.is_valid() || key.is_empty() {
            return false;
        }
        self.get_sub_dict()
            .map(|d| d.contains_key(key.get_text()))
            .unwrap_or(false)
    }

    /// Clears the authored opinion for the given key.
    pub fn clear_key(&self, key: &Token) -> bool {
        if !self.is_valid() || key.is_empty() {
            return false;
        }

        let Some(mut limits) = self.get_limits_dict() else {
            return true;
        };
        let sub_key = self.sub_dict_key.get_text();
        let Some(sub_val) = limits.get(sub_key) else {
            return true;
        };
        let Some(sub_dict) = sub_val.get::<Dictionary>() else {
            return true;
        };
        let mut sub_dict = sub_dict.clone();
        sub_dict.remove(key.get_text());
        if sub_dict.is_empty() {
            limits.remove(sub_key);
            if limits.is_empty() {
                let limits_key = Token::new("limits");
                return self.attr.clear_metadata(&limits_key);
            }
        } else {
            limits.insert(sub_key, Value::from(sub_dict));
        }
        self.set_limits_dict(limits)
    }

    /// Returns whether an authored minimum value exists.
    pub fn has_authored_minimum(&self) -> bool {
        self.has_authored_key(&limits_keys::minimum())
    }

    /// Clears the authored minimum value.
    pub fn clear_minimum(&self) -> bool {
        self.clear_key(&limits_keys::minimum())
    }

    /// Returns whether an authored maximum value exists.
    pub fn has_authored_maximum(&self) -> bool {
        self.has_authored_key(&limits_keys::maximum())
    }

    /// Clears the authored maximum value.
    pub fn clear_maximum(&self) -> bool {
        self.clear_key(&limits_keys::maximum())
    }

    // =========================================================================
    // Validation
    // =========================================================================

    /// Validates a limits sub-dictionary.
    ///
    /// To be valid, minimum and maximum value types must match the attribute's
    /// value type. Matches C++ `UsdAttributeLimits::Validate()`.
    pub fn validate(&self, sub_dict: &Dictionary) -> ValidationResult {
        let mut result = ValidationResult::default();

        if !self.is_valid() {
            return result;
        }

        let attr_type_name = self.attr.type_name();
        result.attr_path = self.attr.path().to_string();
        result.attr_type_name = attr_type_name.get_text().to_string();

        let mut invalid_values = Dictionary::new();
        let mut conformed = sub_dict.clone();

        let min_token = limits_keys::minimum();
        let max_token = limits_keys::maximum();
        let min_key = min_token.get_text();
        let max_key = max_token.get_text();

        for (key, value) in sub_dict.iter() {
            // Only validate min/max entries
            if key != min_key && key != max_key {
                continue;
            }
            // Check if value type matches attribute type name
            let value_type = value.type_name().unwrap_or("");
            let expected = attr_type_name.get_text();
            // Map sdf type names to Rust value type names for comparison
            let matches = value_type_matches(value_type, expected);
            if !matches {
                // Try to cast (e.g. bool→int, f64→i32)
                if let Some(casted) = try_cast_value(value, expected) {
                    conformed.insert(key, casted);
                } else {
                    invalid_values.insert(key, value.clone());
                }
            }
        }

        let success = invalid_values.is_empty();
        result.success = success;
        result.invalid_values = invalid_values;
        result.conformed_sub_dict = if success {
            conformed
        } else {
            Dictionary::new()
        };
        result
    }

    /// Sets the entire limits sub-dictionary.
    ///
    /// Validates min/max types first. Matches C++ `Set(const VtDictionary&)`.
    pub fn set_dict(&self, sub_dict: &Dictionary) -> bool {
        if !self.is_valid() {
            return false;
        }

        let validation = self.validate(sub_dict);
        if !validation.success() {
            eprintln!("{}", validation.error_string());
            return false;
        }

        let mut limits = self.get_limits_dict().unwrap_or_default();
        limits.insert(
            self.sub_dict_key.get_text(),
            Value::from(validation.conformed_sub_dict.clone()),
        );
        self.set_limits_dict(limits)
    }

    // =========================================================================
    // Value retrieval
    // =========================================================================

    /// Returns the value for the given key.
    pub fn get(&self, key: &Token) -> Option<Value> {
        if !self.is_valid() || key.is_empty() {
            return None;
        }
        self.get_sub_dict()
            .and_then(|d| d.get(key.get_text()).cloned())
    }

    /// Returns the value for the given key, or a default.
    pub fn get_or<T: Clone + 'static>(&self, key: &Token, default: T) -> T {
        if let Some(value) = self.get(key) {
            if let Some(v) = value.get::<T>() {
                return v.clone();
            }
        }
        default
    }

    /// Sets the value for the given key.
    ///
    /// For minimum/maximum keys, the value type must match the attribute's
    /// value type. Matches C++ `Set(const TfToken& key, const VtValue& value)`.
    pub fn set(&self, key: &Token, value: Value) -> bool {
        if !self.is_valid() || key.is_empty() {
            return false;
        }

        // For min/max, verify type matches attribute's value type
        let min_key = limits_keys::minimum();
        let max_key = limits_keys::maximum();
        if *key == min_key || *key == max_key {
            let attr_type = self.attr.type_name();
            let value_type = value.type_name().unwrap_or("");
            if !value_type_matches(value_type, attr_type.get_text()) {
                eprintln!(
                    "Unexpected limits value type ({}) for attribute '{}' (expected {})",
                    value_type,
                    self.attr.path(),
                    attr_type.get_text()
                );
                return false;
            }
        }

        let mut limits = self.get_limits_dict().unwrap_or_default();
        let sub_key = self.sub_dict_key.get_text();
        let mut sub_dict = limits
            .get(sub_key)
            .and_then(|v| v.get::<Dictionary>().cloned())
            .unwrap_or_default();
        sub_dict.insert(key.get_text(), value);
        limits.insert(sub_key, Value::from(sub_dict));
        self.set_limits_dict(limits)
    }

    /// Returns the minimum value.
    pub fn get_minimum(&self) -> Option<Value> {
        self.get(&limits_keys::minimum())
    }

    /// Returns the minimum value, or default.
    pub fn get_minimum_or<T: Clone + 'static>(&self, default: T) -> T {
        self.get_or(&limits_keys::minimum(), default)
    }

    /// Sets the minimum value.
    pub fn set_minimum(&self, value: Value) -> bool {
        self.set(&limits_keys::minimum(), value)
    }

    /// Returns the maximum value.
    pub fn get_maximum(&self) -> Option<Value> {
        self.get(&limits_keys::maximum())
    }

    /// Returns the maximum value, or default.
    pub fn get_maximum_or<T: Clone + 'static>(&self, default: T) -> T {
        self.get_or(&limits_keys::maximum(), default)
    }

    /// Sets the maximum value.
    pub fn set_maximum(&self, value: Value) -> bool {
        self.set(&limits_keys::maximum(), value)
    }
}

impl Default for AttributeLimits {
    fn default() -> Self {
        Self::invalid()
    }
}

impl PartialEq for AttributeLimits {
    fn eq(&self, other: &Self) -> bool {
        self.attr == other.attr && self.sub_dict_key == other.sub_dict_key
    }
}

impl Eq for AttributeLimits {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_limits() {
        let limits = AttributeLimits::invalid();
        assert!(!limits.is_valid());
        assert!(!limits.has_authored());
        assert!(limits.get_minimum().is_none());
    }

    #[test]
    fn test_limits_keys() {
        assert_eq!(limits_keys::soft().get_text(), "soft");
        assert_eq!(limits_keys::hard().get_text(), "hard");
        assert_eq!(limits_keys::minimum().get_text(), "minimum");
        assert_eq!(limits_keys::maximum().get_text(), "maximum");
    }

    #[test]
    fn test_validation_result_default() {
        let result = ValidationResult::default();
        assert!(!result.success());
        assert!(result.error_string().is_empty());
    }
}
