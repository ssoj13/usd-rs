//! HdCoordSys - Coordinate system state primitive.
//!
//! Represents a named coordinate system in Hydra. Coordinate systems may be
//! referred to by name from shader networks.
//!
//! Following the UsdShadeCoordSysAPI convention, the Hydra id establishes the
//! name, where the id is a namespaced property path of the form
//! `<.../prim.coordSys:NAME>`. `get_name()` retrieves the name.
//!
//! Each rprim may have bound coordinate systems retrieved via
//! `HdTokens->coordSysBindings`.
//!
//! The transform is the matrix from local space to world space.
//!
//! Port of pxr/imaging/hd/coordSys.h

use super::{HdSceneDelegate, HdSprim};
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Dirty bits for HdCoordSys change tracking.
pub struct HdCoordSysDirtyBits;

impl HdCoordSysDirtyBits {
    /// No changes.
    pub const CLEAN: HdDirtyBits = 0;
    /// Name changed.
    pub const DIRTY_NAME: HdDirtyBits = 1 << 0;
    /// Transform changed.
    pub const DIRTY_TRANSFORM: HdDirtyBits = 1 << 1;
    /// All bits.
    pub const ALL_DIRTY: HdDirtyBits = Self::DIRTY_TRANSFORM | Self::DIRTY_NAME;
}

/// Hydra coordinate system state primitive.
///
/// Represents a coordinate system as a Hydra sprim. The transform value
/// is the matrix from local space to world space (same interpretation as
/// rprim transforms).
///
/// Port of C++ `HdCoordSys`.
#[derive(Debug)]
pub struct HdCoordSys {
    /// Prim path identifier.
    id: SdfPath,
    /// Dirty bits for change tracking.
    dirty_bits: HdDirtyBits,
    /// Coordinate system name (extracted from id).
    name: Token,
}

impl HdCoordSys {
    /// Create a new coordinate system prim.
    pub fn new(id: SdfPath) -> Self {
        // Extract name from id path (last element after "coordSys:")
        let name = Self::extract_name(&id);
        Self {
            id,
            dirty_bits: HdCoordSysDirtyBits::ALL_DIRTY,
            name,
        }
    }

    /// Returns the name bound to this coordinate system.
    ///
    /// There may be multiple coordinate systems with the same name, but they
    /// must associate with disjoint sets of rprims.
    pub fn get_name(&self) -> &Token {
        &self.name
    }

    /// Extract coordinate system name from prim path.
    ///
    /// Handles C++ convention: strips ":binding" suffix if present, then
    /// strips "coordSys:" namespace prefix.
    ///
    /// Examples:
    /// - `/Prim.coordSys:myCoordSys` -> "myCoordSys"
    /// - `/Prim.coordSys:myCoordSys:binding` -> "myCoordSys"
    ///
    /// Matches C++ `_GetNameFromSdfPath` (coordSys.cpp:21-31).
    fn extract_name(id: &SdfPath) -> Token {
        let attr_name = id.get_name();

        // Strip ":binding" suffix if present.
        let name = if attr_name.ends_with(":binding") {
            // Take everything before the last ":" (the binding suffix).
            match attr_name.rfind(':') {
                Some(idx) => &attr_name[..idx],
                None => attr_name,
            }
        } else {
            attr_name
        };

        // Strip "coordSys:" namespace prefix.
        if let Some(stripped) = name.strip_prefix("coordSys:") {
            Token::new(stripped)
        } else {
            // Fallback: use the full name
            Token::new(name)
        }
    }
}

impl HdSprim for HdCoordSys {
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
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn crate::prim::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        if *dirty_bits & HdCoordSysDirtyBits::DIRTY_TRANSFORM != 0 {
            // Transform synced by render delegate via GetTransform.
        }

        if *dirty_bits & HdCoordSysDirtyBits::DIRTY_NAME != 0 {
            // Query delegate for name using coordSys schema key.
            // C++ uses: SdfPath::JoinIdentifier({HdCoordSysSchema::GetSchemaToken(), "name"})
            let key = Token::new("coordSys:name");
            let v_name = delegate.get(&self.id, &key);
            if let Some(name) = v_name.get::<Token>() {
                self.name = name.clone();
            } else if let Some(name_str) = v_name.get::<String>() {
                self.name = Token::new(name_str);
            } else {
                // Fallback: extract from path (for old scene delegates).
                self.name = Self::extract_name(&self.id);
            }
        }

        *dirty_bits = HdCoordSysDirtyBits::CLEAN;
        self.dirty_bits = HdCoordSysDirtyBits::CLEAN;
    }

    fn get_initial_dirty_bits_mask() -> HdDirtyBits
    where
        Self: Sized,
    {
        HdCoordSysDirtyBits::ALL_DIRTY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord_sys_creation() {
        let id = SdfPath::from_string("/World/Light.coordSys:worldCoords").unwrap();
        let cs = HdCoordSys::new(id);
        assert_eq!(cs.get_name().as_str(), "worldCoords");
        assert!(cs.is_dirty());
    }

    #[test]
    fn test_coord_sys_name_fallback() {
        let id = SdfPath::from_string("/World/MyCoordSys").unwrap();
        let cs = HdCoordSys::new(id);
        // Falls back to last path element
        assert_eq!(cs.get_name().as_str(), "MyCoordSys");
    }

    #[test]
    fn test_coord_sys_dirty_bits() {
        let id = SdfPath::from_string("/World/CS").unwrap();
        let mut cs = HdCoordSys::new(id);

        assert!(cs.is_dirty());
        cs.mark_clean(HdCoordSysDirtyBits::ALL_DIRTY);
        assert!(!cs.is_dirty());
    }
}
