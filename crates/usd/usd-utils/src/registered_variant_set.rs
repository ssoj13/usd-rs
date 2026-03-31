//! Registered variant set definitions.
//!
//! Provides [`RegisteredVariantSet`] for pipeline-registered variant sets
//! that may need special handling during import/export.

use std::cmp::Ordering;

/// Specifies how a variant selection should be exported.
///
/// This enum controls the behavior during export operations for registered
/// variant sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionExportPolicy {
    /// Never export this variant selection.
    ///
    /// This typically represents a "session" variant selection that should
    /// not be transmitted down the pipeline.
    Never,

    /// Export the selection only if there is an authored opinion.
    ///
    /// This is only relevant if the application can distinguish between
    /// "default" and "set" opinions.
    IfAuthored,

    /// Always export the variant selection.
    Always,
}

impl SelectionExportPolicy {
    /// Parses a selection export policy from a string.
    ///
    /// Valid values are: "never", "ifAuthored", "always" (case-insensitive).
    ///
    /// Returns `None` if the string doesn't match any known policy.
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "never" => Some(Self::Never),
            "ifauthored" => Some(Self::IfAuthored),
            "always" => Some(Self::Always),
            _ => None,
        }
    }

    /// Returns the string representation of this policy.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::IfAuthored => "ifAuthored",
            Self::Always => "always",
        }
    }
}

impl std::fmt::Display for SelectionExportPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Information about a variant set registered with the pipeline.
///
/// Registered variant sets are known variant sets in a pipeline that may
/// need to be reasoned about by applications during import/export.
///
/// # Examples
///
/// ```ignore
/// use usd_core::usd_utils::{RegisteredVariantSet, SelectionExportPolicy};
///
/// let variant_set = RegisteredVariantSet::new(
///     "modelingVariant",
///     SelectionExportPolicy::Always,
/// );
///
/// assert_eq!(variant_set.name(), "modelingVariant");
/// ```
#[derive(Debug, Clone)]
pub struct RegisteredVariantSet {
    /// The name of the variant set.
    name: String,
    /// How this variant set's selection should be exported.
    selection_export_policy: SelectionExportPolicy,
}

impl RegisteredVariantSet {
    /// Creates a new registered variant set.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the variant set
    /// * `selection_export_policy` - How the selection should be exported
    pub fn new(name: impl Into<String>, selection_export_policy: SelectionExportPolicy) -> Self {
        Self {
            name: name.into(),
            selection_export_policy,
        }
    }

    /// Returns the name of this variant set.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the selection export policy for this variant set.
    pub fn selection_export_policy(&self) -> SelectionExportPolicy {
        self.selection_export_policy
    }
}

impl PartialEq for RegisteredVariantSet {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for RegisteredVariantSet {}

impl PartialOrd for RegisteredVariantSet {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegisteredVariantSet {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl std::hash::Hash for RegisteredVariantSet {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_export_policy_from_string() {
        assert_eq!(
            SelectionExportPolicy::from_string("never"),
            Some(SelectionExportPolicy::Never)
        );
        assert_eq!(
            SelectionExportPolicy::from_string("ifAuthored"),
            Some(SelectionExportPolicy::IfAuthored)
        );
        assert_eq!(
            SelectionExportPolicy::from_string("always"),
            Some(SelectionExportPolicy::Always)
        );
        assert_eq!(SelectionExportPolicy::from_string("invalid"), None);
    }

    #[test]
    fn test_registered_variant_set() {
        let vs = RegisteredVariantSet::new("modelingVariant", SelectionExportPolicy::Always);
        assert_eq!(vs.name(), "modelingVariant");
        assert_eq!(vs.selection_export_policy(), SelectionExportPolicy::Always);
    }

    #[test]
    fn test_ordering() {
        let a = RegisteredVariantSet::new("aaa", SelectionExportPolicy::Never);
        let b = RegisteredVariantSet::new("bbb", SelectionExportPolicy::Always);
        assert!(a < b);
    }
}
