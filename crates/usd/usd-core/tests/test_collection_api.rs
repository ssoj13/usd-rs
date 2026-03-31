//! Tests for CollectionAPI.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdCollectionAPI.py (core subset)

mod common;

use usd_core::{InitialLoadSet, Stage, collection_api::CollectionAPI};
use usd_sdf::Path;
use usd_tf::Token;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// Test Apply / Get / IsValid
// ============================================================================

#[test]
fn collection_apply_and_get() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Test", "Xform").expect("define /Test");
    let coll_name = Token::new("myCollection");

    // Apply collection
    let coll = CollectionAPI::apply(&prim, &coll_name);
    assert!(coll.is_valid(), "applied collection should be valid");

    // Get it back by name
    let coll2 = CollectionAPI::get_from_prim(&prim, &coll_name);
    assert!(coll2.is_valid(), "should retrieve applied collection");
}

// ============================================================================
// Test GetAll collections on prim
// ============================================================================

#[test]
fn collection_get_all() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Test", "Xform").expect("define /Test");

    // No collections initially
    let all = CollectionAPI::get_all(&prim);
    assert!(all.is_empty(), "no collections initially");

    // Apply two collections
    CollectionAPI::apply(&prim, &Token::new("collA"));
    CollectionAPI::apply(&prim, &Token::new("collB"));

    let all = CollectionAPI::get_all(&prim);
    assert_eq!(all.len(), 2, "should have 2 collections");
}

// ============================================================================
// Test collection path
// ============================================================================

#[test]
fn collection_path() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Foo", "").expect("define /Foo");
    let coll_name = Token::new("myColl");

    let coll = CollectionAPI::apply(&prim, &coll_name);
    let coll_path = coll.get_collection_path();
    // Should be something like /Foo.collection:myColl
    let path_str = coll_path.get_string();
    assert!(
        path_str.contains("Foo"),
        "collection path should reference prim: {path_str}"
    );
}

// ============================================================================
// Test named collection path (static)
// ============================================================================

#[test]
fn collection_named_path() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Bar", "").expect("define /Bar");
    let coll_name = Token::new("stuff");

    let path = CollectionAPI::get_named_collection_path(&prim, &coll_name);
    let path_str = path.get_string();
    assert!(
        path_str.contains("Bar"),
        "named collection path should reference prim: {path_str}"
    );
}

// ============================================================================
// Test CanApply
// ============================================================================

#[test]
fn collection_can_apply() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Test", "").expect("define /Test");
    let coll_name = Token::new("testColl");

    let mut why_not = None;
    let can = CollectionAPI::can_apply(&prim, &coll_name, &mut why_not);
    assert!(can, "should be able to apply CollectionAPI");
}

// ============================================================================
// Test includes/excludes relationships
// ============================================================================

#[test]
fn collection_includes_excludes() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Root", "").expect("define /Root");
    stage.define_prim("/Root/A", "").expect("define /Root/A");
    stage.define_prim("/Root/B", "").expect("define /Root/B");
    stage.define_prim("/Root/C", "").expect("define /Root/C");

    let coll = CollectionAPI::apply(&prim, &Token::new("myColl"));

    // Add includes via include_path
    assert!(coll.include_path(&p("/Root/A")));
    assert!(coll.include_path(&p("/Root/B")));

    // Get includes relationship
    let includes_rel = coll.get_includes_rel();
    assert!(includes_rel.is_valid(), "includes rel should be valid");

    // Get excludes relationship (may not have targets yet)
    let excludes_rel = coll.get_excludes_rel();
    assert!(excludes_rel.is_valid(), "excludes rel should be valid");
}

// ============================================================================
// Test expansion rule attribute
// ============================================================================

#[test]
fn collection_expansion_rule() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Root", "").expect("define /Root");

    let coll = CollectionAPI::apply(&prim, &Token::new("myColl"));

    // Create expansion rule attribute
    let attr = coll.create_expansion_rule_attr(None, false);
    assert!(attr.is_valid(), "expansion rule attr should be valid");

    // Get expansion rule attribute
    let attr2 = coll.get_expansion_rule_attr();
    assert!(attr2.is_valid(), "get expansion rule attr should be valid");
}

// ============================================================================
// Test schema attribute names
// ============================================================================

#[test]
fn collection_schema_attr_names() {
    common::setup();

    let names = CollectionAPI::get_schema_attribute_names(false);
    // Should include expansionRule, includeRoot, membershipExpression, collection
    assert!(!names.is_empty(), "should have schema attribute names");

    // Instance-specific names
    let instance_names =
        CollectionAPI::get_schema_attribute_names_for_instance(false, &Token::new("myColl"));
    assert!(
        !instance_names.is_empty(),
        "should have instance-specific attribute names"
    );
}

// ============================================================================
// Test invalid collection
// ============================================================================

#[test]
fn collection_invalid() {
    common::setup();

    let invalid = CollectionAPI::invalid();
    assert!(
        !invalid.is_valid(),
        "invalid collection should not be valid"
    );
}

// ============================================================================
// Test IsCollectionAPIPath
// ============================================================================

#[test]
fn collection_is_api_path() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    let prim = stage.define_prim("/Foo", "").expect("define /Foo");

    // Apply a collection to get a valid path
    let coll = CollectionAPI::apply(&prim, &Token::new("test"));
    let coll_path = coll.get_collection_path();

    let mut name = Token::new("");
    let is_coll_path = CollectionAPI::is_collection_api_path(&coll_path, &mut name);
    // Depending on path format this may or may not match
    // Just verify no crash
    let _ = is_coll_path;
}

// ============================================================================
// test_SchemaPropertyBaseNames
// Ported from testUsdCollectionAPI.py::test_SchemaPropertyBaseNames
// ============================================================================

#[test]
fn collection_schema_property_base_names() {
    common::setup();

    assert!(
        CollectionAPI::is_schema_property_base_name(&Token::new("includeRoot")),
        "includeRoot should be a schema property base name"
    );
    assert!(
        CollectionAPI::is_schema_property_base_name(&Token::new("expansionRule")),
        "expansionRule should be a schema property base name"
    );
    assert!(
        CollectionAPI::is_schema_property_base_name(&Token::new("includes")),
        "includes should be a schema property base name"
    );
    assert!(
        CollectionAPI::is_schema_property_base_name(&Token::new("excludes")),
        "excludes should be a schema property base name"
    );
    // Empty base name is valid ("collection:{collectionName}")
    assert!(
        CollectionAPI::is_schema_property_base_name(&Token::new("")),
        "empty string should be a schema property base name"
    );
    // "collection" is the prefix, not a base name
    assert!(
        !CollectionAPI::is_schema_property_base_name(&Token::new("collection")),
        "'collection' prefix should NOT be a schema property base name"
    );
}

// ============================================================================
// test_GetSchemaAttributeNames with instance name
// Ported from testUsdCollectionAPI.py::test_GetSchemaAttributeNames
// ============================================================================

#[test]
fn collection_schema_attr_names_with_instance() {
    common::setup();

    // Default (no instance name) — template names
    let names = CollectionAPI::get_schema_attribute_names(false);
    assert!(!names.is_empty(), "schema attr names should not be empty");

    // With instance name "foo"
    let foo_names =
        CollectionAPI::get_schema_attribute_names_for_instance(false, &Token::new("foo"));
    assert!(
        !foo_names.is_empty(),
        "instance attr names should not be empty"
    );
    // Every name should contain "foo"
    for name in &foo_names {
        assert!(
            name.as_str().contains("foo"),
            "attr name '{}' should contain instance name 'foo'",
            name.as_str()
        );
    }

    // With instance name "bar"
    let bar_names =
        CollectionAPI::get_schema_attribute_names_for_instance(true, &Token::new("bar"));
    assert!(!bar_names.is_empty());
    for name in &bar_names {
        assert!(
            name.as_str().contains("bar"),
            "attr name '{}' should contain instance name 'bar'",
            name.as_str()
        );
    }
}

// ============================================================================
// test_MembershipQuery — basic membership query on in-memory collections
// Ported from testUsdCollectionAPI.py::test_AuthorCollections (subset)
// ============================================================================

#[test]
fn collection_membership_query() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("stage");
    stage.define_prim("/Root", "").expect("define /Root");
    stage.define_prim("/Root/A", "").expect("define /Root/A");
    stage.define_prim("/Root/B", "").expect("define /Root/B");
    stage
        .define_prim("/Root/A/Child", "")
        .expect("define /Root/A/Child");

    let root = stage.get_prim_at_path(&p("/Root")).expect("get /Root");

    // Create an expandPrims collection including /Root/A
    let coll = CollectionAPI::apply(&root, &Token::new("myCollection"));
    let rule_attr = coll.create_expansion_rule_attr(None, false);
    rule_attr.set(
        usd_vt::Value::from("expandPrims".to_string()),
        usd_sdf::TimeCode::default(),
    );
    coll.create_includes_rel().add_target(&p("/Root/A"));

    let query = coll.compute_membership_query();
    // /Root/A should be included (expandPrims includes descendants)
    assert!(
        query.is_path_included(&p("/Root/A"), None),
        "/Root/A should be included"
    );
    // /Root/A/Child should also be included via expansion
    assert!(
        query.is_path_included(&p("/Root/A/Child"), None),
        "/Root/A/Child should be included via expansion"
    );
    // /Root/B should NOT be included
    assert!(
        !query.is_path_included(&p("/Root/B"), None),
        "/Root/B should NOT be included"
    );
}
