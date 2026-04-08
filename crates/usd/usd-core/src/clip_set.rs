//! Usd_ClipSet - represents a clip set for value resolution.
//!
//! Port of pxr/usd/usd/clipSet.h/cpp
//!
//! Represents a clip set for value resolution. A clip set primarily
//! consists of a list of Usd_Clip objects from which attribute values
//! are retrieved during value resolution.

use ordered_float::OrderedFloat;
use std::collections::BTreeMap;
use std::sync::Arc;
use usd_pcp::{LayerStack, NodeRef};
use usd_sdf::LayerHandle;
use usd_sdf::{AssetPath, Path, TimeCode};
use usd_vt::{Array, Vec2dArray};

// Re-export Clip from clip module
pub use super::clip::{
    CLIP_TIMES_EARLIEST, CLIP_TIMES_LATEST, Clip, ClipRefPtr, ClipRefPtrVector, TimeMapping,
    TimeMappings,
};

/// Reference-counted pointer to a clip set.
pub type ClipSetRefPtr = Arc<ClipSet>;

/// Clip metadata for query operations.
#[derive(Debug, Clone)]
struct ClipMetadata {
    /// Start time of this clip (external time).
    start_time: f64,
    /// End time of this clip (external time).
    end_time: f64,
    /// Authored start time of this clip (from clipActive).
    authored_start_time: f64,
}

// Sentinel values and types are now in clip.rs

// ============================================================================
// Helper Functions
// ============================================================================

/// Validates clip fields.
///
/// Matches C++ `_ValidateClipFields`.
fn validate_clip_fields(
    clip_asset_paths: &Array<AssetPath>,
    clip_prim_path: &str,
    clip_active: &Vec2dArray,
    clip_times: Option<&Vec2dArray>,
) -> Option<String> {
    // Note that we do allow empty clipAssetPath and clipActive data;
    // this provides users with a way to 'block' clips specified in a
    // weaker layer.
    if clip_prim_path.is_empty() {
        use crate::clips_api::ClipsAPIInfoKeys;
        return Some(format!(
            "No clip prim path specified in '{}'",
            ClipsAPIInfoKeys::prim_path().get_text()
        ));
    }

    let num_clips = clip_asset_paths.len();

    // Each entry in the clipAssetPaths array is the asset path to a clip.
    for clip_asset_path in clip_asset_paths.iter() {
        if clip_asset_path.get_authored_path().is_empty() {
            use crate::clips_api::ClipsAPIInfoKeys;
            return Some(format!(
                "Empty clip asset path in '{}'",
                ClipsAPIInfoKeys::asset_paths().get_text()
            ));
        }
    }

    // The 'clipPrimPath' field identifies a prim from which clip data
    // will be read.
    if !Path::is_valid_path_string(clip_prim_path) {
        return Some(format!("Invalid path string: '{}'", clip_prim_path));
    }

    let path = match Path::from_string(clip_prim_path) {
        Some(p) => p,
        None => {
            use crate::clips_api::ClipsAPIInfoKeys;
            return Some(format!(
                "Path '{}' in '{}' must be an absolute path to a prim",
                clip_prim_path,
                ClipsAPIInfoKeys::prim_path().get_text()
            ));
        }
    };

    if !path.is_absolute_path() || !path.is_prim_path() {
        use crate::clips_api::ClipsAPIInfoKeys;
        return Some(format!(
            "Path '{}' in '{}' must be an absolute path to a prim",
            clip_prim_path,
            ClipsAPIInfoKeys::prim_path().get_text()
        ));
    }

    // Each Vec2d in the 'clipActive' array is a (start frame, clip index)
    // tuple. Ensure the clip index points to a valid clip.
    for start_frame_and_clip_index in clip_active.iter() {
        let clip_index = start_frame_and_clip_index[1] as i64;
        if clip_index < 0 || clip_index >= num_clips as i64 {
            use crate::clips_api::ClipsAPIInfoKeys;
            return Some(format!(
                "Invalid clip index {} in '{}'",
                clip_index,
                ClipsAPIInfoKeys::active().get_text()
            ));
        }
    }

    // Ensure that 'clipActive' does not specify multiple clips to be
    // active at the same time.
    let mut active_clip_map: BTreeMap<OrderedFloat<f64>, i64> = BTreeMap::new();
    for start_frame_and_clip_index in clip_active.iter() {
        let start_frame = OrderedFloat::from(start_frame_and_clip_index[0]);
        let clip_index = start_frame_and_clip_index[1] as i64;

        if let Some(existing_index) = active_clip_map.insert(start_frame, clip_index) {
            use crate::clips_api::ClipsAPIInfoKeys;
            return Some(format!(
                "Clip {} cannot be active at time {:.3} in '{}' because clip {} was already specified as active at this time.",
                clip_index,
                start_frame,
                ClipsAPIInfoKeys::active().get_text(),
                existing_index
            ));
        }
    }

    // Ensure there are at most two (stage time, clip time) entries in
    // clip times that have the same stage time.
    if let Some(clip_times) = clip_times {
        let mut stage_times_map: BTreeMap<OrderedFloat<f64>, i32> = BTreeMap::new();
        for stage_time_and_clip_time in clip_times.iter() {
            let stage_time = OrderedFloat::from(stage_time_and_clip_time[0]);
            let count = stage_times_map.entry(stage_time).or_insert(0);
            *count += 1;

            if *count > 2 {
                use crate::clips_api::ClipsAPIInfoKeys;
                return Some(format!(
                    "Cannot have more than two entries in '{}' with the same stage time ({:.3}).",
                    ClipsAPIInfoKeys::times().get_text(),
                    stage_time
                ));
            }
        }
    }

    None
}

/// Generates a clip manifest from the given clips.
///
/// Matches C++ `Usd_GenerateClipManifest(const Usd_ClipRefPtrVector& clips, const SdfPath& clipPrimPath, const std::string& tag, bool writeBlocksForClipsWithMissingValues)`.
#[allow(dead_code)] // API for C++ parity - clips_api uses generate_clip_manifest_with_active_times directly
pub(crate) fn generate_clip_manifest(
    clips: &ClipRefPtrVector,
    clip_prim_path: &Path,
    tag: &str,
    write_blocks_for_clips_with_missing_values: bool,
) -> Option<Arc<usd_sdf::Layer>> {
    if !clip_prim_path.is_prim_path() {
        // Error: clipPrimPath must be a prim path
        return None;
    }

    // Extract clip layers and active times
    let mut clip_layers: Vec<Arc<usd_sdf::Layer>> = Vec::new();
    let mut active_times: Vec<f64> = Vec::new();

    for clip in clips {
        // Get layer from clip
        if let Some(layer) = clip.get_layer_for_clip() {
            clip_layers.push(layer);
        }
        // Note: We don't have access to authoredStartTime here, so we'll need
        // to pass it separately or modify the function signature
        // For now, we'll use 0.0 as placeholder - this will be fixed when we
        // call this from from_definition where we have access to metadata
        active_times.push(0.0);
    }

    // Call the overload that takes clip layers and active times
    generate_clip_manifest_with_active_times(
        &clip_layers,
        clip_prim_path,
        tag,
        if write_blocks_for_clips_with_missing_values {
            Some(&active_times)
        } else {
            None
        },
    )
}

/// Generates a clip manifest from clip layers and active times.
///
/// Matches C++ `Usd_GenerateClipManifest(const SdfLayerHandleVector& clipLayers, const SdfPath& clipPrimPath, const std::string& tag, const std::vector<double>* clipActive)`.
pub(crate) fn generate_clip_manifest_with_active_times(
    clip_layers: &[Arc<usd_sdf::Layer>],
    clip_prim_path: &Path,
    tag: &str,
    clip_active: Option<&[f64]>,
) -> Option<Arc<usd_sdf::Layer>> {
    use usd_sdf::{SpecType, ValueBlock, Variability};
    use usd_tf::Token;

    if !clip_prim_path.is_prim_path() {
        // Error: clipPrimPath must be a prim path
        return None;
    }

    // Check for invalid layers (empty layers are considered invalid)
    if clip_layers.is_empty() {
        return None;
    }

    // Create anonymous layer with tag
    let manifest_layer = usd_sdf::Layer::create_anonymous(Some(&format!("{}.usda", tag)));

    // Traverse all clip layers and collect attribute metadata
    // Helper function to recursively traverse and collect attributes
    fn traverse_for_attributes(
        layer: &Arc<usd_sdf::Layer>,
        path: &Path,
        clip_prim_path: &Path,
        manifest_layer: &Arc<usd_sdf::Layer>,
    ) {
        // Only process paths that are descendants of clipPrimPath
        if !path.has_prefix(clip_prim_path) && path != clip_prim_path {
            return;
        }

        // Only process property paths (attributes)
        if path.is_property_path() {
            // Check if this is an attribute spec
            if layer.get_spec_type(path) == SpecType::Attribute {
                // Check if manifest already has this spec
                if !manifest_layer.has_spec(path) {
                    // Get TypeName field
                    let type_name_token = Token::new("typeName");
                    if let Some(type_name_value) = layer.get_field(path, &type_name_token) {
                        // Get Variability field
                        let variability_token = Token::new("variability");
                        if let Some(variability_value) = layer.get_field(path, &variability_token) {
                            // Check if this attribute has time samples
                            if layer.get_num_time_samples_for_path(path) > 0 {
                                // Extract type name and variability
                                // TypeName is typically a Token
                                let type_name =
                                    if let Some(token) = type_name_value.downcast::<Token>() {
                                        token.clone()
                                    } else if let Some(s) = type_name_value.downcast::<String>() {
                                        Token::new(s)
                                    } else {
                                        return;
                                    };

                                // Variability is typically Variability enum
                                let variability = if let Some(var) =
                                    variability_value.downcast::<Variability>()
                                {
                                    *var
                                } else {
                                    Variability::Varying // Default
                                };

                                // Create attribute spec in manifest layer
                                // First create the spec
                                if manifest_layer.create_spec(path, SpecType::Attribute) {
                                    // Set TypeName field
                                    let type_name_field_value =
                                        usd_sdf::abstract_data::Value::new(type_name.clone());
                                    manifest_layer.set_field(
                                        path,
                                        &type_name_token,
                                        type_name_field_value,
                                    );

                                    // Set Variability field
                                    let variability_field_value =
                                        usd_sdf::abstract_data::Value::new(variability);
                                    manifest_layer.set_field(
                                        path,
                                        &variability_token,
                                        variability_field_value,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Recursively traverse children
        // Get prim children
        let prim_children_token = Token::new("primChildren");
        if let Some(children_value) = layer.get_field(path, &prim_children_token) {
            if let Some(children) = children_value.as_vec_clone::<String>() {
                for child_name in &children {
                    let child_path = path.append_child(child_name).unwrap_or_else(Path::empty);
                    if !child_path.is_empty() {
                        traverse_for_attributes(layer, &child_path, clip_prim_path, manifest_layer);
                    }
                }
            }
        }

        // Get property children
        let prop_children_token = Token::new("properties");
        if let Some(children_value) = layer.get_field(path, &prop_children_token) {
            if let Some(children) = children_value.as_vec_clone::<String>() {
                for child_name in &children {
                    let child_path = path.append_property(child_name).unwrap_or_else(Path::empty);
                    if !child_path.is_empty() {
                        traverse_for_attributes(layer, &child_path, clip_prim_path, manifest_layer);
                    }
                }
            }
        }
    }

    for clip_layer in clip_layers {
        // Traverse from clipPrimPath
        traverse_for_attributes(clip_layer, clip_prim_path, clip_prim_path, &manifest_layer);
    }

    // If writeBlocksForClipsWithMissingValues is true, add blocks for clips
    // that don't have time samples for attributes
    if let Some(active_times) = clip_active {
        if active_times.len() != clip_layers.len() {
            // Mismatch in sizes - skip block writing
            return Some(manifest_layer);
        }

        let mut attr_to_active_times: Vec<(Path, Vec<f64>)> = Vec::new();

        // Traverse manifest layer to find attributes
        // Use the same recursive traversal helper
        fn traverse_manifest_for_blocks(
            layer: &Arc<usd_sdf::Layer>,
            path: &Path,
            clip_prim_path: &Path,
            clip_layers: &[Arc<usd_sdf::Layer>],
            active_times: &[f64],
            attr_to_active_times: &mut Vec<(Path, Vec<f64>)>,
        ) {
            // Only process paths that are descendants of clipPrimPath
            if !path.has_prefix(clip_prim_path) && path != clip_prim_path {
                return;
            }

            // Only process property paths (attributes)
            if path.is_property_path() {
                // Check which clips don't have time samples for this attribute
                let mut missing_times = Vec::new();
                for (i, clip_layer) in clip_layers.iter().enumerate() {
                    if clip_layer.get_num_time_samples_for_path(path) == 0 && i < active_times.len()
                    {
                        missing_times.push(active_times[i]);
                    }
                }

                if !missing_times.is_empty() {
                    attr_to_active_times.push((path.clone(), missing_times));
                }
            }

            // Recursively traverse children
            let prim_children_token = Token::new("primChildren");
            if let Some(children_value) = layer.get_field(path, &prim_children_token) {
                if let Some(children) = children_value.as_vec_clone::<String>() {
                    for child_name in &children {
                        if let Some(child_path) = path.append_child(child_name) {
                            traverse_manifest_for_blocks(
                                layer,
                                &child_path,
                                clip_prim_path,
                                clip_layers,
                                active_times,
                                attr_to_active_times,
                            );
                        }
                    }
                }
            }

            let prop_children_token = Token::new("properties");
            if let Some(children_value) = layer.get_field(path, &prop_children_token) {
                if let Some(children) = children_value.as_vec_clone::<String>() {
                    for child_name in &children {
                        if let Some(child_path) = path.append_property(child_name) {
                            traverse_manifest_for_blocks(
                                layer,
                                &child_path,
                                clip_prim_path,
                                clip_layers,
                                active_times,
                                attr_to_active_times,
                            );
                        }
                    }
                }
            }
        }

        traverse_manifest_for_blocks(
            &manifest_layer,
            clip_prim_path,
            clip_prim_path,
            clip_layers,
            active_times,
            &mut attr_to_active_times,
        );

        // Set ValueBlock for missing times
        let block = ValueBlock;
        let block_value = usd_vt::Value::new(block);

        for (path, times) in attr_to_active_times {
            for time in times {
                manifest_layer.set_time_sample(&path, time, block_value.clone());
            }
        }
    }

    Some(manifest_layer)
}

// ============================================================================
// ClipSet
// ============================================================================

/// Represents a clip set for value resolution.
///
/// Matches C++ `Usd_ClipSet`.
///
/// A clip set primarily consists of a list of Clip objects from which
/// attribute values are retrieved during value resolution.
pub struct ClipSet {
    /// Name of the clip set.
    pub name: String,
    /// Source layer stack.
    pub source_layer_stack: Option<Arc<LayerStack>>,
    /// Source prim path.
    pub source_prim_path: Path,
    /// Source layer.
    pub source_layer: Option<LayerHandle>,
    /// Clip prim path.
    pub clip_prim_path: Path,
    /// Manifest clip.
    pub manifest_clip: Option<ClipRefPtr>,
    /// Value clips.
    pub value_clips: ClipRefPtrVector,
    /// Whether to interpolate missing clip values.
    pub interpolate_missing_clip_values: bool,
    /// Mapping of external to internal times.
    times: Option<Arc<TimeMappings>>,
    /// Metadata for each clip (indexed by position in value_clips).
    clip_metadata: Vec<ClipMetadata>,
}

// TimeMapping and TimeMappings are now in clip.rs

impl ClipSet {
    /// Create a new clip set based on the given definition.
    ///
    /// Matches C++ `New(const std::string& name, const Usd_ClipSetDefinition& definition, std::string* status)`.
    ///
    /// If clip set creation fails, returns None and populates status with an error message.
    /// Otherwise status may be populated with other information or debugging output.
    pub fn new(
        name: String,
        definition: &super::clip_set_definition::ClipSetDefinition,
        status: &mut Option<String>,
    ) -> Option<ClipSetRefPtr> {
        // If we haven't found all of the required clip metadata we can just bail out.
        // Note that clipTimes and clipManifestAssetPath are *not* required.
        let Some(ref clip_asset_paths) = definition.clip_asset_paths else {
            return None;
        };
        let Some(ref clip_prim_path) = definition.clip_prim_path else {
            return None;
        };
        let Some(ref clip_active) = definition.clip_active else {
            return None;
        };

        // Validate clip fields
        if let Some(err_msg) = validate_clip_fields(
            clip_asset_paths,
            clip_prim_path,
            clip_active,
            definition.clip_times.as_ref(),
        ) {
            *status = Some(err_msg);
            return None;
        }

        // The clip manifest is currently optional but can greatly improve
        // performance if specified. For debugging performance problems,
        // issue a message indicating if one hasn't been specified.
        if definition.clip_manifest_asset_path.is_none() {
            *status = Some("No clip manifest specified. Performance may be improved if a manifest is specified.".to_string());
        }

        // Create the clip set from the definition
        Some(Arc::new(ClipSet::from_definition(name, definition)))
    }

    /// Creates a clip set from a validated definition.
    ///
    /// Matches C++ `Usd_ClipSet::Usd_ClipSet(const std::string& name_, const Usd_ClipSetDefinition& clipDef)`.
    ///
    /// NOTE: Assumes definition has already been validated.
    fn from_definition(
        name: String,
        definition: &super::clip_set_definition::ClipSetDefinition,
    ) -> Self {
        use std::collections::BTreeMap;

        let clip_asset_paths = definition
            .clip_asset_paths
            .as_ref()
            .expect("clip asset paths set");
        let clip_prim_path_str = definition
            .clip_prim_path
            .as_ref()
            .expect("clip prim path set");
        let clip_active = definition.clip_active.as_ref().expect("clip active set");

        // Generate a mapping of startTime -> clip entry. This allows us to
        // quickly determine the (startTime, endTime) for a given clip.
        let mut start_time_to_clip: BTreeMap<OrderedFloat<f64>, (usize, usd_sdf::AssetPath)> =
            BTreeMap::new();

        for start_frame_and_clip_index in clip_active.iter() {
            let start_frame = OrderedFloat::from(start_frame_and_clip_index[0]);
            let clip_index = start_frame_and_clip_index[1] as usize;

            if clip_index >= clip_asset_paths.len() {
                // Invalid clip index - should have been caught by validation
                continue;
            }

            let asset_path = clip_asset_paths[clip_index].clone();

            // Validation should have caused us to bail out if there were any
            // conflicting clip activations set.
            start_time_to_clip.insert(start_frame, (clip_index, asset_path));
        }

        // Generate the clip time mapping that applies to all clips.
        let mut times: TimeMappings = Vec::new();
        if let Some(ref clip_times) = definition.clip_times {
            for clip_time in clip_times.iter() {
                let ext_time = clip_time[0];
                let int_time = clip_time[1];
                times.push(TimeMapping {
                    external_time: ext_time,
                    internal_time: int_time,
                    is_jump_discontinuity: false,
                });
            }
        }

        if !times.is_empty() {
            // Maintain the relative order of entries with the same stage time for
            // jump discontinuities in case the authored times array was unsorted.
            times.sort_by(|a, b| {
                a.external_time
                    .partial_cmp(&b.external_time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Jump discontinuities are represented by consecutive entries in the
            // times array with the same stage time, e.g. (10, 10), (10, 0).
            // We represent this internally as (10 - SafeStep(), 10), (10, 0)
            // because a lot of the desired behavior just falls out from this
            // representation.
            for i in 0..times.len() - 1 {
                if times[i].external_time == times[i + 1].external_time {
                    // SafeStep() with default values: maxValue=1e6, maxCompression=10.0
                    let safe_step_value = crate::time_code::TimeCode::safe_step(1e6, 10.0);
                    times[i].external_time -= safe_step_value;
                    times[i].is_jump_discontinuity = true;
                }
            }

            // Add sentinel values to the beginning and end for convenience.
            if let Some(&first) = times.first() {
                times.insert(0, first);
            }
            if let Some(&last) = times.last() {
                times.push(last);
            }
        }

        // Have the clipTimes saved on the ClipSet as well (create Arc early for use in loop).
        let times_arc = if times.is_empty() {
            None
        } else {
            Some(Arc::new(times))
        };

        // Parse clip prim path once
        let clip_prim_path = Path::from_string(clip_prim_path_str).unwrap_or_else(Path::empty);

        // Build up the final vector of clips and their metadata.
        let mut value_clips: ClipRefPtrVector = Vec::new();
        let mut clip_metadata: Vec<ClipMetadata> = Vec::new();
        let mut iter = start_time_to_clip.iter().peekable();

        while let Some((start_time, (_clip_index, asset_path))) = iter.next() {
            let start_time_val = start_time.0;
            let clip_start_time = if iter.peek().is_none() && value_clips.is_empty() {
                CLIP_TIMES_EARLIEST
            } else {
                start_time_val
            };

            let clip_end_time = if let Some((next_start_time, _)) = iter.peek() {
                next_start_time.0
            } else {
                CLIP_TIMES_LATEST
            };

            // Store authored start time (from clipActive)
            let authored_start_time = start_time_val;

            // Create proper Clip struct
            let clip_source_layer_index = definition.index_of_layer_where_asset_paths_found;
            let clip_times = times_arc.clone();

            let clip = Clip::new(
                definition.source_layer_stack.clone(),
                definition.source_prim_path.clone(),
                clip_source_layer_index,
                asset_path.clone(),
                clip_prim_path.clone(),
                authored_start_time,
                clip_start_time,
                clip_end_time,
                clip_times,
            );
            value_clips.push(Arc::new(clip));

            // Store metadata for this clip
            clip_metadata.push(ClipMetadata {
                start_time: clip_start_time,
                end_time: clip_end_time,
                authored_start_time,
            });
        }

        let interpolate_missing_clip_values =
            definition.interpolate_missing_clip_values.unwrap_or(false);

        // Create a clip for the manifest. If no manifest has been specified,
        // we generate one for the user automatically.

        let (manifest_asset_path, generated_manifest) =
            if let Some(ref manifest_path) = definition.clip_manifest_asset_path {
                (manifest_path.clone(), None)
            } else {
                // Generate manifest - we need to pass active times
                let active_times: Vec<f64> = clip_metadata
                    .iter()
                    .map(|m| m.authored_start_time)
                    .collect();

                // Convert clips to layers
                let clip_layers: Vec<Arc<usd_sdf::Layer>> = value_clips
                    .iter()
                    .filter_map(|clip| clip.get_layer_for_clip())
                    .collect();

                let generated = generate_clip_manifest_with_active_times(
                    &clip_layers,
                    &clip_prim_path,
                    "generated_manifest",
                    Some(&active_times),
                );
                if let Some(ref gen_layer) = generated {
                    let asset_path = usd_sdf::AssetPath::new(gen_layer.identifier());
                    (asset_path, generated)
                } else {
                    // Fallback to empty asset path if generation failed
                    (usd_sdf::AssetPath::new(""), None)
                }
            };

        // Create manifest clip
        let manifest_clip = if !manifest_asset_path.get_authored_path().is_empty() {
            // Create proper Clip struct for manifest
            let manifest_clip_source_layer_index =
                definition.index_of_layer_where_asset_paths_found;
            let manifest_clip = Clip::new(
                definition.source_layer_stack.clone(),
                definition.source_prim_path.clone(),
                manifest_clip_source_layer_index,
                manifest_asset_path.clone(),
                clip_prim_path.clone(),
                0.0, // authored_start_time for manifest
                CLIP_TIMES_EARLIEST,
                CLIP_TIMES_LATEST,
                None, // No time mappings for manifest
            );
            Some(Arc::new(manifest_clip))
        } else {
            None
        };

        // If we generated a manifest layer, pull on it here to ensure the manifest
        // takes ownership of it.
        if generated_manifest.is_some() {
            // Reference is held by manifest_clip
        }

        let source_layer_stack = definition.source_layer_stack.clone();
        let source_layer = if let Some(ref stack) = source_layer_stack {
            let layers = stack.get_layers();
            if definition.index_of_layer_where_asset_paths_found < layers.len() {
                Some(usd_sdf::LayerHandle::from_layer(
                    &layers[definition.index_of_layer_where_asset_paths_found],
                ))
            } else {
                None
            }
        } else {
            None
        };

        ClipSet {
            name,
            source_layer_stack: definition.source_layer_stack.clone(),
            source_prim_path: definition.source_prim_path.clone(),
            source_layer,
            clip_prim_path,
            manifest_clip,
            value_clips,
            interpolate_missing_clip_values,
            times: times_arc,
            clip_metadata,
        }
    }

    /// Return the active clip at the given time.
    ///
    /// Matches C++ `GetActiveClip(UsdTimeCode time)`.
    ///
    /// This overload tries to find if time has any jump discontinuity, and
    /// if so, and if querying a pre-time, it will return the previous clip.
    pub fn get_active_clip(&self, time: TimeCode) -> ClipRefPtr {
        let time_value = time.value();
        let is_pre_time = time_value < 0.0; // Pre-time is negative

        if !is_pre_time {
            // When querying an ordinary time, we do not need to check if there
            // is a jump discontinuity at time, the active clip will be decided
            // based on the later time mapping.
            return self.get_active_clip_with_jump(time, false);
        }

        self.get_active_clip_with_jump(time, self.has_jump_discontinuity_at_time(time_value))
    }

    /// Return the active clip at the given time.
    ///
    /// Matches C++ `GetActiveClip(UsdTimeCode time, bool timeHasJumpDiscontinuity)`.
    ///
    /// If timeHasJumpDiscontinuity is true, and time is a pre-time
    /// then our active clip should be previous clip.
    pub fn get_active_clip_with_jump(
        &self,
        time: TimeCode,
        time_has_jump_discontinuity: bool,
    ) -> ClipRefPtr {
        let clip_index = self.find_clip_index_for_time(time.value());
        let is_pre_time = time.value() < 0.0;

        if time_has_jump_discontinuity && is_pre_time && clip_index > 0 {
            self.value_clips
                .get(clip_index - 1)
                .cloned()
                .unwrap_or_else(|| {
                    if !self.value_clips.is_empty() {
                        self.value_clips[0].clone()
                    } else {
                        // Return first clip - this should not happen in practice
                        // In C++, this would be a coding error
                        self.value_clips
                            .first()
                            .cloned()
                            .expect("value_clips should not be empty")
                    }
                })
        } else {
            self.value_clips
                .get(clip_index)
                .cloned()
                .unwrap_or_else(|| {
                    if !self.value_clips.is_empty() {
                        self.value_clips[0].clone()
                    } else {
                        // Return first clip - this should not happen in practice
                        // In C++, this would be a coding error
                        self.value_clips
                            .first()
                            .cloned()
                            .expect("value_clips should not be empty")
                    }
                })
        }
    }

    /// Returns the previous clip given a clip.
    ///
    /// Matches C++ `GetPreviousClip(const Usd_ClipRefPtr& clip)`.
    ///
    /// If there is no previous clip, this clip is returned as the previous clip.
    pub fn get_previous_clip(&self, clip: &ClipRefPtr) -> ClipRefPtr {
        if let Some(pos) = self.value_clips.iter().position(|c| Arc::ptr_eq(c, clip)) {
            if pos > 0 {
                self.value_clips[pos - 1].clone()
            } else {
                clip.clone()
            }
        } else {
            clip.clone()
        }
    }

    /// Return bracketing time samples for the attribute at path at time.
    ///
    /// Matches C++ `GetBracketingTimeSamplesForPath(const SdfPath& path, double time, double* lower, double* upper)`.
    pub fn get_bracketing_time_samples_for_path(
        &self,
        path: &Path,
        time: f64,
        lower: &mut f64,
        upper: &mut f64,
    ) -> bool {
        let mut found_lower = false;
        let mut found_upper = false;

        let clip_index = self.find_clip_index_for_time(time);

        if clip_index >= self.value_clips.len() {
            return false;
        }

        let active_clip = &self.value_clips[clip_index];
        if self.clip_contributes_value(active_clip, path) {
            // Query the clip for bracketing samples
            if let Some((clip_lower, clip_upper)) =
                active_clip.get_bracketing_time_samples_for_path(path, time)
            {
                *lower = clip_lower;
                *upper = clip_upper;

                // Since each clip always has a time sample at its start time,
                // the above call will always establish the lower bracketing sample.
                found_lower = true;

                // If the given time is after the last time sample in the active
                // time range of the clip we need to search forward for the next
                // clip that contributes a value and use it to find the upper bracketing
                // sample. We indicate this by setting foundUpper to false.
                found_upper = !(*lower == *upper && time > *upper);
            }
        }

        // If we haven't found the lower bracketing sample from the active
        // clip, search backwards to find the nearest clip that contributes
        // values and use that to determine the lower sample.
        let mut i = clip_index;
        while !found_lower && i > 0 {
            i -= 1;
            let clip = &self.value_clips[i];
            if !self.clip_contributes_value(clip, path) {
                continue;
            }

            if let Some((_tmp_lower, tmp_upper)) =
                clip.get_bracketing_time_samples_for_path(path, time)
            {
                found_lower = true;
                *lower = tmp_upper;
            }
        }

        // If we haven't found the upper bracketing sample from the active
        // clip, search forwards to find the nearest clip that contributes
        // values and use its start time as the upper sample. We can avoid
        // the cost of calling GetBracketingTimeSamples here since we know
        // a clip always has a time sample at its start frame if it
        // contributes values.
        let mut i = clip_index + 1;
        while !found_upper && i < self.value_clips.len() {
            let clip = &self.value_clips[i];
            if self.clip_contributes_value(clip, path) {
                *upper = self.clip_metadata[i].start_time;
                found_upper = true;
            }
            i += 1;
        }

        // Reconcile foundLower and foundUpper values.
        if found_lower && !found_upper {
            *upper = *lower;
        } else if !found_lower && found_upper {
            *lower = *upper;
        } else if !found_lower && !found_upper {
            // In this case, no clips have been found that contribute
            // values. Use the start time of the first clip as the sole
            // time sample.
            if !self.clip_metadata.is_empty() {
                *lower = self.clip_metadata[0].authored_start_time;
                *upper = self.clip_metadata[0].authored_start_time;
            } else {
                return false;
            }
        }

        true
    }

    /// Returns the previous time sample authored just before the querying time.
    ///
    /// Matches C++ `GetPreviousTimeSampleForPath(const SdfPath& path, double time, double* tPrevious)`.
    ///
    /// If there is no time sample authored just before time, this function
    /// returns false. Otherwise, it returns true and sets tPrevious to the
    /// time of the previous sample.
    pub fn get_previous_time_sample_for_path(
        &self,
        path: &Path,
        time: f64,
        t_previous: &mut f64,
    ) -> bool {
        let all_time_samples = self.list_time_samples_for_path(path);
        if all_time_samples.is_empty() {
            return false;
        }

        // Can't get a previous time sample if the given time is less than
        // or equal to the first time sample.
        let first = all_time_samples[0];
        if time <= first {
            return false;
        }

        // Last time is the previous time if the query time is greater than
        // the last time sample.
        let last = all_time_samples[all_time_samples.len() - 1];
        if time > last {
            *t_previous = last;
            return true;
        }

        // Binary search for the previous time sample
        // Since f64 doesn't implement Ord, we need to use a different approach
        let mut prev_time = None;
        for &sample_time in &all_time_samples {
            if sample_time < time {
                prev_time = Some(sample_time);
            } else {
                break;
            }
        }

        if let Some(prev) = prev_time {
            *t_previous = prev;
            return true;
        }

        false
    }

    /// Return set of time samples for attribute at path.
    ///
    /// Matches C++ `ListTimeSamplesForPath(const SdfPath& path)`.
    pub fn list_time_samples_for_path(&self, path: &Path) -> Vec<f64> {
        use std::collections::BTreeSet;
        let mut samples_set = BTreeSet::new();

        for clip in &self.value_clips {
            if !self.clip_contributes_value(clip, path) {
                continue;
            }

            // Get time samples from the clip
            let clip_samples = clip.list_time_samples_for_path(path);
            for sample_time in clip_samples {
                samples_set.insert(OrderedFloat::from(sample_time));
            }
        }

        if samples_set.is_empty() {
            // In this case, no clips have been found that contribute
            // values. Use the start time of the first clip as the sole
            // time sample.
            if !self.clip_metadata.is_empty() {
                samples_set.insert(OrderedFloat::from(
                    self.clip_metadata[0].authored_start_time,
                ));
            }
        }

        // Convert BTreeSet to Vec and sort
        let mut samples: Vec<f64> = samples_set.into_iter().map(|of| of.0).collect();
        samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        samples
    }

    /// Return list of time samples for attribute at path in the given interval.
    ///
    /// Matches C++ `GetTimeSamplesInInterval(const SdfPath& path, const GfInterval& interval)`.
    pub fn get_time_samples_in_interval(
        &self,
        path: &Path,
        interval_start: f64,
        interval_end: f64,
    ) -> Vec<f64> {
        let mut time_samples = Vec::new();

        for (i, clip) in self.value_clips.iter().enumerate() {
            let metadata = &self.clip_metadata[i];

            // Clips are ordered by increasing start time. Once we hit a clip
            // whose start time is greater than the given interval, we can stop
            // looking.
            if metadata.start_time > interval_end {
                break;
            }

            // Check if clip's time range intersects with the interval
            // Interval is [interval_start, interval_end) (maxClosed = false)
            if metadata.end_time <= interval_start || metadata.start_time >= interval_end {
                continue;
            }

            if !self.clip_contributes_value(clip, path) {
                continue;
            }

            // Get time samples from clip and filter by interval
            let clip_samples = clip.list_time_samples_for_path(path);
            for sample_time in clip_samples {
                if sample_time >= interval_start && sample_time < interval_end {
                    time_samples.push(sample_time);
                }
            }
        }

        // If we haven't found any time samples in the interval, we need to check
        // whether there are any clips that provide samples. If there are none,
        // we always add the start time of the first clip as the sole time sample.
        if time_samples.is_empty() {
            let has_any_contributing_clips = self
                .value_clips
                .iter()
                .any(|clip| self.clip_contributes_value(clip, path));

            if !has_any_contributing_clips && !self.clip_metadata.is_empty() {
                let first_start = self.clip_metadata[0].authored_start_time;
                if first_start >= interval_start && first_start < interval_end {
                    time_samples.push(first_start);
                }
            }
        }

        time_samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        time_samples.dedup();
        time_samples
    }

    /// Get the untyped value for the attribute at path at time from this clip set.
    ///
    /// Used during attribute resolution when the type is not known upfront.
    /// Queries the active clip at `time`; if the clip has no sample at that exact time,
    /// falls back to the manifest default. Returns None if no clips contribute a value.
    pub fn get_value(&self, path: &Path, time: TimeCode) -> Option<usd_vt::Value> {
        if self.value_clips.is_empty() {
            return None;
        }

        let clip = self.get_active_clip_with_jump(time, false);

        // Try exact or interpolated sample from the active clip
        if let Some(val) = clip.query_time_sample_value(path, time.value()) {
            // A ValueBlock means no value at this time
            if val.get::<usd_sdf::ValueBlock>().is_none() {
                return Some(val);
            }
        }

        // No sample in active clip - fall back to manifest default if present
        if let Some(ref manifest) = self.manifest_clip {
            if let Some(val) = manifest.query_time_sample_value(path, 0.0) {
                if val.get::<usd_sdf::ValueBlock>().is_none() {
                    return Some(val);
                }
            }
        }

        None
    }

    /// Matches `Usd_ClipSet::QueryTimeSampleTypeid` followed by `VtValueTypeCanComposeOver`.
    ///
    /// Used by `_SamplesInIntervalResolver::ProcessClips` / bracketing clip logic in `stage.cpp`.
    pub(crate) fn query_time_sample_typeid_can_compose(
        &self,
        path: &Path,
        stage_time: f64,
    ) -> bool {
        use usd_sdf::ValueBlock;
        use usd_vt::value_type_can_compose_over;

        if self.value_clips.is_empty() {
            return false;
        }

        let time = TimeCode::new(stage_time);
        let clip = self.get_active_clip_with_jump(time, false);

        if let Some(val) = clip.query_time_sample_value(path, stage_time) {
            if val.get::<ValueBlock>().is_some() {
                return false;
            }
            return val
                .held_type_id()
                .map(|id| value_type_can_compose_over(id))
                .unwrap_or(false);
        }

        if let Some(ref manifest_clip) = self.manifest_clip {
            if let Some(val) = manifest_clip.query_time_sample_value(path, 0.0) {
                if val.get::<ValueBlock>().is_some() {
                    return false;
                }
                return val
                    .held_type_id()
                    .map(|id| value_type_can_compose_over(id))
                    .unwrap_or(false);
            }
        }

        false
    }

    /// Query time sample for the attribute at path at time.
    ///
    /// Matches C++ `QueryTimeSample<T>(const SdfPath& path, UsdTimeCode time, Usd_InterpolatorBase* interpolator, T* value)`.
    ///
    /// If no time sample exists in the active clip at time,
    /// interpolator will be used to try to interpolate the
    /// value from the surrounding time samples in the active clip.
    /// If the active clip has no time samples, use the default
    /// value for the attribute declared in the manifest. If no
    /// default value is declared, use the fallback value for
    /// the attribute's value type.
    pub fn query_time_sample<T: Clone + 'static>(
        &self,
        path: &Path,
        time: TimeCode,
        _interpolator: Option<&dyn std::any::Any>,
        value: &mut T,
    ) -> bool {
        let clip = self.get_active_clip_with_jump(time, false);

        // First query the clip for time samples at the specified time.
        // Note: Layer's query_time_sample_typed requires T: Clone
        if let Some(clip_value) = clip.query_time_sample_typed::<T>(path, time.value()) {
            *value = clip_value;
            return true;
        }

        // If no samples exist in the clip, get the default value from
        // the manifest. Return true if we get a non-block value, false
        // otherwise.
        if let Some(ref manifest_clip) = self.manifest_clip {
            if let Some(default_value) = manifest_clip.query_time_sample_typed::<T>(path, 0.0) {
                *value = default_value;
                return true;
            }
        }

        false
    }

    /// Query time samples for an attribute at path at pre-time time if
    /// samples represent a jump discontinuity.
    ///
    /// Matches C++ `QueryPreTimeSampleWithJumpDiscontinuity<T>`.
    ///
    /// If time is not a pre-time or it doesn't represent a jump
    /// discontinuity, this function returns false. Otherwise, it returns
    /// true and sets the pre-time sample value to value.
    pub fn query_pre_time_sample_with_jump_discontinuity<T: Clone + 'static>(
        &self,
        path: &Path,
        time: TimeCode,
        _interpolator: Option<&dyn std::any::Any>,
        value: &mut T,
    ) -> bool {
        // Check if time is a pre-time (negative value)
        let time_value = time.value();
        if time_value >= 0.0 {
            return false;
        }

        // Check if time represents a jump discontinuity
        if !self.has_jump_discontinuity_at_time(time_value) {
            return false;
        }

        // Get active clip with jump discontinuity flag set
        let clip = self.get_active_clip_with_jump(time, true);

        // Query the clip for time sample
        if let Some(clip_value) = clip.query_time_sample_typed::<T>(path, time_value) {
            *value = clip_value;
            return true;
        }

        // If no samples exist in the clip, get the default value from
        // the manifest.
        if let Some(ref manifest_clip) = self.manifest_clip {
            if let Some(default_value) = manifest_clip.query_time_sample_typed::<T>(path, 0.0) {
                *value = default_value;
                return true;
            }
        }

        false
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Return the index of the clip that is active at the given time.
    ///
    /// Matches C++ `_FindClipIndexForTime(double time)`.
    ///
    /// This will always return a valid index into the valueClips list.
    fn find_clip_index_for_time(&self, time: f64) -> usize {
        // If there was only one clip, it must be active over all time so
        // we don't need to search any further.
        if self.value_clips.len() <= 1 {
            return 0;
        }

        // Find the first clip whose start time is greater than the given time.
        // Use binary search for efficiency.
        let mut clip_index = 0;
        for (i, metadata) in self.clip_metadata.iter().enumerate() {
            if time < metadata.start_time {
                if i > 0 {
                    clip_index = i - 1;
                }
                break;
            }
            clip_index = i;
        }

        // Verify the clip index is valid and time is in range
        if clip_index < self.clip_metadata.len() {
            let metadata = &self.clip_metadata[clip_index];
            if time >= metadata.start_time && time < metadata.end_time {
                return clip_index;
            }
        }

        // Fallback to first clip
        0
    }

    /// Returns true if the time represents a jump discontinuity.
    ///
    /// Matches C++ `_HasJumpDiscontinuityAtTime(double time)`.
    fn has_jump_discontinuity_at_time(&self, time: f64) -> bool {
        if let Some(ref times) = self.times {
            if times.is_empty() {
                return false;
            }

            // Find the first mapping with external_time >= time
            let it = times.iter().position(|m| m.external_time >= time);

            if let Some(pos) = it {
                // Check if we found a mapping at this time
                if pos < times.len() && times[pos].external_time == time {
                    // Check if the previous entry is a jump discontinuity
                    // (jump discontinuities are represented on the previous mapping entry)
                    if pos > 0 && times[pos - 1].is_jump_discontinuity {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Return whether the specified clip contributes time sample values
    /// to this clip set for the attribute at path.
    ///
    /// Matches C++ `_ClipContributesValue(const Usd_ClipRefPtr& clip, const SdfPath& path)`.
    fn clip_contributes_value(&self, clip: &ClipRefPtr, path: &Path) -> bool {
        // If this clip interpolates values for clips without authored
        // values for an attribute, then we need to check whether the
        // clip actually contains authored values below. Otherwise every
        // clip contributes a value, so we can use the clip.
        if !self.interpolate_missing_clip_values {
            return true;
        }

        // Find the clip index to get its authored start time
        let clip_index = self
            .value_clips
            .iter()
            .position(|c| Arc::ptr_eq(c, clip))
            .unwrap_or(0);

        let authored_start_time = if clip_index < self.clip_metadata.len() {
            self.clip_metadata[clip_index].authored_start_time
        } else {
            return false;
        };

        // Use the clip if it has authored time samples for the attribute.
        // If this attribute is blocked at the clip's start time in the
        // manifest it means the user has declared there are no samples
        // in that clip for this attribute. This allows us to skip
        // opening the layer to check if it has authored time samples.
        if let Some(ref manifest_clip) = self.manifest_clip {
            // Check if path is blocked at authored_start_time in manifest
            // In C++, this checks: !manifestClip->IsBlocked(path, clip->authoredStartTime)
            if manifest_clip.is_blocked(path, authored_start_time) {
                // Path is blocked in manifest, clip doesn't contribute values
                return false;
            }
        }

        // Check if clip has authored time samples
        clip.get_num_time_samples_for_path(path) > 0
    }

    /// Returns true if a value block is authored for the attribute
    /// corresponding to the given path at the given time.
    ///
    /// Matches C++ `IsBlocked(const SdfPath& path, ExternalTime time)`.
    pub fn is_blocked(&self, path: &Path, time: f64) -> bool {
        // Check manifest clip first
        if let Some(ref manifest_clip) = self.manifest_clip {
            if manifest_clip.is_blocked(path, time) {
                return true;
            }
        }

        // Check value clips - if any clip is blocked at this time, return true
        for clip in &self.value_clips {
            if clip.is_blocked(path, time) {
                return true;
            }
        }

        false
    }
}

// -------------------------------------------------------------------------- //
// stage.cpp helpers: clips + time-varying (C++ `_ClipsApplyToLayerStackSite`,
// `_ClipsContainValueForAttribute`, `_HasTimeSamples`, `_ValueFromClipsMightBeTimeVarying`)
// -------------------------------------------------------------------------- //

/// Matches C++ `_ClipsApplyToNode`.
pub(crate) fn clips_apply_to_node(clip_set: &ClipSet, node: &NodeRef) -> bool {
    let Some(node_ls) = node.layer_stack() else {
        return false;
    };
    clip_set
        .source_layer_stack
        .as_ref()
        .is_some_and(|ls| ls.as_ref() == node_ls.as_ref())
        && node.path().has_prefix(&clip_set.source_prim_path)
}

/// Matches C++ `_GetClipsThatApplyToNode` (filter of `clipsAffectingPrim`).
pub(crate) fn get_clips_that_apply_to_node(
    clips_affecting_prim: &[ClipSetRefPtr],
    node: &NodeRef,
    attr_spec_path: &Path,
) -> Vec<ClipSetRefPtr> {
    clips_affecting_prim
        .iter()
        .filter(|cs| {
            clips_apply_to_node(cs.as_ref(), node)
                && clips_contain_value_for_attribute(cs.as_ref(), attr_spec_path)
        })
        .cloned()
        .collect()
}

/// Matches C++ `clipSet->sourceLayer == res->GetLayer()` in `_GetResolvedValueAtTimeWithClipsImpl`.
pub(crate) fn clip_source_layer_matches_resolver_layer(
    clip_set: &ClipSet,
    layer: &std::sync::Arc<usd_sdf::Layer>,
) -> bool {
    clip_set
        .source_layer
        .as_ref()
        .and_then(|h| h.upgrade())
        .map(|l| std::sync::Arc::ptr_eq(&l, layer) || l.identifier() == layer.identifier())
        .unwrap_or(false)
}

/// Matches C++ `_ClipsApplyToLayerStackSite`.
pub(crate) fn clips_apply_to_layer_stack_site(
    clip_set: &ClipSet,
    layer_stack: &Arc<LayerStack>,
    prim_path_in_layer_stack: &Path,
) -> bool {
    clip_set
        .source_layer_stack
        .as_ref()
        .is_some_and(|ls| ls == layer_stack)
        && prim_path_in_layer_stack.has_prefix(&clip_set.source_prim_path)
}

/// Matches C++ `_ClipsContainValueForAttribute`.
pub(crate) fn clips_contain_value_for_attribute(clip_set: &ClipSet, attr_spec_path: &Path) -> bool {
    use usd_tf::Token;
    let variability_token = Token::new("variability");
    if let Some(ref manifest_clip) = clip_set.manifest_clip {
        if manifest_clip.has_field(attr_spec_path, &variability_token) {
            if let Some(v) = manifest_clip
                .get_field_typed::<usd_sdf::Variability>(attr_spec_path, &variability_token)
            {
                return v == usd_sdf::Variability::Varying;
            }
        }
    }
    false
}

/// Matches C++ `_HasTimeSamples(clipSet, specPath)` when no bracketing time is requested.
pub(crate) fn clip_set_has_time_samples(clip_set: &ClipSet, spec_path: &Path) -> bool {
    if !clips_contain_value_for_attribute(clip_set, spec_path) {
        return false;
    }
    true
}

/// Matches C++ `_ValueFromClipsMightBeTimeVarying`.
pub(crate) fn value_from_clips_might_be_time_varying(
    clip_set: &ClipSet,
    attr_spec_path: &Path,
) -> bool {
    if clip_set.value_clips.len() == 1 {
        let clip = &clip_set.value_clips[0];
        if let Some(layer) = clip.get_layer_for_clip() {
            return layer.get_num_time_samples_for_path(attr_spec_path) > 1;
        }
        return false;
    }
    true
}
