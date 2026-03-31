//! Fix broken Pixar schemas in USD layers.
//!
//! Port of pxr/usd/usdUtils/fixBrokenPixarSchemas.py
//!
//! Applies fixes for common schema migration issues:
//! - MaterialBindingAPI: add API to prims with material:binding but no API
//! - SkelBindingAPI: add API to prims with skel properties but no API
//! - UpAxis: set upAxis metadata if missing
//! - CoordSysAPI: migrate old coordSys:name to CoordSysAPI:name encoding
//! - RenderSettingsTerminalsAPI: convert attribute connections to relationships

use std::sync::atomic::{AtomicBool, Ordering};
use usd_geom::metrics::get_fallback_up_axis;
use usd_geom::tokens::usd_geom_tokens;
use usd_sdf::{
    LayerHandle, Path, PathListOp, TokenListOp, Variability, copy_spec,
    create_relationship_in_layer,
};
use usd_skel::tokens;
use usd_tf::Token;
use usd_vt::Value;

/// Fixer for broken Pixar schemas in a USD layer.
///
/// Applies schema migration fixes. Call `apply_all()` then check `is_layer_updated()`.
pub struct FixBrokenPixarSchemas {
    layer: LayerHandle,
    layer_updated: AtomicBool,
}

// SkelBindingAPI property names (from UsdSkelBindingAPI schema)
const SKEL_BINDING_API_PROPS: &[&str] = &[
    "primvars:skel:skinningMethod",
    "primvars:skel:geomBindTransform",
    "skel:joints",
    "primvars:skel:jointIndices",
    "primvars:skel:jointWeights",
    "skel:skeleton",
    "skel:animationSource",
    "skel:blendShapes",
    "skel:blendShapeTargets",
];

// Terminal attribute names in RenderSettings that should become relationships.
const RENDER_TERMINAL_ATTRS: &[&str] = &[
    "outputs:ri:displayFilters",
    "outputs:ri:sampleFilters",
    "outputs:ri:integrator",
];

static API_SCHEMAS: std::sync::OnceLock<Token> = std::sync::OnceLock::new();
fn api_schemas_token() -> Token {
    API_SCHEMAS.get_or_init(|| Token::new("apiSchemas")).clone()
}

impl FixBrokenPixarSchemas {
    /// Create a fixer for the given layer.
    pub fn new(layer: LayerHandle) -> Self {
        Self {
            layer,
            layer_updated: AtomicBool::new(false),
        }
    }

    /// Returns true if any fix was applied.
    pub fn is_layer_updated(&self) -> bool {
        self.layer_updated.load(Ordering::SeqCst)
    }

    fn mark_updated(&self) {
        self.layer_updated.store(true, Ordering::SeqCst);
    }

    /// Add an API schema to the apiSchemas list op (no-op if already present).
    fn apply_api(&self, list_op: &TokenListOp, api_schema: &str) -> TokenListOp {
        let token = Token::new(api_schema);
        if list_op.has_item(&token) {
            return list_op.clone();
        }

        let mut new_op = list_op.clone();
        if list_op.is_explicit() {
            let mut items = list_op.get_explicit_items().to_vec();
            items.push(token);
            let _ = new_op.set_explicit_items(items);
        } else {
            let mut items = list_op.get_prepended_items().to_vec();
            items.push(token);
            let _ = new_op.set_prepended_items(items);
        }
        new_op
    }

    /// FixupMaterialBindingAPI: add MaterialBindingAPI to prims with material:binding.
    pub fn fixup_material_binding_api(&self) {
        let root = Path::absolute_root();
        let callback = |path: &Path| {
            if !path.is_prim_path() {
                return;
            }
            let Some(prim) = self.layer.get_prim_at_path(path) else {
                return;
            };

            let has_material_binding = prim
                .properties()
                .iter()
                .any(|p| p.spec().path().get_name().starts_with("material:binding"));

            if !has_material_binding {
                return;
            }

            let api_schemas_key = api_schemas_token();
            let current: TokenListOp = self
                .layer
                .get_field(path, &api_schemas_key)
                .and_then(|v| v.downcast_clone::<TokenListOp>())
                .unwrap_or_default();

            if current
                .get_applied_items()
                .iter()
                .any(|t| t == "MaterialBindingAPI")
            {
                return;
            }

            self.mark_updated();
            let new_op = self.apply_api(&current, "MaterialBindingAPI");
            self.layer
                .set_field(path, &api_schemas_key, Value::from(new_op));
        };
        self.traverse_prim_paths(&root, &callback);
    }

    /// FixupSkelBindingAPI: add SkelBindingAPI to prims with skel properties.
    pub fn fixup_skel_binding_api(&self) {
        let root = Path::absolute_root();
        let callback = |path: &Path| {
            if !path.is_prim_path() {
                return;
            }
            let Some(prim) = self.layer.get_prim_at_path(path) else {
                return;
            };

            let prop_names: Vec<String> = prim
                .properties()
                .iter()
                .map(|p| p.spec().path().get_name().to_string())
                .collect();

            let has_skel_prop = SKEL_BINDING_API_PROPS
                .iter()
                .any(|&name| prop_names.iter().any(|p| p == name));

            if !has_skel_prop {
                return;
            }

            let api_schemas_key = api_schemas_token();
            let current: TokenListOp = self
                .layer
                .get_field(path, &api_schemas_key)
                .and_then(|v| v.downcast_clone::<TokenListOp>())
                .unwrap_or_default();

            let has_api = current
                .get_applied_items()
                .iter()
                .any(|t| *t == tokens().skel_binding_api);
            if has_api {
                return;
            }

            self.mark_updated();
            let new_op = self.apply_api(&current, tokens().skel_binding_api.as_str());
            self.layer
                .set_field(path, &api_schemas_key, Value::from(new_op));
        };
        self.traverse_prim_paths(&root, &callback);
    }

    /// FixupUpAxis: set upAxis metadata on layer if missing.
    pub fn fixup_up_axis(&self) {
        let root = Path::absolute_root();
        let up_axis_token = usd_geom_tokens().up_axis.clone();
        if self.layer.get_field(&root, &up_axis_token).is_none() {
            self.mark_updated();
            let fallback = get_fallback_up_axis();
            self.layer
                .set_field(&root, &up_axis_token, Value::from_no_hash(fallback));
        }
    }

    /// FixupCoordSysAPI: migrate old-style `coordSys:name` relationships to
    /// the multi-apply `CoordSysAPI:name` encoding with `coordSys:name:binding`.
    ///
    /// Port of Python `FixupCoordSysAPI`.
    ///
    /// For each prim:
    /// 1. Old-style `coordSys:<name>` rel (no `:binding`) → copy to
    ///    `coordSys:<name>:binding`, apply `CoordSysAPI:<name>`, delete old rel.
    /// 2. New-style `coordSys:<name>:binding` without applied `CoordSysAPI:<name>`
    ///    → apply the missing API schema.
    pub fn fixup_coord_sys_api(&self) {
        let root = Path::absolute_root();
        let callback = |path: &Path| {
            if !path.is_prim_path() {
                return;
            }
            let Some(prim) = self.layer.get_prim_at_path(path) else {
                return;
            };

            let api_schemas_key = api_schemas_token();
            let mut api_list_op: TokenListOp = self
                .layer
                .get_field(path, &api_schemas_key)
                .and_then(|v| v.downcast_clone::<TokenListOp>())
                .unwrap_or_default();

            let rels = prim.relationships();
            let mut to_migrate: Vec<String> = Vec::new(); // old-style names
            let mut missing_api: Vec<String> = Vec::new(); // new-style without API

            for rel in &rels {
                let rel_name = rel.name();
                if !rel_name.starts_with("coordSys:") {
                    continue;
                }
                let colon_count = rel_name.chars().filter(|&c| c == ':').count();
                // Old encoding: exactly "coordSys:<name>" (1 colon after "coordSys:")
                if colon_count == 1 {
                    to_migrate.push(rel_name.clone());
                } else if rel_name.ends_with(":binding") && colon_count > 1 {
                    // New encoding: "coordSys:<name>:binding"
                    let instance_name = rel_name
                        .strip_prefix("coordSys:")
                        .unwrap_or("")
                        .strip_suffix(":binding")
                        .unwrap_or("");
                    let api_schema = format!("CoordSysAPI:{instance_name}");
                    if !api_list_op.has_item(&Token::new(&api_schema)) {
                        missing_api.push(api_schema);
                    }
                }
            }

            if to_migrate.is_empty() && missing_api.is_empty() {
                return;
            }

            self.mark_updated();

            // Migrate old-style relationships.
            for old_name in &to_migrate {
                let instance_name = old_name.strip_prefix("coordSys:").unwrap_or("");
                let new_name = format!("coordSys:{instance_name}:binding");
                let api_schema = format!("CoordSysAPI:{instance_name}");

                let old_path = path
                    .append_property(old_name)
                    .unwrap_or_else(|| path.clone());
                let new_path = path
                    .append_property(&new_name)
                    .unwrap_or_else(|| path.clone());

                // CopySpec: duplicate the old relationship spec under the new name.
                if let Some(arc) = self.layer.upgrade() {
                    copy_spec(&arc, &old_path, &arc, &new_path);
                    arc.delete_spec(&old_path);
                }

                // Apply the CoordSysAPI:<name> schema.
                api_list_op = self.apply_api(&api_list_op, &api_schema);
            }

            // Apply missing API schemas for already-new-style rels.
            for api_schema in &missing_api {
                api_list_op = self.apply_api(&api_list_op, api_schema);
            }

            // Persist updated apiSchemas.
            self.layer
                .set_field(path, &api_schemas_key, Value::from(api_list_op));
        };
        self.traverse_prim_paths(&root, &callback);
    }

    /// FixupRenderSettingsTerminalsAPI: convert RenderSettings terminal attributes
    /// (`outputs:ri:*`) that hold connection paths into proper relationships.
    ///
    /// Port of Python `FixupRenderSettingsTerminalsAPI`.
    ///
    /// For each `RenderSettings` prim with `PxrRenderTerminalsAPI`:
    /// - For each terminal attribute (`outputs:ri:displayFilters`, etc.) that has
    ///   connection paths → create a uniform relationship named `ri:displayFilters`
    ///   (strip the `outputs:` prefix) with targets = connected prim paths.
    /// - Delete the original attribute spec.
    pub fn fixup_render_settings_terminals_api(&self) {
        let root = Path::absolute_root();
        let callback = |path: &Path| {
            if !path.is_prim_path() {
                return;
            }
            let Some(prim) = self.layer.get_prim_at_path(path) else {
                return;
            };

            // Only apply to RenderSettings prims.
            if prim.type_name() != "RenderSettings" {
                return;
            }

            for attr_name in RENDER_TERMINAL_ATTRS {
                let Some(attr_path) = path.append_property(attr_name) else {
                    continue;
                };
                let Some(attr) = self.layer.get_attribute_at_path(&attr_path) else {
                    continue;
                };

                // Get connection paths from the attribute.
                let conn_list = attr.connection_paths_list();
                let conn_paths: Vec<Path> = conn_list.get_applied_items();
                if conn_paths.is_empty() {
                    continue;
                }

                self.mark_updated();

                // Relationship name: strip "outputs:" prefix.
                let rel_name = attr_name.strip_prefix("outputs:").unwrap_or(attr_name);
                let Some(rel_path) = path.append_property(rel_name) else {
                    continue;
                };

                // Create uniform, non-custom relationship.
                // Output attributes are conventionally non-custom.
                if let Some(mut rel) = create_relationship_in_layer(
                    &self.layer,
                    &rel_path,
                    Variability::Uniform,
                    false,
                ) {
                    // Targets = prim paths of each connection path.
                    let prim_paths: Vec<Path> =
                        conn_paths.iter().map(|cp| cp.get_prim_path()).collect();
                    let mut target_list = PathListOp::create_explicit(prim_paths);
                    // Deduplicate while preserving order.
                    let _ =
                        target_list.set_explicit_items(target_list.get_explicit_items().to_vec());
                    rel.set_target_path_list(target_list);
                }

                // Remove the original attribute spec from layer.
                if let Some(arc) = self.layer.upgrade() {
                    arc.delete_spec(&attr_path);
                }
                let _ = attr; // already consumed via connection_paths_list()
            }
        };
        self.traverse_prim_paths(&root, &callback);
    }

    /// Traverse all prim paths in the layer (depth-first).
    fn traverse_prim_paths<F>(&self, path: &Path, func: &F)
    where
        F: Fn(&Path),
    {
        if !self.layer.has_spec(path) {
            return;
        }
        func(path);

        if let Some(prim) = self.layer.get_prim_at_path(path) {
            for child in prim.name_children() {
                self.traverse_prim_paths(&child.path(), func);
            }
        }
    }

    /// Apply all fixers.
    pub fn apply_all(&self) {
        self.fixup_material_binding_api();
        self.fixup_skel_binding_api();
        self.fixup_up_axis();
        self.fixup_coord_sys_api();
        self.fixup_render_settings_terminals_api();
    }
}
