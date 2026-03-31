//! Asset Localization Delegate - interface for asset path processing.
//!
//! Defines the interface between LocalizationContext and localization clients.
//! Provides callbacks for processing sublayers, references, payloads, and
//! asset-valued attributes during dependency traversal.
//!
//! # Delegate Types
//!
//! - [`LocalizationDelegate`] - Base trait for all delegates
//! - [`WritableLocalizationDelegate`] - Modifies layers during processing
//! - [`ReadOnlyLocalizationDelegate`] - Read-only traversal
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUtils/assetLocalizationDelegate.h` and `.cpp`

use std::collections::HashMap;
use std::sync::Arc;

use usd_sdf::{AssetPath, Layer, Path, PrimSpec};
use usd_tf::Token;
use usd_vt::Value;

use super::user_processing_func::DependencyInfo;

/// Enum representing the type of dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyType {
    /// Reference to another layer/prim
    Reference,
    /// Sublayer composition
    Sublayer,
    /// Payload composition
    Payload,
    /// Clip template asset path
    ClipTemplateAssetPath,
}

/// Processing function signature for delegates.
///
/// Called for each discovered asset path during localization.
/// Returns updated dependency info (possibly with modified path).
pub type ProcessingFunc =
    Box<dyn Fn(&Arc<Layer>, &DependencyInfo, DependencyType) -> DependencyInfo + Send + Sync>;

/// Trait defining the interface for localization delegates.
///
/// Methods return a vector of additional asset paths that should be
/// enqueued for traversal and processing by the localization context.
pub trait LocalizationDelegate: Send + Sync {
    /// Process sublayer references in a layer.
    fn process_sublayers(&mut self, _layer: &Arc<Layer>) -> Vec<String> {
        Vec::new()
    }

    /// Process payload references on a prim.
    fn process_payloads(&mut self, _layer: &Arc<Layer>, _prim_spec: &PrimSpec) -> Vec<String> {
        Vec::new()
    }

    /// Process reference compositions on a prim.
    fn process_references(&mut self, _layer: &Arc<Layer>, _prim_spec: &PrimSpec) -> Vec<String> {
        Vec::new()
    }

    /// Signal start of processing a new value.
    fn begin_process_value(&mut self, _layer: &Arc<Layer>, _val: &Value) {}

    /// Process a single asset path value.
    fn process_value_path(
        &mut self,
        _layer: &Arc<Layer>,
        _key_path: &str,
        _authored_path: &str,
        _dependencies: &[String],
        _processing_metadata: bool,
        _processing_dictionary: bool,
    ) -> Vec<String> {
        Vec::new()
    }

    /// Process an asset path array element.
    fn process_value_path_array_element(
        &mut self,
        _layer: &Arc<Layer>,
        _key_path: &str,
        _authored_path: &str,
        _dependencies: &[String],
    ) -> Vec<String> {
        Vec::new()
    }

    /// Signal end of processing an asset path array.
    fn end_processing_value_path_array(&mut self, _layer: &Arc<Layer>, _key_path: &str) {}

    /// Signal end of processing a time sample value.
    fn end_process_time_sample_value(
        &mut self,
        _layer: &Arc<Layer>,
        _path: &Path,
        _time: f64,
        _val: &Value,
    ) {
    }

    /// Signal end of processing a value.
    fn end_process_value(&mut self, _layer: &Arc<Layer>, _path: &Path, _key: &Token, _val: &Value) {
    }

    /// Process a clip template asset path.
    fn process_clip_template_asset_path(
        &mut self,
        _layer: &Arc<Layer>,
        _prim_spec: &PrimSpec,
        _clip_set_name: &str,
        _template_asset_path: &str,
        _dependencies: Vec<String>,
    ) -> Vec<String> {
        Vec::new()
    }
}

/// Cache for processed paths to avoid reprocessing.
pub struct ProcessedPathCache {
    cached_paths: HashMap<(String, String), String>,
    processing_func: ProcessingFunc,
}

impl ProcessedPathCache {
    /// Create a new path cache with the given processing function.
    pub fn new(processing_func: ProcessingFunc) -> Self {
        Self {
            cached_paths: HashMap::new(),
            processing_func,
        }
    }

    /// Get processed info, using cache if available.
    pub fn get_processed_info(
        &mut self,
        layer: &Arc<Layer>,
        dependency_info: &DependencyInfo,
        dependency_type: DependencyType,
    ) -> DependencyInfo {
        let key = (
            layer.identifier().to_string(),
            dependency_info.get_asset_path().to_string(),
        );

        if let Some(cached) = self.cached_paths.get(&key) {
            return DependencyInfo::new(cached);
        }

        let result = (self.processing_func)(layer, dependency_info, dependency_type);
        self.cached_paths
            .insert(key, result.get_asset_path().to_string());
        result
    }
}

/// Writable delegate that modifies layers during processing.
///
/// Invokes a user-supplied processing function on every asset path.
/// Updates paths with returned values, removing empty paths from layers.
pub struct WritableLocalizationDelegate {
    path_cache: ProcessedPathCache,
    current_value_path: Option<AssetPath>,
    current_path_array: Vec<AssetPath>,
    edit_layers_in_place: bool,
    keep_empty_paths_in_arrays: bool,
    writable_layer_map: HashMap<String, Arc<Layer>>,
}

impl WritableLocalizationDelegate {
    /// Create a new writable delegate with the given processing function.
    pub fn new(processing_func: ProcessingFunc) -> Self {
        Self {
            path_cache: ProcessedPathCache::new(processing_func),
            current_value_path: None,
            current_path_array: Vec::new(),
            edit_layers_in_place: false,
            keep_empty_paths_in_arrays: false,
            writable_layer_map: HashMap::new(),
        }
    }

    /// Set whether to edit layers in place.
    pub fn set_edit_layers_in_place(&mut self, edit: bool) {
        self.edit_layers_in_place = edit;
    }

    /// Set whether to keep empty paths in arrays.
    pub fn set_keep_empty_paths_in_arrays(&mut self, keep: bool) {
        self.keep_empty_paths_in_arrays = keep;
    }

    /// Get the layer used for writing changes to the source layer.
    pub fn get_layer_used_for_writing(&self, layer: &Arc<Layer>) -> Option<Arc<Layer>> {
        self.writable_layer_map.get(layer.identifier()).cloned()
    }

    /// Clear the reference to the writable layer for a source layer.
    pub fn clear_layer_used_for_writing(&mut self, layer: &Arc<Layer>) {
        self.writable_layer_map.remove(layer.identifier());
    }
}

impl LocalizationDelegate for WritableLocalizationDelegate {
    fn process_sublayers(&mut self, layer: &Arc<Layer>) -> Vec<String> {
        let sublayers = layer.sublayer_paths();
        let mut deps = Vec::new();

        for path in sublayers {
            let dep_info = DependencyInfo::new(&path);
            let processed =
                self.path_cache
                    .get_processed_info(layer, &dep_info, DependencyType::Sublayer);
            if !processed.get_asset_path().is_empty() {
                deps.push(processed.get_asset_path().to_string());
            }
        }

        deps
    }

    fn begin_process_value(&mut self, _layer: &Arc<Layer>, _val: &Value) {
        self.current_value_path = None;
        self.current_path_array.clear();
    }

    fn process_value_path(
        &mut self,
        layer: &Arc<Layer>,
        _key_path: &str,
        authored_path: &str,
        _dependencies: &[String],
        _processing_metadata: bool,
        _processing_dictionary: bool,
    ) -> Vec<String> {
        let dep_info = DependencyInfo::new(authored_path);
        let processed =
            self.path_cache
                .get_processed_info(layer, &dep_info, DependencyType::Reference);

        self.current_value_path = Some(AssetPath::new(processed.get_asset_path()));

        if !processed.get_asset_path().is_empty() {
            vec![processed.get_asset_path().to_string()]
        } else {
            Vec::new()
        }
    }

    fn process_value_path_array_element(
        &mut self,
        layer: &Arc<Layer>,
        _key_path: &str,
        authored_path: &str,
        _dependencies: &[String],
    ) -> Vec<String> {
        let dep_info = DependencyInfo::new(authored_path);
        let processed =
            self.path_cache
                .get_processed_info(layer, &dep_info, DependencyType::Reference);

        let path = processed.get_asset_path();
        if !path.is_empty() || self.keep_empty_paths_in_arrays {
            self.current_path_array.push(AssetPath::new(path));
        }

        if !path.is_empty() {
            vec![path.to_string()]
        } else {
            Vec::new()
        }
    }
}

/// Read-only delegate for traversing without modifying layers.
pub struct ReadOnlyLocalizationDelegate {
    path_cache: ProcessedPathCache,
}

impl ReadOnlyLocalizationDelegate {
    /// Create a new read-only delegate with the given processing function.
    pub fn new(processing_func: ProcessingFunc) -> Self {
        Self {
            path_cache: ProcessedPathCache::new(processing_func),
        }
    }
}

impl LocalizationDelegate for ReadOnlyLocalizationDelegate {
    fn process_sublayers(&mut self, layer: &Arc<Layer>) -> Vec<String> {
        let sublayers = layer.sublayer_paths();
        let mut deps = Vec::new();

        for path in sublayers {
            let dep_info = DependencyInfo::new(&path);
            let processed =
                self.path_cache
                    .get_processed_info(layer, &dep_info, DependencyType::Sublayer);
            if !processed.get_asset_path().is_empty() {
                deps.push(processed.get_asset_path().to_string());
            }
        }

        deps
    }

    fn process_value_path(
        &mut self,
        layer: &Arc<Layer>,
        _key_path: &str,
        authored_path: &str,
        _dependencies: &[String],
        _processing_metadata: bool,
        _processing_dictionary: bool,
    ) -> Vec<String> {
        let dep_info = DependencyInfo::new(authored_path);
        let processed =
            self.path_cache
                .get_processed_info(layer, &dep_info, DependencyType::Reference);

        if !processed.get_asset_path().is_empty() {
            vec![processed.get_asset_path().to_string()]
        } else {
            Vec::new()
        }
    }

    fn process_value_path_array_element(
        &mut self,
        layer: &Arc<Layer>,
        _key_path: &str,
        authored_path: &str,
        _dependencies: &[String],
    ) -> Vec<String> {
        let dep_info = DependencyInfo::new(authored_path);
        let processed =
            self.path_cache
                .get_processed_info(layer, &dep_info, DependencyType::Reference);

        if !processed.get_asset_path().is_empty() {
            vec![processed.get_asset_path().to_string()]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_type() {
        assert_ne!(DependencyType::Reference, DependencyType::Sublayer);
        assert_eq!(DependencyType::Payload, DependencyType::Payload);
    }
}
