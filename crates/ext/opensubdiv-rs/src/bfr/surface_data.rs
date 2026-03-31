//! SurfaceData — internal container for all Surface member variables.
//!
//! Mirrors `Bfr::internal::SurfaceData` from `surfaceData.h/cpp`.
//! Only accessible by `SurfaceFactory` (and builders).

use super::parameterization::Parameterization;
use super::irregular_patch_type::IrregularPatchSharedPtr;

/// Index type used by `SurfaceData` (same as `FaceVertex::Index`).
pub type Index = i32;

/// Internal data bag for a `Surface<R>`.
///
/// All member variables live here so that `SurfaceFactory` can initialise a
/// `Surface` without knowing its concrete precision type.
///
/// Mirrors `Bfr::internal::SurfaceData`.
#[derive(Clone, Debug)]
pub struct SurfaceData {
    /// Control-vertex index list.
    pub(crate) cv_indices:    Vec<Index>,

    /// Parameterization of the face.
    pub(crate) param:         Parameterization,

    pub(crate) is_valid:      bool,
    pub(crate) is_double:     bool,
    pub(crate) is_regular:    bool,
    pub(crate) is_linear:     bool,

    /// Patch type encoding (regular patches only).
    pub(crate) reg_patch_type: u8,
    /// Boundary mask for the regular patch.
    pub(crate) reg_patch_mask: u8,

    /// Shared reference to the irregular patch tree (`None` = regular/linear).
    pub(crate) irreg_patch:   Option<IrregularPatchSharedPtr>,
}

impl Default for SurfaceData {
    fn default() -> Self {
        SurfaceData {
            cv_indices:    Vec::new(),
            param:         Parameterization::default(),
            is_valid:      false,
            is_double:     false,
            // C++ zero-initialises SurfaceData via memset, so _isRegular starts false.
            // SurfaceFactory::initSurface sets it to true only for regular patches.
            is_regular:    false,
            is_linear:     false,
            reg_patch_type: 0,
            reg_patch_mask: 0,
            irreg_patch:   None,
        }
    }
}

impl SurfaceData {
    pub fn new() -> Self { Self::default() }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    #[inline] pub fn get_num_cvs(&self)    -> usize      { self.cv_indices.len() }
    #[inline] pub fn get_cv_indices(&self) -> &[Index]   { &self.cv_indices }
    #[inline] pub fn get_param(&self)      -> Parameterization { self.param }

    #[inline] pub fn is_valid(&self)    -> bool { self.is_valid }
    #[inline] pub fn is_double(&self)   -> bool { self.is_double }
    #[inline] pub fn is_regular(&self)  -> bool { self.is_regular }
    #[inline] pub fn is_linear(&self)   -> bool { self.is_linear }

    #[inline] pub fn get_reg_patch_type(&self) -> u8 { self.reg_patch_type }
    #[inline] pub fn get_reg_patch_mask(&self) -> u8 { self.reg_patch_mask }

    #[inline] pub fn has_irreg_patch(&self)    -> bool { self.irreg_patch.is_some() }
    #[inline] pub fn get_irreg_patch_ptr(&self) -> Option<IrregularPatchSharedPtr> {
        self.irreg_patch.clone()
    }

    // -----------------------------------------------------------------------
    // Mutators (used by SurfaceFactory)
    // -----------------------------------------------------------------------

    /// Mark as invalid and release the irregular patch.
    pub fn invalidate(&mut self) {
        self.irreg_patch = None;
        self.is_valid    = false;
    }

    /// Re-initialise only when currently valid.
    #[inline]
    pub fn reinitialize(&mut self) {
        if self.is_valid {
            self.invalidate();
        }
    }

    /// Return a mutable slice to the CV index buffer.
    #[inline]
    pub fn get_cv_indices_mut(&mut self) -> &mut [Index] { &mut self.cv_indices }

    /// Resize the CV index buffer and return a mutable reference to it.
    pub fn resize_cvs(&mut self, size: usize) -> &mut [Index] {
        self.cv_indices.resize(size, 0);
        &mut self.cv_indices
    }

    #[inline] pub fn set_param(&mut self, p: Parameterization) { self.param = p; }
    #[inline] pub fn set_valid(&mut self, on: bool)             { self.is_valid = on; }
    #[inline] pub fn set_double(&mut self, on: bool)            { self.is_double = on; }
    #[inline] pub fn set_regular(&mut self, on: bool)           { self.is_regular = on; }
    #[inline] pub fn set_linear(&mut self, on: bool)            { self.is_linear = on; }
    #[inline] pub fn set_reg_patch_type(&mut self, t: u8)       { self.reg_patch_type = t; }
    #[inline] pub fn set_reg_patch_mask(&mut self, m: u8)       { self.reg_patch_mask = m; }

    #[inline]
    pub fn set_irreg_patch_ptr(&mut self, ptr: Option<IrregularPatchSharedPtr>) {
        self.irreg_patch = ptr;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_invalid() {
        let sd = SurfaceData::new();
        assert!(!sd.is_valid());
        // C++ memset-initializes to zero, so is_regular starts false.
        assert!(!sd.is_regular());
        assert!(!sd.is_double());
        assert!(!sd.is_linear());
        assert_eq!(sd.get_num_cvs(), 0);
    }

    #[test]
    fn resize_cvs() {
        let mut sd = SurfaceData::new();
        sd.resize_cvs(16);
        assert_eq!(sd.get_num_cvs(), 16);
    }

    #[test]
    fn invalidate_clears_patch() {
        let mut sd = SurfaceData::new();
        sd.is_valid = true;
        sd.reinitialize();
        assert!(!sd.is_valid());
    }
}
