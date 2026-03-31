//! Usd_ClipSetDefinition - collection of metadata that uniquely defines a clip set.
//!
//! Port of pxr/usd/usd/clipSetDefinition.h/cpp
//!
//! Collection of metadata from scene description and other information that
//! uniquely defines a clip set.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use usd_pcp::LayerStack;
use usd_sdf::{AssetPath, Path};
use usd_vt::{Array, Vec2dArray};

// ============================================================================
// ClipSetDefinition
// ============================================================================

/// Collection of metadata from scene description and other information that
/// uniquely defines a clip set.
///
/// Matches C++ `Usd_ClipSetDefinition`.
#[derive(Debug, Clone)]
pub struct ClipSetDefinition {
    /// Optional array of clip asset paths.
    pub clip_asset_paths: Option<Array<AssetPath>>,
    /// Optional manifest asset path.
    pub clip_manifest_asset_path: Option<AssetPath>,
    /// Optional clip prim path (as string).
    pub clip_prim_path: Option<String>,
    /// Optional array of active clip times (Vec2d).
    pub clip_active: Option<Vec2dArray>,
    /// Optional array of clip times (Vec2d).
    pub clip_times: Option<Vec2dArray>,
    /// Optional flag for interpolating missing clip values.
    pub interpolate_missing_clip_values: Option<bool>,
    /// Source layer stack.
    pub source_layer_stack: Option<Arc<LayerStack>>,
    /// Source prim path.
    pub source_prim_path: Path,
    /// Index of layer where asset paths were found.
    pub index_of_layer_where_asset_paths_found: usize,
}

impl Default for ClipSetDefinition {
    fn default() -> Self {
        Self {
            clip_asset_paths: None,
            clip_manifest_asset_path: None,
            clip_prim_path: None,
            clip_active: None,
            clip_times: None,
            interpolate_missing_clip_values: Some(false),
            source_layer_stack: None,
            source_prim_path: Path::empty(),
            index_of_layer_where_asset_paths_found: 0,
        }
    }
}

impl PartialEq for ClipSetDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.clip_asset_paths == other.clip_asset_paths
            && self.clip_manifest_asset_path == other.clip_manifest_asset_path
            && self.clip_prim_path == other.clip_prim_path
            && self.clip_active == other.clip_active
            && self.clip_times == other.clip_times
            && self.interpolate_missing_clip_values == other.interpolate_missing_clip_values
            && self.source_layer_stack.as_ref().map(Arc::as_ptr)
                == other.source_layer_stack.as_ref().map(Arc::as_ptr)
            && self.source_prim_path == other.source_prim_path
            && self.index_of_layer_where_asset_paths_found
                == other.index_of_layer_where_asset_paths_found
    }
}

impl Eq for ClipSetDefinition {}

impl Hash for ClipSetDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index_of_layer_where_asset_paths_found.hash(state);
        self.source_prim_path.hash(state);

        // Hash layer stack pointer
        if let Some(ref stack) = self.source_layer_stack {
            Arc::as_ptr(stack).hash(state);
        }

        if let Some(ref paths) = self.clip_asset_paths {
            // Hash array by converting to debug string (since Array may contain non-Hash types)
            format!("{:?}", paths).hash(state);
        }

        if let Some(ref manifest) = self.clip_manifest_asset_path {
            manifest.hash(state);
        }

        if let Some(ref prim_path) = self.clip_prim_path {
            prim_path.hash(state);
        }

        // Hash Vec2dArray by converting to debug string (since f64 doesn't implement Hash)
        if let Some(ref active) = self.clip_active {
            format!("{:?}", active).hash(state);
        }

        if let Some(ref times) = self.clip_times {
            format!("{:?}", times).hash(state);
        }

        if let Some(ref interpolate) = self.interpolate_missing_clip_values {
            interpolate.hash(state);
        }
    }
}

impl ClipSetDefinition {
    /// Creates a new empty clip set definition.
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes a hash value for this definition.
    ///
    /// Matches C++ `GetHash()`.
    pub fn get_hash(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

// ============================================================================
// Internal Helper Structures
// ============================================================================

/// Internal structure for tracking clip set composition.
struct ClipSet {
    name: String,
    anchor_info: AnchorInfo,
    clip_info: usd_vt::Dictionary,
}

/// Anchor information for a clip set.
struct AnchorInfo {
    layer_stack: Option<Arc<LayerStack>>,
    prim_path: Path,
    layer_index: usize,
    layer_stack_order: usize,
    offset: usd_sdf::LayerOffset,
}

impl ClipSet {
    fn new(name: String) -> Self {
        Self {
            name,
            anchor_info: AnchorInfo {
                layer_stack: None,
                prim_path: Path::empty(),
                layer_index: 0,
                layer_stack_order: 0,
                offset: usd_sdf::LayerOffset::identity(),
            },
            clip_info: usd_vt::Dictionary::new(),
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Default clip offset value (signifies not specified).
const DEFAULT_CLIP_OFFSET_VALUE: f64 = f64::MAX;

/// Gets the layer offset to root for a node and layer.
///
/// Matches C++ `_GetLayerOffsetToRoot`.
fn get_layer_offset_to_root(
    node: &usd_pcp::NodeRef,
    layer: &Arc<usd_sdf::Layer>,
) -> usd_sdf::LayerOffset {
    // Get the node-local path and layer offset
    let map_expression = node.map_to_root();
    let node_to_root_offset = map_expression.time_offset();

    // Each sublayer may have a layer offset, so we must adjust the time accordingly
    let mut local_offset = node_to_root_offset;

    // Get layer offset for this layer in the layer stack
    if let Some(layer_stack) = node.layer_stack() {
        if let Some(layer_to_root_offset) = layer_stack.get_layer_offset(layer) {
            local_offset = local_offset * layer_to_root_offset;
        }
    }

    local_offset
}

/// Applies layer offset to external times in a Vec2dArray.
///
/// Matches C++ `_ApplyLayerOffsetToExternalTimes`.
fn apply_layer_offset_to_external_times(
    layer_offset: &usd_sdf::LayerOffset,
    array: &mut Vec2dArray,
) {
    if layer_offset.is_identity() {
        return;
    }

    // Apply offset to first component (external time) of each Vec2d
    // Vec2dArray is Array<Vec2d>, so we need to create a new array with modified values
    let mut new_array = Vec::new();
    for time_pair in array.iter() {
        let mut new_pair = *time_pair;
        new_pair[0] = layer_offset.apply(time_pair[0]);
        new_array.push(new_pair);
    }
    *array = Vec2dArray::from(new_array);
}

/// Applies layer offset to clip info for a specific field.
///
/// Matches C++ `_ApplyLayerOffsetToClipInfo`.
fn apply_layer_offset_to_clip_info(
    node: &usd_pcp::NodeRef,
    layer: &Arc<usd_sdf::Layer>,
    info_key: &usd_tf::Token,
    clip_info: &mut usd_vt::Dictionary,
) {
    if let Some(value) = clip_info.get(info_key.get_text()) {
        if let Some(vec2d_array) = value.get::<Vec2dArray>() {
            let offset = get_layer_offset_to_root(node, layer);
            let mut vec2d_array_mut = vec2d_array.clone();
            apply_layer_offset_to_external_times(&offset, &mut vec2d_array_mut);
            clip_info.insert_value(
                info_key.get_text(),
                usd_vt::Value::from_no_hash(vec2d_array_mut),
            );
        }
    }
}

/// Records anchor info for a clip set.
///
/// Matches C++ `_RecordAnchorInfo`.
fn record_anchor_info(
    node: &usd_pcp::NodeRef,
    layer_idx: usize,
    clip_info: &usd_vt::Dictionary,
    clip_set: &mut ClipSet,
) {
    // A clip set is anchored to the strongest site containing opinions
    // about asset paths
    use crate::clips_api::ClipsAPIInfoKeys;

    if clip_info.contains_key(ClipsAPIInfoKeys::asset_paths().get_text())
        || clip_info.contains_key(ClipsAPIInfoKeys::template_asset_path().get_text())
    {
        let path = node.path();
        let layer_stack = node.layer_stack();

        if let Some(ref stack) = layer_stack {
            let layers = stack.get_layers();
            if layer_idx < layers.len() {
                let layer = &layers[layer_idx];
                clip_set.anchor_info = AnchorInfo {
                    layer_stack: Some(stack.clone()),
                    prim_path: path,
                    layer_index: layer_idx,
                    layer_stack_order: 0, // Will be filled in later
                    offset: get_layer_offset_to_root(node, layer),
                };
            }
        }
    }
}

/// Resolves clip sets in a node.
///
/// Matches C++ `_ResolveClipSetsInNode`.
fn resolve_clip_sets_in_node(
    node: &usd_pcp::NodeRef,
    result: &mut std::collections::BTreeMap<String, ClipSet>,
) {
    if !node.has_value_clips() {
        return;
    }

    let prim_path = node.path();
    let layer_stack = node.layer_stack();
    let Some(ref stack) = layer_stack else {
        return;
    };

    let layers = stack.get_layers();
    let clips_token = crate::tokens::UsdTokens::new().clips;
    let clip_sets_token = crate::tokens::UsdTokens::new().clip_sets;
    use crate::clips_api::ClipsAPIInfoKeys;

    // Iterate from weak-to-strong to build up the composed clip info
    // dictionaries for each clip set
    let mut clip_sets_in_node: std::collections::BTreeMap<String, ClipSet> =
        std::collections::BTreeMap::new();
    let mut added_clip_sets: Vec<String> = Vec::new();

    for i in (0..layers.len()).rev() {
        let layer = &layers[i];

        // Get clips dictionary
        if let Some(clips_value) = layer.get_field(&prim_path, &clips_token) {
            if let Some(clips_dict) = clips_value.get::<usd_vt::Dictionary>() {
                let mut clip_sets_in_layer: Vec<String> = Vec::new();

                for (clip_set_name, clip_info_value) in clips_dict.iter() {
                    if clip_set_name.is_empty() {
                        // Invalid unnamed clip set - skip
                        continue;
                    }

                    if let Some(clip_info_dict) = clip_info_value.get::<usd_vt::Dictionary>() {
                        let clip_set = clip_sets_in_node
                            .entry(clip_set_name.clone())
                            .or_insert_with(|| ClipSet::new(clip_set_name.clone()));

                        let mut clip_info_for_layer = clip_info_dict.clone();

                        record_anchor_info(node, i, &clip_info_for_layer, clip_set);

                        apply_layer_offset_to_clip_info(
                            node,
                            layer,
                            &ClipsAPIInfoKeys::active(),
                            &mut clip_info_for_layer,
                        );
                        apply_layer_offset_to_clip_info(
                            node,
                            layer,
                            &ClipsAPIInfoKeys::times(),
                            &mut clip_info_for_layer,
                        );

                        // Compose clip info recursively
                        use usd_vt::ValueComposable;
                        let current_clip_info =
                            std::mem::replace(&mut clip_set.clip_info, usd_vt::Dictionary::new());
                        clip_set.clip_info = current_clip_info.compose_over(clip_info_for_layer);

                        clip_sets_in_layer.push(clip_set_name.clone());
                    }
                }

                // Sort clip sets lexicographically for stable default sort order
                clip_sets_in_layer.sort();

                // Treat clip sets specified in clips dictionary as though
                // they were added in clipSets list op
                let mut add_list_op = usd_sdf::ListOp::<String>::new();
                add_list_op.set_appended_items(clip_sets_in_layer).ok();
                add_list_op.apply_operations(
                    &mut added_clip_sets,
                    None::<fn(usd_sdf::ListOpType, &String) -> Option<String>>,
                );
            }
        }

        // Get clipSets list op
        if let Some(clip_sets_value) = layer.get_field(&prim_path, &clip_sets_token) {
            if let Some(clip_sets_list_op) = clip_sets_value.get::<usd_sdf::ListOp<String>>() {
                clip_sets_list_op.apply_operations(
                    &mut added_clip_sets,
                    None::<fn(usd_sdf::ListOpType, &String) -> Option<String>>,
                );
            }
        }
    }

    // Filter out composed clip sets that aren't in added_clip_sets list
    clip_sets_in_node.retain(|name, clip_set| {
        if let Some(pos) = added_clip_sets.iter().position(|n| n == name) {
            if clip_set.anchor_info.layer_stack.is_some() {
                clip_set.anchor_info.layer_stack_order = pos;
            }
            true
        } else {
            false
        }
    });

    *result = clip_sets_in_node;
}

/// Derives clip info from template asset path.
///
/// Matches C++ `_DeriveClipInfo`.
fn derive_clip_info(
    template_asset_path: &str,
    stride: f64,
    active_offset: f64,
    start_time_code: f64,
    end_time_code: f64,
    clip_times: &mut Option<Vec2dArray>,
    clip_active: &mut Option<Vec2dArray>,
    clip_asset_paths: &mut Option<Array<AssetPath>>,
    _usd_prim_path: &Path,
    source_layer_stack: &Arc<LayerStack>,
    index_of_source_layer: usize,
) {
    if stride <= 0.0 {
        // Invalid stride - return without setting values
        return;
    }

    let active_offset_provided = active_offset != DEFAULT_CLIP_OFFSET_VALUE;
    if active_offset_provided && active_offset.abs() > stride {
        // Invalid active offset - return without setting values
        return;
    }

    // Split template asset path into path and args
    let (template_layer_path, args) = usd_sdf::layer_utils::split_identifier(template_asset_path);

    // Extract hash sections from basename
    let path_std = std::path::Path::new(template_layer_path);
    let basename = path_std.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let path_dir = path_std.parent().and_then(|p| p.to_str()).unwrap_or("");

    let tokenized_basename: Vec<&str> = basename.split('.').collect();

    let mut integer_hash_section_index = usize::MAX;
    let mut decimal_hash_section_index = usize::MAX;
    let mut num_integer_hashes = 0;
    let mut num_decimal_hashes = 0;
    let mut matching_groups = 0;

    for (token_index, token) in tokenized_basename.iter().enumerate() {
        if token.chars().all(|c| c == '#') {
            if integer_hash_section_index == usize::MAX {
                num_integer_hashes = token.len();
                integer_hash_section_index = token_index;
            } else {
                num_decimal_hashes = token.len();
                decimal_hash_section_index = token_index;
            }
            matching_groups += 1;
        }
    }

    if (matching_groups != 1 && matching_groups != 2)
        || (matching_groups == 2 && integer_hash_section_index != decimal_hash_section_index - 1)
    {
        // Invalid template format - return without setting values
        return;
    }

    if start_time_code > end_time_code {
        // Invalid time range - return without setting values
        return;
    }

    // Initialize output arrays
    *clip_times = Some(Vec2dArray::new());
    *clip_active = Some(Vec2dArray::new());
    *clip_asset_paths = Some(Array::new());

    let clip_times_mut = clip_times.as_mut().expect("just initialized");
    let clip_active_mut = clip_active.as_mut().expect("just initialized");
    let clip_asset_paths_mut = clip_asset_paths.as_mut().expect("just initialized");

    let layers = source_layer_stack.get_layers();
    if index_of_source_layer >= layers.len() {
        return;
    }
    let source_layer = &layers[index_of_source_layer];

    // Promotion factor for fractional stride handling
    const PROMOTION: f64 = 10000.0;
    let mut clip_active_index = 0;

    // If we have an activeOffset, author a knot on the front
    if active_offset_provided {
        let promoted_start = start_time_code * PROMOTION;
        let promoted_offset = active_offset.abs() * PROMOTION;
        let clip_time = (promoted_start - promoted_offset) / PROMOTION;
        clip_times_mut.push(usd_gf::vec2::Vec2d::new(clip_time, clip_time));
    }

    // Generate clips for each time step
    let mut t = start_time_code * PROMOTION;
    let end_promoted = end_time_code * PROMOTION;
    let stride_promoted = stride * PROMOTION;

    while t <= end_promoted {
        let clip_time = t / PROMOTION;

        // Derive clip time string
        let integer_portion = format!("{:0width$}", clip_time as i64, width = num_integer_hashes);
        let decimal_portion = if num_decimal_hashes > 0 {
            let decimal_part =
                (clip_time - clip_time.floor()) * (10.0_f64.powi(num_decimal_hashes as i32));
            format!(
                "{:0width$}",
                decimal_part as i64,
                width = num_decimal_hashes
            )
        } else {
            String::new()
        };

        // Build file path
        let mut new_basename = tokenized_basename.clone();
        new_basename[integer_hash_section_index] = &integer_portion;
        if num_decimal_hashes > 0 {
            new_basename[decimal_hash_section_index] = &decimal_portion;
        }

        let file_path = if path_dir.is_empty() {
            new_basename.join(".")
        } else {
            format!("{}/{}", path_dir, new_basename.join("."))
        };

        // Compute asset path relative to source layer
        let computed_path =
            usd_sdf::layer_utils::compute_asset_path_relative_to_layer(source_layer, &file_path);

        // Check if file exists using ArResolver
        use usd_ar::resolver::get_resolver;
        let resolver = get_resolver();
        let resolved_path = resolver
            .read()
            .expect("rwlock poisoned")
            .resolve(&computed_path);
        if resolved_path.is_empty() {
            // File doesn't exist, skip this asset path
            continue;
        }
        let asset_path = if let Some(args_str) = args {
            usd_sdf::layer_utils::create_identifier_with_args(&computed_path, args_str)
        } else {
            computed_path
        };

        clip_asset_paths_mut.push(AssetPath::new(&asset_path));
        clip_times_mut.push(usd_gf::vec2::Vec2d::new(clip_time, clip_time));

        if active_offset_provided {
            let offset_time = (t + (active_offset * PROMOTION)) / PROMOTION;
            clip_active_mut.push(usd_gf::vec2::Vec2d::new(
                offset_time,
                clip_active_index as f64,
            ));
        } else {
            clip_active_mut.push(usd_gf::vec2::Vec2d::new(
                clip_time,
                clip_active_index as f64,
            ));
        }

        clip_active_index += 1;
        t += stride_promoted;
    }

    // If we have an offset, author a knot on the end
    if active_offset_provided {
        let promoted_end = end_time_code * PROMOTION;
        let promoted_offset = active_offset.abs() * PROMOTION;
        let clip_time = (promoted_end + promoted_offset) / PROMOTION;
        clip_times_mut.push(usd_gf::vec2::Vec2d::new(clip_time, clip_time));
    }
}

/// Sets info from dictionary if present.
fn set_info<T: Clone + 'static>(
    dict: &usd_vt::Dictionary,
    key: &usd_tf::Token,
    out: &mut Option<T>,
) -> bool {
    if let Some(value) = dict.get(key.get_text()) {
        if let Some(typed_value) = value.get::<T>() {
            *out = Some(typed_value.clone());
            return true;
        }
    }
    false
}

/// Gets info from dictionary if present.
fn get_info<T: Clone + 'static>(dict: &usd_vt::Dictionary, key: &usd_tf::Token) -> Option<T> {
    dict.get(key.get_text()).and_then(|v| v.get::<T>().cloned())
}

/// Computes clip set definitions for the given prim index.
///
/// Matches C++ `Usd_ComputeClipSetDefinitionsForPrimIndex`.
///
/// The clip sets in the returned vector are sorted in strength order.
pub fn compute_clip_set_definitions_for_prim_index(
    prim_index: &usd_pcp::PrimIndex,
    clip_set_definitions: &mut Vec<ClipSetDefinition>,
    clip_set_names: &mut Vec<String>,
) {
    use crate::resolver::Resolver;

    let mut composed_clip_sets: std::collections::BTreeMap<String, ClipSet> =
        std::collections::BTreeMap::new();

    // Iterate over all nodes from strong to weak to compose all clip sets
    let prim_index_arc = Arc::new(prim_index.clone());
    let mut resolver = Resolver::new(&prim_index_arc, false);

    while resolver.is_valid() {
        let node = resolver.get_node();
        let mut clip_sets_in_node: std::collections::BTreeMap<String, ClipSet> =
            std::collections::BTreeMap::new();
        if let Some(node_ref) = node {
            resolve_clip_sets_in_node(&node_ref, &mut clip_sets_in_node);
        }

        for (clip_set_name, node_clip_set) in clip_sets_in_node {
            let composed_clip_set = composed_clip_sets
                .entry(clip_set_name.clone())
                .or_insert_with(|| ClipSet::new(clip_set_name));

            if composed_clip_set.anchor_info.layer_stack.is_none() {
                composed_clip_set.anchor_info = node_clip_set.anchor_info;
            }

            // Compose clip info recursively
            use usd_vt::ValueComposable;
            let current_clip_info =
                std::mem::replace(&mut composed_clip_set.clip_info, usd_vt::Dictionary::new());
            composed_clip_set.clip_info =
                current_clip_info.compose_over(node_clip_set.clip_info.clone());
        }

        resolver.next_node();
    }

    // Remove all clip sets that have no anchor info
    composed_clip_sets.retain(|_, clip_set| clip_set.anchor_info.layer_stack.is_some());

    if composed_clip_sets.is_empty() {
        return;
    }

    // Sort clip sets by layer stack, prim path, and layer stack order
    let mut sorted_clip_sets: Vec<ClipSet> = composed_clip_sets.into_values().collect();
    sorted_clip_sets.sort_by(|x, y| {
        use std::cmp::Ordering;

        // Compare layer stack pointers
        let x_stack_ptr = x.anchor_info.layer_stack.as_ref().map(Arc::as_ptr);
        let y_stack_ptr = y.anchor_info.layer_stack.as_ref().map(Arc::as_ptr);

        match (x_stack_ptr, y_stack_ptr) {
            (Some(xp), Some(yp)) => {
                let ptr_cmp = xp.cmp(&yp);
                if ptr_cmp != Ordering::Equal {
                    return ptr_cmp;
                }
            }
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {}
        }

        // Compare prim paths
        let path_cmp = x.anchor_info.prim_path.cmp(&y.anchor_info.prim_path);
        if path_cmp != Ordering::Equal {
            return path_cmp;
        }

        // Compare layer stack order
        x.anchor_info
            .layer_stack_order
            .cmp(&y.anchor_info.layer_stack_order)
    });

    // Unpack the information into ClipSetDefinition objects
    clip_set_definitions.reserve(sorted_clip_sets.len());
    if !clip_set_names.is_empty() {
        clip_set_names.reserve(sorted_clip_sets.len());
    }

    use crate::clips_api::ClipsAPIInfoKeys;

    for clip_set in sorted_clip_sets {
        let mut out = ClipSetDefinition::new();

        if !clip_set_names.is_empty() {
            clip_set_names.push(clip_set.name);
        }

        out.source_layer_stack = clip_set.anchor_info.layer_stack;
        out.source_prim_path = clip_set.anchor_info.prim_path;
        out.index_of_layer_where_asset_paths_found = clip_set.anchor_info.layer_index;

        let clip_info = &clip_set.clip_info;

        set_info(
            clip_info,
            &ClipsAPIInfoKeys::prim_path(),
            &mut out.clip_prim_path,
        );
        set_info(
            clip_info,
            &ClipsAPIInfoKeys::manifest_asset_path(),
            &mut out.clip_manifest_asset_path,
        );
        set_info(
            clip_info,
            &ClipsAPIInfoKeys::interpolate_missing_clip_values(),
            &mut out.interpolate_missing_clip_values,
        );

        if set_info(
            clip_info,
            &ClipsAPIInfoKeys::asset_paths(),
            &mut out.clip_asset_paths,
        ) {
            set_info(clip_info, &ClipsAPIInfoKeys::active(), &mut out.clip_active);
            set_info(clip_info, &ClipsAPIInfoKeys::times(), &mut out.clip_times);
        } else if let Some(template_asset_path) =
            get_info::<String>(clip_info, &ClipsAPIInfoKeys::template_asset_path())
        {
            let template_active_offset =
                get_info::<f64>(clip_info, &ClipsAPIInfoKeys::template_active_offset())
                    .unwrap_or(DEFAULT_CLIP_OFFSET_VALUE);
            let template_stride = get_info::<f64>(clip_info, &ClipsAPIInfoKeys::template_stride());
            let template_start_time =
                get_info::<f64>(clip_info, &ClipsAPIInfoKeys::template_start_time());
            let template_end_time =
                get_info::<f64>(clip_info, &ClipsAPIInfoKeys::template_end_time());

            if let (Some(stride), Some(start_time), Some(end_time)) =
                (template_stride, template_start_time, template_end_time)
            {
                derive_clip_info(
                    &template_asset_path,
                    stride,
                    template_active_offset,
                    start_time,
                    end_time,
                    &mut out.clip_times,
                    &mut out.clip_active,
                    &mut out.clip_asset_paths,
                    &prim_index.path(),
                    out.source_layer_stack
                        .as_ref()
                        .expect("source layer stack set"),
                    out.index_of_layer_where_asset_paths_found,
                );

                // Apply layer offsets to clipActive and clipTimes
                if let Some(ref clip_times) = out.clip_times {
                    let mut clip_times_mut = clip_times.clone();
                    apply_layer_offset_to_external_times(
                        &clip_set.anchor_info.offset,
                        &mut clip_times_mut,
                    );
                    out.clip_times = Some(clip_times_mut);
                }

                if let Some(ref clip_active) = out.clip_active {
                    let mut clip_active_mut = clip_active.clone();
                    apply_layer_offset_to_external_times(
                        &clip_set.anchor_info.offset,
                        &mut clip_active_mut,
                    );
                    out.clip_active = Some(clip_active_mut);
                }
            }
        }

        clip_set_definitions.push(out);
    }
}
