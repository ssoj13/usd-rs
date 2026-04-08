//! Debug codes for hdMtlx module.

use usd_tf::Token;

/// Debug codes for MaterialX-related operations in Hydra.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdMtlxDebugCode {
    /// Debug MaterialX document creation and manipulation.
    Document,

    /// Debug MaterialX version upgrade operations.
    VersionUpgrade,

    /// Debug MaterialX document writing (with includes).
    WriteDocument,

    /// Debug MaterialX document writing (without includes).
    WriteDocumentWithoutIncludes,
}

impl HdMtlxDebugCode {
    /// Get the debug code as a token for use with TF_DEBUG.
    pub fn as_token(&self) -> Token {
        Token::new(self.as_str())
    }

    /// Get the debug code as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Document => "HDMTLX_DOCUMENT",
            Self::VersionUpgrade => "HDMTLX_VERSION_UPGRADE",
            Self::WriteDocument => "HDMTLX_WRITE_DOCUMENT",
            Self::WriteDocumentWithoutIncludes => "HDMTLX_WRITE_DOCUMENT_WITHOUT_INCLUDES",
        }
    }

    /// Get the log target string for use with the `log` crate.
    pub fn log_target(&self) -> &'static str {
        match self {
            Self::Document | Self::WriteDocument | Self::WriteDocumentWithoutIncludes => {
                "hd_mtlx::document"
            }
            Self::VersionUpgrade => "hd_mtlx::version",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_code_strings() {
        assert_eq!(HdMtlxDebugCode::Document.as_str(), "HDMTLX_DOCUMENT");
        assert_eq!(
            HdMtlxDebugCode::VersionUpgrade.as_str(),
            "HDMTLX_VERSION_UPGRADE"
        );
        assert_eq!(
            HdMtlxDebugCode::WriteDocument.as_str(),
            "HDMTLX_WRITE_DOCUMENT"
        );
        assert_eq!(
            HdMtlxDebugCode::WriteDocumentWithoutIncludes.as_str(),
            "HDMTLX_WRITE_DOCUMENT_WITHOUT_INCLUDES"
        );
    }

    #[test]
    fn test_debug_code_log_targets() {
        assert_eq!(HdMtlxDebugCode::Document.log_target(), "hd_mtlx::document");
        assert_eq!(
            HdMtlxDebugCode::VersionUpgrade.log_target(),
            "hd_mtlx::version"
        );
        assert_eq!(
            HdMtlxDebugCode::WriteDocument.log_target(),
            "hd_mtlx::document"
        );
    }
}
