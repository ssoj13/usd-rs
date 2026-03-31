//! Change type enumerations for layer modification tracking.
//!
//! These types are used by the change notification system to describe
//! what kind of modification occurred in a layer.
//!
//! # Examples
//!
//! ```
//! use usd_sdf::SubLayerChangeType;
//!
//! let change = SubLayerChangeType::Added;
//! assert_eq!(format!("{}", change), "added");
//! ```

use std::fmt;

/// Type of sublayer change.
///
/// Describes what happened to a sublayer reference in a layer's
/// subLayerPaths metadata.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::SubLayerChangeType;
///
/// let change = SubLayerChangeType::Added;
/// assert!(matches!(change, SubLayerChangeType::Added));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SubLayerChangeType {
    /// A sublayer was added.
    Added = 0,
    /// A sublayer was removed.
    Removed = 1,
    /// A sublayer's offset was changed.
    Offset = 2,
}

impl SubLayerChangeType {
    /// Returns true if this is an add operation.
    #[inline]
    #[must_use]
    pub fn is_add(self) -> bool {
        matches!(self, Self::Added)
    }

    /// Returns true if this is a remove operation.
    #[inline]
    #[must_use]
    pub fn is_remove(self) -> bool {
        matches!(self, Self::Removed)
    }

    /// Returns true if this is an offset change.
    #[inline]
    #[must_use]
    pub fn is_offset(self) -> bool {
        matches!(self, Self::Offset)
    }
}

impl fmt::Display for SubLayerChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Removed => write!(f, "removed"),
            Self::Offset => write!(f, "offset"),
        }
    }
}

/// Flags indicating what changed in a spec.
///
/// These flags are used to efficiently track multiple types of changes
/// that can occur on a single spec in one editing operation.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::ChangeFlags;
///
/// let mut flags = ChangeFlags::default();
/// flags.set_did_rename(true);
/// assert!(flags.did_rename());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChangeFlags {
    /// Layer identifier changed.
    did_change_identifier: bool,
    /// Layer resolved path changed.
    did_change_resolved_path: bool,
    /// Layer content was replaced.
    did_replace_content: bool,
    /// Layer content was reloaded.
    did_reload_content: bool,
    /// Children were reordered.
    did_reorder_children: bool,
    /// Properties were reordered.
    did_reorder_properties: bool,
    /// Spec was renamed/moved.
    did_rename: bool,
    /// Prim variant sets changed.
    did_change_prim_variant_sets: bool,
    /// Prim inherit paths changed.
    did_change_prim_inherit_paths: bool,
    /// Prim specializes changed.
    did_change_prim_specializes: bool,
    /// Prim references changed.
    did_change_prim_references: bool,
    /// Attribute time samples changed.
    did_change_attribute_time_samples: bool,
    /// Attribute connections changed.
    did_change_attribute_connection: bool,
    /// Relationship targets changed.
    did_change_relationship_targets: bool,
    /// A target was added.
    did_add_target: bool,
    /// A target was removed.
    did_remove_target: bool,
    /// Inert prim was added.
    did_add_inert_prim: bool,
    /// Non-inert prim was added.
    did_add_non_inert_prim: bool,
    /// Inert prim was removed.
    did_remove_inert_prim: bool,
    /// Non-inert prim was removed.
    did_remove_non_inert_prim: bool,
    /// Property with only required fields was added.
    did_add_property_with_only_required_fields: bool,
    /// Property was added.
    did_add_property: bool,
    /// Property with only required fields was removed.
    did_remove_property_with_only_required_fields: bool,
    /// Property was removed.
    did_remove_property: bool,
}

impl ChangeFlags {
    /// Creates a new empty flags set.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all flags.
    #[inline]
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Returns true if any flag is set.
    #[must_use]
    pub fn any(&self) -> bool {
        self.did_change_identifier
            || self.did_change_resolved_path
            || self.did_replace_content
            || self.did_reload_content
            || self.did_reorder_children
            || self.did_reorder_properties
            || self.did_rename
            || self.did_change_prim_variant_sets
            || self.did_change_prim_inherit_paths
            || self.did_change_prim_specializes
            || self.did_change_prim_references
            || self.did_change_attribute_time_samples
            || self.did_change_attribute_connection
            || self.did_change_relationship_targets
            || self.did_add_target
            || self.did_remove_target
            || self.did_add_inert_prim
            || self.did_add_non_inert_prim
            || self.did_remove_inert_prim
            || self.did_remove_non_inert_prim
            || self.did_add_property_with_only_required_fields
            || self.did_add_property
            || self.did_remove_property_with_only_required_fields
            || self.did_remove_property
    }

    // Layer flags

    /// Returns true if identifier changed.
    #[inline]
    #[must_use]
    pub fn did_change_identifier(&self) -> bool {
        self.did_change_identifier
    }

    /// Sets identifier changed flag.
    #[inline]
    pub fn set_did_change_identifier(&mut self, value: bool) {
        self.did_change_identifier = value;
    }

    /// Returns true if resolved path changed.
    #[inline]
    #[must_use]
    pub fn did_change_resolved_path(&self) -> bool {
        self.did_change_resolved_path
    }

    /// Sets resolved path changed flag.
    #[inline]
    pub fn set_did_change_resolved_path(&mut self, value: bool) {
        self.did_change_resolved_path = value;
    }

    /// Returns true if content was replaced.
    #[inline]
    #[must_use]
    pub fn did_replace_content(&self) -> bool {
        self.did_replace_content
    }

    /// Sets content replaced flag.
    #[inline]
    pub fn set_did_replace_content(&mut self, value: bool) {
        self.did_replace_content = value;
    }

    /// Returns true if content was reloaded.
    #[inline]
    #[must_use]
    pub fn did_reload_content(&self) -> bool {
        self.did_reload_content
    }

    /// Sets content reloaded flag.
    #[inline]
    pub fn set_did_reload_content(&mut self, value: bool) {
        self.did_reload_content = value;
    }

    // Ordering flags

    /// Returns true if children were reordered.
    #[inline]
    #[must_use]
    pub fn did_reorder_children(&self) -> bool {
        self.did_reorder_children
    }

    /// Sets children reordered flag.
    #[inline]
    pub fn set_did_reorder_children(&mut self, value: bool) {
        self.did_reorder_children = value;
    }

    /// Returns true if properties were reordered.
    #[inline]
    #[must_use]
    pub fn did_reorder_properties(&self) -> bool {
        self.did_reorder_properties
    }

    /// Sets properties reordered flag.
    #[inline]
    pub fn set_did_reorder_properties(&mut self, value: bool) {
        self.did_reorder_properties = value;
    }

    /// Returns true if spec was renamed.
    #[inline]
    #[must_use]
    pub fn did_rename(&self) -> bool {
        self.did_rename
    }

    /// Sets renamed flag.
    #[inline]
    pub fn set_did_rename(&mut self, value: bool) {
        self.did_rename = value;
    }

    // Prim flags

    /// Returns true if variant sets changed.
    #[inline]
    #[must_use]
    pub fn did_change_prim_variant_sets(&self) -> bool {
        self.did_change_prim_variant_sets
    }

    /// Sets variant sets changed flag.
    #[inline]
    pub fn set_did_change_prim_variant_sets(&mut self, value: bool) {
        self.did_change_prim_variant_sets = value;
    }

    /// Returns true if inherit paths changed.
    #[inline]
    #[must_use]
    pub fn did_change_prim_inherit_paths(&self) -> bool {
        self.did_change_prim_inherit_paths
    }

    /// Sets inherit paths changed flag.
    #[inline]
    pub fn set_did_change_prim_inherit_paths(&mut self, value: bool) {
        self.did_change_prim_inherit_paths = value;
    }

    /// Returns true if specializes changed.
    #[inline]
    #[must_use]
    pub fn did_change_prim_specializes(&self) -> bool {
        self.did_change_prim_specializes
    }

    /// Sets specializes changed flag.
    #[inline]
    pub fn set_did_change_prim_specializes(&mut self, value: bool) {
        self.did_change_prim_specializes = value;
    }

    /// Returns true if references changed.
    #[inline]
    #[must_use]
    pub fn did_change_prim_references(&self) -> bool {
        self.did_change_prim_references
    }

    /// Sets references changed flag.
    #[inline]
    pub fn set_did_change_prim_references(&mut self, value: bool) {
        self.did_change_prim_references = value;
    }

    // Property flags

    /// Returns true if attribute time samples changed.
    #[inline]
    #[must_use]
    pub fn did_change_attribute_time_samples(&self) -> bool {
        self.did_change_attribute_time_samples
    }

    /// Sets time samples changed flag.
    #[inline]
    pub fn set_did_change_attribute_time_samples(&mut self, value: bool) {
        self.did_change_attribute_time_samples = value;
    }

    /// Returns true if attribute connections changed.
    #[inline]
    #[must_use]
    pub fn did_change_attribute_connection(&self) -> bool {
        self.did_change_attribute_connection
    }

    /// Sets connections changed flag.
    #[inline]
    pub fn set_did_change_attribute_connection(&mut self, value: bool) {
        self.did_change_attribute_connection = value;
    }

    /// Returns true if relationship targets changed.
    #[inline]
    #[must_use]
    pub fn did_change_relationship_targets(&self) -> bool {
        self.did_change_relationship_targets
    }

    /// Sets targets changed flag.
    #[inline]
    pub fn set_did_change_relationship_targets(&mut self, value: bool) {
        self.did_change_relationship_targets = value;
    }

    /// Returns true if a target was added.
    #[inline]
    #[must_use]
    pub fn did_add_target(&self) -> bool {
        self.did_add_target
    }

    /// Sets target added flag.
    #[inline]
    pub fn set_did_add_target(&mut self, value: bool) {
        self.did_add_target = value;
    }

    /// Returns true if a target was removed.
    #[inline]
    #[must_use]
    pub fn did_remove_target(&self) -> bool {
        self.did_remove_target
    }

    /// Sets target removed flag.
    #[inline]
    pub fn set_did_remove_target(&mut self, value: bool) {
        self.did_remove_target = value;
    }

    // Add/remove prim flags

    /// Returns true if an inert prim was added.
    #[inline]
    #[must_use]
    pub fn did_add_inert_prim(&self) -> bool {
        self.did_add_inert_prim
    }

    /// Sets inert prim added flag.
    #[inline]
    pub fn set_did_add_inert_prim(&mut self, value: bool) {
        self.did_add_inert_prim = value;
    }

    /// Returns true if a non-inert prim was added.
    #[inline]
    #[must_use]
    pub fn did_add_non_inert_prim(&self) -> bool {
        self.did_add_non_inert_prim
    }

    /// Sets non-inert prim added flag.
    #[inline]
    pub fn set_did_add_non_inert_prim(&mut self, value: bool) {
        self.did_add_non_inert_prim = value;
    }

    /// Returns true if an inert prim was removed.
    #[inline]
    #[must_use]
    pub fn did_remove_inert_prim(&self) -> bool {
        self.did_remove_inert_prim
    }

    /// Sets inert prim removed flag.
    #[inline]
    pub fn set_did_remove_inert_prim(&mut self, value: bool) {
        self.did_remove_inert_prim = value;
    }

    /// Returns true if a non-inert prim was removed.
    #[inline]
    #[must_use]
    pub fn did_remove_non_inert_prim(&self) -> bool {
        self.did_remove_non_inert_prim
    }

    /// Sets non-inert prim removed flag.
    #[inline]
    pub fn set_did_remove_non_inert_prim(&mut self, value: bool) {
        self.did_remove_non_inert_prim = value;
    }

    // Add/remove property flags

    /// Returns true if property with only required fields was added.
    #[inline]
    #[must_use]
    pub fn did_add_property_with_only_required_fields(&self) -> bool {
        self.did_add_property_with_only_required_fields
    }

    /// Sets flag.
    #[inline]
    pub fn set_did_add_property_with_only_required_fields(&mut self, value: bool) {
        self.did_add_property_with_only_required_fields = value;
    }

    /// Returns true if a property was added.
    #[inline]
    #[must_use]
    pub fn did_add_property(&self) -> bool {
        self.did_add_property
    }

    /// Sets property added flag.
    #[inline]
    pub fn set_did_add_property(&mut self, value: bool) {
        self.did_add_property = value;
    }

    /// Returns true if property with only required fields was removed.
    #[inline]
    #[must_use]
    pub fn did_remove_property_with_only_required_fields(&self) -> bool {
        self.did_remove_property_with_only_required_fields
    }

    /// Sets flag.
    #[inline]
    pub fn set_did_remove_property_with_only_required_fields(&mut self, value: bool) {
        self.did_remove_property_with_only_required_fields = value;
    }

    /// Returns true if a property was removed.
    #[inline]
    #[must_use]
    pub fn did_remove_property(&self) -> bool {
        self.did_remove_property
    }

    /// Sets property removed flag.
    #[inline]
    pub fn set_did_remove_property(&mut self, value: bool) {
        self.did_remove_property = value;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sublayer_change_type() {
        assert!(SubLayerChangeType::Added.is_add());
        assert!(!SubLayerChangeType::Added.is_remove());
        assert!(SubLayerChangeType::Removed.is_remove());
        assert!(SubLayerChangeType::Offset.is_offset());
    }

    #[test]
    fn test_sublayer_change_type_display() {
        assert_eq!(format!("{}", SubLayerChangeType::Added), "added");
        assert_eq!(format!("{}", SubLayerChangeType::Removed), "removed");
        assert_eq!(format!("{}", SubLayerChangeType::Offset), "offset");
    }

    #[test]
    fn test_change_flags_default() {
        let flags = ChangeFlags::default();
        assert!(!flags.any());
        assert!(!flags.did_rename());
    }

    #[test]
    fn test_change_flags_setters() {
        let mut flags = ChangeFlags::new();

        flags.set_did_rename(true);
        assert!(flags.did_rename());
        assert!(flags.any());

        flags.set_did_add_property(true);
        assert!(flags.did_add_property());

        flags.clear();
        assert!(!flags.any());
    }

    #[test]
    fn test_change_flags_layer() {
        let mut flags = ChangeFlags::new();

        flags.set_did_change_identifier(true);
        assert!(flags.did_change_identifier());

        flags.set_did_replace_content(true);
        assert!(flags.did_replace_content());
    }

    #[test]
    fn test_change_flags_prim() {
        let mut flags = ChangeFlags::new();

        flags.set_did_change_prim_variant_sets(true);
        flags.set_did_change_prim_inherit_paths(true);
        flags.set_did_change_prim_references(true);

        assert!(flags.did_change_prim_variant_sets());
        assert!(flags.did_change_prim_inherit_paths());
        assert!(flags.did_change_prim_references());
    }

    #[test]
    fn test_change_flags_property() {
        let mut flags = ChangeFlags::new();

        flags.set_did_change_attribute_time_samples(true);
        flags.set_did_change_relationship_targets(true);

        assert!(flags.did_change_attribute_time_samples());
        assert!(flags.did_change_relationship_targets());
    }

    #[test]
    fn test_change_flags_equality() {
        let mut f1 = ChangeFlags::new();
        let mut f2 = ChangeFlags::new();

        assert_eq!(f1, f2);

        f1.set_did_rename(true);
        assert_ne!(f1, f2);

        f2.set_did_rename(true);
        assert_eq!(f1, f2);
    }
}
