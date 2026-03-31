//! UsdCollectionPredicateLibrary - predicate functions for collection membership.
//!
//! Port of pxr/usd/usd/collectionPredicateLibrary.h/cpp
//!
//! Provides the predicate library used to evaluate SdfPathExpressions in
//! UsdCollectionAPI's membershipExpression attributes.
//!
//! # Predicates
//!
//! - `abstract(isAbstract=true)` - Test if closest-prim is abstract
//! - `defined(isDefined=true)` - Test if closest-prim is defined
//! - `model(isModel=true)` - Test if object is a model prim
//! - `group(isGroup=true)` - Test if object is a group prim
//! - `kind(kind1, ..., strict=false)` - Test prim's kind metadata
//! - `specifier(spec1, ...)` - Test prim's specifier (def/over/class)
//! - `isa(schema1, ..., strict=false)` - Test prim's typed schema
//! - `hasAPI(api1, ..., instanceName=name)` - Test for applied API schemas
//! - `variant(set=selGlob, ...)` - Test variant selections

use std::sync::{Arc, LazyLock};

use crate::object::{ObjType, Object};
use usd_sdf::predicate_library::{FromValue, PredicateFunctionResult, PredicateLibrary};
use usd_sdf::{FnArg, Specifier};
use usd_tf::Token;

/// Type alias matching C++ `UsdObjectPredicateLibrary`.
pub type UsdObjectPredicateLibrary = PredicateLibrary<Object>;

/// Helper: extract `strict` keyword argument from FnArgs.
///
/// Returns `default_strict` if no `strict` argument is present.
fn is_strict(args: &[FnArg], default_strict: bool) -> bool {
    for arg in args {
        if arg.name == "strict" {
            if let Some(&b) = arg.value.get::<bool>() {
                return b;
            }
            if let Some(&i) = arg.value.get::<i32>() {
                return i != 0;
            }
            if let Some(&i) = arg.value.get::<i64>() {
                return i != 0;
            }
            if let Some(s) = arg.value.get::<String>() {
                if let Some(ch) = s.chars().next() {
                    return ch == '1' || ch == 'y' || ch == 'Y';
                }
            }
            return false;
        }
    }
    default_strict
}

/// Helper: get the Prim from an Object (the object itself if it's a prim,
/// or its owning prim if it's a property).
fn get_closest_prim(obj: &Object) -> Option<crate::Prim> {
    let stage = obj.stage()?;
    let prim_path = obj.prim_path();
    stage.get_prim_at_path(&prim_path)
}

/// Helper: true only if the object is a prim.
fn is_prim_obj(obj: &Object) -> bool {
    obj.obj_type() == ObjType::Prim
}

/// Build the collection predicate library.
///
/// Matches C++ `_MakeCollectionPredicateLibrary()`.
fn make_collection_predicate_library() -> UsdObjectPredicateLibrary {
    let lib = PredicateLibrary::new()
        // abstract(isAbstract=true)
        .define_binder("abstract", |args: &[FnArg]| {
            let want_abstract = args
                .first()
                .and_then(|a| {
                    if a.is_positional() || a.name == "isAbstract" {
                        bool::from_value(&a.value)
                    } else {
                        None
                    }
                })
                .unwrap_or(true);

            Some(Arc::new(move |obj: &Object| {
                let prim_is_abstract = get_closest_prim(obj)
                    .map(|p| p.is_abstract())
                    .unwrap_or(false);
                if prim_is_abstract || !is_prim_obj(obj) {
                    PredicateFunctionResult::make_constant(prim_is_abstract == want_abstract)
                } else {
                    PredicateFunctionResult::make_varying(prim_is_abstract == want_abstract)
                }
            }))
        })
        // defined(isDefined=true)
        .define_binder("defined", |args: &[FnArg]| {
            let want_defined = args
                .first()
                .and_then(|a| {
                    if a.is_positional() || a.name == "isDefined" {
                        bool::from_value(&a.value)
                    } else {
                        None
                    }
                })
                .unwrap_or(true);

            Some(Arc::new(move |obj: &Object| {
                let prim_is_defined = get_closest_prim(obj)
                    .map(|p| p.is_defined())
                    .unwrap_or(false);
                if !prim_is_defined || !is_prim_obj(obj) {
                    PredicateFunctionResult::make_constant(prim_is_defined == want_defined)
                } else {
                    PredicateFunctionResult::make_varying(prim_is_defined == want_defined)
                }
            }))
        })
        // model(isModel=true)
        .define_binder("model", |args: &[FnArg]| {
            let want_model = args
                .first()
                .and_then(|a| {
                    if a.is_positional() || a.name == "isModel" {
                        bool::from_value(&a.value)
                    } else {
                        None
                    }
                })
                .unwrap_or(true);

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim_is_model = get_closest_prim(obj).map(|p| p.is_model()).unwrap_or(false);
                if !prim_is_model {
                    PredicateFunctionResult::make_constant(prim_is_model == want_model)
                } else {
                    PredicateFunctionResult::make_varying(prim_is_model == want_model)
                }
            }))
        })
        // group(isGroup=true)
        .define_binder("group", |args: &[FnArg]| {
            let want_group = args
                .first()
                .and_then(|a| {
                    if a.is_positional() || a.name == "isGroup" {
                        bool::from_value(&a.value)
                    } else {
                        None
                    }
                })
                .unwrap_or(true);

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim_is_group = get_closest_prim(obj).map(|p| p.is_group()).unwrap_or(false);
                if !prim_is_group {
                    PredicateFunctionResult::make_constant(prim_is_group == want_group)
                } else {
                    PredicateFunctionResult::make_varying(prim_is_group == want_group)
                }
            }))
        })
        // kind(kind1, ..., strict=false)
        .define_binder("kind", |args: &[FnArg]| {
            let check_sub_kinds = !is_strict(args, false);

            // Collect unnamed string args as kind tokens.
            let query_kinds: Vec<Token> = args
                .iter()
                .filter(|a| a.is_positional())
                .filter_map(|a| a.value.get::<String>().map(|s| Token::new(s)))
                .collect();

            if query_kinds.is_empty() {
                return None;
            }

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim = match get_closest_prim(obj) {
                    Some(p) => p,
                    None => return PredicateFunctionResult::make_varying(false),
                };
                let prim_kind = prim.get_metadata::<String>(&Token::new("kind"));
                let prim_kind = match prim_kind {
                    Some(k) => Token::new(&k),
                    None => return PredicateFunctionResult::make_varying(false),
                };
                for qk in &query_kinds {
                    if check_sub_kinds {
                        // Simple sub-kind check: exact match or starts with "kind:"
                        if &prim_kind == qk || prim_kind.as_str().starts_with(qk.as_str()) {
                            return PredicateFunctionResult::make_varying(true);
                        }
                    } else if prim_kind == *qk {
                        return PredicateFunctionResult::make_varying(true);
                    }
                }
                PredicateFunctionResult::make_varying(false)
            }))
        })
        // specifier(spec1, ...)
        .define_binder("specifier", |args: &[FnArg]| {
            let mut spec_table = [false; 3]; // Def=0, Over=1, Class=2

            for arg in args {
                if !arg.is_positional() {
                    return None;
                }
                let val = arg.value.get::<String>()?;
                match val.as_str() {
                    "over" => spec_table[Specifier::Over as usize] = true,
                    "def" => spec_table[Specifier::Def as usize] = true,
                    "class" => spec_table[Specifier::Class as usize] = true,
                    _ => return None,
                }
            }

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim = match get_closest_prim(obj) {
                    Some(p) => p,
                    None => return PredicateFunctionResult::make_varying(false),
                };
                let spec = prim.specifier();
                PredicateFunctionResult::make_varying(spec_table[spec as usize])
            }))
        })
        // isa(schema1, ..., strict=false)
        .define_binder("isa", |args: &[FnArg]| {
            let exact_match = is_strict(args, false);

            // Collect unnamed string args as schema type names.
            let query_types: Vec<Token> = args
                .iter()
                .filter(|a| a.is_positional())
                .filter_map(|a| a.value.get::<String>().map(|s| Token::new(s)))
                .collect();

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim = match get_closest_prim(obj) {
                    Some(p) => p,
                    None => return PredicateFunctionResult::make_varying(false),
                };
                let prim_type = prim.type_name();
                for qt in &query_types {
                    if exact_match {
                        if prim_type == *qt {
                            return PredicateFunctionResult::make_varying(true);
                        }
                    } else {
                        // is_a checks type hierarchy (exact match + derived)
                        if prim.is_a(qt) {
                            return PredicateFunctionResult::make_varying(true);
                        }
                    }
                }
                PredicateFunctionResult::make_varying(false)
            }))
        })
        // hasAPI(api1, ..., instanceName=name)
        .define_binder("hasAPI", |args: &[FnArg]| {
            // Extract optional instanceName
            let instance_name: Option<Token> = args
                .iter()
                .find(|a| a.name == "instanceName")
                .and_then(|a| a.value.get::<String>().map(|s| Token::new(s)));

            // Collect unnamed string args as API schema names.
            let query_apis: Vec<Token> = args
                .iter()
                .filter(|a| a.is_positional())
                .filter_map(|a| a.value.get::<String>().map(|s| Token::new(s)))
                .collect();

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim = match get_closest_prim(obj) {
                    Some(p) => p,
                    None => return PredicateFunctionResult::make_varying(false),
                };
                if let Some(ref inst) = instance_name {
                    for qa in &query_apis {
                        if prim.has_api_instance(qa, inst) {
                            return PredicateFunctionResult::make_varying(true);
                        }
                    }
                } else {
                    for qa in &query_apis {
                        if prim.has_api(qa) {
                            return PredicateFunctionResult::make_varying(true);
                        }
                    }
                }
                PredicateFunctionResult::make_varying(false)
            }))
        })
        // variant(set1=sel1, ...)
        .define_binder("variant", |args: &[FnArg]| {
            // All args must be named (setName=selection)
            let mut exact_sels: Vec<(String, String)> = Vec::new();

            for arg in args {
                if arg.is_positional() {
                    return None; // Invalid: all args must be named
                }
                let sel_str = arg.value.get::<String>()?;
                exact_sels.push((arg.name.clone(), sel_str.clone()));
            }

            Some(Arc::new(move |obj: &Object| {
                if !is_prim_obj(obj) {
                    return PredicateFunctionResult::make_constant(false);
                }
                let prim = match get_closest_prim(obj) {
                    Some(p) => p,
                    None => return PredicateFunctionResult::make_varying(false),
                };
                let vsets = prim.get_variant_sets();
                for (set_name, expected_sel) in &exact_sels {
                    let actual_sel = vsets.get_variant_selection(set_name);
                    // Simple exact match (C++ also supports glob patterns via ArchRegex)
                    if actual_sel != *expected_sel {
                        return PredicateFunctionResult::make_varying(false);
                    }
                }
                PredicateFunctionResult::make_varying(true)
            }))
        });

    lib
}

/// Singleton predicate library for collection membership expressions.
static COLLECTION_PREDICATE_LIBRARY: LazyLock<UsdObjectPredicateLibrary> =
    LazyLock::new(make_collection_predicate_library);

/// Return the predicate library used to evaluate SdfPathExpressions in
/// UsdCollectionAPI's membershipExpression attributes.
///
/// Matches C++ `UsdGetCollectionPredicateLibrary()`.
pub fn get_collection_predicate_library() -> &'static UsdObjectPredicateLibrary {
    &COLLECTION_PREDICATE_LIBRARY
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_has_all_predicates() {
        let lib = get_collection_predicate_library();
        assert!(lib.has_function("abstract"));
        assert!(lib.has_function("defined"));
        assert!(lib.has_function("model"));
        assert!(lib.has_function("group"));
        assert!(lib.has_function("kind"));
        assert!(lib.has_function("specifier"));
        assert!(lib.has_function("isa"));
        assert!(lib.has_function("hasAPI"));
        assert!(lib.has_function("variant"));
    }

    #[test]
    fn test_library_function_names() {
        let lib = get_collection_predicate_library();
        let names = lib.function_names();
        assert_eq!(names.len(), 9);
    }

    #[test]
    fn test_bind_abstract_no_args() {
        let lib = get_collection_predicate_library();
        // Bind with no args should work (default isAbstract=true)
        let func = lib.bind_call("abstract", &[]);
        assert!(func.is_some());
    }

    #[test]
    fn test_bind_specifier_valid() {
        let lib = get_collection_predicate_library();
        let args = vec![FnArg::positional(usd_vt::Value::new("def".to_string()))];
        let func = lib.bind_call("specifier", &args);
        assert!(func.is_some());
    }

    #[test]
    fn test_bind_specifier_invalid() {
        let lib = get_collection_predicate_library();
        let args = vec![FnArg::positional(usd_vt::Value::new("invalid".to_string()))];
        let func = lib.bind_call("specifier", &args);
        assert!(func.is_none());
    }

    #[test]
    fn test_bind_kind_empty() {
        let lib = get_collection_predicate_library();
        // No kind args -> should return None (invalid)
        let func = lib.bind_call("kind", &[]);
        assert!(func.is_none());
    }

    #[test]
    fn test_bind_isa_with_type() {
        let lib = get_collection_predicate_library();
        let args = vec![FnArg::positional(usd_vt::Value::new("Mesh".to_string()))];
        let func = lib.bind_call("isa", &args);
        assert!(func.is_some());
    }

    #[test]
    fn test_bind_has_api_with_instance() {
        let lib = get_collection_predicate_library();
        let args = vec![
            FnArg::positional(usd_vt::Value::new("CollectionAPI".to_string())),
            FnArg::keyword("instanceName", usd_vt::Value::new("lights".to_string())),
        ];
        let func = lib.bind_call("hasAPI", &args);
        assert!(func.is_some());
    }

    #[test]
    fn test_bind_variant() {
        let lib = get_collection_predicate_library();
        let args = vec![FnArg::keyword(
            "shadingVariant",
            usd_vt::Value::new("red".to_string()),
        )];
        let func = lib.bind_call("variant", &args);
        assert!(func.is_some());
    }

    #[test]
    fn test_bind_variant_invalid_positional() {
        let lib = get_collection_predicate_library();
        let args = vec![FnArg::positional(usd_vt::Value::new("red".to_string()))];
        let func = lib.bind_call("variant", &args);
        assert!(func.is_none()); // Positional args not allowed for variant
    }

    #[test]
    fn test_is_strict_helper() {
        // No strict arg -> default
        assert!(!is_strict(&[], false));
        assert!(is_strict(&[], true));

        // bool strict=true
        let args = vec![FnArg::keyword("strict", usd_vt::Value::new(true))];
        assert!(is_strict(&args, false));

        // bool strict=false
        let args = vec![FnArg::keyword("strict", usd_vt::Value::new(false))];
        assert!(!is_strict(&args, true));

        // int strict=1
        let args = vec![FnArg::keyword("strict", usd_vt::Value::new(1i32))];
        assert!(is_strict(&args, false));

        // string strict="yes"
        let args = vec![FnArg::keyword(
            "strict",
            usd_vt::Value::new("yes".to_string()),
        )];
        assert!(is_strict(&args, false));
    }

    #[test]
    fn test_evaluate_on_invalid_object() {
        let lib = get_collection_predicate_library();

        // Evaluate abstract predicate on an invalid object
        let func = lib.bind_call("abstract", &[]).unwrap();
        let obj = Object::invalid();
        let result = func(&obj);
        // Invalid object: is_abstract() returns false, not a prim -> constant(false == true) = constant(false)
        assert!(!result.get_value());
        assert!(result.is_constant());
    }
}
