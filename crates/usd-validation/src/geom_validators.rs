//! USD Geom Validators - validation for UsdGeom schemas.
//!
//! Port of _ref/OpenUSD/pxr/usdValidation/usdGeomValidators/validatorTokens.h/cpp
//! and usdGeomValidators/validators.cpp
//!
//! Provides 4 validators:
//! - StageMetadataChecker: Validates stage has metersPerUnit and upAxis metadata
//! - SubsetFamilies: Validates UsdGeomSubset family membership
//! - SubsetParentIsImageable: Validates UsdGeomSubset parent is imageable
//! - EncapsulationChecker: Validates proper Gprim encapsulation

use crate::{
    ErrorSite, ErrorType, ValidatePrimTaskFn, ValidateStageTaskFn, ValidationError,
    ValidationRegistry, ValidationTimeRange, ValidatorMetadata,
};
use std::collections::HashSet;
use std::sync::{Arc, LazyLock};
use usd_core::Prim;
use usd_geom::{Boundable, Imageable, Subset, usd_geom_tokens};
use usd_sdf::Path;
use usd_tf::Token;

// ============================================================================
// Validator Name Tokens (prefixed with "usdGeomValidators:")
// ============================================================================

/// Token for StageMetadataChecker validator.
pub static STAGE_METADATA_CHECKER: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdGeomValidators:StageMetadataChecker"));

/// Token for SubsetFamilies validator.
pub static SUBSET_FAMILIES: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdGeomValidators:SubsetFamilies"));

/// Token for SubsetParentIsImageable validator.
pub static SUBSET_PARENT_IS_IMAGEABLE: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdGeomValidators:SubsetParentIsImageable"));

/// Token for EncapsulationChecker validator.
pub static ENCAPSULATION_CHECKER: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdGeomValidators:EncapsulationChecker"));

// ============================================================================
// Error Name Tokens
// ============================================================================

/// Error: missing metersPerUnit metadata.
pub static MISSING_METERS_PER_UNIT_METADATA: LazyLock<Token> =
    LazyLock::new(|| Token::new("MissingMetersPerUnitMetadata"));

/// Error: missing upAxis metadata.
pub static MISSING_UP_AXIS_METADATA: LazyLock<Token> =
    LazyLock::new(|| Token::new("MissingUpAxisMetadata"));

/// Error: invalid subset family.
pub static INVALID_SUBSET_FAMILY: LazyLock<Token> =
    LazyLock::new(|| Token::new("InvalidSubsetFamily"));

/// Error: subset parent is not imageable.
pub static NOT_IMAGEABLE_SUBSET_PARENT: LazyLock<Token> =
    LazyLock::new(|| Token::new("NotImageableSubsetParent"));

/// Error: invalid nested Gprims.
pub static INVALID_NESTED_GPRIMS: LazyLock<Token> =
    LazyLock::new(|| Token::new("InvalidNestedGprims"));

// ============================================================================
// Metadata Key Tokens
// ============================================================================

static METERS_PER_UNIT_KEY: LazyLock<Token> = LazyLock::new(|| Token::new("metersPerUnit"));
static UP_AXIS_KEY: LazyLock<Token> = LazyLock::new(|| Token::new("upAxis"));
static IMAGEABLE_KEY: LazyLock<Token> = LazyLock::new(|| Token::new("Imageable"));
static SUBSET_KEY: LazyLock<Token> = LazyLock::new(|| Token::new("GeomSubset"));
static GPRIM_KEY: LazyLock<Token> = LazyLock::new(|| Token::new("Gprim"));
static BOUNDABLE_KEY: LazyLock<Token> = LazyLock::new(|| Token::new("Boundable"));

/// Check if prim is in the Boundable family (uses SchemaRegistry hierarchy).
fn is_boundable(prim: &Prim) -> bool {
    prim.is_in_family(&BOUNDABLE_KEY)
}

/// Check if prim is in the Gprim family.
fn is_gprim(prim: &Prim) -> bool {
    prim.is_in_family(&GPRIM_KEY)
}

/// Check if prim is a GeomSubset.
fn is_geom_subset(prim: &Prim) -> bool {
    prim.is_a(&SUBSET_KEY)
}

/// Check if prim is in the Imageable family.
fn is_imageable(prim: &Prim) -> bool {
    prim.is_in_family(&IMAGEABLE_KEY)
}

// ============================================================================
// Validator 1: StageMetadataChecker
// ============================================================================

/// Validates that stage has required metadata: metersPerUnit and upAxis.
fn stage_metadata_checker_fn() -> ValidateStageTaskFn {
    Arc::new(
        |stage: &Arc<usd_core::Stage>, _time_range: &ValidationTimeRange| {
            let mut errors = Vec::new();

            // Check metersPerUnit
            if !stage.has_authored_metadata(&METERS_PER_UNIT_KEY) {
                errors.push(ValidationError::new(
                    MISSING_METERS_PER_UNIT_METADATA.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(stage, Path::absolute_root(), None)],
                    "Stage is missing required 'metersPerUnit' metadata.".to_string(),
                ));
            }

            // Check upAxis
            if !stage.has_authored_metadata(&UP_AXIS_KEY) {
                errors.push(ValidationError::new(
                    MISSING_UP_AXIS_METADATA.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(stage, Path::absolute_root(), None)],
                    "Stage is missing required 'upAxis' metadata.".to_string(),
                ));
            }

            errors
        },
    )
}

// ============================================================================
// Validator 2: SubsetFamilies
// ============================================================================

/// Validates UsdGeomSubset family membership for Imageable prims.
fn subset_families_fn() -> ValidatePrimTaskFn {
    Arc::new(|prim: &Prim, _time_range: &ValidationTimeRange| {
        // Only validate Imageable prims
        if !is_imageable(prim) {
            return Vec::new();
        }

        let imageable = Imageable::new(prim.clone());
        if !imageable.is_valid() {
            return Vec::new();
        }

        let mut errors = Vec::new();

        // Get all subset family names from child subsets
        let mut family_names = HashSet::new();
        for child in prim.get_children() {
            if is_geom_subset(&child) {
                let subset = Subset::new(child);
                if let Some(family) = subset.get_family_name(usd_sdf::TimeCode::default_time()) {
                    family_names.insert(family);
                }
            }
        }

        // Convert to sorted Vec for deterministic validation order
        let mut families: Vec<Token> = family_names.into_iter().collect();
        families.sort_by(|a, b| a.as_str().cmp(b.as_str()));

        // Validate each family using Subset::validate_family() per C++ reference.
        // It checks partitioning/non-overlapping constraints and index bounds.
        let face_token = usd_geom_tokens().face.clone();
        for family_name in families {
            let (valid, reason) = Subset::validate_family(&imageable, &face_token, &family_name);
            if !valid {
                errors.push(ValidationError::new(
                    INVALID_SUBSET_FAMILY.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(
                        &prim.stage().unwrap(),
                        prim.get_path().clone(),
                        None,
                    )],
                    format!(
                        "Prim '{}' has invalid subset family '{}': {}",
                        prim.get_path(),
                        family_name.as_str(),
                        reason
                    ),
                ));
            }
        }

        errors
    })
}

// ============================================================================
// Validator 3: SubsetParentIsImageable
// ============================================================================

/// Validates that UsdGeomSubset parent prim is Imageable.
fn subset_parent_is_imageable_fn() -> ValidatePrimTaskFn {
    Arc::new(|prim: &Prim, _time_range: &ValidationTimeRange| {
        // Only validate Subset prims
        if !is_geom_subset(prim) {
            return Vec::new();
        }

        let parent = prim.parent();
        if !parent.is_valid() || parent.is_pseudo_root() {
            return vec![ValidationError::new(
                NOT_IMAGEABLE_SUBSET_PARENT.clone(),
                ErrorType::Error,
                vec![ErrorSite::from_stage(
                    &prim.stage().unwrap(),
                    prim.get_path().clone(),
                    None,
                )],
                format!(
                    "Subset prim '{}' has invalid parent (pseudo-root or invalid).",
                    prim.get_path()
                ),
            )];
        }

        // Check if parent is Imageable
        if !is_imageable(&parent) {
            return vec![ValidationError::new(
                NOT_IMAGEABLE_SUBSET_PARENT.clone(),
                ErrorType::Error,
                vec![ErrorSite::from_stage(
                    &prim.stage().unwrap(),
                    prim.get_path().clone(),
                    None,
                )],
                format!(
                    "Subset prim '{}' parent '{}' is not an Imageable. \
                     Subsets must be children of Imageable prims.",
                    prim.get_path(),
                    parent.get_path()
                ),
            )];
        }

        Vec::new()
    })
}

// ============================================================================
// Validator 4: EncapsulationChecker
// ============================================================================

/// Validates that Gprims are not nested (Gprim encapsulation rule).
fn encapsulation_checker_fn() -> ValidatePrimTaskFn {
    Arc::new(|prim: &Prim, _time_range: &ValidationTimeRange| {
        // Only validate Boundable prims
        if !is_boundable(prim) {
            return Vec::new();
        }

        let boundable = Boundable::new(prim.clone());
        if !boundable.is_valid() {
            return Vec::new();
        }

        // Walk ancestors looking for Gprim
        let mut current = prim.parent();
        while current.is_valid() && !current.is_pseudo_root() {
            if is_gprim(&current) {
                // Found a Gprim ancestor - this is an error
                return vec![ValidationError::new(
                    INVALID_NESTED_GPRIMS.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(
                        &prim.stage().unwrap(),
                        prim.get_path().clone(),
                        None,
                    )],
                    format!(
                        "Boundable prim '{}' has Gprim ancestor '{}'. \
                         Gprims cannot be nested under other Gprims.",
                        prim.get_path(),
                        current.get_path()
                    ),
                )];
            }
            current = current.parent();
        }

        Vec::new()
    })
}

// ============================================================================
// Registration
// ============================================================================

/// Register all UsdGeom validators with the global registry.
///
/// Call this function once during application initialization to make
/// the UsdGeom validators available.
pub fn register_geom_validators(registry: &ValidationRegistry) {
    // 1. StageMetadataChecker
    registry.register_stage_validator(
        ValidatorMetadata::new(STAGE_METADATA_CHECKER.clone())
            .with_doc(
                "Validates that the stage has required metadata: \
                 'metersPerUnit' and 'upAxis'."
                    .to_string(),
            )
            .with_keywords(vec![
                Token::new("UsdGeomValidators"),
                Token::new("metadata"),
            ]),
        stage_metadata_checker_fn(),
        Vec::new(),
    );

    // 2. SubsetFamilies
    registry.register_prim_validator(
        ValidatorMetadata::new(SUBSET_FAMILIES.clone())
            .with_doc(
                "Validates UsdGeomSubset family membership for Imageable prims. \
                 Ensures that subset families are valid and non-empty."
                    .to_string(),
            )
            .with_keywords(vec![Token::new("UsdGeomValidators"), Token::new("subset")])
            .with_schema_types(vec![IMAGEABLE_KEY.clone()]),
        subset_families_fn(),
        Vec::new(),
    );

    // 3. SubsetParentIsImageable
    registry.register_prim_validator(
        ValidatorMetadata::new(SUBSET_PARENT_IS_IMAGEABLE.clone())
            .with_doc(
                "Validates that UsdGeomSubset prims have Imageable parents. \
                 Subsets must be children of Imageable prims."
                    .to_string(),
            )
            .with_keywords(vec![Token::new("UsdGeomValidators"), Token::new("subset")])
            .with_schema_types(vec![SUBSET_KEY.clone()]),
        subset_parent_is_imageable_fn(),
        Vec::new(),
    );

    // 4. EncapsulationChecker
    registry.register_prim_validator(
        ValidatorMetadata::new(ENCAPSULATION_CHECKER.clone())
            .with_doc(
                "Validates Gprim encapsulation rules. Ensures that Gprims \
                 are not nested under other Gprims."
                    .to_string(),
            )
            .with_keywords(vec![
                Token::new("UsdGeomValidators"),
                Token::new("encapsulation"),
            ])
            .with_schema_types(vec![BOUNDABLE_KEY.clone()]),
        encapsulation_checker_fn(),
        Vec::new(),
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::Stage;
    use usd_core::common::InitialLoadSet;

    // -- Token constants --

    #[test]
    fn test_validator_tokens() {
        assert_eq!(
            STAGE_METADATA_CHECKER.as_str(),
            "usdGeomValidators:StageMetadataChecker"
        );
        assert_eq!(SUBSET_FAMILIES.as_str(), "usdGeomValidators:SubsetFamilies");
        assert_eq!(
            SUBSET_PARENT_IS_IMAGEABLE.as_str(),
            "usdGeomValidators:SubsetParentIsImageable"
        );
        assert_eq!(
            ENCAPSULATION_CHECKER.as_str(),
            "usdGeomValidators:EncapsulationChecker"
        );
    }

    #[test]
    fn test_error_tokens() {
        assert_eq!(
            MISSING_METERS_PER_UNIT_METADATA.as_str(),
            "MissingMetersPerUnitMetadata"
        );
        assert_eq!(MISSING_UP_AXIS_METADATA.as_str(), "MissingUpAxisMetadata");
        assert_eq!(INVALID_SUBSET_FAMILY.as_str(), "InvalidSubsetFamily");
        assert_eq!(
            NOT_IMAGEABLE_SUBSET_PARENT.as_str(),
            "NotImageableSubsetParent"
        );
        assert_eq!(INVALID_NESTED_GPRIMS.as_str(), "InvalidNestedGprims");
    }

    // -- StageMetadataChecker --

    #[test]
    fn test_stage_metadata_checker_missing_both() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let validator_fn = stage_metadata_checker_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        assert_eq!(errors.len(), 2);
        assert!(
            errors
                .iter()
                .any(|e| e.get_name() == &*MISSING_METERS_PER_UNIT_METADATA)
        );
        assert!(
            errors
                .iter()
                .any(|e| e.get_name() == &*MISSING_UP_AXIS_METADATA)
        );
    }

    #[test]
    fn test_stage_metadata_checker_with_metadata() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Set both metadata
        stage.set_metadata(&METERS_PER_UNIT_KEY, usd_vt::Value::from(0.01f64));
        stage.set_metadata(&UP_AXIS_KEY, usd_vt::Value::from("Y"));

        let validator_fn = stage_metadata_checker_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        assert!(errors.is_empty());
    }

    #[test]
    fn test_stage_metadata_checker_missing_upaxis() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.set_metadata(&METERS_PER_UNIT_KEY, usd_vt::Value::from(1.0f64));

        let validator_fn = stage_metadata_checker_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].get_name(), &*MISSING_UP_AXIS_METADATA);
    }

    // -- SubsetParentIsImageable --

    #[test]
    fn test_subset_parent_is_imageable_valid() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create Mesh (Imageable) and Subset
        let _mesh = stage.define_prim("/Mesh", "Mesh").unwrap();
        let subset = stage.define_prim("/Mesh/Subset1", "GeomSubset").unwrap();

        let validator_fn = subset_parent_is_imageable_fn();
        let errors = validator_fn(&subset, &ValidationTimeRange::default());

        assert!(errors.is_empty());
    }

    #[test]
    fn test_subset_parent_is_imageable_invalid() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create an untyped prim (not Imageable) as parent of a GeomSubset
        let _parent = stage.define_prim("/Parent", "").unwrap();
        let subset = stage.define_prim("/Parent/Subset1", "GeomSubset").unwrap();

        let validator_fn = subset_parent_is_imageable_fn();
        let errors = validator_fn(&subset, &ValidationTimeRange::default());

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].get_name(), &*NOT_IMAGEABLE_SUBSET_PARENT);
        assert!(errors[0].get_message().contains("not an Imageable"));
    }

    #[test]
    fn test_subset_parent_is_imageable_non_subset_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mesh = stage.define_prim("/Mesh", "Mesh").unwrap();

        let validator_fn = subset_parent_is_imageable_fn();
        let errors = validator_fn(&mesh, &ValidationTimeRange::default());

        // Should not validate non-Subset prims
        assert!(errors.is_empty());
    }

    // -- EncapsulationChecker --

    #[test]
    fn test_encapsulation_checker_valid() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create Xform -> Mesh (valid hierarchy)
        let _xform = stage.define_prim("/Xform", "Xform").unwrap();
        let mesh = stage.define_prim("/Xform/Mesh", "Mesh").unwrap();

        let validator_fn = encapsulation_checker_fn();
        let errors = validator_fn(&mesh, &ValidationTimeRange::default());

        assert!(errors.is_empty());
    }

    #[test]
    fn test_encapsulation_checker_nested_gprims() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        // Create Mesh -> Cube (invalid: nested Gprims)
        let _mesh = stage.define_prim("/Mesh", "Mesh").unwrap();
        let cube = stage.define_prim("/Mesh/Cube", "Cube").unwrap();

        let validator_fn = encapsulation_checker_fn();
        let errors = validator_fn(&cube, &ValidationTimeRange::default());

        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].get_name(), &*INVALID_NESTED_GPRIMS);
        assert!(errors[0].get_message().contains("Gprim ancestor"));
    }

    #[test]
    fn test_encapsulation_checker_non_boundable_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let scope = stage.define_prim("/Scope", "Scope").unwrap();

        let validator_fn = encapsulation_checker_fn();
        let errors = validator_fn(&scope, &ValidationTimeRange::default());

        // Should not validate non-Boundable prims
        assert!(errors.is_empty());
    }

    // -- Integration with registry --

    #[test]
    fn test_register_geom_validators() {
        let registry = ValidationRegistry::get_instance();
        register_geom_validators(registry);

        assert!(registry.has_validator(&STAGE_METADATA_CHECKER));
        assert!(registry.has_validator(&SUBSET_FAMILIES));
        assert!(registry.has_validator(&SUBSET_PARENT_IS_IMAGEABLE));
        assert!(registry.has_validator(&ENCAPSULATION_CHECKER));
    }

    #[test]
    fn test_validators_have_keywords() {
        let registry = ValidationRegistry::get_instance();
        register_geom_validators(registry);

        let metadata = registry
            .get_validator_metadata(&STAGE_METADATA_CHECKER)
            .unwrap();
        assert!(metadata.keywords.iter().any(|k| k == "UsdGeomValidators"));
    }
}
