
//! HdField - Field buffer primitive base.
//!
//! Hydra schema for a USD field primitive. Acts like a texture, combined
//! with other fields to make up a renderable volume.
//! See pxr/imaging/hd/field.h for C++ reference.

use super::{HdBprim, HdRenderParam, HdSceneDelegate};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;

/// Field buffer primitive (OpenVDB, Field3D).
///
/// Base class for volume field data. Storm provides HdStField.
#[derive(Debug)]
pub struct HdField {
    /// Prim path
    id: SdfPath,

    /// Dirty bits
    dirty_bits: HdDirtyBits,
}

impl HdField {
    /// Field transform changed.
    pub const DIRTY_TRANSFORM: HdDirtyBits = 1 << 0;
    /// Field parameters changed.
    pub const DIRTY_PARAMS: HdDirtyBits = 1 << 1;
    /// All field dirty bits.
    pub const ALL_DIRTY: HdDirtyBits = Self::DIRTY_TRANSFORM | Self::DIRTY_PARAMS;

    /// Create a new field prim.
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: Self::ALL_DIRTY,
        }
    }
}

impl HdBprim for HdField {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn get_initial_dirty_bits_mask() -> HdDirtyBits {
        HdField::ALL_DIRTY
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}
