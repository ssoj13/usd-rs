//! UsdSkelTopology - skeleton joint hierarchy topology.
//!
//! Port of `pxr/usd/usdSkel/topology.h/cpp`.
//!
//! # Reference parity note
//!
//! OpenUSD computes joint parent indices by iterating `SdfPath::GetAncestorsRange()`,
//! not by manually chasing parent-path strings. That matters for relative joint
//! paths, where ad hoc walks can drift into `.` / `..` sentinels and diverge from
//! the reference algorithm. This module therefore mirrors the reference shape:
//! build a `Path -> index` map and scan ancestor ranges, skipping the path itself.

use std::collections::HashMap;
use usd_sdf::Path;
use usd_tf::Token;

/// Object holding information describing skeleton topology.
///
/// This provides the hierarchical information needed to reason about joint
/// relationships in a manner suitable to computations.
///
/// Matches C++ `UsdSkelTopology`.
#[derive(Debug, Clone, Default)]
pub struct Topology {
    /// Parent indices for each joint (-1 for root joints).
    parent_indices: Vec<i32>,
}

impl Topology {
    /// Constructs an empty topology.
    pub fn new() -> Self {
        Self {
            parent_indices: Vec::new(),
        }
    }

    /// Constructs a topology from joint paths as tokens.
    ///
    /// Internally, each token must be converted to a Path. If Path
    /// objects are already accessible, it is more efficient to use
    /// `from_paths()`.
    ///
    /// Matches C++ `UsdSkelTopology(TfSpan<const TfToken> paths)`.
    pub fn from_tokens(tokens: &[Token]) -> Self {
        // C++ keeps all joints and reports errors for invalid paths.
        // Use map (not filter_map) so invalid tokens become root joints (-1)
        // rather than being silently dropped, which would misalign joint counts.
        let paths: Vec<Path> = tokens
            .iter()
            .map(|t| {
                Path::from_string(t.as_str()).unwrap_or_else(|| {
                    eprintln!("Invalid joint path token: '{}'", t.as_str());
                    Path::empty()
                })
            })
            .collect();
        Self::from_paths(&paths)
    }

    /// Constructs a topology from an array of joint paths.
    ///
    /// Matches C++ `UsdSkelTopology(TfSpan<const SdfPath> paths)`.
    pub fn from_paths(paths: &[Path]) -> Self {
        Self {
            parent_indices: compute_parent_indices_from_paths(paths),
        }
    }

    /// Constructs a topology from an array of parent indices.
    ///
    /// For each joint, this provides the parent index of that
    /// joint, or -1 if none.
    ///
    /// Matches C++ `UsdSkelTopology(const VtIntArray& parentIndices)`.
    pub fn from_parent_indices(parent_indices: Vec<i32>) -> Self {
        Self { parent_indices }
    }

    /// Validates the topology.
    ///
    /// Returns Ok(()) if valid, or Err with reason if invalid.
    ///
    /// Matches C++ `Validate(std::string* reason)`.
    pub fn validate(&self) -> Result<(), String> {
        for (i, &parent) in self.parent_indices.iter().enumerate() {
            if parent >= 0 {
                let parent_idx = parent as usize;
                if parent_idx >= i {
                    if parent_idx == i {
                        return Err(format!("Joint {} has itself as its parent.", i));
                    }
                    return Err(format!(
                        "Joint {} has mis-ordered parent {}. Joints are \
                         expected to be ordered with parent joints always \
                         coming before children.",
                        i, parent
                    ));
                }
            }
        }
        Ok(())
    }

    /// Returns true if the topology is valid.
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Returns the parent indices array.
    ///
    /// Matches C++ `GetParentIndices()`.
    pub fn parent_indices(&self) -> &[i32] {
        &self.parent_indices
    }

    /// Returns the number of joints in the topology.
    ///
    /// Matches C++ `GetNumJoints()` and `size()`.
    pub fn num_joints(&self) -> usize {
        self.parent_indices.len()
    }

    /// Returns the number of joints (alias for num_joints).
    pub fn len(&self) -> usize {
        self.parent_indices.len()
    }

    /// Returns true if the topology has no joints.
    pub fn is_empty(&self) -> bool {
        self.parent_indices.is_empty()
    }

    /// Returns the parent joint of the index'th joint.
    ///
    /// Returns -1 for joints with no parent (roots).
    ///
    /// Matches C++ `GetParent(size_t index)`.
    pub fn get_parent(&self, index: usize) -> i32 {
        debug_assert!(index < self.parent_indices.len());
        self.parent_indices[index]
    }

    /// Returns true if the index'th joint is a root joint.
    ///
    /// Matches C++ `IsRoot(size_t index)`.
    pub fn is_root(&self, index: usize) -> bool {
        self.get_parent(index) < 0
    }
}

impl PartialEq for Topology {
    fn eq(&self, other: &Self) -> bool {
        self.parent_indices == other.parent_indices
    }
}

impl Eq for Topology {}

/// Computes parent indices from paths.
fn compute_parent_indices_from_paths(paths: &[Path]) -> Vec<i32> {
    // Match OpenUSD's `_PathIndexMap = std::unordered_map<SdfPath, int>`.
    // Using `Path` keys avoids repeated string allocation on hot skeleton-guide
    // paths and keeps the equality/hash semantics aligned with `SdfPath`.
    let mut path_map: HashMap<Path, i32> = HashMap::with_capacity(paths.len());
    for (i, path) in paths.iter().enumerate() {
        path_map.insert(path.clone(), i as i32);
    }

    // Compute parent indices
    let mut parent_indices = vec![-1i32; paths.len()];
    for (i, path) in paths.iter().enumerate() {
        parent_indices[i] = get_parent_index(&path_map, path);
    }
    parent_indices
}

/// Gets the parent index for a path by checking ancestors.
///
/// This deliberately mirrors OpenUSD `_GetParentIndex(...)` and uses
/// `get_ancestors_range()` instead of manual `get_parent_path()` loops. The
/// ancestor iterator already terminates correctly for both absolute and relative
/// paths, which avoids pathological walks outside the joint namespace.
fn get_parent_index(path_map: &HashMap<Path, i32>, path: &Path) -> i32 {
    if path.is_prim_path() {
        // Recurse over all ancestor paths, not just the direct parent.
        // For instance, if the map includes only paths 'a' and 'a/b/c',
        // 'a' will be treated as the parent of 'a/b/c'.
        for ancestor in path.get_ancestors_range().into_iter().skip(1) {
            if let Some(&index) = path_map.get(&ancestor) {
                return index;
            }
        }
    }
    -1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_topology() {
        let topo = Topology::new();
        assert!(topo.is_empty());
        assert_eq!(topo.num_joints(), 0);
        assert!(topo.is_valid());
    }

    #[test]
    fn test_from_parent_indices() {
        // Simple chain: root -> child -> grandchild
        let parent_indices = vec![-1, 0, 1];
        let topo = Topology::from_parent_indices(parent_indices);

        assert_eq!(topo.num_joints(), 3);
        assert!(topo.is_root(0));
        assert!(!topo.is_root(1));
        assert!(!topo.is_root(2));
        assert_eq!(topo.get_parent(0), -1);
        assert_eq!(topo.get_parent(1), 0);
        assert_eq!(topo.get_parent(2), 1);
        assert!(topo.is_valid());
    }

    #[test]
    fn test_invalid_self_parent() {
        let parent_indices = vec![-1, 1]; // Joint 1 is its own parent
        let topo = Topology::from_parent_indices(parent_indices);

        let result = topo.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("itself as its parent"));
    }

    #[test]
    fn test_invalid_ordering() {
        let parent_indices = vec![-1, 2, -1]; // Joint 1 has parent 2, but 2 comes after 1
        let topo = Topology::from_parent_indices(parent_indices);

        let result = topo.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mis-ordered"));
    }

    #[test]
    fn test_equality() {
        let topo1 = Topology::from_parent_indices(vec![-1, 0, 1]);
        let topo2 = Topology::from_parent_indices(vec![-1, 0, 1]);
        let topo3 = Topology::from_parent_indices(vec![-1, 0, 0]);

        assert_eq!(topo1, topo2);
        assert_ne!(topo1, topo3);
    }

    #[test]
    fn test_from_tokens_preserves_joint_count() {
        // from_tokens must produce exactly as many joints as input tokens,
        // even if some fail to parse (they become roots with parent -1).
        let tokens = vec![
            Token::new("/Root"),
            Token::new("/Root/Spine"),
            Token::new("/Root/Spine/Chest"),
        ];
        let topo = Topology::from_tokens(&tokens);
        // Joint count must equal token count — the recent fix ensures this.
        assert_eq!(topo.num_joints(), 3);
        assert!(topo.is_valid());
    }

    #[test]
    fn test_from_paths_chain() {
        // /Root -> /Root/Hip -> /Root/Hip/Knee hierarchy
        let paths = vec![
            Path::from_string("/Root").unwrap(),
            Path::from_string("/Root/Hip").unwrap(),
            Path::from_string("/Root/Hip/Knee").unwrap(),
        ];
        let topo = Topology::from_paths(&paths);
        assert_eq!(topo.num_joints(), 3);
        assert_eq!(topo.get_parent(0), -1); // Root has no parent
        assert_eq!(topo.get_parent(1), 0); // Hip's parent is Root (index 0)
        assert_eq!(topo.get_parent(2), 1); // Knee's parent is Hip (index 1)
        assert!(topo.is_valid());
    }

    #[test]
    fn test_from_paths_skipped_intermediate() {
        // Skipped path: /Root and /Root/A/B with no /Root/A — A is implicitly parent of B
        let paths = vec![
            Path::from_string("/Root").unwrap(),
            Path::from_string("/Root/A/B").unwrap(), // skipped intermediate A
        ];
        let topo = Topology::from_paths(&paths);
        assert_eq!(topo.num_joints(), 2);
        // /Root/A/B's nearest ancestor in the list is /Root (index 0)
        assert_eq!(topo.get_parent(1), 0);
    }

    #[test]
    fn test_from_tokens_multiple_roots() {
        // Two separate root chains should both be roots
        let tokens = vec![Token::new("/LeftArm"), Token::new("/RightArm")];
        let topo = Topology::from_tokens(&tokens);
        assert_eq!(topo.num_joints(), 2);
        assert!(topo.is_root(0));
        assert!(topo.is_root(1));
        assert!(topo.is_valid());
    }

    #[test]
    fn test_from_tokens_relative_paths_match_reference_ancestor_walk() {
        // Real character assets can author relative joint paths. OpenUSD derives
        // parents via GetAncestorsRange(), which terminates at ".". This guards
        // against regressing to a manual parent-path loop that can drift into
        // non-joint relative sentinels and stall guide-mesh construction.
        let tokens = vec![
            Token::new("Root"),
            Token::new("Root/Hips"),
            Token::new("Root/Hips/Spine"),
            Token::new("Root/Hips/Spine/Chest"),
        ];
        let topo = Topology::from_tokens(&tokens);

        assert_eq!(topo.parent_indices(), &[-1, 0, 1, 2]);
        assert!(topo.is_valid());
    }

    #[test]
    fn test_len_matches_num_joints() {
        let topo = Topology::from_parent_indices(vec![-1, 0, 0, 1]);
        assert_eq!(topo.len(), 4);
        assert_eq!(topo.num_joints(), 4);
        assert!(!topo.is_empty());
    }
}
