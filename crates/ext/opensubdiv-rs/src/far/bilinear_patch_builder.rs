// Copyright 2017 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/bilinearPatchBuilder.h/.cpp

//! Bilinear (linear) patch builder — quad and triangle patches.
//!
//! Mirrors C++ `Far::BilinearPatchBuilder`.

use super::patch_builder::{BasisType, PatchBuilder, PatchBuilderOptions, SourcePatch};
use super::patch_descriptor::PatchType;
use super::sparse_matrix::SparseMatrix;
use super::topology_refiner::TopologyRefiner;
use crate::vtr::types::Index;

/// Bilinear patch builder.
///
/// For bilinear subdivision every face is either a quad or triangle patch.
pub struct BilinearPatchBuilder<'r> {
    pub base: PatchBuilder<'r>,
}

impl<'r> BilinearPatchBuilder<'r> {
    /// Construct a new bilinear patch builder.
    pub fn new(refiner: &'r TopologyRefiner, options: PatchBuilderOptions) -> Self {
        let mut base = PatchBuilder::create(refiner, options);
        // Bilinear: regular = Quads, irregular = Quads, native = Quads
        base.reg_patch_type = PatchType::Quads;
        base.irreg_patch_type = PatchType::Quads;
        base.native_patch_type = PatchType::Quads;
        base.linear_patch_type = PatchType::Quads;
        Self { base }
    }

    // ---- Virtual-method equivalents ----------------------------------------

    /// Map a `BasisType` to the patch type for bilinear.
    pub fn patch_type_from_basis(&self, basis: BasisType) -> PatchType {
        match basis {
            BasisType::Linear | BasisType::Regular => PatchType::Quads,
            _ => PatchType::Quads, // bilinear has no higher-order types
        }
    }

    /// Convert `source_patch` to `patch_type` (bilinear: identity matrix).
    pub fn convert_to_patch_type_f32(
        &self,
        source_patch: &SourcePatch,
        patch_type: PatchType,
        matrix: &mut SparseMatrix<f32>,
    ) -> i32 {
        // Bilinear: each output point = exactly one input point (identity)
        let n = source_patch.get_num_source_points();
        let nv = match patch_type {
            PatchType::Quads => 4,
            PatchType::Triangles => 3,
            _ => n,
        };
        matrix.resize(nv, n, nv);
        for i in 0..nv {
            matrix.set_row_size(i, 1);
            let cols = matrix.get_row_columns_mut(i);
            cols[0] = i;
            let elems = matrix.get_row_elements_mut(i);
            elems[0] = 1.0;
        }
        nv
    }

    /// f64 variant of `convert_to_patch_type`.
    pub fn convert_to_patch_type_f64(
        &self,
        source_patch: &SourcePatch,
        patch_type: PatchType,
        matrix: &mut SparseMatrix<f64>,
    ) -> i32 {
        let n = source_patch.get_num_source_points();
        let nv = match patch_type {
            PatchType::Quads => 4,
            PatchType::Triangles => 3,
            _ => n,
        };
        matrix.resize(nv, n, nv);
        for i in 0..nv {
            matrix.set_row_size(i, 1);
            let cols = matrix.get_row_columns_mut(i);
            cols[0] = i;
            let elems = matrix.get_row_elements_mut(i);
            elems[0] = 1.0;
        }
        nv
    }

    // ---- Face queries delegated to base ------------------------------------

    pub fn is_face_a_patch(&self, level: i32, face: Index) -> bool {
        self.base.is_face_a_patch(level, face)
    }
    pub fn is_face_a_leaf(&self, level: i32, face: Index) -> bool {
        self.base.is_face_a_leaf(level, face)
    }
    pub fn is_patch_regular(&self, level: i32, face: Index, fvc: i32) -> bool {
        self.base.is_patch_regular(level, face, fvc)
    }
    pub fn get_regular_patch_boundary_mask(&self, level: i32, face: Index, fvc: i32) -> i32 {
        self.base.get_regular_patch_boundary_mask(level, face, fvc)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::super::topology_refiner::TopologyRefiner;
    use super::*;
    use crate::sdc::{Options, types::SchemeType};

    #[test]
    fn bilinear_patch_types() {
        let refiner = TopologyRefiner::new(SchemeType::Bilinear, Options::default());
        let opts = PatchBuilderOptions::default();
        let pb = BilinearPatchBuilder::new(&refiner, opts);
        assert_eq!(pb.base.get_regular_patch_type(), PatchType::Quads);
    }

    #[test]
    fn bilinear_identity_matrix() {
        let refiner = TopologyRefiner::new(SchemeType::Bilinear, Options::default());
        let opts = PatchBuilderOptions::default();
        let pb = BilinearPatchBuilder::new(&refiner, opts);

        let mut sp = SourcePatch::new();
        for i in 0..4usize {
            sp.corners[i].num_faces = 4;
        }
        sp.finalize(4);

        let mut mat: SparseMatrix<f32> = SparseMatrix::new();
        let nv = pb.convert_to_patch_type_f32(&sp, PatchType::Quads, &mut mat);
        assert_eq!(nv, 4);
        assert_eq!(mat.get_num_rows(), 4);
        for i in 0..4 {
            assert_eq!(mat.get_row_size(i), 1);
            assert!((mat.get_row_elements(i)[0] - 1.0).abs() < 1e-6);
        }
    }
}
