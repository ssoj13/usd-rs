//! Tests for UsdTimeCode.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdTimeCode.py
//!   - pxr/usd/usd/testenv/testUsdTimeCodeRepr.py
//!   - pxr/usd/usd/testenv/testUsdTimeCodeStream.cpp

mod common;

use std::str::FromStr;
use usd_core::TimeCode;

// ============================================================================
// From testUsdTimeCode.py — testUsdTimeCodeOrdering
// ============================================================================

#[test]
fn tc_ordering() {
    common::setup();

    let time1 = TimeCode::new(1.0);
    let time2 = TimeCode::new(2.0);
    let time3 = TimeCode::default_time(); // Default
    let time4 = TimeCode::earliest_time();
    let time5 = TimeCode::pre_time(2.0);
    let time6 = TimeCode::pre_time(3.0);
    let time7 = TimeCode::new(3.0);

    // Ordering: Default < EarliestTime < 1.0 < PreTime(2.0) < 2.0 < PreTime(3.0) < 3.0
    assert!(time3 < time4, "Default < EarliestTime");
    assert!(time4 < time1, "EarliestTime < 1.0");
    assert!(time1 < time5, "1.0 < PreTime(2.0)");
    assert!(time5 < time2, "PreTime(2.0) < 2.0");
    assert!(time2 < time6, "2.0 < PreTime(3.0)");
    assert!(time6 < time7, "PreTime(3.0) < 3.0");
}

// ============================================================================
// From testUsdTimeCode.py — testUsdTimeCodePreTime
// ============================================================================

#[test]
fn tc_pre_time() {
    common::setup();

    let time = TimeCode::pre_time(1.0);
    assert!(!time.is_default());
    assert!(time.is_numeric());
    assert!(time.is_pre_time());
}

// ============================================================================
// From testUsdTimeCodeRepr.py — testDefaultTimeRepr
// ============================================================================

#[test]
fn tc_default_repr() {
    common::setup();

    let default_time = TimeCode::default_time();
    assert!(default_time.is_default());

    // Display should output "DEFAULT"
    let display = format!("{default_time}");
    assert_eq!(display, "DEFAULT");

    // Roundtrip through FromStr
    let parsed = TimeCode::from_str(&display).expect("parse DEFAULT");
    assert_eq!(parsed, default_time);
}

// ============================================================================
// From testUsdTimeCodeRepr.py — testEarliestTimeRepr
// ============================================================================

#[test]
fn tc_earliest_repr() {
    common::setup();

    let earliest = TimeCode::earliest_time();
    assert!(earliest.is_earliest_time());

    // Display should output "EARLIEST"
    let display = format!("{earliest}");
    assert_eq!(display, "EARLIEST");

    // Roundtrip
    let parsed = TimeCode::from_str(&display).expect("parse EARLIEST");
    assert_eq!(parsed, earliest);
}

// ============================================================================
// From testUsdTimeCodeRepr.py — testNumericTimeRepr
// ============================================================================

#[test]
fn tc_numeric_repr() {
    common::setup();

    let time = TimeCode::new(123.0);
    assert!(time.is_numeric());
    assert_eq!(time.value(), 123.0);

    // Display should output the numeric value
    let display = format!("{time}");
    let parsed = TimeCode::from_str(&display).expect("parse numeric");
    assert_eq!(parsed, time);
}

// ============================================================================
// From testUsdTimeCodeRepr.py — testPreTimeRepr
// ============================================================================

#[test]
fn tc_pre_time_repr() {
    common::setup();

    let pre = TimeCode::pre_time(123.0);
    assert!(pre.is_pre_time());
    assert_eq!(pre.value(), 123.0);

    // Display should output "PRE_TIME <value>"
    let display = format!("{pre}");
    assert!(display.starts_with("PRE_TIME"));

    // Roundtrip
    let parsed = TimeCode::from_str(&display).expect("parse PRE_TIME");
    assert_eq!(parsed, pre);
}

// ============================================================================
// From testUsdTimeCodeRepr.py — testPreTimeEarliestRepr
// ============================================================================

#[test]
fn tc_pre_time_earliest_repr() {
    common::setup();

    let pre_earliest = TimeCode::pre_time(TimeCode::earliest_time().value());
    assert!(pre_earliest.is_pre_time());

    let display = format!("{pre_earliest}");
    assert!(display.contains("PRE_TIME"));
    assert!(display.contains("EARLIEST"));

    // Roundtrip
    let parsed = TimeCode::from_str(&display).expect("parse PRE_TIME EARLIEST");
    assert_eq!(parsed, pre_earliest);
}

// ============================================================================
// From testUsdTimeCodeStream.cpp — stream insertion tests
// ============================================================================

#[test]
fn tc_stream_insertion() {
    common::setup();

    let default_constructed = TimeCode::new(0.0);
    let default_time = TimeCode::default_time();
    let earliest_time = TimeCode::earliest_time();
    let pre_time_earliest = TimeCode::pre_time(earliest_time.value());
    let numeric_time = TimeCode::new(123.0);
    let pre_time = TimeCode::pre_time(123.0);

    // default constructed (0.0) — should output "0"
    let s = format!("{default_constructed}");
    assert_eq!(s, "0");

    // Default — should output "DEFAULT"
    let s = format!("{default_time}");
    assert_eq!(s, "DEFAULT");

    // Earliest — should output "EARLIEST"
    let s = format!("{earliest_time}");
    assert_eq!(s, "EARLIEST");

    // PreTime(earliest) — should output "PRE_TIME EARLIEST"
    let s = format!("{pre_time_earliest}");
    assert_eq!(s, "PRE_TIME EARLIEST");

    // Numeric time
    let s = format!("{numeric_time}");
    assert_eq!(s, "123");

    // PreTime numeric
    let s = format!("{pre_time}");
    assert_eq!(s, "PRE_TIME 123");
}

// ============================================================================
// From testUsdTimeCodeStream.cpp — stream extraction tests
// ============================================================================

#[test]
fn tc_stream_extraction() {
    common::setup();

    // "0" → TimeCode(0.0)
    let tc = TimeCode::from_str("0").expect("parse 0");
    assert_eq!(tc, TimeCode::new(0.0));

    // "DEFAULT" → Default
    let tc = TimeCode::from_str("DEFAULT").expect("parse DEFAULT");
    assert_eq!(tc, TimeCode::default_time());

    // "EARLIEST" → EarliestTime
    let tc = TimeCode::from_str("EARLIEST").expect("parse EARLIEST");
    assert_eq!(tc, TimeCode::earliest_time());

    // "123" → TimeCode(123.0)
    let tc = TimeCode::from_str("123").expect("parse 123");
    assert_eq!(tc, TimeCode::new(123.0));

    // "PRE_TIME EARLIEST" → PreTime(earliest)
    let tc = TimeCode::from_str("PRE_TIME EARLIEST").expect("parse PRE_TIME EARLIEST");
    assert_eq!(tc, TimeCode::pre_time(TimeCode::earliest_time().value()));

    // "PRE_TIME 123" → PreTime(123.0)
    let tc = TimeCode::from_str("PRE_TIME 123").expect("parse PRE_TIME 123");
    assert_eq!(tc, TimeCode::pre_time(123.0));

    // Bad data should fail
    assert!(TimeCode::from_str("bogus").is_err());

    // "PRE_TIME bogus" should fail
    assert!(TimeCode::from_str("PRE_TIME bogus").is_err());

    // "PRE_TIME DEFAULT" should fail (per C++ semantics: left unchanged)
    assert!(TimeCode::from_str("PRE_TIME DEFAULT").is_err());
}

// ============================================================================
// Additional: basic identity and properties
// ============================================================================

#[test]
fn tc_basic_properties() {
    common::setup();

    // Default identity
    let d1 = TimeCode::default_time();
    let d2 = TimeCode::default_time();
    assert_eq!(d1, d2);
    assert!(d1.is_default());
    assert!(!d1.is_numeric());

    // Numeric identity
    let t1 = TimeCode::new(5.0);
    let t2 = TimeCode::new(5.0);
    assert_eq!(t1, t2);
    assert!(!t1.is_default());
    assert!(t1.is_numeric());
    assert_eq!(t1.value(), 5.0);

    // Default != numeric
    assert_ne!(d1, t1);

    // From SdfTimeCode
    let sdf_tc = usd_sdf::TimeCode::new(7.5);
    let usd_tc = TimeCode::from_sdf_time_code(&sdf_tc);
    assert_eq!(usd_tc.value(), 7.5);
    assert!(usd_tc.is_numeric());

    // SafeStep produces small positive number
    let step = TimeCode::safe_step(1e6, 10.0);
    assert!(step > 0.0);
    assert!(step < 1.0);
}

// ============================================================================
// Additional: arithmetic (TimeCode is numeric wrapper)
// ============================================================================

#[test]
fn tc_from_conversions() {
    common::setup();

    // From f64
    let tc: TimeCode = 3.14.into();
    assert_eq!(tc.value(), 3.14);

    // From SdfTimeCode via Into
    let sdf: usd_sdf::TimeCode = usd_sdf::TimeCode::new(2.5);
    let tc: TimeCode = sdf.into();
    assert_eq!(tc.value(), 2.5);
}
