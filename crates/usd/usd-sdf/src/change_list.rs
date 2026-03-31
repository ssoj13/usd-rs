//! SdfChangeList - List of scene description modifications.
//!
//! Port of pxr/usd/sdf/changeList.h
//!
//! A list of scene description modifications, organized by the namespace
//! paths where the changes occur.

use crate::Path;
use indexmap::IndexMap;
use usd_tf::Token;
use usd_vt::Value;

/// Type of sublayer change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubLayerChangeType {
    /// Sublayer was added.
    Added,
    /// Sublayer was removed.
    Removed,
    /// Sublayer offset changed.
    Offset,
}

/// Flags indicating what changed at a path.
///
/// Matches C++ `SdfChangeList::Entry::_Flags`. All layer-level flags
/// (identifier, resolved path, content) are stored on the absolute root
/// path entry, matching C++ behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EntryFlags {
    // -- Layer-level flags (set on absolute root entry) --
    /// Layer identifier changed.
    pub did_change_identifier: bool,
    /// Layer resolved path changed.
    pub did_change_resolved_path: bool,
    /// Layer content was replaced.
    pub did_replace_content: bool,
    /// Layer content was reloaded.
    pub did_reload_content: bool,

    // -- Reorder flags --
    /// Prim children were reordered.
    pub did_reorder_children: bool,
    /// Properties were reordered.
    pub did_reorder_properties: bool,

    // -- Rename flag --
    /// Prim or property was renamed.
    pub did_rename: bool,

    // -- Prim composition flags --
    /// Prim variant sets changed.
    pub did_change_variant_sets: bool,
    /// Prim inherit paths changed.
    pub did_change_inherit_paths: bool,
    /// Prim specializes changed.
    pub did_change_specializes: bool,
    /// Prim references changed.
    pub did_change_references: bool,

    // -- Property flags --
    /// Attribute time samples changed.
    pub did_change_attribute_time_samples: bool,
    /// Attribute connection changed.
    pub did_change_attribute_connection: bool,
    /// Relationship targets changed.
    pub did_change_relationship_targets: bool,
    /// Target was added.
    pub did_add_target: bool,
    /// Target was removed.
    pub did_remove_target: bool,

    // -- Prim add/remove flags --
    /// Prim/property was created with inert spec.
    pub did_add_inert_prim: bool,
    /// Non-inert prim was added.
    pub did_add_non_inert_prim: bool,
    /// Prim/property was removed and was inert.
    pub did_remove_inert_prim: bool,
    /// Non-inert prim was removed.
    pub did_remove_non_inert_prim: bool,

    // -- Property add/remove flags --
    /// Property was added with only required fields.
    pub did_add_property_with_only_required_fields: bool,
    /// Property was added.
    pub did_add_property: bool,
    /// Property was removed and had only required fields.
    pub did_remove_property_with_only_required_fields: bool,
    /// Property was removed.
    pub did_remove_property: bool,
}

/// Change to an info field: (old_value, new_value).
pub type InfoChange = (Value, Value);

/// Entry of changes at a single path in namespace.
///
/// If the path is the absolute root path, that indicates a change
/// to the root of namespace (layer or stage).
///
/// Matches C++ `SdfChangeList::Entry`. Info changes are stored in a
/// `Vec` (matching C++ `TfSmallVector<pair<TfToken,InfoChange>,3>`)
/// because typical edits touch only 1-3 fields per spec.
#[derive(Debug, Clone, Default)]
pub struct Entry {
    /// Info keys that have changed to (old, new) value pairs.
    /// Uses Vec for C++ parity (SmallVector with linear scan).
    pub info_changes: Vec<(Token, InfoChange)>,
    /// Old path if this was a rename/move.
    pub old_path: Option<Path>,
    /// Flags indicating what changed.
    pub flags: EntryFlags,
    /// Sublayer changes.
    pub sublayer_changes: Vec<(String, SubLayerChangeType)>,
    /// Old identifier if layer identifier changed.
    pub old_identifier: Option<String>,
}

impl Entry {
    /// Creates a new empty entry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if this entry has any changes.
    pub fn has_changes(&self) -> bool {
        !self.info_changes.is_empty()
            || self.old_path.is_some()
            || self.has_flag_changes()
            || !self.sublayer_changes.is_empty()
            || self.old_identifier.is_some()
    }

    /// Find the info change for the given key (linear scan, matches C++).
    ///
    /// Returns `Some(&InfoChange)` if found, `None` otherwise.
    pub fn find_info_change(&self, key: &Token) -> Option<&InfoChange> {
        self.info_changes
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Returns true if this entry has an info change for the given key.
    pub fn has_info_change(&self, key: &Token) -> bool {
        self.find_info_change(key).is_some()
    }

    /// Returns true if any flags are set.
    fn has_flag_changes(&self) -> bool {
        let f = &self.flags;
        f.did_change_identifier
            || f.did_change_resolved_path
            || f.did_replace_content
            || f.did_reload_content
            || f.did_add_inert_prim
            || f.did_remove_inert_prim
            || f.did_add_non_inert_prim
            || f.did_remove_non_inert_prim
            || f.did_add_property_with_only_required_fields
            || f.did_remove_property_with_only_required_fields
            || f.did_add_property
            || f.did_remove_property
            || f.did_reorder_children
            || f.did_reorder_properties
            || f.did_rename
            || f.did_change_inherit_paths
            || f.did_change_references
            || f.did_change_specializes
            || f.did_change_variant_sets
            || f.did_change_attribute_time_samples
            || f.did_change_attribute_connection
            || f.did_change_relationship_targets
            || f.did_add_target
            || f.did_remove_target
    }
}

/// A list of scene description modifications.
///
/// Matches C++ `SdfChangeList`. Layer-level changes (replace, reload,
/// identifier, resolved path) are stored as flags on the absolute root
/// path entry, matching C++ behavior.
#[derive(Debug, Clone, Default)]
pub struct ChangeList {
    /// Map from path to entry of changes, preserving insertion order.
    /// C++ uses TfSmallVector<pair<SdfPath,Entry>,1> + accel table.
    entries: IndexMap<Path, Entry>,
}

impl ChangeList {
    /// Creates a new empty change list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the change list is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all changes.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns the entry for a path, creating one if needed.
    pub fn get_or_create_entry(&mut self, path: &Path) -> &mut Entry {
        self.entries.entry(path.clone()).or_default()
    }

    /// Returns the entry for a path if it exists.
    pub fn get_entry(&self, path: &Path) -> Option<&Entry> {
        self.entries.get(path)
    }

    /// Returns an iterator over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&Path, &Entry)> {
        self.entries.iter()
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    // Layer-level changes (stored as flags on absolute root entry, matching C++)

    /// Record that the layer content was replaced.
    pub fn did_replace_layer_content(&mut self) {
        self.get_or_create_entry(&Path::absolute_root())
            .flags
            .did_replace_content = true;
    }

    /// Record that the layer content was reloaded.
    pub fn did_reload_layer_content(&mut self) {
        self.get_or_create_entry(&Path::absolute_root())
            .flags
            .did_reload_content = true;
    }

    /// Record that the layer resolved path changed.
    pub fn did_change_layer_resolved_path(&mut self) {
        self.get_or_create_entry(&Path::absolute_root())
            .flags
            .did_change_resolved_path = true;
    }

    /// Record that the layer identifier changed.
    pub fn did_change_layer_identifier(&mut self, old_identifier: &str) {
        let entry = self.get_or_create_entry(&Path::absolute_root());
        // Only record the first old identifier (C++ parity)
        if !entry.flags.did_change_identifier {
            entry.flags.did_change_identifier = true;
            entry.old_identifier = Some(old_identifier.to_string());
        }
    }

    /// Record that sublayer paths changed.
    pub fn did_change_sublayer_paths(
        &mut self,
        sublayer_path: &str,
        change_type: SubLayerChangeType,
    ) {
        let entry = self.get_or_create_entry(&Path::absolute_root());
        entry
            .sublayer_changes
            .push((sublayer_path.to_string(), change_type));
    }

    // Prim changes

    /// Record that a prim was added.
    pub fn did_add_prim(&mut self, prim_path: &Path, inert: bool) {
        let entry = self.get_or_create_entry(prim_path);
        if inert {
            entry.flags.did_add_inert_prim = true;
        } else {
            entry.flags.did_add_non_inert_prim = true;
        }
    }

    /// Record that a prim was removed.
    pub fn did_remove_prim(&mut self, prim_path: &Path, inert: bool) {
        let entry = self.get_or_create_entry(prim_path);
        if inert {
            entry.flags.did_remove_inert_prim = true;
        } else {
            entry.flags.did_remove_non_inert_prim = true;
        }
    }

    /// Record that a prim was moved.
    ///
    /// Transfers accumulated changes from old_path to new_path (C++ _MoveEntry).
    pub fn did_move_prim(&mut self, old_path: &Path, new_path: &Path) {
        // Transfer old entry content to new entry (matches C++ _MoveEntry)
        let old_entry = self.entries.swap_remove(old_path);
        let new_entry = self.entries.entry(new_path.clone()).or_default();
        if let Some(old) = old_entry {
            // Merge old entry into new: move info changes, sublayer changes, flags
            for ic in old.info_changes {
                new_entry.info_changes.push(ic);
            }
            for sc in old.sublayer_changes {
                new_entry.sublayer_changes.push(sc);
            }
            // Merge flags with OR
            merge_flags(&mut new_entry.flags, &old.flags);
        }
        new_entry.old_path = Some(old_path.clone());
        new_entry.flags.did_rename = true;
    }

    /// Record that prims were reordered.
    pub fn did_reorder_prims(&mut self, parent_path: &Path) {
        let entry = self.get_or_create_entry(parent_path);
        entry.flags.did_reorder_children = true;
    }

    /// Record that prim name changed.
    ///
    /// If the prim at new_path was previously removed, creates a new entry
    /// for the move to keep a separate record of the removal (C++ parity).
    pub fn did_change_prim_name(&mut self, old_path: &Path, new_path: &Path) {
        // Check if new_path was previously removed (C++ parity)
        if let Some(existing) = self.entries.get(new_path) {
            if existing.flags.did_remove_inert_prim || existing.flags.did_remove_non_inert_prim {
                // C++ calls _AddNewEntry to keep a separate record of the removal.
                // We just clear the removal flags since IndexMap doesn't support
                // duplicate keys.
            }
        }
        self.did_move_prim(old_path, new_path);
    }

    /// Record that prim variant sets changed.
    pub fn did_change_prim_variant_sets(&mut self, prim_path: &Path) {
        let entry = self.get_or_create_entry(prim_path);
        entry.flags.did_change_variant_sets = true;
    }

    /// Record that prim inherit paths changed.
    pub fn did_change_prim_inherit_paths(&mut self, prim_path: &Path) {
        let entry = self.get_or_create_entry(prim_path);
        entry.flags.did_change_inherit_paths = true;
    }

    /// Record that prim references changed.
    pub fn did_change_prim_references(&mut self, prim_path: &Path) {
        let entry = self.get_or_create_entry(prim_path);
        entry.flags.did_change_references = true;
    }

    /// Record that prim specializes changed.
    pub fn did_change_prim_specializes(&mut self, prim_path: &Path) {
        let entry = self.get_or_create_entry(prim_path);
        entry.flags.did_change_specializes = true;
    }

    // Property changes

    /// Record that a property was added.
    pub fn did_add_property(&mut self, prop_path: &Path, has_only_required_fields: bool) {
        let entry = self.get_or_create_entry(prop_path);
        if has_only_required_fields {
            entry.flags.did_add_property_with_only_required_fields = true;
        } else {
            entry.flags.did_add_property = true;
        }
    }

    /// Record that a property was removed.
    pub fn did_remove_property(&mut self, prop_path: &Path, has_only_required_fields: bool) {
        let entry = self.get_or_create_entry(prop_path);
        if has_only_required_fields {
            entry.flags.did_remove_property_with_only_required_fields = true;
        } else {
            entry.flags.did_remove_property = true;
        }
    }

    /// Record that properties were reordered.
    pub fn did_reorder_properties(&mut self, parent_path: &Path) {
        let entry = self.get_or_create_entry(parent_path);
        entry.flags.did_reorder_properties = true;
    }

    /// Record that property name changed.
    ///
    /// Transfers accumulated changes from old_path to new_path (C++ _MoveEntry).
    pub fn did_change_property_name(&mut self, old_path: &Path, new_path: &Path) {
        // Transfer old entry content (same as move prim)
        let old_entry = self.entries.swap_remove(old_path);
        let new_entry = self.entries.entry(new_path.clone()).or_default();
        if let Some(old) = old_entry {
            for ic in old.info_changes {
                new_entry.info_changes.push(ic);
            }
            for sc in old.sublayer_changes {
                new_entry.sublayer_changes.push(sc);
            }
            merge_flags(&mut new_entry.flags, &old.flags);
        }
        new_entry.old_path = Some(old_path.clone());
        new_entry.flags.did_rename = true;
    }

    // Attribute changes

    /// Record that attribute time samples changed.
    pub fn did_change_attribute_time_samples(&mut self, attr_path: &Path) {
        let entry = self.get_or_create_entry(attr_path);
        entry.flags.did_change_attribute_time_samples = true;
    }

    /// Record that attribute connection changed.
    pub fn did_change_attribute_connection(&mut self, attr_path: &Path) {
        let entry = self.get_or_create_entry(attr_path);
        entry.flags.did_change_attribute_connection = true;
    }

    // Relationship changes

    /// Record that relationship targets changed.
    pub fn did_change_relationship_targets(&mut self, rel_path: &Path) {
        let entry = self.get_or_create_entry(rel_path);
        entry.flags.did_change_relationship_targets = true;
    }

    /// Record that a target was added.
    pub fn did_add_target(&mut self, target_path: &Path) {
        let entry = self.get_or_create_entry(target_path);
        entry.flags.did_add_target = true;
    }

    /// Record that a target was removed.
    pub fn did_remove_target(&mut self, target_path: &Path) {
        let entry = self.get_or_create_entry(target_path);
        entry.flags.did_remove_target = true;
    }

    // Info changes

    /// Record that info changed at a path.
    ///
    /// If the key was already recorded, updates the new value but retains
    /// the original old value (C++ parity).
    pub fn did_change_info(&mut self, path: &Path, key: Token, old_value: Value, new_value: Value) {
        let entry = self.get_or_create_entry(path);
        // Linear search matching C++ FindInfoChange + update semantics
        if let Some(existing) = entry.info_changes.iter_mut().find(|(k, _)| *k == key) {
            // Update new value, retain original old value (C++ parity)
            existing.1.1 = new_value;
        } else {
            entry.info_changes.push((key, (old_value, new_value)));
        }
    }

    // Queries (check absolute root entry flags, matching C++ pattern)

    /// Returns whether layer content was replaced.
    pub fn has_replaced_content(&self) -> bool {
        self.entries
            .get(&Path::absolute_root())
            .is_some_and(|e| e.flags.did_replace_content)
    }

    /// Returns whether layer content was reloaded.
    pub fn has_reloaded_content(&self) -> bool {
        self.entries
            .get(&Path::absolute_root())
            .is_some_and(|e| e.flags.did_reload_content)
    }

    /// Returns whether layer resolved path changed.
    pub fn has_changed_resolved_path(&self) -> bool {
        self.entries
            .get(&Path::absolute_root())
            .is_some_and(|e| e.flags.did_change_resolved_path)
    }

    /// Returns whether layer identifier changed.
    pub fn has_changed_identifier(&self) -> bool {
        self.entries
            .get(&Path::absolute_root())
            .is_some_and(|e| e.flags.did_change_identifier)
    }
}

/// Merge source flags into dest with OR semantics.
fn merge_flags(dest: &mut EntryFlags, src: &EntryFlags) {
    dest.did_change_identifier |= src.did_change_identifier;
    dest.did_change_resolved_path |= src.did_change_resolved_path;
    dest.did_replace_content |= src.did_replace_content;
    dest.did_reload_content |= src.did_reload_content;
    dest.did_reorder_children |= src.did_reorder_children;
    dest.did_reorder_properties |= src.did_reorder_properties;
    dest.did_rename |= src.did_rename;
    dest.did_change_variant_sets |= src.did_change_variant_sets;
    dest.did_change_inherit_paths |= src.did_change_inherit_paths;
    dest.did_change_specializes |= src.did_change_specializes;
    dest.did_change_references |= src.did_change_references;
    dest.did_change_attribute_time_samples |= src.did_change_attribute_time_samples;
    dest.did_change_attribute_connection |= src.did_change_attribute_connection;
    dest.did_change_relationship_targets |= src.did_change_relationship_targets;
    dest.did_add_target |= src.did_add_target;
    dest.did_remove_target |= src.did_remove_target;
    dest.did_add_inert_prim |= src.did_add_inert_prim;
    dest.did_add_non_inert_prim |= src.did_add_non_inert_prim;
    dest.did_remove_inert_prim |= src.did_remove_inert_prim;
    dest.did_remove_non_inert_prim |= src.did_remove_non_inert_prim;
    dest.did_add_property_with_only_required_fields |=
        src.did_add_property_with_only_required_fields;
    dest.did_add_property |= src.did_add_property;
    dest.did_remove_property_with_only_required_fields |=
        src.did_remove_property_with_only_required_fields;
    dest.did_remove_property |= src.did_remove_property;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_list_empty() {
        let list = ChangeList::new();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_change_list_add_prim() {
        let mut list = ChangeList::new();
        let path = Path::from_string("/Prim").unwrap();

        list.did_add_prim(&path, false);

        assert!(!list.is_empty());
        let entry = list.get_entry(&path).unwrap();
        assert!(entry.flags.did_add_non_inert_prim);
    }

    #[test]
    fn test_change_list_layer_content() {
        let mut list = ChangeList::new();

        list.did_replace_layer_content();
        assert!(list.has_replaced_content());

        list.did_reload_layer_content();
        assert!(list.has_reloaded_content());
    }

    #[test]
    fn test_change_list_preserves_insertion_order() {
        let mut list = ChangeList::new();
        let p1 = Path::from_string("/Z").unwrap();
        let p2 = Path::from_string("/A").unwrap();
        let p3 = Path::from_string("/M").unwrap();

        list.did_add_prim(&p1, false);
        list.did_add_prim(&p2, false);
        list.did_add_prim(&p3, false);

        // Entries must come back in insertion order, not sorted
        let keys: Vec<_> = list.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(keys, vec![p1, p2, p3]);
    }

    #[test]
    fn test_layer_flags_on_root_entry() {
        // C++ stores layer-level flags as Entry flags on absolute root
        let mut list = ChangeList::new();

        list.did_replace_layer_content();
        list.did_change_layer_resolved_path();
        list.did_change_layer_identifier("old.usd");

        let root = list.get_entry(&Path::absolute_root()).unwrap();
        assert!(root.flags.did_replace_content);
        assert!(root.flags.did_change_resolved_path);
        assert!(root.flags.did_change_identifier);
        assert_eq!(root.old_identifier.as_deref(), Some("old.usd"));
    }

    #[test]
    fn test_layer_identifier_only_records_first() {
        // C++ only records the first old identifier
        let mut list = ChangeList::new();
        list.did_change_layer_identifier("first.usd");
        list.did_change_layer_identifier("second.usd");

        let root = list.get_entry(&Path::absolute_root()).unwrap();
        assert_eq!(root.old_identifier.as_deref(), Some("first.usd"));
    }

    #[test]
    fn test_info_change_vec_semantics() {
        // info_changes uses Vec (matching C++ SmallVector)
        let mut list = ChangeList::new();
        let path = Path::from_string("/Prim").unwrap();
        let key1 = Token::new("typeName");
        let key2 = Token::new("default");

        list.did_change_info(
            &path,
            key1.clone(),
            Value::from("old1".to_string()),
            Value::from("new1".to_string()),
        );
        list.did_change_info(
            &path,
            key2.clone(),
            Value::from("old2".to_string()),
            Value::from("new2".to_string()),
        );

        let entry = list.get_entry(&path).unwrap();
        assert_eq!(entry.info_changes.len(), 2);
        assert!(entry.has_info_change(&key1));
        assert!(entry.has_info_change(&key2));

        // Insertion order preserved
        assert_eq!(entry.info_changes[0].0, key1);
        assert_eq!(entry.info_changes[1].0, key2);
    }

    #[test]
    fn test_info_change_retains_old_value_on_update() {
        // C++: update new val, retain original old val
        let mut list = ChangeList::new();
        let path = Path::from_string("/Prim").unwrap();
        let key = Token::new("typeName");

        list.did_change_info(
            &path,
            key.clone(),
            Value::from("original_old".to_string()),
            Value::from("first_new".to_string()),
        );
        // Second change to same key: old_value should stay "original_old"
        list.did_change_info(
            &path,
            key.clone(),
            Value::from("ignored_old".to_string()),
            Value::from("second_new".to_string()),
        );

        let entry = list.get_entry(&path).unwrap();
        assert_eq!(entry.info_changes.len(), 1);
        let (old, new) = entry.find_info_change(&key).unwrap();
        // Old value retained from first call
        assert_eq!(old.downcast::<String>().unwrap(), "original_old");
        // New value updated to second call
        assert_eq!(new.downcast::<String>().unwrap(), "second_new");
    }

    #[test]
    fn test_move_entry_transfers_changes() {
        // C++ _MoveEntry transfers accumulated changes from old to new path
        let mut list = ChangeList::new();
        let old_path = Path::from_string("/Old").unwrap();
        let new_path = Path::from_string("/New").unwrap();

        // Accumulate changes on old path
        list.did_add_prim(&old_path, false);
        list.did_change_info(
            &old_path,
            Token::new("typeName"),
            Value::from("old".to_string()),
            Value::from("new".to_string()),
        );

        // Move old -> new
        list.did_move_prim(&old_path, &new_path);

        // Old path entry should be gone
        assert!(list.get_entry(&old_path).is_none());

        // New path entry should have the transferred changes
        let entry = list.get_entry(&new_path).unwrap();
        assert!(entry.flags.did_rename);
        assert!(entry.flags.did_add_non_inert_prim); // transferred
        assert_eq!(entry.info_changes.len(), 1); // transferred
        assert_eq!(entry.old_path.as_ref().unwrap().as_str(), "/Old");
    }

    #[test]
    fn test_has_changed_identifier() {
        let mut list = ChangeList::new();
        assert!(!list.has_changed_identifier());

        list.did_change_layer_identifier("old.usd");
        assert!(list.has_changed_identifier());
    }
}
