//! UsdPhysics - Physics simulation schemas for USD.
//!
//! This module provides schemas for physics simulation including:
//! - Rigid body dynamics (RigidBodyAPI, CollisionAPI, MassAPI, MeshCollisionAPI)
//! - Joints (Joint, RevoluteJoint, PrismaticJoint, SphericalJoint, DistanceJoint, FixedJoint)
//! - Joint drives and limits (DriveAPI, LimitAPI)
//! - Physics materials (MaterialAPI)
//! - Collision groups (CollisionGroup, FilteredPairsAPI)
//! - Scene configuration (Scene)
//! - Articulation (ArticulationRootAPI)
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdPhysics/` module.
//!
//! # Architecture
//!
//! The physics schemas follow a layered approach:
//! - **API schemas** (RigidBodyAPI, CollisionAPI, etc.) add physics behavior to existing prims
//! - **Typed schemas** (Joint, Scene, CollisionGroup) define standalone physics primitives
//! - **Multiple-apply API schemas** (DriveAPI, LimitAPI) allow multiple instances per prim
//!
//! # Usage
//!
//! ```ignore
//! use usd::usd_physics::{RigidBodyAPI, CollisionAPI, Scene, Joint, MassAPI};
//!
//! // Create a physics scene
//! let scene = Scene::define(&stage, &Path::from_string("/World/PhysicsScene")?)?;
//! scene.create_gravity_magnitude_attr(Some(9.81))?;
//!
//! // Add rigid body behavior to a mesh
//! let rigid_body = RigidBodyAPI::apply(&mesh_prim)?;
//! rigid_body.create_velocity_attr(Some(Vec3f::new(0.0, 0.0, 0.0)))?;
//!
//! // Add collision and mass
//! let collision = CollisionAPI::apply(&mesh_prim)?;
//! let mass = MassAPI::apply(&mesh_prim)?;
//! mass.create_mass_attr(Some(10.0))?;
//! ```

// Module declarations - implemented
mod articulation_root_api;
mod collision_api;
mod distance_joint;
mod fixed_joint;
mod joint;
mod mass_api;
mod material_api;
mod mesh_collision_api;
mod metrics;
mod prismatic_joint;
mod revolute_joint;
mod rigid_body_api;
mod scene;
mod spherical_joint;
mod tokens;

// Recently implemented
mod collision_group;
mod drive_api;
mod filtered_pairs_api;
mod limit_api;
mod mass_properties;
mod parse_desc;
mod parse_utils;

// Public re-exports - Typed schemas
pub use collision_group::{CollisionGroup, CollisionGroupCollectionAPI, CollisionGroupTable};
pub use distance_joint::DistanceJoint;
pub use fixed_joint::FixedJoint;
pub use joint::Joint;
pub use prismatic_joint::PrismaticJoint;
pub use revolute_joint::RevoluteJoint;
pub use scene::Scene;
pub use spherical_joint::SphericalJoint;

// Public re-exports - API schemas
pub use articulation_root_api::ArticulationRootAPI;
pub use collision_api::CollisionAPI;
pub use drive_api::DriveAPI;
pub use filtered_pairs_api::FilteredPairsAPI;
pub use limit_api::LimitAPI;
pub use mass_api::MassAPI;
pub use mass_properties::MassProperties;
pub use material_api::MaterialAPI;
pub use mesh_collision_api::MeshCollisionAPI;
pub use parse_desc::*;
pub use parse_utils::{
    CustomPhysicsTokens, ParsedPhysicsData, ReportFn, collect_physics_from_range,
    load_physics_from_range,
};
pub use rigid_body_api::{MassInformation, MassInformationFn, RigidBodyAPI};

// Public re-exports - Utilities
pub use metrics::{
    MassUnits, get_stage_kilograms_per_unit, mass_units_are, mass_units_are_default,
    set_stage_kilograms_per_unit, stage_has_authored_kilograms_per_unit,
};
pub use tokens::{USD_PHYSICS_TOKENS, UsdPhysicsTokensType};
