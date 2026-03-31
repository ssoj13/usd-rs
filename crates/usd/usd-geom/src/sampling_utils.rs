//! UsdGeom sampling utilities - helper functions for sampling geometric attributes.
//!
//! Port of pxr/usd/usdGeom/samplingUtils.h/cpp
//!
//! Utility functions for sampling positions, velocities, accelerations, scales,
//! orientations, and angular velocities from USD attributes.

use usd_core::Attribute;
use usd_gf::quat::{Quatf, Quath};
use usd_gf::vec3::Vec3f;
use usd_sdf::TimeCode;
use usd_vt::Array;

// ============================================================================
// Time Delta Calculation
// ============================================================================

/// Calculate the time delta between two time codes, accounting for
/// timeCodesPerSecond.
///
/// Matches C++ `UsdGeom_CalculateTimeDelta()`.
pub fn calculate_time_delta(
    time: TimeCode,
    sample_time: TimeCode,
    time_codes_per_second: f64,
) -> f64 {
    if time.is_default() || sample_time.is_default() {
        return 0.0;
    }
    (time.value() - sample_time.value()) / time_codes_per_second
}

// ============================================================================
// Position, Velocity, Acceleration Sampling
// ============================================================================

/// Result of sampling positions, velocities, and accelerations.
#[derive(Debug, Clone)]
pub struct PositionsVelocitiesAccelerations {
    /// Sampled positions.
    pub positions: Array<Vec3f>,
    /// Sampled velocities (may be empty).
    pub velocities: Array<Vec3f>,
    /// Sampled accelerations (may be empty).
    pub accelerations: Array<Vec3f>,
}

/// Get positions, velocities, and accelerations from attributes at the given time.
///
/// If velocities or accelerations are not present, the corresponding arrays
/// will be empty.
///
/// Matches C++ `UsdGeom_GetPositionsVelocitiesAndAccelerations()`.
pub fn get_positions_velocities_and_accelerations(
    positions_attr: &Attribute,
    velocities_attr: Option<&Attribute>,
    accelerations_attr: Option<&Attribute>,
    time: TimeCode,
) -> Option<PositionsVelocitiesAccelerations> {
    if !positions_attr.is_valid() {
        return None;
    }

    // Get positions
    let positions = positions_attr.get_typed::<Array<Vec3f>>(time)?;

    // Get velocities if available
    let velocities = if let Some(vel_attr) = velocities_attr {
        if vel_attr.is_valid() {
            vel_attr.get_typed::<Array<Vec3f>>(time).unwrap_or_default()
        } else {
            Array::new()
        }
    } else {
        Array::new()
    };

    // Get accelerations if available
    let accelerations = if let Some(accel_attr) = accelerations_attr {
        if accel_attr.is_valid() {
            accel_attr
                .get_typed::<Array<Vec3f>>(time)
                .unwrap_or_default()
        } else {
            Array::new()
        }
    } else {
        Array::new()
    };

    Some(PositionsVelocitiesAccelerations {
        positions,
        velocities,
        accelerations,
    })
}

// ============================================================================
// Scale Sampling
// ============================================================================

/// Get scales from attribute at the given time.
///
/// If the attribute is not valid or has no value, returns None.
///
/// Matches C++ `UsdGeom_GetScales()`.
pub fn get_scales(scales_attr: &Attribute, time: TimeCode) -> Option<Array<Vec3f>> {
    if !scales_attr.is_valid() {
        return None;
    }
    scales_attr.get_typed::<Array<Vec3f>>(time)
}

// ============================================================================
// Orientation Sampling
// ============================================================================

/// Result of sampling orientations and angular velocities.
#[derive(Debug, Clone)]
pub struct OrientationsAngularVelocities {
    /// Sampled orientations (Quatf).
    pub orientations: Array<Quatf>,
    /// Sampled angular velocities (may be empty).
    pub angular_velocities: Array<Vec3f>,
}

/// Result of sampling orientations and angular velocities (Quath).
#[derive(Debug, Clone)]
pub struct OrientationsAngularVelocitiesHalf {
    /// Sampled orientations (Quath).
    pub orientations: Array<Quath>,
    /// Sampled angular velocities (may be empty).
    pub angular_velocities: Array<Vec3f>,
}

/// Get orientations and angular velocities from attributes at the given time (Quatf).
///
/// If angular velocities are not present, the array will be empty.
///
/// Matches C++ `UsdGeom_GetOrientationsAndAngularVelocities<GfQuatf>()`.
pub fn get_orientations_and_angular_velocities(
    orientations_attr: &Attribute,
    angular_velocities_attr: Option<&Attribute>,
    time: TimeCode,
) -> Option<OrientationsAngularVelocities> {
    if !orientations_attr.is_valid() {
        return None;
    }

    // Get orientations
    let orientations = orientations_attr.get_typed::<Array<Quatf>>(time)?;

    // Get angular velocities if available
    let angular_velocities = if let Some(ang_vel_attr) = angular_velocities_attr {
        if ang_vel_attr.is_valid() {
            ang_vel_attr
                .get_typed::<Array<Vec3f>>(time)
                .unwrap_or_default()
        } else {
            Array::new()
        }
    } else {
        Array::new()
    };

    Some(OrientationsAngularVelocities {
        orientations,
        angular_velocities,
    })
}

/// Get orientations and angular velocities from attributes at the given time (Quath).
///
/// If angular velocities are not present, the array will be empty.
///
/// Matches C++ `UsdGeom_GetOrientationsAndAngularVelocities<GfQuath>()`.
pub fn get_orientations_and_angular_velocities_half(
    orientations_attr: &Attribute,
    angular_velocities_attr: Option<&Attribute>,
    time: TimeCode,
) -> Option<OrientationsAngularVelocitiesHalf> {
    if !orientations_attr.is_valid() {
        return None;
    }

    // Get orientations
    let orientations = orientations_attr.get_typed::<Array<Quath>>(time)?;

    // Get angular velocities if available
    let angular_velocities = if let Some(ang_vel_attr) = angular_velocities_attr {
        if ang_vel_attr.is_valid() {
            ang_vel_attr
                .get_typed::<Array<Vec3f>>(time)
                .unwrap_or_default()
        } else {
            Array::new()
        }
    } else {
        Array::new()
    };

    Some(OrientationsAngularVelocitiesHalf {
        orientations,
        angular_velocities,
    })
}
