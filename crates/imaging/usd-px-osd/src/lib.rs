//! PxOsd - OpenSubdiv wrapper for USD subdivision surfaces.
//!
//! This module provides data structures and utilities for working with
//! OpenSubdiv subdivision surfaces in USD. It includes:
//!
//! - Subdivision scheme tokens (Catmull-Clark, Loop, Bilinear)
//! - Subdivision tags (creases, corners, interpolation rules)
//! - Mesh topology representation
//! - Topology validation utilities
//!
//! # Overview
//!
//! OpenSubdiv is Pixar's library for evaluating subdivision surfaces.
//! This module provides the USD data structures for describing subdivision
//! surface topology without requiring the full OpenSubdiv library.
//!
//! # Subdivision Schemes
//!
//! Three main subdivision schemes are supported:
//! - **Catmull-Clark**: For quadrilateral meshes (most common)
//! - **Loop**: For triangular meshes
//! - **Bilinear**: No smoothing, just linear interpolation
//!
//! # Example
//!
//! ```ignore
//! use usd_px_osd::{MeshTopology, tokens};
//!
//! // Create a simple quad mesh
//! let topology = MeshTopology::new(
//!     tokens::CATMULL_CLARK.clone(),
//!     tokens::RIGHT_HANDED.clone(),
//!     vec![4], // One quad
//!     vec![0, 1, 2, 3], // Vertex indices
//! );
//!
//! // Validate the topology
//! let validation = topology.validate();
//! if validation.is_valid() {
//!     println!("Topology is valid!");
//! } else {
//!     for error in validation.errors() {
//!         eprintln!("Validation error: {}", error);
//!     }
//! }
//! ```
//!
//! # Subdivision Tags
//!
//! Subdivision tags control edge sharpness (creases) and corner sharpness:
//!
//! ```ignore
//! use usd_px_osd::{SubdivTags, tokens};
//!
//! let tags = SubdivTags::new(
//!     tokens::EDGE_AND_CORNER.clone(),
//!     tokens::BOUNDARIES.clone(),
//!     tokens::UNIFORM.clone(),
//!     tokens::SMOOTH.clone(),
//!     vec![0, 1, 1, 2], // Crease edge indices
//!     vec![2, 2],       // Two creases of length 2
//!     vec![1.0, 0.5],   // Crease sharpness
//!     vec![0],          // Corner at vertex 0
//!     vec![2.0],        // Corner sharpness
//! );
//! ```
//!
//! # References
//!
//! - [OpenSubdiv Documentation](http://graphics.pixar.com/opensubdiv/docs/intro.html)
//! - [USD Subdivision Surfaces](https://graphics.pixar.com/usd/docs/api/class_usd_geom_mesh.html)

pub mod enums;
pub mod mesh_topology;
pub mod mesh_topology_validation;
pub mod refiner_factory;
pub mod subdiv_tags;
pub mod tokens;

// Re-export main types
pub use enums::{
    CreasingMethod, FVarLinearInterpolation, TriangleSubdivision, VtxBoundaryInterpolation,
};
pub use mesh_topology::MeshTopology;
pub use mesh_topology_validation::{Invalidation, MeshTopologyValidation, ValidationCode};
pub use refiner_factory::{RefinerFactory, TopologyRefiner, TopologyRefinerSharedPtr};
pub use subdiv_tags::SubdivTags;
