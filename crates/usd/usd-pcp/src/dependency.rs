//! PCP dependency types and utilities.
//!
//! Dependencies track the relationship between prim indices and the sites
//! that contribute to them.
//!
//! # Dependency Types
//!
//! Dependencies are classified by composition structure:
//!
//! - **Root**: The root dependency of a cache on its root site
//! - **Direct**: Dependencies from arcs at this level of namespace
//! - **Ancestral**: Dependencies from arcs at ancestral levels
//! - **Virtual**: Dependencies that don't contribute scene description
//!
//! # Examples
//!
//! ```
//! use usd_pcp::{DependencyType, DependencyFlags, dependency_flags_to_string};
//!
//! // Create a combined flag for direct dependencies
//! let flags = DependencyType::DIRECT;
//! assert!(flags.contains(DependencyType::PURELY_DIRECT));
//! assert!(flags.contains(DependencyType::PARTLY_DIRECT));
//!
//! // Convert to string for debugging
//! let s = dependency_flags_to_string(flags);
//! assert!(s.contains("direct"));
//! ```

use std::fmt;

use usd_sdf::Path;

use super::map_function::MapFunction;

bitflags::bitflags! {
    /// A classification of PcpPrimIndex->PcpSite dependencies
    /// by composition structure.
    ///
    /// These flags can be combined to form a bitmask for filtering
    /// dependencies by type.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct DependencyType: u32 {
        /// No type of dependency.
        const NONE = 0;

        /// The root dependency of a cache on its root site.
        ///
        /// This may be useful to either include, as when invalidating
        /// caches in response to scene edits, or to exclude, as when
        /// scanning dependency arcs to compensate for a namespace edit.
        const ROOT = 1 << 0;

        /// Purely direct dependencies involve only arcs introduced
        /// directly at this level of namespace.
        const PURELY_DIRECT = 1 << 1;

        /// Partly direct dependencies involve at least one arc introduced
        /// directly at this level of namespace; they may also involve
        /// ancestral arcs along the chain as well.
        const PARTLY_DIRECT = 1 << 2;

        /// Ancestral dependencies involve only arcs from ancestral
        /// levels of namespace, and no direct arcs.
        const ANCESTRAL = 1 << 3;

        /// Virtual dependencies do not contribute scene description,
        /// yet represent sites whose scene description (or ancestral
        /// scene description) informed the structure of the cache.
        ///
        /// One case is when a reference or payload arc does not specify
        /// a prim, and the target layer stack does not provide defaultPrim
        /// metadata. In that case, a virtual dependency to the root of
        /// that layer stack represents the latent dependency on metadata.
        ///
        /// Another case is "spooky ancestral" dependencies from relocates.
        /// These are referred to as "spooky" because they can be seen as
        /// a form of action-at-a-distance.
        const VIRTUAL = 1 << 4;

        /// Non-virtual dependencies.
        const NON_VIRTUAL = 1 << 5;

        /// Combined mask value representing both pure and partly direct deps.
        const DIRECT = Self::PARTLY_DIRECT.bits() | Self::PURELY_DIRECT.bits();

        /// Combined mask value representing any kind of dependency,
        /// except virtual ones.
        const ANY_NON_VIRTUAL = Self::ROOT.bits()
            | Self::DIRECT.bits()
            | Self::ANCESTRAL.bits()
            | Self::NON_VIRTUAL.bits();

        /// Combined mask value representing any kind of dependency.
        const ANY_INCLUDING_VIRTUAL = Self::ANY_NON_VIRTUAL.bits() | Self::VIRTUAL.bits();
    }
}

impl fmt::Display for DependencyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", dependency_flags_to_string(*self))
    }
}

/// A typedef for a bitmask of flags from DependencyType.
pub type DependencyFlags = DependencyType;

/// Description of a dependency.
///
/// A dependency records the relationship between a path in a PcpCache's
/// root layer stack and a site that contributes to it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Dependency {
    /// The path in this PcpCache's root layer stack that depends on the site.
    pub index_path: Path,
    /// The site path. When using recurse_down_namespace, this may be
    /// a path beneath the initial site path.
    pub site_path: Path,
    /// The map function that applies to values from the site.
    pub map_func: MapFunction,
}

impl Dependency {
    /// Creates a new dependency.
    ///
    /// # Arguments
    ///
    /// * `index_path` - The path in the PcpCache's root layer stack
    /// * `site_path` - The site path
    /// * `map_func` - The map function for value transformation
    pub fn new(index_path: Path, site_path: Path, map_func: MapFunction) -> Self {
        Self {
            index_path,
            site_path,
            map_func,
        }
    }
}

/// A vector of dependencies.
pub type DependencyVector = Vec<Dependency>;

/// Returns true if this node introduces a dependency in its PrimIndex.
///
/// Inert propagated class-based arcs (inherit/specialize that were copied
/// to the root for strength ordering) do NOT introduce real dependencies.
///
/// Matches C++ `PcpNodeIntroducesDependency()`.
pub fn node_introduces_dependency(node: &super::NodeRef) -> bool {
    use super::ArcType;

    if !node.is_valid() {
        return false;
    }

    if node.is_inert() {
        match node.arc_type() {
            ArcType::Specialize => {
                // Specializes nodes not under root are propagated copies -- no dependency
                if node.parent_node() != node.root_node() {
                    return false;
                }
                // Otherwise fall through to inherit check
                // (inert specialize under root still checks origin)
                if node.origin_node() != node.parent_node() {
                    return false;
                }
            }
            ArcType::Inherit => {
                // Inert implied/propagated inherit nodes do not introduce dependencies
                if node.origin_node() != node.parent_node() {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// Classifies the dependency represented by a node.
///
/// Returns a bitmask of flags from DependencyType.
/// Matches C++ `PcpClassifyNodeDependency()`.
pub fn classify_node_dependency(node: &super::NodeRef) -> DependencyFlags {
    use super::{ArcType, DependencyType};

    if !node.is_valid() {
        return DependencyType::NONE;
    }

    // Root node is always a root dependency
    if node.arc_type() == ArcType::Root {
        return DependencyType::ROOT;
    }

    let mut flags = DependencyType::NONE;

    // Inert nodes can represent virtual dependencies (e.g. relocates,
    // permission-denied arcs, refs without defaultPrim). But propagated
    // class-based inert nodes don't represent any dependency at all.
    if node.is_inert() {
        if !node_introduces_dependency(node) {
            return DependencyType::NONE;
        }
        flags |= DependencyType::VIRTUAL;
    }

    // Classify as direct / ancestral based on the transitive arc flags
    // set during indexing (has_transitive_direct_dependency and
    // has_transitive_ancestral_dependency).
    let any_direct = node.has_transitive_direct_dependency();
    let any_ancestral = node.has_transitive_ancestral_dependency();

    if any_direct {
        if any_ancestral {
            flags |= DependencyType::PARTLY_DIRECT;
        } else {
            flags |= DependencyType::PURELY_DIRECT;
        }
    } else if any_ancestral {
        flags |= DependencyType::ANCESTRAL;
    }

    // Mark as non-virtual if no virtual flag
    if !flags.contains(DependencyType::VIRTUAL) {
        flags |= DependencyType::NON_VIRTUAL;
    }

    flags
}

/// Converts dependency flags to a human-readable string.
///
/// # Examples
///
/// ```
/// use usd_pcp::{DependencyType, dependency_flags_to_string};
///
/// let flags = DependencyType::ROOT | DependencyType::DIRECT;
/// let s = dependency_flags_to_string(flags);
/// assert!(s.contains("root"));
/// assert!(s.contains("direct"));
/// ```
pub fn dependency_flags_to_string(flags: DependencyFlags) -> String {
    if flags.is_empty() {
        return "none".to_string();
    }

    let mut parts = Vec::new();

    if flags.contains(DependencyType::ROOT) {
        parts.push("root");
    }

    // Check for combined DIRECT first, then individual parts
    if flags.contains(DependencyType::DIRECT) {
        parts.push("direct");
    } else {
        if flags.contains(DependencyType::PURELY_DIRECT) {
            parts.push("purely-direct");
        }
        if flags.contains(DependencyType::PARTLY_DIRECT) {
            parts.push("partly-direct");
        }
    }

    if flags.contains(DependencyType::ANCESTRAL) {
        parts.push("ancestral");
    }

    if flags.contains(DependencyType::VIRTUAL) {
        parts.push("virtual");
    }

    if flags.contains(DependencyType::NON_VIRTUAL) {
        parts.push("non-virtual");
    }

    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_type_none() {
        let flags = DependencyType::NONE;
        assert!(flags.is_empty());
        assert_eq!(dependency_flags_to_string(flags), "none");
    }

    #[test]
    fn test_dependency_type_root() {
        let flags = DependencyType::ROOT;
        assert!(flags.contains(DependencyType::ROOT));
        assert!(!flags.contains(DependencyType::ANCESTRAL));
    }

    #[test]
    fn test_dependency_type_direct() {
        let flags = DependencyType::DIRECT;
        assert!(flags.contains(DependencyType::PURELY_DIRECT));
        assert!(flags.contains(DependencyType::PARTLY_DIRECT));
    }

    #[test]
    fn test_dependency_type_any_non_virtual() {
        let flags = DependencyType::ANY_NON_VIRTUAL;
        assert!(flags.contains(DependencyType::ROOT));
        assert!(flags.contains(DependencyType::DIRECT));
        assert!(flags.contains(DependencyType::ANCESTRAL));
        assert!(flags.contains(DependencyType::NON_VIRTUAL));
        assert!(!flags.contains(DependencyType::VIRTUAL));
    }

    #[test]
    fn test_dependency_type_any_including_virtual() {
        let flags = DependencyType::ANY_INCLUDING_VIRTUAL;
        assert!(flags.contains(DependencyType::ANY_NON_VIRTUAL));
        assert!(flags.contains(DependencyType::VIRTUAL));
    }

    #[test]
    fn test_dependency_type_combine() {
        let flags = DependencyType::ROOT | DependencyType::ANCESTRAL;
        assert!(flags.contains(DependencyType::ROOT));
        assert!(flags.contains(DependencyType::ANCESTRAL));
        assert!(!flags.contains(DependencyType::DIRECT));
    }

    #[test]
    fn test_dependency_flags_to_string() {
        assert_eq!(dependency_flags_to_string(DependencyType::NONE), "none");
        assert!(dependency_flags_to_string(DependencyType::ROOT).contains("root"));
        assert!(dependency_flags_to_string(DependencyType::DIRECT).contains("direct"));

        let combined = DependencyType::ROOT | DependencyType::ANCESTRAL;
        let s = dependency_flags_to_string(combined);
        assert!(s.contains("root"));
        assert!(s.contains("ancestral"));
    }

    #[test]
    fn test_dependency_new() {
        let index_path = Path::from_string("/World/Model").unwrap();
        let site_path = Path::from_string("/Model").unwrap();
        let map_func = MapFunction::identity().clone();

        let dep = Dependency::new(index_path.clone(), site_path.clone(), map_func);

        assert_eq!(dep.index_path, index_path);
        assert_eq!(dep.site_path, site_path);
    }

    #[test]
    fn test_dependency_default() {
        let dep = Dependency::default();
        assert!(dep.index_path.is_empty());
        assert!(dep.site_path.is_empty());
        assert!(dep.map_func.is_null());
    }

    #[test]
    fn test_dependency_equality() {
        let dep1 = Dependency::new(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
            MapFunction::identity().clone(),
        );

        let dep2 = Dependency::new(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
            MapFunction::identity().clone(),
        );

        let dep3 = Dependency::new(
            Path::from_string("/A").unwrap(),
            Path::from_string("/C").unwrap(),
            MapFunction::identity().clone(),
        );

        assert_eq!(dep1, dep2);
        assert_ne!(dep1, dep3);
    }

    #[test]
    fn test_dependency_type_display() {
        let flags = DependencyType::ROOT | DependencyType::DIRECT;
        let display = format!("{}", flags);
        assert!(display.contains("root"));
        assert!(display.contains("direct"));
    }

    // =========================================================================
    // Tests for classify_node_dependency / node_introduces_dependency
    // =========================================================================

    /// Invalid node -> NONE, does not introduce a dependency.
    #[test]
    fn test_classify_invalid_node_is_none() {
        use crate::NodeRef;
        let node = NodeRef::invalid();
        assert_eq!(classify_node_dependency(&node), DependencyType::NONE);
        assert!(!node_introduces_dependency(&node));
    }

    /// Root node (ArcType::Root) must classify as ROOT.
    #[test]
    fn test_classify_root_node_is_root() {
        use crate::{LayerStackIdentifier, PrimIndexGraph, Site};
        use usd_sdf::Path;

        let site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site, true);
        let root = graph.root_node();

        let flags = classify_node_dependency(&root);
        assert!(
            flags.contains(DependencyType::ROOT),
            "root node must have ROOT dependency flag"
        );
        assert!(
            node_introduces_dependency(&root),
            "root node introduces a dependency"
        );
    }

    /// Direct child of root (parent.arc_type = Root) -> PURELY_DIRECT.
    ///
    /// In C++ primIndex.cpp AddArc (line ~1986), a newly-added non-propagated
    /// node always gets SetHasTransitiveDirectDependency(true). Here we model
    /// that by calling set_has_transitive_direct_dependency(true) explicitly.
    #[test]
    fn test_classify_direct_child_of_root_is_purely_direct() {
        use crate::{
            Arc as PcpArc, ArcType, LayerStackIdentifier, MapFunction, PrimIndexGraph, Site,
        };
        use usd_sdf::Path;

        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();

        // Insert child at same namespace depth as root (same depth = purely direct)
        let child_site = Site::new(
            LayerStackIdentifier::new("ref.usda"),
            Path::from_string("/Model").unwrap(),
        );
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(root.node_index());
        arc.set_origin_index(root.node_index());
        arc.set_namespace_depth(root.namespace_depth());
        arc.set_map_to_parent(MapFunction::identity().clone());

        let child = root.insert_child(&child_site, &arc, None);
        assert!(child.is_valid(), "child node must be valid after insertion");

        // Simulate what primIndex.cpp AddArc does: mark as directly added
        child.set_has_transitive_direct_dependency(true);

        let flags = classify_node_dependency(&child);
        assert!(
            flags.contains(DependencyType::PURELY_DIRECT),
            "child of root at same namespace depth must be PURELY_DIRECT, got: {flags}"
        );
        assert!(node_introduces_dependency(&child));
    }

    /// Child with namespace_depth > parent's -> PARTLY_DIRECT.
    ///
    /// In C++ primIndex.cpp AddArc, a node introduced at an ancestral namespace
    /// gets both HasTransitiveDirectDependency(true) and
    /// HasTransitiveAncestralDependency(true), which makes it PARTLY_DIRECT.
    #[test]
    fn test_classify_deeper_child_is_partly_direct() {
        use crate::{
            Arc as PcpArc, ArcType, LayerStackIdentifier, MapFunction, PrimIndexGraph, Site,
        };
        use usd_sdf::Path;

        let root_site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(root_site, true);
        let root = graph.root_node();
        root.set_namespace_depth(1); // root at depth 1

        let child_site = Site::new(
            LayerStackIdentifier::new("ref.usda"),
            Path::from_string("/Model/Child").unwrap(),
        );
        let mut arc = PcpArc::new(ArcType::Reference);
        arc.set_parent_index(root.node_index());
        arc.set_origin_index(root.node_index());
        arc.set_namespace_depth(2); // deeper than parent's 1 -> partly direct
        arc.set_map_to_parent(MapFunction::identity().clone());

        let child = root.insert_child(&child_site, &arc, None);
        assert!(child.is_valid());

        // Simulate what primIndex.cpp AddArc does: both direct AND ancestral
        // because the node is introduced at a deeper namespace level
        child.set_has_transitive_direct_dependency(true);
        child.set_has_transitive_ancestral_dependency(true);

        let flags = classify_node_dependency(&child);
        assert!(
            flags.contains(DependencyType::PARTLY_DIRECT),
            "child with deeper namespace must be PARTLY_DIRECT, got: {flags}"
        );
    }

    /// DIRECT is the union of PURELY_DIRECT and PARTLY_DIRECT.
    #[test]
    fn test_direct_flag_covers_both() {
        assert!(DependencyType::DIRECT.contains(DependencyType::PURELY_DIRECT));
        assert!(DependencyType::DIRECT.contains(DependencyType::PARTLY_DIRECT));
        // PURELY_DIRECT alone does not imply PARTLY_DIRECT
        assert!(!DependencyType::PURELY_DIRECT.contains(DependencyType::PARTLY_DIRECT));
    }
}
