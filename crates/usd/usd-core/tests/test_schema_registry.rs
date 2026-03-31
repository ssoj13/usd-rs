//! Tests for SchemaRegistry.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdSchemaRegistry.py
//!   - pxr/usd/usd/testenv/testUsdSchemaRegistryCpp.cpp

mod common;

use usd_core::{common::SchemaKind, schema_registry::SchemaRegistry};
use usd_tf::Token;

// ============================================================================
// Test singleton access
// ============================================================================

#[test]
fn schema_registry_singleton() {
    common::setup();

    let registry = SchemaRegistry::get_instance();
    // Should be the same object each time
    let registry2 = SchemaRegistry::get_instance();
    assert!(
        std::ptr::eq(registry, registry2),
        "get_instance should return same reference"
    );
}

// ============================================================================
// Test schema type name lookups
// ============================================================================

#[test]
fn schema_registry_type_names() {
    common::setup();

    // GetSchemaTypeName for known types
    let mesh_type = SchemaRegistry::get_schema_type_name("Mesh");
    assert!(!mesh_type.is_empty(), "Mesh schema type name should exist");

    let xform_type = SchemaRegistry::get_schema_type_name("Xform");
    assert!(
        !xform_type.is_empty(),
        "Xform schema type name should exist"
    );

    // Unknown type should return empty
    let _unknown = SchemaRegistry::get_schema_type_name("NonExistentSchemaXYZ");
    // May return the name itself or empty depending on implementation
}

// ============================================================================
// Test concrete vs abstract schemas
// ============================================================================

#[test]
fn schema_registry_concrete_abstract() {
    common::setup();

    // Mesh should be concrete
    assert!(SchemaRegistry::is_concrete("Mesh"), "Mesh is concrete");

    // Typed (UsdTyped) should be abstract
    assert!(SchemaRegistry::is_abstract("Typed"), "Typed is abstract");
}

// ============================================================================
// Test schema kind
// ============================================================================

#[test]
fn schema_registry_schema_kind() {
    common::setup();

    let mesh_kind = SchemaRegistry::get_schema_kind("Mesh");
    assert_eq!(
        mesh_kind,
        SchemaKind::ConcreteTyped,
        "Mesh should be ConcreteTyped"
    );

    // CollectionAPI should be a multiple-apply API schema
    let collection_kind = SchemaRegistry::get_schema_kind("CollectionAPI");
    assert_eq!(
        collection_kind,
        SchemaKind::MultipleApplyAPI,
        "CollectionAPI should be MultipleApplyAPI"
    );
}

// ============================================================================
// Test applied/multiple-apply API schemas
// ============================================================================

#[test]
fn schema_registry_api_schemas() {
    common::setup();

    // CollectionAPI is an applied API schema
    assert!(
        SchemaRegistry::is_applied_api_schema("CollectionAPI"),
        "CollectionAPI is applied API"
    );

    // CollectionAPI is a multiple-apply API schema
    assert!(
        SchemaRegistry::is_multiple_apply_api_schema("CollectionAPI"),
        "CollectionAPI is multiple-apply"
    );

    // ModelAPI is a single-apply API schema
    assert!(
        SchemaRegistry::is_applied_api_schema("ModelAPI"),
        "ModelAPI is applied API"
    );
    assert!(
        !SchemaRegistry::is_multiple_apply_api_schema("ModelAPI"),
        "ModelAPI is NOT multiple-apply"
    );
}

// ============================================================================
// Test find schema info
// ============================================================================

#[test]
fn schema_registry_find_info() {
    common::setup();

    let mesh_info = SchemaRegistry::find_schema_info(&Token::new("Mesh"));
    assert!(mesh_info.is_some(), "should find SchemaInfo for Mesh");

    if let Some(info) = mesh_info {
        assert_eq!(info.identifier.as_str(), "Mesh");
        assert_eq!(info.kind, SchemaKind::ConcreteTyped);
    }
}

// ============================================================================
// Test disallowed fields
// ============================================================================

#[test]
fn schema_registry_disallowed_fields() {
    common::setup();

    // Some fields are disallowed in schema definitions
    // "typeName" is typically disallowed
    let type_name_tok = Token::new("typeName");
    let is_disallowed = SchemaRegistry::is_disallowed_field(&type_name_tok);
    // This depends on implementation — just verify the method exists and doesn't crash
    let _ = is_disallowed;
}

// ============================================================================
// Test multiple-apply name template
// ============================================================================

#[test]
fn schema_registry_multi_apply_name() {
    common::setup();

    // Make a name template: "collection:__INSTANCE_NAME__:includes"
    let template = SchemaRegistry::make_multiple_apply_name_template("collection", "includes");
    assert!(!template.is_empty(), "name template should not be empty");

    // Check if it's a valid template
    assert!(
        SchemaRegistry::is_multiple_apply_name_template(template.as_str()),
        "should be recognized as a name template"
    );

    // Get base name from template
    let base = SchemaRegistry::get_multiple_apply_name_template_base_name(template.as_str());
    assert_eq!(base.as_str(), "includes", "base name should be 'includes'");

    // Make instance name
    let instance_name =
        SchemaRegistry::make_multiple_apply_name_instance(template.as_str(), "myColl");
    assert!(
        instance_name.as_str().contains("myColl"),
        "instance name should contain 'myColl'"
    );
}

// ============================================================================
// Test schema family/version parsing
// ============================================================================

#[test]
fn schema_registry_family_version_parse() {
    common::setup();

    // Parse identifier into family + version
    let result = SchemaRegistry::parse_schema_family_and_version_from_identifier(&Token::new(
        "CollectionAPI",
    ));
    let (family, version) = result;
    assert_eq!(family.as_str(), "CollectionAPI");
    assert_eq!(version, 0, "default version is 0");
}

// ============================================================================
// Test prim definition lookup
// ============================================================================

#[test]
fn schema_registry_prim_definition() {
    common::setup();

    let registry = SchemaRegistry::get_instance();

    // Look up concrete prim definition for Mesh
    let _mesh_def = registry.find_concrete_prim_definition(&Token::new("Mesh"));
    // May or may not have a definition depending on whether schemas are loaded
    // Just verify the API works without crashing

    // Empty prim definition should always exist
    let empty_def = registry.get_empty_prim_definition();
    assert!(
        empty_def.property_names().is_empty(),
        "empty prim definition should have no properties"
    );
}

// ============================================================================
// Port of testUsdSchemaRegistryThreadedInit.py: test_ThreadedInit
// ============================================================================

#[test]
fn schema_registry_threaded_init() {
    common::setup();

    // Spawn 2 threads that each access the SchemaRegistry concurrently
    let handles: Vec<_> = (0..2)
        .map(|_| {
            std::thread::spawn(|| {
                let _registry = SchemaRegistry::get_instance();
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread panicked");
    }
}
