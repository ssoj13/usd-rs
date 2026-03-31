//! UsdInherits - API for editing inherit paths.
//!
//! Port of pxr/usd/usd/inherits.h/cpp
//!
//! A proxy class for applying listOp edits to the inherit paths list for a prim.

use crate::common::ListPosition;
use crate::prim::Prim;
use usd_sdf::Path;
use usd_sdf::list_op::PathListOp;

/// A proxy class for applying listOp edits to the inherit paths list for a prim.
///
/// Matches C++ `UsdInherits`.
///
/// All paths passed to the UsdInherits API are expected to be in the namespace
/// of the owning prim's stage. Subroot prim inherit paths will be translated
/// from this namespace to the namespace of the current edit target, if necessary.
pub struct Inherits {
    prim: Prim,
}

impl Inherits {
    /// Creates a new Inherits object for the given prim.
    ///
    /// Matches C++ `UsdInherits(const UsdPrim& prim)` (private constructor).
    pub(crate) fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Adds a path to the inheritPaths listOp at the current EditTarget,
    /// in the position specified by position.
    ///
    /// Matches C++ `AddInherit(const SdfPath &primPath, UsdListPosition position)`.
    pub fn add_inherit(&self, prim_path: &Path, position: ListPosition) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        // Get or create the inheritPaths metadata as a PathListOp
        let inherit_paths_token = usd_tf::Token::new("inheritPaths");
        let mut list_op: PathListOp = self
            .prim
            .get_metadata(&inherit_paths_token)
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
        let result = self
            .prim
            .set_metadata(&inherit_paths_token, usd_vt::Value::from(list_op));

        // Invalidate PrimIndex cache so inherit arc is picked up on recomposition
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }

        result
    }

    /// Removes the specified path from the inheritPaths listOp at the current EditTarget.
    ///
    /// Matches C++ `RemoveInherit(const SdfPath &primPath)`.
    pub fn remove_inherit(&self, prim_path: &Path) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let inherit_paths_token = usd_tf::Token::new("inheritPaths");
        let mut list_op: PathListOp = self
            .prim
            .get_metadata(&inherit_paths_token)
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

        let result = self
            .prim
            .set_metadata(&inherit_paths_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Removes the authored inheritPaths listOp edits at the current edit target.
    ///
    /// Matches C++ `ClearInherits()`.
    pub fn clear_inherits(&self) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let inherit_paths_token = usd_tf::Token::new("inheritPaths");
        let result = self.prim.clear_metadata(&inherit_paths_token);
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Explicitly set the inherited paths, potentially blocking weaker opinions.
    ///
    /// Matches C++ `SetInherits(const SdfPathVector& items)`.
    pub fn set_inherits(&self, items: Vec<Path>) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        let inherit_paths_token = usd_tf::Token::new("inheritPaths");
        let mut list_op = PathListOp::new();
        if list_op.set_explicit_items(items).is_err() {
            return false;
        }

        let result = self
            .prim
            .set_metadata(&inherit_paths_token, usd_vt::Value::from(list_op));
        if result {
            if let Some(stage) = self.prim.stage() {
                if let Some(pcp_cache) = stage.pcp_cache() {
                    pcp_cache.invalidate_prim_index(self.prim.path());
                }
                stage.handle_local_change(self.prim.path());
            }
        }
        result
    }

    /// Return all the paths in this prim's stage's local layer stack that would
    /// compose into this prim via direct inherits (excluding prim specs that
    /// would be composed into this prim due to inherits authored on ancestral
    /// prims) in strong-to-weak order.
    ///
    /// Matches C++ `GetAllDirectInherits()`.
    ///
    /// This method traverses the prim index to find all direct inherit arcs.
    /// It searches both inherit and specialize arcs, as specializes can introduce
    /// inherits that need to be included. Only direct inherits (not due to ancestors)
    /// are returned.
    ///
    /// # Returns
    ///
    /// A vector of paths representing all direct inherit arcs for this prim.
    pub fn get_all_direct_inherits(&self) -> Vec<Path> {
        if !self.prim.is_valid() {
            return Vec::new();
        }

        // Get the prim index to traverse inherit arcs
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

        // Helper function to check if a node is a direct inherit
        let mut add_if_direct_inherit = |node: &usd_pcp::NodeRef| {
            let arc_type = node.arc_type();

            // Check if this is an inherit or specialize arc
            if arc_type != usd_pcp::ArcType::Inherit && arc_type != usd_pcp::ArcType::Specialize {
                return;
            }

            // Check if the node's layer stack matches the root node's layer stack
            let Some(node_layer_stack) = node.layer_stack() else {
                return;
            };

            // Check if layer stacks match (same layer stack as root)
            if node_layer_stack.identifier() != root_layer_stack.identifier() {
                return;
            }

            // Check if the origin root node is not due to an ancestor
            let origin_root = node.origin_root_node();
            if origin_root.is_valid() && origin_root.is_due_to_ancestor() {
                return;
            }

            // Add the path if not already seen
            let path = node.path();
            if seen.insert(path.clone()) {
                result.push(path);
            }
        };

        // Search inherit nodes
        let (inherit_start, inherit_end) = prim_index.get_node_range(usd_pcp::RangeType::Inherit);
        for i in inherit_start..inherit_end {
            if let Some(node) = prim_index.nodes().get(i) {
                add_if_direct_inherit(node);
            }
        }

        // Search specialize nodes (as they can introduce inherits)
        let (specialize_start, specialize_end) =
            prim_index.get_node_range(usd_pcp::RangeType::Specialize);
        for i in specialize_start..specialize_end {
            if let Some(node) = prim_index.nodes().get(i) {
                add_if_direct_inherit(node);
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

impl std::ops::Deref for Inherits {
    type Target = Prim;

    fn deref(&self) -> &Self::Target {
        &self.prim
    }
}

impl From<Prim> for Inherits {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}
