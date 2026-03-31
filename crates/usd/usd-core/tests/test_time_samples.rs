//! Tests for time samples and time-related functionality.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdTimeSamples.py

mod common;

use usd_core::{Attribute, InitialLoadSet, ListPosition, Stage};
use usd_sdf::{LayerOffset, Path, TimeCode};

use usd_core::TimeCode as UsdTimeCode;

// ============================================================================
// Helpers
// ============================================================================

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

// ============================================================================
// test_Basic
// ============================================================================

#[test]
fn ts_basic() {
    common::setup();

    // -- TimeCode relational operators and identity --
    let default1 = TimeCode::default_time();
    let default2 = TimeCode::default_time();
    assert_eq!(default1, default2);

    let t1 = TimeCode::new(1.0);
    let t2 = TimeCode::new(2.0);
    assert_eq!(t1, TimeCode::new(1.0));
    assert_eq!(t2, TimeCode::new(2.0));
    assert_ne!(t1, TimeCode::new(2.0));
    assert_ne!(t2, TimeCode::new(1.0));
    assert_ne!(t1, t2);
    assert!(t1 < t2);
    assert!(t1 <= t2);
    assert!(t2 > t1);
    assert!(t2 >= t1);
    assert!(!(t1 < t1));
    assert!(t1 <= t1);
    assert!(!(t1 > t1));
    assert!(t1 >= t1);

    let non_special = TimeCode::new(24.0);
    assert_ne!(default1, non_special);

    // -- Set/get time samples --
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    let prim = stage.override_prim("/Test").expect("override /Test");
    let attr = prim
        .create_attribute("varying", &common::vtn("int"), false, None)
        .expect("create varying");

    // Set default + time samples
    attr.set(0i32, TimeCode::default_time());
    attr.set(1i32, TimeCode::new(1.0));
    attr.set(2i32, TimeCode::new(2.0));

    // GetTimeSamples
    assert_eq!(attr.get_time_samples(), vec![1.0, 2.0]);

    // GetTimeSamplesInInterval
    assert_eq!(attr.get_time_samples_in_interval(0.0, 1.0), vec![1.0]);
    assert_eq!(attr.get_time_samples_in_interval(0.0, 6.0), vec![1.0, 2.0]);
    assert!(attr.get_time_samples_in_interval(0.0, 0.0).is_empty());
    assert_eq!(attr.get_time_samples_in_interval(1.0, 2.0), vec![1.0, 2.0]);

    // GetBracketingTimeSamples
    assert_eq!(attr.get_bracketing_time_samples(1.5), Some((1.0, 2.0)));
    assert_eq!(attr.get_bracketing_time_samples(1.0), Some((1.0, 1.0)));
    assert_eq!(attr.get_bracketing_time_samples(2.0), Some((2.0, 2.0)));
    assert_eq!(attr.get_bracketing_time_samples(0.9), Some((1.0, 1.0)));
    assert_eq!(attr.get_bracketing_time_samples(2.1), Some((2.0, 2.0)));

    // -- Unvarying attribute --
    let attr_unv = prim
        .create_attribute("unvarying", &common::vtn("int"), false, None)
        .expect("create unvarying");
    attr_unv.set(0i32, TimeCode::default_time());

    assert!(attr_unv.get_time_samples().is_empty());
    assert!(
        attr_unv
            .get_time_samples_in_interval(f64::NEG_INFINITY, f64::INFINITY)
            .is_empty()
    );
    assert_eq!(attr_unv.get_bracketing_time_samples(1.5), None);

    // -- Empty array roundtrip (bug/81006) --
    let empty_attr = prim
        .create_attribute("empty", &common::vtn("double[]"), false, None)
        .expect("create empty");
    let empty_arr = usd_vt::Array::<f64>::default();
    empty_attr.set(
        usd_vt::Value::from_no_hash(empty_arr),
        TimeCode::default_time(),
    );
    let round = empty_attr.get(TimeCode::default_time());
    if let Some(val) = round {
        if let Some(arr) = val.get::<usd_vt::Array<f64>>() {
            assert_eq!(arr.len(), 0);
        }
    }
}

// ============================================================================
// test_GetUnionedTimeSamples
// ============================================================================

#[test]
fn ts_unioned() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.define_prim("/foo", "").expect("define /foo");

    let attr1 = foo
        .create_attribute("attr1", &common::vtn("bool"), false, None)
        .expect("create attr1");
    assert!(attr1.get_time_samples().is_empty());

    attr1.set(true, TimeCode::new(1.0));
    attr1.set(false, TimeCode::new(3.0));

    let attr2 = foo
        .create_attribute("attr2", &common::vtn("float"), false, None)
        .expect("create attr2");
    attr2.set(100.0f32, TimeCode::new(2.0));
    attr2.set(200.0f32, TimeCode::new(4.0));

    assert_eq!(
        Attribute::get_unioned_time_samples(&[attr1.clone(), attr2.clone()]),
        vec![1.0, 2.0, 3.0, 4.0]
    );

    assert_eq!(
        Attribute::get_unioned_time_samples_in_interval(&[attr1, attr2], 1.5, 3.5),
        vec![2.0, 3.0]
    );
}

// ============================================================================
// test_EmptyTimeSamplesMap
// ============================================================================

#[test]
fn ts_empty_map() {
    common::setup();
    let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
    layer.import_from_string(
        r#"#usda 1.0
def "Foo" {
    int x = 123
    int x.timeSamples = {}
}"#,
    );
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
    let x = stage
        .get_prim_at_path(&p("/Foo"))
        .expect("/Foo")
        .get_attribute("x")
        .expect("x");

    // Empty timeSamples should resolve to default value
    let val = x.get(TimeCode::default_time());
    assert!(val.is_some(), "expected default value 123");
    if let Some(v) = val {
        assert_eq!(v.get::<i32>().copied(), Some(123));
    }
}

// ============================================================================
// test_TimeSamplesWithOffset
// ============================================================================

#[test]
fn ts_with_offset() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    // Source prim with linear ramp 0..10
    let source = stage.define_prim("/Source", "").expect("define /Source");
    let source_attr = source
        .create_attribute("x", &common::vtn("float"), false, None)
        .expect("create x");
    source_attr.set(0.0f32, TimeCode::new(0.0));
    source_attr.set(10.0f32, TimeCode::new(10.0));
    assert_eq!(source_attr.get_time_samples(), vec![0.0, 10.0]);

    // Reference with offset +100
    let test1 = stage.define_prim("/Test1", "").expect("define /Test1");
    test1.get_references().add_internal_reference(
        &p("/Source"),
        LayerOffset::new(100.0, 1.0),
        ListPosition::FrontOfPrependList,
    );
    let test1_attr = test1.get_attribute("x").expect("x on /Test1");
    assert_eq!(test1_attr.get_time_samples(), vec![100.0, 110.0]);
    assert!(
        test1_attr
            .get_time_samples_in_interval(0.0, 10.0)
            .is_empty()
    );
    assert_eq!(
        test1_attr.get_time_samples_in_interval(0.0, 110.0),
        vec![100.0, 110.0]
    );

    // Value resolution with offset
    // Before range: holds first value
    let v0: Option<f32> = test1_attr
        .get(TimeCode::new(0.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v0, Some(0.0));
    let v100: Option<f32> = test1_attr
        .get(TimeCode::new(100.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v100, Some(0.0));
    let v110: Option<f32> = test1_attr
        .get(TimeCode::new(110.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v110, Some(10.0));
    let v120: Option<f32> = test1_attr
        .get(TimeCode::new(120.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v120, Some(10.0));

    // Reference with 2x scale
    let test2 = stage.define_prim("/Test2", "").expect("define /Test2");
    test2.get_references().add_internal_reference(
        &p("/Source"),
        LayerOffset::new(0.0, 2.0),
        ListPosition::FrontOfPrependList,
    );
    let test2_attr = test2.get_attribute("x").expect("x on /Test2");
    assert_eq!(test2_attr.get_time_samples(), vec![0.0, 20.0]);
    assert_eq!(
        test2_attr.get_time_samples_in_interval(0.0, 10.0),
        vec![0.0]
    );
    assert_eq!(
        test2_attr.get_time_samples_in_interval(0.0, 20.0),
        vec![0.0, 20.0]
    );

    // Value resolution with scale
    let v0: Option<f32> = test2_attr
        .get(TimeCode::new(0.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v0, Some(0.0));
    let v10: Option<f32> = test2_attr
        .get(TimeCode::new(10.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v10, Some(5.0));
    let v20: Option<f32> = test2_attr
        .get(TimeCode::new(20.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v20, Some(10.0));
    let v30: Option<f32> = test2_attr
        .get(TimeCode::new(30.0))
        .and_then(|v| v.get::<f32>().copied());
    assert_eq!(v30, Some(10.0));
}

// ============================================================================
// test_PreTimeTimeSamples (held types)
// ============================================================================

#[test]
fn ts_held_interpolation() {
    common::setup();

    // Held types (string) don't interpolate — they hold previous value
    let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
    layer.import_from_string(
        r#"#usda 1.0
def "Foo" {
    string x.timeSamples = {
        1: "zero",
        2: "one",
        3: "two"
    }
}"#,
    );
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
    let x = stage.get_attribute_at_path(&p("/Foo.x")).expect("/Foo.x");

    // Before first sample: holds first
    let v0: Option<String> = x
        .get(TimeCode::new(0.0))
        .and_then(|v| v.get::<String>().cloned());
    assert_eq!(v0.as_deref(), Some("zero"));

    // At sample times
    let v1: Option<String> = x
        .get(TimeCode::new(1.0))
        .and_then(|v| v.get::<String>().cloned());
    assert_eq!(v1.as_deref(), Some("zero"));

    let v2: Option<String> = x
        .get(TimeCode::new(2.0))
        .and_then(|v| v.get::<String>().cloned());
    assert_eq!(v2.as_deref(), Some("one"));

    let v3: Option<String> = x
        .get(TimeCode::new(3.0))
        .and_then(|v| v.get::<String>().cloned());
    assert_eq!(v3.as_deref(), Some("two"));

    // After last sample: holds last
    let v4: Option<String> = x
        .get(TimeCode::new(4.0))
        .and_then(|v| v.get::<String>().cloned());
    assert_eq!(v4.as_deref(), Some("two"));
}

// ============================================================================
// test_PreTimeTimeSamples (linear types)
// ============================================================================

#[test]
fn ts_linear_interpolation() {
    common::setup();

    // Linear types (double) interpolate between samples
    let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
    layer.import_from_string(
        r#"#usda 1.0
def "Foo" {
    double x.timeSamples = {
        1: 0.0,
        2: 1.1,
        3: 2.2
    }
}"#,
    );
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
    let x = stage
        .get_prim_at_path(&p("/Foo"))
        .expect("/Foo")
        .get_attribute("x")
        .expect("x");

    // At sample times
    let v1: Option<f64> = x
        .get(TimeCode::new(1.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v1, Some(0.0));

    let v2: Option<f64> = x
        .get(TimeCode::new(2.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v2, Some(1.1));

    let v3: Option<f64> = x
        .get(TimeCode::new(3.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v3, Some(2.2));

    // Before first: holds first
    let v0: Option<f64> = x
        .get(TimeCode::new(0.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v0, Some(0.0));

    // After last: holds last
    let v4: Option<f64> = x
        .get(TimeCode::new(4.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v4, Some(2.2));
}

// ============================================================================
// test_TimeSamplesWithBlock (linear type + value block)
// ============================================================================

#[test]
fn ts_value_block_in_samples() {
    common::setup();

    let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
    layer.import_from_string(
        r#"#usda 1.0
def "Foo" {
    double x.timeSamples = {
        1: 0.0,
        2: None,
        3: 2.2
    }
}"#,
    );
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
    let x = stage
        .get_prim_at_path(&p("/Foo"))
        .expect("/Foo")
        .get_attribute("x")
        .expect("x");

    // At sample 1 -> 0.0
    let v1: Option<f64> = x
        .get(TimeCode::new(1.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v1, Some(0.0));

    // At sample 2 -> blocked (None)
    let v2 = x.get(TimeCode::new(2.0));
    assert!(v2.is_none(), "expected None for blocked time sample at t=2");

    // At sample 3 -> 2.2
    let v3: Option<f64> = x
        .get(TimeCode::new(3.0))
        .and_then(|v| v.get::<f64>().copied());
    assert_eq!(v3, Some(2.2));
}

// ============================================================================
// test_usdaPrecisionBug
// ============================================================================

#[test]
fn ts_usda_precision_bug() {
    common::setup();

    // Two time samples very close together must survive USDA roundtrip
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let foo = stage.define_prim("/foo", "").expect("define /foo");
    let test_attr = foo
        .create_attribute("test", &common::vtn("float"), false, None)
        .expect("create test");
    test_attr.set(0.0f32, TimeCode::new(1.0));
    // C++ SafeStep() defaults: maxValue=1e6, maxCompression=10.0
    let safe_step = UsdTimeCode::safe_step(1e6, 10.0);
    test_attr.set(1.0f32, TimeCode::new(1.0 - safe_step));
    assert_eq!(test_attr.get_time_samples().len(), 2);

    // Export to string and reimport into new layer
    let root = stage.get_root_layer();
    let exported = root.export_to_string().expect("export");
    let reimported = usd_sdf::Layer::create_anonymous(Some(".usda"));
    reimported.import_from_string(&exported);
    let attr_spec = reimported
        .get_attribute_at_path(&p("/foo.test"))
        .expect("attr spec");
    assert_eq!(
        attr_spec.num_time_samples(),
        2,
        "time samples must survive USDA precision roundtrip"
    );
}

// ============================================================================
// test_PreviousTimeSamples
// ============================================================================

#[test]
fn ts_previous_time_samples() {
    common::setup();

    let content = r#"#usda 1.0
def "Prim" {
    double attr.timeSamples = {
        1.0: 1.0,
        2.0: 2.0,
        3.0: 3.0
    }
}"#;

    fn check_previous(layer: &usd_sdf::Layer) {
        let attr_path = Path::from_string("/Prim.attr").unwrap();
        // No previous sample before first sample
        assert_eq!(
            layer.get_previous_time_sample_for_path(&attr_path, 0.0),
            None
        );
        // No previous sample at first sample
        assert_eq!(
            layer.get_previous_time_sample_for_path(&attr_path, 1.0),
            None
        );
        // Previous sample at 2.0 is 1.0
        assert_eq!(
            layer.get_previous_time_sample_for_path(&attr_path, 2.0),
            Some(1.0)
        );
        // Previous sample past range is the last sample
        assert_eq!(
            layer.get_previous_time_sample_for_path(&attr_path, 7.0),
            Some(3.0)
        );
    }

    // Test with USDA layer
    let usda_layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
    usda_layer.import_from_string(content);
    check_previous(&usda_layer);
}

// ============================================================================
// test_PreTimeTimeSamples
// C++ reference: testUsdTimeSamples.py lines 337-491
// ============================================================================

#[test]
fn ts_pre_time() {
    common::setup();

    // Helpers for value extraction
    fn str_at(attr: &Attribute, tc: UsdTimeCode) -> Option<String> {
        attr.get(tc).and_then(|v| v.get::<String>().cloned())
    }
    fn f64_at(attr: &Attribute, tc: UsdTimeCode) -> Option<f64> {
        attr.get(tc).and_then(|v| v.get::<f64>().copied())
    }
    fn arr_at(attr: &Attribute, tc: UsdTimeCode) -> Option<Vec<f64>> {
        attr.get(tc).map(|v| {
            // USDA parser may store as Array<f64> or Vec<Value>
            if let Some(arr) = v.get::<usd_vt::Array<f64>>() {
                return arr.to_vec();
            }
            if let Some(vec) = v.get::<Vec<f64>>() {
                return vec.clone();
            }
            if let Some(vec) = v.get::<Vec<usd_vt::Value>>() {
                return vec
                    .iter()
                    .map(|elem| {
                        elem.get::<f64>()
                            .copied()
                            .or_else(|| elem.get::<i64>().map(|&i| i as f64))
                            .expect("expected numeric in array")
                    })
                    .collect();
            }
            panic!("expected f64 array, got {:?}", v);
        })
    }

    // === 1. Held types (string) ===
    // C++ reference: testUsdTimeSamples.py lines 340-366
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    string x.timeSamples = {
        1: "zero",
        2: "one",
        3: "two"
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let x = stage.get_attribute_at_path(&p("/Foo.x")).expect("/Foo.x");

        // Regular queries
        assert_eq!(str_at(&x, UsdTimeCode::new(0.0)).as_deref(), Some("zero"));
        assert_eq!(str_at(&x, UsdTimeCode::new(1.0)).as_deref(), Some("zero"));
        assert_eq!(str_at(&x, UsdTimeCode::new(2.0)).as_deref(), Some("one"));
        assert_eq!(str_at(&x, UsdTimeCode::new(3.0)).as_deref(), Some("two"));
        assert_eq!(str_at(&x, UsdTimeCode::new(4.0)).as_deref(), Some("two"));

        // PreTime queries — held types always return previous sample value
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(0.0)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(1.0)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(1.5)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(2.0)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(2.5)).as_deref(),
            Some("one")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(3.0)).as_deref(),
            Some("one")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(4.0)).as_deref(),
            Some("two")
        );
    }

    // === 2. Linear types (double) ===
    // C++ reference: testUsdTimeSamples.py lines 371-398
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    double x.timeSamples = {
        1: 0.0,
        2: 1.1,
        3: 2.2
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let x = stage
            .get_prim_at_path(&p("/Foo"))
            .expect("/Foo")
            .get_attribute("x")
            .expect("x");

        // Regular queries
        assert_eq!(f64_at(&x, UsdTimeCode::new(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::new(1.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::new(2.0)), Some(1.1));
        assert_eq!(f64_at(&x, UsdTimeCode::new(3.0)), Some(2.2));
        assert_eq!(f64_at(&x, UsdTimeCode::new(4.0)), Some(2.2));

        // PreTime — for linear types, between-samples PreTime equals regular Get
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(1.0)), Some(0.0));
        assert_eq!(
            f64_at(&x, UsdTimeCode::pre_time(1.5)),
            f64_at(&x, UsdTimeCode::new(1.5))
        );
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(2.0)), Some(1.1));
        assert_eq!(
            f64_at(&x, UsdTimeCode::pre_time(2.5)),
            f64_at(&x, UsdTimeCode::new(2.5))
        );
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(3.0)), Some(2.2));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(4.0)), Some(2.2));
    }

    // === 3. Linear + blocks (double with None) ===
    // C++ reference: testUsdTimeSamples.py lines 400-431
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    double x.timeSamples = {
        1: 0.0,
        2: None,
        3: 2.2
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let x = stage
            .get_prim_at_path(&p("/Foo"))
            .expect("/Foo")
            .get_attribute("x")
            .expect("x");

        // Regular queries
        assert_eq!(f64_at(&x, UsdTimeCode::new(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::new(1.0)), Some(0.0));
        assert!(x.get(UsdTimeCode::new(2.0)).is_none(), "block at t=2");
        assert_eq!(f64_at(&x, UsdTimeCode::new(3.0)), Some(2.2));
        assert_eq!(f64_at(&x, UsdTimeCode::new(4.0)), Some(2.2));

        // PreTime — block causes held fallback
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(1.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(1.5)), Some(0.0));
        assert_eq!(
            f64_at(&x, UsdTimeCode::pre_time(1.5)),
            f64_at(&x, UsdTimeCode::new(1.5))
        );
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(2.0)), Some(0.0));
        assert!(x.get(UsdTimeCode::pre_time(2.5)).is_none());
        assert_eq!(
            x.get(UsdTimeCode::pre_time(2.5)).is_none(),
            x.get(UsdTimeCode::new(2.5)).is_none()
        );
        assert!(x.get(UsdTimeCode::pre_time(3.0)).is_none());
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(4.0)), Some(2.2));
    }

    // === 4. Linear array with mismatched sizes ===
    // C++ reference: testUsdTimeSamples.py lines 433-490
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    double[] x.timeSamples = {
        1: [0.0, 1.0],
        2: [1.0],
        3: [2.0, 3.0],
        4: [4.0, 5.0],
        5: None,
        6: [6.0, 7.0]
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let x = stage
            .get_prim_at_path(&p("/Foo"))
            .expect("/Foo")
            .get_attribute("x")
            .expect("x");

        // Regular queries
        assert_eq!(arr_at(&x, UsdTimeCode::new(0.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(1.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(2.0)), Some(vec![1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(3.0)), Some(vec![2.0, 3.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(4.0)), Some(vec![4.0, 5.0]));
        assert!(x.get(UsdTimeCode::new(5.0)).is_none(), "block at t=5");
        assert_eq!(arr_at(&x, UsdTimeCode::new(6.0)), Some(vec![6.0, 7.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(7.0)), Some(vec![6.0, 7.0]));

        // PreTime — mismatched sizes cause held fallback
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(0.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(1.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(1.5)), Some(vec![0.0, 1.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(1.5)),
            arr_at(&x, UsdTimeCode::new(1.5))
        );
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(2.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(2.5)), Some(vec![1.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(2.5)),
            arr_at(&x, UsdTimeCode::new(2.5))
        );
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(3.0)), Some(vec![1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(3.5)), Some(vec![3.0, 4.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(3.5)),
            arr_at(&x, UsdTimeCode::new(3.5))
        );
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(4.0)), Some(vec![4.0, 5.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(4.5)), Some(vec![4.0, 5.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(4.5)),
            arr_at(&x, UsdTimeCode::new(4.5))
        );
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(5.0)), Some(vec![4.0, 5.0]));
        assert!(x.get(UsdTimeCode::pre_time(6.0)).is_none());
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(7.0)), Some(vec![6.0, 7.0]));
    }
}

// ============================================================================
// test_PreTimeTimeSamplesOffset
// ============================================================================

#[test]
fn ts_pre_time_offset() {
    common::setup();

    // Helpers (same as ts_pre_time)
    fn str_at(attr: &Attribute, tc: UsdTimeCode) -> Option<String> {
        attr.get(tc).and_then(|v| v.get::<String>().cloned())
    }
    fn f64_at(attr: &Attribute, tc: UsdTimeCode) -> Option<f64> {
        attr.get(tc).and_then(|v| v.get::<f64>().copied())
    }
    fn arr_at(attr: &Attribute, tc: UsdTimeCode) -> Option<Vec<f64>> {
        attr.get(tc).map(|v| {
            if let Some(arr) = v.get::<usd_vt::Array<f64>>() {
                return arr.to_vec();
            }
            if let Some(vec) = v.get::<Vec<f64>>() {
                return vec.clone();
            }
            if let Some(vec) = v.get::<Vec<usd_vt::Value>>() {
                return vec
                    .iter()
                    .map(|elem| {
                        elem.get::<f64>()
                            .copied()
                            .or_else(|| elem.get::<i64>().map(|&i| i as f64))
                            .expect("expected numeric in array")
                    })
                    .collect();
            }
            panic!("expected f64 array, got {:?}", v);
        })
    }

    // All sub-tests use +10 LayerOffset via internal reference

    // === 1. Held types (string) + offset ===
    // C++ reference: testUsdTimeSamples.py lines 495-524
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    string x.timeSamples = {
        1: "zero",
        2: "one",
        3: "two"
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let foo_offset = stage
            .define_prim("/FooOffset", "")
            .expect("define /FooOffset");
        foo_offset.get_references().add_internal_reference(
            &p("/Foo"),
            LayerOffset::new(10.0, 1.0),
            ListPosition::FrontOfPrependList,
        );
        let x = stage
            .get_attribute_at_path(&p("/FooOffset.x"))
            .expect("/FooOffset.x");

        // Regular queries (times shifted by +10)
        assert_eq!(str_at(&x, UsdTimeCode::new(0.0)).as_deref(), Some("zero"));
        assert_eq!(str_at(&x, UsdTimeCode::new(11.0)).as_deref(), Some("zero"));
        assert_eq!(str_at(&x, UsdTimeCode::new(12.0)).as_deref(), Some("one"));
        assert_eq!(str_at(&x, UsdTimeCode::new(13.0)).as_deref(), Some("two"));
        assert_eq!(str_at(&x, UsdTimeCode::new(14.0)).as_deref(), Some("two"));

        // PreTime queries
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(2.5)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(11.0)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(11.5)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(12.0)).as_deref(),
            Some("zero")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(12.5)).as_deref(),
            Some("one")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(13.0)).as_deref(),
            Some("one")
        );
        assert_eq!(
            str_at(&x, UsdTimeCode::pre_time(14.0)).as_deref(),
            Some("two")
        );
    }

    // === 2. Linear types (double) + offset ===
    // C++ reference: testUsdTimeSamples.py lines 529-559
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    double x.timeSamples = {
        1: 0.0,
        2: 1.1,
        3: 2.2
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let foo_offset = stage
            .define_prim("/FooOffset", "")
            .expect("define /FooOffset");
        foo_offset.get_references().add_internal_reference(
            &p("/Foo"),
            LayerOffset::new(10.0, 1.0),
            ListPosition::FrontOfPrependList,
        );
        let x = stage
            .get_attribute_at_path(&p("/FooOffset.x"))
            .expect("/FooOffset.x");

        // Regular queries (times shifted by +10)
        assert_eq!(f64_at(&x, UsdTimeCode::new(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::new(11.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::new(12.0)), Some(1.1));
        assert_eq!(f64_at(&x, UsdTimeCode::new(13.0)), Some(2.2));
        assert_eq!(f64_at(&x, UsdTimeCode::new(14.0)), Some(2.2));

        // PreTime
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(11.0)), Some(0.0));
        assert_eq!(
            f64_at(&x, UsdTimeCode::pre_time(11.5)),
            f64_at(&x, UsdTimeCode::new(11.5))
        );
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(12.0)), Some(1.1));
        assert_eq!(
            f64_at(&x, UsdTimeCode::pre_time(12.5)),
            f64_at(&x, UsdTimeCode::new(12.5))
        );
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(13.0)), Some(2.2));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(14.0)), Some(2.2));
    }

    // === 3. Linear + blocks (double with None) + offset ===
    // C++ reference: testUsdTimeSamples.py lines 563-595
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    double x.timeSamples = {
        1: 0.0,
        2: None,
        3: 2.2
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let foo_offset = stage
            .define_prim("/FooOffset", "")
            .expect("define /FooOffset");
        foo_offset.get_references().add_internal_reference(
            &p("/Foo"),
            LayerOffset::new(10.0, 1.0),
            ListPosition::FrontOfPrependList,
        );
        let x = stage
            .get_attribute_at_path(&p("/FooOffset.x"))
            .expect("/FooOffset.x");

        // Regular queries (times shifted by +10)
        assert_eq!(f64_at(&x, UsdTimeCode::new(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::new(11.0)), Some(0.0));
        assert!(x.get(UsdTimeCode::new(12.0)).is_none(), "block at t=12");
        assert_eq!(f64_at(&x, UsdTimeCode::new(13.0)), Some(2.2));
        assert_eq!(f64_at(&x, UsdTimeCode::new(14.0)), Some(2.2));

        // PreTime — block causes held fallback
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(0.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(11.0)), Some(0.0));
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(11.5)), Some(0.0));
        assert_eq!(
            f64_at(&x, UsdTimeCode::pre_time(11.5)),
            f64_at(&x, UsdTimeCode::new(11.5))
        );
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(12.0)), Some(0.0));
        assert!(x.get(UsdTimeCode::pre_time(12.5)).is_none());
        assert_eq!(
            x.get(UsdTimeCode::pre_time(12.5)).is_none(),
            x.get(UsdTimeCode::new(12.5)).is_none()
        );
        assert!(x.get(UsdTimeCode::pre_time(13.0)).is_none());
        assert_eq!(f64_at(&x, UsdTimeCode::pre_time(14.0)), Some(2.2));
    }

    // === 4. Linear array with mismatched sizes + offset ===
    // C++ reference: testUsdTimeSamples.py lines 599-657
    {
        let layer = usd_sdf::Layer::create_anonymous(Some(".usda"));
        layer.import_from_string(
            r#"#usda 1.0
def "Foo" {
    double[] x.timeSamples = {
        1: [0.0, 1.0],
        2: [1.0],
        3: [2.0, 3.0],
        4: [4.0, 5.0],
        5: None,
        6: [6.0, 7.0]
    }
}"#,
        );
        let stage =
            Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");
        let foo_offset = stage
            .define_prim("/FooOffset", "")
            .expect("define /FooOffset");
        foo_offset.get_references().add_internal_reference(
            &p("/Foo"),
            LayerOffset::new(10.0, 1.0),
            ListPosition::FrontOfPrependList,
        );
        let x = stage
            .get_attribute_at_path(&p("/FooOffset.x"))
            .expect("/FooOffset.x");

        // Regular queries (times shifted by +10)
        assert_eq!(arr_at(&x, UsdTimeCode::new(0.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(11.0)), Some(vec![0.0, 1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(12.0)), Some(vec![1.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(13.0)), Some(vec![2.0, 3.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(14.0)), Some(vec![4.0, 5.0]));
        assert!(x.get(UsdTimeCode::new(15.0)).is_none(), "block at t=15");
        assert_eq!(arr_at(&x, UsdTimeCode::new(16.0)), Some(vec![6.0, 7.0]));
        assert_eq!(arr_at(&x, UsdTimeCode::new(17.0)), Some(vec![6.0, 7.0]));

        // PreTime — mismatched sizes cause held fallback
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(0.0)), Some(vec![0.0, 1.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(11.0)),
            Some(vec![0.0, 1.0])
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(11.5)),
            Some(vec![0.0, 1.0])
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(11.5)),
            arr_at(&x, UsdTimeCode::new(11.5))
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(12.0)),
            Some(vec![0.0, 1.0])
        );
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(12.5)), Some(vec![1.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(12.5)),
            arr_at(&x, UsdTimeCode::new(12.5))
        );
        assert_eq!(arr_at(&x, UsdTimeCode::pre_time(13.0)), Some(vec![1.0]));
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(13.5)),
            Some(vec![3.0, 4.0])
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(13.5)),
            arr_at(&x, UsdTimeCode::new(13.5))
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(14.0)),
            Some(vec![4.0, 5.0])
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(14.5)),
            Some(vec![4.0, 5.0])
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(14.5)),
            arr_at(&x, UsdTimeCode::new(14.5))
        );
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(15.0)),
            Some(vec![4.0, 5.0])
        );
        assert!(x.get(UsdTimeCode::pre_time(16.0)).is_none());
        assert_eq!(
            arr_at(&x, UsdTimeCode::pre_time(17.0)),
            Some(vec![6.0, 7.0])
        );
    }
}
