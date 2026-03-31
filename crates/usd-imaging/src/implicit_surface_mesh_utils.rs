//! Implicit surface mesh generation utilities.
//!
//! Port of pxr/usdImaging/usdImaging/implicitSurfaceMeshUtils.h/cpp
//!
//! Provides hardcoded mesh topology and vertex positions for implicit surfaces:
//! sphere, cube, cone, cylinder, capsule, and plane. Matches C++ reference exactly.

use std::f32::consts::PI;
use usd_gf::{Matrix4d, Vec3f};
use usd_tf::Token;

// ============================================================================
// Types
// ============================================================================

/// Axis enumeration for oriented primitives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// Parse axis from token string ("X", "Y", "Z").
    pub fn from_token(token: &Token) -> Option<Self> {
        match token.as_str() {
            "X" => Some(Axis::X),
            "Y" => Some(Axis::Y),
            "Z" => Some(Axis::Z),
            _ => None,
        }
    }

    /// Convert axis to token.
    pub fn to_token(&self) -> Token {
        Token::new(match self {
            Axis::X => "X",
            Axis::Y => "Y",
            Axis::Z => "Z",
        })
    }
}

/// Mesh topology: face counts, vertex indices, subdivision scheme.
#[derive(Debug, Clone)]
pub struct MeshTopology {
    pub face_vertex_counts: Vec<i32>,
    pub face_vertex_indices: Vec<i32>,
    /// "catmullClark" or "bilinear".
    pub subdivision_scheme: Token,
}

impl MeshTopology {
    pub fn new(
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        subdivision_scheme: Token,
    ) -> Self {
        Self {
            face_vertex_counts,
            face_vertex_indices,
            subdivision_scheme,
        }
    }

    pub fn num_faces(&self) -> usize {
        self.face_vertex_counts.len()
    }

    pub fn num_face_vertices(&self) -> usize {
        self.face_vertex_indices.len()
    }
}

// ============================================================================
// Sphere — 92 hardcoded points, 100 faces (80 quads + 20 tris)
// ============================================================================

/// Unit sphere mesh topology (catmullClark, rightHanded).
/// 80 quad faces + 20 triangle faces = 100 faces total.
pub fn get_unit_sphere_topology() -> MeshTopology {
    #[rustfmt::skip]
    let face_vertex_counts: Vec<i32> = vec![
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,  4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,  4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,  4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,  4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        3, 3, 3, 3, 3, 3, 3, 3, 3, 3,  3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    ];

    #[rustfmt::skip]
    let face_vertex_indices: Vec<i32> = vec![
        // 80 quads
         0,  1, 11, 10,    1,  2, 12, 11,    2,  3, 13, 12,    3,  4, 14, 13,
         4,  5, 15, 14,    5,  6, 16, 15,    6,  7, 17, 16,    7,  8, 18, 17,
         8,  9, 19, 18,    9,  0, 10, 19,   10, 11, 21, 20,   11, 12, 22, 21,
        12, 13, 23, 22,   13, 14, 24, 23,   14, 15, 25, 24,   15, 16, 26, 25,
        16, 17, 27, 26,   17, 18, 28, 27,   18, 19, 29, 28,   19, 10, 20, 29,
        20, 21, 31, 30,   21, 22, 32, 31,   22, 23, 33, 32,   23, 24, 34, 33,
        24, 25, 35, 34,   25, 26, 36, 35,   26, 27, 37, 36,   27, 28, 38, 37,
        28, 29, 39, 38,   29, 20, 30, 39,   30, 31, 41, 40,   31, 32, 42, 41,
        32, 33, 43, 42,   33, 34, 44, 43,   34, 35, 45, 44,   35, 36, 46, 45,
        36, 37, 47, 46,   37, 38, 48, 47,   38, 39, 49, 48,   39, 30, 40, 49,
        40, 41, 51, 50,   41, 42, 52, 51,   42, 43, 53, 52,   43, 44, 54, 53,
        44, 45, 55, 54,   45, 46, 56, 55,   46, 47, 57, 56,   47, 48, 58, 57,
        48, 49, 59, 58,   49, 40, 50, 59,   50, 51, 61, 60,   51, 52, 62, 61,
        52, 53, 63, 62,   53, 54, 64, 63,   54, 55, 65, 64,   55, 56, 66, 65,
        56, 57, 67, 66,   57, 58, 68, 67,   58, 59, 69, 68,   59, 50, 60, 69,
        60, 61, 71, 70,   61, 62, 72, 71,   62, 63, 73, 72,   63, 64, 74, 73,
        64, 65, 75, 74,   65, 66, 76, 75,   66, 67, 77, 76,   67, 68, 78, 77,
        68, 69, 79, 78,   69, 60, 70, 79,   70, 71, 81, 80,   71, 72, 82, 81,
        72, 73, 83, 82,   73, 74, 84, 83,   74, 75, 85, 84,   75, 76, 86, 85,
        76, 77, 87, 86,   77, 78, 88, 87,   78, 79, 89, 88,   79, 70, 80, 89,
        // 20 tris: 10 bottom cap + 10 top cap
         1,  0, 90,    2,  1, 90,    3,  2, 90,    4,  3, 90,    5,  4, 90,
         6,  5, 90,    7,  6, 90,    8,  7, 90,    9,  8, 90,    0,  9, 90,
        80, 81, 91,   81, 82, 91,   82, 83, 91,   83, 84, 91,   84, 85, 91,
        85, 86, 91,   86, 87, 91,   87, 88, 91,   88, 89, 91,   89, 80, 91,
    ];

    MeshTopology::new(
        face_vertex_counts,
        face_vertex_indices,
        Token::new("catmullClark"),
    )
}

/// Unit sphere hardcoded points (92 vertices, UV sphere with pole caps).
#[rustfmt::skip]
pub fn get_unit_sphere_points() -> Vec<Vec3f> {
    vec![
        Vec3f::new( 0.1250,  0.0908, -0.4755), Vec3f::new( 0.0477,  0.1469, -0.4755),
        Vec3f::new(-0.0477,  0.1469, -0.4755), Vec3f::new(-0.1250,  0.0908, -0.4755),
        Vec3f::new(-0.1545, -0.0000, -0.4755), Vec3f::new(-0.1250, -0.0908, -0.4755),
        Vec3f::new(-0.0477, -0.1469, -0.4755), Vec3f::new( 0.0477, -0.1469, -0.4755),
        Vec3f::new( 0.1250, -0.0908, -0.4755), Vec3f::new( 0.1545, -0.0000, -0.4755),
        Vec3f::new( 0.2378,  0.1727, -0.4045), Vec3f::new( 0.0908,  0.2795, -0.4045),
        Vec3f::new(-0.0908,  0.2795, -0.4045), Vec3f::new(-0.2378,  0.1727, -0.4045),
        Vec3f::new(-0.2939, -0.0000, -0.4045), Vec3f::new(-0.2378, -0.1727, -0.4045),
        Vec3f::new(-0.0908, -0.2795, -0.4045), Vec3f::new( 0.0908, -0.2795, -0.4045),
        Vec3f::new( 0.2378, -0.1727, -0.4045), Vec3f::new( 0.2939, -0.0000, -0.4045),
        Vec3f::new( 0.3273,  0.2378, -0.2939), Vec3f::new( 0.1250,  0.3847, -0.2939),
        Vec3f::new(-0.1250,  0.3847, -0.2939), Vec3f::new(-0.3273,  0.2378, -0.2939),
        Vec3f::new(-0.4045, -0.0000, -0.2939), Vec3f::new(-0.3273, -0.2378, -0.2939),
        Vec3f::new(-0.1250, -0.3847, -0.2939), Vec3f::new( 0.1250, -0.3847, -0.2939),
        Vec3f::new( 0.3273, -0.2378, -0.2939), Vec3f::new( 0.4045, -0.0000, -0.2939),
        Vec3f::new( 0.3847,  0.2795, -0.1545), Vec3f::new( 0.1469,  0.4523, -0.1545),
        Vec3f::new(-0.1469,  0.4523, -0.1545), Vec3f::new(-0.3847,  0.2795, -0.1545),
        Vec3f::new(-0.4755, -0.0000, -0.1545), Vec3f::new(-0.3847, -0.2795, -0.1545),
        Vec3f::new(-0.1469, -0.4523, -0.1545), Vec3f::new( 0.1469, -0.4523, -0.1545),
        Vec3f::new( 0.3847, -0.2795, -0.1545), Vec3f::new( 0.4755, -0.0000, -0.1545),
        Vec3f::new( 0.4045,  0.2939, -0.0000), Vec3f::new( 0.1545,  0.4755, -0.0000),
        Vec3f::new(-0.1545,  0.4755, -0.0000), Vec3f::new(-0.4045,  0.2939, -0.0000),
        Vec3f::new(-0.5000, -0.0000,  0.0000), Vec3f::new(-0.4045, -0.2939,  0.0000),
        Vec3f::new(-0.1545, -0.4755,  0.0000), Vec3f::new( 0.1545, -0.4755,  0.0000),
        Vec3f::new( 0.4045, -0.2939,  0.0000), Vec3f::new( 0.5000,  0.0000,  0.0000),
        Vec3f::new( 0.3847,  0.2795,  0.1545), Vec3f::new( 0.1469,  0.4523,  0.1545),
        Vec3f::new(-0.1469,  0.4523,  0.1545), Vec3f::new(-0.3847,  0.2795,  0.1545),
        Vec3f::new(-0.4755, -0.0000,  0.1545), Vec3f::new(-0.3847, -0.2795,  0.1545),
        Vec3f::new(-0.1469, -0.4523,  0.1545), Vec3f::new( 0.1469, -0.4523,  0.1545),
        Vec3f::new( 0.3847, -0.2795,  0.1545), Vec3f::new( 0.4755,  0.0000,  0.1545),
        Vec3f::new( 0.3273,  0.2378,  0.2939), Vec3f::new( 0.1250,  0.3847,  0.2939),
        Vec3f::new(-0.1250,  0.3847,  0.2939), Vec3f::new(-0.3273,  0.2378,  0.2939),
        Vec3f::new(-0.4045, -0.0000,  0.2939), Vec3f::new(-0.3273, -0.2378,  0.2939),
        Vec3f::new(-0.1250, -0.3847,  0.2939), Vec3f::new( 0.1250, -0.3847,  0.2939),
        Vec3f::new( 0.3273, -0.2378,  0.2939), Vec3f::new( 0.4045,  0.0000,  0.2939),
        Vec3f::new( 0.2378,  0.1727,  0.4045), Vec3f::new( 0.0908,  0.2795,  0.4045),
        Vec3f::new(-0.0908,  0.2795,  0.4045), Vec3f::new(-0.2378,  0.1727,  0.4045),
        Vec3f::new(-0.2939, -0.0000,  0.4045), Vec3f::new(-0.2378, -0.1727,  0.4045),
        Vec3f::new(-0.0908, -0.2795,  0.4045), Vec3f::new( 0.0908, -0.2795,  0.4045),
        Vec3f::new( 0.2378, -0.1727,  0.4045), Vec3f::new( 0.2939,  0.0000,  0.4045),
        Vec3f::new( 0.1250,  0.0908,  0.4755), Vec3f::new( 0.0477,  0.1469,  0.4755),
        Vec3f::new(-0.0477,  0.1469,  0.4755), Vec3f::new(-0.1250,  0.0908,  0.4755),
        Vec3f::new(-0.1545, -0.0000,  0.4755), Vec3f::new(-0.1250, -0.0908,  0.4755),
        Vec3f::new(-0.0477, -0.1469,  0.4755), Vec3f::new( 0.0477, -0.1469,  0.4755),
        Vec3f::new( 0.1250, -0.0908,  0.4755), Vec3f::new( 0.1545,  0.0000,  0.4755),
        Vec3f::new( 0.0000, -0.0000, -0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
    ]
}

// ============================================================================
// Cube — 8 points at +/-0.5, 6 quad faces (bilinear)
// ============================================================================

/// Unit cube mesh topology (bilinear, rightHanded). 6 quad faces.
pub fn get_unit_cube_topology() -> MeshTopology {
    let face_vertex_counts = vec![4i32; 6];

    #[rustfmt::skip]
    let face_vertex_indices: Vec<i32> = vec![
        0, 1, 2, 3,
        4, 5, 6, 7,
        0, 6, 5, 1,
        4, 7, 3, 2,
        0, 3, 7, 6,
        4, 2, 1, 5,
    ];

    MeshTopology::new(
        face_vertex_counts,
        face_vertex_indices,
        Token::new("bilinear"),
    )
}

/// Unit cube hardcoded points (8 vertices at +/-0.5).
#[rustfmt::skip]
pub fn get_unit_cube_points() -> Vec<Vec3f> {
    vec![
        Vec3f::new( 0.5,  0.5,  0.5),
        Vec3f::new(-0.5,  0.5,  0.5),
        Vec3f::new(-0.5, -0.5,  0.5),
        Vec3f::new( 0.5, -0.5,  0.5),
        Vec3f::new(-0.5, -0.5, -0.5),
        Vec3f::new(-0.5,  0.5, -0.5),
        Vec3f::new( 0.5,  0.5, -0.5),
        Vec3f::new( 0.5, -0.5, -0.5),
    ]
}

// ============================================================================
// Cone — 31 hardcoded points, 20 faces (10 tris + 10 quads)
// ============================================================================

/// Unit cone mesh topology (catmullClark, rightHanded). 10 tris + 10 quads = 20 faces.
pub fn get_unit_cone_topology() -> MeshTopology {
    #[rustfmt::skip]
    let face_vertex_counts: Vec<i32> = vec![
        3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
    ];

    #[rustfmt::skip]
    let face_vertex_indices: Vec<i32> = vec![
        // 10 base cap tris (center pt 0, ring pts 1..10)
         2,  1,  0,    3,  2,  0,    4,  3,  0,    5,  4,  0,    6,  5,  0,
         7,  6,  0,    8,  7,  0,    9,  8,  0,   10,  9,  0,    1, 10,  0,
        // 10 side quads (bottom ring dup 11..20, apex ring dup 21..30)
        11, 12, 22, 21,   12, 13, 23, 22,   13, 14, 24, 23,   14, 15, 25, 24,
        15, 16, 26, 25,   16, 17, 27, 26,   17, 18, 28, 27,   18, 19, 29, 28,
        19, 20, 30, 29,   20, 11, 21, 30,
    ];

    MeshTopology::new(
        face_vertex_counts,
        face_vertex_indices,
        Token::new("catmullClark"),
    )
}

/// Unit cone hardcoded points (31 vertices).
/// pt 0 = bottom center, pts 1-10 = bottom ring, pts 11-20 = bottom ring dup,
/// pts 21-30 = apex ring (all at top center z=0.5).
#[rustfmt::skip]
pub fn get_unit_cone_points() -> Vec<Vec3f> {
    vec![
        Vec3f::new( 0.0000,  0.0000, -0.5000), Vec3f::new( 0.5000,  0.0000, -0.5000),
        Vec3f::new( 0.4045,  0.2939, -0.5000), Vec3f::new( 0.1545,  0.4755, -0.5000),
        Vec3f::new(-0.1545,  0.4755, -0.5000), Vec3f::new(-0.4045,  0.2939, -0.5000),
        Vec3f::new(-0.5000,  0.0000, -0.5000), Vec3f::new(-0.4045, -0.2939, -0.5000),
        Vec3f::new(-0.1545, -0.4755, -0.5000), Vec3f::new( 0.1545, -0.4755, -0.5000),
        Vec3f::new( 0.4045, -0.2939, -0.5000), Vec3f::new( 0.5000,  0.0000, -0.5000),
        Vec3f::new( 0.4045,  0.2939, -0.5000), Vec3f::new( 0.1545,  0.4755, -0.5000),
        Vec3f::new(-0.1545,  0.4755, -0.5000), Vec3f::new(-0.4045,  0.2939, -0.5000),
        Vec3f::new(-0.5000,  0.0000, -0.5000), Vec3f::new(-0.4045, -0.2939, -0.5000),
        Vec3f::new(-0.1545, -0.4755, -0.5000), Vec3f::new( 0.1545, -0.4755, -0.5000),
        Vec3f::new( 0.4045, -0.2939, -0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
        Vec3f::new( 0.0000,  0.0000,  0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
        Vec3f::new( 0.0000,  0.0000,  0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
        Vec3f::new( 0.0000,  0.0000,  0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
        Vec3f::new( 0.0000,  0.0000,  0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
        Vec3f::new( 0.0000,  0.0000,  0.5000),
    ]
}

// ============================================================================
// Cylinder — 42 hardcoded points, 30 faces (10 tris + 10 quads + 10 tris)
// ============================================================================

/// Unit cylinder mesh topology (catmullClark, rightHanded). 10+10+10 = 30 faces.
pub fn get_unit_cylinder_topology() -> MeshTopology {
    #[rustfmt::skip]
    let face_vertex_counts: Vec<i32> = vec![
        3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
        4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    ];

    #[rustfmt::skip]
    let face_vertex_indices: Vec<i32> = vec![
        // 10 bottom cap tris
         2,  1,  0,    3,  2,  0,    4,  3,  0,    5,  4,  0,    6,  5,  0,
         7,  6,  0,    8,  7,  0,    9,  8,  0,   10,  9,  0,    1, 10,  0,
        // 10 side quads
        11, 12, 22, 21,   12, 13, 23, 22,   13, 14, 24, 23,   14, 15, 25, 24,
        15, 16, 26, 25,   16, 17, 27, 26,   17, 18, 28, 27,   18, 19, 29, 28,
        19, 20, 30, 29,   20, 11, 21, 30,
        // 10 top cap tris
        31, 32, 41,   32, 33, 41,   33, 34, 41,   34, 35, 41,   35, 36, 41,
        36, 37, 41,   37, 38, 41,   38, 39, 41,   39, 40, 41,   40, 31, 41,
    ];

    MeshTopology::new(
        face_vertex_counts,
        face_vertex_indices,
        Token::new("catmullClark"),
    )
}

/// Unit cylinder hardcoded points (42 vertices).
/// pt 0 = bottom center, pts 1-10 = bottom ring, pts 11-20 = bottom ring dup,
/// pts 21-30 = top ring, pts 31-40 = top ring dup, pt 41 = top center.
#[rustfmt::skip]
pub fn get_unit_cylinder_points() -> Vec<Vec3f> {
    vec![
        Vec3f::new( 0.0000,  0.0000, -0.5000), Vec3f::new( 0.5000,  0.0000, -0.5000),
        Vec3f::new( 0.4045,  0.2939, -0.5000), Vec3f::new( 0.1545,  0.4755, -0.5000),
        Vec3f::new(-0.1545,  0.4755, -0.5000), Vec3f::new(-0.4045,  0.2939, -0.5000),
        Vec3f::new(-0.5000,  0.0000, -0.5000), Vec3f::new(-0.4045, -0.2939, -0.5000),
        Vec3f::new(-0.1545, -0.4755, -0.5000), Vec3f::new( 0.1545, -0.4755, -0.5000),
        Vec3f::new( 0.4045, -0.2939, -0.5000), Vec3f::new( 0.5000,  0.0000, -0.5000),
        Vec3f::new( 0.4045,  0.2939, -0.5000), Vec3f::new( 0.1545,  0.4755, -0.5000),
        Vec3f::new(-0.1545,  0.4755, -0.5000), Vec3f::new(-0.4045,  0.2939, -0.5000),
        Vec3f::new(-0.5000,  0.0000, -0.5000), Vec3f::new(-0.4045, -0.2939, -0.5000),
        Vec3f::new(-0.1545, -0.4755, -0.5000), Vec3f::new( 0.1545, -0.4755, -0.5000),
        Vec3f::new( 0.4045, -0.2939, -0.5000), Vec3f::new( 0.5000,  0.0000,  0.5000),
        Vec3f::new( 0.4045,  0.2939,  0.5000), Vec3f::new( 0.1545,  0.4755,  0.5000),
        Vec3f::new(-0.1545,  0.4755,  0.5000), Vec3f::new(-0.4045,  0.2939,  0.5000),
        Vec3f::new(-0.5000,  0.0000,  0.5000), Vec3f::new(-0.4045, -0.2939,  0.5000),
        Vec3f::new(-0.1545, -0.4755,  0.5000), Vec3f::new( 0.1545, -0.4755,  0.5000),
        Vec3f::new( 0.4045, -0.2939,  0.5000), Vec3f::new( 0.5000,  0.0000,  0.5000),
        Vec3f::new( 0.4045,  0.2939,  0.5000), Vec3f::new( 0.1545,  0.4755,  0.5000),
        Vec3f::new(-0.1545,  0.4755,  0.5000), Vec3f::new(-0.4045,  0.2939,  0.5000),
        Vec3f::new(-0.5000,  0.0000,  0.5000), Vec3f::new(-0.4045, -0.2939,  0.5000),
        Vec3f::new(-0.1545, -0.4755,  0.5000), Vec3f::new( 0.1545, -0.4755,  0.5000),
        Vec3f::new( 0.4045, -0.2939,  0.5000), Vec3f::new( 0.0000,  0.0000,  0.5000),
    ]
}

// ============================================================================
// Capsule — procedural (slices=10, stacks=1, capStacks=4)
// ============================================================================

const CAPSULE_SLICES: usize = 10;
const CAPSULE_STACKS: usize = 1;
const CAPSULE_CAP_STACKS: usize = 4;

/// Capsule mesh topology (catmullClark, rightHanded). Procedurally generated.
pub fn get_capsule_topology() -> MeshTopology {
    let num_counts = CAPSULE_SLICES * (CAPSULE_STACKS + 2 * CAPSULE_CAP_STACKS);
    let num_indices = 4 * CAPSULE_SLICES * CAPSULE_STACKS                       // cylinder quads
        + 4 * 2 * CAPSULE_SLICES * (CAPSULE_CAP_STACKS - 1)                     // hemisphere quads
        + 3 * 2 * CAPSULE_SLICES; // end cap tris

    let mut counts: Vec<i32> = Vec::with_capacity(num_counts);
    let mut indices: Vec<i32> = Vec::with_capacity(num_indices);

    let mut p: i32 = 0;

    // Base hemisphere end cap triangles
    let base = p;
    p += 1;
    for i in 0..CAPSULE_SLICES {
        counts.push(3);
        indices.push(p + ((i + 1) % CAPSULE_SLICES) as i32);
        indices.push(p + i as i32);
        indices.push(base);
    }

    // Middle cylinder + hemisphere quads
    for _ in 0..(CAPSULE_STACKS + 2 * (CAPSULE_CAP_STACKS - 1)) {
        for j in 0..CAPSULE_SLICES {
            let x0: i32 = 0;
            let x1 = CAPSULE_SLICES as i32;
            let y0 = j as i32;
            let y1 = ((j + 1) % CAPSULE_SLICES) as i32;
            counts.push(4);
            indices.push(p + x0 + y0);
            indices.push(p + x0 + y1);
            indices.push(p + x1 + y1);
            indices.push(p + x1 + y0);
        }
        p += CAPSULE_SLICES as i32;
    }

    // Top hemisphere end cap triangles
    let top = p + CAPSULE_SLICES as i32;
    for i in 0..CAPSULE_SLICES {
        counts.push(3);
        indices.push(p + i as i32);
        indices.push(p + ((i + 1) % CAPSULE_SLICES) as i32);
        indices.push(top);
    }

    debug_assert_eq!(counts.len(), num_counts);
    debug_assert_eq!(indices.len(), num_indices);

    MeshTopology::new(counts, indices, Token::new("catmullClark"))
}

/// Generate capsule vertex positions.
///
/// # Arguments
/// * `height` - Spine height (distance between hemisphere centers)
/// * `radius` - Capsule radius
/// * `axis` - Spine axis token ("X", "Y", or "Z")
pub fn gen_capsule_points(height: f64, radius: f64, axis: &Token) -> Vec<Vec3f> {
    let r = radius as f32;
    let h = height as f32;

    // Basis vectors aligned with the spine axis
    let (u, v, spine): (Vec3f, Vec3f, Vec3f) = match axis.as_str() {
        "X" => (
            Vec3f::new(0.0, 1.0, 0.0),
            Vec3f::new(0.0, 0.0, 1.0),
            Vec3f::new(1.0, 0.0, 0.0),
        ),
        "Y" => (
            Vec3f::new(0.0, 0.0, 1.0),
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
        ),
        _ => (
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
            Vec3f::new(0.0, 0.0, 1.0),
        ),
    };

    // Unit ring in the uv plane
    let ring: Vec<Vec3f> = (0..CAPSULE_SLICES)
        .map(|i| {
            let a = 2.0 * PI * (i as f32) / (CAPSULE_SLICES as f32);
            u * a.cos() + v * a.sin()
        })
        .collect();

    let num_points = CAPSULE_SLICES * (CAPSULE_STACKS + 1)          // cylinder rings
        + 2 * CAPSULE_SLICES * (CAPSULE_CAP_STACKS - 1)             // hemisphere rings
        + 2; // poles

    let mut pts: Vec<Vec3f> = Vec::with_capacity(num_points);

    // Base hemisphere pole
    pts.push(spine * (-h / 2.0 - r));

    // Base hemisphere rings
    for i in 0..(CAPSULE_CAP_STACKS - 1) {
        let a = (PI / 2.0) * (1.0 - (i + 1) as f32 / CAPSULE_CAP_STACKS as f32);
        let rr = r * a.cos();
        let w = r * a.sin();
        for j in 0..CAPSULE_SLICES {
            pts.push(ring[j] * rr + spine * (-h / 2.0 - w));
        }
    }

    // Cylinder middle rings
    for i in 0..=CAPSULE_STACKS {
        let t = i as f32 / CAPSULE_STACKS as f32;
        let w = h * (t - 0.5);
        for j in 0..CAPSULE_SLICES {
            pts.push(ring[j] * r + spine * w);
        }
    }

    // Top hemisphere rings
    for i in 0..(CAPSULE_CAP_STACKS - 1) {
        let a = (PI / 2.0) * ((i + 1) as f32 / CAPSULE_CAP_STACKS as f32);
        let rr = r * a.cos();
        let w = r * a.sin();
        for j in 0..CAPSULE_SLICES {
            pts.push(ring[j] * rr + spine * (h / 2.0 + w));
        }
    }

    // Top hemisphere pole
    pts.push(spine * (h / 2.0 + r));

    debug_assert_eq!(pts.len(), num_points);
    pts
}

// ============================================================================
// Plane — 4 points, 1 quad face (bilinear)
// ============================================================================

/// Plane mesh topology (bilinear, rightHanded). 1 quad face.
pub fn get_plane_topology() -> MeshTopology {
    MeshTopology::new(vec![4], vec![0, 1, 2, 3], Token::new("bilinear"))
}

/// Generate plane vertex positions.
///
/// # Arguments
/// * `width` - Plane width
/// * `length` - Plane length
/// * `axis` - Normal axis token ("X", "Y", or "Z")
pub fn gen_plane_points(width: f64, length: f64, axis: &Token) -> Vec<Vec3f> {
    let w = (width * 0.5) as f32;
    let l = (length * 0.5) as f32;

    match axis.as_str() {
        "X" => vec![
            Vec3f::new(0.0, l, w),
            Vec3f::new(0.0, -l, w),
            Vec3f::new(0.0, -l, -w),
            Vec3f::new(0.0, l, -w),
        ],
        "Y" => vec![
            Vec3f::new(-w, 0.0, l),
            Vec3f::new(w, 0.0, l),
            Vec3f::new(w, 0.0, -l),
            Vec3f::new(-w, 0.0, -l),
        ],
        _ => vec![
            // Z axis (default)
            Vec3f::new(w, l, 0.0),
            Vec3f::new(-w, l, 0.0),
            Vec3f::new(-w, -l, 0.0),
            Vec3f::new(w, -l, 0.0),
        ],
    }
}

// ============================================================================
// Transforms
// ============================================================================

/// Uniform scale transform for sphere or cube (size = diameter or edge length).
pub fn gen_sphere_or_cube_xform(size: f64) -> Matrix4d {
    Matrix4d::new(
        size, 0.0, 0.0, 0.0, 0.0, size, 0.0, 0.0, 0.0, 0.0, size, 0.0, 0.0, 0.0, 0.0, 1.0,
    )
}

/// Scale+axis-remap transform for cone or cylinder.
///
/// The hardcoded mesh lives in Z-axis space (z in [-0.5, 0.5], xy radius=0.5).
/// This matrix remaps it to the desired axis with `diameter = 2*radius` and `height`.
pub fn gen_cone_or_cylinder_xform(height: f64, radius: f64, axis: &Token) -> Matrix4d {
    let d = 2.0 * radius; // diameter
    match axis.as_str() {
        "X" => Matrix4d::new(
            0.0, d, 0.0, 0.0, 0.0, 0.0, d, 0.0, height, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ),
        "Y" => Matrix4d::new(
            0.0, 0.0, d, 0.0, d, 0.0, 0.0, 0.0, 0.0, height, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ),
        _ => Matrix4d::new(
            // Z axis (default)
            d, 0.0, 0.0, 0.0, 0.0, d, 0.0, 0.0, 0.0, 0.0, height, 0.0, 0.0, 0.0, 0.0, 1.0,
        ),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_conversion() {
        assert_eq!(Axis::from_token(&Token::new("X")), Some(Axis::X));
        assert_eq!(Axis::from_token(&Token::new("Y")), Some(Axis::Y));
        assert_eq!(Axis::from_token(&Token::new("Z")), Some(Axis::Z));
        assert_eq!(Axis::from_token(&Token::new("W")), None);
    }

    // --- Sphere ---

    #[test]
    fn test_sphere_topology() {
        let topo = get_unit_sphere_topology();
        // 80 quads + 20 tris = 100 faces
        assert_eq!(topo.num_faces(), 100);
        assert_eq!(topo.subdivision_scheme.as_str(), "catmullClark");
        // 80*4 + 20*3 = 320 + 60 = 380 indices
        assert_eq!(topo.num_face_vertices(), 380);
        // First 80 faces are quads
        assert!(topo.face_vertex_counts[..80].iter().all(|&c| c == 4));
        // Last 20 faces are tris
        assert!(topo.face_vertex_counts[80..].iter().all(|&c| c == 3));
    }

    #[test]
    fn test_sphere_points() {
        let pts = get_unit_sphere_points();
        assert_eq!(pts.len(), 92);
        // First point should be (0.1250, 0.0908, -0.4755)
        assert!((pts[0].x - 0.1250).abs() < 1e-4);
        assert!((pts[0].y - 0.0908).abs() < 1e-4);
        assert!((pts[0].z - (-0.4755)).abs() < 1e-4);
        // South pole: pt 90 = (0, 0, -0.5)
        assert!((pts[90].z - (-0.5000)).abs() < 1e-4);
        // North pole: pt 91 = (0, 0, 0.5)
        assert!((pts[91].z - 0.5000).abs() < 1e-4);
    }

    // --- Cube ---

    #[test]
    fn test_cube_topology() {
        let topo = get_unit_cube_topology();
        assert_eq!(topo.num_faces(), 6);
        assert_eq!(topo.subdivision_scheme.as_str(), "bilinear");
        assert!(topo.face_vertex_counts.iter().all(|&c| c == 4));
        // Verify exact indices match C++ reference
        let expected: Vec<i32> = vec![
            0, 1, 2, 3, 4, 5, 6, 7, 0, 6, 5, 1, 4, 7, 3, 2, 0, 3, 7, 6, 4, 2, 1, 5,
        ];
        assert_eq!(topo.face_vertex_indices, expected);
    }

    #[test]
    fn test_cube_points() {
        let pts = get_unit_cube_points();
        assert_eq!(pts.len(), 8);
        // All coords should be exactly +/-0.5
        for p in &pts {
            assert!((p.x.abs() - 0.5).abs() < 1e-6);
            assert!((p.y.abs() - 0.5).abs() < 1e-6);
            assert!((p.z.abs() - 0.5).abs() < 1e-6);
        }
        // First point is (0.5, 0.5, 0.5)
        assert_eq!(pts[0], Vec3f::new(0.5, 0.5, 0.5));
    }

    // --- Cone ---

    #[test]
    fn test_cone_topology() {
        let topo = get_unit_cone_topology();
        assert_eq!(topo.num_faces(), 20); // 10 tris + 10 quads
        assert_eq!(topo.subdivision_scheme.as_str(), "catmullClark");
        // First 10 faces are tris
        assert!(topo.face_vertex_counts[..10].iter().all(|&c| c == 3));
        // Last 10 faces are quads
        assert!(topo.face_vertex_counts[10..].iter().all(|&c| c == 4));
        // 10*3 + 10*4 = 70 indices
        assert_eq!(topo.num_face_vertices(), 70);
    }

    #[test]
    fn test_cone_points() {
        let pts = get_unit_cone_points();
        assert_eq!(pts.len(), 31);
        // pt 0 = bottom center
        assert_eq!(pts[0], Vec3f::new(0.0, 0.0, -0.5));
        // pts 21-30 = apex (all at (0,0,0.5))
        for i in 21..=30 {
            assert_eq!(pts[i], Vec3f::new(0.0, 0.0, 0.5));
        }
    }

    // --- Cylinder ---

    #[test]
    fn test_cylinder_topology() {
        let topo = get_unit_cylinder_topology();
        assert_eq!(topo.num_faces(), 30); // 10+10+10
        assert_eq!(topo.subdivision_scheme.as_str(), "catmullClark");
        assert!(topo.face_vertex_counts[..10].iter().all(|&c| c == 3));
        assert!(topo.face_vertex_counts[10..20].iter().all(|&c| c == 4));
        assert!(topo.face_vertex_counts[20..].iter().all(|&c| c == 3));
        // 10*3 + 10*4 + 10*3 = 100 indices
        assert_eq!(topo.num_face_vertices(), 100);
    }

    #[test]
    fn test_cylinder_points() {
        let pts = get_unit_cylinder_points();
        assert_eq!(pts.len(), 42);
        // pt 0 = bottom center
        assert_eq!(pts[0], Vec3f::new(0.0, 0.0, -0.5));
        // pt 41 = top center
        assert_eq!(pts[41], Vec3f::new(0.0, 0.0, 0.5));
    }

    // --- Capsule ---

    #[test]
    fn test_capsule_topology() {
        let topo = get_capsule_topology();
        // slices=10, stacks=1, capStacks=4
        // counts = 10*(1 + 2*4) = 10*9 = 90 faces
        assert_eq!(topo.num_faces(), 90);
        assert_eq!(topo.subdivision_scheme.as_str(), "catmullClark");
        // 2 end caps: 2*10*3 = 60 tri indices
        // quads: (stacks + 2*(capStacks-1)) * slices * 4 = (1+6)*10*4 = 280
        // total = 60 + 280 = 340
        assert_eq!(topo.num_face_vertices(), 340);
    }

    #[test]
    fn test_capsule_points_z() {
        let axis = Token::new("Z");
        let pts = gen_capsule_points(2.0, 0.5, &axis);
        // num = slices*(stacks+1) + 2*slices*(capStacks-1) + 2
        // = 10*2 + 2*10*3 + 2 = 20 + 60 + 2 = 82
        assert_eq!(pts.len(), 82);
        // South pole: spine*(-h/2 - r) = Z*(−1.0 − 0.5) = (0,0,−1.5)
        assert!((pts[0].z - (-1.5)).abs() < 1e-5);
        // North pole: last point = (0,0,1.5)
        assert!((pts.last().unwrap().z - 1.5).abs() < 1e-5);
    }

    #[test]
    fn test_capsule_points_x() {
        let axis = Token::new("X");
        let pts = gen_capsule_points(2.0, 0.5, &axis);
        assert_eq!(pts.len(), 82);
        // South pole along X: (-1.5, 0, 0)
        assert!((pts[0].x - (-1.5)).abs() < 1e-5);
        assert!((pts.last().unwrap().x - 1.5).abs() < 1e-5);
    }

    // --- Plane ---

    #[test]
    fn test_plane_topology() {
        let topo = get_plane_topology();
        assert_eq!(topo.num_faces(), 1);
        assert_eq!(topo.face_vertex_counts[0], 4);
        assert_eq!(topo.subdivision_scheme.as_str(), "bilinear");
        assert_eq!(topo.face_vertex_indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_plane_points_y() {
        let axis = Token::new("Y");
        let pts = gen_plane_points(2.0, 4.0, &axis);
        assert_eq!(pts.len(), 4);
        for p in &pts {
            assert_eq!(p.y, 0.0);
        }
        // width=2 -> half=1, length=4 -> half=2
        assert_eq!(pts[0], Vec3f::new(-1.0, 0.0, 2.0));
        assert_eq!(pts[1], Vec3f::new(1.0, 0.0, 2.0));
        assert_eq!(pts[2], Vec3f::new(1.0, 0.0, -2.0));
        assert_eq!(pts[3], Vec3f::new(-1.0, 0.0, -2.0));
    }

    #[test]
    fn test_plane_points_x() {
        let axis = Token::new("X");
        let pts = gen_plane_points(2.0, 4.0, &axis);
        assert_eq!(pts.len(), 4);
        for p in &pts {
            assert_eq!(p.x, 0.0);
        }
    }

    #[test]
    fn test_plane_points_z() {
        let axis = Token::new("Z");
        let pts = gen_plane_points(2.0, 4.0, &axis);
        assert_eq!(pts.len(), 4);
        for p in &pts {
            assert_eq!(p.z, 0.0);
        }
    }

    // --- Transforms ---

    #[test]
    fn test_sphere_or_cube_xform() {
        let m = gen_sphere_or_cube_xform(3.0);
        // Diagonal = 3.0, last element = 1.0
        assert_eq!(m[0][0], 3.0);
        assert_eq!(m[1][1], 3.0);
        assert_eq!(m[2][2], 3.0);
        assert_eq!(m[3][3], 1.0);
        assert_eq!(m[0][1], 0.0);
    }

    #[test]
    fn test_cone_cylinder_xform_z() {
        let m = gen_cone_or_cylinder_xform(4.0, 1.0, &Token::new("Z"));
        // diameter=2, height=4
        // Row-major: m[row][col]
        // Expected: [[2,0,0,0],[0,2,0,0],[0,0,4,0],[0,0,0,1]]
        assert_eq!(m[0][0], 2.0);
        assert_eq!(m[1][1], 2.0);
        assert_eq!(m[2][2], 4.0);
        assert_eq!(m[3][3], 1.0);
    }

    #[test]
    fn test_cone_cylinder_xform_x() {
        let m = gen_cone_or_cylinder_xform(4.0, 1.0, &Token::new("X"));
        // X axis: [[0,2,0,0],[0,0,2,0],[4,0,0,0],[0,0,0,1]]
        assert_eq!(m[0][0], 0.0);
        assert_eq!(m[0][1], 2.0);
        assert_eq!(m[2][0], 4.0);
    }

    #[test]
    fn test_cone_cylinder_xform_y() {
        let m = gen_cone_or_cylinder_xform(4.0, 1.0, &Token::new("Y"));
        // Y axis: [[0,0,2,0],[2,0,0,0],[0,4,0,0],[0,0,0,1]]
        assert_eq!(m[1][0], 2.0);
        assert_eq!(m[2][1], 4.0);
    }
}
