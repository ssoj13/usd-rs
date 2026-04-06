use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| usd_sdf::init());
}

//! Port of testUsdGeomXformCache.cpp
//!
//! Tests XformCache: GetLocalToWorldTransform, GetParentToWorldTransform,
//! ComputeRelativeTransform, SetTime, Clear, and multi-stage usage.

use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::*;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::Vec3d;
use usd_sdf::TimeCode;

fn assert_close(a: &Matrix4d, b: &Matrix4d, msg: &str) {
    let eps = 1e-4;
    for row in 0..4 {
        for col in 0..4 {
            let va = a[row][col];
            let vb = b[row][col];
            assert!((va - vb).abs() < eps, "{msg}: [{row}][{col}]: {va} vs {vb}");
        }
    }
}

fn get_xform() -> Matrix4d {
    let mut m = Matrix4d::identity();
    m.set_translate(&Vec3d::new(10.0, 20.0, 30.0));
    m
}

/// Create test scene with time_shift applied to keyframe times.
///
/// Hierarchy:
///   /RootPrim         (Xform, has transform op)
///   /RootPrim/Scope   (Scope, no transform)
///   /RootPrim/Scope/Foo          (Xform, has transform op)
///   /RootPrim/Scope/Foo/Bar      (Xform, resets xform stack, has transform op)
///   /RootPrim/Scope/Foo/Bar/Baz  (Xform, has transform op)
///   /RootPrim/Scope/Bar          (Xform, has transform op)
fn create_test_data(time_shift: f64) -> Arc<Stage> {
    let stage =
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("Failed to create in-memory stage");

    let root_prim_path = usd_sdf::Path::from_string("/RootPrim").unwrap();
    let scope_prim_path = usd_sdf::Path::from_string("/RootPrim/Scope").unwrap();
    let foo_path = usd_sdf::Path::from_string("/RootPrim/Scope/Foo").unwrap();
    let foo_bar_path = usd_sdf::Path::from_string("/RootPrim/Scope/Foo/Bar").unwrap();
    let foo_bar_baz_path = usd_sdf::Path::from_string("/RootPrim/Scope/Foo/Bar/Baz").unwrap();
    let bar_path = usd_sdf::Path::from_string("/RootPrim/Scope/Bar").unwrap();

    // Define prims
    let _scope = Scope::define(&stage, &scope_prim_path);
    let root_xf = Xform::define(&stage, &root_prim_path);
    let foo_xf = Xform::define(&stage, &foo_path);
    let foo_bar_xf = Xform::define(&stage, &foo_bar_path);
    let foo_bar_baz_xf = Xform::define(&stage, &foo_bar_baz_path);
    let bar_xf = Xform::define(&stage, &bar_path);

    let xform = get_xform();
    let xform2 = xform * xform;
    let xform3 = xform * xform * xform;

    // Root: set transform at default, time 1+shift, time 2+shift
    let root_op = root_xf
        .xformable()
        .add_transform_op(XformOpPrecision::Double, None, false);
    root_op.set(xform, TimeCode::default_time());
    root_op.set(xform2, TimeCode::new(1.0 + time_shift));
    root_op.set(xform3, TimeCode::new(2.0 + time_shift));

    // Foo: same pattern
    let foo_op = foo_xf
        .xformable()
        .add_transform_op(XformOpPrecision::Double, None, false);
    foo_op.set(xform, TimeCode::default_time());
    foo_op.set(xform2, TimeCode::new(1.0 + time_shift));
    foo_op.set(xform3, TimeCode::new(2.0 + time_shift));

    // FooBar: resets xform stack
    let foo_bar_xfable = foo_bar_xf.xformable();
    foo_bar_xfable.set_reset_xform_stack(true);
    let foo_bar_op = foo_bar_xfable.add_transform_op(XformOpPrecision::Double, None, false);
    foo_bar_op.set(xform, TimeCode::default_time());
    foo_bar_op.set(xform2, TimeCode::new(1.0 + time_shift));
    foo_bar_op.set(xform3, TimeCode::new(2.0 + time_shift));

    // FooBarBaz
    let foo_bar_baz_op =
        foo_bar_baz_xf
            .xformable()
            .add_transform_op(XformOpPrecision::Double, None, false);
    foo_bar_baz_op.set(xform, TimeCode::default_time());
    foo_bar_baz_op.set(xform2, TimeCode::new(1.0 + time_shift));
    foo_bar_baz_op.set(xform3, TimeCode::new(2.0 + time_shift));

    // Bar
    let bar_op = bar_xf
        .xformable()
        .add_transform_op(XformOpPrecision::Double, None, false);
    bar_op.set(xform, TimeCode::default_time());
    bar_op.set(xform2, TimeCode::new(1.0 + time_shift));
    bar_op.set(xform3, TimeCode::new(2.0 + time_shift));

    stage
}

/// Verify all transforms match the C++ VerifyTransforms function.
///
/// `xform` is the local transform that each xformable prim has at the current time.
fn verify_transforms(stage: &Arc<Stage>, xf_cache: &mut XformCache, xform: Matrix4d) {
    let root_prim_path = usd_sdf::Path::from_string("/RootPrim").unwrap();
    let foo_path = usd_sdf::Path::from_string("/RootPrim/Scope/Foo").unwrap();
    let foo_bar_path = usd_sdf::Path::from_string("/RootPrim/Scope/Foo/Bar").unwrap();
    let foo_bar_baz_path = usd_sdf::Path::from_string("/RootPrim/Scope/Foo/Bar/Baz").unwrap();
    let bar_path = usd_sdf::Path::from_string("/RootPrim/Scope/Bar").unwrap();

    let root = stage.get_prim_at_path(&root_prim_path).expect("root prim");
    let foo = stage.get_prim_at_path(&foo_path).expect("foo prim");
    let foo_bar = stage.get_prim_at_path(&foo_bar_path).expect("fooBar prim");
    let foo_bar_baz = stage
        .get_prim_at_path(&foo_bar_baz_path)
        .expect("fooBarBaz prim");
    let bar = stage.get_prim_at_path(&bar_path).expect("bar prim");

    let identity = Matrix4d::identity();

    // ---------------------------------------------------------------
    // GetLocalToWorldTransform
    // ---------------------------------------------------------------

    // Pseudo root: identity
    let ctm = xf_cache.get_local_to_world_transform(&stage.pseudo_root());
    assert_close(&ctm, &identity, "pseudo_root L2W");

    assert!(
        !xf_cache.transform_might_be_time_varying(&stage.pseudo_root()),
        "pseudo_root should not be time varying"
    );
    assert!(
        !xf_cache.get_reset_xform_stack(&stage.pseudo_root()),
        "pseudo_root should not reset xform stack"
    );

    // Root: xform
    let ctm = xf_cache.get_local_to_world_transform(&root);
    assert_close(&ctm, &xform, "root L2W");
    assert!(
        xf_cache.transform_might_be_time_varying(&root),
        "root should be time varying"
    );
    assert!(
        !xf_cache.get_reset_xform_stack(&root),
        "root should not reset xform stack"
    );

    // Foo: xform * xform (Scope has no transform, so Foo's parent world = root's xform)
    let ctm = xf_cache.get_local_to_world_transform(&foo);
    assert_close(&ctm, &(xform * xform), "foo L2W");
    assert!(
        xf_cache.transform_might_be_time_varying(&foo),
        "foo should be time varying"
    );
    assert!(
        !xf_cache.get_reset_xform_stack(&foo),
        "foo should not reset xform stack"
    );

    // FooBar: resets xform stack, so world = xform (just local)
    let ctm = xf_cache.get_local_to_world_transform(&foo_bar);
    assert_close(&ctm, &xform, "fooBar L2W");
    assert!(
        xf_cache.transform_might_be_time_varying(&foo_bar),
        "fooBar should be time varying"
    );
    assert!(
        xf_cache.get_reset_xform_stack(&foo_bar),
        "fooBar should reset xform stack"
    );

    // FooBarBaz: parent (fooBar) resets, so world = xform * xform
    let ctm = xf_cache.get_local_to_world_transform(&foo_bar_baz);
    assert_close(&ctm, &(xform * xform), "fooBarBaz L2W");
    assert!(
        xf_cache.transform_might_be_time_varying(&foo_bar_baz),
        "fooBarBaz should be time varying"
    );
    assert!(
        !xf_cache.get_reset_xform_stack(&foo_bar_baz),
        "fooBarBaz should not reset xform stack"
    );

    // Bar: parent is Scope (no transform), grandparent is Root (xform), so world = xform * xform
    let ctm = xf_cache.get_local_to_world_transform(&bar);
    assert_close(&ctm, &(xform * xform), "bar L2W");
    assert!(
        xf_cache.transform_might_be_time_varying(&bar),
        "bar should be time varying"
    );
    assert!(
        !xf_cache.get_reset_xform_stack(&bar),
        "bar should not reset xform stack"
    );

    // ---------------------------------------------------------------
    // GetParentToWorldTransform
    // ---------------------------------------------------------------

    // Pseudo root parent: identity
    let ctm = xf_cache.get_parent_to_world_transform(&stage.pseudo_root());
    assert_close(&ctm, &identity, "pseudo_root P2W");

    // Root parent (pseudo root): identity
    let ctm = xf_cache.get_parent_to_world_transform(&root);
    assert_close(&ctm, &identity, "root P2W");

    // Foo parent (Scope->Root): xform
    let ctm = xf_cache.get_parent_to_world_transform(&foo);
    assert_close(&ctm, &xform, "foo P2W");

    // FooBar parent (Foo): xform*xform
    let ctm = xf_cache.get_parent_to_world_transform(&foo_bar);
    assert_close(&ctm, &(xform * xform), "fooBar P2W");

    // FooBarBaz parent (FooBar, which resets): xform
    let ctm = xf_cache.get_parent_to_world_transform(&foo_bar_baz);
    assert_close(&ctm, &xform, "fooBarBaz P2W");

    // Bar parent (Scope->Root): xform
    let ctm = xf_cache.get_parent_to_world_transform(&bar);
    assert_close(&ctm, &xform, "bar P2W");

    // ---------------------------------------------------------------
    // ComputeRelativeTransform
    // ---------------------------------------------------------------

    // Root relative to pseudo root: xform
    let (ctm, _resets) = xf_cache.compute_relative_transform(&root, &stage.pseudo_root());
    assert_close(&ctm, &xform, "root rel pseudo_root");

    // Foo relative to root: xform (Scope contributes identity)
    let (ctm, _resets) = xf_cache.compute_relative_transform(&foo, &root);
    assert_close(&ctm, &xform, "foo rel root");

    // FooBar relative to root: xform (resets xform stack)
    let (ctm, _resets) = xf_cache.compute_relative_transform(&foo_bar, &root);
    assert_close(&ctm, &xform, "fooBar rel root");

    // FooBarBaz relative to root: xform * xform (fooBar resets)
    let (ctm, _resets) = xf_cache.compute_relative_transform(&foo_bar_baz, &root);
    assert_close(&ctm, &(xform * xform), "fooBarBaz rel root");

    // Bar relative to root: xform
    let (ctm, _resets) = xf_cache.compute_relative_transform(&bar, &root);
    assert_close(&ctm, &xform, "bar rel root");
}

// ============================================================================
// Main test
// ============================================================================

#[test]
fn test_xform_cache() {
    setup();
    let stage = create_test_data(0.0);
    let xform = get_xform();

    // Verify at default time
    let mut xf_cache = XformCache::default();
    verify_transforms(&stage, &mut xf_cache, xform);

    // Verify at time=1.0 via SetTime
    xf_cache.set_time(TimeCode::new(1.0));
    let xform2 = xform * xform;
    verify_transforms(&stage, &mut xf_cache, xform2);

    // Verify at time=2.0 via new cache
    let mut xf_cache = XformCache::new(TimeCode::new(2.0));
    let xform3 = xform * xform * xform;
    verify_transforms(&stage, &mut xf_cache, xform3);

    // Verify after Clear(), same time=2.0
    xf_cache.clear();
    verify_transforms(&stage, &mut xf_cache, xform3);

    // Verify after SetTime to default
    xf_cache.set_time(TimeCode::default_time());
    verify_transforms(&stage, &mut xf_cache, xform);

    // Verify mixed stages
    xf_cache.set_time(TimeCode::new(2.0));
    verify_transforms(&stage, &mut xf_cache, xform3);

    // Alternate stage with time_shift=1 -> at time=2.0 it should read what
    // the default stage has at time=1.0 (xform*xform)
    let alt_stage = create_test_data(1.0);
    verify_transforms(&alt_stage, &mut xf_cache, xform2);

    // Original stage should still work
    verify_transforms(&stage, &mut xf_cache, xform3);
}

// ============================================================================
// Additional coverage: get_time / swap
// ============================================================================

#[test]
fn test_xform_cache_get_time() {
    setup();
    let mut cache = XformCache::new(TimeCode::new(5.0));
    assert!((cache.get_time().value() - 5.0).abs() < 1e-9);

    cache.set_time(TimeCode::new(10.0));
    assert!((cache.get_time().value() - 10.0).abs() < 1e-9);
}

#[test]
fn test_xform_cache_swap() {
    setup();
    let mut a = XformCache::new(TimeCode::new(1.0));
    let mut b = XformCache::new(TimeCode::new(2.0));

    a.swap(&mut b);

    assert!((a.get_time().value() - 2.0).abs() < 1e-9);
    assert!((b.get_time().value() - 1.0).abs() < 1e-9);
}
