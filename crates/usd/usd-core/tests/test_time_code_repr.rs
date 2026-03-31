//! Port of testUsdTimeCodeRepr.py from OpenUSD pxr/usd/usd/testenv/
//! 6 tests: Display/FromStr roundtrip for Default, Earliest, numeric, PreTime.

mod common;

use std::str::FromStr;
use usd_core::TimeCode;

// ============================================================================
// 1. testDefaultTimeRepr
// ============================================================================

#[test]
fn time_code_repr_default() {
    common::setup();

    let default_time = TimeCode::default_time();
    let repr = format!("{}", default_time);
    assert_eq!(repr, "DEFAULT");

    let parsed = TimeCode::from_str(&repr).expect("parse DEFAULT");
    assert!(parsed.is_default());
}

// ============================================================================
// 2. testEarliestTimeRepr
// ============================================================================

#[test]
fn time_code_repr_earliest() {
    common::setup();

    let earliest = TimeCode::earliest_time();
    let repr = format!("{}", earliest);
    assert_eq!(repr, "EARLIEST");

    let parsed = TimeCode::from_str(&repr).expect("parse EARLIEST");
    assert!(parsed.is_earliest_time());
}

// ============================================================================
// 3. testDefaultConstructedTimeRepr
// ============================================================================

#[test]
fn time_code_repr_default_constructed() {
    common::setup();

    // Default-constructed UsdTimeCode is Default sentinel
    let time_code = TimeCode::default();
    let repr = format!("{}", time_code);
    assert_eq!(repr, "DEFAULT");

    let parsed = TimeCode::from_str(&repr).expect("parse default-constructed");
    assert!(parsed.is_default());

    // UsdTimeCode from SdfTimeCode(default) should also be Default
    // NOTE: In our impl SdfTimeCode::default() = NaN (unlike C++ where it's 0.0)
    // so from_sdf_time_code(NaN) produces Default, matching Python repr test
    let sdf_tc = usd_sdf::TimeCode::default();
    let time_code = TimeCode::from_sdf_time_code(&sdf_tc);
    assert!(time_code.is_default());
}

// ============================================================================
// 4. testNumericTimeRepr
// ============================================================================

#[test]
fn time_code_repr_numeric() {
    common::setup();

    let time_code = TimeCode::new(123.0);
    let repr = format!("{}", time_code);
    assert_eq!(repr, "123");

    let parsed = TimeCode::from_str(&repr).expect("parse numeric");
    assert!(parsed.is_numeric());
    assert_eq!(parsed.value(), 123.0);

    // From SdfTimeCode
    let sdf_tc = usd_sdf::TimeCode::new(12.0);
    let time_code = TimeCode::from_sdf_time_code(&sdf_tc);
    let repr = format!("{}", time_code);
    assert_eq!(repr, "12");

    let parsed = TimeCode::from_str(&repr).expect("parse from sdf");
    assert!(parsed.is_numeric());
    assert_eq!(parsed.value(), 12.0);
}

// ============================================================================
// 5. testPreTimeRepr
// ============================================================================

#[test]
fn time_code_repr_pre_time() {
    common::setup();

    let time_code = TimeCode::pre_time(123.0);
    let repr = format!("{}", time_code);
    assert_eq!(repr, "PRE_TIME 123");

    let parsed = TimeCode::from_str(&repr).expect("parse pre_time");
    assert!(parsed.is_pre_time());
    assert_eq!(parsed.value(), 123.0);

    // From SdfTimeCode
    let sdf_tc = usd_sdf::TimeCode::new(12.0);
    let time_code = TimeCode::pre_time_from_sdf(&sdf_tc);
    let repr = format!("{}", time_code);
    assert_eq!(repr, "PRE_TIME 12");

    let parsed = TimeCode::from_str(&repr).expect("parse pre_time from sdf");
    assert!(parsed.is_pre_time());
    assert_eq!(parsed.value(), 12.0);
}

// ============================================================================
// 6. testPreTimeEarliestRepr
// ============================================================================

#[test]
fn time_code_repr_pre_time_earliest() {
    common::setup();

    let time_code = TimeCode::pre_time(TimeCode::earliest_time().value());
    let repr = format!("{}", time_code);
    // PreTime + earliest value should display as "PRE_TIME EARLIEST"
    assert_eq!(repr, "PRE_TIME EARLIEST");

    let parsed = TimeCode::from_str(&repr).expect("parse pre_time earliest");
    assert!(parsed.is_pre_time());
    assert!(parsed.value() == f64::MIN);
}
