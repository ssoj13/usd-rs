//! HdMaterial - Material state primitive.
//!
//! Represents a material/shader network in Hydra. Materials are:
//! - Shader networks connecting nodes
//! - Parameter values
//! - Texture bindings
//! - Material terminals (surface, displacement, volume)
//!
//! # Material Networks
//!
//! Materials are represented as networks of shader nodes with
//! connections between parameters. Common terminals:
//! - **surface**: Surface shading
//! - **displacement**: Displacement/normal mapping
//! - **volume**: Volume shading

use super::{HdRenderParam, HdSceneDelegate, HdSprim};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;

/// Material network terminal type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdMaterialTerminal {
    /// Surface shading output.
    Surface,

    /// Displacement shading output.
    Displacement,

    /// Volume shading output.
    Volume,
}

impl HdMaterialTerminal {
    /// Get token string for terminal.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Surface => "surface",
            Self::Displacement => "displacement",
            Self::Volume => "volume",
        }
    }
}

/// Simplified material network flags (legacy stub).
///
/// NOTE: For full material network types (HdMaterialNetworkV1,
/// HdMaterialNetwork2, etc.) see `material_network` module.
#[derive(Debug, Clone, Default)]
pub struct HdMaterialNetworkFlags {
    /// Material has surface shader.
    pub has_surface: bool,

    /// Material has displacement shader.
    pub has_displacement: bool,

    /// Material has volume shader.
    pub has_volume: bool,
}

impl HdMaterialNetworkFlags {
    /// Create empty flags.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Material state primitive.
///
/// Represents a material with its shader network and parameters.
#[derive(Debug)]
pub struct HdMaterial {
    /// Prim identifier.
    id: SdfPath,

    /// Current dirty bits.
    dirty_bits: HdDirtyBits,

    /// Material network flags.
    network: HdMaterialNetworkFlags,
}

impl HdMaterial {
    /// Create a new material.
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            network: HdMaterialNetworkFlags::new(),
        }
    }

    /// Get material network flags.
    pub fn get_network(&self) -> &HdMaterialNetworkFlags {
        &self.network
    }

    /// Set material network flags.
    pub fn set_network(&mut self, network: HdMaterialNetworkFlags) {
        self.network = network;
        self.mark_dirty(Self::DIRTY_PARAMS);
    }

    /// Check if material has surface shader.
    pub fn has_surface(&self) -> bool {
        self.network.has_surface
    }

    /// Check if material has displacement shader.
    pub fn has_displacement(&self) -> bool {
        self.network.has_displacement
    }

    /// Check if material has volume shader.
    pub fn has_volume(&self) -> bool {
        self.network.has_volume
    }
}

impl HdSprim for HdMaterial {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        if (*dirty_bits & Self::DIRTY_PARAMS) != 0 {
            // Query material network from delegate
            // self.network = delegate.get_material_network(self.get_id());
        }

        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_material_creation() {
        let id = SdfPath::from_string("/Material").unwrap();
        let material = HdMaterial::new(id.clone());

        assert_eq!(material.get_id(), &id);
        assert!(material.is_dirty());
    }

    #[test]
    fn test_material_network() {
        let mut material = HdMaterial::new(SdfPath::from_string("/Material").unwrap());

        assert!(!material.has_surface());

        let mut network = HdMaterialNetworkFlags::new();
        network.has_surface = true;
        network.has_displacement = true;

        material.set_network(network);

        assert!(material.has_surface());
        assert!(material.has_displacement());
        assert!(!material.has_volume());
    }

    #[test]
    fn test_material_terminals() {
        assert_eq!(HdMaterialTerminal::Surface.as_str(), "surface");
        assert_eq!(HdMaterialTerminal::Displacement.as_str(), "displacement");
        assert_eq!(HdMaterialTerminal::Volume.as_str(), "volume");
    }
}
