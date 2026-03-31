//! HdCollectionPredicateLibrary - predicate functions for scene index collections.
//!
//! Port of pxr/imaging/hd/collectionPredicateLibrary.h/cpp
//!
//! Provides predicate functions for SdfPathExpression evaluation on scene index
//! prims. Available predicates:
//!
//! - `hdType(primType)` / `type(primType)` — match by prim type string
//! - `hdVisible(bool = true)` / `visible` — match by authored visibility
//! - `hdPurpose(purpose)` / `purpose` — match by authored purpose token
//! - `hdHasDataSource("a.b.c")` / `hasDataSource` — presence test by locator
//! - `hdHasPrimvar(name)` / `hasPrimvar` — primvar presence test
//! - `hdHasMaterialBinding(substr)` / `hasMaterialBinding` — allPurpose
//!   binding path substring match

use crate::data_source::cast_to_container;
use crate::scene_index::HdSceneIndexPrim;
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_sdf::{FnArg, PredicateLibFunctionResult as PredResult, PredicateLibrary};
use usd_tf::Token;

/// Type alias: `SdfPredicateLibrary<HdSceneIndexPrim>`.
pub type HdCollectionPredicateLibrary = PredicateLibrary<HdSceneIndexPrim>;

/// Shared default library instance.
static LIBRARY: Lazy<HdCollectionPredicateLibrary> = Lazy::new(build_library);

/// Return the default collection predicate library.
pub fn hd_get_collection_predicate_library() -> &'static HdCollectionPredicateLibrary {
    &LIBRARY
}

// ---------------------------------------------------------------------------
// Argument helpers
// ---------------------------------------------------------------------------

/// Extract a `String` from the first positional arg.
fn first_string(args: &[FnArg]) -> Option<String> {
    args.first().and_then(|a| a.value.get::<String>().cloned())
}

/// Extract a `bool` from the first positional arg, defaulting to `true`.
fn first_bool_or_true(args: &[FnArg]) -> bool {
    args.first()
        .and_then(|a| a.value.get::<bool>().copied())
        .unwrap_or(true)
}

// ---------------------------------------------------------------------------
// Data source navigation helpers
// ---------------------------------------------------------------------------

/// Walk a dot-delimited locator string through a container and return whether
/// the final key exists.
///
/// E.g. `"primvars.fresh"` → look up `"primvars"` first, then `"fresh"` inside.
fn has_locator(prim: &HdSceneIndexPrim, locator: &str) -> bool {
    let ds = match &prim.data_source {
        Some(ds) => ds.clone(),
        None => return false,
    };

    let tokens: Vec<&str> = locator.split('.').collect();
    if tokens.is_empty() {
        return false;
    }

    let mut current = ds;
    let last = tokens.len() - 1;
    for (i, tok) in tokens.iter().enumerate() {
        let child = match current.get(&Token::new(tok)) {
            Some(c) => c,
            None => return false,
        };
        if i == last {
            return true; // Presence check — value doesn't matter.
        }
        // Must be a container to descend further.
        current = match cast_to_container(&child) {
            Some(c) => c,
            None => return false,
        };
    }
    true
}

/// Read a `bool` from `{ outer_key: { inner_key: <bool> } }` in the prim DS.
///
/// Used for the visibility schema layout:
/// `prim.data_source["visibility"]["visibility"] = <bool>`.
fn read_nested_bool(prim: &HdSceneIndexPrim, outer_key: &str, inner_key: &str) -> Option<bool> {
    let ds = prim.data_source.as_ref()?;
    let outer = ds.get(&Token::new(outer_key))?;
    let outer_c = cast_to_container(&outer)?;
    let inner = outer_c.get(&Token::new(inner_key))?;
    let sampled = inner.as_sampled()?;
    sampled.get_value(0.0).get::<bool>().copied()
}

/// Read a `Token` from `{ outer_key: { inner_key: <Token> } }` in the prim DS.
///
/// Used for the purpose schema layout:
/// `prim.data_source["purpose"]["purpose"] = <Token>`.
fn read_nested_token(prim: &HdSceneIndexPrim, outer_key: &str, inner_key: &str) -> Option<Token> {
    let ds = prim.data_source.as_ref()?;
    let outer = ds.get(&Token::new(outer_key))?;
    let outer_c = cast_to_container(&outer)?;
    let inner = outer_c.get(&Token::new(inner_key))?;
    let sampled = inner.as_sampled()?;
    sampled.get_value(0.0).get::<Token>().cloned()
}

/// Read the allPurpose (key = `""`) material binding path string from:
/// `prim.data_source["materialBindings"][""]["path"] = <SdfPath>`.
fn read_all_purpose_material_binding(prim: &HdSceneIndexPrim) -> Option<String> {
    let ds = prim.data_source.as_ref()?;
    let bindings = ds.get(&Token::new("materialBindings"))?;
    let bindings_c = cast_to_container(&bindings)?;
    // allPurpose is stored under the empty-string key ("").
    let all_purpose = bindings_c.get(&Token::new(""))?;
    let all_purpose_c = cast_to_container(&all_purpose)?;
    let path_ds = all_purpose_c.get(&Token::new("path"))?;
    let sampled = path_ds.as_sampled()?;
    // The value is stored as SdfPath; get its string representation.
    let val = sampled.get_value(0.0);
    if let Some(p) = val.get::<usd_sdf::Path>() {
        return Some(p.get_string().to_string());
    }
    // Fallback: try string directly.
    val.get::<String>().cloned()
}

// ---------------------------------------------------------------------------
// Build the library
// ---------------------------------------------------------------------------

fn build_library() -> HdCollectionPredicateLibrary {
    let lib = HdCollectionPredicateLibrary::new();

    // -------------------------------------------------------------------
    // hdType / type — match prim type string
    // -------------------------------------------------------------------
    let lib = lib.define_binder("hdType", |args| {
        let expected_type = first_string(args)?;
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            PredResult::make_varying(prim.prim_type.as_str() == expected_type.as_str())
        }))
    });
    // Deprecated alias.
    let lib = lib.define_binder("type", |args| {
        let expected_type = first_string(args)?;
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            PredResult::make_varying(prim.prim_type.as_str() == expected_type.as_str())
        }))
    });

    // -------------------------------------------------------------------
    // hdVisible / visible — match authored visibility bool
    //
    // Data layout (HdVisibilitySchema):
    //   prim.data_source["visibility"]["visibility"] = <bool>
    //
    // If the key is absent the prim has no visibility opinion → returns false.
    // -------------------------------------------------------------------
    let lib = lib.define_binder("hdVisible", |args| {
        let expected = first_bool_or_true(args);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let result = read_nested_bool(prim, "visibility", "visibility")
                .map(|v| v == expected)
                .unwrap_or(false); // no opinion → false
            PredResult::make_varying(result)
        }))
    });
    // Deprecated alias.
    let lib = lib.define_binder("visible", |args| {
        let expected = first_bool_or_true(args);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let result = read_nested_bool(prim, "visibility", "visibility")
                .map(|v| v == expected)
                .unwrap_or(false);
            PredResult::make_varying(result)
        }))
    });

    // -------------------------------------------------------------------
    // hdPurpose / purpose — match authored purpose token
    //
    // Data layout (HdPurposeSchema):
    //   prim.data_source["purpose"]["purpose"] = <Token>
    // -------------------------------------------------------------------
    let lib = lib.define_binder("hdPurpose", |args| {
        let target = Token::new(&first_string(args)?);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let result = read_nested_token(prim, "purpose", "purpose")
                .map(|t| t == target)
                .unwrap_or(false);
            PredResult::make_varying(result)
        }))
    });
    // Deprecated alias.
    let lib = lib.define_binder("purpose", |args| {
        let target = Token::new(&first_string(args)?);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let result = read_nested_token(prim, "purpose", "purpose")
                .map(|t| t == target)
                .unwrap_or(false);
            PredResult::make_varying(result)
        }))
    });

    // -------------------------------------------------------------------
    // hdHasDataSource / hasDataSource — data source presence test
    //
    // Argument is a dot-delimited locator string, e.g. "primvars.fresh".
    // -------------------------------------------------------------------
    let lib = lib.define_binder("hdHasDataSource", |args| {
        let locator = first_string(args)?;
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            PredResult::make_varying(has_locator(prim, &locator))
        }))
    });
    // Deprecated alias.
    let lib = lib.define_binder("hasDataSource", |args| {
        let locator = first_string(args)?;
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            PredResult::make_varying(has_locator(prim, &locator))
        }))
    });

    // -------------------------------------------------------------------
    // hdHasPrimvar / hasPrimvar — primvar presence test
    //
    // Data layout (HdPrimvarsSchema):
    //   prim.data_source["primvars"][<name>] = <container>
    // -------------------------------------------------------------------
    let lib = lib.define_binder("hdHasPrimvar", |args| {
        let name = Token::new(&first_string(args)?);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let found = prim
                .data_source
                .as_ref()
                .and_then(|ds| ds.get(&Token::new("primvars")))
                .and_then(|pv| cast_to_container(&pv))
                .and_then(|c| c.get(&name))
                .is_some();
            PredResult::make_varying(found)
        }))
    });
    // Deprecated alias.
    let lib = lib.define_binder("hasPrimvar", |args| {
        let name = Token::new(&first_string(args)?);
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let found = prim
                .data_source
                .as_ref()
                .and_then(|ds| ds.get(&Token::new("primvars")))
                .and_then(|pv| cast_to_container(&pv))
                .and_then(|c| c.get(&name))
                .is_some();
            PredResult::make_varying(found)
        }))
    });

    // -------------------------------------------------------------------
    // hdHasMaterialBinding / hasMaterialBinding — allPurpose binding substring
    //
    // Queries the allPurpose ("") material binding path and checks whether
    // the given string is a substring of that path.
    // -------------------------------------------------------------------
    let lib = lib.define_binder("hdHasMaterialBinding", |args| {
        let substr = first_string(args)?;
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let result = read_all_purpose_material_binding(prim)
                .map(|path_str| path_str.contains(substr.as_str()))
                .unwrap_or(false);
            PredResult::make_varying(result)
        }))
    });
    // Deprecated alias.
    let lib = lib.define_binder("hasMaterialBinding", |args| {
        let substr = first_string(args)?;
        Some(Arc::new(move |prim: &HdSceneIndexPrim| {
            let result = read_all_purpose_material_binding(prim)
                .map(|path_str| path_str.contains(substr.as_str()))
                .unwrap_or(false);
            PredResult::make_varying(result)
        }))
    });

    lib
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_has_predicates() {
        let lib = hd_get_collection_predicate_library();
        assert!(lib.has_function("hdType"), "hdType missing");
        assert!(lib.has_function("hdVisible"), "hdVisible missing");
        assert!(lib.has_function("hdPurpose"), "hdPurpose missing");
        assert!(
            lib.has_function("hdHasDataSource"),
            "hdHasDataSource missing"
        );
        assert!(lib.has_function("hdHasPrimvar"), "hdHasPrimvar missing");
        assert!(
            lib.has_function("hdHasMaterialBinding"),
            "hdHasMaterialBinding missing"
        );
        assert!(lib.has_function("type"), "deprecated 'type' alias missing");
    }
}
