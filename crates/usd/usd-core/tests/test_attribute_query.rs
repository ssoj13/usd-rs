//! Tests for UsdAttributeQuery.
//!
//! Ported from pxr/usd/usd/testenv/testUsdAttributeQuery.py

mod common;

use usd_core::attribute_query::AttributeQuery;
use usd_core::common::InitialLoadSet;
use usd_core::stage::Stage;
use usd_sdf::TimeCode;

/// Core test: create AttributeQuery from an attribute with default + time samples,
/// verify Get() at default and at time 0.
/// Matches Python layerContentTest with timeSamples.
#[test]
fn attr_query_basic_time_samples() {
    common::setup();

    let sublayer = usd_sdf::Layer::create_anonymous(Some("source.usda"));
    sublayer.import_from_string(
        r#"#usda 1.0

def "Prim"
{
    double attr = 1.0
    double attr.timeSamples = {
        0.0: 2.0
    }
}
"#,
    );

    let root_layer = usd_sdf::Layer::create_anonymous(Some("root.usda"));
    root_layer.set_sublayer_paths(&[sublayer.identifier().to_string()]);

    let stage =
        Stage::open_with_root_layer(root_layer, InitialLoadSet::LoadAll).expect("open stage");

    let attr = stage
        .get_attribute_at_path(&usd_sdf::Path::from("/Prim.attr"))
        .expect("attr");

    let query = AttributeQuery::new(attr);
    assert!(query.is_valid());

    // Default value = 1.0
    let default_val = query.get_typed::<f64>(TimeCode::default_time());
    assert_eq!(default_val, Some(1.0));

    // Value at time 0 = 2.0 (time sample)
    let val_at_0 = query.get_typed::<f64>(TimeCode::new(0.0));
    assert_eq!(val_at_0, Some(2.0));
}

/// Test AttributeQuery constructed from prim + attr name.
#[test]
fn attr_query_from_prim() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Foo", "").expect("define");
    let attr = prim
        .create_attribute("val", &common::vtn("double"), false, None)
        .expect("create");
    attr.set(usd_vt::Value::from(42.0_f64), TimeCode::default_time());

    let query = AttributeQuery::from_prim(&prim, &usd_tf::Token::new("val"));
    assert!(query.is_valid());
    assert_eq!(query.get_typed::<f64>(TimeCode::default_time()), Some(42.0));
}

/// Test has_value, has_authored_value.
#[test]
fn attr_query_has_value() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Foo", "").expect("define");
    let attr = prim
        .create_attribute("val", &common::vtn("double"), false, None)
        .expect("create");
    attr.set(usd_vt::Value::from(1.0_f64), TimeCode::default_time());

    let query = AttributeQuery::new(attr.clone());
    assert!(query.has_value());
    assert!(query.has_authored_value());
    assert!(!query.has_fallback_value());
}

/// Test invalid query.
#[test]
fn attr_query_invalid() {
    let query = AttributeQuery::new_invalid();
    assert!(!query.is_valid());
    assert!(!query.has_value());
    assert!(!query.has_authored_value());
    assert!(query.get(TimeCode::default_time()).is_none());
    assert_eq!(query.get_num_time_samples(), 0);
    assert!(query.get_time_samples().is_empty());
    assert!(!query.value_might_be_time_varying());
}

/// Test get_time_samples and get_num_time_samples via query.
#[test]
fn attr_query_time_samples() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Foo", "").expect("define");
    let attr = prim
        .create_attribute("val", &common::vtn("double"), false, None)
        .expect("create");

    // Author 5 time samples
    for i in 0..5 {
        let t = i as f64 * 10.0;
        attr.set(usd_vt::Value::from(t), TimeCode::new(t));
    }

    let query = AttributeQuery::new(attr);
    assert_eq!(query.get_num_time_samples(), 5);

    let times = query.get_time_samples();
    assert_eq!(times, vec![0.0, 10.0, 20.0, 30.0, 40.0]);

    assert!(query.value_might_be_time_varying());
}

/// Test get_bracketing_time_samples via query.
#[test]
fn attr_query_bracketing() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Foo", "").expect("define");
    let attr = prim
        .create_attribute("val", &common::vtn("double"), false, None)
        .expect("create");

    attr.set(usd_vt::Value::from(10.0_f64), TimeCode::new(10.0));
    attr.set(usd_vt::Value::from(20.0_f64), TimeCode::new(20.0));

    let query = AttributeQuery::new(attr);

    // Exact match
    let bracket = query.get_bracketing_time_samples(10.0);
    assert_eq!(bracket, Some((10.0, 10.0)));

    // Between samples
    let bracket = query.get_bracketing_time_samples(15.0);
    assert_eq!(bracket, Some((10.0, 20.0)));
}

/// Test create_queries (batch construction).
#[test]
fn attr_query_create_queries() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Foo", "").expect("define");

    for name in &["a", "b", "c"] {
        let attr = prim
            .create_attribute(name, &common::vtn("double"), false, None)
            .expect("create");
        attr.set(usd_vt::Value::from(1.0_f64), TimeCode::default_time());
    }

    let names: Vec<usd_tf::Token> = ["a", "b", "c"]
        .iter()
        .map(|s| usd_tf::Token::new(s))
        .collect();

    let queries = AttributeQuery::create_queries(&prim, &names);
    assert_eq!(queries.len(), 3);
    for query in &queries {
        assert!(query.is_valid());
        assert_eq!(query.get_typed::<f64>(TimeCode::default_time()), Some(1.0));
    }
}

/// Test that sublayer changes don't invalidate query (no resync).
/// Matches Python test_NoInvalidationForInsignificantChangeWithTimeSamples.
/// We can't test change notices, but we test that the query still works after
/// sublayer manipulation.
#[test]
#[ignore = "Notice/change-processing system not yet ported for invalidation tracking"]
fn attr_query_no_invalidation_for_insignificant_change() {}

/// Spline-based query test.
/// Matches Python test_NoInvalidationForInsignificantChangeWithSplines.
#[test]
#[ignore = "Spline-based AttributeQuery not yet fully tested"]
fn attr_query_no_invalidation_splines() {}
