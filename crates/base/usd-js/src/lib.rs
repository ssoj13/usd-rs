//! JSON Support (js) - JSON parsing and serialization.
//!
//! This module provides JSON parsing and serialization utilities, equivalent to
//! OpenUSD's Js library. It wraps `serde_json` to provide a familiar API.
//!
//! # Examples
//!
//! ```
//! use usd_js::{JsValue, parse_string, write_to_string};
//!
//! // Parse JSON string
//! let value = parse_string(r#"{"name": "test", "value": 42}"#).unwrap();
//! assert!(value.is_object());
//!
//! // Write JSON to string
//! let json = write_to_string(&value);
//! assert!(json.contains("test"));
//! ```

pub mod converter;
pub mod error;
pub mod utils;
pub mod value;

pub use converter::{JsValueTypeConverter, convert_to_container_type};
pub use error::JsParseError;
pub use utils::{
    find_value, get_array, get_bool, get_int, get_object, get_real, get_string, get_value,
};
pub use value::{JsArray, JsObject, JsValue};

/// Optional JSON value. Matches C++ `JsOptionalValue` (std::optional\<JsValue\>).
pub type JsOptionalValue = Option<JsValue>;

use std::io::{Read, Write};

/// Parse JSON from an input stream.
///
/// # Examples
///
/// ```
/// use usd_js::parse_stream;
/// use std::io::Cursor;
///
/// let data = r#"{"key": "value"}"#;
/// let mut cursor = Cursor::new(data);
/// let value = parse_stream(&mut cursor).unwrap();
/// assert!(value.is_object());
/// ```
pub fn parse_stream<R: Read>(reader: &mut R) -> Result<JsValue, JsParseError> {
    let value: serde_json::Value = serde_json::from_reader(reader).map_err(JsParseError::from)?;
    Ok(JsValue::from(value))
}

/// Parse JSON from a string.
///
/// # Examples
///
/// ```
/// use usd_js::parse_string;
///
/// let value = parse_string(r#"{"name": "test"}"#).unwrap();
/// assert!(value.is_object());
///
/// let arr = parse_string("[1, 2, 3]").unwrap();
/// assert!(arr.is_array());
/// ```
pub fn parse_string(data: &str) -> Result<JsValue, JsParseError> {
    let value: serde_json::Value = serde_json::from_str(data).map_err(JsParseError::from)?;
    Ok(JsValue::from(value))
}

/// Write JSON value to an output stream.
///
/// # Examples
///
/// ```
/// use usd_js::{JsValue, write_to_stream};
/// use std::io::Cursor;
///
/// let value = JsValue::from(42);
/// let mut output = Vec::new();
/// write_to_stream(&value, &mut output).unwrap();
/// assert_eq!(String::from_utf8(output).unwrap(), "42");
/// ```
pub fn write_to_stream<W: Write>(value: &JsValue, writer: &mut W) -> std::io::Result<()> {
    let json_value: serde_json::Value = value.clone().into();
    serde_json::to_writer(writer, &json_value)?;
    Ok(())
}

/// Write JSON value to a pretty-formatted output stream.
///
/// # Examples
///
/// ```
/// use usd_js::{JsValue, JsObject, write_to_stream_pretty};
/// use std::io::Cursor;
///
/// let mut obj = JsObject::new();
/// obj.insert("key".to_string(), JsValue::from("value"));
/// let value = JsValue::Object(obj);
///
/// let mut output = Vec::new();
/// write_to_stream_pretty(&value, &mut output).unwrap();
/// let result = String::from_utf8(output).unwrap();
/// assert!(result.contains('\n'));
/// ```
pub fn write_to_stream_pretty<W: Write>(value: &JsValue, writer: &mut W) -> std::io::Result<()> {
    let json_value: serde_json::Value = value.clone().into();
    serde_json::to_writer_pretty(writer, &json_value)?;
    Ok(())
}

/// Write JSON value to a string.
///
/// # Examples
///
/// ```
/// use usd_js::{JsValue, write_to_string};
///
/// let value = JsValue::from("hello");
/// let json = write_to_string(&value);
/// assert_eq!(json, r#""hello""#);
/// ```
#[must_use]
pub fn write_to_string(value: &JsValue) -> String {
    let json_value: serde_json::Value = value.clone().into();
    serde_json::to_string(&json_value).unwrap_or_default()
}

/// Write JSON value to a pretty-formatted string.
///
/// # Examples
///
/// ```
/// use usd_js::{JsValue, JsObject, write_to_string_pretty};
///
/// let mut obj = JsObject::new();
/// obj.insert("key".to_string(), JsValue::from("value"));
/// let value = JsValue::Object(obj);
///
/// let json = write_to_string_pretty(&value);
/// assert!(json.contains('\n'));
/// ```
#[must_use]
pub fn write_to_string_pretty(value: &JsValue) -> String {
    let json_value: serde_json::Value = value.clone().into();
    serde_json::to_string_pretty(&json_value).unwrap_or_default()
}

/// A writer for streaming JSON values directly to a stream.
///
/// This class provides an interface to writing json values directly to a
/// stream. This can be much more efficient than constructing a JsValue instance
/// and using write_to_stream if the data size is significant.
///
/// Matches C++ `JsWriter` class.
pub struct JsWriter<W: Write> {
    writer: W,
    style: WriterStyle,
    /// Stack of first-item flags per nesting level (matches rapidjson internal state).
    first_item_stack: Vec<bool>,
    /// True after write_key(); next value write skips comma prefix.
    after_key: bool,
    indent_level: usize,
}

/// Style for JSON output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterStyle {
    /// Compact output (no extra whitespace).
    Compact,
    /// Pretty-printed output (with indentation and newlines).
    Pretty,
}

impl<W: Write> JsWriter<W> {
    /// Creates a new JsWriter with the given writer and style.
    ///
    /// Matches C++ `JsWriter::JsWriter(std::ostream& ostr, Style style)`.
    pub fn new(writer: W, style: WriterStyle) -> Self {
        Self {
            writer,
            style,
            first_item_stack: vec![true],
            after_key: false,
            indent_level: 0,
        }
    }

    /// Write a null value.
    ///
    /// Matches C++ `JsWriter::WriteValue(std::nullptr_t)`.
    pub fn write_null(&mut self) -> std::io::Result<()> {
        self.write_value(&JsValue::Null)
    }

    /// Write a boolean value.
    ///
    /// Matches C++ `JsWriter::WriteValue(bool b)`.
    pub fn write_bool(&mut self, b: bool) -> std::io::Result<()> {
        self.write_value(&JsValue::from(b))
    }

    /// Write an integer value.
    ///
    /// Matches C++ `JsWriter::WriteValue(int i)`.
    pub fn write_int(&mut self, i: i32) -> std::io::Result<()> {
        self.write_value(&JsValue::from(i))
    }

    /// Write a 64-bit integer value.
    ///
    /// Matches C++ `JsWriter::WriteValue(int64_t i)`.
    pub fn write_i64(&mut self, i: i64) -> std::io::Result<()> {
        self.write_value(&JsValue::from(i))
    }

    /// Write a 64-bit unsigned integer value.
    ///
    /// Matches C++ `JsWriter::WriteValue(uint64_t u)`.
    pub fn write_u64(&mut self, u: u64) -> std::io::Result<()> {
        self.write_value(&JsValue::from(u))
    }

    /// Write a double value.
    ///
    /// Matches C++ `JsWriter::WriteValue(double d)`.
    pub fn write_f64(&mut self, d: f64) -> std::io::Result<()> {
        self.write_value(&JsValue::from(d))
    }

    /// Write a string value.
    ///
    /// Matches C++ `JsWriter::WriteValue(const std::string& s)`.
    pub fn write_string_value(&mut self, s: &str) -> std::io::Result<()> {
        self.write_value(&JsValue::from(s))
    }

    /// Returns true if the current nesting level has not yet emitted an element.
    fn is_first(&self) -> bool {
        self.first_item_stack.last().copied().unwrap_or(true)
    }

    /// Mark current nesting level as having emitted at least one element.
    fn mark_not_first(&mut self) {
        if let Some(last) = self.first_item_stack.last_mut() {
            *last = false;
        }
    }

    /// Emit comma/separator before a value element (not after a key).
    fn write_separator(&mut self) -> std::io::Result<()> {
        if self.after_key {
            // Value follows key -- no separator needed, key already wrote colon
            self.after_key = false;
            return Ok(());
        }
        if !self.is_first() {
            write!(self.writer, ",")?;
        }
        if self.style == WriterStyle::Pretty {
            writeln!(self.writer)?;
            self.write_indent()?;
        } else if !self.is_first() {
            write!(self.writer, " ")?;
        }
        Ok(())
    }

    /// Write an escaped JSON string to the output.
    fn write_escaped_str(&mut self, s: &str) -> std::io::Result<()> {
        write!(self.writer, "\"")?;
        for ch in s.chars() {
            match ch {
                '"' => write!(self.writer, "\\\"")?,
                '\\' => write!(self.writer, "\\\\")?,
                '\n' => write!(self.writer, "\\n")?,
                '\r' => write!(self.writer, "\\r")?,
                '\t' => write!(self.writer, "\\t")?,
                c => write!(self.writer, "{}", c)?,
            }
        }
        write!(self.writer, "\"")
    }

    /// Write a JsValue.
    ///
    /// Matches C++ `JsWriter::WriteValue` for various types.
    pub fn write_value(&mut self, value: &JsValue) -> std::io::Result<()> {
        self.write_separator()?;

        match value {
            JsValue::Null => write!(self.writer, "null")?,
            JsValue::Bool(b) => write!(self.writer, "{}", b)?,
            JsValue::Int(i) => write!(self.writer, "{}", i)?,
            JsValue::UInt(u) => write!(self.writer, "{}", u)?,
            JsValue::Real(f) => {
                if f.fract() == 0.0 {
                    write!(self.writer, "{:.1}", f)?;
                } else {
                    write!(self.writer, "{}", f)?;
                }
            }
            JsValue::String(s) => {
                self.write_escaped_str(s)?;
            }
            JsValue::Array(arr) => {
                write!(self.writer, "[")?;
                self.first_item_stack.push(true);
                self.indent_level += 1;
                for item in arr {
                    self.write_value(item)?;
                }
                self.indent_level -= 1;
                self.first_item_stack.pop();
                if self.style == WriterStyle::Pretty {
                    writeln!(self.writer)?;
                    self.write_indent()?;
                }
                write!(self.writer, "]")?;
            }
            JsValue::Object(obj) => {
                write!(self.writer, "{{")?;
                self.first_item_stack.push(true);
                self.indent_level += 1;
                for (key, val) in obj {
                    // Key separator (comma between key-value pairs)
                    if !self.is_first() {
                        write!(self.writer, ",")?;
                    }
                    if self.style == WriterStyle::Pretty {
                        writeln!(self.writer)?;
                        self.write_indent()?;
                    } else if !self.is_first() {
                        write!(self.writer, " ")?;
                    }
                    self.write_escaped_str(key)?;
                    write!(self.writer, ":")?;
                    if self.style == WriterStyle::Pretty {
                        write!(self.writer, " ")?;
                    }
                    self.mark_not_first();
                    // Value after key -- push a fresh level so write_value
                    // doesn't emit a comma prefix
                    self.first_item_stack.push(true);
                    self.after_key = false;
                    self.write_value(val)?;
                    self.first_item_stack.pop();
                }
                self.indent_level -= 1;
                self.first_item_stack.pop();
                if self.style == WriterStyle::Pretty {
                    writeln!(self.writer)?;
                    self.write_indent()?;
                }
                write!(self.writer, "}}")?;
            }
        }

        self.mark_not_first();
        Ok(())
    }

    /// Write the start of an object.
    ///
    /// Matches C++ `JsWriter::BeginObject()`.
    pub fn begin_object(&mut self) -> std::io::Result<()> {
        self.write_separator()?;
        write!(self.writer, "{{")?;
        self.first_item_stack.push(true);
        self.indent_level += 1;
        Ok(())
    }

    /// Write an object key.
    ///
    /// Matches C++ `JsWriter::WriteKey(const std::string&)`.
    pub fn write_key(&mut self, key: &str) -> std::io::Result<()> {
        if !self.is_first() {
            write!(self.writer, ",")?;
        }
        if self.style == WriterStyle::Pretty {
            writeln!(self.writer)?;
            self.write_indent()?;
        } else if !self.is_first() {
            write!(self.writer, " ")?;
        }
        self.write_escaped_str(key)?;
        write!(self.writer, ":")?;
        if self.style == WriterStyle::Pretty {
            write!(self.writer, " ")?;
        }
        self.mark_not_first();
        self.after_key = true;
        Ok(())
    }

    /// Write the end of an object.
    ///
    /// Matches C++ `JsWriter::EndObject()`.
    pub fn end_object(&mut self) -> std::io::Result<()> {
        self.indent_level -= 1;
        self.first_item_stack.pop();
        if self.style == WriterStyle::Pretty {
            writeln!(self.writer)?;
            self.write_indent()?;
        }
        write!(self.writer, "}}")?;
        self.mark_not_first();
        Ok(())
    }

    /// Write the start of an array.
    ///
    /// Matches C++ `JsWriter::BeginArray()`.
    pub fn begin_array(&mut self) -> std::io::Result<()> {
        self.write_separator()?;
        write!(self.writer, "[")?;
        self.first_item_stack.push(true);
        self.indent_level += 1;
        Ok(())
    }

    /// Write the end of an array.
    ///
    /// Matches C++ `JsWriter::EndArray()`.
    pub fn end_array(&mut self) -> std::io::Result<()> {
        self.indent_level -= 1;
        self.first_item_stack.pop();
        if self.style == WriterStyle::Pretty {
            writeln!(self.writer)?;
            self.write_indent()?;
        }
        write!(self.writer, "]")?;
        self.mark_not_first();
        Ok(())
    }

    /// Write a key-value pair in one call.
    ///
    /// Matches C++ `JsWriter::WriteKeyValue(K, V)`.
    pub fn write_key_value(&mut self, key: &str, value: &JsValue) -> std::io::Result<()> {
        self.write_key(key)?;
        // after_key is set by write_key, so write_value won't emit comma
        self.write_value(value)?;
        Ok(())
    }

    /// Convenience: write an array of JsValues from a slice.
    ///
    /// Matches C++ `JsWriter::WriteArray(Container)`.
    pub fn write_array(&mut self, items: &[JsValue]) -> std::io::Result<()> {
        self.begin_array()?;
        for item in items {
            self.write_value(item)?;
        }
        self.end_array()
    }

    /// Convenience: write an array using a custom writer function per item.
    ///
    /// Matches C++ `JsWriter::WriteArray(Container, ItemWriteFn)`.
    pub fn write_array_with<T, F>(&mut self, items: &[T], f: F) -> std::io::Result<()>
    where
        F: Fn(&mut JsWriter<W>, &T) -> std::io::Result<()>,
    {
        self.begin_array()?;
        for item in items {
            f(self, item)?;
        }
        self.end_array()
    }

    /// Convenience: write an object from key-value pairs.
    ///
    /// Matches C++ `JsWriter::WriteObject(T...)`.
    pub fn write_object(&mut self, pairs: &[(&str, &JsValue)]) -> std::io::Result<()> {
        self.begin_object()?;
        for (key, value) in pairs {
            self.write_key_value(key, value)?;
        }
        self.end_object()
    }

    fn write_indent(&mut self) -> std::io::Result<()> {
        for _ in 0..self.indent_level {
            write!(self.writer, "  ")?;
        }
        Ok(())
    }
}

/// Write a json value using a JsWriter.
///
/// Matches C++ `JsWriteValue(JsWriter* writer, const JsValue& value)`.
pub fn write_value<W: Write>(writer: &mut JsWriter<W>, value: &JsValue) -> std::io::Result<()> {
    writer.write_value(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_object() {
        let json = r#"{"name": "test", "value": 42}"#;
        let value = parse_string(json).unwrap();

        assert!(value.is_object());
        let obj = value.as_object().unwrap();
        assert_eq!(obj.get("name").and_then(|v| v.as_string()), Some("test"));
        assert_eq!(obj.get("value").and_then(|v| v.as_i64()), Some(42));
    }

    #[test]
    fn test_parse_array() {
        let json = "[1, 2, 3, 4, 5]";
        let value = parse_string(json).unwrap();

        assert!(value.is_array());
        let arr = value.as_array().unwrap();
        assert_eq!(arr.len(), 5);
    }

    #[test]
    fn test_parse_primitives() {
        assert_eq!(parse_string("42").unwrap().as_i64(), Some(42));
        assert_eq!(parse_string("3.14").unwrap().as_f64(), Some(3.14));
        assert_eq!(parse_string("true").unwrap().as_bool(), Some(true));
        assert_eq!(parse_string("false").unwrap().as_bool(), Some(false));
        assert!(parse_string("null").unwrap().is_null());
        assert_eq!(
            parse_string(r#""hello""#).unwrap().as_string(),
            Some("hello")
        );
    }

    #[test]
    fn test_write_object() {
        let mut obj = JsObject::new();
        obj.insert("name".to_string(), JsValue::from("test"));
        obj.insert("value".to_string(), JsValue::from(42));
        let value = JsValue::Object(obj);

        let json = write_to_string(&value);
        assert!(json.contains("name"));
        assert!(json.contains("test"));
    }

    #[test]
    fn test_write_array() {
        let arr: JsArray = vec![JsValue::from(1), JsValue::from(2), JsValue::from(3)];
        let value = JsValue::Array(arr);

        let json = write_to_string(&value);
        assert_eq!(json, "[1,2,3]");
    }

    #[test]
    fn test_roundtrip() {
        let original = r#"{"nested":{"array":[1,2,3],"bool":true},"string":"hello"}"#;
        let value = parse_string(original).unwrap();
        let output = write_to_string(&value);

        // Parse both and compare (order may differ)
        let v1 = parse_string(original).unwrap();
        let v2 = parse_string(&output).unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_parse_error() {
        let result = parse_string("{invalid json}");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(!err.reason.is_empty());
    }

    #[test]
    fn test_js_writer_write_key_value() {
        let mut buf = Vec::new();
        let mut w = JsWriter::new(&mut buf, WriterStyle::Compact);
        w.begin_object().unwrap();
        w.write_key_value("name", &JsValue::from("test")).unwrap();
        w.write_key_value("count", &JsValue::from(42)).unwrap();
        w.end_object().unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\"name\":"));
        assert!(s.contains("\"test\""));
        assert!(s.contains("\"count\":"));
        assert!(s.contains("42"));
    }

    #[test]
    fn test_js_writer_write_array() {
        let mut buf = Vec::new();
        let items = vec![JsValue::from(1), JsValue::from(2), JsValue::from(3)];
        let mut w = JsWriter::new(&mut buf, WriterStyle::Compact);
        w.write_array(&items).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with('['));
        assert!(s.ends_with(']'));
        assert!(s.contains('1'));
        assert!(s.contains('3'));
    }

    #[test]
    fn test_js_writer_write_array_with() {
        let mut buf = Vec::new();
        let numbers: Vec<i32> = vec![10, 20, 30];
        let mut w = JsWriter::new(&mut buf, WriterStyle::Compact);
        w.write_array_with(&numbers, |writer, n| writer.write_value(&JsValue::from(*n)))
            .unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("10"));
        assert!(s.contains("30"));
    }

    #[test]
    fn test_js_writer_write_object() {
        let mut buf = Vec::new();
        let name = JsValue::from("Alice");
        let age = JsValue::from(30);
        let mut w = JsWriter::new(&mut buf, WriterStyle::Compact);
        w.write_object(&[("name", &name), ("age", &age)]).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.starts_with('{'));
        assert!(s.ends_with('}'));
        assert!(s.contains("\"name\""));
        assert!(s.contains("\"Alice\""));
    }

    /// Verify nested streaming API (write_key + begin_array) works correctly.
    /// This was the P2 bug: single first_item bool got clobbered on nesting.
    #[test]
    fn test_js_writer_nested_streaming() {
        let mut buf = Vec::new();
        let mut w = JsWriter::new(&mut buf, WriterStyle::Compact);
        w.begin_object().unwrap();
        w.write_key("arr").unwrap();
        w.begin_array().unwrap();
        w.write_value(&JsValue::from(1)).unwrap();
        w.write_value(&JsValue::from(2)).unwrap();
        w.end_array().unwrap();
        w.write_key("val").unwrap();
        w.write_value(&JsValue::from(42)).unwrap();
        w.end_object().unwrap();
        let s = String::from_utf8(buf).unwrap();
        // Should produce: {"arr":[1, 2], "val":42}
        let parsed = parse_string(&s).unwrap();
        let obj = parsed.as_object().unwrap();
        let arr = obj.get("arr").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(obj.get("val").unwrap().as_i64(), Some(42));
    }

    /// Verify deeply nested streaming (object inside array inside object).
    #[test]
    fn test_js_writer_deep_nesting() {
        let mut buf = Vec::new();
        let mut w = JsWriter::new(&mut buf, WriterStyle::Compact);
        w.begin_object().unwrap();
        w.write_key("items").unwrap();
        w.begin_array().unwrap();
        // First array element: nested object
        w.begin_object().unwrap();
        w.write_key_value("id", &JsValue::from(1)).unwrap();
        w.end_object().unwrap();
        // Second array element: nested object
        w.begin_object().unwrap();
        w.write_key_value("id", &JsValue::from(2)).unwrap();
        w.end_object().unwrap();
        w.end_array().unwrap();
        w.end_object().unwrap();
        let s = String::from_utf8(buf).unwrap();
        let parsed = parse_string(&s).unwrap();
        let items = parsed
            .as_object()
            .unwrap()
            .get("items")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0].as_object().unwrap().get("id").unwrap().as_i64(),
            Some(1)
        );
        assert_eq!(
            items[1].as_object().unwrap().get("id").unwrap().as_i64(),
            Some(2)
        );
    }
}
