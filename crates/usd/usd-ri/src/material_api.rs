//! RiMaterialAPI schema.
//!
//! API for connecting material prims to RenderMan shaders.
//! Provides outputs that connect a material prim to prman shaders and RIS objects.
//!
//! # Deprecation Notice
//!
//! This schema is deprecated. Materials should use UsdShadeMaterial instead.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRi/materialAPI.h` and `materialAPI.cpp`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::schema_base::APISchemaBase;
use usd_core::{Attribute, Prim, Stage};
use usd_sdf::Path;
use usd_shade::node_graph::InterfaceInputConsumersMap;
use usd_shade::{ConnectableAPI, Material, NodeGraph, Output, Shader};
use usd_tf::Token;

use super::tokens::USD_RI_TOKENS;

/// Private tokens for internal use.
struct PrivateTokens {
    /// Default output name for connections
    default_output_name: Token,
    /// RenderMan render context
    ri: Token,
    /// Deprecated bxdf output attribute name
    bxdf_output_attr_name: Token,
}

impl PrivateTokens {
    fn new() -> Self {
        Self {
            default_output_name: Token::new("outputs:out"),
            ri: Token::new("ri"),
            bxdf_output_attr_name: Token::new("outputs:ri:bxdf"),
        }
    }
}

/// RiMaterialAPI - connects materials to RenderMan shaders.
///
/// This API provides outputs that connect a material prim to prman
/// shaders and RIS objects.
///
/// # Schema Kind
///
/// This is a SingleApplyAPI schema.
///
/// # Deprecation
///
/// This schema is deprecated. Use UsdShadeMaterial instead.
#[derive(Debug, Clone)]
pub struct RiMaterialAPI {
    prim: Prim,
}

impl RiMaterialAPI {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "RiMaterialAPI";

    /// Construct a RiMaterialAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Construct from a Material.
    pub fn from_material(material: &Material) -> Self {
        Self::new(material.get_prim().clone())
    }

    /// Return a RiMaterialAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        if prim.has_api(&USD_RI_TOKENS.ri_material_api) {
            Some(Self::new(prim))
        } else {
            None
        }
    }

    /// Check if this API can be applied to the given prim.
    pub fn can_apply(prim: &Prim) -> bool {
        prim.can_apply_api(&USD_RI_TOKENS.ri_material_api)
    }

    /// Apply this API to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_RI_TOKENS.ri_material_api) {
            Some(Self::new(prim.clone()))
        } else {
            None
        }
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
    // Surface Attribute
    // =========================================================================

    /// Get the surface output attribute.
    ///
    /// Declaration: `token outputs:ri:surface`
    pub fn get_surface_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RI_TOKENS.outputs_ri_surface.as_str())
    }

    /// Create the surface output attribute.
    ///
    /// Creates a token-typed attribute with varying variability.
    /// Matches C++ _CreateAttr(outputsRiSurface, Token, custom=false, Varying).
    pub fn create_surface_attr(&self) -> Option<Attribute> {
        self.prim.create_attribute(
            USD_RI_TOKENS.outputs_ri_surface.as_str(),
            &usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("token"),
            false,
            Some(Variability::Varying),
        )
    }

    // =========================================================================
    // Displacement Attribute
    // =========================================================================

    /// Get the displacement output attribute.
    ///
    /// Declaration: `token outputs:ri:displacement`
    pub fn get_displacement_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RI_TOKENS.outputs_ri_displacement.as_str())
    }

    /// Create the displacement output attribute.
    ///
    /// Creates a token-typed attribute with varying variability.
    /// Matches C++ _CreateAttr(outputsRiDisplacement, Token, custom=false, Varying).
    pub fn create_displacement_attr(&self) -> Option<Attribute> {
        self.prim.create_attribute(
            USD_RI_TOKENS.outputs_ri_displacement.as_str(),
            &usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("token"),
            false,
            Some(Variability::Varying),
        )
    }

    // =========================================================================
    // Volume Attribute
    // =========================================================================

    /// Get the volume output attribute.
    ///
    /// Declaration: `token outputs:ri:volume`
    pub fn get_volume_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_RI_TOKENS.outputs_ri_volume.as_str())
    }

    /// Create the volume output attribute.
    ///
    /// Creates a token-typed attribute with varying variability.
    /// Matches C++ _CreateAttr(outputsRiVolume, Token, custom=false, Varying).
    pub fn create_volume_attr(&self) -> Option<Attribute> {
        self.prim.create_attribute(
            USD_RI_TOKENS.outputs_ri_volume.as_str(),
            &usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("token"),
            false,
            Some(Variability::Varying),
        )
    }

    // =========================================================================
    // Output API
    // =========================================================================

    /// Returns the "surface" output associated with the material.
    ///
    /// Uses the "ri" render context to get the surface output from the
    /// underlying UsdShadeMaterial.
    pub fn get_surface_output(&self) -> Output {
        let tokens = PrivateTokens::new();
        Material::new(self.prim.clone()).get_surface_output(&tokens.ri)
    }

    /// Returns the "displacement" output associated with the material.
    pub fn get_displacement_output(&self) -> Output {
        let tokens = PrivateTokens::new();
        Material::new(self.prim.clone()).get_displacement_output(&tokens.ri)
    }

    /// Returns the "volume" output associated with the material.
    pub fn get_volume_output(&self) -> Output {
        let tokens = PrivateTokens::new();
        Material::new(self.prim.clone()).get_volume_output(&tokens.ri)
    }

    // =========================================================================
    // Source Setting API
    // =========================================================================

    /// Set the surface shader source.
    ///
    /// Creates the surface output for the "ri" render context and connects
    /// it to the given source path. If the path is not a property path,
    /// appends "outputs:out" to it.
    pub fn set_surface_source(&self, surface_path: &Path) -> bool {
        let tokens = PrivateTokens::new();
        let material = Material::new(self.prim.clone());
        let surface_output = material.create_surface_output(&tokens.ri);

        let connect_path = if surface_path.is_property_path() {
            surface_path.clone()
        } else {
            surface_path
                .append_property(tokens.default_output_name.as_str())
                .unwrap_or_else(|| surface_path.clone())
        };

        if let Some(attr) = surface_output.get_attr() {
            ConnectableAPI::connect_to_source_path(&attr, &connect_path)
        } else {
            false
        }
    }

    /// Set the displacement shader source.
    pub fn set_displacement_source(&self, displacement_path: &Path) -> bool {
        let tokens = PrivateTokens::new();
        let material = Material::new(self.prim.clone());
        let displacement_output = material.create_displacement_output(&tokens.ri);

        let connect_path = if displacement_path.is_property_path() {
            displacement_path.clone()
        } else {
            displacement_path
                .append_property(tokens.default_output_name.as_str())
                .unwrap_or_else(|| displacement_path.clone())
        };

        if let Some(attr) = displacement_output.get_attr() {
            ConnectableAPI::connect_to_source_path(&attr, &connect_path)
        } else {
            false
        }
    }

    /// Set the volume shader source.
    pub fn set_volume_source(&self, volume_path: &Path) -> bool {
        let tokens = PrivateTokens::new();
        let material = Material::new(self.prim.clone());
        let volume_output = material.create_volume_output(&tokens.ri);

        let connect_path = if volume_path.is_property_path() {
            volume_path.clone()
        } else {
            volume_path
                .append_property(tokens.default_output_name.as_str())
                .unwrap_or_else(|| volume_path.clone())
        };

        if let Some(attr) = volume_output.get_attr() {
            ConnectableAPI::connect_to_source_path(&attr, &connect_path)
        } else {
            false
        }
    }

    // =========================================================================
    // Shader Retrieval API
    // =========================================================================

    /// Returns a valid shader object if the "surface" output is connected.
    ///
    /// If `ignore_base_material` is true and if the "surface" shader source
    /// is specified in the base-material of this material, returns an
    /// invalid shader object.
    pub fn get_surface(&self, ignore_base_material: bool) -> Shader {
        if let Some(shader) =
            self.get_source_shader_object(&self.get_surface_output(), ignore_base_material)
        {
            return shader;
        }

        // Check deprecated bxdf output for backwards compatibility
        if let Some(bxdf_output) = self.get_bxdf_output() {
            if let Some(shader) = self.get_source_shader_object(&bxdf_output, ignore_base_material)
            {
                return shader;
            }
        }

        Shader::invalid()
    }

    /// Returns a valid shader object if the "displacement" output is connected.
    pub fn get_displacement(&self, ignore_base_material: bool) -> Shader {
        self.get_source_shader_object(&self.get_displacement_output(), ignore_base_material)
            .unwrap_or_else(Shader::invalid)
    }

    /// Returns a valid shader object if the "volume" output is connected.
    pub fn get_volume(&self, ignore_base_material: bool) -> Shader {
        self.get_source_shader_object(&self.get_volume_output(), ignore_base_material)
            .unwrap_or_else(Shader::invalid)
    }

    /// Helper to get connected shader from an output.
    ///
    /// Returns None if output has no valid attribute, or if ignoring base
    /// material and the connection comes from base material.
    fn get_source_shader_object(
        &self,
        output: &Output,
        ignore_base_material: bool,
    ) -> Option<Shader> {
        // Check if output has valid attribute
        let attr = output.get_attr()?;
        if !attr.is_valid() {
            return None;
        }

        // Check if connection is from base material
        if ignore_base_material && ConnectableAPI::is_source_connection_from_base_material(&attr) {
            return None;
        }

        // Get connected source
        let mut invalid_paths = Vec::new();
        let sources = output.get_connected_sources(&mut invalid_paths);

        if let Some(source_info) = sources.first() {
            let source_prim = source_info.source.get_prim();
            if source_prim.is_valid() {
                return Some(Shader::new(source_prim));
            }
        }

        None
    }

    /// Get deprecated bxdf output for backwards compatibility.
    fn get_bxdf_output(&self) -> Option<Output> {
        let tokens = PrivateTokens::new();
        let attr = self
            .prim
            .get_attribute(tokens.bxdf_output_attr_name.as_str())?;
        Some(Output::from_attribute(attr))
    }

    // =========================================================================
    // Interface Input Consumers
    // =========================================================================

    /// Walks the namespace subtree below the material and computes a map
    /// containing the list of all inputs on the material and the associated
    /// vector of consumers of their values.
    ///
    /// The consumers can be inputs on shaders within the material or on
    /// node-graphs under it.
    pub fn compute_interface_input_consumers_map(
        &self,
        compute_transitive_consumers: bool,
    ) -> InterfaceInputConsumersMap {
        NodeGraph::new(self.prim.clone())
            .compute_interface_input_consumers_map(compute_transitive_consumers)
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// When `include_inherited` is true, concatenates parent APISchemaBase names
    /// with local names, matching C++ `_ConcatenateAttributeNames` behavior.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            USD_RI_TOKENS.outputs_ri_surface.clone(),
            USD_RI_TOKENS.outputs_ri_displacement.clone(),
            USD_RI_TOKENS.outputs_ri_volume.clone(),
        ];

        if include_inherited {
            // Prepend inherited names from APISchemaBase (empty for base, but correct hierarchy)
            let mut all = APISchemaBase::get_schema_attribute_names(true);
            all.extend(local_names);
            all
        } else {
            local_names
        }
    }
}

impl From<Prim> for RiMaterialAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<RiMaterialAPI> for Prim {
    fn from(api: RiMaterialAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for RiMaterialAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(RiMaterialAPI::SCHEMA_TYPE_NAME, "RiMaterialAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = RiMaterialAPI::get_schema_attribute_names(false);
        assert_eq!(names.len(), 3);
        assert!(names.iter().any(|n| n == "outputs:ri:surface"));
        assert!(names.iter().any(|n| n == "outputs:ri:displacement"));
        assert!(names.iter().any(|n| n == "outputs:ri:volume"));
    }
}
