//! Geometry utilities for procedural mesh generation.
//!
//! This module provides utilities for generating procedural geometric primitives
//! as triangle or quad meshes with proper topology, normals, and optional texture coordinates.
//! It is the Rust equivalent of OpenUSD's `pxr/imaging/geomUtil` library.
//!
//! # Overview
//!
//! The module includes generators for common geometric primitives:
//! - Cuboids (rectangular boxes)
//! - Spheres (UV or icosahedral)
//! - Cylinders (with optional caps)
//! - Cones (with optional base cap)
//! - Capsules (cylinder with hemispherical caps)
//! - Planes (rectangular quads)
//! - Disks (circular or elliptical)
//!
//! Each generator produces mesh topology compatible with [`MeshTopology`](usd_px_osd::MeshTopology),
//! along with vertex positions, normals, and optionally texture coordinates.
//!
//! # Example
//!
//! ```ignore
//! use usd_geom_util::SphereMeshGenerator;
//! use usd_gf::Vec3f;
//!
//! // Generate a UV sphere with 20x20 segments
//! let points = SphereMeshGenerator::generate_points_f32(
//!     20,    // num_radial
//!     20,    // num_axial
//!     1.0,   // radius
//!     360.0, // sweep_degrees
//!     None   // transform
//! );
//! println!("Generated {} points", points.len());
//! ```
//!
//! # Reference
//!
//! Based on OpenUSD `pxr/imaging/geomUtil`:
//! <https://openusd.org/dev/api/geom_util_page_front.html>

/// Interpolation mode tokens for mesh attributes
pub mod tokens;

/// Base mesh generation utilities and shared topology helpers
pub mod mesh_generator;

/// Cuboid (rectangular box) mesh generator
pub mod cuboid;

/// Sphere mesh generator (UV and icosahedral)
pub mod sphere;

/// Cylinder mesh generator with optional end caps
pub mod cylinder;

/// Cone mesh generator with optional base cap
pub mod cone;

/// Capsule mesh generator (cylinder with hemispherical caps)
pub mod capsule;

/// Plane (rectangular quad) mesh generator
pub mod plane;

/// Disk (circular/elliptical) mesh generator
pub mod disk;

// Re-export commonly used types

/// Interpolation mode tokens for geometry attributes
pub use tokens::InterpolationTokens;

/// Cap style options for capped geometry (cylinders, cones, capsules)
pub use mesh_generator::CapStyle;

// Re-export generators

/// Cuboid (rectangular box) mesh generator
pub use cuboid::CuboidMeshGenerator;

/// Sphere mesh generator
pub use sphere::SphereMeshGenerator;

/// Cylinder mesh generator
pub use cylinder::CylinderMeshGenerator;

/// Cone mesh generator
pub use cone::ConeMeshGenerator;

/// Capsule mesh generator
pub use capsule::CapsuleMeshGenerator;

/// Plane mesh generator
pub use plane::PlaneMeshGenerator;

/// Disk mesh generator
pub use disk::DiskMeshGenerator;
