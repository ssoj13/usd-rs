//! JSON value type converter.
//!
//! Provides utilities for converting recursive JsValue structures to
//! identical structures using different container types.
//!
//! This module matches the functionality of `pxr/base/js/converter.h`.

use crate::{JsArray, JsObject, JsValue};

/// Helper for converting JsValue to integer types.
///
/// Matches C++ `Js_ValueToInt` template struct.
/// When `USE_INT64` is true, converts to i64/u64.
/// When `USE_INT64` is false, converts to i32.
trait ValueToIntHelper<ValueType, const USE_INT64: bool> {
    fn apply(value: &JsValue) -> ValueType;
}

/// Implementation for UseInt64 = true (i64/u64).
struct ValueToInt64Impl;

impl<ValueType> ValueToIntHelper<ValueType, true> for ValueToInt64Impl
where
    ValueType: From<i64> + From<u64>,
{
    fn apply(value: &JsValue) -> ValueType {
        match value {
            JsValue::UInt(u) => ValueType::from(*u),
            JsValue::Int(i) => ValueType::from(*i),
            _ => ValueType::from(0i64), // Fallback
        }
    }
}

/// Implementation for UseInt64 = false (i32).
struct ValueToInt32Impl;

impl<ValueType> ValueToIntHelper<ValueType, false> for ValueToInt32Impl
where
    ValueType: From<i32>,
{
    fn apply(value: &JsValue) -> ValueType {
        match value {
            JsValue::UInt(u) => ValueType::from(*u as i32),
            JsValue::Int(i) => ValueType::from(*i as i32),
            _ => ValueType::from(0i32), // Fallback
        }
    }
}

/// A helper that can convert recursive JsValue structures to
/// identical structures using a different container type.
///
/// The destination container type is determined by the `ValueType` type parameter,
/// while the type to map objects to is determined by the `MapType` type parameter.
///
/// It is expected that `ValueType` is default constructible. A default constructed
/// `ValueType` is used to represent JSON null. The value type must also support
/// construction from the fundamental bool, string, real and integer types supported
/// by JsValue.
///
/// JsArray values are converted to `Vec<ValueType>`, and JsObject values are
/// converted to the `MapType`. `MapType` must have a value type of `ValueType`,
/// and support insertion operations.
///
/// If the `USE_INT64` const parameter is `true` (default), value types converted
/// from JsValue::Int hold u64 or i64. If the parameter is `false`, all IntType
/// values are converted to i32. Note that this may cause truncation if the JsValue
/// holds values too large to be stored in an i32.
///
/// This matches C++ `JsValueTypeConverter` template class.
pub struct JsValueTypeConverter<ValueType, MapType, const USE_INT64: bool = true> {
    _phantom: std::marker::PhantomData<(ValueType, MapType)>,
}

impl<ValueType, MapType, const USE_INT64: bool> JsValueTypeConverter<ValueType, MapType, USE_INT64>
where
    ValueType: Default
        + From<bool>
        + From<String>
        + From<f64>
        + From<MapType>
        + From<Vec<ValueType>>
        + From<i64>
        + From<u64>
        + From<i32>,
    MapType: FromIterator<(String, ValueType)>,
{
    /// Converts the given `value` recursively to a structure using the value
    /// and map types specified by the `ValueType` and `MapType` type parameters.
    ///
    /// Matches C++ `JsValueTypeConverter::Convert`.
    pub fn convert(value: &JsValue) -> ValueType {
        Self::to_value_type(value)
    }

    /// Converts `value` to `ValueType`.
    ///
    /// Matches C++ `JsValueTypeConverter::_ToValueType`.
    fn to_value_type(value: &JsValue) -> ValueType {
        match value {
            JsValue::Object(obj) => {
                // Convert object to MapType, then to ValueType
                // Matches C++: return ValueType(_ObjectToMap(value.GetJsObject()));
                let map: MapType = Self::object_to_map(obj);
                ValueType::from(map)
            }
            JsValue::Array(arr) => {
                // Convert array to Vec<ValueType>, then to ValueType
                // Matches C++: return ValueType(_ArrayToVector(value.GetJsArray()));
                let vec: Vec<ValueType> = Self::array_to_vector(arr);
                ValueType::from(vec)
            }
            JsValue::Bool(b) => ValueType::from(*b),
            JsValue::String(s) => ValueType::from(s.clone()),
            JsValue::Real(r) => ValueType::from(*r),
            JsValue::Int(_) | JsValue::UInt(_) => {
                // Matches C++: return Js_ValueToInt<ValueType, MapType, UseInt64>::Apply(value);
                if USE_INT64 {
                    <ValueToInt64Impl as ValueToIntHelper<ValueType, true>>::apply(value)
                } else {
                    <ValueToInt32Impl as ValueToIntHelper<ValueType, false>>::apply(value)
                }
            }
            JsValue::Null => ValueType::default(),
        }
    }

    /// Converts `object` to `MapType`.
    ///
    /// Matches C++ `JsValueTypeConverter::_ObjectToMap`.
    fn object_to_map(object: &JsObject) -> MapType {
        object
            .iter()
            .map(|(k, v)| (k.clone(), Self::to_value_type(v)))
            .collect()
    }

    /// Converts `array` to `Vec<ValueType>`.
    ///
    /// Matches C++ `JsValueTypeConverter::_ArrayToVector`.
    fn array_to_vector(array: &JsArray) -> Vec<ValueType> {
        let mut result = Vec::with_capacity(array.len());
        for value in array {
            result.push(Self::to_value_type(value));
        }
        result
    }
}

/// Returns `value` converted recursively to the template and map types given
/// by the `ValueType` and `MapType` parameters.
///
/// Matches C++ `JsConvertToContainerType` function.
///
/// # Type Parameters
///
/// * `ValueType` - The destination value type. Must be constructible from:
///   - `bool`, `String`, `f64` (primitive types)
///   - `MapType` (for objects)
///   - `Vec<ValueType>` (for arrays)
///   - Default (for null)
/// * `MapType` - The map type for objects. Must be constructible from
///   `(String, ValueType)` pairs.
/// * `USE_INT64` - If `true`, integers are converted to i64/u64. If `false`, to i32.
///
/// # Examples
///
/// ```ignore
/// use usd_js::{JsValue, convert_to_container_type};
/// use std::collections::HashMap;
///
/// // Convert primitive value
/// let value = JsValue::from(42);
/// let result: i64 = convert_to_container_type::<i64, HashMap<String, i64>, true>(&value);
/// ```
pub fn convert_to_container_type<ValueType, MapType, const USE_INT64: bool>(
    value: &JsValue,
) -> ValueType
where
    ValueType: Default
        + From<bool>
        + From<String>
        + From<f64>
        + From<MapType>
        + From<Vec<ValueType>>
        + From<i64>
        + From<u64>
        + From<i32>,
    MapType: FromIterator<(String, ValueType)>,
{
    JsValueTypeConverter::<ValueType, MapType, USE_INT64>::convert(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JsValue;
    use std::collections::HashMap;

    // Helper type that can be constructed from all required types
    #[derive(Debug, Clone, PartialEq)]
    enum TestValue {
        Int(i64),
        String(String),
        Bool(bool),
        Real(f64),
        Array(Vec<TestValue>),
        Object(HashMap<String, TestValue>),
        Null,
    }

    impl Default for TestValue {
        fn default() -> Self {
            TestValue::Null
        }
    }

    impl From<bool> for TestValue {
        fn from(b: bool) -> Self {
            TestValue::Bool(b)
        }
    }

    impl From<String> for TestValue {
        fn from(s: String) -> Self {
            TestValue::String(s)
        }
    }

    impl From<f64> for TestValue {
        fn from(f: f64) -> Self {
            TestValue::Real(f)
        }
    }

    impl From<i64> for TestValue {
        fn from(i: i64) -> Self {
            TestValue::Int(i)
        }
    }

    impl From<u64> for TestValue {
        fn from(u: u64) -> Self {
            TestValue::Int(u as i64)
        }
    }

    impl From<i32> for TestValue {
        fn from(i: i32) -> Self {
            TestValue::Int(i as i64)
        }
    }

    impl From<HashMap<String, TestValue>> for TestValue {
        fn from(map: HashMap<String, TestValue>) -> Self {
            TestValue::Object(map)
        }
    }

    impl From<Vec<TestValue>> for TestValue {
        fn from(vec: Vec<TestValue>) -> Self {
            TestValue::Array(vec)
        }
    }

    #[test]
    fn test_convert_int_i64() {
        let value = JsValue::from(42);
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        assert_eq!(result, TestValue::Int(42));
    }

    #[test]
    fn test_convert_int_i32() {
        let value = JsValue::from(42);
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, false>::convert(&value);
        assert_eq!(result, TestValue::Int(42));
    }

    #[test]
    fn test_convert_string() {
        let value = JsValue::from("hello");
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        assert_eq!(result, TestValue::String("hello".to_string()));
    }

    #[test]
    fn test_convert_bool() {
        let value = JsValue::from(true);
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        assert_eq!(result, TestValue::Bool(true));
    }

    #[test]
    fn test_convert_real() {
        let value = JsValue::from(3.14);
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        match result {
            TestValue::Real(f) => assert!((f - 3.14).abs() < 1e-10),
            _ => panic!("Expected Real"),
        }
    }

    #[test]
    fn test_convert_null() {
        let value = JsValue::Null;
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        assert_eq!(result, TestValue::Null);
    }

    #[test]
    fn test_convert_array() {
        let value = JsValue::from(vec![JsValue::from(1), JsValue::from(2), JsValue::from(3)]);
        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        match result {
            TestValue::Array(arr) => {
                assert_eq!(arr.len(), 3);
                assert_eq!(arr[0], TestValue::Int(1));
                assert_eq!(arr[1], TestValue::Int(2));
                assert_eq!(arr[2], TestValue::Int(3));
            }
            _ => panic!("Expected Array"),
        }
    }

    #[test]
    fn test_convert_object() {
        use crate::JsObject;
        let mut obj = JsObject::new();
        obj.insert("a".to_string(), JsValue::from(1));
        obj.insert("b".to_string(), JsValue::from(2));
        let value = JsValue::Object(obj);

        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        match result {
            TestValue::Object(map) => {
                assert_eq!(map.get("a"), Some(&TestValue::Int(1)));
                assert_eq!(map.get("b"), Some(&TestValue::Int(2)));
            }
            _ => panic!("Expected Object"),
        }
    }

    #[test]
    fn test_convert_nested() {
        use crate::JsObject;
        let mut inner = JsObject::new();
        inner.insert("x".to_string(), JsValue::from(10));
        let mut outer = JsObject::new();
        outer.insert("inner".to_string(), JsValue::Object(inner));
        let value = JsValue::Object(outer);

        let result =
            JsValueTypeConverter::<TestValue, HashMap<String, TestValue>, true>::convert(&value);
        match result {
            TestValue::Object(outer_map) => match outer_map.get("inner") {
                Some(TestValue::Object(inner_map)) => {
                    assert_eq!(inner_map.get("x"), Some(&TestValue::Int(10)));
                }
                _ => panic!("Expected nested object"),
            },
            _ => panic!("Expected outer object"),
        }
    }
}
