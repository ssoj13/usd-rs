//! HdBasisCurvesTopology - Topology data for basis curves.
//!
//! Corresponds to pxr/imaging/hd/basisCurvesTopology.h.

use super::topology::{HdTopology, HdTopologyId};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Curve vertex counts (number of vertices per curve).
pub type VtIntArray = Vec<i32>;

/// Topology data for basis curves.
///
/// Corresponds to C++ `HdBasisCurvesTopology`.
#[derive(Clone, Debug)]
pub struct HdBasisCurvesTopology {
    curve_type: Token,
    curve_basis: Token,
    curve_wrap: Token,
    curve_vertex_counts: VtIntArray,
    curve_indices: VtIntArray,
    invisible_points: VtIntArray,
    invisible_curves: VtIntArray,
    num_points: usize,
}

fn compute_num_points(curve_vertex_counts: &[i32], indices: &[i32]) -> usize {
    if indices.is_empty() {
        curve_vertex_counts.iter().map(|&c| c as usize).sum()
    } else {
        indices
            .iter()
            .map(|&i| i as usize)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0)
    }
}

impl Default for HdBasisCurvesTopology {
    fn default() -> Self {
        Self {
            curve_type: Token::new("linear"),
            curve_basis: Token::new(""),
            curve_wrap: Token::new("nonperiodic"),
            curve_vertex_counts: Vec::new(),
            curve_indices: Vec::new(),
            invisible_points: Vec::new(),
            invisible_curves: Vec::new(),
            num_points: 0,
        }
    }
}

impl HdBasisCurvesTopology {
    /// Create new with given curve parameters.
    pub fn new(
        curve_type: Token,
        curve_basis: Token,
        curve_wrap: Token,
        curve_vertex_counts: VtIntArray,
        curve_indices: VtIntArray,
    ) -> Self {
        let (ct, cb) = if curve_type != "linear" && curve_type != "cubic" {
            (Token::new("linear"), Token::new(""))
        } else if curve_basis == "linear" && curve_type == "cubic" {
            (Token::new("linear"), Token::new(""))
        } else {
            (curve_type, curve_basis)
        };
        let num_points = compute_num_points(&curve_vertex_counts, &curve_indices);
        Self {
            curve_type: ct,
            curve_basis: cb,
            curve_wrap,
            curve_vertex_counts,
            curve_indices,
            invisible_points: Vec::new(),
            invisible_curves: Vec::new(),
            num_points,
        }
    }

    /// Set invisible points.
    pub fn set_invisible_points(&mut self, invisible_points: VtIntArray) {
        self.invisible_points = invisible_points;
    }

    /// Get invisible points.
    pub fn get_invisible_points(&self) -> &VtIntArray {
        &self.invisible_points
    }

    /// Set invisible curves.
    pub fn set_invisible_curves(&mut self, invisible_curves: VtIntArray) {
        self.invisible_curves = invisible_curves;
    }

    /// Get invisible curves.
    pub fn get_invisible_curves(&self) -> &VtIntArray {
        &self.invisible_curves
    }

    /// Get curve vertex counts.
    pub fn get_curve_vertex_counts(&self) -> &VtIntArray {
        &self.curve_vertex_counts
    }

    /// Get curve indices.
    pub fn get_curve_indices(&self) -> &VtIntArray {
        &self.curve_indices
    }

    /// Get number of curves.
    pub fn get_num_curves(&self) -> usize {
        self.curve_vertex_counts.len()
    }

    /// Get number of points.
    pub fn get_num_points(&self) -> usize {
        self.num_points
    }

    /// Get curve type.
    pub fn get_curve_type(&self) -> &Token {
        &self.curve_type
    }

    /// Get curve basis.
    pub fn get_curve_basis(&self) -> &Token {
        &self.curve_basis
    }

    /// Get curve wrap.
    pub fn get_curve_wrap(&self) -> &Token {
        &self.curve_wrap
    }

    /// Whether topology uses an index buffer.
    pub fn has_indices(&self) -> bool {
        !self.curve_indices.is_empty()
    }

    /// Calculate needed number of control points.
    pub fn calculate_needed_number_of_control_points(&self) -> usize {
        if self.curve_indices.is_empty() {
            self.curve_vertex_counts.iter().map(|&c| c as usize).sum()
        } else {
            1 + self
                .curve_indices
                .iter()
                .map(|&i| i as usize)
                .max()
                .unwrap_or(0)
        }
    }

    /// Calculate needed number of varying control points.
    pub fn calculate_needed_number_of_varying_control_points(&self) -> usize {
        self.calculate_needed_number_of_control_points()
    }
}

impl HdTopology for HdBasisCurvesTopology {
    fn compute_hash(&self) -> HdTopologyId {
        let mut hasher = DefaultHasher::new();
        self.curve_type.as_str().hash(&mut hasher);
        self.curve_basis.as_str().hash(&mut hasher);
        self.curve_wrap.as_str().hash(&mut hasher);
        self.curve_vertex_counts.hash(&mut hasher);
        self.curve_indices.hash(&mut hasher);
        self.invisible_points.hash(&mut hasher);
        self.invisible_curves.hash(&mut hasher);
        hasher.finish()
    }
}

impl PartialEq for HdBasisCurvesTopology {
    fn eq(&self, other: &Self) -> bool {
        self.curve_type == other.curve_type
            && self.curve_basis == other.curve_basis
            && self.curve_wrap == other.curve_wrap
            && self.curve_vertex_counts == other.curve_vertex_counts
            && self.curve_indices == other.curve_indices
            && self.invisible_points == other.invisible_points
            && self.invisible_curves == other.invisible_curves
    }
}

impl Eq for HdBasisCurvesTopology {}
