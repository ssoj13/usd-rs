//! Recent files list.
//!
//! Storage is now handled by persistence::AppPersistState — this module
//! only manages the in-memory list.

use std::path::PathBuf;

/// Manages a list of recently opened files.
#[derive(Debug, Clone)]
pub struct RecentFiles {
    /// Ordered list (most recent first).
    paths: Vec<PathBuf>,
    /// Maximum number of entries to keep.
    max_count: usize,
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self::new()
    }
}

impl RecentFiles {
    /// Creates empty recent files list (max 10).
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            max_count: 10,
        }
    }

    /// Creates with custom max count.
    pub fn with_max(max_count: usize) -> Self {
        Self {
            paths: Vec::new(),
            max_count,
        }
    }

    /// Populate from a pre-loaded list (e.g. from AppPersistState).
    pub fn from_vec(paths: Vec<PathBuf>) -> Self {
        let mut rf = Self::new();
        // Insert in reverse so the first element ends up most-recent after add()
        for p in paths.into_iter().rev() {
            rf.add(p);
        }
        rf
    }

    /// Add a file to the top of the list. Deduplicates and trims to max.
    pub fn add(&mut self, path: PathBuf) {
        self.paths.retain(|p| p != &path);
        self.paths.insert(0, path);
        self.paths.truncate(self.max_count);
    }

    /// Returns the list of recent file paths (most recent first).
    pub fn list(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Export to Vec<PathBuf> for persistence.
    pub fn to_vec(&self) -> Vec<PathBuf> {
        self.paths.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_dedup() {
        let mut rf = RecentFiles::with_max(3);
        rf.add(PathBuf::from("/a.usd"));
        rf.add(PathBuf::from("/b.usd"));
        rf.add(PathBuf::from("/a.usd")); // moves to top
        assert_eq!(rf.list().len(), 2);
        assert_eq!(rf.list()[0], PathBuf::from("/a.usd"));
        assert_eq!(rf.list()[1], PathBuf::from("/b.usd"));
    }

    #[test]
    fn test_max_count() {
        let mut rf = RecentFiles::with_max(2);
        rf.add(PathBuf::from("/a.usd"));
        rf.add(PathBuf::from("/b.usd"));
        rf.add(PathBuf::from("/c.usd"));
        assert_eq!(rf.list().len(), 2);
        assert_eq!(rf.list()[0], PathBuf::from("/c.usd"));
    }

    #[test]
    fn test_from_vec() {
        let paths = vec![
            PathBuf::from("/a.usd"),
            PathBuf::from("/b.usd"),
            PathBuf::from("/c.usd"),
        ];
        let rf = RecentFiles::from_vec(paths.clone());
        assert_eq!(rf.list(), paths.as_slice());
    }
}
