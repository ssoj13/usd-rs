//! Debug codes for the tf module.
//!
//! Port of pxr/base/tf/debugCodes.h
//!
//! Defines debug symbol names for enabling/disabling debug output.

/// Debug code enumeration for tf module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TfDebugCode {
    /// Terse discovery output
    DiscoveryTerse,
    /// Detailed discovery output
    DiscoveryDetailed,
    /// Debug registry operations
    DebugRegistry,
    /// Debug dlopen calls
    DlOpen,
    /// Debug dlclose calls
    DlClose,
    /// Script module loader debug
    ScriptModuleLoader,
    /// Extra script module loader debug
    ScriptModuleLoaderExtra,
    /// Type registry debug
    TypeRegistry,
    /// Attach debugger on error
    AttachDebuggerOnError,
    /// Attach debugger on fatal error
    AttachDebuggerOnFatalError,
    /// Attach debugger on warning
    AttachDebuggerOnWarning,
}

impl TfDebugCode {
    /// Returns the debug code name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DiscoveryTerse => "TF_DISCOVERY_TERSE",
            Self::DiscoveryDetailed => "TF_DISCOVERY_DETAILED",
            Self::DebugRegistry => "TF_DEBUG_REGISTRY",
            Self::DlOpen => "TF_DLOPEN",
            Self::DlClose => "TF_DLCLOSE",
            Self::ScriptModuleLoader => "TF_SCRIPT_MODULE_LOADER",
            Self::ScriptModuleLoaderExtra => "TF_SCRIPT_MODULE_LOADER_EXTRA",
            Self::TypeRegistry => "TF_TYPE_REGISTRY",
            Self::AttachDebuggerOnError => "TF_ATTACH_DEBUGGER_ON_ERROR",
            Self::AttachDebuggerOnFatalError => "TF_ATTACH_DEBUGGER_ON_FATAL_ERROR",
            Self::AttachDebuggerOnWarning => "TF_ATTACH_DEBUGGER_ON_WARNING",
        }
    }

    /// Returns all debug codes.
    pub fn all() -> &'static [TfDebugCode] {
        &[
            Self::DiscoveryTerse,
            Self::DiscoveryDetailed,
            Self::DebugRegistry,
            Self::DlOpen,
            Self::DlClose,
            Self::ScriptModuleLoader,
            Self::ScriptModuleLoaderExtra,
            Self::TypeRegistry,
            Self::AttachDebuggerOnError,
            Self::AttachDebuggerOnFatalError,
            Self::AttachDebuggerOnWarning,
        ]
    }
}

impl std::fmt::Display for TfDebugCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_code_name() {
        assert_eq!(TfDebugCode::DiscoveryTerse.name(), "TF_DISCOVERY_TERSE");
        assert_eq!(TfDebugCode::DlOpen.name(), "TF_DLOPEN");
    }

    #[test]
    fn test_all_codes() {
        let all = TfDebugCode::all();
        assert_eq!(all.len(), 11);
    }

    #[test]
    fn test_display() {
        let code = TfDebugCode::TypeRegistry;
        assert_eq!(format!("{}", code), "TF_TYPE_REGISTRY");
    }
}
