//! Port of testUsdClasses.py from OpenUSD pxr/usd/usd/testenv/
//! 1 test: test_Basic — CreateClassPrim + inherits + attribute override.

mod common;

use usd_core::common::InitialLoadSet;
use usd_core::stage::Stage;
use usd_sdf::Path;
use usd_sdf::Specifier;
use usd_vt::Value;

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap()
}

// ============================================================================
// 1. test_Basic (C++ test_Basic)
// ============================================================================

#[test]
fn classes_basic() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

    // Create a new class "foo" and set some attributes
    let foo = stage.create_class_prim("/foo").unwrap();
    let bar = foo
        .create_attribute("bar", &common::vtn("int"), false, None)
        .expect("create bar");
    bar.set(Value::from(42i32), usd_sdf::TimeCode::default());
    assert_eq!(
        bar.get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<i32>().copied()),
        Some(42)
    );

    let baz = foo
        .create_attribute("baz", &common::vtn("int"), false, None)
        .expect("create baz");
    baz.set(Value::from(24i32), usd_sdf::TimeCode::default());
    assert_eq!(
        baz.get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<i32>().copied()),
        Some(24)
    );

    // Create a new prim that will become an instance of "foo"
    let foo_derived = stage.define_prim("/fooDerived", "Scope").unwrap();
    // C++ uses IsDefined() — in Rust, get_attribute always returns Some (lazy proxy).
    // Check via is_valid() or has_value() instead.
    assert!(
        !foo_derived
            .get_attribute("bar")
            .map_or(false, |a| a.is_valid()),
        "bar should not be defined on fooDerived yet"
    );
    assert!(
        !foo_derived
            .get_attribute("baz")
            .map_or(false, |a| a.is_valid()),
        "baz should not be defined on fooDerived yet"
    );

    // Author local baz=42 on fooDerived
    let fd_baz = foo_derived
        .create_attribute("baz", &common::vtn("int"), false, None)
        .expect("create fd baz");
    fd_baz.set(Value::from(42i32), usd_sdf::TimeCode::default());
    assert_eq!(
        fd_baz
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<i32>().copied()),
        Some(42)
    );

    // Add inherit arc: fooDerived inherits /foo
    let inherits = foo_derived.get_inherits();
    assert!(inherits.add_inherit(
        &p("/foo"),
        usd_core::common::ListPosition::BackOfPrependList
    ));

    // bar should now come through from the class
    let bar_attr = foo_derived.get_attribute("bar").unwrap();
    assert!(bar_attr.is_valid(), "bar should be defined via inherit");
    assert_eq!(
        bar_attr
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<i32>().copied()),
        Some(42)
    );

    // baz should be 42 (local opinion overrides class's 24)
    let baz_attr = foo_derived.get_attribute("baz").unwrap();
    assert!(baz_attr.is_valid(), "baz should be defined");
    assert_eq!(
        baz_attr
            .get(usd_sdf::TimeCode::default())
            .and_then(|v| v.get::<i32>().copied()),
        Some(42)
    );

    // CreateClassPrim can create at subroot path
    assert!(stage.get_prim_at_path(&p("/foo")).is_some());
    let child = stage.create_class_prim("/foo/child").unwrap();
    assert_eq!(child.get_path(), &p("/foo/child"));
    assert_eq!(child.specifier(), Specifier::Class);

    // CreateClassPrim at subroot path even if parent doesn't exist yet
    assert!(stage.get_prim_at_path(&p("/foo2")).is_none());
    let child2 = stage.create_class_prim("/foo2/child").unwrap();
    assert_eq!(child2.get_path(), &p("/foo2/child"));
    assert_eq!(child2.specifier(), Specifier::Class);
    assert!(
        stage.get_prim_at_path(&p("/foo2")).is_some(),
        "parent /foo2 should have been auto-created"
    );
}
