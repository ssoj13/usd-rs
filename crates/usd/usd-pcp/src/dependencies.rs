//! PCP Dependencies - dependency tracking for prim indexes in cache.
//!
//! This module tracks dependencies of PcpPrimIndex entries in a PcpCache.
//! It is used for cache invalidation when layers or layer stacks change.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/dependencies.h` and `dependencies.cpp`.
//!
//! # Key Functionality
//!
//! - Track which prim indexes depend on which layer stacks and sites
//! - Query dependencies for cache invalidation
//! - Track culled dependencies for nodes that were removed during optimization

use crate::{LayerStackPtr, LayerStackRefPtr, Lifeboat, NodeRef, PrimIndex};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use usd_sdf::{Layer, Path};

// ============================================================================
// Dependencies
// ============================================================================

/// Tracks dependencies of PcpPrimIndex entries in a PcpCache.
///
/// This is an internal class for use by Cache for tracking what prim indexes
/// depend on which layer stacks and sites.
#[derive(Default)]
pub struct Dependencies {
    /// Map of layer stack to site dependencies.
    ///
    /// C++ uses `unordered_map<PcpLayerStackRefPtr, _SiteDepMap>` —
    /// the key is the actual layer stack shared pointer. We use
    /// LayerStackKey (string-based) for HashMap but store the actual
    /// LayerStackRefPtr alongside so lifeboat can retain them.
    deps: RwLock<HashMap<LayerStackKey, (LayerStackRefPtr, SiteDepMap)>>,

    /// Revision number for layer stack changes.
    layer_stacks_revision: RwLock<usize>,

    /// Map of prim index paths to culled dependencies.
    culled_dependencies: RwLock<HashMap<Path, Vec<CulledDependency>>>,

    /// Map of prim index paths to dynamic file format dependency data.
    file_format_deps: RwLock<
        HashMap<Path, super::dynamic_file_format_dependency_data::DynamicFileFormatDependencyData>,
    >,

    /// Map of field names to count of prim indexes that depend on them.
    possible_file_format_argument_fields: RwLock<HashMap<String, usize>>,

    /// Map of attribute names to count of prim indexes that depend on them.
    possible_file_format_argument_attributes: RwLock<HashMap<String, usize>>,

    /// Map of prim index paths to expression variables dependency data.
    expr_vars_deps: RwLock<
        HashMap<
            Path,
            super::expression_variables_dependency_data::ExpressionVariablesDependencyData,
        >,
    >,

    /// Map of layer stack to prim index paths that use expression vars from it.
    layer_stack_expr_vars: RwLock<HashMap<LayerStackKey, Vec<Path>>>,
}

/// Key for identifying a layer stack (using root layer identifier).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct LayerStackKey {
    root_layer_id: String,
}

impl LayerStackKey {
    fn from_layer_stack(layer_stack: &LayerStackRefPtr) -> Self {
        Self {
            root_layer_id: layer_stack
                .root_layer()
                .map(|l| l.identifier().to_string())
                .unwrap_or_default(),
        }
    }

    fn from_ptr(layer_stack: &LayerStackPtr) -> Option<Self> {
        layer_stack.upgrade().map(|ls| Self::from_layer_stack(&ls))
    }
}

/// Map of site paths to prim index paths that depend on them.
type SiteDepMap = HashMap<Path, Vec<Path>>;

impl Dependencies {
    /// Creates a new empty dependencies tracker.
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================================================
    // Registration
    // ========================================================================

    /// Adds dependency information for the given PrimIndex.
    ///
    /// Assumptions:
    /// - A computed prim index will be added exactly once
    /// - Parent indexes will be added before children
    pub fn add(
        &self,
        prim_index: &PrimIndex,
        culled_deps: Vec<CulledDependency>,
        file_format_dep_data: super::dynamic_file_format_dependency_data::DynamicFileFormatDependencyData,
        expr_var_dep_data: super::expression_variables_dependency_data::ExpressionVariablesDependencyData,
    ) {
        let prim_path = prim_index.path();
        if prim_path.is_empty() {
            return;
        }

        // Add dependencies for each node in the prim index
        for node in prim_index.nodes() {
            if !node.is_valid() || !node_introduces_dependency(&node) {
                continue;
            }

            if let Some(layer_stack) = node.layer_stack() {
                let key = LayerStackKey::from_layer_stack(&layer_stack);
                let site_path = node.path();

                let mut deps = self.deps.write();
                let entry = deps
                    .entry(key)
                    .or_insert_with(|| (layer_stack.clone(), SiteDepMap::new()));
                entry
                    .1
                    .entry(site_path)
                    .or_default()
                    .push(prim_path.clone());
            }
        }

        // Store culled dependencies
        if !culled_deps.is_empty() {
            self.culled_dependencies
                .write()
                .insert(prim_path.clone(), culled_deps);
        }

        // Store file format dependency data
        if file_format_dep_data.has_dependencies() {
            // Track fields
            for field in file_format_dep_data.get_relevant_field_names() {
                *self
                    .possible_file_format_argument_fields
                    .write()
                    .entry(field.to_string())
                    .or_insert(0) += 1;
            }
            // Track attributes
            for attr in file_format_dep_data.get_relevant_attribute_names() {
                *self
                    .possible_file_format_argument_attributes
                    .write()
                    .entry(attr.to_string())
                    .or_insert(0) += 1;
            }
            self.file_format_deps
                .write()
                .insert(prim_path.clone(), file_format_dep_data);
        }

        // Store expression variables dependency data
        if expr_var_dep_data.has_dependencies() {
            self.expr_vars_deps
                .write()
                .insert(prim_path.clone(), expr_var_dep_data);
        }
    }

    /// Removes dependency information for the given PrimIndex.
    ///
    /// Any layer stacks in use are added to the lifeboat if provided.
    pub fn remove(&self, prim_index: &PrimIndex, lifeboat: Option<&mut Lifeboat>) {
        let prim_path = prim_index.path();
        if prim_path.is_empty() {
            return;
        }

        // Remove from deps map
        let mut deps = self.deps.write();
        let mut empty_keys = Vec::new();

        for (key, (_ls_ref, site_map)) in deps.iter_mut() {
            for (_site_path, prim_paths) in site_map.iter_mut() {
                prim_paths.retain(|p| p != &prim_path);
            }
            site_map.retain(|_, v| !v.is_empty());
            if site_map.is_empty() {
                empty_keys.push(key.clone());
            }
        }

        for key in empty_keys {
            deps.remove(&key);
        }

        // Remove culled dependencies
        self.culled_dependencies.write().remove(&prim_path);

        // Remove file format deps and update field counts
        if let Some(data) = self.file_format_deps.write().remove(&prim_path) {
            for field in data.get_relevant_field_names() {
                let mut fields = self.possible_file_format_argument_fields.write();
                if let Some(count) = fields.get_mut(&field.to_string()) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        fields.remove(&field.to_string());
                    }
                }
            }
            for attr in data.get_relevant_attribute_names() {
                let mut attrs = self.possible_file_format_argument_attributes.write();
                if let Some(count) = attrs.get_mut(&attr.to_string()) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        attrs.remove(&attr.to_string());
                    }
                }
            }
        }

        // Remove expression vars deps
        self.expr_vars_deps.write().remove(&prim_path);

        // Retain layer stacks in lifeboat
        if let Some(lb) = lifeboat {
            for node in prim_index.nodes() {
                if let Some(ls) = node.layer_stack() {
                    lb.retain_layer_stack(ls);
                }
            }
        }
    }

    /// Removes all dependencies. Layer stacks are retained in lifeboat
    /// to prevent deallocation during change processing.
    ///
    /// C++ dependencies.cpp RemoveAll: iterates all entries, moves layer
    /// stack refs into lifeboat before clearing the map.
    pub fn remove_all(&self, lifeboat: Option<&mut Lifeboat>) {
        if let Some(lb) = lifeboat {
            let deps = self.deps.read();
            for (ls_ref, _site_map) in deps.values() {
                lb.retain_layer_stack(ls_ref.clone());
            }
        }

        self.deps.write().clear();
        self.culled_dependencies.write().clear();
        self.file_format_deps.write().clear();
        self.possible_file_format_argument_fields.write().clear();
        self.possible_file_format_argument_attributes
            .write()
            .clear();
        self.expr_vars_deps.write().clear();
        self.layer_stack_expr_vars.write().clear();
    }

    /// Notifies that layer stacks may have changed.
    pub fn layer_stacks_changed(&self) {
        *self.layer_stacks_revision.write() += 1;
    }

    /// Creates a context for concurrent population.
    ///
    /// This protects member data with a mutex during its lifetime,
    /// enabling safe concurrent calls to `add()`.
    ///
    /// Matches C++ `Pcp_Dependencies::ConcurrentPopulationContext`.
    pub fn concurrent_population_context(&self) -> ConcurrentPopulationContext<'_> {
        ConcurrentPopulationContext::new(self)
    }

    // ========================================================================
    // Queries
    // ========================================================================

    /// Iterates over all prim indexes that depend on the given site.
    pub fn for_each_dependency_on_site<F>(
        &self,
        site_layer_stack: &LayerStackRefPtr,
        site_path: &Path,
        include_ancestral: bool,
        recurse_below_site: bool,
        mut callback: F,
    ) where
        F: FnMut(&Path, &Path),
    {
        let key = LayerStackKey::from_layer_stack(site_layer_stack);
        let deps = self.deps.read();

        let site_dep_map = match deps.get(&key) {
            Some((_ls, m)) => m,
            None => return,
        };

        if recurse_below_site {
            // Find all paths at or below site_path
            for (path, prim_paths) in site_dep_map.iter() {
                if path.has_prefix(site_path) {
                    for prim_path in prim_paths {
                        callback(prim_path, path);
                    }
                }
            }
        } else {
            // Just the exact path
            if let Some(prim_paths) = site_dep_map.get(site_path) {
                for prim_path in prim_paths {
                    callback(prim_path, site_path);
                }
            }
        }

        if !include_ancestral {
            return;
        }

        // Walk up ancestors
        let mut ancestor_path = site_path.get_parent_path();
        while !ancestor_path.is_empty() {
            // Skip variant selection paths in USD mode
            if ancestor_path.is_prim_variant_selection_path() && site_layer_stack.is_usd() {
                break;
            }

            if let Some(prim_paths) = site_dep_map.get(&ancestor_path) {
                for prim_path in prim_paths {
                    callback(prim_path, &ancestor_path);
                }
            }

            ancestor_path = ancestor_path.get_parent_path();
        }
    }

    /// Returns all layers from all layer stacks with dependencies.
    pub fn get_used_layers(&self) -> HashSet<Arc<Layer>> {
        // Would need to track actual layer stacks to implement fully
        // For now return empty
        HashSet::new()
    }

    /// Returns the layer stacks revision number.
    pub fn get_layer_stacks_revision(&self) -> usize {
        *self.layer_stacks_revision.read()
    }

    /// Returns true if there are dependencies on the given layer stack.
    pub fn uses_layer_stack(&self, layer_stack: &LayerStackPtr) -> bool {
        if let Some(key) = LayerStackKey::from_ptr(layer_stack) {
            self.deps.read().contains_key(&key)
        } else {
            false
        }
    }

    /// Returns culled dependencies for the given prim index path.
    pub fn get_culled_dependencies(&self, prim_index_path: &Path) -> Vec<CulledDependency> {
        self.culled_dependencies
            .read()
            .get(prim_index_path)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns true if there are any dynamic file format field dependencies.
    pub fn has_any_dynamic_file_format_argument_field_dependencies(&self) -> bool {
        !self.possible_file_format_argument_fields.read().is_empty()
    }

    /// Returns true if there are any dynamic file format attribute dependencies.
    pub fn has_any_dynamic_file_format_argument_attribute_dependencies(&self) -> bool {
        !self
            .possible_file_format_argument_attributes
            .read()
            .is_empty()
    }

    /// Returns true if the field is a possible dynamic file format argument field.
    pub fn is_possible_dynamic_file_format_argument_field(&self, field: &str) -> bool {
        self.possible_file_format_argument_fields
            .read()
            .contains_key(field)
    }

    /// Returns true if the attribute is a possible dynamic file format argument attribute.
    pub fn is_possible_dynamic_file_format_argument_attribute(&self, attribute: &str) -> bool {
        self.possible_file_format_argument_attributes
            .read()
            .contains_key(attribute)
    }

    /// Returns the dynamic file format dependency data for a prim index.
    pub fn get_dynamic_file_format_argument_dependency_data(
        &self,
        prim_index_path: &Path,
    ) -> Option<super::dynamic_file_format_dependency_data::DynamicFileFormatDependencyData> {
        self.file_format_deps.read().get(prim_index_path).cloned()
    }

    /// Returns prim index paths using expression variables from the layer stack.
    pub fn get_prims_using_expression_variables_from_layer_stack(
        &self,
        layer_stack: &LayerStackPtr,
    ) -> Vec<Path> {
        if let Some(key) = LayerStackKey::from_ptr(layer_stack) {
            self.layer_stack_expr_vars
                .read()
                .get(&key)
                .cloned()
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

// ============================================================================
// Concurrent Population Context
// ============================================================================

/// Context for enabling concurrent population of dependencies.
///
/// Protects member data with a mutex during its lifetime.
/// Matches C++ `Pcp_Dependencies::ConcurrentPopulationContext`.
pub struct ConcurrentPopulationContext<'a> {
    deps: &'a Dependencies,
    _guard:
        parking_lot::RwLockWriteGuard<'a, HashMap<LayerStackKey, (LayerStackRefPtr, SiteDepMap)>>,
}

impl<'a> ConcurrentPopulationContext<'a> {
    /// Creates a new concurrent population context.
    fn new(deps: &'a Dependencies) -> Self {
        Self {
            deps,
            _guard: deps.deps.write(),
        }
    }

    /// Adds dependency information (thread-safe version).
    ///
    /// This method is protected by the context's mutex.
    pub fn add(
        &mut self,
        prim_index: &PrimIndex,
        culled_deps: Vec<CulledDependency>,
        file_format_dep_data: super::dynamic_file_format_dependency_data::DynamicFileFormatDependencyData,
        expr_var_dep_data: super::expression_variables_dependency_data::ExpressionVariablesDependencyData,
    ) {
        // Delegate to the regular add method - the guard ensures thread safety
        self.deps.add(
            prim_index,
            culled_deps,
            file_format_dep_data,
            expr_var_dep_data,
        );
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Returns true if the node introduces a dependency that should be tracked.
///
/// Re-exports from dependency module for convenience.
pub use super::dependency::{classify_node_dependency, node_introduces_dependency};

/// Returns true if the node uses the given layer.
pub fn node_uses_layer_or_layer_stack_layer(node: &NodeRef, layer: &Arc<Layer>) -> bool {
    node.layer_stack()
        .map(|ls| ls.get_layers().iter().any(|l| Arc::ptr_eq(l, layer)))
        .unwrap_or(false)
}

/// Returns true if the node uses the given layer stack.
pub fn node_uses_layer_or_layer_stack(node: &NodeRef, layer_stack: &LayerStackRefPtr) -> bool {
    node.layer_stack()
        .map(|ls| Arc::ptr_eq(&ls, layer_stack))
        .unwrap_or(false)
}

/// Helper function to iterate over dependent nodes for a given site.
///
/// Matches C++ `Pcp_ForEachDependentNode`.
pub fn for_each_dependent_node<F, C>(
    site_path: &Path,
    site_layer_stack: &LayerStackRefPtr,
    dep_index_path: &Path,
    cache: &C,
    node_fn: &mut F,
) -> bool
where
    F: FnMut(&Path, &NodeRef),
    C: DependentNodeCache,
{
    // Walk up as needed to find a containing prim index.
    let mut index_path = dep_index_path.get_absolute_root_or_prim_path();
    let mut found_dep = false;

    while !index_path.is_empty() {
        if let Some(prim_index) = cache.find_prim_index(&index_path) {
            if prim_index.is_valid() {
                // Find which node corresponds to (site_layer_stack, site_path).
                for node in prim_index.nodes() {
                    if super::dependency::node_introduces_dependency(&node)
                        && node_uses_layer_or_layer_stack(&node, site_layer_stack)
                        && site_path.has_prefix(&node.path())
                    {
                        found_dep = true;
                        node_fn(dep_index_path, &node);
                    }
                }
                break;
            }
        }
        index_path = index_path.get_parent_path();
    }

    found_dep
}

/// Helper function to iterate over dependent nodes and culled dependencies.
///
/// Matches C++ `Pcp_ForEachDependentNode` with culled dependencies.
pub fn for_each_dependent_node_with_culled<F, G, C>(
    site_path: &Path,
    site_layer_stack: &LayerStackRefPtr,
    dep_index_path: &Path,
    cache: &C,
    mut node_fn: F,
    mut culled_dep_fn: G,
) where
    F: FnMut(&Path, &NodeRef),
    G: FnMut(&Path, &CulledDependency),
    C: DependentNodeCache,
{
    let mut _found_dep = for_each_dependent_node(
        site_path,
        site_layer_stack,
        dep_index_path,
        cache,
        &mut node_fn,
    );

    // Process culled dependencies
    let culled_deps = cache.get_culled_dependencies(dep_index_path);
    for dep in &culled_deps {
        if Arc::ptr_eq(&dep.layer_stack, site_layer_stack) && site_path.has_prefix(&dep.site_path) {
            _found_dep = true;
            culled_dep_fn(dep_index_path, dep);
        }
    }

    // In C++: TF_VERIFY(foundDep) - skipped in Rust
    debug_assert!(_found_dep, "Expected to find at least one dependency");
}

/// Trait for cache objects that can find prim indices and culled dependencies.
pub trait DependentNodeCache {
    /// Finds a prim index by path.
    fn find_prim_index(&self, path: &Path) -> Option<PrimIndex>;
    /// Gets culled dependencies for a prim at the given path.
    fn get_culled_dependencies(&self, path: &Path) -> Vec<CulledDependency>;
}

/// Records CulledDependency for nodes that would be tracked if they remained.
pub fn add_culled_dependencies(prim_index: &PrimIndex, culled_deps: &mut Vec<CulledDependency>) {
    use super::dependency::{classify_node_dependency, node_introduces_dependency};

    for node in prim_index.nodes() {
        if !node.is_valid() || node.is_inert() {
            continue;
        }

        // Nodes that will be culled but would introduce dependencies
        if node.is_culled() && node_introduces_dependency(&node) {
            if let Some(layer_stack) = node.layer_stack() {
                let flags = classify_node_dependency(&node);
                let arc_type = node.arc_type();
                let site_path = node.path();
                let map_to_root = node.map_to_root().evaluate();

                // For culled nodes, we need to determine the unrelocated site path
                // This is typically the same as site_path unless relocations were applied
                let unrelocated_site_path = if arc_type == super::ArcType::Relocate {
                    // For relocate nodes, we need to walk up to find the unrelocated path
                    // Simplified: use site_path for now, full implementation would track relocations
                    site_path.clone()
                } else {
                    Path::empty()
                };

                culled_deps.push(CulledDependency {
                    flags,
                    arc_type,
                    layer_stack: layer_stack.clone(),
                    site_path,
                    unrelocated_site_path,
                    map_to_root,
                });
            }
        }
    }
}

// ============================================================================
// Culled Dependency
// ============================================================================

/// A dependency that was culled during prim index optimization.
#[derive(Clone, Debug)]
pub struct CulledDependency {
    /// Flag representing the type of dependency.
    pub flags: super::DependencyFlags,
    /// Arc type for this dependency.
    pub arc_type: super::ArcType,
    /// Layer stack containing the specs the prim index depends on.
    pub layer_stack: LayerStackRefPtr,
    /// Path of the dependency specs in the layer stack.
    pub site_path: Path,
    /// If relocations applied to the dependency node, this is the
    /// unrelocated site path. Otherwise, this is empty.
    pub unrelocated_site_path: Path,
    /// The map function that applies to values from the site.
    pub map_to_root: super::MapFunction,
}

// ============================================================================
// Dynamic File Format Dependency Data
// ============================================================================
// Note: Full implementation is in dynamic_file_format_dependency_data.rs
// Re-exported for use in this module.
pub use super::dynamic_file_format_dependency_data::DynamicFileFormatDependencyData;

// ============================================================================
// Expression Variables Dependency Data
// ============================================================================
// Note: Full implementation is in expression_variables_dependency_data.rs
// Re-exported for use in this module.
pub use super::expression_variables_dependency_data::ExpressionVariablesDependencyData;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_dependencies() {
        let deps = Dependencies::new();
        assert_eq!(deps.get_layer_stacks_revision(), 0);
        assert!(!deps.has_any_dynamic_file_format_argument_field_dependencies());
    }

    #[test]
    fn test_layer_stacks_changed() {
        let deps = Dependencies::new();
        assert_eq!(deps.get_layer_stacks_revision(), 0);
        deps.layer_stacks_changed();
        assert_eq!(deps.get_layer_stacks_revision(), 1);
    }

    #[test]
    fn test_culled_dependency() {
        use crate::{ArcType, DependencyFlags, LayerStack, LayerStackIdentifier, MapFunction};

        // LayerStack::new already returns Arc<LayerStack>
        let layer_stack = LayerStack::new(LayerStackIdentifier::default());
        let dep = CulledDependency {
            flags: DependencyFlags::DIRECT,
            arc_type: ArcType::Reference,
            layer_stack,
            site_path: Path::from_string("/World").unwrap(),
            unrelocated_site_path: Path::empty(),
            map_to_root: MapFunction::identity().clone(),
        };
        assert_eq!(dep.site_path.as_str(), "/World");
        assert!(dep.flags.contains(DependencyFlags::DIRECT));
    }

    #[test]
    fn test_dynamic_file_format_dependency_data() {
        use crate::DynamicFileFormatInterface;
        use std::collections::HashSet;
        use usd_tf::Token;
        use usd_vt::Value;

        // Mock implementation for testing
        struct MockFormat;
        impl DynamicFileFormatInterface for MockFormat {
            fn compose_fields_for_file_format_arguments(
                &self,
                _asset_path: &str,
                _context: &mut crate::DynamicFileFormatContext,
                _args: &mut usd_sdf::FileFormatArguments,
                _dep_data: &mut Option<Value>,
            ) {
            }
        }
        static MOCK_FORMAT: MockFormat = MockFormat;

        let mut data = DynamicFileFormatDependencyData::new();
        assert!(!data.has_dependencies());

        // Add dependency context with a relevant field
        let mut fields = HashSet::new();
        fields.insert(Token::new("myField"));
        data.add_dependency_context(
            &MOCK_FORMAT as *const dyn DynamicFileFormatInterface,
            Value::default(),
            fields,
            HashSet::new(),
        );
        assert!(data.has_dependencies());
        assert_eq!(data.relevant_fields().len(), 1);
    }

    #[test]
    fn test_expression_variables_dependency_data() {
        let data = ExpressionVariablesDependencyData::new();
        assert!(!data.has_dependencies());
        assert!(data.is_empty());

        // Note: add_dependency(String, String) is a compatibility stub
        // The real API uses add_dependencies(LayerStackRefPtr, HashSet<String>)
        // For now, just verify the basic empty state
    }
}
