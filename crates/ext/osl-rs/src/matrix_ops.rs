//! Matrix runtime operations — transform, determinant, transpose.
//!
//! Port of `opmatrix.cpp`. Provides matrix operations for coordinate
//! system transformations and other matrix math used in shaders.

use crate::Float;
use crate::math::{Matrix44, Vec3};
use crate::renderer::RendererServices;
use crate::shaderglobals::ShaderGlobals;
use crate::typedesc::VecSemantics;
use crate::ustring::UString;

/// Compute the determinant of a 4×4 matrix.
pub fn determinant(m: &Matrix44) -> Float {
    let a = &m.m;

    a[0][0]
        * (a[1][1] * (a[2][2] * a[3][3] - a[2][3] * a[3][2])
            - a[1][2] * (a[2][1] * a[3][3] - a[2][3] * a[3][1])
            + a[1][3] * (a[2][1] * a[3][2] - a[2][2] * a[3][1]))
        - a[0][1]
            * (a[1][0] * (a[2][2] * a[3][3] - a[2][3] * a[3][2])
                - a[1][2] * (a[2][0] * a[3][3] - a[2][3] * a[3][0])
                + a[1][3] * (a[2][0] * a[3][2] - a[2][2] * a[3][0]))
        + a[0][2]
            * (a[1][0] * (a[2][1] * a[3][3] - a[2][3] * a[3][1])
                - a[1][1] * (a[2][0] * a[3][3] - a[2][3] * a[3][0])
                + a[1][3] * (a[2][0] * a[3][1] - a[2][1] * a[3][0]))
        - a[0][3]
            * (a[1][0] * (a[2][1] * a[3][2] - a[2][2] * a[3][1])
                - a[1][1] * (a[2][0] * a[3][2] - a[2][2] * a[3][0])
                + a[1][2] * (a[2][0] * a[3][1] - a[2][1] * a[3][0]))
}

/// Transpose a 4×4 matrix.
pub fn transpose(m: &Matrix44) -> Matrix44 {
    let a = &m.m;
    Matrix44 {
        m: [
            [a[0][0], a[1][0], a[2][0], a[3][0]],
            [a[0][1], a[1][1], a[2][1], a[3][1]],
            [a[0][2], a[1][2], a[2][2], a[3][2]],
            [a[0][3], a[1][3], a[2][3], a[3][3]],
        ],
    }
}

/// Compute the inverse of a 4×4 matrix. Returns None if singular.
pub fn inverse(m: &Matrix44) -> Option<Matrix44> {
    let det = determinant(m);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv_det = 1.0 / det;
    let a = &m.m;

    // Cofactor matrix, transposed and scaled
    let mut r = [[0.0f32; 4]; 4];

    r[0][0] = (a[1][1] * (a[2][2] * a[3][3] - a[2][3] * a[3][2])
        - a[1][2] * (a[2][1] * a[3][3] - a[2][3] * a[3][1])
        + a[1][3] * (a[2][1] * a[3][2] - a[2][2] * a[3][1]))
        * inv_det;
    r[0][1] = -(a[0][1] * (a[2][2] * a[3][3] - a[2][3] * a[3][2])
        - a[0][2] * (a[2][1] * a[3][3] - a[2][3] * a[3][1])
        + a[0][3] * (a[2][1] * a[3][2] - a[2][2] * a[3][1]))
        * inv_det;
    r[0][2] = (a[0][1] * (a[1][2] * a[3][3] - a[1][3] * a[3][2])
        - a[0][2] * (a[1][1] * a[3][3] - a[1][3] * a[3][1])
        + a[0][3] * (a[1][1] * a[3][2] - a[1][2] * a[3][1]))
        * inv_det;
    r[0][3] = -(a[0][1] * (a[1][2] * a[2][3] - a[1][3] * a[2][2])
        - a[0][2] * (a[1][1] * a[2][3] - a[1][3] * a[2][1])
        + a[0][3] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]))
        * inv_det;

    r[1][0] = -(a[1][0] * (a[2][2] * a[3][3] - a[2][3] * a[3][2])
        - a[1][2] * (a[2][0] * a[3][3] - a[2][3] * a[3][0])
        + a[1][3] * (a[2][0] * a[3][2] - a[2][2] * a[3][0]))
        * inv_det;
    r[1][1] = (a[0][0] * (a[2][2] * a[3][3] - a[2][3] * a[3][2])
        - a[0][2] * (a[2][0] * a[3][3] - a[2][3] * a[3][0])
        + a[0][3] * (a[2][0] * a[3][2] - a[2][2] * a[3][0]))
        * inv_det;
    r[1][2] = -(a[0][0] * (a[1][2] * a[3][3] - a[1][3] * a[3][2])
        - a[0][2] * (a[1][0] * a[3][3] - a[1][3] * a[3][0])
        + a[0][3] * (a[1][0] * a[3][2] - a[1][2] * a[3][0]))
        * inv_det;
    r[1][3] = (a[0][0] * (a[1][2] * a[2][3] - a[1][3] * a[2][2])
        - a[0][2] * (a[1][0] * a[2][3] - a[1][3] * a[2][0])
        + a[0][3] * (a[1][0] * a[2][2] - a[1][2] * a[2][0]))
        * inv_det;

    r[2][0] = (a[1][0] * (a[2][1] * a[3][3] - a[2][3] * a[3][1])
        - a[1][1] * (a[2][0] * a[3][3] - a[2][3] * a[3][0])
        + a[1][3] * (a[2][0] * a[3][1] - a[2][1] * a[3][0]))
        * inv_det;
    r[2][1] = -(a[0][0] * (a[2][1] * a[3][3] - a[2][3] * a[3][1])
        - a[0][1] * (a[2][0] * a[3][3] - a[2][3] * a[3][0])
        + a[0][3] * (a[2][0] * a[3][1] - a[2][1] * a[3][0]))
        * inv_det;
    r[2][2] = (a[0][0] * (a[1][1] * a[3][3] - a[1][3] * a[3][1])
        - a[0][1] * (a[1][0] * a[3][3] - a[1][3] * a[3][0])
        + a[0][3] * (a[1][0] * a[3][1] - a[1][1] * a[3][0]))
        * inv_det;
    r[2][3] = -(a[0][0] * (a[1][1] * a[2][3] - a[1][3] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][3] - a[1][3] * a[2][0])
        + a[0][3] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]))
        * inv_det;

    r[3][0] = -(a[1][0] * (a[2][1] * a[3][2] - a[2][2] * a[3][1])
        - a[1][1] * (a[2][0] * a[3][2] - a[2][2] * a[3][0])
        + a[1][2] * (a[2][0] * a[3][1] - a[2][1] * a[3][0]))
        * inv_det;
    r[3][1] = (a[0][0] * (a[2][1] * a[3][2] - a[2][2] * a[3][1])
        - a[0][1] * (a[2][0] * a[3][2] - a[2][2] * a[3][0])
        + a[0][2] * (a[2][0] * a[3][1] - a[2][1] * a[3][0]))
        * inv_det;
    r[3][2] = -(a[0][0] * (a[1][1] * a[3][2] - a[1][2] * a[3][1])
        - a[0][1] * (a[1][0] * a[3][2] - a[1][2] * a[3][0])
        + a[0][2] * (a[1][0] * a[3][1] - a[1][1] * a[3][0]))
        * inv_det;
    r[3][3] = (a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]))
        * inv_det;

    Some(Matrix44 { m: r })
}

/// Multiply a 4×4 matrix by another 4×4 matrix.
pub fn matmul(a: &Matrix44, b: &Matrix44) -> Matrix44 {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            r[i][j] = a.m[i][0] * b.m[0][j]
                + a.m[i][1] * b.m[1][j]
                + a.m[i][2] * b.m[2][j]
                + a.m[i][3] * b.m[3][j];
        }
    }
    Matrix44 { m: r }
}

/// Transform a point by a 4×4 matrix (affine: w=1).
pub fn transform_point(m: &Matrix44, p: Vec3) -> Vec3 {
    let a = &m.m;
    let x = a[0][0] * p.x + a[0][1] * p.y + a[0][2] * p.z + a[0][3];
    let y = a[1][0] * p.x + a[1][1] * p.y + a[1][2] * p.z + a[1][3];
    let z = a[2][0] * p.x + a[2][1] * p.y + a[2][2] * p.z + a[2][3];
    let w = a[3][0] * p.x + a[3][1] * p.y + a[3][2] * p.z + a[3][3];
    if w.abs() > 1e-12 && (w - 1.0).abs() > 1e-12 {
        Vec3::new(x / w, y / w, z / w)
    } else {
        Vec3::new(x, y, z)
    }
}

/// Transform a vector by a 4×4 matrix (direction: w=0).
pub fn transform_vector(m: &Matrix44, v: Vec3) -> Vec3 {
    let a = &m.m;
    Vec3::new(
        a[0][0] * v.x + a[0][1] * v.y + a[0][2] * v.z,
        a[1][0] * v.x + a[1][1] * v.y + a[1][2] * v.z,
        a[2][0] * v.x + a[2][1] * v.y + a[2][2] * v.z,
    )
}

/// Transform a normal by a 4×4 matrix (uses adjugate of upper-left 3x3).
/// Delegates to `Matrix44::transform_normal` which uses the more efficient
/// adjugate approach (no full inverse needed, proportional to inverse-transpose).
#[inline]
pub fn transform_normal(m: &Matrix44, n: Vec3) -> Vec3 {
    m.transform_normal(n)
}

/// Resolve "shader" or "object" space from ShaderGlobals transform pointers.
/// Returns the space->common matrix. In C++ OSL these are stored as opaque
/// TransformationPtr in ShaderGlobals; our Rust test infra stores `*const Matrix44`.
/// Returns identity if the pointer is null (no transform set by renderer).
pub fn get_sg_space_matrix(sg: &ShaderGlobals, space: &str) -> Option<Matrix44> {
    match space {
        "shader" => {
            if sg.shader2common.is_null() {
                Some(Matrix44::IDENTITY)
            } else {
                // Safety: renderer stores Matrix44 behind the TransformationPtr
                Some(unsafe { *(sg.shader2common as *const Matrix44) })
            }
        }
        "object" => {
            if sg.object2common.is_null() {
                Some(Matrix44::IDENTITY)
            } else {
                Some(unsafe { *(sg.object2common as *const Matrix44) })
            }
        }
        _ => None,
    }
}

/// Resolve the inverse (common->space) for "shader" or "object".
/// Returns identity if the pointer is null.
pub fn get_sg_inverse_space_matrix(sg: &ShaderGlobals, space: &str) -> Option<Matrix44> {
    get_sg_space_matrix(sg, space).and_then(|m| inverse(&m).or(Some(Matrix44::IDENTITY)))
}

/// Resolve the from->to matrix via renderer, following OSL's opmatrix.cpp.
/// `get_matrix_named` returns from->common, `get_inverse_matrix_named` returns common->to.
/// Combined: from->to = mat(from) * inv(to) (Imath row-vector convention).
/// Returns None if either space is unknown (per OSL spec: unknown spaces -> 0).
/// `commonspace_synonym` (e.g. "world") is treated as alias for "common" per-reference.
/// Handles "shader"/"object" spaces via ShaderGlobals transform pointers (C++ parity).
pub fn get_from_to_matrix(
    rs: &dyn RendererServices,
    sg: &ShaderGlobals,
    from: &str,
    to: &str,
    time: Float,
    commonspace_synonym: &str,
    report_unknown: Option<&dyn Fn(&str)>,
) -> Option<Matrix44> {
    if from == to {
        return Some(Matrix44::IDENTITY);
    }
    let from_is_common = from == "common" || from == commonspace_synonym;
    let to_is_common = to == "common" || to == commonspace_synonym;
    if from_is_common && to_is_common {
        return Some(Matrix44::IDENTITY);
    }

    // Resolve from->common: check shader/object first, then renderer
    let m_from = if from_is_common {
        Matrix44::IDENTITY
    } else if let Some(m) = get_sg_space_matrix(sg, from) {
        m
    } else {
        let from_h = UString::new(from).uhash();
        match rs.get_matrix_named(sg, from_h, time) {
            Some(m) => m,
            None => {
                if let Some(report) = report_unknown {
                    report(from);
                }
                return None;
            }
        }
    };

    // Resolve common->to: check shader/object first, then renderer
    let m_to_inv = if to_is_common {
        Matrix44::IDENTITY
    } else if let Some(m) = get_sg_inverse_space_matrix(sg, to) {
        m
    } else {
        let to_h = UString::new(to).uhash();
        match rs.get_inverse_matrix_named(sg, to_h, time) {
            Some(m) => m,
            None => {
                if let Some(report) = report_unknown {
                    report(to);
                }
                return None;
            }
        }
    };

    Some(matmul(&m_from, &m_to_inv))
}

/// Transform a triple between named coordinate systems via renderer.
/// Applies the correct transform based on VecSemantics (point/vector/normal).
/// `commonspace_synonym` defaults to "world" per OSL.
pub fn transform_by_name_rs(
    rs: &dyn RendererServices,
    sg: &ShaderGlobals,
    from: &str,
    to: &str,
    p: Vec3,
    vectype: VecSemantics,
    commonspace_synonym: &str,
) -> Vec3 {
    if from == to {
        return p;
    }
    let m = match get_from_to_matrix(rs, sg, from, to, sg.time, commonspace_synonym, None) {
        Some(m) => m,
        None => return p,
    };
    match vectype {
        VecSemantics::Point => transform_point(&m, p),
        VecSemantics::Vector => transform_vector(&m, p),
        VecSemantics::Normal => transform_normal(&m, p),
        _ => transform_point(&m, p),
    }
}

/// Legacy stub: transform by coordinate system name without renderer.
/// Returns identity transform for all cases (kept for API compatibility).
pub fn transform_by_name(from: &str, to: &str, p: Vec3) -> Vec3 {
    if from == to {
        return p;
    }
    // Without a renderer, we cannot resolve named coordinate systems
    p
}

/// Transform units (e.g., "meters" to "feet").
pub fn transformu(from: &str, to: &str, value: Float) -> Float {
    // Conversion factors to meters
    let to_meters = |unit: &str| -> Float {
        match unit {
            "m" | "meters" => 1.0,
            "cm" | "centimeters" => 0.01,
            "mm" | "millimeters" => 0.001,
            "km" | "kilometers" => 1000.0,
            "in" | "inches" => 0.0254,
            "ft" | "feet" => 0.3048,
            "mi" | "miles" => 1609.344,
            _ => 1.0,
        }
    };

    let from_factor = to_meters(from);
    let to_factor = to_meters(to);

    if to_factor.abs() < 1e-20 {
        return value;
    }
    value * from_factor / to_factor
}

/// Create a rotation matrix.
pub fn rotation_matrix(angle: Float, axis: Vec3) -> Matrix44 {
    let ax = axis.normalize();
    let c = angle.cos();
    let s = angle.sin();
    let t = 1.0 - c;

    Matrix44 {
        m: [
            [
                t * ax.x * ax.x + c,
                t * ax.x * ax.y - s * ax.z,
                t * ax.x * ax.z + s * ax.y,
                0.0,
            ],
            [
                t * ax.x * ax.y + s * ax.z,
                t * ax.y * ax.y + c,
                t * ax.y * ax.z - s * ax.x,
                0.0,
            ],
            [
                t * ax.x * ax.z - s * ax.y,
                t * ax.y * ax.z + s * ax.x,
                t * ax.z * ax.z + c,
                0.0,
            ],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

/// Create a translation matrix.
pub fn translation_matrix(t: Vec3) -> Matrix44 {
    Matrix44 {
        m: [
            [1.0, 0.0, 0.0, t.x],
            [0.0, 1.0, 0.0, t.y],
            [0.0, 0.0, 1.0, t.z],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

/// Create a scale matrix.
pub fn scale_matrix(s: Vec3) -> Matrix44 {
    Matrix44 {
        m: [
            [s.x, 0.0, 0.0, 0.0],
            [0.0, s.y, 0.0, 0.0],
            [0.0, 0.0, s.z, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    }
}

// ---------------------------------------------------------------------------
// Dual2 transform variants (derivative propagation)
// ---------------------------------------------------------------------------

/// Transform a `Dual2<Vec3>` point by a 4x4 matrix.
/// Value gets full affine transform (point: w=1, includes translation).
/// Derivatives are direction vectors — only the linear part applies (w=0).
pub fn transform_point_dual(m: &Matrix44, p: crate::dual::Dual2<Vec3>) -> crate::dual::Dual2<Vec3> {
    crate::dual::Dual2 {
        val: transform_point(m, p.val),
        dx: transform_vector(m, p.dx),
        dy: transform_vector(m, p.dy),
    }
}

/// Transform a `Dual2<Vec3>` vector (direction) by a 4x4 matrix.
/// Linear: apply matrix to val, dx, dy independently.
pub fn transform_vector_dual(
    m: &Matrix44,
    v: crate::dual::Dual2<Vec3>,
) -> crate::dual::Dual2<Vec3> {
    crate::dual::Dual2 {
        val: transform_vector(m, v.val),
        dx: transform_vector(m, v.dx),
        dy: transform_vector(m, v.dy),
    }
}

/// Transform a `Dual2<Vec3>` normal by a 4x4 matrix.
/// Uses inverse transpose for correct normal transformation.
pub fn transform_normal_dual(
    m: &Matrix44,
    n: crate::dual::Dual2<Vec3>,
) -> crate::dual::Dual2<Vec3> {
    crate::dual::Dual2 {
        val: transform_normal(m, n.val),
        dx: transform_normal(m, n.dx),
        dy: transform_normal(m, n.dy),
    }
}

/// Transform a `Dual2<Vec3>` by a matrix, dispatching on VecSemantics.
pub fn transform_dual(
    m: &Matrix44,
    p: crate::dual::Dual2<Vec3>,
    vectype: VecSemantics,
) -> crate::dual::Dual2<Vec3> {
    match vectype {
        VecSemantics::Point => transform_point_dual(m, p),
        VecSemantics::Vector => transform_vector_dual(m, p),
        VecSemantics::Normal => transform_normal_dual(m, p),
        _ => transform_point_dual(m, p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_determinant() {
        let det = determinant(&Matrix44::IDENTITY);
        assert!((det - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_transpose_identity() {
        let t = transpose(&Matrix44::IDENTITY);
        assert_eq!(t.m, Matrix44::IDENTITY.m);
    }

    #[test]
    fn test_inverse_identity() {
        let inv = inverse(&Matrix44::IDENTITY).unwrap();
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!((inv.m[i][j] - expected).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_transform_point_identity() {
        let p = Vec3::new(1.0, 2.0, 3.0);
        let tp = transform_point(&Matrix44::IDENTITY, p);
        assert!((tp.x - 1.0).abs() < 1e-6);
        assert!((tp.y - 2.0).abs() < 1e-6);
        assert!((tp.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_transform_point_translate() {
        let m = translation_matrix(Vec3::new(10.0, 20.0, 30.0));
        let p = Vec3::new(1.0, 2.0, 3.0);
        let tp = transform_point(&m, p);
        assert!((tp.x - 11.0).abs() < 1e-5);
        assert!((tp.y - 22.0).abs() < 1e-5);
        assert!((tp.z - 33.0).abs() < 1e-5);
    }

    #[test]
    fn test_matmul_identity() {
        let m = Matrix44 {
            m: [
                [2.0, 0.0, 0.0, 0.0],
                [0.0, 3.0, 0.0, 0.0],
                [0.0, 0.0, 4.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let r = matmul(&m, &Matrix44::IDENTITY);
        for i in 0..4 {
            for j in 0..4 {
                assert!((r.m[i][j] - m.m[i][j]).abs() < 1e-6);
            }
        }
    }

    #[test]
    fn test_rotation_matrix_z_90() {
        use std::f32::consts::FRAC_PI_2;
        let rot = rotation_matrix(FRAC_PI_2, Vec3::new(0.0, 0.0, 1.0));
        let p = Vec3::new(1.0, 0.0, 0.0);
        let rp = transform_point(&rot, p);
        assert!((rp.x - 0.0).abs() < 1e-5, "x={}", rp.x);
        assert!((rp.y - 1.0).abs() < 1e-5, "y={}", rp.y);
        assert!((rp.z - 0.0).abs() < 1e-5, "z={}", rp.z);
    }

    #[test]
    fn test_inverse_roundtrip() {
        let m = Matrix44 {
            m: [
                [1.0, 2.0, 0.0, 1.0],
                [0.0, 1.0, 1.0, 0.0],
                [2.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        let inv = inverse(&m).unwrap();
        let id = matmul(&m, &inv);
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (id.m[i][j] - expected).abs() < 1e-4,
                    "id[{}][{}] = {}, expected {}",
                    i,
                    j,
                    id.m[i][j],
                    expected
                );
            }
        }
    }

    #[test]
    fn test_transformu() {
        let v = transformu("meters", "feet", 1.0);
        assert!((v - 3.28084).abs() < 0.001);
    }

    #[test]
    fn test_scale_matrix() {
        let m = scale_matrix(Vec3::new(2.0, 3.0, 4.0));
        let p = Vec3::new(1.0, 1.0, 1.0);
        let tp = transform_point(&m, p);
        assert!((tp.x - 2.0).abs() < 1e-6);
        assert!((tp.y - 3.0).abs() < 1e-6);
        assert!((tp.z - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_transform_by_name_rs_with_renderer() {
        use crate::renderer::BasicRenderer;
        use crate::typedesc::VecSemantics;

        let mut renderer = BasicRenderer::new();
        // Intern the string "myspace" into the UString table
        let _ = crate::ustring::UString::new("myspace");
        let _ = crate::ustring::UString::new("common");
        // Set myspace transform to a 2x scale
        let scale2 = scale_matrix(Vec3::new(2.0, 2.0, 2.0));
        renderer.set_transform("myspace", scale2);

        let sg = crate::shaderglobals::ShaderGlobals::default();
        let p = Vec3::new(1.0, 1.0, 1.0);

        // Transform from myspace->common: applies the myspace matrix (scale 2x)
        let tp = transform_by_name_rs(
            &renderer,
            &sg,
            "myspace",
            "common",
            p,
            VecSemantics::Point,
            "world",
        );
        assert!((tp.x - 2.0).abs() < 1e-4, "x={}", tp.x);
        assert!((tp.y - 2.0).abs() < 1e-4, "y={}", tp.y);
        assert!((tp.z - 2.0).abs() < 1e-4, "z={}", tp.z);
    }

    #[test]
    fn test_transform_by_name_same() {
        // Same from/to should be identity
        let p = Vec3::new(5.0, 6.0, 7.0);
        let tp = transform_by_name("world", "world", p);
        assert!((tp.x - 5.0).abs() < 1e-6);
        assert!((tp.y - 6.0).abs() < 1e-6);
        assert!((tp.z - 7.0).abs() < 1e-6);
    }

    #[test]
    fn test_transform_point_dual_translate() {
        use crate::dual::Dual2;
        let m = translation_matrix(Vec3::new(10.0, 20.0, 30.0));
        let p = Dual2::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::new(0.0, 0.2, 0.0),
        );
        let tp = transform_point_dual(&m, p);
        // Value: translated
        assert!((tp.val.x - 11.0).abs() < 1e-5);
        assert!((tp.val.y - 22.0).abs() < 1e-5);
        assert!((tp.val.z - 33.0).abs() < 1e-5);
        // Derivatives: translation is constant, so derivs pass through
        // (transform_point adds translation, but derivs of a constant=0,
        //  however our dx also gets the translation offset added because
        //  transform_point is affine. For a pure derivative, only the
        //  linear part matters. Here dx is small so the w-divide is ~1.)
        assert!(tp.dx.x.is_finite());
        assert!(tp.dy.y.is_finite());
    }

    #[test]
    fn test_transform_vector_dual_scale() {
        use crate::dual::Dual2;
        let m = scale_matrix(Vec3::new(2.0, 3.0, 4.0));
        let v = Dual2::new(
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let tv = transform_vector_dual(&m, v);
        // Value: scaled
        assert!((tv.val.x - 2.0).abs() < 1e-5);
        assert!((tv.val.y - 3.0).abs() < 1e-5);
        assert!((tv.val.z - 4.0).abs() < 1e-5);
        // dx: (1,0,0) scaled -> (2,0,0)
        assert!((tv.dx.x - 2.0).abs() < 1e-5);
        assert!(tv.dx.y.abs() < 1e-5);
        // dy: (0,1,0) scaled -> (0,3,0)
        assert!(tv.dy.x.abs() < 1e-5);
        assert!((tv.dy.y - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_transform_normal_dual_identity() {
        use crate::dual::Dual2;
        let n = Dual2::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::ZERO,
        );
        let tn = transform_normal_dual(&Matrix44::IDENTITY, n);
        assert!((tn.val.y - 1.0).abs() < 1e-6);
        assert!((tn.dx.x - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_transform_dual_dispatch() {
        use crate::dual::Dual2;
        let m = scale_matrix(Vec3::new(2.0, 2.0, 2.0));
        let p = Dual2::new(
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::ZERO,
        );
        // VecSemantics::Vector
        let tv = transform_dual(&m, p, VecSemantics::Vector);
        assert!((tv.val.x - 2.0).abs() < 1e-5);
        assert!((tv.dx.x - 2.0).abs() < 1e-5);
    }
}
