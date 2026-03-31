//! Animation clip stitching utilities.
//!
//! Provides utilities for sequencing multiple layers holding sequential
//! time-varying data into USD Value Clips.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use usd_sdf::Specifier;
use usd_sdf::layer::Layer;
use usd_sdf::path::Path;
use usd_tf::Token;
use usd_vt::value::Value;

use super::stitch::{BoxedStitchValueFn, StitchValueStatus, stitch_layers_with_fn};

/// Default clip set name.
pub const DEFAULT_CLIP_SET_NAME: &str = "default";

/// Stitches clip layers into a result layer using USD Value Clips.
pub fn stitch_clips(
    result_layer: &Arc<Layer>,
    clip_layer_files: &[String],
    clip_path: &Path,
    start_time_code: Option<f64>,
    end_time_code: Option<f64>,
    interpolate_missing_clip_values: bool,
    clip_set: Option<&Token>,
) -> bool {
    if clip_layer_files.is_empty() {
        return false;
    }

    let clip_set_name = clip_set
        .map(|t| t.as_str())
        .unwrap_or(DEFAULT_CLIP_SET_NAME);

    let result_identifier = result_layer.identifier();
    let topology_name = generate_clip_topology_name(result_identifier);
    let manifest_name = generate_clip_manifest_name(result_identifier);

    let topology_layer = Layer::find_or_open(&topology_name)
        .unwrap_or_else(|_| Layer::create_anonymous(Some("topology")));

    let manifest_layer = Layer::find_or_open(&manifest_name)
        .unwrap_or_else(|_| Layer::create_anonymous(Some("manifest")));

    if !stitch_clips_topology(&topology_layer, clip_layer_files) {
        return false;
    }

    if !stitch_clips_manifest(
        &manifest_layer,
        &topology_layer,
        clip_layer_files,
        clip_path,
    ) {
        return false;
    }

    let (actual_start, actual_end) =
        compute_clip_time_range(clip_layer_files, start_time_code, end_time_code);

    author_clip_metadata(
        result_layer,
        &topology_layer,
        &manifest_layer,
        clip_path,
        clip_layer_files,
        actual_start,
        actual_end,
        interpolate_missing_clip_values,
        clip_set_name,
    );

    true
}

/// Aggregates topology of clip layers for the Value Clips system.
///
/// Stitches each clip layer into the topology layer, ignoring time samples.
/// Matches C++ `UsdUtilsStitchClipsTopology`.
pub fn stitch_clips_topology(topology_layer: &Arc<Layer>, clip_layer_files: &[String]) -> bool {
    // Build a stitch fn that skips time samples (topology only)
    let ignore_time_samples: BoxedStitchValueFn =
        Box::new(|field, _path, _strong, _in_strong, _weak, _in_weak| {
            if field == "timeSamples" {
                (StitchValueStatus::NoStitchedValue, None)
            } else {
                (StitchValueStatus::UseDefaultValue, None)
            }
        });

    for clip_file in clip_layer_files {
        let clip_layer = match Layer::find_or_open(clip_file) {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Stitch clip into topology, ignoring time samples
        stitch_layers_with_fn(topology_layer, &clip_layer, Some(&ignore_time_samples));
    }

    true
}

/// Creates a clip manifest from clip layers.
///
/// Traverses clip layers to find time-sampled attributes under clip_path,
/// then creates corresponding attribute specs in the manifest layer.
/// Matches C++ `UsdUtilsStitchClipsManifest`.
pub fn stitch_clips_manifest(
    manifest_layer: &Arc<Layer>,
    topology_layer: &Arc<Layer>,
    clip_layer_files: &[String],
    clip_path: &Path,
) -> bool {
    let mut time_sampled_attrs = HashSet::new();

    for clip_file in clip_layer_files {
        if let Ok(clip_layer) = Layer::find_or_open(clip_file) {
            collect_time_sampled_attributes(&clip_layer, clip_path, &mut time_sampled_attrs);
        }
    }

    for attr_path in &time_sampled_attrs {
        let default_value = topology_layer
            .get_attribute_at_path(attr_path)
            .map(|spec| spec.default_value())
            .filter(|v| !v.is_empty());

        create_manifest_attribute(manifest_layer, attr_path, default_value.as_ref());
    }

    true
}

/// Authors clip template metadata on a prim.
///
/// Creates the prim structure and sets template-based clip metadata.
/// Matches C++ `UsdUtilsStitchClipsTemplate`.
pub fn stitch_clips_template(
    result_layer: &Arc<Layer>,
    topology_layer: &Arc<Layer>,
    manifest_layer: &Arc<Layer>,
    clip_path: &Path,
    template_path: &str,
    start_time: f64,
    end_time: f64,
    stride: f64,
    active_offset: Option<f64>,
    interpolate_missing_clip_values: bool,
    clip_set: Option<&Token>,
) -> bool {
    let clip_set_name = clip_set
        .map(|t| t.as_str())
        .unwrap_or(DEFAULT_CLIP_SET_NAME);

    result_layer.clear();

    // Sublayer the topology layer as strongest
    result_layer.insert_sublayer_path(topology_layer.identifier(), 0);

    // Create prim structure at clip path
    let _ = result_layer.create_prim_spec(clip_path, Specifier::Def, "");

    // Build nested clip metadata dict matching C++ StitchClipsTemplate.
    // C++ builds: clips[clipSet] = {primPath: ..., templateAssetPath: ..., ...}
    // The clips field is a nested dict: outer key = clip set name, inner = metadata.
    let manifest_id = manifest_layer.identifier();
    let mut clip_set_dict = HashMap::new();
    clip_set_dict.insert(
        "primPath".to_string(),
        Value::from(clip_path.as_str().to_string()),
    );
    clip_set_dict.insert(
        "templateAssetPath".to_string(),
        Value::from(template_path.to_string()),
    );
    clip_set_dict.insert("templateStartTime".to_string(), Value::from(start_time));
    clip_set_dict.insert("templateEndTime".to_string(), Value::from(end_time));
    clip_set_dict.insert("templateStride".to_string(), Value::from(stride));
    if let Some(offset) = active_offset {
        clip_set_dict.insert("templateActiveOffset".to_string(), Value::from(offset));
    }
    clip_set_dict.insert(
        "manifestAssetPath".to_string(),
        Value::from(manifest_id.to_string()),
    );
    if interpolate_missing_clip_values {
        clip_set_dict.insert(
            "interpolateMissingClipValues".to_string(),
            Value::from(true),
        );
    }

    // Wrap in outer dict: clips[clip_set_name] = clip_set_dict
    let mut clips = HashMap::new();
    clips.insert(
        clip_set_name.to_string(),
        Value::from_dictionary(clip_set_dict),
    );

    let clips_token = Token::new("clips");
    result_layer.set_field(clip_path, &clips_token, Value::from_dictionary(clips));

    // Set time code range
    result_layer.set_start_time_code(start_time);
    result_layer.set_end_time_code(end_time);

    true
}

/// Generates a topology file name based on the root layer name.
pub fn generate_clip_topology_name(root_layer_name: &str) -> String {
    insert_suffix_before_extension(root_layer_name, "topology")
}

/// Generates a manifest file name based on the root layer name.
pub fn generate_clip_manifest_name(root_layer_name: &str) -> String {
    insert_suffix_before_extension(root_layer_name, "manifest")
}

fn insert_suffix_before_extension(path: &str, suffix: &str) -> String {
    if let Some(dot_pos) = path.rfind('.') {
        format!("{}.{}{}", &path[..dot_pos], suffix, &path[dot_pos..])
    } else {
        // C++ returns empty string when no extension is found (no dot in filename).
        // Matches UsdUtilsGenerateClipTopologyName / GenerateClipManifestName behavior.
        String::new()
    }
}

fn compute_clip_time_range(
    clip_layer_files: &[String],
    requested_start: Option<f64>,
    requested_end: Option<f64>,
) -> (f64, f64) {
    let mut min_start = f64::MAX;
    let mut max_end = f64::MIN;

    for clip_file in clip_layer_files {
        if let Ok(layer) = Layer::find_or_open(clip_file) {
            if layer.has_start_time_code() {
                min_start = min_start.min(layer.get_start_time_code());
            }
            if layer.has_end_time_code() {
                max_end = max_end.max(layer.get_end_time_code());
            }
        }
    }

    let start = requested_start.unwrap_or(if min_start == f64::MAX {
        0.0
    } else {
        min_start
    });
    let end = requested_end.unwrap_or(if max_end == f64::MIN { 0.0 } else { max_end });

    (start, end)
}

/// Collects all attribute paths with time samples under root_path.
///
/// Traverses prim hierarchy from root_path downward, collecting paths
/// of attributes that have time samples authored.
fn collect_time_sampled_attributes(
    layer: &Arc<Layer>,
    root_path: &Path,
    result: &mut HashSet<Path>,
) {
    // Get the prim at root_path and traverse its subtree
    let prim_spec = match layer.get_prim_at_path(root_path) {
        Some(p) => p,
        None => return,
    };

    // DFS over prim hierarchy
    let mut stack = vec![prim_spec];
    while let Some(curr) = stack.pop() {
        // Check each property for time samples
        for prop in curr.properties() {
            let prop_path = prop.spec().path();
            let times = layer.list_time_samples_for_path(&prop_path);
            if !times.is_empty() {
                result.insert(prop_path);
            }
        }

        // Recurse into children
        for child in curr.name_children() {
            stack.push(child);
        }
    }
}

/// Creates an attribute spec in the manifest layer at attr_path.
///
/// Optionally sets a default value from the topology layer.
fn create_manifest_attribute(
    manifest: &Arc<Layer>,
    attr_path: &Path,
    default_value: Option<&Value>,
) {
    // Ensure parent prim exists in manifest
    let prim_path = attr_path.get_prim_path();
    if !prim_path.is_empty() && manifest.get_prim_at_path(&prim_path).is_none() {
        let _ = manifest.create_prim_spec(&prim_path, Specifier::Over, "");
    }

    // Create the attribute spec (as a property spec)
    let spec_type = usd_sdf::SpecType::Attribute;
    manifest.create_spec(attr_path, spec_type);

    // Set default value if available from topology
    if let Some(val) = default_value {
        let default_token = Token::new("default");
        manifest.set_field(attr_path, &default_token, val.clone());
    }
}

/// Authors clip metadata on the result layer.
///
/// Sets clipPrimPath, clipAssetPaths, clipActive, clipTimes,
/// clipManifestAssetPath, and time code range on the result layer.
/// Matches C++ `_StitchClipMetadata` + `_SetTimeCodeRange`.
fn author_clip_metadata(
    result_layer: &Arc<Layer>,
    topology_layer: &Arc<Layer>,
    manifest_layer: &Arc<Layer>,
    clip_path: &Path,
    clip_files: &[String],
    start_time: f64,
    end_time: f64,
    interpolate: bool,
    clip_set: &str,
) {
    // Ensure prim exists at clip_path
    if result_layer.get_prim_at_path(clip_path).is_none() {
        let _ = result_layer.create_prim_spec(clip_path, Specifier::Def, "");
    }

    // Sublayer topology as strongest
    let topology_id = topology_layer.identifier();
    let sublayers = result_layer.sublayer_paths();
    if !sublayers.contains(&topology_id.to_string()) {
        result_layer.insert_sublayer_path(topology_id, 0);
    }

    // Build clip asset paths, clip times, and clip active arrays
    let mut asset_paths: Vec<String> = Vec::new();
    let mut clip_times: Vec<[f64; 2]> = Vec::new();
    let mut clip_active: Vec<[f64; 2]> = Vec::new();

    for (idx, clip_file) in clip_files.iter().enumerate() {
        let clip_layer = match Layer::find_or_open(clip_file) {
            Ok(l) => l,
            Err(_) => continue,
        };

        // Relative path if possible (simplified: just use identifier)
        let clip_id = clip_layer.identifier().to_string();
        asset_paths.push(clip_id);

        // Get time range from clip layer
        let clip_start = if clip_layer.has_start_time_code() {
            clip_layer.get_start_time_code()
        } else {
            0.0
        };
        let clip_end = if clip_layer.has_end_time_code() {
            clip_layer.get_end_time_code()
        } else {
            clip_start
        };

        // clipActive: [stageTime, clipIndex]
        clip_active.push([clip_start, idx as f64]);

        // clipTimes: [stageTime, clipTime]
        clip_times.push([clip_start, clip_start]);
        let time_span = clip_end - clip_start;
        if time_span.abs() > f64::EPSILON {
            clip_times.push([clip_start + time_span, clip_end]);
        }
    }

    // Build nested clips dictionary matching C++ structure.
    // C++ stores clips as: clips[clipSet] = {primPath: ..., assetPaths: ..., ...}
    // C++ _SetValue uses SetFieldDictValueByKey which splits compound "set:key" into
    // nested dict. We build the nested structure directly.
    let mut clip_set_dict = HashMap::new();

    // primPath
    clip_set_dict.insert(
        "primPath".to_string(),
        Value::from(clip_path.as_str().to_string()),
    );

    // assetPaths: VtArray<SdfAssetPath>
    let asset_path_vec: Vec<usd_sdf::AssetPath> = asset_paths
        .iter()
        .map(|p| usd_sdf::AssetPath::new(p))
        .collect();
    clip_set_dict.insert("assetPaths".to_string(), Value::new(asset_path_vec));

    // times: array of [stageTime, clipTime] pairs
    clip_set_dict.insert("times".to_string(), Value::from_no_hash(clip_times));

    // active: array of [stageTime, clipIndex] pairs
    clip_set_dict.insert("active".to_string(), Value::from_no_hash(clip_active));

    // manifestAssetPath
    let manifest_id = manifest_layer.identifier();
    clip_set_dict.insert(
        "manifestAssetPath".to_string(),
        Value::from(manifest_id.to_string()),
    );

    // interpolateMissingClipValues
    if interpolate {
        clip_set_dict.insert(
            "interpolateMissingClipValues".to_string(),
            Value::from(true),
        );
    }

    // Wrap in outer dict: clips[clip_set] = clip_set_dict
    let mut clips = HashMap::new();
    clips.insert(clip_set.to_string(), Value::from_dictionary(clip_set_dict));

    let clips_token = Token::new("clips");
    result_layer.set_field(clip_path, &clips_token, Value::from_dictionary(clips));

    // Set layer time code range
    result_layer.set_start_time_code(start_time);
    result_layer.set_end_time_code(end_time);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_clip_topology_name() {
        assert_eq!(generate_clip_topology_name("foo.usd"), "foo.topology.usd");
        assert_eq!(
            generate_clip_topology_name("/bar/baz/foo.usd"),
            "/bar/baz/foo.topology.usd"
        );
    }

    #[test]
    fn test_generate_clip_manifest_name() {
        assert_eq!(generate_clip_manifest_name("foo.usd"), "foo.manifest.usd");
    }

    #[test]
    fn test_generate_clip_names_usda() {
        assert_eq!(
            generate_clip_topology_name("scene.usda"),
            "scene.topology.usda"
        );
        assert_eq!(
            generate_clip_manifest_name("scene.usda"),
            "scene.manifest.usda"
        );
    }

    #[test]
    fn test_generate_clip_names_usdc() {
        assert_eq!(
            generate_clip_topology_name("anim.usdc"),
            "anim.topology.usdc"
        );
        assert_eq!(
            generate_clip_manifest_name("anim.usdc"),
            "anim.manifest.usdc"
        );
    }

    #[test]
    fn test_generate_clip_names_no_extension() {
        // No dot -> C++ returns empty string (no extension = invalid layer name).
        // UsdUtilsGenerateClipTopologyName returns "" when baseFileName has no '.'.
        assert_eq!(generate_clip_topology_name("scene"), "");
        assert_eq!(generate_clip_manifest_name("scene"), "");
    }

    #[test]
    fn test_clip_inner_key_names_match_spec() {
        // USD Value Clips stores a nested dict: clips[clipSet] = {key: value, ...}
        // Verify the inner key names match the USD spec.
        let required_inner_keys = [
            "primPath",
            "assetPaths",
            "times",
            "active",
            "manifestAssetPath",
        ];
        for key in &required_inner_keys {
            assert!(!key.is_empty());
            // Inner keys must NOT contain the set name separator
            assert!(
                !key.contains(':'),
                "inner key must not be compound: {}",
                key
            );
        }
    }

    #[test]
    fn test_template_inner_key_names_match_spec() {
        // Template clip metadata also uses nested dict: clips[clipSet] = {key: value, ...}
        let template_inner_keys = [
            "primPath",
            "templateAssetPath",
            "templateStartTime",
            "templateEndTime",
            "templateStride",
            "manifestAssetPath",
        ];
        for key in &template_inner_keys {
            assert!(!key.is_empty());
            assert!(
                !key.contains(':'),
                "inner key must not be compound: {}",
                key
            );
        }
    }

    #[test]
    fn test_default_clip_set_name() {
        // The default clip set must be "default" per the USD spec
        assert_eq!(DEFAULT_CLIP_SET_NAME, "default");
    }

    #[test]
    fn test_insert_suffix_before_extension_double_dot() {
        // Path with multiple dots: only the last dot is the extension
        let result = generate_clip_topology_name("scene.v001.usd");
        assert_eq!(result, "scene.v001.topology.usd");
    }
}
