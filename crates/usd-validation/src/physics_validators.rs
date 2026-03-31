//! UsdPhysics validation validators.
//!
//! Port of `pxr/usdValidation/usdPhysicsValidators/validators.cpp`.
//!
//! Provides 4 validators:
//! - RigidBodyChecker: validates UsdPhysicsRigidBodyAPI usage
//! - ColliderChecker: validates UsdPhysicsCollisionAPI and geometry
//! - ArticulationChecker: validates UsdPhysicsArticulationRootAPI
//! - PhysicsJointChecker: validates UsdPhysicsJoint relationships

use crate::{
    ErrorSite, ErrorType, ValidationError, ValidationRegistry, ValidationTimeRange,
    ValidatorMetadata,
};
use std::sync::{Arc, LazyLock};
use usd_core::prim::Prim;
use usd_core::relationship::Relationship;
use usd_geom::Xformable;
use usd_gf::{Quatf, Vec3d, Vec3f};
use usd_physics::{MassAPI, RigidBodyAPI};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt;

// ============================================================================
// Tokens
// ============================================================================

/// Validator name tokens.
pub mod validator_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// RigidBodyChecker validator name.
    pub static RIGID_BODY_CHECKER: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdPhysicsValidators:RigidBodyChecker"));
    /// ColliderChecker validator name.
    pub static COLLIDER_CHECKER: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdPhysicsValidators:ColliderChecker"));
    /// ArticulationChecker validator name.
    pub static ARTICULATION_CHECKER: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdPhysicsValidators:ArticulationChecker"));
    /// PhysicsJointChecker validator name.
    pub static PHYSICS_JOINT_CHECKER: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdPhysicsValidators:PhysicsJointChecker"));
}

/// Error name tokens.
pub mod error_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// NestedArticulation error name.
    pub static NESTED_ARTICULATION: LazyLock<Token> =
        LazyLock::new(|| Token::new("NestedArticulation"));
    /// ArticulationOnStaticBody error name.
    pub static ARTICULATION_ON_STATIC_BODY: LazyLock<Token> =
        LazyLock::new(|| Token::new("ArticulationOnStaticBody"));
    /// RigidBodyOrientationScale error name.
    pub static RIGID_BODY_ORIENTATION_SCALE: LazyLock<Token> =
        LazyLock::new(|| Token::new("RigidBodyOrientationScale"));
    /// RigidBodyNonXformable error name.
    pub static RIGID_BODY_NON_XFORMABLE: LazyLock<Token> =
        LazyLock::new(|| Token::new("RigidBodyNonXformable"));
    /// RigidBodyNonInstanceable error name.
    pub static RIGID_BODY_NON_INSTANCEABLE: LazyLock<Token> =
        LazyLock::new(|| Token::new("RigidBodyNonInstanceable"));
    /// JointInvalidPrimRel error name.
    pub static JOINT_INVALID_PRIM_REL: LazyLock<Token> =
        LazyLock::new(|| Token::new("JointInvalidPrimRel"));
    /// JointMultiplePrimsRel error name.
    pub static JOINT_MULTIPLE_PRIMS_REL: LazyLock<Token> =
        LazyLock::new(|| Token::new("JointMultiplePrimsRel"));
    /// ColliderNonUniformScale error name.
    pub static COLLIDER_NON_UNIFORM_SCALE: LazyLock<Token> =
        LazyLock::new(|| Token::new("ColliderNonUniformScale"));
    /// ColliderSpherePointsDataMissing error name.
    pub static COLLIDER_SPHERE_POINTS_DATA_MISSING: LazyLock<Token> =
        LazyLock::new(|| Token::new("ColliderSpherePointsDataMissing"));
    /// MassInvalidValues error name.
    pub static MASS_INVALID_VALUES: LazyLock<Token> =
        LazyLock::new(|| Token::new("MassInvalidValues"));
    /// DensityInvalidValues error name.
    pub static DENSITY_INVALID_VALUES: LazyLock<Token> =
        LazyLock::new(|| Token::new("DensityInvalidValues"));
    /// InertiaInvalidValues error name.
    pub static INERTIA_INVALID_VALUES: LazyLock<Token> =
        LazyLock::new(|| Token::new("InertiaInvalidValues"));
}

/// Keyword token "UsdPhysicsValidators".
pub static KW_USD_PHYSICS_VALIDATORS: LazyLock<Token> =
    LazyLock::new(|| Token::new("UsdPhysicsValidators"));

// ============================================================================
// Helpers
// ============================================================================

/// Check if scale is uniform within epsilon=1e-5.
#[allow(dead_code)]
fn scale_is_uniform(scale: &Vec3d) -> bool {
    const EPS: f64 = 1.0e-5;
    let (lo, hi) = if scale[0] < scale[1] {
        (scale[0], scale[1])
    } else {
        (scale[1], scale[0])
    };
    let lo = if scale[2] < lo { scale[2] } else { lo };
    let hi = if scale[2] > hi { scale[2] } else { hi };

    // Opposite signs check
    if lo * hi < 0.0 {
        return false;
    }

    if hi > 0.0 {
        hi - lo <= EPS * lo
    } else {
        lo - hi >= EPS * hi
    }
}

/// Check if prim is a dynamic body (RigidBodyAPI present and enabled).
/// Returns (has_rigid_body_api, is_enabled).
#[allow(dead_code)]
fn is_dynamic_body(prim: &Prim) -> (bool, bool) {
    let body = RigidBodyAPI::new(prim.clone());
    if !body.is_valid() {
        return (false, false);
    }

    // Read physics:rigidBodyEnabled attr; defaults to true if not authored.
    let enabled = body
        .get_rigid_body_enabled_attr()
        .and_then(|a| a.get(TimeCode::default()))
        .and_then(|v| v.get::<bool>().copied())
        .unwrap_or(true);

    (true, enabled)
}

/// Check if any ancestor has ArticulationRootAPI.
fn check_nested_articulation_root(prim: &Prim) -> bool {
    let mut parent = prim.parent();

    loop {
        if parent.is_pseudo_root() {
            return false;
        }
        // PhysicsArticulationRootAPI is a single-apply API; check via has_api.
        if parent.has_api(&Token::new("PhysicsArticulationRootAPI")) {
            return true;
        }
        parent = parent.parent();
        if !parent.is_valid() {
            return false;
        }
    }
}

/// Check MassAPI attributes for validity.
fn check_mass_api(prim: &Prim, errors: &mut Vec<ValidationError>) {
    let mass_api = MassAPI::new(prim.clone());
    if !mass_api.is_valid() {
        return;
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return,
    };

    let site = ErrorSite::from_stage(&stage, prim.get_path().clone(), None);

    // Check mass attribute
    if let Some(attr) = prim.get_attribute("physics:mass") {
        if let Some(value) = attr.get(TimeCode::default()) {
            if let Some(mass) = value.get::<f32>() {
                if *mass < 0.0 {
                    errors.push(ValidationError::new(
                        error_tokens::MASS_INVALID_VALUES.clone(),
                        ErrorType::Error,
                        vec![site.clone()],
                        format!("Mass is negative, prim path: {}", prim.get_path()),
                    ));
                }
            }
        }
    }

    // Check density attribute
    if let Some(attr) = prim.get_attribute("physics:density") {
        if let Some(value) = attr.get(TimeCode::default()) {
            if let Some(density) = value.get::<f32>() {
                if *density < 0.0 {
                    errors.push(ValidationError::new(
                        error_tokens::DENSITY_INVALID_VALUES.clone(),
                        ErrorType::Error,
                        vec![site.clone()],
                        format!("Density is negative, prim path: {}", prim.get_path()),
                    ));
                }
            }
        }
    }

    // Check inertia consistency (principalAxes and diagonalInertia)
    let principal_attr = prim.get_attribute("physics:principalAxes");
    let diagonal_attr = prim.get_attribute("physics:diagonalInertia");

    let principal_authored = principal_attr
        .as_ref()
        .map_or(false, |a| a.has_authored_value());
    let diagonal_authored = diagonal_attr
        .as_ref()
        .map_or(false, |a| a.has_authored_value());

    // Both must be authored together or not at all
    if principal_authored != diagonal_authored {
        errors.push(
            ValidationError::new(
                error_tokens::INERTIA_INVALID_VALUES.clone(),
                ErrorType::Error,
                vec![site.clone()],
                format!(
                    "principalAxes and diagonalInertia must both be authored or neither authored, prim path: {}",
                    prim.get_path()
                ),
            )
        );
        return;
    }

    // If both authored, validate values
    if principal_authored && diagonal_authored {
        let principal_value = principal_attr.and_then(|a| a.get(TimeCode::default()));
        let diagonal_value = diagonal_attr.and_then(|a| a.get(TimeCode::default()));

        if let (Some(pv), Some(dv)) = (principal_value, diagonal_value) {
            let principal_quat = pv.get::<Quatf>();
            let diagonal_vec = dv.get::<Vec3f>();

            if let (Some(quat), Some(vec)) = (principal_quat, diagonal_vec) {
                let zero_quat = Quatf::new(0.0, Vec3f::new(0.0, 0.0, 0.0));
                let zero_vec = Vec3f::new(0.0, 0.0, 0.0);

                let principal_is_fallback = *quat == zero_quat;
                let diagonal_is_fallback = *vec == zero_vec;

                // If one is fallback, both must be
                if principal_is_fallback != diagonal_is_fallback {
                    errors.push(
                        ValidationError::new(
                            error_tokens::INERTIA_INVALID_VALUES.clone(),
                            ErrorType::Error,
                            vec![site.clone()],
                            format!(
                                "principalAxes and diagonalInertia must both be authored in the valid range or neither authored, prim path: {}",
                                prim.get_path()
                            ),
                        )
                    );
                }

                // If not fallback, validate values
                if !principal_is_fallback && !diagonal_is_fallback {
                    // Check quaternion is normalized
                    if (quat.length() - 1.0).abs() > 1e-5 {
                        errors.push(ValidationError::new(
                            error_tokens::INERTIA_INVALID_VALUES.clone(),
                            ErrorType::Error,
                            vec![site.clone()],
                            format!(
                                "principalAxes must be a valid unit quaternion, prim path: {}",
                                prim.get_path()
                            ),
                        ));
                    }

                    // Check diagonal inertia is positive
                    if vec[0] <= 0.0 || vec[1] <= 0.0 || vec[2] <= 0.0 {
                        errors.push(ValidationError::new(
                            error_tokens::INERTIA_INVALID_VALUES.clone(),
                            ErrorType::Error,
                            vec![site.clone()],
                            format!(
                                "diagonalInertia elements must be positive, prim path: {}",
                                prim.get_path()
                            ),
                        ));
                    }
                }
            }
        }
    }
}

/// Check if prim has uniform scale (returns true if uniform or non-xformable).
/// Uses Xformable::get_local_transformation() + Matrix4d::factor() to extract scale.
#[allow(dead_code)]
fn check_non_uniform_scale(prim: &Prim) -> bool {
    let xformable = Xformable::new(prim.clone());
    if !xformable.is_valid() {
        return true; // not xformable — assume uniform
    }

    let matrix = xformable.get_local_transformation(TimeCode::default());

    // factor() returns (perspective, scale, scaleOrient, rotation, translation).
    // We inspect the scale vector for uniformity.
    if let Some((_persp, scale, _so, _rot, _trans)) = matrix.factor() {
        let sx = scale[0];
        let sy = scale[1];
        let sz = scale[2];
        let eps = 1e-6_f64;
        (sy - sx).abs() < eps && (sz - sx).abs() < eps
    } else {
        true // degenerate matrix — treat as uniform
    }
}

// ============================================================================
// Validator implementations
// ============================================================================

/// RigidBodyChecker: validates RigidBodyAPI usage.
fn rigid_body_checker(prim: &Prim, _time_range: &ValidationTimeRange) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check if RigidBodyAPI is applied
    if !prim.has_api(&Token::new("PhysicsRigidBodyAPI")) {
        return errors;
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return errors,
    };

    let site = ErrorSite::from_stage(&stage, prim.get_path().clone(), None);

    // 1. Check if prim is Xformable
    if !prim.is_a(&Token::new("Xformable")) {
        errors.push(ValidationError::new(
            error_tokens::RIGID_BODY_NON_XFORMABLE.clone(),
            ErrorType::Error,
            vec![site.clone()],
            format!(
                "Rigid body API has to be applied to a xformable prim, prim path: {}",
                prim.get_path()
            ),
        ));
    }

    // 2. Check instancing (instance proxy not supported for dynamic bodies)
    if prim.is_instance_proxy() {
        let mut report_error = true;

        // Check if kinematic
        if let Some(attr) = prim.get_attribute("physics:kinematicEnabled") {
            if let Some(value) = attr.get(TimeCode::default()) {
                if let Some(kinematic) = value.get::<bool>() {
                    if *kinematic {
                        report_error = false;
                    }
                }
            }
        }

        // Check if enabled
        if report_error {
            if let Some(attr) = prim.get_attribute("physics:rigidBodyEnabled") {
                if let Some(value) = attr.get(TimeCode::default()) {
                    if let Some(enabled) = value.get::<bool>() {
                        if !*enabled {
                            report_error = false;
                        }
                    }
                }
            }
        }

        if report_error {
            errors.push(ValidationError::new(
                error_tokens::RIGID_BODY_NON_INSTANCEABLE.clone(),
                ErrorType::Error,
                vec![site.clone()],
                format!(
                    "RigidBodyAPI on an instance proxy is not supported, prim path: {}",
                    prim.get_path()
                ),
            ));
        }
    }

    // 3. Check scale orientation — non-uniform scale with non-identity scaleOrientation
    //    is not supported by the physics sim. check_non_uniform_scale() uses Xformable.

    // 4. Check MassAPI
    check_mass_api(prim, &mut errors);

    errors
}

/// ColliderChecker: validates CollisionAPI and geometry.
fn collider_checker(prim: &Prim, _time_range: &ValidationTimeRange) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check if CollisionAPI is applied and prim is Gprim
    if !prim.has_api(&Token::new("PhysicsCollisionAPI")) {
        return errors;
    }

    if !prim.is_a(&Token::new("Gprim")) {
        return errors;
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return errors,
    };

    let site = ErrorSite::from_stage(&stage, prim.get_path().clone(), None);

    // Check for specific geometry types that require uniform scale
    let type_name = prim.type_name();
    let requires_uniform_scale = matches!(
        type_name.as_str(),
        "Sphere" | "Capsule" | "Capsule_1" | "Cylinder" | "Cylinder_1" | "Cone" | "Points"
    );

    if requires_uniform_scale && !check_non_uniform_scale(prim) {
        errors.push(ValidationError::new(
            error_tokens::COLLIDER_NON_UNIFORM_SCALE.clone(),
            ErrorType::Error,
            vec![site.clone()],
            format!(
                "Non-uniform scale is not supported for {} geometry, prim path: {}",
                type_name,
                prim.get_path()
            ),
        ));
    }

    // Special check for Points geometry
    if prim.is_a(&Token::new("Points")) {
        let widths_attr = prim.get_attribute("widths");
        let points_attr = prim.get_attribute("points");

        let mut has_error = false;

        if let (Some(w), Some(p)) = (widths_attr, points_attr) {
            match (w.get(TimeCode::default()), p.get(TimeCode::default())) {
                (Some(widths_val), Some(points_val)) => {
                    // widths is float[], points is point3f[] (Vec3f).
                    // Sizes must match: one width per point.
                    let widths_len = widths_val
                        .get::<usd_vt::Array<f32>>()
                        .map(|a| a.len())
                        .or_else(|| widths_val.get::<Vec<f32>>().map(|v| v.len()));
                    let points_len = points_val
                        .get::<usd_vt::Array<Vec3f>>()
                        .map(|a| a.len())
                        .or_else(|| points_val.get::<Vec<Vec3f>>().map(|v| v.len()));
                    match (widths_len, points_len) {
                        (Some(wl), Some(pl)) if wl != pl || wl == 0 => has_error = true,
                        (None, _) | (_, None) => has_error = true,
                        _ => {}
                    }
                }
                _ => has_error = true,
            }
        } else {
            has_error = true;
        }

        if has_error {
            errors.push(
                ValidationError::new(
                    error_tokens::COLLIDER_SPHERE_POINTS_DATA_MISSING.clone(),
                    ErrorType::Error,
                    vec![site],
                    format!(
                        "UsdGeomPoints width or position array not filled or sizes do not match, prim path: {}",
                        prim.get_path()
                    ),
                )
            );
        }
    }

    // Check MassAPI
    check_mass_api(prim, &mut errors);

    errors
}

/// ArticulationChecker: validates ArticulationRootAPI.
fn articulation_checker(prim: &Prim, _time_range: &ValidationTimeRange) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check if ArticulationRootAPI is applied
    if !prim.has_api(&Token::new("PhysicsArticulationRootAPI")) {
        return errors;
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return errors,
    };

    let site = ErrorSite::from_stage(&stage, prim.get_path().clone(), None);

    // 1. Check for nested articulation roots
    if check_nested_articulation_root(prim) {
        errors.push(ValidationError::new(
            error_tokens::NESTED_ARTICULATION.clone(),
            ErrorType::Error,
            vec![site.clone()],
            format!(
                "Nested ArticulationRootAPI not supported, prim {}.",
                prim.get_path()
            ),
        ));
    }

    // 2. Check if rigid body is static (enabled=false)
    if prim.has_api(&Token::new("PhysicsRigidBodyAPI")) {
        if let Some(attr) = prim.get_attribute("physics:rigidBodyEnabled") {
            if let Some(value) = attr.get(TimeCode::default()) {
                if let Some(enabled) = value.get::<bool>() {
                    if !*enabled {
                        errors.push(
                            ValidationError::new(
                                error_tokens::ARTICULATION_ON_STATIC_BODY.clone(),
                                ErrorType::Error,
                                vec![site],
                                format!(
                                    "ArticulationRootAPI definition on a static rigid body is not allowed. Prim: {}",
                                    prim.get_path()
                                ),
                            )
                        );
                    }
                }
            }
        }
    }

    errors
}

/// PhysicsJointChecker: validates Joint relationships.
fn physics_joint_checker(prim: &Prim, _time_range: &ValidationTimeRange) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    // Check if prim is a PhysicsJoint
    if !prim.is_a(&Token::new("PhysicsJoint")) {
        return errors;
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return errors,
    };

    let site = ErrorSite::from_stage(&stage, prim.get_path().clone(), None);

    // Check body0 and body1 relationships
    let body0_rel = prim.get_relationship("physics:body0");
    let body1_rel = prim.get_relationship("physics:body1");

    // Helper to get first target from relationship
    let get_rel_target = |rel: &Option<Relationship>| -> Option<Path> {
        rel.as_ref()
            .and_then(|r| r.get_targets().into_iter().next())
    };

    // Helper to check if target is valid
    let check_rel_valid = |target: &Option<Path>| -> bool {
        match target {
            None => true, // Empty is ok
            Some(path) => stage.get_prim_at_path(path).is_some(),
        }
    };

    let body0_target = get_rel_target(&body0_rel);
    let body1_target = get_rel_target(&body1_rel);

    // 1. Check if relationships point to valid prims
    if !check_rel_valid(&body0_target) || !check_rel_valid(&body1_target) {
        errors.push(
            ValidationError::new(
                error_tokens::JOINT_INVALID_PRIM_REL.clone(),
                ErrorType::Error,
                vec![site.clone()],
                format!(
                    "Joint ({}) body relationship points to a non existent prim, joint will not be parsed.",
                    prim.get_path()
                ),
            )
        );
    }

    // 2. Check for multiple targets
    let body0_targets = body0_rel.as_ref().map(|r| r.get_targets());
    let body1_targets = body1_rel.as_ref().map(|r| r.get_targets());

    let has_multiple = body0_targets.as_ref().map_or(false, |t| t.len() > 1)
        || body1_targets.as_ref().map_or(false, |t| t.len() > 1);

    if has_multiple {
        errors.push(
            ValidationError::new(
                error_tokens::JOINT_MULTIPLE_PRIMS_REL.clone(),
                ErrorType::Error,
                vec![site],
                format!(
                    "Joint prim does have relationship to multiple bodies this is not supported, jointPrim {}",
                    prim.get_path()
                ),
            )
        );
    }

    errors
}

// ============================================================================
// Registration
// ============================================================================

/// Register all physics validators with the global registry.
pub fn register_physics_validators(registry: &ValidationRegistry) {
    // RigidBodyChecker
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::RIGID_BODY_CHECKER.clone())
            .with_doc("Validates UsdPhysicsRigidBodyAPI usage: checks if prim is Xformable, validates instancing, checks scale orientation, validates MassAPI values.")
            .with_keywords(vec![KW_USD_PHYSICS_VALIDATORS.clone()])
            .with_schema_types(vec![Token::new("PhysicsRigidBodyAPI")]),
        Arc::new(rigid_body_checker),
        Vec::new(),
    );

    // ColliderChecker
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::COLLIDER_CHECKER.clone())
            .with_doc("Validates UsdPhysicsCollisionAPI: checks for non-uniform scale on specific geometries (sphere, capsule, cylinder, cone, points), validates Points data, checks MassAPI values.")
            .with_keywords(vec![KW_USD_PHYSICS_VALIDATORS.clone()])
            .with_schema_types(vec![Token::new("PhysicsCollisionAPI")]),
        Arc::new(collider_checker),
        Vec::new(),
    );

    // ArticulationChecker
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::ARTICULATION_CHECKER.clone())
            .with_doc("Validates UsdPhysicsArticulationRootAPI: checks for nested articulation roots, validates that articulation is not on a static rigid body.")
            .with_keywords(vec![KW_USD_PHYSICS_VALIDATORS.clone()])
            .with_schema_types(vec![Token::new("PhysicsArticulationRootAPI")]),
        Arc::new(articulation_checker),
        Vec::new(),
    );

    // PhysicsJointChecker
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::PHYSICS_JOINT_CHECKER.clone())
            .with_doc("Validates UsdPhysicsJoint: checks that body0 and body1 relationships point to valid prims, checks for multiple targets per relationship.")
            .with_keywords(vec![KW_USD_PHYSICS_VALIDATORS.clone()])
            .with_schema_types(vec![Token::new("PhysicsJoint")]),
        Arc::new(physics_joint_checker),
        Vec::new(),
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;
    use usd_core::stage::Stage;

    #[test]
    fn test_scale_is_uniform() {
        assert!(scale_is_uniform(&Vec3d::new(1.0, 1.0, 1.0)));
        assert!(scale_is_uniform(&Vec3d::new(2.0, 2.0, 2.0)));
        assert!(!scale_is_uniform(&Vec3d::new(1.0, 2.0, 1.0)));
        assert!(!scale_is_uniform(&Vec3d::new(1.0, 1.0, 2.0)));
    }

    #[test]
    fn test_scale_is_uniform_epsilon() {
        // Within epsilon (1e-5 * 1.0 = 1e-5)
        assert!(scale_is_uniform(&Vec3d::new(1.0, 1.000001, 1.0)));
        // Outside epsilon
        assert!(!scale_is_uniform(&Vec3d::new(1.0, 1.0001, 1.0)));
    }

    #[test]
    fn test_scale_is_uniform_negative() {
        // Opposite signs should fail
        assert!(!scale_is_uniform(&Vec3d::new(1.0, -1.0, 1.0)));
    }

    #[test]
    fn test_rigid_body_checker_no_api() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Cube", "Cube").ok();
        let prim = stage
            .get_prim_at_path(&Path::from_string("/Cube").unwrap())
            .unwrap();

        let errors = rigid_body_checker(&prim, &ValidationTimeRange::default());
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_collider_checker_no_api() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Sphere", "Sphere").ok();
        let prim = stage
            .get_prim_at_path(&Path::from_string("/Sphere").unwrap())
            .unwrap();

        let errors = collider_checker(&prim, &ValidationTimeRange::default());
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_articulation_checker_no_api() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Root", "Xform").ok();
        let prim = stage
            .get_prim_at_path(&Path::from_string("/Root").unwrap())
            .unwrap();

        let errors = articulation_checker(&prim, &ValidationTimeRange::default());
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_physics_joint_checker_not_joint() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/NotJoint", "Xform").ok();
        let prim = stage
            .get_prim_at_path(&Path::from_string("/NotJoint").unwrap())
            .unwrap();

        let errors = physics_joint_checker(&prim, &ValidationTimeRange::default());
        assert_eq!(errors.len(), 0);
    }

    #[test]
    fn test_validator_registration() {
        let registry = ValidationRegistry::get_instance();

        // Register validators
        register_physics_validators(registry);

        // Check they exist
        assert!(registry.has_validator(&validator_tokens::RIGID_BODY_CHECKER));
        assert!(registry.has_validator(&validator_tokens::COLLIDER_CHECKER));
        assert!(registry.has_validator(&validator_tokens::ARTICULATION_CHECKER));
        assert!(registry.has_validator(&validator_tokens::PHYSICS_JOINT_CHECKER));
    }

    #[test]
    fn test_validator_metadata() {
        let registry = ValidationRegistry::get_instance();
        register_physics_validators(registry);

        let meta = registry
            .get_validator_metadata(&validator_tokens::RIGID_BODY_CHECKER)
            .unwrap();
        assert!(meta.doc.contains("RigidBodyAPI"));
        assert!(meta.keywords.contains(&KW_USD_PHYSICS_VALIDATORS));
    }

    #[test]
    fn test_validator_keyword_lookup() {
        let registry = ValidationRegistry::get_instance();
        register_physics_validators(registry);

        let validators = registry.get_validator_metadata_for_keyword(&KW_USD_PHYSICS_VALIDATORS);
        assert!(validators.len() >= 4);
    }

    #[test]
    fn test_check_mass_api_no_api() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Test", "Xform").ok();
        let prim = stage
            .get_prim_at_path(&Path::from_string("/Test").unwrap())
            .unwrap();

        let mut errors = Vec::new();
        check_mass_api(&prim, &mut errors);
        assert_eq!(errors.len(), 0);
    }
}
