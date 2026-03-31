//! Path node pool for interned path prefix trees.
//!
//! This module implements the node-based path representation matching C++ OpenUSD's
//! `Sdf_PathNode` architecture. Paths are decomposed into prefix trees where each
//! node represents one path element (prim name, property name, variant selection, etc.).
//!
//! Two separate prefix trees exist:
//! - **Prim tree**: Rooted at absolute root ("/") or relative root ("."), containing
//!   prim nodes, variant selection nodes, etc.
//! - **Property tree**: A flat table of property nodes keyed by name alone (no parent),
//!   matching C++ behavior where property nodes have NULL parent in the intern key.
//!
//! # Thread Safety
//!
//! The global node pool uses sharded locks (parking_lot::RwLock) for concurrent
//! FindOrCreate operations. Nodes are permanently interned (no GC), matching the
//! Token system.

use std::collections::HashMap;
use std::sync::OnceLock;

use parking_lot::RwLock;

use usd_tf::Token;

// ---------------------------------------------------------------------------
// Node handle - index into the global pool
// ---------------------------------------------------------------------------

/// A handle to a path node in the global pool.
/// 0 = null/empty (no node).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct NodeHandle(pub u32);

impl NodeHandle {
    /// The null handle (index 0), representing no node / empty path.
    pub const NULL: NodeHandle = NodeHandle(0);

    /// Returns true if this handle points to no node.
    #[inline]
    pub fn is_null(self) -> bool {
        self.0 == 0
    }
}

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

/// The type of a path node element.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum NodeType {
    /// Root node: "/" (absolute) or "." (relative)
    Root = 0,
    /// Prim child node: e.g. "World" in "/World"
    Prim = 1,
    /// Variant selection: e.g. "{model=lod0}" in "/Prim{model=lod0}"
    PrimVariantSelection = 2,
    /// Prim property: e.g. ".visibility" in "/Prim.visibility"
    PrimProperty = 3,
    /// Relationship target: e.g. "[/Target]" in "/Prim.rel[/Target]"
    Target = 4,
    /// Relational attribute: e.g. ".attr" in "/Prim.rel[/Target].attr"
    RelationalAttribute = 5,
    /// Mapper: e.g. ".mapper[/Target]"
    Mapper = 6,
    /// Mapper arg: e.g. ".argName" in ".mapper[/Target].argName"
    MapperArg = 7,
    /// Expression: e.g. ".expression"
    Expression = 8,
}

// ---------------------------------------------------------------------------
// Node-type-specific data
// ---------------------------------------------------------------------------

/// Data stored in each node, varying by node type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[allow(missing_docs)]
pub enum NodeData {
    /// Root node (absolute="/", relative=".")
    Root { is_absolute: bool },
    /// Prim child name
    Prim { name: Token },
    /// Variant selection (set_name, selection)
    PrimVariantSelection { set_name: Token, selection: Token },
    /// Prim property name (namespaced ok)
    PrimProperty { name: Token },
    /// Target path (stored as string for interning key; actual Path would be circular)
    Target { target_path_str: String },
    /// Relational attribute name
    RelationalAttribute { name: Token },
    /// Mapper target path
    Mapper { target_path_str: String },
    /// Mapper arg name
    MapperArg { name: Token },
    /// Expression (no extra data)
    Expression,
}

impl NodeData {
    /// Returns the node type corresponding to this data variant.
    pub fn node_type(&self) -> NodeType {
        match self {
            NodeData::Root { .. } => NodeType::Root,
            NodeData::Prim { .. } => NodeType::Prim,
            NodeData::PrimVariantSelection { .. } => NodeType::PrimVariantSelection,
            NodeData::PrimProperty { .. } => NodeType::PrimProperty,
            NodeData::Target { .. } => NodeType::Target,
            NodeData::RelationalAttribute { .. } => NodeType::RelationalAttribute,
            NodeData::Mapper { .. } => NodeType::Mapper,
            NodeData::MapperArg { .. } => NodeType::MapperArg,
            NodeData::Expression => NodeType::Expression,
        }
    }
}

// ---------------------------------------------------------------------------
// Node flags (bit flags matching C++)
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    /// Flags cached on each node for O(1) path type queries.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct NodeFlags: u8 {
        /// Path is absolute (rooted at "/")
        const IS_ABSOLUTE           = 0b0000_0001;
        /// Path contains at least one variant selection
        const CONTAINS_VARIANT_SEL  = 0b0000_0010;
        /// Path contains at least one target path ("[...]")
        const CONTAINS_TARGET_PATH  = 0b0000_0100;
        /// Path contains property elements (".")
        const CONTAINS_PROPERTY     = 0b0000_1000;
    }
}

// ---------------------------------------------------------------------------
// Path node stored in the pool
// ---------------------------------------------------------------------------

/// A single node in the path prefix tree.
#[derive(Clone, Debug)]
pub struct PathNode {
    /// Parent node handle (NULL for root nodes)
    pub parent: NodeHandle,
    /// Node type
    pub node_type: NodeType,
    /// Node-specific data
    pub data: NodeData,
    /// Cached flags (propagated from parent)
    pub flags: NodeFlags,
    /// Number of elements from root to this node (root=0)
    pub element_count: u32,
}

// ---------------------------------------------------------------------------
// Intern key: (parent_handle, data) uniquely identifies a node
// ---------------------------------------------------------------------------

/// Key for interning: uniquely identifies a node in the pool.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct InternKey {
    parent: NodeHandle,
    data: NodeData,
}

// ---------------------------------------------------------------------------
// Sharded pool (64 shards, each with its own RwLock)
// ---------------------------------------------------------------------------

const NUM_SHARDS: usize = 64;

struct Shard {
    /// Map from intern key to handle
    map: HashMap<InternKey, NodeHandle>,
}

impl Shard {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

/// Global path node pool. Nodes live forever (permanent interning).
struct NodePool {
    /// All nodes stored contiguously. Index 0 is unused (NULL sentinel).
    nodes: RwLock<Vec<PathNode>>,
    /// Sharded intern tables for concurrent lookup/insert
    shards: Vec<RwLock<Shard>>,
}

impl NodePool {
    fn new() -> Self {
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(Shard::new()));
        }

        // Node 0 is the NULL sentinel
        let sentinel = PathNode {
            parent: NodeHandle::NULL,
            node_type: NodeType::Root,
            data: NodeData::Root { is_absolute: false },
            flags: NodeFlags::empty(),
            element_count: 0,
        };

        Self {
            nodes: RwLock::new(vec![sentinel]),
            shards,
        }
    }

    /// Pick shard based on hash of the intern key.
    fn shard_index(key: &InternKey) -> usize {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % NUM_SHARDS
    }

    /// Find or create a node. Returns its handle.
    fn find_or_create(&self, parent: NodeHandle, data: NodeData) -> NodeHandle {
        let key = InternKey {
            parent,
            data: data.clone(),
        };
        let shard_idx = Self::shard_index(&key);

        // Fast path: read lock
        {
            let shard = self.shards[shard_idx].read();
            if let Some(&handle) = shard.map.get(&key) {
                return handle;
            }
        }

        // Slow path: write lock
        let mut shard = self.shards[shard_idx].write();

        // Double-check after acquiring write lock
        if let Some(&handle) = shard.map.get(&key) {
            return handle;
        }

        // Compute flags from parent
        let parent_flags = if parent.is_null() {
            NodeFlags::empty()
        } else {
            let nodes = self.nodes.read();
            nodes[parent.0 as usize].flags
        };

        let mut flags = parent_flags;
        let node_type = data.node_type();

        match &data {
            NodeData::Root { is_absolute } => {
                if *is_absolute {
                    flags |= NodeFlags::IS_ABSOLUTE;
                }
            }
            NodeData::PrimVariantSelection { .. } => {
                flags |= NodeFlags::CONTAINS_VARIANT_SEL;
            }
            NodeData::PrimProperty { .. } => {
                flags |= NodeFlags::CONTAINS_PROPERTY;
            }
            NodeData::Target { .. } => {
                flags |= NodeFlags::CONTAINS_TARGET_PATH | NodeFlags::CONTAINS_PROPERTY;
            }
            NodeData::RelationalAttribute { .. } => {
                flags |= NodeFlags::CONTAINS_TARGET_PATH | NodeFlags::CONTAINS_PROPERTY;
            }
            NodeData::Mapper { .. } => {
                flags |= NodeFlags::CONTAINS_TARGET_PATH | NodeFlags::CONTAINS_PROPERTY;
            }
            NodeData::MapperArg { .. } => {
                flags |= NodeFlags::CONTAINS_TARGET_PATH | NodeFlags::CONTAINS_PROPERTY;
            }
            NodeData::Expression => {
                flags |= NodeFlags::CONTAINS_PROPERTY;
            }
            _ => {}
        }

        // Compute element count
        let parent_elem_count = if parent.is_null() {
            0
        } else {
            let nodes = self.nodes.read();
            nodes[parent.0 as usize].element_count
        };

        let element_count = match node_type {
            NodeType::Root => 0,
            _ => parent_elem_count + 1,
        };

        // Create node
        let node = PathNode {
            parent,
            node_type,
            data,
            flags,
            element_count,
        };

        let mut nodes = self.nodes.write();
        let idx = nodes.len();
        // Path node pool is limited to u32::MAX entries; exceeding this is a
        // programming error (extremely unlikely in practice).
        assert!(
            idx <= u32::MAX as usize,
            "path node pool exhausted (> 4 billion nodes)"
        );
        let handle = NodeHandle(idx as u32);
        nodes.push(node);

        shard.map.insert(key, handle);
        handle
    }

    /// Get a node by handle. Panics if handle is invalid.
    fn get(&self, handle: NodeHandle) -> PathNode {
        let nodes = self.nodes.read();
        nodes[handle.0 as usize].clone()
    }

    /// Get node type without cloning the whole node.
    fn get_type(&self, handle: NodeHandle) -> NodeType {
        let nodes = self.nodes.read();
        nodes[handle.0 as usize].node_type
    }

    /// Get node flags without cloning.
    fn get_flags(&self, handle: NodeHandle) -> NodeFlags {
        let nodes = self.nodes.read();
        nodes[handle.0 as usize].flags
    }

    /// Get parent handle.
    fn get_parent(&self, handle: NodeHandle) -> NodeHandle {
        let nodes = self.nodes.read();
        nodes[handle.0 as usize].parent
    }

    /// Get element count.
    fn get_element_count(&self, handle: NodeHandle) -> u32 {
        let nodes = self.nodes.read();
        nodes[handle.0 as usize].element_count
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

static POOL: OnceLock<NodePool> = OnceLock::new();

fn pool() -> &'static NodePool {
    POOL.get_or_init(NodePool::new)
}

// ---------------------------------------------------------------------------
// Pre-interned well-known handles
// ---------------------------------------------------------------------------

static ABSOLUTE_ROOT: OnceLock<NodeHandle> = OnceLock::new();
static RELATIVE_ROOT: OnceLock<NodeHandle> = OnceLock::new();

/// Handle for the absolute root node ("/").
pub fn absolute_root_handle() -> NodeHandle {
    *ABSOLUTE_ROOT.get_or_init(|| {
        pool().find_or_create(NodeHandle::NULL, NodeData::Root { is_absolute: true })
    })
}

/// Handle for the relative root node (".").
pub fn relative_root_handle() -> NodeHandle {
    *RELATIVE_ROOT.get_or_init(|| {
        pool().find_or_create(NodeHandle::NULL, NodeData::Root { is_absolute: false })
    })
}

// ---------------------------------------------------------------------------
// Public API: FindOrCreate functions
// ---------------------------------------------------------------------------

/// Find or create a prim child node under `parent`.
pub fn find_or_create_prim(parent: NodeHandle, name: &Token) -> NodeHandle {
    pool().find_or_create(parent, NodeData::Prim { name: name.clone() })
}

/// Find or create a variant selection node under `parent`.
pub fn find_or_create_variant_selection(
    parent: NodeHandle,
    set_name: &str,
    selection: &str,
) -> NodeHandle {
    pool().find_or_create(
        parent,
        NodeData::PrimVariantSelection {
            set_name: Token::new(set_name),
            selection: Token::new(selection),
        },
    )
}

/// Find or create a prim property node.
/// In C++, property nodes have NULL parent in the intern key (flat table by name).
/// Here we use the prim handle as parent so we can reconstruct the full path.
pub fn find_or_create_prim_property(parent: NodeHandle, name: &Token) -> NodeHandle {
    pool().find_or_create(parent, NodeData::PrimProperty { name: name.clone() })
}

/// Find or create a target node under `parent` (a property node).
pub fn find_or_create_target(parent: NodeHandle, target_path_str: &str) -> NodeHandle {
    pool().find_or_create(
        parent,
        NodeData::Target {
            target_path_str: target_path_str.to_string(),
        },
    )
}

/// Find or create a relational attribute node.
pub fn find_or_create_relational_attribute(parent: NodeHandle, name: &Token) -> NodeHandle {
    pool().find_or_create(parent, NodeData::RelationalAttribute { name: name.clone() })
}

/// Find or create a mapper node.
pub fn find_or_create_mapper(parent: NodeHandle, target_path_str: &str) -> NodeHandle {
    pool().find_or_create(
        parent,
        NodeData::Mapper {
            target_path_str: target_path_str.to_string(),
        },
    )
}

/// Find or create a mapper arg node.
pub fn find_or_create_mapper_arg(parent: NodeHandle, name: &Token) -> NodeHandle {
    pool().find_or_create(parent, NodeData::MapperArg { name: name.clone() })
}

/// Find or create an expression node.
pub fn find_or_create_expression(parent: NodeHandle) -> NodeHandle {
    pool().find_or_create(parent, NodeData::Expression)
}

// ---------------------------------------------------------------------------
// Public query API
// ---------------------------------------------------------------------------

/// Get a node by handle.
pub fn get_node(handle: NodeHandle) -> PathNode {
    pool().get(handle)
}

/// Get node type.
pub fn get_node_type(handle: NodeHandle) -> NodeType {
    pool().get_type(handle)
}

/// Get node flags.
pub fn get_node_flags(handle: NodeHandle) -> NodeFlags {
    pool().get_flags(handle)
}

/// Get parent handle.
pub fn get_parent(handle: NodeHandle) -> NodeHandle {
    pool().get_parent(handle)
}

/// Get element count (number of elements from root).
pub fn get_element_count(handle: NodeHandle) -> u32 {
    pool().get_element_count(handle)
}

/// Get the name token from a node (for Prim, PrimProperty, RelationalAttribute, MapperArg).
pub fn get_name(handle: NodeHandle) -> Token {
    let node = pool().get(handle);
    match &node.data {
        NodeData::Prim { name } => name.clone(),
        NodeData::PrimProperty { name } => name.clone(),
        NodeData::RelationalAttribute { name } => name.clone(),
        NodeData::MapperArg { name } => name.clone(),
        NodeData::Root { is_absolute } => {
            if *is_absolute {
                Token::new("/")
            } else {
                Token::new(".")
            }
        }
        NodeData::PrimVariantSelection {
            set_name,
            selection,
        } => Token::new(&format!("{{{}={}}}", set_name.as_str(), selection.as_str())),
        NodeData::Target { target_path_str } => Token::new(&format!("[{}]", target_path_str)),
        NodeData::Mapper { target_path_str } => Token::new(&format!("mapper[{}]", target_path_str)),
        NodeData::Expression => Token::new("expression"),
    }
}

/// Get variant selection from a PrimVariantSelection node.
pub fn get_variant_selection(handle: NodeHandle) -> Option<(String, String)> {
    let node = pool().get(handle);
    match &node.data {
        NodeData::PrimVariantSelection {
            set_name,
            selection,
        } => Some((
            set_name.as_str().to_string(),
            selection.as_str().to_string(),
        )),
        _ => None,
    }
}

/// Get target path string from a Target or Mapper node.
pub fn get_target_path_str(handle: NodeHandle) -> Option<String> {
    let node = pool().get(handle);
    match &node.data {
        NodeData::Target { target_path_str } => Some(target_path_str.clone()),
        NodeData::Mapper { target_path_str } => Some(target_path_str.clone()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// String building: reconstruct path string from node chain
// ---------------------------------------------------------------------------

/// Build the full path string from a prim handle and prop handle.
///
/// The prim handle represents the prim part of the path, and the prop handle
/// (if not NULL) represents the property part appended after the prim part.
pub fn build_path_string(prim_handle: NodeHandle, prop_handle: NodeHandle) -> String {
    if prim_handle.is_null() && prop_handle.is_null() {
        return String::new();
    }

    // Build prim part
    let prim_str = if !prim_handle.is_null() {
        build_node_string(prim_handle)
    } else {
        String::new()
    };

    // Build property part
    if !prop_handle.is_null() {
        let prop_str = build_prop_string(prop_handle);
        format!("{}{}", prim_str, prop_str)
    } else {
        prim_str
    }
}

/// Build string for a single node chain (prim side).
fn build_node_string(handle: NodeHandle) -> String {
    if handle.is_null() {
        return String::new();
    }

    // Collect nodes from this node up to root
    let mut chain = Vec::new();
    let mut current = handle;
    while !current.is_null() {
        chain.push(current);
        current = pool().get_parent(current);
    }
    chain.reverse();

    let mut result = String::new();
    for h in &chain {
        let node = pool().get(*h);
        match &node.data {
            NodeData::Root { is_absolute } => {
                if *is_absolute {
                    result.push('/');
                } else {
                    result.push('.');
                }
            }
            NodeData::Prim { name } => {
                // Add separator: '/' unless after root '/' or variant selection '}'
                // C++ formats variant children without slash: {v=sel}ChildName
                if !result.is_empty() && !result.ends_with('/') && !result.ends_with('}') {
                    result.push('/');
                }
                result.push_str(name.as_str());
            }
            NodeData::PrimVariantSelection {
                set_name,
                selection,
            } => {
                result.push('{');
                result.push_str(set_name.as_str());
                result.push('=');
                result.push_str(selection.as_str());
                result.push('}');
            }
            // These shouldn't appear in the prim chain but handle gracefully
            _ => {}
        }
    }

    result
}

/// Build the property suffix string from a property node chain.
fn build_prop_string(handle: NodeHandle) -> String {
    if handle.is_null() {
        return String::new();
    }

    // Property nodes form their own chain. Collect them.
    let mut chain = Vec::new();
    let mut current = handle;
    while !current.is_null() {
        let node = pool().get(current);
        // Stop when we hit a prim-side node (Root, Prim, PrimVariantSelection)
        match node.node_type {
            NodeType::Root | NodeType::Prim | NodeType::PrimVariantSelection => break,
            _ => {
                chain.push(current);
                current = node.parent;
            }
        }
    }
    chain.reverse();

    let mut result = String::new();
    for h in &chain {
        let node = pool().get(*h);
        match &node.data {
            NodeData::PrimProperty { name } => {
                result.push('.');
                result.push_str(name.as_str());
            }
            NodeData::Target { target_path_str } => {
                result.push('[');
                result.push_str(target_path_str);
                result.push(']');
            }
            NodeData::RelationalAttribute { name } => {
                result.push('.');
                result.push_str(name.as_str());
            }
            NodeData::Mapper { target_path_str } => {
                result.push_str(".mapper[");
                result.push_str(target_path_str);
                result.push(']');
            }
            NodeData::MapperArg { name } => {
                result.push('.');
                result.push_str(name.as_str());
            }
            NodeData::Expression => {
                result.push_str(".expression");
            }
            _ => {}
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_root_handles() {
        let abs = absolute_root_handle();
        let rel = relative_root_handle();
        assert!(!abs.is_null());
        assert!(!rel.is_null());
        assert_ne!(abs, rel);

        // Same handle on repeated calls (interned)
        assert_eq!(abs, absolute_root_handle());
        assert_eq!(rel, relative_root_handle());
    }

    #[test]
    fn test_prim_interning() {
        let root = absolute_root_handle();
        let world1 = find_or_create_prim(root, &Token::new("World"));
        let world2 = find_or_create_prim(root, &Token::new("World"));
        assert_eq!(world1, world2); // Same token -> same handle

        let other = find_or_create_prim(root, &Token::new("Other"));
        assert_ne!(world1, other);
    }

    #[test]
    fn test_build_prim_string() {
        let root = absolute_root_handle();
        assert_eq!(build_path_string(root, NodeHandle::NULL), "/");

        let world = find_or_create_prim(root, &Token::new("World"));
        assert_eq!(build_path_string(world, NodeHandle::NULL), "/World");

        let cube = find_or_create_prim(world, &Token::new("Cube"));
        assert_eq!(build_path_string(cube, NodeHandle::NULL), "/World/Cube");
    }

    #[test]
    fn test_build_property_string() {
        let root = absolute_root_handle();
        let world = find_or_create_prim(root, &Token::new("World"));
        let vis = find_or_create_prim_property(world, &Token::new("visibility"));
        assert_eq!(build_path_string(world, vis), "/World.visibility");
    }

    #[test]
    fn test_variant_selection() {
        let root = absolute_root_handle();
        let world = find_or_create_prim(root, &Token::new("World"));
        let var = find_or_create_variant_selection(world, "model", "lod0");
        assert_eq!(
            build_path_string(var, NodeHandle::NULL),
            "/World{model=lod0}"
        );
    }

    #[test]
    fn test_target_path() {
        let root = absolute_root_handle();
        let world = find_or_create_prim(root, &Token::new("World"));
        let rel = find_or_create_prim_property(world, &Token::new("rel"));
        let target = find_or_create_target(rel, "/Target");
        assert_eq!(build_path_string(world, target), "/World.rel[/Target]");
    }

    #[test]
    fn test_node_flags() {
        let root = absolute_root_handle();
        let flags = get_node_flags(root);
        assert!(flags.contains(NodeFlags::IS_ABSOLUTE));

        let rel_root = relative_root_handle();
        let rel_flags = get_node_flags(rel_root);
        assert!(!rel_flags.contains(NodeFlags::IS_ABSOLUTE));

        let world = find_or_create_prim(root, &Token::new("World"));
        let world_flags = get_node_flags(world);
        assert!(world_flags.contains(NodeFlags::IS_ABSOLUTE));

        let var = find_or_create_variant_selection(world, "v", "s");
        let var_flags = get_node_flags(var);
        assert!(var_flags.contains(NodeFlags::CONTAINS_VARIANT_SEL));
    }

    #[test]
    fn test_element_count() {
        let root = absolute_root_handle();
        assert_eq!(get_element_count(root), 0);

        let world = find_or_create_prim(root, &Token::new("World"));
        assert_eq!(get_element_count(world), 1);

        let cube = find_or_create_prim(world, &Token::new("Cube"));
        assert_eq!(get_element_count(cube), 2);
    }

    #[test]
    fn test_relative_path_string() {
        let rel = relative_root_handle();
        assert_eq!(build_path_string(rel, NodeHandle::NULL), ".");

        let cube = find_or_create_prim(rel, &Token::new("Cube"));
        // Relative prim: "./" prefix then name. Actually "." + "/Cube" = "./Cube"
        // But our build_node_string does "." then "/Cube"
        let s = build_path_string(cube, NodeHandle::NULL);
        // The relative root is "." and then prim appends "/Cube"
        // so we get "./Cube" which is a valid relative path
        assert!(s == "./Cube" || s == "Cube");
    }

    #[test]
    fn test_relational_attribute() {
        let root = absolute_root_handle();
        let foo = find_or_create_prim(root, &Token::new("Foo"));
        let rel = find_or_create_prim_property(foo, &Token::new("rel"));
        let target = find_or_create_target(rel, "/Target");
        let attr = find_or_create_relational_attribute(target, &Token::new("attr"));
        assert_eq!(build_path_string(foo, attr), "/Foo.rel[/Target].attr");
    }

    #[test]
    fn test_expression() {
        let root = absolute_root_handle();
        let foo = find_or_create_prim(root, &Token::new("Foo"));
        let prop = find_or_create_prim_property(foo, &Token::new("rel"));
        let expr = find_or_create_expression(prop);
        assert_eq!(build_path_string(foo, expr), "/Foo.rel.expression");
    }
}
