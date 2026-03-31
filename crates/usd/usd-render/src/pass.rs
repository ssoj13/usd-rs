//! Render Pass schema.
//!
//! A RenderPass prim encapsulates necessary information to generate
//! multi-pass renders. It houses properties for generating dependencies
//! and commands to run for renders.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/pass.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::collection_api::CollectionAPI;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_sdf::{AssetPath, Path, ValueTypeRegistry};
use usd_tf::Token;

use super::tokens::USD_RENDER_TOKENS;

/// Render pass schema.
///
/// Encapsulates information for generating multi-pass renders.
#[derive(Debug, Clone)]
pub struct RenderPass {
    prim: Prim,
}

impl RenderPass {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RenderPass";

    /// Construct a RenderPass on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a RenderPass holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdRenderPass::Get()` — no type check performed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Attempt to ensure a prim adhering to this schema at `path` is defined.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // PassType Attribute
    // =========================================================================

    /// Get the passType attribute.
    pub fn get_pass_type_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.pass_type.as_str())
    }

    /// Creates the passType attribute.
    pub fn create_pass_type_attr(&self, _default_value: Option<Token>) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.pass_type.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Note: Default value setting omitted - use attr.set() with TimeCode if needed
        attr
    }

    // =========================================================================
    // Command Attribute
    // =========================================================================

    /// Get the command attribute.
    pub fn get_command_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(USD_RENDER_TOKENS.command.as_str())
    }

    /// Creates the command attribute.
    pub fn create_command_attr(&self, _default_value: Option<Vec<String>>) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let string_array_type = registry.find_type_by_token(&Token::new("string[]"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.command.as_str(),
                &string_array_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Note: Default value setting omitted - use attr.set() with TimeCode if needed
        attr
    }

    // =========================================================================
    // FileName Attribute
    // =========================================================================

    /// Get the fileName attribute.
    pub fn get_file_name_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RENDER_TOKENS.file_name.as_str())
    }

    /// Creates the fileName attribute.
    pub fn create_file_name_attr(&self, _default_value: Option<AssetPath>) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let asset_type = registry.find_type_by_token(&Token::new("asset"));

        let attr = self
            .prim
            .create_attribute(
                USD_RENDER_TOKENS.file_name.as_str(),
                &asset_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Note: Default value setting omitted - use attr.set() with TimeCode if needed
        attr
    }

    // =========================================================================
    // RenderSource Relationship
    // =========================================================================

    /// Get the renderSource relationship.
    pub fn get_render_source_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_RENDER_TOKENS.render_source.as_str())
    }

    /// Creates the renderSource relationship.
    pub fn create_render_source_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_RENDER_TOKENS.render_source.as_str(), false)
    }

    // =========================================================================
    // InputPasses Relationship
    // =========================================================================

    /// Get the inputPasses relationship.
    pub fn get_input_passes_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(USD_RENDER_TOKENS.input_passes.as_str())
    }

    /// Creates the inputPasses relationship.
    pub fn create_input_passes_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(USD_RENDER_TOKENS.input_passes.as_str(), false)
    }

    // =========================================================================
    // Collection API accessors
    // =========================================================================

    /// Get the collection name for render visibility.
    pub fn get_render_visibility_collection_name() -> &'static str {
        "renderVisibility"
    }

    /// Get the collection name for camera visibility.
    pub fn get_camera_visibility_collection_name() -> &'static str {
        "cameraVisibility"
    }

    /// Get the collection name for matte objects.
    pub fn get_matte_collection_name() -> &'static str {
        "matte"
    }

    /// Get the collection name for pruned objects.
    pub fn get_prune_collection_name() -> &'static str {
        "prune"
    }

    /// Return the CollectionAPI interface for examining and modifying
    /// the render visibility of this prim.
    ///
    /// The render visibility collection controls which objects participate
    /// in the render. By default, includeRoot is set to true so all objects
    /// are visible unless explicitly excluded.
    pub fn get_render_visibility_collection_api(&self) -> CollectionAPI {
        CollectionAPI::new(
            self.prim.clone(),
            USD_RENDER_TOKENS.render_visibility.clone(),
        )
    }

    /// Return the CollectionAPI interface for camera visibility.
    ///
    /// The camera visibility collection defines which scene objects should
    /// be directly visible in camera. Objects not in this collection still
    /// participate in other light paths (shadows, reflections, refraction).
    pub fn get_camera_visibility_collection_api(&self) -> CollectionAPI {
        // "cameraVisibility" is a collection name string, not in UsdRender tokens
        CollectionAPI::new(self.prim.clone(), Token::new("cameraVisibility"))
    }

    /// Return the CollectionAPI interface for matte objects.
    ///
    /// The matte collection defines scene objects that should act as
    /// matte objects. Matte objects render with zero alpha.
    pub fn get_matte_collection_api(&self) -> CollectionAPI {
        // "matte" is a collection name string, not in UsdRender tokens
        CollectionAPI::new(self.prim.clone(), Token::new("matte"))
    }

    /// Return the CollectionAPI interface for pruned objects.
    ///
    /// The prune collection specifies objects to be removed from the scene
    /// prior to rendering. Pruning entirely removes objects from the
    /// renderer's representation, providing greater runtime cost savings
    /// than visibility toggling.
    pub fn get_prune_collection_api(&self) -> CollectionAPI {
        // "prune" is a collection name string, not in UsdRender tokens
        CollectionAPI::new(self.prim.clone(), Token::new("prune"))
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// Inherits from UsdTyped (no attributes), so inherited == local.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local = vec![
            USD_RENDER_TOKENS.pass_type.clone(),
            USD_RENDER_TOKENS.command.clone(),
            USD_RENDER_TOKENS.file_name.clone(),
        ];
        if include_inherited {
            // UsdTyped has no attributes, so allNames == localNames
            local
        } else {
            local
        }
    }
}

impl From<Prim> for RenderPass {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RenderPass> for Prim {
    fn from(pass: RenderPass) -> Self {
        pass.prim
    }
}

impl AsRef<Prim> for RenderPass {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RenderPass::SCHEMA_TYPE_NAME, "RenderPass");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RenderPass::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n.get_text() == "passType"));
        assert!(names.iter().any(|n| n.get_text() == "command"));
        assert!(names.iter().any(|n| n.get_text() == "fileName"));
    }

    #[test]
    fn test_collection_names() {
        assert_eq!(
            RenderPass::get_render_visibility_collection_name(),
            "renderVisibility"
        );
        assert_eq!(
            RenderPass::get_camera_visibility_collection_name(),
            "cameraVisibility"
        );
        assert_eq!(RenderPass::get_matte_collection_name(), "matte");
        assert_eq!(RenderPass::get_prune_collection_name(), "prune");
    }
}
