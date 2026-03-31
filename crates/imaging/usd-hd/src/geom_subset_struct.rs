
//! HdGeomSubset - Geometry subset struct (faces/points).
//!
//! Corresponds to pxr/imaging/hd/geomSubset.h.
//! Describes a subset of geometry as a set of indices.

use usd_sdf::Path;
use usd_vt::Array;

/// Type of geometry subset elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdGeomSubsetType {
    /// A subset of faces
    TypeFaceSet,
    /// A subset of points (for future use)
    TypePointSet,
    /// A subset of curves (for future use)
    TypeCurveSet,
}

/// Describes a subset of geometry as a set of indices.
///
/// Corresponds to C++ `HdGeomSubset`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdGeomSubset {
    /// The type of elements this subset includes.
    pub type_: HdGeomSubsetType,

    /// Path identifying this subset in the scene.
    pub id: Path,

    /// Path of material bound to this subset.
    pub material_id: Path,

    /// Element indices (faces, points, or curves).
    pub indices: Array<i32>,
}

impl HdGeomSubset {
    /// Create new subset.
    pub fn new(type_: HdGeomSubsetType, id: Path, material_id: Path, indices: Array<i32>) -> Self {
        Self {
            type_,
            id,
            material_id,
            indices,
        }
    }

    /// Create face set subset.
    pub fn face_set(id: Path, material_id: Path, indices: Array<i32>) -> Self {
        Self::new(HdGeomSubsetType::TypeFaceSet, id, material_id, indices)
    }
}

/// Vector of geometry subsets.
pub type HdGeomSubsets = Vec<HdGeomSubset>;
