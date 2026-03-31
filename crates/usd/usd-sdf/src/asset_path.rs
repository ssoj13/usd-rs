//! Layer-level utility functions for anchoring and resolving asset paths.
//!
//! The `AssetPath` and `AssetPathParams` types are now defined in `usd-vt` and
//! re-exported from `usd-sdf` for backward compatibility. This module only
//! contains utility functions that need access to SDF-level types (`Layer`,
//! `VariableExpression`).

// Re-export so `super::asset_path::AssetPath` paths in other sdf modules keep working.
pub use usd_vt::{AssetPath, AssetPathHash, AssetPathParams};

// =========================================================================
// Utility functions for anchoring and resolving asset paths
// =========================================================================

/// Anchors all paths in `asset_paths` to the `anchor` layer.
///
/// Evaluates any expression variables in `expr_vars`, then anchors relative
/// paths against the anchor layer's identifier.
pub fn anchor_asset_paths(
    anchor: &std::sync::Arc<crate::Layer>,
    expr_vars: &usd_vt::Dictionary,
    asset_paths: &mut [AssetPath],
    errors: &mut Vec<String>,
) {
    resolve_or_anchor_asset_paths(anchor, expr_vars, asset_paths, false, errors);
}

/// Anchors and resolves `asset_paths` with respect to the `anchor` layer.
///
/// Evaluates expression variables in `expr_vars`, then both anchors and
/// resolves each path via the asset resolution system.
pub fn resolve_asset_paths(
    anchor: &std::sync::Arc<crate::Layer>,
    expr_vars: &usd_vt::Dictionary,
    asset_paths: &mut [AssetPath],
    errors: &mut Vec<String>,
) {
    resolve_or_anchor_asset_paths(anchor, expr_vars, asset_paths, true, errors);
}

/// Swaps two `AssetPath` values. Kept for C++ API compatibility.
pub fn swap(lhs: &mut AssetPath, rhs: &mut AssetPath) {
    std::mem::swap(lhs, rhs);
}

fn resolve_or_anchor_asset_paths(
    anchor: &std::sync::Arc<crate::Layer>,
    expr_vars: &usd_vt::Dictionary,
    asset_paths: &mut [AssetPath],
    set_resolved_path: bool,
    errors: &mut Vec<String>,
) {
    use crate::{Layer, layer_utils, variable_expression::VariableExpression};

    for asset_path in asset_paths.iter_mut() {
        // Evaluate expression variables if the authored path is an expression.
        let authored = asset_path.get_authored_path().to_owned();
        if VariableExpression::is_expression(&authored) {
            let expr = VariableExpression::new(&authored);
            let result = expr.evaluate(expr_vars);
            if !result.errors.is_empty() {
                errors.extend(result.errors);
                continue;
            }
            if let Some(value) = result.value {
                if let Some(evaluated_str) = value.get::<String>() {
                    asset_path.set_evaluated_path(evaluated_str.clone());
                }
            }
        }

        if set_resolved_path {
            let asset_path_str = asset_path.get_asset_path().to_owned();
            let resolved =
                layer_utils::resolve_asset_path_relative_to_layer(anchor, &asset_path_str);
            asset_path.set_resolved_path(resolved);
        } else {
            let asset_path_str = asset_path.get_asset_path().to_owned();

            // Skip empty paths and anonymous layer identifiers.
            if asset_path_str.is_empty() || Layer::is_anonymous_layer_identifier(&asset_path_str) {
                continue;
            }

            let anchored_path =
                layer_utils::compute_asset_path_relative_to_layer(anchor, &asset_path_str);

            if anchored_path != asset_path_str {
                *asset_path = AssetPath::new(anchored_path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use usd_vt::{AssetPath, AssetPathParams};

    #[test]
    fn test_empty() {
        let path = AssetPath::empty();
        assert!(path.get_authored_path().is_empty());
        assert!(path.get_evaluated_path().is_empty());
        assert!(path.get_resolved_path().is_empty());
        assert!(path.is_empty());
    }

    #[test]
    fn test_new() {
        let path = AssetPath::new("model.usd");
        assert_eq!(path.get_authored_path(), "model.usd");
        assert!(path.get_evaluated_path().is_empty());
        assert!(path.get_resolved_path().is_empty());
        assert!(!path.is_empty());
    }

    #[test]
    fn test_with_resolved() {
        let path = AssetPath::with_resolved("model.usd", "/root/model.usd");
        assert_eq!(path.get_authored_path(), "model.usd");
        assert!(path.get_evaluated_path().is_empty());
        assert_eq!(path.get_resolved_path(), "/root/model.usd");
    }

    #[test]
    fn test_from_params() {
        let path = AssetPath::from_params(
            AssetPathParams::new()
                .authored("model_{VAR}.usd")
                .evaluated("model_a.usd")
                .resolved("/root/model_a.usd"),
        );
        assert_eq!(path.get_authored_path(), "model_{VAR}.usd");
        assert_eq!(path.get_evaluated_path(), "model_a.usd");
        assert_eq!(path.get_resolved_path(), "/root/model_a.usd");
    }

    #[test]
    fn test_get_asset_path_prefers_evaluated() {
        let path = AssetPath::from_params(
            AssetPathParams::new()
                .authored("model_{VAR}.usd")
                .evaluated("model_a.usd"),
        );
        assert_eq!(path.get_asset_path(), "model_a.usd");
    }

    #[test]
    fn test_equality() {
        let path1 = AssetPath::new("model.usd");
        let path2 = AssetPath::new("model.usd");
        let path3 = AssetPath::new("other.usd");
        assert_eq!(path1, path2);
        assert_ne!(path1, path3);
    }

    #[test]
    fn test_ordering() {
        let path1 = AssetPath::new("a.usd");
        let path2 = AssetPath::new("b.usd");
        assert!(path1 < path2);
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;
        let path1 = AssetPath::new("model.usd");
        let path2 = AssetPath::new("model.usd");
        let path3 = AssetPath::new("other.usd");
        let mut set = HashSet::new();
        set.insert(path1.clone());
        assert!(set.contains(&path2));
        assert!(!set.contains(&path3));
    }

    #[test]
    fn test_invalid_path_with_control_chars() {
        let path = AssetPath::new("model\x00.usd");
        assert!(path.is_empty());
        let path2 = AssetPath::new("model\x1F.usd");
        assert!(path2.is_empty());
    }

    #[test]
    fn test_valid_unicode() {
        let path = AssetPath::new("модель.usd");
        assert_eq!(path.get_authored_path(), "модель.usd");
    }
}
