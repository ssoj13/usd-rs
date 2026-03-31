//! Tessellation — parametric tessellation pattern for a face.
//!
//! Ported from OpenSubdiv bfr/tessellation.h/.cpp.
//!
//! `Tessellation` encapsulates a tessellation pattern for a given
//! `Parameterization`. On construction the full inventory of coordinates
//! (boundary + interior) and facets is computed. Clients then retrieve UV
//! coordinate pairs and facet index tuples via generic methods.
//!
//! Design notes:
//!   - The C++ implementation uses raw pointer-based stride arrays.  Here we
//!     store the generated data directly in `Vec<f64>` (coords) and `Vec<i32>`
//!     (facets) with configurable strides so that the public API can remain
//!     close to the C++ one while being safe.
//!   - The three parameterization-specific building blocks (quad, tri, qsub)
//!     are implemented as free functions in submodules to keep this file
//!     manageable.

use super::parameterization::{Parameterization, ParameterizationType};
use num_traits::Float;

// ---------------------------------------------------------------------------
//  Options
// ---------------------------------------------------------------------------

/// Options that configure a `Tessellation` pattern.
///
/// Mirrors `Bfr::Tessellation::Options`.
#[derive(Clone, Copy, Debug)]
pub struct TessellationOptions {
    /// Preserve quads where possible (requires 4-sided facets, default: off).
    pub preserve_quads: bool,
    /// Number of indices per facet (3 or 4, default: 3).
    pub facet_size:     i32,
    /// Stride between consecutive facets (default: facet_size).
    pub facet_stride:   i32,
    /// Stride between consecutive (u,v) pairs (default: 2).
    pub coord_stride:   i32,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        TessellationOptions {
            preserve_quads: false,
            facet_size:     3,
            facet_stride:   0,
            coord_stride:   0,
        }
    }
}

impl TessellationOptions {
    /// Set quad-preservation flag, returning self for chaining.
    pub fn with_preserve_quads(mut self, on: bool) -> Self {
        self.preserve_quads = on;
        self
    }
    /// Set facet size (3 or 4), returning self for chaining.
    pub fn with_facet_size(mut self, n: i32) -> Self {
        self.facet_size = n;
        self
    }
    /// Set facet stride, returning self for chaining.
    pub fn with_facet_stride(mut self, s: i32) -> Self {
        self.facet_stride = s;
        self
    }
    /// Set coord stride, returning self for chaining.
    pub fn with_coord_stride(mut self, s: i32) -> Self {
        self.coord_stride = s;
        self
    }
    /// Effective facet size (always 3 or 4).
    /// Mirrors C++ `3 + (int)_facetSize4` — any value other than 4 produces 3.
    pub fn get_facet_size(&self) -> i32 { if self.facet_size == 4 { 4 } else { 3 } }
    /// Effective facet stride.
    pub fn get_facet_stride(&self) -> i32 {
        if self.facet_stride > 0 { self.facet_stride } else { self.get_facet_size() }
    }
    /// Effective coord stride.
    pub fn get_coord_stride(&self) -> i32 {
        if self.coord_stride > 0 { self.coord_stride } else { 2 }
    }
}

// ---------------------------------------------------------------------------
//  Tessellation
// ---------------------------------------------------------------------------

/// Tessellation pattern for a `Parameterization`.
///
/// Mirrors `Bfr::Tessellation`.
pub struct Tessellation {
    is_valid:    bool,
    is_uniform:  bool,
    /// True when only 2 triangles are generated (split-quad shortcut).
    split_quad:      bool,
    /// True when the entire face is a single facet.
    single_face:     bool,
    /// True when facets are a fan from boundary to a single centre point.
    triangle_fan:    bool,
    /// True when facets span the face edge-to-edge (quad, no interior pts).
    segmented_face:  bool,

    param:           Parameterization,

    facet_size:      i32,
    facet_stride:    i32,
    coord_stride:    i32,

    triangulate:     bool,

    num_given_rates:   i32,
    num_boundary_pts:  i32,
    num_interior_pts:  i32,
    num_facets:        i32,

    inner_rates: [i32; 2],
    /// Per-edge outer tessellation rates (length = face_size).
    outer_rates: Vec<i32>,
}

impl Tessellation {
    // -----------------------------------------------------------------------
    //  Construction
    // -----------------------------------------------------------------------

    /// Construct with a single uniform tessellation rate.
    pub fn new_uniform(
        param:       Parameterization,
        uniform_rate: i32,
        options:     TessellationOptions,
    ) -> Self {
        let rates = [uniform_rate];
        Self::new(param, &rates, options)
    }

    /// Construct with an explicit set of rates (outer ± inner).
    pub fn new(
        param:   Parameterization,
        rates:   &[i32],
        options: TessellationOptions,
    ) -> Self {
        let mut t = Tessellation {
            is_valid:       false,
            is_uniform:     false,
            split_quad:     false,
            single_face:    false,
            triangle_fan:   false,
            segmented_face: false,
            param,
            facet_size:     0,
            facet_stride:   0,
            coord_stride:   0,
            triangulate:    true,
            num_given_rates:  0,
            num_boundary_pts: 0,
            num_interior_pts: 0,
            num_facets:       0,
            inner_rates:    [0; 2],
            outer_rates:    Vec::new(),
        };
        t.initialize(param, rates, options);
        t
    }

    /// Return `true` if correctly initialized.
    #[inline]
    pub fn is_valid(&self) -> bool { self.is_valid }

    // -----------------------------------------------------------------------
    //  Simple queries
    // -----------------------------------------------------------------------

    /// Return the parameterization.
    #[inline]
    pub fn get_parameterization(&self) -> Parameterization { self.param }

    /// Return the face size.
    #[inline]
    pub fn get_face_size(&self) -> i32 { self.param.get_face_size() }

    /// Return whether the tessellation is uniform.
    #[inline]
    pub fn is_uniform(&self) -> bool { self.is_uniform }

    /// Fill `rates` (must have length `num_given_rates`) and return count.
    pub fn get_rates(&self, rates: &mut [i32]) -> i32 {
        let n = self.get_face_size();
        let n_outer = (n as usize).min(self.num_given_rates as usize);
        let n_inner = (self.num_given_rates as usize).saturating_sub(n as usize);

        for i in 0..n_outer { rates[i] = self.outer_rates[i]; }
        for i in 0..n_inner { rates[n as usize + i] = self.inner_rates[if i > 0 { 1 } else { 0 }]; }
        self.num_given_rates
    }

    // -----------------------------------------------------------------------
    //  Coordinate counts
    // -----------------------------------------------------------------------

    /// Total number of coordinates (boundary + interior).
    #[inline]
    pub fn get_num_coords(&self) -> i32 { self.num_boundary_pts + self.num_interior_pts }
    /// Number of boundary coordinates.
    #[inline]
    pub fn get_num_boundary_coords(&self) -> i32 { self.num_boundary_pts }
    /// Number of interior coordinates.
    #[inline]
    pub fn get_num_interior_coords(&self) -> i32 { self.num_interior_pts }
    /// Number of coordinates on a specific edge (excluding end vertices).
    #[inline]
    pub fn get_num_edge_coords(&self, edge: i32) -> i32 {
        self.outer_rates[edge as usize] - 1
    }
    /// Stride between coord tuples.
    #[inline]
    pub fn get_coord_stride(&self) -> i32 { self.coord_stride }

    // -----------------------------------------------------------------------
    //  Facet counts
    // -----------------------------------------------------------------------

    /// Total number of facets.
    #[inline]
    pub fn get_num_facets(&self) -> i32 { self.num_facets }
    /// Number of indices per facet.
    #[inline]
    pub fn get_facet_size(&self) -> i32 { self.facet_size }
    /// Stride between facets.
    #[inline]
    pub fn get_facet_stride(&self) -> i32 { self.facet_stride }

    // -----------------------------------------------------------------------
    //  Coordinate retrieval
    // -----------------------------------------------------------------------

    /// Fill `buf` with all UV coordinate pairs (boundary then interior).
    ///
    /// The `buf` slice must contain at least `get_num_coords() * coord_stride`
    /// elements. Returns the number of coordinates written.
    pub fn get_coords<R: Float>(&self, buf: &mut [R]) -> i32 {
        let n = self.get_boundary_coords(buf);
        let n2 = self.get_interior_coords(&mut buf[(n as usize * self.coord_stride as usize)..]);
        n + n2
    }

    /// Fill `buf` with boundary UV coordinate pairs.
    pub fn get_boundary_coords<R: Float>(&self, buf: &mut [R]) -> i32 {
        let stride = self.coord_stride as usize;
        match self.param.get_type() {
            ParameterizationType::Quad  => quad_get_boundary_coords(&self.outer_rates, stride, buf),
            ParameterizationType::Tri   => tri_get_boundary_coords(&self.outer_rates, stride, buf),
            ParameterizationType::QuadSubFaces => qsub_get_boundary_coords(self.param, &self.outer_rates, stride, buf),
        }
    }

    /// Fill `buf` with interior UV coordinate pairs.
    pub fn get_interior_coords<R: Float>(&self, buf: &mut [R]) -> i32 {
        if self.num_interior_pts == 0 { return 0; }
        let stride = self.coord_stride as usize;
        if self.num_interior_pts == 1 {
            // Centre point:
            let c = self.param.get_center_coord::<R>();
            buf[0] = c[0]; buf[1] = c[1];
            return 1;
        }
        match self.param.get_type() {
            ParameterizationType::Quad  => quad_get_interior_coords(&self.inner_rates, stride, buf),
            ParameterizationType::Tri   => tri_get_interior_coords(self.inner_rates[0], stride, buf),
            ParameterizationType::QuadSubFaces => qsub_get_interior_coords(self.param, self.inner_rates[0], stride, buf),
        }
    }

    /// Fill `buf` with the (u,v) coordinate for `vertex` of the face.
    pub fn get_vertex_coord<R: Float>(&self, vertex: i32, buf: &mut [R]) -> i32 {
        let uv = self.param.get_vertex_coord::<R>(vertex);
        buf[0] = uv[0]; buf[1] = uv[1];
        1
    }

    /// Fill `buf` with the edge coordinates for `edge` (excluding end vertices).
    pub fn get_edge_coords<R: Float>(&self, edge: i32, buf: &mut [R]) -> i32 {
        let edge_res = self.outer_rates[edge as usize];
        let stride   = self.coord_stride as usize;
        match self.param.get_type() {
            ParameterizationType::Quad  => quad_get_edge_coords(edge, edge_res, stride, buf),
            ParameterizationType::Tri   => tri_get_edge_coords(edge, edge_res, stride, buf),
            ParameterizationType::QuadSubFaces => qsub_get_edge_coords(self.param, edge, edge_res, stride, buf),
        }
    }

    // -----------------------------------------------------------------------
    //  Facet retrieval
    // -----------------------------------------------------------------------

    /// Fill `facet_buf` with facet index tuples and return facet count.
    ///
    /// `facet_buf` must have length at least `num_facets * facet_stride`.
    pub fn get_facets(&self, facet_buf: &mut [i32]) -> i32 {
        let fsize  = self.facet_size  as usize;
        let fstride = self.facet_stride as usize;
        let n = self.get_face_size();

        if self.single_face {
            if n == 3 {
                facet_set3(facet_buf, 0, fsize, 0, 1, 2);
                return 1;
            } else {
                facet_set4(facet_buf, 0, fsize, 0, 1, 2, 3);
                return 1;
            }
        }
        if self.triangle_fan {
            // Fan from boundary points 0..n_facets to center (index n_facets).
            let nf = self.num_facets as usize;
            for i in 0..nf {
                let a = i;
                let b = (i + 1) % nf;
                let c = nf; // center point index
                facet_set3(facet_buf, i * fstride, fsize, a as i32, b as i32, c as i32);
            }
            return self.num_facets;
        }
        if self.split_quad {
            // Two triangles splitting a quad:
            let tri_sign = if self.triangulate { 1i32 } else { 0 };
            return append_quad(facet_buf, 0, fsize, fstride, 0, 1, 2, 3, tri_sign);
        }

        let nb = self.num_boundary_pts as usize;
        match self.param.get_type() {
            ParameterizationType::Quad => {
                if self.is_uniform {
                    quad_get_uniform_facets(self.inner_rates[0], self.triangulate, fsize, fstride, facet_buf)
                } else if self.segmented_face {
                    quad_get_segmented_facets(&self.outer_rates, self.triangulate, fsize, fstride, facet_buf)
                } else {
                    quad_get_non_uniform_facets(&self.outer_rates, &self.inner_rates, nb, self.triangulate, fsize, fstride, facet_buf)
                }
            }
            ParameterizationType::Tri => {
                if self.is_uniform {
                    tri_get_uniform_facets(self.inner_rates[0], fsize, fstride, facet_buf)
                } else {
                    tri_get_non_uniform_facets(&self.outer_rates, self.inner_rates[0], nb, fsize, fstride, facet_buf)
                }
            }
            ParameterizationType::QuadSubFaces => {
                if self.is_uniform {
                    qsub_get_uniform_facets(n, self.inner_rates[0], self.triangulate, fsize, fstride, facet_buf)
                } else {
                    qsub_get_non_uniform_facets(n, &self.outer_rates, self.inner_rates[0], nb, self.triangulate, fsize, fstride, facet_buf)
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Facet index transforms
    // -----------------------------------------------------------------------

    /// Apply a common offset to all valid facet coordinate indices.
    pub fn transform_facet_coord_indices_offset(&self, facet_buf: &mut [i32], offset: i32) {
        let fsize   = self.facet_size as usize;
        let fstride = self.facet_stride as usize;
        for i in 0..self.num_facets as usize {
            let base = i * fstride;
            for j in 0..fsize {
                if facet_buf[base + j] >= 0 {
                    facet_buf[base + j] += offset;
                }
            }
        }
    }

    /// Remap boundary indices via `boundary_indices`, apply `interior_offset`
    /// to interior indices.
    pub fn transform_facet_coord_indices_boundary(
        &self,
        facet_buf:        &mut [i32],
        boundary_indices: &[i32],
        interior_offset:  i32,
    ) {
        let nb      = self.num_boundary_pts;
        let fsize   = self.facet_size as usize;
        let fstride = self.facet_stride as usize;
        for i in 0..self.num_facets as usize {
            let base = i * fstride;
            for j in 0..fsize {
                let idx = facet_buf[base + j];
                if idx >= 0 {
                    facet_buf[base + j] = if idx < nb {
                        boundary_indices[idx as usize]
                    } else {
                        idx + interior_offset
                    };
                }
            }
        }
    }

    /// Remap all indices: boundary via `boundary_indices`, interior via
    /// `interior_indices`.
    pub fn transform_facet_coord_indices_all(
        &self,
        facet_buf:        &mut [i32],
        boundary_indices: &[i32],
        interior_indices: &[i32],
    ) {
        let nb      = self.num_boundary_pts;
        let fsize   = self.facet_size as usize;
        let fstride = self.facet_stride as usize;
        for i in 0..self.num_facets as usize {
            let base = i * fstride;
            for j in 0..fsize {
                let idx = facet_buf[base + j];
                if idx >= 0 {
                    facet_buf[base + j] = if idx < nb {
                        boundary_indices[idx as usize]
                    } else {
                        interior_indices[(idx - nb) as usize]
                    };
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Private: initialization
    // -----------------------------------------------------------------------

    fn initialize(&mut self, param: Parameterization, rates: &[i32], options: TessellationOptions) {
        // Validate:
        if !param.is_valid() { return; }
        if rates.is_empty()  { return; }
        for &r in rates { if r < 1 { return; } }

        let coord_stride = options.get_coord_stride();
        if coord_stride < 2 { return; }
        let facet_size   = options.get_facet_size();
        let facet_stride = options.get_facet_stride();
        if facet_stride < facet_size { return; }

        self.param        = param;
        self.facet_size   = facet_size;
        self.facet_stride = facet_stride;
        self.coord_stride = coord_stride;
        self.triangulate  = (facet_size == 3) || !options.preserve_quads;

        let sum_outer = self.init_rates(rates);

        match param.get_type() {
            ParameterizationType::Quad       => self.init_inventory_quad(sum_outer),
            ParameterizationType::Tri        => self.init_inventory_tri(sum_outer),
            ParameterizationType::QuadSubFaces => self.init_inventory_qpoly(sum_outer),
        }
        self.is_valid = true;
    }

    fn init_rates(&mut self, given: &[i32]) -> i32 {
        self.num_given_rates = given.len() as i32;
        let n     = self.param.get_face_size() as usize;
        let is_quad = n == 4;
        let ng    = given.len();
        const MAX_RATE: i32 = i16::MAX as i32;

        self.outer_rates.resize(n, 1);
        let mut total = 0i32;

        if ng < n {
            if ng == 2 && is_quad {
                // Two inner rates given, infer outer rates for quad:
                self.inner_rates[0] = given[0].min(MAX_RATE);
                self.inner_rates[1] = given[1].min(MAX_RATE);
                self.outer_rates[0] = self.inner_rates[0];
                self.outer_rates[2] = self.inner_rates[0];
                self.outer_rates[1] = self.inner_rates[1];
                self.outer_rates[3] = self.inner_rates[1];
                self.is_uniform = self.inner_rates[0] == self.inner_rates[1];
                total = 2 * (self.inner_rates[0] + self.inner_rates[1]);
            } else {
                // Single uniform inner rate:
                let r = given[0].min(MAX_RATE);
                self.inner_rates[0] = r;
                self.inner_rates[1] = r;
                for i in 0..n { self.outer_rates[i] = r; }
                self.is_uniform = true;
                total = r * n as i32;
            }
        } else {
            // Explicit outer rates:
            self.is_uniform = true;
            for i in 0..n {
                self.outer_rates[i] = given[i].min(MAX_RATE);
                if self.outer_rates[i] != self.outer_rates[0] { self.is_uniform = false; }
                total += self.outer_rates[i];
            }
            // Inner rates:
            if ng > n {
                self.inner_rates[0] = given[n].min(MAX_RATE);
                self.inner_rates[1] = if ng >= n + 2 && is_quad {
                    given[n + 1].min(MAX_RATE)
                } else {
                    self.inner_rates[0]
                };
                if self.inner_rates[0] != self.outer_rates[0] { self.is_uniform = false; }
                if self.inner_rates[1] != self.outer_rates[0] { self.is_uniform = false; }
            } else if is_quad {
                // Infer inner rates as the integer average of opposite edge pairs.
                // Matches C++ tessellation.cpp lines 2181-2182 exactly.
                self.inner_rates[0] = (self.outer_rates[0] + self.outer_rates[2]) / 2;
                self.inner_rates[1] = (self.outer_rates[1] + self.outer_rates[3]) / 2;
            } else {
                self.inner_rates[0] = total / n as i32;
                self.inner_rates[1] = self.inner_rates[0];
            }
        }
        total
    }

    fn init_inventory_quad(&mut self, sum_outer: i32) {
        let inner = self.inner_rates;
        let outer = &self.outer_rates;

        if self.is_uniform {
            if inner[0] > 1 {
                self.num_interior_pts = quad_count_interior_coords_uv(inner[0], inner[0]);
                self.num_facets = quad_count_uniform_facets(inner[0], self.triangulate);
            } else if self.triangulate {
                self.num_interior_pts = 0;
                self.num_facets = 2;
                self.split_quad = true;
            } else {
                self.num_interior_pts = 0;
                self.num_facets = 1;
                self.single_face = true;
            }
        } else {
            if (inner[0] > 1) && (inner[1] > 1) {
                self.num_interior_pts = quad_count_interior_coords_uv(inner[0], inner[1]);
                self.num_facets = quad_count_non_uniform_facets(outer, &inner, self.triangulate);
            } else if ((inner[0] == 1) && (outer[0] == 1) && (outer[2] == 1)) ||
                      ((inner[1] == 1) && (outer[1] == 1) && (outer[3] == 1)) {
                self.num_interior_pts = 0;
                self.num_facets = quad_count_segmented_facets(outer, self.triangulate);
                if sum_outer == 4 {
                    self.split_quad  = self.triangulate;
                    self.single_face = !self.triangulate;
                } else {
                    self.segmented_face = true;
                }
            } else {
                self.num_interior_pts = 1;
                self.num_facets = sum_outer;
                self.triangle_fan = true;
            }
        }
        self.num_boundary_pts = sum_outer;
    }

    fn init_inventory_tri(&mut self, sum_outer: i32) {
        let res = self.inner_rates[0];
        if self.is_uniform {
            if res > 1 {
                self.num_interior_pts = tri_count_interior_coords(res);
                self.num_facets = tri_count_uniform_facets(res);
            } else {
                self.num_interior_pts = 0;
                self.num_facets = 1;
                self.single_face = true;
            }
        } else {
            if res > 2 {
                self.num_interior_pts = tri_count_interior_coords(res);
                self.num_facets = tri_count_non_uniform_facets(&self.outer_rates, res);
            } else {
                self.num_interior_pts = 1;
                self.num_facets = sum_outer;
                self.triangle_fan = true;
            }
        }
        self.num_boundary_pts = sum_outer;
    }

    fn init_inventory_qpoly(&mut self, sum_outer: i32) {
        let n   = self.param.get_face_size();
        let res = self.inner_rates[0];
        if self.is_uniform {
            if res > 1 {
                self.num_interior_pts = qsub_count_interior_coords(n, res);
                self.num_facets = qsub_count_uniform_facets(n, res, self.triangulate);
            } else if n == 3 {
                self.num_interior_pts = 0;
                self.num_facets = 1;
                self.single_face = true;
            } else {
                self.num_interior_pts = 1;
                self.num_facets = n;
                self.triangle_fan = true;
            }
        } else {
            if res > 1 {
                self.num_interior_pts = qsub_count_interior_coords(n, res);
                self.num_facets = qsub_count_non_uniform_facets(n, &self.outer_rates, res, self.triangulate);
            } else {
                self.num_interior_pts = 1;
                self.num_facets = sum_outer;
                self.triangle_fan = true;
            }
        }
        self.num_boundary_pts = sum_outer;
    }
}

// ===========================================================================
//  Coordinate / facet helper primitives
// ===========================================================================

// ---------------------------------------------------------------------------
//  Coordinate writing helpers — UV isoparametric lines
// ---------------------------------------------------------------------------

/// Write `n` UV pairs along a line of constant U=u.
fn append_u_isoline<R: Float>(buf: &mut [R], stride: usize, n: usize, u: R, mut v: R, dv: R) {
    for i in 0..n { buf[i * stride] = u; buf[i * stride + 1] = v; v = v + dv; }
}
/// Write `n` UV pairs along a line of constant V=v.
fn append_v_isoline<R: Float>(buf: &mut [R], stride: usize, n: usize, mut u: R, v: R, du: R) {
    for i in 0..n { buf[i * stride] = u; buf[i * stride + 1] = v; u = u + du; }
}
/// Write `n` UV pairs along a diagonal.
fn append_uv_line<R: Float>(buf: &mut [R], stride: usize, n: usize, mut u: R, mut v: R, du: R, dv: R) {
    for i in 0..n { buf[i * stride] = u; buf[i * stride + 1] = v; u = u + du; v = v + dv; }
}

// ---------------------------------------------------------------------------
//  Facet writing helpers
// ---------------------------------------------------------------------------

/// Write a triangle facet at `buf[base..]`.  If the facet buffer uses size-4
/// slots, writes -1 into the fourth slot to match C++ `Facet::Set(a,b,c)`.
fn facet_set3(buf: &mut [i32], base: usize, fsize: usize, a: i32, b: i32, c: i32) {
    buf[base] = a; buf[base + 1] = b; buf[base + 2] = c;
    if fsize == 4 { buf[base + 3] = -1; }
}

/// Write a triangle or quad facet.  For triangle in size-4 buffer sets buf[3]=-1.
fn facet_set4(buf: &mut [i32], base: usize, fsize: usize, a: i32, b: i32, c: i32, d: i32) {
    buf[base] = a; buf[base + 1] = b; buf[base + 2] = c;
    if fsize == 4 { buf[base + 3] = d; }
}

/// Append a single triangle, returns 1.
fn append_tri(buf: &mut [i32], base: usize, t0: i32, t1: i32, t2: i32, fsize: usize) -> i32 {
    buf[base] = t0; buf[base + 1] = t1; buf[base + 2] = t2;
    if fsize == 4 { buf[base + 3] = -1; }
    1
}

/// Append one or two facets for a quad. `sign` = 0 → quad, >0 → tri A, <0 → tri B.
/// Returns facet count written.
fn append_quad(buf: &mut [i32], base: usize, fsize: usize, fstride: usize,
               q0: i32, q1: i32, q2: i32, q3: i32, sign: i32) -> i32 {
    if sign == 0 {
        buf[base] = q0; buf[base+1] = q1; buf[base+2] = q2;
        if fsize == 4 { buf[base+3] = q3; }
        1
    } else if sign > 0 {
        append_tri(buf, base, q0, q1, q2, fsize);
        append_tri(buf, base + fstride, q2, q3, q0, fsize);
        2
    } else {
        append_tri(buf, base, q2, q3, q1, fsize);
        append_tri(buf, base + fstride, q0, q1, q3, fsize);
        2
    }
}

/// Append a triangle fan from `start_index` of `size` points around centre.
fn append_tri_fan(buf: &mut [i32], base: usize, fstride: usize, fsize: usize,
                  size: i32, start_index: i32) -> i32 {
    let size = size as usize;
    for i in 1..=size {
        let a = start_index + (i - 1) as i32;
        let b = start_index + (if i < size { i as i32 } else { 0 });
        let c = start_index + size as i32;
        append_tri(buf, base + (i - 1) * fstride, a, b, c, fsize);
    }
    size as i32
}

// ---------------------------------------------------------------------------
//  FacetStrip — concentric-ring facet connector
// ---------------------------------------------------------------------------

/// Topology record for one "strip" of facets between an outer and inner ring.
///
/// Mirrors the anonymous `FacetStrip` struct in `tessellation.cpp`.
#[derive(Default)]
struct FacetStrip {
    quad_topology:    bool,
    quad_triangulate: bool,
    inner_reversed:   bool,
    exclude_first:    bool,
    split_first:      bool,
    split_last:       bool,
    include_last:     bool,

    outer_edges: i32,
    inner_edges: i32,

    outer_first: i32,
    outer_last:  i32,
    outer_prev:  i32,
    inner_first: i32,
    inner_last:  i32,
}

impl FacetStrip {
    /// Connect a uniform strip of quads, returning facet count.
    fn connect_uniform_quads(&self, buf: &mut [i32], base: usize,
                              fsize: usize, fstride: usize) -> i32 {
        debug_assert!(self.quad_topology);
        debug_assert!(self.inner_edges == self.outer_edges - 2);

        let tri_sign = if self.quad_triangulate { 1i32 } else { 0 };
        let mut nf = 0usize;

        let out0 = self.outer_first;
        let in0  = self.inner_first;

        if self.split_first {
            nf += append_tri(buf, base + nf * fstride, out0, out0 + 1, in0, fsize) as usize;
        } else if !self.exclude_first {
            nf += append_quad(buf, base + nf * fstride, fsize, fstride,
                              out0, out0 + 1, in0, self.outer_prev, tri_sign) as usize;
        }

        let mut out_i = self.outer_first + 1;
        let mut in_i  = self.inner_first;
        if self.inner_edges > 0 {
            let d_in = if self.inner_reversed { -1i32 } else { 1 };
            let mut cur_sign = tri_sign;
            for i in 1..=self.inner_edges {
                if i > self.inner_edges / 2 { cur_sign = -tri_sign; }
                let out_j = out_i + 1;
                let in_j  = if i < self.inner_edges { in_i + d_in } else { self.inner_last };
                nf += append_quad(buf, base + nf * fstride, fsize, fstride,
                                  out_i, out_j, in_j, in_i, cur_sign) as usize;
                out_i += 1;
                in_i  += d_in;
            }
        }

        let out_n = self.outer_last;
        let in_n  = self.inner_last;
        if self.split_last {
            nf += append_tri(buf, base + nf * fstride, out_i, out_n, in_n, fsize) as usize;
        } else if self.include_last {
            nf += append_quad(buf, base + nf * fstride, fsize, fstride,
                              out_i, out_n, out_n + 1, in_n, -tri_sign) as usize;
        }
        nf as i32
    }

    /// Connect a uniform strip of triangles, returning facet count.
    fn connect_uniform_tris(&self, buf: &mut [i32], base: usize,
                             fsize: usize, fstride: usize) -> i32 {
        debug_assert!(!self.quad_topology);
        let mut nf = 0usize;

        let out0 = self.outer_first;
        let in0  = self.inner_first;

        if self.split_first {
            nf += append_tri(buf, base + nf * fstride, out0, out0 + 1, in0, fsize) as usize;
        } else {
            nf += append_tri(buf, base + nf * fstride, out0, out0 + 1, self.outer_prev, fsize) as usize;
            nf += append_tri(buf, base + nf * fstride, in0, self.outer_prev, out0 + 1, fsize) as usize;
        }

        nf += append_tri(buf, base + nf * fstride, out0 + 1, out0 + 2, in0, fsize) as usize;

        let mut out_i = self.outer_first + 2;
        let mut in_i  = self.inner_first;

        for i in 1..=self.inner_edges {
            let out_j = out_i + 1;
            let in_j  = if i < self.inner_edges { in_i + 1 } else { self.inner_last };
            nf += append_tri(buf, base + nf * fstride, in_j, in_i, out_i, fsize) as usize;
            nf += append_tri(buf, base + nf * fstride, out_i, out_j, in_j, fsize) as usize;
            out_i += 1;
            in_i  += 1;
        }

        if self.split_last {
            nf += append_tri(buf, base + nf * fstride, out_i, self.outer_last, self.inner_last, fsize) as usize;
        }
        nf as i32
    }

    /// Connect a non-uniform strip of facets, returning facet count.
    fn connect_non_uniform_facets(&self, buf: &mut [i32], base: usize,
                                   fsize: usize, fstride: usize) -> i32 {
        let all_inner_edges = self.inner_edges + 2
            - self.split_first as i32 - self.split_last as i32;

        let big_m = self.inner_edges + (if self.quad_topology { 2 } else { 3 });
        let big_n = self.outer_edges;

        let dt_outer = big_m;
        let dt_inner = big_n;

        let dt_min = dt_inner.min(dt_outer);
        let dt_max = dt_inner.max(dt_outer);
        let dt_slope_max = if (dt_max / 2) < dt_min { dt_min - 1 } else { dt_max / 2 };

        let t_outer_last   = dt_outer * big_n;
        let t_outer_middle = t_outer_last / 2;

        let mut t_inner_offset = 0i32;
        let mut t_inner_last   = dt_inner * (big_m - 1);

        if !self.quad_topology {
            t_inner_offset = dt_inner / 2;
            t_inner_last  += t_inner_offset - dt_inner;
        }

        let d_inner = if self.inner_reversed { -1i32 } else { 1 };

        debug_assert_eq!(self.split_first, self.split_last);

        let mut t_inner0;
        let mut t_inner1;
        let mut c_inner0;
        let mut c_inner1;

        if !self.split_first {
            t_inner0 = t_inner_offset;
            t_inner1 = t_inner_offset + dt_inner;
            c_inner0 = self.outer_prev;
            c_inner1 = self.inner_first;
        } else {
            t_inner0 = t_inner_offset + dt_inner;
            t_inner1 = t_inner0 + (if all_inner_edges > 0 { dt_inner } else { 0 });
            c_inner0 = self.inner_first;
            c_inner1 = if all_inner_edges == 1 {
                self.inner_last
            } else {
                self.inner_first + d_inner
            };
        }
        if !self.split_last {
            t_inner_last += dt_inner;
        }

        let mut t_outer0 = 0i32;
        let mut c_outer0 = self.outer_first;
        let mut t_outer1 = dt_outer;
        let mut c_outer1 = if big_n == 1 { self.outer_last } else { self.outer_first + 1 };

        let keep_quads = self.quad_topology && !self.quad_triangulate;

        let n_facets_expected = if keep_quads {
            let n = all_inner_edges.max(self.outer_edges);
            let sym_center = if (n & 1) == 0 {
                ((all_inner_edges & 1) | (self.outer_edges & 1)) as i32
            } else { 0 };
            n + sym_center
        } else {
            all_inner_edges + self.outer_edges
        };

        let n_leading  = n_facets_expected / 2;
        let n_middle   = n_facets_expected & 1;
        let middle_facet = if n_middle != 0 { n_leading } else { -1 };
        let middle_quad  = keep_quads && (big_m & 1 != 0) && (big_n & 1 != 0);

        let mut nf = 0i32;
        for fi in 0..n_facets_expected {
            let mut gen_tri_outer = false;
            let mut gen_tri_inner = false;
            let mut gen_quad      = false;

            if fi == middle_facet {
                if middle_quad                { gen_quad = true; }
                else if self.outer_edges & 1 != 0 { gen_tri_outer = true; }
                else                          { gen_tri_inner = true; }
            } else if t_inner1 == t_inner0 {
                gen_tri_outer = true;
            } else if t_outer1 == t_outer0 {
                gen_tri_inner = true;
            } else {
                if keep_quads {
                    if fi >= n_leading {
                        let mirror = n_leading - 1 - (fi - n_leading - n_middle);
                        gen_quad = buf[base + mirror as usize * fstride + (fsize - 1)] >= 0;
                    } else if (t_inner1 > t_outer_middle) || (t_outer1 > t_outer_middle) {
                        gen_quad = false;
                    } else {
                        let dt_slope1 = (t_outer1 - t_inner1).abs();
                        gen_quad = dt_slope1 <= dt_slope_max;
                    }
                }
                if !gen_quad {
                    let dt_diag_outer = t_outer1 - t_inner0;
                    let dt_diag_inner = t_inner1 - t_outer0;
                    let use_outer = if dt_diag_outer == dt_diag_inner {
                        t_outer1 > t_outer_middle
                    } else {
                        dt_diag_outer < dt_diag_inner
                    };
                    if use_outer { gen_tri_outer = true; }
                    else         { gen_tri_inner = true; }
                }
            }

            let b = base + fi as usize * fstride;
            if gen_tri_outer {
                append_tri(buf, b, c_outer0, c_outer1, c_inner0, fsize);
            } else if gen_tri_inner {
                append_tri(buf, b, c_inner1, c_inner0, c_outer0, fsize);
            } else {
                // quad:
                buf[b] = c_outer0; buf[b+1] = c_outer1; buf[b+2] = c_inner1;
                if fsize == 4 { buf[b+3] = c_inner0; }
            }

            let advance_outer = gen_tri_outer || gen_quad;
            if advance_outer {
                t_outer0 = t_outer1;
                c_outer0 = c_outer1;
                t_outer1 += dt_outer;
                c_outer1 += 1;
                if t_outer1 >= t_outer_last {
                    t_outer1 = t_outer_last;
                    c_outer1 = self.outer_last;
                }
            }
            let advance_inner = gen_tri_inner || gen_quad;
            if advance_inner {
                t_inner0 = t_inner1;
                c_inner0 = c_inner1;
                t_inner1 += dt_inner;
                c_inner1 += d_inner;
                if t_inner1 >= t_inner_last {
                    t_inner1 = t_inner_last;
                    c_inner1 = self.inner_last;
                }
            }
            nf += 1;
        }
        nf
    }
}

// ===========================================================================
//  QUAD parameterization helpers
// ===========================================================================

fn quad_count_uniform_facets(res: i32, tri: bool) -> i32 {
    (res * res) << (tri as i32)
}

fn quad_count_segmented_facets(outer: &[i32], tri: bool) -> i32 {
    // Pick the direction with non-unit rates:
    let t_res = if (outer[0] * outer[2]) == 1 { &outer[1..] } else { &outer[0..] };
    if tri {
        t_res[0] + t_res[2]
    } else {
        let n_quads  = t_res[0].min(t_res[2]);
        let n_facets = t_res[0].max(t_res[2]);
        let split = ((n_quads & 1) != 0) && ((n_facets & 1) == 0);
        n_facets + split as i32
    }
}

fn quad_count_non_uniform_edge_facets(outer_res: i32, inner_res: i32) -> i32 {
    let mut n = outer_res.max(inner_res - 2);
    if (n & 1) == 0 {
        n += ((outer_res & 1) | (inner_res & 1)) as i32;
    }
    n
}

fn quad_count_non_uniform_facets(outer: &[i32], inner: &[i32; 2], tri: bool) -> i32 {
    let u = inner[0]; let v = inner[1];
    let inner_u = u - 2; let inner_v = v - 2;
    let n_interior = inner_u * inner_v;
    if tri {
        let mut n = n_interior * 2;
        n += inner_u + outer[0];
        n += inner_v + outer[1];
        n += inner_u + outer[2];
        n += inner_v + outer[3];
        return n;
    }
    let uni_e = [outer[0]==u, outer[1]==v, outer[2]==u, outer[3]==v];
    let uni_c = [uni_e[0]&&uni_e[3], uni_e[1]&&uni_e[0], uni_e[2]&&uni_e[1], uni_e[3]&&uni_e[2]];
    let mut nb = 0i32;
    nb += if uni_e[0] { inner_u + 1 + !uni_c[1] as i32 } else { quad_count_non_uniform_edge_facets(outer[0], u) };
    nb += if uni_e[1] { inner_v + 1 + !uni_c[2] as i32 } else { quad_count_non_uniform_edge_facets(outer[1], v) };
    nb += if uni_e[2] { inner_u + 1 + !uni_c[3] as i32 } else { quad_count_non_uniform_edge_facets(outer[2], u) };
    nb += if uni_e[3] { inner_v + 1 + !uni_c[0] as i32 } else { quad_count_non_uniform_edge_facets(outer[3], v) };
    n_interior + nb
}

fn quad_count_interior_coords_uv(u: i32, v: i32) -> i32 {
    (u - 1) * (v - 1)
}

fn quad_get_boundary_coords<R: Float>(outer: &[i32], stride: usize, buf: &mut [R]) -> i32 {
    let one = R::one();
    let zero = R::zero();
    let mut n = 0usize;
    let dt0 = one / R::from(outer[0]).unwrap();
    append_v_isoline(buf, stride, outer[0] as usize, zero, zero, dt0);
    n += outer[0] as usize;
    let dt1 = one / R::from(outer[1]).unwrap();
    append_u_isoline(&mut buf[n * stride..], stride, outer[1] as usize, one, zero, dt1);
    n += outer[1] as usize;
    let dt2 = one / R::from(outer[2]).unwrap();
    append_v_isoline(&mut buf[n * stride..], stride, outer[2] as usize, one, one, -dt2);
    n += outer[2] as usize;
    let dt3 = one / R::from(outer[3]).unwrap();
    append_u_isoline(&mut buf[n * stride..], stride, outer[3] as usize, zero, one, -dt3);
    n += outer[3] as usize;
    n as i32
}

fn quad_get_edge_coords<R: Float>(edge: i32, res: i32, stride: usize, buf: &mut [R]) -> i32 {
    let one = R::one();
    let zero = R::zero();
    let dt = one / R::from(res).unwrap();
    let t0 = dt;
    let t1 = one - dt;
    let n  = (res - 1) as usize;
    match edge {
        0 => { append_v_isoline(buf, stride, n, t0, zero, dt); }
        1 => { append_u_isoline(buf, stride, n, one, t0, dt); }
        2 => { append_v_isoline(buf, stride, n, t1, one, -dt); }
        3 => { append_u_isoline(buf, stride, n, zero, t1, -dt); }
        _ => {}
    }
    n as i32
}

fn quad_get_interior_coords<R: Float>(inner: &[i32; 2], stride: usize, buf: &mut [R]) -> i32 {
    let n_int_rings = (inner[0] / 2).min(inner[1] / 2);
    if n_int_rings == 0 { return 0; }
    let du = R::one() / R::from(inner[0]).unwrap();
    let dv = R::one() / R::from(inner[1]).unwrap();
    let mut u = du;
    let mut v = dv;
    let mut u_res = inner[0] - 2;
    let mut v_res = inner[1] - 2;
    let mut n = 0usize;
    for _ in 0..n_int_rings {
        n += quad_get_interior_ring_coords(u_res, v_res, u, v, du, dv, stride, &mut buf[n * stride..]) as usize;
        u_res -= 2; v_res -= 2;
        u = u + du; v = v + dv;
    }
    n as i32
}

fn quad_get_interior_ring_coords<R: Float>(u_res: i32, v_res: i32,
                                            u0: R, v0: R, du: R, dv: R,
                                            stride: usize, buf: &mut [R]) -> i32 {
    if u_res > 0 && v_res > 0 {
        let u1 = R::one() - u0;
        let v1 = R::one() - v0;
        let mut n = 0usize;
        append_v_isoline(&mut buf[n * stride..], stride, u_res as usize, u0, v0, du); n += u_res as usize;
        append_u_isoline(&mut buf[n * stride..], stride, v_res as usize, u1, v0, dv); n += v_res as usize;
        append_v_isoline(&mut buf[n * stride..], stride, u_res as usize, u1, v1, -du); n += u_res as usize;
        append_u_isoline(&mut buf[n * stride..], stride, v_res as usize, u0, v1, -dv); n += v_res as usize;
        n as i32
    } else if u_res > 0 {
        append_v_isoline(buf, stride, (u_res + 1) as usize, u0, v0, du);
        u_res + 1
    } else if v_res > 0 {
        append_u_isoline(buf, stride, (v_res + 1) as usize, u0, v0, dv);
        v_res + 1
    } else {
        // centre:
        buf[0] = R::from(0.5).unwrap(); buf[1] = R::from(0.5).unwrap();
        1
    }
}

// -----  Quad facet builders -----

fn quad_get_uniform_facets(res: i32, tri: bool, fsize: usize, fstride: usize,
                            buf: &mut [i32]) -> i32 {
    let n_rings = (res + 1) / 2;
    let mut nf  = 0i32;
    let mut coord0 = 0i32;
    let mut cur_res = res;
    for _ in 0..n_rings {
        let n = quad_get_interior_ring_facets(cur_res, cur_res, coord0, tri, fsize, fstride,
                                              &mut buf[nf as usize * fstride..]);
        nf += n;
        coord0 += 4 * cur_res;
        cur_res -= 2;
    }
    nf
}

fn quad_get_segmented_facets(outer: &[i32], tri: bool, fsize: usize, fstride: usize,
                              buf: &mut [i32]) -> i32 {
    // Uniform in both directions:
    if outer[0] == outer[2] && outer[1] == outer[3] {
        return quad_get_single_strip_facets(outer[0], outer[1], 0, tri, fsize, fstride, buf);
    }
    let n_facet_coords = outer[0] + outer[1] + outer[2] + outer[3];
    let mut strip = FacetStrip {
        quad_topology:    true,
        quad_triangulate: tri,
        split_first:      false,
        split_last:       false,
        inner_reversed:   true,
        include_last:     true,
        ..Default::default()
    };
    if (outer[0] != 1) || (outer[2] != 1) {
        strip.outer_edges = outer[0];
        strip.inner_edges = outer[2] - 2;
        strip.outer_first = 0;
        strip.outer_last  = strip.outer_edges;
        strip.inner_last  = strip.outer_last + 1;
        strip.inner_first = n_facet_coords - 2;
        strip.outer_prev  = n_facet_coords - 1;
    } else {
        strip.outer_edges = outer[1];
        strip.inner_edges = outer[3] - 2;
        strip.outer_prev  = 0;
        strip.outer_first = 1;
        strip.outer_last  = 1 + strip.outer_edges;
        strip.inner_last  = strip.outer_last + 1;
        strip.inner_first = n_facet_coords - 1;
    }
    strip.connect_non_uniform_facets(buf, 0, fsize, fstride)
}

fn quad_get_non_uniform_facets(outer: &[i32], inner: &[i32; 2], n_boundary: usize,
                                tri: bool, fsize: usize, fstride: usize,
                                buf: &mut [i32]) -> i32 {
    let u = inner[0]; let v = inner[1];
    let mut nf = quad_get_boundary_ring_facets(outer, u, v, n_boundary, tri, fsize, fstride, buf);
    let n_rings = (u.min(v) + 1) / 2;
    let mut coord0 = n_boundary as i32;
    let mut cur_u  = u; let mut cur_v = v;
    for ring in 1..n_rings {
        let _ = ring;
        cur_u = 0.max(cur_u - 2);
        cur_v = 0.max(cur_v - 2);
        let n = quad_get_interior_ring_facets(cur_u, cur_v, coord0, tri, fsize, fstride,
                                              &mut buf[nf as usize * fstride..]);
        nf += n;
        coord0 += 2 * (cur_u + cur_v);
    }
    nf
}

fn quad_get_single_strip_facets(u_res: i32, v_res: i32, coord0: i32,
                                 tri: bool, fsize: usize, fstride: usize,
                                 buf: &mut [i32]) -> i32 {
    let mut strip = FacetStrip {
        quad_topology:    true,
        quad_triangulate: tri,
        split_first:      false,
        split_last:       false,
        inner_reversed:   true,
        include_last:     true,
        ..Default::default()
    };
    if u_res > 1 {
        strip.outer_edges = u_res;
        strip.inner_edges = u_res - 2;
        strip.outer_first = coord0;
        strip.outer_last  = strip.outer_first + u_res;
        strip.inner_last  = strip.outer_last + 2;
        strip.inner_first = strip.outer_last + u_res;
        strip.outer_prev  = strip.inner_first + 1;
    } else {
        strip.outer_edges = v_res;
        strip.inner_edges = v_res - 2;
        strip.outer_prev  = coord0;
        strip.outer_first = coord0 + 1;
        strip.outer_last  = strip.outer_first + v_res;
        strip.inner_last  = strip.outer_last + 2;
        strip.inner_first = strip.outer_last + v_res;
    }
    strip.connect_uniform_quads(buf, 0, fsize, fstride)
}

fn quad_get_interior_ring_facets(u_res: i32, v_res: i32, coord0: i32,
                                  tri: bool, fsize: usize, fstride: usize,
                                  buf: &mut [i32]) -> i32 {
    let total = u_res * v_res;
    if total == 0 { return 0; }
    let tri_sign = if tri { 1i32 } else { 0 };
    if total == 1 {
        return append_quad(buf, 0, fsize, fstride, coord0, coord0+1, coord0+2, coord0+3, tri_sign);
    }
    if u_res == 1 || v_res == 1 {
        return quad_get_single_strip_facets(u_res, v_res, coord0, tri, fsize, fstride, buf);
    }
    let u_inner = u_res - 2;
    let v_inner = v_res - 2;
    let outer_ring_start = coord0;
    let inner_ring_start = coord0 + 2 * (u_res + v_res);
    let mut nf = 0i32;
    let mut strip = FacetStrip {
        quad_topology:    true,
        quad_triangulate: tri,
        split_first:      false,
        split_last:       false,
        ..Default::default()
    };
    strip.outer_edges    = u_res;
    strip.outer_first    = outer_ring_start;
    strip.outer_prev     = inner_ring_start - 1;
    strip.outer_last     = outer_ring_start + u_res;
    strip.inner_edges    = u_inner;
    strip.inner_reversed = false;
    strip.inner_first    = inner_ring_start;
    strip.inner_last     = inner_ring_start + u_inner;
    nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride);

    strip.outer_edges    = v_res;
    let prev_outer = strip.outer_first;
    strip.outer_first    = strip.outer_last;
    strip.outer_prev     = strip.outer_first - 1;
    strip.outer_last     = prev_outer + u_res + v_res;
    strip.inner_edges    = v_inner;
    strip.inner_first    = strip.inner_last;
    strip.inner_last     = strip.inner_first + v_inner;
    nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride);

    strip.outer_edges    = u_res;
    strip.outer_first    = strip.outer_last;
    strip.outer_prev     = strip.outer_first - 1;
    strip.outer_last     = strip.outer_first + u_res;
    strip.inner_edges    = u_inner;
    strip.inner_reversed = v_inner == 0;
    strip.inner_first    = strip.inner_last;
    strip.inner_last    += u_inner * (if strip.inner_reversed { -1 } else { 1 });
    nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride);

    strip.outer_edges    = v_res;
    strip.outer_first    = strip.outer_last;
    strip.outer_prev     = strip.outer_first - 1;
    strip.outer_last     = outer_ring_start;
    strip.inner_edges    = v_inner;
    strip.inner_reversed = u_inner == 0;
    strip.inner_first    = strip.inner_last;
    strip.inner_last     = inner_ring_start;
    nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride);

    nf
}

fn quad_get_boundary_ring_facets(outer: &[i32], u_res: i32, v_res: i32,
                                  n_boundary: usize, tri: bool, fsize: usize,
                                  fstride: usize, buf: &mut [i32]) -> i32 {
    let uni_e = [outer[0]==u_res, outer[1]==v_res, outer[2]==u_res, outer[3]==v_res];
    let uni_c = [uni_e[0]&&uni_e[3], uni_e[1]&&uni_e[0], uni_e[2]&&uni_e[1], uni_e[3]&&uni_e[2]];
    let inner_u = u_res - 2;
    let inner_v = v_res - 2;
    let inner_start = n_boundary as i32;
    let mut nf = 0i32;
    let mut strip = FacetStrip {
        quad_topology:    true,
        quad_triangulate: tri,
        ..Default::default()
    };

    strip.outer_edges    = outer[0];
    strip.outer_first    = 0;
    strip.outer_prev     = inner_start - 1;
    strip.outer_last     = outer[0];
    strip.inner_edges    = inner_u;
    strip.inner_reversed = false;
    strip.inner_first    = inner_start;
    strip.inner_last     = inner_start + inner_u;
    if uni_e[0] { strip.split_first = !uni_c[0]; strip.split_last = !uni_c[1];
                  nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }

    strip.outer_edges    = outer[1];
    strip.outer_first    = strip.outer_last;
    strip.outer_prev     = strip.outer_first - 1;
    strip.outer_last    += outer[1];
    strip.inner_edges    = inner_v;
    strip.inner_first    = strip.inner_last;
    strip.inner_last    += inner_v;
    if uni_e[1] { strip.split_first = !uni_c[1]; strip.split_last = !uni_c[2];
                  nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }

    strip.outer_edges    = outer[2];
    strip.outer_first    = strip.outer_last;
    strip.outer_prev     = strip.outer_first - 1;
    strip.outer_last    += outer[2];
    strip.inner_edges    = inner_u;
    strip.inner_reversed = inner_v == 0;
    strip.inner_first    = strip.inner_last;
    strip.inner_last    += inner_u * (if strip.inner_reversed { -1 } else { 1 });
    if uni_e[2] { strip.split_first = !uni_c[2]; strip.split_last = !uni_c[3];
                  nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }

    strip.outer_edges    = outer[3];
    strip.outer_first    = strip.outer_last;
    strip.outer_prev     = strip.outer_first - 1;
    strip.outer_last     = 0;
    strip.inner_edges    = inner_v;
    strip.inner_reversed = inner_u == 0;
    strip.inner_first    = strip.inner_last;
    strip.inner_last     = inner_start;
    if uni_e[3] { strip.split_first = !uni_c[3]; strip.split_last = !uni_c[0];
                  nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }
    nf
}

// ===========================================================================
//  TRI parameterization helpers
// ===========================================================================

fn tri_count_uniform_facets(res: i32) -> i32 { res * res }
fn tri_count_uniform_coords(res: i32)  -> i32 { res * (res + 1) / 2 }
fn tri_count_interior_coords(res: i32) -> i32 { tri_count_uniform_coords(res - 2) }

fn tri_count_non_uniform_facets(outer: &[i32], inner: i32) -> i32 {
    let inner_edges = inner - 3;
    let n_interior  = if inner_edges > 0 { tri_count_uniform_facets(inner_edges) } else { 0 };
    let n_boundary  = (inner_edges + outer[0])
                    + (inner_edges + outer[1])
                    + (inner_edges + outer[2]);
    n_interior + n_boundary
}

fn tri_get_boundary_coords<R: Float>(outer: &[i32], stride: usize, buf: &mut [R]) -> i32 {
    let one = R::one();
    let zero = R::zero();
    let mut n = 0usize;
    let dt0 = one / R::from(outer[0]).unwrap();
    append_v_isoline(&mut buf[n * stride..], stride, outer[0] as usize, zero, zero, dt0);
    n += outer[0] as usize;
    let dt1 = one / R::from(outer[1]).unwrap();
    append_uv_line(&mut buf[n * stride..], stride, outer[1] as usize, one, zero, -dt1, dt1);
    n += outer[1] as usize;
    let dt2 = one / R::from(outer[2]).unwrap();
    append_u_isoline(&mut buf[n * stride..], stride, outer[2] as usize, zero, one, -dt2);
    n += outer[2] as usize;
    n as i32
}

fn tri_get_edge_coords<R: Float>(edge: i32, res: i32, stride: usize, buf: &mut [R]) -> i32 {
    let one = R::one();
    let zero = R::zero();
    let dt = one / R::from(res).unwrap();
    let t0 = dt;
    let t1 = one - dt;
    let n  = (res - 1) as usize;
    match edge {
        0 => { append_v_isoline(buf, stride, n, t0, zero, dt); }
        1 => { append_uv_line(buf, stride, n, t1, t0, -dt, dt); }
        2 => { append_u_isoline(buf, stride, n, zero, t1, -dt); }
        _ => {}
    }
    n as i32
}

fn tri_get_interior_coords<R: Float>(res: i32, stride: usize, buf: &mut [R]) -> i32 {
    let n_rings = res / 3;
    if n_rings == 0 { return 0; }
    let dt      = R::one() / R::from(res).unwrap();
    let mut u   = dt;
    let mut v   = dt;
    let mut ring_res = res - 3;
    let mut n = 0usize;
    for _ in 0..n_rings {
        if ring_res == 0 {
            buf[n * stride] = R::from(1.0 / 3.0).unwrap();
            buf[n * stride + 1] = R::from(1.0 / 3.0).unwrap();
            n += 1;
        } else {
            n += tri_get_interior_ring_coords(ring_res, u, v, dt, stride, &mut buf[n * stride..]) as usize;
        }
        ring_res -= 3;
        u = u + dt; v = v + dt;
    }
    n as i32
}

fn tri_get_interior_ring_coords<R: Float>(res: i32, u0: R, v0: R, dt: R,
                                           stride: usize, buf: &mut [R]) -> i32 {
    let one = R::one();
    let two = R::from(2.0).unwrap();
    let u1  = one - u0 * two;
    let v1  = one - v0 * two;
    let mut n = 0usize;
    append_v_isoline(&mut buf[n * stride..], stride, res as usize, u0, v0, dt); n += res as usize;
    append_uv_line(&mut buf[n * stride..], stride, res as usize, u1, v0, -dt, dt); n += res as usize;
    append_u_isoline(&mut buf[n * stride..], stride, res as usize, u0, v1, -dt); n += res as usize;
    n as i32
}

fn tri_get_uniform_facets(res: i32, fsize: usize, fstride: usize, buf: &mut [i32]) -> i32 {
    let n_rings = 1 + res / 3;
    let mut nf  = 0i32;
    let mut coord0 = 0i32;
    let mut cur = res;
    for _ in 0..n_rings {
        let n = tri_get_interior_ring_facets(cur, coord0, fsize, fstride, &mut buf[nf as usize * fstride..]);
        nf += n;
        coord0 += 3 * cur;
        cur -= 3;
    }
    nf
}

fn tri_get_non_uniform_facets(outer: &[i32], inner: i32, n_boundary: usize,
                               fsize: usize, fstride: usize, buf: &mut [i32]) -> i32 {
    let mut nf = tri_get_boundary_ring_facets(outer, inner, n_boundary, fsize, fstride, buf);
    let n_rings = 1 + inner / 3;
    let mut coord0 = n_boundary as i32;
    let mut cur = inner;
    for ring in 1..n_rings {
        let _ = ring;
        cur -= 3;
        let n = tri_get_interior_ring_facets(cur, coord0, fsize, fstride, &mut buf[nf as usize * fstride..]);
        nf += n;
        coord0 += 3 * cur;
    }
    nf
}

fn tri_get_interior_ring_facets(res: i32, coord0: i32, fsize: usize, fstride: usize,
                                 buf: &mut [i32]) -> i32 {
    if res < 1 { return 0; }
    if res == 1 {
        return append_tri(buf, 0, coord0, coord0+1, coord0+2, fsize);
    }
    if res == 2 {
        append_tri(buf, 0 * fstride, coord0+0, coord0+1, coord0+5, fsize);
        append_tri(buf, 1 * fstride, coord0+2, coord0+3, coord0+1, fsize);
        append_tri(buf, 2 * fstride, coord0+4, coord0+5, coord0+3, fsize);
        append_tri(buf, 3 * fstride, coord0+1, coord0+3, coord0+5, fsize);
        return 4;
    }
    let outer_edges = res;
    let inner_edges = res - 3;
    let outer_start = coord0;
    let inner_start = coord0 + 3 * res;
    let mut nf = 0i32;
    let mut strip = FacetStrip {
        quad_topology:  false,
        inner_reversed: false,
        inner_edges,
        outer_edges,
        ..Default::default()
    };
    strip.outer_first = outer_start;
    strip.outer_last  = outer_start + outer_edges;
    strip.outer_prev  = inner_start - 1;
    strip.inner_first = inner_start;
    strip.inner_last  = inner_start + inner_edges;
    nf += strip.connect_uniform_tris(buf, nf as usize * fstride, fsize, fstride);

    strip.outer_first += outer_edges;
    strip.outer_last  += outer_edges;
    strip.outer_prev   = strip.outer_first - 1;
    strip.inner_first += inner_edges;
    strip.inner_last  += inner_edges;
    nf += strip.connect_uniform_tris(buf, nf as usize * fstride, fsize, fstride);

    strip.outer_first += outer_edges;
    strip.outer_last   = outer_start;
    strip.outer_prev   = strip.outer_first - 1;
    strip.inner_first += inner_edges;
    strip.inner_last   = inner_start;
    nf += strip.connect_uniform_tris(buf, nf as usize * fstride, fsize, fstride);
    nf
}

fn tri_get_boundary_ring_facets(outer: &[i32], inner: i32, n_boundary: usize,
                                 fsize: usize, fstride: usize, buf: &mut [i32]) -> i32 {
    let uni_e = [outer[0]==inner, outer[1]==inner, outer[2]==inner];
    let uni_c = [uni_e[0]&&uni_e[2], uni_e[1]&&uni_e[0], uni_e[2]&&uni_e[1]];
    let inner_edges = inner - 3;
    let inner_start = n_boundary as i32;
    let mut nf = 0i32;
    let mut strip = FacetStrip {
        quad_topology:  false,
        inner_reversed: false,
        inner_edges,
        ..Default::default()
    };
    strip.outer_edges   = outer[0];
    strip.outer_first   = 0;
    strip.outer_last    = outer[0];
    strip.outer_prev    = inner_start - 1;
    strip.inner_first   = inner_start;
    strip.inner_last    = inner_start + inner_edges;
    if uni_e[0] { strip.split_first = !uni_c[0]; strip.split_last = !uni_c[1];
                  nf += strip.connect_uniform_tris(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }

    strip.outer_edges   = outer[1];
    strip.outer_first   = strip.outer_last;
    strip.outer_last   += outer[1];
    strip.outer_prev    = strip.outer_first - 1;
    strip.inner_first   = strip.inner_last;
    strip.inner_last   += inner_edges;
    if uni_e[1] { strip.split_first = !uni_c[1]; strip.split_last = !uni_c[2];
                  nf += strip.connect_uniform_tris(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }

    strip.outer_edges   = outer[2];
    strip.outer_first   = strip.outer_last;
    strip.outer_last    = 0;
    strip.outer_prev    = strip.outer_first - 1;
    strip.inner_first   = strip.inner_last;
    strip.inner_last    = inner_start;
    if uni_e[2] { strip.split_first = !uni_c[2]; strip.split_last = !uni_c[0];
                  nf += strip.connect_uniform_tris(buf, nf as usize * fstride, fsize, fstride); }
    else        { strip.split_first = true; strip.split_last = true;
                  nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride); }
    nf
}

// ===========================================================================
//  QSUB (quad sub-faces / N-gon) helpers
// ===========================================================================

fn qsub_count_uniform_coords(n: i32, res: i32) -> i32 {
    let h = res / 2;
    if res & 1 != 0 {
        (h + 1) * (h + 1) * n + (if n == 3 { 0 } else { 1 })
    } else {
        h * (h + 1) * n + 1
    }
}

fn qsub_count_interior_coords(n: i32, res: i32) -> i32 {
    qsub_count_uniform_coords(n, res - 2)
}

fn qsub_count_uniform_facets(n: i32, res: i32, tri: bool) -> i32 {
    let res_is_odd = (res & 1) != 0;
    let h = res / 2;
    let n_quads  = (h + res_is_odd as i32) * h * n;
    let n_center = if res_is_odd { if n == 3 { 1 } else { n } } else { 0 };
    (n_quads << (tri as i32)) + n_center
}

fn qsub_count_non_uniform_facets(n: i32, outer: &[i32], inner: i32, tri: bool) -> i32 {
    let inner_edges = inner - 2;
    let n_interior  = if inner_edges > 0 {
        qsub_count_uniform_facets(n, inner_edges, tri)
    } else { 0 };
    let mut n_boundary = 0i32;
    for i in 0..n as usize {
        if tri {
            n_boundary += inner_edges + outer[i];
        } else if outer[i] == inner {
            n_boundary += inner_edges + 1 + (inner != outer[(i + 1) % n as usize]) as i32;
        } else {
            let mut ne = inner_edges.max(outer[i]);
            if (ne & 1) == 0 {
                ne += ((inner_edges & 1) | (outer[i] & 1)) as i32;
            }
            n_boundary += ne;
        }
    }
    n_interior + n_boundary
}

fn qsub_get_boundary_coords<R: Float>(param: Parameterization, outer: &[i32],
                                       stride: usize, buf: &mut [R]) -> i32 {
    let n  = param.get_face_size();
    let mut off = 0usize;
    for i in 0..n {
        // Boundary coords include the leading vertex (inc_first=true) but
        // exclude the trailing vertex (inc_last=false) — it is the leading
        // vertex of the next edge.  Mirrors C++ qsub::GetBoundaryCoords.
        off += qsub_get_ring_edge_coords(param, i, outer[i as usize], true, false,
                                          R::zero(), R::one() / R::from(outer[i as usize]).unwrap(),
                                          stride, &mut buf[off * stride..]) as usize;
    }
    off as i32
}

fn qsub_get_edge_coords<R: Float>(param: Parameterization, edge: i32, res: i32,
                                   stride: usize, buf: &mut [R]) -> i32 {
    qsub_get_ring_edge_coords(param, edge, res, false, false,
                               R::zero(), R::one() / R::from(res).unwrap(),
                               stride, buf)
}

fn qsub_get_ring_edge_coords<R: Float>(param: Parameterization, edge: i32, res: i32,
                                        inc_first: bool, inc_last: bool,
                                        t_origin: R, dt: R,
                                        stride: usize, buf: &mut [R]) -> i32 {
    let n0 = (res - 1) / 2;
    let n1 = (res - 1) - n0;
    let mut nc = 0usize;

    if inc_first || n0 > 0 {
        let uv0 = param.get_vertex_coord::<R>(edge);
        if inc_first {
            buf[nc * stride]     = uv0[0] + t_origin;
            buf[nc * stride + 1] = uv0[1] + t_origin;
            nc += 1;
        }
        if n0 > 0 {
            let u = uv0[0] + t_origin + dt;
            let v = uv0[1] + t_origin;
            append_v_isoline(&mut buf[nc * stride..], stride, n0 as usize, u, v, dt);
            nc += n0 as usize;
        }
    }
    if n1 > 0 || inc_last {
        let next = (edge + 1) % param.get_face_size();
        let uv1  = param.get_vertex_coord::<R>(next);
        if n1 > 0 {
            let u = uv1[0] + t_origin;
            let v = uv1[1] + if (res & 1) != 0 {
                R::from(0.5).unwrap() - R::from(0.5).unwrap() * dt
            } else {
                R::from(0.5).unwrap()
            };
            append_u_isoline(&mut buf[nc * stride..], stride, n1 as usize, u, v, -dt);
            nc += n1 as usize;
        }
        if inc_last {
            buf[nc * stride]     = uv1[0] + t_origin;
            buf[nc * stride + 1] = uv1[1] + t_origin;
            nc += 1;
        }
    }
    nc as i32
}

fn qsub_get_interior_coords<R: Float>(param: Parameterization, res: i32,
                                       stride: usize, buf: &mut [R]) -> i32 {
    let n_rings = res / 2;
    if n_rings == 0 { return 0; }
    let dt = R::one() / R::from(res).unwrap();
    let mut t = dt;
    let mut ring_res = res - 2;
    let mut n = 0usize;
    for _ in 0..n_rings {
        if ring_res == 0 {
            buf[n * stride] = R::from(0.5).unwrap(); buf[n * stride + 1] = R::from(0.5).unwrap();
            n += 1;
        } else if ring_res == 1 {
            // centre ring coords:
            let big_n = param.get_face_size();
            for i in 0..big_n {
                let uv = param.get_vertex_coord::<R>(i);
                buf[n * stride]     = uv[0] + t;
                buf[n * stride + 1] = uv[1] + t;
                n += 1;
            }
            if big_n != 3 {
                buf[n * stride] = R::from(0.5).unwrap(); buf[n * stride + 1] = R::from(0.5).unwrap();
                n += 1;
            }
        } else {
            n += qsub_get_interior_ring_coords(param, ring_res, t, dt, stride, &mut buf[n * stride..]) as usize;
        }
        ring_res -= 2;
        t = t + dt;
    }
    n as i32
}

fn qsub_get_interior_ring_coords<R: Float>(param: Parameterization, res: i32,
                                            t_origin: R, dt: R,
                                            stride: usize, buf: &mut [R]) -> i32 {
    let n = param.get_face_size();
    let mut off = 0usize;
    for i in 0..n {
        off += qsub_get_ring_edge_coords(param, i, res, true, false,
                                          t_origin, dt, stride, &mut buf[off * stride..]) as usize;
    }
    off as i32
}

fn qsub_get_uniform_facets(n: i32, res: i32, tri: bool, fsize: usize, fstride: usize,
                            buf: &mut [i32]) -> i32 {
    if res == 1 {
        return qsub_get_center_facets(n, 0, fsize, fstride, buf);
    }
    let n_rings = (res + 1) / 2;
    let mut nf  = 0i32;
    let mut coord0 = 0i32;
    let mut cur = res;
    for _ in 0..n_rings {
        let cnt = qsub_get_interior_ring_facets(n, cur, coord0, tri, fsize, fstride,
                                                 &mut buf[nf as usize * fstride..]);
        nf += cnt;
        coord0 += n * cur;
        cur -= 2;
    }
    nf
}

fn qsub_get_non_uniform_facets(n: i32, outer: &[i32], inner: i32, n_boundary: usize,
                                tri: bool, fsize: usize, fstride: usize,
                                buf: &mut [i32]) -> i32 {
    let mut nf = qsub_get_boundary_ring_facets(n, outer, inner, n_boundary, tri, fsize, fstride, buf);
    let n_rings = (inner + 1) / 2;
    let mut coord0 = n_boundary as i32;
    let mut cur = inner;
    for ring in 1..n_rings {
        let _ = ring;
        cur = 0.max(cur - 2);
        let cnt = qsub_get_interior_ring_facets(n, cur, coord0, tri, fsize, fstride,
                                                 &mut buf[nf as usize * fstride..]);
        nf += cnt;
        coord0 += n * cur;
    }
    nf
}

fn qsub_get_center_facets(n: i32, coord0: i32, fsize: usize, fstride: usize,
                           buf: &mut [i32]) -> i32 {
    if n == 3 {
        append_tri(buf, 0, coord0, coord0+1, coord0+2, fsize)
    } else {
        append_tri_fan(buf, 0, fstride, fsize, n, coord0)
    }
}

fn qsub_get_interior_ring_facets(n: i32, res: i32, coord0: i32,
                                  tri: bool, fsize: usize, fstride: usize,
                                  buf: &mut [i32]) -> i32 {
    if res < 1 { return 0; }
    if res == 1 {
        return qsub_get_center_facets(n, coord0, fsize, fstride, buf);
    }
    let outer_res  = res;
    let outer_ring = coord0;
    let inner_res  = outer_res - 2;
    let inner_ring = outer_ring + n * outer_res;
    let mut nf     = 0i32;
    let mut strip  = FacetStrip {
        quad_topology:    true,
        quad_triangulate: tri,
        outer_edges:      outer_res,
        inner_edges:      inner_res,
        inner_reversed:   false,
        split_first:      false,
        split_last:       false,
        ..Default::default()
    };
    for edge in 0..n {
        strip.outer_first = outer_ring + edge * outer_res;
        strip.inner_first = inner_ring + edge * inner_res;
        strip.outer_prev  = if edge > 0 { strip.outer_first - 1 } else { inner_ring - 1 };
        if edge < n - 1 {
            strip.outer_last = strip.outer_first + outer_res;
            strip.inner_last = strip.inner_first + inner_res;
        } else {
            strip.outer_last = outer_ring;
            strip.inner_last = inner_ring;
        }
        nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride);
    }
    nf
}

fn qsub_get_boundary_ring_facets(n: i32, outer: &[i32], inner: i32,
                                  n_boundary: usize, tri: bool,
                                  fsize: usize, fstride: usize, buf: &mut [i32]) -> i32 {
    let inner_edges = 0.max(inner - 2);
    let outer_start = 0i32;
    let inner_start = n_boundary as i32;
    let mut nf = 0i32;
    let mut strip = FacetStrip {
        quad_topology:    true,
        quad_triangulate: tri,
        inner_reversed:   false,
        inner_edges,
        ..Default::default()
    };
    for edge in 0..n {
        let e = edge as usize;
        strip.outer_edges = outer[e];
        if edge > 0 {
            strip.outer_first = strip.outer_last;
            strip.outer_prev  = strip.outer_first - 1;
            strip.inner_first = strip.inner_last;
        } else {
            strip.outer_first = outer_start;
            strip.outer_prev  = inner_start - 1;
            strip.inner_first = inner_start;
        }
        if edge < n - 1 {
            strip.outer_last = strip.outer_first + strip.outer_edges;
            strip.inner_last = strip.inner_first + inner_edges;
        } else {
            strip.outer_last = outer_start;
            strip.inner_last = inner_start;
        }
        if (outer[e] == inner) && (inner > 1) {
            let prev_e = ((edge - 1 + n) % n) as usize;
            strip.split_first = outer[prev_e] != inner;
            strip.split_last  = outer[(edge as usize + 1) % n as usize] != inner;
            nf += strip.connect_uniform_quads(buf, nf as usize * fstride, fsize, fstride);
        } else {
            strip.split_first = true;
            strip.split_last  = true;
            nf += strip.connect_non_uniform_facets(buf, nf as usize * fstride, fsize, fstride);
        }
    }
    nf
}

// ===========================================================================
//  Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::SchemeType;

    fn quad_param() -> Parameterization {
        Parameterization::new(SchemeType::Catmark, 4)
    }
    fn tri_param() -> Parameterization {
        Parameterization::new(SchemeType::Loop, 3)
    }

    // --- options ---

    #[test]
    fn options_default() {
        let o = TessellationOptions::default();
        assert_eq!(o.get_facet_size(), 3);
        assert_eq!(o.get_coord_stride(), 2);
        assert!(!o.preserve_quads);
    }

    #[test]
    fn options_chaining() {
        let o = TessellationOptions::default()
            .with_facet_size(4)
            .with_coord_stride(3)
            .with_preserve_quads(true);
        assert_eq!(o.get_facet_size(), 4);
        assert_eq!(o.get_coord_stride(), 3);
        assert!(o.preserve_quads);
    }

    // --- construction / validity ---

    #[test]
    fn invalid_param_makes_invalid_tess() {
        let bad = Parameterization::default(); // face_size=0 → invalid
        let t = Tessellation::new_uniform(bad, 2, TessellationOptions::default());
        assert!(!t.is_valid());
    }

    #[test]
    fn invalid_rate_makes_invalid_tess() {
        let t = Tessellation::new_uniform(quad_param(), 0, TessellationOptions::default());
        assert!(!t.is_valid());
    }

    #[test]
    fn quad_uniform_rate1_is_valid() {
        let t = Tessellation::new_uniform(quad_param(), 1, TessellationOptions::default());
        assert!(t.is_valid());
        assert!(t.is_uniform());
    }

    #[test]
    fn tri_uniform_rate1_is_valid() {
        let t = Tessellation::new_uniform(tri_param(), 1, TessellationOptions::default());
        assert!(t.is_valid());
        assert!(t.is_uniform());
    }

    // --- coordinate counts ---

    #[test]
    fn quad_uniform_rate2_coord_counts() {
        let t = Tessellation::new_uniform(quad_param(), 2, TessellationOptions::default());
        assert!(t.is_valid());
        // boundary: 2+2+2+2 = 8, interior: 1x1 = 1
        assert_eq!(t.get_num_boundary_coords(), 8);
        assert_eq!(t.get_num_interior_coords(), 1);
        assert_eq!(t.get_num_coords(), 9);
    }

    #[test]
    fn quad_uniform_rate4_coord_counts() {
        let t = Tessellation::new_uniform(quad_param(), 4, TessellationOptions::default());
        assert!(t.is_valid());
        assert_eq!(t.get_num_boundary_coords(), 16);
        assert_eq!(t.get_num_interior_coords(), 9);
    }

    #[test]
    fn tri_uniform_rate3_coord_counts() {
        let t = Tessellation::new_uniform(tri_param(), 3, TessellationOptions::default());
        // boundary: 3*3=9, interior: tri_uniform_coords(3-2)=tri_uniform_coords(1)=1
        assert_eq!(t.get_num_boundary_coords(), 9);
        assert_eq!(t.get_num_interior_coords(), 1);
    }

    // --- facet counts ---

    #[test]
    fn quad_uniform_rate1_single_facet() {
        let t = Tessellation::new_uniform(quad_param(), 1,
            TessellationOptions::default().with_facet_size(4).with_preserve_quads(true));
        assert_eq!(t.get_num_facets(), 1);
    }

    #[test]
    fn quad_uniform_rate2_tri_facets() {
        let t = Tessellation::new_uniform(quad_param(), 2, TessellationOptions::default());
        // 2*2 tris = 8
        assert_eq!(t.get_num_facets(), 8);
    }

    #[test]
    fn quad_uniform_rate2_quad_facets() {
        let t = Tessellation::new_uniform(quad_param(), 2,
            TessellationOptions::default().with_facet_size(4).with_preserve_quads(true));
        // 2*2 quads = 4
        assert_eq!(t.get_num_facets(), 4);
    }

    #[test]
    fn tri_uniform_rate1_single_facet() {
        let t = Tessellation::new_uniform(tri_param(), 1, TessellationOptions::default());
        assert_eq!(t.get_num_facets(), 1);
    }

    #[test]
    fn tri_uniform_rate2_facets() {
        let t = Tessellation::new_uniform(tri_param(), 2, TessellationOptions::default());
        // 2*2 = 4 tris
        assert_eq!(t.get_num_facets(), 4);
    }

    // --- boundary coord retrieval ---

    #[test]
    fn quad_boundary_coords_rate1() {
        let t = Tessellation::new_uniform(quad_param(), 1, TessellationOptions::default());
        let mut buf = vec![0.0f32; t.get_num_boundary_coords() as usize * 2];
        let n = t.get_boundary_coords(&mut buf);
        assert_eq!(n, 4);
        // corner 0 at (0,0):
        assert!((buf[0] - 0.0).abs() < 1e-5);
        assert!((buf[1] - 0.0).abs() < 1e-5);
    }

    #[test]
    fn quad_boundary_coords_rate2() {
        let t = Tessellation::new_uniform(quad_param(), 2, TessellationOptions::default());
        let mut buf = vec![0.0f64; t.get_num_boundary_coords() as usize * 2];
        let n = t.get_boundary_coords(&mut buf);
        assert_eq!(n, 8);
    }

    #[test]
    fn tri_boundary_coords_rate2() {
        let t = Tessellation::new_uniform(tri_param(), 2, TessellationOptions::default());
        let mut buf = vec![0.0f32; t.get_num_boundary_coords() as usize * 2];
        let n = t.get_boundary_coords(&mut buf);
        assert_eq!(n, 6);
    }

    // --- facet retrieval ---

    #[test]
    fn quad_rate2_quad_facets_retrieved() {
        let t = Tessellation::new_uniform(quad_param(), 2,
            TessellationOptions::default().with_facet_size(4).with_preserve_quads(true));
        let mut buf = vec![-1i32; t.get_num_facets() as usize * 4];
        let n = t.get_facets(&mut buf);
        assert_eq!(n, 4);
        // all indices should be >= 0:
        for &v in &buf { assert!(v >= 0); }
    }

    #[test]
    fn tri_rate2_facets_retrieved() {
        let t = Tessellation::new_uniform(tri_param(), 2, TessellationOptions::default());
        let mut buf = vec![-1i32; t.get_num_facets() as usize * 3];
        let n = t.get_facets(&mut buf);
        assert_eq!(n, 4);
        for &v in &buf { assert!(v >= 0); }
    }

    // --- transform ---

    #[test]
    fn transform_offset() {
        let t = Tessellation::new_uniform(quad_param(), 2, TessellationOptions::default());
        let mut buf = vec![0i32; t.get_num_facets() as usize * 3];
        t.get_facets(&mut buf);
        let orig = buf.clone();
        t.transform_facet_coord_indices_offset(&mut buf, 100);
        for (a, b) in orig.iter().zip(buf.iter()) {
            assert_eq!(*b, *a + 100);
        }
    }

    // --- non-uniform rates ---

    #[test]
    fn quad_non_uniform_two_rates() {
        let t = Tessellation::new(quad_param(), &[2, 3], TessellationOptions::default());
        assert!(t.is_valid());
        assert!(!t.is_uniform());
    }

    #[test]
    fn quad_explicit_four_outer_rates() {
        let t = Tessellation::new(quad_param(), &[2, 3, 2, 3], TessellationOptions::default());
        assert!(t.is_valid());
        assert!(!t.is_uniform());
        assert_eq!(t.get_num_boundary_coords(), 10);
    }
}
