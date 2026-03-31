//! JSON Value types.
//!
//! This module provides the core JSON value type `JsValue`, which is a
//! discriminated union that can hold any JSON value type.

use std::collections::BTreeMap;
use std::fmt;

/// A JSON object (dictionary with string keys).
pub type JsObject = BTreeMap<String, JsValue>;

/// A JSON array.
pub type JsArray = Vec<JsValue>;

/// A discriminated union type for JSON values.
///
/// `JsValue` can hold one of the following types:
/// - `Object` - a dictionary (map from string to JsValue)
/// - `Array` - a vector of JsValues
/// - `String` - a string
/// - `Bool` - a boolean
/// - `Int` - a 64-bit signed integer
/// - `Real` - a 64-bit floating point number
/// - `Null` - null value
///
/// # Examples
///
/// ```
/// use usd_js::JsValue;
///
/// let int_val = JsValue::from(42);
/// assert!(int_val.is_int());
/// assert_eq!(int_val.as_i64(), Some(42));
///
/// let str_val = JsValue::from("hello");
/// assert!(str_val.is_string());
/// assert_eq!(str_val.as_string(), Some("hello"));
/// ```
#[derive(Clone, PartialEq, Default)]
pub enum JsValue {
    /// A JSON object (dictionary).
    Object(JsObject),
    /// A JSON array.
    Array(JsArray),
    /// A JSON string.
    String(String),
    /// A JSON boolean.
    Bool(bool),
    /// A JSON integer (stored as i64).
    Int(i64),
    /// A JSON unsigned integer (stored as u64).
    UInt(u64),
    /// A JSON real number (stored as f64).
    Real(f64),
    /// A JSON null value.
    #[default]
    Null,
}

/// Type of JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsType {
    /// Object type.
    Object,
    /// Array type.
    Array,
    /// String type.
    String,
    /// Boolean type.
    Bool,
    /// Integer type.
    Int,
    /// Real (float) type.
    Real,
    /// Null type.
    Null,
}

impl JsValue {
    /// Creates a null value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_js::JsValue;
    ///
    /// let v = JsValue::null();
    /// assert!(v.is_null());
    /// ```
    #[inline]
    #[must_use]
    pub const fn null() -> Self {
        Self::Null
    }

    /// Returns the type of this value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_js::{JsValue, value::JsType};
    ///
    /// let v = JsValue::from(42);
    /// assert_eq!(v.get_type(), JsType::Int);
    /// ```
    #[must_use]
    pub fn get_type(&self) -> JsType {
        match self {
            Self::Object(_) => JsType::Object,
            Self::Array(_) => JsType::Array,
            Self::String(_) => JsType::String,
            Self::Bool(_) => JsType::Bool,
            Self::Int(_) | Self::UInt(_) => JsType::Int,
            Self::Real(_) => JsType::Real,
            Self::Null => JsType::Null,
        }
    }

    /// Returns the type name as a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_js::JsValue;
    ///
    /// let v = JsValue::from(42);
    /// assert_eq!(v.type_name(), "int");
    /// ```
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Object(_) => "object",
            Self::Array(_) => "array",
            Self::String(_) => "string",
            Self::Bool(_) => "bool",
            Self::Int(_) | Self::UInt(_) => "int",
            Self::Real(_) => "real",
            Self::Null => "null",
        }
    }

    /// Returns true if this value is an object.
    #[inline]
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// Returns true if this value is an array.
    #[inline]
    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// Returns true if this value is a string.
    #[inline]
    #[must_use]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// Returns true if this value is a boolean.
    #[inline]
    #[must_use]
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Returns true if this value is an integer.
    #[inline]
    #[must_use]
    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int(_) | Self::UInt(_))
    }

    /// Returns true if this value is a real number.
    #[inline]
    #[must_use]
    pub fn is_real(&self) -> bool {
        matches!(self, Self::Real(_))
    }

    /// Returns true if this value is null.
    #[inline]
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Returns true if this value is holding a 64-bit unsigned integer.
    ///
    /// Matches C++ `JsValue::IsUInt64()`.
    #[inline]
    #[must_use]
    pub fn is_uint64(&self) -> bool {
        matches!(self, Self::UInt(_))
    }

    /// Returns true if this value holds a type corresponding to `T`.
    ///
    /// Matches C++ `JsValue::Is<T>()`. Supported types: `JsObject`, `JsArray`,
    /// `String`, `bool`, `i64`, `u64`, `f64`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_js::{JsValue, JsObject};
    ///
    /// let obj = JsValue::Object(JsObject::new());
    /// assert!(obj.is::<JsObject>());
    /// assert!(!obj.is::<i64>());
    ///
    /// let num = JsValue::from(42i64);
    /// assert!(num.is::<i64>());
    /// ```
    #[inline]
    #[must_use]
    pub fn is<T: 'static>(&self) -> bool {
        use std::any::TypeId;
        let id = TypeId::of::<T>();
        if id == TypeId::of::<JsObject>() {
            return self.is_object();
        }
        if id == TypeId::of::<JsArray>() {
            return self.is_array();
        }
        if id == TypeId::of::<String>() {
            return self.is_string();
        }
        if id == TypeId::of::<bool>() {
            return self.is_bool();
        }
        if id == TypeId::of::<i64>() {
            return matches!(self, Self::Int(_));
        }
        if id == TypeId::of::<u64>() {
            return self.is_uint64();
        }
        if id == TypeId::of::<f64>() {
            return self.is_real();
        }
        false
    }

    /// Returns true if this value is a number (int or real).
    #[inline]
    #[must_use]
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Int(_) | Self::UInt(_) | Self::Real(_))
    }

    /// Returns the object if this value holds one.
    #[must_use]
    pub fn as_object(&self) -> Option<&JsObject> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Returns a mutable reference to the object if this value holds one.
    pub fn as_object_mut(&mut self) -> Option<&mut JsObject> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Returns the array if this value holds one.
    #[must_use]
    pub fn as_array(&self) -> Option<&JsArray> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Returns a mutable reference to the array if this value holds one.
    pub fn as_array_mut(&mut self) -> Option<&mut JsArray> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Returns the string if this value holds one.
    #[must_use]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the bool if this value holds one.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the value as i64 if it's an integer.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::UInt(u) => i64::try_from(*u).ok(),
            _ => None,
        }
    }

    /// Returns the value as u64 if it's an unsigned integer.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::UInt(u) => Some(*u),
            Self::Int(i) => u64::try_from(*i).ok(),
            _ => None,
        }
    }

    /// Returns the value as f64 if it's a real number.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Real(f) => Some(*f),
            Self::Int(i) => Some(*i as f64),
            Self::UInt(u) => Some(*u as f64),
            _ => None,
        }
    }

    /// Returns the value as i32, truncating if necessary.
    #[must_use]
    pub fn as_int(&self) -> Option<i32> {
        self.as_i64().map(|i| i as i32)
    }

    /// Extracts the object from this value, consuming it.
    pub fn into_object(self) -> Option<JsObject> {
        match self {
            Self::Object(obj) => Some(obj),
            _ => None,
        }
    }

    /// Extracts the array from this value, consuming it.
    pub fn into_array(self) -> Option<JsArray> {
        match self {
            Self::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Extracts the string from this value, consuming it.
    pub fn into_string(self) -> Option<String> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    /// Access a value by key if this is an object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&JsValue> {
        self.as_object().and_then(|obj| obj.get(key))
    }

    /// Access a value by index if this is an array.
    #[must_use]
    pub fn get_index(&self, index: usize) -> Option<&JsValue> {
        self.as_array().and_then(|arr| arr.get(index))
    }

    /// Returns a vector holding the elements of this value's array that
    /// correspond to the C++ type specified as the template parameter.
    /// If this value is not holding an array, an empty vector is returned.
    /// If any of the array's elements does not correspond to the C++ type,
    /// it is replaced with the default value used by the Get functions above.
    ///
    /// Matches C++ `JsValue::GetArrayOf<T>()`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_js::JsValue;
    ///
    /// let arr = JsValue::Array(vec![
    ///     JsValue::from(1),
    ///     JsValue::from(2),
    ///     JsValue::from(3),
    /// ]);
    /// // get_array_of requires T: From<JsValue>
    /// ```
    pub fn get_array_of<T>(&self) -> Vec<T>
    where
        T: From<JsValue>,
    {
        self.as_array()
            .map(|arr| arr.iter().map(|v| T::from(v.clone())).collect())
            .unwrap_or_default()
    }

    /// Returns true if this value is holding an array whose elements all
    /// correspond to the C++ type specified as the template parameter.
    ///
    /// Matches C++ `JsValue::IsArrayOf<T>()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_js::JsValue;
    ///
    /// let arr = JsValue::Array(vec![
    ///     JsValue::from(1),
    ///     JsValue::from(2),
    ///     JsValue::from(3),
    /// ]);
    /// assert!(arr.is_array_of::<i64>());
    ///
    /// let mixed = JsValue::Array(vec![
    ///     JsValue::from(1),
    ///     JsValue::from("string"),
    /// ]);
    /// assert!(!mixed.is_array_of::<i64>());
    /// ```
    pub fn is_array_of<T>(&self) -> bool
    where
        T: 'static,
    {
        if !self.is_array() {
            return false;
        }
        self.as_array()
            .map(|arr| {
                arr.iter().all(|v| {
                    // For primitive types, check type directly
                    // This is a simplified check - actual conversion happens in get_array_of
                    match std::any::TypeId::of::<T>() {
                        id if id == std::any::TypeId::of::<i64>() => v.is_int(),
                        id if id == std::any::TypeId::of::<u64>() => v.is_int(),
                        id if id == std::any::TypeId::of::<f64>() => v.is_real() || v.is_int(),
                        id if id == std::any::TypeId::of::<bool>() => v.is_bool(),
                        id if id == std::any::TypeId::of::<String>() => v.is_string(),
                        id if id == std::any::TypeId::of::<JsObject>() => v.is_object(),
                        id if id == std::any::TypeId::of::<JsArray>() => v.is_array(),
                        _ => {
                            // For other types, assume they can be converted
                            // Actual validation happens in get_array_of
                            true
                        }
                    }
                })
            })
            .unwrap_or(false)
    }
}

impl fmt::Debug for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Object(obj) => f.debug_map().entries(obj.iter()).finish(),
            Self::Array(arr) => f.debug_list().entries(arr.iter()).finish(),
            Self::String(s) => write!(f, "{:?}", s),
            Self::Bool(b) => write!(f, "{}", b),
            Self::Int(i) => write!(f, "{}", i),
            Self::UInt(u) => write!(f, "{}", u),
            Self::Real(r) => write!(f, "{}", r),
            Self::Null => write!(f, "null"),
        }
    }
}

impl fmt::Display for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Object(_) => write!(f, "[object]"),
            Self::Array(arr) => write!(f, "[array:{}]", arr.len()),
            Self::String(s) => write!(f, "{}", s),
            Self::Bool(b) => write!(f, "{}", b),
            Self::Int(i) => write!(f, "{}", i),
            Self::UInt(u) => write!(f, "{}", u),
            Self::Real(r) => write!(f, "{}", r),
            Self::Null => write!(f, "null"),
        }
    }
}

// Conversion from serde_json::Value
impl From<serde_json::Value> for JsValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Int(i)
                } else if let Some(u) = n.as_u64() {
                    Self::UInt(u)
                } else if let Some(f) = n.as_f64() {
                    Self::Real(f)
                } else {
                    Self::Null
                }
            }
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(arr) => {
                Self::Array(arr.into_iter().map(JsValue::from).collect())
            }
            serde_json::Value::Object(obj) => Self::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, JsValue::from(v)))
                    .collect(),
            ),
        }
    }
}

// Conversion to serde_json::Value
impl From<JsValue> for serde_json::Value {
    fn from(value: JsValue) -> Self {
        match value {
            JsValue::Null => serde_json::Value::Null,
            JsValue::Bool(b) => serde_json::Value::Bool(b),
            JsValue::Int(i) => serde_json::Value::Number(i.into()),
            JsValue::UInt(u) => serde_json::Value::Number(u.into()),
            JsValue::Real(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            JsValue::String(s) => serde_json::Value::String(s),
            JsValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(serde_json::Value::from).collect())
            }
            JsValue::Object(obj) => serde_json::Value::Object(
                obj.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
        }
    }
}

// From<JsValue> for common types — enables get_array_of::<T>() to match C++ GetArrayOf<T>().
// On type mismatch, returns default (matches C++ Get* behavior).
impl From<JsValue> for String {
    fn from(value: JsValue) -> Self {
        value.into_string().unwrap_or_default()
    }
}

impl From<JsValue> for i64 {
    fn from(value: JsValue) -> Self {
        value.as_i64().unwrap_or(0)
    }
}

impl From<JsValue> for u64 {
    fn from(value: JsValue) -> Self {
        value.as_u64().unwrap_or(0)
    }
}

impl From<JsValue> for f64 {
    fn from(value: JsValue) -> Self {
        value.as_f64().unwrap_or(0.0)
    }
}

impl From<JsValue> for bool {
    fn from(value: JsValue) -> Self {
        value.as_bool().unwrap_or(false)
    }
}

impl From<JsValue> for i32 {
    fn from(value: JsValue) -> Self {
        value.as_int().unwrap_or(0)
    }
}

impl From<JsValue> for JsObject {
    fn from(value: JsValue) -> Self {
        value.into_object().unwrap_or_default()
    }
}

impl From<JsValue> for JsArray {
    fn from(value: JsValue) -> Self {
        value.into_array().unwrap_or_default()
    }
}

// From implementations for common types
impl From<bool> for JsValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i32> for JsValue {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<i64> for JsValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<u32> for JsValue {
    fn from(value: u32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<u64> for JsValue {
    fn from(value: u64) -> Self {
        Self::UInt(value)
    }
}

impl From<f32> for JsValue {
    fn from(value: f32) -> Self {
        Self::Real(f64::from(value))
    }
}

impl From<f64> for JsValue {
    fn from(value: f64) -> Self {
        Self::Real(value)
    }
}

impl From<String> for JsValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for JsValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<JsObject> for JsValue {
    fn from(value: JsObject) -> Self {
        Self::Object(value)
    }
}

impl From<JsArray> for JsValue {
    fn from(value: JsArray) -> Self {
        Self::Array(value)
    }
}

impl<T: Into<JsValue>> From<Option<T>> for JsValue {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

/// Converts to `bool`: `true` if the value is not null, `false` if null.
///
/// Matches C++ `JsValue::operator bool()`.
///
/// # Examples
///
/// ```
/// use usd_js::JsValue;
///
/// let v = JsValue::from(42);
/// assert!(bool::from(&v));
///
/// let null = JsValue::null();
/// assert!(!bool::from(&null));
/// ```
impl From<&JsValue> for bool {
    #[inline]
    fn from(value: &JsValue) -> bool {
        !value.is_null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_checking() {
        assert!(JsValue::from(42).is_int());
        assert!(JsValue::from(3.14).is_real());
        assert!(JsValue::from(true).is_bool());
        assert!(JsValue::from("hello").is_string());
        assert!(JsValue::Null.is_null());
        assert!(JsValue::Object(JsObject::new()).is_object());
        assert!(JsValue::Array(Vec::new()).is_array());
    }

    #[test]
    fn test_is_generic() {
        let obj = JsValue::Object(JsObject::new());
        assert!(obj.is::<JsObject>());
        assert!(!obj.is::<i64>());

        let arr = JsValue::Array(Vec::new());
        assert!(arr.is::<JsArray>());
        assert!(!arr.is::<JsObject>());

        let num = JsValue::from(42i64);
        assert!(num.is::<i64>());
        assert!(!num.is::<f64>());

        let real = JsValue::from(3.14);
        assert!(real.is::<f64>());
        assert!(!real.is::<i64>());

        let s = JsValue::from("hi");
        assert!(s.is::<String>());
        assert!(!s.is::<bool>());

        let b = JsValue::from(true);
        assert!(b.is::<bool>());
    }

    #[test]
    fn test_operator_bool() {
        assert!(bool::from(&JsValue::from(42)));
        assert!(bool::from(&JsValue::from("hi")));
        assert!(bool::from(&JsValue::Object(JsObject::new())));
        assert!(!bool::from(&JsValue::null()));
    }

    #[test]
    fn test_value_extraction() {
        assert_eq!(JsValue::from(42).as_i64(), Some(42));
        assert_eq!(JsValue::from(3.14).as_f64(), Some(3.14));
        assert_eq!(JsValue::from(true).as_bool(), Some(true));
        assert_eq!(JsValue::from("hello").as_string(), Some("hello"));
    }

    #[test]
    fn test_object_access() {
        let mut obj = JsObject::new();
        obj.insert("key".to_string(), JsValue::from(42));
        let value = JsValue::Object(obj);

        assert_eq!(value.get("key").and_then(|v| v.as_i64()), Some(42));
        assert!(value.get("missing").is_none());
    }

    #[test]
    fn test_array_access() {
        let arr: JsArray = vec![JsValue::from(1), JsValue::from(2), JsValue::from(3)];
        let value = JsValue::Array(arr);

        assert_eq!(value.get_index(0).and_then(|v| v.as_i64()), Some(1));
        assert_eq!(value.get_index(2).and_then(|v| v.as_i64()), Some(3));
        assert!(value.get_index(10).is_none());
    }

    #[test]
    fn test_serde_roundtrip() {
        let original = JsValue::from(vec![
            JsValue::from(1),
            JsValue::from("two"),
            JsValue::from(3.0),
        ]);

        let json: serde_json::Value = original.clone().into();
        let restored = JsValue::from(json);

        assert_eq!(original, restored);
    }

    #[test]
    fn test_type_names() {
        assert_eq!(JsValue::from(42).type_name(), "int");
        assert_eq!(JsValue::from(3.14).type_name(), "real");
        assert_eq!(JsValue::from(true).type_name(), "bool");
        assert_eq!(JsValue::from("hello").type_name(), "string");
        assert_eq!(JsValue::Null.type_name(), "null");
    }

    #[test]
    fn test_get_array_of_string() {
        let arr = JsValue::Array(vec![
            JsValue::from("a"),
            JsValue::from("b"),
            JsValue::from("c"),
        ]);
        let result: Vec<String> = arr.get_array_of();
        assert_eq!(
            result,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_get_array_of_i64() {
        let arr = JsValue::Array(vec![JsValue::from(1), JsValue::from(2), JsValue::from(3)]);
        let result: Vec<i64> = arr.get_array_of();
        assert_eq!(result, vec![1i64, 2, 3]);
    }

    #[test]
    fn test_is_array_of() {
        let arr = JsValue::Array(vec![JsValue::from(1), JsValue::from(2)]);
        assert!(arr.is_array_of::<i64>());

        let mixed = JsValue::Array(vec![JsValue::from(1), JsValue::from("x")]);
        assert!(!mixed.is_array_of::<i64>());
    }
}
