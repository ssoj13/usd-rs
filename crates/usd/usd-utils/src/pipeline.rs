//! Pipeline conventions for USD.
//!
//! Provides module-scoped utilities for establishing pipeline conventions
//! for things not currently suitable or possible to canonize in USD's schema modules.

use super::registered_variant_set::{RegisteredVariantSet, SelectionExportPolicy};
use once_cell::sync::Lazy;
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};
use usd_core::prim::Prim;
use usd_core::stage::Stage;
use usd_sdf::layer::Layer;
use usd_sdf::path::Path;
use usd_tf::Token;

/// Default materials scope name.
const DEFAULT_MATERIALS_SCOPE_NAME: &str = "Looks";

/// Default primary camera name.
const DEFAULT_PRIMARY_CAMERA_NAME: &str = "main_cam";

/// Default primary UV set name.
const DEFAULT_PRIMARY_UV_SET_NAME: &str = "st";

/// Default reference position name.
const DEFAULT_PREF_NAME: &str = "pref";

/// Global registry of variant sets.
static REGISTERED_VARIANT_SETS: Lazy<RwLock<BTreeSet<RegisteredVariantSet>>> =
    Lazy::new(|| RwLock::new(BTreeSet::new()));

/// Global pipeline configuration.
static PIPELINE_CONFIG: Lazy<RwLock<PipelineConfig>> =
    Lazy::new(|| RwLock::new(PipelineConfig::default()));

#[derive(Debug, Clone)]
struct PipelineConfig {
    materials_scope_name: String,
    primary_camera_name: String,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            materials_scope_name: DEFAULT_MATERIALS_SCOPE_NAME.to_string(),
            primary_camera_name: DEFAULT_PRIMARY_CAMERA_NAME.to_string(),
        }
    }
}

/// Returns the alpha/opacity attribute name for a given color attribute.
pub fn get_alpha_attribute_name_for_color(color_attr_name: &Token) -> Token {
    Token::from(format!("{}Alpha", color_attr_name.as_str()))
}

/// Returns the model name associated with a root layer.
pub fn get_model_name_from_root_layer(root_layer: &Arc<Layer>) -> Token {
    // Check default prim first
    let default_prim = root_layer.get_default_prim();
    if !default_prim.is_empty() {
        return default_prim;
    }

    // Try to derive from layer identifier
    let identifier = root_layer.identifier();
    let file_stem = std::path::Path::new(identifier)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if !file_stem.is_empty() {
        if let Some(path) = Path::from_string(&format!("/{}", file_stem)) {
            if root_layer.get_prim_at_path(&path).is_some() {
                return Token::from(file_stem);
            }
        }
    }

    // Fall back to first root prim
    let root_prims = root_layer.root_prims();
    if let Some(first) = root_prims.first() {
        return first.name_token();
    }

    Token::empty()
}

/// Returns the set of registered variant sets.
pub fn get_registered_variant_sets() -> BTreeSet<RegisteredVariantSet> {
    if let Ok(sets) = REGISTERED_VARIANT_SETS.read() {
        sets.clone()
    } else {
        BTreeSet::new()
    }
}

/// Registers a variant set with the given export policy.
pub fn register_variant_set(
    variant_set_name: &str,
    selection_export_policy: SelectionExportPolicy,
) {
    let variant_set = RegisteredVariantSet::new(variant_set_name, selection_export_policy);

    if let Ok(mut sets) = REGISTERED_VARIANT_SETS.write() {
        sets.insert(variant_set);
    }
}

/// Returns the prim at a path, following instance-to-prototype forwarding.
pub fn get_prim_at_path_with_forwarding(stage: &Arc<Stage>, path: &Path) -> Option<Prim> {
    if let Some(prim) = stage.get_prim_at_path(path) {
        return Some(prim);
    }

    let mut current_path = path.get_parent_path();

    while !current_path.is_empty() && current_path.as_str() != "/" {
        if let Some(ancestor) = stage.get_prim_at_path(&current_path) {
            if ancestor.is_instance() {
                let prototype = ancestor.get_prototype();
                if prototype.is_valid() {
                    if let Some(relative) = path.make_relative(&current_path) {
                        if let Some(prototype_path) = prototype.path().append_path(&relative) {
                            return stage.get_prim_at_path(&prototype_path);
                        }
                    }
                }
            }
        }
        current_path = current_path.get_parent_path();
    }

    None
}

/// Uninstances all prims in the namespace chain and returns the prim at the path.
pub fn uninstance_prim_at_path(stage: &Arc<Stage>, path: &Path) -> Option<Prim> {
    if let Some(prim) = stage.get_prim_at_path(path) {
        return Some(prim);
    }

    let mut current_path = path.get_parent_path();
    let mut instances_to_uninstance = Vec::new();

    while !current_path.is_empty() && current_path.as_str() != "/" {
        if let Some(ancestor) = stage.get_prim_at_path(&current_path) {
            if ancestor.is_instance() {
                instances_to_uninstance.push(current_path.clone());
            }
        }
        current_path = current_path.get_parent_path();
    }

    instances_to_uninstance.reverse();
    for instance_path in &instances_to_uninstance {
        if let Some(prim) = stage.get_prim_at_path(instance_path) {
            prim.set_instanceable(false);
        }
    }

    stage.get_prim_at_path(path)
}

/// Returns the name of the primary UV set used on meshes and nurbs.
pub fn get_primary_uv_set_name() -> &'static Token {
    static PRIMARY_UV_SET: Lazy<Token> = Lazy::new(|| Token::from(DEFAULT_PRIMARY_UV_SET_NAME));
    &PRIMARY_UV_SET
}

/// Returns the name of the reference position used on meshes and nurbs.
pub fn get_pref_name() -> &'static Token {
    static PREF_NAME: Lazy<Token> = Lazy::new(|| Token::from(DEFAULT_PREF_NAME));
    &PREF_NAME
}

/// Returns the name of the USD prim under which materials are expected.
pub fn get_materials_scope_name(force_default: bool) -> Token {
    if force_default {
        return Token::from(DEFAULT_MATERIALS_SCOPE_NAME);
    }

    if let Ok(config) = PIPELINE_CONFIG.read() {
        Token::from(config.materials_scope_name.as_str())
    } else {
        Token::from(DEFAULT_MATERIALS_SCOPE_NAME)
    }
}

/// Returns the name of the USD prim representing the primary camera.
pub fn get_primary_camera_name(force_default: bool) -> Token {
    if force_default {
        return Token::from(DEFAULT_PRIMARY_CAMERA_NAME);
    }

    if let Ok(config) = PIPELINE_CONFIG.read() {
        Token::from(config.primary_camera_name.as_str())
    } else {
        Token::from(DEFAULT_PRIMARY_CAMERA_NAME)
    }
}

/// Sets the materials scope name (for testing or runtime configuration).
pub fn set_materials_scope_name(name: &str) {
    if let Ok(mut config) = PIPELINE_CONFIG.write() {
        config.materials_scope_name = name.to_string();
    }
}

/// Sets the primary camera name (for testing or runtime configuration).
pub fn set_primary_camera_name(name: &str) {
    if let Ok(mut config) = PIPELINE_CONFIG.write() {
        config.primary_camera_name = name.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_alpha_attribute_name() {
        let color = Token::from("diffuseColor");
        let alpha = get_alpha_attribute_name_for_color(&color);
        assert_eq!(alpha.as_str(), "diffuseColorAlpha");
    }

    #[test]
    fn test_get_primary_uv_set_name() {
        assert_eq!(get_primary_uv_set_name().as_str(), "st");
    }

    #[test]
    fn test_get_pref_name() {
        assert_eq!(get_pref_name().as_str(), "pref");
    }

    #[test]
    fn test_get_materials_scope_name_default() {
        let name = get_materials_scope_name(true);
        assert_eq!(name.as_str(), "Looks");
    }

    #[test]
    fn test_get_primary_camera_name_default() {
        let name = get_primary_camera_name(true);
        assert_eq!(name.as_str(), "main_cam");
    }

    #[test]
    fn test_register_variant_set() {
        register_variant_set("testVariant", SelectionExportPolicy::Always);

        let sets = get_registered_variant_sets();
        let found = sets.iter().any(|s| s.name() == "testVariant");
        assert!(found);
    }
}
