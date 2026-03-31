//! Physics Object Descriptors for scene parsing.
//!
//! Defines all descriptor types used by the physics parsing system.
//! These descriptors represent the parsed form of USD physics prims,
//! ready for consumption by physics engines.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/parseDesc.h`
//!
//! # Architecture
//!
//! - `ObjectDesc` - Base descriptor with type, path, validity
//! - Shape descriptors - Collision geometry (sphere, box, capsule, mesh, etc.)
//! - Joint descriptors - Constraints between bodies
//! - Body descriptors - Rigid bodies, articulations
//! - Scene descriptors - Physics scene configuration
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::parse_desc::*;
//!
//! // Descriptors are created by the parsing system
//! let sphere = SphereShapeDesc::new(1.0);
//! let joint = RevoluteJointDesc::default();
//! ```

use usd_gf::{Quatf, Vec3f};
use usd_sdf::Path;
use usd_tf::Token;

/// Sentinel value for float max comparisons (0.5e38).
pub const SENTINEL_LIMIT: f32 = 0.5e38;

// ============================================================================
// Enums
// ============================================================================

/// Physics object type enumeration.
///
/// Identifies the type of parsed physics object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum ObjectType {
    /// Undefined/invalid object type
    #[default]
    Undefined = 0,

    /// Physics scene object (UsdPhysicsScene)
    Scene,

    /// Rigid body object (UsdPhysicsRigidBodyAPI)
    RigidBody,

    // Shape types
    /// Sphere collision shape (UsdPhysicsSphereShape)
    SphereShape,
    /// Cube collision shape (UsdPhysicsCubeShape)
    CubeShape,
    /// Capsule collision shape (UsdPhysicsCapsuleShape)
    CapsuleShape,
    /// Capsule with different top/bottom radii (UsdPhysicsCapsule1Shape)
    Capsule1Shape,
    /// Cylinder collision shape (UsdPhysicsCylinderShape)
    CylinderShape,
    /// Cylinder with different top/bottom radii (UsdPhysicsCylinder1Shape)
    Cylinder1Shape,
    /// Cone collision shape (UsdPhysicsConeShape)
    ConeShape,
    /// Mesh collision shape (UsdPhysicsMeshShape)
    MeshShape,
    /// Plane collision shape (UsdPhysicsPlaneShape)
    PlaneShape,
    /// Custom collision shape with user-defined geometry
    CustomShape,
    /// Sphere points collision shape from UsdGeomPoints
    SpherePointsShape,

    // Joint types
    /// Fixed joint with no degrees of freedom (UsdPhysicsFixedJoint)
    FixedJoint,
    /// Revolute joint with single rotational DOF (UsdPhysicsRevoluteJoint)
    RevoluteJoint,
    /// Prismatic joint with single translational DOF (UsdPhysicsPrismaticJoint)
    PrismaticJoint,
    /// Spherical joint with three rotational DOFs (UsdPhysicsSphericalJoint)
    SphericalJoint,
    /// Distance joint maintaining fixed distance (UsdPhysicsDistanceJoint)
    DistanceJoint,
    /// Generic 6-DOF joint (UsdPhysicsD6Joint)
    D6Joint,
    /// Custom joint with user-defined constraints
    CustomJoint,

    /// Rigid body material properties (UsdPhysicsRigidBodyMaterial)
    RigidBodyMaterial,

    /// Articulation root (UsdPhysicsArticulation)
    Articulation,

    /// Collision filtering group (UsdPhysicsCollisionGroup)
    CollisionGroup,

    /// Sentinel value marking the end of enum
    Last,
}

/// Physics axis enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum Axis {
    /// X axis
    #[default]
    X = 0,
    /// Y axis
    Y = 1,
    /// Z axis
    Z = 2,
}

/// Joint degree of freedom enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum JointDOF {
    /// Distance constraint
    Distance = 0,
    /// Translation along X axis
    TransX,
    /// Translation along Y axis
    TransY,
    /// Translation along Z axis
    TransZ,
    /// Rotation around X axis
    RotX,
    /// Rotation around Y axis
    RotY,
    /// Rotation around Z axis
    RotZ,
}

// ============================================================================
// Base Descriptor
// ============================================================================

/// Base physics object descriptor.
///
/// All specific descriptors inherit from this base.
#[derive(Debug, Clone, Default)]
pub struct ObjectDesc {
    /// The type of physics object this descriptor represents
    pub object_type: ObjectType,
    /// USD path to the source prim
    pub prim_path: Path,
    /// Whether this descriptor was successfully parsed and is valid
    pub is_valid: bool,
}

impl ObjectDesc {
    /// Create a new object descriptor with the given type.
    pub fn new(object_type: ObjectType) -> Self {
        Self {
            object_type,
            prim_path: Path::default(),
            is_valid: true,
        }
    }
}

// ============================================================================
// Material Descriptor
// ============================================================================

/// Rigid body material descriptor.
///
/// Describes physics material properties like friction and restitution.
#[derive(Debug, Clone)]
pub struct RigidBodyMaterialDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// Static friction coefficient (0.0 = frictionless)
    pub static_friction: f32,
    /// Dynamic friction coefficient (0.0 = frictionless)
    pub dynamic_friction: f32,
    /// Restitution coefficient (0.0 = inelastic, 1.0 = perfectly elastic)
    pub restitution: f32,
    /// Material density in kg/m³ (-1.0 = use default)
    pub density: f32,
}

impl Default for RigidBodyMaterialDesc {
    fn default() -> Self {
        Self {
            base: ObjectDesc::new(ObjectType::RigidBodyMaterial),
            static_friction: 0.0,
            dynamic_friction: 0.0,
            restitution: 0.0,
            density: -1.0,
        }
    }
}

// ============================================================================
// Scene Descriptor
// ============================================================================

/// Scene descriptor.
///
/// Describes physics scene configuration including gravity.
#[derive(Debug, Clone)]
pub struct SceneDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// Gravity direction vector (0,0,0 = use stage's negative up axis)
    pub gravity_direction: Vec3f,
    /// Gravity magnitude in m/s² (-inf = use Earth gravity scaled by metersPerUnit)
    pub gravity_magnitude: f32,
}

impl Default for SceneDesc {
    fn default() -> Self {
        Self {
            base: ObjectDesc::new(ObjectType::Scene),
            gravity_direction: Vec3f::zero(),
            gravity_magnitude: f32::NEG_INFINITY,
        }
    }
}

// ============================================================================
// Collision Group Descriptor
// ============================================================================

/// Collision group descriptor.
///
/// Describes collision filtering groups.
#[derive(Debug, Clone, Default)]
pub struct CollisionGroupDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// If true, group collides only with filtered groups instead of all except filtered
    pub invert_filtered_groups: bool,
    /// USD paths to collision groups filtered by this group
    pub filtered_groups: Vec<Path>,
    /// Name of the merged collision group
    pub merge_group_name: String,
    /// USD paths to collision groups merged into this group
    pub merged_groups: Vec<Path>,
}

impl CollisionGroupDesc {
    /// Create a new collision group descriptor.
    pub fn new() -> Self {
        Self {
            base: ObjectDesc::new(ObjectType::CollisionGroup),
            ..Default::default()
        }
    }

    /// Get the filtered groups as a slice.
    pub fn filtered_groups(&self) -> &[Path] {
        &self.filtered_groups
    }

    /// Get the merged groups as a slice.
    pub fn merged_groups(&self) -> &[Path] {
        &self.merged_groups
    }
}

// ============================================================================
// Shape Descriptors
// ============================================================================

/// Base shape descriptor.
///
/// Common properties for all collision shapes.
/// Note: Shape sizes include scale (except mesh which reports geometry scale).
#[derive(Debug, Clone, Default)]
pub struct ShapeDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// USD path to owning rigid body (empty for static colliders)
    pub rigid_body: Path,
    /// Local position offset from the rigid body
    pub local_pos: Vec3f,
    /// Local rotation offset from the rigid body
    pub local_rot: Quatf,
    /// Local scale applied to the shape
    pub local_scale: Vec3f,
    /// USD paths to physics materials applied to this shape
    pub materials: Vec<Path>,
    /// USD paths to simulation owners that control this shape
    pub simulation_owners: Vec<Path>,
    /// USD paths to objects with which collision is disabled
    pub filtered_collisions: Vec<Path>,
    /// USD paths to collision groups this shape belongs to
    pub collision_groups: Vec<Path>,
    /// Whether collision detection is enabled for this shape
    pub collision_enabled: bool,
}

impl ShapeDesc {
    /// Create a new shape descriptor with the given type.
    pub fn new(object_type: ObjectType) -> Self {
        Self {
            base: ObjectDesc::new(object_type),
            rigid_body: Path::default(),
            local_pos: Vec3f::zero(),
            local_rot: Quatf::identity(),
            local_scale: Vec3f::new(1.0, 1.0, 1.0),
            materials: Vec::new(),
            simulation_owners: Vec::new(),
            filtered_collisions: Vec::new(),
            collision_groups: Vec::new(),
            collision_enabled: true,
        }
    }
}

/// Sphere shape descriptor.
#[derive(Debug, Clone)]
pub struct SphereShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Sphere radius (includes scale from ShapeDesc)
    pub radius: f32,
}

impl SphereShapeDesc {
    /// Create a new sphere shape with the given radius.
    pub fn new(radius: f32) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::SphereShape),
            radius,
        }
    }
}

impl Default for SphereShapeDesc {
    fn default() -> Self {
        Self::new(0.0)
    }
}

/// Capsule shape descriptor.
#[derive(Debug, Clone)]
pub struct CapsuleShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Capsule radius (includes scale)
    pub radius: f32,
    /// Half the capsule height along axis (excludes hemisphere caps)
    pub half_height: f32,
    /// Primary axis of the capsule (X, Y, or Z)
    pub axis: Axis,
}

impl CapsuleShapeDesc {
    /// Create a new capsule shape with the given parameters.
    pub fn new(radius: f32, half_height: f32, axis: Axis) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::CapsuleShape),
            radius,
            half_height,
            axis,
        }
    }
}

impl Default for CapsuleShapeDesc {
    fn default() -> Self {
        Self::new(0.0, 0.0, Axis::X)
    }
}

/// Capsule1 shape descriptor (with different top/bottom radii).
#[derive(Debug, Clone)]
pub struct Capsule1ShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Radius at the top end of the capsule
    pub top_radius: f32,
    /// Radius at the bottom end of the capsule
    pub bottom_radius: f32,
    /// Half the capsule height along axis (excludes hemisphere caps)
    pub half_height: f32,
    /// Primary axis of the capsule (X, Y, or Z)
    pub axis: Axis,
}

impl Capsule1ShapeDesc {
    /// Create a new capsule1 shape with different top/bottom radii.
    pub fn new(top_radius: f32, bottom_radius: f32, half_height: f32, axis: Axis) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::Capsule1Shape),
            top_radius,
            bottom_radius,
            half_height,
            axis,
        }
    }
}

impl Default for Capsule1ShapeDesc {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, Axis::X)
    }
}

/// Cylinder shape descriptor.
#[derive(Debug, Clone)]
pub struct CylinderShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Cylinder radius (includes scale)
    pub radius: f32,
    /// Half the cylinder height along axis
    pub half_height: f32,
    /// Primary axis of the cylinder (X, Y, or Z)
    pub axis: Axis,
}

impl CylinderShapeDesc {
    /// Create a new cylinder shape with the given parameters.
    pub fn new(radius: f32, half_height: f32, axis: Axis) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::CylinderShape),
            radius,
            half_height,
            axis,
        }
    }
}

impl Default for CylinderShapeDesc {
    fn default() -> Self {
        Self::new(0.0, 0.0, Axis::X)
    }
}

/// Cylinder1 shape descriptor (with different top/bottom radii).
#[derive(Debug, Clone)]
pub struct Cylinder1ShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Radius at the top end of the cylinder
    pub top_radius: f32,
    /// Radius at the bottom end of the cylinder
    pub bottom_radius: f32,
    /// Half the cylinder height along axis
    pub half_height: f32,
    /// Primary axis of the cylinder (X, Y, or Z)
    pub axis: Axis,
}

impl Cylinder1ShapeDesc {
    /// Create a new cylinder1 shape with different top/bottom radii.
    pub fn new(top_radius: f32, bottom_radius: f32, half_height: f32, axis: Axis) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::Cylinder1Shape),
            top_radius,
            bottom_radius,
            half_height,
            axis,
        }
    }
}

impl Default for Cylinder1ShapeDesc {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0, Axis::X)
    }
}

/// Cone shape descriptor.
#[derive(Debug, Clone)]
pub struct ConeShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Cone base radius (includes scale)
    pub radius: f32,
    /// Half the cone height along axis
    pub half_height: f32,
    /// Primary axis of the cone (X, Y, or Z)
    pub axis: Axis,
}

impl ConeShapeDesc {
    /// Create a new cone shape with the given parameters.
    pub fn new(radius: f32, half_height: f32, axis: Axis) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::ConeShape),
            radius,
            half_height,
            axis,
        }
    }
}

impl Default for ConeShapeDesc {
    fn default() -> Self {
        Self::new(0.0, 0.0, Axis::X)
    }
}

/// Plane shape descriptor.
#[derive(Debug, Clone)]
pub struct PlaneShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Normal axis of the infinite plane (X, Y, or Z)
    pub axis: Axis,
}

impl PlaneShapeDesc {
    /// Create a new plane shape with the given normal axis.
    pub fn new(axis: Axis) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::PlaneShape),
            axis,
        }
    }
}

impl Default for PlaneShapeDesc {
    fn default() -> Self {
        Self::new(Axis::X)
    }
}

/// Custom shape descriptor.
#[derive(Debug, Clone)]
pub struct CustomShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Token identifying the custom geometry type
    pub custom_geometry_token: Token,
}

impl Default for CustomShapeDesc {
    fn default() -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::CustomShape),
            custom_geometry_token: Token::empty(),
        }
    }
}

/// Cube shape descriptor.
#[derive(Debug, Clone)]
pub struct CubeShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Half extents of the box along each axis (includes scale)
    pub half_extents: Vec3f,
}

impl CubeShapeDesc {
    /// Create a new cube shape with the given half extents.
    pub fn new(half_extents: Vec3f) -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::CubeShape),
            half_extents,
        }
    }
}

impl Default for CubeShapeDesc {
    fn default() -> Self {
        Self::new(Vec3f::new(1.0, 1.0, 1.0))
    }
}

/// Mesh shape descriptor.
#[derive(Debug, Clone)]
pub struct MeshShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Collision approximation type (e.g., "convexHull", "meshSimplification")
    pub approximation: Token,
    /// Scale applied to the mesh geometry
    pub mesh_scale: Vec3f,
    /// Whether the mesh collision is two-sided
    pub double_sided: bool,
}

impl Default for MeshShapeDesc {
    fn default() -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::MeshShape),
            approximation: Token::empty(),
            mesh_scale: Vec3f::new(1.0, 1.0, 1.0),
            double_sided: false,
        }
    }
}

/// Single sphere point (position + radius).
#[derive(Debug, Clone, Copy, Default)]
pub struct SpherePoint {
    /// Center position of the sphere
    pub center: Vec3f,
    /// Radius of the sphere
    pub radius: f32,
}

/// Sphere points shape descriptor (from UsdGeomPoints).
#[derive(Debug, Clone, Default)]
pub struct SpherePointsShapeDesc {
    /// Common shape properties
    pub shape: ShapeDesc,
    /// Collection of sphere instances with individual positions and radii
    pub sphere_points: Vec<SpherePoint>,
}

impl SpherePointsShapeDesc {
    /// Create a new sphere points shape descriptor.
    pub fn new() -> Self {
        Self {
            shape: ShapeDesc::new(ObjectType::SpherePointsShape),
            sphere_points: Vec::new(),
        }
    }
}

// ============================================================================
// Rigid Body Descriptor
// ============================================================================

/// Rigid body descriptor.
#[derive(Debug, Clone)]
pub struct RigidBodyDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// USD paths to collision shapes attached to this body
    pub collisions: Vec<Path>,
    /// USD paths to objects with which collision is disabled
    pub filtered_collisions: Vec<Path>,
    /// USD paths to simulation owners that control this body
    pub simulation_owners: Vec<Path>,
    /// World-space position of the body
    pub position: Vec3f,
    /// World-space orientation of the body
    pub rotation: Quatf,
    /// World-space scale of the body
    pub scale: Vec3f,
    /// If false, body is treated as static (immovable)
    pub rigid_body_enabled: bool,
    /// If true, body is kinematic (moved by animation, not forces)
    pub kinematic_body: bool,
    /// If true, body begins simulation in sleeping state
    pub starts_asleep: bool,
    /// Initial linear velocity in m/s
    pub linear_velocity: Vec3f,
    /// Initial angular velocity in rad/s
    pub angular_velocity: Vec3f,
}

impl Default for RigidBodyDesc {
    fn default() -> Self {
        Self {
            base: ObjectDesc::new(ObjectType::RigidBody),
            collisions: Vec::new(),
            filtered_collisions: Vec::new(),
            simulation_owners: Vec::new(),
            position: Vec3f::zero(),
            rotation: Quatf::identity(),
            scale: Vec3f::new(1.0, 1.0, 1.0),
            rigid_body_enabled: true,
            kinematic_body: false,
            starts_asleep: false,
            linear_velocity: Vec3f::zero(),
            angular_velocity: Vec3f::zero(),
        }
    }
}

// ============================================================================
// Joint Limit and Drive
// ============================================================================

/// Joint limit descriptor.
#[derive(Debug, Clone, Copy)]
pub struct JointLimit {
    /// Whether this limit is active
    pub enabled: bool,
    /// Lower bound of the limit (distance/angle depending on joint type)
    pub lower: f32,
    /// Upper bound of the limit (distance/angle depending on joint type)
    pub upper: f32,
}

impl Default for JointLimit {
    fn default() -> Self {
        Self {
            enabled: false,
            lower: 90.0, // Default values match C++
            upper: -90.0,
        }
    }
}

impl JointLimit {
    /// Create an enabled limit with the given bounds.
    pub fn new(lower: f32, upper: f32) -> Self {
        Self {
            enabled: true,
            lower,
            upper,
        }
    }
}

/// Joint drive descriptor.
///
/// Drive formula: force = stiffness * (targetPos - pos) + damping * (targetVel - vel)
#[derive(Debug, Clone, Copy)]
pub struct JointDrive {
    /// Whether this drive is active
    pub enabled: bool,
    /// Target position the drive tries to reach
    pub target_position: f32,
    /// Target velocity the drive tries to maintain
    pub target_velocity: f32,
    /// Maximum force/torque the drive can apply
    pub force_limit: f32,
    /// Spring stiffness coefficient (higher = stiffer)
    pub stiffness: f32,
    /// Damping coefficient (higher = more damping)
    pub damping: f32,
    /// If true, drive applies acceleration instead of force
    pub acceleration: bool,
}

impl Default for JointDrive {
    fn default() -> Self {
        Self {
            enabled: false,
            target_position: 0.0,
            target_velocity: 0.0,
            force_limit: f32::MAX,
            stiffness: 0.0,
            damping: 0.0,
            acceleration: false,
        }
    }
}

// ============================================================================
// Articulation Descriptor
// ============================================================================

/// Articulation descriptor.
#[derive(Debug, Clone, Default)]
pub struct ArticulationDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// USD paths to root prims where the articulation hierarchy begins
    pub root_prims: Vec<Path>,
    /// USD paths to objects with which collision is disabled
    pub filtered_collisions: Vec<Path>,
    /// USD paths to joints that are part of this articulation
    pub articulated_joints: Vec<Path>,
    /// USD paths to rigid bodies that are part of this articulation
    pub articulated_bodies: Vec<Path>,
}

impl ArticulationDesc {
    /// Create a new articulation descriptor.
    pub fn new() -> Self {
        Self {
            base: ObjectDesc::new(ObjectType::Articulation),
            ..Default::default()
        }
    }
}

// ============================================================================
// Joint Descriptors
// ============================================================================

/// Vector of joint limits indexed by degree of freedom.
pub type JointLimits = Vec<(JointDOF, JointLimit)>;
/// Vector of joint drives indexed by degree of freedom.
pub type JointDrives = Vec<(JointDOF, JointDrive)>;

/// Base joint descriptor.
#[derive(Debug, Clone)]
pub struct JointDesc {
    /// Base descriptor fields
    pub base: ObjectDesc,
    /// USD path to first body/joint relationship target
    pub rel0: Path,
    /// USD path to second body/joint relationship target
    pub rel1: Path,
    /// USD path to first rigid body (resolved from rel0)
    pub body0: Path,
    /// USD path to second rigid body (resolved from rel1)
    pub body1: Path,
    /// Local frame position on body0
    pub local_pose0_position: Vec3f,
    /// Local frame orientation on body0
    pub local_pose0_orientation: Quatf,
    /// Local frame position on body1
    pub local_pose1_position: Vec3f,
    /// Local frame orientation on body1
    pub local_pose1_orientation: Quatf,
    /// Whether the joint constraint is active
    pub joint_enabled: bool,
    /// Force threshold for breaking the joint (N)
    pub break_force: f32,
    /// Torque threshold for breaking the joint (Nm)
    pub break_torque: f32,
    /// If true, joint will not be included in articulation solver
    pub exclude_from_articulation: bool,
    /// If true, connected bodies can collide with each other
    pub collision_enabled: bool,
}

impl JointDesc {
    /// Create a new joint descriptor with the given type.
    pub fn new(object_type: ObjectType) -> Self {
        Self {
            base: ObjectDesc::new(object_type),
            rel0: Path::default(),
            rel1: Path::default(),
            body0: Path::default(),
            body1: Path::default(),
            local_pose0_position: Vec3f::zero(),
            local_pose0_orientation: Quatf::identity(),
            local_pose1_position: Vec3f::zero(),
            local_pose1_orientation: Quatf::identity(),
            joint_enabled: true,
            break_force: f32::MAX,
            break_torque: f32::MAX,
            exclude_from_articulation: false,
            collision_enabled: false,
        }
    }
}

/// Fixed joint descriptor.
#[derive(Debug, Clone)]
pub struct FixedJointDesc {
    /// Common joint properties
    pub joint: JointDesc,
}

impl Default for FixedJointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::FixedJoint),
        }
    }
}

/// Custom joint descriptor.
#[derive(Debug, Clone)]
pub struct CustomJointDesc {
    /// Common joint properties
    pub joint: JointDesc,
}

impl Default for CustomJointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::CustomJoint),
        }
    }
}

/// D6 (generic) joint descriptor.
#[derive(Debug, Clone)]
pub struct D6JointDesc {
    /// Common joint properties
    pub joint: JointDesc,
    /// Limits for each degree of freedom
    pub joint_limits: JointLimits,
    /// Drives for each degree of freedom
    pub joint_drives: JointDrives,
}

impl Default for D6JointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::D6Joint),
            joint_limits: Vec::new(),
            joint_drives: Vec::new(),
        }
    }
}

/// Prismatic joint descriptor.
#[derive(Debug, Clone)]
pub struct PrismaticJointDesc {
    /// Common joint properties
    pub joint: JointDesc,
    /// Axis of translation (X, Y, or Z)
    pub axis: Axis,
    /// Translation limit along the axis
    pub limit: JointLimit,
    /// Translation drive along the axis
    pub drive: JointDrive,
}

impl Default for PrismaticJointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::PrismaticJoint),
            axis: Axis::X,
            limit: JointLimit::default(),
            drive: JointDrive::default(),
        }
    }
}

/// Spherical joint descriptor.
#[derive(Debug, Clone)]
pub struct SphericalJointDesc {
    /// Common joint properties
    pub joint: JointDesc,
    /// Cone limit axis (X, Y, or Z)
    pub axis: Axis,
    /// Cone angle limit for rotations
    pub limit: JointLimit,
}

impl Default for SphericalJointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::SphericalJoint),
            axis: Axis::X,
            limit: JointLimit::default(),
        }
    }
}

/// Revolute joint descriptor.
#[derive(Debug, Clone)]
pub struct RevoluteJointDesc {
    /// Common joint properties
    pub joint: JointDesc,
    /// Axis of rotation (X, Y, or Z)
    pub axis: Axis,
    /// Angular limit around the axis
    pub limit: JointLimit,
    /// Angular drive around the axis
    pub drive: JointDrive,
}

impl Default for RevoluteJointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::RevoluteJoint),
            axis: Axis::X,
            limit: JointLimit::default(),
            drive: JointDrive::default(),
        }
    }
}

/// Distance joint descriptor.
#[derive(Debug, Clone)]
pub struct DistanceJointDesc {
    /// Common joint properties
    pub joint: JointDesc,
    /// Whether minimum distance limit is active
    pub min_enabled: bool,
    /// Whether maximum distance limit is active
    pub max_enabled: bool,
    /// Distance limits (lower = min distance, upper = max distance)
    pub limit: JointLimit,
}

impl Default for DistanceJointDesc {
    fn default() -> Self {
        Self {
            joint: JointDesc::new(ObjectType::DistanceJoint),
            min_enabled: false,
            max_enabled: false,
            limit: JointLimit::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_type_values() {
        assert_eq!(ObjectType::Undefined as u32, 0);
        assert_eq!(ObjectType::Scene as u32, 1);
    }

    #[test]
    fn test_sphere_shape_desc() {
        let sphere = SphereShapeDesc::new(2.5);
        assert_eq!(sphere.radius, 2.5);
        assert_eq!(sphere.shape.base.object_type, ObjectType::SphereShape);
    }

    #[test]
    fn test_joint_limit_default() {
        let limit = JointLimit::default();
        assert!(!limit.enabled);
    }

    #[test]
    fn test_axis_enum() {
        assert_eq!(Axis::X as u32, 0);
        assert_eq!(Axis::Y as u32, 1);
        assert_eq!(Axis::Z as u32, 2);
    }
}
