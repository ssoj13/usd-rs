//! RegularPatchBuilder — gathers control-vertex indices for a regular patch.
//!
//! Ported from OpenSubdiv bfr/regularPatchBuilder.h/.cpp.

use super::face_surface::FaceSurface;

pub type Index = super::face_surface::Index;

// ---------------------------------------------------------------------------
//  PatchType (Far::PatchDescriptor::Type equivalent)
// ---------------------------------------------------------------------------

/// Subset of `Far::PatchDescriptor::Type` used by RegularPatchBuilder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegularPatchType {
    /// B-Spline patch for Catmark/quad faces (16 control points).
    Regular,
    /// Box-Spline patch for Loop/tri faces (12 control points).
    Loop,
}

// ---------------------------------------------------------------------------
//  RegularPatchBuilder
// ---------------------------------------------------------------------------

/// Gathers the control-vertex indices for the single regular patch
/// representing a `FaceSurface`.
///
/// Mirrors `Bfr::RegularPatchBuilder`.
pub struct RegularPatchBuilder<'a> {
    surface: &'a FaceSurface<'a>,
    is_quad: bool,
    is_boundary: bool,
    boundary_mask: i32,
    patch_size: i32,
    patch_type: RegularPatchType,
}

impl<'a> RegularPatchBuilder<'a> {
    /// Construct from a fully-initialized `FaceSurface`.
    pub fn new(surface: &'a FaceSurface<'a>) -> Self {
        let is_quad = surface.get_face_size() == 4;
        let (patch_type, patch_size) = if is_quad {
            (RegularPatchType::Regular, 16)
        } else {
            (RegularPatchType::Loop, 12)
        };

        let is_boundary = surface.get_tag().has_boundary_vertices();
        let boundary_mask = if !is_boundary {
            0
        } else if is_quad {
            // One bit per boundary edge (leading edge from corner).
            let c = surface.get_subsets();
            (((c[0].is_boundary() && c[0].num_faces_before == 0) as i32) << 0)
                | (((c[1].is_boundary() && c[1].num_faces_before == 0) as i32) << 1)
                | (((c[2].is_boundary() && c[2].num_faces_before == 0) as i32) << 2)
                | (((c[3].is_boundary() && c[3].num_faces_before == 0) as i32) << 3)
        } else {
            // Triangle: encode edge bits and vertex bits together.
            let c = surface.get_subsets();
            let e_mask = (((c[0].is_boundary() && c[0].num_faces_before == 0) as i32) << 0)
                | (((c[1].is_boundary() && c[1].num_faces_before == 0) as i32) << 1)
                | (((c[2].is_boundary() && c[2].num_faces_before == 0) as i32) << 2);
            let v_mask = ((c[0].is_boundary() as i32) << 0)
                | ((c[1].is_boundary() as i32) << 1)
                | ((c[2].is_boundary() as i32) << 2);
            encode_tri_boundary_mask(e_mask, v_mask)
        };

        RegularPatchBuilder {
            surface,
            is_quad,
            is_boundary,
            boundary_mask,
            patch_size,
            patch_type,
        }
    }

    // -----------------------------------------------------------------------
    //  Accessors
    // -----------------------------------------------------------------------

    pub fn get_num_control_vertices(&self) -> i32 {
        self.patch_size
    }
    pub fn is_quad_patch(&self) -> bool {
        self.is_quad
    }
    pub fn is_boundary_patch(&self) -> bool {
        self.is_boundary
    }
    pub fn get_patch_type(&self) -> RegularPatchType {
        self.patch_type
    }
    pub fn get_patch_param_boundary_mask(&self) -> i32 {
        self.boundary_mask
    }

    // -----------------------------------------------------------------------
    //  Static helpers
    // -----------------------------------------------------------------------

    pub fn patch_size_for(reg_face_size: i32) -> i32 {
        if reg_face_size == 4 { 16 } else { 12 }
    }

    pub fn patch_type_for(reg_face_size: i32) -> RegularPatchType {
        if reg_face_size == 4 {
            RegularPatchType::Regular
        } else {
            RegularPatchType::Loop
        }
    }

    /// Compute boundary mask from the actual CV indices by checking for -1.
    pub fn boundary_mask_from_cvs(reg_face_size: i32, cvs: &[Index]) -> i32 {
        if reg_face_size == 4 {
            ((cvs[1] < 0) as i32) << 0
                | ((cvs[7] < 0) as i32) << 1
                | ((cvs[14] < 0) as i32) << 2
                | ((cvs[8] < 0) as i32) << 3
        } else {
            let e_mask = ((cvs[1] < 0) as i32) << 0
                | ((cvs[9] < 0) as i32) << 1
                | ((cvs[7] < 0) as i32) << 2;
            let v_mask = (((cvs[0] < 0) | (cvs[3] < 0)) as i32) << 0
                | (((cvs[2] < 0) | (cvs[6] < 0)) as i32) << 1
                | (((cvs[10] < 0) | (cvs[11] < 0)) as i32) << 2;
            encode_tri_boundary_mask(e_mask, v_mask)
        }
    }

    // -----------------------------------------------------------------------
    //  Gather CV indices
    // -----------------------------------------------------------------------

    /// Fill `cv_indices` with the control-vertex indices and return count.
    pub fn gather_control_vertex_indices(&self, cv_indices: &mut [Index]) -> i32 {
        if self.is_quad {
            if self.is_boundary {
                self.gather_boundary_patch_points4(cv_indices);
            } else {
                self.gather_interior_patch_points4(cv_indices);
            }
        } else {
            if self.is_boundary {
                self.gather_boundary_patch_points3(cv_indices);
            } else {
                self.gather_interior_patch_points3(cv_indices);
            }
        }
        self.patch_size
    }

    // -----------------------------------------------------------------------
    //  Interior quad patch (4x4 = 16 points)
    // -----------------------------------------------------------------------

    fn gather_interior_patch_points4(&self, p: &mut [Index]) {
        let indices = self.surface.get_indices();
        let mut fv_off = 0usize;

        let c0 = self.surface.get_corner_topology(0);
        let opp0 = fv_off + c0.get_face_index_offset(c0.get_face_after(2)) as usize;
        p[5] = indices[opp0 + 0];
        p[4] = indices[opp0 + 1];
        p[0] = indices[opp0 + 2];
        p[1] = indices[opp0 + 3];
        fv_off += c0.get_num_face_vertices() as usize;

        let c1 = self.surface.get_corner_topology(1);
        let opp1 = fv_off + c1.get_face_index_offset(c1.get_face_after(2)) as usize;
        p[6] = indices[opp1 + 0];
        p[2] = indices[opp1 + 1];
        p[3] = indices[opp1 + 2];
        p[7] = indices[opp1 + 3];
        fv_off += c1.get_num_face_vertices() as usize;

        let c2 = self.surface.get_corner_topology(2);
        let opp2 = fv_off + c2.get_face_index_offset(c2.get_face_after(2)) as usize;
        p[10] = indices[opp2 + 0];
        p[11] = indices[opp2 + 1];
        p[15] = indices[opp2 + 2];
        p[14] = indices[opp2 + 3];
        fv_off += c2.get_num_face_vertices() as usize;

        let c3 = self.surface.get_corner_topology(3);
        let opp3 = fv_off + c3.get_face_index_offset(c3.get_face_after(2)) as usize;
        p[9] = indices[opp3 + 0];
        p[13] = indices[opp3 + 1];
        p[12] = indices[opp3 + 2];
        p[8] = indices[opp3 + 3];
    }

    // -----------------------------------------------------------------------
    //  Boundary quad patch
    // -----------------------------------------------------------------------

    fn gather_boundary_patch_points4(&self, p: &mut [Index]) {
        let indices = self.surface.get_indices();
        let mut fv_off = 0usize;

        for i in 0..4usize {
            let c_top = self.surface.get_corner_topology(i);
            let c_sub = self.surface.get_corner_subset(i);

            let face_corner = c_top.get_face();
            let face_other = if !c_sub.is_boundary() {
                c_top.get_face_after(2)
            } else if c_sub.num_faces_after > 0 {
                c_top.get_face_next(face_corner)
            } else if c_sub.num_faces_before > 0 {
                c_top.get_face_previous(face_corner)
            } else {
                face_corner
            };

            let fv_other = fv_off + c_top.get_face_index_offset(face_other) as usize;
            let phantom = indices[fv_other];

            match i {
                0 => {
                    p[5] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[4] = indices[fv_other + 1];
                        p[0] = indices[fv_other + 2];
                        p[1] = indices[fv_other + 3];
                    } else {
                        p[4] = if c_sub.num_faces_after > 0 {
                            indices[fv_other + 3]
                        } else {
                            phantom
                        };
                        p[0] = phantom;
                        p[1] = if c_sub.num_faces_before > 0 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                1 => {
                    p[6] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[2] = indices[fv_other + 1];
                        p[3] = indices[fv_other + 2];
                        p[7] = indices[fv_other + 3];
                    } else {
                        p[2] = if c_sub.num_faces_after > 0 {
                            indices[fv_other + 3]
                        } else {
                            phantom
                        };
                        p[3] = phantom;
                        p[7] = if c_sub.num_faces_before > 0 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                2 => {
                    p[10] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[11] = indices[fv_other + 1];
                        p[15] = indices[fv_other + 2];
                        p[14] = indices[fv_other + 3];
                    } else {
                        p[11] = if c_sub.num_faces_after > 0 {
                            indices[fv_other + 3]
                        } else {
                            phantom
                        };
                        p[15] = phantom;
                        p[14] = if c_sub.num_faces_before > 0 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                3 => {
                    p[9] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[13] = indices[fv_other + 1];
                        p[12] = indices[fv_other + 2];
                        p[8] = indices[fv_other + 3];
                    } else {
                        p[13] = if c_sub.num_faces_after > 0 {
                            indices[fv_other + 3]
                        } else {
                            phantom
                        };
                        p[12] = phantom;
                        p[8] = if c_sub.num_faces_before > 0 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                _ => unreachable!(),
            }
            fv_off += c_top.get_num_face_vertices() as usize;
        }
    }

    // -----------------------------------------------------------------------
    //  Interior tri patch (12 points)
    // -----------------------------------------------------------------------

    fn gather_interior_patch_points3(&self, p: &mut [Index]) {
        let indices = self.surface.get_indices();
        let mut fv_off = 0usize;

        let c0 = self.surface.get_corner_topology(0);
        let n2_0 = fv_off + c0.get_face_index_offset(c0.get_face_after(2)) as usize;
        let n3_0 = fv_off + c0.get_face_index_offset(c0.get_face_after(3)) as usize;
        p[4] = indices[n2_0];
        p[7] = indices[n2_0 + 1];
        p[3] = indices[n2_0 + 2];
        p[0] = indices[n3_0 + 2];
        fv_off += c0.get_num_face_vertices() as usize;

        let c1 = self.surface.get_corner_topology(1);
        let n2_1 = fv_off + c1.get_face_index_offset(c1.get_face_after(2)) as usize;
        let n3_1 = fv_off + c1.get_face_index_offset(c1.get_face_after(3)) as usize;
        p[5] = indices[n2_1];
        p[1] = indices[n2_1 + 1];
        p[2] = indices[n2_1 + 2];
        p[6] = indices[n3_1 + 2];
        fv_off += c1.get_num_face_vertices() as usize;

        let c2 = self.surface.get_corner_topology(2);
        let n2_2 = fv_off + c2.get_face_index_offset(c2.get_face_after(2)) as usize;
        let n3_2 = fv_off + c2.get_face_index_offset(c2.get_face_after(3)) as usize;
        p[8] = indices[n2_2];
        p[9] = indices[n2_2 + 1];
        p[11] = indices[n2_2 + 2];
        p[10] = indices[n3_2 + 2];
    }

    // -----------------------------------------------------------------------
    //  Boundary tri patch
    // -----------------------------------------------------------------------

    fn gather_boundary_patch_points3(&self, p: &mut [Index]) {
        let indices = self.surface.get_indices();
        let mut fv_off = 0usize;

        for i in 0..3usize {
            let c_top = self.surface.get_corner_topology(i);
            let c_sub = self.surface.get_corner_subset(i);
            let face_corner = c_top.get_face();

            let face_other = if !c_sub.is_boundary() {
                c_top.get_face_after(2)
            } else if c_sub.num_faces_total == 1 {
                face_corner
            } else if c_sub.num_faces_before == 0 {
                c_top.get_face_after(2)
            } else if c_sub.num_faces_after == 0 {
                c_top.get_face_before(2)
            } else {
                c_top.get_face_next(face_corner)
            };

            let fv_other = fv_off + c_top.get_face_index_offset(face_other) as usize;
            let phantom = indices[fv_other];
            let na = c_sub.num_faces_after as usize;
            let nb = c_sub.num_faces_before as usize;

            match i {
                0 => {
                    p[4] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[7] = indices[fv_other + 1];
                        p[3] = indices[fv_other + 2];
                        let nxt = fv_off
                            + c_top.get_face_index_offset(c_top.get_face_next(face_other)) as usize;
                        p[0] = indices[nxt + 2];
                    } else {
                        p[7] = if na > 0 {
                            indices[fv_other + (3 - na)]
                        } else {
                            phantom
                        };
                        p[3] = if na == 2 {
                            indices[fv_other + 2]
                        } else {
                            phantom
                        };
                        p[0] = if nb == 2 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                1 => {
                    p[5] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[1] = indices[fv_other + 1];
                        p[2] = indices[fv_other + 2];
                        let nxt = fv_off
                            + c_top.get_face_index_offset(c_top.get_face_next(face_other)) as usize;
                        p[6] = indices[nxt + 2];
                    } else {
                        p[1] = if na > 0 {
                            indices[fv_other + (3 - na)]
                        } else {
                            phantom
                        };
                        p[2] = if na == 2 {
                            indices[fv_other + 2]
                        } else {
                            phantom
                        };
                        p[6] = if nb == 2 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                2 => {
                    p[8] = indices[fv_other];
                    if !c_sub.is_boundary() {
                        p[9] = indices[fv_other + 1];
                        p[11] = indices[fv_other + 2];
                        let nxt = fv_off
                            + c_top.get_face_index_offset(c_top.get_face_next(face_other)) as usize;
                        p[10] = indices[nxt + 2];
                    } else {
                        p[9] = if na > 0 {
                            indices[fv_other + (3 - na)]
                        } else {
                            phantom
                        };
                        p[11] = if na == 2 {
                            indices[fv_other + 2]
                        } else {
                            phantom
                        };
                        p[10] = if nb == 2 {
                            indices[fv_other + 1]
                        } else {
                            phantom
                        };
                    }
                }
                _ => unreachable!(),
            }
            fv_off += c_top.get_num_face_vertices() as usize;
        }
    }
}

// ---------------------------------------------------------------------------
//  Helper: encode triangle boundary mask
// ---------------------------------------------------------------------------

fn encode_tri_boundary_mask(e_bits: i32, v_bits: i32) -> i32 {
    let mut upper = 0i32;
    let mut lower = e_bits;

    if v_bits != 0 {
        if e_bits == 0 {
            upper = 1;
            lower = v_bits;
        } else if v_bits == 7 && (e_bits == 1 || e_bits == 2 || e_bits == 4) {
            upper = 2;
            lower = e_bits;
        }
    }
    (upper << 3) | lower
}

// ---------------------------------------------------------------------------
//  Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_tri_boundary_mask_no_v_bits() {
        assert_eq!(encode_tri_boundary_mask(3, 0), 3);
    }

    #[test]
    fn encode_tri_boundary_mask_only_v_bits() {
        // e=0, v=5 -> upper=1, lower=5
        assert_eq!(encode_tri_boundary_mask(0, 5), (1 << 3) | 5);
    }

    #[test]
    fn patch_size_static() {
        assert_eq!(RegularPatchBuilder::patch_size_for(4), 16);
        assert_eq!(RegularPatchBuilder::patch_size_for(3), 12);
    }
}
