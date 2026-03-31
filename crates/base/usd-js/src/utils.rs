//! JSON utility functions.
//!
//! Convenience functions for working with JSON values.

use super::value::{JsObject, JsValue};

/// Returns the value associated with `key` in the given object.
///
/// If no such key exists, returns `None` unless a `default_value` is provided.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, find_value};
///
/// let mut obj = JsObject::new();
/// obj.insert("name".to_string(), JsValue::from("Alice"));
/// obj.insert("age".to_string(), JsValue::from(30));
///
/// // Found key
/// let name = find_value(&obj, "name", None);
/// assert_eq!(name, Some(JsValue::from("Alice")));
///
/// // Missing key without default
/// let missing = find_value(&obj, "email", None);
/// assert_eq!(missing, None);
///
/// // Missing key with default
/// let with_default = find_value(&obj, "email", Some(JsValue::from("default@example.com")));
/// assert_eq!(with_default, Some(JsValue::from("default@example.com")));
/// ```
#[must_use]
pub fn find_value(object: &JsObject, key: &str, default_value: Option<JsValue>) -> Option<JsValue> {
    if key.is_empty() {
        return default_value;
    }
    object.get(key).cloned().or(default_value)
}

/// Returns the value associated with `key` in the given object, or `None` if not found.
///
/// This is a simpler version of `find_value` without default value support.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, get_value};
///
/// let mut obj = JsObject::new();
/// obj.insert("count".to_string(), JsValue::from(42));
///
/// assert_eq!(get_value(&obj, "count"), Some(&JsValue::from(42)));
/// assert_eq!(get_value(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_value<'a>(object: &'a JsObject, key: &str) -> Option<&'a JsValue> {
    object.get(key)
}

/// Returns the string value associated with `key`, or `None` if not found or not a string.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, get_string};
///
/// let mut obj = JsObject::new();
/// obj.insert("name".to_string(), JsValue::from("Bob"));
/// obj.insert("age".to_string(), JsValue::from(25));
///
/// assert_eq!(get_string(&obj, "name"), Some("Bob"));
/// assert_eq!(get_string(&obj, "age"), None); // Not a string
/// assert_eq!(get_string(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_string<'a>(object: &'a JsObject, key: &str) -> Option<&'a str> {
    object.get(key).and_then(|v| v.as_string())
}

/// Returns the integer value associated with `key`, or `None` if not found or not an int.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, get_int};
///
/// let mut obj = JsObject::new();
/// obj.insert("count".to_string(), JsValue::from(100));
/// obj.insert("name".to_string(), JsValue::from("test"));
///
/// assert_eq!(get_int(&obj, "count"), Some(100));
/// assert_eq!(get_int(&obj, "name"), None); // Not an int
/// assert_eq!(get_int(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_int(object: &JsObject, key: &str) -> Option<i64> {
    object.get(key).and_then(|v| v.as_i64())
}

/// Returns the real (f64) value associated with `key`, or `None` if not found or not a number.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, get_real};
///
/// let mut obj = JsObject::new();
/// obj.insert("pi".to_string(), JsValue::from(3.14159));
/// obj.insert("name".to_string(), JsValue::from("test"));
///
/// assert!((get_real(&obj, "pi").unwrap() - 3.14159).abs() < 1e-5);
/// assert_eq!(get_real(&obj, "name"), None); // Not a number
/// assert_eq!(get_real(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_real(object: &JsObject, key: &str) -> Option<f64> {
    object.get(key).and_then(|v| v.as_f64())
}

/// Returns the boolean value associated with `key`, or `None` if not found or not a bool.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, get_bool};
///
/// let mut obj = JsObject::new();
/// obj.insert("enabled".to_string(), JsValue::from(true));
/// obj.insert("name".to_string(), JsValue::from("test"));
///
/// assert_eq!(get_bool(&obj, "enabled"), Some(true));
/// assert_eq!(get_bool(&obj, "name"), None); // Not a bool
/// assert_eq!(get_bool(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_bool(object: &JsObject, key: &str) -> Option<bool> {
    object.get(key).and_then(|v| v.as_bool())
}

/// Returns the array value associated with `key`, or `None` if not found or not an array.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, JsArray, get_array};
///
/// let mut obj = JsObject::new();
/// obj.insert("items".to_string(), JsValue::Array(vec![
///     JsValue::from(1),
///     JsValue::from(2),
///     JsValue::from(3),
/// ]));
///
/// let arr = get_array(&obj, "items").unwrap();
/// assert_eq!(arr.len(), 3);
/// assert_eq!(get_array(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_array<'a>(object: &'a JsObject, key: &str) -> Option<&'a super::value::JsArray> {
    object.get(key).and_then(|v| v.as_array())
}

/// Returns the object value associated with `key`, or `None` if not found or not an object.
///
/// # Examples
///
/// ```
/// use usd_js::{JsObject, JsValue, get_object};
///
/// let mut inner = JsObject::new();
/// inner.insert("x".to_string(), JsValue::from(10));
///
/// let mut obj = JsObject::new();
/// obj.insert("position".to_string(), JsValue::Object(inner));
///
/// let pos = get_object(&obj, "position").unwrap();
/// assert!(pos.contains_key("x"));
/// assert_eq!(get_object(&obj, "missing"), None);
/// ```
#[inline]
#[must_use]
pub fn get_object<'a>(object: &'a JsObject, key: &str) -> Option<&'a JsObject> {
    object.get(key).and_then(|v| v.as_object())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_object() -> JsObject {
        let mut obj = JsObject::new();
        obj.insert("string".to_string(), JsValue::from("hello"));
        obj.insert("int".to_string(), JsValue::from(42));
        obj.insert("real".to_string(), JsValue::from(3.14));
        obj.insert("bool".to_string(), JsValue::from(true));
        obj.insert(
            "array".to_string(),
            JsValue::Array(vec![JsValue::from(1), JsValue::from(2)]),
        );

        let mut nested = JsObject::new();
        nested.insert("nested_key".to_string(), JsValue::from("nested_value"));
        obj.insert("object".to_string(), JsValue::Object(nested));

        obj
    }

    #[test]
    fn test_find_value_found() {
        let obj = make_test_object();
        let result = find_value(&obj, "string", None);
        assert_eq!(result, Some(JsValue::from("hello")));
    }

    #[test]
    fn test_find_value_not_found_no_default() {
        let obj = make_test_object();
        let result = find_value(&obj, "missing", None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_value_not_found_with_default() {
        let obj = make_test_object();
        let default = JsValue::from("default");
        let result = find_value(&obj, "missing", Some(default.clone()));
        assert_eq!(result, Some(default));
    }

    #[test]
    fn test_get_value() {
        let obj = make_test_object();
        assert!(get_value(&obj, "int").is_some());
        assert!(get_value(&obj, "missing").is_none());
    }

    #[test]
    fn test_get_string() {
        let obj = make_test_object();
        assert_eq!(get_string(&obj, "string"), Some("hello"));
        assert_eq!(get_string(&obj, "int"), None);
        assert_eq!(get_string(&obj, "missing"), None);
    }

    #[test]
    fn test_get_int() {
        let obj = make_test_object();
        assert_eq!(get_int(&obj, "int"), Some(42));
        assert_eq!(get_int(&obj, "string"), None);
        assert_eq!(get_int(&obj, "missing"), None);
    }

    #[test]
    fn test_get_real() {
        let obj = make_test_object();
        let result = get_real(&obj, "real").unwrap();
        assert!((result - 3.14).abs() < 1e-10);
        assert_eq!(get_real(&obj, "string"), None);
    }

    #[test]
    fn test_get_bool() {
        let obj = make_test_object();
        assert_eq!(get_bool(&obj, "bool"), Some(true));
        assert_eq!(get_bool(&obj, "string"), None);
    }

    #[test]
    fn test_get_array() {
        let obj = make_test_object();
        let arr = get_array(&obj, "array").unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(get_array(&obj, "string"), None);
    }

    #[test]
    fn test_get_object() {
        let obj = make_test_object();
        let nested = get_object(&obj, "object").unwrap();
        assert!(nested.contains_key("nested_key"));
        assert_eq!(get_object(&obj, "string"), None);
    }

    #[test]
    fn test_find_value_empty_key_returns_default() {
        // C++ JsFindValue: empty key returns default (or nullopt)
        let obj = make_test_object();
        let default = JsValue::from("default");
        assert_eq!(find_value(&obj, "", None), None);
        assert_eq!(find_value(&obj, "", Some(default.clone())), Some(default));
    }
}
