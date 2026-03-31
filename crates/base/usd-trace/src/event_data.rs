//! TraceEventData - Data that can be stored in trace events.
//!
//! Port of pxr/base/trace/eventData.h

use std::fmt;

/// Data types that can be stored in trace events.
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    /// Invalid/no data.
    Invalid,
    /// Boolean value.
    Bool,
    /// Signed 64-bit integer.
    Int,
    /// Unsigned 64-bit integer.
    UInt,
    /// 64-bit floating point.
    Float,
    /// String value.
    String,
}

/// Data that can be stored in trace events.
///
/// This class holds typed data that can be attached to TraceEvents.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum EventData {
    /// No data.
    #[default]
    None,
    /// Boolean value.
    Bool(bool),
    /// Signed 64-bit integer.
    Int(i64),
    /// Unsigned 64-bit integer.
    UInt(u64),
    /// 64-bit floating point.
    Float(f64),
    /// String value.
    String(String),
}

impl EventData {
    /// Returns the data type.
    #[inline]
    pub fn data_type(&self) -> DataType {
        match self {
            EventData::None => DataType::Invalid,
            EventData::Bool(_) => DataType::Bool,
            EventData::Int(_) => DataType::Int,
            EventData::UInt(_) => DataType::UInt,
            EventData::Float(_) => DataType::Float,
            EventData::String(_) => DataType::String,
        }
    }

    /// Returns the boolean value if this is a Bool, None otherwise.
    #[inline]
    pub fn get_bool(&self) -> Option<bool> {
        match self {
            EventData::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the i64 value if this is an Int, None otherwise.
    #[inline]
    pub fn get_int(&self) -> Option<i64> {
        match self {
            EventData::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the u64 value if this is a UInt, None otherwise.
    #[inline]
    pub fn get_uint(&self) -> Option<u64> {
        match self {
            EventData::UInt(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns the f64 value if this is a Float, None otherwise.
    #[inline]
    pub fn get_float(&self) -> Option<f64> {
        match self {
            EventData::Float(v) => Some(*v),
            _ => None,
        }
    }

    /// Returns a reference to the string if this is a String, None otherwise.
    #[inline]
    pub fn get_string(&self) -> Option<&str> {
        match self {
            EventData::String(v) => Some(v),
            _ => None,
        }
    }

    /// Returns true if this contains no data.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, EventData::None)
    }

    /// Writes JSON representation of the data.
    pub fn write_json(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        match self {
            EventData::None => write!(writer, "null"),
            EventData::Bool(v) => write!(writer, "{}", v),
            EventData::Int(v) => write!(writer, "{}", v),
            EventData::UInt(v) => write!(writer, "{}", v),
            EventData::Float(v) => {
                if v.is_nan() {
                    write!(writer, "\"NaN\"")
                } else if v.is_infinite() {
                    if *v > 0.0 {
                        write!(writer, "\"Infinity\"")
                    } else {
                        write!(writer, "\"-Infinity\"")
                    }
                } else {
                    write!(writer, "{}", v)
                }
            }
            EventData::String(v) => {
                write!(writer, "\"")?;
                for c in v.chars() {
                    match c {
                        '"' => write!(writer, "\\\"")?,
                        '\\' => write!(writer, "\\\\")?,
                        '\n' => write!(writer, "\\n")?,
                        '\r' => write!(writer, "\\r")?,
                        '\t' => write!(writer, "\\t")?,
                        c if c.is_control() => write!(writer, "\\u{:04x}", c as u32)?,
                        c => write!(writer, "{}", c)?,
                    }
                }
                write!(writer, "\"")
            }
        }
    }
}

impl From<bool> for EventData {
    fn from(v: bool) -> Self {
        EventData::Bool(v)
    }
}

impl From<i64> for EventData {
    fn from(v: i64) -> Self {
        EventData::Int(v)
    }
}

impl From<i32> for EventData {
    fn from(v: i32) -> Self {
        EventData::Int(v as i64)
    }
}

impl From<u64> for EventData {
    fn from(v: u64) -> Self {
        EventData::UInt(v)
    }
}

impl From<u32> for EventData {
    fn from(v: u32) -> Self {
        EventData::UInt(v as u64)
    }
}

impl From<f64> for EventData {
    fn from(v: f64) -> Self {
        EventData::Float(v)
    }
}

impl From<f32> for EventData {
    fn from(v: f32) -> Self {
        EventData::Float(v as f64)
    }
}

impl From<String> for EventData {
    fn from(v: String) -> Self {
        EventData::String(v)
    }
}

impl From<&str> for EventData {
    fn from(v: &str) -> Self {
        EventData::String(v.to_string())
    }
}

impl fmt::Display for EventData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventData::None => write!(f, "<none>"),
            EventData::Bool(v) => write!(f, "{}", v),
            EventData::Int(v) => write!(f, "{}", v),
            EventData::UInt(v) => write!(f, "{}", v),
            EventData::Float(v) => write!(f, "{}", v),
            EventData::String(v) => write!(f, "{}", v),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_types() {
        assert_eq!(EventData::None.data_type(), DataType::Invalid);
        assert_eq!(EventData::Bool(true).data_type(), DataType::Bool);
        assert_eq!(EventData::Int(42).data_type(), DataType::Int);
        assert_eq!(EventData::UInt(42).data_type(), DataType::UInt);
        assert_eq!(EventData::Float(3.14).data_type(), DataType::Float);
        assert_eq!(
            EventData::String("test".into()).data_type(),
            DataType::String
        );
    }

    #[test]
    fn test_getters() {
        assert_eq!(EventData::Bool(true).get_bool(), Some(true));
        assert_eq!(EventData::Int(42).get_int(), Some(42));
        assert_eq!(EventData::UInt(42).get_uint(), Some(42));
        assert_eq!(EventData::Float(3.14).get_float(), Some(3.14));
        assert_eq!(EventData::String("test".into()).get_string(), Some("test"));
    }

    #[test]
    fn test_json() {
        let mut buf = String::new();
        EventData::Bool(true)
            .write_json(&mut buf)
            .expect("fmt write");
        assert_eq!(buf, "true");

        buf.clear();
        EventData::String("hello\nworld".into())
            .write_json(&mut buf)
            .expect("fmt write");
        assert_eq!(buf, "\"hello\\nworld\"");
    }
}
