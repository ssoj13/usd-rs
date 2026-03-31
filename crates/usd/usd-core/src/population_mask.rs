//! Stage population mask for controlling which prims are populated.
//!
//! A population mask specifies which prim paths should be populated on a stage.
//! This is different from load rules - load rules control payload loading,
//! while population masks control which prims exist at all on the stage.

use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// StagePopulationMask
// ============================================================================

/// Mask for controlling which prims are populated on a stage.
///
/// A population mask restricts which prims a stage will populate. If a mask
/// is set, only prims matching the mask (and their ancestors) will exist
/// on the stage.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::StagePopulationMask;
/// use usd_sdf::Path;
///
/// // Create mask including only /World and descendants
/// let mut mask = StagePopulationMask::new();
/// mask.add(Path::from_string("/World").unwrap());
///
/// // Open stage with mask
/// let stage = UsdStage::open_masked("scene.usda", mask)?;
/// ```
#[derive(Debug, Clone, Default)]
pub struct StagePopulationMask {
    /// Set of included paths
    paths: HashSet<Path>,
}

impl StagePopulationMask {
    /// Creates an empty population mask.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a mask that includes all prims.
    ///
    /// Matches C++ `UsdStagePopulationMask::All()` which returns a mask
    /// containing the absolute root path.
    pub fn all() -> Self {
        let mut mask = Self::new();
        mask.paths.insert(Path::absolute_root());
        mask
    }

    /// Creates a mask from a vector of paths.
    pub fn from_paths(paths: impl IntoIterator<Item = Path>) -> Self {
        Self {
            paths: paths.into_iter().collect(),
        }
    }

    /// Adds a path to the mask.
    pub fn add(&mut self, path: Path) -> &mut Self {
        self.paths.insert(path);
        self
    }

    /// Returns true if this mask includes all prims.
    ///
    /// This is true either when the mask is empty (default), or when
    /// it contains the absolute root path.
    pub fn includes_all(&self) -> bool {
        self.paths.is_empty() || self.paths.contains(&Path::absolute_root())
    }

    /// Returns true if the mask is empty (no paths specified).
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Returns the number of paths in the mask.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Returns the paths in the mask.
    pub fn get_paths(&self) -> Vec<&Path> {
        self.paths.iter().collect()
    }

    /// Returns true if the given path is included in the mask.
    ///
    /// A path is included if:
    /// - The mask is empty or includes all (root path present)
    /// - The path is in the mask
    /// - The path is a descendant of a path in the mask
    /// - The path is an ancestor of a path in the mask
    pub fn includes(&self, path: &Path) -> bool {
        if self.includes_all() {
            return true;
        }

        // Check if path is directly in mask
        if self.paths.contains(path) {
            return true;
        }

        // Check if path is a descendant of any mask path
        for mask_path in &self.paths {
            if path.has_prefix(mask_path) {
                return true;
            }
        }

        // Check if path is an ancestor of any mask path
        for mask_path in &self.paths {
            if mask_path.has_prefix(path) {
                return true;
            }
        }

        false
    }

    /// Returns true if this mask is a superset of `other`.
    ///
    /// Matches C++ `UsdStagePopulationMask::Includes(mask)` overload.
    pub fn includes_mask(&self, other: &Self) -> bool {
        if self.includes_all() {
            return true;
        }
        if other.is_empty() {
            return true;
        }
        for path in &other.paths {
            if !self.includes_subtree(path) {
                return false;
            }
        }
        true
    }

    /// Returns true if the given path and all its descendants are included.
    pub fn includes_subtree(&self, path: &Path) -> bool {
        if self.includes_all() {
            return true;
        }

        // Path is fully included if it's a descendant of a mask path
        for mask_path in &self.paths {
            if path.has_prefix(mask_path) {
                return true;
            }
        }

        false
    }

    /// Returns the intersection of this mask with another.
    ///
    /// The result is minimized (no path is a descendant of another).
    /// Matches C++ `UsdStagePopulationMask::GetIntersection()`.
    pub fn intersection(&self, other: &Self) -> Self {
        if self.paths.is_empty() {
            return other.clone();
        }
        if other.paths.is_empty() {
            return self.clone();
        }

        // Compute actual intersection
        let mut result = Self::new();
        for path in &self.paths {
            if other.includes(path) {
                result.paths.insert(path.clone());
            }
        }
        for path in &other.paths {
            if self.includes(path) {
                result.paths.insert(path.clone());
            }
        }
        // Minimize: remove paths that are descendants of other paths in result
        result.minimize();
        result
    }

    /// Remove paths that are descendants of other paths, keeping only ancestors.
    fn minimize(&mut self) {
        let paths: Vec<Path> = self.paths.iter().cloned().collect();
        let mut to_remove = Vec::new();
        for (i, path) in paths.iter().enumerate() {
            for (j, other) in paths.iter().enumerate() {
                if i != j && path.has_prefix(other) && path != other {
                    to_remove.push(path.clone());
                    break;
                }
            }
        }
        for path in to_remove {
            self.paths.remove(&path);
        }
    }

    /// Returns child names that are included beneath `path`.
    ///
    /// Returns true if any children are included. If all children are
    /// included, `child_names` will be empty. If only specific children
    /// are included, their names are returned.
    ///
    /// Matches C++ `UsdStagePopulationMask::GetIncludedChildNames()`.
    pub fn get_included_child_names(&self, path: &Path, child_names: &mut Vec<Token>) -> bool {
        child_names.clear();

        if self.includes_all() {
            return true;
        }

        // If path's subtree is fully included, all children are included
        if self.includes_subtree(path) {
            return true;
        }

        // If path is included as an ancestor, find specific child paths
        let mut has_children = false;
        for mask_path in &self.paths {
            if mask_path.has_prefix(path) && mask_path != path {
                // mask_path is a descendant of path
                // Get the immediate child name under path
                let mask_str = mask_path.get_string();
                let path_str = path.get_string();
                let suffix = &mask_str[path_str.len()..];
                // Strip leading /
                let suffix = suffix.strip_prefix('/').unwrap_or(suffix);
                // Get first component
                if let Some(child_name) = suffix.split('/').next() {
                    if !child_name.is_empty() {
                        let token = Token::new(child_name);
                        if !child_names.contains(&token) {
                            child_names.push(token);
                        }
                        has_children = true;
                    }
                }
            }
        }

        has_children
    }

    /// Returns the union of this mask with another.
    pub fn get_union(&self, other: &Self) -> Self {
        if self.includes_all() || other.includes_all() {
            return Self::all();
        }

        let mut result = self.clone();
        for path in &other.paths {
            result.add(path.clone());
        }
        result.minimize();
        result
    }

    /// Returns the union of this mask with a single path.
    pub fn get_union_path(&self, path: &Path) -> Self {
        let mut result = self.clone();
        result.add(path.clone());
        result.minimize();
        result
    }

    /// Static union of two masks.
    ///
    /// Matches C++ `UsdStagePopulationMask::Union(l, r)`.
    pub fn union_of(l: &Self, r: &Self) -> Self {
        l.get_union(r)
    }

    /// Static intersection of two masks.
    ///
    /// Matches C++ `UsdStagePopulationMask::Intersection(l, r)`.
    pub fn intersection_of(l: &Self, r: &Self) -> Self {
        l.intersection(r)
    }

    /// Clears the mask.
    pub fn clear(&mut self) {
        self.paths.clear();
    }
}

impl PartialEq for StagePopulationMask {
    fn eq(&self, other: &Self) -> bool {
        self.paths == other.paths
    }
}

impl Eq for StagePopulationMask {}

impl fmt::Display for StagePopulationMask {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StagePopulationMask(")?;
        let mut paths: Vec<_> = self.paths.iter().collect();
        paths.sort();
        for (i, path) in paths.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", path.get_string())?;
        }
        write!(f, ")")
    }
}

impl Hash for StagePopulationMask {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Sort paths for deterministic hashing
        let mut paths: Vec<_> = self.paths.iter().collect();
        paths.sort();
        paths.len().hash(state);
        for path in paths {
            path.hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_mask_includes_all() {
        let mask = StagePopulationMask::new();
        assert!(mask.includes_all());

        let path = Path::from_string("/World").unwrap();
        assert!(mask.includes(&path));
    }

    #[test]
    fn test_add_path() {
        let mut mask = StagePopulationMask::new();
        let world = Path::from_string("/World").unwrap();

        mask.add(world.clone());
        assert!(!mask.includes_all());
        assert!(mask.includes(&world));
    }

    #[test]
    fn test_includes_descendants() {
        let mut mask = StagePopulationMask::new();
        let world = Path::from_string("/World").unwrap();
        let child = Path::from_string("/World/Child").unwrap();
        let grandchild = Path::from_string("/World/Child/Grandchild").unwrap();

        mask.add(world);
        assert!(mask.includes(&child));
        assert!(mask.includes(&grandchild));
    }

    #[test]
    fn test_includes_ancestors() {
        let mut mask = StagePopulationMask::new();
        let child = Path::from_string("/World/Child").unwrap();
        let world = Path::from_string("/World").unwrap();
        let root = Path::absolute_root();

        mask.add(child);
        assert!(mask.includes(&world));
        assert!(mask.includes(&root));
    }

    #[test]
    fn test_excludes_siblings() {
        let mut mask = StagePopulationMask::new();
        let world = Path::from_string("/World").unwrap();
        let other = Path::from_string("/Other").unwrap();

        mask.add(world);
        assert!(!mask.includes(&other));
    }

    #[test]
    fn test_from_paths() {
        let paths = vec![
            Path::from_string("/A").unwrap(),
            Path::from_string("/B").unwrap(),
        ];

        let mask = StagePopulationMask::from_paths(paths);
        assert_eq!(mask.len(), 2);
    }

    // M5: intersection() minimizes redundant descendant paths
    #[test]
    fn test_intersection_minimizes() {
        let mut a = StagePopulationMask::new();
        a.add(Path::from_string("/A").unwrap());
        a.add(Path::from_string("/A/B").unwrap());
        a.add(Path::from_string("/C").unwrap());

        let mut b = StagePopulationMask::new();
        b.add(Path::from_string("/A").unwrap());
        b.add(Path::from_string("/C").unwrap());

        let result = a.intersection(&b);
        // /A/B is descendant of /A, should be removed by minimize
        assert!(result.includes(&Path::from_string("/A").unwrap()));
        assert!(result.includes(&Path::from_string("/C").unwrap()));
        // Result should have only 2 paths (/A and /C), not /A/B
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_all_mask() {
        let mask = StagePopulationMask::all();
        assert!(mask.includes_all());
        assert!(!mask.is_empty()); // all() has root path
        assert!(mask.includes(&Path::from_string("/Anything").unwrap()));
    }

    #[test]
    fn test_includes_mask() {
        let mut big = StagePopulationMask::new();
        big.add(Path::from_string("/World").unwrap());
        big.add(Path::from_string("/Other").unwrap());

        let mut small = StagePopulationMask::new();
        small.add(Path::from_string("/World/Child").unwrap());

        assert!(big.includes_mask(&small));
        assert!(!small.includes_mask(&big));
    }

    #[test]
    fn test_get_union() {
        let mut a = StagePopulationMask::new();
        a.add(Path::from_string("/A").unwrap());
        let mut b = StagePopulationMask::new();
        b.add(Path::from_string("/B").unwrap());

        let result = a.get_union(&b);
        assert!(result.includes(&Path::from_string("/A").unwrap()));
        assert!(result.includes(&Path::from_string("/B").unwrap()));
    }

    #[test]
    fn test_get_included_child_names() {
        let mut mask = StagePopulationMask::new();
        mask.add(Path::from_string("/World/A").unwrap());
        mask.add(Path::from_string("/World/B/C").unwrap());

        let world = Path::from_string("/World").unwrap();
        let mut names = Vec::new();
        let has_children = mask.get_included_child_names(&world, &mut names);
        assert!(has_children);
        assert!(names.iter().any(|n| n == "A"));
        assert!(names.iter().any(|n| n == "B"));
    }

    #[test]
    fn test_display_mask() {
        let mut mask = StagePopulationMask::new();
        mask.add(Path::from_string("/World").unwrap());
        let s = mask.to_string();
        assert!(s.contains("StagePopulationMask"));
        assert!(s.contains("/World"));
    }

    #[test]
    fn test_intersection_no_overlap() {
        let mut a = StagePopulationMask::new();
        a.add(Path::from_string("/A").unwrap());

        let mut b = StagePopulationMask::new();
        b.add(Path::from_string("/B").unwrap());

        let result = a.intersection(&b);
        assert_eq!(result.len(), 0);
    }
}
