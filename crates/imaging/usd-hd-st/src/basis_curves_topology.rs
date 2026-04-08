//! HdSt_BasisCurvesTopology - Storm curve topology management.
//!
//! Extends Hydra `HdBasisCurvesTopology` with Storm-specific index building for
//! drawing curves as line segments or cubic patches.
//! See `pxr/imaging/hdSt/basisCurvesTopology.h` for the reference contract.

use crate::basis_curves::{CurveBasis, CurveType, CurveWrap};
use std::sync::Arc;
use usd_hd::prim::basis_curves::HdBasisCurvesTopology as HdInputBasisCurvesTopology;

// ---------------------------------------------------------------------------
// Index building results
// ---------------------------------------------------------------------------

/// Result of curve index building.
#[derive(Debug, Clone, Default)]
pub struct CurveIndexResult {
    /// Line segment or patch indices
    pub indices: Vec<u32>,
    /// Primitive parameter: maps each segment/patch to curve index
    pub primitive_params: Vec<i32>,
    /// Number of segments or patches
    pub num_segments: usize,
}

// ---------------------------------------------------------------------------
// HdStBasisCurvesTopology (Storm-specific)
// ---------------------------------------------------------------------------

/// Storm basis curves topology with index building.
///
/// Wraps the base topology data and provides index buffer computation
/// for rendering curves as line segments or cubic patches.
#[derive(Debug, Clone)]
pub struct HdStBasisCurvesTopology {
    /// Number of vertices per curve
    pub curve_vertex_counts: Vec<i32>,
    /// Optional vertex indices (for shared vertices)
    pub curve_indices: Vec<i32>,
    /// Curve basis type
    pub basis: CurveBasis,
    /// Curve type (linear, cubic)
    pub curve_type: CurveType,
    /// Wrap mode
    pub wrap: CurveWrap,
}

impl HdStBasisCurvesTopology {
    /// Create from base topology data.
    pub fn new(
        curve_vertex_counts: Vec<i32>,
        curve_indices: Vec<i32>,
        basis: CurveBasis,
        curve_type: CurveType,
        wrap: CurveWrap,
    ) -> Self {
        Self {
            curve_vertex_counts,
            curve_indices,
            basis,
            curve_type,
            wrap,
        }
    }

    /// Create Storm topology from Hydra basis-curves topology.
    ///
    /// This keeps Storm-side index building on the same semantic inputs the
    /// delegate exposes, rather than reinterpreting authored USD data through a
    /// second ad-hoc topology contract.
    pub fn from_hd_topology(topology: &HdInputBasisCurvesTopology) -> Self {
        let basis = match topology.basis.map(|b| b.as_token_str()).unwrap_or("") {
            "bezier" => CurveBasis::Bezier,
            "bspline" => CurveBasis::BSpline,
            "catmullRom" => CurveBasis::CatmullRom,
            "hermite" => CurveBasis::Hermite,
            _ => CurveBasis::BSpline,
        };
        let curve_type = match topology
            .curve_type
            .map(|t| t.as_token_str())
            .unwrap_or("linear")
        {
            "cubic" => CurveType::Cubic,
            _ => CurveType::Linear,
        };
        let wrap = match topology.wrap.as_token_str() {
            "periodic" => CurveWrap::Periodic,
            "pinned" => CurveWrap::Pinned,
            _ => CurveWrap::NonPeriodic,
        };
        Self::new(
            topology.curve_vertex_counts.clone(),
            Vec::new(),
            basis,
            curve_type,
            wrap,
        )
    }

    /// Number of authored curves.
    pub fn get_curve_count(&self) -> usize {
        self.curve_vertex_counts.len()
    }

    /// Whether indices are provided (vs sequential).
    pub fn has_indices(&self) -> bool {
        !self.curve_indices.is_empty()
    }

    /// Get total expected vertex count from curve_vertex_counts.
    pub fn get_total_vertex_count(&self) -> usize {
        self.curve_vertex_counts.iter().map(|&c| c as usize).sum()
    }

    /// Calculate number of control points needed (accounting for indices).
    pub fn calc_needed_control_points(&self) -> usize {
        if self.has_indices() {
            self.curve_indices
                .iter()
                .map(|&i| i as usize)
                .max()
                .map(|m| m + 1)
                .unwrap_or(0)
        } else {
            self.get_total_vertex_count()
        }
    }

    /// Calculate number of varying control points needed.
    ///
    /// For cubic curves, varying count differs from vertex count:
    /// varying = number of segments + 1 (for non-periodic).
    pub fn calc_needed_varying_control_points(&self) -> usize {
        match self.curve_type {
            CurveType::Linear => self.get_total_vertex_count(),
            CurveType::Cubic => match self.basis {
                CurveBasis::Bezier => {
                    let mut count = 0usize;
                    for &nv in &self.curve_vertex_counts {
                        let segments = ((nv - 1) / 3).max(0) as usize;
                        count += segments + 1;
                    }
                    count
                }
                CurveBasis::BSpline | CurveBasis::CatmullRom | CurveBasis::Hermite => {
                    let mut count = 0usize;
                    for &nv in &self.curve_vertex_counts {
                        if nv < 4 {
                            count += 1;
                            continue;
                        }
                        let segments = if self.wrap == CurveWrap::Periodic {
                            nv as usize
                        } else {
                            (nv - 3) as usize
                        };
                        count += segments + 1;
                    }
                    count
                }
            },
        }
    }

    /// Build point indices (for drawing endpoints only).
    pub fn build_points_index(&self) -> CurveIndexResult {
        let total = self.calc_needed_control_points();
        CurveIndexResult {
            indices: (0..total as u32).collect(),
            primitive_params: vec![0; total],
            num_segments: total,
        }
    }

    /// Build line segment indices for drawing.
    ///
    /// `force_lines` = true forces line segments even for cubic curves;
    /// otherwise cubic curves produce multi-segment line strips from CVs.
    pub fn build_index(&self, force_lines: bool) -> CurveIndexResult {
        if self.curve_type == CurveType::Linear || force_lines {
            self.build_lines_index()
        } else {
            self.build_cubic_index()
        }
    }

    /// Build linear line segment indices.
    fn build_lines_index(&self) -> CurveIndexResult {
        let mut indices = Vec::new();
        let mut prim_params = Vec::new();
        let mut vertex_offset = 0u32;

        for (curve_idx, &count) in self.curve_vertex_counts.iter().enumerate() {
            let count = count as u32;
            if count < 2 {
                vertex_offset += count;
                continue;
            }

            let num_segments = if self.wrap == CurveWrap::Periodic {
                count
            } else {
                count - 1
            };

            for s in 0..num_segments {
                let v0 = vertex_offset + s;
                let v1 = vertex_offset + (s + 1) % count;

                if self.has_indices() {
                    let i0 = self.curve_indices.get(v0 as usize).copied().unwrap_or(0) as u32;
                    let i1 = self.curve_indices.get(v1 as usize).copied().unwrap_or(0) as u32;
                    indices.extend_from_slice(&[i0, i1]);
                } else {
                    indices.extend_from_slice(&[v0, v1]);
                }
                prim_params.push(curve_idx as i32);
            }

            vertex_offset += count;
        }

        let num_segments = indices.len() / 2;
        CurveIndexResult {
            indices,
            primitive_params: prim_params,
            num_segments,
        }
    }

    /// Build cubic curve indices (4 CVs per segment).
    fn build_cubic_index(&self) -> CurveIndexResult {
        let mut indices = Vec::new();
        let mut prim_params = Vec::new();
        let mut vertex_offset = 0u32;

        for (curve_idx, &nv) in self.curve_vertex_counts.iter().enumerate() {
            let nv = nv as u32;
            let vstep = match self.basis {
                CurveBasis::Bezier => 3u32,
                _ => 1u32, // BSpline, CatmullRom
            };

            let num_segments = match self.basis {
                CurveBasis::Bezier => {
                    if nv < 4 {
                        0
                    } else {
                        (nv - 1) / 3
                    }
                }
                _ => {
                    if nv < 4 {
                        0
                    } else if self.wrap == CurveWrap::Periodic {
                        nv
                    } else {
                        nv - 3
                    }
                }
            };

            for s in 0..num_segments {
                let base = vertex_offset + s * vstep;

                // 4 control vertices per cubic segment
                for cv in 0..4u32 {
                    let vi = if self.wrap == CurveWrap::Periodic {
                        vertex_offset + (s * vstep + cv) % nv
                    } else {
                        base + cv
                    };

                    if self.has_indices() {
                        let idx = self.curve_indices.get(vi as usize).copied().unwrap_or(0) as u32;
                        indices.push(idx);
                    } else {
                        indices.push(vi);
                    }
                }

                prim_params.push(curve_idx as i32);
            }

            vertex_offset += nv;
        }

        let num_segments = indices.len() / 4;
        CurveIndexResult {
            indices,
            primitive_params: prim_params,
            num_segments,
        }
    }
}

/// Shared pointer to Storm basis curves topology.
pub type HdStBasisCurvesTopologySharedPtr = Arc<HdStBasisCurvesTopology>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_lines() {
        let topo = HdStBasisCurvesTopology::new(
            vec![4],
            vec![],
            CurveBasis::BSpline,
            CurveType::Linear,
            CurveWrap::NonPeriodic,
        );

        let result = topo.build_index(false);
        assert_eq!(result.num_segments, 3); // 4 verts -> 3 line segments
        assert_eq!(result.indices, vec![0, 1, 1, 2, 2, 3]);
    }

    #[test]
    fn test_linear_periodic() {
        let topo = HdStBasisCurvesTopology::new(
            vec![4],
            vec![],
            CurveBasis::BSpline,
            CurveType::Linear,
            CurveWrap::Periodic,
        );

        let result = topo.build_index(false);
        assert_eq!(result.num_segments, 4); // 4 verts -> 4 segments (wraps)
        // Last segment connects v3 -> v0
        assert_eq!(result.indices[6], 3);
        assert_eq!(result.indices[7], 0);
    }

    #[test]
    fn test_bezier_cubic() {
        // 7 CVs = 2 Bezier segments (step=3)
        let topo = HdStBasisCurvesTopology::new(
            vec![7],
            vec![],
            CurveBasis::Bezier,
            CurveType::Cubic,
            CurveWrap::NonPeriodic,
        );

        let result = topo.build_index(false);
        assert_eq!(result.num_segments, 2);
        assert_eq!(result.indices.len(), 8); // 2 segments * 4 CVs
        // First segment: 0,1,2,3; second: 3,4,5,6
        assert_eq!(result.indices[0..4], [0, 1, 2, 3]);
        assert_eq!(result.indices[4..8], [3, 4, 5, 6]);
    }

    #[test]
    fn test_bspline_cubic() {
        // 6 CVs, BSpline, non-periodic -> 3 segments
        let topo = HdStBasisCurvesTopology::new(
            vec![6],
            vec![],
            CurveBasis::BSpline,
            CurveType::Cubic,
            CurveWrap::NonPeriodic,
        );

        let result = topo.build_index(false);
        assert_eq!(result.num_segments, 3);
        assert_eq!(result.indices.len(), 12); // 3 * 4
    }

    #[test]
    fn test_with_indices() {
        let topo = HdStBasisCurvesTopology::new(
            vec![3],
            vec![5, 10, 15], // shared vertex indices
            CurveBasis::BSpline,
            CurveType::Linear,
            CurveWrap::NonPeriodic,
        );

        let result = topo.build_index(false);
        assert_eq!(result.num_segments, 2);
        assert_eq!(result.indices, vec![5, 10, 10, 15]);
    }

    #[test]
    fn test_points_index() {
        let topo = HdStBasisCurvesTopology::new(
            vec![4, 3],
            vec![],
            CurveBasis::BSpline,
            CurveType::Linear,
            CurveWrap::NonPeriodic,
        );

        let result = topo.build_points_index();
        assert_eq!(result.indices, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_varying_control_points() {
        // Linear: varying == vertex
        let topo = HdStBasisCurvesTopology::new(
            vec![5],
            vec![],
            CurveBasis::BSpline,
            CurveType::Linear,
            CurveWrap::NonPeriodic,
        );
        assert_eq!(topo.calc_needed_varying_control_points(), 5);

        // BSpline 6 CVs, non-periodic: 3 segments -> 4 varying points
        let topo2 = HdStBasisCurvesTopology::new(
            vec![6],
            vec![],
            CurveBasis::BSpline,
            CurveType::Cubic,
            CurveWrap::NonPeriodic,
        );
        assert_eq!(topo2.calc_needed_varying_control_points(), 4);
    }

    #[test]
    fn test_force_lines() {
        // Cubic curve forced to lines
        let topo = HdStBasisCurvesTopology::new(
            vec![7],
            vec![],
            CurveBasis::Bezier,
            CurveType::Cubic,
            CurveWrap::NonPeriodic,
        );

        let lines = topo.build_index(true);
        assert_eq!(lines.num_segments, 6); // 7 verts -> 6 line segments
        assert_eq!(lines.indices[0..2], [0, 1]);
    }
}
