//! Physics scene parsing utilities.
//!
//! Provides functions to parse USD physics content from a stage and report
//! parsed descriptors via callbacks. This is the main entry point for physics
//! engines to consume USD physics data.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/parseUtils.h` and `parseUtils.cpp`
//!
//! # Architecture
//!
//! The parsing system works as follows:
//! 1. Traverse specified prim ranges
//! 2. Identify physics prims (bodies, shapes, joints, etc.)
//! 3. Parse each prim into appropriate descriptor
//! 4. Report batches of descriptors via callback
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::parse_utils::*;
//!
//! // Define report callback
//! let report_fn = |obj_type, paths, descs, user_data| {
//!     match obj_type {
//!         ObjectType::RigidBody => { /* handle bodies */ }
//!         ObjectType::SphereShape => { /* handle spheres */ }
//!         _ => {}
//!     }
//! };
//!
//! // Parse physics from stage
//! load_physics_from_range(
//!     &stage,
//!     &[Path::from_string("/World")?],
//!     report_fn,
//!     VtValue::default(),
//!     None, None, None
//! )?;
//! ```

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use usd_core::{Prim, Stage};
use usd_geom::Xformable;
use usd_gf::{Quatf, Transform, Vec3f};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

use super::parse_desc::*;
use super::tokens::USD_PHYSICS_TOKENS;

/// Trait for types that contain a ShapeDesc, used for generic simulation owner filtering.
trait HasShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc;
}
impl HasShapeDesc for SphereShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
impl HasShapeDesc for CubeShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
impl HasShapeDesc for CapsuleShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
impl HasShapeDesc for CylinderShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
impl HasShapeDesc for ConeShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
impl HasShapeDesc for MeshShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
impl HasShapeDesc for PlaneShapeDesc {
    fn shape_desc(&self) -> &ShapeDesc {
        &self.shape
    }
}
use super::{
    ArticulationRootAPI, CollisionAPI, CollisionGroup, DistanceJoint, DriveAPI, FilteredPairsAPI,
    FixedJoint, Joint, MaterialAPI, MeshCollisionAPI, PrismaticJoint, RevoluteJoint, RigidBodyAPI,
    Scene, SphericalJoint,
};

// ============================================================================
// Constants
// ============================================================================

/// Sentinel value for infinity comparison.
const INF_SENTINEL: f32 = 0.5e38;

/// Default Earth gravity (m/s²).
const DEFAULT_GRAVITY: f32 = 9.81;

// ============================================================================
// Report Callback Types
// ============================================================================

/// Report function callback type.
///
/// Called after parsing to report batched physics data.
///
/// # Arguments
/// * `obj_type` - Type of parsed physics objects
/// * `prim_paths` - Paths of the reported prims
/// * `object_descs` - Corresponding object descriptors
/// * `user_data` - User-provided data from the parse call
pub type ReportFn = Box<dyn Fn(ObjectType, &[Path], &[ObjectDesc], &Value) + Send + Sync>;

/// Custom physics tokens for extending parsing.
///
/// Allows physics engines to define custom schema types that should
/// be recognized and reported during parsing.
#[derive(Debug, Clone, Default)]
pub struct CustomPhysicsTokens {
    /// Custom joint tokens to be reported
    pub joint_tokens: Vec<Token>,
    /// Custom shape tokens to be reported
    pub shape_tokens: Vec<Token>,
    /// Custom instancer tokens (subhierarchies are skipped)
    pub instancer_tokens: Vec<Token>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Parse filtered pairs from FilteredPairsAPI if applied.
fn parse_filtered_pairs(prim: &Prim, stage: &Arc<Stage>) -> Vec<Path> {
    let mut result = Vec::new();

    if let Some(api) = FilteredPairsAPI::get(stage, prim.get_path()) {
        if let Some(rel) = api.get_filtered_pairs_rel() {
            result = rel.get_targets();
        }
    }

    result
}

/// Get collision shape type based on geometry type.
fn get_collision_type(prim: &Prim, custom_tokens: Option<&[Token]>) -> (ObjectType, Option<Token>) {
    // Check for custom shapes first
    if let Some(tokens) = custom_tokens {
        let applied_apis = prim.get_applied_schemas();
        let prim_type = prim.get_type_name();

        for token in tokens {
            // Check applied APIs
            if applied_apis.iter().any(|api| api == token) {
                return (ObjectType::CustomShape, Some(token.clone()));
            }
            // Check prim type
            if &prim_type == token {
                return (ObjectType::CustomShape, Some(token.clone()));
            }
        }
    }

    // Determine type from geometry prim type.
    // C++ uses IsA<UsdGeomX>() hierarchy checks; we match type names.
    let prim_type = prim.get_type_name();
    if prim_type == Token::new("Mesh") {
        (ObjectType::MeshShape, None)
    } else if prim_type == Token::new("Cube") {
        (ObjectType::CubeShape, None)
    } else if prim_type == Token::new("Sphere") {
        (ObjectType::SphereShape, None)
    } else if prim_type == Token::new("Capsule") {
        (ObjectType::CapsuleShape, None)
    } else if prim_type == Token::new("Capsule_1") {
        (ObjectType::Capsule1Shape, None)
    } else if prim_type == Token::new("Cylinder") {
        (ObjectType::CylinderShape, None)
    } else if prim_type == Token::new("Cylinder_1") {
        (ObjectType::Cylinder1Shape, None)
    } else if prim_type == Token::new("Cone") {
        (ObjectType::ConeShape, None)
    } else if prim_type == Token::new("Plane") {
        (ObjectType::PlaneShape, None)
    } else if prim_type == Token::new("Points") {
        (ObjectType::SpherePointsShape, None)
    } else {
        (ObjectType::Undefined, None)
    }
}

/// Parse axis token to enum.
fn parse_axis(token: &Token) -> Axis {
    if token == &USD_PHYSICS_TOKENS.y {
        Axis::Y
    } else if token == &USD_PHYSICS_TOKENS.z {
        Axis::Z
    } else {
        Axis::X
    }
}

/// Check if a limit value is finite and within valid range.
fn is_valid_limit(value: f32) -> bool {
    value.is_finite() && value.abs() < INF_SENTINEL
}

/// Finalize a collision shape descriptor with material, filtered pairs,
/// collision enabled flag and simulation owners.
/// C++ equivalent: `_FinalizeCollisionDesc`.
fn finalize_collision_desc(prim: &Prim, stage: &Arc<Stage>, desc: &mut ShapeDesc) {
    desc.base.prim_path = prim.get_path().clone();

    // Extract local transform (position, rotation, scale)
    let xformable = Xformable::new(prim.clone());
    let matrix = xformable.get_local_transformation(TimeCode::default());
    let xform = Transform::from_matrix(&matrix);

    let trans = xform.translation();
    desc.local_pos = Vec3f::new(trans.x as f32, trans.y as f32, trans.z as f32);

    let quat_d = xform.rotation().get_quat();
    desc.local_rot = Quatf::new(
        quat_d.real() as f32,
        usd_gf::Vec3f::new(
            quat_d.imaginary().x as f32,
            quat_d.imaginary().y as f32,
            quat_d.imaginary().z as f32,
        ),
    );

    let scale = xform.scale();
    desc.local_scale = Vec3f::new(scale.x as f32, scale.y as f32, scale.z as f32);

    // Gather filtered pairs
    desc.filtered_collisions = parse_filtered_pairs(prim, stage);

    // Collision enabled
    if let Some(col_api) = CollisionAPI::get(stage, prim.get_path()) {
        if let Some(attr) = col_api.get_collision_enabled_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<bool>())
            {
                desc.collision_enabled = val;
            }
        }

        // Simulation owner
        if let Some(rel) = col_api.get_simulation_owner_rel() {
            desc.simulation_owners = rel.get_targets();
        }
    }
}

/// Parse axis, radius, and half-height from a prim with those attributes.
/// Used for Capsule, Cylinder, and Cone shapes.
/// C++ equivalent: `_GetAxisRadiusHalfHeight`.
fn parse_axis_radius_height(prim: &Prim, axis: &mut Axis, radius: &mut f32, half_height: &mut f32) {
    // Read radius
    if let Some(attr) = prim.get_attribute("radius") {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f64>())
        {
            *radius = val as f32;
        }
    }

    // Read height -> half_height
    if let Some(attr) = prim.get_attribute("height") {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f64>())
        {
            *half_height = (val as f32) * 0.5;
        }
    }

    // Read axis
    if let Some(attr) = prim.get_attribute("axis") {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Token>())
        {
            *axis = parse_axis(&val);
        }
    }
}

// ============================================================================
// Scene Parsing
// ============================================================================

/// Parse scene descriptor.
fn parse_scene_desc(scene: &Scene, stage: &Arc<Stage>) -> Option<SceneDesc> {
    let mut desc = SceneDesc::default();
    desc.base.prim_path = scene.get_prim().get_path().clone();

    // Gravity direction
    let mut gravity_dir = Vec3f::zero();
    if let Some(attr) = scene.get_gravity_direction_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Vec3f>())
        {
            gravity_dir = val;
        }
    }

    // Default to negative up axis if not specified
    if gravity_dir == Vec3f::zero() {
        // Get stage up axis (default Y)
        gravity_dir = Vec3f::new(0.0, -1.0, 0.0);
    } else {
        gravity_dir = gravity_dir.normalized();
    }
    desc.gravity_direction = gravity_dir;

    // Gravity magnitude
    let mut gravity_mag = f32::NEG_INFINITY;
    if let Some(attr) = scene.get_gravity_magnitude_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            gravity_mag = val;
        }
    }

    // Default to Earth gravity adjusted for stage units
    if gravity_mag < -INF_SENTINEL {
        let meters_per_unit = usd_geom::get_stage_meters_per_unit(stage);
        gravity_mag = DEFAULT_GRAVITY / meters_per_unit as f32;
    }
    desc.gravity_magnitude = gravity_mag;

    Some(desc)
}

// ============================================================================
// Collision Group Parsing
// ============================================================================

/// Parse collision group descriptor.
fn parse_collision_group_desc(group: &CollisionGroup) -> Option<CollisionGroupDesc> {
    let mut desc = CollisionGroupDesc::new();
    desc.base.prim_path = group.get_prim().get_path().clone();

    // Filtered groups
    if let Some(rel) = group.get_filtered_groups_rel() {
        desc.filtered_groups = rel.get_targets();
    }

    // Invert flag
    if let Some(attr) = group.get_invert_filtered_groups_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.invert_filtered_groups = val;
        }
    }

    // Merge group name
    if let Some(attr) = group.get_merge_group_name_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<String>())
        {
            desc.merge_group_name = val;
        }
    }

    Some(desc)
}

// ============================================================================
// Rigid Body Parsing
// ============================================================================

/// Parse rigid body descriptor.
fn parse_rigid_body_desc(api: &RigidBodyAPI, stage: &Arc<Stage>) -> Option<RigidBodyDesc> {
    let mut desc = RigidBodyDesc::default();
    let prim = api.get_prim();
    desc.base.prim_path = prim.get_path().clone();

    // Get world transform and decompose into TRS
    let xformable = Xformable::new(prim.clone());
    let matrix = xformable.get_local_transformation(TimeCode::default());
    let xform = Transform::from_matrix(&matrix);

    // Extract translation (Vec3d -> Vec3f)
    let trans = xform.translation();
    desc.position = Vec3f::new(trans.x as f32, trans.y as f32, trans.z as f32);

    // Extract rotation (Rotation -> Quatd -> Quatf)
    let quat_d = xform.rotation().get_quat();
    desc.rotation = Quatf::new(
        quat_d.real() as f32,
        usd_gf::Vec3f::new(
            quat_d.imaginary().x as f32,
            quat_d.imaginary().y as f32,
            quat_d.imaginary().z as f32,
        ),
    );

    // Extract scale (Vec3d -> Vec3f)
    let scale = xform.scale();
    desc.scale = Vec3f::new(scale.x as f32, scale.y as f32, scale.z as f32);

    // Parse filtered pairs
    desc.filtered_collisions = parse_filtered_pairs(prim, stage);

    // Velocity
    if let Some(attr) = api.get_velocity_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Vec3f>())
        {
            desc.linear_velocity = val;
        }
    }

    // Angular velocity
    if let Some(attr) = api.get_angular_velocity_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Vec3f>())
        {
            desc.angular_velocity = val;
        }
    }

    // Flags
    if let Some(attr) = api.get_rigid_body_enabled_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.rigid_body_enabled = val;
        }
    }

    if let Some(attr) = api.get_kinematic_enabled_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.kinematic_body = val;
        }
    }

    if let Some(attr) = api.get_starts_asleep_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.starts_asleep = val;
        }
    }

    // Simulation owner
    if let Some(rel) = api.get_simulation_owner_rel() {
        desc.simulation_owners = rel.get_targets();
    }

    Some(desc)
}

// ============================================================================
// Material Parsing
// ============================================================================

/// Parse material descriptor.
fn parse_material_desc(api: &MaterialAPI) -> Option<RigidBodyMaterialDesc> {
    let mut desc = RigidBodyMaterialDesc::default();
    desc.base.prim_path = api.get_prim().get_path().clone();

    if let Some(attr) = api.get_static_friction_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            desc.static_friction = val;
        }
    }

    if let Some(attr) = api.get_dynamic_friction_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            desc.dynamic_friction = val;
        }
    }

    if let Some(attr) = api.get_restitution_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            desc.restitution = val;
        }
    }

    if let Some(attr) = api.get_density_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            desc.density = val;
        }
    }

    Some(desc)
}

// ============================================================================
// Joint Parsing Helpers
// ============================================================================

/// Parse common joint properties.
fn parse_common_joint_desc(joint: &impl std::ops::Deref<Target = Joint>, desc: &mut JointDesc) {
    let prim = joint.get_prim();
    desc.base.prim_path = prim.get_path().clone();

    // Body relationships
    if let Some(rel) = joint.get_body0_rel() {
        let targets = rel.get_targets();
        if let Some(path) = targets.first() {
            desc.rel0 = path.clone();
            desc.body0 = path.clone();
        }
    }

    if let Some(rel) = joint.get_body1_rel() {
        let targets = rel.get_targets();
        if let Some(path) = targets.first() {
            desc.rel1 = path.clone();
            desc.body1 = path.clone();
        }
    }

    // Local poses
    if let Some(attr) = joint.get_local_pos0_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Vec3f>())
        {
            desc.local_pose0_position = val;
        }
    }

    if let Some(attr) = joint.get_local_rot0_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Quatf>())
        {
            desc.local_pose0_orientation = val;
        }
    }

    if let Some(attr) = joint.get_local_pos1_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Vec3f>())
        {
            desc.local_pose1_position = val;
        }
    }

    if let Some(attr) = joint.get_local_rot1_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Quatf>())
        {
            desc.local_pose1_orientation = val;
        }
    }

    // Joint enabled
    if let Some(attr) = joint.get_joint_enabled_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.joint_enabled = val;
        }
    }

    // Collision enabled
    if let Some(attr) = joint.get_collision_enabled_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.collision_enabled = val;
        }
    }

    // Break force/torque
    if let Some(attr) = joint.get_break_force_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            desc.break_force = val;
        }
    }

    if let Some(attr) = joint.get_break_torque_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            desc.break_torque = val;
        }
    }

    // Exclude from articulation
    if let Some(attr) = joint.get_exclude_from_articulation_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<bool>())
        {
            desc.exclude_from_articulation = val;
        }
    }
}

/// Parse angular drive from prim.
fn parse_angular_drive(prim: &Prim) -> JointDrive {
    let mut drive = JointDrive::default();

    if let Some(api) = DriveAPI::get_from_prim(prim, &USD_PHYSICS_TOKENS.angular) {
        drive.enabled = true;

        if let Some(attr) = api.get_target_position_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.target_position = val;
            }
        }

        if let Some(attr) = api.get_target_velocity_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.target_velocity = val;
            }
        }

        if let Some(attr) = api.get_max_force_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.force_limit = val;
            }
        }

        if let Some(attr) = api.get_stiffness_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.stiffness = val;
            }
        }

        if let Some(attr) = api.get_damping_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.damping = val;
            }
        }

        if let Some(attr) = api.get_type_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<Token>())
            {
                drive.acceleration = val == USD_PHYSICS_TOKENS.acceleration;
            }
        }
    }

    drive
}

/// Parse linear drive from prim.
fn parse_linear_drive(prim: &Prim) -> JointDrive {
    let mut drive = JointDrive::default();

    if let Some(api) = DriveAPI::get_from_prim(prim, &USD_PHYSICS_TOKENS.linear) {
        drive.enabled = true;

        if let Some(attr) = api.get_target_position_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.target_position = val;
            }
        }

        if let Some(attr) = api.get_target_velocity_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.target_velocity = val;
            }
        }

        if let Some(attr) = api.get_max_force_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.force_limit = val;
            }
        }

        if let Some(attr) = api.get_stiffness_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.stiffness = val;
            }
        }

        if let Some(attr) = api.get_damping_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<f32>())
            {
                drive.damping = val;
            }
        }

        if let Some(attr) = api.get_type_attr() {
            if let Some(val) = attr
                .get(TimeCode::default())
                .and_then(|v| v.downcast_clone::<Token>())
            {
                drive.acceleration = val == USD_PHYSICS_TOKENS.acceleration;
            }
        }
    }

    drive
}

// ============================================================================
// Joint Parsing
// ============================================================================

/// Parse fixed joint descriptor.
fn parse_fixed_joint_desc(joint: &FixedJoint) -> Option<FixedJointDesc> {
    let mut desc = FixedJointDesc::default();
    parse_common_joint_desc(joint, &mut desc.joint);
    Some(desc)
}

/// Parse revolute joint descriptor.
fn parse_revolute_joint_desc(joint: &RevoluteJoint) -> Option<RevoluteJointDesc> {
    let mut desc = RevoluteJointDesc::default();
    parse_common_joint_desc(joint, &mut desc.joint);

    // Axis
    if let Some(attr) = joint.get_axis_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Token>())
        {
            desc.axis = parse_axis(&val);
        }
    }

    // Limits
    let mut lower = f32::NEG_INFINITY;
    let mut upper = f32::INFINITY;

    if let Some(attr) = joint.get_lower_limit_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            lower = val;
        }
    }

    if let Some(attr) = joint.get_upper_limit_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            upper = val;
        }
    }

    if is_valid_limit(lower) && is_valid_limit(upper) {
        desc.limit = JointLimit::new(lower, upper);
    }

    // Drive
    desc.drive = parse_angular_drive(joint.get_prim());

    Some(desc)
}

/// Parse prismatic joint descriptor.
fn parse_prismatic_joint_desc(joint: &PrismaticJoint) -> Option<PrismaticJointDesc> {
    let mut desc = PrismaticJointDesc::default();
    parse_common_joint_desc(joint, &mut desc.joint);

    // Axis
    if let Some(attr) = joint.get_axis_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Token>())
        {
            desc.axis = parse_axis(&val);
        }
    }

    // Limits
    let mut lower = f32::NEG_INFINITY;
    let mut upper = f32::INFINITY;

    if let Some(attr) = joint.get_lower_limit_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            lower = val;
        }
    }

    if let Some(attr) = joint.get_upper_limit_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            upper = val;
        }
    }

    if is_valid_limit(lower) && is_valid_limit(upper) {
        desc.limit = JointLimit::new(lower, upper);
    }

    // Drive
    desc.drive = parse_linear_drive(joint.get_prim());

    Some(desc)
}

/// Parse spherical joint descriptor.
fn parse_spherical_joint_desc(joint: &SphericalJoint) -> Option<SphericalJointDesc> {
    let mut desc = SphericalJointDesc::default();
    parse_common_joint_desc(joint, &mut desc.joint);

    // Axis
    if let Some(attr) = joint.get_axis_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<Token>())
        {
            desc.axis = parse_axis(&val);
        }
    }

    // Cone limits
    let mut angle0 = f32::INFINITY;
    let mut angle1 = f32::INFINITY;

    if let Some(attr) = joint.get_cone_angle0_limit_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            angle0 = val;
        }
    }

    if let Some(attr) = joint.get_cone_angle1_limit_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            angle1 = val;
        }
    }

    if angle0.is_finite() && angle1.is_finite() && angle0 >= 0.0 && angle1 >= 0.0 {
        desc.limit = JointLimit {
            enabled: true,
            lower: angle0,
            upper: angle1,
        };
    }

    Some(desc)
}

/// Parse distance joint descriptor.
fn parse_distance_joint_desc(joint: &DistanceJoint) -> Option<DistanceJointDesc> {
    let mut desc = DistanceJointDesc::default();
    parse_common_joint_desc(joint, &mut desc.joint);

    // Min distance
    if let Some(attr) = joint.get_min_distance_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            if val >= 0.0 {
                desc.min_enabled = true;
                desc.limit.lower = val;
            }
        }
    }

    // Max distance
    if let Some(attr) = joint.get_max_distance_attr() {
        if let Some(val) = attr
            .get(TimeCode::default())
            .and_then(|v| v.downcast_clone::<f32>())
        {
            if (0.0..INF_SENTINEL).contains(&val) {
                desc.max_enabled = true;
                desc.limit.upper = val;
            }
        }
    }

    desc.limit.enabled = desc.min_enabled || desc.max_enabled;

    Some(desc)
}

// ============================================================================
// Articulation Parsing
// ============================================================================

/// Parse articulation descriptor.
fn parse_articulation_desc(
    api: &ArticulationRootAPI,
    stage: &Arc<Stage>,
) -> Option<ArticulationDesc> {
    let mut desc = ArticulationDesc::new();
    desc.base.prim_path = api.get_prim().get_path().clone();
    desc.filtered_collisions = parse_filtered_pairs(api.get_prim(), stage);
    Some(desc)
}

// ============================================================================
// Main Parsing Function
// ============================================================================

/// Parsed physics data container.
///
/// Holds all parsed descriptors organized by type.
/// Shape-specific descriptors are stored in typed vectors matching
/// the C++ parse output structure.
#[derive(Default)]
pub struct ParsedPhysicsData {
    /// Scene descriptors
    pub scenes: Vec<(Path, SceneDesc)>,
    /// Collision group descriptors
    pub collision_groups: Vec<(Path, CollisionGroupDesc)>,
    /// Rigid body descriptors
    pub rigid_bodies: Vec<(Path, RigidBodyDesc)>,
    /// Material descriptors
    pub materials: Vec<(Path, RigidBodyMaterialDesc)>,
    /// Fixed joint descriptors
    pub fixed_joints: Vec<(Path, FixedJointDesc)>,
    /// Revolute joint descriptors
    pub revolute_joints: Vec<(Path, RevoluteJointDesc)>,
    /// Prismatic joint descriptors
    pub prismatic_joints: Vec<(Path, PrismaticJointDesc)>,
    /// Spherical joint descriptors
    pub spherical_joints: Vec<(Path, SphericalJointDesc)>,
    /// Distance joint descriptors
    pub distance_joints: Vec<(Path, DistanceJointDesc)>,
    /// Articulation descriptors
    pub articulations: Vec<(Path, ArticulationDesc)>,
    /// Generic shape descriptors (for Capsule1, Cylinder1, custom, etc.)
    pub shapes: Vec<(Path, ObjectType, ShapeDesc)>,
    /// Sphere shape descriptors with radius
    pub sphere_shapes: Vec<(Path, SphereShapeDesc)>,
    /// Cube shape descriptors with half-extents
    pub cube_shapes: Vec<(Path, CubeShapeDesc)>,
    /// Capsule shape descriptors with radius, half-height, axis
    pub capsule_shapes: Vec<(Path, CapsuleShapeDesc)>,
    /// Cylinder shape descriptors with radius, half-height, axis
    pub cylinder_shapes: Vec<(Path, CylinderShapeDesc)>,
    /// Cone shape descriptors with radius, half-height, axis
    pub cone_shapes: Vec<(Path, ConeShapeDesc)>,
    /// Mesh shape descriptors with approximation and scale
    pub mesh_shapes: Vec<(Path, MeshShapeDesc)>,
    /// Plane shape descriptors
    pub plane_shapes: Vec<(Path, PlaneShapeDesc)>,
}

/// Load and parse USD physics from specified paths.
///
/// Traverses the stage from given include paths, parses all physics prims,
/// and reports the results via the callback function.
///
/// # Arguments
/// * `stage` - Stage to traverse and parse
/// * `include_paths` - Paths to start traversal from
/// * `report_fn` - Callback to receive parsed physics data
/// * `user_data` - User data passed to the callback
/// * `exclude_paths` - Optional paths to exclude from parsing
/// * `custom_tokens` - Optional custom physics tokens
/// * `simulation_owners` - Optional simulation owner filter
///
/// # Returns
/// True if parsing was successful
pub fn load_physics_from_range(
    stage: &Arc<Stage>,
    include_paths: &[Path],
    report_fn: impl Fn(ObjectType, &[Path], &[ObjectDesc], &Value),
    user_data: &Value,
    exclude_paths: Option<&[Path]>,
    custom_tokens: Option<&CustomPhysicsTokens>,
    simulation_owners: Option<&[Path]>,
) -> bool {
    if include_paths.is_empty() {
        return false;
    }

    // Build exclude set
    let exclude_set: HashSet<&Path> = exclude_paths
        .map(|paths| paths.iter().collect())
        .unwrap_or_default();

    // Build simulation owner set
    let mut _sim_owner_set: HashSet<&Path> = HashSet::new();
    let mut _default_sim_owner = false;
    if let Some(owners) = simulation_owners {
        for owner in owners {
            if owner.is_empty() {
                _default_sim_owner = true;
            } else {
                _sim_owner_set.insert(owner);
            }
        }
    }

    let mut parsed = ParsedPhysicsData::default();

    // Traverse each include path
    for include_path in include_paths {
        // Traverse subtree
        for prim in stage.traverse_from(
            include_path,
            usd_core::prim_flags::PrimFlagsPredicate::default(),
        ) {
            let prim_path = prim.get_path();

            // Skip excluded paths
            if exclude_set.contains(&prim_path) {
                continue;
            }

            // Check for physics schemas
            parse_prim_physics(&prim, stage, custom_tokens, &mut parsed);
        }
    }

    // Report parsed data
    report_parsed_data(&parsed, &report_fn, user_data);

    true
}

/// Collect all parsed physics data from stage into a typed container.
///
/// This is the Rust equivalent of the Python `LoadUsdPhysicsFromRange`
/// which returns a dict of typed descriptors. Unlike the callback-based
/// `load_physics_from_range`, this returns `ParsedPhysicsData` directly.
pub fn collect_physics_from_range(
    stage: &Arc<Stage>,
    include_paths: &[Path],
    exclude_paths: Option<&[Path]>,
    custom_tokens: Option<&CustomPhysicsTokens>,
    simulation_owners: Option<&[Path]>,
) -> ParsedPhysicsData {
    if include_paths.is_empty() {
        return ParsedPhysicsData::default();
    }

    let exclude_set: HashSet<&Path> = exclude_paths
        .map(|paths| paths.iter().collect())
        .unwrap_or_default();

    let mut parsed = ParsedPhysicsData::default();

    for include_path in include_paths {
        for prim in stage.traverse_from(
            include_path,
            usd_core::prim_flags::PrimFlagsPredicate::default(),
        ) {
            let prim_path = prim.get_path();
            if exclude_set.contains(&prim_path) {
                continue;
            }
            parse_prim_physics(&prim, stage, custom_tokens, &mut parsed);
        }
    }

    // Second pass: link collision shapes to rigid bodies
    // Build body path set for lookup
    let body_paths: HashSet<String> = parsed
        .rigid_bodies
        .iter()
        .map(|(p, _)| p.to_string())
        .collect();

    // Helper: find the nearest ancestor rigid body for a given path
    fn find_rigid_body(shape_path: &Path, body_paths: &HashSet<String>) -> Option<Path> {
        let mut current = shape_path.get_parent_path();
        while !current.is_empty() {
            if body_paths.contains(&current.to_string()) {
                return Some(current);
            }
            current = current.get_parent_path();
        }
        None
    }

    // Collect all shape paths and their rigid body associations
    let mut shape_to_body: Vec<(String, Path)> = Vec::new();

    for (path, desc) in &mut parsed.sphere_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }
    for (path, desc) in &mut parsed.cube_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }
    for (path, desc) in &mut parsed.capsule_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }
    for (path, desc) in &mut parsed.cylinder_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }
    for (path, desc) in &mut parsed.cone_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }
    for (path, desc) in &mut parsed.mesh_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }
    for (path, desc) in &mut parsed.plane_shapes {
        if let Some(body_path) = find_rigid_body(path, &body_paths) {
            desc.shape.rigid_body = body_path.clone();
            shape_to_body.push((body_path.to_string(), path.clone()));
        }
    }

    // Add collision paths to rigid body descriptors
    for (body_path_str, shape_path) in &shape_to_body {
        for (rb_path, rb_desc) in &mut parsed.rigid_bodies {
            if rb_path.to_string() == *body_path_str {
                rb_desc.collisions.push(shape_path.clone());
            }
        }
    }

    // Third pass: collision group membership resolution
    // For each collision group, check if shapes are in its colliders collection
    let cg_includes: Vec<(String, Vec<Path>)> = parsed
        .collision_groups
        .iter()
        .map(|(cg_path, _)| {
            let cg_path_str = cg_path.to_string();
            let mut includes = Vec::new();
            if let Some(cg) = CollisionGroup::get(stage, cg_path) {
                if let Some(collection_api) = cg.get_colliders_collection_api() {
                    if let Some(includes_rel) = collection_api.get_includes_rel() {
                        includes = includes_rel.get_targets();
                    }
                }
            }
            (cg_path_str, includes)
        })
        .collect();

    // Helper: assign collision groups to a shape
    fn assign_collision_groups(
        shape_path: &Path,
        shape_cg: &mut Vec<Path>,
        cg_includes: &[(String, Vec<Path>)],
    ) {
        for (cg_path_str, includes) in cg_includes {
            if includes
                .iter()
                .any(|inc| inc.to_string() == shape_path.to_string())
            {
                shape_cg.push(Path::from_string(cg_path_str).unwrap_or_default());
            }
        }
    }

    for (path, desc) in &mut parsed.sphere_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }
    for (path, desc) in &mut parsed.cube_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }
    for (path, desc) in &mut parsed.capsule_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }
    for (path, desc) in &mut parsed.cylinder_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }
    for (path, desc) in &mut parsed.cone_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }
    for (path, desc) in &mut parsed.mesh_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }
    for (path, desc) in &mut parsed.plane_shapes {
        assign_collision_groups(path, &mut desc.shape.collision_groups, &cg_includes);
    }

    // Fourth pass: collision group merge
    // Groups with same mergeGroupName are merged into one representative.
    // First pass: determine representative and collect merge actions
    let mut merge_map: HashMap<String, usize> = HashMap::new();
    let mut merged_indices: Vec<bool> = vec![false; parsed.collision_groups.len()];
    // (representative_idx, merged_path, merged_filters)
    let mut merge_actions: Vec<(usize, Path, Vec<Path>)> = Vec::new();
    for (idx, (_path, desc)) in parsed.collision_groups.iter().enumerate() {
        if !desc.merge_group_name.is_empty() {
            if let Some(&representative) = merge_map.get(&desc.merge_group_name) {
                merged_indices[idx] = true;
                merge_actions.push((
                    representative,
                    parsed.collision_groups[idx].0.clone(),
                    parsed.collision_groups[idx].1.filtered_groups.clone(),
                ));
            } else {
                merge_map.insert(desc.merge_group_name.clone(), idx);
            }
        }
    }
    // Apply merge actions
    for (rep_idx, merged_path, merged_filters) in merge_actions {
        parsed.collision_groups[rep_idx]
            .1
            .merged_groups
            .push(merged_path);
        for fg in merged_filters {
            if !parsed.collision_groups[rep_idx]
                .1
                .filtered_groups
                .contains(&fg)
            {
                parsed.collision_groups[rep_idx].1.filtered_groups.push(fg);
            }
        }
    }
    // Remove merged groups (reverse to preserve indices)
    let mut idx = parsed.collision_groups.len();
    while idx > 0 {
        idx -= 1;
        if merged_indices[idx] {
            parsed.collision_groups.remove(idx);
        }
    }

    // Fifth pass: simulation owner filtering
    if let Some(owners) = simulation_owners {
        let mut sim_owner_set: HashSet<String> = HashSet::new();
        let mut default_sim_owner = false;
        for owner in owners {
            if owner.is_empty() {
                default_sim_owner = true;
            } else {
                sim_owner_set.insert(owner.to_string());
            }
        }

        // Filter scenes: scene is reported only if its path is in sim_owner_set.
        // Default owner (empty path) does NOT include scenes.
        parsed
            .scenes
            .retain(|(path, _)| sim_owner_set.contains(&path.to_string()));

        // Filter rigid bodies
        let reported_bodies: HashSet<String> = {
            let mut set = HashSet::new();
            parsed.rigid_bodies.retain(|(_, desc)| {
                let keep = if desc.simulation_owners.is_empty() {
                    default_sim_owner
                } else {
                    desc.simulation_owners
                        .iter()
                        .any(|o| sim_owner_set.contains(&o.to_string()))
                };
                if keep {
                    set.insert(desc.base.prim_path.to_string());
                }
                keep
            });
            set
        };

        // Filter collision shapes: keep if body is reported, or if standalone with matching owner
        fn filter_shapes<T: HasShapeDesc>(
            shapes: &mut Vec<(Path, T)>,
            reported_bodies: &HashSet<String>,
            default_sim_owner: bool,
            sim_owner_set: &HashSet<String>,
        ) {
            shapes.retain(|(_, desc)| {
                let sd = desc.shape_desc();
                if !sd.rigid_body.is_empty() {
                    reported_bodies.contains(&sd.rigid_body.to_string())
                } else if sd.simulation_owners.is_empty() {
                    default_sim_owner
                } else {
                    sd.simulation_owners
                        .iter()
                        .any(|o| sim_owner_set.contains(&o.to_string()))
                }
            });
        }

        filter_shapes(
            &mut parsed.sphere_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );
        filter_shapes(
            &mut parsed.cube_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );
        filter_shapes(
            &mut parsed.capsule_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );
        filter_shapes(
            &mut parsed.cylinder_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );
        filter_shapes(
            &mut parsed.cone_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );
        filter_shapes(
            &mut parsed.mesh_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );
        filter_shapes(
            &mut parsed.plane_shapes,
            &reported_bodies,
            default_sim_owner,
            &sim_owner_set,
        );

        // Filter joints: keep if at least one body is reported
        parsed.fixed_joints.retain(|(_, desc)| {
            let b0 = !desc.joint.body0.is_empty()
                && reported_bodies.contains(&desc.joint.body0.to_string());
            let b1 = !desc.joint.body1.is_empty()
                && reported_bodies.contains(&desc.joint.body1.to_string());
            b0 || b1
        });
        parsed.revolute_joints.retain(|(_, desc)| {
            let b0 = !desc.joint.body0.is_empty()
                && reported_bodies.contains(&desc.joint.body0.to_string());
            let b1 = !desc.joint.body1.is_empty()
                && reported_bodies.contains(&desc.joint.body1.to_string());
            b0 || b1
        });
        parsed.prismatic_joints.retain(|(_, desc)| {
            let b0 = !desc.joint.body0.is_empty()
                && reported_bodies.contains(&desc.joint.body0.to_string());
            let b1 = !desc.joint.body1.is_empty()
                && reported_bodies.contains(&desc.joint.body1.to_string());
            b0 || b1
        });
        parsed.spherical_joints.retain(|(_, desc)| {
            let b0 = !desc.joint.body0.is_empty()
                && reported_bodies.contains(&desc.joint.body0.to_string());
            let b1 = !desc.joint.body1.is_empty()
                && reported_bodies.contains(&desc.joint.body1.to_string());
            b0 || b1
        });
        parsed.distance_joints.retain(|(_, desc)| {
            let b0 = !desc.joint.body0.is_empty()
                && reported_bodies.contains(&desc.joint.body0.to_string());
            let b1 = !desc.joint.body1.is_empty()
                && reported_bodies.contains(&desc.joint.body1.to_string());
            b0 || b1
        });

        // Filter articulations: keep if at least one body is reported,
        // or if articulation has no bodies listed (keep by default)
        parsed.articulations.retain(|(_, desc)| {
            if desc.articulated_bodies.is_empty() {
                // No body info — keep if default owner
                default_sim_owner
            } else {
                desc.articulated_bodies
                    .iter()
                    .any(|b| reported_bodies.contains(&b.to_string()))
            }
        });
    }

    parsed
}

/// Parse physics from a single prim.
fn parse_prim_physics(
    prim: &Prim,
    stage: &Arc<Stage>,
    custom_tokens: Option<&CustomPhysicsTokens>,
    parsed: &mut ParsedPhysicsData,
) {
    let path = prim.get_path().clone();

    // Check for Scene
    if prim.get_type_name() == Token::new(Scene::SCHEMA_TYPE_NAME) {
        if let Some(scene) = Scene::get(stage, &path) {
            if let Some(desc) = parse_scene_desc(&scene, stage) {
                parsed.scenes.push((path.clone(), desc));
            }
        }
    }

    // Check for CollisionGroup
    if prim.get_type_name() == Token::new(CollisionGroup::SCHEMA_TYPE_NAME) {
        if let Some(group) = CollisionGroup::get(stage, &path) {
            if let Some(desc) = parse_collision_group_desc(&group) {
                parsed.collision_groups.push((path.clone(), desc));
            }
        }
    }

    // Check for RigidBodyAPI
    if prim.has_api(&Token::new(RigidBodyAPI::SCHEMA_TYPE_NAME)) {
        if let Some(api) = RigidBodyAPI::get(stage, &path) {
            if let Some(desc) = parse_rigid_body_desc(&api, stage) {
                parsed.rigid_bodies.push((path.clone(), desc));
            }
        }
    }

    // Check for MaterialAPI
    if prim.has_api(&Token::new(MaterialAPI::SCHEMA_TYPE_NAME)) {
        if let Some(api) = MaterialAPI::get(stage, &path) {
            if let Some(desc) = parse_material_desc(&api) {
                parsed.materials.push((path.clone(), desc));
            }
        }
    }

    // Check for ArticulationRootAPI
    if prim.has_api(&Token::new(ArticulationRootAPI::SCHEMA_TYPE_NAME)) {
        if let Some(api) = ArticulationRootAPI::get(stage, &path) {
            if let Some(desc) = parse_articulation_desc(&api, stage) {
                parsed.articulations.push((path.clone(), desc));
            }
        }
    }

    // Check for joints
    let prim_type = prim.get_type_name();
    if prim_type == Token::new(FixedJoint::SCHEMA_TYPE_NAME) {
        if let Some(joint) = FixedJoint::get(stage, &path) {
            if let Some(desc) = parse_fixed_joint_desc(&joint) {
                parsed.fixed_joints.push((path.clone(), desc));
            }
        }
    } else if prim_type == Token::new(RevoluteJoint::SCHEMA_TYPE_NAME) {
        if let Some(joint) = RevoluteJoint::get(stage, &path) {
            if let Some(desc) = parse_revolute_joint_desc(&joint) {
                parsed.revolute_joints.push((path.clone(), desc));
            }
        }
    } else if prim_type == Token::new(PrismaticJoint::SCHEMA_TYPE_NAME) {
        if let Some(joint) = PrismaticJoint::get(stage, &path) {
            if let Some(desc) = parse_prismatic_joint_desc(&joint) {
                parsed.prismatic_joints.push((path.clone(), desc));
            }
        }
    } else if prim_type == Token::new(SphericalJoint::SCHEMA_TYPE_NAME) {
        if let Some(joint) = SphericalJoint::get(stage, &path) {
            if let Some(desc) = parse_spherical_joint_desc(&joint) {
                parsed.spherical_joints.push((path.clone(), desc));
            }
        }
    } else if prim_type == Token::new(DistanceJoint::SCHEMA_TYPE_NAME) {
        if let Some(joint) = DistanceJoint::get(stage, &path) {
            if let Some(desc) = parse_distance_joint_desc(&joint) {
                parsed.distance_joints.push((path.clone(), desc));
            }
        }
    }

    // Check for CollisionAPI (shapes)
    if prim.has_api(&Token::new(CollisionAPI::SCHEMA_TYPE_NAME)) {
        let custom_shape_tokens = custom_tokens.map(|ct| ct.shape_tokens.as_slice());
        let (obj_type, _custom_token) = get_collision_type(prim, custom_shape_tokens);

        if obj_type != ObjectType::Undefined {
            // Build base shape desc with collision props (material, enabled, etc.)
            let mut base = ShapeDesc::new(obj_type);
            finalize_collision_desc(prim, stage, &mut base);

            match obj_type {
                ObjectType::SphereShape => {
                    let mut desc = SphereShapeDesc {
                        shape: base,
                        radius: 1.0,
                    };
                    // Read radius attribute from Sphere geometry
                    if let Some(attr) = prim.get_attribute("radius") {
                        if let Some(val) = attr
                            .get(TimeCode::default())
                            .and_then(|v| v.downcast_clone::<f64>())
                        {
                            // Scale radius by max axis scale (C++ parity)
                            let scale = desc.shape.local_scale;
                            let max_scale = scale.x.abs().max(scale.y.abs()).max(scale.z.abs());
                            desc.radius = val as f32 * max_scale;
                        }
                    }
                    parsed.sphere_shapes.push((path.clone(), desc));
                }
                ObjectType::CubeShape => {
                    let mut desc = CubeShapeDesc {
                        shape: base,
                        half_extents: Vec3f::new(0.5, 0.5, 0.5),
                    };
                    // Read size attribute from Cube geometry
                    if let Some(attr) = prim.get_attribute("size") {
                        if let Some(val) = attr
                            .get(TimeCode::default())
                            .and_then(|v| v.downcast_clone::<f64>())
                        {
                            let half = (val.abs() * 0.5) as f32;
                            desc.half_extents = Vec3f::new(half, half, half);
                        }
                    }
                    parsed.cube_shapes.push((path.clone(), desc));
                }
                ObjectType::CapsuleShape => {
                    let mut desc = CapsuleShapeDesc::default();
                    desc.shape = base;
                    parse_axis_radius_height(
                        prim,
                        &mut desc.axis,
                        &mut desc.radius,
                        &mut desc.half_height,
                    );
                    parsed.capsule_shapes.push((path.clone(), desc));
                }
                ObjectType::CylinderShape => {
                    let mut desc = CylinderShapeDesc::default();
                    desc.shape = base;
                    parse_axis_radius_height(
                        prim,
                        &mut desc.axis,
                        &mut desc.radius,
                        &mut desc.half_height,
                    );
                    parsed.cylinder_shapes.push((path.clone(), desc));
                }
                ObjectType::ConeShape => {
                    let mut desc = ConeShapeDesc::default();
                    desc.shape = base;
                    parse_axis_radius_height(
                        prim,
                        &mut desc.axis,
                        &mut desc.radius,
                        &mut desc.half_height,
                    );
                    parsed.cone_shapes.push((path.clone(), desc));
                }
                ObjectType::MeshShape => {
                    let mut desc = MeshShapeDesc::default();
                    desc.shape = base;
                    // Check MeshCollisionAPI for approximation
                    if let Some(mesh_api) = MeshCollisionAPI::get(stage, &path) {
                        if let Some(attr) = mesh_api.get_approximation_attr() {
                            if let Some(val) = attr
                                .get(TimeCode::default())
                                .and_then(|v| v.downcast_clone::<Token>())
                            {
                                desc.approximation = val;
                            }
                        }
                    }
                    parsed.mesh_shapes.push((path.clone(), desc));
                }
                ObjectType::PlaneShape => {
                    let mut desc = PlaneShapeDesc::default();
                    desc.shape = base;
                    // Read axis from Plane geometry
                    if let Some(attr) = prim.get_attribute("axis") {
                        if let Some(val) = attr
                            .get(TimeCode::default())
                            .and_then(|v| v.downcast_clone::<Token>())
                        {
                            desc.axis = parse_axis(&val);
                        }
                    }
                    parsed.plane_shapes.push((path.clone(), desc));
                }
                _ => {
                    // Capsule1, Cylinder1, Custom, SpherePoints -> generic shapes
                    parsed.shapes.push((path.clone(), obj_type, base));
                }
            }
        }
    }
}

/// Helper macro for reporting typed shape descriptors via callback.
macro_rules! report_typed_shapes {
    ($vec:expr, $obj_type:expr, $report_fn:expr, $user_data:expr) => {
        if !$vec.is_empty() {
            let paths: Vec<Path> = $vec.iter().map(|(p, _)| p.clone()).collect();
            let descs: Vec<ObjectDesc> = $vec.iter().map(|(_, d)| d.shape.base.clone()).collect();
            $report_fn($obj_type, &paths, &descs, $user_data);
        }
    };
}

/// Report all parsed data via callback.
fn report_parsed_data(
    parsed: &ParsedPhysicsData,
    report_fn: &impl Fn(ObjectType, &[Path], &[ObjectDesc], &Value),
    user_data: &Value,
) {
    // Report scenes
    if !parsed.scenes.is_empty() {
        let paths: Vec<Path> = parsed.scenes.iter().map(|(p, _)| p.clone()).collect();
        let descs: Vec<ObjectDesc> = parsed.scenes.iter().map(|(_, d)| d.base.clone()).collect();
        report_fn(ObjectType::Scene, &paths, &descs, user_data);
    }

    // Report collision groups
    if !parsed.collision_groups.is_empty() {
        let paths: Vec<Path> = parsed
            .collision_groups
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        let descs: Vec<ObjectDesc> = parsed
            .collision_groups
            .iter()
            .map(|(_, d)| d.base.clone())
            .collect();
        report_fn(ObjectType::CollisionGroup, &paths, &descs, user_data);
    }

    // Report rigid bodies
    if !parsed.rigid_bodies.is_empty() {
        let paths: Vec<Path> = parsed.rigid_bodies.iter().map(|(p, _)| p.clone()).collect();
        let descs: Vec<ObjectDesc> = parsed
            .rigid_bodies
            .iter()
            .map(|(_, d)| d.base.clone())
            .collect();
        report_fn(ObjectType::RigidBody, &paths, &descs, user_data);
    }

    // Report materials
    if !parsed.materials.is_empty() {
        let paths: Vec<Path> = parsed.materials.iter().map(|(p, _)| p.clone()).collect();
        let descs: Vec<ObjectDesc> = parsed
            .materials
            .iter()
            .map(|(_, d)| d.base.clone())
            .collect();
        report_fn(ObjectType::RigidBodyMaterial, &paths, &descs, user_data);
    }

    // Report articulations
    if !parsed.articulations.is_empty() {
        let paths: Vec<Path> = parsed
            .articulations
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        let descs: Vec<ObjectDesc> = parsed
            .articulations
            .iter()
            .map(|(_, d)| d.base.clone())
            .collect();
        report_fn(ObjectType::Articulation, &paths, &descs, user_data);
    }

    // Report joints
    if !parsed.fixed_joints.is_empty() {
        let paths: Vec<Path> = parsed.fixed_joints.iter().map(|(p, _)| p.clone()).collect();
        let descs: Vec<ObjectDesc> = parsed
            .fixed_joints
            .iter()
            .map(|(_, d)| d.joint.base.clone())
            .collect();
        report_fn(ObjectType::FixedJoint, &paths, &descs, user_data);
    }

    if !parsed.revolute_joints.is_empty() {
        let paths: Vec<Path> = parsed
            .revolute_joints
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        let descs: Vec<ObjectDesc> = parsed
            .revolute_joints
            .iter()
            .map(|(_, d)| d.joint.base.clone())
            .collect();
        report_fn(ObjectType::RevoluteJoint, &paths, &descs, user_data);
    }

    if !parsed.prismatic_joints.is_empty() {
        let paths: Vec<Path> = parsed
            .prismatic_joints
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        let descs: Vec<ObjectDesc> = parsed
            .prismatic_joints
            .iter()
            .map(|(_, d)| d.joint.base.clone())
            .collect();
        report_fn(ObjectType::PrismaticJoint, &paths, &descs, user_data);
    }

    if !parsed.spherical_joints.is_empty() {
        let paths: Vec<Path> = parsed
            .spherical_joints
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        let descs: Vec<ObjectDesc> = parsed
            .spherical_joints
            .iter()
            .map(|(_, d)| d.joint.base.clone())
            .collect();
        report_fn(ObjectType::SphericalJoint, &paths, &descs, user_data);
    }

    if !parsed.distance_joints.is_empty() {
        let paths: Vec<Path> = parsed
            .distance_joints
            .iter()
            .map(|(p, _)| p.clone())
            .collect();
        let descs: Vec<ObjectDesc> = parsed
            .distance_joints
            .iter()
            .map(|(_, d)| d.joint.base.clone())
            .collect();
        report_fn(ObjectType::DistanceJoint, &paths, &descs, user_data);
    }

    // Report typed shapes
    report_typed_shapes!(
        parsed.sphere_shapes,
        ObjectType::SphereShape,
        report_fn,
        user_data
    );
    report_typed_shapes!(
        parsed.cube_shapes,
        ObjectType::CubeShape,
        report_fn,
        user_data
    );
    report_typed_shapes!(
        parsed.capsule_shapes,
        ObjectType::CapsuleShape,
        report_fn,
        user_data
    );
    report_typed_shapes!(
        parsed.cylinder_shapes,
        ObjectType::CylinderShape,
        report_fn,
        user_data
    );
    report_typed_shapes!(
        parsed.cone_shapes,
        ObjectType::ConeShape,
        report_fn,
        user_data
    );
    report_typed_shapes!(
        parsed.mesh_shapes,
        ObjectType::MeshShape,
        report_fn,
        user_data
    );
    report_typed_shapes!(
        parsed.plane_shapes,
        ObjectType::PlaneShape,
        report_fn,
        user_data
    );

    // Report generic shapes (Capsule1, Cylinder1, Custom, etc.)
    // Group by type since C++ reports each ObjectType as separate batch
    if !parsed.shapes.is_empty() {
        let mut by_type: std::collections::HashMap<ObjectType, (Vec<Path>, Vec<ObjectDesc>)> =
            std::collections::HashMap::new();
        for (p, t, d) in &parsed.shapes {
            let entry = by_type
                .entry(*t)
                .or_insert_with(|| (Vec::new(), Vec::new()));
            entry.0.push(p.clone());
            entry.1.push(d.base.clone());
        }
        for (obj_type, (paths, descs)) in &by_type {
            report_fn(*obj_type, paths, descs, user_data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_axis() {
        assert_eq!(parse_axis(&Token::new("X")), Axis::X);
        assert_eq!(parse_axis(&Token::new("Y")), Axis::Y);
        assert_eq!(parse_axis(&Token::new("Z")), Axis::Z);
    }

    #[test]
    fn test_is_valid_limit() {
        assert!(is_valid_limit(45.0));
        assert!(is_valid_limit(-45.0));
        assert!(!is_valid_limit(f32::INFINITY));
        assert!(!is_valid_limit(f32::NEG_INFINITY));
        assert!(!is_valid_limit(f32::NAN));
    }

    #[test]
    fn test_custom_physics_tokens_default() {
        let tokens = CustomPhysicsTokens::default();
        assert!(tokens.joint_tokens.is_empty());
        assert!(tokens.shape_tokens.is_empty());
        assert!(tokens.instancer_tokens.is_empty());
    }

    #[test]
    fn test_parsed_physics_data_default() {
        let data = ParsedPhysicsData::default();
        assert!(data.scenes.is_empty());
        assert!(data.sphere_shapes.is_empty());
        assert!(data.cube_shapes.is_empty());
        assert!(data.capsule_shapes.is_empty());
        assert!(data.cylinder_shapes.is_empty());
        assert!(data.cone_shapes.is_empty());
        assert!(data.mesh_shapes.is_empty());
        assert!(data.plane_shapes.is_empty());
        assert!(data.shapes.is_empty());
    }

    #[test]
    fn test_parse_axis_radius_height_defaults() {
        // Verify default values when no attributes are present
        let mut axis = Axis::X;
        let mut radius = 1.0_f32;
        let mut half_height = 0.5_f32;
        // No prim to read from, so create a dummy Prim
        let prim = Prim::invalid();
        parse_axis_radius_height(&prim, &mut axis, &mut radius, &mut half_height);
        // Values should remain unchanged since the prim has no attributes
        assert_eq!(axis, Axis::X);
        assert_eq!(radius, 1.0);
        assert_eq!(half_height, 0.5);
    }

    #[test]
    fn test_finalize_collision_desc_invalid_prim() {
        // Verify no panic with invalid prim
        let prim = Prim::invalid();
        let stage =
            usd_core::Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll).unwrap();
        let mut desc = ShapeDesc::new(ObjectType::SphereShape);
        finalize_collision_desc(&prim, &stage, &mut desc);
        // collision_enabled stays default (true)
        assert!(desc.collision_enabled);
    }
}
