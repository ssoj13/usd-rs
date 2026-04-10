//! USD Geometry Module (usdGeom)
//!
//! This module provides geometry schemas for USD, including primitives,
//! transforms, and geometric properties.

pub mod basis_curves;
pub mod bbox_cache;
pub mod boundable;
pub mod boundable_compute_extent;
pub mod camera;
pub mod capsule;
pub mod cone;
pub mod constraint_target;
pub mod cube;
pub mod curves;
pub mod cylinder;
pub mod gprim;
pub mod hermite_curves;
pub mod imageable;
pub mod mesh;
pub mod metrics;
pub mod model_api;
pub mod motion_api;
pub mod nurbs_curves;
pub mod nurbs_patch;
pub mod plane;
pub mod point_based;
pub mod point_instancer;
pub mod points;
pub mod primvar;
pub mod primvars_api;
pub mod sampling_utils;
mod schema_create_default;
pub mod scope;
pub mod sphere;
pub mod subset;
pub mod tet_mesh;
pub mod tokens;
pub mod visibility_api;
pub mod xform;
pub mod xform_cache;
pub mod xform_common_api;
pub mod xform_op;
pub mod xformable;

// Validation
// pub mod geom_validators; // Removed - validators moved to usd_validation module

pub use basis_curves::BasisCurves;
pub use bbox_cache::BBoxCache;
pub use boundable::Boundable;
pub use boundable_compute_extent::{
    ComputeExtentFunction, compute_extent_from_plugins, register_compute_extent_function,
};
pub use camera::Camera;
pub use capsule::{Capsule, Capsule1};
pub use cone::Cone;
pub use constraint_target::ConstraintTarget;
pub use cube::Cube;
pub use curves::Curves;
pub use cylinder::{Cylinder, Cylinder1};
pub use gprim::Gprim;
pub use hermite_curves::{HermiteCurves, PointAndTangentArrays};
pub use imageable::{Imageable, PurposeInfo};
pub use mesh::{Mesh, SHARPNESS_INFINITE};
pub use metrics::{
    LinearUnits, get_fallback_up_axis, get_stage_meters_per_unit, get_stage_up_axis,
    linear_units_are, set_stage_meters_per_unit, set_stage_up_axis,
    stage_has_authored_meters_per_unit,
};
pub use model_api::ModelAPI;
pub use motion_api::MotionAPI;
pub use nurbs_curves::NurbsCurves;
pub use nurbs_patch::NurbsPatch;
pub use plane::Plane;
pub use point_based::PointBased;
pub use point_instancer::PointInstancer;
pub use points::Points;
pub use primvar::Primvar;
pub use primvars_api::PrimvarsAPI;
pub use sampling_utils::{
    OrientationsAngularVelocities, OrientationsAngularVelocitiesHalf,
    PositionsVelocitiesAccelerations, calculate_time_delta,
    get_orientations_and_angular_velocities, get_orientations_and_angular_velocities_half,
    get_positions_velocities_and_accelerations, get_scales,
};
pub use scope::Scope;
pub use sphere::Sphere;
pub use subset::Subset;
pub use tet_mesh::TetMesh;
pub use tokens::usd_geom_tokens;
pub use visibility_api::VisibilityAPI;
pub use xform::Xform;
pub use xform_cache::XformCache;
pub use xform_common_api::{OpFlags, Ops, RotationOrder, XformCommonAPI};
pub use xform_op::{XformOp, XformOpPrecision, XformOpType};
pub use xformable::{XformQuery, Xformable};
