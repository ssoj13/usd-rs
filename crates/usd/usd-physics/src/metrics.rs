//! Physics metrics utilities.
//!
//! This module provides helper APIs for physics-related metrics operations,
//! specifically for managing the `kilogramsPerUnit` stage metadata.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/metrics.h` and `metrics.cpp`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::metrics::*;
//! use std::sync::Arc;
//!
//! let stage = Stage::open("scene.usda")?;
//!
//! // Get current mass units
//! let kpu = get_stage_kilograms_per_unit(&stage);
//!
//! // Check if using grams
//! if mass_units_are(kpu, MassUnits::GRAMS, 1e-5) {
//!     println!("Stage uses grams");
//! }
//! ```

use super::tokens::USD_PHYSICS_TOKENS;
use std::sync::Arc;
use usd_core::Stage;
use usd_vt::Value;

/// Common mass units expressed in kilograms.
///
/// Container for static double-precision symbols representing common
/// mass units of measure.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsMassUnits` class.
pub struct MassUnits;

impl MassUnits {
    /// Mass unit: grams (0.001 kg)
    pub const GRAMS: f64 = 0.001;

    /// Mass unit: kilograms (1.0 kg)
    pub const KILOGRAMS: f64 = 1.0;

    /// Mass unit: slugs (14.5939 kg)
    pub const SLUGS: f64 = 14.5939;
}

/// Returns the stage's authored `kilogramsPerUnit`, or 1.0 if unauthored.
///
/// # Arguments
///
/// * `stage` - The stage to query
///
/// # Returns
///
/// The authored kilogramsPerUnit value, or 1.0 (kilograms) as default.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsGetStageKilogramsPerUnit()`.
pub fn get_stage_kilograms_per_unit(stage: &Arc<Stage>) -> f64 {
    stage
        .get_metadata(&USD_PHYSICS_TOKENS.kilograms_per_unit)
        .and_then(|v| v.downcast_clone::<f64>())
        .unwrap_or(MassUnits::KILOGRAMS)
}

/// Returns whether the stage has an authored `kilogramsPerUnit`.
///
/// # Arguments
///
/// * `stage` - The stage to check
///
/// # Returns
///
/// `true` if the stage has authored kilogramsPerUnit metadata.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsStageHasAuthoredKilogramsPerUnit()`.
pub fn stage_has_authored_kilograms_per_unit(stage: &Arc<Stage>) -> bool {
    stage.has_authored_metadata(&USD_PHYSICS_TOKENS.kilograms_per_unit)
}

/// Authors the stage's `kilogramsPerUnit`.
///
/// # Arguments
///
/// * `stage` - The stage to modify
/// * `kilograms_per_unit` - The value to set
///
/// # Returns
///
/// `true` if kilogramsPerUnit was successfully set. The stage's
/// UsdEditTarget must be either its root layer or session layer.
///
/// # C++ Reference
///
/// Port of `UsdPhysicsSetStageKilogramsPerUnit()`.
pub fn set_stage_kilograms_per_unit(stage: &Arc<Stage>, kilograms_per_unit: f64) -> bool {
    stage.set_metadata(
        &USD_PHYSICS_TOKENS.kilograms_per_unit,
        Value::from(kilograms_per_unit),
    )
}

/// Compares two mass unit values within a relative epsilon.
///
/// Use this when you need to know an absolute metric rather than a scaling factor.
///
/// # Arguments
///
/// * `authored_units` - The authored units value
/// * `standard_units` - The standard units value to compare against
/// * `epsilon` - Relative tolerance (default 1e-5)
///
/// # Returns
///
/// `false` if either input is zero or negative, otherwise returns `true`
/// if the relative floating-point comparison succeeds.
///
/// # Example
///
/// ```ignore
/// let stage_units = get_stage_kilograms_per_unit(&stage);
///
/// if mass_units_are(stage_units, MassUnits::KILOGRAMS, 1e-5) {
///     // do something for kilograms
/// } else if mass_units_are(stage_units, MassUnits::GRAMS, 1e-5) {
///     // do something for grams
/// }
/// ```
///
/// # C++ Reference
///
/// Port of `UsdPhysicsMassUnitsAre()`.
pub fn mass_units_are(authored_units: f64, standard_units: f64, epsilon: f64) -> bool {
    if authored_units <= 0.0 || standard_units <= 0.0 {
        return false;
    }

    let diff = (authored_units - standard_units).abs();
    (diff / authored_units < epsilon) && (diff / standard_units < epsilon)
}

/// Convenience function with default epsilon of 1e-5.
pub fn mass_units_are_default(authored_units: f64, standard_units: f64) -> bool {
    mass_units_are(authored_units, standard_units, 1e-5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mass_units_constants() {
        assert_eq!(MassUnits::GRAMS, 0.001);
        assert_eq!(MassUnits::KILOGRAMS, 1.0);
        assert!((MassUnits::SLUGS - 14.5939).abs() < 1e-10);
    }

    #[test]
    fn test_mass_units_are() {
        // Exact match
        assert!(mass_units_are(1.0, 1.0, 1e-5));

        // Close match
        assert!(mass_units_are(1.0, 1.000001, 1e-5));

        // Not close enough
        assert!(!mass_units_are(1.0, 1.1, 1e-5));

        // Zero or negative
        assert!(!mass_units_are(0.0, 1.0, 1e-5));
        assert!(!mass_units_are(-1.0, 1.0, 1e-5));
        assert!(!mass_units_are(1.0, 0.0, 1e-5));
    }

    #[test]
    fn test_mass_units_are_default() {
        assert!(mass_units_are_default(MassUnits::KILOGRAMS, 1.0));
        assert!(mass_units_are_default(MassUnits::GRAMS, 0.001));
    }
}
