//! Debug codes for the vt (Value Types) module.
//!
//! Port of pxr/base/vt/debugCodes.h

/// Debug code enumeration for the vt module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VtDebugCode {
    /// Debug array edit bounds checking.
    ArrayEditBounds,
}

impl VtDebugCode {
    /// Returns the debug code name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            Self::ArrayEditBounds => "VT_ARRAY_EDIT_BOUNDS",
        }
    }

    /// Returns all debug codes.
    pub fn all() -> &'static [VtDebugCode] {
        &[Self::ArrayEditBounds]
    }
}

impl std::fmt::Display for VtDebugCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_code_name() {
        assert_eq!(VtDebugCode::ArrayEditBounds.name(), "VT_ARRAY_EDIT_BOUNDS");
    }

    #[test]
    fn test_all_codes() {
        assert_eq!(VtDebugCode::all().len(), 1);
    }

    #[test]
    fn test_display() {
        assert_eq!(
            format!("{}", VtDebugCode::ArrayEditBounds),
            "VT_ARRAY_EDIT_BOUNDS"
        );
    }
}
