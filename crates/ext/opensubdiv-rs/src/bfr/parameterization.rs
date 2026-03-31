//! 2D parameterization of a face for surface evaluation.
//!
//! Mirrors `Bfr::Parameterization` from `parameterization.h/cpp`.

use crate::sdc::{SchemeType, SchemeTypeTraits};
use super::limits::Limits;

/// The three kinds of face parameterizations.
///
/// Mirrors `Bfr::Parameterization::Type`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ParameterizationType {
    /// Quadrilateral domain `[0,1]^2`.
    Quad         = 0,
    /// Triangular domain.
    Tri          = 1,
    /// N-sided face partitioned into quadrilateral sub-faces.
    QuadSubFaces = 2,
}

// ---------------------------------------------------------------------------

/// 2D parameterization for a single face.
///
/// Mirrors `Bfr::Parameterization`.
#[derive(Clone, Copy, Debug)]
pub struct Parameterization {
    kind:      u8,   // ParameterizationType discriminant
    u_dim:     u8,   // columns in the sub-face grid (QUAD_SUBFACES only)
    face_size: u16,  // 0 means invalid
}

impl Default for Parameterization {
    fn default() -> Self {
        Parameterization { kind: 0, u_dim: 0, face_size: 0 }
    }
}

impl Parameterization {
    /// Construct from subdivision scheme and face size.
    ///
    /// Returns an invalid `Parameterization` when arguments are out of range.
    pub fn new(scheme: SchemeType, face_size: i32) -> Self {
        let reg_face_size = SchemeTypeTraits::regular_face_size(scheme);

        let mut p = Parameterization {
            kind: if reg_face_size == 4 {
                ParameterizationType::Quad as u8
            } else {
                ParameterizationType::Tri as u8
            },
            face_size: face_size as u16,
            u_dim: 0,
        };

        if face_size != reg_face_size {
            if face_size < 3 || face_size > Limits::max_face_size() {
                p.face_size = 0; // invalid
            } else if reg_face_size == 3 {
                // Loop scheme doesn't support non-triangles:
                p.face_size = 0;
            } else {
                // Quad sub-faces:
                p.kind = ParameterizationType::QuadSubFaces as u8;
                p.u_dim = if face_size < 10 {
                    (2 + (face_size > 4) as i32) as u8
                } else {
                    (1 + (face_size as f32 - 1.0).sqrt() as i32) as u8
                };
            }
        }
        p
    }

    /// Returns `true` if this is a valid, usable parameterization.
    #[inline]
    pub fn is_valid(self) -> bool {
        self.face_size > 0
    }

    /// Return the parameterization type.
    #[inline]
    pub fn get_type(self) -> ParameterizationType {
        match self.kind {
            0 => ParameterizationType::Quad,
            1 => ParameterizationType::Tri,
            _ => ParameterizationType::QuadSubFaces,
        }
    }

    /// Return the number of vertices of the corresponding face.
    #[inline]
    pub fn get_face_size(self) -> i32 {
        self.face_size as i32
    }

    /// Returns `true` if the parameterization has been partitioned into
    /// sub-faces (i.e. is `QuadSubFaces`).
    #[inline]
    pub fn has_sub_faces(self) -> bool {
        self.kind == ParameterizationType::QuadSubFaces as u8
    }

    // -----------------------------------------------------------------------
    // Coordinate queries
    // -----------------------------------------------------------------------

    /// Return the `(u,v)` coordinate of `vertex_index`.
    pub fn get_vertex_coord<R: num_traits::Float>(self, vertex: i32) -> [R; 2] {
        let one = R::one();
        let zero = R::zero();

        match self.get_type() {
            ParameterizationType::Quad => [
                if vertex != 0 && vertex < 3 { one } else { zero },
                if vertex > 1 { one } else { zero },
            ],
            ParameterizationType::Tri => [
                if vertex == 1 { one } else { zero },
                if vertex == 2 { one } else { zero },
            ],
            ParameterizationType::QuadSubFaces => {
                let u = R::from(vertex % self.u_dim as i32).unwrap();
                let v = R::from(vertex / self.u_dim as i32).unwrap();
                [u, v]
            }
        }
    }

    /// Return the `(u,v)` coordinate at parameter `t` along `edge`.
    pub fn get_edge_coord<R: num_traits::Float>(self, edge: i32, t: R) -> [R; 2] {
        let one = R::one();
        let zero = R::zero();

        match self.get_type() {
            ParameterizationType::Quad => match edge {
                0 => [t, zero],
                1 => [one, t],
                2 => [one - t, one],
                3 => [zero, one - t],
                _ => [-one, -one],
            },
            ParameterizationType::Tri => match edge {
                0 => [t, zero],
                1 => [one - t, t],
                2 => [zero, one - t],
                _ => [-one, -one],
            },
            ParameterizationType::QuadSubFaces => {
                let half = R::from(0.5).unwrap();
                if t < half {
                    let mut uv = self.get_vertex_coord::<R>(edge);
                    uv[0] = uv[0] + t;
                    uv
                } else {
                    let next = (edge + 1) % self.face_size as i32;
                    let mut uv = self.get_vertex_coord::<R>(next);
                    uv[1] = uv[1] + (one - t);
                    uv
                }
            }
        }
    }

    /// Return the `(u,v)` coordinate of the face centre.
    pub fn get_center_coord<R: num_traits::Float>(self) -> [R; 2] {
        let third  = R::from(1.0 / 3.0).unwrap();
        let half   = R::from(0.5).unwrap();

        match self.get_type() {
            ParameterizationType::Tri => [third, third],
            _                         => [half,  half],
        }
    }

    // -----------------------------------------------------------------------
    // Sub-face helpers
    // -----------------------------------------------------------------------

    /// Return the integer index of the sub-face containing `uv`.
    ///
    /// Returns 0 when there are no sub-faces.
    pub fn get_sub_face<R: num_traits::Float>(self, uv: [R; 2]) -> i32 {
        if !self.has_sub_faces() {
            return 0;
        }
        let u_tile = uv[0].to_i32().unwrap_or(0);
        let v_tile = uv[1].to_i32().unwrap_or(0);
        let three_quarter = R::from(0.75).unwrap();
        (v_tile + ((uv[1] - R::from(v_tile).unwrap()) > three_quarter) as i32)
            * self.u_dim as i32
            + (u_tile + ((uv[0] - R::from(u_tile).unwrap()) > three_quarter) as i32)
    }

    /// Convert `uv` to a sub-face index + local (unnormalised) `uv`.
    pub fn convert_coord_to_sub_face<R: num_traits::Float>(
        self,
        uv: [R; 2],
    ) -> (i32, [R; 2]) {
        self.convert_coord_to_sub_face_impl(false, uv)
    }

    /// Convert sub-face + local (unnormalised) `uv` to face-space `uv`.
    pub fn convert_sub_face_to_coord<R: num_traits::Float>(
        self,
        sub_face: i32,
        sub_coord: [R; 2],
    ) -> [R; 2] {
        self.convert_sub_face_to_coord_impl(false, sub_face, sub_coord)
    }

    /// Convert `uv` to sub-face + local **normalised** `uv` (Ptex-style).
    pub fn convert_coord_to_normalized_sub_face<R: num_traits::Float>(
        self,
        uv: [R; 2],
    ) -> (i32, [R; 2]) {
        self.convert_coord_to_sub_face_impl(true, uv)
    }

    /// Convert sub-face + **normalised** `uv` to face-space `uv`.
    pub fn convert_normalized_sub_face_to_coord<R: num_traits::Float>(
        self,
        sub_face: i32,
        sub_coord: [R; 2],
    ) -> [R; 2] {
        self.convert_sub_face_to_coord_impl(true, sub_face, sub_coord)
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn convert_coord_to_sub_face_impl<R: num_traits::Float>(
        self,
        normalized: bool,
        uv: [R; 2],
    ) -> (i32, [R; 2]) {
        debug_assert!(self.has_sub_faces());

        let quarter = R::from(0.25).unwrap();

        let u_tile = (uv[0] + quarter).to_i32().unwrap_or(0);
        let v_tile = (uv[1] + quarter).to_i32().unwrap_or(0);

        let u_dim  = self.u_dim as i32;
        let fs     = self.face_size as i32;

        let u_tile = u_tile.max(0).min(u_dim - 1);
        let v_tile = {
            let vt = v_tile.max(0);
            if vt * u_dim + u_tile >= fs {
                (fs / u_dim) - 1 + (u_tile < (fs % u_dim)) as i32
            } else {
                vt
            }
        };

        let mut sub_u = uv[0] - R::from(u_tile).unwrap();
        let mut sub_v = uv[1] - R::from(v_tile).unwrap();

        if normalized {
            let two = R::from(2.0).unwrap();
            sub_u = sub_u * two;
            sub_v = sub_v * two;
        }

        (v_tile * u_dim + u_tile, [sub_u, sub_v])
    }

    fn convert_sub_face_to_coord_impl<R: num_traits::Float>(
        self,
        normalized: bool,
        sub_face: i32,
        sub_coord: [R; 2],
    ) -> [R; 2] {
        debug_assert!(self.has_sub_faces());

        let u_tile = sub_face % self.u_dim as i32;
        let v_tile = sub_face / self.u_dim as i32;

        if normalized {
            let half = R::from(0.5).unwrap();
            [
                R::from(u_tile).unwrap() + sub_coord[0] * half,
                R::from(v_tile).unwrap() + sub_coord[1] * half,
            ]
        } else {
            [
                R::from(u_tile).unwrap() + sub_coord[0],
                R::from(v_tile).unwrap() + sub_coord[1],
            ]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::SchemeType;

    #[test]
    fn quad_parameterization() {
        let p = Parameterization::new(SchemeType::Catmark, 4);
        assert!(p.is_valid());
        assert_eq!(p.get_type(), ParameterizationType::Quad);
        assert_eq!(p.get_face_size(), 4);
        assert!(!p.has_sub_faces());
    }

    #[test]
    fn tri_parameterization() {
        let p = Parameterization::new(SchemeType::Loop, 3);
        assert!(p.is_valid());
        assert_eq!(p.get_type(), ParameterizationType::Tri);
    }

    #[test]
    fn quad_subfaces_ngon() {
        // 5-gon in Catmark scheme becomes QuadSubFaces.
        let p = Parameterization::new(SchemeType::Catmark, 5);
        assert!(p.is_valid());
        assert_eq!(p.get_type(), ParameterizationType::QuadSubFaces);
        assert!(p.has_sub_faces());
    }

    #[test]
    fn invalid_tri_non_triangle() {
        // Loop scheme only works for triangles:
        let p = Parameterization::new(SchemeType::Loop, 4);
        assert!(!p.is_valid());
    }

    #[test]
    fn quad_vertex_coords() {
        let p = Parameterization::new(SchemeType::Catmark, 4);
        let v0: [f32; 2] = p.get_vertex_coord(0);
        let v1: [f32; 2] = p.get_vertex_coord(1);
        let v2: [f32; 2] = p.get_vertex_coord(2);
        let v3: [f32; 2] = p.get_vertex_coord(3);
        assert_eq!(v0, [0.0, 0.0]);
        assert_eq!(v1, [1.0, 0.0]);
        assert_eq!(v2, [1.0, 1.0]);
        assert_eq!(v3, [0.0, 1.0]);
    }

    #[test]
    fn tri_vertex_coords() {
        let p = Parameterization::new(SchemeType::Loop, 3);
        let v0: [f32; 2] = p.get_vertex_coord(0);
        let v1: [f32; 2] = p.get_vertex_coord(1);
        let v2: [f32; 2] = p.get_vertex_coord(2);
        assert_eq!(v0, [0.0, 0.0]);
        assert_eq!(v1, [1.0, 0.0]);
        assert_eq!(v2, [0.0, 1.0]);
    }

    #[test]
    fn center_coords() {
        let pq = Parameterization::new(SchemeType::Catmark, 4);
        let pt = Parameterization::new(SchemeType::Loop, 3);
        let cq: [f64; 2] = pq.get_center_coord();
        let ct: [f64; 2] = pt.get_center_coord();
        assert!((cq[0] - 0.5).abs() < 1e-10);
        assert!((ct[0] - 1.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn subface_roundtrip() {
        // 5-gon: pick sub-face 0 at (0.25, 0.25).
        let p = Parameterization::new(SchemeType::Catmark, 5);
        assert!(p.has_sub_faces());

        let uv = [0.25f64, 0.25];
        let (sf, sub) = p.convert_coord_to_sub_face(uv);
        let uv2 = p.convert_sub_face_to_coord(sf, sub);
        assert!((uv2[0] - uv[0]).abs() < 1e-12);
        assert!((uv2[1] - uv[1]).abs() < 1e-12);
    }

    #[test]
    fn normalized_subface_roundtrip() {
        let p = Parameterization::new(SchemeType::Catmark, 5);
        let uv = [0.6f32, 0.1];
        let (sf, sub) = p.convert_coord_to_normalized_sub_face(uv);
        let uv2 = p.convert_normalized_sub_face_to_coord(sf, sub);
        assert!((uv2[0] - uv[0]).abs() < 1e-6);
        assert!((uv2[1] - uv[1]).abs() < 1e-6);
    }
}
