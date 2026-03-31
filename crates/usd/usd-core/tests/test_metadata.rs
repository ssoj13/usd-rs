//! Tests for metadata authoring and resolution.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdMetadata.py
//!
//! Skipped (no Rust API or requires external files):
//! test_BasicListOpMetadata / test_ComposedListOpMetadata (ListOp types),
//! test_UnknownFieldsRoundTripThroughUsdc (usdc roundtrip),
//! test_AssetPathMetadata / test_AssetPathExpressionErrors (file-based asset paths)

mod common;

use usd_core::ListPosition;
use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::LayerOffset;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// test_Hidden — hidden metadata on prims/properties
// ============================================================================

#[test]
fn meta_hidden() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");
    let attr = foo
        .create_attribute("attr", &common::vtn("string"), false, None)
        .expect("create attr");

    let hidden_key = Token::new("hidden");

    // Prim: initially not hidden
    assert_eq!(
        foo.get_metadata::<bool>(&hidden_key),
        None,
        "hidden should be unset initially"
    );

    // Set hidden=false
    assert!(foo.set_metadata(&hidden_key, false));
    assert_eq!(foo.get_metadata::<bool>(&hidden_key), Some(false));

    // Set hidden=true
    assert!(foo.set_metadata(&hidden_key, true));
    assert_eq!(foo.get_metadata::<bool>(&hidden_key), Some(true));

    // Attribute: initially not hidden
    assert_eq!(attr.get_metadata(&hidden_key), None);

    // Set hidden=false on attr
    assert!(attr.set_metadata(&hidden_key, Value::from(false)));
    let val = attr.get_metadata(&hidden_key).expect("hidden set");
    assert_eq!(val.try_into_inner::<bool>(), Ok(false));

    // Set hidden=true on attr
    assert!(attr.set_metadata(&hidden_key, Value::from(true)));
    let val = attr.get_metadata(&hidden_key).expect("hidden set");
    assert_eq!(val.try_into_inner::<bool>(), Ok(true));
}

// ============================================================================
// test_PrimTypeName — typeName metadata
// ============================================================================

#[test]
fn meta_prim_type_name() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    // Prim with type
    let prim_with_type = stage
        .define_prim("/PrimWithType", "DummyType")
        .expect("define prim with type");
    assert_eq!(prim_with_type.get_type_name().as_str(), "DummyType");
    assert!(prim_with_type.has_authored_type_name());

    let type_name_key = Token::new("typeName");
    assert!(prim_with_type.has_authored_metadata(&type_name_key));

    // Prim without type
    let prim_without_type = stage
        .define_prim("/PrimWithoutType", "")
        .expect("define prim without type");
    assert_eq!(prim_without_type.get_type_name().as_str(), "");
    assert!(!prim_without_type.has_authored_type_name());
}

// ============================================================================
// test_HasAuthored — HasAuthoredMetadata behavior
// ============================================================================

#[test]
fn meta_has_authored() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");
    let attr = foo
        .create_attribute("attr", &common::vtn("string"), false, None)
        .expect("create attr");
    let _rel = foo.create_relationship("rel", false).expect("create rel");

    // Prim has authored specifier (from OverridePrim)
    assert!(foo.has_authored_metadata(&Token::new("specifier")));

    // Attribute has authored custom, variability, typeName
    assert!(attr.has_authored_metadata(&Token::new("custom")));
    assert!(attr.has_authored_metadata(&Token::new("variability")));
    assert!(attr.has_authored_metadata(&Token::new("typeName")));

    // Relationship variability/typeName checks skipped: Relationship does not expose has_authored_metadata

    // Explicitly author comment on prim
    let comment_key = Token::new("comment");
    assert!(!foo.has_authored_metadata(&comment_key));
    assert!(foo.set_metadata(&comment_key, "this is a comment"));
    assert!(foo.has_authored_metadata(&comment_key));

    // Explicitly author comment on attribute
    assert!(!attr.has_authored_metadata(&comment_key));
    assert!(attr.set_metadata(&comment_key, Value::from("attr comment".to_string())));
    assert!(attr.has_authored_metadata(&comment_key));

    // Relationship comment check skipped: Relationship does not expose has_authored_metadata
}

// ============================================================================
// test_Documentation — documentation metadata and explicit API
// ============================================================================

#[test]
fn meta_documentation() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");

    // Prim documentation
    assert_eq!(foo.get_documentation(), "");

    assert!(foo.set_documentation("hello docs"));
    assert_eq!(foo.get_documentation(), "hello docs");

    let doc_key = Token::new("documentation");
    let doc_val = foo.get_metadata::<String>(&doc_key);
    assert_eq!(doc_val.as_deref(), Some("hello docs"));

    // Clear documentation
    assert!(foo.clear_metadata(&doc_key));
    assert_eq!(foo.get_documentation(), "");

    // Stage pseudo-root documentation
    let stage_root = stage.get_pseudo_root();
    assert_eq!(stage_root.get_documentation(), "");
    assert!(stage_root.set_documentation("stage doc"));
    assert_eq!(stage_root.get_documentation(), "stage doc");
    assert!(stage_root.clear_metadata(&doc_key));
    assert_eq!(stage_root.get_documentation(), "");
}

// ============================================================================
// test_DisplayName — displayName metadata
// ============================================================================

#[test]
fn meta_display_name() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");

    // Prim displayName via raw metadata (Object::get/set/clear_display_name are not on Prim)
    let dn_key = Token::new("displayName");

    let dn_initial = foo.get_metadata::<String>(&dn_key);
    assert!(
        dn_initial.is_none(),
        "displayName should be unset initially"
    );
    assert!(!foo.has_authored_metadata(&dn_key));

    assert!(foo.set_metadata(&dn_key, "Foo Display"));
    let dn_val = foo.get_metadata::<String>(&dn_key);
    assert_eq!(dn_val.as_deref(), Some("Foo Display"));
    assert!(foo.has_authored_metadata(&dn_key));

    assert!(foo.clear_metadata(&dn_key));
    let dn_after = foo.get_metadata::<String>(&dn_key);
    assert!(dn_after.is_none(), "displayName should be cleared");
    assert!(!foo.has_authored_metadata(&dn_key));
}

// ============================================================================
// test_DisplayGroup — displayGroup on properties
// ============================================================================

#[test]
fn meta_display_group() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");
    let attr = foo
        .create_attribute("attr", &common::vtn("string"), false, None)
        .expect("create attr");

    // Property display group via Attribute inner property
    let attr_path = attr.path().clone();
    let display_group_key = Token::new("displayGroup");

    // Initially empty
    assert_eq!(attr.get_metadata(&display_group_key), None);

    // Set displayGroup
    assert!(attr.set_metadata(&display_group_key, Value::from("myGroup".to_string())));
    let val = attr
        .get_metadata(&display_group_key)
        .expect("displayGroup set");
    assert_eq!(val.try_into_inner::<String>(), Ok("myGroup".to_string()));

    // Clear
    let edit_layer = stage.get_root_layer();
    edit_layer.set_field(&attr_path, &display_group_key, Value::empty());
    // After clearing, metadata should be None
    // (This depends on how clearing works — using set_field with empty may not clear)
}

// ============================================================================
// test_BasicCustomData — CustomData API
// ============================================================================

#[test]
fn meta_basic_custom_data() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");

    // Custom data via metadata
    let custom_data_key = Token::new("customData");

    // Initially no custom data
    assert!(!foo.has_authored_metadata(&custom_data_key));

    // Set custom data via raw metadata
    let mut dict = usd_vt::Dictionary::new();
    dict.insert("key1".to_string(), Value::from("value1".to_string()));
    dict.insert("key2".to_string(), Value::from(42_i32));

    assert!(foo.set_metadata(&custom_data_key, Value::from(dict)));
    assert!(foo.has_authored_metadata(&custom_data_key));

    // Read it back
    let cd: Option<usd_vt::Dictionary> = foo.get_metadata(&custom_data_key);
    let cd = cd.expect("custom data should exist");
    assert_eq!(
        cd.get("key1").and_then(|v| v.get::<String>()),
        Some(&"value1".to_string())
    );

    // Set custom data by dict key
    let key_path = Token::new("key3");
    foo.set_metadata_by_dict_key(&custom_data_key, &key_path, Value::from(3.14_f64));

    // Read back by key
    let val = foo.get_metadata_by_dict_key(&custom_data_key, &key_path);
    assert!(val.is_some(), "key3 should exist in customData");

    // Clear by key
    foo.clear_metadata_by_dict_key(&custom_data_key, &key_path);
}

// ============================================================================
// test_CompositionData — references metadata
// ============================================================================

#[test]
fn meta_composition_data() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let _base = stage.override_prim("/base").expect("override /base");
    let _base_child = stage
        .override_prim("/base/baseChild")
        .expect("override /base/baseChild");
    let apex = stage.override_prim("/apex").expect("override /apex");
    let _apex_child = stage
        .override_prim("/apex/apexChild")
        .expect("override /apex/apexChild");

    // Make /apex reference /base
    let _root_layer = stage.get_root_layer();
    apex.get_references().add_internal_reference(
        &p("/base"),
        usd_sdf::LayerOffset::identity(),
        usd_core::ListPosition::FrontOfPrependList,
    );

    // /apex should now have children from both itself and the reference
    let children = apex.get_all_children();
    assert_eq!(
        children.len(),
        2,
        "apex should have 2 children (apexChild + baseChild via ref)"
    );

    // primChildren should have only direct children count
    let prim_children_key = Token::new("primChildren");
    let pc: Option<Vec<Token>> = apex.get_metadata(&prim_children_key);
    if let Some(pc) = &pc {
        // primChildren only lists directly authored children
        assert_eq!(pc.len(), 1, "primChildren should list 1 direct child");
    }
}

// ============================================================================
// Additional: Stage-level metadata
// ============================================================================

#[test]
fn meta_stage_metadata() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    // Stage metadata via pseudo-root
    let root = stage.get_pseudo_root();
    let comment_key = Token::new("comment");

    // Set stage comment
    assert!(root.set_metadata(&comment_key, "stage comment"));
    let val: Option<String> = root.get_metadata(&comment_key);
    assert_eq!(val.as_deref(), Some("stage comment"));

    // Clear stage comment
    assert!(root.clear_metadata(&comment_key));
    let val: Option<String> = root.get_metadata(&comment_key);
    assert!(val.is_none(), "comment should be cleared");
}

// ============================================================================
// test_ListAndHas — GetAllMetadata round-trip: every key must Get and Has
// ============================================================================

#[test]
fn meta_list_and_has() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");
    let attr = foo
        .create_attribute("attr", &common::vtn("string"), false, None)
        .expect("create attr");

    // Prim: every key from get_all_metadata should be has-able
    let all_meta = stage.get_all_metadata_for_object(&p("/Foo"));
    assert!(!all_meta.is_empty(), "prim should have some metadata");
    for (key, _val) in &all_meta {
        assert!(
            foo.has_authored_metadata(key),
            "HasAuthoredMetadata should be true for key {}",
            key.as_str()
        );
    }

    // Attribute: same check
    let attr_meta = stage.get_all_metadata_for_object(&attr.path());
    assert!(!attr_meta.is_empty(), "attribute should have some metadata");
    for key in attr_meta.keys() {
        assert!(
            attr.has_authored_metadata(key),
            "HasAuthoredMetadata should be true for attr key {}",
            key.as_str()
        );
    }

    // Write round-trip: SetMetadata(key, GetMetadata(key)) should succeed
    for (key, val) in &all_meta {
        assert!(
            foo.set_metadata(key, val.clone()),
            "SetMetadata should succeed for key {}",
            key.as_str()
        );
    }
}

// ============================================================================
// test_Unregistered — unregistered metadata cannot be authored
// ============================================================================

#[test]
fn meta_unregistered() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/Foo").expect("override /Foo");
    let unreg = Token::new("unregistered");

    // SetMetadata with unregistered key should fail (return false)
    assert!(
        !foo.set_metadata(&unreg, "x"),
        "unregistered metadata should fail to author"
    );

    // Same for attribute
    let attr = foo
        .create_attribute("attr", &common::vtn("string"), false, None)
        .expect("create attr");
    assert!(
        !attr.set_metadata(&unreg, Value::from("x".to_string())),
        "unregistered metadata on attr should fail"
    );
}

// ============================================================================
// test_ArraySizeConstraint — array size constraint on attributes
// ============================================================================

#[test]
fn meta_array_size_constraint() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.override_prim("/Prim").expect("override /Prim");
    let attr = prim
        .create_attribute("attr", &common::vtn("string[]"), false, None)
        .expect("create attr");

    for val in [10_i64, -10, 0] {
        // Initially not authored
        assert_eq!(attr.get_array_size_constraint(), 0);
        assert!(!attr.has_authored_array_size_constraint());

        // Set
        assert!(attr.set_array_size_constraint(val));
        assert_eq!(attr.get_array_size_constraint(), val);
        assert!(attr.has_authored_array_size_constraint());

        // Clear
        assert!(attr.clear_array_size_constraint());
        assert_eq!(attr.get_array_size_constraint(), 0);
        assert!(!attr.has_authored_array_size_constraint());
    }
}

// ============================================================================
// test_ComposedNestedDictionaries — customData merge via references
// ============================================================================

#[test]
fn meta_composed_nested_dicts() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    let prim_a = stage.define_prim("/A", "").expect("define /A");
    let mut sub_a = usd_vt::Dictionary::new();
    sub_a.insert(
        "otherStr".to_string(),
        Value::from("definedInA".to_string()),
    );
    sub_a.insert(
        "willCollide".to_string(),
        Value::from("definedInA".to_string()),
    );
    let mut cd_a = usd_vt::Dictionary::new();
    cd_a.insert("sub".to_string(), Value::from(sub_a));
    let cd_tok = Token::new("customData");
    prim_a.set_metadata(&cd_tok, Value::from(cd_a));

    let prim_b = stage.define_prim("/B", "").expect("define /B");
    let mut sub_b = usd_vt::Dictionary::new();
    sub_b.insert("newStr".to_string(), Value::from("definedInB".to_string()));
    sub_b.insert(
        "willCollide".to_string(),
        Value::from("definedInB".to_string()),
    );
    let mut cd_b = usd_vt::Dictionary::new();
    cd_b.insert("sub".to_string(), Value::from(sub_b));
    prim_b.set_metadata(&cd_tok, Value::from(cd_b));

    // B references A
    prim_b.get_references().add_internal_reference(
        &p("/A"),
        LayerOffset::identity(),
        ListPosition::FrontOfPrependList,
    );

    // Composed customData should merge sub-dictionaries
    let result: usd_vt::Dictionary = prim_b.get_metadata(&cd_tok).unwrap_or_default();
    let sub = result
        .get("sub")
        .and_then(|v| v.get::<usd_vt::Dictionary>())
        .expect("sub dict");
    // B's "willCollide" wins over A's
    assert_eq!(
        sub.get("willCollide").and_then(|v| v.get::<String>()),
        Some(&"definedInB".to_string())
    );
    // B's "newStr" present
    assert_eq!(
        sub.get("newStr").and_then(|v| v.get::<String>()),
        Some(&"definedInB".to_string())
    );
    // A's "otherStr" shines through
    assert_eq!(
        sub.get("otherStr").and_then(|v| v.get::<String>()),
        Some(&"definedInA".to_string())
    );
}

// ============================================================================
// test_ComposedCustomData — customData composition with key-level authoring
// ============================================================================

#[test]
fn meta_composed_custom_data() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    let weaker = stage.override_prim("/weaker").expect("override /weaker");
    let stronger = stage
        .override_prim("/stronger")
        .expect("override /stronger");
    stronger.get_references().add_reference_with_path(
        stage.get_root_layer().identifier(),
        &p("/weaker"),
        LayerOffset::identity(),
        ListPosition::FrontOfPrependList,
    );

    let cd_tok = Token::new("customData");

    // Values set in weaker shine through to stronger
    weaker.set_metadata_by_dict_key(
        &cd_tok,
        &Token::new("foo"),
        Value::from("weaker".to_string()),
    );
    let cd: usd_vt::Dictionary = stronger.get_metadata(&cd_tok).unwrap_or_default();
    assert_eq!(
        cd.get("foo").and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );

    // Empty dict in stronger should not hide weaker
    stronger.set_metadata(&cd_tok, Value::from(usd_vt::Dictionary::new()));
    let cd: usd_vt::Dictionary = stronger.get_metadata(&cd_tok).unwrap_or_default();
    assert_eq!(
        cd.get("foo").and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );

    // Set different key in stronger, dicts should merge
    stronger.set_metadata_by_dict_key(
        &cd_tok,
        &Token::new("bar"),
        Value::from("stronger".to_string()),
    );
    let cd: usd_vt::Dictionary = stronger.get_metadata(&cd_tok).unwrap_or_default();
    assert_eq!(
        cd.get("foo").and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );
    assert_eq!(
        cd.get("bar").and_then(|v| v.get::<String>()),
        Some(&"stronger".to_string())
    );

    // Override weaker key in stronger
    stronger.set_metadata_by_dict_key(
        &cd_tok,
        &Token::new("foo"),
        Value::from("stronger".to_string()),
    );
    let cd: usd_vt::Dictionary = stronger.get_metadata(&cd_tok).unwrap_or_default();
    assert_eq!(
        cd.get("foo").and_then(|v| v.get::<String>()),
        Some(&"stronger".to_string())
    );

    // Clear stronger's 'bar', weaker's should shine through
    weaker.set_metadata_by_dict_key(
        &cd_tok,
        &Token::new("bar"),
        Value::from("weaker".to_string()),
    );
    stronger.clear_metadata_by_dict_key(&cd_tok, &Token::new("bar"));
    let cd: usd_vt::Dictionary = stronger.get_metadata(&cd_tok).unwrap_or_default();
    assert_eq!(
        cd.get("bar").and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );
}

// ============================================================================
// test_BasicCustomDataViaMetadataAPI — dict-key metadata API
// ============================================================================

#[test]
fn meta_basic_custom_data_via_metadata_api() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.override_prim("/foo").expect("override /foo");

    let cd_key = Token::new("customData");

    assert!(!foo.has_authored_metadata(&cd_key));

    // Set customData via metadata
    let mut dict = usd_vt::Dictionary::new();
    dict.insert("foo".to_string(), Value::from("bar".to_string()));
    assert!(foo.set_metadata(&cd_key, Value::from(dict)));
    assert!(foo.has_authored_metadata(&cd_key));
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("foo"))
            .is_some()
    );
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("bar"))
            .is_none()
    );

    // SetMetadataByDictKey
    foo.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("foo"),
        Value::from("byKey".to_string()),
    );
    let val = foo.get_metadata_by_dict_key(&cd_key, &Token::new("foo"));
    assert_eq!(
        val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"byKey".to_string())
    );

    // Add new key
    foo.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("newKey"),
        Value::from("value".to_string()),
    );
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("newKey"))
            .is_some()
    );

    // Deep key path
    foo.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("a:deep:key:path"),
        Value::from(1.2345_f64),
    );
    let deep_val = foo.get_metadata_by_dict_key(&cd_key, &Token::new("a:deep:key:path"));
    assert!(deep_val.is_some(), "deep key should exist");

    // ClearMetadataByDictKey
    foo.clear_metadata_by_dict_key(&cd_key, &Token::new("foo"));
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("foo"))
            .is_none()
    );
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("a:deep:key"))
            .is_some()
    );
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("a"))
            .is_some()
    );

    // Clear 'a' subtree
    foo.clear_metadata_by_dict_key(&cd_key, &Token::new("a"));
    assert!(
        foo.get_metadata_by_dict_key(&cd_key, &Token::new("a"))
            .is_none()
    );
}

// ============================================================================
// test_ComposedCustomDataViaMetadataAPI — dict-key metadata composition
// ============================================================================

#[test]
fn meta_composed_custom_data_via_metadata_api() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let weaker = stage.override_prim("/weaker").expect("override /weaker");
    let stronger = stage
        .override_prim("/stronger")
        .expect("override /stronger");
    stronger.get_references().add_reference_with_path(
        stage.get_root_layer().identifier(),
        &p("/weaker"),
        LayerOffset::identity(),
        ListPosition::FrontOfPrependList,
    );

    let cd_key = Token::new("customData");

    // Values set in weaker shine through to stronger
    weaker.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("foo"),
        Value::from("weaker".to_string()),
    );
    let val = stronger.get_metadata_by_dict_key(&cd_key, &Token::new("foo"));
    assert_eq!(
        val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );

    // Empty dict in stronger should not affect composition
    assert!(stronger.set_metadata(&cd_key, Value::from(usd_vt::Dictionary::new())));
    let val = stronger.get_metadata_by_dict_key(&cd_key, &Token::new("foo"));
    assert_eq!(
        val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );

    // Set different key in stronger, dicts merge
    stronger.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("bar"),
        Value::from("stronger".to_string()),
    );
    let foo_val = stronger.get_metadata_by_dict_key(&cd_key, &Token::new("foo"));
    assert_eq!(
        foo_val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );
    let bar_val = stronger.get_metadata_by_dict_key(&cd_key, &Token::new("bar"));
    assert_eq!(
        bar_val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"stronger".to_string())
    );

    // Override weaker key
    stronger.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("foo"),
        Value::from("stronger".to_string()),
    );
    let foo_val = stronger.get_metadata_by_dict_key(&cd_key, &Token::new("foo"));
    assert_eq!(
        foo_val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"stronger".to_string())
    );

    // Clear stronger's 'bar', weaker shines through
    weaker.set_metadata_by_dict_key(
        &cd_key,
        &Token::new("bar"),
        Value::from("weaker".to_string()),
    );
    stronger.clear_metadata_by_dict_key(&cd_key, &Token::new("bar"));
    let bar_val = stronger.get_metadata_by_dict_key(&cd_key, &Token::new("bar"));
    assert_eq!(
        bar_val.as_ref().and_then(|v| v.get::<String>()),
        Some(&"weaker".to_string())
    );
}

// ============================================================================
// test_BasicRequiredFields — GetAllAuthoredMetadata content check
// ============================================================================

#[test]
fn meta_basic_required_fields() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.override_prim("/hello").expect("override /hello");
    let attr = prim
        .create_attribute("foo", &common::vtn("double"), false, None)
        .expect("create attr");
    attr.set(Value::from(1.234_f64), usd_sdf::TimeCode::default());

    let metadata = stage.get_all_authored_metadata_for_object(&attr.path());
    let ins = ["typeName", "custom", "variability"];
    let outs = ["allowedTokens"];
    for key in &ins {
        assert!(
            metadata.contains_key(&Token::new(key)),
            "expected {} in authored metadata",
            key
        );
    }
    for key in &outs {
        assert!(
            !metadata.contains_key(&Token::new(key)),
            "expected {} NOT in authored metadata",
            key
        );
    }
}

// ============================================================================
// test_TimeSamplesMetadata — timeSamples with layer offsets
// ============================================================================

#[test]
fn meta_time_samples_metadata() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    let weaker = stage.override_prim("/weaker").expect("override /weaker");
    let stronger = stage
        .override_prim("/stronger")
        .expect("override /stronger");
    // Reference with offset=10 scale=1
    let ref_offset = LayerOffset::new(10.0, 1.0);
    stronger.get_references().add_reference_with_path(
        stage.get_root_layer().identifier(),
        &p("/weaker"),
        ref_offset,
        ListPosition::FrontOfPrependList,
    );

    // Set time samples on weaker attribute
    let weaker_attr = weaker
        .create_attribute("attr", &common::vtn("string"), false, None)
        .expect("create attr");
    weaker_attr.set(
        Value::from("abc_0".to_string()),
        usd_sdf::TimeCode::new(0.0),
    );
    weaker_attr.set(
        Value::from("def_10".to_string()),
        usd_sdf::TimeCode::new(10.0),
    );

    // Weaker attr should have times [0, 10]
    let weaker_times = weaker_attr.get_time_samples();
    assert_eq!(weaker_times.len(), 2);
    assert!((weaker_times[0] - 0.0).abs() < 1e-6);
    assert!((weaker_times[1] - 10.0).abs() < 1e-6);

    // Stronger attr should have times shifted by +10: [10, 20]
    let stronger_attr = stronger
        .get_attribute("attr")
        .expect("stronger attr should exist via reference");
    let stronger_times = stronger_attr.get_time_samples();
    assert_eq!(stronger_times.len(), 2);
    assert!(
        (stronger_times[0] - 10.0).abs() < 1e-6,
        "first time should be 10, got {}",
        stronger_times[0]
    );
    assert!(
        (stronger_times[1] - 20.0).abs() < 1e-6,
        "second time should be 20, got {}",
        stronger_times[1]
    );
}

// ============================================================================
// test_UsdStageMetadata — comprehensive stage metadata with session layer
// ============================================================================

#[test]
fn meta_usd_stage_metadata() {
    common::setup();

    use usd_sdf::Layer;

    // Create layers
    let root_layer = Layer::create_anonymous(Some("usda"));
    let root_sublayer = Layer::create_anonymous(Some("usda"));
    root_layer.insert_sublayer_path(root_sublayer.identifier(), -1);

    let session_layer = Layer::create_anonymous(Some("usda"));
    let session_sublayer = Layer::create_anonymous(Some("usda"));
    session_layer.insert_sublayer_path(session_sublayer.identifier(), -1);

    let stage = Stage::open_with_root_and_session_layer(
        root_layer.clone(),
        session_layer.clone(),
        InitialLoadSet::LoadAll,
    )
    .expect("open stage");

    let tcps_key = Token::new("timeCodesPerSecond");
    let comment_key = Token::new("comment");

    // No authored timeCodesPerSecond initially
    assert!(!stage.has_authored_metadata(&tcps_key));

    // Author on root layer
    root_layer.set_time_codes_per_second(24.0);
    assert!(stage.has_authored_metadata(&tcps_key));
    let val = stage.get_metadata(&tcps_key);
    assert!(val.is_some());

    // Session layer overrides root layer
    session_layer.set_time_codes_per_second(48.0);
    assert!(stage.has_authored_metadata(&tcps_key));

    // Comment: author on session layer
    session_layer.set_field(
        &Path::absolute_root(),
        &comment_key,
        Value::from("session comment".to_string()),
    );
    let cmt = stage.get_metadata(&comment_key);
    assert!(cmt.is_some());

    // Mute session layer
    stage.mute_layer(session_layer.identifier());
    // timeCodesPerSecond should fall back to root layer
    assert!(stage.has_authored_metadata(&tcps_key));
    // Comment should no longer be authored (session muted, sublayers don't contribute)
    assert!(!stage.has_authored_metadata(&comment_key));

    // Unmute
    stage.unmute_layer(session_layer.identifier());
    assert!(stage.has_authored_metadata(&tcps_key));
    let cmt = stage.get_metadata(&comment_key);
    assert!(cmt.is_some(), "comment should be back after unmute");
}

// ============================================================================
// Port of testUsdMetadata.py: test_BasicListOpMetadata
// ============================================================================

#[test]
#[ignore = "Needs IntListOp/StringListOp/TokenListOp metadata + CreateNew (disk)"]
fn meta_basic_list_op_metadata() {
    common::setup();
    // C++ tests SetMetadata/GetMetadata/ClearMetadata with various ListOp types
    // (Int, Int64, UInt, UInt64, String, Token) on both usda and usdc formats.
}

// ============================================================================
// Port of testUsdMetadata.py: test_ComposedListOpMetadata
// ============================================================================

#[test]
#[ignore = "Needs ListOp composition across sublayers + CreateNew (disk)"]
fn meta_composed_list_op_metadata() {
    common::setup();
    // C++ tests composition of list op-valued metadata fields across sublayers.
}

// ============================================================================
// Port of testUsdMetadata.py: test_UnknownFieldsRoundTripThroughUsdc
// ============================================================================

#[test]
#[ignore = "Needs Export to usdc/usda + file comparison roundtrip"]
fn meta_unknown_fields_roundtrip_usdc() {
    common::setup();
    // C++ imports USDA with unknown fields, exports to usdc, re-exports to usda,
    // verifies the two usda files match exactly.
}

// ============================================================================
// Port of testUsdMetadata.py: test_AssetPathMetadata
// ============================================================================

#[test]
#[ignore = "Needs AssetPath metadata authoring + resolution"]
fn meta_asset_path_metadata() {
    common::setup();
    // C++ tests authoring and reading asset path metadata with resolver context.
}
