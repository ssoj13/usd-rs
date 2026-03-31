//! Debug codes for the pcp module.
//!
//! Port of pxr/usd/pcp/debugCodes.h
//!
//! Defines debug symbol names for enabling/disabling debug output during
//! prim cache population.

/// Debug code enumeration for pcp module.
///
/// Matches C++ `TF_DEBUG_CODES` macro expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PcpDebugCode {
    /// Pcp change processing
    Changes,
    /// Pcp dependencies
    Dependencies,
    /// Print debug output to terminal during prim indexing
    PrimIndex,
    /// Write graphviz 'dot' files during prim indexing (requires PCP_PRIM_INDEX)
    PrimIndexGraphs,
    /// Include namespace mappings in graphviz files generated during prim indexing
    /// (requires PCP_PRIM_INDEX_GRAPHS)
    PrimIndexGraphsMappings,
    /// Pcp namespace edits
    NamespaceEdit,
}

impl PcpDebugCode {
    /// Returns the debug code name as a string.
    ///
    /// Matches C++ `TF_DEBUG_ENVIRONMENT_SYMBOL` names.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Changes => "PCP_CHANGES",
            Self::Dependencies => "PCP_DEPENDENCIES",
            Self::PrimIndex => "PCP_PRIM_INDEX",
            Self::PrimIndexGraphs => "PCP_PRIM_INDEX_GRAPHS",
            Self::PrimIndexGraphsMappings => "PCP_PRIM_INDEX_GRAPHS_MAPPINGS",
            Self::NamespaceEdit => "PCP_NAMESPACE_EDIT",
        }
    }

    /// Returns the description of the debug code.
    ///
    /// Matches C++ `TF_DEBUG_ENVIRONMENT_SYMBOL` descriptions.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Changes => "Pcp change processing",
            Self::Dependencies => "Pcp dependencies",
            Self::PrimIndex => "Print debug output to terminal during prim indexing",
            Self::PrimIndexGraphs => {
                "Write graphviz 'dot' files during prim indexing (requires PCP_PRIM_INDEX)"
            }
            Self::PrimIndexGraphsMappings => {
                "Include namespace mappings in graphviz files generated during prim indexing (requires PCP_PRIM_INDEX_GRAPHS)"
            }
            Self::NamespaceEdit => "Pcp namespace edits",
        }
    }

    /// Returns all debug codes.
    pub fn all() -> &'static [PcpDebugCode] {
        &[
            Self::Changes,
            Self::Dependencies,
            Self::PrimIndex,
            Self::PrimIndexGraphs,
            Self::PrimIndexGraphsMappings,
            Self::NamespaceEdit,
        ]
    }
}

impl std::fmt::Display for PcpDebugCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_code_name() {
        assert_eq!(PcpDebugCode::Changes.name(), "PCP_CHANGES");
        assert_eq!(PcpDebugCode::PrimIndex.name(), "PCP_PRIM_INDEX");
    }

    #[test]
    fn test_all_codes() {
        let all = PcpDebugCode::all();
        assert_eq!(all.len(), 6);
    }

    #[test]
    fn test_display() {
        let code = PcpDebugCode::Dependencies;
        assert_eq!(format!("{}", code), "PCP_DEPENDENCIES");
    }
}
