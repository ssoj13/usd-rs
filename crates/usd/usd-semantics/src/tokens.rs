//! UsdSemantics tokens.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdSemantics/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdSemantics schemas.
pub struct UsdSemanticsTokensType {
    /// "semantics:labels" - Full namespace for labels attribute (e.g. semantics:labels:taxonomy)
    pub semantics_labels: Token,
    /// "semantics:labels:__INSTANCE_NAME__" - Multiple-apply template token used as base name
    /// for MakeMultipleApplyNameInstance. The literal string includes __INSTANCE_NAME__.
    pub semantics_labels_multiple_apply_template: Token,
    /// "SemanticsLabelsAPI" - Schema identifier
    pub semantics_labels_api: Token,
}

impl UsdSemanticsTokensType {
    fn new() -> Self {
        Self {
            semantics_labels: Token::new("semantics:labels"),
            semantics_labels_multiple_apply_template: Token::new(
                "semantics:labels:__INSTANCE_NAME__",
            ),
            semantics_labels_api: Token::new("SemanticsLabelsAPI"),
        }
    }
}

/// Global tokens instance for UsdSemantics schemas.
pub static USD_SEMANTICS_TOKENS: LazyLock<UsdSemanticsTokensType> =
    LazyLock::new(UsdSemanticsTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(
            USD_SEMANTICS_TOKENS.semantics_labels.as_str(),
            "semantics:labels"
        );
        assert_eq!(
            USD_SEMANTICS_TOKENS
                .semantics_labels_multiple_apply_template
                .as_str(),
            "semantics:labels:__INSTANCE_NAME__"
        );
        assert_eq!(
            USD_SEMANTICS_TOKENS.semantics_labels_api.as_str(),
            "SemanticsLabelsAPI"
        );
    }
}
