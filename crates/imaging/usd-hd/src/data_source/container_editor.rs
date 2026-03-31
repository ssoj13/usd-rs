
//! Utility for lazily constructing and composing data source hierarchies.
//!
//! Port of pxr/imaging/hd/containerDataSourceEditor.{h,cpp}

use super::HdBlockDataSource;
use super::base::HdDataSourceBaseHandle;
use super::container::{HdContainerDataSource, HdContainerDataSourceHandle, cast_to_container};
use super::locator::{HdDataSourceLocator, HdDataSourceLocatorSet};
use super::retained::HdOverlayContainerDataSource;
use std::collections::HashMap;
use std::sync::Arc;
use usd_tf::Token;

/// Node in the editor's overlay tree.
#[derive(Clone, Debug, Default)]
struct Node {
    entries: HashMap<Token, Entry>,
}

#[derive(Clone, Debug)]
struct Entry {
    data_source: Option<HdDataSourceBaseHandle>,
    child_node: Option<Arc<Node>>,
}

/// Recursive container that reads from the editor's node tree.
#[derive(Clone, Debug)]
struct NodeContainerDataSource {
    node: Arc<Node>,
}

impl NodeContainerDataSource {
    fn new(node: Arc<Node>) -> Arc<Self> {
        Arc::new(Self { node })
    }
}

impl super::base::HdDataSourceBase for NodeContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone()) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for NodeContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.node
            .entries
            .iter()
            .filter(|(_, e)| e.data_source.is_some() || e.child_node.is_some())
            .map(|(k, _)| k.clone())
            .collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let entry = self.node.entries.get(name)?;
        if let Some(ref ds) = entry.data_source {
            if let Some(container) = cast_to_container(ds) {
                if let Some(ref child) = entry.child_node {
                    return Some(HdOverlayContainerDataSource::new_2(
                        NodeContainerDataSource::new(child.clone()),
                        container,
                    ) as HdDataSourceBaseHandle);
                }
                return Some(ds.clone());
            }
            return Some(ds.clone());
        }
        if let Some(ref child) = entry.child_node {
            return Some(NodeContainerDataSource::new(child.clone()) as HdDataSourceBaseHandle);
        }
        None
    }
}

/// Editor for composing container data source with Set/Overlay operations.
pub struct HdContainerDataSourceEditor {
    root: Option<Arc<Node>>,
    initial_container: Option<HdContainerDataSourceHandle>,
    direct_container_sets: Vec<HdDataSourceLocator>,
}

impl Default for HdContainerDataSourceEditor {
    fn default() -> Self {
        Self {
            root: None,
            initial_container: None,
            direct_container_sets: Vec::new(),
        }
    }
}

impl HdContainerDataSourceEditor {
    /// Create editor with optional initial container.
    pub fn new(initial_container: Option<HdContainerDataSourceHandle>) -> Self {
        Self {
            root: Some(Arc::new(Node::default())),
            initial_container,
            direct_container_sets: Vec::new(),
        }
    }

    /// Traverses/builds the path and applies a mutation to the entry at the locator.
    fn with_entry_at<F>(node_ref: &mut Arc<Node>, path: &[Token], f: F) -> bool
    where
        F: FnOnce(&mut Entry),
    {
        match path {
            [] => false,
            [name] => {
                let node = Arc::make_mut(node_ref);
                let entry = node.entries.entry(name.clone()).or_insert_with(|| Entry {
                    data_source: None,
                    child_node: None,
                });
                f(entry);
                true
            }
            [name, rest @ ..] => {
                let node = Arc::make_mut(node_ref);
                let entry = node.entries.entry(name.clone()).or_insert_with(|| Entry {
                    data_source: None,
                    child_node: None,
                });
                let child = entry
                    .child_node
                    .get_or_insert_with(|| Arc::new(Node::default()));
                Self::with_entry_at(child, rest, f)
            }
        }
    }

    /// Replaces data source at given locator. Use None/HdBlockDataSource for deletion.
    pub fn set(
        &mut self,
        locator: &HdDataSourceLocator,
        data_source: Option<HdDataSourceBaseHandle>,
    ) -> &mut Self {
        if locator.is_empty() {
            return self;
        }
        if self.root.is_none() {
            self.root = Some(Arc::new(Node::default()));
        }
        let root = self.root.as_mut().unwrap();
        let last = locator.last_element().cloned().unwrap_or_default();
        let parent_loc = locator.remove_last();

        if parent_loc.is_empty() {
            let parent_node = Arc::make_mut(root);
            let entry = parent_node.entries.entry(last).or_insert_with(|| Entry {
                data_source: None,
                child_node: None,
            });
            entry.child_node = None;
            entry.data_source = Some(
                data_source.unwrap_or_else(|| HdBlockDataSource::new() as HdDataSourceBaseHandle),
            );
            if self.initial_container.is_some() {
                let is_block = entry
                    .data_source
                    .as_ref()
                    .map(|ds| ds.as_any().downcast_ref::<HdBlockDataSource>().is_some())
                    .unwrap_or(false);
                let is_container = entry
                    .data_source
                    .as_ref()
                    .and_then(|ds| cast_to_container(ds))
                    .is_some();
                if is_block || is_container {
                    self.direct_container_sets.push(locator.clone());
                }
            }
            return self;
        }

        let locator_clone = locator.clone();
        let is_block_or_container = self.initial_container.is_some()
            && (data_source.is_none()
                || data_source
                    .as_ref()
                    .and_then(|d| cast_to_container(d))
                    .is_some());
        let ds = data_source.unwrap_or_else(|| HdBlockDataSource::new() as HdDataSourceBaseHandle);
        Self::with_entry_at(root, locator.elements(), |entry| {
            entry.child_node = None;
            entry.data_source = Some(ds);
        });
        if is_block_or_container {
            self.direct_container_sets.push(locator_clone);
        }
        self
    }

    /// Overlays container at given locator so descending locations from initial can still show.
    pub fn overlay(
        &mut self,
        locator: &HdDataSourceLocator,
        container_data_source: Option<HdContainerDataSourceHandle>,
    ) -> &mut Self {
        if locator.is_empty() || container_data_source.is_none() {
            return self;
        }
        if self.root.is_none() {
            self.root = Some(Arc::new(Node::default()));
        }
        let root = self.root.as_mut().unwrap();
        let container_handle = container_data_source.unwrap();
        Self::with_entry_at(root, locator.elements(), |entry| {
            entry.data_source = Some(container_handle as HdDataSourceBaseHandle);
        });
        self
    }

    /// Returns final container with all edits applied.
    pub fn finish(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(ref initial) = self.initial_container {
            let root = self.root.as_ref()?;
            let overlay_top =
                NodeContainerDataSource::new(root.clone()) as HdContainerDataSourceHandle;

            if self.direct_container_sets.is_empty() {
                return Some(HdOverlayContainerDataSource::new_2(
                    overlay_top,
                    initial.clone(),
                ));
            }

            // Sort so shorter locators come first (C++ _LongerLocatorIsLess reverses this)
            let mut sorted = self.direct_container_sets.clone();
            sorted.sort_by(|a, b| a.len().cmp(&b.len()));

            let mut blocks_editor = Self::new(None);
            for loc in &sorted {
                blocks_editor.set(
                    loc,
                    Some(HdBlockDataSource::new() as HdDataSourceBaseHandle),
                );
            }
            let blocks = blocks_editor.finish()?;

            return Some(HdOverlayContainerDataSource::new(vec![
                overlay_top,
                blocks,
                initial.clone(),
            ]));
        }

        if self.root.is_none() {
            return None;
        }

        let root = self.root.as_ref().unwrap();
        if root.entries.is_empty() {
            return None;
        }

        Some(NodeContainerDataSource::new(root.clone()) as HdContainerDataSourceHandle)
    }

    /// Computes locators that need invalidation given the set of locators being set/overlaid.
    pub fn compute_dirty_locators(locator_set: &HdDataSourceLocatorSet) -> HdDataSourceLocatorSet {
        if locator_set.is_universal() {
            return locator_set.clone();
        }

        let mut result = HdDataSourceLocatorSet::new();
        for loc in locator_set.iter() {
            let mut cur_loc = HdDataSourceLocator::empty();
            for i in 0..loc.len() {
                // HdDataSourceLocatorSentinelTokens->container = "__containerDataSource"
                let container_token = Token::new("__containerDataSource");
                result.insert(cur_loc.append(&container_token));
                if let Some(name) = loc.get_element(i) {
                    cur_loc = cur_loc.append(name);
                }
            }
            result.insert(loc.clone());
        }
        result
    }
}
