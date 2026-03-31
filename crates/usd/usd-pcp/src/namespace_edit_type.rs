//! Namespace edit type enumeration.
//!
//! Defines types of edits required to respond to namespace changes.
//!
//! # Examples
//!
//! ```
//! use usd_pcp::NamespaceEditType;
//!
//! let edit = NamespaceEditType::EditPath;
//! assert!(edit.affects_spec());
//! ```

use std::fmt;

/// Type of namespace edit required at a layer stack site.
///
/// When a namespace edit occurs (rename, reparent, remove), sites that
/// depend on the edited path must respond appropriately. This enum
/// describes what type of edit each affected site needs to perform.
///
/// # Examples
///
/// ```
/// use usd_pcp::NamespaceEditType;
///
/// // Direct spec edit
/// let edit = NamespaceEditType::EditPath;
/// assert!(edit.affects_spec());
///
/// // Composition arc edits
/// let ref_edit = NamespaceEditType::EditReference;
/// assert!(ref_edit.affects_arc());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NamespaceEditType {
    /// Must namespace edit the spec directly.
    EditPath = 0,
    /// Must fix up inherit paths.
    EditInherit = 1,
    /// Must fix up specializes paths.
    EditSpecializes = 2,
    /// Must fix up reference paths.
    EditReference = 3,
    /// Must fix up payload paths.
    EditPayload = 4,
    /// Must fix up relocate paths.
    EditRelocate = 5,
}

impl NamespaceEditType {
    /// Returns true if this edit type affects a spec path directly.
    #[inline]
    #[must_use]
    pub fn affects_spec(self) -> bool {
        matches!(self, Self::EditPath)
    }

    /// Returns true if this edit type affects a composition arc.
    #[inline]
    #[must_use]
    pub fn affects_arc(self) -> bool {
        matches!(
            self,
            Self::EditInherit
                | Self::EditSpecializes
                | Self::EditReference
                | Self::EditPayload
                | Self::EditRelocate
        )
    }

    /// Returns true if this is an inherit edit.
    #[inline]
    #[must_use]
    pub fn is_inherit(self) -> bool {
        matches!(self, Self::EditInherit)
    }

    /// Returns true if this is a specializes edit.
    #[inline]
    #[must_use]
    pub fn is_specializes(self) -> bool {
        matches!(self, Self::EditSpecializes)
    }

    /// Returns true if this is a reference edit.
    #[inline]
    #[must_use]
    pub fn is_reference(self) -> bool {
        matches!(self, Self::EditReference)
    }

    /// Returns true if this is a payload edit.
    #[inline]
    #[must_use]
    pub fn is_payload(self) -> bool {
        matches!(self, Self::EditPayload)
    }

    /// Returns true if this is a relocate edit.
    #[inline]
    #[must_use]
    pub fn is_relocate(self) -> bool {
        matches!(self, Self::EditRelocate)
    }

    /// Returns all edit types.
    pub fn all() -> &'static [Self] {
        &[
            Self::EditPath,
            Self::EditInherit,
            Self::EditSpecializes,
            Self::EditReference,
            Self::EditPayload,
            Self::EditRelocate,
        ]
    }
}

impl fmt::Display for NamespaceEditType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EditPath => write!(f, "edit path"),
            Self::EditInherit => write!(f, "edit inherit"),
            Self::EditSpecializes => write!(f, "edit specializes"),
            Self::EditReference => write!(f, "edit reference"),
            Self::EditPayload => write!(f, "edit payload"),
            Self::EditRelocate => write!(f, "edit relocate"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_affects_spec() {
        assert!(NamespaceEditType::EditPath.affects_spec());
        assert!(!NamespaceEditType::EditInherit.affects_spec());
        assert!(!NamespaceEditType::EditReference.affects_spec());
    }

    #[test]
    fn test_affects_arc() {
        assert!(!NamespaceEditType::EditPath.affects_arc());
        assert!(NamespaceEditType::EditInherit.affects_arc());
        assert!(NamespaceEditType::EditSpecializes.affects_arc());
        assert!(NamespaceEditType::EditReference.affects_arc());
        assert!(NamespaceEditType::EditPayload.affects_arc());
        assert!(NamespaceEditType::EditRelocate.affects_arc());
    }

    #[test]
    fn test_specific_checks() {
        assert!(NamespaceEditType::EditInherit.is_inherit());
        assert!(NamespaceEditType::EditSpecializes.is_specializes());
        assert!(NamespaceEditType::EditReference.is_reference());
        assert!(NamespaceEditType::EditPayload.is_payload());
        assert!(NamespaceEditType::EditRelocate.is_relocate());
    }

    #[test]
    fn test_all() {
        let all = NamespaceEditType::all();
        assert_eq!(all.len(), 6);
        assert_eq!(all[0], NamespaceEditType::EditPath);
        assert_eq!(all[5], NamespaceEditType::EditRelocate);
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", NamespaceEditType::EditPath), "edit path");
        assert_eq!(
            format!("{}", NamespaceEditType::EditInherit),
            "edit inherit"
        );
        assert_eq!(
            format!("{}", NamespaceEditType::EditReference),
            "edit reference"
        );
    }

    #[test]
    fn test_equality() {
        assert_eq!(NamespaceEditType::EditPath, NamespaceEditType::EditPath);
        assert_ne!(NamespaceEditType::EditPath, NamespaceEditType::EditInherit);
    }
}
