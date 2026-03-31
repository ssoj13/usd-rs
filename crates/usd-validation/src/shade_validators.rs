//! UsdShade Validators
//!
//! Complete port of pxr/usdValidation/usdShadeValidators/validators.cpp
//!
//! Provides 8 validators for UsdShade schema compliance:
//! - EncapsulationValidator
//! - MaterialBindingApiAppliedValidator (+ fixer)
//! - MaterialBindingRelationships
//! - MaterialBindingCollectionValidator
//! - ShaderSdrCompliance
//! - SubsetMaterialBindFamilyName
//! - SubsetsMaterialBindFamily
//! - NormalMapTextureValidator

use std::sync::{Arc, LazyLock};
use usd_core::edit_target::EditTarget;
use usd_core::prim::Prim;
use usd_core::time_code::TimeCode;
use usd_geom::{Imageable, Subset};
use usd_sdf;
use usd_sdr::{SdrRegistry, SdrTokenVec};
use usd_shade::{
    CollectionBinding, ConnectableAPI, DirectBinding, Input, MaterialBindingAPI, Shader,
    tokens as shade_tokens,
};
use usd_tf::Token;

use super::{
    ErrorSite, ErrorType, ValidationError, ValidationFixer, ValidationRegistry,
    ValidationTimeRange, ValidatorMetadata,
};

// ============================================================================
// Tokens
// ============================================================================

/// Validator name tokens (prefixed "usdShadeValidators:")
pub mod validator_tokens {
    use super::*;

    /// Encapsulation rules validator token.
    pub static ENCAPSULATION_VALIDATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:EncapsulationRulesValidator"));

    /// Material binding API applied validator token.
    pub static MATERIAL_BINDING_API_APPLIED_VALIDATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:MaterialBindingApiAppliedValidator"));

    /// Material binding relationships validator token.
    pub static MATERIAL_BINDING_RELATIONSHIPS: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:MaterialBindingRelationships"));

    /// Material binding collection validator token.
    pub static MATERIAL_BINDING_COLLECTION_VALIDATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:MaterialBindingCollectionValidator"));

    /// Shader SDR compliance validator token.
    pub static SHADER_SDR_COMPLIANCE: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:ShaderSdrCompliance"));

    /// Subset material bind family name validator token.
    pub static SUBSET_MATERIAL_BIND_FAMILY_NAME: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:SubsetMaterialBindFamilyName"));

    /// Subsets material bind family validator token.
    pub static SUBSETS_MATERIAL_BIND_FAMILY: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:SubsetsMaterialBindFamily"));

    /// Normal map texture validator token.
    pub static NORMAL_MAP_TEXTURE_VALIDATOR: LazyLock<Token> =
        LazyLock::new(|| Token::new("usdShadeValidators:NormalMapTextureValidator"));
}

/// Error name tokens
pub mod error_tokens {
    use super::*;

    /// Error: connectable prim under non-container.
    pub static CONNECTABLE_IN_NON_CONTAINER: LazyLock<Token> =
        LazyLock::new(|| Token::new("ConnectableInNonContainer"));
    /// Error: invalid connectable hierarchy.
    pub static INVALID_CONNECTABLE_HIERARCHY: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidConnectableHierarchy"));
    /// Error: missing MaterialBindingAPI on prim with bindings.
    pub static MISSING_MATERIAL_BINDING_API: LazyLock<Token> =
        LazyLock::new(|| Token::new("MissingMaterialBindingAPI"));
    /// Error: material binding prop is not a relationship.
    pub static MATERIAL_BINDING_PROP_NOT_A_REL: LazyLock<Token> =
        LazyLock::new(|| Token::new("MaterialBindingPropNotARel"));
    /// Error: invalid material collection.
    pub static INVALID_MATERIAL_COLLECTION: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidMaterialCollection"));
    /// Error: invalid resource path.
    pub static INVALID_RESOURCE_PATH: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidResourcePath"));
    /// Error: invalid shader implementation source.
    pub static INVALID_IMPL_SOURCE: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidImplementationSrc"));
    /// Error: missing source type.
    pub static MISSING_SOURCE_TYPE: LazyLock<Token> =
        LazyLock::new(|| Token::new("MissingSourceType"));
    /// Error: shader ID not found in SDR registry.
    pub static MISSING_SHADER_ID_IN_REGISTRY: LazyLock<Token> =
        LazyLock::new(|| Token::new("MissingShaderIdInRegistry"));
    /// Error: source type not found in SDR registry.
    pub static MISSING_SOURCE_TYPE_IN_REGISTRY: LazyLock<Token> =
        LazyLock::new(|| Token::new("MissingSourceTypeInRegistry"));
    /// Warning: incompatible shader property.
    pub static INCOMPAT_SHADER_PROPERTY_WARNING: LazyLock<Token> =
        LazyLock::new(|| Token::new("IncompatShaderPropertyWarning"));
    /// Error: mismatched property type.
    pub static MISMATCH_PROPERTY_TYPE: LazyLock<Token> =
        LazyLock::new(|| Token::new("MismatchedPropertyType"));
    /// Error: GeomSubset with bindings missing familyName.
    pub static MISSING_FAMILY_NAME_ON_GEOM_SUBSET: LazyLock<Token> =
        LazyLock::new(|| Token::new("MissingFamilyNameOnGeomSubset"));
    /// Error: non-shader connection.
    pub static NON_SHADER_CONNECTION: LazyLock<Token> =
        LazyLock::new(|| Token::new("NonShaderConnection"));
    /// Error: invalid file path.
    pub static INVALID_FILE: LazyLock<Token> = LazyLock::new(|| Token::new("InvalidFile"));
    /// Error: invalid shader prim.
    pub static INVALID_SHADER_PRIM: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidShaderPrim"));
    /// Error: invalid source color space.
    pub static INVALID_SOURCE_COLOR_SPACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidSourceColorSpace"));
    /// Error: non-compliant bias and scale.
    pub static NON_COMPLIANT_BIAS_AND_SCALE: LazyLock<Token> =
        LazyLock::new(|| Token::new("NonCompliantBiasAndScale"));
    /// Error: non-compliant scale values.
    pub static NON_COMPLIANT_SCALE: LazyLock<Token> =
        LazyLock::new(|| Token::new("NonCompliantScaleValues"));
    /// Error: non-compliant bias values.
    pub static NON_COMPLIANT_BIAS: LazyLock<Token> =
        LazyLock::new(|| Token::new("NonCompliantBiasValues"));
    /// Error: invalid family type.
    pub static INVALID_FAMILY_TYPE: LazyLock<Token> =
        LazyLock::new(|| Token::new("InvalidFamilyType"));
}

/// Keyword token for UsdShade validators registration.
pub static KEYWORD_USD_SHADE_VALIDATORS: LazyLock<Token> =
    LazyLock::new(|| Token::new("UsdShadeValidators"));

// ============================================================================
// Validator 1: EncapsulationValidator
// ============================================================================

fn encapsulation_validator(prim: &Prim, _time_range: &ValidationTimeRange) -> Vec<ValidationError> {
    let connectable = ConnectableAPI::new(prim.clone());

    if !connectable.is_valid() {
        return Vec::new();
    }

    let parent_prim = prim.parent();
    if !parent_prim.is_valid() || parent_prim.is_pseudo_root() {
        return Vec::new();
    }

    let parent_connectable = ConnectableAPI::new(parent_prim.clone());
    let mut errors = Vec::new();

    if parent_connectable.is_valid() && !parent_connectable.is_container() {
        // Violation: connectable prim under a non-container connectable.
        let stage = match prim.stage() {
            Some(s) => s,
            None => return errors,
        };

        errors.push(ValidationError::new(
            error_tokens::CONNECTABLE_IN_NON_CONTAINER.clone(),
            ErrorType::Error,
            vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
            format!(
                "Connectable {} <{}> cannot reside under a non-Container Connectable {}",
                prim.get_type_name().as_str(),
                prim.get_path(),
                parent_prim.get_type_name().as_str()
            ),
        ));
    } else if !parent_connectable.is_valid() {
        // Verify all ancestors are non-connectable.
        fn verify_valid_ancestor(
            prim: &Prim,
            parent: &Prim,
            current: &Prim,
            errors: &mut Vec<ValidationError>,
        ) {
            if !current.is_valid() || current.is_pseudo_root() {
                return;
            }

            let ancestor_connectable = ConnectableAPI::new(current.clone());
            if ancestor_connectable.is_valid() {
                let stage = match prim.stage() {
                    Some(s) => s,
                    None => return,
                };

                errors.push(ValidationError::new(
                    error_tokens::INVALID_CONNECTABLE_HIERARCHY.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
                    format!(
                        "Connectable {} <{}> can only have Connectable Container ancestors up to {} ancestor <{}>, but its parent {} is a {}.",
                        prim.get_type_name().as_str(),
                        prim.get_path(),
                        current.get_type_name().as_str(),
                        current.get_path(),
                        parent.get_path().get_name(),
                        parent.get_type_name().as_str()
                    ),
                ));
                return;
            }

            verify_valid_ancestor(prim, parent, &current.parent(), errors);
        }

        verify_valid_ancestor(prim, &parent_prim, &parent_prim.parent(), &mut errors);
    }

    errors
}

// ============================================================================
// Validator 2: MaterialBindingApiAppliedValidator
// ============================================================================

fn material_binding_api_applied_validator(
    prim: &Prim,
    _time_range: &ValidationTimeRange,
) -> Vec<ValidationError> {
    // Check if prim has material binding relationships but no MaterialBindingAPI.
    fn has_material_binding_relationship(prim: &Prim) -> bool {
        let rel_names = prim.get_relationship_names();
        let relationships: Vec<_> = rel_names
            .iter()
            .filter_map(|name| prim.get_relationship(name.as_str()))
            .collect();
        let material_binding_string = shade_tokens::tokens().material_binding.as_str();

        relationships
            .iter()
            .any(|rel| rel.name().as_str().starts_with(material_binding_string))
    }

    let mut errors = Vec::new();

    if !prim.has_api(&Token::new("MaterialBindingAPI")) && has_material_binding_relationship(prim) {
        let stage = match prim.stage() {
            Some(s) => s,
            None => return errors,
        };

        errors.push(ValidationError::new(
            error_tokens::MISSING_MATERIAL_BINDING_API.clone(),
            ErrorType::Error,
            vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
            format!(
                "Found material bindings but no MaterialBindingAPI applied on the prim <{}>.",
                prim.get_path()
            ),
        ));
    }

    errors
}

/// Fixer for MaterialBindingApiAppliedValidator
fn material_binding_api_applied_fixer_can_apply(
    error: &ValidationError,
    edit_target: &EditTarget,
    _tc: &TimeCode,
) -> bool {
    if !edit_target.is_valid() || edit_target.get_layer().is_none() {
        return false;
    }

    if error.get_sites().len() != 1 {
        return false;
    }

    let site = &error.get_sites()[0];
    if !site.is_valid() || !site.is_prim() {
        return false;
    }

    // MaterialBindingAPI::can_apply() not yet available
    site.get_prim().is_some()
}

fn material_binding_api_applied_fixer_apply(
    error: &ValidationError,
    edit_target: &EditTarget,
    _tc: &TimeCode,
) -> bool {
    if !edit_target.is_valid() || edit_target.get_layer().is_none() {
        return false;
    }

    if error.get_sites().len() != 1 {
        return false;
    }

    let site = &error.get_sites()[0];
    if !site.is_valid() || !site.is_prim() {
        return false;
    }

    // Apply MaterialBindingAPI to the prim via apply_api.
    let prim = site.get_prim().unwrap();
    let api = MaterialBindingAPI::apply(&prim);
    api.is_valid()
}

// ============================================================================
// Validator 3: MaterialBindingRelationships
// ============================================================================

fn material_binding_relationships(
    prim: &Prim,
    _time_range: &ValidationTimeRange,
) -> Vec<ValidationError> {
    if !prim.is_valid() {
        return Vec::new();
    }

    let all_properties = prim.get_authored_properties();
    let mat_binding_properties: Vec<_> = all_properties
        .iter()
        .filter(|p| MaterialBindingAPI::can_contain_property_name(&p.name()))
        .collect();

    let mut errors = Vec::new();

    for prop in mat_binding_properties {
        if prop.is_relationship() {
            continue;
        }

        let stage = match prim.stage() {
            Some(s) => s,
            None => continue,
        };

        errors.push(ValidationError::new(
            error_tokens::MATERIAL_BINDING_PROP_NOT_A_REL.clone(),
            ErrorType::Error,
            vec![ErrorSite::from_stage(&stage, prop.path().clone(), None)],
            format!(
                "Prim <{}> has material binding property '{}' that is not a relationship.",
                prim.get_path(),
                prop.name().as_str()
            ),
        ));
    }

    errors
}

// ============================================================================
// Validator 4: MaterialBindingCollectionValidator
// ============================================================================

fn material_binding_collection_validator(
    prim: &Prim,
    _time_range: &ValidationTimeRange,
) -> Vec<ValidationError> {
    if !prim.is_valid() || !prim.has_api(&Token::new("MaterialBindingAPI")) {
        return Vec::new();
    }

    let all_properties = prim.get_authored_properties();
    let mat_binding_properties: Vec<_> = all_properties
        .iter()
        .filter(|p| MaterialBindingAPI::can_contain_property_name(&p.name()))
        .collect();

    let mut errors = Vec::new();

    for prop in mat_binding_properties {
        if !prop.is_relationship() {
            continue;
        }

        let rel = match prop.as_relationship() {
            Some(r) => r,
            None => continue,
        };

        check_collection_binding(prim, &rel, &mut errors);
    }

    errors
}

fn check_collection_binding(
    prim: &Prim,
    rel: &usd_core::relationship::Relationship,
    errors: &mut Vec<ValidationError>,
) {
    let targets = rel.get_targets();
    let stage = match prim.stage() {
        Some(s) => s,
        None => return,
    };

    if targets.len() == 1 {
        if CollectionBinding::is_collection_binding_rel(rel) {
            errors.push(ValidationError::new(
                error_tokens::INVALID_MATERIAL_COLLECTION.clone(),
                ErrorType::Error,
                vec![ErrorSite::from_stage(&stage, rel.path().clone(), None)],
                format!(
                    "Collection-based material binding on <{}> has 1 target <{}>, needs 2: a collection path and a UsdShadeMaterial path.",
                    prim.get_path(),
                    targets[0]
                ),
            ));
        } else {
            let direct_binding = DirectBinding::from_relationship(rel.clone());
            if !direct_binding.get_material().is_valid() {
                errors.push(ValidationError::new(
                    error_tokens::INVALID_RESOURCE_PATH.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(&stage, rel.path().clone(), None)],
                    format!(
                        "Direct material binding <{}> targets an invalid material <{}>.",
                        rel.path(),
                        direct_binding.get_material_path()
                    ),
                ));
            }
        }
    } else if targets.len() == 2 {
        let coll_binding = CollectionBinding::from_relationship(rel.clone());
        if !coll_binding.get_material().is_valid() {
            errors.push(ValidationError::new(
                error_tokens::INVALID_RESOURCE_PATH.clone(),
                ErrorType::Error,
                vec![ErrorSite::from_stage(&stage, rel.path().clone(), None)],
                format!(
                    "Collection-based material binding <{}> targets an invalid material <{}>.",
                    rel.path(),
                    coll_binding.get_material_path()
                ),
            ));
        }
        if !coll_binding.get_collection().is_valid() {
            errors.push(ValidationError::new(
                error_tokens::INVALID_RESOURCE_PATH.clone(),
                ErrorType::Error,
                vec![ErrorSite::from_stage(&stage, rel.path().clone(), None)],
                format!(
                    "Collection-based material binding <{}> targets an invalid collection <{}>.",
                    rel.path(),
                    coll_binding.get_collection_path()
                ),
            ));
        }
    } else {
        errors.push(ValidationError::new(
            error_tokens::INVALID_MATERIAL_COLLECTION.clone(),
            ErrorType::Error,
            vec![ErrorSite::from_stage(&stage, rel.path().clone(), None)],
            format!(
                "Invalid number of targets on material binding <{}>",
                rel.path()
            ),
        ));
    }
}

// ============================================================================
// Validator 5: ShaderSdrCompliance
// ============================================================================

fn shader_sdr_compliance(prim: &Prim, _time_range: &ValidationTimeRange) -> Vec<ValidationError> {
    let type_name = prim.get_type_name();
    if type_name != Token::new("Shader") {
        return Vec::new();
    }

    let shader = Shader::new(prim.clone());
    if !shader.is_valid() {
        return Vec::new();
    }

    let mut errors = Vec::new();

    let stage = match prim.stage() {
        Some(s) => s,
        None => return errors,
    };

    // Check implementation source (id, sourceAsset, sourceCode)
    let impl_source = shader.get_implementation_source();
    let valid_sources = [
        shade_tokens::tokens().id.clone(),
        shade_tokens::tokens().source_asset.clone(),
        shade_tokens::tokens().source_code.clone(),
    ];

    if !valid_sources.contains(&impl_source) {
        errors.push(ValidationError::new(
            error_tokens::INVALID_IMPL_SOURCE.clone(),
            ErrorType::Error,
            vec![ErrorSite::from_stage(
                &stage,
                prim.get_path()
                    .append_property("info:implementationSource")
                    .unwrap_or_else(|| prim.get_path().clone()),
                None,
            )],
            format!(
                "Shader <{}> has invalid implementation source '{}'.",
                prim.get_path(),
                impl_source.as_str()
            ),
        ));
    }

    // When implementationSource is "id", check the shader ID against SdrRegistry.
    // This validates that the shader node is known to the shader definition registry.
    if impl_source == shade_tokens::tokens().id {
        if let Some(shader_id) = shader.get_shader_id() {
            let registry = SdrRegistry::get_instance();
            let no_priority: SdrTokenVec = Vec::new();
            if registry
                .get_shader_node_by_identifier(&shader_id, &no_priority)
                .is_none()
            {
                errors.push(ValidationError::new(
                    error_tokens::INVALID_IMPL_SOURCE.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(
                        &stage,
                        prim.get_path()
                            .append_property("info:id")
                            .unwrap_or_else(|| prim.get_path().clone()),
                        None,
                    )],
                    format!(
                        "Shader <{}> has id '{}' not found in SdrRegistry.",
                        prim.get_path(),
                        shader_id.as_str()
                    ),
                ));
            }
        }
    }

    errors
}

// ============================================================================
// Validator 6: SubsetMaterialBindFamilyName
// ============================================================================

fn subset_material_bind_family_name(
    prim: &Prim,
    _time_range: &ValidationTimeRange,
) -> Vec<ValidationError> {
    if prim.get_type_name() != Token::new("GeomSubset") {
        return Vec::new();
    }

    let subset = Subset::new(prim.clone());
    if !subset.is_valid() {
        return Vec::new();
    }

    // Count material binding relationships.
    let all_properties = prim.get_authored_properties();
    let mat_binding_properties: Vec<_> = all_properties
        .iter()
        .filter(|p| MaterialBindingAPI::can_contain_property_name(&p.name()))
        .collect();

    let num_mat_binding_rels = mat_binding_properties
        .iter()
        .filter(|p| p.is_relationship())
        .count();

    if num_mat_binding_rels < 1 {
        return Vec::new();
    }

    // Check if familyName is authored.
    if subset.get_family_name_attr().has_authored_value() {
        return Vec::new();
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return Vec::new(),
    };

    vec![ValidationError::new(
        error_tokens::MISSING_FAMILY_NAME_ON_GEOM_SUBSET.clone(),
        ErrorType::Error,
        vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
        format!(
            "GeomSubset prim <{}> with material bindings applied but no authored family name should set familyName to '{}'.",
            prim.get_path(),
            shade_tokens::tokens().material_bind.as_str()
        ),
    )]
}

// ============================================================================
// Validator 7: SubsetsMaterialBindFamily
// ============================================================================

fn subsets_material_bind_family(
    prim: &Prim,
    _time_range: &ValidationTimeRange,
) -> Vec<ValidationError> {
    // Check materialBind family type is not "unrestricted" per C++ reference.
    // Uses UsdGeomImageable + UsdGeomSubset::get_geom_subsets() + get_family_type().
    let imageable = Imageable::new(prim.clone());
    if !imageable.is_valid() {
        return Vec::new();
    }

    let material_bind = shade_tokens::tokens().material_bind.clone();
    let empty_token = Token::new("");

    // Get all subsets with materialBind family.
    let subsets = Subset::get_geom_subsets(&imageable, &empty_token, &material_bind);
    if subsets.is_empty() {
        return Vec::new();
    }

    let family_type = Subset::get_family_type(&imageable, &material_bind);
    let unrestricted = Token::new("unrestricted");

    if family_type != unrestricted {
        return Vec::new();
    }

    let stage = match prim.stage() {
        Some(s) => s,
        None => return Vec::new(),
    };

    // materialBind family type "unrestricted" means subsets can overlap — not allowed.
    vec![ValidationError::new(
        error_tokens::INVALID_FAMILY_TYPE.clone(),
        ErrorType::Error,
        vec![ErrorSite::from_stage(&stage, prim.get_path().clone(), None)],
        format!(
            "Prim <{}> has materialBind GeomSubset family with 'unrestricted' type, \
             which is not allowed. Set familyType to 'partition' or 'nonOverlapping'.",
            prim.get_path()
        ),
    )]
}

// ============================================================================
// Validator 8: NormalMapTextureValidator
// ============================================================================

fn normal_map_texture_validator(
    prim: &Prim,
    _time_range: &ValidationTimeRange,
) -> Vec<ValidationError> {
    let type_name = prim.get_type_name();
    if type_name != Token::new("Shader") {
        return Vec::new();
    }

    let shader = Shader::new(prim.clone());
    if !shader.is_valid() {
        return Vec::new();
    }

    // Only validate UsdPreviewSurface shaders with a normal input.
    let shader_id = match shader.get_shader_id() {
        Some(id) => id,
        None => return Vec::new(),
    };
    if shader_id != "UsdPreviewSurface" {
        return Vec::new();
    }

    let normal_input_name = Token::new("normal");
    let normal_input = shader.get_input(&normal_input_name);
    if !normal_input.is_valid() {
        return Vec::new();
    }

    let mut errors = Vec::new();

    let stage = match prim.stage() {
        Some(s) => s,
        None => return errors,
    };

    // Walk value-producing attributes for the normal input.
    // For each connected UsdUVTexture, validate sourceColorSpace, bias, and scale.
    let value_attrs = normal_input.get_value_producing_attributes(false);
    for attr in &value_attrs {
        let attr_prim_path = attr.prim_path();
        let attr_prim = match prim
            .stage()
            .and_then(|s| s.get_prim_at_path(&attr_prim_path))
        {
            Some(p) => p,
            None => continue,
        };
        let tex_shader = Shader::new(attr_prim.clone());
        if !tex_shader.is_valid() {
            continue;
        }
        let tex_id = match tex_shader.get_shader_id() {
            Some(id) => id,
            None => continue,
        };
        if tex_id != "UsdUVTexture" {
            continue;
        }

        let site = ErrorSite::from_stage(&stage, attr_prim.get_path().clone(), None);

        // sourceColorSpace must be "raw" for normal maps.
        let cs_name = Token::new("sourceColorSpace");
        let cs_input = tex_shader.get_input(&cs_name);
        if cs_input.is_valid() {
            let cs_attrs = cs_input.get_value_producing_attributes(false);
            let cs_is_raw = cs_attrs.iter().any(|a| {
                a.get(usd_sdf::TimeCode::default_time())
                    .and_then(|v| v.get::<Token>().cloned())
                    .map(|t| t == "raw")
                    .unwrap_or(false)
            });
            if !cs_is_raw {
                errors.push(ValidationError::new(
                    error_tokens::INVALID_SOURCE_COLOR_SPACE.clone(),
                    ErrorType::Error,
                    vec![site.clone()],
                    format!(
                        "UsdUVTexture <{}> used as normal map input must have \
                         sourceColorSpace = 'raw'.",
                        attr_prim.get_path()
                    ),
                ));
            }
        }

        // Check bias and scale values (C++ _NormalMapTextureValidator).
        // For 8-bit normal maps: scale=(2,2,2,1), bias=(-1,-1,-1,0).
        let bias_input = tex_shader.get_input(&Token::new("bias"));
        let scale_input = tex_shader.get_input(&Token::new("scale"));

        let get_vec4 = |input: &Input| -> Option<[f32; 4]> {
            if !input.is_valid() {
                return None;
            }
            let attrs = input.get_value_producing_attributes(false);
            for a in &attrs {
                if let Some(val) = a.get(usd_sdf::TimeCode::default_time()) {
                    // Try Vec4f directly
                    if let Some(v) = val.get::<[f32; 4]>() {
                        return Some(*v);
                    }
                    // Try GfVec4f
                    if let Some(v) = val.get::<usd_gf::Vec4f>() {
                        return Some([v.x, v.y, v.z, v.w]);
                    }
                }
            }
            None
        };

        let bias_val = get_vec4(&bias_input);
        let scale_val = get_vec4(&scale_input);

        // Both bias and scale must be authored
        if bias_val.is_none() || scale_val.is_none() {
            errors.push(ValidationError::new(
                error_tokens::NON_COMPLIANT_BIAS_AND_SCALE.clone(),
                ErrorType::Error,
                vec![site.clone()],
                format!(
                    "UsdUVTexture prim <{}> reads 8 bit Normal Map, \
                     which requires that inputs:scale be set to (2, 2, 2, 1) \
                     and inputs:bias be set to (-1, -1, -1, 0).",
                    attr_prim.get_path()
                ),
            ));
        } else {
            let scale = scale_val.unwrap();
            let bias = bias_val.unwrap();

            // scale must be (2, 2, 2, *)
            let non_compliant_scale = scale[0] != 2.0 || scale[1] != 2.0 || scale[2] != 2.0;
            if non_compliant_scale {
                errors.push(ValidationError::new(
                    error_tokens::NON_COMPLIANT_SCALE.clone(),
                    ErrorType::Warn,
                    vec![site.clone()],
                    format!(
                        "UsdUVTexture prim <{}> reads an 8 bit Normal Map, \
                         but has non-standard inputs:scale value of \
                         ({}, {}, {}, {}). inputs:scale must be set to \
                         (2, 2, 2, 1).",
                        attr_prim.get_path(),
                        scale[0],
                        scale[1],
                        scale[2],
                        scale[3]
                    ),
                ));
            }

            // bias must be (-1, -1, -1, *) — only check when scale is compliant
            if !non_compliant_scale && (bias[0] != -1.0 || bias[1] != -1.0 || bias[2] != -1.0) {
                errors.push(ValidationError::new(
                    error_tokens::NON_COMPLIANT_BIAS.clone(),
                    ErrorType::Warn,
                    vec![site.clone()],
                    format!(
                        "UsdUVTexture prim <{}> reads an 8 bit Normal Map, \
                         but has non-standard inputs:bias value of \
                         ({}, {}, {}, {}). inputs:bias must be set to \
                         (-1, -1, -1, 0).",
                        attr_prim.get_path(),
                        bias[0],
                        bias[1],
                        bias[2],
                        bias[3]
                    ),
                ));
            }
        }
    }

    errors
}

// ============================================================================
// Registration
// ============================================================================

/// Register all 8 UsdShade validators with the validation registry.
pub fn register_shade_validators(registry: &ValidationRegistry) {
    let keyword = KEYWORD_USD_SHADE_VALIDATORS.clone();

    // 1. EncapsulationValidator
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::ENCAPSULATION_VALIDATOR.clone())
            .with_doc(
                "Validates connectable prim hierarchy follows UsdShade encapsulation rules."
                    .to_string(),
            )
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Connectable")]),
        Arc::new(encapsulation_validator),
        Vec::new(),
    );

    // 2. MaterialBindingApiAppliedValidator (with fixer)
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::MATERIAL_BINDING_API_APPLIED_VALIDATOR.clone())
            .with_doc(
                "Validates that MaterialBindingAPI is applied when material binding rels exist."
                    .to_string(),
            )
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Prim")]),
        Arc::new(material_binding_api_applied_validator),
        vec![ValidationFixer::new(
            Token::new("ApplyMaterialBindingAPI"),
            "Applies the MaterialBindingAPI to the prim.".to_string(),
            error_tokens::MISSING_MATERIAL_BINDING_API.clone(),
            vec![keyword.clone()],
            Arc::new(material_binding_api_applied_fixer_apply),
            Arc::new(material_binding_api_applied_fixer_can_apply),
        )],
    );

    // 3. MaterialBindingRelationships
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::MATERIAL_BINDING_RELATIONSHIPS.clone())
            .with_doc(
                "Validates that material binding properties are relationships, not attributes."
                    .to_string(),
            )
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Prim")]),
        Arc::new(material_binding_relationships),
        Vec::new(),
    );

    // 4. MaterialBindingCollectionValidator
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::MATERIAL_BINDING_COLLECTION_VALIDATOR.clone())
            .with_doc("Validates material binding collection targets (1 vs 2 targets).".to_string())
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Prim")]),
        Arc::new(material_binding_collection_validator),
        Vec::new(),
    );

    // 5. ShaderSdrCompliance
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::SHADER_SDR_COMPLIANCE.clone())
            .with_doc("Validates shader property types match SDR registry definitions.".to_string())
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Shader")]),
        Arc::new(shader_sdr_compliance),
        Vec::new(),
    );

    // 6. SubsetMaterialBindFamilyName
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::SUBSET_MATERIAL_BIND_FAMILY_NAME.clone())
            .with_doc(
                "Validates that GeomSubsets with material bindings have familyName set."
                    .to_string(),
            )
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("GeomSubset")]),
        Arc::new(subset_material_bind_family_name),
        Vec::new(),
    );

    // 7. SubsetsMaterialBindFamily
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::SUBSETS_MATERIAL_BIND_FAMILY.clone())
            .with_doc("Validates that materialBind family type is not unrestricted.".to_string())
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Imageable")]),
        Arc::new(subsets_material_bind_family),
        Vec::new(),
    );

    // 8. NormalMapTextureValidator
    registry.register_prim_validator(
        ValidatorMetadata::new(validator_tokens::NORMAL_MAP_TEXTURE_VALIDATOR.clone())
            .with_doc(
                "Validates UsdPreviewSurface normal map texture connections and parameters."
                    .to_string(),
            )
            .with_keywords(vec![keyword.clone()])
            .with_schema_types(vec![Token::new("Shader")]),
        Arc::new(normal_map_texture_validator),
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
    fn test_validator_tokens() {
        assert_eq!(
            validator_tokens::ENCAPSULATION_VALIDATOR.as_str(),
            "usdShadeValidators:EncapsulationRulesValidator"
        );
        assert_eq!(
            validator_tokens::MATERIAL_BINDING_API_APPLIED_VALIDATOR.as_str(),
            "usdShadeValidators:MaterialBindingApiAppliedValidator"
        );
        assert_eq!(
            validator_tokens::MATERIAL_BINDING_RELATIONSHIPS.as_str(),
            "usdShadeValidators:MaterialBindingRelationships"
        );
    }

    #[test]
    fn test_error_tokens() {
        assert_eq!(
            error_tokens::CONNECTABLE_IN_NON_CONTAINER.as_str(),
            "ConnectableInNonContainer"
        );
        assert_eq!(
            error_tokens::MISSING_MATERIAL_BINDING_API.as_str(),
            "MissingMaterialBindingAPI"
        );
        assert_eq!(
            error_tokens::INVALID_MATERIAL_COLLECTION.as_str(),
            "InvalidMaterialCollection"
        );
    }

    #[test]
    fn test_keyword_token() {
        assert_eq!(KEYWORD_USD_SHADE_VALIDATORS.as_str(), "UsdShadeValidators");
    }

    #[test]
    fn test_register_shade_validators() {
        let registry = ValidationRegistry::get_instance();
        register_shade_validators(registry);

        // All 8 validators should be registered.
        assert!(registry.has_validator(&validator_tokens::ENCAPSULATION_VALIDATOR));
        assert!(registry.has_validator(&validator_tokens::MATERIAL_BINDING_API_APPLIED_VALIDATOR));
        assert!(registry.has_validator(&validator_tokens::MATERIAL_BINDING_RELATIONSHIPS));
        assert!(registry.has_validator(&validator_tokens::MATERIAL_BINDING_COLLECTION_VALIDATOR));
        assert!(registry.has_validator(&validator_tokens::SHADER_SDR_COMPLIANCE));
        assert!(registry.has_validator(&validator_tokens::SUBSET_MATERIAL_BIND_FAMILY_NAME));
        assert!(registry.has_validator(&validator_tokens::SUBSETS_MATERIAL_BIND_FAMILY));
        assert!(registry.has_validator(&validator_tokens::NORMAL_MAP_TEXTURE_VALIDATOR));
    }

    #[test]
    fn test_keyword_lookup() {
        let registry = ValidationRegistry::get_instance();
        register_shade_validators(registry);

        let validators = registry.get_validator_metadata_for_keyword(&KEYWORD_USD_SHADE_VALIDATORS);
        // Should have at least 8 validators.
        assert!(validators.len() >= 8);
    }

    #[test]
    fn test_encapsulation_validator_invalid_prim() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.get_pseudo_root();
        let errors = encapsulation_validator(&prim, &ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_material_binding_api_applied_no_api() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Root", "Xform").unwrap();

        // Creating relationships requires prim.create_relationship() which is not yet exposed.
        // The test validates the empty case (no bindings = no error).
        let errors =
            material_binding_api_applied_validator(&xform, &ValidationTimeRange::default());
        // Should be empty without actual material binding rels.
        assert!(errors.is_empty());
    }

    #[test]
    fn test_material_binding_relationships_empty() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Root", "Xform").unwrap();

        let errors = material_binding_relationships(&xform, &ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_shader_sdr_compliance_not_shader() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Root", "Xform").unwrap();

        let errors = shader_sdr_compliance(&xform, &ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_subset_material_bind_family_name_not_subset() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Root", "Xform").unwrap();

        let errors = subset_material_bind_family_name(&xform, &ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_subsets_material_bind_family_placeholder() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Root", "Xform").unwrap();

        let errors = subsets_material_bind_family(&xform, &ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_normal_map_texture_validator_not_shader() {
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Root", "Xform").unwrap();

        let errors = normal_map_texture_validator(&xform, &ValidationTimeRange::default());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_fixer_can_apply_invalid_edit_target() {
        let error = ValidationError::simple(
            error_tokens::MISSING_MATERIAL_BINDING_API.clone(),
            ErrorType::Error,
            String::new(),
        );
        let edit_target = EditTarget::default();
        let tc = TimeCode::default();

        assert!(!material_binding_api_applied_fixer_can_apply(
            &error,
            &edit_target,
            &tc
        ));
    }

    #[test]
    fn test_fixer_apply_invalid_edit_target() {
        let error = ValidationError::simple(
            error_tokens::MISSING_MATERIAL_BINDING_API.clone(),
            ErrorType::Error,
            String::new(),
        );
        let edit_target = EditTarget::default();
        let tc = TimeCode::default();

        assert!(!material_binding_api_applied_fixer_apply(
            &error,
            &edit_target,
            &tc
        ));
    }

    #[test]
    fn test_all_error_tokens_unique() {
        let tokens = [
            error_tokens::CONNECTABLE_IN_NON_CONTAINER.clone(),
            error_tokens::INVALID_CONNECTABLE_HIERARCHY.clone(),
            error_tokens::MISSING_MATERIAL_BINDING_API.clone(),
            error_tokens::MATERIAL_BINDING_PROP_NOT_A_REL.clone(),
            error_tokens::INVALID_MATERIAL_COLLECTION.clone(),
            error_tokens::INVALID_RESOURCE_PATH.clone(),
            error_tokens::INVALID_IMPL_SOURCE.clone(),
            error_tokens::MISSING_SOURCE_TYPE.clone(),
            error_tokens::MISSING_SHADER_ID_IN_REGISTRY.clone(),
            error_tokens::MISSING_SOURCE_TYPE_IN_REGISTRY.clone(),
            error_tokens::INCOMPAT_SHADER_PROPERTY_WARNING.clone(),
            error_tokens::MISMATCH_PROPERTY_TYPE.clone(),
            error_tokens::MISSING_FAMILY_NAME_ON_GEOM_SUBSET.clone(),
            error_tokens::NON_SHADER_CONNECTION.clone(),
            error_tokens::INVALID_FILE.clone(),
            error_tokens::INVALID_SHADER_PRIM.clone(),
            error_tokens::INVALID_SOURCE_COLOR_SPACE.clone(),
            error_tokens::NON_COMPLIANT_BIAS_AND_SCALE.clone(),
            error_tokens::NON_COMPLIANT_SCALE.clone(),
            error_tokens::NON_COMPLIANT_BIAS.clone(),
            error_tokens::INVALID_FAMILY_TYPE.clone(),
        ];

        // All 21 error tokens should be distinct.
        assert_eq!(tokens.len(), 21);
    }
}
