//! SdfPseudoRootSpec - the pseudo-root spec of a layer.
//!
//! Port of pxr/usd/sdf/pseudoRootSpec.h
//!
//! Represents the pseudo-root of a layer's namespace hierarchy.
//! The pseudo-root is the parent of all root prims in a layer.

use crate::{Layer, LayerHandle, Path, PrimSpec, SpecType};
use std::sync::Arc;

/// Represents the pseudo-root of a layer's namespace hierarchy.
///
/// The pseudo-root is the implicit parent of all root prims in a layer.
/// It contains layer-level metadata and the list of root prims.
#[derive(Clone)]
pub struct PseudoRootSpec {
    /// Layer containing this spec.
    layer: LayerHandle,
}

impl std::fmt::Debug for PseudoRootSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PseudoRootSpec")
            .field("layer", &"<layer>")
            .finish()
    }
}

impl PseudoRootSpec {
    /// Creates a pseudo-root spec for a layer.
    pub fn new(layer: &Arc<Layer>) -> Self {
        Self {
            layer: LayerHandle::from_layer(layer),
        }
    }

    /// Returns the layer.
    pub fn layer(&self) -> &LayerHandle {
        &self.layer
    }

    /// Returns the path (always the absolute root).
    pub fn path(&self) -> Path {
        Path::absolute_root()
    }

    /// Returns the spec type.
    pub fn spec_type(&self) -> SpecType {
        SpecType::PseudoRoot
    }

    /// Returns the root prim children.
    pub fn root_prims(&self) -> Vec<PrimSpec> {
        if let Some(layer) = self.layer.upgrade() {
            layer.root_prims()
        } else {
            Vec::new()
        }
    }

    /// Returns the number of root prims.
    pub fn root_prim_count(&self) -> usize {
        if let Some(layer) = self.layer.upgrade() {
            layer.root_prims().len()
        } else {
            0
        }
    }

    /// Checks if a root prim exists.
    pub fn has_root_prim(&self, name: &str) -> bool {
        if let Some(layer) = self.layer.upgrade() {
            let path = Path::absolute_root().append_child(name);
            if let Some(p) = path {
                return layer.has_spec(&p);
            }
        }
        false
    }

    /// Gets a root prim by name.
    pub fn get_root_prim(&self, name: &str) -> Option<PrimSpec> {
        if let Some(layer) = self.layer.upgrade() {
            let path = Path::absolute_root().append_child(name)?;
            layer.get_prim_at_path(&path)
        } else {
            None
        }
    }

    /// Returns the default prim name.
    pub fn default_prim(&self) -> Option<usd_tf::Token> {
        self.layer.upgrade().map(|l| l.default_prim())
    }

    /// Sets the default prim name.
    pub fn set_default_prim(&self, name: &str) -> bool {
        if let Some(layer) = self.layer.upgrade() {
            layer.set_default_prim(&usd_tf::Token::from(name));
            true
        } else {
            false
        }
    }

    /// Returns the layer's comment.
    pub fn comment(&self) -> Option<String> {
        self.layer.upgrade().and_then(|l| {
            let c = l.comment();
            if c.is_empty() {
                None
            } else {
                Some(c.to_string())
            }
        })
    }

    /// Sets the layer's comment.
    pub fn set_comment(&self, comment: &str) -> bool {
        if let Some(layer) = self.layer.upgrade() {
            layer.set_comment(comment);
            true
        } else {
            false
        }
    }

    /// Returns the layer's documentation.
    pub fn documentation(&self) -> Option<String> {
        self.layer.upgrade().and_then(|l| {
            let d = l.documentation();
            if d.is_empty() {
                None
            } else {
                Some(d.to_string())
            }
        })
    }

    /// Sets the layer's documentation.
    pub fn set_documentation(&self, doc: &str) -> bool {
        if let Some(layer) = self.layer.upgrade() {
            layer.set_documentation(doc);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pseudo_root_path() {
        let layer = Layer::create_anonymous(Some("test"));
        let pseudo_root = PseudoRootSpec::new(&layer);

        assert_eq!(pseudo_root.path(), Path::absolute_root());
        assert_eq!(pseudo_root.spec_type(), SpecType::PseudoRoot);
    }

    #[test]
    fn test_root_prims() {
        let layer = Layer::create_anonymous(Some("test"));
        let pseudo_root = PseudoRootSpec::new(&layer);

        assert_eq!(pseudo_root.root_prim_count(), 0);
        assert!(pseudo_root.root_prims().is_empty());
    }
}
