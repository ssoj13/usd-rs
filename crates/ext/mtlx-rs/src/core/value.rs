//! Value -- typed value for MaterialX (int, float, string, color, vector, matrix, aggregate).

use crate::core::types::{
    ARRAY_PREFERRED_SEPARATOR, ARRAY_VALID_SEPARATORS, Color3, Color4, Matrix33, Matrix44, Vector2,
    Vector3, Vector4,
};
use std::cell::Cell;

// ---------------------------------------------------------------------------
// Float formatting -- thread-local, matching C++ thread_local semantics
// ---------------------------------------------------------------------------

/// Float output format for value-to-string conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FloatFormat {
    #[default]
    Default,
    Fixed,
    Scientific,
}

thread_local! {
    static FLOAT_FORMAT: Cell<FloatFormat> = Cell::new(FloatFormat::Default);
    static FLOAT_PRECISION: Cell<i32> = Cell::new(6);
}

/// Get the current thread-local float format.
pub fn get_float_format() -> FloatFormat {
    FLOAT_FORMAT.with(|f| f.get())
}

/// Set the current thread-local float format.
pub fn set_float_format(fmt: FloatFormat) {
    FLOAT_FORMAT.with(|f| f.set(fmt));
}

/// Get the current thread-local float precision.
pub fn get_float_precision() -> i32 {
    FLOAT_PRECISION.with(|p| p.get())
}

/// Set the current thread-local float precision.
pub fn set_float_precision(precision: i32) {
    FLOAT_PRECISION.with(|p| p.set(precision));
}

/// Format a single f32 according to the current thread-local float format/precision.
pub fn format_float(v: f32) -> String {
    let prec = get_float_precision() as usize;
    match get_float_format() {
        FloatFormat::Fixed => format!("{:.prec$}", v, prec = prec),
        FloatFormat::Scientific => format!("{:.prec$e}", v, prec = prec),
        FloatFormat::Default => {
            // Mimic C++ default: use Rust's default Display (shortest round-trip),
            // but respect precision for extra sig-figs when set away from default 6.
            if prec == 6 {
                format!("{}", v)
            } else {
                format!("{:.prec$}", v, prec = prec)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ScopedFloatFormatting -- RAII guard
// ---------------------------------------------------------------------------

/// RAII guard that temporarily overrides float format and precision,
/// restoring original values on drop. Mirrors C++ ScopedFloatFormatting.
pub struct ScopedFloatFormatting {
    saved_format: FloatFormat,
    saved_precision: i32,
}

impl ScopedFloatFormatting {
    /// Set format (and optionally precision; pass -1 to leave unchanged).
    pub fn new(format: FloatFormat, precision: i32) -> Self {
        let saved_format = get_float_format();
        let saved_precision = get_float_precision();
        set_float_format(format);
        if precision >= 0 {
            set_float_precision(precision);
        }
        Self {
            saved_format,
            saved_precision,
        }
    }
}

impl Drop for ScopedFloatFormatting {
    fn drop(&mut self) {
        set_float_format(self.saved_format);
        set_float_precision(self.saved_precision);
    }
}

// ---------------------------------------------------------------------------
// AggregateValue -- struct-typed value, e.g. "{1.0;0.0;0.0}"
// ---------------------------------------------------------------------------

/// A value whose type is a user-defined struct (typedef with members).
/// Members are stored as a flat Vec and serialise as "{v0;v1;...}".
#[derive(Clone, Debug)]
pub struct AggregateValue {
    /// The MaterialX type name, e.g. "MyStruct".
    pub type_name: String,
    /// Ordered member values.
    pub members: Vec<Value>,
}

impl AggregateValue {
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            members: Vec::new(),
        }
    }

    pub fn append_value(&mut self, v: Value) {
        self.members.push(v);
    }

    pub fn get_members(&self) -> &[Value] {
        &self.members
    }

    pub fn get_member(&self, index: usize) -> Option<&Value> {
        self.members.get(index)
    }

    /// Serialise as "{v0;v1;...}" matching C++ AggregateValue::getValueString().
    pub fn get_value_string(&self) -> String {
        if self.members.is_empty() {
            return String::new();
        }
        let inner: Vec<String> = self.members.iter().map(|v| v.get_value_string()).collect();
        format!("{{{}}}", inner.join(";"))
    }
}

impl PartialEq for AggregateValue {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name && self.members == other.members
    }
}

// ---------------------------------------------------------------------------
// Value enum
// ---------------------------------------------------------------------------

/// MaterialX value -- discriminated union of supported types.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Integer(i32),
    Boolean(bool),
    Float(f32),
    String(String),
    Color3(Color3),
    Color4(Color4),
    Vector2(Vector2),
    Vector3(Vector3),
    Vector4(Vector4),
    Matrix33(Matrix33),
    Matrix44(Matrix44),
    IntegerArray(Vec<i32>),
    FloatArray(Vec<f32>),
    StringArray(Vec<String>),
    /// Struct/aggregate value with a runtime type name and ordered members.
    Aggregate(Box<AggregateValue>),
}

impl Value {
    pub fn get_type_string(&self) -> &str {
        match self {
            Self::Integer(_) => "integer",
            Self::Boolean(_) => "boolean",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Color3(_) => "color3",
            Self::Color4(_) => "color4",
            Self::Vector2(_) => "vector2",
            Self::Vector3(_) => "vector3",
            Self::Vector4(_) => "vector4",
            Self::Matrix33(_) => "matrix33",
            Self::Matrix44(_) => "matrix44",
            Self::IntegerArray(_) => "integerarray",
            Self::FloatArray(_) => "floatarray",
            Self::StringArray(_) => "stringarray",
            Self::Aggregate(a) => &a.type_name,
        }
    }

    pub fn get_value_string(&self) -> String {
        match self {
            Self::Integer(v) => v.to_string(),
            Self::Boolean(v) => if *v { "true" } else { "false" }.to_string(),
            Self::Float(v) => format_float(*v),
            Self::String(v) => v.clone(),
            Self::Color3(v) => format_vec_floats(&v.0),
            Self::Color4(v) => format_vec_floats(&v.0),
            Self::Vector2(v) => format_vec_floats(&v.0),
            Self::Vector3(v) => format_vec_floats(&v.0),
            Self::Vector4(v) => format_vec_floats(&v.0),
            Self::Matrix33(m) => {
                let flat: Vec<f32> = m.0.iter().flatten().copied().collect();
                format_vec_floats(&flat)
            }
            Self::Matrix44(m) => {
                let flat: Vec<f32> = m.0.iter().flatten().copied().collect();
                format_vec_floats(&flat)
            }
            Self::IntegerArray(v) => v
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(&ARRAY_PREFERRED_SEPARATOR.to_string()),
            Self::FloatArray(v) => v
                .iter()
                .map(|x| format_float(*x))
                .collect::<Vec<_>>()
                .join(&ARRAY_PREFERRED_SEPARATOR.to_string()),
            Self::StringArray(v) => v.join(&ARRAY_PREFERRED_SEPARATOR.to_string()),
            Self::Aggregate(a) => a.get_value_string(),
        }
    }

    /// Create from value string and type string (built-in types only).
    /// For struct types, use `parse_struct_value_string` + `AggregateValue`.
    pub fn from_strings(value: &str, type_str: &str) -> Option<Self> {
        let value = value.trim();
        match type_str {
            "integer" => value.parse::<i32>().ok().map(Self::Integer),
            "boolean" => match value.to_lowercase().as_str() {
                "true" | "1" => Some(Self::Boolean(true)),
                "false" | "0" => Some(Self::Boolean(false)),
                _ => None,
            },
            "float" => value.parse::<f32>().ok().map(Self::Float),
            "string" | "filename" | "geomname" => Some(Self::String(value.to_string())),
            "color3" => parse_color3(value).map(Self::Color3),
            "color4" => parse_color4(value).map(Self::Color4),
            "vector2" => parse_vector2(value).map(Self::Vector2),
            "vector3" => parse_vector3(value).map(Self::Vector3),
            "vector4" => parse_vector4(value).map(Self::Vector4),
            "matrix33" => parse_matrix33(value).map(Self::Matrix33),
            "matrix44" => parse_matrix44(value).map(Self::Matrix44),
            "integerarray" => {
                let v: Vec<i32> = value
                    .split(|c| ARRAY_VALID_SEPARATORS.contains(c))
                    .filter_map(|p| p.trim().parse().ok())
                    .collect();
                if v.is_empty() {
                    None
                } else {
                    Some(Self::IntegerArray(v))
                }
            }
            "floatarray" => {
                let v: Vec<f32> = value
                    .split(|c| ARRAY_VALID_SEPARATORS.contains(c))
                    .filter_map(|p| p.trim().parse().ok())
                    .collect();
                if v.is_empty() {
                    None
                } else {
                    Some(Self::FloatArray(v))
                }
            }
            "stringarray" => {
                let v: Vec<String> = value.split(',').map(|s| s.trim().to_string()).collect();
                Some(Self::StringArray(v))
            }
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// parse_struct_value_string
// ---------------------------------------------------------------------------

/// Tokenise a struct value string like "{1;2;{3;4}}" into ["1", "2", "{3;4}"].
/// Strips the outer braces and splits on ';' respecting nested brace depth.
/// Returns empty Vec for empty or malformed input.
pub fn parse_struct_value_string(value: &str) -> Vec<String> {
    const SEPARATOR: char = ';';
    const OPEN: char = '{';
    const CLOSE: char = '}';

    if value.len() < 2 {
        return vec![];
    }

    // Must be wrapped in { }
    let bytes = value.as_bytes();
    if bytes[0] != b'{' || bytes[bytes.len() - 1] != b'}' {
        return vec![];
    }

    // Strip surrounding braces
    let inner = &value[1..value.len() - 1];

    let mut result = Vec::new();
    let mut part = String::new();
    let mut depth = 0i32;

    for ch in inner.chars() {
        if ch == OPEN {
            depth += 1;
        }
        if depth > 0 && ch == CLOSE {
            depth -= 1;
        }
        if depth == 0 && ch == SEPARATOR {
            result.push(part.clone());
            part.clear();
        } else {
            part.push(ch);
        }
    }

    if !part.is_empty() {
        result.push(part);
    }

    result
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a float slice with the current float format/precision, comma-separated.
fn format_vec_floats(vals: &[f32]) -> String {
    vals.iter()
        .map(|v| format_float(*v))
        .collect::<Vec<_>>()
        .join(&ARRAY_PREFERRED_SEPARATOR.to_string())
}

fn parse_floats(s: &str, n: usize) -> Option<Vec<f32>> {
    let parts: Vec<f32> = s
        .split(|c| ARRAY_VALID_SEPARATORS.contains(c))
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    if parts.len() >= n { Some(parts) } else { None }
}

fn parse_color3(s: &str) -> Option<Color3> {
    let v = parse_floats(s, 3)?;
    Some(Color3([v[0], v[1], v[2]]))
}
fn parse_color4(s: &str) -> Option<Color4> {
    let v = parse_floats(s, 4)?;
    Some(Color4([v[0], v[1], v[2], v[3]]))
}
fn parse_vector2(s: &str) -> Option<Vector2> {
    let v = parse_floats(s, 2)?;
    Some(Vector2([v[0], v[1]]))
}
fn parse_vector3(s: &str) -> Option<Vector3> {
    let v = parse_floats(s, 3)?;
    Some(Vector3([v[0], v[1], v[2]]))
}
fn parse_vector4(s: &str) -> Option<Vector4> {
    let v = parse_floats(s, 4)?;
    Some(Vector4([v[0], v[1], v[2], v[3]]))
}
fn parse_matrix33(s: &str) -> Option<Matrix33> {
    let v = parse_floats(s, 9)?;
    Some(Matrix33([
        [v[0], v[1], v[2]],
        [v[3], v[4], v[5]],
        [v[6], v[7], v[8]],
    ]))
}
fn parse_matrix44(s: &str) -> Option<Matrix44> {
    let v = parse_floats(s, 16)?;
    Some(Matrix44([
        [v[0], v[1], v[2], v[3]],
        [v[4], v[5], v[6], v[7]],
        [v[8], v[9], v[10], v[11]],
        [v[12], v[13], v[14], v[15]],
    ]))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_struct_value_string ---

    #[test]
    fn parse_struct_empty() {
        assert!(parse_struct_value_string("").is_empty());
        assert!(parse_struct_value_string("{}").is_empty());
    }

    #[test]
    fn parse_struct_flat() {
        let parts = parse_struct_value_string("{1;2;3}");
        assert_eq!(parts, vec!["1", "2", "3"]);
    }

    #[test]
    fn parse_struct_nested() {
        let parts = parse_struct_value_string("{1;2;{3;4;5}}");
        assert_eq!(parts, vec!["1", "2", "{3;4;5}"]);
    }

    #[test]
    fn parse_struct_invalid() {
        // Missing braces -> empty
        assert!(parse_struct_value_string("1;2;3").is_empty());
    }

    // --- AggregateValue ---

    #[test]
    fn aggregate_value_string() {
        let mut agg = AggregateValue::new("MyStruct");
        agg.append_value(Value::Integer(1));
        agg.append_value(Value::Float(2.0));
        agg.append_value(Value::String("hello".to_string()));
        let s = agg.get_value_string();
        assert_eq!(s, "{1;2;hello}");
    }

    #[test]
    fn aggregate_nested() {
        let mut inner = AggregateValue::new("Inner");
        inner.append_value(Value::Integer(3));
        inner.append_value(Value::Integer(4));

        let mut outer = AggregateValue::new("Outer");
        outer.append_value(Value::Integer(1));
        outer.append_value(Value::Aggregate(Box::new(inner)));

        assert_eq!(outer.get_value_string(), "{1;{3;4}}");
    }

    // --- ScopedFloatFormatting ---

    #[test]
    fn scoped_float_format_restores() {
        set_float_format(FloatFormat::Default);
        set_float_precision(6);

        {
            let _guard = ScopedFloatFormatting::new(FloatFormat::Fixed, 2);
            assert_eq!(get_float_format(), FloatFormat::Fixed);
            assert_eq!(get_float_precision(), 2);
            assert_eq!(format_float(3.14159), "3.14");
        }

        // After drop: restored
        assert_eq!(get_float_format(), FloatFormat::Default);
        assert_eq!(get_float_precision(), 6);
    }

    #[test]
    fn scoped_float_scientific() {
        let _guard = ScopedFloatFormatting::new(FloatFormat::Scientific, 3);
        let s = format_float(0.001);
        // Should contain 'e' notation
        assert!(s.contains('e'), "expected scientific: {}", s);
    }

    // --- get_value_string respects float format ---

    #[test]
    fn value_float_format_fixed() {
        let _guard = ScopedFloatFormatting::new(FloatFormat::Fixed, 3);
        let v = Value::Float(1.23456);
        assert_eq!(v.get_value_string(), "1.235");
    }

    #[test]
    fn value_aggregate_in_enum() {
        let mut agg = AggregateValue::new("Pair");
        agg.append_value(Value::Integer(10));
        agg.append_value(Value::Integer(20));
        let v = Value::Aggregate(Box::new(agg));
        assert_eq!(v.get_type_string(), "Pair");
        assert_eq!(v.get_value_string(), "{10;20}");
    }
}
