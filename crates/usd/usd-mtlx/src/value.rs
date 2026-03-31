//! MaterialX value parsing - convert string representations to typed values.
//!
//! This module provides parsing for all MaterialX data types, matching the
//! behavior of mx::Value::createValueFromStrings() in C++ MaterialX.

/// MaterialX typed value. Matches mx::Value type enum in C++ MaterialX.
#[derive(Debug, Clone, PartialEq)]
pub enum MtlxValue {
    /// Boolean scalar.
    Bool(bool),
    /// Integer scalar.
    Int(i32),
    /// Float scalar.
    Float(f32),
    /// String value (filename, geomname, etc.).
    String(String),
    /// RGB color (3 floats).
    Color3([f32; 3]),
    /// RGBA color (4 floats).
    Color4([f32; 4]),
    /// 2D vector.
    Vector2([f32; 2]),
    /// 3D vector.
    Vector3([f32; 3]),
    /// 4D vector.
    Vector4([f32; 4]),
    /// 3×3 matrix.
    Matrix33([[f32; 3]; 3]),
    /// 4×4 matrix.
    Matrix44([[f32; 4]; 4]),
    /// Array of booleans.
    BoolArray(Vec<bool>),
    /// Array of integers.
    IntArray(Vec<i32>),
    /// Array of floats.
    FloatArray(Vec<f32>),
    /// Array of strings.
    StringArray(Vec<String>),
    /// Array of Color3 values.
    Color3Array(Vec<[f32; 3]>),
    /// Array of Color4 values.
    Color4Array(Vec<[f32; 4]>),
    /// Array of Vector2 values.
    Vector2Array(Vec<[f32; 2]>),
    /// Array of Vector3 values.
    Vector3Array(Vec<[f32; 3]>),
    /// Array of Vector4 values.
    Vector4Array(Vec<[f32; 4]>),
}

/// Parse a MaterialX value from string representation
pub fn create_value_from_strings(value_str: &str, type_name: &str) -> Option<MtlxValue> {
    let trimmed = trim_spaces(value_str);

    match type_name {
        // Boolean types
        "boolean" | "bool" => parse_bool(trimmed).map(MtlxValue::Bool),

        // Integer types ("long" is alias for integer in C++ MaterialX)
        "integer" | "int" | "long" => parse_int(trimmed).map(MtlxValue::Int),

        // Float types ("double" is alias for float in C++ MaterialX)
        "float" | "double" => parse_float(trimmed).map(MtlxValue::Float),

        // String types (includes filename, geomname, etc.)
        "string" | "filename" | "geomname" => Some(MtlxValue::String(trimmed.to_string())),

        // Vector types
        "color3" => parse_float3(trimmed).map(MtlxValue::Color3),
        "color4" => parse_float4(trimmed).map(MtlxValue::Color4),
        "vector2" => parse_float2(trimmed).map(MtlxValue::Vector2),
        "vector3" => parse_float3(trimmed).map(MtlxValue::Vector3),
        "vector4" => parse_float4(trimmed).map(MtlxValue::Vector4),

        // Matrix types
        "matrix33" => parse_matrix33(trimmed).map(MtlxValue::Matrix33),
        "matrix44" => parse_matrix44(trimmed).map(MtlxValue::Matrix44),

        // Array types
        "booleanarray" => parse_bool_array(trimmed).map(MtlxValue::BoolArray),
        "integerarray" => parse_int_array(trimmed).map(MtlxValue::IntArray),
        "floatarray" => parse_float_array(trimmed).map(MtlxValue::FloatArray),
        "stringarray" => Some(MtlxValue::StringArray(split_string(trimmed, ","))),
        "color3array" => parse_float3_array(trimmed).map(MtlxValue::Color3Array),
        "color4array" => parse_float4_array(trimmed).map(MtlxValue::Color4Array),
        "vector2array" => parse_float2_array(trimmed).map(MtlxValue::Vector2Array),
        "vector3array" => parse_float3_array(trimmed).map(MtlxValue::Vector3Array),
        "vector4array" => parse_float4_array(trimmed).map(MtlxValue::Vector4Array),

        _ => None,
    }
}

/// Split string by separator (handles MaterialX separators: comma or semicolon)
pub fn split_string(s: &str, sep: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }

    s.split(sep)
        .map(|part| trim_spaces(part).to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

/// Trim leading and trailing whitespace
pub fn trim_spaces(s: &str) -> &str {
    s.trim()
}

// Parsing helpers

fn parse_bool(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_int(s: &str) -> Option<i32> {
    s.parse().ok()
}

fn parse_float(s: &str) -> Option<f32> {
    s.parse().ok()
}

fn parse_float2(s: &str) -> Option<[f32; 2]> {
    let parts = split_string(s, ",");
    if parts.len() != 2 {
        return None;
    }

    let x = parts[0].parse().ok()?;
    let y = parts[1].parse().ok()?;
    Some([x, y])
}

fn parse_float3(s: &str) -> Option<[f32; 3]> {
    let parts = split_string(s, ",");
    if parts.len() != 3 {
        return None;
    }

    let x = parts[0].parse().ok()?;
    let y = parts[1].parse().ok()?;
    let z = parts[2].parse().ok()?;
    Some([x, y, z])
}

fn parse_float4(s: &str) -> Option<[f32; 4]> {
    let parts = split_string(s, ",");
    if parts.len() != 4 {
        return None;
    }

    let x = parts[0].parse().ok()?;
    let y = parts[1].parse().ok()?;
    let z = parts[2].parse().ok()?;
    let w = parts[3].parse().ok()?;
    Some([x, y, z, w])
}

fn parse_matrix33(s: &str) -> Option<[[f32; 3]; 3]> {
    let parts = split_string(s, ",");
    if parts.len() != 9 {
        return None;
    }

    let mut values = [0.0f32; 9];
    for (i, part) in parts.iter().enumerate() {
        values[i] = part.parse().ok()?;
    }

    Some([
        [values[0], values[1], values[2]],
        [values[3], values[4], values[5]],
        [values[6], values[7], values[8]],
    ])
}

fn parse_matrix44(s: &str) -> Option<[[f32; 4]; 4]> {
    let parts = split_string(s, ",");
    if parts.len() != 16 {
        return None;
    }

    let mut values = [0.0f32; 16];
    for (i, part) in parts.iter().enumerate() {
        values[i] = part.parse().ok()?;
    }

    Some([
        [values[0], values[1], values[2], values[3]],
        [values[4], values[5], values[6], values[7]],
        [values[8], values[9], values[10], values[11]],
        [values[12], values[13], values[14], values[15]],
    ])
}

// Array parsing

fn parse_bool_array(s: &str) -> Option<Vec<bool>> {
    let parts = split_string(s, ",");
    let mut result = Vec::new();

    for part in parts {
        result.push(parse_bool(&part)?);
    }

    Some(result)
}

fn parse_int_array(s: &str) -> Option<Vec<i32>> {
    let parts = split_string(s, ",");
    let mut result = Vec::new();

    for part in parts {
        result.push(part.parse().ok()?);
    }

    Some(result)
}

fn parse_float_array(s: &str) -> Option<Vec<f32>> {
    let parts = split_string(s, ",");
    let mut result = Vec::new();

    for part in parts {
        result.push(part.parse().ok()?);
    }

    Some(result)
}

fn parse_float2_array(s: &str) -> Option<Vec<[f32; 2]>> {
    // Expect semicolon-separated groups of 2 floats
    let groups = split_string(s, ";");
    let mut result = Vec::new();

    for group in groups {
        result.push(parse_float2(&group)?);
    }

    Some(result)
}

fn parse_float3_array(s: &str) -> Option<Vec<[f32; 3]>> {
    // Expect semicolon-separated groups of 3 floats
    let groups = split_string(s, ";");
    let mut result = Vec::new();

    for group in groups {
        result.push(parse_float3(&group)?);
    }

    Some(result)
}

fn parse_float4_array(s: &str) -> Option<Vec<[f32; 4]>> {
    // Expect semicolon-separated groups of 4 floats
    let groups = split_string(s, ";");
    let mut result = Vec::new();

    for group in groups {
        result.push(parse_float4(&group)?);
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_parsing() {
        assert_eq!(
            create_value_from_strings("true", "boolean"),
            Some(MtlxValue::Bool(true))
        );
        assert_eq!(
            create_value_from_strings("false", "boolean"),
            Some(MtlxValue::Bool(false))
        );
        assert_eq!(
            create_value_from_strings("1", "boolean"),
            Some(MtlxValue::Bool(true))
        );
        assert_eq!(
            create_value_from_strings("0", "boolean"),
            Some(MtlxValue::Bool(false))
        );
    }

    #[test]
    fn test_numeric_parsing() {
        assert_eq!(
            create_value_from_strings("42", "integer"),
            Some(MtlxValue::Int(42))
        );
        assert_eq!(
            create_value_from_strings("-10", "integer"),
            Some(MtlxValue::Int(-10))
        );
        assert_eq!(
            create_value_from_strings("3.14", "float"),
            Some(MtlxValue::Float(3.14))
        );
        assert_eq!(
            create_value_from_strings("-2.5", "float"),
            Some(MtlxValue::Float(-2.5))
        );
    }

    #[test]
    fn test_string_parsing() {
        assert_eq!(
            create_value_from_strings("hello", "string"),
            Some(MtlxValue::String("hello".to_string()))
        );
        assert_eq!(
            create_value_from_strings("path/to/file.png", "filename"),
            Some(MtlxValue::String("path/to/file.png".to_string()))
        );
    }

    #[test]
    fn test_vector_parsing() {
        assert_eq!(
            create_value_from_strings("1.0, 2.0", "vector2"),
            Some(MtlxValue::Vector2([1.0, 2.0]))
        );
        assert_eq!(
            create_value_from_strings("1.0, 2.0, 3.0", "vector3"),
            Some(MtlxValue::Vector3([1.0, 2.0, 3.0]))
        );
        assert_eq!(
            create_value_from_strings("1.0, 2.0, 3.0, 4.0", "vector4"),
            Some(MtlxValue::Vector4([1.0, 2.0, 3.0, 4.0]))
        );
    }

    #[test]
    fn test_color_parsing() {
        assert_eq!(
            create_value_from_strings("1.0, 0.5, 0.25", "color3"),
            Some(MtlxValue::Color3([1.0, 0.5, 0.25]))
        );
        assert_eq!(
            create_value_from_strings("1.0, 0.5, 0.25, 0.8", "color4"),
            Some(MtlxValue::Color4([1.0, 0.5, 0.25, 0.8]))
        );
    }

    #[test]
    fn test_matrix_parsing() {
        let mat33_str = "1,0,0, 0,1,0, 0,0,1";
        let expected_mat33 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(
            create_value_from_strings(mat33_str, "matrix33"),
            Some(MtlxValue::Matrix33(expected_mat33))
        );

        let mat44_str = "1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1";
        let expected_mat44 = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(
            create_value_from_strings(mat44_str, "matrix44"),
            Some(MtlxValue::Matrix44(expected_mat44))
        );
    }

    #[test]
    fn test_array_parsing() {
        assert_eq!(
            create_value_from_strings("true, false, true", "booleanarray"),
            Some(MtlxValue::BoolArray(vec![true, false, true]))
        );
        assert_eq!(
            create_value_from_strings("1, 2, 3, 4", "integerarray"),
            Some(MtlxValue::IntArray(vec![1, 2, 3, 4]))
        );
        assert_eq!(
            create_value_from_strings("1.0, 2.5, 3.7", "floatarray"),
            Some(MtlxValue::FloatArray(vec![1.0, 2.5, 3.7]))
        );
        assert_eq!(
            create_value_from_strings("red, green, blue", "stringarray"),
            Some(MtlxValue::StringArray(vec![
                "red".to_string(),
                "green".to_string(),
                "blue".to_string()
            ]))
        );
    }

    #[test]
    fn test_vector_array_parsing() {
        assert_eq!(
            create_value_from_strings("1.0,2.0; 3.0,4.0", "vector2array"),
            Some(MtlxValue::Vector2Array(vec![[1.0, 2.0], [3.0, 4.0]]))
        );
        assert_eq!(
            create_value_from_strings("1.0,2.0,3.0; 4.0,5.0,6.0", "vector3array"),
            Some(MtlxValue::Vector3Array(vec![
                [1.0, 2.0, 3.0],
                [4.0, 5.0, 6.0]
            ]))
        );
    }

    #[test]
    fn test_trim_spaces() {
        assert_eq!(trim_spaces("  hello  "), "hello");
        assert_eq!(trim_spaces("world"), "world");
        assert_eq!(trim_spaces("  "), "");
    }

    #[test]
    fn test_split_string() {
        assert_eq!(split_string("a,b,c", ","), vec!["a", "b", "c"]);
        assert_eq!(split_string("a, b, c", ","), vec!["a", "b", "c"]);
        assert_eq!(split_string("", ","), Vec::<String>::new());
    }

    #[test]
    fn test_long_alias() {
        // "long" is alias for integer in C++ MaterialX
        assert_eq!(
            create_value_from_strings("42", "long"),
            Some(MtlxValue::Int(42))
        );
        assert_eq!(
            create_value_from_strings("-10", "long"),
            Some(MtlxValue::Int(-10))
        );
    }

    #[test]
    fn test_double_alias() {
        // "double" is alias for float in C++ MaterialX
        assert_eq!(
            create_value_from_strings("3.14", "double"),
            Some(MtlxValue::Float(3.14))
        );
        assert_eq!(
            create_value_from_strings("-2.5", "double"),
            Some(MtlxValue::Float(-2.5))
        );
    }
}
