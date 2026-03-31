//! USD Skel Validators - validators for skeletal animation schemas.
//!
//! Port of pxr/usdValidation/usdSkelValidators

use super::{
    ErrorSite, ErrorType, ValidatePrimTaskFn, ValidationError, ValidationRegistry,
    ValidatorMetadata,
};
use std::sync::Arc;
use usd_skel::{BindingAPI, SkelRoot};

// ============================================================================
// Tokens
// ============================================================================

/// Token constants for skel validators.
pub mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Validator token: SkelBindingApiAppliedValidator.
    pub static SKEL_BINDING_API_APPLIED_VALIDATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdSkelValidators:SkelBindingApiAppliedValidator"));

    /// Validator token: SkelBindingApiValidator.
    pub static SKEL_BINDING_API_VALIDATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdSkelValidators:SkelBindingApiValidator"));

    /// Error token: missing SkelBindingAPI.
    pub static MISSING_SKEL_BINDING_API: LazyLock<Token> =
        LazyLock::new(|| Token::new("MissingSkelBindingAPI"));

    /// Error token: invalid SkelBindingAPI apply.
    pub static INVALID_SKEL_BINDING_API_APPLY: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidSkelBindingAPIApply"));

    /// Schema family token: SkelBindingAPI.
    pub static SKEL_BINDING_API: LazyLock<Token> = LazyLock::new(|| Token::new("SkelBindingAPI"));

    /// Keyword for UsdSkel validators registration.
    pub static USD_SKEL_VALIDATORS: LazyLock<Token> =
        LazyLock::new(|| Token::new("UsdSkelValidators"));
}

// ============================================================================
// Helpers
// ============================================================================

/// Check if prim has SkelBindingAPI applied.
///
/// Checks both PrimData cache and raw metadata (handles runtime-applied APIs).
fn has_skel_binding_api(prim: &usd_core::Prim) -> bool {
    // Check PrimData cache first, then metadata fallback for runtime-applied APIs
    if prim.has_api_in_family(&tokens::SKEL_BINDING_API) {
        return true;
    }
    // Fallback: read apiSchemas metadata directly (PrimData cache may not reflect
    // runtime changes from add_applied_schema/apply_api)
    let key = usd_core::tokens::usd_tokens().api_schemas.clone();
    prim.get_metadata::<usd_sdf::list_op::TokenListOp>(&key)
        .map(|list_op| list_op.has_item(&tokens::SKEL_BINDING_API))
        .unwrap_or(false)
}

// ============================================================================
// SkelBindingApiAppliedValidator
// ============================================================================

/// Validator that checks if prims with SkelBindingAPI properties have the API applied.
///
/// If a prim does NOT have SkelBindingAPI applied but has properties that belong
/// to SkelBindingAPI, this validator reports an error.
///
/// Matches C++ `SkelBindingApiAppliedValidator`.
fn skel_binding_api_applied_validator_fn() -> ValidatePrimTaskFn {
    Arc::new(|prim, _time_range| {
        // If prim already has SkelBindingAPI applied, nothing to check
        if has_skel_binding_api(prim) {
            return Vec::new();
        }

        // Get all property names defined by SkelBindingAPI
        let skel_binding_props = BindingAPI::get_schema_attribute_names(true);

        // Get all property names on this prim
        let prim_props = prim.get_property_names();

        // Check if any prim property matches a SkelBindingAPI property
        for prop_name in &prim_props {
            if skel_binding_props.contains(prop_name) {
                // Found a SkelBindingAPI property without the API applied
                let stage = match prim.stage() {
                    Some(s) => s,
                    None => {
                        return vec![ValidationError::simple(
                            tokens::MISSING_SKEL_BINDING_API.clone(),
                            ErrorType::Error,
                            format!(
                                "Found a UsdSkelBinding property ({}), but no SkelBindingAPI \
                                 applied on the prim <{}>.",
                                prop_name.as_str(),
                                prim.get_path()
                            ),
                        )];
                    }
                };

                return vec![ValidationError::new(
                    tokens::MISSING_SKEL_BINDING_API.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
                    format!(
                        "Found a UsdSkelBinding property ({}), but no SkelBindingAPI \
                         applied on the prim <{}>.",
                        prop_name.as_str(),
                        prim.get_path()
                    ),
                )];
            }
        }

        Vec::new()
    })
}

// ============================================================================
// SkelBindingApiValidator
// ============================================================================

/// Validator that checks if SkelBindingAPI is applied correctly.
///
/// If a prim has SkelBindingAPI applied but is not of type SkelRoot and is not
/// rooted at a SkelRoot ancestor, this validator reports an error.
///
/// Matches C++ `SkelBindingApiValidator`.
fn skel_binding_api_validator_fn() -> ValidatePrimTaskFn {
    Arc::new(|prim, _time_range| {
        // Only check if prim has SkelBindingAPI applied
        if !has_skel_binding_api(prim) {
            return Vec::new();
        }

        // If prim is a SkelRoot, it's valid
        let prim_type = prim.get_type_name();
        let skel_root_type = SkelRoot::schema_type_name();
        if prim_type == skel_root_type {
            return Vec::new();
        }

        // Walk up parent chain to find SkelRoot
        let mut current = prim.parent();
        while current.is_valid() && !current.is_pseudo_root() {
            let current_type = current.get_type_name();
            if current_type == skel_root_type {
                // Found SkelRoot ancestor - valid
                return Vec::new();
            }
            current = current.parent();
        }

        // No SkelRoot found in ancestor chain
        let stage = match prim.stage() {
            Some(s) => s,
            None => {
                return vec![ValidationError::simple(
                    tokens::INVALID_SKEL_BINDING_API_APPLY.clone(),
                    ErrorType::Error,
                    format!(
                        "UsdSkelBindingAPI applied on prim: <{}>, which is not of type \
                         SkelRoot or is not rooted at a prim of type SkelRoot",
                        prim.get_path()
                    ),
                )];
            }
        };

        vec![ValidationError::new(
            tokens::INVALID_SKEL_BINDING_API_APPLY.clone(),
            ErrorType::Error,
            vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
            format!(
                "UsdSkelBindingAPI applied on prim: <{}>, which is not of type SkelRoot \
                 or is not rooted at a prim of type SkelRoot",
                prim.get_path()
            ),
        )]
    })
}

// ============================================================================
// Registration
// ============================================================================

/// Register all skel validators with the validation registry.
///
/// Matches C++ `UsdSkelValidators` registration.
pub fn register_skel_validators(registry: &ValidationRegistry) {
    // Register SkelBindingApiAppliedValidator
    registry.register_prim_validator(
        ValidatorMetadata::new(tokens::SKEL_BINDING_API_APPLIED_VALIDATOR.clone())
            .with_doc(
                "Checks if prims with SkelBindingAPI properties have the API applied.".to_string(),
            )
            .with_keywords(vec![tokens::USD_SKEL_VALIDATORS.clone()]),
        skel_binding_api_applied_validator_fn(),
        Vec::new(),
    );

    // Register SkelBindingApiValidator
    registry.register_prim_validator(
        ValidatorMetadata::new(tokens::SKEL_BINDING_API_VALIDATOR.clone())
            .with_doc(
                "Checks if SkelBindingAPI is applied on prims rooted at SkelRoot.".to_string(),
            )
            .with_keywords(vec![tokens::USD_SKEL_VALIDATORS.clone()]),
        skel_binding_api_validator_fn(),
        Vec::new(),
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;
    use usd_core::stage::Stage;

    #[test]
    fn test_skel_binding_api_applied_validator_no_api_no_props() {
        // Prim with no SkelBindingAPI and no SkelBinding properties - should pass
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();

        let validator = skel_binding_api_applied_validator_fn();
        let errors = validator(&prim, &super::super::ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_skel_binding_api_applied_validator_has_api() {
        // Prim with SkelBindingAPI applied - should pass even with properties
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();

        // Properly apply SkelBindingAPI via metadata
        prim.add_applied_schema(&usd_tf::Token::new("SkelBindingAPI"));

        let validator = skel_binding_api_applied_validator_fn();
        let errors = validator(&prim, &super::super::ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_skel_binding_api_applied_validator_missing_api() {
        // Prim with SkelBinding property but no API applied - should fail
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();

        // Create a SkelBinding property without applying the API
        let prop_name = usd_skel::tokens::tokens().skel_joints.as_str();
        let type_name = usd_sdf::ValueTypeRegistry::instance().find_type("token[]");
        let _ = prim.create_attribute(prop_name, &type_name, false, None);

        let validator = skel_binding_api_applied_validator_fn();
        let errors = validator(&prim, &super::super::ValidationTimeRange::default());

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].get_name(), &*tokens::MISSING_SKEL_BINDING_API);
        assert_eq!(errors[0].get_type(), ErrorType::Error);
        assert!(
            errors[0]
                .get_message()
                .contains("no SkelBindingAPI applied")
        );
    }

    #[test]
    fn test_skel_binding_api_validator_no_api() {
        // Prim without SkelBindingAPI - should pass
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();

        let validator = skel_binding_api_validator_fn();
        let errors = validator(&prim, &super::super::ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_skel_binding_api_validator_on_skel_root() {
        // SkelBindingAPI on SkelRoot - should pass
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let root = stage.define_prim("/Root", "SkelRoot").unwrap();

        // Properly apply SkelBindingAPI via metadata
        root.add_applied_schema(&usd_tf::Token::new("SkelBindingAPI"));

        let validator = skel_binding_api_validator_fn();
        let errors = validator(&root, &super::super::ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_skel_binding_api_validator_under_skel_root() {
        // SkelBindingAPI on child of SkelRoot - should pass
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let _root = stage.define_prim("/Root", "SkelRoot").unwrap();
        let child = stage.define_prim("/Root/Child", "Xform").unwrap();

        // Properly apply SkelBindingAPI via metadata
        child.add_applied_schema(&usd_tf::Token::new("SkelBindingAPI"));

        let validator = skel_binding_api_validator_fn();
        let errors = validator(&child, &super::super::ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_skel_binding_api_validator_not_under_skel_root() {
        // SkelBindingAPI on prim not under SkelRoot - should fail
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();

        // Properly apply SkelBindingAPI via metadata
        prim.add_applied_schema(&usd_tf::Token::new("SkelBindingAPI"));

        let validator = skel_binding_api_validator_fn();
        let errors = validator(&prim, &super::super::ValidationTimeRange::default());

        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].get_name(),
            &*tokens::INVALID_SKEL_BINDING_API_APPLY
        );
        assert_eq!(errors[0].get_type(), ErrorType::Error);
        assert!(errors[0].get_message().contains("not rooted at"));
    }

    #[test]
    fn test_skel_binding_api_validator_deep_hierarchy() {
        // SkelBindingAPI on deeply nested child of SkelRoot - should pass
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let _root = stage.define_prim("/Root", "SkelRoot").unwrap();
        let _mid = stage.define_prim("/Root/Mid", "Xform").unwrap();
        let child = stage.define_prim("/Root/Mid/Child", "Xform").unwrap();

        // Properly apply SkelBindingAPI via metadata
        child.add_applied_schema(&usd_tf::Token::new("SkelBindingAPI"));

        let validator = skel_binding_api_validator_fn();
        let errors = validator(&child, &super::super::ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_token_constants() {
        assert_eq!(
            tokens::SKEL_BINDING_API_APPLIED_VALIDATOR.as_str(),
            "usdSkelValidators:SkelBindingApiAppliedValidator"
        );
        assert_eq!(
            tokens::SKEL_BINDING_API_VALIDATOR.as_str(),
            "usdSkelValidators:SkelBindingApiValidator"
        );
        assert_eq!(
            tokens::MISSING_SKEL_BINDING_API.as_str(),
            "MissingSkelBindingAPI"
        );
        assert_eq!(
            tokens::INVALID_SKEL_BINDING_API_APPLY.as_str(),
            "InvalidSkelBindingAPIApply"
        );
    }
}
