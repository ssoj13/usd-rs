//! SdfVariantSpec - a single variant in a variant set.
//!
//! Port of pxr/usd/sdf/variantSpec.h
//!
//! Represents a single variant in a variant set. A variant contains a prim
//! which is the root prim of the variant.

use crate::{Layer, LayerHandle, Path, PrimSpec, SpecType, VariantSetSpec};
use std::sync::Arc;
use usd_tf::Token;

/// Represents a single variant in a variant set.
///
/// A variant contains a prim. This prim is the root prim of the variant.
/// VariantSpecs are conceptually value objects - to modify a variant spec,
/// you typically modify the prim spec it contains.
#[derive(Clone)]
pub struct VariantSpec {
    /// Layer containing this variant.
    layer: LayerHandle,
    /// Path to the variant.
    path: Path,
    /// Name of the variant.
    name: Token,
}

impl std::fmt::Debug for VariantSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VariantSpec")
            .field("path", &self.path)
            .field("name", &self.name)
            .finish()
    }
}

impl VariantSpec {
    /// Creates a new variant spec in the given variant set.
    pub fn new(owner: &VariantSetSpec, name: impl Into<String>) -> Option<Self> {
        let name_str = name.into();
        let name_token = Token::new(&name_str);

        // Build the variant path: {ownerPath}{variantSetName=variantName}
        let variant_path = owner
            .path()
            .append_variant_selection(owner.name().as_str(), &name_str)?;

        Some(Self {
            layer: owner.layer(),
            path: variant_path,
            name: name_token,
        })
    }

    /// Creates a variant spec from path and layer.
    pub fn from_path(layer: &Arc<Layer>, path: &Path) -> Option<Self> {
        let (_, variant_name) = path.get_variant_selection()?;
        Some(Self {
            layer: LayerHandle::from_layer(layer),
            path: path.clone(),
            name: Token::new(&variant_name),
        })
    }

    /// Returns the name of this variant.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the name of this variant as a token.
    pub fn name_token(&self) -> Token {
        self.name.clone()
    }

    /// Returns the path to this variant spec.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the layer handle.
    pub fn layer(&self) -> &LayerHandle {
        &self.layer
    }

    /// Returns the prim spec owned by this variant.
    pub fn prim_spec(&self) -> Option<PrimSpec> {
        self.layer.get_prim_at_path(&self.path)
    }

    /// Returns the spec type.
    pub fn spec_type(&self) -> SpecType {
        SpecType::Variant
    }
}

/// Convenience function to create a variant spec in a layer.
pub fn create_variant_in_layer(
    layer: &Arc<Layer>,
    prim_path: &Path,
    variant_set_name: &str,
    variant_name: &str,
) -> Option<VariantSpec> {
    use crate::Specifier;

    // Create the prim if it doesn't exist
    if !layer.has_spec(prim_path) {
        layer.create_prim_spec(prim_path, Specifier::Over, "");
    }

    // Get the prim spec
    let prim = layer.get_prim_at_path(prim_path)?;

    // Get or create the variant set
    let variant_set = match VariantSetSpec::new(&prim, variant_set_name) {
        Ok(vs) => vs,
        Err(_) => return None,
    };

    VariantSpec::new(&variant_set, variant_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Specifier;

    #[test]
    fn test_variant_spec_name() {
        let layer = Layer::create_anonymous(Some("test"));
        let prim_path = Path::from_string("/Model").unwrap();
        layer.create_prim_spec(&prim_path, Specifier::Def, "");

        if let Some(prim) = layer.get_prim_at_path(&prim_path) {
            if let Ok(variant_set) = VariantSetSpec::new(&prim, "lod") {
                if let Some(variant) = VariantSpec::new(&variant_set, "high") {
                    assert_eq!(variant.name(), "high");
                    assert_eq!(variant.name_token().as_str(), "high");
                }
            }
        }
    }

    #[test]
    fn test_create_variant_in_layer() {
        let layer = Layer::create_anonymous(Some("test"));
        let prim_path = Path::from_string("/Model").unwrap();

        let variant = create_variant_in_layer(&layer, &prim_path, "appearance", "red");

        if let Some(v) = variant {
            assert_eq!(v.name(), "red");
        }
    }
}
