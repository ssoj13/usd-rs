
//! Basis curves primvar interpolation and index building computations.
//!
//! Provides:
//! - Index builder computation for line segments and cubic patches
//! - Varying-to-vertex primvar expansion for different curve bases
//! - Primvar size validation and fallback handling
//!
//! See pxr/imaging/hdSt/basisCurvesComputations.h for C++ reference.

use crate::basis_curves::{CurveBasis, CurveType};
use crate::basis_curves_topology::HdStBasisCurvesTopology;

// ---------------------------------------------------------------------------
// Varying -> vertex expansion
// ---------------------------------------------------------------------------

/// Interpolation mode for primvar expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveInterpolation {
    /// Per-vertex: one value per control vertex
    Vertex,
    /// Varying: one value per segment endpoint (fewer than vertex for cubic)
    Varying,
    /// Uniform: one value per curve
    Uniform,
    /// Constant: single value for all curves
    Constant,
}

/// Expand varying primvar data to per-vertex.
///
/// For cubic curves, varying data has fewer values than vertices.
/// This function expands varying values to match vertex count by
/// duplicating at curve endpoints according to the basis type.
///
/// For linear curves, varying == vertex, so this is a no-op passthrough.
pub fn expand_varying<T: Clone>(
    topology: &HdStBasisCurvesTopology,
    authored_values: &[T],
    fallback: &T,
) -> Vec<T> {
    let num_verts = topology.get_total_vertex_count();

    match topology.curve_type {
        CurveType::Linear => {
            if authored_values.len() == num_verts {
                authored_values.to_vec()
            } else {
                vec![fallback.clone(); num_verts]
            }
        }
        CurveType::Cubic if matches!(topology.basis, CurveBasis::CatmullRom | CurveBasis::BSpline | CurveBasis::Hermite) => {
            expand_varying_spline(topology, authored_values, num_verts, fallback)
        }
        CurveType::Cubic => expand_varying_bezier(topology, authored_values, num_verts, fallback),
    }
}

/// Expand varying for CatmullRom/BSpline.
///
/// For splines with vstep=1, varying values map to segments.
/// First and last vertex values are duplicated from the first/last
/// varying value.
fn expand_varying_spline<T: Clone>(
    topology: &HdStBasisCurvesTopology,
    authored: &[T],
    num_verts: usize,
    fallback: &T,
) -> Vec<T> {
    let mut output = Vec::with_capacity(num_verts);
    let mut src_idx = 0usize;

    for &nv in &topology.curve_vertex_counts {
        let nv = nv as usize;
        if nv < 1 {
            continue;
        }

        // First vertex: duplicate first varying value
        if src_idx < authored.len() {
            output.push(authored[src_idx].clone());
        } else {
            output.push(fallback.clone());
        }

        // Middle vertices: one-to-one mapping
        for _ in 1..nv.saturating_sub(2) {
            if src_idx < authored.len() {
                output.push(authored[src_idx].clone());
            } else {
                output.push(fallback.clone());
            }
            src_idx += 1;
        }

        // Second-to-last and last: duplicate last varying value
        if nv >= 2 {
            if src_idx < authored.len() {
                output.push(authored[src_idx].clone());
            } else {
                output.push(fallback.clone());
            }

            if src_idx < authored.len() {
                output.push(authored[src_idx].clone());
            } else {
                output.push(fallback.clone());
            }
            src_idx += 1;
        }
    }

    output.truncate(num_verts);
    output
}

/// Expand varying for Bezier curves.
///
/// For Bezier with vstep=3, each varying value maps to a segment.
/// The begin value maps to first 2 CVs, end value maps to last 2 CVs,
/// with intermediate CVs getting the same value as their segment.
fn expand_varying_bezier<T: Clone>(
    topology: &HdStBasisCurvesTopology,
    authored: &[T],
    num_verts: usize,
    fallback: &T,
) -> Vec<T> {
    let mut output = Vec::with_capacity(num_verts);
    let mut src_idx = 0usize;

    for &nv in &topology.curve_vertex_counts {
        let nv = nv as usize;
        if nv < 1 {
            continue;
        }

        // First 2 CVs get the first varying value
        let val = if src_idx < authored.len() {
            &authored[src_idx]
        } else {
            fallback
        };
        output.push(val.clone());
        if nv > 1 {
            output.push(val.clone());
        }
        src_idx += 1;

        // Middle CVs: groups of 3 (vstep) get the same varying value
        let mut i = 2;
        while i < nv.saturating_sub(2) {
            let val = if src_idx < authored.len() {
                &authored[src_idx]
            } else {
                fallback
            };
            // 3 CVs per segment
            for _ in 0..3.min(nv - i - 2) {
                output.push(val.clone());
            }
            i += 3;
            src_idx += 1;
        }

        // Last 2 CVs get the last varying value
        if nv > 2 {
            let val = if src_idx < authored.len() {
                &authored[src_idx]
            } else {
                fallback
            };
            output.push(val.clone());
            if nv > 3 {
                output.push(val.clone());
            }
            src_idx += 1;
        }
    }

    output.truncate(num_verts);
    output
}

// ---------------------------------------------------------------------------
// Primvar validation
// ---------------------------------------------------------------------------

/// Validate primvar size for vertex interpolation.
///
/// Returns `Ok(())` if the authored size matches expected, or the authored
/// data can be used (e.g., size 1 treated as constant).
/// Returns `Err` with description if the size is wrong.
pub fn validate_vertex_primvar(
    topology: &HdStBasisCurvesTopology,
    authored_size: usize,
    primvar_name: &str,
) -> Result<PrimvarValidation, String> {
    let expected = topology.calc_needed_control_points();

    if authored_size == expected {
        Ok(PrimvarValidation::Valid)
    } else if authored_size == 1 {
        Ok(PrimvarValidation::TreatAsConstant)
    } else if topology.has_indices() && authored_size > expected {
        Ok(PrimvarValidation::Truncate(expected))
    } else {
        Err(format!(
            "Primvar '{}' has incorrect size for vertex interpolation \
             (need {}, got {})",
            primvar_name, expected, authored_size
        ))
    }
}

/// Validate primvar size for varying interpolation.
pub fn validate_varying_primvar(
    topology: &HdStBasisCurvesTopology,
    authored_size: usize,
    primvar_name: &str,
) -> Result<PrimvarValidation, String> {
    let expected = topology.calc_needed_varying_control_points();

    if authored_size == expected {
        Ok(PrimvarValidation::Valid)
    } else if authored_size == 1 {
        Ok(PrimvarValidation::TreatAsConstant)
    } else {
        Err(format!(
            "Primvar '{}' has incorrect size for varying interpolation \
             (need {}, got {})",
            primvar_name, expected, authored_size
        ))
    }
}

/// Primvar validation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimvarValidation {
    /// Size matches expected.
    Valid,
    /// Size is 1; treat as constant across all elements.
    TreatAsConstant,
    /// Authored size is larger; truncate to given count.
    Truncate(usize),
}

// ---------------------------------------------------------------------------
// Interpolate primvar (full pipeline)
// ---------------------------------------------------------------------------

/// Interpolate a primvar for basis curves rendering.
///
/// Handles both vertex and varying interpolation, expanding varying
/// to per-vertex as needed. Validates sizes and fills fallback on error.
pub fn interpolate_primvar<T: Clone>(
    topology: &HdStBasisCurvesTopology,
    authored: &[T],
    interpolation: CurveInterpolation,
    fallback: &T,
) -> Vec<T> {
    let num_verts = topology.calc_needed_control_points();

    match interpolation {
        CurveInterpolation::Vertex => {
            if authored.len() == num_verts {
                authored.to_vec()
            } else if authored.len() == 1 {
                vec![authored[0].clone(); num_verts]
            } else if topology.has_indices() && authored.len() > num_verts {
                authored[..num_verts].to_vec()
            } else {
                log::warn!(
                    "Primvar has incorrect vertex size (need {}, got {}), using fallback",
                    num_verts,
                    authored.len()
                );
                vec![fallback.clone(); num_verts]
            }
        }
        CurveInterpolation::Varying => {
            let num_varying = topology.calc_needed_varying_control_points();

            if authored.len() == num_varying {
                if topology.curve_type == CurveType::Linear {
                    authored.to_vec()
                } else {
                    expand_varying(topology, authored, fallback)
                }
            } else if authored.len() == 1 {
                vec![authored[0].clone(); num_verts]
            } else {
                log::warn!(
                    "Primvar has incorrect varying size (need {}, got {}), using fallback",
                    num_varying,
                    authored.len()
                );
                vec![fallback.clone(); num_verts]
            }
        }
        CurveInterpolation::Uniform => {
            // One value per curve -> expand to per-vertex
            let mut result = Vec::with_capacity(num_verts);
            for (ci, &nv) in topology.curve_vertex_counts.iter().enumerate() {
                let val = authored.get(ci).unwrap_or(fallback);
                for _ in 0..nv {
                    result.push(val.clone());
                }
            }
            result
        }
        CurveInterpolation::Constant => {
            let val = authored.first().unwrap_or(fallback);
            vec![val.clone(); num_verts]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basis_curves::CurveWrap;

    fn make_linear_topo(counts: Vec<i32>) -> HdStBasisCurvesTopology {
        HdStBasisCurvesTopology::new(
            counts,
            vec![],
            CurveBasis::BSpline,
            CurveType::Linear,
            CurveWrap::NonPeriodic,
        )
    }

    fn make_bspline_topo(counts: Vec<i32>) -> HdStBasisCurvesTopology {
        HdStBasisCurvesTopology::new(
            counts,
            vec![],
            CurveBasis::BSpline,
            CurveType::Cubic,
            CurveWrap::NonPeriodic,
        )
    }

    #[test]
    fn test_linear_vertex() {
        let topo = make_linear_topo(vec![4]);
        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        let result = interpolate_primvar(&topo, &data, CurveInterpolation::Vertex, &0.0);
        assert_eq!(result, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn test_linear_constant() {
        let topo = make_linear_topo(vec![4]);
        let data = vec![42.0f32];
        let result = interpolate_primvar(&topo, &data, CurveInterpolation::Constant, &0.0);
        assert_eq!(result, vec![42.0; 4]);
    }

    #[test]
    fn test_uniform_interpolation() {
        let topo = make_linear_topo(vec![3, 2]);
        let data = vec![10.0f32, 20.0]; // one per curve
        let result = interpolate_primvar(&topo, &data, CurveInterpolation::Uniform, &0.0);
        assert_eq!(result, vec![10.0, 10.0, 10.0, 20.0, 20.0]);
    }

    #[test]
    fn test_vertex_primvar_validation() {
        let topo = make_linear_topo(vec![4]);
        assert_eq!(
            validate_vertex_primvar(&topo, 4, "points"),
            Ok(PrimvarValidation::Valid)
        );
        assert_eq!(
            validate_vertex_primvar(&topo, 1, "points"),
            Ok(PrimvarValidation::TreatAsConstant)
        );
        assert!(validate_vertex_primvar(&topo, 3, "points").is_err());
    }

    #[test]
    fn test_varying_validation() {
        let topo = make_bspline_topo(vec![6]);
        // BSpline 6 CVs -> 3 segments -> 4 varying points
        assert_eq!(
            validate_varying_primvar(&topo, 4, "widths"),
            Ok(PrimvarValidation::Valid)
        );
        assert!(validate_varying_primvar(&topo, 6, "widths").is_err());
    }

    #[test]
    fn test_expand_varying_bspline() {
        let topo = make_bspline_topo(vec![6]);
        // 4 varying values -> 6 vertex values
        let varying = vec![1.0f32, 2.0, 3.0, 4.0];
        let expanded = expand_varying(&topo, &varying, &0.0);
        assert_eq!(expanded.len(), 6);
    }

    #[test]
    fn test_vertex_size_1_broadcast() {
        let topo = make_linear_topo(vec![5]);
        let data = vec![99.0f32];
        let result = interpolate_primvar(&topo, &data, CurveInterpolation::Vertex, &0.0);
        assert_eq!(result, vec![99.0; 5]);
    }

    #[test]
    fn test_wrong_size_fallback() {
        let topo = make_linear_topo(vec![4]);
        let data = vec![1.0f32, 2.0]; // wrong size
        let result = interpolate_primvar(&topo, &data, CurveInterpolation::Vertex, &0.0);
        assert_eq!(result, vec![0.0; 4]); // fallback
    }
}
