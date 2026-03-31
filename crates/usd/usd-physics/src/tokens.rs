//! UsdPhysics schema tokens.
//!
//! This module provides static TfTokens for use in the UsdPhysics API.
//! These tokens are generated from the module's schema, representing
//! property names and allowed values.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/tokens.h`
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::tokens::USD_PHYSICS_TOKENS;
//!
//! let axis = USD_PHYSICS_TOKENS.physics_axis.clone();
//! ```

use std::sync::LazyLock;
use usd_tf::Token;

/// Static tokens for UsdPhysics schemas.
///
/// Provides efficient TfToken constants for property names and allowed values.
/// These tokens are auto-generated from the schema definitions.
#[derive(Debug)]
pub struct UsdPhysicsTokensType {
    // Drive type values
    /// "acceleration" - Possible value for UsdPhysicsDriveAPI::GetTypeAttr()
    pub acceleration: Token,
    /// "force" - Fallback value for UsdPhysicsDriveAPI::GetTypeAttr()
    pub force: Token,

    // Degree of freedom tokens
    /// "angular" - Angular degree of freedom for Revolute Joint Drive
    pub angular: Token,
    /// "linear" - Linear degree of freedom for Prismatic Joint Drive
    pub linear: Token,
    /// "distance" - Distance limit for generic D6 joint
    pub distance: Token,
    /// "rotX" - Rotate around X axis
    pub rot_x: Token,
    /// "rotY" - Rotate around Y axis
    pub rot_y: Token,
    /// "rotZ" - Rotate around Z axis
    pub rot_z: Token,
    /// "transX" - Translate along X axis
    pub trans_x: Token,
    /// "transY" - Translate along Y axis
    pub trans_y: Token,
    /// "transZ" - Translate along Z axis
    pub trans_z: Token,

    // Mesh collision approximation values
    /// "boundingCube" - Bounding cube approximation
    pub bounding_cube: Token,
    /// "boundingSphere" - Bounding sphere approximation
    pub bounding_sphere: Token,
    /// "convexDecomposition" - Convex decomposition
    pub convex_decomposition: Token,
    /// "convexHull" - Convex hull
    pub convex_hull: Token,
    /// "meshSimplification" - Mesh simplification
    pub mesh_simplification: Token,
    /// "none" - No approximation (use original mesh)
    pub none: Token,

    // Axis values
    /// "X" - X axis
    pub x: Token,
    /// "Y" - Y axis
    pub y: Token,
    /// "Z" - Z axis
    pub z: Token,

    // Collection name
    /// "colliders" - Collection name for CollisionGroup colliders
    pub colliders: Token,

    // Namespace prefixes
    /// "drive" - Property namespace prefix for DriveAPI
    pub drive: Token,
    /// "limit" - Property namespace prefix for LimitAPI
    pub limit: Token,

    // DriveAPI multiple-apply template properties
    /// "drive:__INSTANCE_NAME__:physics:damping"
    pub drive_template_physics_damping: Token,
    /// "drive:__INSTANCE_NAME__:physics:maxForce"
    pub drive_template_physics_max_force: Token,
    /// "drive:__INSTANCE_NAME__:physics:stiffness"
    pub drive_template_physics_stiffness: Token,
    /// "drive:__INSTANCE_NAME__:physics:targetPosition"
    pub drive_template_physics_target_position: Token,
    /// "drive:__INSTANCE_NAME__:physics:targetVelocity"
    pub drive_template_physics_target_velocity: Token,
    /// "drive:__INSTANCE_NAME__:physics:type"
    pub drive_template_physics_type: Token,

    // LimitAPI multiple-apply template properties
    /// "limit:__INSTANCE_NAME__:physics:high"
    pub limit_template_physics_high: Token,
    /// "limit:__INSTANCE_NAME__:physics:low"
    pub limit_template_physics_low: Token,

    // Stage metadata
    /// "kilogramsPerUnit" - Stage-level mass unit metadata
    pub kilograms_per_unit: Token,

    // Physics attribute tokens
    /// "physics:angularVelocity"
    pub physics_angular_velocity: Token,
    /// "physics:approximation"
    pub physics_approximation: Token,
    /// "physics:axis"
    pub physics_axis: Token,
    /// "physics:body0"
    pub physics_body0: Token,
    /// "physics:body1"
    pub physics_body1: Token,
    /// "physics:breakForce"
    pub physics_break_force: Token,
    /// "physics:breakTorque"
    pub physics_break_torque: Token,
    /// "physics:centerOfMass"
    pub physics_center_of_mass: Token,
    /// "physics:collisionEnabled"
    pub physics_collision_enabled: Token,
    /// "physics:coneAngle0Limit"
    pub physics_cone_angle0_limit: Token,
    /// "physics:coneAngle1Limit"
    pub physics_cone_angle1_limit: Token,
    /// "physics:density"
    pub physics_density: Token,
    /// "physics:diagonalInertia"
    pub physics_diagonal_inertia: Token,
    /// "physics:dynamicFriction"
    pub physics_dynamic_friction: Token,
    /// "physics:excludeFromArticulation"
    pub physics_exclude_from_articulation: Token,
    /// "physics:filteredGroups"
    pub physics_filtered_groups: Token,
    /// "physics:filteredPairs"
    pub physics_filtered_pairs: Token,
    /// "physics:gravityDirection"
    pub physics_gravity_direction: Token,
    /// "physics:gravityMagnitude"
    pub physics_gravity_magnitude: Token,
    /// "physics:invertFilteredGroups"
    pub physics_invert_filtered_groups: Token,
    /// "physics:jointEnabled"
    pub physics_joint_enabled: Token,
    /// "physics:kinematicEnabled"
    pub physics_kinematic_enabled: Token,
    /// "physics:localPos0"
    pub physics_local_pos0: Token,
    /// "physics:localPos1"
    pub physics_local_pos1: Token,
    /// "physics:localRot0"
    pub physics_local_rot0: Token,
    /// "physics:localRot1"
    pub physics_local_rot1: Token,
    /// "physics:lowerLimit"
    pub physics_lower_limit: Token,
    /// "physics:mass"
    pub physics_mass: Token,
    /// "physics:maxDistance"
    pub physics_max_distance: Token,
    /// "physics:mergeGroup"
    pub physics_merge_group: Token,
    /// "physics:minDistance"
    pub physics_min_distance: Token,
    /// "physics:principalAxes"
    pub physics_principal_axes: Token,
    /// "physics:restitution"
    pub physics_restitution: Token,
    /// "physics:rigidBodyEnabled"
    pub physics_rigid_body_enabled: Token,
    /// "physics:simulationOwner"
    pub physics_simulation_owner: Token,
    /// "physics:startsAsleep"
    pub physics_starts_asleep: Token,
    /// "physics:staticFriction"
    pub physics_static_friction: Token,
    /// "physics:upperLimit"
    pub physics_upper_limit: Token,
    /// "physics:velocity"
    pub physics_velocity: Token,

    // Schema identifiers
    /// "PhysicsArticulationRootAPI"
    pub physics_articulation_root_api: Token,
    /// "PhysicsCollisionAPI"
    pub physics_collision_api: Token,
    /// "PhysicsCollisionGroup"
    pub physics_collision_group: Token,
    /// "PhysicsDistanceJoint"
    pub physics_distance_joint: Token,
    /// "PhysicsDriveAPI"
    pub physics_drive_api: Token,
    /// "PhysicsFilteredPairsAPI"
    pub physics_filtered_pairs_api: Token,
    /// "PhysicsFixedJoint"
    pub physics_fixed_joint: Token,
    /// "PhysicsJoint"
    pub physics_joint: Token,
    /// "PhysicsLimitAPI"
    pub physics_limit_api: Token,
    /// "PhysicsMassAPI"
    pub physics_mass_api: Token,
    /// "PhysicsMaterialAPI"
    pub physics_material_api: Token,
    /// "PhysicsMeshCollisionAPI"
    pub physics_mesh_collision_api: Token,
    /// "PhysicsPrismaticJoint"
    pub physics_prismatic_joint: Token,
    /// "PhysicsRevoluteJoint"
    pub physics_revolute_joint: Token,
    /// "PhysicsRigidBodyAPI"
    pub physics_rigid_body_api: Token,
    /// "PhysicsScene"
    pub physics_scene: Token,
    /// "PhysicsSphericalJoint"
    pub physics_spherical_joint: Token,
}

impl UsdPhysicsTokensType {
    /// Creates a new instance with all tokens initialized.
    fn new() -> Self {
        Self {
            // Drive type values
            acceleration: Token::new("acceleration"),
            force: Token::new("force"),

            // Degree of freedom tokens
            angular: Token::new("angular"),
            linear: Token::new("linear"),
            distance: Token::new("distance"),
            rot_x: Token::new("rotX"),
            rot_y: Token::new("rotY"),
            rot_z: Token::new("rotZ"),
            trans_x: Token::new("transX"),
            trans_y: Token::new("transY"),
            trans_z: Token::new("transZ"),

            // Mesh collision approximation values
            bounding_cube: Token::new("boundingCube"),
            bounding_sphere: Token::new("boundingSphere"),
            convex_decomposition: Token::new("convexDecomposition"),
            convex_hull: Token::new("convexHull"),
            mesh_simplification: Token::new("meshSimplification"),
            none: Token::new("none"),

            // Axis values
            x: Token::new("X"),
            y: Token::new("Y"),
            z: Token::new("Z"),

            // Collection name
            colliders: Token::new("colliders"),

            // Namespace prefixes
            drive: Token::new("drive"),
            limit: Token::new("limit"),

            // DriveAPI multiple-apply template properties
            drive_template_physics_damping: Token::new("drive:__INSTANCE_NAME__:physics:damping"),
            drive_template_physics_max_force: Token::new(
                "drive:__INSTANCE_NAME__:physics:maxForce",
            ),
            drive_template_physics_stiffness: Token::new(
                "drive:__INSTANCE_NAME__:physics:stiffness",
            ),
            drive_template_physics_target_position: Token::new(
                "drive:__INSTANCE_NAME__:physics:targetPosition",
            ),
            drive_template_physics_target_velocity: Token::new(
                "drive:__INSTANCE_NAME__:physics:targetVelocity",
            ),
            drive_template_physics_type: Token::new("drive:__INSTANCE_NAME__:physics:type"),

            // LimitAPI multiple-apply template properties
            limit_template_physics_high: Token::new("limit:__INSTANCE_NAME__:physics:high"),
            limit_template_physics_low: Token::new("limit:__INSTANCE_NAME__:physics:low"),

            // Stage metadata
            kilograms_per_unit: Token::new("kilogramsPerUnit"),

            // Physics attribute tokens
            physics_angular_velocity: Token::new("physics:angularVelocity"),
            physics_approximation: Token::new("physics:approximation"),
            physics_axis: Token::new("physics:axis"),
            physics_body0: Token::new("physics:body0"),
            physics_body1: Token::new("physics:body1"),
            physics_break_force: Token::new("physics:breakForce"),
            physics_break_torque: Token::new("physics:breakTorque"),
            physics_center_of_mass: Token::new("physics:centerOfMass"),
            physics_collision_enabled: Token::new("physics:collisionEnabled"),
            physics_cone_angle0_limit: Token::new("physics:coneAngle0Limit"),
            physics_cone_angle1_limit: Token::new("physics:coneAngle1Limit"),
            physics_density: Token::new("physics:density"),
            physics_diagonal_inertia: Token::new("physics:diagonalInertia"),
            physics_dynamic_friction: Token::new("physics:dynamicFriction"),
            physics_exclude_from_articulation: Token::new("physics:excludeFromArticulation"),
            physics_filtered_groups: Token::new("physics:filteredGroups"),
            physics_filtered_pairs: Token::new("physics:filteredPairs"),
            physics_gravity_direction: Token::new("physics:gravityDirection"),
            physics_gravity_magnitude: Token::new("physics:gravityMagnitude"),
            physics_invert_filtered_groups: Token::new("physics:invertFilteredGroups"),
            physics_joint_enabled: Token::new("physics:jointEnabled"),
            physics_kinematic_enabled: Token::new("physics:kinematicEnabled"),
            physics_local_pos0: Token::new("physics:localPos0"),
            physics_local_pos1: Token::new("physics:localPos1"),
            physics_local_rot0: Token::new("physics:localRot0"),
            physics_local_rot1: Token::new("physics:localRot1"),
            physics_lower_limit: Token::new("physics:lowerLimit"),
            physics_mass: Token::new("physics:mass"),
            physics_max_distance: Token::new("physics:maxDistance"),
            physics_merge_group: Token::new("physics:mergeGroup"),
            physics_min_distance: Token::new("physics:minDistance"),
            physics_principal_axes: Token::new("physics:principalAxes"),
            physics_restitution: Token::new("physics:restitution"),
            physics_rigid_body_enabled: Token::new("physics:rigidBodyEnabled"),
            physics_simulation_owner: Token::new("physics:simulationOwner"),
            physics_starts_asleep: Token::new("physics:startsAsleep"),
            physics_static_friction: Token::new("physics:staticFriction"),
            physics_upper_limit: Token::new("physics:upperLimit"),
            physics_velocity: Token::new("physics:velocity"),

            // Schema identifiers
            physics_articulation_root_api: Token::new("PhysicsArticulationRootAPI"),
            physics_collision_api: Token::new("PhysicsCollisionAPI"),
            physics_collision_group: Token::new("PhysicsCollisionGroup"),
            physics_distance_joint: Token::new("PhysicsDistanceJoint"),
            physics_drive_api: Token::new("PhysicsDriveAPI"),
            physics_filtered_pairs_api: Token::new("PhysicsFilteredPairsAPI"),
            physics_fixed_joint: Token::new("PhysicsFixedJoint"),
            physics_joint: Token::new("PhysicsJoint"),
            physics_limit_api: Token::new("PhysicsLimitAPI"),
            physics_mass_api: Token::new("PhysicsMassAPI"),
            physics_material_api: Token::new("PhysicsMaterialAPI"),
            physics_mesh_collision_api: Token::new("PhysicsMeshCollisionAPI"),
            physics_prismatic_joint: Token::new("PhysicsPrismaticJoint"),
            physics_revolute_joint: Token::new("PhysicsRevoluteJoint"),
            physics_rigid_body_api: Token::new("PhysicsRigidBodyAPI"),
            physics_scene: Token::new("PhysicsScene"),
            physics_spherical_joint: Token::new("PhysicsSphericalJoint"),
        }
    }

    /// Returns all tokens as a vector.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.acceleration.clone(),
            self.force.clone(),
            self.angular.clone(),
            self.linear.clone(),
            self.distance.clone(),
            self.rot_x.clone(),
            self.rot_y.clone(),
            self.rot_z.clone(),
            self.trans_x.clone(),
            self.trans_y.clone(),
            self.trans_z.clone(),
            self.bounding_cube.clone(),
            self.bounding_sphere.clone(),
            self.convex_decomposition.clone(),
            self.convex_hull.clone(),
            self.mesh_simplification.clone(),
            self.none.clone(),
            self.x.clone(),
            self.y.clone(),
            self.z.clone(),
            self.colliders.clone(),
            self.drive.clone(),
            self.limit.clone(),
            self.kilograms_per_unit.clone(),
            self.physics_angular_velocity.clone(),
            self.physics_approximation.clone(),
            self.physics_axis.clone(),
            self.physics_body0.clone(),
            self.physics_body1.clone(),
            self.physics_break_force.clone(),
            self.physics_break_torque.clone(),
            self.physics_center_of_mass.clone(),
            self.physics_collision_enabled.clone(),
            self.physics_cone_angle0_limit.clone(),
            self.physics_cone_angle1_limit.clone(),
            self.physics_density.clone(),
            self.physics_diagonal_inertia.clone(),
            self.physics_dynamic_friction.clone(),
            self.physics_exclude_from_articulation.clone(),
            self.physics_filtered_groups.clone(),
            self.physics_filtered_pairs.clone(),
            self.physics_gravity_direction.clone(),
            self.physics_gravity_magnitude.clone(),
            self.physics_invert_filtered_groups.clone(),
            self.physics_joint_enabled.clone(),
            self.physics_kinematic_enabled.clone(),
            self.physics_local_pos0.clone(),
            self.physics_local_pos1.clone(),
            self.physics_local_rot0.clone(),
            self.physics_local_rot1.clone(),
            self.physics_lower_limit.clone(),
            self.physics_mass.clone(),
            self.physics_max_distance.clone(),
            self.physics_merge_group.clone(),
            self.physics_min_distance.clone(),
            self.physics_principal_axes.clone(),
            self.physics_restitution.clone(),
            self.physics_rigid_body_enabled.clone(),
            self.physics_simulation_owner.clone(),
            self.physics_starts_asleep.clone(),
            self.physics_static_friction.clone(),
            self.physics_upper_limit.clone(),
            self.physics_velocity.clone(),
            self.physics_articulation_root_api.clone(),
            self.physics_collision_api.clone(),
            self.physics_collision_group.clone(),
            self.physics_distance_joint.clone(),
            self.physics_drive_api.clone(),
            self.physics_filtered_pairs_api.clone(),
            self.physics_fixed_joint.clone(),
            self.physics_joint.clone(),
            self.physics_limit_api.clone(),
            self.physics_mass_api.clone(),
            self.physics_material_api.clone(),
            self.physics_mesh_collision_api.clone(),
            self.physics_prismatic_joint.clone(),
            self.physics_revolute_joint.clone(),
            self.physics_rigid_body_api.clone(),
            self.physics_scene.clone(),
            self.physics_spherical_joint.clone(),
        ]
    }
}

/// Global static instance of UsdPhysics tokens.
///
/// Use this for efficient token access in all public API.
pub static USD_PHYSICS_TOKENS: LazyLock<UsdPhysicsTokensType> =
    LazyLock::new(UsdPhysicsTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens_exist() {
        assert_eq!(USD_PHYSICS_TOKENS.physics_mass.get_text(), "physics:mass");
        assert_eq!(USD_PHYSICS_TOKENS.x.get_text(), "X");
        assert_eq!(USD_PHYSICS_TOKENS.force.get_text(), "force");
    }

    #[test]
    fn test_schema_identifiers() {
        assert_eq!(
            USD_PHYSICS_TOKENS.physics_rigid_body_api.get_text(),
            "PhysicsRigidBodyAPI"
        );
        assert_eq!(
            USD_PHYSICS_TOKENS.physics_collision_api.get_text(),
            "PhysicsCollisionAPI"
        );
    }

    #[test]
    fn test_all_tokens() {
        let tokens = USD_PHYSICS_TOKENS.all_tokens();
        assert!(!tokens.is_empty());
        assert!(tokens.len() > 60); // Should have many tokens
    }
}
