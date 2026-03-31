//! Layer tree structure for sublayer hierarchy.
//!
//! Port of pxr/usd/sdf/layerTree.h
//!
//! A LayerTree is an immutable tree structure representing a sublayer
//! stack and its recursive structure.

use crate::{Layer, LayerOffset};
use std::fmt;
use std::sync::Arc;

/// Handle to a layer tree node.
pub type LayerTreeHandle = Arc<LayerTree>;

/// Vector of layer tree handles.
pub type LayerTreeHandleVector = Vec<LayerTreeHandle>;

/// An immutable tree structure representing a sublayer stack.
///
/// Layers can have sublayers, which can in turn have sublayers of their
/// own. This structure represents that hierarchy in memory.
pub struct LayerTree {
    /// The layer this node represents.
    layer: Arc<Layer>,
    /// Cumulative offset from root of tree.
    offset: LayerOffset,
    /// Child trees (sublayers).
    child_trees: LayerTreeHandleVector,
}

impl fmt::Debug for LayerTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayerTree")
            .field("layer", &self.layer.identifier())
            .field("offset", &self.offset)
            .field("child_count", &self.child_trees.len())
            .finish()
    }
}

impl LayerTree {
    /// Creates a new layer tree node.
    ///
    /// # Arguments
    /// * `layer` - The layer this node represents
    /// * `child_trees` - Child layer trees (sublayers)
    /// * `cumulative_offset` - Offset from the tree root
    pub fn new(
        layer: Arc<Layer>,
        child_trees: LayerTreeHandleVector,
        cumulative_offset: LayerOffset,
    ) -> LayerTreeHandle {
        Arc::new(Self {
            layer,
            offset: cumulative_offset,
            child_trees,
        })
    }

    /// Creates a new layer tree node with default offset.
    pub fn new_simple(layer: Arc<Layer>, child_trees: LayerTreeHandleVector) -> LayerTreeHandle {
        Self::new(layer, child_trees, LayerOffset::identity())
    }

    /// Creates a leaf node with no children.
    pub fn new_leaf(layer: Arc<Layer>) -> LayerTreeHandle {
        Self::new(layer, Vec::new(), LayerOffset::identity())
    }

    /// Creates a leaf node with offset.
    pub fn new_leaf_with_offset(layer: Arc<Layer>, offset: LayerOffset) -> LayerTreeHandle {
        Self::new(layer, Vec::new(), offset)
    }

    /// Returns the layer handle this tree node represents.
    pub fn get_layer(&self) -> &Arc<Layer> {
        &self.layer
    }

    /// Returns the cumulative layer offset from the root of the tree.
    pub fn get_offset(&self) -> &LayerOffset {
        &self.offset
    }

    /// Returns the children of this tree node.
    pub fn get_child_trees(&self) -> &LayerTreeHandleVector {
        &self.child_trees
    }

    /// Returns true if this is a leaf node (no children).
    pub fn is_leaf(&self) -> bool {
        self.child_trees.is_empty()
    }

    /// Returns the number of direct children.
    pub fn child_count(&self) -> usize {
        self.child_trees.len()
    }

    /// Returns the total number of nodes in this tree (including self).
    pub fn total_node_count(&self) -> usize {
        1 + self
            .child_trees
            .iter()
            .map(|c| c.total_node_count())
            .sum::<usize>()
    }

    /// Returns the depth of this tree (1 for leaf, max child depth + 1 otherwise).
    pub fn depth(&self) -> usize {
        if self.child_trees.is_empty() {
            1
        } else {
            1 + self
                .child_trees
                .iter()
                .map(|c| c.depth())
                .max()
                .unwrap_or(0)
        }
    }

    /// Finds a layer in this tree.
    ///
    /// Returns the tree node containing the layer, or None if not found.
    pub fn find_layer(&self, layer: &Arc<Layer>) -> Option<&LayerTree> {
        if Arc::ptr_eq(&self.layer, layer) {
            return Some(self);
        }
        for child in &self.child_trees {
            if let Some(found) = child.find_layer(layer) {
                return Some(found);
            }
        }
        None
    }

    /// Returns all layers in this tree as a flat vector.
    pub fn collect_layers(&self) -> Vec<Arc<Layer>> {
        let mut result = vec![self.layer.clone()];
        for child in &self.child_trees {
            result.extend(child.collect_layers());
        }
        result
    }

    /// Visits all nodes in depth-first order.
    pub fn visit<F>(&self, visitor: &mut F)
    where
        F: FnMut(&LayerTree),
    {
        visitor(self);
        for child in &self.child_trees {
            child.visit(visitor);
        }
    }

    /// Visits all nodes with their depth level.
    pub fn visit_with_depth<F>(&self, depth: usize, visitor: &mut F)
    where
        F: FnMut(&LayerTree, usize),
    {
        visitor(self, depth);
        for child in &self.child_trees {
            child.visit_with_depth(depth + 1, visitor);
        }
    }
}

impl Clone for LayerTree {
    fn clone(&self) -> Self {
        Self {
            layer: self.layer.clone(),
            offset: self.offset,
            child_trees: self.child_trees.clone(),
        }
    }
}

/// Builder for constructing layer trees.
pub struct LayerTreeBuilder {
    layer: Arc<Layer>,
    offset: LayerOffset,
    children: Vec<LayerTreeHandle>,
}

impl LayerTreeBuilder {
    /// Creates a new builder for the given layer.
    pub fn new(layer: Arc<Layer>) -> Self {
        Self {
            layer,
            offset: LayerOffset::identity(),
            children: Vec::new(),
        }
    }

    /// Sets the cumulative offset.
    pub fn with_offset(mut self, offset: LayerOffset) -> Self {
        self.offset = offset;
        self
    }

    /// Adds a child tree.
    pub fn add_child(mut self, child: LayerTreeHandle) -> Self {
        self.children.push(child);
        self
    }

    /// Adds multiple child trees.
    pub fn add_children(mut self, children: impl IntoIterator<Item = LayerTreeHandle>) -> Self {
        self.children.extend(children);
        self
    }

    /// Builds the layer tree.
    pub fn build(self) -> LayerTreeHandle {
        LayerTree::new(self.layer, self.children, self.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_layer(name: &str) -> Arc<Layer> {
        Layer::create_anonymous(Some(name))
    }

    #[test]
    fn test_leaf_node() {
        let layer = make_test_layer("test");
        let tree = LayerTree::new_leaf(layer.clone());

        assert!(tree.is_leaf());
        assert_eq!(tree.child_count(), 0);
        assert_eq!(tree.total_node_count(), 1);
        assert_eq!(tree.depth(), 1);
    }

    #[test]
    fn test_tree_with_children() {
        let root = make_test_layer("root");
        let child1 = make_test_layer("child1");
        let child2 = make_test_layer("child2");

        let child1_tree = LayerTree::new_leaf(child1);
        let child2_tree = LayerTree::new_leaf(child2);

        let tree = LayerTree::new_simple(root, vec![child1_tree, child2_tree]);

        assert!(!tree.is_leaf());
        assert_eq!(tree.child_count(), 2);
        assert_eq!(tree.total_node_count(), 3);
        assert_eq!(tree.depth(), 2);
    }

    #[test]
    fn test_collect_layers() {
        let root = make_test_layer("root");
        let child = make_test_layer("child");

        let child_tree = LayerTree::new_leaf(child);
        let tree = LayerTree::new_simple(root, vec![child_tree]);

        let layers = tree.collect_layers();
        assert_eq!(layers.len(), 2);
    }

    #[test]
    fn test_builder() {
        let layer = make_test_layer("test");
        let offset = LayerOffset::new(10.0, 2.0);

        let tree = LayerTreeBuilder::new(layer)
            .with_offset(offset.clone())
            .build();

        assert_eq!(tree.get_offset(), &offset);
    }
}
