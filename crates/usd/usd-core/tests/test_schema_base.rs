//! Tests for SchemaBase.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdSchemaBase.cpp (TestPrimQueries)
//!   - pxr/usd/usd/testenv/testUsdSchemaBasePy.py

mod common;

use usd_core::{InitialLoadSet, Stage, collection_api::CollectionAPI, schema_base::SchemaBase};
use usd_sdf::Path;
use usd_tf::Token;

#[allow(dead_code)]
fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// TestPrimQueries — SchemaBase wrapping and validity
// ============================================================================

#[test]
fn schema_base_valid_invalid() {
    common::setup();

    // Invalid schema
    let invalid = SchemaBase::invalid();
    assert!(!invalid.is_valid(), "invalid schema should not be valid");

    // Valid schema from prim
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Test", "Xform").expect("define /Test");

    let schema = SchemaBase::new(prim.clone());
    assert!(schema.is_valid(), "schema from valid prim should be valid");
    assert_eq!(schema.path(), prim.path());
}

// ============================================================================
// Schema wrapping preserves prim state
// ============================================================================

#[test]
fn schema_base_prim_access() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Foo/Bar", "Mesh").expect("define");

    let schema = SchemaBase::new(prim.clone());
    assert_eq!(schema.prim().get_path().get_string(), "/Foo/Bar");
    assert!(schema.stage().is_some());
}

// ============================================================================
// CollectionAPI apply and HasAPI (from testUsdSchemaBase.cpp TestPrimQueries)
// ============================================================================

#[test]
fn schema_base_collection_api_apply() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/p", "").expect("define /p");

    // Before applying, prim should not have CollectionAPI
    let applied = prim.get_applied_schemas();
    let has_collection = applied.iter().any(|s| s.as_str().contains("CollectionAPI"));
    // Initially no collections
    assert!(
        !has_collection,
        "prim should not have CollectionAPI initially"
    );

    // Apply CollectionAPI
    let coll = CollectionAPI::apply(&prim, &Token::new("testColl"));

    // Now prim should have the applied schema
    let applied = prim.get_applied_schemas();
    let has_collection = applied.iter().any(|s| s.as_str().contains("CollectionAPI"));
    assert!(has_collection, "prim should have CollectionAPI after apply");

    // The collection should be valid
    assert!(coll.is_valid(), "applied collection should be valid");
}
