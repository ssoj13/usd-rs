//! Tests for UsdAttributeLimits API.
//! Ported from C++ testUsdAttributeLimits.cpp and testUsdAttributeLimits.py.

mod common;

use usd_core::Stage;
use usd_core::attribute_limits::{AttributeLimits, limits_keys};
use usd_core::common::InitialLoadSet;
use usd_sdf::Layer;
use usd_tf::Token;
use usd_vt::{Dictionary, Value};

// ============================================================================
// Helpers
// ============================================================================

fn make_stage() -> std::sync::Arc<Stage> {
    common::setup();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage")
}

fn make_stage_from_usda(usda: &str) -> std::sync::Arc<Stage> {
    common::setup();
    let layer = Layer::create_anonymous(Some("attribute_limits.usda"));
    assert!(layer.import_from_string(usda), "import_from_string failed");
    Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage")
}

fn get_prim(stage: &Stage, path: &str) -> usd_core::Prim {
    stage
        .get_prim_at_path(&usd_sdf::Path::from_string(path).unwrap())
        .expect("prim exists")
}

fn sdf_int() -> usd_sdf::ValueTypeName {
    usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("int")
}

#[allow(dead_code)]
fn sdf_double() -> usd_sdf::ValueTypeName {
    usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("double")
}

// ============================================================================
// Section 1: TestBasicUsage — from C++ testUsdAttributeLimits.cpp
// ============================================================================

/// C++ TestBasicUsage: empty limits, set/get/GetOr, for hard/soft/custom.
#[test]
fn test_basic_usage() {
    let stage = make_stage();
    let prim = stage.define_prim("/TestBasicUsage", "").unwrap();
    let attr = prim
        .create_attribute("attr", &sdf_int(), true, None)
        .expect("create attr");

    let hard = attr.get_hard_limits();
    let soft = attr.get_soft_limits();
    let custom = attr.get_limits_for_subdict(&Token::new("customSubDict"));

    for limits in [&hard, &soft, &custom] {
        // Limits dict is empty, simple getters should return empty
        assert!(limits.get(&limits_keys::minimum()).is_none());
        assert!(limits.get(&limits_keys::maximum()).is_none());
        assert!(limits.get(&Token::new("customKey")).is_none());

        // "Or" getters should return the passed-in defaults
        assert_eq!(limits.get_or::<i32>(&limits_keys::minimum(), 3), 3);
        assert_eq!(limits.get_or::<i32>(&limits_keys::maximum(), 7), 7);
        assert_eq!(
            limits.get_or::<String>(&Token::new("customKey"), "foo".to_string()),
            "foo"
        );

        // Set and confirm values
        assert!(limits.set_minimum(Value::from(5i32)));
        assert!(limits.set_maximum(Value::from(10i32)));
        assert!(limits.set(&Token::new("customKey"), Value::from("custom".to_string())));

        assert_eq!(limits.get_minimum_or::<i32>(3), 5);
        assert_eq!(limits.get_maximum_or::<i32>(7), 10);
        assert_eq!(
            limits.get_or::<String>(&Token::new("customKey"), String::new()),
            "custom"
        );

        // Get typed min/max
        let min = limits.get_minimum().and_then(|v| v.get::<i32>().copied());
        assert_eq!(min, Some(5));

        let max = limits.get_maximum().and_then(|v| v.get::<i32>().copied());
        assert_eq!(max, Some(10));

        let custom_val = limits
            .get(&Token::new("customKey"))
            .and_then(|v| v.get::<String>().cloned());
        assert_eq!(custom_val, Some("custom".to_string()));
    }
}

// ============================================================================
// Section 2: TestWrongTypes — from C++ testUsdAttributeLimits.cpp
// ============================================================================

/// C++ TestWrongTypes: set min/max with wrong type should fail.
#[test]
fn test_wrong_types() {
    let stage = make_stage_from_usda(
        r#"#usda 1.0

def "TestWrongTypes"
{
    custom int attr = 7 (
        limits = {
            dictionary soft = {
                int minimum = 5
                int maximum = 10
                string customKey = "bleep"
            }
        }
    )

    custom double badLimits = 7.0 (
        limits = {
            dictionary hard = {
                int minimumValue = 5
                string maximumValue = "ten"
            }
        }
    )
}
"#,
    );

    let attr = get_prim(&stage, "/TestWrongTypes")
        .get_attribute("attr")
        .unwrap();
    let soft = attr.get_soft_limits();

    // Setting min/max with a type other than the attribute's value type
    // should fail (attr type is "int", setting double should fail)
    assert!(!soft.set_minimum(Value::from(5.5f64)));
    // Value should be unchanged
    let min = soft.get_minimum().and_then(|v| v.get::<i32>().copied());
    assert_eq!(min, Some(5));
    assert_eq!(soft.get_minimum_or::<i32>(3), 5);

    assert!(!soft.set_maximum(Value::from("foo".to_string())));
    // Value should be unchanged
    let max = soft.get_maximum().and_then(|v| v.get::<i32>().copied());
    assert_eq!(max, Some(10));
    assert_eq!(soft.get_maximum_or::<i32>(7), 10);

    // Getting with the wrong type will return None (not the stored type)
    assert!(
        soft.get_minimum()
            .and_then(|v| v.get::<bool>().copied())
            .is_none()
    );
    assert_eq!(soft.get_minimum_or::<bool>(false), false);

    assert!(
        soft.get_maximum()
            .and_then(|v| v.get::<String>().cloned())
            .is_none()
    );
    // Note: this tests MinimumOr with wrong type, should return default
    assert_eq!(
        soft.get_minimum_or::<String>("str".to_string()),
        "str".to_string()
    );

    // Custom key is "string", get as int should fail
    assert!(
        soft.get(&Token::new("customKey"))
            .and_then(|v| v.get::<i32>().copied())
            .is_none()
    );
    assert_eq!(soft.get_or::<i32>(&Token::new("customKey"), 10), 10);

    // Getting min/max with the right type when the stored value is of the
    // wrong type (badLimits.hard has int minimumValue, not double minimum)
    let bad_attr = get_prim(&stage, "/TestWrongTypes")
        .get_attribute("badLimits")
        .unwrap();
    let bad_hard = bad_attr.get_hard_limits();

    // "minimumValue" is not "minimum", so GetMinimum should be None
    assert!(bad_hard.get_minimum().is_none());
    assert_eq!(bad_hard.get_minimum_or::<f64>(10.5), 10.5);

    assert!(bad_hard.get_maximum().is_none());
    assert_eq!(bad_hard.get_maximum_or::<f64>(15.5), 15.5);
}

// ============================================================================
// Section 3: test_LimitsObject — from Python testUsdAttributeLimits.py
// ============================================================================

/// Python test_LimitsObject: invalid, valid, equality.
#[test]
fn test_limits_object() {
    let stage = make_stage();
    let prim = stage.define_prim("/test_LimitsObject", "").unwrap();
    let attr = prim
        .create_attribute("attr", &sdf_int(), true, None)
        .expect("create attr");

    // Invalid limits
    let invalid = AttributeLimits::invalid();
    assert!(!invalid.is_valid());
    assert!(invalid.sub_dict_key().is_empty());

    // Soft limits
    let soft = attr.get_soft_limits();
    assert!(soft.is_valid());
    assert_eq!(soft.sub_dict_key(), &limits_keys::soft());

    // Hard limits
    let hard = attr.get_hard_limits();
    assert!(hard.is_valid());
    assert_eq!(hard.sub_dict_key(), &limits_keys::hard());

    // Custom limits
    let custom = attr.get_limits_for_subdict(&Token::new("customLimits"));
    assert!(custom.is_valid());
    assert_eq!(custom.sub_dict_key(), &Token::new("customLimits"));

    // Equality ops
    assert_eq!(soft, soft);
    assert_eq!(hard, hard);
    assert_eq!(custom, custom);

    assert_ne!(soft, hard);
    assert_ne!(hard, custom);
    assert_ne!(soft, custom);

    assert_eq!(
        attr.get_limits_for_subdict(&Token::new("foo")),
        attr.get_limits_for_subdict(&Token::new("foo"))
    );
    assert_ne!(
        attr.get_limits_for_subdict(&Token::new("foo")),
        attr.get_limits_for_subdict(&Token::new("bar"))
    );
}

// ============================================================================
// Section 4: test_Opinions — from Python testUsdAttributeLimits.py
// ============================================================================

/// Python test_Opinions: HasAuthored, Clear, HasAuthoredMinimum/Maximum.
#[test]
fn test_opinions() {
    let stage = make_stage_from_usda(
        r#"#usda 1.0

def "test_Opinions"
{
    int attr = 1 (
        limits = {
            dictionary hard = {
                int minimum = 1
                int maximum = 10
                int customInt = 25
            }
            dictionary soft = {
                int maximum = 7
                double customDouble = 10.5
            }
        }
    )
}
"#,
    );

    let attr = get_prim(&stage, "/test_Opinions")
        .get_attribute("attr")
        .unwrap();
    let hard = attr.get_hard_limits();
    let soft = attr.get_soft_limits();

    assert!(attr.has_authored_limits());

    // Verify opinions are present
    assert!(hard.has_authored());
    assert!(hard.has_authored_minimum());
    assert!(hard.has_authored_maximum());
    assert!(hard.has_authored_key(&Token::new("customInt")));
    assert!(!hard.has_authored_key(&Token::new("non-existent")));

    // Clear individual fields and re-check
    assert!(hard.clear_minimum());
    assert!(!hard.has_authored_minimum());

    assert!(hard.clear_maximum());
    assert!(!hard.has_authored_maximum());

    assert!(hard.clear_key(&Token::new("customInt")));
    assert!(!hard.has_authored_key(&Token::new("customInt")));

    // Nothing should be left
    assert!(!hard.has_authored());

    // Clear whole subdict at once
    assert!(soft.has_authored());
    assert!(!soft.has_authored_minimum());
    assert!(soft.has_authored_maximum());
    assert!(soft.has_authored_key(&Token::new("customDouble")));

    assert!(soft.clear());

    assert!(!soft.has_authored());
    assert!(!soft.has_authored_minimum());
    assert!(!soft.has_authored_maximum());
    assert!(!soft.has_authored_key(&Token::new("customDouble")));

    assert!(!attr.has_authored_limits());
}

// ============================================================================
// Section 5: test_BasicUsage from Python — read, modify, replace, clear
// ============================================================================

/// Python test_BasicUsage: loaded values, modify, replace subdicts, clear.
#[test]
fn test_basic_usage_py() {
    let stage = make_stage_from_usda(
        r#"#usda 1.0

def "test_BasicUsage"
{
    int attr = 1 (
        limits = {
            dictionary hard = {
                int minimum = 1
                int maximum = 10
                int customInt = 25
            }
            dictionary soft = {
                int minimum = 3
                int maximum = 7
                double customDouble = 10.5
            }
            dictionary customLimits = {
                int minimum = 20
                int maximum = 30
                int customBool = 0
            }
        }
    )
}
"#,
    );

    let attr = get_prim(&stage, "/test_BasicUsage")
        .get_attribute("attr")
        .unwrap();
    assert!(attr.has_authored_limits());

    // Check hard limits
    let hard = attr.get_hard_limits();
    assert_eq!(
        hard.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(1)
    );
    assert_eq!(
        hard.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(10)
    );
    assert_eq!(
        hard.get(&Token::new("customInt"))
            .and_then(|v| v.get::<i32>().copied()),
        Some(25)
    );
    assert!(hard.get(&Token::new("non-existent")).is_none());
    assert!(hard.get(&Token::new("")).is_none());

    // Check soft limits
    let soft = attr.get_soft_limits();
    assert_eq!(
        soft.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(3)
    );
    assert_eq!(
        soft.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(7)
    );
    assert_eq!(
        soft.get(&Token::new("customDouble"))
            .and_then(|v| v.get::<f64>().copied()),
        Some(10.5)
    );
    assert!(soft.get(&Token::new("non-existent")).is_none());
    assert!(soft.get(&Token::new("")).is_none());

    // Check custom limits
    let custom = attr.get_limits_for_subdict(&Token::new("customLimits"));
    assert_eq!(
        custom.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(20)
    );
    assert_eq!(
        custom.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(30)
    );
    assert!(custom.get(&Token::new("non-existent")).is_none());
    assert!(custom.get(&Token::new("")).is_none());

    // Modify hard limits
    assert!(hard.set_minimum(Value::from(0i32)));
    assert_eq!(
        hard.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(0)
    );

    assert!(hard.set_maximum(Value::from(11i32)));
    assert_eq!(
        hard.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(11)
    );

    assert!(hard.set(&Token::new("customInt"), Value::from(50i32)));
    assert_eq!(
        hard.get(&Token::new("customInt"))
            .and_then(|v| v.get::<i32>().copied()),
        Some(50)
    );

    assert!(hard.set(&Token::new("newValue"), Value::from(100i32)));
    assert_eq!(
        hard.get(&Token::new("newValue"))
            .and_then(|v| v.get::<i32>().copied()),
        Some(100)
    );

    // Modify soft limits
    assert!(soft.set_minimum(Value::from(2i32)));
    assert_eq!(
        soft.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(2)
    );

    assert!(soft.set_maximum(Value::from(8i32)));
    assert_eq!(
        soft.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(8)
    );

    assert!(soft.set(&Token::new("customDouble"), Value::from(12.75f64)));
    assert_eq!(
        soft.get(&Token::new("customDouble"))
            .and_then(|v| v.get::<f64>().copied()),
        Some(12.75)
    );

    // Modify custom limits
    assert!(custom.set_minimum(Value::from(40i32)));
    assert_eq!(
        custom.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(40)
    );

    assert!(custom.set_maximum(Value::from(60i32)));
    assert_eq!(
        custom.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(60)
    );

    // Replace hard subdict
    let mut new_hard = Dictionary::new();
    new_hard.insert("minimum", 120i32);
    new_hard.insert("maximum", 140i32);
    assert!(hard.set_dict(&new_hard));
    assert_eq!(
        hard.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(120)
    );
    assert_eq!(
        hard.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(140)
    );

    // Replace soft subdict
    let mut new_soft = Dictionary::new();
    new_soft.insert("minimum", 130i32);
    new_soft.insert("maximum", 150i32);
    assert!(soft.set_dict(&new_soft));
    assert_eq!(
        soft.get_minimum().and_then(|v| v.get::<i32>().copied()),
        Some(130)
    );
    assert_eq!(
        soft.get_maximum().and_then(|v| v.get::<i32>().copied()),
        Some(150)
    );

    // Clear
    assert!(attr.clear_limits());
    assert!(!attr.has_authored_limits());

    assert!(hard.get_minimum().is_none());
    assert!(hard.get_maximum().is_none());
    assert!(soft.get_minimum().is_none());
    assert!(soft.get_maximum().is_none());
    assert!(custom.get_minimum().is_none());
    assert!(custom.get_maximum().is_none());
}

// ============================================================================
// Section 6: test_Validation — from Python testUsdAttributeLimits.py
// ============================================================================

/// Python test_Validation: validate subdicts with correct/conformable/bad types.
#[test]
fn test_validation() {
    let stage = make_stage();
    let prim = stage.define_prim("/test_Validation", "").unwrap();
    let attr = prim
        .create_attribute("attr", &sdf_int(), true, None)
        .expect("create attr");
    let soft = attr.get_soft_limits();

    // Default-constructed result should be invalid
    let result = usd_core::attribute_limits::ValidationResult::default();
    assert!(!result.success());
    assert!(result.invalid_values_dict().is_empty());
    assert!(result.conformed_sub_dict().is_empty());
    assert!(result.error_string().is_empty());

    // Empty subdict should be valid
    let result = soft.validate(&Dictionary::new());
    assert!(result.success());
    assert!(result.invalid_values_dict().is_empty());
    assert!(result.conformed_sub_dict().is_empty());
    assert!(result.error_string().is_empty());

    // Good values should be valid
    let mut subdict = Dictionary::new();
    subdict.insert("minimum", 5i32);
    subdict.insert("maximum", 10i32);
    subdict.insert("customStr", "foo".to_string());
    let result = soft.validate(&subdict);
    assert!(result.success());
    assert!(result.invalid_values_dict().is_empty());
    assert!(!result.conformed_sub_dict().is_empty());
    assert!(result.error_string().is_empty());

    // Non-conformable value should not be valid
    let mut bad_subdict = Dictionary::new();
    bad_subdict.insert("minimum", 1i32);
    bad_subdict.insert("maximum", "forty-two".to_string());
    bad_subdict.insert("customStr", "foo".to_string());
    let result = soft.validate(&bad_subdict);
    assert!(!result.success());
    assert!(!result.invalid_values_dict().is_empty());
    assert!(result.conformed_sub_dict().is_empty());
    assert!(!result.error_string().is_empty());
}

// ============================================================================
// Section 7: test_SetWrongType — from Python testUsdAttributeLimits.py
// ============================================================================

/// Python test_SetWrongType: min/max type must match or be castable.
#[test]
fn test_set_wrong_type() {
    let stage = make_stage();
    let prim = stage.define_prim("/test_SetWrongType", "").unwrap();
    let attr = prim
        .create_attribute("attr", &sdf_int(), true, None)
        .expect("create attr");
    let hard = attr.get_hard_limits();

    // Min/max types must match the attribute's value type - strings should fail
    assert!(!hard.set_minimum(Value::from("a string".to_string())));
    assert!(!hard.set_maximum(Value::from("a string".to_string())));

    // Whole-subdict with bad min/max
    let mut bad_dict = Dictionary::new();
    bad_dict.insert("minimum", "min".to_string());
    assert!(!hard.set_dict(&bad_dict));

    let mut bad_dict2 = Dictionary::new();
    bad_dict2.insert("minimum", 5i32);
    bad_dict2.insert("maximum", "max".to_string());
    assert!(!hard.set_dict(&bad_dict2));
}

// ============================================================================
// Section 8: test_GetWrongType — from Python testUsdAttributeLimits.py
// ============================================================================

/// Python test_GetWrongType: wrong-typed stored values returned as-is via VtValue API.
#[test]
fn test_get_wrong_type() {
    let stage = make_stage_from_usda(
        r#"#usda 1.0

def "test_GetWrongType"
{
    int attr = 1 (
        limits = {
            dictionary hard = {
                double minimum = 1.5
                bool maximum = 0
            }
            dictionary soft = {
                string minimum = "min"
            }
        }
    )
}
"#,
    );

    let attr = get_prim(&stage, "/test_GetWrongType")
        .get_attribute("attr")
        .unwrap();
    let hard = attr.get_hard_limits();
    let soft = attr.get_soft_limits();

    // The VtValue-based API returns unexpected values as-is
    // Hard minimum is stored as double
    let hard_min = hard.get_minimum();
    assert!(hard_min.is_some());
    assert_eq!(
        hard_min.as_ref().and_then(|v| v.get::<f64>().copied()),
        Some(1.5)
    );

    // Hard maximum is stored as bool (false)
    let hard_max = hard.get_maximum();
    assert!(hard_max.is_some());
    assert_eq!(
        hard_max.as_ref().and_then(|v| v.get::<bool>().copied()),
        Some(false)
    );

    // Soft minimum is stored as string
    let soft_min = soft.get_minimum();
    assert!(soft_min.is_some());
    assert_eq!(
        soft_min.as_ref().and_then(|v| v.get::<String>().cloned()),
        Some("min".to_string())
    );
}

// ============================================================================
// Section 9: Empty key edge case
// ============================================================================

/// Passing empty key to Get should return None, to Set should return false.
#[test]
fn test_empty_key() {
    let stage = make_stage();
    let prim = stage.define_prim("/test_EmptyKey", "").unwrap();
    let attr = prim
        .create_attribute("attr", &sdf_int(), true, None)
        .expect("create attr");
    let hard = attr.get_hard_limits();

    assert!(hard.get(&Token::new("")).is_none());
    assert!(!hard.set(&Token::new(""), Value::from(5i32)));
}
