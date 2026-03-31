//! UsdReferences - API for editing reference paths.
//!
//! Port of pxr/usd/usd/references.h/cpp
//!
//! A proxy class for applying listOp edits to the reference paths list for a prim.

use crate::common::ListPosition;
use crate::prim::Prim;
use usd_sdf::list_op::ListOp;
use usd_sdf::{LayerOffset, Path, Reference};

/// A proxy class for applying listOp edits to the reference paths list for a prim.
///
/// Matches C++ `UsdReferences`.
///
/// References are the primary operator for "encapsulated aggregation" of scene description.
/// They allow building rich scenes by composing scene description recorded in different layers.
pub struct References {
    prim: Prim,
}

impl References {
    /// Creates a new References object for the given prim.
    ///
    /// Matches C++ `UsdReferences(const UsdPrim& prim)` (private constructor).
    pub(crate) fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Adds a reference to the reference listOp at the current EditTarget,
    /// in the position specified by position.
    ///
    /// Matches C++ `AddReference(const SdfReference& ref, UsdListPosition position)`.
    pub fn add_reference(&self, reference: &Reference, position: ListPosition) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        // Get or create the references metadata as a ReferenceListOp
        let references_token = usd_tf::Token::new("references");
        let mut list_op: ListOp<Reference> = self
            .prim
            .get_metadata(&references_token)
            .unwrap_or_default();

        // Add the reference based on position
        match position {
            ListPosition::FrontOfPrependList => {
                let mut prepended = list_op.get_prepended_items().to_vec();
                prepended.insert(0, reference.clone());
                if list_op.set_prepended_items(prepended).is_err() {
                    return false;
                }
            }
            ListPosition::BackOfPrependList => {
                let mut prepended = list_op.get_prepended_items().to_vec();
                prepended.push(reference.clone());
                if list_op.set_prepended_items(prepended).is_err() {
                    return false;
                }
            }
            ListPosition::FrontOfAppendList => {
                let mut appended = list_op.get_appended_items().to_vec();
                appended.insert(0, reference.clone());
                if list_op.set_appended_items(appended).is_err() {
                    return false;
                }
            }
            ListPosition::BackOfAppendList => {
                let mut appended = list_op.get_appended_items().to_vec();
                appended.push(reference.clone());
                if list_op.set_appended_items(appended).is_err() {
                    return false;
                }
            }
        }

        // Store as proper ListOp<Reference> so PCP can read it via get_reference_list_op
        let result = self
            .prim
            .set_metadata(&references_token, usd_vt::Value::from(list_op));

        // Invalidate PrimIndex cache so reference arc is picked up on recomposition
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.clear_prim_index_cache();
                }
                stage.handle_local_change(self.prim.path());
            }
        }

        result
    }

    /// Adds a reference with the given identifier, prim path, and layer offset.
    ///
    /// Matches C++ `AddReference(const std::string &identifier, const SdfPath &primPath, const SdfLayerOffset &layerOffset, UsdListPosition position)`.
    pub fn add_reference_with_path(
        &self,
        identifier: &str,
        prim_path: &Path,
        layer_offset: LayerOffset,
        position: ListPosition,
    ) -> bool {
        let reference = Reference::with_metadata(
            identifier,
            prim_path.get_string(),
            layer_offset,
            std::collections::HashMap::new(),
        );
        self.add_reference(&reference, position)
    }

    /// Adds a reference to the default prim in the given layer.
    ///
    /// Matches C++ `AddReference(const std::string &identifier, const SdfLayerOffset &layerOffset, UsdListPosition position)`.
    pub fn add_reference_to_default_prim(
        &self,
        identifier: &str,
        layer_offset: LayerOffset,
        position: ListPosition,
    ) -> bool {
        let reference = Reference::with_metadata(
            identifier,
            "",
            layer_offset,
            std::collections::HashMap::new(),
        );
        self.add_reference(&reference, position)
    }

    /// Add an internal reference to the specified prim.
    ///
    /// Matches C++ `AddInternalReference(const SdfPath &primPath, const SdfLayerOffset &layerOffset, UsdListPosition position)`.
    pub fn add_internal_reference(
        &self,
        prim_path: &Path,
        layer_offset: LayerOffset,
        position: ListPosition,
    ) -> bool {
        let reference = Reference::with_metadata(
            "",
            prim_path.get_string(),
            layer_offset,
            std::collections::HashMap::new(),
        );
        self.add_reference(&reference, position)
    }

    /// Removes the specified reference from the references listOp at the current EditTarget.
    ///
    /// Matches C++ `RemoveReference(const SdfReference& ref)`.
    pub fn remove_reference(&self, reference: &Reference) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let references_token = usd_tf::Token::new("references");
        let mut list_op: ListOp<Reference> = self
            .prim
            .get_metadata(&references_token)
            .unwrap_or_default();

        if list_op.is_explicit() {
            let mut explicit = list_op.get_explicit_items().to_vec();
            explicit.retain(|item| item != reference);
            if list_op.set_explicit_items(explicit).is_err() {
                return false;
            }
        } else {
            let mut added = list_op.get_added_items().to_vec();
            added.retain(|item| item != reference);
            list_op.set_added_items(added);

            let mut prepended = list_op.get_prepended_items().to_vec();
            prepended.retain(|item| item != reference);
            if list_op.set_prepended_items(prepended).is_err() {
                return false;
            }

            let mut appended = list_op.get_appended_items().to_vec();
            appended.retain(|item| item != reference);
            if list_op.set_appended_items(appended).is_err() {
                return false;
            }

            let mut deleted = list_op.get_deleted_items().to_vec();
            if !deleted.iter().any(|item| item == reference) {
                deleted.push(reference.clone());
            }
            if list_op.set_deleted_items(deleted).is_err() {
                return false;
            }
        }

        let result = self
            .prim
            .set_metadata(&references_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.clear_prim_index_cache();
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Removes the authored reference listOp edits at the current edit target.
    ///
    /// Matches C++ `ClearReferences()`.
    pub fn clear_references(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let references_token = usd_tf::Token::new("references");
        let result = self.prim.clear_metadata(&references_token);
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.clear_prim_index_cache();
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Explicitly set the references, potentially blocking weaker opinions.
    ///
    /// Matches C++ `SetReferences(const SdfReferenceVector& items)`.
    pub fn set_references(&self, items: Vec<Reference>) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let references_token = usd_tf::Token::new("references");
        let mut list_op = ListOp::new();
        if list_op.set_explicit_items(items).is_err() {
            return false;
        }

        let result = self
            .prim
            .set_metadata(&references_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.clear_prim_index_cache();
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Return the prim this object is bound to.
    ///
    /// Matches C++ `GetPrim()`.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }
}

impl std::ops::Deref for References {
    type Target = Prim;

    fn deref(&self) -> &Self::Target {
        &self.prim
    }
}

impl From<Prim> for References {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}
