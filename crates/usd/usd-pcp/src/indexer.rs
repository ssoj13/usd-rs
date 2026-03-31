//! PCP Prim Indexer - composition task processor.
//!
//! The indexer manages the task-based composition process, evaluating
//! nodes in LIVRPS order and building the composition graph.
//!
//! # C++ Parity
//!
//! This is a port of the Pcp_PrimIndexer class from `pxr/usd/pcp/primIndex.cpp`.
//!
//! # Task-Based Composition
//!
//! Composition is driven by a task queue. Tasks are processed in priority order
//! (LIVRPS) to ensure correct composition semantics. Each task evaluation may
//! add new tasks to the queue.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

use std::collections::BTreeMap;

use crate::{
    ArcInfo, ErrorType, LayerStackRefPtr, MapExpression, MapFunction, NodeRef, PrimIndexGraph,
    Site, VariantFallbackMap, compare_node_strength, compose_site_has_specs,
    compose_site_has_value_clips, compose_site_inherits, compose_site_payloads,
    compose_site_references, compose_site_specializes, compose_site_variant_selection,
    compose_site_variant_set_options, compose_site_variant_sets,
};
use usd_sdf::{Layer, LayerOffset, Path, Payload, Reference};
use usd_tf::Token;

/// Counts non-variant path elements (namespace depth).
///
/// Equivalent to `path.StripAllVariantSelections().GetPathElementCount()`.
/// Matches C++ `PcpNode_GetNonVariantPathElementCount` (node.cpp:472):
/// walks up stripping variant selection nodes and subtracting from total count.
fn count_namespace_depth(path: &Path) -> i32 {
    // Fast path: if no variant selections, just return element count directly.
    if !path.contains_prim_variant_selection() {
        return path.get_path_element_count() as i32;
    }
    // Strip all variant selections then count — matches C++ semantics exactly.
    path.strip_all_variant_selections().get_path_element_count() as i32
}

/// Creates a `MapExpression` for an arc mapping `source` path to `target` path.
///
/// C++ `_CreateMapExpressionForArc` in primIndex.cpp:660-680.
/// Returns a constant expression wrapping a single-pair `MapFunction`.
/// Callers add `.add_root_identity()` for class-based arcs (inherit/specialize)
/// to allow paths outside the direct source→target pair to map through.
fn create_arc_map_expression(source: &Path, target: &Path, offset: LayerOffset) -> MapExpression {
    let mut path_map: BTreeMap<Path, Path> = BTreeMap::new();
    path_map.insert(source.clone(), target.clone());
    let map_func =
        MapFunction::create(path_map, offset).unwrap_or_else(|| MapFunction::identity().clone());
    MapExpression::constant(map_func)
}

/// Applies the strongest matching variant-selected ancestor prefix from `parent`
/// onto an authored absolute target path.
///
/// Internal arcs and class-based arcs are authored in root namespace, but when
/// they appear under a selected variant they must target the corresponding site
/// inside that variant namespace.
fn translate_target_path_into_parent_variant_namespace(
    parent: &NodeRef,
    target_path: &Path,
) -> Path {
    if !target_path.is_absolute_path() || !parent.path().contains_prim_variant_selection() {
        return target_path.clone();
    }

    let parent_path = parent.path();
    let parent_path_text = parent_path.as_str();
    let target_path_text = target_path.as_str();
    let mut variant_prefixes = Vec::new();
    for (idx, ch) in parent_path_text.char_indices() {
        if ch != '}' {
            continue;
        }
        if let Some(prefix) = Path::from_string(&parent_path_text[..=idx]) {
            if prefix.contains_prim_variant_selection() {
                variant_prefixes.push(prefix);
            }
        }
    }

    for prefix in variant_prefixes.into_iter().rev() {
        let stripped = prefix.strip_all_variant_selections();
        let stripped_text = stripped.as_str();
        if let Some(mut suffix) = target_path_text.strip_prefix(stripped_text) {
            if suffix.starts_with('/') {
                suffix = &suffix[1..];
            }
            let translated_text = format!("{}{}", prefix.as_str(), suffix);
            if let Some(translated) = Path::from_string(&translated_text) {
                return translated;
            }
        }
    }

    let mut ancestor = parent.path();
    while !ancestor.is_empty() {
        if ancestor.contains_prim_variant_selection() {
            let stripped = ancestor.strip_all_variant_selections();
            if let Some(translated) = target_path.replace_prefix(&stripped, &ancestor) {
                return translated;
            }
        }
        ancestor = ancestor.get_parent_path();
    }

    target_path.clone()
}

fn target_path_requires_ancestral_opinions(target_path: &Path) -> bool {
    if !target_path.contains_prim_variant_selection() {
        return !target_path.is_root_prim_path();
    }

    target_path
        .strip_all_variant_selections()
        .get_path_element_count()
        > 2
}

/// Mark an entire subtree of nodes as inert.
/// C++ primIndex.cpp:1528-1537 `_InertSubtree`.
fn inert_subtree(node: &NodeRef) {
    node.set_inert(true);
    for child in node.children() {
        inert_subtree(&child);
    }
}

/// Elide a subtree: marks node + descendants as inert with restricted depth.
/// C++ primIndex.cpp `_ElideSubtree`. Prevents the subtree from contributing
/// opinions. Used by relocations to suppress ancestral arcs that are
/// superseded by the relocation source.
fn elide_subtree(node: &NodeRef) {
    let mut stack = vec![node.clone()];
    while let Some(current) = stack.pop() {
        current.set_inert(true);
        current.set_spec_contribution_restricted_depth(1);
        stack.extend(current.children());
    }
}

/// Checks if node is a relocates placeholder implied arc.
/// C++ primIndex.cpp:3873-3883 `_IsRelocatesPlaceholderImpliedArc`.
/// These placeholders exist under relocation nodes only to allow class-based
/// arcs to be implied up the prim index. They are not valid sources of opinions.
fn is_relocates_placeholder_implied_arc(node: &NodeRef) -> bool {
    let parent = node.parent_node();
    if !parent.is_valid() {
        return false;
    }
    // C++: parentNode != node.GetOriginNode() && parentNode.GetArcType() == Relocate && parentNode.GetSite() == node.GetSite()
    let origin = node.origin_node();
    parent.node_index() != origin.node_index()
        && parent.arc_type() == crate::ArcType::Relocate
        && parent.path() == node.path()
}

/// Returns true if this is an implied inherit/specialize arc, not a directly-authored one.
/// C++ primIndex.cpp:1566-1572 `_IsImpliedClassBasedArc`.
fn is_implied_class_based_arc(
    arc_type: crate::ArcType,
    parent: &NodeRef,
    origin: &NodeRef,
) -> bool {
    arc_type.is_class_based() && parent != origin
}

/// Returns the propagated specializes node for a given node, if any.
///
/// Matches C++ `_GetPropagatedSpecializesNode`: if `node` is a specialize arc,
/// searches root children in reverse for a propagated-specializes child whose
/// origin matches `node`. Specializes are weakest so they appear at the end.
fn get_propagated_specializes_node(node: &NodeRef) -> Option<NodeRef> {
    if node.arc_type() != crate::ArcType::Specialize {
        return None;
    }

    let root = node.root_node();
    for child in root.children().into_iter().rev() {
        if child.arc_type() < crate::ArcType::Specialize {
            break;
        }
        if child.origin_node() == *node && crate::is_propagated_specializes_node(&child) {
            return Some(child);
        }
    }
    None
}

/// Checks if a node has any class-based (inherit or specialize) children.
///
/// Matches C++ `_HasClassBasedChild`: also checks propagated specializes node's
/// children when the node is a specialize arc.
fn has_class_based_child(node: &NodeRef) -> bool {
    let check = |n: &NodeRef| -> bool {
        for child in n.children() {
            if is_class_based_arc(child.arc_type()) {
                return true;
            }
        }
        false
    };

    if let Some(propagated) = get_propagated_specializes_node(node) {
        return check(&propagated);
    }
    check(node)
}

/// Checks if an arc type is class-based (inherit or specialize).
fn is_class_based_arc(arc_type: crate::ArcType) -> bool {
    arc_type == crate::ArcType::Inherit || arc_type == crate::ArcType::Specialize
}

/// Checks if an arc type is a specialize arc. Matches C++ `PcpIsSpecializeArc`.
fn is_specialize_arc(arc_type: crate::ArcType) -> bool {
    arc_type == crate::ArcType::Specialize
}

/// C++ `_FindStartingNodeForImpliedClasses`: walk up nested class hierarchy
/// chains to find the starting node for implied class propagation.
///
/// When a class-based node is itself part of a class hierarchy, we need to
/// find the starting instance node of that chain and propagate the entire
/// chain as a single unit.
fn find_starting_node_for_implied_classes(n: &NodeRef) -> Option<NodeRef> {
    debug_assert!(is_class_based_arc(n.arc_type()));

    let mut start_node = n.clone();

    while is_class_based_arc(start_node.arc_type()) {
        let (instance_node, class_node) =
            crate::utils::find_starting_node_of_class_hierarchy(&start_node);

        start_node = instance_node.clone();

        // If the instance that inherits the class hierarchy is itself
        // class-based, check if the class is a namespace child of the
        // ancestral class. If so, we're done.
        if is_class_based_arc(instance_node.arc_type()) {
            let ancestral_class_path = instance_node.path_at_introduction();
            if class_node.path().has_prefix(&ancestral_class_path) {
                break;
            }
        }
    }

    if start_node.is_valid() {
        Some(start_node)
    } else {
        None
    }
}

/// C++ `_HasSpecializesChildInSubtree`: checks if any node in the subtree
/// rooted at `parent` has a specialize arc type. Iterative to avoid stack overflow.
fn has_specializes_child_in_subtree(parent: &NodeRef) -> bool {
    let mut stack: Vec<NodeRef> = parent.children();
    while let Some(node) = stack.pop() {
        if node.arc_type() == crate::ArcType::Specialize {
            return true;
        }
        stack.extend(node.children());
    }
    false
}

/// Returns whether a node can contribute ancestral opinions at the given path.
///
/// Matches C++ `_NodeCanContributeAncestralOpinions` (primIndex.cpp:3954-3965):
/// A node can contribute if it has no spec-contribution restriction, OR if
/// the restriction depth is greater than the path's element count (meaning
/// the restriction is deeper in namespace than `ancestral_path`).
///
/// `GetSpecContributionRestrictedDepth() == 0` means no restriction.
fn node_can_contribute_ancestral_opinions(node: &NodeRef, ancestral_path: &Path) -> bool {
    let restriction_depth = node.spec_contribution_restricted_depth();
    restriction_depth == 0 || restriction_depth > ancestral_path.get_path_element_count()
}

// ============================================================================
// Task Types
// ============================================================================

/// Task types for composition, ordered by evaluation priority.
///
/// The ordering follows LIVRPS (Local, Inherits, Variants, References,
/// Payloads, Specializes) with additional task types for implied arcs
/// and special handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum TaskType {
    /// Evaluate relocations on a node.
    EvalNodeRelocations = 1,
    /// Evaluate implied relocations.
    EvalImpliedRelocations = 2,
    /// Evaluate references on a node.
    EvalNodeReferences = 3,
    /// Evaluate payloads on a node.
    EvalNodePayloads = 4,
    /// Evaluate inherits on a node.
    EvalNodeInherits = 5,
    /// Evaluate specializes on a node.
    EvalNodeSpecializes = 6,
    /// Evaluate implied specializes.
    EvalImpliedSpecializes = 7,
    /// Evaluate implied classes (inherits).
    EvalImpliedClasses = 8,
    /// Evaluate ancestral variant sets.
    EvalNodeAncestralVariantSets = 9,
    /// Evaluate ancestral authored variant.
    EvalNodeAncestralVariantAuthored = 10,
    /// Evaluate ancestral fallback variant.
    EvalNodeAncestralVariantFallback = 11,
    /// Evaluate ancestral variant none found.
    EvalNodeAncestralVariantNoneFound = 12,
    /// Evaluate ancestral dynamic payloads.
    EvalNodeAncestralDynamicPayloads = 13,
    /// Evaluate variant sets on a node.
    EvalNodeVariantSets = 14,
    /// Evaluate authored variant.
    EvalNodeVariantAuthored = 15,
    /// Evaluate fallback variant.
    EvalNodeVariantFallback = 16,
    /// Evaluate variant none found.
    EvalNodeVariantNoneFound = 17,
    /// Evaluate dynamic payloads.
    EvalNodeDynamicPayloads = 18,
    /// Evaluate unresolved prim path error.
    EvalUnresolvedPrimPathError = 19,
    /// No task.
    None = 0,
}

impl TaskType {
    /// Returns the priority of this task type (lower = higher priority).
    pub fn priority(self) -> u32 {
        self as u32
    }
}

// ============================================================================
// Task
// ============================================================================

/// A composition task to be processed.
#[derive(Clone, Debug)]
pub struct Task {
    /// The type of task.
    pub task_type: TaskType,
    /// The node this task operates on.
    pub node: NodeRef,
    /// Variant set path (for variant tasks).
    pub vset_path: Option<Path>,
    /// Variant set name (for variant tasks).
    pub vset_name: Option<String>,
    /// Variant set number (for variant tasks).
    pub vset_num: i32,
}

impl Task {
    /// Creates a new task for a node.
    pub fn new(task_type: TaskType, node: NodeRef) -> Self {
        Self {
            task_type,
            node,
            vset_path: None,
            vset_name: None,
            vset_num: 0,
        }
    }

    /// Creates a variant task.
    pub fn variant(
        task_type: TaskType,
        node: NodeRef,
        vset_path: Path,
        vset_name: String,
        vset_num: i32,
    ) -> Self {
        Self {
            task_type,
            node,
            vset_path: Some(vset_path),
            vset_name: Some(vset_name),
            vset_num,
        }
    }

    /// Returns true if this is a null task.
    pub fn is_none(&self) -> bool {
        self.task_type == TaskType::None
    }
}

impl Default for Task {
    fn default() -> Self {
        Self {
            task_type: TaskType::None,
            node: NodeRef::invalid(),
            vset_path: None,
            vset_name: None,
            vset_num: 0,
        }
    }
}

// For heap ordering. We want highest-priority (lowest TaskType value) to be
// returned first. The Vec-as-max-heap approach pops the "largest" element,
// so we define Task ordering such that higher priority = "larger" in Ord.
impl PartialEq for Task {
    fn eq(&self, other: &Self) -> bool {
        self.task_type == other.task_type
            && self.node.unique_identifier() == other.node.unique_identifier()
            && self.vset_path == other.vset_path
            && self.vset_name == other.vset_name
            && self.vset_num == other.vset_num
    }
}

impl Eq for Task {}

impl PartialOrd for Task {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Task {
    /// C++ primIndex.cpp:751-850 PriorityOrder comparator.
    /// The heap pops the "max" element; we define max = highest priority.
    /// Highest priority = lowest TaskType enum value.
    ///
    /// Secondary ordering (when task types match) per C++:
    ///   - VariantAuthored/VariantFallback: by node strength, then (vsetPath, vsetNum) desc
    ///   - VariantNoneFound: by (node, vsetPath, vsetNum) desc (consistent arbitrary)
    ///   - EvalImpliedClasses: ancestor nodes after descendants
    ///   - default: node.unique_identifier() desc (consistent arbitrary)
    fn cmp(&self, other: &Self) -> Ordering {
        // Primary: lower TaskType value = higher priority = "greater" in max-heap.
        let type_cmp = other.task_type.priority().cmp(&self.task_type.priority());
        if type_cmp != Ordering::Equal {
            return type_cmp;
        }
        // Same type: secondary comparisons.
        match self.task_type {
            TaskType::EvalNodeVariantAuthored
            | TaskType::EvalNodeVariantFallback
            | TaskType::EvalNodeAncestralVariantAuthored
            | TaskType::EvalNodeAncestralVariantFallback => {
                // C++: same node -> sort by (vsetPath, vsetNum) descending
                // (lower vsetNum = higher priority = processed first = "greater").
                let node_cmp = other
                    .node
                    .unique_identifier()
                    .cmp(&self.node.unique_identifier());
                if node_cmp != Ordering::Equal {
                    return node_cmp;
                }
                // Same node: lower vsetNum = higher priority.
                let path_a = self.vset_path.as_ref().map(|p| p.get_string());
                let path_b = other.vset_path.as_ref().map(|p| p.get_string());
                let path_cmp = path_b.cmp(&path_a); // desc -> b.cmp(a)
                if path_cmp != Ordering::Equal {
                    return path_cmp;
                }
                // Lower vsetNum is higher priority -> other.vset_num.cmp(self.vset_num) for desc.
                other.vset_num.cmp(&self.vset_num)
            }
            TaskType::EvalNodeVariantNoneFound | TaskType::EvalNodeAncestralVariantNoneFound => {
                // C++: consistent arbitrary ordering by (node, vsetPath, vsetNum).
                let node_cmp = other
                    .node
                    .unique_identifier()
                    .cmp(&self.node.unique_identifier());
                if node_cmp != Ordering::Equal {
                    return node_cmp;
                }
                let path_a = self.vset_path.as_ref().map(|p| p.get_string());
                let path_b = other.vset_path.as_ref().map(|p| p.get_string());
                let path_cmp = path_b.cmp(&path_a);
                if path_cmp != Ordering::Equal {
                    return path_cmp;
                }
                other.vset_num.cmp(&self.vset_num)
            }
            _ => {
                // Default: consistent arbitrary ordering by node index.
                other
                    .node
                    .unique_identifier()
                    .cmp(&self.node.unique_identifier())
            }
        }
    }
}

// ============================================================================
// Indexer
// ============================================================================

/// The prim indexer manages composition task processing.
///
/// The indexer maintains a priority queue of tasks and processes them
/// in LIVRPS order to build the composition graph.
pub struct PrimIndexer {
    /// The prim index being built.
    graph: Arc<PrimIndexGraph>,
    /// Root site for this composition.
    root_site: Site,
    /// Task queue (Vec used as max-heap via push/pop_heap, matching C++).
    /// C++ primIndex.cpp:1113 — tasks is std::vector<Task> used as heap.
    tasks: Vec<Task>,
    /// Dedup set for IMPLIED tasks only (EvalImpliedClasses, EvalImpliedSpecializes).
    /// C++ primIndex.cpp:1118 — taskUniq = pxr_tsl::robin_set<Task, TfHash>.
    /// Variant tasks are NEVER deduplicated — each (node, vsetPath, vsetName, vsetNum)
    /// is a unique task and multiple variant sets on one node all get enqueued.
    implied_task_uniq: HashSet<(TaskType, usize)>, // (task_type, node_index)
    /// Layer stack for the root.
    layer_stack: LayerStackRefPtr,
    /// Whether this is USD mode.
    is_usd: bool,
    /// Variant fallback map.
    variant_fallbacks: VariantFallbackMap,
    /// Errors encountered during composition.
    errors: Vec<ErrorType>,
    /// Ancestor recursion depth (for nested composition).
    ancestor_recursion_depth: i32,
    /// Skip duplicate nodes flag.
    skip_duplicate_nodes: bool,
    /// File format target for dynamic payloads.
    file_format_target: Option<String>,
    /// P1-5: Link to previous stack frame for cross-frame duplicate detection.
    /// C++ threads PcpPrimIndex_StackFrame* through recursive Pcp_BuildPrimIndex calls.
    previous_frame: Option<Box<super::prim_index_stack_frame::PrimIndexStackFrame>>,
    /// Payload inclusion set. None = never include (C++ nullptr).
    /// Some(set) = check set, then predicate.
    included_payloads: Option<Vec<Path>>,
    /// Predicate for newly discovered payloads. C++ includePayloadPredicate.
    include_payload_predicate: Option<Arc<dyn Fn(&Path) -> bool + Send + Sync>>,
    /// Tracks the payload state for the current composition.
    payload_state: super::prim_index::PayloadState,
    /// Matches C++ evaluateImpliedSpecializes build option.
    evaluate_implied_specializes: bool,
    /// Matches C++ evaluateVariantsAndDynamicPayloads build option.
    evaluate_variants_and_dynamic_payloads: bool,
}

/// Options for arc addition, matching C++ _ArcOptions (primIndex.cpp:1698).
#[derive(Clone)]
pub struct ArcOpts {
    pub direct_node_should_contribute_specs: bool,
    pub include_ancestral_opinions: bool,
    pub skip_duplicate_nodes: bool,
    pub copy_ancestor_flag_from_origin: bool,
}

impl Default for ArcOpts {
    fn default() -> Self {
        Self {
            direct_node_should_contribute_specs: true,
            include_ancestral_opinions: false,
            skip_duplicate_nodes: false,
            copy_ancestor_flag_from_origin: false,
        }
    }
}

impl PrimIndexer {
    /// Creates a new indexer for the given graph.
    pub fn new(
        graph: Arc<PrimIndexGraph>,
        root_site: Site,
        layer_stack: LayerStackRefPtr,
        is_usd: bool,
    ) -> Self {
        Self {
            graph,
            root_site,
            tasks: Vec::new(),
            implied_task_uniq: HashSet::new(),
            layer_stack,
            is_usd,
            variant_fallbacks: VariantFallbackMap::new(),
            errors: Vec::new(),
            ancestor_recursion_depth: 0,
            skip_duplicate_nodes: false,
            file_format_target: None,
            previous_frame: None,
            included_payloads: None,
            include_payload_predicate: None,
            payload_state: super::prim_index::PayloadState::NoPayload,
            evaluate_implied_specializes: true,
            evaluate_variants_and_dynamic_payloads: true,
        }
    }

    /// P1-5: Sets the previous stack frame for cross-frame duplicate detection.
    pub fn set_previous_frame(
        &mut self,
        frame: Option<Box<super::prim_index_stack_frame::PrimIndexStackFrame>>,
    ) {
        self.previous_frame = frame;
    }

    /// Maps a node's path to the payload inclusion path in root namespace.
    ///
    /// C++ `Pcp_PrimIndexer::MapNodePathToPayloadInclusionPath` (primIndex.cpp:1239-1267).
    /// Handles identity mappings in internal references/payloads and stack frame traversal.
    fn map_node_path_to_payload_inclusion_path(&self, node: &NodeRef, path: &Path) -> Path {
        let stripped = path.strip_all_variant_selections();
        let mut p = Self::map_path_to_node_root_inclusion(node, &stripped);

        // Walk stack frames for recursive prim indexing
        if !p.is_empty() {
            let mut frame_ref = self.previous_frame.as_ref();
            while let Some(frame) = frame_ref {
                if p.is_empty() {
                    break;
                }
                // Map through arc_to_parent
                p = Self::map_path_to_node_parent_inclusion(
                    frame.arc_to_parent.map_to_parent(),
                    frame.arc_to_parent.arc_type(),
                    &p,
                );
                // Map from parent node to its root
                if !p.is_empty() {
                    p = Self::map_path_to_node_root_inclusion(&frame.parent_node, &p);
                }
                frame_ref = frame.previous_frame.as_ref();
            }
        }
        p
    }

    /// Maps a path to the payload inclusion path for a node's prim index root.
    ///
    /// C++ `_MapPathToNodeRootPayloadInclusionPath` (primIndex.cpp:1211-1235).
    fn map_path_to_node_root_inclusion(node: &NodeRef, path: &Path) -> Path {
        let map_to_root = node.map_to_root().evaluate();
        let mapped = map_to_root.map_source_to_target(path);

        match mapped {
            Some(ref mapped_path) if *mapped_path == *path && map_to_root.has_root_identity() => {
                // Identity mapping with root identity — manually walk up to handle
                // internal reference/payload arcs that have unwanted identity mappings.
                let mut result = path.clone();
                let mut cur = node.clone();
                while !result.is_empty() && !cur.is_root_node() {
                    let map_expr = cur.map_to_parent();
                    result =
                        Self::map_path_to_node_parent_inclusion(&map_expr, cur.arc_type(), &result);
                    cur = cur.parent_node();
                }
                result
            }
            Some(mapped_path) => mapped_path,
            None => Path::empty(),
        }
    }

    /// Maps a path from a node to its parent, handling internal ref/payload identity stripping.
    ///
    /// C++ `_MapPathToNodeParentPayloadInclusionPath` (primIndex.cpp:1180-1207).
    fn map_path_to_node_parent_inclusion(
        map_to_parent_expr: &MapExpression,
        arc_type: crate::ArcType,
        path: &Path,
    ) -> Path {
        use crate::ArcType;

        let map_to_parent = map_to_parent_expr.evaluate();

        // Internal references and payloads have an identity mapping we need to skip
        let is_internal = map_to_parent.has_root_identity()
            && (arc_type == ArcType::Reference || arc_type == ArcType::Payload);

        if is_internal {
            // Remove root identity and map with remaining entries
            let mut src_map = map_to_parent.source_to_target_map();
            src_map.remove(&Path::absolute_root());
            if let Some(new_fn) = MapFunction::create(src_map, map_to_parent.time_offset().clone())
            {
                new_fn
                    .map_source_to_target(path)
                    .unwrap_or_else(Path::empty)
            } else {
                Path::empty()
            }
        } else {
            map_to_parent
                .map_source_to_target(path)
                .unwrap_or_else(Path::empty)
        }
    }

    /// Sets the file format target for dynamic payloads.
    pub fn set_file_format_target(&mut self, target: Option<String>) {
        self.file_format_target = target;
    }

    /// Returns the file format target.
    pub fn file_format_target(&self) -> Option<&str> {
        self.file_format_target.as_deref()
    }

    /// Sets the variant fallbacks.
    pub fn set_variant_fallbacks(&mut self, fallbacks: VariantFallbackMap) {
        self.variant_fallbacks = fallbacks;
    }

    pub fn set_evaluate_implied_specializes(&mut self, enabled: bool) {
        self.evaluate_implied_specializes = enabled;
    }

    pub fn set_evaluate_variants_and_dynamic_payloads(&mut self, enabled: bool) {
        self.evaluate_variants_and_dynamic_payloads = enabled;
    }

    /// Sets the payload inclusion set. None = never include payloads.
    pub fn set_included_payloads(&mut self, payloads: Option<Vec<Path>>) {
        self.included_payloads = payloads;
    }

    /// Sets the include payload predicate for newly discovered payloads.
    pub fn set_include_payload_predicate(
        &mut self,
        pred: Arc<dyn Fn(&Path) -> bool + Send + Sync>,
    ) {
        self.include_payload_predicate = Some(pred);
    }

    /// Returns the payload state after composition.
    pub fn payload_state(&self) -> super::prim_index::PayloadState {
        self.payload_state
    }

    /// Returns the graph being built.
    pub fn graph(&self) -> &Arc<PrimIndexGraph> {
        &self.graph
    }

    /// Returns the layer stack.
    pub fn layer_stack(&self) -> &LayerStackRefPtr {
        &self.layer_stack
    }

    /// Returns the root site for this composition.
    pub fn root_site(&self) -> &Site {
        &self.root_site
    }

    /// Returns whether this indexer is in USD mode.
    ///
    /// In USD mode, additional composition features are enabled.
    pub fn is_usd(&self) -> bool {
        self.is_usd
    }

    /// Returns the ancestor recursion depth.
    ///
    /// Used to track nested composition for recursive references.
    pub fn ancestor_recursion_depth(&self) -> i32 {
        self.ancestor_recursion_depth
    }

    /// Sets the ancestor recursion depth.
    pub fn set_ancestor_recursion_depth(&mut self, depth: i32) {
        self.ancestor_recursion_depth = depth;
    }

    /// Returns whether to skip duplicate nodes.
    pub fn skip_duplicate_nodes(&self) -> bool {
        self.skip_duplicate_nodes
    }

    /// Sets whether to skip duplicate nodes.
    pub fn set_skip_duplicate_nodes(&mut self, skip: bool) {
        self.skip_duplicate_nodes = skip;
    }

    /// Returns collected errors.
    pub fn errors(&self) -> &[ErrorType] {
        &self.errors
    }

    /// Takes collected errors.
    pub fn take_errors(&mut self) -> Vec<ErrorType> {
        std::mem::take(&mut self.errors)
    }

    /// Adds an error.
    pub fn add_error(&mut self, error: ErrorType) {
        self.errors.push(error);
    }

    // ========================================================================
    // Task Management
    // ========================================================================

    /// Returns true if a task type is an "implied" task that must be deduplicated.
    ///
    /// C++ primIndex.cpp:1275 — only EvalImpliedClasses and EvalImpliedSpecializes
    /// use the taskUniq dedup set. ALL other tasks (including ALL variant tasks)
    /// are allowed to have multiple copies in the queue.
    #[inline]
    fn is_implied_task(tt: TaskType) -> bool {
        matches!(
            tt,
            TaskType::EvalImpliedClasses | TaskType::EvalImpliedSpecializes
        )
    }

    /// Adds a task to the queue.
    ///
    /// C++ primIndex.cpp:1275 AddTask:
    ///   - Only EvalImpliedClasses / EvalImpliedSpecializes are deduplicated via taskUniq.
    ///   - All other tasks (variant tasks especially) are pushed unconditionally.
    ///   - The task vector is used as a max-heap (highest priority task at top).
    pub fn add_task(&mut self, task: Task) {
        if Self::is_implied_task(task.task_type) {
            // Deduplicate implied tasks by (type, node_id).
            let key = (task.task_type, task.node.unique_identifier());
            if !self.implied_task_uniq.insert(key) {
                return; // Already scheduled
            }
        }
        // Push and maintain heap invariant.
        self.tasks.push(task);
        // sort so highest priority (smallest TaskType value) is last for pop().
        // We use a simple Vec-as-max-heap: since Ord for Task reverses priority
        // (lower type value = higher priority = should be popped first), we use
        // std BinaryHeap semantics via manual sort-and-pop.
        // Actually: re-heap after push using standard BinaryHeap approach.
        // We store tasks in a Vec and keep it heap-ordered.
        let len = self.tasks.len();
        // Sift up: the new element was pushed at the end.
        Self::sift_up(&mut self.tasks, len - 1);
    }

    /// Sifts element at `pos` upward to restore max-heap property.
    /// "Max" here means "highest priority" = smallest TaskType value.
    fn sift_up(tasks: &mut Vec<Task>, mut pos: usize) {
        while pos > 0 {
            let parent = (pos - 1) / 2;
            // If current has higher priority than parent, swap.
            if tasks[pos] > tasks[parent] {
                tasks.swap(pos, parent);
                pos = parent;
            } else {
                break;
            }
        }
    }

    /// Sifts element at `pos` downward to restore max-heap property.
    fn sift_down(tasks: &mut Vec<Task>, pos: usize) {
        let len = tasks.len();
        let mut cur = pos;
        loop {
            let left = 2 * cur + 1;
            let right = 2 * cur + 2;
            let mut largest = cur;
            if left < len && tasks[left] > tasks[largest] {
                largest = left;
            }
            if right < len && tasks[right] > tasks[largest] {
                largest = right;
            }
            if largest == cur {
                break;
            }
            tasks.swap(cur, largest);
            cur = largest;
        }
    }

    /// Pops the highest-priority task from the queue.
    ///
    /// C++ primIndex.cpp:1290 PopTask:
    ///   - pop_heap moves max to back, then pop_back removes it.
    ///   - For implied tasks: remove from taskUniq dedup set.
    pub fn pop_task(&mut self) -> Task {
        if self.tasks.is_empty() {
            return Task::default();
        }
        // Swap root (highest priority) with last element, then sift down.
        let last = self.tasks.len() - 1;
        self.tasks.swap(0, last);
        let task = self.tasks.pop().unwrap();
        if !self.tasks.is_empty() {
            Self::sift_down(&mut self.tasks, 0);
        }
        // Remove from implied dedup set if it was an implied task.
        if Self::is_implied_task(task.task_type) {
            let key = (task.task_type, task.node.unique_identifier());
            self.implied_task_uniq.remove(&key);
        }
        task
    }

    /// Returns true if there are tasks remaining.
    pub fn has_tasks(&self) -> bool {
        !self.tasks.is_empty()
    }

    /// Promotes pending Fallback/NoneFound variant tasks to Authored tasks.
    ///
    /// C++ primIndex.cpp:1473 RetryVariantTasks:
    /// Called after every successful _AddVariantArc. Walks all tasks in the
    /// queue and promotes Fallback->Authored and NoneFound->Authored so they
    /// will be re-evaluated with any newly available authored selections.
    ///
    /// CRITICAL: The in-place promotion + re-heap is intentional. After a
    /// variant expansion adds new specs that may contain authored variant
    /// selections, pending fallback tasks must be re-checked.
    pub fn retry_variant_tasks(&mut self) {
        let mut changed = false;
        for task in &mut self.tasks {
            let new_type = match task.task_type {
                TaskType::EvalNodeVariantFallback => TaskType::EvalNodeVariantAuthored,
                TaskType::EvalNodeVariantNoneFound => TaskType::EvalNodeVariantAuthored,
                TaskType::EvalNodeAncestralVariantFallback => {
                    TaskType::EvalNodeAncestralVariantAuthored
                }
                TaskType::EvalNodeAncestralVariantNoneFound => {
                    TaskType::EvalNodeAncestralVariantAuthored
                }
                _ => continue,
            };
            task.task_type = new_type;
            changed = true;
        }
        if changed {
            // Re-heapify after in-place modifications.
            let len = self.tasks.len();
            // Build heap from scratch (heapify): sift down all non-leaf nodes.
            if len > 1 {
                let mut i = (len - 2) / 2;
                loop {
                    Self::sift_down(&mut self.tasks, i);
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                }
            }
        }
    }

    /// Adds initial tasks for ALL nodes in the graph (recursive).
    ///
    /// C++ `AddTasksForRootNode` -> `_AddTasksForNodeRecursively`
    /// (primIndex.cpp:1372-1386, 1303-1369):
    /// Traverses the entire graph tree, adding composition tasks for every
    /// node that can contribute specs. This is critical for inherited nodes
    /// from ancestor recursion — without it, composition arcs on inherited
    /// nodes (e.g. nested references) are never evaluated at the child level.
    pub fn add_tasks_for_root_node(&mut self, root: &NodeRef) {
        if !root.is_valid() {
            return;
        }
        // C++ excludes AncestralVariantsAndDynamicPayloadTasks for root init.
        // No expressed-arc filtering for root node (skip_expressed=false).
        self.add_tasks_for_node_recursively(root, false, false);
    }

    /// Add composition tasks for a node and all its descendants (iterative).
    ///
    /// C++ `_AddTasksForNodeRecursively` (primIndex.cpp:1303-1369):
    /// Traverses entire subtree, adding tasks for nodes with specs.
    /// Uses iterative BFS to avoid stack overflow on deep hierarchies.
    fn add_tasks_for_node_recursively(
        &mut self,
        node: &NodeRef,
        include_ancestral: bool,
        skip_expressed: bool,
    ) {
        // Iterative traversal to avoid stack overflow (caldera has deep hierarchies)
        let mut stack = vec![node.node_idx];
        while let Some(idx) = stack.pop() {
            // Enqueue children for traversal
            let children = self.graph().get_children_indices(idx);
            for &child_idx in children.iter().rev() {
                stack.push(child_idx);
            }

            let n = NodeRef::new(self.graph().clone(), idx);

            // C++ _ScanArcs: skip nodes that can't contribute specs
            if !n.can_contribute_specs() {
                continue;
            }

            // C++ _ScanArcs (primIndex.cpp:929-987): preflight scan layers
            // to only add tasks for arc types actually present.
            // IMPORTANT: Relocates check comes BEFORE has_specs gate (C++ 949-954),
            // because relocates are layer-stack-wide, not per-spec.
            let mut has_relocates = false;
            if let Some(ls) = n.layer_stack() {
                if ls.has_relocates() {
                    has_relocates = true;
                }
            }

            // C++ _ScanArcs: only nodes with specs can contribute composition arcs
            let node_path = n.path();
            let mut has_refs = false;
            let mut has_payloads = false;
            let mut has_inherits = false;
            let mut has_specializes = false;
            let mut has_variants = false;

            if n.has_specs() {
                if let Some(ls) = n.layer_stack() {
                    let tk_refs = Token::new("references");
                    let tk_payload = Token::new("payload");
                    let tk_inherit = Token::new("inheritPaths");
                    let tk_spec = Token::new("specializes");
                    let tk_vsets = Token::new("variantSetNames");

                    for layer in ls.get_layers() {
                        if !layer.has_spec(&node_path) {
                            continue;
                        }
                        if layer.has_field(&node_path, &tk_refs) {
                            has_refs = true;
                        }
                        if layer.has_field(&node_path, &tk_payload) {
                            has_payloads = true;
                        }
                        if layer.has_field(&node_path, &tk_inherit) {
                            has_inherits = true;
                        }
                        if layer.has_field(&node_path, &tk_spec) {
                            has_specializes = true;
                        }
                        if layer.has_field(&node_path, &tk_vsets) {
                            has_variants = true;
                        }
                    }
                }
            }

            // C++ _AddTasksForNodeRecursively (primIndex.cpp:1352-1368):
            // only enqueue tasks for arcs actually present.
            // When skip_expressed=true (C++ includeAncestralOpinions), skip
            // ExpressedArcTasks = {Specializes, Inherits, Payloads, References, Relocations}
            // because the recursive Pcp_BuildPrimIndex call already evaluated those.
            if !skip_expressed {
                if has_relocates {
                    self.add_task(Task::new(TaskType::EvalNodeRelocations, n.clone()));
                }
                if has_refs {
                    self.add_task(Task::new(TaskType::EvalNodeReferences, n.clone()));
                }
                if has_payloads {
                    self.add_task(Task::new(TaskType::EvalNodePayloads, n.clone()));
                }
                if has_inherits {
                    self.add_task(Task::new(TaskType::EvalNodeInherits, n.clone()));
                }
                if has_specializes {
                    self.add_task(Task::new(TaskType::EvalNodeSpecializes, n.clone()));
                }
            }
            // DynamicPayloads and VariantSets are NOT in ExpressedArcTasks —
            // they're in VariantsAndDynamicPayloadTasks, always kept.
            if has_payloads && self.evaluate_variants_and_dynamic_payloads {
                self.add_task(Task::new(TaskType::EvalNodeDynamicPayloads, n.clone()));
            }
            if has_variants && self.evaluate_variants_and_dynamic_payloads {
                self.add_task(Task::new(TaskType::EvalNodeVariantSets, n.clone()));
            }
            // C++ 1328-1333: EvalImpliedRelocations only for Relocate-type nodes
            if n.arc_type() == crate::ArcType::Relocate {
                self.add_task(Task::new(TaskType::EvalImpliedRelocations, n.clone()));
            }

            // C++ _ScanAncestralArcs + _AddTasksForNodeRecursively:
            // Walk ancestor paths of this node looking for payload/variant fields.
            // Only if include_ancestral (excluded for root init).
            if include_ancestral && !node_path.is_absolute_root_path() {
                let mut anc_has_variants = false;
                let mut anc_has_payloads = false;

                // Walk from parent path up, respecting restricted depth
                let mut anc_path = node_path.get_parent_path();
                let restricted = n.spec_contribution_restricted_depth();
                if restricted != 0 {
                    while anc_path.get_path_element_count() >= restricted
                        && !anc_path.is_absolute_root_path()
                    {
                        anc_path = anc_path.get_parent_path();
                    }
                }

                if let Some(ls) = n.layer_stack() {
                    let tk_payload = Token::new("payload");
                    let tk_vsets = Token::new("variantSetNames");
                    while !anc_path.is_absolute_root_path() {
                        for layer in ls.get_layers() {
                            if layer.has_field(&anc_path, &tk_payload) {
                                anc_has_payloads = true;
                            }
                            if layer.has_field(&anc_path, &tk_vsets) {
                                anc_has_variants = true;
                            }
                        }
                        anc_path = anc_path.get_parent_path();
                    }
                }

                // C++ AddTasksForNode (line 1434) only removes
                // VariantsAndDynamicPayloadTasks when evaluateVariantsAndDynamicPayloads
                // is false — AncestralVariantsAndDynamicPayloadTasks are always kept
                // for non-root nodes so ancestral variant opinions are picked up.
                if anc_has_variants {
                    self.add_task(Task::new(TaskType::EvalNodeAncestralVariantSets, n.clone()));
                }
                if anc_has_payloads {
                    self.add_task(Task::new(
                        TaskType::EvalNodeAncestralDynamicPayloads,
                        n.clone(),
                    ));
                }
            }
        }
    }

    /// Adds tasks for a newly added node (and its subtree).
    ///
    /// C++ `AddTasksForNode` (primIndex.cpp:1401-1454): includes ancestral
    /// variant/dynamic payload tasks, unlike `AddTasksForRootNode`.
    pub fn add_tasks_for_node(&mut self, node: &NodeRef) {
        if !node.is_valid() {
            return;
        }
        // C++ AddTasksForNode includes ancestral tasks, no expressed-arc filtering
        self.add_tasks_for_node_recursively(node, true, false);
    }

    /// Adds tasks for a newly-added node with C++ _AddArc task bitmask filtering.
    ///
    /// Per C++ _AddArc (primIndex.cpp:2008-2034):
    /// - `include_ancestral_opinions`: when true, strips ExpressedArcTasks
    ///   (Specializes, Inherits, Payloads, References, Relocations) because
    ///   recursive Pcp_BuildPrimIndex already evaluated them. Also enables
    ///   AncestralVariantsAndDynamicPayloadTasks.
    /// - When false, strips EvalImpliedSpecializes (no subtree to search)
    ///   and AncestralVariantsAndDynamicPayloadTasks.
    fn add_tasks_for_node_filtered(&mut self, node: &NodeRef, include_ancestral_opinions: bool) {
        if !node.is_valid() {
            return;
        }

        // C++ AddTasksForNode: handle implied classes/specializes BEFORE
        // recursing into the subtree (they're separate from the bitmask system).
        if is_class_based_arc(node.arc_type()) {
            if let Some(base) = find_starting_node_for_implied_classes(node) {
                self.add_task(Task::new(TaskType::EvalImpliedClasses, base));
            }
        } else if has_class_based_child(node) {
            self.add_task(Task::new(TaskType::EvalImpliedClasses, node.clone()));
        }

        // C++ (line 2029-2031): only add EvalImpliedSpecializes when
        // includeAncestralOpinions is true (there's a subtree to search).
        if include_ancestral_opinions && self.evaluate_implied_specializes {
            if has_specializes_child_in_subtree(node) {
                self.add_task(Task::new(TaskType::EvalImpliedSpecializes, node.clone()));
            }
        }

        // C++ (line 2015-2016): when includeAncestralOpinions, skip ExpressedArcTasks
        let skip_expressed = include_ancestral_opinions;

        // C++ (line 2019-2025): evaluateAncestralVariantsAndDynamicPayloads =
        //   indexer.evaluateVariantsAndDynamicPayloads && includeAncestralOpinions.
        let include_ancestral_tasks =
            include_ancestral_opinions && self.evaluate_variants_and_dynamic_payloads;

        self.add_tasks_for_node_recursively(node, include_ancestral_tasks, skip_expressed);
    }

    // NOTE: add_ancestral_tasks was removed in the PCP-C5 fix.
    // It was a simplified workaround that only added variant tasks for ancestors.
    // Now replaced by recursive compute_prim_index_with_frame in add_reference_arc
    // and add_payload_arc, matching C++ _AddArc with includeAncestralOpinions=true
    // which recursively calls Pcp_BuildPrimIndex for the target site.

    // ========================================================================
    // Arc Evaluation
    // ========================================================================

    /// Evaluates references on a node.
    pub fn eval_node_references(&mut self, node: &NodeRef) {
        if !node.is_valid() || !node.can_contribute_specs() {
            return;
        }

        // P1-14 FIX: Use the node's own layer stack, not root layer stack.
        // C++ _EvalNodeReferences uses node.GetLayerStack().
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let (refs, ref_arcs, errors) = compose_site_references(&node_layer_stack, &node.path());

        // Collect any errors
        self.errors.extend(errors);

        if refs.is_empty() {
            return;
        }

        // Process each reference
        for (i, (reference, arc_info)) in refs.iter().zip(ref_arcs.iter()).enumerate() {
            self.add_reference_arc(node, reference, arc_info, i);
        }
    }

    /// Evaluates static (non-dynamic) payloads on a node.
    ///
    /// This processes only payloads whose file format does NOT implement
    /// dynamic argument generation. Dynamic payloads are processed separately
    /// by `eval_node_dynamic_payloads`.
    pub fn eval_node_payloads(&mut self, node: &NodeRef) {
        let path = node.path().clone();
        self.eval_node_payloads_impl(node, false, &path);
    }

    /// Evaluates dynamic payloads on a node.
    ///
    /// Dynamic payloads are those whose file format implements
    /// `DynamicFileFormatInterface`. Their arguments are computed from
    /// composed field values at runtime.
    pub fn eval_node_dynamic_payloads(&mut self, node: &NodeRef) {
        let path = node.path().clone();
        self.eval_node_payloads_impl(node, true, &path);
    }

    /// C++ _EvalNodeAncestralDynamicPayloads: walks ancestor paths of the node,
    /// checking for payload fields and evaluating them as dynamic payloads.
    fn eval_node_ancestral_dynamic_payloads(&mut self, node: &NodeRef) {
        let mut path = node.path().get_parent_path();
        while !path.is_absolute_root_path() {
            // C++ _NodeCanContributeAncestralOpinions: restrictionDepth == 0 ||
            // restrictionDepth > ancestralPath.GetPathElementCount()
            if !node_can_contribute_ancestral_opinions(node, &path) {
                path = path.get_parent_path();
                continue;
            }
            // Evaluate payloads at this ancestor path (dynamic only)
            self.eval_node_payloads_at_path(node, &path, true);
            path = path.get_parent_path();
        }
    }

    /// C++ _EvalNodePayloads with explicit path override.
    /// Used by ancestral dynamic payload evaluation where the path differs
    /// from node.path().
    fn eval_node_payloads_at_path(&mut self, node: &NodeRef, path: &Path, dynamic_only: bool) {
        self.eval_node_payloads_impl(node, dynamic_only, path);
    }

    /// Implementation for evaluating payloads, filtering by dynamic or static.
    ///
    /// # Arguments
    ///
    /// * `node` - The node to evaluate payloads on
    /// * `dynamic_only` - If true, only process dynamic payloads; if false, only static
    /// * `path_at_introduction` - Path to compose payloads at (may differ from node.path()
    ///   for ancestral payloads)
    fn eval_node_payloads_impl(
        &mut self,
        node: &NodeRef,
        dynamic_only: bool,
        path_at_introduction: &Path,
    ) {
        use usd_sdf::is_dynamic_file_format;

        if !node.is_valid() || !node.can_contribute_specs() {
            return;
        }

        // P1-14 FIX: Use the node's own layer stack, not root layer stack.
        // C++ _EvalNodePayloads uses node.GetLayerStack().
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        // C++ 2520: compose at nodePathAtIntroduction, not node.GetPath()
        let (payloads, payload_arcs, errors) =
            compose_site_payloads(&node_layer_stack, path_at_introduction);

        // Collect any errors
        self.errors.extend(errors);

        if payloads.is_empty() {
            return;
        }

        // C++ 2544: only mark has_payloads when path matches node's own path
        if *path_at_introduction == node.path() {
            self.graph.set_has_payloads(true);
        }

        // C++ primIndex.cpp:2548-2557: if includedPayloads is nullptr, never include.
        let included_payloads = match &self.included_payloads {
            None => {
                // No payload inclusion set — skip all payloads (C++ nullptr case)
                self.payload_state = super::prim_index::PayloadState::ExcludedByIncludeSet;
                return;
            }
            Some(set) => set,
        };

        // C++ primIndex.cpp:2586-2651: determine whether to compose payload.
        // Map node path to root namespace for payload inclusion check.
        // C++ 2574: use path_at_introduction for inclusion mapping
        let inclusion_path =
            self.map_node_path_to_payload_inclusion_path(node, path_at_introduction);

        if inclusion_path.is_empty() {
            // C++ primIndex.cpp:2596-2633: path mapping failed (subroot ref ancestral case).
            // Policy: always include the payload using node's own path.
            for (i, (payload, arc_info)) in payloads.iter().zip(payload_arcs.iter()).enumerate() {
                let asset_path = payload.asset_path();
                let is_dynamic =
                    is_dynamic_file_format(asset_path, self.file_format_target.as_deref());
                if is_dynamic != dynamic_only {
                    continue;
                }
                if dynamic_only {
                    self.add_dynamic_payload_arc(node, payload, arc_info, i);
                } else {
                    self.add_payload_arc(node, payload, arc_info, i);
                }
            }
            // Schedule dynamic payload evaluation if this was static pass
            if !dynamic_only {
                self.tasks
                    .push(Task::new(TaskType::EvalNodeDynamicPayloads, node.clone()));
            }
            return;
        }

        let compose_payload;

        // C++ order: predicate first (if exists), then set. If predicate exists,
        // set is NOT checked — predicate is the sole authority.
        if let Some(ref pred) = self.include_payload_predicate {
            // C++ primIndex.cpp:2635-2641: predicate decides inclusion
            compose_payload = pred(&inclusion_path);
            self.payload_state = if compose_payload {
                super::prim_index::PayloadState::IncludedByPredicate
            } else {
                super::prim_index::PayloadState::ExcludedByPredicate
            };
        } else {
            // C++ primIndex.cpp:2643-2650: no predicate, check includedPayloads set
            compose_payload = included_payloads.contains(&inclusion_path);
            self.payload_state = if compose_payload {
                super::prim_index::PayloadState::IncludedByIncludeSet
            } else {
                super::prim_index::PayloadState::ExcludedByIncludeSet
            };
        }

        if !compose_payload {
            return;
        }

        // Process each payload, filtering by dynamic/static
        for (i, (payload, arc_info)) in payloads.iter().zip(payload_arcs.iter()).enumerate() {
            let asset_path = payload.asset_path();
            let is_dynamic = is_dynamic_file_format(asset_path, self.file_format_target.as_deref());

            // Filter: if dynamic_only, only process dynamic; otherwise only static
            if is_dynamic != dynamic_only {
                continue;
            }

            if dynamic_only {
                // For dynamic payloads, compute file format arguments from composed fields
                self.add_dynamic_payload_arc(node, payload, arc_info, i);
            } else {
                // For static payloads, use standard processing
                self.add_payload_arc(node, payload, arc_info, i);
            }
        }
    }

    /// Adds a dynamic payload arc with computed file format arguments.
    ///
    /// This creates a DynamicFileFormatContext, composes fields to generate
    /// arguments, and adds the payload arc with those arguments.
    fn add_dynamic_payload_arc(
        &mut self,
        node: &NodeRef,
        payload: &Payload,
        arc_info: &ArcInfo,
        arc_num: usize,
    ) {
        use crate::dynamic_file_format::DynamicFileFormatContext;
        use usd_sdf::{FileFormatArguments, get_dynamic_file_format};

        let asset_path = payload.asset_path();

        // Get the dynamic file format
        let _format = match get_dynamic_file_format(asset_path, self.file_format_target.as_deref())
        {
            Some(f) => f,
            None => {
                // Not a dynamic format, fall back to regular processing
                self.add_payload_arc(node, payload, arc_info, arc_num);
                return;
            }
        };

        // Create context for composing field values
        let context = DynamicFileFormatContext::new(
            node.clone(),
            node.path().clone(),
            arc_num as i32,
            None, // previous_stack_frame
            None, // composed_field_names
            None, // composed_attribute_names
        );

        // Compose file format arguments from the context
        let _args = FileFormatArguments::new();
        let _dependency_context_data: Option<usd_vt::Value> = None;

        // The file format composes fields to generate arguments
        // In a full implementation, this would call:
        // format.compose_fields_for_file_format_arguments(
        //     asset_path, &mut context, &mut args, &mut dependency_context_data);

        // For now, we track the dependency but use the composed field names
        // from the context for dependency tracking
        let composed_fields: HashSet<Token> =
            context.composed_field_names().cloned().unwrap_or_default();
        let composed_attrs: HashSet<Token> = context
            .composed_attribute_names()
            .cloned()
            .unwrap_or_default();

        // Track dependencies for change processing
        if !composed_fields.is_empty() || !composed_attrs.is_empty() {
            // In full implementation, would add to outputs.dynamicFileFormatDependency
            // using the format interface and composed data
        }

        // Add the payload arc with computed arguments (if any)
        // The args would modify how the payload layer is opened
        self.add_payload_arc(node, payload, arc_info, arc_num);
    }

    /// Evaluates inherits on a node.
    pub fn eval_node_inherits(&mut self, node: &NodeRef) {
        if !node.is_valid() || !node.can_contribute_specs() {
            return;
        }

        // P1-14 FIX: Use the node's own layer stack, not root layer stack.
        // C++ _EvalNodeInherits uses node.GetLayerStack().
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let inherits = compose_site_inherits(&node_layer_stack, &node.path());

        if inherits.is_empty() {
            return;
        }

        // Process each inherit path
        for (i, inherit_path) in inherits.iter().enumerate() {
            // C++ _AddClassBasedArcs 3442-3452: validate arc targets prim path, no variant selection
            if !inherit_path.is_empty()
                && !(inherit_path.is_prim_path() && !inherit_path.contains_prim_variant_selection())
            {
                continue;
            }
            self.add_inherit_arc(node, inherit_path, i);
        }
    }

    /// Evaluates specializes on a node.
    pub fn eval_node_specializes(&mut self, node: &NodeRef) {
        if !node.is_valid() || !node.can_contribute_specs() {
            return;
        }

        // P1-14 FIX: Use the node's own layer stack, not root layer stack.
        // C++ _EvalNodeSpecializes uses node.GetLayerStack().
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let specializes = compose_site_specializes(&node_layer_stack, &node.path());

        if specializes.is_empty() {
            return;
        }

        // Process each specialize path
        for (i, spec_path) in specializes.iter().enumerate() {
            // C++ _AddClassBasedArcs 3442-3452: validate arc targets prim path, no variant selection
            if !spec_path.is_empty()
                && !(spec_path.is_prim_path() && !spec_path.contains_prim_variant_selection())
            {
                continue;
            }
            self.add_specialize_arc(node, spec_path, i);
        }
    }

    /// Evaluates variant sets on a node.
    ///
    /// C++ primIndex.cpp:4328 _EvalNodeVariantSets -> _EvalVariantSetsAtSite:
    ///   vsetNames = PcpComposeSiteVariantSets(node.GetLayerStack(), node.GetPath())
    ///   Creates one EvalNodeVariantAuthored task per variant set name.
    pub fn eval_node_variant_sets(&mut self, node: &NodeRef) {
        if !node.is_valid() || !node.can_contribute_specs() {
            return;
        }

        // P1-14 FIX: Use the NODE'S layer stack, not root layer stack.
        // C++ uses node.GetLayerStack() via _EvalVariantSetsAtSite.
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let vsets = compose_site_variant_sets(&node_layer_stack, &node.path());

        if vsets.is_empty() {
            return;
        }

        // C++ _EvalVariantSetsAtSite:4302 -- taskType = EvalNodeVariantAuthored
        // vset_path = sitePath (the node's path, not appended with variant selection)
        // Each variant set becomes its own task with its vset_num index.
        for (i, vset_name) in vsets.iter().enumerate() {
            // vset_path is node.path() -- the site path where the variantSet lives.
            // C++ passes sitePath (= node.GetPath()) as vsetPath in the task,
            // NOT path.AppendVariantSelection. The variant selection is only appended
            // when we actually add the arc in _AddVariantArc.
            self.add_task(Task::variant(
                TaskType::EvalNodeVariantAuthored,
                node.clone(),
                node.path(), // vset_path = site path (not variant-appended)
                vset_name.clone(),
                i as i32,
            ));
        }
    }

    /// Evaluates an authored variant selection.
    ///
    /// C++ primIndex.cpp:4403 _EvalNodeAuthoredVariant:
    ///   Calls _ComposeVariantSelection which searches ALL nodes in the prim index
    ///   in strength order for an authored selection. If none found, falls back to
    ///   EvalNodeVariantFallback task.
    ///   `is_ancestral` selects between _AddVariantArc and _AddAncestralVariantArc.
    pub fn eval_node_variant_authored(
        &mut self,
        node: &NodeRef,
        vset_path: &Path,
        vset_name: &str,
        vset_num: i32,
        is_ancestral: bool,
    ) {
        if !node.is_valid() {
            return;
        }

        // C++ _EvalNodeAuthoredVariant:4418 checks this before composing selection.
        if !node_can_contribute_ancestral_opinions(node, vset_path) {
            return;
        }

        // C++ _ComposeVariantSelection walks all nodes via traversal cache.
        let selection = self.compose_variant_selection_across_graph(node, vset_path, vset_name);
        if let Some(sel) = selection {
            // C++ 4440-4442: isAncestral ? _AddAncestralVariantArc : _AddVariantArc
            if is_ancestral {
                self.add_ancestral_variant_arc(node, vset_path, vset_name, &sel, vset_num);
            } else {
                self.add_variant_arc(node, vset_name, &sel, vset_num);
            }
        } else {
            // No authored selection found -- defer to fallback.
            // C++ 4432-4436: isAncestral ? AncestralVariantFallback : VariantFallback
            let fallback_type = if is_ancestral {
                TaskType::EvalNodeAncestralVariantFallback
            } else {
                TaskType::EvalNodeVariantFallback
            };
            self.add_task(Task::variant(
                fallback_type,
                node.clone(),
                vset_path.clone(),
                vset_name.to_string(),
                vset_num,
            ));
        }
    }

    /// Searches ALL nodes in the composition graph for an authored variant selection.
    ///
    /// C++ primIndex.cpp:4057 _ComposeVariantSelectionAcrossNodes:
    /// Walks nodes in strength order (root first), checking each node's layer stack
    /// for an authored variant selection for the given variant set.
    fn compose_variant_selection_across_graph(
        &self,
        node: &NodeRef,
        vset_path: &Path,
        vset_name: &str,
    ) -> Option<String> {
        // Root node is always at index 0. Walk from root in strength order.
        let root = NodeRef::new(self.graph.clone(), 0);

        // C++ _ComposeVariantSelection:4130 translates pathInNode up to root first,
        // then the traversal translates back down via each node's mapToParent inverse.
        // We must do the same: translate vset_path from the originating node's
        // namespace to the root namespace before searching.
        let stripped = vset_path.strip_all_variant_selections();
        let search_path = if node.node_index() == 0 {
            stripped
        } else {
            let (translated, _) =
                crate::path_translation::translate_path_from_node_to_root(node, &stripped);
            if translated.is_empty() {
                stripped
            } else {
                translated
            }
        };

        self.compose_variant_selection_in_subtree(&root, &search_path, vset_name)
    }

    /// Recursively searches for variant selection in a node subtree.
    ///
    /// C++ _ComposeVariantSelectionAcrossNodes uses Pcp_TraversalCache to
    /// walk nodes in strength order. We approximate with recursive descent.
    ///
    /// P0-5 FIX: Each node's search path is translated through its mapToParent
    /// as we descend from the root. C++ Pcp_TranslatePathFromNodeToRootOrClosestNode
    /// translates pathInNode up to the root, and then the traversal translates
    /// back down via each node's mapToParent inverse. For a simpler but correct
    /// approximation: translate the parent search_path through each child's
    /// map function to get the child-local path before searching.
    fn compose_variant_selection_in_subtree(
        &self,
        node: &NodeRef,
        search_path: &Path,
        vset_name: &str,
    ) -> Option<String> {
        if !node.is_valid() {
            return None;
        }

        // C++ _ComposeVariantSelectionForNode uses pathInNode (the translated
        // search path from the traversal cache, which preserves variant
        // selections). Check both the translated search_path (needed for
        // ancestral variants where the vset is at an ancestor prim) and
        // node.path() (needed for nested variants where the variant selection
        // field is at a variant-selected path like /Foo{which=A}/Number).
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        if let Some(sel) = compose_site_variant_selection(&node_layer_stack, search_path, vset_name)
        {
            return Some(sel);
        }
        let node_path = node.path();
        if node_path != *search_path {
            if let Some(sel) =
                compose_site_variant_selection(&node_layer_stack, &node_path, vset_name)
            {
                return Some(sel);
            }
        }

        // Recurse into children (children are weaker than parent).
        // Translate search_path through each child's mapToParent (inverse direction:
        // mapToParent maps child-space -> parent-space, so we use map_target_to_source
        // to go from parent-space back to child-space).
        for child in node.children() {
            let map = child.map_to_parent();
            // map_target_to_source: parent path -> child (source) path.
            // If identity or mapping succeeds, use translated path; else skip subtree.
            let child_path = if map.is_identity() {
                search_path.clone()
            } else {
                match map.map_target_to_source(search_path) {
                    Some(p) if !p.is_empty() => p,
                    _ => continue, // Path not mappable into this subtree -- prune
                }
            };
            if let Some(sel) =
                self.compose_variant_selection_in_subtree(&child, &child_path, vset_name)
            {
                return Some(sel);
            }
        }

        None
    }

    /// Evaluates a fallback variant selection.
    ///
    /// C++ primIndex.cpp:4445 _EvalNodeFallbackVariant:
    ///   1. Get available variant options from the node's layer stack.
    ///   2. Find the first fallback preference that exists in the options.
    ///   3. If none found, add EvalNodeVariantNoneFound as a placeholder.
    pub fn eval_node_variant_fallback(
        &mut self,
        node: &NodeRef,
        vset_path: &Path,
        vset_name: &str,
        vset_num: i32,
        is_ancestral: bool,
    ) {
        if !node.is_valid() {
            return;
        }

        // P1-13 FIX: Check _NodeCanContributeAncestralOpinions before evaluating.
        // C++ _EvalNodeFallbackVariant:4460 checks this.
        if !node_can_contribute_ancestral_opinions(node, vset_path) {
            return;
        }

        // NEW-2 FIX: Get the actual available variant options from the node's layer stack.
        // C++ calls PcpComposeSiteVariantSetOptions(node.layerStack, vsetPath, vset).
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let options = compose_site_variant_set_options(&node_layer_stack, vset_path, vset_name);

        // C++ _ChooseBestFallbackAmongOptions (primIndex.cpp:4197-4212):
        // iterate fallback preferences, return the first one that exists
        // in the available options. Returns empty string (None) if no match.
        // C++ does NOT pick an arbitrary option as fallback.
        let chosen = self
            .variant_fallbacks
            .get(vset_name)
            .and_then(|fallbacks| fallbacks.iter().find(|f| options.contains(*f)).cloned());

        if let Some(fallback) = chosen {
            // C++ 4486-4488: isAncestral ? _AddAncestralVariantArc : _AddVariantArc
            if is_ancestral {
                self.add_ancestral_variant_arc(node, vset_path, vset_name, &fallback, vset_num);
            } else {
                self.add_variant_arc(node, vset_name, &fallback, vset_num);
            }
        } else {
            // NEW-1 FIX: No fallback found -- add NoneFound as a placeholder.
            // C++ _EvalNodeFallbackVariant adds EvalNodeVariantNoneFound when no fallback
            // matches, so that RetryVariantTasks() has something to promote later.
            let none_found_type = if is_ancestral {
                TaskType::EvalNodeAncestralVariantNoneFound
            } else {
                TaskType::EvalNodeVariantNoneFound
            };
            self.add_task(Task::variant(
                none_found_type,
                node.clone(),
                vset_path.clone(),
                vset_name.to_string(),
                vset_num,
            ));
        }
    }

    /// Evaluates variant sets at ancestor paths.
    ///
    /// Matches C++ `_EvalNodeAncestralVariantSets` (primIndex.cpp:4373-4393):
    /// walks up from node.path() to root, calling _EvalVariantSetsAtSite for each ancestor.
    /// Skips ancestors where `_NodeCanContributeAncestralOpinions` returns false (P1-13 fix).
    fn eval_node_ancestral_variant_sets(&mut self, node: &NodeRef) {
        if !node.is_valid() {
            return;
        }

        // P1-14 FIX: Use node's own layer stack, not root layer stack.
        let node_layer_stack = node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());

        let mut path = node.path().get_parent_path();

        while !path.is_empty() && !path.is_absolute_root_path() {
            // P1-13 FIX: Skip ancestors where restriction depth prevents contribution.
            // C++ _NodeCanContributeAncestralOpinions checks GetSpecContributionRestrictedDepth.
            if node_can_contribute_ancestral_opinions(node, &path) {
                let variant_sets = compose_site_variant_sets(&node_layer_stack, &path);

                for (vset_num, vset_name) in variant_sets.iter().enumerate() {
                    self.add_task(Task::variant(
                        TaskType::EvalNodeAncestralVariantAuthored,
                        node.clone(),
                        path.clone(),
                        vset_name.clone(),
                        vset_num as i32,
                    ));
                }

                // C++ stops at first variant-selection ancestor path.
                if path.is_prim_variant_selection_path() {
                    break;
                }
            }

            path = path.get_parent_path();
        }
    }

    // ========================================================================
    // Arc Addition
    // ========================================================================

    /// Checks if adding an arc from parent to target_site would create a cycle.
    ///
    /// Per C++ primIndex.cpp _CheckForCycle: walks up the node ancestry and
    /// checks if the target site already appears. Variant arcs are exempt.
    /// Checks whether adding a child arc would introduce a cycle.
    ///
    /// C++ primIndex.cpp:1576-1693 `_CheckForCycle`:
    /// 1. Implied class arcs (inherit/specialize where parent != origin) under
    ///    relocate nodes are exempted — they're placeholders that don't add opinions.
    /// 2. Variant arcs are never cycles.
    /// 3. Walk parent ancestry comparing each node's site (layer stack + path)
    ///    against the target site using HasPrefix in both directions (not equality).
    ///    This catches /A/B adding arc to /A and /A adding arc to /A/B.
    ///
    /// C++ also walks across PcpPrimIndex_StackFrame boundaries for recursive
    /// composition via `PcpPrimIndex_StackFrameIterator::NextFrame()`, translating
    /// the child site path between frames via `ReplacePrefix`.
    fn check_for_cycle(
        &mut self,
        parent: &NodeRef,
        arc_type: crate::ArcType,
        target_site: &Site,
    ) -> bool {
        // C++ line 1608: variant arcs are never cycles
        if arc_type == crate::ArcType::Variant {
            return false;
        }

        // C++ line 1589-1601: implied class arcs under relocates are exempted.
        // An implied class arc is a class-based arc (inherit/specialize) where
        // parent != origin (it was propagated, not directly authored).
        // If walking up from parent through class arcs lands on a Relocate node,
        // this is a placeholder that doesn't contribute opinions — skip cycle check.
        if is_class_based_arc(arc_type) {
            // Walk up through consecutive class-based arcs
            let mut walk = parent.clone();
            loop {
                let p = walk.parent_node();
                if !p.is_valid() {
                    break;
                }
                if !is_class_based_arc(p.arc_type()) {
                    // Landed on a non-class arc. If it's Relocate, exempt.
                    if p.arc_type() == crate::ArcType::Relocate {
                        return false;
                    }
                    break;
                }
                walk = p;
            }
        }

        // C++ lines 1617-1667: walk through stack frames for cross-graph cycle detection.
        // Each frame represents a recursive call to compute_prim_index.
        // We check each graph in the chain, translating the child site path
        // between frames via ReplacePrefix when crossing frame boundaries.
        let mut child_site_path = target_site.path.clone();
        let child_site_ls = target_site.layer_stack_identifier.clone();

        // Use StackFrameIterator to walk across recursive composition boundaries
        use super::prim_index_stack_frame::PrimIndexStackFrameIterator;
        let mut frame_iter =
            PrimIndexStackFrameIterator::new(parent.clone(), self.previous_frame.clone());

        let mut found_cycle = false;

        // Outer loop: iterate across stack frames
        loop {
            if !frame_iter.node.is_valid() {
                break;
            }

            // Inner loop: _FindAncestorCycleInParentGraph
            // Walk ancestry within this graph
            let mut current = frame_iter.node.clone();
            loop {
                let current_ls = current
                    .layer_stack()
                    .map(|ls| ls.identifier().clone())
                    .unwrap_or_else(|| self.layer_stack.identifier().clone());

                // C++ _HasAncestorCycle: same layer stack + prefix in either direction
                if current_ls == child_site_ls
                    && (current.path().has_prefix(&child_site_path)
                        || child_site_path.has_prefix(&current.path()))
                {
                    found_cycle = true;
                    break;
                }

                let p = current.parent_node();
                if !p.is_valid() {
                    break;
                }
                current = p;
            }

            if found_cycle {
                break;
            }

            // C++ lines 1654-1666: translate child site path when crossing frame boundary.
            // The child site path needs to be converted to the path it will have
            // in the parent graph via ReplacePrefix.
            if let Some(ref frame) = frame_iter.previous_frame {
                let requested_path = &frame.requested_site.path;
                let current_graph_root = frame_iter.node.root_node().path();
                if current_graph_root == child_site_path {
                    child_site_path = requested_path.clone();
                } else {
                    child_site_path = child_site_path
                        .replace_prefix(&current_graph_root, requested_path)
                        .unwrap_or(child_site_path);
                }
            }

            // Step to parent graph
            frame_iter.next_frame();
        }

        if found_cycle {
            self.errors.push(ErrorType::ArcCycle);
        }

        found_cycle
    }

    /// Returns true if adding `site` under `parent` would duplicate an existing
    /// non-inert node in the current graph or any outer recursive stack frame.
    ///
    /// C++ primIndex.cpp:1791-1837 `_AddArc` duplicate-node scan.
    fn duplicate_node_exists_across_frames(
        &self,
        parent: &NodeRef,
        site: &Site,
        skip_duplicate_nodes: bool,
    ) -> bool {
        if !skip_duplicate_nodes {
            return self.graph.get_node_using_site_index(site).is_some();
        }

        use super::prim_index_stack_frame::PrimIndexStackFrameIterator;

        let mut site_in_current_graph = site.clone();
        let mut frame_iter =
            PrimIndexStackFrameIterator::new(parent.clone(), self.previous_frame.clone());

        while frame_iter.node.is_valid() {
            if let Some(graph) = frame_iter.node.owning_graph() {
                if graph
                    .get_node_using_site_index(&site_in_current_graph)
                    .is_some()
                {
                    return true;
                }
            }

            if let Some(ref frame) = frame_iter.previous_frame {
                let requested_path = &frame.requested_site.path;
                let current_graph_root = frame_iter.node.root_node().path();
                site_in_current_graph.path = requested_path
                    .replace_prefix(&current_graph_root, &site_in_current_graph.path)
                    .unwrap_or(site_in_current_graph.path);
            }

            frame_iter.next_frame();
        }

        false
    }

    /// Resolves an external asset path to a layer stack.
    ///
    /// Per C++ _EvalRefOrPayloadArcs: opens the layer via SdfLayer::FindOrOpen,
    /// creates a LayerStackIdentifier from it, and builds a new LayerStack.
    /// Returns None if the layer can't be opened (records InvalidAssetPath error).
    fn resolve_external_layer_stack(
        &mut self,
        asset_path: &str,
        parent: &NodeRef,
    ) -> Option<LayerStackRefPtr> {
        // C++ primIndex.cpp:2291-2294: "Relative asset paths will already have been
        // anchored to their source layers in PcpComposeSiteReferences, so we can
        // just call SdfLayer::FindOrOpen"
        let result = Layer::find_or_open(asset_path);

        match result {
            Ok(layer) => {
                // Build a new layer stack rooted at this layer
                let layer_stack = crate::LayerStack::from_root_layer(layer);
                Some(layer_stack)
            }
            Err(_) => {
                // Record error: invalid asset path
                eprintln!(
                    "PCP WARNING: Failed to open external layer '{}' referenced from {}",
                    asset_path,
                    parent.path().get_string()
                );
                self.errors.push(ErrorType::InvalidAssetPath);
                None
            }
        }
    }

    /// Reads the defaultPrim metadata from the root layer of a layer stack.
    ///
    /// Per C++ _GetDefaultPrimPath: calls layer->GetDefaultPrimAsPath().
    /// Returns the default prim path, or empty path if not set.
    fn get_default_prim_path(layer_stack: &LayerStackRefPtr) -> Path {
        if let Some(root_layer) = layer_stack.root_layer() {
            root_layer.get_default_prim_as_path()
        } else {
            Path::empty()
        }
    }

    /// C++ primIndex.cpp:2370-2383: Apply TCPS scaling when source layer
    /// and target layer stack have different TimeCodesPerSecond values.
    fn apply_tcps_scaling(
        &self,
        mut layer_offset: LayerOffset,
        arc_info: &ArcInfo,
        target_layer_stack: &LayerStackRefPtr,
    ) -> LayerOffset {
        if let Some(ref src_layer) = arc_info.source_layer {
            let src_tcps = src_layer.get_time_codes_per_second();
            let dest_tcps = target_layer_stack.get_time_codes_per_second();
            if (src_tcps - dest_tcps).abs() > 1e-10 {
                layer_offset.set_scale(layer_offset.scale() * src_tcps / dest_tcps);
            }
        }
        layer_offset
    }

    /// Common post-insert logic for all arc types.
    /// C++ _AddArc lines 1967-2065: new_nodes flag, dependency flags, value clips, pseudo-root inert.
    fn post_insert_arc_common(
        &self,
        new_node: &NodeRef,
        parent: &NodeRef,
        target_layer_stack: &LayerStackRefPtr,
        target_path: &Path,
    ) {
        // C++ 1968: mark graph as having new nodes
        self.graph.set_has_new_nodes(true);

        // C++ 1983-1989: default transitive dependency flags
        new_node.set_has_transitive_direct_dependency(true);
        new_node
            .set_has_transitive_ancestral_dependency(parent.has_transitive_ancestral_dependency());

        // C++ 1870-1874: compose value clips for non-inert nodes with specs (USD mode)
        if !new_node.is_inert() && new_node.has_specs() {
            new_node.set_has_value_clips(compose_site_has_value_clips(
                target_layer_stack,
                target_path,
            ));
        }

        // C++ 2059-2065: pseudo-root path → inert subtree (unresolved default prim placeholder)
        if new_node.path() == Path::absolute_root() {
            inert_subtree(new_node);
        }
    }

    // ========================================================================
    // Unified _AddArc (C++ primIndex.cpp:1729-2067)
    // ========================================================================

    /// Unified arc addition. Matches C++ _AddArc exactly.
    /// All arc types (reference, payload, inherit, specialize) go through here.
    fn add_arc(
        &mut self,
        arc_type: crate::ArcType,
        parent: &NodeRef,
        origin: &NodeRef,
        site: &Site,
        target_layer_stack: &LayerStackRefPtr,
        map_expr: MapExpression,
        arc_sibling_num: i32,
        namespace_depth: i32,
        opts: ArcOpts,
    ) -> NodeRef {
        if map_expr.is_null() {
            return NodeRef::invalid();
        }

        // C++ 1777: cycle check
        if self.check_for_cycle(parent, arc_type, site) {
            return NodeRef::invalid();
        }

        // C++ 1788-1840: skip duplicate nodes across stack frames
        let mut skip_dup = opts.skip_duplicate_nodes;
        if let Some(ref frame) = self.previous_frame {
            skip_dup |= frame.skip_duplicate_nodes;
        }
        if skip_dup && self.duplicate_node_exists_across_frames(parent, site, true) {
            return NodeRef::invalid();
        }

        // Set up the arc
        let mut arc = crate::Arc::new(arc_type);
        arc.set_parent_index(parent.node_index());
        arc.set_origin_index(origin.node_index());
        arc.set_sibling_num_at_origin(arc_sibling_num);
        arc.set_namespace_depth(namespace_depth);
        arc.set_map_to_parent_expr(map_expr);

        // C++ 1854-1960: create node
        let new_node = if !opts.include_ancestral_opinions {
            // Simple insert — no ancestral opinions
            parent.insert_child(site, &arc, Some(target_layer_stack.clone()))
        } else {
            // C++ 1901-1960: recursive composition for ancestral opinions
            use super::prim_index_stack_frame::PrimIndexStackFrame;

            let frame = PrimIndexStackFrame::new(
                site.clone(),
                parent.clone(),
                arc.clone(),
                // C++ stores previousFrame as a pointer (shared, not moved).
                // Clone instead of take so that subsequent add_arc / add_specialize_arc
                // calls in the same indexer still see self.previous_frame.
                self.previous_frame.as_ref().map(|f| f.clone()),
                None,
                skip_dup,
            );

            let target_ls = target_layer_stack.clone();
            let inputs = super::prim_index::PrimIndexInputs {
                usd: self.is_usd,
                variant_fallbacks: Some(self.variant_fallbacks.clone()),
                included_payloads: self.included_payloads.clone(),
                include_payload_predicate: self.include_payload_predicate.clone(),
                ..Default::default()
            };

            let child_outputs = super::prim_index::compute_prim_index_with_frame(
                &site.path,
                &target_ls,
                &inputs,
                super::prim_index::PrimIndexBuildOptions {
                    evaluate_implied_specializes: false,
                    evaluate_variants_and_dynamic_payloads: false,
                    root_node_should_contribute_specs: opts.direct_node_should_contribute_specs,
                },
                Some(Box::new(frame)),
            );

            self.errors.extend(child_outputs.all_errors);

            if let Some(cg) = child_outputs.prim_index.graph().cloned() {
                let sub_node_count = cg.num_nodes();
                let new_root = parent.insert_child_subgraph(cg, &arc, None);

                // C++ _BuildInitialPrimIndexFromAncestor carries previousFrame
                // as a shared pointer through every recursive ancestor level,
                // letting cross-frame duplicate detection see the parent graph
                // at every depth. Our iterative ancestor chain approximates
                // this but cannot fully replicate cross-frame visibility for
                // paths extended by AppendChildNameToAllSites at intermediate
                // levels. Compensate by marking as inert any non-root subgraph
                // node whose site already exists elsewhere in the graph.
                if opts.skip_duplicate_nodes && new_root.is_valid() && sub_node_count > 1 {
                    let start = new_root.node_index();
                    for ni in (start + 1)..(start + sub_node_count) {
                        let child_node = NodeRef::new(self.graph.clone(), ni);
                        if child_node.is_inert() {
                            continue;
                        }
                        if let Some(dup_idx) =
                            self.graph.get_node_using_site_index(&child_node.site())
                        {
                            if dup_idx < start || dup_idx >= start + sub_node_count {
                                inert_subtree(&child_node);
                            }
                        }
                    }
                }

                new_root
            } else {
                NodeRef::invalid()
            }
        };

        if !new_node.is_valid() {
            return NodeRef::invalid();
        }

        // C++ 1855-1895: post-insert for simple (non-ancestral) case
        if !opts.include_ancestral_opinions {
            if !opts.direct_node_should_contribute_specs {
                new_node.set_inert(true);
                new_node.set_spec_contribution_restricted_depth(1);
            }
            new_node.set_has_specs(compose_site_has_specs(target_layer_stack, &site.path));
            if !new_node.is_inert() && new_node.has_specs() && self.is_usd {
                new_node.set_has_value_clips(compose_site_has_value_clips(
                    target_layer_stack,
                    &site.path,
                ));
            }
        }

        // C++ 1979-2002: dependency flags
        if opts.copy_ancestor_flag_from_origin {
            new_node.set_is_due_to_ancestor(origin.is_due_to_ancestor());
            new_node
                .set_has_transitive_direct_dependency(origin.has_transitive_direct_dependency());
            new_node.set_has_transitive_ancestral_dependency(
                origin.has_transitive_ancestral_dependency(),
            );
        } else {
            new_node.set_has_transitive_direct_dependency(true);
            new_node.set_has_transitive_ancestral_dependency(
                parent.has_transitive_ancestral_dependency(),
            );
        }

        self.graph.set_has_new_nodes(true);

        // C++ 2017-2045: task filtering
        let skip_expressed = opts.include_ancestral_opinions;
        let include_ancestral_tasks = opts.include_ancestral_opinions;
        self.add_tasks_for_node_recursively(&new_node, include_ancestral_tasks, skip_expressed);

        // C++ 2047: class-based arc implied class tasks
        if is_class_based_arc(arc_type) {
            if let Some(base) = find_starting_node_for_implied_classes(&new_node) {
                self.add_task(Task::new(TaskType::EvalImpliedClasses, base));
            }
        } else if has_class_based_child(&new_node) {
            self.add_task(Task::new(TaskType::EvalImpliedClasses, new_node.clone()));
        }

        // C++ 2031: EvalImpliedSpecializes only with ancestral opinions
        if opts.include_ancestral_opinions && self.evaluate_implied_specializes {
            if has_specializes_child_in_subtree(&new_node) {
                self.add_task(Task::new(
                    TaskType::EvalImpliedSpecializes,
                    new_node.clone(),
                ));
            }
        }

        // C++ 2051-2058: permission enforcement
        if new_node.permission() == super::node::Permission::Private {
            inert_subtree(&new_node);
        }

        // C++ 2063: pseudo-root target → inert
        if new_node.path() == Path::absolute_root() {
            inert_subtree(&new_node);
        }

        new_node
    }

    /// Adds a reference arc to the graph.
    ///
    /// Per C++ _EvalRefOrPayloadArcs + _AddArc in primIndex.cpp:
    /// - Resolves external asset paths via Layer::find_or_open
    /// - Reads defaultPrim when prim path is empty
    /// - Checks for cycles before adding the arc
    /// - Uses reference's layer offset (composed with source layer stack offset)
    /// - Sets includeAncestralOpinions for non-root target paths
    fn add_reference_arc(
        &mut self,
        parent: &NodeRef,
        reference: &Reference,
        arc_info: &ArcInfo,
        arc_num: usize,
    ) {
        let asset_path = reference.asset_path();
        let prim_path = reference.prim_path().clone();
        let is_internal = asset_path.is_empty();

        // C++ 2206-2220: validate prim path (must be absolute prim path, no variant selections)
        if !prim_path.is_empty()
            && !(prim_path.is_absolute_path()
                && prim_path.is_prim_path()
                && !prim_path.contains_prim_variant_selection())
        {
            self.errors.push(ErrorType::InvalidPrimPath);
            return;
        }

        // C++ 2222-2245: validate layer offset, reset to identity if invalid
        let raw_offset = reference.layer_offset();
        let layer_offset = if raw_offset.scale() < 0.0
            || !raw_offset.is_valid()
            || !raw_offset.inverse().is_valid()
        {
            // Invalid offset: record error but continue with identity offset
            self.errors.push(ErrorType::InvalidReferenceOffset);
            LayerOffset::identity()
        } else {
            // C++ 2244: layerOffset = info.sourceLayerStackOffset * layerOffset
            arc_info.source_layer_stack_offset.compose(&raw_offset)
        };

        // Determine target layer stack
        let target_layer_stack = if is_internal {
            parent
                .layer_stack()
                .unwrap_or_else(|| self.layer_stack.clone())
        } else {
            match self.resolve_external_layer_stack(asset_path, parent) {
                Some(ls) => ls,
                None => return,
            }
        };

        // C++ 2370-2383: TCPS scaling for external refs
        let layer_offset = if !is_internal {
            self.apply_tcps_scaling(layer_offset, arc_info, &target_layer_stack)
        } else {
            layer_offset
        };

        // Determine prim path: explicit path, or defaultPrim from target layer
        let mut direct_node_should_contribute_specs = true;
        let target_path = if prim_path.is_empty() {
            let default_path = Self::get_default_prim_path(&target_layer_stack);
            if default_path.is_empty() {
                self.errors.push(ErrorType::InvalidPrimPath);
                direct_node_should_contribute_specs = false;
                Path::absolute_root()
            } else {
                default_path
            }
        } else {
            prim_path
        };
        let target_path = if is_internal {
            translate_target_path_into_parent_variant_namespace(parent, &target_path)
        } else {
            target_path
        };

        let target_site = Site::new(target_layer_stack.identifier().clone(), target_path.clone());

        if self.check_for_cycle(parent, crate::ArcType::Reference, &target_site) {
            return;
        }

        // C++ 665: target path in map uses variant-stripped parent path
        let map_target = parent.path().strip_all_variant_selections();
        let map_expr = create_arc_map_expression(&target_path, &map_target, layer_offset);

        let include_ancestral = target_path_requires_ancestral_opinions(&target_path);

        // C++ _EvalRefOrPayloadArcs always delegates to _AddArc which handles
        // both simple inserts and ancestral sub-builds, plus full post-insert
        // task scheduling (implied classes/specializes, ancestral variants).
        let opts = ArcOpts {
            include_ancestral_opinions: include_ancestral,
            direct_node_should_contribute_specs,
            ..Default::default()
        };

        let new_node = self.add_arc(
            crate::ArcType::Reference,
            parent,
            parent,
            &target_site,
            &target_layer_stack,
            map_expr,
            arc_num as i32,
            count_namespace_depth(&parent.path()),
            opts,
        );

        if new_node.is_valid() && is_internal {
            let map_expr = new_node.map_to_parent().add_root_identity();
            new_node.set_map_to_parent_expr(map_expr);
        }
    }

    /// Adds a payload arc to the graph.
    ///
    /// Per C++ _EvalRefOrPayloadArcs + _AddArc: same as references but:
    /// - Marks graph as having payloads
    /// - Resolves external asset paths via Layer::find_or_open
    /// - Reads defaultPrim when prim path is empty
    /// - Checks for cycles before adding
    fn add_payload_arc(
        &mut self,
        parent: &NodeRef,
        payload: &Payload,
        arc_info: &ArcInfo,
        arc_num: usize,
    ) {
        let asset_path = payload.asset_path();
        let prim_path = payload.prim_path().clone();
        let is_internal = asset_path.is_empty();

        // C++ 2206-2220: validate prim path
        if !prim_path.is_empty()
            && !(prim_path.is_absolute_path()
                && prim_path.is_prim_path()
                && !prim_path.contains_prim_variant_selection())
        {
            self.errors.push(ErrorType::InvalidPrimPath);
            return;
        }

        // C++ 2222-2245: validate layer offset
        let raw_offset = payload.layer_offset();
        let layer_offset = if raw_offset.scale() < 0.0
            || !raw_offset.is_valid()
            || !raw_offset.inverse().is_valid()
        {
            self.errors.push(ErrorType::InvalidReferenceOffset);
            LayerOffset::identity()
        } else {
            arc_info.source_layer_stack_offset.compose(&raw_offset)
        };

        // Mark graph as having payloads
        self.graph.set_has_payloads(true);

        let target_layer_stack = if is_internal {
            parent
                .layer_stack()
                .unwrap_or_else(|| self.layer_stack.clone())
        } else {
            match self.resolve_external_layer_stack(asset_path, parent) {
                Some(ls) => ls,
                None => return,
            }
        };

        // C++ 2370-2383: TCPS scaling for external payloads
        let layer_offset = if !is_internal {
            self.apply_tcps_scaling(layer_offset, arc_info, &target_layer_stack)
        } else {
            layer_offset
        };

        let mut direct_node_should_contribute_specs = true;
        let target_path = if prim_path.is_empty() {
            let default_path = Self::get_default_prim_path(&target_layer_stack);
            if default_path.is_empty() {
                self.errors.push(ErrorType::InvalidPrimPath);
                direct_node_should_contribute_specs = false;
                Path::absolute_root()
            } else {
                default_path
            }
        } else {
            prim_path
        };
        let target_path = if is_internal {
            translate_target_path_into_parent_variant_namespace(parent, &target_path)
        } else {
            target_path
        };

        let target_site = Site::new(target_layer_stack.identifier().clone(), target_path.clone());

        if self.check_for_cycle(parent, crate::ArcType::Payload, &target_site) {
            return;
        }

        // C++ 665: variant-stripped parent path for map
        let map_target = parent.path().strip_all_variant_selections();
        let map_expr = create_arc_map_expression(&target_path, &map_target, layer_offset);

        let include_ancestral = target_path_requires_ancestral_opinions(&target_path);

        // C++ _EvalRefOrPayloadArcs delegates to _AddArc for both
        // simple inserts and ancestral sub-builds.
        let opts = ArcOpts {
            include_ancestral_opinions: include_ancestral,
            direct_node_should_contribute_specs,
            ..Default::default()
        };

        let new_node = self.add_arc(
            crate::ArcType::Payload,
            parent,
            parent,
            &target_site,
            &target_layer_stack,
            map_expr,
            arc_num as i32,
            count_namespace_depth(&parent.path()),
            opts,
        );

        if new_node.is_valid() && is_internal {
            let map_expr = new_node.map_to_parent().add_root_identity();
            new_node.set_map_to_parent_expr(map_expr);
        }
    }

    /// Adds an inherit arc to the graph.
    ///
    /// C++ `_EvalNodeInherits` + `_AddClassBasedArc` (primIndex.cpp:3268, 3456-3460):
    /// - Layer stack: uses parent node's layer stack, not the root. This ensures
    ///   inherits authored inside referenced files target the correct layer stack.
    /// - Map expression: `_CreateMapExpressionForArc().AddRootIdentity()` adds
    ///   a root identity mapping (/ -> /) so paths outside the direct class->instance
    ///   pair can still be mapped through (needed for implied class propagation).
    /// C++ _AddClassBasedArcs + _AddClassBasedArc for inherit arcs.
    /// Computes map expression and site, then delegates to add_arc.
    fn add_inherit_arc(&mut self, parent: &NodeRef, inherit_path: &Path, arc_num: usize) {
        let target_layer_stack = parent
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let inherit_path =
            translate_target_path_into_parent_variant_namespace(parent, inherit_path);
        let target_site = Site::new(
            target_layer_stack.identifier().clone(),
            inherit_path.clone(),
        );
        // C++ _CreateMapExpressionForArc().AddRootIdentity()
        let map_expr =
            create_arc_map_expression(&inherit_path, &parent.path(), LayerOffset::identity())
                .add_root_identity();

        let opts = ArcOpts {
            include_ancestral_opinions: target_path_requires_ancestral_opinions(&inherit_path),
            ..Default::default()
        };

        self.add_arc(
            crate::ArcType::Inherit,
            parent,
            parent, // origin = parent for direct arcs
            &target_site,
            &target_layer_stack,
            map_expr,
            arc_num as i32,
            count_namespace_depth(&parent.path()),
            opts,
        );
    }

    /// Adds a specialize arc to the graph.
    ///
    /// C++ `_EvalNodeSpecializes` + `_AddClassBasedArc` (primIndex.cpp:3318-3373, 3456-3460):
    /// - Layer stack: uses parent node's layer stack (same as inherit).
    /// - Map expression: `_CreateMapExpressionForArc().AddRootIdentity()` (same as inherit).
    /// - Specializes under non-root parents are added as INERT placeholders
    ///   (no spec contribution, no tasks), then immediately propagated to root.
    ///   Only specializes directly under the root node contribute specs.
    /// C++ _AddClassBasedArcs + _AddClassBasedArc for specialize arcs.
    fn add_specialize_arc(&mut self, parent: &NodeRef, spec_path: &Path, arc_num: usize) {
        let target_layer_stack = parent
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let spec_path = translate_target_path_into_parent_variant_namespace(parent, spec_path);
        let target_site = Site::new(target_layer_stack.identifier().clone(), spec_path.clone());
        let map_expr =
            create_arc_map_expression(&spec_path, &parent.path(), LayerOffset::identity())
                .add_root_identity();
        let ns_depth = count_namespace_depth(&parent.path());

        // C++ _AddClassBasedArc 3318-3373: specializes under non-root parents
        // are inert placeholders immediately propagated to root.
        let add_inert_placeholder = !parent.is_root_node() || self.previous_frame.is_some();

        if add_inert_placeholder {
            // Placeholder: no specs, no tasks
            let placeholder_opts = ArcOpts {
                direct_node_should_contribute_specs: false,
                include_ancestral_opinions: false,
                skip_duplicate_nodes: false,
                ..Default::default()
            };
            let placeholder = self.add_arc(
                crate::ArcType::Specialize,
                parent,
                parent,
                &target_site,
                &target_layer_stack,
                map_expr,
                arc_num as i32,
                ns_depth,
                placeholder_opts,
            );
            if placeholder.is_valid() && !self.previous_frame.is_some() {
                if !is_relocates_placeholder_implied_arc(&placeholder) {
                    let propagated = self.propagate_specialize_to_root(&placeholder);

                    if propagated.is_valid()
                        && propagated.origin_node().node_index() != placeholder.node_index()
                    {
                        self.add_task(Task::new(TaskType::EvalImpliedClasses, placeholder.clone()));
                    }
                }
            }
        } else {
            // Parent IS root: normal contributing specialize via add_arc
            let opts = ArcOpts {
                include_ancestral_opinions: target_path_requires_ancestral_opinions(&spec_path),
                ..Default::default()
            };
            self.add_arc(
                crate::ArcType::Specialize,
                parent,
                parent,
                &target_site,
                &target_layer_stack,
                map_expr,
                arc_num as i32,
                ns_depth,
                opts,
            );
        }
    }

    /// Adds a variant arc to the graph.
    ///
    /// C++ primIndex.cpp:4215 _AddVariantArc:
    ///   - Variant path = parent.path.AppendVariantSelection(vset, vsel)
    ///   - MapExpr = Identity() -- variants do NOT remap namespace, they just
    ///     branch into a different section of layer storage (comment at 4220-4223).
    ///   - After successful addition, call RetryVariantTasks().
    fn add_variant_arc(
        &mut self,
        parent: &NodeRef,
        vset_name: &str,
        selection: &str,
        arc_num: i32,
    ) {
        // Construct variant path: /Prim{vset=sel}
        let variant_path = match parent.path().append_variant_selection(vset_name, selection) {
            Some(p) => p,
            None => return,
        };

        // Variants use the same layer stack as parent (node.GetLayerStack())
        let target_layer_stack = if let Some(ls) = parent.layer_stack() {
            ls
        } else {
            self.layer_stack.clone()
        };

        // Create site for the variant
        let target_site = Site::new(
            target_layer_stack.identifier().clone(),
            variant_path.clone(),
        );

        // Check for duplicate nodes
        if self.graph.get_node_using_site_index(&target_site).is_some() {
            return;
        }

        // Create the arc
        let mut arc = crate::Arc::new(crate::ArcType::Variant);
        arc.set_parent_index(parent.node_index());
        arc.set_origin_index(parent.node_index());
        arc.set_sibling_num_at_origin(arc_num);
        arc.set_namespace_depth(count_namespace_depth(&parent.path()));

        // P0-2 FIX: Variants use IDENTITY mapping, NOT a path-based map.
        // C++ primIndex.cpp:4229 -- mapExpr = PcpMapExpression::Identity()
        // Variants don't remap the scenegraph namespace; the variant selection
        // is encoded directly in the path (/Prim{vset=sel}).
        arc.set_map_to_parent(MapFunction::identity().clone());

        // Insert child node
        let new_node = parent.insert_child(&target_site, &arc, Some(target_layer_stack.clone()));

        if new_node.is_valid() {
            let has_specs = compose_site_has_specs(&target_layer_stack, &variant_path);
            new_node.set_has_specs(has_specs);
            new_node.set_origin_index(parent.node_index());

            self.post_insert_arc_common(&new_node, parent, &target_layer_stack, &variant_path);

            // C++ _AddVariantArc calls _AddArc with default opts (no includeAncestralOpinions).
            // includeAncestralOpinions=false: keep expressed arcs, skip ancestral tasks.
            self.add_tasks_for_node_filtered(&new_node, false);

            // C++ primIndex.cpp:4233 -- if (result) indexer.RetryVariantTasks()
            self.retry_variant_tasks();
        }
    }

    /// Adds an ancestral variant arc to the graph.
    ///
    /// C++ primIndex.cpp:4238 _AddAncestralVariantArc:
    ///   - varPath = node.GetPath().ReplacePrefix(vsetPath, vsetPath.AppendVariantSelection(vset, vsel))
    ///   - namespaceDepth = PcpNode_GetNonVariantPathElementCount(vsetPath)
    ///   - includeAncestralOpinions = true
    ///   - skipDuplicateNodes if class-based arc ancestor at depth 0
    fn add_ancestral_variant_arc(
        &mut self,
        parent: &NodeRef,
        vset_path: &Path,
        vset_name: &str,
        selection: &str,
        arc_num: i32,
    ) {
        // C++ 4245-4246: varPath = node.GetPath().ReplacePrefix(
        //     vsetPath, vsetPath.AppendVariantSelection(vset, vsel))
        let vset_with_sel = match vset_path.append_variant_selection(vset_name, selection) {
            Some(p) => p,
            None => return,
        };
        let variant_path = match parent.path().replace_prefix(vset_path, &vset_with_sel) {
            Some(p) if !p.is_empty() => p,
            _ => return,
        };

        // C++ 4247-4248: namespaceDepth from vsetPath, not parent.path()
        let namespace_depth = count_namespace_depth(vset_path);

        // C++ 4266-4275: skipDuplicateNodes if class-based arc ancestor at depth 0
        let skip_dups = {
            let mut n = parent.clone();
            let mut found = false;
            while !n.is_root_node() {
                if is_class_based_arc(n.arc_type())
                    && n.depth_below_introduction() == 0
                    && !n.is_inert()
                {
                    found = true;
                    break;
                }
                n = n.parent_node();
            }
            found
        };
        // C++ primIndex.cpp:1788: propagate from previous frame
        let skip_dups = skip_dups
            || self
                .previous_frame
                .as_ref()
                .map_or(false, |f| f.skip_duplicate_nodes);

        let target_layer_stack = parent
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let target_site = Site::new(
            target_layer_stack.identifier().clone(),
            variant_path.clone(),
        );

        // C++ primIndex.cpp:1791-1836: skip if duplicate node exists
        if self.duplicate_node_exists_across_frames(parent, &target_site, skip_dups) {
            return;
        }

        let mut arc = crate::Arc::new(crate::ArcType::Variant);
        arc.set_parent_index(parent.node_index());
        arc.set_origin_index(parent.node_index());
        arc.set_sibling_num_at_origin(arc_num);
        arc.set_namespace_depth(namespace_depth);
        arc.set_map_to_parent(MapFunction::identity().clone());

        let new_node = parent.insert_child(&target_site, &arc, Some(target_layer_stack.clone()));

        if new_node.is_valid() {
            let has_specs = compose_site_has_specs(&target_layer_stack, &variant_path);
            new_node.set_has_specs(has_specs);
            new_node.set_origin_index(parent.node_index());

            self.post_insert_arc_common(&new_node, parent, &target_layer_stack, &variant_path);

            // An ancestral variant arc introduces a new variant-selected site
            // (for example `/Model{v=sel}Child`) that was not directly scanned
            // before the variant selection existed. Keep ancestral tasks, but
            // do not suppress expressed arcs on this new node.
            self.add_tasks_for_node_recursively(&new_node, true, false);

            // C++ 4288: RetryVariantTasks
            self.retry_variant_tasks();
        }
    }

    // ========================================================================
    // Relocations
    // ========================================================================

    /// Evaluates relocations for a node.
    ///
    /// Relocations remap prim paths within a layer stack. If this node's path
    /// is the target of a relocation, we add a relocate arc back to the source.
    fn eval_node_relocations(&mut self, node: &NodeRef) {
        // C++ 2863: skip if node can't contribute specs (NOT has_specs — different!)
        // Nodes without specs can still have relocates that affect them.
        if !node.can_contribute_specs() {
            return;
        }

        // C++ 2875-2876: use the NODE's layer stack (not indexer's)
        let relocates = match node.layer_stack() {
            Some(ls) => ls.incremental_relocates_target_to_source(),
            None => return,
        };

        // Check if this node's path is a relocation target
        let node_path = node.path();
        let reloc_source = match relocates.get(&node_path) {
            Some(source) => source.clone(),
            None => return, // This node was not relocated
        };

        // C++ primIndex.cpp:2910-2944: before adding the relocate arc,
        // elide existing child subtrees that would provide duplicate opinions.
        // Variant children are allowed to continue (they can override relocated prims).
        // All other arc types (Relocate, Reference, Payload, Inherit, Specialize)
        // are elided because their opinions are superseded by the relocation source.
        for child in node.children() {
            match child.arc_type() {
                crate::ArcType::Variant => continue,
                crate::ArcType::Root => continue,
                _ => elide_subtree(&child),
            }
        }

        // Create the relocation arc back to the source
        self.add_relocate_arc(node, &reloc_source);
    }

    /// Adds a relocate arc from a target node to its source path.
    ///
    /// C++ primIndex.cpp:2980-2994:
    /// - `directNodeShouldContributeSpecs = false`: the relocation source node
    ///   itself doesn't contribute specs, but its children do via ancestral arcs.
    /// - `includeAncestralOpinions = true`: ancestral tasks are added so that
    ///   opinions from ancestors of the relocation source are included.
    /// - Uses `node.GetLayerStack()` for the site, not the root layer stack.
    fn add_relocate_arc(&mut self, parent: &NodeRef, source_path: &Path) {
        // C++ primIndex.cpp:2992: PcpLayerStackSite(node.GetLayerStack(), relocSource)
        let parent_ls = parent
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let target_site = Site::new(parent_ls.identifier().clone(), source_path.clone());

        if self.graph.get_node_using_site_index(&target_site).is_some() {
            return;
        }

        let mut arc = crate::Arc::new(crate::ArcType::Relocate);
        arc.set_parent_index(parent.node_index());
        arc.set_origin_index(parent.node_index());
        arc.set_sibling_num_at_origin(0);
        arc.set_namespace_depth(count_namespace_depth(&parent.path()));

        let map_func = MapFunction::identity().clone();
        arc.set_map_to_parent(map_func);

        let new_node = parent.insert_child(&target_site, &arc, Some(parent_ls));

        if new_node.is_valid() {
            // C++ opts.directNodeShouldContributeSpecs = false
            new_node.set_has_specs(false);
            new_node.set_origin_index(parent.node_index());

            // C++ opts.includeAncestralOpinions = true: add filtered tasks
            // so ancestral opinions of the relocation source are included
            let include_ancestral = !source_path.is_root_prim_path();
            self.add_tasks_for_node_filtered(&new_node, include_ancestral);
        }
    }

    /// Evaluates implied relocations for a node.
    ///
    /// This propagates relocate arcs up the composition graph.
    fn eval_implied_relocations(&mut self, node: &NodeRef) {
        // Only process direct relocate arcs (not those added due to ancestors)
        if node.arc_type() != crate::ArcType::Relocate {
            return;
        }

        // Check if this is a direct arc vs ancestral
        if node.is_due_to_ancestor() {
            return;
        }

        // Get parent and grandparent
        let parent = node.parent_node();
        if !parent.is_valid() {
            return;
        }

        let grandparent = parent.parent_node();
        if !grandparent.is_valid() {
            return;
        }

        // Map the relocation source path to the grandparent's namespace
        let map_to_parent = parent.map_to_parent();
        let gp_reloc_source = match map_to_parent.map_source_to_target(&node.path()) {
            Some(path) if !path.is_empty() => path,
            _ => return, // No valid mapping
        };

        // Check if already propagated
        for gp_child in grandparent.children() {
            if gp_child.path() == gp_reloc_source && gp_child.arc_type() == crate::ArcType::Relocate
            {
                return; // Already exists
            }
        }

        // Add the implied relocate arc
        self.add_relocate_arc(&grandparent, &gp_reloc_source);
    }

    // ========================================================================
    // Implied Classes (Inherits) Propagation
    // ========================================================================

    /// Evaluates implied classes for a node.
    ///
    /// Matches C++ `_EvalImpliedClasses`. Builds a transfer function from the
    /// node's map-to-parent (with added root identity) and propagates class-
    /// based children upward to the parent node.
    fn eval_implied_classes(&mut self, node: &NodeRef) {
        // Root node has no parent to propagate to
        if !node.parent_node().is_valid() {
            return;
        }

        // Optimization: early-out if there are no class arcs to propagate
        if !has_class_based_child(node) {
            return;
        }

        // Build transfer function: node.GetMapToParent().AddRootIdentity()
        // Adding root identity allows class arcs to cross reference namespace
        // boundaries (class arcs deliberately work this way).
        let transfer_func = node.map_to_parent().add_root_identity();

        let parent = node.parent_node();
        self.eval_implied_class_tree(&parent, node, &transfer_func, true);
    }

    /// Computes the implied class map expression for a given transfer and class arc.
    ///
    /// Matches C++ `_GetImpliedClass`:
    ///   if transfer.IsConstantIdentity(): return classArc
    ///   else: return transfer.Compose(classArc.Compose(transfer.Inverse())).AddRootIdentity()
    fn get_implied_class(transfer: &MapExpression, class_arc: &MapExpression) -> MapExpression {
        if transfer.is_constant_identity() {
            return class_arc.clone();
        }
        transfer
            .compose(&class_arc.compose(&transfer.inverse()))
            .add_root_identity()
    }

    /// Iteratively propagates class-based arcs from srcNode to destNode.
    ///
    /// Matches C++ `_EvalImpliedClassTree`. The `transfer_func` maps the
    /// srcNode namespace to the destNode namespace and is used to compute
    /// the implied class mapping via `_GetImpliedClass`.
    fn eval_implied_class_tree(
        &mut self,
        dest_node: &NodeRef,
        src_node: &NodeRef,
        transfer_func: &MapExpression,
        src_node_is_start_of_tree: bool,
    ) {
        // Iterative DFS to avoid stack overflow on deep class hierarchies.
        // Each frame corresponds to one recursive call in C++ _EvalImpliedClassTree.
        struct Frame {
            dest: NodeRef,
            src: NodeRef,
            transfer: MapExpression,
            is_start: bool,
        }

        let mut stack = vec![Frame {
            dest: dest_node.clone(),
            src: src_node.clone(),
            transfer: transfer_func.clone(),
            is_start: src_node_is_start_of_tree,
        }];

        while let Some(frame) = stack.pop() {
            let mut dest = frame.dest;
            let mut transfer = frame.transfer;
            let is_start = frame.is_start;
            let src = frame.src;

            // Handle relocate chain iteratively (C++ XXX:RelocatesSourceNodes).
            // Original code tail-recurses with parent; we loop up the chain.
            while dest.arc_type() == crate::ArcType::Relocate {
                let parent = dest.parent_node();
                if !parent.is_valid() {
                    break;
                }
                transfer = dest.map_to_parent().add_root_identity().compose(&transfer);
                self.add_task(Task::new(TaskType::EvalImpliedClasses, dest.clone()));
                dest = parent;
            }
            // If we couldn't escape the relocate chain (invalid parent), skip frame
            if dest.arc_type() == crate::ArcType::Relocate {
                continue;
            }

            // C++: if srcNode is a specialize, get children from the propagated
            // specializes node instead.
            let src_children: Vec<NodeRef> =
                if let Some(propagated) = get_propagated_specializes_node(&src) {
                    propagated.children()
                } else {
                    src.children()
                };

            for src_child in src_children {
                if !is_class_based_arc(src_child.arc_type()) {
                    continue;
                }

                // Skip ancestral class chain: don't re-propagate the srcNode->otherNode arc
                // when srcNode itself is a class arc at the same depth.
                if is_start
                    && is_class_based_arc(src.arc_type())
                    && src.depth_below_introduction() == src_child.depth_below_introduction()
                {
                    continue;
                }

                // Compute destClassFunc = _GetImpliedClass(transferFunc, srcChild.GetMapToParent())
                let dest_class_func =
                    Self::get_implied_class(&transfer, &src_child.map_to_parent());

                // Check if this implied class already exists under destNode (origin match
                // AND evaluated map must match, per C++).
                let dest_class_value = dest_class_func.evaluate();
                let mut dest_child: Option<NodeRef> = None;
                for existing_child in dest.children() {
                    let origin = existing_child.origin_node();
                    if origin.is_valid() && origin.node_index() == src_child.node_index() {
                        if existing_child.map_to_parent().evaluate() == dest_class_value {
                            dest_child = Some(existing_child);
                            break;
                        }
                    }
                }

                // Try to add the implied class arc if not already present.
                // Pass srcChild.GetSite() as ignoreIfSameAsSite per C++ 3707.
                if dest_child.is_none() {
                    let arc_type = src_child.arc_type();
                    let sibling_num = src_child.sibling_num_at_origin();
                    // C++ passes srcChild.GetSite() which uses the node's own layer stack
                    let src_child_ls_id = src_child
                        .layer_stack()
                        .map(|ls| ls.identifier().clone())
                        .unwrap_or_else(|| self.layer_stack.identifier().clone());
                    let ignore_site = Site::new(src_child_ls_id, src_child.path());
                    let implied_node = self.add_implied_class_arc(
                        &dest,
                        &src_child,
                        arc_type,
                        sibling_num,
                        &dest_class_func,
                        &ignore_site,
                    );
                    if implied_node.is_valid() {
                        dest_child = Some(implied_node);
                    }
                }

                // Push child frame instead of recursing on nested class hierarchy
                if let Some(ref dc) = dest_child {
                    if has_class_based_child(&src_child) {
                        let child_transfer = dest_class_func
                            .inverse()
                            .compose(&transfer.compose(&src_child.map_to_parent()));

                        let recurse_dest =
                            if let Some(propagated) = get_propagated_specializes_node(dc) {
                                propagated
                            } else {
                                dc.clone()
                            };
                        stack.push(Frame {
                            dest: recurse_dest,
                            src: src_child,
                            transfer: child_transfer,
                            is_start: false,
                        });
                    }
                }
            }
        }
    }

    /// Adds an implied class arc (inherit or specialize) under `parent`.
    ///
    /// Uses `dest_class_func` as the map-to-parent expression, matching C++
    /// `_AddClassBasedArc` (3208-3422).
    ///
    /// `ignore_if_same_as_site`: C++ 3404-3406 — if the resulting inheritSite
    /// equals this site, the node is a redundant implied arc and should not
    /// contribute specs (SetInert). Used for relocations/variants.
    fn add_implied_class_arc(
        &mut self,
        parent: &NodeRef,
        origin: &NodeRef,
        arc_type: crate::ArcType,
        sibling_num: i32,
        dest_class_func: &MapExpression,
        ignore_if_same_as_site: &Site,
    ) -> NodeRef {
        // Evaluate the map expression to get the actual MapFunction
        let map_func = dest_class_func.evaluate();

        // C++ _DetermineInheritPath: apply map inversely to parent's path
        let target_path = match map_func.map_target_to_source(&parent.path()) {
            Some(p) if !p.is_empty() => p,
            _ => return NodeRef::invalid(), // No appropriate site for inheriting
        };

        // C++ uses parent.GetLayerStack() for inheritSite, NOT root layer stack
        let parent_ls = parent
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let target_site = Site::new(parent_ls.identifier().clone(), target_path.clone());

        // C++ 3273-3310: _FindMatchingChild — check if child with same site exists.
        // This MUST be checked BEFORE specializes placeholder logic to prevent
        // duplicate creation and infinite task feedback loops.
        // For non-relocate parent: match by site. For relocate: match by map+depth.
        let parent_arc_type = parent.arc_type();
        for existing in parent.children() {
            if parent_arc_type == crate::ArcType::Relocate {
                // Relocate-specific matching: arcType + mapToParent + depthBelowIntroduction
                if existing.arc_type() == arc_type
                    && existing.map_to_parent().evaluate() == map_func
                    && existing.origin_node().depth_below_introduction()
                        == origin.depth_below_introduction()
                {
                    return existing; // Already exists
                }
            } else {
                // Non-relocate: match by site (C++ 3123-3125)
                if existing.site() == target_site {
                    // C++ 3288-3308: for specializes implied to the root, prefer
                    // the existing child only if its origin is at least as strong.
                    if is_specialize_arc(arc_type)
                        && parent.is_root_node()
                        && is_implied_class_based_arc(arc_type, parent, origin)
                    {
                        if compare_node_strength(origin, &existing.origin_node()) == -1 {
                            inert_subtree(&existing);
                        } else {
                            return existing;
                        }
                    } else {
                        return existing; // Already exists
                    }
                }
            }
        }

        // C++ 3404-3406: directNodeShouldContributeSpecs =
        //   (inheritPath != parent.GetPath()) && (inheritSite != ignoreIfSameAsSite)
        let should_contribute_specs =
            target_path != parent.path() && target_site != *ignore_if_same_as_site;

        // C++ 3413: skipDuplicateNodes = directNodeShouldContributeSpecs
        // When !shouldContribute, placeholder nodes needed for continued implied
        // class propagation — don't skip duplicates.
        if should_contribute_specs {
            if self.graph.get_node_using_site_index(&target_site).is_some() {
                return NodeRef::invalid();
            }
        }

        // C++ 3318-3373: specialize inert placeholder + immediate propagation
        if is_specialize_arc(arc_type) && !parent.is_root_node() {
            let mut arc = crate::Arc::new(arc_type);
            arc.set_parent_index(parent.node_index());
            arc.set_origin_index(origin.node_index());
            arc.set_sibling_num_at_origin(sibling_num);
            arc.set_namespace_depth(count_namespace_depth(&parent.path()));
            arc.set_map_to_parent(map_func);

            // C++ opts: directNodeShouldContributeSpecs=false, tasks=None
            let placeholder = parent.insert_child(&target_site, &arc, Some(parent_ls.clone()));
            if placeholder.is_valid() {
                placeholder.set_inert(true);
                placeholder.set_spec_contribution_restricted_depth(1);
                let has_specs = compose_site_has_specs(&parent_ls, &target_path);
                placeholder.set_has_specs(has_specs);
                placeholder.set_origin_index(origin.node_index());

                // C++ 3348-3366: propagate to root, handle pre-existing node case
                if !is_relocates_placeholder_implied_arc(&placeholder) {
                    let propagated = self.propagate_specialize_to_root(&placeholder);

                    // C++ 3362-3365: if pre-existing node found (origin != placeholder),
                    // add EvalImpliedClasses to the placeholder to continue propagation
                    if propagated.is_valid()
                        && propagated.origin_node().node_index() != placeholder.node_index()
                    {
                        self.add_task(Task::new(TaskType::EvalImpliedClasses, placeholder.clone()));
                    }

                    if propagated.is_valid() {
                        return propagated;
                    }
                }
            }
            return placeholder;
        }

        // Non-specialize or specialize at root: delegate to unified add_arc
        // C++ _AddClassBasedArc lines 3404-3422 sets these opts then calls _AddArc
        let opts = ArcOpts {
            direct_node_should_contribute_specs: should_contribute_specs,
            skip_duplicate_nodes: should_contribute_specs,
            include_ancestral_opinions: should_contribute_specs && !target_path.is_root_prim_path(),
            ..Default::default()
        };

        self.add_arc(
            arc_type,
            parent,
            origin,
            &target_site,
            &parent_ls,
            dest_class_func.clone(),
            sibling_num,
            count_namespace_depth(&parent.path()),
            opts,
        )
    }

    // ========================================================================
    // Implied Specializes Propagation
    // ========================================================================

    /// Evaluates implied specializes for a node.
    ///
    /// Specializes arcs need to be propagated to the root of the prim index
    /// so that all specializes opinions come after all other opinions
    /// (implementing the LIVRPS ordering where S is weakest).
    fn eval_implied_specializes(&mut self, node: &NodeRef) {
        // Root node has no parent - nothing to propagate
        if !node.parent_node().is_valid() {
            return;
        }

        // Find all specializes arcs in the subtree and propagate them to root
        self.find_and_propagate_specializes(node);
    }

    /// Iteratively finds specializes arcs and propagates them to the root.
    /// C++ primIndex.cpp:3885-3906 `_FindSpecializesToPropagateToRoot`.
    /// Converted from recursion to iterative stack to avoid stack overflow
    /// on deep hierarchies (caldera: 10k+ node depth).
    fn find_and_propagate_specializes(&mut self, node: &NodeRef) {
        let mut stack = vec![node.clone()];
        while let Some(current) = stack.pop() {
            // C++ 3890: skip relocates placeholder implied arcs
            if is_relocates_placeholder_implied_arc(&current) {
                continue;
            }

            // If this node is a specialize arc, propagate it to root
            if is_specialize_arc(current.arc_type()) {
                let _ = self.propagate_specialize_to_root(&current);
            }

            // Push children in reverse order to maintain DFS left-to-right order
            let children: Vec<NodeRef> = current.children();
            for child in children.into_iter().rev() {
                stack.push(child);
            }
        }
    }

    /// Propagates a specialize arc to the root of the prim index.
    /// Returns the propagated (or pre-existing) node.
    ///
    /// C++ primIndex.cpp:3832-3863 `_PropagateNodeToRoot`:
    /// 1. _FindMatchingChild on root — if match found, return existing (no tasks)
    /// 2. _AddArc with skipDuplicateNodes=true, copyAncestorFlagFromOrigin=true
    /// 3. includeAncestralOpinions = !srcNode.GetPath().IsRootPrimPath()
    fn propagate_specialize_to_root(&mut self, src_node: &NodeRef) -> NodeRef {
        let root = src_node.root_node();
        if !root.is_valid() || root.node_index() == src_node.node_index() {
            return NodeRef::invalid();
        }

        // C++ 3840-3843: _FindMatchingChild(root, srcNode.arcType, srcNode.GetSite(),
        //   srcNode.arcType, mapToRoot, srcNode.GetDepthBelowIntroduction())
        // Root is never a Relocate arc, so the check is simply: child.GetSite() == site
        let src_site = src_node.site();
        for existing in root.children() {
            if existing.site() == src_site {
                return existing; // C++ 3845: already propagated, return existing
            }
        }

        let src_site_path = src_node.path();
        let target_ls = src_node
            .layer_stack()
            .unwrap_or_else(|| self.layer_stack.clone());
        let target_site = Site::new(target_ls.identifier().clone(), src_site_path.clone());
        let map_to_root = src_node.map_to_root();

        // C++ uses skipDuplicateNodes=true here. Now that GetNodeUsingSite
        // ignores inert placeholders, we can match that behavior directly and
        // avoid introducing duplicate propagated specializes that already
        // exist elsewhere in the merged subgraph.
        let opts = ArcOpts {
            skip_duplicate_nodes: true,
            include_ancestral_opinions: !src_site_path.is_root_prim_path(),
            copy_ancestor_flag_from_origin: true,
            ..Default::default()
        };

        self.add_arc(
            src_node.arc_type(),
            &root,
            src_node,
            &target_site,
            &target_ls,
            map_to_root,
            src_node.sibling_num_at_origin(),
            count_namespace_depth(&root.path()),
            opts,
        )
    }

    // ========================================================================
    // Main Loop
    // ========================================================================

    /// Runs the indexer until all tasks are complete.
    pub fn run(&mut self) {
        while self.has_tasks() {
            let task = self.pop_task();
            self.process_task(&task);
        }
    }

    /// Processes a single task.
    fn process_task(&mut self, task: &Task) {
        match task.task_type {
            TaskType::EvalNodeRelocations => {
                self.eval_node_relocations(&task.node);
            }
            TaskType::EvalImpliedRelocations => {
                self.eval_implied_relocations(&task.node);
            }
            TaskType::EvalNodeReferences => {
                self.eval_node_references(&task.node);
            }
            TaskType::EvalNodePayloads => {
                self.eval_node_payloads(&task.node);
            }
            TaskType::EvalNodeInherits => {
                self.eval_node_inherits(&task.node);
            }
            TaskType::EvalNodeSpecializes => {
                self.eval_node_specializes(&task.node);
            }
            TaskType::EvalImpliedSpecializes => {
                self.eval_implied_specializes(&task.node);
            }
            TaskType::EvalImpliedClasses => {
                self.eval_implied_classes(&task.node);
            }
            TaskType::EvalNodeVariantSets => {
                self.eval_node_variant_sets(&task.node);
            }
            TaskType::EvalNodeVariantAuthored => {
                if let (Some(vset_path), Some(vset_name)) = (&task.vset_path, &task.vset_name) {
                    self.eval_node_variant_authored(
                        &task.node,
                        vset_path,
                        vset_name,
                        task.vset_num,
                        false,
                    );
                }
            }
            TaskType::EvalNodeVariantFallback => {
                if let (Some(vset_path), Some(vset_name)) = (&task.vset_path, &task.vset_name) {
                    self.eval_node_variant_fallback(
                        &task.node,
                        vset_path,
                        vset_name,
                        task.vset_num,
                        false,
                    );
                }
            }
            TaskType::EvalNodeAncestralVariantSets => {
                self.eval_node_ancestral_variant_sets(&task.node);
            }
            TaskType::EvalNodeAncestralVariantAuthored => {
                if let (Some(vset_path), Some(vset_name)) = (&task.vset_path, &task.vset_name) {
                    self.eval_node_variant_authored(
                        &task.node,
                        vset_path,
                        vset_name,
                        task.vset_num,
                        true,
                    );
                }
            }
            TaskType::EvalNodeAncestralVariantFallback => {
                if let (Some(vset_path), Some(vset_name)) = (&task.vset_path, &task.vset_name) {
                    self.eval_node_variant_fallback(
                        &task.node,
                        vset_path,
                        vset_name,
                        task.vset_num,
                        true,
                    );
                }
            }
            TaskType::EvalNodeAncestralVariantNoneFound | TaskType::EvalNodeVariantNoneFound => {
                // No variant found - nothing to do, composition continues
            }
            TaskType::EvalNodeAncestralDynamicPayloads => {
                // C++ _EvalNodeAncestralDynamicPayloads: walk ancestor paths
                self.eval_node_ancestral_dynamic_payloads(&task.node);
            }
            TaskType::EvalNodeDynamicPayloads => {
                // Evaluate dynamic payloads at node's own path
                self.eval_node_dynamic_payloads(&task.node);
            }
            TaskType::EvalUnresolvedPrimPathError => {
                // Error state - the prim path couldn't be resolved
                // Would typically log an error, but composition continues
            }
            TaskType::None => {}
        }
    }

    /// Consumes the indexer and returns the composition graph.
    ///
    /// This should be called after `run()` to obtain the completed graph.
    pub fn into_graph(self) -> Arc<PrimIndexGraph> {
        self.graph
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LayerStackIdentifier, Site};

    #[test]
    fn test_task_type_priority() {
        // Lower priority value = higher priority (processed first)
        assert!(TaskType::EvalNodeRelocations.priority() < TaskType::EvalNodeReferences.priority());
        assert!(TaskType::EvalNodeReferences.priority() < TaskType::EvalNodePayloads.priority());
        assert!(TaskType::EvalNodePayloads.priority() < TaskType::EvalNodeInherits.priority());
        assert!(TaskType::EvalNodeInherits.priority() < TaskType::EvalNodeSpecializes.priority());
    }

    #[test]
    fn test_task_ordering() {
        let node = NodeRef::invalid();
        let t1 = Task::new(TaskType::EvalNodeReferences, node.clone());
        let t2 = Task::new(TaskType::EvalNodePayloads, node.clone());

        // t1 should be processed before t2 (lower priority value)
        assert!(t1 > t2); // In BinaryHeap terms, greater = popped first (for max-heap, but we use reverse ordering)
    }

    #[test]
    fn test_indexer_task_queue() {
        let site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site.clone(), true);
        let layer_stack = crate::LayerStack::new(LayerStackIdentifier::new("test.usda"));

        let mut indexer = PrimIndexer::new(graph, site, layer_stack, true);

        // Initially no tasks
        assert!(!indexer.has_tasks());

        // Add a task
        let task = Task::new(TaskType::EvalNodeReferences, NodeRef::invalid());
        indexer.add_task(task);
        assert!(indexer.has_tasks());

        // Pop the task
        let popped = indexer.pop_task();
        assert_eq!(popped.task_type, TaskType::EvalNodeReferences);
        assert!(!indexer.has_tasks());
    }

    // =========================================================================
    // Tests for implied class propagation and LIVRPS task ordering
    // =========================================================================

    /// Verifies the implied-class transfer formula directly via MapExpression.
    ///
    /// Formula: implied = transfer.compose(classArc.compose(transfer.inverse())).add_root_identity()
    /// With identity transfer this simplifies to classArc.add_root_identity().
    #[test]
    fn test_implied_class_transfer_formula_identity_transfer() {
        use crate::{MapExpression, MapFunction};
        use std::collections::BTreeMap;
        use usd_sdf::{LayerOffset, Path};

        let mut pm = BTreeMap::new();
        pm.insert(Path::absolute_root(), Path::absolute_root());
        pm.insert(
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        );
        let class_arc =
            MapExpression::constant(MapFunction::create(pm, LayerOffset::identity()).unwrap());

        // identity transfer: result = class_arc.add_root_identity()
        let transfer = MapExpression::identity();
        let implied = transfer
            .compose(&class_arc.compose(&transfer.inverse()))
            .add_root_identity();

        assert_eq!(
            implied.map_source_to_target(&Path::from_string("/A").unwrap()),
            Path::from_string("/B"),
            "identity transfer must preserve classArc mapping"
        );
        assert!(implied.evaluate().has_root_identity());
    }

    /// Verifies the implied-class formula with a non-identity transfer (reference arc).
    ///
    /// transfer: / -> /, /Ref -> /Instance
    /// classArc: / -> /, /_class_Model -> /_class_Model
    /// Result must not be null and must have root identity.
    #[test]
    fn test_implied_class_transfer_formula_non_identity_transfer() {
        use crate::{MapExpression, MapFunction};
        use std::collections::BTreeMap;
        use usd_sdf::{LayerOffset, Path};

        let mut transfer_map = BTreeMap::new();
        transfer_map.insert(Path::absolute_root(), Path::absolute_root());
        transfer_map.insert(
            Path::from_string("/Ref").unwrap(),
            Path::from_string("/Instance").unwrap(),
        );
        let transfer = MapExpression::constant(
            MapFunction::create(transfer_map, LayerOffset::identity()).unwrap(),
        );

        let mut class_map = BTreeMap::new();
        class_map.insert(Path::absolute_root(), Path::absolute_root());
        class_map.insert(
            Path::from_string("/_class_Model").unwrap(),
            Path::from_string("/_class_Model").unwrap(),
        );
        let class_arc = MapExpression::constant(
            MapFunction::create(class_map, LayerOffset::identity()).unwrap(),
        );

        let implied = transfer
            .compose(&class_arc.compose(&transfer.inverse()))
            .add_root_identity();
        let fn_result = implied.evaluate();

        assert!(!fn_result.is_null(), "implied class must not be null");
        assert!(
            fn_result.has_root_identity(),
            "implied class must have root identity"
        );
    }

    /// Full LIVRPS priority ordering: each step has a strictly lower priority number
    /// than the next, ensuring correct composition order.
    #[test]
    fn test_task_priority_full_livrps_sequence() {
        let ordered = [
            TaskType::EvalNodeRelocations,
            TaskType::EvalImpliedRelocations,
            TaskType::EvalNodeReferences,
            TaskType::EvalNodePayloads,
            TaskType::EvalNodeInherits,
            TaskType::EvalNodeSpecializes,
            TaskType::EvalImpliedSpecializes,
            TaskType::EvalImpliedClasses,
            TaskType::EvalNodeAncestralVariantSets,
        ];
        for w in ordered.windows(2) {
            assert!(
                w[0].priority() < w[1].priority(),
                "{:?} (pri={}) must be lower than {:?} (pri={})",
                w[0],
                w[0].priority(),
                w[1],
                w[1].priority()
            );
        }
    }

    /// Implied tasks (EvalImpliedClasses, EvalImpliedSpecializes) are deduplicated.
    /// Non-implied tasks (including all variant tasks) are NOT deduplicated.
    #[test]
    fn test_indexer_deduplicates_implied_tasks_only() {
        let site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site.clone(), true);
        let layer_stack = crate::LayerStack::new(LayerStackIdentifier::new("test.usda"));
        let mut indexer = PrimIndexer::new(graph, site, layer_stack, true);

        let node = NodeRef::invalid();

        // Implied tasks ARE deduplicated.
        indexer.add_task(Task::new(TaskType::EvalImpliedClasses, node.clone()));
        indexer.add_task(Task::new(TaskType::EvalImpliedClasses, node.clone()));
        let t1 = indexer.pop_task();
        assert_eq!(t1.task_type, TaskType::EvalImpliedClasses);
        assert!(
            !indexer.has_tasks(),
            "duplicate EvalImpliedClasses must be deduplicated"
        );

        // Non-implied tasks (e.g. variant tasks) are NOT deduplicated.
        // C++ primIndex.cpp:1275 -- variant tasks are never in taskUniq.
        indexer.add_task(Task::variant(
            TaskType::EvalNodeVariantAuthored,
            node.clone(),
            Path::from_string("/World").unwrap(),
            "lod".to_string(),
            0,
        ));
        indexer.add_task(Task::variant(
            TaskType::EvalNodeVariantAuthored,
            node.clone(),
            Path::from_string("/World").unwrap(),
            "shading".to_string(),
            1,
        ));
        // Both variant tasks must be present.
        let mut count = 0;
        while indexer.has_tasks() {
            let t = indexer.pop_task();
            assert_eq!(t.task_type, TaskType::EvalNodeVariantAuthored);
            count += 1;
        }
        assert_eq!(count, 2, "two distinct variant tasks must both be enqueued");
    }

    /// add_tasks_for_root_node schedules tasks based on _ScanArcs preflight.
    /// With no real layer content, only EvalNodeRelocations is always added.
    /// Other tasks require actual field data in the layer (C++ _ScanArcs parity).
    #[test]
    fn test_add_tasks_for_root_node_schedules_initial_tasks() {
        let site = Site::new(
            LayerStackIdentifier::new("test.usda"),
            Path::from_string("/World").unwrap(),
        );
        let graph = PrimIndexGraph::new(site.clone(), true);
        let layer_stack = crate::LayerStack::new(LayerStackIdentifier::new("test.usda"));
        let mut indexer = PrimIndexer::new(graph.clone(), site, layer_stack, true);

        let root = graph.root_node();
        root.set_has_specs(true);
        indexer.add_tasks_for_root_node(&root);

        let mut seen = std::collections::HashSet::new();
        while indexer.has_tasks() {
            seen.insert(indexer.pop_task().task_type);
        }

        // C++ _ScanArcs: EvalNodeRelocations only scheduled if layer stack has relocates
        assert!(
            !seen.contains(&TaskType::EvalNodeRelocations),
            "relocations not scheduled without relocates in layer stack"
        );

        // Other tasks are NOT scheduled without real layer fields
        // (C++ _ScanArcs only adds tasks when HasField returns true)
        assert!(
            !seen.contains(&TaskType::EvalNodeReferences),
            "references not scheduled without layer data"
        );
        assert!(
            !seen.contains(&TaskType::EvalNodeAncestralVariantSets),
            "ancestral tasks excluded for root init"
        );
    }
}
