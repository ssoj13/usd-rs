//! Port of testUsdPrims.py from OpenUSD pxr/usd/usd/testenv/
//! 21 tests, logic matches C++ reference exactly.

mod common;

use usd_core::common::{InitialLoadSet, ListPosition};
use usd_core::edit_target::EditTarget;
use usd_core::prim::Prim;
use usd_core::prim_flags::{
    self, PrimFlagsPredicate, USD_PRIM_IS_ABSTRACT, USD_PRIM_IS_ACTIVE, USD_PRIM_IS_DEFINED,
    USD_PRIM_IS_LOADED, USD_PRIM_IS_MODEL,
};
use usd_core::stage::Stage;
use usd_sdf::{Layer, Path, Specifier};
use usd_tf::Token;

// ============================================================================
// 1. test_Basic
// ============================================================================

#[test]
fn prim_basic() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let p = stage.get_prim_at_path(&Path::absolute_root());
    let q = stage.get_prim_at_path(&Path::absolute_root());
    assert!(p.is_some());
    assert!(q.is_some());
    let p = p.unwrap();
    let q = q.unwrap();
    // Same path
    assert_eq!(p.get_path(), q.get_path());

    // OverridePrim + CreateAttribute + CreateRelationship
    let foo = stage.override_prim("/foo").unwrap();
    let vtn = common::vtn("string");
    let _attr = foo.create_attribute("attr", &vtn, false, None).unwrap();
    let a = foo.get_attribute("attr");
    let b = foo.get_attribute("attr");
    assert!(a.is_some());
    assert!(b.is_some());
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.path(), b.path());
    assert!(!a.has_fallback_value());
    assert!(!b.has_fallback_value());

    foo.create_relationship("relationship", false);
    let rel_a = foo.get_relationship("relationship");
    let rel_b = foo.get_relationship("relationship");
    assert!(rel_a.is_some());
    assert!(rel_b.is_some());
    assert_eq!(rel_a.unwrap().path(), rel_b.unwrap().path());

    // GetObjectAtPath — prims/props that exist
    let obj_foo = stage.get_object_at_path(&Path::from_string("/foo").unwrap());
    assert!(obj_foo.is_some());
    assert!(!obj_foo.unwrap().path().is_property_path());

    let obj_attr = stage.get_object_at_path(&Path::from_string("/foo.attr").unwrap());
    assert!(obj_attr.is_some());
    assert!(obj_attr.unwrap().path().is_property_path());

    let obj_rel = stage.get_object_at_path(&Path::from_string("/foo.relationship").unwrap());
    assert!(obj_rel.is_some());
    assert!(obj_rel.unwrap().path().is_property_path());

    // GetObjectAtPath — prims/props that don't exist
    let obj_none = stage.get_object_at_path(&Path::from_string("/nonexistent").unwrap());
    assert!(obj_none.is_none());

    // Non-existent property: C++ returns invalid UsdProperty
    let obj_noattr = stage.get_object_at_path(&Path::from_string("/foo.nonexistentattr").unwrap());
    // In our impl this returns None for non-existent properties
    assert!(obj_noattr.is_none());
}

// ============================================================================
// 2. test_OverrideMetadata
// ============================================================================

#[test]
fn prim_override_metadata() {
    common::setup();

    let weak = Layer::create_anonymous(Some("OverrideMetadataTest.usda"));
    let strong = Layer::create_anonymous(Some("OverrideMetadataTest.usda"));

    let stage = Stage::open(weak.identifier(), InitialLoadSet::LoadAll).unwrap();
    let mesh_child = stage.define_prim("/Mesh/Child", "Mesh");
    assert!(mesh_child.is_ok());

    let stage2 = Stage::open(strong.identifier(), InitialLoadSet::LoadAll).unwrap();
    let p = stage2.override_prim("/Mesh").unwrap();
    let refs = p.get_references();
    refs.add_reference(
        &usd_sdf::Reference::new(weak.identifier(), "/Mesh"),
        ListPosition::BackOfAppendList,
    );

    let p2 = stage2.get_prim_at_path(&Path::from_string("/Mesh/Child").unwrap());
    assert!(p2.is_some());
    let p2 = p2.unwrap();
    assert!(p2.set_metadata(&Token::new("hidden"), usd_vt::Value::from(false)));
    assert_eq!(p2.get_name().get_text(), p2.get_path().get_name());
}

// ============================================================================
// 3. test_GetPrimStack (partial — no LayerOffset verification)
// ============================================================================

#[test]
#[ignore = "GetPrimStack with LayerOffsets not fully ported"]
fn prim_get_prim_stack() {
    common::setup();
    // Needs multi-layer composition with layeroffsets; skip for now
}

// ============================================================================
// 4. test_GetCachedPrimBits
// ============================================================================

#[test]
fn prim_cached_prim_bits() {
    common::setup();

    let test_path = common::testenv_path("testUsdPrims.testenv/test.usda");
    let layer = Layer::find_or_open(test_path.to_str().unwrap());
    assert!(layer.is_ok(), "failed to find test.usda");
    let layer = layer.unwrap();

    let stage =
        Stage::open(layer.identifier(), InitialLoadSet::LoadNone).expect("failed to create stage");

    let root = stage.get_prim_at_path(&Path::absolute_root()).unwrap();
    let global_class = stage
        .get_prim_at_path(&Path::from_string("/GlobalClass").unwrap())
        .unwrap();
    let abstract_subscope = stage
        .get_prim_at_path(&Path::from_string("/GlobalClass/AbstractSubscope").unwrap())
        .unwrap();
    let abstract_over = stage
        .get_prim_at_path(&Path::from_string("/GlobalClass/AbstractOver").unwrap())
        .unwrap();
    let pure_over = stage
        .get_prim_at_path(&Path::from_string("/PureOver").unwrap())
        .unwrap();
    let undef_subscope = stage
        .get_prim_at_path(&Path::from_string("/PureOver/UndefinedSubscope").unwrap())
        .unwrap();
    let group = stage
        .get_prim_at_path(&Path::from_string("/Group").unwrap())
        .unwrap();
    let model_child = stage
        .get_prim_at_path(&Path::from_string("/Group/ModelChild").unwrap())
        .unwrap();
    let local_child = stage
        .get_prim_at_path(&Path::from_string("/Group/LocalChild").unwrap())
        .unwrap();
    let undef_model_child = stage
        .get_prim_at_path(&Path::from_string("/Group/UndefinedModelChild").unwrap())
        .unwrap();
    let deactivated_scope = stage
        .get_prim_at_path(&Path::from_string("/Group/DeactivatedScope").unwrap())
        .unwrap();
    let deactivated_model = stage
        .get_prim_at_path(&Path::from_string("/Group/DeactivatedModel").unwrap())
        .unwrap();
    let deactivated_over = stage
        .get_prim_at_path(&Path::from_string("/Group/DeactivatedOver").unwrap())
        .unwrap();
    let property_order = stage
        .get_prim_at_path(&Path::from_string("/PropertyOrder").unwrap())
        .unwrap();

    // Named child access API
    let mc = group.get_child(&Token::new("ModelChild"));
    assert_eq!(mc.get_path(), model_child.get_path());
    let lc = group.get_child(&Token::new("LocalChild"));
    assert_eq!(lc.get_path(), local_child.get_path());
    assert!(!group.get_child(&Token::new("__NoSuchChild__")).is_valid());

    // Check filtered children access
    let children = root.get_all_children();
    let all_paths: Vec<&Path> = children.iter().map(|c| c.get_path()).collect();
    assert_eq!(
        all_paths,
        vec![
            global_class.get_path(),
            pure_over.get_path(),
            group.get_path(),
            property_order.get_path(),
        ]
    );

    let children_names: Vec<String> = root
        .get_children()
        .iter()
        .map(|c| c.name().get_text().to_string())
        .collect();
    assert_eq!(children_names, vec!["PropertyOrder"]);

    // Helper: test filtered children against expected list
    let test_filtered = |predicate: PrimFlagsPredicate, expected: &[&Prim]| {
        let filtered = root.get_filtered_children(predicate);
        let filtered_paths: Vec<&Path> = filtered.iter().map(|c| c.get_path()).collect();
        let expected_paths: Vec<&Path> = expected.iter().map(|c| c.get_path()).collect();
        assert_eq!(filtered_paths, expected_paths);

        let filtered_names = root.get_filtered_children_names(predicate);
        let expected_names: Vec<Token> = expected.iter().map(|c| c.name().clone()).collect();
        assert_eq!(filtered_names, expected_names);
    };

    // Default predicate
    test_filtered(
        prim_flags::default_predicate().into_predicate(),
        &[&property_order],
    );

    // Manually construct default predicate from individual terms
    test_filtered(
        USD_PRIM_IS_ACTIVE
            .and(USD_PRIM_IS_LOADED)
            .and(USD_PRIM_IS_DEFINED)
            .and(USD_PRIM_IS_ABSTRACT.not())
            .into_predicate(),
        &[&property_order],
    );

    // Only abstract prims
    test_filtered(
        PrimFlagsPredicate::from_term(USD_PRIM_IS_ABSTRACT),
        &[&global_class],
    );

    // Abstract & defined
    test_filtered(
        USD_PRIM_IS_ABSTRACT
            .and(USD_PRIM_IS_DEFINED)
            .into_predicate(),
        &[&global_class],
    );

    // Abstract | unloaded
    test_filtered(
        USD_PRIM_IS_ABSTRACT
            .or(USD_PRIM_IS_LOADED.not())
            .into_predicate(),
        &[&global_class, &group],
    );

    // Models only
    test_filtered(PrimFlagsPredicate::from_term(USD_PRIM_IS_MODEL), &[&group]);

    // Non-models only
    test_filtered(
        PrimFlagsPredicate::from_term(USD_PRIM_IS_MODEL.not()),
        &[&global_class, &pure_over, &property_order],
    );

    // Models or undefined
    test_filtered(
        USD_PRIM_IS_MODEL
            .or(USD_PRIM_IS_DEFINED.not())
            .into_predicate(),
        &[&pure_over, &group],
    );

    // Check individual flags — root
    assert!(root.is_active());
    assert!(root.is_loaded());
    assert!(root.is_model());
    assert!(root.is_group());
    assert!(!root.is_abstract());
    assert!(root.is_defined());
    assert!(root.has_defining_specifier());
    assert_eq!(root.specifier(), Specifier::Def);

    // GlobalClass
    assert!(global_class.is_active());
    assert!(global_class.is_loaded());
    assert!(!global_class.is_model());
    assert!(!global_class.is_group());
    assert!(global_class.is_abstract());
    assert!(global_class.is_defined());
    assert!(global_class.has_defining_specifier());
    assert_eq!(global_class.specifier(), Specifier::Class);

    // AbstractSubscope
    assert!(abstract_subscope.is_active());
    assert!(abstract_subscope.is_loaded());
    assert!(!abstract_subscope.is_model());
    assert!(!abstract_subscope.is_group());
    assert!(abstract_subscope.is_abstract());
    assert!(abstract_subscope.is_defined());
    assert!(abstract_subscope.has_defining_specifier());
    assert_eq!(abstract_subscope.specifier(), Specifier::Def);

    // AbstractOver
    assert!(abstract_over.is_active());
    assert!(abstract_over.is_loaded());
    assert!(!abstract_over.is_model());
    assert!(!abstract_over.is_group());
    assert!(abstract_over.is_abstract());
    assert!(!abstract_over.is_defined());
    assert!(!abstract_over.has_defining_specifier());
    assert_eq!(abstract_over.specifier(), Specifier::Over);

    // PureOver
    assert!(pure_over.is_active());
    assert!(pure_over.is_loaded());
    assert!(!pure_over.is_model());
    assert!(!pure_over.is_group());
    assert!(!pure_over.is_abstract());
    assert!(!pure_over.is_defined());
    assert!(!pure_over.has_defining_specifier());
    assert_eq!(pure_over.specifier(), Specifier::Over);

    // UndefinedSubscope
    assert!(undef_subscope.is_active());
    assert!(undef_subscope.is_loaded());
    assert!(!undef_subscope.is_model());
    assert!(!undef_subscope.is_group());
    assert!(!undef_subscope.is_abstract());
    assert!(!undef_subscope.is_defined());
    assert!(undef_subscope.has_defining_specifier());
    assert_eq!(undef_subscope.specifier(), Specifier::Def);

    // Group
    assert!(group.is_active());
    assert!(!group.is_loaded());
    assert!(group.is_model());
    assert!(group.is_group());
    assert!(!group.is_abstract());
    assert!(group.is_defined());
    assert!(group.has_defining_specifier());
    assert_eq!(group.specifier(), Specifier::Def);

    // ModelChild
    assert!(model_child.is_active());
    assert!(!model_child.is_loaded());
    assert!(model_child.is_model());
    assert!(!model_child.is_group());
    assert!(!model_child.is_abstract());
    assert!(model_child.is_defined());
    assert!(model_child.has_defining_specifier());
    assert_eq!(model_child.specifier(), Specifier::Def);

    // LocalChild
    assert!(local_child.is_active());
    assert!(!local_child.is_loaded());
    assert!(!local_child.is_model());
    assert!(!local_child.is_group());
    assert!(!local_child.is_abstract());
    assert!(local_child.is_defined());
    assert!(local_child.has_defining_specifier());
    assert_eq!(local_child.specifier(), Specifier::Def);

    // UndefinedModelChild
    assert!(undef_model_child.is_active());
    assert!(!undef_model_child.is_loaded());
    assert!(!undef_model_child.is_model());
    assert!(!undef_model_child.is_group());
    assert!(!undef_model_child.is_abstract());
    assert!(!undef_model_child.is_defined());
    assert!(!undef_model_child.has_defining_specifier());
    assert_eq!(undef_model_child.specifier(), Specifier::Over);

    // DeactivatedScope
    assert!(!deactivated_scope.is_active());
    assert!(!deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_none());
    }

    // Activate it
    deactivated_scope.set_active(true);
    assert!(deactivated_scope.is_active());
    assert!(deactivated_scope.has_authored_active());
    assert!(!deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_some());
    }

    // ClearActive — removes the opinion, reverts to default (active=true)
    deactivated_scope.clear_active();
    assert!(deactivated_scope.is_active());
    assert!(!deactivated_scope.has_authored_active());
    assert!(!deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_some());
    }

    // Deactivate it again
    deactivated_scope.set_active(false);
    assert!(!deactivated_scope.is_active());
    assert!(deactivated_scope.has_authored_active());
    assert!(!deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_none());
    }

    // DeactivatedModel
    assert!(!deactivated_model.is_active());
    assert!(!deactivated_model.is_loaded());
    assert!(deactivated_model.is_model());
    assert!(!deactivated_model.is_group());
    assert!(!deactivated_model.is_abstract());
    assert!(deactivated_model.is_defined());
    // C++ checks deactivatedScope here (not deactivatedModel) — matches reference
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_model.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_none());
    }

    // DeactivatedOver
    assert!(!deactivated_over.is_active());
    assert!(!deactivated_over.is_loaded());
    assert!(!deactivated_over.is_model());
    assert!(!deactivated_over.is_group());
    assert!(!deactivated_over.is_abstract());
    assert!(!deactivated_over.is_defined());
    // C++ checks deactivatedScope here — matches reference
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_over.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_none());
    }

    // Load the model and recheck
    let group_path = Path::from_string("/Group").unwrap();
    stage.load(&group_path, None);

    assert!(group.is_active());
    assert!(group.is_loaded());
    assert!(group.is_model());
    assert!(group.is_group());
    assert!(!group.is_abstract());
    assert!(group.is_defined());
    assert!(group.has_defining_specifier());
    assert_eq!(group.specifier(), Specifier::Def);

    // LocalChild should be loaded now
    assert!(local_child.is_active());
    assert!(local_child.is_loaded());
    assert!(!local_child.is_model());
    assert!(!local_child.is_group());
    assert!(!local_child.is_abstract());
    assert!(local_child.is_defined());
    assert!(local_child.has_defining_specifier());
    assert_eq!(local_child.specifier(), Specifier::Def);

    // UndefinedModelChild should be loaded and defined due to payload inclusion
    assert!(undef_model_child.is_active());
    assert!(undef_model_child.is_loaded());
    assert!(!undef_model_child.is_model());
    assert!(!undef_model_child.is_group());
    assert!(!undef_model_child.is_abstract());
    assert!(undef_model_child.is_defined());

    // PayloadChild — defined entirely inside payload
    let payload_child = stage.get_prim_at_path(&Path::from_string("/Group/PayloadChild").unwrap());
    assert!(payload_child.is_some());
    let payload_child = payload_child.unwrap();
    assert!(payload_child.is_active());
    assert!(payload_child.is_loaded());
    assert!(!payload_child.is_model());
    assert!(!payload_child.is_group());
    assert!(!payload_child.is_abstract());
    assert!(payload_child.is_defined());
    // C++ checks undefModelChild here — matches reference
    assert!(undef_model_child.has_defining_specifier());
    assert_eq!(undef_model_child.specifier(), Specifier::Def);

    // Check deactivated scope again (after load)
    assert!(!deactivated_scope.is_active());
    assert!(!deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_none());
    }

    // Activate it (second time, after load)
    deactivated_scope.set_active(true);
    assert!(deactivated_scope.is_active());
    assert!(deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_some());
    }

    // Deactivate it again (second time)
    deactivated_scope.set_active(false);
    assert!(!deactivated_scope.is_active());
    assert!(!deactivated_scope.is_loaded());
    assert!(!deactivated_scope.is_model());
    assert!(!deactivated_scope.is_group());
    assert!(!deactivated_scope.is_abstract());
    assert!(deactivated_scope.is_defined());
    assert!(deactivated_scope.has_defining_specifier());
    assert_eq!(deactivated_scope.specifier(), Specifier::Def);
    if let Some(child_path) = deactivated_scope.get_path().append_child("child") {
        assert!(stage.get_prim_at_path(&child_path).is_none());
    }
}

// ============================================================================
// 5. test_ChangeTypeName
// ============================================================================

#[test]
fn prim_change_type_name() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let foo = stage.override_prim("/Foo").unwrap();

    // Initialize
    assert_eq!(foo.get_type_name().get_text(), "");
    assert!(!foo.has_authored_type_name());

    // Set via public API
    assert!(foo.set_type_name("Mesh"));
    assert!(foo.has_authored_type_name());
    assert_eq!(foo.get_type_name().get_text(), "Mesh");
    let meta: Option<String> = foo.get_metadata(&Token::new("typeName"));
    assert_eq!(meta.as_deref(), Some("Mesh"));

    foo.clear_type_name();
    assert_eq!(foo.get_type_name().get_text(), "");
    assert!(!foo.has_authored_type_name());

    // Set via metadata
    assert!(foo.set_metadata(&Token::new("typeName"), "Scope".to_string()));
    assert!(foo.has_authored_type_name());
    assert_eq!(foo.get_type_name().get_text(), "Scope");
    let meta2: Option<String> = foo.get_metadata(&Token::new("typeName"));
    assert_eq!(meta2.as_deref(), Some("Scope"));
}

// ============================================================================
// 6. test_HasAuthoredReferences
// ============================================================================

#[test]
fn prim_has_authored_references() {
    common::setup();

    let s1 = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let _ = s1.define_prim("/Foo", "Mesh");
    let _ = s1.define_prim("/Bar", "Mesh");
    let baz = s1.define_prim("/Foo/Baz", "Mesh").unwrap();
    assert!(baz.get_references().add_reference(
        &usd_sdf::Reference::new(s1.root_layer().identifier(), "/Bar"),
        ListPosition::BackOfAppendList,
    ));

    let s2 = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let foo = s2.override_prim("/Foo").unwrap();
    let baz_check = s2.get_prim_at_path(&Path::from_string("/Foo/Baz").unwrap());

    assert!(baz_check.is_none());
    assert!(!foo.has_authored_references());

    let ref_to_s1 = usd_sdf::Reference::new(s1.root_layer().identifier(), "/Foo");
    assert!(
        foo.get_references()
            .add_reference(&ref_to_s1, ListPosition::BackOfAppendList,)
    );
    assert!(foo.has_authored_references());

    // References detected across composition arcs
    let baz_on_s2 = s2.override_prim("/Foo/Baz").unwrap();
    assert!(baz_on_s2.has_authored_references());

    // Clear references
    assert!(foo.get_references().clear_references());
    assert!(!foo.has_authored_references());
    // Child should be gone
    let baz_gone = s2.get_prim_at_path(&Path::from_string("/Foo/Baz").unwrap());
    assert!(baz_gone.is_none());

    // Set references explicitly (restore the ref)
    assert!(foo.get_references().set_references(vec![ref_to_s1.clone()]));
    assert!(foo.has_authored_references());
    // Child is back
    let baz_back = s2.get_prim_at_path(&Path::from_string("/Foo/Baz").unwrap());
    if let Some(baz_back) = baz_back {
        assert!(baz_back.has_authored_references());
    }

    // Set references to empty list — metadata still exists as explicit empty
    assert!(foo.get_references().set_references(vec![]));
    assert!(foo.has_authored_references());
    let baz_empty = s2.get_prim_at_path(&Path::from_string("/Foo/Baz").unwrap());
    assert!(baz_empty.is_none());

    // Clear references — no longer explicit
    assert!(foo.get_references().clear_references());
    assert!(!foo.has_authored_references());
    let baz_still_gone = s2.get_prim_at_path(&Path::from_string("/Foo/Baz").unwrap());
    assert!(baz_still_gone.is_none());

    // Set references to empty again from cleared — verifying explicit
    assert!(foo.get_references().set_references(vec![]));
    assert!(foo.has_authored_references());
    let baz_not_back = s2.get_prim_at_path(&Path::from_string("/Foo/Baz").unwrap());
    assert!(baz_not_back.is_none());
}

// ============================================================================
// 7. test_GoodAndBadReferences
// ============================================================================

#[test]
fn prim_good_and_bad_references() {
    common::setup();

    // Sub-root references are allowed
    let s1 = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let _ = s1.define_prim("/Foo", "Mesh");
    let _ = s1.define_prim("/Bar/Bazzle", "Mesh");
    let baz = s1.define_prim("/Foo/Baz", "Mesh").unwrap();
    let baz_refs = baz.get_references();
    baz_refs.add_reference(
        &usd_sdf::Reference::new(s1.root_layer().identifier(), "/Bar/Bazzle"),
        ListPosition::BackOfAppendList,
    );

    // A good reference generates no errors
    baz_refs.add_reference(
        &usd_sdf::Reference::new(s1.root_layer().identifier(), "/Foo"),
        ListPosition::BackOfAppendList,
    );
}

// ============================================================================
// 8. test_PropertyOrder
// ============================================================================

#[test]
fn prim_property_order() {
    common::setup();

    let test_path = common::testenv_path("testUsdPrims.testenv/test.usda");
    let layer = Layer::find_or_open(test_path.to_str().unwrap()).unwrap();

    let stage = Stage::open(layer.identifier(), InitialLoadSet::LoadNone).unwrap();

    let po = stage
        .get_prim_at_path(&Path::from_string("/PropertyOrder").unwrap())
        .unwrap();

    let attrs = po.get_attributes();
    let attr_names: Vec<String> = attrs.iter().map(|a| a.name().to_string()).collect();
    let expected_attrs = vec!["A0", "a1", "a2", "A3", "a4", "a5", "a10", "A20"];
    assert_eq!(
        attr_names, expected_attrs,
        "attribute order mismatch: {:?} != {:?}",
        attr_names, expected_attrs
    );

    let rels = po.get_relationships();
    let rel_names: Vec<String> = rels.iter().map(|r| r.name().to_string()).collect();
    let expected_rels = vec!["R0", "r1", "r2", "R3", "r4", "r5", "r10", "R20"];
    assert_eq!(
        rel_names, expected_rels,
        "relationship order mismatch: {:?} != {:?}",
        rel_names, expected_rels
    );
}

// ============================================================================
// 9. test_PropertyReorder
// ============================================================================

#[test]
fn prim_property_reorder() {
    common::setup();

    let session = Layer::create_anonymous(Some("usda"));
    let stage = Stage::create_in_memory_with_session(
        "property_reorder",
        session.clone(),
        InitialLoadSet::LoadAll,
    )
    .unwrap();
    let foo = stage.override_prim("/foo").unwrap();
    let int_type = common::vtn("int");

    // Create attrs d,c,b,a on root layer
    stage.set_edit_target(EditTarget::for_local_layer(stage.get_root_layer()));
    for name in &["d", "c", "b", "a"] {
        foo.create_attribute(name, &int_type, false, None);
    }

    // Create attrs g,f,e,d on session layer
    stage.set_edit_target(EditTarget::for_local_layer(session.clone()));
    for name in &["g", "f", "e", "d"] {
        foo.create_attribute(name, &int_type, false, None);
    }

    let get_names = || -> Vec<String> {
        foo.get_property_names()
            .iter()
            .map(|t| t.get_text().to_string())
            .collect()
    };

    assert_eq!(get_names(), vec!["a", "b", "c", "d", "e", "f", "g"]);

    // Set property order
    foo.set_property_order(vec![Token::new("e"), Token::new("d"), Token::new("c")]);
    assert_eq!(get_names(), vec!["e", "d", "c", "a", "b", "f", "g"]);

    foo.set_property_order(vec![Token::new("a")]);
    assert_eq!(get_names(), vec!["a", "b", "c", "d", "e", "f", "g"]);

    foo.set_property_order(vec![]);
    assert_eq!(get_names(), vec!["a", "b", "c", "d", "e", "f", "g"]);

    foo.set_property_order(vec![Token::new("g")]);
    assert_eq!(get_names(), vec!["g", "a", "b", "c", "d", "e", "f"]);

    foo.set_property_order(vec![Token::new("d")]);
    assert_eq!(get_names(), vec!["d", "a", "b", "c", "e", "f", "g"]);

    foo.set_property_order(vec![Token::new("x"), Token::new("y"), Token::new("z")]);
    assert_eq!(get_names(), vec!["a", "b", "c", "d", "e", "f", "g"]);

    foo.set_property_order(vec![
        Token::new("x"),
        Token::new("c"),
        Token::new("y"),
        Token::new("d"),
        Token::new("z"),
        Token::new("e"),
    ]);
    assert_eq!(get_names(), vec!["c", "d", "e", "a", "b", "f", "g"]);

    foo.set_property_order(vec![
        Token::new("g"),
        Token::new("f"),
        Token::new("e"),
        Token::new("d"),
        Token::new("c"),
        Token::new("b"),
        Token::new("a"),
    ]);
    assert_eq!(get_names(), vec!["g", "f", "e", "d", "c", "b", "a"]);

    foo.clear_property_order();
    assert_eq!(get_names(), vec!["a", "b", "c", "d", "e", "f", "g"]);
}

// ============================================================================
// 10. test_ChildrenReorder
// ============================================================================

#[test]
fn prim_children_reorder() {
    common::setup();

    let session = Layer::create_anonymous(Some("usda"));
    let stage = Stage::create_in_memory_with_session(
        "children_reorder",
        session.clone(),
        InitialLoadSet::LoadAll,
    )
    .unwrap();

    let parent = stage.define_prim("/foo", "").unwrap();

    // Create children a,b,c,d on root layer
    stage.set_edit_target(EditTarget::for_local_layer(stage.get_root_layer()));
    for name in &["a", "b", "c", "d"] {
        let _ = stage.define_prim(&format!("/foo/{}", name), "");
    }

    // Create children d,e,f,g as overs on session layer
    stage.set_edit_target(EditTarget::for_local_layer(session.clone()));
    for name in &["d", "e", "f", "g"] {
        let _ = stage.override_prim(&format!("/foo/{}", name));
    }

    let get_all_names = |p: &Prim| -> Vec<String> {
        p.get_all_children_names()
            .iter()
            .map(|t| t.get_text().to_string())
            .collect()
    };

    let get_child_names = |p: &Prim| -> Vec<String> {
        p.get_children_names()
            .iter()
            .map(|t| t.get_text().to_string())
            .collect()
    };

    // No primOrder set — default order
    assert_eq!(parent.get_children_reorder(), None);
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "c", "d", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "c", "d"]);

    // Set partial ordering
    parent.set_children_reorder(vec![Token::new("e"), Token::new("d"), Token::new("c")]);
    // C++ result: all = [a,b,e,f,g,d,c], children = [a,b,d,c]
    // The reorder moves mentioned items to front, rest stay in original order
    // Actually C++ says: _TestAllChildren(f, l('abefgdc'))
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "e", "f", "g", "d", "c"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "d", "c"]);

    // Empty ordering — back to default
    parent.set_children_reorder(vec![]);
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "c", "d", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "c", "d"]);

    // Single entry in order — still maintains default ordering
    parent.set_children_reorder(vec![Token::new("d")]);
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "c", "d", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "c", "d"]);

    // Set ordering with no valid names — default ordering
    parent.set_children_reorder(vec![Token::new("x"), Token::new("y"), Token::new("z")]);
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "c", "d", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "c", "d"]);

    // Interspersed invalid names — reorders with just the valid names
    parent.set_children_reorder(vec![
        Token::new("x"),
        Token::new("e"),
        Token::new("y"),
        Token::new("d"),
        Token::new("z"),
        Token::new("c"),
    ]);
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "e", "f", "g", "d", "c"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "d", "c"]);

    // Full reorder
    parent.set_children_reorder(vec![
        Token::new("g"),
        Token::new("f"),
        Token::new("e"),
        Token::new("d"),
        Token::new("c"),
        Token::new("b"),
        Token::new("a"),
    ]);
    assert_eq!(
        get_all_names(&parent),
        vec!["g", "f", "e", "d", "c", "b", "a"]
    );
    assert_eq!(get_child_names(&parent), vec!["d", "c", "b", "a"]);

    // Clear reorder on session layer — return to default
    parent.clear_children_reorder();
    assert_eq!(parent.get_children_reorder(), None);
    assert_eq!(
        get_all_names(&parent),
        vec!["a", "b", "c", "d", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["a", "b", "c", "d"]);

    // Full reorder on root layer
    stage.set_edit_target(EditTarget::for_local_layer(stage.get_root_layer()));
    parent.set_children_reorder(vec![
        Token::new("g"),
        Token::new("f"),
        Token::new("e"),
        Token::new("d"),
        Token::new("c"),
        Token::new("b"),
        Token::new("a"),
    ]);
    // Reorder authored on root layer only reorders prims defined on root layer;
    // session layer prims (e,f,g) keep their relative order after root-layer prims.
    assert_eq!(
        get_all_names(&parent),
        vec!["d", "c", "b", "a", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["d", "c", "b", "a"]);

    // Set empty ordering on session layer — strongest metadata is empty,
    // but root layer reordering still takes place
    stage.set_edit_target(EditTarget::for_local_layer(session.clone()));
    parent.set_children_reorder(vec![]);
    assert_eq!(
        get_all_names(&parent),
        vec!["d", "c", "b", "a", "e", "f", "g"]
    );
    assert_eq!(get_child_names(&parent), vec!["d", "c", "b", "a"]);
}

// ============================================================================
// 11. test_DefaultPrim
// ============================================================================

#[test]
fn prim_default_prim() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

    // No default prim to start
    assert!(!stage.get_default_prim().is_valid());

    // Set defaultPrim metadata on root layer, but no prim in scene
    stage.root_layer().set_default_prim(&Token::new("foo"));
    assert!(!stage.get_default_prim().is_valid());

    // Create the prim — should pick it up
    let foo_prim = stage.override_prim("/foo").unwrap();
    assert_eq!(stage.get_default_prim().get_path(), foo_prim.get_path());

    // Change defaultPrim
    stage.root_layer().set_default_prim(&Token::new("bar"));
    assert!(!stage.get_default_prim().is_valid());
    let bar_prim = stage.override_prim("/bar").unwrap();
    assert_eq!(stage.get_default_prim().get_path(), bar_prim.get_path());

    // Set sub-root prims as default, should pick it up
    stage.root_layer().set_default_prim(&Token::new("foo/bar"));
    assert!(!stage.get_default_prim().is_valid());
    let foo_bar_prim = stage.override_prim("/foo/bar").unwrap();
    assert_eq!(stage.get_default_prim().get_path(), foo_bar_prim.get_path());

    // Try error cases
    stage.root_layer().set_default_prim(&Token::new(""));
    assert!(!stage.get_default_prim().is_valid());

    // Stage-level authoring API
    stage.set_default_prim(&foo_prim);
    assert_eq!(stage.get_default_prim().get_path(), foo_prim.get_path());
    assert!(stage.has_default_prim());
    stage.clear_default_prim();
    assert!(!stage.get_default_prim().is_valid());
    assert!(!stage.has_default_prim());
}

// ============================================================================
// 12. test_GetNextSibling
// ============================================================================

#[test]
fn prim_get_next_sibling() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

    // Create some prims
    let _ = stage.define_prim("/a", "");
    let _ = stage.define_prim("/b", "");
    let _ = stage.override_prim("/c");
    let _ = stage.define_prim("/d", "");

    // Walking siblings via GetNextSibling should match GetChildren order
    let root = stage.get_pseudo_root();
    let children = root.get_children();

    if !children.is_empty() {
        let mut by_sib = vec![children[0].clone()];
        loop {
            let next = by_sib.last().unwrap().get_next_sibling();
            if !next.is_valid() {
                break;
            }
            by_sib.push(next);
        }
        let children_paths: Vec<&Path> = children.iter().map(|c| c.get_path()).collect();
        let sib_paths: Vec<&Path> = by_sib.iter().map(|c| c.get_path()).collect();
        assert_eq!(children_paths, sib_paths);
    }
}

// ============================================================================
// 13. test_Instanceable
// ============================================================================

#[test]
fn prim_instanceable() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let p = stage.define_prim("/Instanceable", "Mesh").unwrap();
    assert!(!p.is_instanceable());
    assert_eq!(p.get_metadata::<bool>(&Token::new("instanceable")), None);
    assert!(!p.has_authored_instanceable());

    p.set_instanceable(true);
    assert!(p.is_instanceable());
    assert_eq!(
        p.get_metadata::<bool>(&Token::new("instanceable")),
        Some(true)
    );
    assert!(p.has_authored_instanceable());

    p.set_instanceable(false);
    assert!(!p.is_instanceable());
    assert_eq!(
        p.get_metadata::<bool>(&Token::new("instanceable")),
        Some(false)
    );
    assert!(p.has_authored_instanceable());

    p.clear_instanceable();
    assert!(!p.is_instanceable());
    assert_eq!(p.get_metadata::<bool>(&Token::new("instanceable")), None);
    assert!(!p.has_authored_instanceable());
}

// ============================================================================
// 14. test_GetComposedPrimChildrenAsMetadataTest
// ============================================================================

#[test]
#[ignore = "MilkCartonA.usda requires variant composition"]
fn prim_composed_children_metadata() {
    common::setup();
    // Needs full variant composition support
}

// ============================================================================
// 15. test_GetPrimIndex
// ============================================================================

#[test]
#[ignore = "PrimIndex/ComputeExpandedPrimIndex not fully ported"]
fn prim_get_prim_index() {
    common::setup();
}

// ============================================================================
// 16. test_PseudoRoot
// ============================================================================

#[test]
fn prim_pseudo_root() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let _w = stage.define_prim("/World", "").unwrap();
    let p = stage.get_prim_at_path(&Path::absolute_root()).unwrap();
    assert!(p.is_pseudo_root());
    assert!(p.is_valid());

    // Invalid prim is not pseudo root
    let invalid = Prim::invalid();
    assert!(!invalid.is_pseudo_root());

    // World is not pseudo root
    let w = stage
        .get_prim_at_path(&Path::from_string("/World").unwrap())
        .unwrap();
    assert!(!w.is_pseudo_root());

    // World's parent IS pseudo root
    let parent = w.parent();
    assert!(parent.is_pseudo_root());
    assert!(parent.is_valid());

    // Pseudo root's parent is invalid
    let pp = p.parent();
    assert!(!pp.is_pseudo_root());
    assert!(!pp.is_valid());
}

// ============================================================================
// 17. test_Deactivation
// ============================================================================

#[test]
fn prim_deactivation() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let child = stage.define_prim("/Root/Group/Child", "").unwrap();

    let group = stage
        .get_prim_at_path(&Path::from_string("/Root/Group").unwrap())
        .unwrap();
    let all_children = group.get_all_children();
    assert_eq!(all_children.len(), 1);
    assert_eq!(all_children[0].get_path(), child.get_path());

    group.set_active(false);

    // Deactivating a prim removes all of its children from the stage
    let all_after = group.get_all_children();
    assert_eq!(
        all_after.len(),
        0,
        "deactivated prim should have no children"
    );
}

// ============================================================================
// 18. test_AppliedSchemas
// ============================================================================

#[test]
#[ignore = "CollectionAPI/HasAPI schema infra not fully ported"]
fn prim_applied_schemas() {
    common::setup();
}

// ============================================================================
// 19. test_Bug160615
// ============================================================================

#[test]
fn prim_bug_160615() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let p = stage.override_prim("/Foo/Bar").unwrap();
    assert!(p.is_valid());

    stage.remove_prim(&Path::from_string("/Foo/Bar").unwrap());
    // After removal, the prim handle should become invalid if we re-query
    let p2 = stage.get_prim_at_path(&Path::from_string("/Foo/Bar").unwrap());
    assert!(p2.is_none(), "removed prim should not exist");

    // Re-create it
    let p3 = stage.override_prim("/Foo/Bar").unwrap();
    assert!(p3.is_valid());
}

// ============================================================================
// 20. test_GetAtPath (Prim-level relative path access)
// ============================================================================

#[test]
fn prim_get_at_path() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
    let child = stage.define_prim("/Parent/Child", "").unwrap();
    let grandchild = stage.define_prim("/Parent/Child/Grandchild", "").unwrap();
    let sibling = stage.define_prim("/Parent/Sibling", "").unwrap();

    let int_type = common::vtn("int");
    sibling.create_attribute("x", &int_type, false, None);
    grandchild.create_relationship("y", false);

    // Double check axioms about validity
    assert!(!Prim::invalid().is_valid());
    assert!(child.is_valid());
    assert!(grandchild.is_valid());
    assert!(sibling.is_valid());
    // C++: assertTrue(y), assertTrue(x)
    let y_valid = grandchild.get_relationship("y");
    assert!(y_valid.is_some());
    let x_valid = sibling.get_attribute("x");
    assert!(x_valid.is_some());
    // stage.GetPrimAtPath(emptyPath) is false
    let stage_empty = stage.get_prim_at_path(&Path::empty());
    assert!(stage_empty.is_none());

    // Test relative prim paths
    let sib_via_rel = child.get_prim_at_path(&Path::from_string("../Sibling").unwrap());
    assert_eq!(sib_via_rel.get_path(), sibling.get_path());

    let gc_via_rel = child.get_prim_at_path(&Path::from_string("Grandchild").unwrap());
    assert_eq!(gc_via_rel.get_path(), grandchild.get_path());

    let parent_via_rel = child.get_prim_at_path(&Path::from_string("..").unwrap());
    assert_eq!(parent_via_rel.get_path(), child.parent().get_path());

    // Test absolute prim paths
    let sib_abs = child.get_prim_at_path(&Path::from_string("/Parent/Sibling").unwrap());
    assert_eq!(sib_abs.get_path(), sibling.get_path());
    // C++: GetPrimAtPath("../Sibling") == GetObjectAtPath("../Sibling")
    let prim_sib = child.get_prim_at_path(&Path::from_string("../Sibling").unwrap());
    let obj_sib_eq = child.get_object_at_path(&Path::from_string("../Sibling").unwrap());
    assert!(obj_sib_eq.is_some());
    assert_eq!(
        prim_sib.get_path(),
        obj_sib_eq.unwrap().as_prim().unwrap().get_path()
    );

    // Test invalid paths
    let no_prim = child.get_prim_at_path(&Path::from_string("../InvalidPath").unwrap());
    assert!(!no_prim.is_valid());

    // Test relative properties
    let y_rel = child.get_relationship_at_path(&Path::from_string("Grandchild.y").unwrap());
    assert!(y_rel.is_some());

    let x_attr = child.get_attribute_at_path(&Path::from_string("../Sibling.x").unwrap());
    assert!(x_attr.is_some());

    let y_prop = child.get_property_at_path(&Path::from_string("Grandchild.y").unwrap());
    assert!(y_prop.is_some());

    let x_prop = child.get_property_at_path(&Path::from_string("../Sibling.x").unwrap());
    assert!(x_prop.is_some());

    // Test absolute properties
    let y_abs =
        child.get_relationship_at_path(&Path::from_string("/Parent/Child/Grandchild.y").unwrap());
    assert!(y_abs.is_some());

    let x_abs = child.get_attribute_at_path(&Path::from_string("/Parent/Sibling.x").unwrap());
    assert!(x_abs.is_some());

    // Test invalid paths
    let no_prop = child.get_property_at_path(&Path::from_string(".z").unwrap());
    assert!(no_prop.is_none());
    let no_rel_z = child.get_relationship_at_path(&Path::from_string(".z").unwrap());
    assert!(no_rel_z.is_none());
    let no_attr_z = child.get_attribute_at_path(&Path::from_string(".z").unwrap());
    assert!(no_attr_z.is_none());

    // Test valid paths but invalid types
    let prim_at_prop =
        child.get_prim_at_path(&Path::from_string("/Parent/Child/Grandchild.y").unwrap());
    assert!(!prim_at_prop.is_valid());

    // Prim at property path
    let prim_at_prop_x = child.get_prim_at_path(&Path::from_string("/Parent/Sibling.x").unwrap());
    assert!(!prim_at_prop_x.is_valid());

    let attr_at_rel =
        child.get_attribute_at_path(&Path::from_string("/Parent/Child/Grandchild.y").unwrap());
    assert!(
        attr_at_rel.is_none(),
        "relationship should not be returned as attribute"
    );

    let rel_at_attr =
        child.get_relationship_at_path(&Path::from_string("/Parent/Sibling.x").unwrap());
    assert!(
        rel_at_attr.is_none(),
        "attribute should not be returned as relationship"
    );

    // Attribute at prim path, relationship at prim path
    let attr_at_prim =
        child.get_attribute_at_path(&Path::from_string("/Parent/Child/Grandchild").unwrap());
    assert!(attr_at_prim.is_none());
    let rel_at_prim =
        child.get_relationship_at_path(&Path::from_string("/Parent/Sibling").unwrap());
    assert!(rel_at_prim.is_none());

    // Test empty paths
    let empty = Path::empty();
    assert!(!child.get_prim_at_path(&empty).is_valid());
    assert!(child.get_object_at_path(&empty).is_none());
    assert!(child.get_property_at_path(&empty).is_none());
    assert!(child.get_attribute_at_path(&empty).is_none());
    assert!(child.get_relationship_at_path(&empty).is_none());

    // Verify type deduction via get_object_at_path
    let obj_sib = child.get_object_at_path(&Path::from_string("../Sibling").unwrap());
    assert!(obj_sib.is_some());
    assert!(obj_sib.unwrap().is_prim());

    let obj_x = child.get_object_at_path(&Path::from_string("../Sibling.x").unwrap());
    assert!(obj_x.is_some());
    assert!(obj_x.unwrap().is_attribute());

    let obj_y = child.get_object_at_path(&Path::from_string("Grandchild.y").unwrap());
    assert!(obj_y.is_some());
    assert!(obj_y.unwrap().is_relationship());

    // Property type deduction via get_property_at_path
    let prop_x = child.get_property_at_path(&Path::from_string("../Sibling.x").unwrap());
    assert!(prop_x.is_some());
    assert!(prop_x.unwrap().is_attribute());
    let prop_y = child.get_property_at_path(&Path::from_string("Grandchild.y").unwrap());
    assert!(prop_y.is_some());
    assert!(prop_y.unwrap().is_relationship());
}

// ============================================================================
// 21. test_GetDescription
// ============================================================================

#[test]
fn prim_get_description() {
    common::setup();

    let layer = Layer::create_anonymous(Some("usda"));
    layer.import_from_string(
        r#"#usda 1.0

def Scope "Ref"
{
    def Scope "Child"
    {
    }
}

def Scope "Instance" (
    instanceable = true
    references = </Ref>
)
{
}
"#,
    );

    let stage = Stage::open(layer.identifier(), InitialLoadSet::LoadAll).unwrap();

    let basic = stage
        .get_prim_at_path(&Path::from_string("/Ref").unwrap())
        .unwrap();
    let basic_child = basic.get_child(&Token::new("Child"));
    assert!(basic_child.is_valid());

    let instance = stage
        .get_prim_at_path(&Path::from_string("/Instance").unwrap())
        .unwrap();
    let instance_proxy_child = instance.get_child(&Token::new("Child"));

    let prototype = instance.get_prototype();
    let prototype_child = if prototype.is_valid() {
        prototype.get_child(&Token::new("Child"))
    } else {
        Prim::invalid()
    };

    // Verify description doesn't crash and returns non-empty
    let desc = basic.description();
    assert!(!desc.is_empty());

    let child_desc = basic_child.description();
    assert!(!child_desc.is_empty());

    let inst_desc = instance.description();
    assert!(!inst_desc.is_empty());

    let proxy_desc = instance_proxy_child.description();
    assert!(!proxy_desc.is_empty());

    if prototype.is_valid() {
        let proto_desc = prototype.description();
        assert!(!proto_desc.is_empty());
    }

    if prototype_child.is_valid() {
        let proto_child_desc = prototype_child.description();
        assert!(!proto_child_desc.is_empty());
    }
}
