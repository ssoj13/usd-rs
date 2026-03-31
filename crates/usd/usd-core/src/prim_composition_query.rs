//! Prim Composition Query - query composition arcs for a prim.
//!
//! Port of pxr/usd/usd/primCompositionQuery.h/cpp
//!
//! UsdPrimCompositionQuery provides a way to query and filter composition arcs
//! for a prim. It creates a list of strength-ordered UsdPrimCompositionQueryArc
//! objects that can be filtered by various criteria.

use std::sync::Arc;

use super::prim::Prim;
use super::resolve_target::ResolveTarget;
use usd_pcp::{ArcType, NodeRef, PrimIndex, compose_site};
use usd_sdf::{LayerHandle, Path, Payload, Reference};

// ============================================================================
// ArcTypeFilter
// ============================================================================

/// Choices for filtering composition arcs based on arc type.
///
/// Matches C++ `UsdPrimCompositionQuery::ArcTypeFilter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ArcTypeFilter {
    /// All arc types.
    #[default]
    All = 0,
    /// Single arc types.
    /// Reference arc.
    Reference,
    /// Payload arc.
    Payload,
    /// Inherit arc.
    Inherit,
    /// Specialize arc.
    Specialize,
    /// Variant arc.
    Variant,
    /// Relocate arc.
    Relocate,
    /// Related arc types.
    /// Reference or payload arc.
    ReferenceOrPayload,
    /// Inherit or specialize arc.
    InheritOrSpecialize,
    /// Inverse of related arc types.
    /// Not reference or payload arc.
    NotReferenceOrPayload,
    /// Not inherit or specialize arc.
    NotInheritOrSpecialize,
    /// Not variant arc.
    NotVariant,
    /// Not relocate arc.
    NotRelocate,
}

impl ArcTypeFilter {
    /// Returns true if the given arc type matches this filter.
    pub fn matches(&self, arc_type: ArcType) -> bool {
        match self {
            ArcTypeFilter::All => true,
            ArcTypeFilter::Reference => arc_type == ArcType::Reference,
            ArcTypeFilter::Payload => arc_type == ArcType::Payload,
            ArcTypeFilter::Inherit => arc_type == ArcType::Inherit,
            ArcTypeFilter::Specialize => arc_type == ArcType::Specialize,
            ArcTypeFilter::Variant => arc_type == ArcType::Variant,
            ArcTypeFilter::Relocate => arc_type == ArcType::Relocate,
            ArcTypeFilter::ReferenceOrPayload => {
                arc_type == ArcType::Reference || arc_type == ArcType::Payload
            }
            ArcTypeFilter::InheritOrSpecialize => {
                arc_type == ArcType::Inherit || arc_type == ArcType::Specialize
            }
            ArcTypeFilter::NotReferenceOrPayload => {
                arc_type != ArcType::Reference && arc_type != ArcType::Payload
            }
            ArcTypeFilter::NotInheritOrSpecialize => {
                arc_type != ArcType::Inherit && arc_type != ArcType::Specialize
            }
            ArcTypeFilter::NotVariant => arc_type != ArcType::Variant,
            ArcTypeFilter::NotRelocate => arc_type != ArcType::Relocate,
        }
    }
}

// ============================================================================
// DependencyTypeFilter
// ============================================================================

/// Choices for filtering composition arcs on dependency type.
///
/// Matches C++ `UsdPrimCompositionQuery::DependencyTypeFilter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DependencyTypeFilter {
    /// All dependency types.
    #[default]
    All = 0,
    /// Direct - arc introduced at the prim's level in namespace.
    Direct,
    /// Ancestral - arc introduced by a namespace parent of the prim.
    Ancestral,
}

// ============================================================================
// ArcIntroducedFilter
// ============================================================================

/// Choices for filtering composition arcs based on where the arc is introduced.
///
/// Matches C++ `UsdPrimCompositionQuery::ArcIntroducedFilter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ArcIntroducedFilter {
    /// All arcs.
    #[default]
    All = 0,
    /// Arcs that are authored somewhere in the root layer stack.
    IntroducedInRootLayerStack,
    /// Arcs that are authored directly in the prim's prim spec in the root layer stack.
    IntroducedInRootLayerPrimSpec,
}

// ============================================================================
// HasSpecsFilter
// ============================================================================

/// Choices for filtering composition arcs on whether the node contributes specs.
///
/// Matches C++ `UsdPrimCompositionQuery::HasSpecsFilter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HasSpecsFilter {
    /// All arcs.
    #[default]
    All = 0,
    /// Arcs that have specs.
    HasSpecs,
    /// Arcs that have no specs.
    HasNoSpecs,
}

// ============================================================================
// Filter
// ============================================================================

/// Aggregate filter for filtering composition arcs by the previously defined criteria.
///
/// Matches C++ `UsdPrimCompositionQuery::Filter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    /// Filters by arc type.
    pub arc_type_filter: ArcTypeFilter,
    /// Filters by dependency type, direct or ancestral.
    pub dependency_type_filter: DependencyTypeFilter,
    /// Filters by where the arc is introduced.
    pub arc_introduced_filter: ArcIntroducedFilter,
    /// Filters by whether the arc provides specs for the prim.
    pub has_specs_filter: HasSpecsFilter,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            arc_type_filter: ArcTypeFilter::All,
            dependency_type_filter: DependencyTypeFilter::All,
            arc_introduced_filter: ArcIntroducedFilter::All,
            has_specs_filter: HasSpecsFilter::All,
        }
    }
}

// ============================================================================
// PrimCompositionQueryArc
// ============================================================================

/// Represents a composition arc that is returned by a PrimCompositionQuery.
///
/// Matches C++ `UsdPrimCompositionQueryArc`.
#[derive(Clone)]
pub struct PrimCompositionQueryArc {
    /// The target node of this composition arc.
    target_node: NodeRef,
    /// The originally introduced node (before following origin chain).
    original_introduced_node: NodeRef,
    /// The node that introduces this arc into composition graph.
    introducing_node: NodeRef,
    /// The prim index (held to keep nodes valid).
    prim_index: Option<Arc<PrimIndex>>,
}

impl PrimCompositionQueryArc {
    /// Creates a new composition query arc from a node.
    ///
    /// Matches C++ private constructor `UsdPrimCompositionQueryArc(const PcpNodeRef &node)`.
    pub(crate) fn new(node: NodeRef) -> Self {
        // C++: _originalIntroducedNode starts as _node, then may be overridden.
        let mut original_introduced_node = node.clone();

        let introducing_node = if node.is_root_node() {
            // Root node introduces itself.
            node.clone()
        } else {
            let origin_node = node.origin_node();
            let parent_node = node.parent_node();
            if origin_node != parent_node {
                // Non-parent origin: this is an implicit/copied node.
                // C++: _originalIntroducedNode = _node.GetOriginRootNode()
                //      _introducingNode = _originalIntroducedNode.GetParentNode()
                // Mirrors C++ exactly -- no validity guard. If origin_root is
                // invalid, its parent_node() returns an invalid NodeRef, which
                // is the same result C++ produces.
                let origin_root = node.origin_root_node();
                original_introduced_node = origin_root.clone();
                origin_root.parent_node()
            } else {
                // Normal case: arc introduced by parent node.
                // _introducingNode = _originalIntroducedNode.GetParentNode()
                original_introduced_node.parent_node()
            }
        };

        Self {
            target_node: node,
            original_introduced_node,
            introducing_node,
            prim_index: None, // Set by query after construction.
        }
    }

    /// Sets the prim index (for keeping nodes valid).
    pub(crate) fn set_prim_index(&mut self, prim_index: Arc<PrimIndex>) {
        self.prim_index = Some(prim_index);
    }

    /// Returns the targeted node of this composition arc.
    ///
    /// Matches C++ `GetTargetNode()`.
    pub fn target_node(&self) -> &NodeRef {
        &self.target_node
    }

    /// Returns the node that introduces this arc into composition graph.
    ///
    /// Matches C++ `GetIntroducingNode()`.
    pub fn introducing_node(&self) -> &NodeRef {
        &self.introducing_node
    }

    /// Returns the root layer of the layer stack that holds the prim spec
    /// targeted by this composition arc.
    ///
    /// Matches C++ `GetTargetLayer()`.
    pub fn target_layer(&self) -> Option<LayerHandle> {
        self.target_node
            .layer_stack()
            .and_then(|ls| ls.root_layer().map(|l| LayerHandle::from_layer(&l)))
    }

    /// Returns the path of the prim spec that is targeted by this composition
    /// arc in the target layer stack.
    ///
    /// Matches C++ `GetTargetPrimPath()`.
    pub fn target_prim_path(&self) -> Path {
        self.target_node.path()
    }

    /// Creates and returns a resolve target that, when passed to a
    /// UsdAttributeQuery for one of this prim's attributes, causes value
    /// resolution to only consider node sites weaker than this arc, up to and
    /// including this arc's site itself.
    ///
    /// Matches C++ `MakeResolveTargetUpTo()`.
    pub fn make_resolve_target_up_to(&self, sub_layer: Option<LayerHandle>) -> ResolveTarget {
        if let Some(sub_layer) = sub_layer {
            if let Some(layer_stack) = self.target_node.layer_stack() {
                // Check if layer is in layer stack
                let has_layer = layer_stack.get_layers().iter().any(|l| {
                    let l_handle = LayerHandle::from_layer(l);
                    l_handle == sub_layer
                });
                if has_layer {
                    if let Some(prim_index) = &self.prim_index {
                        return ResolveTarget::with_start(
                            prim_index.clone(),
                            self.target_node.clone(),
                            sub_layer,
                        );
                    }
                }
            }
        }
        if let Some(prim_index) = &self.prim_index {
            ResolveTarget::with_start(
                prim_index.clone(),
                self.target_node.clone(),
                LayerHandle::default(), // nullptr equivalent
            )
        } else {
            ResolveTarget::new()
        }
    }

    /// Creates and returns a resolve target that, when passed to a
    /// UsdAttributeQuery for one of this prim's attributes, causes value
    /// resolution to only consider node sites stronger than this arc, not
    /// including this arc itself (unless sub_layer is provided).
    ///
    /// Matches C++ `MakeResolveTargetStrongerThan()`.
    pub fn make_resolve_target_stronger_than(
        &self,
        sub_layer: Option<LayerHandle>,
    ) -> ResolveTarget {
        // root_node() always returns a valid NodeRef (or invalid if graph is None)
        let root_node = self.target_node.root_node();

        if let Some(sub_layer) = sub_layer {
            if let Some(layer_stack) = self.target_node.layer_stack() {
                // Check if layer is in layer stack
                let has_layer = layer_stack.get_layers().iter().any(|l| {
                    let l_handle = LayerHandle::from_layer(l);
                    l_handle == sub_layer
                });
                if has_layer {
                    if let Some(prim_index) = &self.prim_index {
                        // For nullptr equivalent, use root layer
                        let start_layer = self
                            .target_node
                            .layer_stack()
                            .and_then(|ls| ls.root_layer())
                            .map(|l| LayerHandle::from_layer(&l))
                            .unwrap_or_default();
                        return ResolveTarget::with_start_and_stop(
                            prim_index.clone(),
                            root_node,
                            start_layer,
                            self.target_node.clone(),
                            sub_layer,
                        );
                    }
                }
            }
        }

        if let Some(prim_index) = &self.prim_index {
            // For nullptr equivalent, use root layer
            let start_layer = self
                .target_node
                .layer_stack()
                .and_then(|ls| ls.root_layer())
                .map(|l| LayerHandle::from_layer(&l))
                .unwrap_or_default();
            let stop_layer = LayerHandle::default();
            ResolveTarget::with_start_and_stop(
                prim_index.clone(),
                root_node,
                start_layer,
                self.target_node.clone(),
                stop_layer,
            )
        } else {
            ResolveTarget::new()
        }
    }

    /// Returns the specific layer in the layer stack that adds this arc to the
    /// composition graph.
    ///
    /// Matches C++ `GetIntroducingLayer()`.
    ///
    /// Returns the layer that introduces this composition arc.
    /// For root arcs, returns None. For other arcs, returns the layer where
    /// the arc is authored.
    pub fn introducing_layer(&self) -> Option<LayerHandle> {
        // Root arcs don't have an introducing layer
        if self.target_node.is_root_node() {
            return None;
        }

        let Some(layer_stack) = self.introducing_node.layer_stack() else {
            return None;
        };

        let introducing_path = self.introducing_prim_path();
        let sibling_num = self.target_node.sibling_num_at_origin();
        let arc_type = self.target_node.arc_type();

        // Get arc info based on arc type
        let arc_info_opt = match arc_type {
            ArcType::Reference => {
                let (_, info, _) =
                    compose_site::compose_site_references(&layer_stack, &introducing_path);
                if sibling_num >= 0 && (sibling_num as usize) < info.len() {
                    Some(info[sibling_num as usize].clone())
                } else {
                    None
                }
            }
            ArcType::Payload => {
                let (_, info, _) =
                    compose_site::compose_site_payloads(&layer_stack, &introducing_path);
                if sibling_num >= 0 && (sibling_num as usize) < info.len() {
                    Some(info[sibling_num as usize].clone())
                } else {
                    None
                }
            }
            ArcType::Inherit => {
                let (_, info) =
                    compose_site::compose_site_inherits_with_info(&layer_stack, &introducing_path);
                if sibling_num >= 0 && (sibling_num as usize) < info.len() {
                    Some(info[sibling_num as usize].clone())
                } else {
                    None
                }
            }
            ArcType::Specialize => {
                let (_, info) = compose_site::compose_site_specializes_with_info(
                    &layer_stack,
                    &introducing_path,
                );
                if sibling_num >= 0 && (sibling_num as usize) < info.len() {
                    Some(info[sibling_num as usize].clone())
                } else {
                    None
                }
            }
            ArcType::Variant => {
                let (_, info) = compose_site::compose_site_variant_sets_with_info(
                    &layer_stack,
                    &introducing_path,
                );
                if sibling_num >= 0 && (sibling_num as usize) < info.len() {
                    Some(info[sibling_num as usize].clone())
                } else {
                    None
                }
            }
            ArcType::Relocate => {
                // Special handling for relocate arcs - search through relocates
                let intro_path = self.target_node.intro_path();
                let layers = layer_stack.get_layers();
                for layer in layers.iter() {
                    let relocates = layer.get_relocates();
                    for (_, target_path) in relocates.iter() {
                        if *target_path == intro_path {
                            return Some(LayerHandle::from_layer(layer));
                        }
                    }
                }
                None
            }
            _ => None,
        };

        arc_info_opt.and_then(|info| info.source_layer.as_ref().map(LayerHandle::from_layer))
    }

    /// Returns the path of the prim that introduces this arc to the composition
    /// graph within the layer in which the composition opinion is authored.
    ///
    /// Matches C++ `GetIntroducingPrimPath()`.
    pub fn introducing_prim_path(&self) -> Path {
        // Special case for the root node
        if self.target_node.is_root_node() {
            return Path::default();
        }
        // Special case for relocate arcs - they are authored at layer metadata
        if self.target_node.arc_type() == ArcType::Relocate {
            return Path::absolute_root();
        }
        // Return the intro path of the originally introduced node
        self.original_introduced_node.intro_path()
    }

    /// Returns the arc type.
    ///
    /// Matches C++ `GetArcType()`.
    pub fn arc_type(&self) -> ArcType {
        self.target_node.arc_type()
    }

    /// Returns whether this arc was implicitly added to this prim.
    ///
    /// Matches C++ `IsImplicit()`:
    /// `!node.IsRootNode() && node.GetParentNode() != _introducingNode && node.GetOriginNode().GetSite() != node.GetSite()`
    pub fn is_implicit(&self) -> bool {
        // Exact C++ translation -- no validity guard on origin.
        // An invalid origin's site() is a default/empty SdfSite;
        // since a real node's site is never empty the inequality still
        // produces the correct result.
        if self.target_node.is_root_node() {
            return false;
        }
        if self.target_node.parent_node() == self.introducing_node {
            return false;
        }
        // origin.site != node.site distinguishes implicit from copied nodes.
        let origin = self.target_node.origin_node();
        origin.site() != self.target_node.site()
    }

    /// Returns whether this arc is ancestral, i.e. it exists because it was
    /// composed in by a namespace parent's prim index.
    ///
    /// Matches C++ `IsAncestral()`: `return _node.IsDueToAncestor()`.
    pub fn is_ancestral(&self) -> bool {
        self.target_node.is_due_to_ancestor()
    }

    /// Returns whether the target node of this arc contributes any local spec
    /// opinions that are composed for the prim.
    ///
    /// Matches C++ `HasSpecs()`.
    pub fn has_specs(&self) -> bool {
        // Check if the target node has specs
        self.target_node.has_specs()
    }

    /// Returns whether the composition opinion that introduces this arc
    /// is authored in the root layer stack.
    ///
    /// Matches C++ `IsIntroducedInRootLayerStack()`:
    /// root arc always true; otherwise compare introducing node root layer vs
    /// `node.GetRootNode().GetLayerStack()->GetIdentifier().rootLayer`.
    pub fn is_introduced_in_root_layer_stack(&self) -> bool {
        // Root arc is always in the root layer stack
        if self.target_node.is_root_node() {
            return true;
        }

        // C++: compare root layers of introducing_node layer stack vs root_node layer stack.
        // We use identifier().root_layer (AssetPath) for comparison to handle session-layer mismatch.
        let root_node = self.target_node.root_node();
        let introducing_root_layer = self
            .introducing_node
            .layer_stack()
            .and_then(|ls| Some(ls.identifier().root_layer.clone()));
        let stage_root_layer = root_node
            .layer_stack()
            .and_then(|ls| Some(ls.identifier().root_layer.clone()));

        match (introducing_root_layer, stage_root_layer) {
            (Some(a), Some(b)) => a == b,
            // If either layer stack is unavailable, fall back to node identity check
            _ => self.introducing_node.is_root_node(),
        }
    }

    /// Returns whether the composition opinion that introduces this arc is
    /// authored directly on the prim's prim spec within the root layer stack.
    ///
    /// Matches C++ `IsIntroducedInRootLayerPrimSpec()`: `return _introducingNode.IsRootNode()`.
    pub fn is_introduced_in_root_layer_prim_spec(&self) -> bool {
        self.introducing_node.is_root_node()
    }

    /// Returns the introduced SdfReference for Reference arcs.
    ///
    /// Returns None if this arc is not a Reference arc or no info is found.
    pub fn get_introducing_reference(&self) -> Option<Reference> {
        if self.target_node.arc_type() != ArcType::Reference {
            return None;
        }
        let layer_stack = self.introducing_node.layer_stack()?;
        let introducing_path = self.introducing_prim_path();
        let sibling_num = self.target_node.sibling_num_at_origin();
        let (refs, _, _) = compose_site::compose_site_references(&layer_stack, &introducing_path);
        if sibling_num >= 0 && (sibling_num as usize) < refs.len() {
            Some(refs[sibling_num as usize].clone())
        } else {
            None
        }
    }

    /// Returns the introduced SdfPayload for Payload arcs.
    ///
    /// Returns None if this arc is not a Payload arc or no info is found.
    pub fn get_introducing_payload(&self) -> Option<Payload> {
        if self.target_node.arc_type() != ArcType::Payload {
            return None;
        }
        let layer_stack = self.introducing_node.layer_stack()?;
        let introducing_path = self.introducing_prim_path();
        let sibling_num = self.target_node.sibling_num_at_origin();
        let (payloads, _, _) = compose_site::compose_site_payloads(&layer_stack, &introducing_path);
        if sibling_num >= 0 && (sibling_num as usize) < payloads.len() {
            Some(payloads[sibling_num as usize].clone())
        } else {
            None
        }
    }

    /// Returns the introduced SdfPath for Inherit or Specialize arcs.
    ///
    /// Returns None if this arc is not an Inherit/Specialize arc or no info is found.
    pub fn get_introducing_path(&self) -> Option<Path> {
        let arc_type = self.target_node.arc_type();
        if arc_type != ArcType::Inherit && arc_type != ArcType::Specialize {
            return None;
        }
        let layer_stack = self.introducing_node.layer_stack()?;
        let introducing_path = self.introducing_prim_path();
        let sibling_num = self.target_node.sibling_num_at_origin();
        let (paths, _) = if arc_type == ArcType::Inherit {
            compose_site::compose_site_inherits_with_info(&layer_stack, &introducing_path)
        } else {
            compose_site::compose_site_specializes_with_info(&layer_stack, &introducing_path)
        };
        if sibling_num >= 0 && (sibling_num as usize) < paths.len() {
            Some(paths[sibling_num as usize].clone())
        } else {
            None
        }
    }

    /// Returns the introduced variant set name for Variant arcs.
    ///
    /// Returns None if this arc is not a Variant arc or no info is found.
    pub fn get_introducing_variant_name(&self) -> Option<String> {
        if self.target_node.arc_type() != ArcType::Variant {
            return None;
        }
        let layer_stack = self.introducing_node.layer_stack()?;
        let introducing_path = self.introducing_prim_path();
        let sibling_num = self.target_node.sibling_num_at_origin();
        let (names, _) =
            compose_site::compose_site_variant_sets_with_info(&layer_stack, &introducing_path);
        if sibling_num >= 0 && (sibling_num as usize) < names.len() {
            Some(names[sibling_num as usize].clone())
        } else {
            None
        }
    }
}

// ============================================================================
// PrimCompositionQuery
// ============================================================================

/// Object for making optionally filtered composition queries about a prim.
///
/// Matches C++ `UsdPrimCompositionQuery`.
#[derive(Clone)]
pub struct PrimCompositionQuery {
    /// The prim to query.
    prim: Prim,
    /// The filter to apply.
    filter: Filter,
    /// The expanded prim index (held to keep nodes valid).
    expanded_prim_index: Option<Arc<PrimIndex>>,
    /// Unfiltered arcs (cached).
    unfiltered_arcs: Vec<PrimCompositionQueryArc>,
}

impl PrimCompositionQuery {
    /// Returns a prim composition query for the given prim with a preset
    /// filter that only returns reference arcs that are not ancestral.
    ///
    /// Matches C++ `GetDirectReferences()`.
    pub fn get_direct_references(prim: Prim) -> Self {
        let mut filter = Filter::default();
        filter.arc_type_filter = ArcTypeFilter::Reference;
        filter.dependency_type_filter = DependencyTypeFilter::Direct;
        Self::new(prim, filter)
    }

    /// Returns a prim composition query for the given prim with a preset
    /// filter that only returns inherit arcs that are not ancestral.
    ///
    /// Matches C++ `GetDirectInherits()`.
    pub fn get_direct_inherits(prim: Prim) -> Self {
        let mut filter = Filter::default();
        filter.arc_type_filter = ArcTypeFilter::Inherit;
        filter.dependency_type_filter = DependencyTypeFilter::Direct;
        Self::new(prim, filter)
    }

    /// Returns a prim composition query for the given prim with a preset
    /// filter that only returns direct arcs that were introduced by opinions
    /// defined in a layer in the root layer stack.
    ///
    /// Matches C++ `GetDirectRootLayerArcs()`.
    pub fn get_direct_root_layer_arcs(prim: Prim) -> Self {
        let mut filter = Filter::default();
        filter.dependency_type_filter = DependencyTypeFilter::Direct;
        filter.arc_introduced_filter = ArcIntroducedFilter::IntroducedInRootLayerStack;
        Self::new(prim, filter)
    }

    /// Create a prim composition query for the prim with the given filter.
    ///
    /// Computes arcs once at construction and caches them.
    /// Matches C++ `UsdPrimCompositionQuery(const UsdPrim &prim, const Filter &filter)`.
    pub fn new(prim: Prim, filter: Filter) -> Self {
        let mut query = Self {
            prim,
            filter,
            expanded_prim_index: None,
            unfiltered_arcs: Vec::new(),
        };
        query.compute_arcs();
        query
    }

    /// Change the filter for this query.
    ///
    /// Matches C++ `SetFilter()`.
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
    }

    /// Return a copy of the current filter parameters.
    ///
    /// Matches C++ `GetFilter()`.
    pub fn filter(&self) -> &Filter {
        &self.filter
    }

    /// Return a copy of the current filter parameters (C++ API name).
    ///
    /// Alias for `filter()`. Matches C++ `GetFilter()`.
    pub fn get_filter(&self) -> Filter {
        self.filter.clone()
    }

    /// Compute and cache all unfiltered arcs from the prim index.
    /// Called once at construction.
    fn compute_arcs(&mut self) {
        // C++: uses ComputeExpandedPrimIndex() so all composition arcs are visible
        // even if they don't currently contribute opinions.
        let expanded_index = match self.prim.compute_expanded_prim_index() {
            Some(idx) => idx,
            None => return,
        };

        if !expanded_index.is_valid() {
            return;
        }

        let prim_index_arc = Arc::new(expanded_index);
        self.expanded_prim_index = Some(prim_index_arc.clone());

        // C++: skip inert nodes EXCEPT relocate arcs
        // "We still skip inert nodes in the unfiltered query, with the exception
        // of relocates, to avoid picking up things like the original copies of
        // specialize nodes that have been moved for strength ordering purposes."
        let mut arcs = Vec::new();
        for node in prim_index_arc.nodes() {
            if !node.is_valid() {
                continue;
            }
            // Include non-inert nodes + relocate arcs (even if inert)
            if !node.is_inert() || node.arc_type() == ArcType::Relocate {
                let mut arc = PrimCompositionQueryArc::new(node);
                arc.set_prim_index(prim_index_arc.clone());
                arcs.push(arc);
            }
        }

        self.unfiltered_arcs = arcs;
    }

    /// Return a list of composition arcs for this query's prim using the
    /// current query filter. The composition arcs are always returned in order
    /// from strongest to weakest regardless of the filter.
    ///
    /// Uses cached arcs computed at construction; only applies filtering.
    /// Matches C++ `GetCompositionArcs()`.
    pub fn get_composition_arcs(&self) -> Vec<PrimCompositionQueryArc> {
        self.unfiltered_arcs
            .iter()
            .filter(|arc| {
                // Arc type filter
                if !self.filter.arc_type_filter.matches(arc.arc_type()) {
                    return false;
                }
                // Dependency type filter
                match self.filter.dependency_type_filter {
                    DependencyTypeFilter::Direct => {
                        if arc.is_ancestral() {
                            return false;
                        }
                    }
                    DependencyTypeFilter::Ancestral => {
                        if !arc.is_ancestral() {
                            return false;
                        }
                    }
                    DependencyTypeFilter::All => {}
                }
                // Has specs filter
                match self.filter.has_specs_filter {
                    HasSpecsFilter::HasSpecs => {
                        if !arc.has_specs() {
                            return false;
                        }
                    }
                    HasSpecsFilter::HasNoSpecs => {
                        if arc.has_specs() {
                            return false;
                        }
                    }
                    HasSpecsFilter::All => {}
                }
                // Arc introduced filter
                match self.filter.arc_introduced_filter {
                    ArcIntroducedFilter::IntroducedInRootLayerStack => {
                        if !arc.is_introduced_in_root_layer_stack() {
                            return false;
                        }
                    }
                    ArcIntroducedFilter::IntroducedInRootLayerPrimSpec => {
                        if !arc.is_introduced_in_root_layer_prim_spec() {
                            return false;
                        }
                    }
                    ArcIntroducedFilter::All => {}
                }
                true
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::InitialLoadSet;
    use crate::stage::Stage;

    #[test]
    fn test_filter_default_values() {
        let f = Filter::default();
        assert_eq!(f.arc_type_filter, ArcTypeFilter::All);
        assert_eq!(f.dependency_type_filter, DependencyTypeFilter::All);
        assert_eq!(f.arc_introduced_filter, ArcIntroducedFilter::All);
        assert_eq!(f.has_specs_filter, HasSpecsFilter::All);
    }

    #[test]
    fn test_get_filter_matches_construction_filter() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/F", "Xform").unwrap();
        let mut filter = Filter::default();
        filter.arc_type_filter = ArcTypeFilter::Reference;
        filter.dependency_type_filter = DependencyTypeFilter::Direct;
        let q = PrimCompositionQuery::new(prim, filter.clone());
        assert_eq!(q.get_filter(), filter);
    }

    #[test]
    fn test_set_filter_updates_query() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/SetF", "Xform").unwrap();
        let mut q = PrimCompositionQuery::new(prim, Filter::default());
        let mut new_f = Filter::default();
        new_f.arc_type_filter = ArcTypeFilter::Inherit;
        q.set_filter(new_f.clone());
        assert_eq!(*q.filter(), new_f);
    }

    #[test]
    fn test_arc_type_filter_all_matches_every_type() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::All;
        for t in [
            ArcType::Root,
            ArcType::Reference,
            ArcType::Payload,
            ArcType::Inherit,
            ArcType::Specialize,
            ArcType::Variant,
            ArcType::Relocate,
        ] {
            assert!(f.matches(t));
        }
    }

    #[test]
    fn test_arc_type_filter_reference_only() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::Reference;
        assert!(f.matches(ArcType::Reference));
        assert!(!f.matches(ArcType::Payload));
        assert!(!f.matches(ArcType::Inherit));
    }

    #[test]
    fn test_arc_type_filter_reference_or_payload() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::ReferenceOrPayload;
        assert!(f.matches(ArcType::Reference));
        assert!(f.matches(ArcType::Payload));
        assert!(!f.matches(ArcType::Inherit));
    }

    #[test]
    fn test_arc_type_filter_not_reference_or_payload() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::NotReferenceOrPayload;
        assert!(!f.matches(ArcType::Reference));
        assert!(!f.matches(ArcType::Payload));
        assert!(f.matches(ArcType::Inherit));
    }

    #[test]
    fn test_arc_type_filter_inherit_or_specialize() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::InheritOrSpecialize;
        assert!(f.matches(ArcType::Inherit));
        assert!(f.matches(ArcType::Specialize));
        assert!(!f.matches(ArcType::Reference));
    }

    #[test]
    fn test_arc_type_filter_not_inherit_or_specialize() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::NotInheritOrSpecialize;
        assert!(f.matches(ArcType::Reference));
        assert!(!f.matches(ArcType::Inherit));
    }

    #[test]
    fn test_arc_type_filter_not_variant() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::NotVariant;
        assert!(f.matches(ArcType::Reference));
        assert!(!f.matches(ArcType::Variant));
    }

    #[test]
    fn test_arc_type_filter_not_relocate() {
        use usd_pcp::ArcType;
        let f = ArcTypeFilter::NotRelocate;
        assert!(f.matches(ArcType::Inherit));
        assert!(!f.matches(ArcType::Relocate));
    }

    #[test]
    fn test_get_direct_references_factory_filter() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Ref", "Xform").unwrap();
        let q = PrimCompositionQuery::get_direct_references(prim);
        assert_eq!(q.filter().arc_type_filter, ArcTypeFilter::Reference);
        assert_eq!(
            q.filter().dependency_type_filter,
            DependencyTypeFilter::Direct
        );
    }

    #[test]
    fn test_get_direct_inherits_factory_filter() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Inh", "Xform").unwrap();
        let q = PrimCompositionQuery::get_direct_inherits(prim);
        assert_eq!(q.filter().arc_type_filter, ArcTypeFilter::Inherit);
        assert_eq!(
            q.filter().dependency_type_filter,
            DependencyTypeFilter::Direct
        );
    }

    #[test]
    fn test_get_direct_root_layer_arcs_factory_filter() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Root", "Xform").unwrap();
        let q = PrimCompositionQuery::get_direct_root_layer_arcs(prim);
        assert_eq!(
            q.filter().dependency_type_filter,
            DependencyTypeFilter::Direct
        );
        assert_eq!(
            q.filter().arc_introduced_filter,
            ArcIntroducedFilter::IntroducedInRootLayerStack
        );
    }

    #[test]
    fn test_get_composition_arcs_does_not_panic() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Simple", "Xform").unwrap();
        let q = PrimCompositionQuery::new(prim, Filter::default());
        let _arcs = q.get_composition_arcs();
    }

    #[test]
    fn test_reference_filter_only_returns_reference_arcs() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/FRef", "Xform").unwrap();
        let mut filter = Filter::default();
        filter.arc_type_filter = ArcTypeFilter::Reference;
        let q = PrimCompositionQuery::new(prim, filter);
        for arc in q.get_composition_arcs() {
            assert_eq!(
                arc.arc_type(),
                usd_pcp::ArcType::Reference,
                "non-reference arc leaked through Reference filter"
            );
        }
    }

    #[test]
    fn test_get_introducing_reference_none_for_non_reference_arc() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/NR", "Xform").unwrap();
        let q = PrimCompositionQuery::new(prim, Filter::default());
        for arc in q.get_composition_arcs() {
            if arc.arc_type() != usd_pcp::ArcType::Reference {
                assert!(arc.get_introducing_reference().is_none());
            }
        }
    }

    #[test]
    fn test_get_introducing_payload_none_for_non_payload_arc() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/NP", "Xform").unwrap();
        let q = PrimCompositionQuery::new(prim, Filter::default());
        for arc in q.get_composition_arcs() {
            if arc.arc_type() != usd_pcp::ArcType::Payload {
                assert!(arc.get_introducing_payload().is_none());
            }
        }
    }

    #[test]
    fn test_get_introducing_path_none_for_non_inherit_specialize() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/NIS", "Xform").unwrap();
        let q = PrimCompositionQuery::new(prim, Filter::default());
        for arc in q.get_composition_arcs() {
            let t = arc.arc_type();
            if t != usd_pcp::ArcType::Inherit && t != usd_pcp::ArcType::Specialize {
                assert!(arc.get_introducing_path().is_none());
            }
        }
    }

    #[test]
    fn test_get_introducing_variant_name_none_for_non_variant() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/NV", "Xform").unwrap();
        let q = PrimCompositionQuery::new(prim, Filter::default());
        for arc in q.get_composition_arcs() {
            if arc.arc_type() != usd_pcp::ArcType::Variant {
                assert!(arc.get_introducing_variant_name().is_none());
            }
        }
    }
}
