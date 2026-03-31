//! USD Shade Material - container for shading networks.
//!
//! Port of pxr/usd/usdShade/material.h and material.cpp
//!
//! A Material provides a container into which multiple "render contexts"
//! can add data that defines a "shading material" for a renderer.

use super::node_graph::NodeGraph;
use super::output::Output;
use super::shader::Shader;
use super::tokens::tokens;
use super::types::{AttributeType, AttributeVector};
use super::utils::Utils;
use std::sync::Arc;
use usd_core::attribute::Attribute;
use usd_core::edit_target::EditTarget;
use usd_core::prim::Prim;
use usd_core::stage::Stage;
use usd_core::variant_sets::VariantSet;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// A Material provides a container into which multiple "render contexts"
/// can add data that defines a "shading material" for a renderer.
#[derive(Debug, Clone)]
pub struct Material {
    /// Base node graph.
    node_graph: NodeGraph,
}

impl Material {
    /// Construct a Material on UsdPrim.
    pub fn new(prim: Prim) -> Self {
        Self {
            node_graph: NodeGraph::new(prim),
        }
    }

    /// Construct a Material from a NodeGraph.
    pub fn from_node_graph(node_graph: NodeGraph) -> Self {
        Self { node_graph }
    }

    /// Creates an invalid Material.
    pub fn invalid() -> Self {
        Self {
            node_graph: NodeGraph::invalid(),
        }
    }

    /// Return a Material holding the prim adhering to this schema at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        NodeGraph::get(stage, path).into()
    }

    /// Attempt to ensure a UsdPrim adhering to this schema at `path` is defined on this stage.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Self {
        match stage.define_prim(path.to_string(), "Material") {
            Ok(prim) => Self::new(prim),
            Err(_) => Self::invalid(),
        }
    }

    /// Returns true if this Material is valid.
    ///
    /// Uses schema hierarchy check (matches C++ `IsA<UsdShadeMaterial>()`).
    /// Accepts Material and any user-defined types deriving from Material.
    pub fn is_valid(&self) -> bool {
        let prim = self.node_graph.get_prim();
        prim.is_valid() && prim.is_a(&usd_tf::Token::new("Material"))
    }

    /// Returns the wrapped prim.
    pub fn get_prim(&self) -> Prim {
        self.node_graph.get_prim()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.node_graph.path()
    }

    /// Returns the stage.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.node_graph.stage()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Cached per C++ pattern (static local vectors, returned by reference).
    pub fn get_schema_attribute_names(include_inherited: bool) -> &'static Vec<Token> {
        use std::sync::OnceLock;
        static LOCAL: OnceLock<Vec<Token>> = OnceLock::new();
        static ALL: OnceLock<Vec<Token>> = OnceLock::new();

        if include_inherited {
            ALL.get_or_init(|| {
                let mut all_names = NodeGraph::get_schema_attribute_names(true).clone();
                all_names.extend_from_slice(Self::get_schema_attribute_names(false));
                all_names
            })
        } else {
            LOCAL.get_or_init(|| {
                vec![
                    tokens().outputs_surface.clone(),
                    tokens().outputs_displacement.clone(),
                    tokens().outputs_volume.clone(),
                ]
            })
        }
    }

    // ========================================================================
    // Standard Material Terminal Attributes
    // ========================================================================

    /// Returns the surface attribute.
    ///
    /// This attribute represents the universal "surface" output terminal of a material.
    pub fn get_surface_attr(&self) -> Option<Attribute> {
        self.get_prim().get_attribute("outputs:surface")
    }

    /// Creates the surface attribute.
    ///
    /// This attribute represents the universal "surface" output terminal of a material.
    pub fn create_surface_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = "outputs:surface";

        if prim.has_authored_attribute(attr_name) {
            return prim.get_attribute(attr_name);
        }

        // Create attribute with token type
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        let attr = prim.create_attribute(attr_name, &token_type, false, None)?;

        if let Some(value) = default_value {
            if write_sparsely {
                // Note: Would compare to fallback value for sparse authoring.
                let _ = attr.set(value, usd_sdf::TimeCode::default());
            } else {
                let _ = attr.set(value, usd_sdf::TimeCode::default());
            }
        }

        Some(attr)
    }

    /// Returns the displacement attribute.
    ///
    /// This attribute represents the universal "displacement" output terminal of a material.
    pub fn get_displacement_attr(&self) -> Option<Attribute> {
        self.get_prim().get_attribute("outputs:displacement")
    }

    /// Creates the displacement attribute.
    ///
    /// This attribute represents the universal "displacement" output terminal of a material.
    pub fn create_displacement_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = "outputs:displacement";

        if prim.has_authored_attribute(attr_name) {
            return prim.get_attribute(attr_name);
        }

        // Create attribute with token type
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        let attr = prim.create_attribute(attr_name, &token_type, false, None)?;

        if let Some(value) = default_value {
            if write_sparsely {
                // Note: Would compare to fallback value for sparse authoring.
                let _ = attr.set(value, usd_sdf::TimeCode::default());
            } else {
                let _ = attr.set(value, usd_sdf::TimeCode::default());
            }
        }

        Some(attr)
    }

    /// Returns the volume attribute.
    ///
    /// This attribute represents the universal "volume" output terminal of a material.
    pub fn get_volume_attr(&self) -> Option<Attribute> {
        self.get_prim().get_attribute("outputs:volume")
    }

    /// Creates the volume attribute.
    ///
    /// This attribute represents the universal "volume" output terminal of a material.
    pub fn create_volume_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Option<Attribute> {
        let prim = self.get_prim();
        let attr_name = "outputs:volume";

        if prim.has_authored_attribute(attr_name) {
            return prim.get_attribute(attr_name);
        }

        // Create attribute with token type
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        let attr = prim.create_attribute(attr_name, &token_type, false, None)?;

        if let Some(value) = default_value {
            if write_sparsely {
                // Note: Would compare to fallback value for sparse authoring.
                let _ = attr.set(value, usd_sdf::TimeCode::default());
            } else {
                let _ = attr.set(value, usd_sdf::TimeCode::default());
            }
        }

        Some(attr)
    }

    // ========================================================================
    // Standard Material Terminal Outputs
    // ========================================================================

    /// Creates and returns the "surface" output on this material for the
    /// specified `render_context`.
    pub fn create_surface_output(&self, render_context: &Token) -> Output {
        let output_name =
            Self::_get_output_name_for_render_context(&tokens().surface, render_context);
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        self.node_graph.create_output(&output_name, &token_type)
    }

    /// Returns the "surface" output of this material for the specified `render_context`.
    pub fn get_surface_output(&self, render_context: &Token) -> Output {
        let output_name =
            Self::_get_output_name_for_render_context(&tokens().surface, render_context);
        self.node_graph.get_output(&output_name)
    }

    /// Returns the "surface" outputs of this material for all available renderContexts.
    pub fn get_surface_outputs(&self) -> Vec<Output> {
        Self::_get_outputs_for_terminal_name(&self.node_graph, &tokens().surface)
    }

    /// Computes the resolved "surface" output source for the given `context_vector`.
    pub fn compute_surface_source(
        &self,
        context_vector: &[Token],
        source_name: &mut Token,
        source_type: &mut AttributeType,
    ) -> Shader {
        Self::_compute_named_output_shader(
            &self.node_graph,
            &tokens().surface,
            context_vector,
            source_name,
            source_type,
        )
    }

    /// Creates and returns the "displacement" output on this material for the
    /// specified `render_context`.
    pub fn create_displacement_output(&self, render_context: &Token) -> Output {
        let output_name =
            Self::_get_output_name_for_render_context(&tokens().displacement, render_context);
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        self.node_graph.create_output(&output_name, &token_type)
    }

    /// Returns the "displacement" output of this material for the specified `render_context`.
    pub fn get_displacement_output(&self, render_context: &Token) -> Output {
        let output_name =
            Self::_get_output_name_for_render_context(&tokens().displacement, render_context);
        self.node_graph.get_output(&output_name)
    }

    /// Returns the "displacement" outputs of this material for all available renderContexts.
    pub fn get_displacement_outputs(&self) -> Vec<Output> {
        Self::_get_outputs_for_terminal_name(&self.node_graph, &tokens().displacement)
    }

    /// Computes the resolved "displacement" output source for the given `context_vector`.
    pub fn compute_displacement_source(
        &self,
        context_vector: &[Token],
        source_name: &mut Token,
        source_type: &mut AttributeType,
    ) -> Shader {
        Self::_compute_named_output_shader(
            &self.node_graph,
            &tokens().displacement,
            context_vector,
            source_name,
            source_type,
        )
    }

    /// Creates and returns the "volume" output on this material for the
    /// specified `render_context`.
    pub fn create_volume_output(&self, render_context: &Token) -> Output {
        let output_name =
            Self::_get_output_name_for_render_context(&tokens().volume, render_context);
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        self.node_graph.create_output(&output_name, &token_type)
    }

    /// Returns the "volume" output of this material for the specified `render_context`.
    pub fn get_volume_output(&self, render_context: &Token) -> Output {
        let output_name =
            Self::_get_output_name_for_render_context(&tokens().volume, render_context);
        self.node_graph.get_output(&output_name)
    }

    /// Returns the "volume" outputs of this material for all available renderContexts.
    pub fn get_volume_outputs(&self) -> Vec<Output> {
        Self::_get_outputs_for_terminal_name(&self.node_graph, &tokens().volume)
    }

    /// Computes the resolved "volume" output source for the given `context_vector`.
    pub fn compute_volume_source(
        &self,
        context_vector: &[Token],
        source_name: &mut Token,
        source_type: &mut AttributeType,
    ) -> Shader {
        Self::_compute_named_output_shader(
            &self.node_graph,
            &tokens().volume,
            context_vector,
            source_name,
            source_type,
        )
    }

    // ========================================================================
    // Material Variations
    // ========================================================================

    /// Helper function for configuring a UsdStage's UsdEditTarget to author
    /// Material variations.
    ///
    /// Matches C++ `GetEditContextForVariant(materialVariation, layer)`:
    /// - Returns original stage edit target if variant operations fail.
    /// - Passes layer to GetVariantEditTarget when provided.
    pub fn get_edit_context_for_variant(
        &self,
        material_variant_name: &Token,
        layer: Option<Arc<usd_sdf::Layer>>,
    ) -> (Arc<Stage>, EditTarget) {
        let Some(stage) = self.stage() else {
            // Stage unavailable — cannot build a useful target, return a temp
            let temp_stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
                .expect("temp stage");
            return (temp_stage, EditTarget::default());
        };

        // Start from the current stage edit target as fallback (C++ material.cpp:217)
        let fallback_target = stage.get_edit_target();

        let variant_set = self.get_material_variant();

        // Try to add the variant and set selection; if either fails, return original target.
        let added = variant_set.add_variant(
            material_variant_name.as_str(),
            usd_core::common::ListPosition::BackOfAppendList,
        );
        let selected = variant_set.set_variant_selection(material_variant_name.as_str());

        let edit_target = if added && selected {
            // Build variant edit target, using the provided layer if available.
            // C++ GetVariantEditTarget(layer) uses the given layer to construct
            // a variant-scoped edit target.
            if let Some(ref specific_layer) = layer {
                // Build the variant path and construct the edit target with the given layer.
                let prim_path = variant_set.prim().path().clone();
                let variant_name = variant_set.get_variant_selection();
                if let Some(variant_path) =
                    prim_path.append_variant_selection(variant_set.name(), &variant_name)
                {
                    usd_core::edit_target::EditTarget::for_local_direct_variant(
                        usd_sdf::LayerHandle::from_layer(specific_layer),
                        variant_path,
                    )
                } else {
                    variant_set.get_variant_edit_target()
                }
            } else {
                variant_set.get_variant_edit_target()
            }
        } else {
            fallback_target
        };

        (stage, edit_target)
    }

    /// Return a UsdVariantSet object for interacting with the Material variant variantSet.
    pub fn get_material_variant(&self) -> VariantSet {
        self.get_prim()
            .get_variant_set(tokens().material_variant.as_str())
    }

    // ========================================================================
    // BaseMaterial
    // ========================================================================

    /// Get the base Material of this Material.
    pub fn get_base_material(&self) -> Material {
        let base_path = self.get_base_material_path();
        if base_path.is_empty() {
            return Material::invalid();
        }

        let Some(stage) = self.stage() else {
            return Material::invalid();
        };

        Material::get(&stage, &base_path)
    }

    /// Get the path to the base Material of this Material.
    ///
    /// Traverses specializes arcs in the prim index to find the base material.
    /// Matches C++ `UsdShadeMaterial::GetBaseMaterialPath()`.
    pub fn get_base_material_path(&self) -> Path {
        let prim = self.get_prim();

        // Use PrimIndex traversal if available (proper composed scene).
        let parent_material_path = Self::find_base_material_path_in_prim_index_impl(&prim);

        if !parent_material_path.is_empty() {
            // Handle instance proxies: return prototype path instead.
            if let Some(stage) = prim.stage() {
                if let Some(p) = stage.get_prim_at_path(&parent_material_path) {
                    if p.is_instance_proxy() {
                        return p.get_prim_in_prototype().path().clone();
                    }
                }
            }
        }

        parent_material_path
    }

    /// Set the base Material of this Material.
    pub fn set_base_material(&self, base_material: &Material) {
        if !base_material.is_valid() {
            self.clear_base_material();
            return;
        }

        let base_path = base_material.path();
        self.set_base_material_path(base_path);
    }

    /// Set the path to the base Material of this Material.
    ///
    /// Replaces ALL existing specializes arcs with exactly one path,
    /// matching C++ `specializes.SetSpecializes({baseMaterialPath})`.
    pub fn set_base_material_path(&self, base_material_path: &Path) {
        if base_material_path.is_empty() {
            self.clear_base_material();
            return;
        }

        // Replace all specializes with exactly this one path (C++ material.cpp:449-450)
        let prim = self.get_prim();
        let specializes = prim.get_specializes();
        let _ = specializes.set_specializes(vec![base_material_path.clone()]);
    }

    /// Clear the base Material of this Material.
    pub fn clear_base_material(&self) {
        let prim = self.get_prim();
        let specializes = prim.get_specializes();
        let _ = specializes.clear_specializes();
    }

    /// Check if this Material has a base Material.
    pub fn has_base_material(&self) -> bool {
        !self.get_base_material_path().is_empty()
    }

    /// Creates a master material variant set and variant for the given
    /// material prim.
    ///
    /// This is used in production workflows where a single material prim
    /// can have multiple material variant definitions (e.g., "wood", "metal").
    /// Each variant can contain a different shading network.
    ///
    /// Matches C++ `UsdShadeMaterial::CreateMasterMaterialVariant()`.
    ///
    /// C++ signature is a static function; this matches that contract.
    /// `master_variant_set_name` defaults to the standard materialVariant token when None.
    pub fn create_master_material_variant(
        master_prim: &Prim,
        material_prims: &[Prim],
        master_variant_set_name: Option<&Token>,
    ) -> bool {
        if !master_prim.is_valid() || material_prims.is_empty() {
            return false;
        }

        // Determine the variant set name to use on master_prim.
        let master_set_name = master_variant_set_name
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| tokens().material_variant.clone());

        let Some(stage) = master_prim.stage() else {
            return false;
        };

        // Collect variant names from the first material prim as the canonical list.
        let first_variants = material_prims[0]
            .get_variant_set(tokens().material_variant.as_str())
            .get_variant_names();
        if first_variants.is_empty() {
            return false;
        }

        // Validate all material prims live on the same stage and have the same variants.
        for mat_prim in material_prims.iter().skip(1) {
            let Some(prim_stage) = mat_prim.stage() else {
                return false;
            };
            if !std::sync::Arc::ptr_eq(&stage, &prim_stage) {
                return false;
            }
            let variants = mat_prim
                .get_variant_set(tokens().material_variant.as_str())
                .get_variant_names();
            if variants != first_variants {
                return false;
            }
        }

        // For each variant: create it on master, set selection, then inside the edit context
        // author variant selections on each material prim (C++ material.cpp:307-358).
        let master_vs = master_prim.get_variant_set(master_set_name.as_str());
        for variant_name in &first_variants {
            if !master_vs.add_variant(
                variant_name,
                usd_core::common::ListPosition::BackOfAppendList,
            ) {
                return false;
            }
            // Select this variant so we can enter its edit context
            master_vs.set_variant_selection(variant_name);

            // Get the edit target for this variant's scope
            let variant_edit_target = master_vs.get_variant_edit_target();

            // Within the variant's edit context, author materialVariant selections
            // on each material prim (or an over at a derived path if outside master).
            let prev_target = stage.get_edit_target();
            stage.set_edit_target(variant_edit_target);

            for mat_prim in material_prims.iter() {
                if mat_prim.path().has_prefix(master_prim.path()) {
                    // Material is under master_prim — set selection directly
                    mat_prim
                        .get_variant_set(tokens().material_variant.as_str())
                        .set_variant_selection(variant_name);
                } else {
                    // Material is outside master_prim — create an over at derived path
                    let root_path = {
                        let mut p = mat_prim.path().clone();
                        while !p.is_root_prim_path() {
                            p = p.get_parent_path();
                        }
                        p
                    };
                    if let Some(derived_path) = mat_prim
                        .path()
                        .replace_prefix(&root_path, master_prim.path())
                    {
                        match stage.override_prim(derived_path.get_string()) {
                            Ok(over_prim) => {
                                over_prim
                                    .get_variant_set(tokens().material_variant.as_str())
                                    .set_variant_selection(variant_name);
                            }
                            Err(_) => {
                                // Restore edit target before failing
                                stage.set_edit_target(prev_target);
                                return false;
                            }
                        }
                    }
                }
            }

            stage.set_edit_target(prev_target);
        }

        true
    }

    /// Finds base material path by traversing PrimIndex specializes arcs.
    ///
    /// Matches C++ `UsdShadeMaterial::FindBaseMaterialPathInPrimIndex()`.
    /// Walks PcpPrimIndex nodes, looking for Specialize arcs that are
    /// direct children of the root node and don't cross reference boundaries.
    /// Falls back to reading specializes metadata when PrimIndex has no arcs.
    fn find_base_material_path_in_prim_index_impl(prim: &Prim) -> Path {
        if !prim.is_valid() {
            return Path::empty();
        }

        // Try PrimIndex-based traversal (handles composition properly).
        if let Some(prim_index) = prim.prim_index() {
            let nodes = prim_index.nodes();
            let root_node = prim_index.root_node();

            for node in &nodes {
                if node.arc_type() != usd_pcp::ArcType::Specialize {
                    continue;
                }

                // Only consider direct children of root node (implied arcs).
                if node.parent_node() != root_node {
                    continue;
                }

                // Skip nodes that cross a reference arc boundary.
                // Reference mappings never map the absolute root path </>.
                let map = node.map_to_parent();
                if map.map_source_to_target(&Path::absolute_root()).is_none() {
                    continue;
                }

                // Verify the target is actually a material.
                let path = node.path();
                if let Some(stage) = prim.stage() {
                    if let Some(target_prim) = stage.get_prim_at_path(&path) {
                        if Material::new(target_prim).is_valid() {
                            return path;
                        }
                    }
                }
            }
        }

        // Fallback: read specializes metadata directly.
        // Handles single-layer stages where PrimIndex may not have
        // specialize nodes populated yet.
        Self::find_base_material_from_metadata(prim)
    }

    /// Fallback for finding base material from specializes metadata.
    /// Used when PrimIndex is unavailable (uncomposed single-layer stage).
    /// Checks explicit, prepended, and appended items (C++ SetSpecializes uses explicit list).
    fn find_base_material_from_metadata(prim: &Prim) -> Path {
        let specializes_token = usd_tf::Token::new("specializes");
        if let Some(list_op) = prim.get_metadata::<usd_sdf::list_op::PathListOp>(&specializes_token)
        {
            // SetSpecializes writes to explicit list; add_specialize writes to appended.
            // Check all three to be robust.
            let candidates = list_op
                .get_explicit_items()
                .iter()
                .chain(list_op.get_prepended_items().iter())
                .chain(list_op.get_appended_items().iter());
            for path in candidates {
                if let Some(stage) = prim.stage() {
                    if let Some(target_prim) = stage.get_prim_at_path(path) {
                        if Material::new(target_prim).is_valid() {
                            return path.clone();
                        }
                    }
                }
            }
        }
        Path::empty()
    }

    /// Public static version matching C++ signature.
    pub fn find_base_material_path_in_prim_index(prim: &Prim) -> Path {
        Self::find_base_material_path_in_prim_index_impl(prim)
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Helper to get output name for render context.
    ///
    /// Returns the output base name WITHOUT the "outputs:" prefix.
    /// For universal context returns just the terminal name (e.g. "surface").
    /// For specific context returns "renderContext:terminalName" (e.g. "ri:surface").
    fn _get_output_name_for_render_context(terminal_name: &Token, render_context: &Token) -> Token {
        if render_context.as_str().is_empty() {
            // Universal render context - just use terminal name
            terminal_name.clone()
        } else {
            // Specific render context - use format "renderContext:terminalName"
            Token::new(&format!(
                "{}:{}",
                render_context.as_str(),
                terminal_name.as_str()
            ))
        }
    }

    /// Helper to get outputs for terminal name.
    ///
    /// Returns all outputs matching the terminal name across all render contexts.
    /// The universal render context output is placed first (C++ material.cpp:575-601).
    fn _get_outputs_for_terminal_name(
        node_graph: &NodeGraph,
        terminal_name: &Token,
    ) -> Vec<Output> {
        let mut outputs = Vec::new();

        // First: explicitly get the universal output and place it at front
        let universal_output_name = Self::_get_output_name_for_render_context(
            terminal_name,
            &tokens().universal_render_context,
        );
        let universal_output = node_graph.get_output(&universal_output_name);
        if universal_output.is_defined() {
            outputs.push(universal_output);
        }

        // Then: iterate all outputs, check render-context-specific ones via
        // TokenizeIdentifier (split by ':'), require >= 2 parts (C++ material.cpp:589-593)
        for output in node_graph.get_outputs(true) {
            let base_name = output.get_base_name();
            let base_str = base_name.as_str();
            let parts: Vec<&str> = base_str.split(':').collect();
            if parts.len() >= 2 && parts.last() == Some(&terminal_name.as_str()) {
                outputs.push(output);
            }
        }

        outputs
    }

    /// Helper to compute named output shader.
    /// Compute the named output sources (value-producing attributes).
    ///
    /// Matches C++ `_ComputeNamedOutputSources()`: iterates the context vector,
    /// calls `GetValueProducingAttributes(output, /*shaderOutputsOnly=*/true)`
    /// to recursively follow connections through NodeGraphs, and falls back to
    /// universalRenderContext if not already tried.
    fn _compute_named_output_sources(
        node_graph: &NodeGraph,
        base_name: &Token,
        context_vector: &[Token],
    ) -> AttributeVector {
        let universal = &tokens().universal_render_context;
        let mut universal_computed = false;

        for render_context in context_vector {
            if render_context == universal {
                universal_computed = true;
            }

            let output_name = Self::_get_output_name_for_render_context(base_name, render_context);
            let output = node_graph.get_output(&output_name);

            if output.is_defined() {
                // C++ material.cpp:500-503: if universal output exists but is NOT authored,
                // this means "explicitly no terminal here" — return empty immediately.
                // If output exists and IS authored (or is not universal), continue.
                if render_context == universal {
                    let is_authored = output
                        .get_attr()
                        .map(|a| a.as_property().is_authored())
                        .unwrap_or(false);
                    if !is_authored {
                        // C++ material.cpp:500-503: `return {}` from the entire function.
                        // An unAuthored universal output means "explicitly no terminal here".
                        // This prevents falling through to other contexts — return empty now.
                        return Vec::new();
                    }
                }

                // Recursively follow connections to find actual shader outputs
                let value_attrs = Utils::get_value_producing_attributes_output(&output, true);

                if !value_attrs.is_empty() {
                    return value_attrs;
                }
            }
        }

        // Fallback to universal render context if not already tried
        if !universal_computed {
            let universal_output_name =
                Self::_get_output_name_for_render_context(base_name, universal);
            let universal_output = node_graph.get_output(&universal_output_name);
            // TF_VERIFY equivalent: warn if universal output doesn't exist on material
            if !universal_output.is_defined() {
                eprintln!(
                    "TF_VERIFY failed: Universal render context output '{}:{}' not found on \
                     material prim '{}'. Material is missing its terminal output.",
                    universal.as_str(),
                    base_name.as_str(),
                    node_graph.path()
                );
            } else {
                return Utils::get_value_producing_attributes_output(&universal_output, true);
            }
        }

        Vec::new()
    }

    /// Helper to compute named output shader.
    ///
    /// Matches C++ `_ComputeNamedOutputShader()`: calls `_compute_named_output_sources`
    /// then extracts the shader from the first value-producing attribute.
    fn _compute_named_output_shader(
        node_graph: &NodeGraph,
        base_name: &Token,
        context_vector: &[Token],
        source_name: &mut Token,
        source_type: &mut AttributeType,
    ) -> Shader {
        let value_attrs =
            Self::_compute_named_output_sources(node_graph, base_name, context_vector);

        if value_attrs.is_empty() {
            return Shader::invalid();
        }

        // Extract source name and type from the first value attribute
        let attr = &value_attrs[0];
        let (src_name, src_type) = Utils::get_base_name_and_type(&attr.name());
        *source_name = src_name;
        *source_type = src_type;

        // Get the prim that owns this attribute via stage
        if let Some(stage) = attr.stage() {
            let prim_path = attr.prim_path();
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                return Shader::new(prim);
            }
        }

        Shader::invalid()
    }
}

impl From<NodeGraph> for Material {
    fn from(node_graph: NodeGraph) -> Self {
        Self { node_graph }
    }
}

impl PartialEq for Material {
    fn eq(&self, other: &Self) -> bool {
        self.get_prim().path() == other.get_prim().path()
    }
}

impl Eq for Material {}

impl std::hash::Hash for Material {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.get_prim().path().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;
    use usd_core::stage::Stage;
    use usd_sdf::Path;
    use usd_tf::Token;

    /// Helper: create an in-memory stage with a Material at /Mat.
    fn setup_stage_with_material() -> (Arc<Stage>, Material) {
        let stage =
            Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");
        // define_prim directly to ensure prim is created
        let prim = stage.define_prim("/Mat", "Material").expect("define prim");
        assert_eq!(prim.type_name().as_str(), "Material");
        let mat = Material::new(prim);
        assert!(
            mat.is_valid(),
            "Material not valid; type_name={}",
            mat.get_prim().type_name().as_str()
        );
        (stage, mat)
    }

    // ====================================================================
    // Basic construction / validity
    // ====================================================================

    #[test]
    fn test_material_define_and_validity() {
        let (_stage, mat) = setup_stage_with_material();
        assert!(mat.is_valid());
        assert_eq!(mat.path().get_string(), "/Mat");
    }

    #[test]
    fn test_material_invalid() {
        let mat = Material::invalid();
        assert!(!mat.is_valid());
    }

    #[test]
    fn test_material_get() {
        let (stage, _mat) = setup_stage_with_material();
        let mat2 = Material::get(&stage, &Path::from_string("/Mat").unwrap());
        assert!(mat2.is_valid());
    }

    // ====================================================================
    // Schema attribute names
    // ====================================================================

    #[test]
    fn test_schema_attribute_names() {
        let names = Material::get_schema_attribute_names(false);
        assert_eq!(names.len(), 3);
        let name_strs: Vec<&str> = names.iter().map(|t| t.as_str()).collect();
        assert!(name_strs.contains(&"outputs:surface"));
        assert!(name_strs.contains(&"outputs:displacement"));
        assert!(name_strs.contains(&"outputs:volume"));
    }

    // ====================================================================
    // Terminal Output creation / retrieval (universal context)
    // ====================================================================

    #[test]
    fn test_create_surface_output_universal() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        let output = mat.create_surface_output(&universal);
        assert!(output.is_defined());
        assert_eq!(output.get_full_name().as_str(), "outputs:surface");
    }

    #[test]
    fn test_create_displacement_output_universal() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        let output = mat.create_displacement_output(&universal);
        assert!(output.is_defined());
        assert_eq!(output.get_full_name().as_str(), "outputs:displacement");
    }

    #[test]
    fn test_create_volume_output_universal() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        let output = mat.create_volume_output(&universal);
        assert!(output.is_defined());
        assert_eq!(output.get_full_name().as_str(), "outputs:volume");
    }

    // ====================================================================
    // Render-context-specific outputs
    // ====================================================================

    #[test]
    fn test_create_surface_output_ri_context() {
        let (_stage, mat) = setup_stage_with_material();
        let ri = Token::new("ri");
        let output = mat.create_surface_output(&ri);
        assert!(output.is_defined());
        // "ri" context -> output name is "ri:surface"
        // ConnectableAPI prepends "outputs:", so full name = "outputs:ri:surface"
        assert_eq!(output.get_full_name().as_str(), "outputs:ri:surface");
        assert_eq!(output.get_base_name().as_str(), "ri:surface");
    }

    #[test]
    fn test_get_surface_output_ri_context() {
        let (_stage, mat) = setup_stage_with_material();
        let ri = Token::new("ri");
        // Create first
        let _created = mat.create_surface_output(&ri);
        // Now retrieve
        let output = mat.get_surface_output(&ri);
        assert!(output.is_defined());
        assert_eq!(output.get_full_name().as_str(), "outputs:ri:surface");
    }

    // ====================================================================
    // get_*_outputs() - collects across render contexts
    // ====================================================================

    #[test]
    fn test_get_surface_outputs_multiple_contexts() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        let ri = Token::new("ri");
        let glslfx = Token::new("glslfx");

        mat.create_surface_output(&universal);
        mat.create_surface_output(&ri);
        mat.create_surface_output(&glslfx);
        // Also create a displacement output to ensure it's not included
        mat.create_displacement_output(&universal);

        let outputs = mat.get_surface_outputs();
        // Should have 3 surface outputs
        assert_eq!(
            outputs.len(),
            3,
            "Expected 3 surface outputs, got {}",
            outputs.len()
        );

        // Universal should be first
        assert_eq!(outputs[0].get_full_name().as_str(), "outputs:surface");
    }

    // ====================================================================
    // _get_output_name_for_render_context helper
    // ====================================================================

    #[test]
    fn test_output_name_universal_context() {
        let universal = tokens().universal_render_context.clone();
        let name = Material::_get_output_name_for_render_context(&tokens().surface, &universal);
        assert_eq!(name.as_str(), "surface");
    }

    #[test]
    fn test_output_name_specific_context() {
        let ri = Token::new("ri");
        let name = Material::_get_output_name_for_render_context(&tokens().surface, &ri);
        assert_eq!(name.as_str(), "ri:surface");
    }

    #[test]
    fn test_output_name_displacement_context() {
        let glslfx = Token::new("glslfx");
        let name = Material::_get_output_name_for_render_context(&tokens().displacement, &glslfx);
        assert_eq!(name.as_str(), "glslfx:displacement");
    }

    // ====================================================================
    // ComputeSurfaceSource - no connection returns invalid
    // ====================================================================

    #[test]
    fn test_compute_surface_source_no_connection() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        mat.create_surface_output(&universal);

        let mut source_name = Token::new("");
        let mut source_type = AttributeType::Invalid;
        let shader = mat.compute_surface_source(&[universal], &mut source_name, &mut source_type);
        // No connection established, so shader is invalid
        assert!(!shader.is_valid());
    }

    // ====================================================================
    // ComputeSurfaceSource - with connection
    // ====================================================================

    #[test]
    fn test_compute_surface_source_with_connection() {
        let (stage, mat) = setup_stage_with_material();

        // Create a shader prim
        let shader_prim = Shader::define(&stage, &Path::from_string("/Mat/PBR").unwrap());
        assert!(shader_prim.is_valid());

        // Create the shader's output
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        let shader_output = shader_prim.create_output(&Token::new("surface"), &token_type);
        assert!(shader_output.is_defined());

        // Create material's surface output and connect it to the shader output
        let universal = tokens().universal_render_context.clone();
        let mat_output = mat.create_surface_output(&universal);
        assert!(mat_output.is_defined());

        // Connect material output -> shader output
        let connected = mat_output
            .connect_to_source_path(&Path::from_string("/Mat/PBR.outputs:surface").unwrap());
        assert!(
            connected,
            "Failed to connect material output to shader output"
        );

        // Now compute surface source
        let mut source_name = Token::new("");
        let mut source_type = AttributeType::Invalid;
        let result = mat.compute_surface_source(&[universal], &mut source_name, &mut source_type);

        assert!(
            result.is_valid(),
            "Expected valid shader from compute_surface_source"
        );
        assert_eq!(source_name.as_str(), "surface");
        assert_eq!(source_type, AttributeType::Output);
    }

    // ====================================================================
    // ComputeSurfaceSource with render context fallback
    // ====================================================================

    #[test]
    fn test_compute_surface_source_context_fallback() {
        let (stage, mat) = setup_stage_with_material();

        // Create shader and connect to universal context only
        let shader_prim = Shader::define(&stage, &Path::from_string("/Mat/PBR").unwrap());
        let token_type = usd_sdf::ValueTypeRegistry::instance().find_type("token");
        shader_prim.create_output(&Token::new("surface"), &token_type);

        let universal = tokens().universal_render_context.clone();
        let mat_output = mat.create_surface_output(&universal);
        mat_output.connect_to_source_path(&Path::from_string("/Mat/PBR.outputs:surface").unwrap());

        // Ask for "ri" context, which doesn't exist - should fall back to universal
        let ri = Token::new("ri");
        let mut source_name = Token::new("");
        let mut source_type = AttributeType::Invalid;
        let result = mat.compute_surface_source(&[ri], &mut source_name, &mut source_type);

        assert!(result.is_valid(), "Expected fallback to universal context");
        assert_eq!(source_name.as_str(), "surface");
    }

    // ====================================================================
    // ComputeDisplacementSource
    // ====================================================================

    #[test]
    fn test_compute_displacement_source_no_connection() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        mat.create_displacement_output(&universal);

        let mut source_name = Token::new("");
        let mut source_type = AttributeType::Invalid;
        let shader =
            mat.compute_displacement_source(&[universal], &mut source_name, &mut source_type);
        assert!(!shader.is_valid());
    }

    // ====================================================================
    // ComputeVolumeSource
    // ====================================================================

    #[test]
    fn test_compute_volume_source_no_connection() {
        let (_stage, mat) = setup_stage_with_material();
        let universal = tokens().universal_render_context.clone();
        mat.create_volume_output(&universal);

        let mut source_name = Token::new("");
        let mut source_type = AttributeType::Invalid;
        let shader = mat.compute_volume_source(&[universal], &mut source_name, &mut source_type);
        assert!(!shader.is_valid());
    }

    // ====================================================================
    // Equality and hashing
    // ====================================================================

    #[test]
    fn test_material_equality() {
        let (stage, mat) = setup_stage_with_material();
        let mat2 = Material::get(&stage, &Path::from_string("/Mat").unwrap());
        assert_eq!(mat, mat2);
    }

    #[test]
    fn test_material_from_node_graph() {
        let (stage, _mat) = setup_stage_with_material();
        let prim = stage
            .get_prim_at_path(&Path::from_string("/Mat").unwrap())
            .unwrap();
        let ng = NodeGraph::new(prim);
        let mat2 = Material::from_node_graph(ng);
        assert!(mat2.is_valid());
    }

    // ====================================================================
    // get_base_material_path / set_base_material_path / has_base_material
    // ====================================================================

    #[test]
    fn test_no_base_material_by_default() {
        let (_stage, mat) = setup_stage_with_material();
        assert!(!mat.has_base_material());
        assert!(mat.get_base_material_path().is_empty());
        assert!(!mat.get_base_material().is_valid());
    }

    #[test]
    fn test_set_base_material_path_and_has_base() {
        let (stage, mat) = setup_stage_with_material();
        // Define a second material to use as base
        let base_prim = stage
            .define_prim("/BaseMat", "Material")
            .expect("base prim");
        let base_mat = Material::new(base_prim);
        assert!(base_mat.is_valid());

        mat.set_base_material(&base_mat);
        // After setting, has_base_material should be true
        assert!(mat.has_base_material());
        assert!(!mat.get_base_material_path().is_empty());
    }

    #[test]
    fn test_clear_base_material() {
        let (stage, mat) = setup_stage_with_material();
        let base_prim = stage.define_prim("/BaseMat2", "Material").expect("prim");
        let base_mat = Material::new(base_prim);
        mat.set_base_material(&base_mat);
        mat.clear_base_material();
        // Cleared: no base material
        assert!(!mat.has_base_material());
    }

    #[test]
    fn test_set_invalid_base_material_clears() {
        let (stage, mat) = setup_stage_with_material();
        // First set a valid base
        let base_prim = stage.define_prim("/BaseMat3", "Material").expect("prim");
        mat.set_base_material(&Material::new(base_prim));
        // Now set invalid -> should clear
        mat.set_base_material(&Material::invalid());
        assert!(!mat.has_base_material());
    }
}
