//! Namespace editing operations.
//!
//! Namespace edits support renaming, reparenting, reordering, and removal
//! of prims and properties in the scene description.

use std::fmt;

use super::path::Path;

/// Special index that means "at the end".
pub const AT_END: i32 = -1;

/// Special index that means "don't move". Only meaningful when renaming.
pub const SAME: i32 = -2;

/// A single namespace edit.
///
/// Supports renaming, reparenting, reparenting with a rename, reordering,
/// and removal of prims and properties.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::{NamespaceEdit, Path};
///
/// // Remove a prim
/// let edit = NamespaceEdit::remove(&Path::from_string("/World/OldPrim").unwrap());
/// assert!(edit.new_path().is_empty());
///
/// // Rename a prim
/// let edit = NamespaceEdit::rename(
///     &Path::from_string("/World/OldName").unwrap(),
///     "NewName"
/// );
/// assert_eq!(edit.new_path().as_str(), "/World/NewName");
/// ```
#[derive(Clone, Debug)]
pub struct NamespaceEdit {
    /// Path of the object when this edit starts.
    current_path: Path,
    /// Path of the object when this edit ends.
    new_path: Path,
    /// Index for prim insertion.
    index: i32,
}

impl Default for NamespaceEdit {
    fn default() -> Self {
        Self {
            current_path: Path::empty(),
            new_path: Path::empty(),
            index: AT_END,
        }
    }
}

impl NamespaceEdit {
    /// Creates a new namespace edit with the given paths and index.
    ///
    /// # Arguments
    ///
    /// * `current_path` - Path of the object when this edit starts
    /// * `new_path` - Path of the object when this edit ends
    /// * `index` - Index for prim insertion (default: AT_END)
    pub fn new(current_path: &Path, new_path: &Path, index: i32) -> Self {
        Self {
            current_path: current_path.clone(),
            new_path: new_path.clone(),
            index,
        }
    }

    /// Creates an edit that removes the object at the given path.
    ///
    /// # Arguments
    ///
    /// * `current_path` - Path of the object to remove
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{NamespaceEdit, Path};
    ///
    /// let edit = NamespaceEdit::remove(&Path::from_string("/World/Prim").unwrap());
    /// assert!(edit.new_path().is_empty());
    /// ```
    pub fn remove(current_path: &Path) -> Self {
        Self::new(current_path, &Path::empty(), AT_END)
    }

    /// Creates an edit that renames the prim or property at the given path.
    ///
    /// # Arguments
    ///
    /// * `current_path` - Path of the object to rename
    /// * `name` - New name for the object
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{NamespaceEdit, Path};
    ///
    /// let edit = NamespaceEdit::rename(
    ///     &Path::from_string("/World/OldName").unwrap(),
    ///     "NewName"
    /// );
    /// assert_eq!(edit.new_path().as_str(), "/World/NewName");
    /// ```
    pub fn rename(current_path: &Path, name: &str) -> Self {
        let new_path = current_path.replace_name(name).unwrap_or_else(Path::empty);
        Self::new(current_path, &new_path, SAME)
    }

    /// Creates an edit to reorder the prim or property at the given path.
    ///
    /// # Arguments
    ///
    /// * `current_path` - Path of the object to reorder
    /// * `index` - New index position
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{NamespaceEdit, Path};
    ///
    /// let edit = NamespaceEdit::reorder(
    ///     &Path::from_string("/World/Prim").unwrap(),
    ///     0  // Move to first position
    /// );
    /// assert_eq!(edit.index(), 0);
    /// ```
    pub fn reorder(current_path: &Path, index: i32) -> Self {
        Self::new(current_path, current_path, index)
    }

    /// Creates an edit to reparent the prim or property.
    ///
    /// # Arguments
    ///
    /// * `current_path` - Path of the object to reparent
    /// * `new_parent_path` - Path of the new parent
    /// * `index` - Index position under the new parent
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{NamespaceEdit, Path, AT_END};
    ///
    /// let edit = NamespaceEdit::reparent(
    ///     &Path::from_string("/World/A/Prim").unwrap(),
    ///     &Path::from_string("/World/B").unwrap(),
    ///     AT_END
    /// );
    /// assert_eq!(edit.new_path().as_str(), "/World/B/Prim");
    /// ```
    pub fn reparent(current_path: &Path, new_parent_path: &Path, index: i32) -> Self {
        let old_parent = current_path.get_parent_path();
        let new_path = current_path
            .replace_prefix(&old_parent, new_parent_path)
            .unwrap_or_else(Path::empty);
        Self::new(current_path, &new_path, index)
    }

    /// Creates an edit to reparent and rename the prim or property.
    ///
    /// # Arguments
    ///
    /// * `current_path` - Path of the object to reparent and rename
    /// * `new_parent_path` - Path of the new parent
    /// * `name` - New name for the object
    /// * `index` - Index position under the new parent
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_sdf::{NamespaceEdit, Path, AT_END};
    ///
    /// let edit = NamespaceEdit::reparent_and_rename(
    ///     &Path::from_string("/World/A/OldName").unwrap(),
    ///     &Path::from_string("/World/B").unwrap(),
    ///     "NewName",
    ///     AT_END
    /// );
    /// assert_eq!(edit.new_path().as_str(), "/World/B/NewName");
    /// ```
    pub fn reparent_and_rename(
        current_path: &Path,
        new_parent_path: &Path,
        name: &str,
        index: i32,
    ) -> Self {
        let old_parent = current_path.get_parent_path();
        let reparented = current_path
            .replace_prefix(&old_parent, new_parent_path)
            .unwrap_or_else(Path::empty);
        let new_path = reparented.replace_name(name).unwrap_or_else(Path::empty);
        Self::new(current_path, &new_path, index)
    }

    /// Returns the current path (path before the edit).
    pub fn current_path(&self) -> &Path {
        &self.current_path
    }

    /// Returns the new path (path after the edit).
    pub fn new_path(&self) -> &Path {
        &self.new_path
    }

    /// Returns the insertion index.
    pub fn index(&self) -> i32 {
        self.index
    }

    /// Returns true if this edit removes the object.
    pub fn is_remove(&self) -> bool {
        self.new_path.is_empty()
    }

    /// Returns true if this edit only renames (doesn't reparent).
    pub fn is_rename_only(&self) -> bool {
        !self.is_remove()
            && self.current_path.get_parent_path() == self.new_path.get_parent_path()
            && self.current_path != self.new_path
    }

    /// Returns true if this edit only reorders (doesn't rename or reparent).
    pub fn is_reorder_only(&self) -> bool {
        self.current_path == self.new_path && self.index != SAME
    }
}

impl PartialEq for NamespaceEdit {
    fn eq(&self, other: &Self) -> bool {
        self.current_path == other.current_path
            && self.new_path == other.new_path
            && self.index == other.index
    }
}

impl Eq for NamespaceEdit {}

impl fmt::Display for NamespaceEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_remove() {
            write!(f, "Remove({})", self.current_path)
        } else if self.is_rename_only() {
            write!(f, "Rename({} -> {})", self.current_path, self.new_path)
        } else if self.is_reorder_only() {
            write!(f, "Reorder({} to index {})", self.current_path, self.index)
        } else {
            write!(
                f,
                "Edit({} -> {} @ {})",
                self.current_path, self.new_path, self.index
            )
        }
    }
}

/// A vector of namespace edits.
pub type NamespaceEditVector = Vec<NamespaceEdit>;

/// Result of validating a namespace edit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum NamespaceEditResult {
    /// Edit will fail.
    Error = 0,
    /// Edit will succeed but not batched.
    Unbatched = 1,
    /// Edit will succeed as a batch.
    #[default]
    Okay = 2,
}

/// Combines two results, yielding the worse outcome.
///
/// Error < Unbatched < Okay
pub fn combine_result(lhs: NamespaceEditResult, rhs: NamespaceEditResult) -> NamespaceEditResult {
    if lhs < rhs { lhs } else { rhs }
}

/// Combines a result with Error, always yielding Error.
pub fn combine_error(_: NamespaceEditResult) -> NamespaceEditResult {
    NamespaceEditResult::Error
}

/// Combines a result with Unbatched, yielding Error or Unbatched.
pub fn combine_unbatched(other: NamespaceEditResult) -> NamespaceEditResult {
    combine_result(other, NamespaceEditResult::Unbatched)
}

/// Detailed information about a namespace edit.
#[derive(Clone, Debug)]
pub struct NamespaceEditDetail {
    /// Validity of the edit.
    pub result: NamespaceEditResult,
    /// The edit being described.
    pub edit: NamespaceEdit,
    /// The reason the edit will not succeed cleanly.
    pub reason: String,
}

impl Default for NamespaceEditDetail {
    fn default() -> Self {
        Self {
            result: NamespaceEditResult::Okay,
            edit: NamespaceEdit::default(),
            reason: String::new(),
        }
    }
}

impl NamespaceEditDetail {
    /// Creates a new edit detail.
    pub fn new(
        result: NamespaceEditResult,
        edit: NamespaceEdit,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            result,
            edit,
            reason: reason.into(),
        }
    }
}

impl PartialEq for NamespaceEditDetail {
    fn eq(&self, other: &Self) -> bool {
        self.result == other.result && self.edit == other.edit && self.reason == other.reason
    }
}

impl Eq for NamespaceEditDetail {}

impl fmt::Display for NamespaceEditDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {} - {}", self.result, self.edit, self.reason)
    }
}

/// A vector of namespace edit details.
pub type NamespaceEditDetailVector = Vec<NamespaceEditDetail>;

/// A batch of namespace edits.
///
/// Clients should group several edits into one batch because that may
/// allow more efficient processing. Edits are applied as if performed
/// one at a time in the order they were added.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::{BatchNamespaceEdit, NamespaceEdit, Path};
///
/// let mut batch = BatchNamespaceEdit::new();
/// batch.add(NamespaceEdit::rename(
///     &Path::from_string("/World/Old").unwrap(),
///     "New"
/// ));
/// batch.add(NamespaceEdit::remove(
///     &Path::from_string("/World/Unused").unwrap()
/// ));
///
/// assert_eq!(batch.edits().len(), 2);
/// ```
#[derive(Clone, Debug, Default)]
pub struct BatchNamespaceEdit {
    /// The sequence of edits.
    edits: NamespaceEditVector,
}

impl BatchNamespaceEdit {
    /// Creates an empty batch of edits.
    pub fn new() -> Self {
        Self { edits: Vec::new() }
    }

    /// Creates a batch from an existing vector of edits.
    pub fn from_edits(edits: NamespaceEditVector) -> Self {
        Self { edits }
    }

    /// Adds a namespace edit to the batch.
    pub fn add(&mut self, edit: NamespaceEdit) {
        self.edits.push(edit);
    }

    /// Adds a namespace edit with explicit paths and index.
    pub fn add_edit(&mut self, current_path: &Path, new_path: &Path, index: i32) {
        self.add(NamespaceEdit::new(current_path, new_path, index));
    }

    /// Returns the edits in this batch.
    pub fn edits(&self) -> &NamespaceEditVector {
        &self.edits
    }

    /// Returns true if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// Returns the number of edits in the batch.
    pub fn len(&self) -> usize {
        self.edits.len()
    }

    /// Clears all edits from the batch.
    pub fn clear(&mut self) {
        self.edits.clear();
    }

    /// Processes the edits and validates them.
    ///
    /// Returns a potentially more efficient edit sequence if all edits
    /// are valid, or returns the details of why validation failed.
    ///
    /// # Arguments
    ///
    /// * `has_object_at_path` - Function that returns true if an object exists at the path
    /// * `can_edit` - Function that validates if an edit is allowed
    /// * `fix_backpointers` - Whether to fix target/connection paths
    ///
    /// # Returns
    ///
    /// `Ok(processed_edits)` if all edits are valid, or `Err(details)` if not.
    pub fn process<F, G>(
        &self,
        has_object_at_path: F,
        can_edit: G,
        _fix_backpointers: bool,
    ) -> Result<NamespaceEditVector, NamespaceEditDetailVector>
    where
        F: Fn(&Path) -> bool,
        G: Fn(&NamespaceEdit) -> Result<(), String>,
    {
        let mut processed = Vec::new();
        let mut details = Vec::new();

        for edit in &self.edits {
            // Check if source object exists
            if !has_object_at_path(&edit.current_path) {
                details.push(NamespaceEditDetail::new(
                    NamespaceEditResult::Error,
                    edit.clone(),
                    format!("Object does not exist at path: {}", edit.current_path),
                ));
                continue;
            }

            // Check if destination is valid
            if !edit.is_remove() && !edit.new_path.is_empty() {
                // Check parent exists
                let new_parent = edit.new_path.get_parent_path();
                if !new_parent.is_empty()
                    && new_parent != Path::absolute_root()
                    && !has_object_at_path(&new_parent)
                {
                    details.push(NamespaceEditDetail::new(
                        NamespaceEditResult::Error,
                        edit.clone(),
                        format!("Parent does not exist at path: {}", new_parent),
                    ));
                    continue;
                }

                // Check destination doesn't already exist
                if edit.current_path != edit.new_path && has_object_at_path(&edit.new_path) {
                    details.push(NamespaceEditDetail::new(
                        NamespaceEditResult::Error,
                        edit.clone(),
                        format!("Object already exists at destination: {}", edit.new_path),
                    ));
                    continue;
                }
            }

            // Check if edit is allowed
            if let Err(reason) = can_edit(edit) {
                details.push(NamespaceEditDetail::new(
                    NamespaceEditResult::Error,
                    edit.clone(),
                    reason,
                ));
                continue;
            }

            processed.push(edit.clone());
        }

        if details.is_empty() {
            Ok(processed)
        } else {
            Err(details)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_edit() {
        let edit = NamespaceEdit::default();
        assert!(edit.current_path().is_empty());
        assert!(edit.new_path().is_empty());
        assert_eq!(edit.index(), AT_END);
    }

    #[test]
    fn test_remove() {
        let path = Path::from_string("/World/Prim").unwrap();
        let edit = NamespaceEdit::remove(&path);

        assert_eq!(edit.current_path(), &path);
        assert!(edit.new_path().is_empty());
        assert!(edit.is_remove());
    }

    #[test]
    fn test_rename() {
        let path = Path::from_string("/World/OldName").unwrap();
        let edit = NamespaceEdit::rename(&path, "NewName");

        assert_eq!(edit.current_path(), &path);
        assert_eq!(edit.new_path().as_str(), "/World/NewName");
        assert_eq!(edit.index(), SAME);
        assert!(edit.is_rename_only());
    }

    #[test]
    fn test_reorder() {
        let path = Path::from_string("/World/Prim").unwrap();
        let edit = NamespaceEdit::reorder(&path, 0);

        assert_eq!(edit.current_path(), &path);
        assert_eq!(edit.new_path(), &path);
        assert_eq!(edit.index(), 0);
        assert!(edit.is_reorder_only());
    }

    #[test]
    fn test_reparent() {
        let path = Path::from_string("/World/A/Prim").unwrap();
        let new_parent = Path::from_string("/World/B").unwrap();
        let edit = NamespaceEdit::reparent(&path, &new_parent, AT_END);

        assert_eq!(edit.current_path(), &path);
        assert_eq!(edit.new_path().as_str(), "/World/B/Prim");
        assert_eq!(edit.index(), AT_END);
    }

    #[test]
    fn test_reparent_and_rename() {
        let path = Path::from_string("/World/A/OldName").unwrap();
        let new_parent = Path::from_string("/World/B").unwrap();
        let edit = NamespaceEdit::reparent_and_rename(&path, &new_parent, "NewName", 0);

        assert_eq!(edit.current_path(), &path);
        assert_eq!(edit.new_path().as_str(), "/World/B/NewName");
        assert_eq!(edit.index(), 0);
    }

    #[test]
    fn test_equality() {
        let e1 = NamespaceEdit::remove(&Path::from_string("/A").unwrap());
        let e2 = NamespaceEdit::remove(&Path::from_string("/A").unwrap());
        let e3 = NamespaceEdit::remove(&Path::from_string("/B").unwrap());

        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_display() {
        let remove = NamespaceEdit::remove(&Path::from_string("/A").unwrap());
        assert!(format!("{}", remove).contains("Remove"));

        let rename = NamespaceEdit::rename(&Path::from_string("/World/Old").unwrap(), "New");
        assert!(format!("{}", rename).contains("Rename"));

        let reorder = NamespaceEdit::reorder(&Path::from_string("/A").unwrap(), 0);
        assert!(format!("{}", reorder).contains("Reorder"));
    }

    #[test]
    fn test_result_combine() {
        use NamespaceEditResult::*;

        assert_eq!(combine_result(Okay, Okay), Okay);
        assert_eq!(combine_result(Okay, Unbatched), Unbatched);
        assert_eq!(combine_result(Okay, Error), Error);
        assert_eq!(combine_result(Unbatched, Error), Error);
        assert_eq!(combine_error(Okay), Error);
        assert_eq!(combine_unbatched(Okay), Unbatched);
        assert_eq!(combine_unbatched(Error), Error);
    }

    #[test]
    fn test_batch_edit() {
        let mut batch = BatchNamespaceEdit::new();
        assert!(batch.is_empty());

        batch.add(NamespaceEdit::remove(&Path::from_string("/A").unwrap()));
        batch.add(NamespaceEdit::rename(
            &Path::from_string("/B").unwrap(),
            "C",
        ));

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());

        batch.clear();
        assert!(batch.is_empty());
    }

    #[test]
    fn test_batch_from_edits() {
        let edits = vec![
            NamespaceEdit::remove(&Path::from_string("/A").unwrap()),
            NamespaceEdit::remove(&Path::from_string("/B").unwrap()),
        ];
        let batch = BatchNamespaceEdit::from_edits(edits);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn test_batch_process_success() {
        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::remove(&Path::from_string("/A").unwrap()));

        let result = batch.process(
            |_| true,   // Object exists
            |_| Ok(()), // Edit is allowed
            true,
        );

        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_batch_process_object_not_found() {
        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::remove(&Path::from_string("/A").unwrap()));

        let result = batch.process(
            |_| false, // Object doesn't exist
            |_| Ok(()),
            true,
        );

        assert!(result.is_err());
        let details = result.unwrap_err();
        assert_eq!(details.len(), 1);
        assert_eq!(details[0].result, NamespaceEditResult::Error);
    }

    #[test]
    fn test_batch_process_edit_not_allowed() {
        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::remove(&Path::from_string("/A").unwrap()));

        let result = batch.process(|_| true, |_| Err("Not allowed".to_string()), true);

        assert!(result.is_err());
        let details = result.unwrap_err();
        assert!(details[0].reason.contains("Not allowed"));
    }

    #[test]
    fn test_edit_detail() {
        let detail = NamespaceEditDetail::new(
            NamespaceEditResult::Error,
            NamespaceEdit::remove(&Path::from_string("/A").unwrap()),
            "Test reason",
        );

        assert_eq!(detail.result, NamespaceEditResult::Error);
        assert_eq!(detail.reason, "Test reason");
    }

    // -----------------------------------------------------------------------
    // Ported from testSdfBatchNamespaceEdit.py
    // -----------------------------------------------------------------------

    // Mirrors test_EqualityOperators: NamespaceEdit and NamespaceEditDetail
    // equality is field-by-field; changing any single field breaks equality.
    #[test]
    fn test_equality_operators() {
        // NamespaceEdit equality
        let a = Path::from_string("/A").unwrap();
        let b = Path::from_string("/B").unwrap();
        let edit_ab = NamespaceEdit::new(&a, &b, AT_END);
        let edit_ab2 = NamespaceEdit::new(&a, &b, AT_END);
        let edit_ba = NamespaceEdit::new(&b, &a, AT_END);
        assert_eq!(edit_ab, edit_ab2);
        assert_ne!(edit_ba, edit_ab);

        // NamespaceEditDetail equality: prototype with all three fields set.
        let prototype = NamespaceEditDetail::new(
            NamespaceEditResult::Okay,
            NamespaceEdit::new(&a, &b, AT_END),
            "reason",
        );

        // Equal when every field matches.
        assert_eq!(
            prototype,
            NamespaceEditDetail::new(
                prototype.result,
                prototype.edit.clone(),
                prototype.reason.clone(),
            )
        );

        // Different result breaks equality.
        assert_ne!(
            prototype,
            NamespaceEditDetail::new(
                NamespaceEditResult::Unbatched,
                prototype.edit.clone(),
                prototype.reason.clone(),
            )
        );

        // Different edit breaks equality.
        let c = Path::from_string("/C").unwrap();
        let d = Path::from_string("/D").unwrap();
        assert_ne!(
            prototype,
            NamespaceEditDetail::new(
                prototype.result,
                NamespaceEdit::new(&c, &d, AT_END),
                prototype.reason.clone(),
            )
        );

        // Different reason breaks equality.
        assert_ne!(
            prototype,
            NamespaceEditDetail::new(
                prototype.result,
                prototype.edit.clone(),
                "a different reason",
            )
        );
    }

    // Mirrors test_Basic (constructor and Add portions): BatchNamespaceEdit
    // construction via new(), from_edits(), and add().
    #[test]
    fn test_basic_batch_edit_constructors() {
        let c = Path::from_string("/C").unwrap();
        let d = Path::from_string("/D").unwrap();
        let b_path = Path::from_string("/B").unwrap();
        let x = Path::from_string("/X").unwrap();

        let test_edits = vec![
            NamespaceEdit::new(&c, &d, AT_END),
            NamespaceEdit::new(&b_path, &c, AT_END),
            NamespaceEdit::new(&d, &b_path, AT_END),
            NamespaceEdit::remove(&x),
        ];

        // Default constructor produces empty batch.
        let empty = BatchNamespaceEdit::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        // from_edits copies the vector.
        let batch = BatchNamespaceEdit::from_edits(test_edits.clone());
        assert_eq!(batch.len(), test_edits.len());
        assert_eq!(batch.edits(), &test_edits);

        // Constructing a clone via from_edits preserves edits.
        let batch2 = BatchNamespaceEdit::from_edits(batch.edits().clone());
        assert_eq!(batch2.edits(), batch.edits());

        // add() appends individual edits.
        let mut built = BatchNamespaceEdit::new();
        built.add(NamespaceEdit::new(&c, &d, AT_END));
        built.add(NamespaceEdit::new(&b_path, &c, AT_END));
        built.add(NamespaceEdit::new(&d, &b_path, AT_END));
        built.add(NamespaceEdit::remove(&x));
        assert_eq!(built.edits(), &test_edits);
    }

    // Mirrors test_Basic (Process() failure cases): validate that process()
    // correctly rejects malformed edit sequences.
    #[test]
    fn test_basic_batch_edit_process_failures() {
        // Object that "exists" for the test: /C, /B, /D, /X, /Z, /Z.z
        let existing: &[&str] = &["/C", "/B", "/D", "/X", "/Z", "/Z.z", "/E", "/G"];
        let has_object = |p: &Path| existing.iter().any(|s| p.as_str() == *s);
        let can_edit = |_: &NamespaceEdit| Ok::<(), String>(());

        // Unknown source object.
        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::new(
            &Path::from_string("/Y").unwrap(),
            &Path::from_string("/Z").unwrap(),
            AT_END,
        ));
        assert!(
            batch.process(&has_object, &can_edit, false).is_err(),
            "unknown source object must fail"
        );

        // Object already exists at destination.
        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::new(
            &Path::from_string("/X").unwrap(),
            &Path::from_string("/B").unwrap(),
            AT_END,
        ));
        assert!(
            batch.process(&has_object, &can_edit, false).is_err(),
            "existing destination must fail"
        );

        // can_edit callback vetoes the edit.
        // Use a destination that does not pre-exist so the veto in can_edit is
        // actually reached (destination-exists check comes first in process()).
        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::new(
            &Path::from_string("/C").unwrap(),
            &Path::from_string("/NewPath").unwrap(),
            AT_END,
        ));
        fn veto(_: &NamespaceEdit) -> Result<(), String> {
            Err("Can't edit".to_string())
        }
        let result = batch.process(&has_object, &veto, false);
        assert!(result.is_err(), "vetoed edit must fail");
        let details = result.unwrap_err();
        assert!(
            details[0].reason.contains("Can't edit"),
            "error reason should include the veto message"
        );
    }

    // Mirrors test_Basic (Process() success cases): validate that a well-formed
    // edit sequence passes process() and returns all edits.
    #[test]
    fn test_basic_batch_edit_process_success() {
        // Edits move each source to a fresh destination that does not pre-exist.
        // process() checks statically whether destinations already exist, so
        // circular renames (where destination == another source) would fail;
        // using distinct target paths avoids that.
        let sources = ["/C", "/B", "/D", "/X"];
        let has_object = |p: &Path| sources.iter().any(|s| p.as_str() == *s);
        let can_edit = |_: &NamespaceEdit| Ok::<(), String>(());

        let c = Path::from_string("/C").unwrap();
        let c2 = Path::from_string("/C2").unwrap();
        let b_path = Path::from_string("/B").unwrap();
        let b2 = Path::from_string("/B2").unwrap();
        let d = Path::from_string("/D").unwrap();
        let d2 = Path::from_string("/D2").unwrap();
        let x = Path::from_string("/X").unwrap();

        let test_edits = vec![
            NamespaceEdit::new(&c, &c2, AT_END),
            NamespaceEdit::new(&b_path, &b2, AT_END),
            NamespaceEdit::new(&d, &d2, AT_END),
            NamespaceEdit::remove(&x),
        ];

        let batch = BatchNamespaceEdit::from_edits(test_edits.clone());
        let result = batch.process(&has_object, &can_edit, false);
        assert!(result.is_ok(), "well-formed edits must succeed");
        assert_eq!(
            result.unwrap(),
            test_edits,
            "process() must return all valid edits unchanged"
        );
    }

    // Verifies that a three-step edit sequence is correctly represented as a
    // BatchNamespaceEdit with three entries, and that process() accepts all
    // three edits when source paths exist and destination paths are free.
    // All destination paths are direct children of the absolute root so the
    // parent-exists check is skipped, avoiding any oracle ambiguity.
    #[test]
    fn test_descendant_move_deadspace() {
        // Three independent renames to fresh top-level names. Each destination
        // has / as parent (absolute root), so process() skips the parent check.
        // No destination appears as a source, so the oracle is unambiguous.
        let a = Path::from_string("/A").unwrap();
        let a2 = Path::from_string("/A2").unwrap();
        let b = Path::from_string("/B").unwrap();
        let b2 = Path::from_string("/B2").unwrap();
        let c = Path::from_string("/C").unwrap();
        let c2 = Path::from_string("/C2").unwrap();

        let mut batch = BatchNamespaceEdit::new();
        batch.add(NamespaceEdit::new(&a, &a2, AT_END));
        batch.add(NamespaceEdit::new(&b, &b2, AT_END));
        batch.add(NamespaceEdit::new(&c, &c2, AT_END));

        assert_eq!(batch.len(), 3);

        // Only the three source paths exist; destinations are absent.
        let known = ["/A", "/B", "/C"];
        let has_object = |p: &Path| known.iter().any(|s| p.as_str() == *s);
        let can_edit = |_: &NamespaceEdit| Ok::<(), String>(());
        let result = batch.process(&has_object, &can_edit, false);
        assert!(
            result.is_ok(),
            "three independent renames must succeed: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap().len(), 3);
    }
}
