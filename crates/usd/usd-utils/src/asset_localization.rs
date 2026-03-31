//! Asset Localization Context - recursive dependency discovery.
//!
//! The `LocalizationContext` recursively computes an asset's dependencies
//! by traversing layers, sublayers, references, payloads, and asset-valued
//! attributes. As paths are discovered, they're handed to a delegate for
//! processing.
//!
//! # Reference Types
//!
//! - `CompositionOnly` - Only composition arcs (sublayers, references, payloads)
//! - `All` - All external references including asset-valued attributes
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUtils/assetLocalization.h` and `.cpp`

use std::collections::HashSet;
use std::sync::Arc;

use usd_core::Stage;
use usd_sdf::prim_spec::PrimSpec;
use usd_sdf::{Layer, Path};
use usd_tf::Token;
use usd_vt::value::Value;

use super::asset_localization_delegate::LocalizationDelegate;

/// Reference types to include in dependency discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    /// Only composition arcs (sublayers, references, payloads)
    CompositionOnly,
    /// All external references including asset-valued attributes
    All,
}

/// Context for recursive asset dependency discovery.
///
/// Traverses layers and hands discovered asset paths to a delegate
/// for processing.
pub struct LocalizationContext<D: LocalizationDelegate> {
    delegate: D,
    root_layer: Option<Arc<Layer>>,
    queue: Vec<String>,
    encountered_paths: HashSet<String>,
    ref_types_to_include: ReferenceType,
    recurse_layer_dependencies: bool,
    metadata_filtering_enabled: bool,
    resolve_udim_paths: bool,
    dependencies_to_skip: HashSet<String>,
}

impl<D: LocalizationDelegate> LocalizationContext<D> {
    /// Create a new localization context with the given delegate.
    pub fn new(delegate: D) -> Self {
        Self {
            delegate,
            root_layer: None,
            queue: Vec::new(),
            encountered_paths: HashSet::new(),
            ref_types_to_include: ReferenceType::All,
            recurse_layer_dependencies: true,
            metadata_filtering_enabled: false,
            resolve_udim_paths: true,
            dependencies_to_skip: HashSet::new(),
        }
    }

    /// Get the root layer of the asset.
    pub fn get_root_layer(&self) -> Option<&Arc<Layer>> {
        self.root_layer.as_ref()
    }

    /// Consume the context and return the delegate.
    ///
    /// Use this to access delegate state after processing is complete.
    pub fn into_delegate(self) -> D {
        self.delegate
    }

    /// Enable or disable metadata filtering.
    pub fn set_metadata_filtering_enabled(&mut self, enabled: bool) {
        self.metadata_filtering_enabled = enabled;
    }

    /// Set whether to recurse into layer dependencies.
    pub fn set_recurse_layer_dependencies(&mut self, recurse: bool) {
        self.recurse_layer_dependencies = recurse;
    }

    /// Set the reference types to include during processing.
    pub fn set_ref_types_to_include(&mut self, ref_types: ReferenceType) {
        self.ref_types_to_include = ref_types;
    }

    /// Set dependencies to skip during processing.
    pub fn set_dependencies_to_skip(&mut self, deps: Vec<String>) {
        self.dependencies_to_skip = deps.into_iter().collect();
    }

    /// Set whether to resolve UDIM paths.
    pub fn set_resolve_udim_paths(&mut self, resolve: bool) {
        self.resolve_udim_paths = resolve;
    }

    /// Begin recursive dependency analysis on the supplied layer.
    pub fn process(&mut self, layer: &Arc<Layer>) -> bool {
        self.root_layer = Some(layer.clone());
        self.encountered_paths
            .insert(layer.identifier().to_string());
        self.process_layer(layer);

        // Process queued dependencies
        while let Some(anchored_path) = self.queue.pop() {
            if !Stage::is_supported_file(&anchored_path) {
                continue;
            }

            if let Ok(dep_layer) = Layer::find_or_open(&anchored_path) {
                self.process_layer(&dep_layer);
            }
        }

        true
    }

    /// Enqueue a dependency for later processing.
    fn enqueue_dependency(&mut self, layer: &Arc<Layer>, asset_path: &str) {
        if !self.recurse_layer_dependencies || asset_path.is_empty() {
            return;
        }

        let anchored_path = layer.compute_absolute_path(asset_path);

        if self.encountered_paths.contains(&anchored_path)
            || self.dependencies_to_skip.contains(&anchored_path)
        {
            return;
        }

        self.encountered_paths.insert(anchored_path.clone());
        self.queue.push(anchored_path);
    }

    /// Enqueue multiple dependencies.
    fn enqueue_dependencies(&mut self, layer: &Arc<Layer>, deps: &[String]) {
        for dep in deps {
            self.enqueue_dependency(layer, dep);
        }
    }

    /// Process a single layer: sublayers + DFS over all prims.
    fn process_layer(&mut self, layer: &Arc<Layer>) {
        // Process sublayers first
        self.process_sublayers(layer);

        // DFS traversal of prim hierarchy starting at pseudo root
        let pseudo_root = layer.get_pseudo_root();
        let mut stack: Vec<PrimSpec> = vec![pseudo_root.clone()];

        while let Some(curr) = stack.pop() {
            // Metadata is processed even on pseudo root (layer metadata)
            self.process_metadata(layer, &curr);

            // Skip payload/reference/property on pseudo root
            if curr.path() != Path::absolute_root() {
                self.process_payloads(layer, &curr);
                self.process_references(layer, &curr);
                self.process_properties(layer, &curr);
            }

            // Traverse into variant sets
            let variant_sets = curr.variant_sets();
            for set_name in variant_sets.names() {
                if let Some(vset) = variant_sets.get(&set_name) {
                    for variant_spec in vset.variants() {
                        if let Some(prim) = variant_spec.prim_spec() {
                            stack.push(prim);
                        }
                    }
                }
            }

            // Traverse name children
            for child in curr.name_children() {
                stack.push(child);
            }
        }
    }

    /// Process sublayer references.
    fn process_sublayers(&mut self, layer: &Arc<Layer>) {
        let sublayers = layer.sublayer_paths();
        if sublayers.is_empty() {
            return;
        }

        for sublayer_path in &sublayers {
            self.enqueue_dependency(layer, sublayer_path);
        }

        let processed_deps = self.delegate.process_sublayers(layer);
        self.enqueue_dependencies(layer, &processed_deps);
    }

    /// Process metadata on a prim spec (including clip template paths).
    fn process_metadata(&mut self, layer: &Arc<Layer>, prim_spec: &PrimSpec) {
        if self.ref_types_to_include == ReferenceType::All {
            let path = prim_spec.path();
            let info_keys = layer.list_fields(&path);
            for key in &info_keys {
                if let Some(value) = layer.get_field(&path, key) {
                    if !Self::value_type_is_relevant(&value) {
                        continue;
                    }
                    self.delegate.begin_process_value(layer, &value);
                    self.process_asset_value(layer, &key.as_str().to_string(), &value, true, false);
                    self.delegate.end_process_value(layer, &path, key, &value);
                }
            }
        }

        // Process clip template asset paths
        let clips = prim_spec.clips();
        if clips.is_empty() {
            return;
        }

        for (clip_set_name, clip_set_val) in clips.iter() {
            if let Some(clip_dict) = clip_set_val.as_dictionary() {
                let template_key = "templateAssetPath".to_string();
                if let Some(template_val) = clip_dict.get(&template_key) {
                    if let Some(template_path) = template_val.get::<String>() {
                        if !template_path.is_empty() {
                            let deps = self.delegate.process_clip_template_asset_path(
                                layer,
                                prim_spec,
                                clip_set_name,
                                template_path,
                                Vec::new(),
                            );
                            self.enqueue_dependencies(layer, &deps);
                        }
                    }
                }
            }
        }
    }

    /// Process payloads on a prim spec.
    fn process_payloads(&mut self, layer: &Arc<Layer>, prim_spec: &PrimSpec) {
        if !prim_spec.has_payloads() {
            return;
        }

        let payloads = prim_spec.payloads_list();
        for payload in payloads.get_applied_items() {
            let asset = payload.asset_path();
            if !asset.is_empty() {
                self.enqueue_dependency(layer, asset);
            }
        }

        let processed_deps = self.delegate.process_payloads(layer, prim_spec);
        self.enqueue_dependencies(layer, &processed_deps);
    }

    /// Process references on a prim spec.
    fn process_references(&mut self, layer: &Arc<Layer>, prim_spec: &PrimSpec) {
        if !prim_spec.has_references() {
            return;
        }

        let references = prim_spec.references_list();
        for reference in references.get_applied_items() {
            let asset = reference.asset_path();
            if !asset.is_empty() {
                self.enqueue_dependency(layer, asset);
            }
        }

        let processed_deps = self.delegate.process_references(layer, prim_spec);
        self.enqueue_dependencies(layer, &processed_deps);
    }

    /// Process properties on a prim spec for asset-valued attributes.
    fn process_properties(&mut self, layer: &Arc<Layer>, prim_spec: &PrimSpec) {
        if self.ref_types_to_include == ReferenceType::CompositionOnly {
            return;
        }

        for property in prim_spec.properties() {
            let prop_path = property.spec().path();

            // Check property metadata fields
            let fields = layer.list_fields(&prop_path);
            let default_token = Token::new("default");
            let time_samples_token = Token::new("timeSamples");

            for field_key in &fields {
                if *field_key == default_token || *field_key == time_samples_token {
                    continue;
                }
                if let Some(value) = layer.get_field(&prop_path, field_key) {
                    if !Self::value_type_is_relevant(&value) {
                        continue;
                    }
                    self.delegate.begin_process_value(layer, &value);
                    self.process_asset_value(layer, &String::new(), &value, false, false);
                    self.delegate
                        .end_process_value(layer, &prop_path, field_key, &value);
                }
            }

            // Check if this is an asset-typed attribute
            if let Some(attr) = property.as_attribute() {
                let type_name = attr.type_name();
                let type_str = type_name.as_str();
                if type_str == "asset" || type_str == "asset[]" {
                    // Check default value
                    let default_val = attr.default_value();
                    if !default_val.is_empty() && Self::value_type_is_relevant(&default_val) {
                        self.delegate.begin_process_value(layer, &default_val);
                        self.process_asset_value(layer, &String::new(), &default_val, false, false);
                        self.delegate.end_process_value(
                            layer,
                            &prop_path,
                            &default_token,
                            &default_val,
                        );
                    }

                    // Check time samples
                    let times = layer.list_time_samples_for_path(&prop_path);
                    for t in times {
                        if let Some(ts_val) = layer.query_time_sample(&prop_path, t) {
                            if !Self::value_type_is_relevant(&ts_val) {
                                continue;
                            }
                            self.delegate.begin_process_value(layer, &ts_val);
                            self.process_asset_value(layer, &String::new(), &ts_val, false, false);
                            self.delegate
                                .end_process_time_sample_value(layer, &prop_path, t, &ts_val);
                        }
                    }
                }
            }
        }
    }

    /// Process an asset value (SdfAssetPath, VtArray<SdfAssetPath>, or VtDictionary).
    fn process_asset_value(
        &mut self,
        layer: &Arc<Layer>,
        key_path: &String,
        val: &Value,
        processing_metadata: bool,
        processing_dictionary: bool,
    ) {
        if self.should_filter_asset_path(key_path, processing_metadata) {
            return;
        }

        // Single asset path
        if let Some(asset_path) = val.get::<usd_sdf::AssetPath>() {
            let raw = asset_path.get_asset_path();
            let deps = self.delegate.process_value_path(
                layer,
                key_path,
                &raw,
                &[],
                processing_metadata,
                processing_dictionary,
            );
            self.enqueue_dependency(layer, &raw);
            self.enqueue_dependencies(layer, &deps);
            return;
        }

        // Array of asset paths
        if let Some(paths) = val.get::<Vec<usd_sdf::AssetPath>>() {
            if paths.is_empty() {
                return;
            }
            for ap in paths {
                let raw = ap.get_asset_path();
                let deps =
                    self.delegate
                        .process_value_path_array_element(layer, key_path, &raw, &[]);
                self.enqueue_dependency(layer, &raw);
                self.enqueue_dependencies(layer, &deps);
            }
            self.delegate
                .end_processing_value_path_array(layer, key_path);
            return;
        }

        // Dictionary - recurse into values
        if let Some(dict) = val.as_dictionary() {
            if dict.is_empty() {
                return;
            }
            for (k, v) in &dict {
                let dict_key = if key_path.is_empty() {
                    k.clone()
                } else {
                    format!("{}:{}", key_path, k)
                };
                self.process_asset_value(layer, &dict_key, v, processing_metadata, true);
            }
        }
    }

    /// Check if an asset path should be filtered out.
    fn should_filter_asset_path(&self, key: &str, processing_metadata: bool) -> bool {
        if !processing_metadata || !self.metadata_filtering_enabled {
            return false;
        }
        key == "assetInfo:identifier"
    }

    /// Check if a value type is relevant (holds asset paths or dictionaries).
    fn value_type_is_relevant(val: &Value) -> bool {
        val.get::<usd_sdf::AssetPath>().is_some()
            || val.get::<Vec<usd_sdf::AssetPath>>().is_some()
            || val.as_dictionary().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reference_type() {
        assert_ne!(ReferenceType::CompositionOnly, ReferenceType::All);
    }
}
