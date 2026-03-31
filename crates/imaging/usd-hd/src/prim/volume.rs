
//! HdVolume - Renderable volume primitive.
//!
//! Port of pxr/imaging/hd/volume.h / volume.cpp.
//!
//! Represents a renderable volume prim in Hydra. Volumes reference
//! HdField prims via volume field descriptors to define their data.

use super::{HdRenderParam, HdRprim, HdSceneDelegate};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Renderable volume primitive.
///
/// Corresponds to C++ `HdVolume` (pxr/imaging/hd/volume.h).
/// Volumes bind to HdField prims that provide the actual field data
/// (e.g. OpenVDB grids).
#[derive(Debug)]
pub struct HdVolume {
    /// Prim identifier.
    id: SdfPath,
    /// Current dirty bits.
    dirty_bits: HdDirtyBits,
    /// Instancer id if instanced.
    instancer_id: Option<SdfPath>,
    /// Visibility state.
    visible: bool,
    /// Material id.
    material_id: Option<SdfPath>,
}

impl HdVolume {
    /// Create a new volume primitive.
    pub fn new(id: SdfPath, instancer_id: Option<SdfPath>) -> Self {
        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            instancer_id,
            visible: true,
            material_id: None,
        }
    }
}

impl HdRprim for HdVolume {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn get_instancer_id(&self) -> Option<&SdfPath> {
        self.instancer_id.as_ref()
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        _repr_token: &Token,
    ) {
        // Volume sync: query volume field descriptors, material, transform etc.
        // Backend-specific work is done in render delegate's volume prim.
        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn get_material_id(&self) -> Option<&SdfPath> {
        self.material_id.as_ref()
    }

    /// C++ HdVolume::GetBuiltinPrimvarNames returns empty vector.
    fn get_builtin_primvar_names() -> Vec<Token>
    where
        Self: Sized,
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_creation() {
        let id = SdfPath::from_string("/Volume").unwrap();
        let vol = HdVolume::new(id.clone(), None);
        assert_eq!(vol.get_id(), &id);
        assert!(vol.is_visible());
        assert!(vol.is_dirty());
    }

    #[test]
    fn test_volume_builtin_primvars() {
        let names = HdVolume::get_builtin_primvar_names();
        assert!(names.is_empty(), "C++ HdVolume returns empty primvar names");
    }
}
