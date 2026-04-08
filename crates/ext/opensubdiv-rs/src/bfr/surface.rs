//! Surface -- encapsulates the limit surface for a single mesh face.
//!
//! Ported from OpenSubdiv bfr/surface.h/.cpp.
//!
//! This is a generic struct parameterized over a floating-point type (`f32` or
//! `f64`), mirroring the C++ template `Surface<REAL>`.

use super::irregular_patch_type::IrregularPatchSharedPtr;
use super::parameterization::Parameterization;
use super::surface_data::SurfaceData;

pub type Index = i32;

// ---------------------------------------------------------------------------
//  PointDescriptor
// ---------------------------------------------------------------------------

/// Describes the size and stride of point data in flat arrays.
///
/// Mirrors `Surface::PointDescriptor`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointDescriptor {
    /// Number of scalar components per point (e.g. 3 for XYZ).
    pub size: i32,
    /// Distance in scalars between consecutive points (>= size).
    pub stride: i32,
}

impl PointDescriptor {
    pub fn new(size: i32) -> Self {
        PointDescriptor { size, stride: size }
    }
    pub fn with_stride(size: i32, stride: i32) -> Self {
        PointDescriptor { size, stride }
    }
}

impl Default for PointDescriptor {
    fn default() -> Self {
        PointDescriptor { size: 0, stride: 0 }
    }
}

// ---------------------------------------------------------------------------
//  Surface<REAL>
// ---------------------------------------------------------------------------

/// Limit surface for one face of a mesh, parameterized over a float type.
///
/// An instance of `Surface` is always initialized by a `SurfaceFactory`.  The
/// default-constructed instance is *invalid* (not yet assigned a surface).
///
/// Mirrors `Bfr::Surface<REAL>`.
pub struct Surface<REAL> {
    pub(crate) data: SurfaceData,
    /// Phantom to carry the precision type without storing it.
    _phantom: std::marker::PhantomData<REAL>,
}

impl<REAL: SurfaceReal> Default for Surface<REAL> {
    fn default() -> Self {
        let mut data = SurfaceData::default();
        // Mark double precision when REAL = f64.
        if REAL::IS_DOUBLE {
            data.set_double(true);
        }
        Surface {
            data,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<REAL: SurfaceReal> Surface<REAL> {
    pub fn new() -> Self {
        Self::default()
    }

    // -----------------------------------------------------------------------
    //  Simple queries
    // -----------------------------------------------------------------------

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.data.is_valid()
    }
    #[inline]
    pub fn is_regular(&self) -> bool {
        self.data.is_regular()
    }
    #[inline]
    pub fn is_linear(&self) -> bool {
        self.data.is_linear()
    }

    /// Clear a previously initialised surface (marks it invalid).
    pub fn clear(&mut self) {
        self.data.reinitialize();
    }

    /// Return the parameterization of this surface's face.
    pub fn get_parameterization(&self) -> Parameterization {
        self.data.get_param()
    }

    /// Return the face size (number of corners).
    pub fn get_face_size(&self) -> i32 {
        self.get_parameterization().get_face_size()
    }

    // -----------------------------------------------------------------------
    //  Control-point queries
    // -----------------------------------------------------------------------

    /// Return the total number of control points affecting this surface.
    pub fn get_num_control_points(&self) -> i32 {
        self.data.get_num_cvs() as i32
    }

    /// Copy CV indices into `out_indices`; returns the count.
    pub fn get_control_point_indices(&self, out_indices: &mut [Index]) -> i32 {
        let src = self.data.get_cv_indices();
        let n = src.len();
        out_indices[..n].copy_from_slice(src);
        n as i32
    }

    // -----------------------------------------------------------------------
    //  Patch-point queries
    // -----------------------------------------------------------------------

    /// Return the total number of patch points (control + additional).
    pub fn get_num_patch_points(&self) -> i32 {
        if self.is_regular() {
            self.get_num_control_points()
        } else if self.is_linear() {
            // N-sided linear: face centre + N edge midpoints.
            let n = self.get_num_control_points();
            2 * n + 1
        } else {
            self.get_irreg_patch().get_num_points_total()
        }
    }

    // -----------------------------------------------------------------------
    //  Prepare / compute patch points
    // -----------------------------------------------------------------------

    /// Gather control points from `mesh_points` into the leading part of
    /// `patch_points`, then compute any remaining patch points.
    ///
    /// Mirrors `Surface::PreparePatchPoints`.
    pub fn prepare_patch_points(
        &self,
        mesh_points: &[REAL],
        mesh_desc: PointDescriptor,
        patch_points: &mut [REAL],
        patch_desc: PointDescriptor,
    ) where
        REAL: std::ops::Add<Output = REAL>
            + std::ops::Mul<Output = REAL>
            + std::ops::Div<Output = REAL>
            + Default
            + Copy
            + From<f32>,
    {
        self.gather_control_points(mesh_points, mesh_desc, patch_points, patch_desc);
        self.compute_patch_points(patch_points, patch_desc);
    }

    /// Gather control points from a flat `mesh_points` array into
    /// `control_points`, using CV indices stored in this surface.
    pub fn gather_control_points(
        &self,
        mesh_points: &[REAL],
        mesh_desc: PointDescriptor,
        control_points: &mut [REAL],
        control_desc: PointDescriptor,
    ) {
        let n = self.get_num_control_points() as usize;
        let cvs = self.data.get_cv_indices();
        let msz = mesh_desc.size as usize;
        let mstr = mesh_desc.stride as usize;
        let cstr = control_desc.stride as usize;

        for (i, &cv) in cvs[..n].iter().enumerate() {
            let src = &mesh_points[cv as usize * mstr..cv as usize * mstr + msz];
            let dst = &mut control_points[i * cstr..i * cstr + msz];
            dst.copy_from_slice(src);
        }
    }

    /// Compute any patch points *after* the control points.
    ///
    /// Does nothing for regular surfaces (no additional points needed).
    pub fn compute_patch_points(&self, points: &mut [REAL], desc: PointDescriptor)
    where
        REAL: std::ops::Add<Output = REAL>
            + std::ops::Mul<Output = REAL>
            + std::ops::Div<Output = REAL>
            + Default
            + Copy
            + From<f32>,
    {
        if !self.is_regular() {
            if self.is_linear() {
                self.compute_linear_patch_points(points, desc);
            } else {
                self.compute_irregular_patch_points(points, desc);
            }
        }
    }

    /// Compute bounding box of control points from a local array.
    pub fn bound_control_points(
        &self,
        control_points: &[REAL],
        desc: PointDescriptor,
        bound_min: &mut [REAL],
        bound_max: &mut [REAL],
    ) where
        REAL: PartialOrd + Copy,
    {
        let n = self.get_num_control_points() as usize;
        let sz = desc.size as usize;
        let str_ = desc.stride as usize;

        bound_min[..sz].copy_from_slice(&control_points[..sz]);
        bound_max[..sz].copy_from_slice(&control_points[..sz]);

        for i in 1..n {
            let p = &control_points[i * str_..i * str_ + sz];
            for j in 0..sz {
                if p[j] < bound_min[j] {
                    bound_min[j] = p[j];
                }
                if p[j] > bound_max[j] {
                    bound_max[j] = p[j];
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Evaluation of positions and derivatives
    // -----------------------------------------------------------------------

    /// Evaluate position at `uv` using prepared `patch_points`.
    ///
    /// Mirrors `Surface::Evaluate(uv, patchPoints, desc, P)`.
    pub fn evaluate(
        &self,
        uv: [REAL; 2],
        patch_points: &[REAL],
        desc: PointDescriptor,
        p: &mut [REAL],
    ) where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        let num_derivs = 1;
        if self.is_regular() {
            let w = eval_basis_regular_f32(self, uv, num_derivs);
            apply_weighted_sum(
                p,
                desc,
                patch_points,
                &w[0],
                self.get_num_control_points() as usize,
                None,
            );
        } else if self.is_linear() {
            self.eval_multi_linear_derivs_impl(
                uv,
                patch_points,
                desc,
                Some(p),
                None,
                None,
                None,
                None,
                None,
            );
        } else {
            let (w, indices) = eval_basis_irregular_f32(self, uv, num_derivs);
            apply_weighted_sum(p, desc, patch_points, &w[0], indices.len(), Some(&indices));
        }
    }

    /// Evaluate position + 1st derivatives at `uv`.
    ///
    /// Mirrors `Surface::Evaluate(uv, patchPoints, desc, P, Du, Dv)`.
    pub fn evaluate_d1(
        &self,
        uv: [REAL; 2],
        patch_points: &[REAL],
        desc: PointDescriptor,
        p: &mut [REAL],
        du: &mut [REAL],
        dv: &mut [REAL],
    ) where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        let num_derivs = 3;
        if self.is_regular() {
            let w = eval_basis_regular_f32(self, uv, num_derivs);
            let nc = self.get_num_control_points() as usize;
            apply_weighted_sum(p, desc, patch_points, &w[0], nc, None);
            apply_weighted_sum(du, desc, patch_points, &w[1], nc, None);
            apply_weighted_sum(dv, desc, patch_points, &w[2], nc, None);
        } else if self.is_linear() {
            self.eval_multi_linear_derivs_impl(
                uv,
                patch_points,
                desc,
                Some(p),
                Some(du),
                Some(dv),
                None,
                None,
                None,
            );
        } else {
            let (w, indices) = eval_basis_irregular_f32(self, uv, num_derivs);
            let n = indices.len();
            apply_weighted_sum(p, desc, patch_points, &w[0], n, Some(&indices));
            apply_weighted_sum(du, desc, patch_points, &w[1], n, Some(&indices));
            apply_weighted_sum(dv, desc, patch_points, &w[2], n, Some(&indices));
        }
    }

    /// Evaluate position + 1st + 2nd derivatives at `uv`.
    ///
    /// Mirrors `Surface::Evaluate(uv, patchPoints, desc, P, Du, Dv, Duu, Duv, Dvv)`.
    pub fn evaluate_d2(
        &self,
        uv: [REAL; 2],
        patch_points: &[REAL],
        desc: PointDescriptor,
        p: &mut [REAL],
        du: &mut [REAL],
        dv: &mut [REAL],
        duu: &mut [REAL],
        duv: &mut [REAL],
        dvv: &mut [REAL],
    ) where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        let num_derivs = 6;
        if self.is_regular() {
            let w = eval_basis_regular_f32(self, uv, num_derivs);
            let nc = self.get_num_control_points() as usize;
            apply_weighted_sum(p, desc, patch_points, &w[0], nc, None);
            apply_weighted_sum(du, desc, patch_points, &w[1], nc, None);
            apply_weighted_sum(dv, desc, patch_points, &w[2], nc, None);
            apply_weighted_sum(duu, desc, patch_points, &w[3], nc, None);
            apply_weighted_sum(duv, desc, patch_points, &w[4], nc, None);
            apply_weighted_sum(dvv, desc, patch_points, &w[5], nc, None);
        } else if self.is_linear() {
            self.eval_multi_linear_derivs_impl(
                uv,
                patch_points,
                desc,
                Some(p),
                Some(du),
                Some(dv),
                Some(duu),
                Some(duv),
                Some(dvv),
            );
        } else {
            let (w, indices) = eval_basis_irregular_f32(self, uv, num_derivs);
            let n = indices.len();
            apply_weighted_sum(p, desc, patch_points, &w[0], n, Some(&indices));
            apply_weighted_sum(du, desc, patch_points, &w[1], n, Some(&indices));
            apply_weighted_sum(dv, desc, patch_points, &w[2], n, Some(&indices));
            apply_weighted_sum(duu, desc, patch_points, &w[3], n, Some(&indices));
            apply_weighted_sum(duv, desc, patch_points, &w[4], n, Some(&indices));
            apply_weighted_sum(dvv, desc, patch_points, &w[5], n, Some(&indices));
        }
    }

    // -----------------------------------------------------------------------
    //  Limit stencil evaluation
    // -----------------------------------------------------------------------

    /// Evaluate limit stencil weights for position only.
    ///
    /// Returns `num_control_points`. `sp` must have length >= num_control_points.
    /// Mirrors `Surface::EvaluateStencil(uv, sP)`.
    pub fn evaluate_stencil(&self, uv: [REAL; 2], sp: &mut [REAL]) -> i32
    where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        self.evaluate_stencils_dispatch(uv, sp, None, None, None, None, None)
    }

    /// Evaluate limit stencil weights for position + 1st derivatives.
    ///
    /// Mirrors `Surface::EvaluateStencil(uv, sP, sDu, sDv)`.
    pub fn evaluate_stencil_d1(
        &self,
        uv: [REAL; 2],
        sp: &mut [REAL],
        sdu: &mut [REAL],
        sdv: &mut [REAL],
    ) -> i32
    where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        self.evaluate_stencils_dispatch(uv, sp, Some(sdu), Some(sdv), None, None, None)
    }

    /// Evaluate limit stencil weights for position + 1st + 2nd derivatives.
    ///
    /// Mirrors `Surface::EvaluateStencil(uv, sP, sDu, sDv, sDuu, sDuv, sDvv)`.
    pub fn evaluate_stencil_d2(
        &self,
        uv: [REAL; 2],
        sp: &mut [REAL],
        sdu: &mut [REAL],
        sdv: &mut [REAL],
        sduu: &mut [REAL],
        sduv: &mut [REAL],
        sdvv: &mut [REAL],
    ) -> i32
    where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        self.evaluate_stencils_dispatch(
            uv,
            sp,
            Some(sdu),
            Some(sdv),
            Some(sduu),
            Some(sduv),
            Some(sdvv),
        )
    }

    // -----------------------------------------------------------------------
    //  Stencil application
    // -----------------------------------------------------------------------

    /// Apply a stencil vector (length == num_control_points) to control points
    /// from a local array, writing the result into `result`.
    pub fn apply_stencil(
        &self,
        stencil: &[REAL],
        control_points: &[REAL],
        desc: PointDescriptor,
        result: &mut [REAL],
    ) where
        REAL: std::ops::Mul<Output = REAL> + std::ops::Add<Output = REAL> + Default + Copy,
    {
        let nc = self.get_num_control_points() as usize;
        let sz = desc.size as usize;
        let str_ = desc.stride as usize;

        for j in 0..sz {
            result[j] = REAL::default();
        }
        for i in 0..nc {
            let w = stencil[i];
            let p = &control_points[i * str_..i * str_ + sz];
            for j in 0..sz {
                result[j] = result[j] + w * p[j];
            }
        }
    }

    /// Apply a stencil to control points read directly from `mesh_points`,
    /// using the CV indices stored in this surface.
    ///
    /// Mirrors `Surface::ApplyStencilFromMesh`.
    pub fn apply_stencil_from_mesh(
        &self,
        stencil: &[REAL],
        mesh_points: &[REAL],
        desc: PointDescriptor,
        result: &mut [REAL],
    ) where
        REAL: std::ops::Mul<Output = REAL> + std::ops::Add<Output = REAL> + Default + Copy,
    {
        let nc = self.get_num_control_points() as usize;
        let sz = desc.size as usize;
        let str_ = desc.stride as usize;
        let cvs = self.data.get_cv_indices();

        for j in 0..sz {
            result[j] = REAL::default();
        }
        for i in 0..nc {
            let w = stencil[i];
            let pi = cvs[i] as usize;
            let p = &mesh_points[pi * str_..pi * str_ + sz];
            for j in 0..sz {
                result[j] = result[j] + w * p[j];
            }
        }
    }

    /// Compute bounding box of control points read directly from `mesh_points`.
    ///
    /// Mirrors `Surface::BoundControlPointsFromMesh`.
    pub fn bound_control_points_from_mesh(
        &self,
        mesh_points: &[REAL],
        desc: PointDescriptor,
        bound_min: &mut [REAL],
        bound_max: &mut [REAL],
    ) where
        REAL: PartialOrd + Copy,
    {
        let nc = self.get_num_control_points() as usize;
        let sz = desc.size as usize;
        let str_ = desc.stride as usize;
        let cvs = self.data.get_cv_indices();

        let p0 = cvs[0] as usize;
        bound_min[..sz].copy_from_slice(&mesh_points[p0 * str_..p0 * str_ + sz]);
        bound_max[..sz].copy_from_slice(&mesh_points[p0 * str_..p0 * str_ + sz]);

        for i in 1..nc {
            let pi = cvs[i] as usize;
            let p = &mesh_points[pi * str_..pi * str_ + sz];
            for j in 0..sz {
                if p[j] < bound_min[j] {
                    bound_min[j] = p[j];
                }
                if p[j] > bound_max[j] {
                    bound_max[j] = p[j];
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Internal: multi-linear derivative evaluation
    // -----------------------------------------------------------------------

    /// Evaluate N-sided linear (quadrangulated) patch derivatives directly.
    fn eval_multi_linear_derivs_impl(
        &self,
        uv: [REAL; 2],
        patch_points: &[REAL],
        desc: PointDescriptor,
        p: Option<&mut [REAL]>,
        du: Option<&mut [REAL]>,
        dv: Option<&mut [REAL]>,
        duu: Option<&mut [REAL]>,
        duv: Option<&mut [REAL]>,
        dvv: Option<&mut [REAL]>,
    ) where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        let sz = desc.size as usize;
        let str_ = desc.stride as usize;
        let n = self.get_num_control_points() as usize;
        let num_derivs = if du.is_some() && dv.is_some() {
            if duu.is_some() && duv.is_some() && dvv.is_some() {
                6
            } else {
                3
            }
        } else {
            1
        };

        // Evaluate linear quad basis on sub-face.
        let w = eval_basis_multi_linear_f32(self, uv, num_derivs);
        let sub_quad = w.sub_face;

        // Identify the 4 patch points for this sub-quad.
        let qi = [
            sub_quad as usize,
            n + 1 + sub_quad as usize,
            n,
            n + 1 + (sub_quad as usize + n - 1) % n,
        ];

        // Helper: weighted sum of 4 points into output.
        let combine = |out: &mut [REAL], weights: &[f32; 4]| {
            for j in 0..sz {
                out[j] = REAL::default();
            }
            for (wi, &pi) in qi.iter().enumerate() {
                let wt = real_from_f32::<REAL>(weights[wi]);
                let src = &patch_points[pi * str_..pi * str_ + sz];
                for j in 0..sz {
                    out[j] = out[j] + wt * src[j];
                }
            }
        };

        if let Some(out) = p {
            combine(out, &w.wp);
        }
        if let Some(out) = du {
            combine(out, &w.wdu);
        }
        if let Some(out) = dv {
            combine(out, &w.wdv);
        }
        if let Some(out) = duu {
            combine(out, &w.wduu);
        }
        if let Some(out) = duv {
            combine(out, &w.wduv);
        }
        if let Some(out) = dvv {
            combine(out, &w.wdvv);
        }
    }

    // -----------------------------------------------------------------------
    //  Internal: stencils dispatcher
    // -----------------------------------------------------------------------

    fn evaluate_stencils_dispatch(
        &self,
        uv: [REAL; 2],
        sp: &mut [REAL],
        sdu: Option<&mut [REAL]>,
        sdv: Option<&mut [REAL]>,
        sduu: Option<&mut [REAL]>,
        sduv: Option<&mut [REAL]>,
        sdvv: Option<&mut [REAL]>,
    ) -> i32
    where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        if self.is_regular() {
            self.eval_regular_stencils_impl(uv, sp, sdu, sdv, sduu, sduv, sdvv)
        } else if self.is_linear() {
            self.eval_multi_linear_stencils_impl(uv, sp, sdu, sdv, sduu, sduv, sdvv)
        } else {
            self.eval_irregular_stencils_impl(uv, sp, sdu, sdv, sduu, sduv, sdvv)
        }
    }

    /// Regular patch stencils: basis weights == stencil weights.
    fn eval_regular_stencils_impl(
        &self,
        uv: [REAL; 2],
        sp: &mut [REAL],
        sdu: Option<&mut [REAL]>,
        sdv: Option<&mut [REAL]>,
        sduu: Option<&mut [REAL]>,
        sduv: Option<&mut [REAL]>,
        sdvv: Option<&mut [REAL]>,
    ) -> i32
    where
        REAL: num_traits::Float + From<f32> + Default,
    {
        let nc = self.get_num_control_points() as usize;
        let num_derivs = deriv_count(
            sdu.is_some(),
            sdv.is_some(),
            sduu.is_some(),
            sduv.is_some(),
            sdvv.is_some(),
        );

        let w = eval_basis_regular_f32(self, uv, num_derivs);
        for i in 0..nc {
            sp[i] = real_from_f32(w[0][i]);
        }
        if let Some(s) = sdu {
            for i in 0..nc {
                s[i] = real_from_f32(w[1][i]);
            }
        }
        if let Some(s) = sdv {
            for i in 0..nc {
                s[i] = real_from_f32(w[2][i]);
            }
        }
        if let Some(s) = sduu {
            for i in 0..nc {
                s[i] = real_from_f32(w[3][i]);
            }
        }
        if let Some(s) = sduv {
            for i in 0..nc {
                s[i] = real_from_f32(w[4][i]);
            }
        }
        if let Some(s) = sdvv {
            for i in 0..nc {
                s[i] = real_from_f32(w[5][i]);
            }
        }
        nc as i32
    }

    /// Irregular patch stencils via PatchTree.
    fn eval_irregular_stencils_impl(
        &self,
        uv: [REAL; 2],
        sp: &mut [REAL],
        sdu: Option<&mut [REAL]>,
        sdv: Option<&mut [REAL]>,
        sduu: Option<&mut [REAL]>,
        sduv: Option<&mut [REAL]>,
        sdvv: Option<&mut [REAL]>,
    ) -> i32
    where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        let param = self.get_parameterization();
        let (sub_face, sub_uv) = if param.has_sub_faces() {
            param.convert_coord_to_normalized_sub_face::<f64>([
                uv[0].to_f64().unwrap(),
                uv[1].to_f64().unwrap(),
            ])
        } else {
            (0, [uv[0].to_f64().unwrap(), uv[1].to_f64().unwrap()])
        };

        let irreg = self.get_irreg_patch();
        let sub_patch = irreg.find_sub_patch(sub_uv[0], sub_uv[1], sub_face, -1);
        debug_assert!(sub_patch >= 0);

        let d1 = sdu.is_some() && sdv.is_some();
        let d2 = d1 && sduu.is_some() && sduv.is_some() && sdvv.is_some();
        let nc = irreg.get_num_control_points() as usize;
        let u = sub_uv[0] as f32;
        let v = sub_uv[1] as f32;

        let mut sp_tmp = vec![0.0f32; nc];
        let mut sdu_tmp = if d1 { vec![0.0f32; nc] } else { vec![] };
        let mut sdv_tmp = if d1 { vec![0.0f32; nc] } else { vec![] };
        let mut sduu_tmp = if d2 { vec![0.0f32; nc] } else { vec![] };
        let mut sduv_tmp = if d2 { vec![0.0f32; nc] } else { vec![] };
        let mut sdvv_tmp = if d2 { vec![0.0f32; nc] } else { vec![] };

        irreg.eval_sub_patch_stencils_f32(
            sub_patch,
            u,
            v,
            &mut sp_tmp,
            if d1 { Some(&mut sdu_tmp) } else { None },
            if d1 { Some(&mut sdv_tmp) } else { None },
            if d2 { Some(&mut sduu_tmp) } else { None },
            if d2 { Some(&mut sduv_tmp) } else { None },
            if d2 { Some(&mut sdvv_tmp) } else { None },
        );

        for i in 0..nc {
            sp[i] = real_from_f32(sp_tmp[i]);
        }
        if let Some(s) = sdu {
            for i in 0..nc {
                s[i] = real_from_f32(sdu_tmp[i]);
            }
        }
        if let Some(s) = sdv {
            for i in 0..nc {
                s[i] = real_from_f32(sdv_tmp[i]);
            }
        }
        if let Some(s) = sduu {
            for i in 0..nc {
                s[i] = real_from_f32(sduu_tmp[i]);
            }
        }
        if let Some(s) = sduv {
            for i in 0..nc {
                s[i] = real_from_f32(sduv_tmp[i]);
            }
        }
        if let Some(s) = sdvv {
            for i in 0..nc {
                s[i] = real_from_f32(sdvv_tmp[i]);
            }
        }

        irreg.get_num_control_points()
    }

    /// Multi-linear (N-sided) stencils.
    /// Mirrors `Surface::evalMultiLinearStencils`.
    fn eval_multi_linear_stencils_impl(
        &self,
        uv: [REAL; 2],
        sp: &mut [REAL],
        mut sdu: Option<&mut [REAL]>,
        mut sdv: Option<&mut [REAL]>,
        mut sduu: Option<&mut [REAL]>,
        mut sduv: Option<&mut [REAL]>,
        mut sdvv: Option<&mut [REAL]>,
    ) -> i32
    where
        REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default,
    {
        let nc = self.get_num_control_points() as usize;
        let num_derivs = deriv_count(
            sdu.is_some(),
            sdv.is_some(),
            sduu.is_some(),
            sduv.is_some(),
            sdvv.is_some(),
        );

        let mut w = eval_basis_multi_linear_f32(self, uv, num_derivs);
        let i_origin = w.sub_face as usize;

        // Transform bilinear weights to stencil weights.
        transform_linear_quad_to_stencil(&mut w.wp, nc);
        if num_derivs > 1 {
            transform_linear_quad_to_stencil(&mut w.wdu, nc);
            transform_linear_quad_to_stencil(&mut w.wdv, nc);
            if num_derivs > 3 {
                transform_linear_quad_to_stencil(&mut w.wduv, nc);
            }
        }

        let i_next = (i_origin + 1) % nc;
        let i_prev = (i_origin + nc - 1) % nc;

        for i in 0..nc {
            let wi = if i == i_origin {
                0
            } else if i == i_next {
                1
            } else if i == i_prev {
                3
            } else {
                2
            };

            sp[i] = real_from_f32(w.wp[wi]);
            if let Some(ref mut s) = sdu {
                s[i] = real_from_f32(w.wdu[wi]);
            }
            if let Some(ref mut s) = sdv {
                s[i] = real_from_f32(w.wdv[wi]);
            }
            if num_derivs > 3 {
                if let Some(ref mut s) = sduu {
                    s[i] = real_from_f32(0.0);
                }
                if let Some(ref mut s) = sduv {
                    s[i] = real_from_f32(w.wduv[wi]);
                }
                if let Some(ref mut s) = sdvv {
                    s[i] = real_from_f32(0.0);
                }
            }
        }
        nc as i32
    }

    // -----------------------------------------------------------------------
    //  Internal helpers
    // -----------------------------------------------------------------------

    /// Access the IrregularPatchType (panics if surface is regular/linear).
    fn get_irreg_patch(&self) -> IrregularPatchSharedPtr {
        self.data
            .get_irreg_patch_ptr()
            .expect("Surface: no irregular patch present")
    }

    fn compute_linear_patch_points(&self, points: &mut [REAL], desc: PointDescriptor)
    where
        REAL: std::ops::Add<Output = REAL>
            + std::ops::Mul<Output = REAL>
            + std::ops::Div<Output = REAL>
            + Default
            + Copy
            + From<f32>,
    {
        // For an N-sided face the additional points are:
        //   [N]   = face midpoint  (avg of N control pts)
        //   [N+1..N+N] = edge midpoints  (avg of consecutive pairs)
        //
        // Mirrors C++ SplitFace<REAL>::Apply.
        let n = self.get_num_control_points() as usize;
        let sz = desc.size as usize;
        let str_ = desc.stride as usize;
        let rn: REAL = <REAL as From<f32>>::from(n as f32);

        // Face midpoint (index N): accumulate control points then divide.
        let face_off = n * str_;
        for j in 0..sz {
            points[face_off + j] = REAL::default();
        }
        for i in 0..n {
            // Use direct index arithmetic to avoid borrow-conflict with face_off slice.
            for j in 0..sz {
                let v = points[i * str_ + j];
                points[face_off + j] = points[face_off + j] + v;
            }
        }
        for j in 0..sz {
            let v = points[face_off + j];
            points[face_off + j] = v / rn;
        }

        // Edge midpoints (indices N+1..N+N): avg of consecutive vertex pairs.
        let half: REAL = <REAL as From<f32>>::from(0.5f32);
        for e in 0..n {
            let a = e;
            let b = (e + 1) % n;
            let edge_off = (n + 1 + e) * str_;
            for j in 0..sz {
                let pa = points[a * str_ + j];
                let pb = points[b * str_ + j];
                points[edge_off + j] = (pa + pb) * half;
            }
        }
    }

    fn compute_irregular_patch_points(&self, points: &mut [REAL], desc: PointDescriptor)
    where
        REAL: std::ops::Mul<Output = REAL>
            + std::ops::Add<Output = REAL>
            + Default
            + Copy
            + From<f32>,
    {
        // Mirrors C++ CombineConsecutive<REAL>::Apply applied to the stencil matrix.
        let irreg = self.get_irreg_patch();
        let nc = self.get_num_control_points() as usize;
        let np = irreg.get_num_points_total() as usize;
        if np == nc {
            return;
        }

        let sz = desc.size as usize;
        let str_ = desc.stride as usize;
        let extra = np - nc;

        // The stencil matrix is borrowed directly — no allocation needed.
        let matrix = irreg.get_stencil_matrix_f32();

        for r in 0..extra {
            let row = &matrix[r * nc..(r + 1) * nc];
            let dst_off = (nc + r) * str_;
            for j in 0..sz {
                points[dst_off + j] = REAL::default();
            }
            for (i, &w_f32) in row.iter().enumerate() {
                let w: REAL = <REAL as From<f32>>::from(w_f32);
                // Read control point components via index arithmetic to avoid
                // borrow conflict with the dst_off slice.
                for j in 0..sz {
                    let v = points[i * str_ + j];
                    points[dst_off + j] = points[dst_off + j] + w * v;
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    //  Access to SurfaceData (package-private, for SurfaceFactory)
    // -----------------------------------------------------------------------

    /// Immutable access for inspection / testing (also used in test code).
    #[allow(dead_code)]
    pub(crate) fn get_surface_data(&self) -> &SurfaceData {
        &self.data
    }
    pub(crate) fn get_surface_data_mut(&mut self) -> &mut SurfaceData {
        &mut self.data
    }
}

// ---------------------------------------------------------------------------
//  Free-standing basis evaluation helpers (avoid borrow issues with arrays)
// ---------------------------------------------------------------------------

/// Convert f32 to REAL using the From<f32> trait explicitly.
#[inline]
fn real_from_f32<REAL: From<f32>>(v: f32) -> REAL {
    <REAL as From<f32>>::from(v)
}

/// Compute derivative count from option presence.
fn deriv_count(has_du: bool, has_dv: bool, has_duu: bool, has_duv: bool, has_dvv: bool) -> usize {
    if has_du && has_dv {
        if has_duu && has_duv && has_dvv { 6 } else { 3 }
    } else {
        1
    }
}

/// Evaluate regular patch basis into 6 separate f32 buffers.
fn eval_basis_regular_f32<REAL: SurfaceReal + num_traits::Float>(
    surf: &Surface<REAL>,
    uv: [REAL; 2],
    num_derivs: usize,
) -> [[f32; 20]; 6] {
    use crate::far::patch_basis::evaluate_patch_basis_normalized;
    use crate::osd::patch_basis::OsdPatchParam;

    // Encode reg_patch_mask into PatchParam field1.
    let mask = surf.data.get_reg_patch_mask();
    let pp = OsdPatchParam::new(0, (mask as i32) << 4, 0.0);
    let patch_type_id = surf.data.get_reg_patch_type() as i32;
    let s = uv[0].to_f32().unwrap();
    let t = uv[1].to_f32().unwrap();

    let mut w0 = [0.0f32; 20];
    let mut w1 = [0.0f32; 20];
    let mut w2 = [0.0f32; 20];
    let mut w3 = [0.0f32; 20];
    let mut w4 = [0.0f32; 20];
    let mut w5 = [0.0f32; 20];

    evaluate_patch_basis_normalized(
        patch_type_id,
        &pp,
        s,
        t,
        &mut w0,
        if num_derivs > 1 { Some(&mut w1) } else { None },
        if num_derivs > 1 { Some(&mut w2) } else { None },
        if num_derivs > 3 { Some(&mut w3) } else { None },
        if num_derivs > 3 { Some(&mut w4) } else { None },
        if num_derivs > 3 { Some(&mut w5) } else { None },
    );

    [w0, w1, w2, w3, w4, w5]
}

/// Evaluate irregular patch basis. Returns (weights, point_indices).
fn eval_basis_irregular_f32<REAL: SurfaceReal + num_traits::Float>(
    surf: &Surface<REAL>,
    uv: [REAL; 2],
    num_derivs: usize,
) -> ([[f32; 20]; 6], Vec<i32>) {
    let param = surf.get_parameterization();
    let (sub_face, sub_uv) = if param.has_sub_faces() {
        param.convert_coord_to_normalized_sub_face::<f64>([
            uv[0].to_f64().unwrap(),
            uv[1].to_f64().unwrap(),
        ])
    } else {
        (0, [uv[0].to_f64().unwrap(), uv[1].to_f64().unwrap()])
    };

    let irreg = surf.get_irreg_patch();
    let sub_patch = irreg.find_sub_patch(sub_uv[0], sub_uv[1], sub_face, -1);
    debug_assert!(sub_patch >= 0);

    let mut w0 = [0.0f32; 20];
    let mut w1 = [0.0f32; 20];
    let mut w2 = [0.0f32; 20];
    let mut w3 = [0.0f32; 20];
    let mut w4 = [0.0f32; 20];
    let mut w5 = [0.0f32; 20];

    irreg.eval_sub_patch_basis_f32(
        sub_patch,
        sub_uv[0] as f32,
        sub_uv[1] as f32,
        Some(&mut w0),
        if num_derivs > 1 { Some(&mut w1) } else { None },
        if num_derivs > 1 { Some(&mut w2) } else { None },
        if num_derivs > 3 { Some(&mut w3) } else { None },
        if num_derivs > 3 { Some(&mut w4) } else { None },
        if num_derivs > 3 { Some(&mut w5) } else { None },
    );

    let indices = irreg.get_sub_patch_points(sub_patch).to_vec();
    ([w0, w1, w2, w3, w4, w5], indices)
}

/// Multi-linear basis weights: (w_P, w_Du, w_Dv, w_Duu, w_Duv, w_Dvv, sub_face).
struct MultiLinearBasis {
    wp: [f32; 4],
    wdu: [f32; 4],
    wdv: [f32; 4],
    wduu: [f32; 4],
    wduv: [f32; 4],
    wdvv: [f32; 4],
    sub_face: i32,
}

/// Evaluate multi-linear (N-sided quadrangulated) basis.
fn eval_basis_multi_linear_f32<REAL: SurfaceReal + num_traits::Float>(
    surf: &Surface<REAL>,
    uv: [REAL; 2],
    num_derivs: usize,
) -> MultiLinearBasis {
    use crate::far::patch_basis::evaluate_patch_basis_normalized;
    use crate::osd::patch_basis::OsdPatchParam;
    use crate::osd::patch_basis::patch_param::patch_type;

    let param = surf.get_parameterization();
    let uv64 = [uv[0].to_f64().unwrap(), uv[1].to_f64().unwrap()];
    let (sub_face, sub_uv) = param.convert_coord_to_normalized_sub_face::<f64>(uv64);

    let pp = OsdPatchParam::new(0, 0, 0.0);
    let s = sub_uv[0] as f32;
    let t = sub_uv[1] as f32;

    let mut w0 = [0.0f32; 20];
    let mut w1 = [0.0f32; 20];
    let mut w2 = [0.0f32; 20];
    let mut w3 = [0.0f32; 20];
    let mut w4 = [0.0f32; 20];
    let mut w5 = [0.0f32; 20];

    evaluate_patch_basis_normalized(
        patch_type::QUADS,
        &pp,
        s,
        t,
        &mut w0,
        if num_derivs > 1 { Some(&mut w1) } else { None },
        if num_derivs > 1 { Some(&mut w2) } else { None },
        if num_derivs > 3 { Some(&mut w3) } else { None },
        if num_derivs > 3 { Some(&mut w4) } else { None },
        if num_derivs > 3 { Some(&mut w5) } else { None },
    );

    let mut r = MultiLinearBasis {
        wp: [0.0; 4],
        wdu: [0.0; 4],
        wdv: [0.0; 4],
        wduu: [0.0; 4],
        wduv: [0.0; 4],
        wdvv: [0.0; 4],
        sub_face,
    };

    r.wp.copy_from_slice(&w0[..4]);
    r.wdu.copy_from_slice(&w1[..4]);
    r.wdv.copy_from_slice(&w2[..4]);
    r.wduu.copy_from_slice(&w3[..4]);
    r.wduv.copy_from_slice(&w4[..4]);
    r.wdvv.copy_from_slice(&w5[..4]);

    // Scale derivatives: 1st *= 2, mixed 2nd *= 4.
    if num_derivs > 1 {
        for v in r.wdu.iter_mut() {
            *v *= 2.0;
        }
        for v in r.wdv.iter_mut() {
            *v *= 2.0;
        }
    }
    if num_derivs > 3 {
        for v in r.wduv.iter_mut() {
            *v *= 4.0;
        }
    }

    r
}

/// Apply basis weights to patch points, writing weighted sum into `out`.
fn apply_weighted_sum<REAL>(
    out: &mut [REAL],
    desc: PointDescriptor,
    patch_points: &[REAL],
    weights: &[f32],
    num_pts: usize,
    indices: Option<&[i32]>,
) where
    REAL: num_traits::Float + From<f32> + std::ops::AddAssign + Default + Copy,
{
    let sz = desc.size as usize;
    let stride = desc.stride as usize;

    for j in 0..sz {
        out[j] = REAL::default();
    }
    for wi in 0..num_pts {
        let pi = if let Some(idx) = indices {
            idx[wi] as usize
        } else {
            wi
        };
        let wt: REAL = real_from_f32(weights[wi]);
        let src = &patch_points[pi * stride..pi * stride + sz];
        for j in 0..sz {
            out[j] = out[j] + wt * src[j];
        }
    }
}

/// Transform 4 bilinear weights into stencil weights for an N-sided face.
/// Mirrors C++ `transformLinearQuadWeightsToStencil`.
fn transform_linear_quad_to_stencil(w: &mut [f32; 4], n: usize) {
    let w_origin = w[0];
    let w_next = w[1] * 0.5;
    let w_center = w[2] / n as f32;
    let w_prev = w[3] * 0.5;

    w[0] = w_center + w_next + w_prev + w_origin;
    w[1] = w_center + w_next;
    w[2] = w_center;
    w[3] = w_center + w_prev;
}

// ---------------------------------------------------------------------------
//  SurfaceReal -- marker trait constraining REAL to f32 or f64
// ---------------------------------------------------------------------------

/// Marker trait for the floating-point precision parameter of `Surface`.
pub trait SurfaceReal: Copy + 'static {
    const IS_DOUBLE: bool;
}

impl SurfaceReal for f32 {
    const IS_DOUBLE: bool = false;
}
impl SurfaceReal for f64 {
    const IS_DOUBLE: bool = true;
}

// ---------------------------------------------------------------------------
//  Convenience type aliases
// ---------------------------------------------------------------------------

/// Single-precision Surface (the common case).
pub type SurfaceF32 = Surface<f32>;

/// Double-precision Surface.
pub type SurfaceF64 = Surface<f64>;

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_surface_is_invalid() {
        let s = SurfaceF32::new();
        assert!(!s.is_valid());
        assert!(!s.is_linear());
        // C++ SurfaceData is zero-initialised — is_regular starts false.
        // SurfaceFactory sets it to true only after initialising a regular patch.
        assert!(!s.is_regular());
    }

    #[test]
    fn double_precision_surface_is_double() {
        let s = SurfaceF64::new();
        assert!(s.get_surface_data().is_double());
    }

    #[test]
    fn clear_marks_invalid() {
        let mut s = SurfaceF32::new();
        s.data.set_valid(true);
        s.clear();
        assert!(!s.is_valid());
    }

    #[test]
    fn point_descriptor_stride_defaults_to_size() {
        let pd = PointDescriptor::new(3);
        assert_eq!(pd.size, 3);
        assert_eq!(pd.stride, 3);
    }

    #[test]
    fn gather_control_points_copies_correctly() {
        let mesh: Vec<f32> = vec![
            1.0, 0.0, 0.0, // vertex 0
            0.0, 1.0, 0.0, // vertex 1
            0.0, 0.0, 1.0, // vertex 2
            0.0, 0.0, 0.0, // vertex 3
        ];
        let desc = PointDescriptor::new(3);

        let mut s = SurfaceF32::new();
        s.data.resize_cvs(2);
        s.data.get_cv_indices_mut()[0] = 2;
        s.data.get_cv_indices_mut()[1] = 0;

        let mut cp = vec![0.0f32; 6];
        s.gather_control_points(&mesh, desc, &mut cp, desc);

        // cp[0..3] should be vertex 2 = (0, 0, 1)
        assert!((cp[0] - 0.0).abs() < 1e-6);
        assert!((cp[1] - 0.0).abs() < 1e-6);
        assert!((cp[2] - 1.0).abs() < 1e-6);
        // cp[3..6] should be vertex 0 = (1, 0, 0)
        assert!((cp[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn num_patch_points_regular() {
        let mut s = SurfaceF32::new();
        s.data.resize_cvs(16);
        s.data.set_valid(true);
        s.data.set_regular(true);
        assert_eq!(s.get_num_patch_points(), 16);
    }
}
