#![allow(dead_code)]

//! Field subtexture identifiers for volume data.
//!
//! Identifies specific grids within OpenVDB or Field3D files,
//! paralleling the usdVol schema types.
//!
//! Port of pxr/imaging/hdSt/fieldSubtextureIdentifier.h

use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Base data shared by all field subtexture identifiers.
#[derive(Debug, Clone)]
pub struct FieldBaseSubtextureIdentifier {
    /// Field/grid name within the file
    pub field_name: Token,
    /// Field/partition index
    pub field_index: i32,
}

impl FieldBaseSubtextureIdentifier {
    /// Create a new field base identifier.
    pub fn new(field_name: Token, field_index: i32) -> Self {
        Self {
            field_name,
            field_index,
        }
    }
}

impl PartialEq for FieldBaseSubtextureIdentifier {
    fn eq(&self, other: &Self) -> bool {
        self.field_name == other.field_name && self.field_index == other.field_index
    }
}

impl Eq for FieldBaseSubtextureIdentifier {}

impl Hash for FieldBaseSubtextureIdentifier {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.field_name, state);
        self.field_index.hash(state);
    }
}

// ---------------------------------------------------------------------------
// HdStOpenVDBAssetSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Identifies a grid in an OpenVDB file.
///
/// Parallels OpenVDBAsset in usdVol. The `field_name` corresponds
/// to the gridName in the OpenVDB file.
///
/// Port of HdStOpenVDBAssetSubtextureIdentifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OpenVDBAssetSubtextureId {
    /// Base field identification
    pub base: FieldBaseSubtextureIdentifier,
}

impl OpenVDBAssetSubtextureId {
    /// Create an OpenVDB subtexture identifier.
    ///
    /// `field_name` corresponds to the gridName in the OpenVDB file.
    pub fn new(field_name: Token, field_index: i32) -> Self {
        Self {
            base: FieldBaseSubtextureIdentifier::new(field_name, field_index),
        }
    }

    /// Get the field (grid) name.
    pub fn field_name(&self) -> &Token {
        &self.base.field_name
    }

    /// Get the field index.
    pub fn field_index(&self) -> i32 {
        self.base.field_index
    }
}

// ---------------------------------------------------------------------------
// HdStField3DAssetSubtextureIdentifier
// ---------------------------------------------------------------------------

/// Identifies a grid in a Field3D file.
///
/// Parallels Field3DAsset in usdVol. Uses field_name (layer/attribute name),
/// field_index (partition index), and field_purpose (partition name/grouping).
///
/// Port of HdStField3DAssetSubtextureIdentifier
#[derive(Debug, Clone)]
pub struct Field3DAssetSubtextureId {
    /// Base field identification
    pub base: FieldBaseSubtextureIdentifier,
    /// Partition name / grouping (e.g., "BigCloud")
    pub field_purpose: Token,
}

impl Field3DAssetSubtextureId {
    /// Create a Field3D subtexture identifier.
    ///
    /// - `field_name`: layer/attribute name (e.g., "density")
    /// - `field_index`: partition index
    /// - `field_purpose`: partition name/grouping (e.g., "BigCloud")
    pub fn new(field_name: Token, field_index: i32, field_purpose: Token) -> Self {
        Self {
            base: FieldBaseSubtextureIdentifier::new(field_name, field_index),
            field_purpose,
        }
    }

    /// Get the field (layer/attribute) name.
    pub fn field_name(&self) -> &Token {
        &self.base.field_name
    }

    /// Get the field (partition) index.
    pub fn field_index(&self) -> i32 {
        self.base.field_index
    }

    /// Get the field purpose (partition name/grouping).
    pub fn field_purpose(&self) -> &Token {
        &self.field_purpose
    }
}

impl PartialEq for Field3DAssetSubtextureId {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.field_purpose == other.field_purpose
    }
}

impl Eq for Field3DAssetSubtextureId {}

impl Hash for Field3DAssetSubtextureId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.base.hash(state);
        Hash::hash(&self.field_purpose, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_openvdb_id() {
        let id = OpenVDBAssetSubtextureId::new(Token::new("density"), 0);
        assert_eq!(id.field_name(), &Token::new("density"));
        assert_eq!(id.field_index(), 0);
    }

    #[test]
    fn test_field3d_id() {
        let id = Field3DAssetSubtextureId::new(Token::new("density"), 1, Token::new("BigCloud"));
        assert_eq!(id.field_name(), &Token::new("density"));
        assert_eq!(id.field_index(), 1);
        assert_eq!(id.field_purpose(), &Token::new("BigCloud"));
    }

    #[test]
    fn test_hash_dedup() {
        let mut set = HashSet::new();
        set.insert(OpenVDBAssetSubtextureId::new(Token::new("density"), 0));
        set.insert(OpenVDBAssetSubtextureId::new(Token::new("density"), 0));
        set.insert(OpenVDBAssetSubtextureId::new(Token::new("temperature"), 0));
        assert_eq!(set.len(), 2);
    }
}
