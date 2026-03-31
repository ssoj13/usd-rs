//! RenderMan utilities.
//!
//! Utilities for converting between USD encodings and RenderMan encodings.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/rmanUtilities.h`

use usd_sdf::StringListOp;
use usd_tf::Token;

/// RenderMan interpolate boundary enum values.
///
/// Values match C++ UsdRiConvertToRManInterpolateBoundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RManInterpolateBoundary {
    /// No interpolation at boundaries.
    None = 0,
    /// Edge and corner interpolation.
    EdgeAndCorner = 1,
    /// Edge only interpolation.
    EdgeOnly = 2,
}

/// RenderMan face-varying linear interpolation enum values.
///
/// Values match C++ UsdRiConvertToRManFaceVaryingLinearInterpolation.
/// Note: cornersOnly, cornersPlus1, cornersPlus2 all map to rman value 1.
/// The canonical reverse mapping for 1 is cornersPlus1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RManFaceVaryingLinearInterpolation {
    /// All linear interpolation.
    All = 0,
    /// Corners plus 1 (also covers cornersOnly and cornersPlus2).
    CornersPlus1 = 1,
    /// No linear interpolation.
    None = 2,
    /// Boundaries.
    Boundaries = 3,
}

/// RenderMan triangle subdivision rule enum values.
///
/// Values match C++ UsdRiConvertToRManTriangleSubdivisionRule.
/// Note: smooth uses value 2 (not 1) per RenderMan spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RManTriangleSubdivisionRule {
    /// Catmull-Clark rule.
    CatmullClark = 0,
    /// Smooth rule (value 2 required for the smoothing algorithm).
    Smooth = 2,
}

// UsdGeom tokens for boundary interpolation
const EDGE_NONE: &str = "none";
const EDGE_ONLY: &str = "edgeOnly";
const EDGE_AND_CORNER: &str = "edgeAndCorner";

// UsdGeom tokens for face-varying interpolation
const FV_NONE: &str = "none";
const FV_CORNERS_ONLY: &str = "cornersOnly";
const FV_CORNERS_PLUS1: &str = "cornersPlus1";
const FV_CORNERS_PLUS2: &str = "cornersPlus2";
const FV_BOUNDARIES: &str = "boundaries";
const FV_ALL: &str = "all";

// UsdGeom tokens for triangle subdivision
const TRI_CATMULL_CLARK: &str = "catmullClark";
const TRI_SMOOTH: &str = "smooth";

/// Convert a UsdGeom interpolate boundary token to RenderMan enum.
///
/// Given a token representing a UsdGeom interpolate boundary value,
/// returns corresponding rman enum (converted to int).
pub fn convert_to_rman_interpolate_boundary(token: &Token) -> i32 {
    match token.as_str() {
        EDGE_NONE => 0,
        EDGE_AND_CORNER => 1,
        EDGE_ONLY => 2,
        _ => {
            log::error!("Invalid InterpolateBoundary Token: {}", token.as_str());
            0
        }
    }
}

/// Convert a RenderMan interpolate boundary enum to UsdGeom token.
///
/// Given the integer that corresponds to an rman enum for interpolate
/// boundary condition, returns the equivalent UsdGeom token.
pub fn convert_from_rman_interpolate_boundary(i: i32) -> Token {
    match i {
        0 => Token::new(EDGE_NONE),
        1 => Token::new(EDGE_AND_CORNER),
        2 => Token::new(EDGE_ONLY),
        _ => {
            log::error!("Invalid InterpolateBoundary int: {}", i);
            Token::new(EDGE_NONE)
        }
    }
}

/// Convert a UsdGeom face-varying interpolation token to RenderMan enum.
///
/// Given a token representing a UsdGeom face-varying interpolate boundary
/// value, returns corresponding rman enum (converted to int).
/// Note: cornersOnly, cornersPlus1, cornersPlus2 all map to 1.
pub fn convert_to_rman_face_varying_linear_interpolation(token: &Token) -> i32 {
    match token.as_str() {
        FV_ALL => 0,
        FV_CORNERS_ONLY | FV_CORNERS_PLUS1 | FV_CORNERS_PLUS2 => 1,
        FV_NONE => 2,
        FV_BOUNDARIES => 3,
        _ => {
            log::error!(
                "Invalid FaceVaryingLinearInterpolation Token: {}",
                token.as_str()
            );
            1
        }
    }
}

/// Convert a RenderMan face-varying interpolation enum to UsdGeom token.
///
/// Given the integer that corresponds to an rman enum for face-varying
/// interpolate boundary condition, returns the equivalent UsdGeom token.
/// The canonical token for value 1 is "cornersPlus1".
pub fn convert_from_rman_face_varying_linear_interpolation(i: i32) -> Token {
    match i {
        0 => Token::new(FV_ALL),
        1 => Token::new(FV_CORNERS_PLUS1),
        2 => Token::new(FV_NONE),
        3 => Token::new(FV_BOUNDARIES),
        _ => {
            log::error!("Invalid FaceVaryingLinearInterpolation int: {}", i);
            Token::new(FV_NONE)
        }
    }
}

/// Convert a UsdGeom triangle subdivision rule token to RenderMan enum.
///
/// Given a token representing a UsdGeom Catmull-Clark triangle subdivision
/// rule value, returns corresponding rman enum (converted to int).
pub fn convert_to_rman_triangle_subdivision_rule(token: &Token) -> i32 {
    match token.as_str() {
        // Value 2 is needed for the smoothing algorithm to work.
        TRI_CATMULL_CLARK => 0,
        TRI_SMOOTH => 2,
        _ => {
            log::error!("Invalid TriangleSubdivisionRule Token: {}", token.as_str());
            0
        }
    }
}

/// Convert a RenderMan triangle subdivision rule enum to UsdGeom token.
///
/// Given the integer that corresponds to an rman enum for a Catmull-Clark
/// triangle subdivision rule, returns the equivalent UsdGeom token.
pub fn convert_from_rman_triangle_subdivision_rule(i: i32) -> Token {
    // Value 2 is needed for the smoothing algorithm to work.
    match i {
        0 => Token::new(TRI_CATMULL_CLARK),
        2 => Token::new(TRI_SMOOTH),
        _ => {
            log::error!("Invalid TriangleSubdivisionRule int: {}", i);
            Token::new(TRI_CATMULL_CLARK)
        }
    }
}

/// Convert a RenderMan set specification to SdfStringListOp.
///
/// RenderMan specifies certain set operations using a string encoding.
/// The string form contains either a list of named groups, or a unary
/// operator ("+" or "-") followed by a list of named groups.
/// In set-algebra terms "+" is a union and "-" is a difference operator.
///
/// This method converts the string form to an equivalent USD type,
/// SdfStringListOp.
///
/// # Note
///
/// SdfStringListOp is more expressive than the RenderMan grouping
/// membership representation, so lossless round-trip conversion
/// is not possible in general.
pub fn convert_rman_set_specification_to_list_op(spec: &str) -> StringListOp {
    let trimmed = spec.trim();

    if trimmed.is_empty() {
        return StringListOp::default();
    }

    // Check for unary operators (C++ checks repr[0])
    let first_char = trimmed.as_bytes()[0];
    let mut list_op = StringListOp::default();

    if first_char == b'+' {
        // Union operation - appended items
        let items: Vec<String> = tokenize_set_spec(&trimmed[1..]);
        let _ = list_op.set_appended_items(items);
    } else if first_char == b'-' {
        // Difference operation - deleted items
        let items: Vec<String> = tokenize_set_spec(&trimmed[1..]);
        let _ = list_op.set_deleted_items(items);
    } else {
        // No operator - explicit items
        let items: Vec<String> = tokenize_set_spec(trimmed);
        let _ = list_op.set_explicit_items(items);
    }

    list_op
}

/// Tokenize a set specification string by whitespace, tabs, newlines, and commas.
///
/// Matches C++ TfStringTokenize(repr, " \t\n,").
fn tokenize_set_spec(s: &str) -> Vec<String> {
    s.split(|c: char| c == ' ' || c == '\t' || c == '\n' || c == ',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Check if an attribute uses set specification representation.
///
/// Return true if the given attribute name uses a string set specification
/// representation in the RenderMan interface.
///
/// Uses suffix matching (ends_with) to be robust with regard to various
/// namespacing, e.g. primvars:ri:attributes, ri:attributes, or none at all.
///
/// This includes:
/// - grouping:membership
/// - lighting:excludesubset
/// - lighting:subset
/// - lightfilter:subset
pub fn does_attribute_use_set_specification(attr_name: &Token) -> bool {
    let s = attr_name.as_str();
    s.ends_with("grouping:membership")
        || s.ends_with("lighting:excludesubset")
        || s.ends_with("lighting:subset")
        || s.ends_with("lightfilter:subset")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_boundary_conversion() {
        // C++: none=0, edgeAndCorner=1, edgeOnly=2
        assert_eq!(convert_to_rman_interpolate_boundary(&Token::new("none")), 0);
        assert_eq!(
            convert_to_rman_interpolate_boundary(&Token::new("edgeAndCorner")),
            1
        );
        assert_eq!(
            convert_to_rman_interpolate_boundary(&Token::new("edgeOnly")),
            2
        );

        assert_eq!(convert_from_rman_interpolate_boundary(0).as_str(), "none");
        assert_eq!(
            convert_from_rman_interpolate_boundary(1).as_str(),
            "edgeAndCorner"
        );
        assert_eq!(
            convert_from_rman_interpolate_boundary(2).as_str(),
            "edgeOnly"
        );
    }

    #[test]
    fn test_face_varying_conversion() {
        // C++: all=0, cornersOnly/Plus1/Plus2=1, none=2, boundaries=3
        assert_eq!(
            convert_to_rman_face_varying_linear_interpolation(&Token::new("all")),
            0
        );
        assert_eq!(
            convert_to_rman_face_varying_linear_interpolation(&Token::new("cornersOnly")),
            1
        );
        assert_eq!(
            convert_to_rman_face_varying_linear_interpolation(&Token::new("cornersPlus1")),
            1
        );
        assert_eq!(
            convert_to_rman_face_varying_linear_interpolation(&Token::new("cornersPlus2")),
            1
        );
        assert_eq!(
            convert_to_rman_face_varying_linear_interpolation(&Token::new("none")),
            2
        );
        assert_eq!(
            convert_to_rman_face_varying_linear_interpolation(&Token::new("boundaries")),
            3
        );

        // Reverse: canonical token for 1 is "cornersPlus1"
        assert_eq!(
            convert_from_rman_face_varying_linear_interpolation(0).as_str(),
            "all"
        );
        assert_eq!(
            convert_from_rman_face_varying_linear_interpolation(1).as_str(),
            "cornersPlus1"
        );
        assert_eq!(
            convert_from_rman_face_varying_linear_interpolation(2).as_str(),
            "none"
        );
        assert_eq!(
            convert_from_rman_face_varying_linear_interpolation(3).as_str(),
            "boundaries"
        );
    }

    #[test]
    fn test_triangle_subdivision_conversion() {
        // C++: catmullClark=0, smooth=2 (not 1!)
        assert_eq!(
            convert_to_rman_triangle_subdivision_rule(&Token::new("catmullClark")),
            0
        );
        assert_eq!(
            convert_to_rman_triangle_subdivision_rule(&Token::new("smooth")),
            2
        );

        assert_eq!(
            convert_from_rman_triangle_subdivision_rule(0).as_str(),
            "catmullClark"
        );
        assert_eq!(
            convert_from_rman_triangle_subdivision_rule(2).as_str(),
            "smooth"
        );
    }

    #[test]
    fn test_set_specification_conversion() {
        // Test explicit items
        let list_op = convert_rman_set_specification_to_list_op("group1 group2");
        assert_eq!(list_op.get_explicit_items(), vec!["group1", "group2"]);

        // Test union (appended, not added)
        let list_op = convert_rman_set_specification_to_list_op("+ group3");
        assert_eq!(list_op.get_appended_items(), vec!["group3"]);

        // Test difference
        let list_op = convert_rman_set_specification_to_list_op("- group4");
        assert_eq!(list_op.get_deleted_items(), vec!["group4"]);

        // Test comma-separated
        let list_op = convert_rman_set_specification_to_list_op("a,b,c");
        assert_eq!(list_op.get_explicit_items(), vec!["a", "b", "c"]);

        // Test empty
        let list_op = convert_rman_set_specification_to_list_op("");
        assert!(list_op.get_explicit_items().is_empty());
    }

    #[test]
    fn test_attribute_uses_set_specification() {
        // Exact names
        assert!(does_attribute_use_set_specification(&Token::new(
            "grouping:membership"
        )));
        assert!(does_attribute_use_set_specification(&Token::new(
            "lighting:subset"
        )));
        assert!(!does_attribute_use_set_specification(&Token::new(
            "primvars:foo"
        )));
        // Namespaced names (suffix matching)
        assert!(does_attribute_use_set_specification(&Token::new(
            "ri:attributes:grouping:membership"
        )));
        assert!(does_attribute_use_set_specification(&Token::new(
            "primvars:ri:attributes:lighting:excludesubset"
        )));
    }
}
