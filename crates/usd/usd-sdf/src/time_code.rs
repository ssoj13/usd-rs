//! Time code type.
//!
//! `TimeCode` is now defined in `usd-vt` and re-exported here for backward
//! compatibility. This eliminates the circular dependency that arose when
//! `usd-vt::Value` needed to store `TimeCode` but `usd-sdf` depended on
//! `usd-vt`.
//!
//! All implementation details are in `usd_vt::time_code`.

// Re-export so existing `use usd_sdf::TimeCode` imports keep working.
pub use usd_vt::TimeCode;

#[cfg(test)]
mod tests {
    use super::TimeCode;
    use crate::LayerOffset;

    #[test]
    fn test_basic() {
        let tc = TimeCode::new(1.5);
        assert_eq!(tc.value(), 1.5);
    }

    #[test]
    fn test_default_time() {
        let tc = TimeCode::default_time();
        assert!(tc.is_default());
        assert!(TimeCode::DEFAULT.is_default());
    }

    #[test]
    fn test_arithmetic() {
        let t1 = TimeCode::new(10.0);
        let t2 = TimeCode::new(5.0);
        assert_eq!((t1 + t2).value(), 15.0);
        assert_eq!((t1 - t2).value(), 5.0);
    }

    // Ported from testSdfTimeCode.py :: test_ReprAndConversion
    #[test]
    fn test_repr_and_conversion() {
        // Default TimeCode equals TimeCode(0) — note: our Default is NaN (the "default time"
        // sentinel), so we test the zero case explicitly per the C++ typed-value semantics.
        // TODO: C++ SdfTimeCode() (default-constructed) is 0.0, not NaN; our Default is NaN
        // (the "default time" sentinel used for non-time-sampled values). The two concepts
        // diverge here. Until resolved, we test what the Rust API actually provides.
        let tc0 = TimeCode::new(0.0);
        let tc3 = TimeCode::new(3.0);
        let tc_neg = TimeCode::new(-2.5);

        // value() returns the underlying f64
        assert_eq!(tc0.value(), 0.0);
        assert_eq!(tc3.value(), 3.0);
        assert_eq!(tc_neg.value(), -2.5);

        // From/Into f64 conversions
        let f: f64 = tc3.into();
        assert_eq!(f, 3.0);
        let tc_from: TimeCode = 3.0_f64.into();
        assert_eq!(tc_from, tc3);

        // Display matches the wrapped value
        assert_eq!(format!("{}", tc0), "0");
        assert_eq!(format!("{}", tc3), "3");
        assert_eq!(format!("{}", tc_neg), "-2.5");

        // TODO: bool conversion (operator bool in C++: time != 0.0) is not yet implemented.
        // C++ behavior: TimeCode(0) -> false, TimeCode(3.0) -> true, TimeCode(-2.5) -> true.
        // Add `impl From<TimeCode> for bool` or `fn is_nonzero(&self) -> bool` in usd-vt.
    }

    // Ported from testSdfTimeCode.py :: test_Comparison
    #[test]
    fn test_comparison() {
        let tc0 = TimeCode::new(0.0);
        let tc3 = TimeCode::new(3.0);
        let tc_neg = TimeCode::new(-2.5);

        // TimeCode == TimeCode
        assert_eq!(tc3, TimeCode::new(3.0));
        assert_ne!(tc_neg, TimeCode::new(3.0));

        // TimeCode == f64
        assert!(tc3 == 3.0_f64);
        assert!(tc_neg != 3.0_f64);

        // f64 == TimeCode
        assert!(3.0_f64 == tc3);
        assert!(3.0_f64 != tc_neg);

        // < operator: TimeCode < TimeCode
        assert!(!(tc0 < tc0));
        assert!(tc0 < tc3);
        assert!(!(tc0 < tc_neg));

        // < operator: TimeCode < f64
        assert!(!(tc0 < 0.0_f64));
        assert!(tc0 < 3.0_f64);
        assert!(!(tc0 < -2.5_f64));

        // < operator: f64 < TimeCode
        assert!(!(0.0_f64 < tc0));
        assert!(!(3.0_f64 < tc0));
        assert!(-2.5_f64 < tc0);

        // <= operator: TimeCode <= TimeCode
        assert!(tc0 <= tc0);
        assert!(tc0 <= tc3);
        assert!(!(tc0 <= tc_neg));

        // <= operator: TimeCode <= f64
        assert!(tc0 <= 0.0_f64);
        assert!(tc0 <= 3.0_f64);
        assert!(!(tc0 <= -2.5_f64));

        // <= operator: f64 <= TimeCode
        assert!(0.0_f64 <= tc0);
        assert!(!(3.0_f64 <= tc0));
        assert!(-2.5_f64 <= tc0);

        // > operator: TimeCode > TimeCode
        assert!(!(tc0 > tc0));
        assert!(!(tc0 > tc3));
        assert!(tc0 > tc_neg);

        // > operator: TimeCode > f64
        assert!(!(tc0 > 0.0_f64));
        assert!(!(tc0 > 3.0_f64));
        assert!(tc0 > -2.5_f64);

        // > operator: f64 > TimeCode
        assert!(!(0.0_f64 > tc0));
        assert!(3.0_f64 > tc0);
        assert!(!(-2.5_f64 > tc0));

        // >= operator: TimeCode >= TimeCode
        assert!(tc0 >= tc0);
        assert!(!(tc0 >= tc3));
        assert!(tc0 >= tc_neg);

        // >= operator: TimeCode >= f64
        assert!(tc0 >= 0.0_f64);
        assert!(!(tc0 >= 3.0_f64));
        assert!(tc0 >= -2.5_f64);

        // >= operator: f64 >= TimeCode
        assert!(0.0_f64 >= tc0);
        assert!(3.0_f64 >= tc0);
        assert!(!(-2.5_f64 >= tc0));
    }

    // Ported from testSdfTimeCode.py :: test_Arithmetic
    #[test]
    fn test_arithmetic_full() {
        let tc0 = TimeCode::new(0.0);
        let tc3 = TimeCode::new(3.0);
        let tc_neg = TimeCode::new(-2.5);

        // TimeCode + TimeCode
        assert_eq!(tc3 + tc_neg, TimeCode::new(0.5));
        assert_eq!(tc_neg + tc3, TimeCode::new(0.5));

        // TimeCode + f64 and f64 + TimeCode
        assert_eq!(tc3 + 5.0_f64, TimeCode::new(8.0));
        assert_eq!(5.0_f64 + tc3, TimeCode::new(8.0));

        // TimeCode - TimeCode
        assert_eq!(tc3 - tc_neg, TimeCode::new(5.5));
        assert_eq!(tc_neg - tc3, TimeCode::new(-5.5));

        // TimeCode - f64 and f64 - TimeCode
        assert_eq!(tc3 - 5.0_f64, TimeCode::new(-2.0));
        assert_eq!(5.0_f64 - tc3, TimeCode::new(2.0));

        // TimeCode * TimeCode
        assert_eq!(tc3 * tc_neg, TimeCode::new(-7.5));
        assert_eq!(tc_neg * tc3, TimeCode::new(-7.5));

        // TimeCode * f64 and f64 * TimeCode
        assert_eq!(tc3 * 5.0_f64, TimeCode::new(15.0));
        assert_eq!(5.0_f64 * tc3, TimeCode::new(15.0));

        // TimeCode / TimeCode
        assert_eq!(tc3 / TimeCode::new(2.0), TimeCode::new(1.5));
        assert_eq!(TimeCode::new(6.0) / tc3, TimeCode::new(2.0));

        // TimeCode / f64 and f64 / TimeCode
        assert_eq!(tc3 / 5.0_f64, TimeCode::new(0.6));
        assert_eq!(6.0_f64 / tc3, TimeCode::new(2.0));

        // Suppress unused variable warning for tc0 (used to mirror Python fixture)
        let _ = tc0;
    }

    // Ported from testSdfTimeCode.py :: test_LayerOffset
    #[test]
    fn test_layer_offset() {
        // offset=3, scale=1 (default scale)
        let lo_offset = LayerOffset::new(3.0, 1.0);
        // offset=0 (default), scale=2
        let lo_scale = LayerOffset::new(0.0, 2.0);
        // offset=3, scale=2
        let lo_both = LayerOffset::new(3.0, 2.0);

        let tc0 = TimeCode::new(0.0);
        let tc3 = TimeCode::new(3.0);
        let tc_neg = TimeCode::new(-2.5);

        // LayerOffset * TimeCode returns TimeCode (formula: scale * t + offset)
        assert_eq!(lo_offset * tc0, TimeCode::new(3.0)); // 1*0  + 3 = 3
        assert_eq!(lo_offset * tc3, TimeCode::new(6.0)); // 1*3  + 3 = 6
        assert_eq!(lo_offset * tc_neg, TimeCode::new(0.5)); // 1*-2.5 + 3 = 0.5

        assert_eq!(lo_scale * tc0, TimeCode::new(0.0)); // 2*0  + 0 = 0
        assert_eq!(lo_scale * tc3, TimeCode::new(6.0)); // 2*3  + 0 = 6
        assert_eq!(lo_scale * tc_neg, TimeCode::new(-5.0)); // 2*-2.5 + 0 = -5

        assert_eq!(lo_both * tc0, TimeCode::new(3.0)); // 2*0  + 3 = 3
        assert_eq!(lo_both * tc3, TimeCode::new(9.0)); // 2*3  + 3 = 9
        assert_eq!(lo_both * tc_neg, TimeCode::new(-2.0)); // 2*-2.5 + 3 = -2

        // LayerOffset * f64 returns f64, not TimeCode (type-level distinction)
        let result_f64: f64 = lo_offset * 0.0_f64;
        assert_eq!(result_f64, 3.0);
    }
}
