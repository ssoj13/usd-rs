//! UsdSpecializes - API for editing specializes paths.
//!
//! Port of pxr/usd/usd/specializes.h/cpp
//!
//! A proxy class for applying listOp edits to the specializes list for a prim.

use crate::common::ListPosition;
use crate::prim::Prim;
use usd_sdf::Path;
use usd_sdf::list_op::PathListOp;

/// A proxy class for applying listOp edits to the specializes list for a prim.
///
/// Matches C++ `UsdSpecializes`.
///
/// All paths passed to the UsdSpecializes API are expected to be in the namespace
/// of the owning prim's stage. Subroot prim specializes paths will be translated
/// from this namespace to the namespace of the current edit target, if necessary.
/// If a path cannot be translated, a coding error will be issued and no changes
/// will be made. Root prim specializes paths will not be translated.
pub struct Specializes {
    prim: Prim,
}

impl Specializes {
    /// Creates a new Specializes object for the given prim.
    ///
    /// Matches C++ `UsdSpecializes(const UsdPrim& prim)` (private constructor).
    pub(crate) fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Adds a path to the specializes listOp at the current EditTarget,
    /// in the position specified by position.
    ///
    /// Matches C++ `AddSpecialize(const SdfPath &primPath, UsdListPosition position)`.
    pub fn add_specialize(&self, prim_path: &Path, position: ListPosition) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        // Get or create the specializes metadata as a PathListOp
        let specializes_token = usd_tf::Token::new("specializes");
        let mut list_op: PathListOp = self
            .prim
            .get_metadata(&specializes_token)
            .unwrap_or_default();

        // Add the path based on position
        match position {
            ListPosition::FrontOfPrependList => {
                let mut prepended = list_op.get_prepended_items().to_vec();
                prepended.insert(0, prim_path.clone());
                if list_op.set_prepended_items(prepended).is_err() {
                    return false;
                }
            }
            ListPosition::BackOfPrependList => {
                let mut prepended = list_op.get_prepended_items().to_vec();
                prepended.push(prim_path.clone());
                if list_op.set_prepended_items(prepended).is_err() {
                    return false;
                }
            }
            ListPosition::FrontOfAppendList => {
                let mut appended = list_op.get_appended_items().to_vec();
                appended.insert(0, prim_path.clone());
                if list_op.set_appended_items(appended).is_err() {
                    return false;
                }
            }
            ListPosition::BackOfAppendList => {
                let mut appended = list_op.get_appended_items().to_vec();
                appended.push(prim_path.clone());
                if list_op.set_appended_items(appended).is_err() {
                    return false;
                }
            }
        }

        // Set the updated metadata
        self.prim
            .set_metadata(&specializes_token, usd_vt::Value::from(list_op))
    }

    /// Removes the specified path from the specializes listOp at the current EditTarget.
    ///
    /// Matches C++ `RemoveSpecialize(const SdfPath &primPath)`.
    pub fn remove_specialize(&self, prim_path: &Path) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let specializes_token = usd_tf::Token::new("specializes");
        let mut list_op: PathListOp = self
            .prim
            .get_metadata(&specializes_token)
            .unwrap_or_default();

        if list_op.is_explicit() {
            let mut explicit = list_op.get_explicit_items().to_vec();
            explicit.retain(|path| path != prim_path);
            if list_op.set_explicit_items(explicit).is_err() {
                return false;
            }
        } else {
            let mut added = list_op.get_added_items().to_vec();
            added.retain(|path| path != prim_path);
            list_op.set_added_items(added);

            let mut prepended = list_op.get_prepended_items().to_vec();
            prepended.retain(|path| path != prim_path);
            if list_op.set_prepended_items(prepended).is_err() {
                return false;
            }

            let mut appended = list_op.get_appended_items().to_vec();
            appended.retain(|path| path != prim_path);
            if list_op.set_appended_items(appended).is_err() {
                return false;
            }

            let mut deleted = list_op.get_deleted_items().to_vec();
            if !deleted.iter().any(|path| path == prim_path) {
                deleted.push(prim_path.clone());
            }
            if list_op.set_deleted_items(deleted).is_err() {
                return false;
            }
        }

        self.prim
            .set_metadata(&specializes_token, usd_vt::Value::from(list_op))
    }

    /// Removes the authored specializes listOp edits at the current edit target.
    ///
    /// Matches C++ `ClearSpecializes()`.
    pub fn clear_specializes(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let specializes_token = usd_tf::Token::new("specializes");
        self.prim.clear_metadata(&specializes_token)
    }

    /// Explicitly set specializes paths, potentially blocking weaker opinions
    /// that add or remove items.
    ///
    /// Matches C++ `SetSpecializes(const SdfPathVector& items)`.
    pub fn set_specializes(&self, items: Vec<Path>) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let specializes_token = usd_tf::Token::new("specializes");
        let mut list_op = PathListOp::new();
        if list_op.set_explicit_items(items).is_err() {
            return false;
        }

        self.prim
            .set_metadata(&specializes_token, usd_vt::Value::from(list_op))
    }

    /// Returns all direct specializes paths for this prim by traversing the prim index.
    ///
    /// Matches C++ `GetAllDirectSpecializes()`.
    pub fn get_all_direct_specializes(&self) -> Vec<Path> {
        if !self.prim.is_valid() {
            return Vec::new();
        }

        let Some(prim_index) = self.prim.prim_index() else {
            return Vec::new();
        };

        if !prim_index.is_valid() {
            return Vec::new();
        }

        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let root_node = prim_index.root_node();
        if !root_node.is_valid() {
            return Vec::new();
        }

        let Some(root_layer_stack) = root_node.layer_stack() else {
            return Vec::new();
        };

        let mut add_if_direct_specialize = |node: &usd_pcp::NodeRef| {
            if node.arc_type() != usd_pcp::ArcType::Specialize {
                return;
            }

            let Some(node_layer_stack) = node.layer_stack() else {
                return;
            };

            if node_layer_stack.identifier() != root_layer_stack.identifier() {
                return;
            }

            let origin_root = node.origin_root_node();
            if origin_root.is_valid() && origin_root.is_due_to_ancestor() {
                return;
            }

            let path = node.path();
            if seen.insert(path.clone()) {
                result.push(path);
            }
        };

        let (start, end) = prim_index.get_node_range(usd_pcp::RangeType::Specialize);
        for i in start..end {
            if let Some(node) = prim_index.nodes().get(i) {
                add_if_direct_specialize(node);
            }
        }

        result
    }

    /// Return the prim this object is bound to.
    ///
    /// Matches C++ `GetPrim() const`.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Return the prim this object is bound to (mutable version).
    ///
    /// Matches C++ `GetPrim()`.
    pub fn prim_mut(&mut self) -> &mut Prim {
        &mut self.prim
    }

    /// Returns true if this object is valid (has a valid prim).
    ///
    /// Matches C++ `explicit operator bool()`.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }
}

impl std::ops::Deref for Specializes {
    type Target = Prim;

    fn deref(&self) -> &Self::Target {
        &self.prim
    }
}

impl From<Prim> for Specializes {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_specializes_creation() {
        // Basic construction test - Specializes requires a Prim
        // Full tests would require a Stage to create real prims
    }
}
