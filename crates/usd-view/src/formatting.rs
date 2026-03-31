//! Value formatting utilities for USD viewer.
//!
//! Matches reference prettyPrint.py + scalarTypes.py from usdviewq.
//! Provides human-readable formatting for USD attribute values, matrices,
//! bounding boxes, arrays, and file sizes.

use usd_gf::{BBox3d, Vec2f, Vec3d, Vec3f, Vec4f};
use usd_vt::Value;

/// Format an integer with thousands separators (e.g. 1,234,567).
pub fn fmt_int(n: i64) -> String {
    if n < 0 {
        return format!("-{}", fmt_int(-n));
    }
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// Format a VtValue for display (full representation, max 5 array elements).
pub fn fmt_val(val: &Value) -> String {
    fmt_val_n(val, 5)
}

/// Format a VtValue for display with configurable max array element count.
pub fn fmt_val_n(val: &Value, max_items: usize) -> String {
    // Scalars
    if let Some(s) = val.get::<String>() {
        return format!("\"{}\"", s);
    }
    if let Some(t) = val.get::<usd_tf::Token>() {
        return t.get_text().to_string();
    }
    if let Some(&b) = val.get::<bool>() {
        return if b { "true" } else { "false" }.to_string();
    }
    if let Some(&i) = val.get::<i32>() {
        return i.to_string();
    }
    if let Some(&i) = val.get::<i64>() {
        return i.to_string();
    }
    if let Some(&f) = val.get::<f32>() {
        return fmt_float(f as f64);
    }
    if let Some(&f) = val.get::<f64>() {
        return fmt_float(f);
    }

    // Vectors
    if let Some(v) = val.get::<Vec2f>() {
        return format!("({}, {})", fmt_float(v.x as f64), fmt_float(v.y as f64));
    }
    if let Some(v) = val.get::<Vec3f>() {
        return fmt_vec3f(v);
    }
    if let Some(v) = val.get::<Vec3d>() {
        return fmt_vec3d(v);
    }
    if let Some(v) = val.get::<Vec4f>() {
        return format!(
            "({}, {}, {}, {})",
            fmt_float(v.x as f64),
            fmt_float(v.y as f64),
            fmt_float(v.z as f64),
            fmt_float(v.w as f64)
        );
    }

    // SdfPath
    if let Some(p) = val.get::<usd_sdf::Path>() {
        return p.to_string();
    }

    // Arrays (show scalar type prefix + count + first max_items elements)
    // Format: "type[N]: [elem, elem, ...]", matching Python's common.py
    if let Some(arr) = val.get::<Vec<i32>>() {
        return fmt_typed_arr_n(arr, max_items, "int", |x| x.to_string());
    }
    if let Some(arr) = val.get::<Vec<i64>>() {
        return fmt_typed_arr_n(arr, max_items, "int64", |x| x.to_string());
    }
    if let Some(arr) = val.get::<Vec<f32>>() {
        return fmt_typed_arr_n(arr, max_items, "float", |x| fmt_float(*x as f64));
    }
    if let Some(arr) = val.get::<Vec<f64>>() {
        return fmt_typed_arr_n(arr, max_items, "double", |x| fmt_float(*x));
    }
    if let Some(arr) = val.get::<Vec<String>>() {
        return fmt_typed_arr_n(arr, max_items, "string", |s| format!("\"{}\"", s));
    }
    if let Some(arr) = val.get::<Vec<usd_tf::Token>>() {
        return fmt_typed_arr_n(arr, max_items, "token", |t| t.get_text().to_string());
    }
    if let Some(arr) = val.get::<Vec<usd_sdf::Path>>() {
        return fmt_typed_arr_n(arr, max_items, "path", |p| p.to_string());
    }
    if let Some(arr) = val.get::<Vec<Vec3f>>() {
        return fmt_typed_arr_n(arr, max_items, "Vec3f", |v| fmt_vec3f(v));
    }
    if let Some(arr) = val.get::<Vec<Vec3d>>() {
        return fmt_typed_arr_n(arr, max_items, "Vec3d", |v| fmt_vec3d(v));
    }

    // Generic array fallback
    if val.is_array_valued() {
        return format!("[array: {} elements]", val.array_size());
    }

    format!("{:?}", val)
}

/// Format a page slice of an array VtValue with indexed rows.
///
/// Shows elements from `start..end` with format "idx: value" per line.
/// Matches Python arrayAttributeView.py display style.
pub fn fmt_val_page(val: &Value, start: usize, end: usize) -> String {
    // Try each known array type and format the slice with indices
    macro_rules! try_arr {
        ($ty:ty, $fmt:expr) => {
            if let Some(arr) = val.get::<Vec<$ty>>() {
                let s = start.min(arr.len());
                let e = end.min(arr.len());
                if s >= e {
                    return "(empty slice)".to_string();
                }
                let mut lines = Vec::with_capacity(e - s);
                for i in s..e {
                    lines.push(format!("{}: {}", i, $fmt(&arr[i])));
                }
                return lines.join("\n");
            }
        };
    }
    try_arr!(i32, |x: &i32| x.to_string());
    try_arr!(i64, |x: &i64| x.to_string());
    try_arr!(f32, |x: &f32| fmt_float(*x as f64));
    try_arr!(f64, |x: &f64| fmt_float(*x));
    try_arr!(String, |s: &String| format!("\"{}\"", s));
    try_arr!(usd_tf::Token, |t: &usd_tf::Token| t.get_text().to_string());
    try_arr!(usd_sdf::Path, |p: &usd_sdf::Path| p.to_string());
    try_arr!(usd_gf::Vec3f, |v: &usd_gf::Vec3f| fmt_vec3f(v));
    try_arr!(usd_gf::Vec3d, |v: &usd_gf::Vec3d| fmt_vec3d(v));

    // Fallback for unknown array types
    if val.is_array_valued() {
        return format!(
            "[array: {} elements, page {}-{}]",
            val.array_size(),
            start,
            end
        );
    }
    fmt_val(val)
}

/// Format a VtValue for display, truncated to max_len characters.
pub fn fmt_val_short(val: &Value, max_len: usize) -> String {
    let full = fmt_val(val);
    if full.len() <= max_len {
        full
    } else {
        // Safe UTF-8 truncation: find char boundary to avoid panic on multi-byte chars
        let trunc = max_len.saturating_sub(3);
        let boundary = full.floor_char_boundary(trunc);
        let mut s = full[..boundary].to_string();
        s.push_str("...");
        s
    }
}

/// Format a Vec3f as "(x, y, z)".
pub fn fmt_vec3f(v: &Vec3f) -> String {
    format!(
        "({}, {}, {})",
        fmt_float(v.x as f64),
        fmt_float(v.y as f64),
        fmt_float(v.z as f64)
    )
}

/// Format a Vec3d as "(x, y, z)".
pub fn fmt_vec3d(v: &Vec3d) -> String {
    format!(
        "({}, {}, {})",
        fmt_float(v.x),
        fmt_float(v.y),
        fmt_float(v.z)
    )
}

/// Format a 4x4 matrix as 4 rows, one per line.
///
/// Uses the matrix row accessor if available; otherwise formats raw data.
pub fn fmt_matrix4(data: &[f64; 16]) -> String {
    // Row-major layout: data[row*4 + col]
    let mut lines = Vec::with_capacity(4);
    for row in 0..4 {
        let r = row * 4;
        lines.push(format!(
            "  [{}, {}, {}, {}]",
            fmt_float(data[r]),
            fmt_float(data[r + 1]),
            fmt_float(data[r + 2]),
            fmt_float(data[r + 3])
        ));
    }
    format!("[\n{}\n]", lines.join("\n"))
}

/// Format a bounding box range as "min=(x,y,z) max=(x,y,z)".
pub fn fmt_bbox(min: &Vec3d, max: &Vec3d) -> String {
    format!(
        "min=({}, {}, {})  max=({}, {}, {})",
        fmt_float(min.x),
        fmt_float(min.y),
        fmt_float(min.z),
        fmt_float(max.x),
        fmt_float(max.y),
        fmt_float(max.z)
    )
}

/// Detailed BBox3d formatting matching Python's scalarTypes.py bboxToString.
///
/// Shows: box diagonal endpoints, center, dimensions, volume,
/// matrix, zero-area primitives flag, and world-space range.
pub fn fmt_bbox3d(bbox: &BBox3d) -> String {
    let range = bbox.range();
    if range.is_empty() {
        return "(empty)".to_string();
    }

    let mut result = String::new();

    // Box diagonal endpoints (corner 0 = min, corner 7 = max)
    let c0 = range.corner(0);
    let c7 = range.corner(7);
    result.push_str(&format!(
        "Endpts of box diagonal:\n  ({}, {}, {})\n  ({}, {}, {})\n",
        fmt_float(c0.x),
        fmt_float(c0.y),
        fmt_float(c0.z),
        fmt_float(c7.x),
        fmt_float(c7.y),
        fmt_float(c7.z)
    ));

    // Center and dimensions
    let center = bbox.compute_centroid();
    let size = range.size();
    result.push_str(&format!(
        "Center: ({}, {}, {})\n",
        fmt_float(center.x),
        fmt_float(center.y),
        fmt_float(center.z)
    ));
    result.push_str(&format!(
        "Dimensions: {} x {} x {}\n",
        fmt_float(size.x),
        fmt_float(size.y),
        fmt_float(size.z)
    ));

    // Volume
    let vol = bbox.volume();
    result.push_str(&format!("Volume: {}\n", fmt_float(vol)));

    // Transform matrix
    let m = bbox.matrix();
    let data: [f64; 16] = {
        let s = m.as_slice();
        std::array::from_fn(|i| s[i])
    };
    result.push_str(&format!("Transform matrix:\n{}\n", fmt_matrix4(&data)));

    // Zero-area primitives flag
    if bbox.has_zero_area_primitives() {
        result.push_str("Has zero-area primitives\n");
    } else {
        result.push_str("Does not have zero-area primitives\n");
    }

    // Degenerate matrix detection
    if bbox.is_degenerate() {
        result.push_str("Matrix is degenerate (non-invertible)\n");
    }

    // World-space aligned range
    let ws = bbox.compute_aligned_range();
    result.push_str(&format!(
        "World-space range:\n  min=({}, {}, {})\n  max=({}, {}, {})",
        fmt_float(ws.min().x),
        fmt_float(ws.min().y),
        fmt_float(ws.min().z),
        fmt_float(ws.max().x),
        fmt_float(ws.max().y),
        fmt_float(ws.max().z)
    ));

    result
}

/// Format an array with count and first `max_items` elements.
pub fn fmt_arr<T, F>(arr: &[T], fmt: F) -> String
where
    F: Fn(&T) -> String,
{
    fmt_arr_n(arr, 5, fmt)
}

/// Format an array with count and first `max_items` elements (configurable).
pub fn fmt_arr_n<T, F>(arr: &[T], max_items: usize, fmt: F) -> String
where
    F: Fn(&T) -> String,
{
    if arr.is_empty() {
        return "[]".to_string();
    }
    let parts: Vec<String> = arr.iter().take(max_items).map(|x| fmt(x)).collect();
    let joined = parts.join(", ");
    if arr.len() > max_items {
        format!("[{}, ...{} more elements]", joined, arr.len() - max_items)
    } else {
        format!("[{}]", joined)
    }
}

/// Format a typed array with scalar type prefix: "type[N]: [elements...]".
///
/// Matches Python's common.py format: e.g. "float[100]: [1.0, 2.0, ...]".
pub fn fmt_typed_arr_n<T, F>(arr: &[T], max_items: usize, type_name: &str, fmt: F) -> String
where
    F: Fn(&T) -> String,
{
    if arr.is_empty() {
        return format!("{}[]", type_name);
    }
    format!(
        "{}[{}]: {}",
        type_name,
        arr.len(),
        fmt_arr_n(arr, max_items, fmt)
    )
}

/// Format file size as human-readable B/KB/MB/GB/TB.
pub fn fmt_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;
    let sz = bytes as f64;
    if sz >= TB {
        format!("{:.1} TB", sz / TB)
    } else if sz >= GB {
        format!("{:.1} GB", sz / GB)
    } else if sz >= MB {
        format!("{:.1} MB", sz / MB)
    } else if sz >= KB {
        format!("{:.1} KB", sz / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a float, trimming trailing zeros for clean display.
pub fn fmt_float(f: f64) -> String {
    if f == f.floor() && f.abs() < 1e15 {
        format!("{:.1}", f)
    } else {
        // Use enough precision without excessive trailing zeros
        let s = format!("{:.6}", f);
        let s = s.trim_end_matches('0');
        let s = s.trim_end_matches('.');
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_size() {
        assert_eq!(fmt_size(0), "0 B");
        assert_eq!(fmt_size(512), "512 B");
        assert_eq!(fmt_size(1024), "1.0 KB");
        assert_eq!(fmt_size(1_048_576), "1.0 MB");
    }

    #[test]
    fn test_fmt_float() {
        assert_eq!(fmt_float(1.0), "1.0");
        assert_eq!(fmt_float(3.14), "3.14");
        assert_eq!(fmt_float(0.0), "0.0");
    }

    #[test]
    fn test_fmt_arr_empty() {
        let empty: Vec<i32> = vec![];
        assert_eq!(fmt_arr(&empty, |x| x.to_string()), "[]");
    }

    #[test]
    fn test_fmt_arr_truncation() {
        let big: Vec<i32> = (0..20).collect();
        let s = fmt_arr(&big, |x| x.to_string());
        assert!(s.contains("15 more elements"));
    }

    #[test]
    fn test_fmt_typed_arr() {
        let arr = vec![1.0f32, 2.0, 3.0];
        let s = fmt_typed_arr_n(&arr, 5, "float", |x| fmt_float(*x as f64));
        assert_eq!(s, "float[3]: [1.0, 2.0, 3.0]");
    }

    #[test]
    fn test_fmt_typed_arr_empty() {
        let arr: Vec<f32> = vec![];
        let s = fmt_typed_arr_n(&arr, 5, "float", |x| fmt_float(*x as f64));
        assert_eq!(s, "float[]");
    }

    #[test]
    fn test_fmt_typed_arr_truncated() {
        let arr: Vec<i32> = (0..10).collect();
        let s = fmt_typed_arr_n(&arr, 3, "int", |x| x.to_string());
        assert!(s.starts_with("int[10]: [0, 1, 2,"));
        assert!(s.contains("7 more elements"));
    }
}
