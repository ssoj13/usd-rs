//! Usd_Clip - represents a clip from which time samples may be read.
//!
//! Port of pxr/usd/usd/clip.h/cpp
//!
//! Represents a clip from which time samples may be read during value resolution.
//! A clip has two time domains: an external (stage) and an internal (clip layer) domain.
//! TimeMapping objects specify the mapping from external to internal time.

use crate::time_code::TimeCode;
use ordered_float::OrderedFloat;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use usd_pcp::LayerStack;
use usd_sdf::{AssetPath, Layer, LayerHandle, Path, PropertySpec};
use usd_tf::Token;
use usd_vt;

// ============================================================================
// Constants
// ============================================================================

/// Sentinel values for clip times.
/// Represents the earliest possible clip time.
pub const CLIP_TIMES_EARLIEST: f64 = f64::NEG_INFINITY;
/// Represents the latest possible clip time.
pub const CLIP_TIMES_LATEST: f64 = f64::INFINITY;

// ============================================================================
// TimeMapping
// ============================================================================

/// Mapping from external time to internal time.
///
/// Matches C++ `Usd_Clip::TimeMapping`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeMapping {
    /// External time (stage time).
    pub external_time: f64,
    /// Internal time (clip time).
    pub internal_time: f64,
    /// Whether this represents a jump discontinuity.
    pub is_jump_discontinuity: bool,
}

impl TimeMapping {
    /// Creates a new time mapping.
    pub fn new(external_time: f64, internal_time: f64) -> Self {
        Self {
            external_time,
            internal_time,
            is_jump_discontinuity: false,
        }
    }
}

/// Vector of time mappings.
///
/// Matches C++ `Usd_Clip::TimeMappings`.
pub type TimeMappings = Vec<TimeMapping>;

// ============================================================================
// Clip
// ============================================================================

/// Represents a clip from which time samples may be read during value resolution.
///
/// Matches C++ `Usd_Clip`.
///
/// A clip has two time domains: an external and an internal domain.
/// The internal time domain is what is authored in the clip layer.
/// The external time domain is what is used by clients of Clip.
pub struct Clip {
    /// Layer stack, prim spec path, and layer where this clip was introduced.
    pub source_layer_stack: Option<Arc<LayerStack>>,
    /// Prim spec path where this clip was introduced.
    pub source_prim_path: Path,
    /// Layer where this clip was introduced.
    pub source_layer: Option<LayerHandle>,

    /// Asset path for the clip and the path to the prim in the clip that provides data.
    pub asset_path: AssetPath,
    /// Path to the prim in the clip that provides data.
    pub prim_path: Path,

    /// The authored start time for this clip.
    pub authored_start_time: f64,

    /// A clip is active in the time range [start_time, end_time).
    /// Start time of the clip's active range.
    pub start_time: f64,
    /// End time of the clip's active range.
    pub end_time: f64,

    /// Mapping of external to internal times.
    pub times: Arc<TimeMappings>,

    /// Cached layer (lazy-loaded).
    layer: Mutex<Option<Arc<Layer>>>,
    has_layer: Mutex<bool>,
}

impl Clip {
    /// Creates a new clip.
    ///
    /// Matches C++ `Usd_Clip::Usd_Clip(...)` constructor.
    pub fn new(
        clip_source_layer_stack: Option<Arc<LayerStack>>,
        clip_source_prim_path: Path,
        clip_source_layer_index: usize,
        clip_asset_path: AssetPath,
        clip_prim_path: Path,
        clip_authored_start_time: f64,
        clip_start_time: f64,
        clip_end_time: f64,
        time_mapping: Option<Arc<TimeMappings>>,
    ) -> Self {
        // Get source layer from layer stack if available
        let source_layer = clip_source_layer_stack.as_ref().and_then(|stack| {
            let layers = stack.get_layers();
            if clip_source_layer_index < layers.len() {
                Some(LayerHandle::from_layer(&layers[clip_source_layer_index]))
            } else {
                None
            }
        });

        // Try to find layer if source layer is available
        // For performance reasons, we want to defer loading until needed,
        // but if the layer is already open, we can take advantage of that
        let initial_layer = if let Some(ref src_layer) = source_layer {
            // LayerHandle is Arc<Layer>, so we can use it directly
            if let Some(layer_arc) = src_layer.upgrade() {
                Layer::find_relative_to_layer(&layer_arc, clip_asset_path.get_asset_path())
            } else {
                None
            }
        } else {
            None
        };

        let has_layer = initial_layer.is_some();

        // Use empty time mappings if None provided
        let time_mapping = time_mapping.unwrap_or_else(|| Arc::new(Vec::new()));

        Self {
            source_layer_stack: clip_source_layer_stack,
            source_prim_path: clip_source_prim_path,
            source_layer,
            asset_path: clip_asset_path,
            prim_path: clip_prim_path,
            authored_start_time: clip_authored_start_time,
            start_time: clip_start_time,
            end_time: clip_end_time,
            times: time_mapping,
            layer: Mutex::new(initial_layer),
            has_layer: Mutex::new(has_layer),
        }
    }

    /// Returns true if the clip has the given field at the given path.
    ///
    /// Matches C++ `HasField(const SdfPath& path, const TfToken& field)`.
    pub fn has_field(&self, path: &Path, field: &Token) -> bool {
        if let Some(layer) = self.get_layer_for_clip() {
            let clip_path = self.translate_path_to_clip(path);
            return layer.has_field(&clip_path, field);
        }
        false
    }

    /// Returns the property spec at the given path.
    ///
    /// Matches C++ `GetPropertyAtPath(const SdfPath& path)`.
    pub fn get_property_at_path(&self, path: &Path) -> Option<PropertySpec> {
        if let Some(layer) = self.get_layer_for_clip() {
            let clip_path = self.translate_path_to_clip(path);
            return layer.get_property_at_path(&clip_path);
        }
        None
    }

    /// Returns the number of time samples for the given path.
    ///
    /// Matches C++ `GetNumTimeSamplesForPath(const SdfPath& path)`.
    pub fn get_num_time_samples_for_path(&self, path: &Path) -> usize {
        // Simple but inefficient - list all samples and count
        self.list_time_samples_for_path(path).len()
    }

    /// Lists all time samples for the given path.
    ///
    /// Matches C++ `ListTimeSamplesForPath(const SdfPath& path)`.
    pub fn list_time_samples_for_path(&self, path: &Path) -> Vec<f64> {
        let mut time_samples_set = BTreeSet::new();

        // Retrieve time samples from the clip layer mapped to external times
        self.list_time_samples_for_path_from_clip_layer(path, &mut time_samples_set);

        // Each entry in the clip's time mapping is considered a time sample
        for time_mapping in self.times.iter() {
            if self.start_time <= time_mapping.external_time
                && time_mapping.external_time < self.end_time
            {
                // Use a wrapper that implements Ord for f64
                time_samples_set.insert(OrderedFloat::from(time_mapping.external_time));
            }
        }

        // Clips introduce time samples at their start time to isolate them
        // from surrounding clips
        time_samples_set.insert(OrderedFloat::from(self.authored_start_time));

        // Convert back to Vec<f64>
        time_samples_set.into_iter().map(|f| f.0).collect()
    }

    /// Gets bracketing time samples for the given path and time.
    ///
    /// Matches C++ `GetBracketingTimeSamplesForPath(...)`.
    pub fn get_bracketing_time_samples_for_path(
        &self,
        path: &Path,
        time: f64,
    ) -> Option<(f64, f64)> {
        let mut bracketing_times = [0.0; 5];
        let mut num_times = 0;

        // Add time samples from the clip layer
        if let Some((lower, upper)) =
            self.get_bracketing_time_samples_for_path_from_clip_layer(path, time)
        {
            bracketing_times[num_times] = lower;
            bracketing_times[num_times + 1] = upper;
            num_times += 2;
        }

        // Each external time in the clip times array is considered a time sample
        if let Some((lower, upper)) = self.get_bracketing_time_samples_from_mappings(time) {
            bracketing_times[num_times] = lower;
            bracketing_times[num_times + 1] = upper;
            num_times += 2;
        }

        // Clips introduce time samples at their start time even if time samples
        // don't actually exist. This isolates each clip from its neighbors.
        bracketing_times[num_times] = self.authored_start_time;
        num_times += 1;

        // Remove bracketing times that are outside the clip's active range
        let mut filtered_times: Vec<f64> = bracketing_times[..num_times]
            .iter()
            .copied()
            .filter(|&t| t >= self.start_time && t < self.end_time)
            .collect();

        if filtered_times.is_empty() {
            return None;
        }

        if filtered_times.len() == 1 {
            let t = filtered_times[0];
            return Some((t, t));
        }

        // Sort and deduplicate
        filtered_times.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        filtered_times.dedup();

        // Find bracketing times
        self.get_bracketing_time_samples_from_sorted(&filtered_times, time)
    }

    /// Returns true if this clip has authored time samples for the attribute
    /// corresponding to the given path.
    ///
    /// Matches C++ `HasAuthoredTimeSamples(const SdfPath& path)`.
    pub fn has_authored_time_samples(&self, path: &Path) -> bool {
        if let Some(layer) = self.get_layer_for_clip() {
            let clip_path = self.translate_path_to_clip(path);
            return layer.get_num_time_samples_for_path(&clip_path) > 0;
        }
        false
    }

    /// Returns true if a value block is authored for the attribute
    /// corresponding to the given path at the given time.
    ///
    /// Matches C++ `IsBlocked(const SdfPath& path, ExternalTime time)`.
    pub fn is_blocked(&self, path: &Path, time: f64) -> bool {
        if let Some(layer) = self.get_layer_for_clip() {
            let clip_path = self.translate_path_to_clip(path);
            let internal_time = self.translate_time_to_internal(TimeCode::new(time));

            if let Some(value) = layer.query_time_sample(&clip_path, internal_time) {
                use usd_sdf::types::ValueBlock;
                return value.get::<ValueBlock>().is_some();
            }
        }
        false
    }

    /// Returns the layer associated with this clip, opening it if it hasn't
    /// been opened already.
    ///
    /// Matches C++ `GetLayer()`.
    pub fn get_layer(&self) -> Option<LayerHandle> {
        let layer = self.get_layer_for_clip()?;

        // Check if this is a dummy clip layer
        let identifier = layer.identifier();
        if identifier.starts_with("dummy_clip.") {
            return None;
        }

        Some(LayerHandle::from_layer(&layer))
    }

    /// Returns the layer associated with this clip iff it has already been
    /// opened successfully.
    ///
    /// Matches C++ `GetLayerIfOpen()`.
    pub fn get_layer_if_open(&self) -> Option<LayerHandle> {
        let has_layer = *self.has_layer.lock().expect("lock poisoned");
        if !has_layer {
            return None;
        }
        self.get_layer()
    }

    /// Query a time sample value (untyped) from the clip layer.
    ///
    /// Translates path and time to clip space, returns raw Value from the clip layer.
    /// Used by attribute resolution to fetch clip values without knowing the type upfront.
    pub fn query_time_sample_value(&self, path: &Path, time: f64) -> Option<usd_vt::Value> {
        let layer = self.get_layer_for_clip()?;
        let clip_path = self.translate_path_to_clip(path);
        let time_in_clip = self.translate_time_to_internal(TimeCode::new(time));
        layer.query_time_sample(&clip_path, time_in_clip)
    }

    /// Returns a field value of type T from the clip's manifest layer.
    ///
    /// Reads attribute spec metadata (e.g. variability) — NOT time-sampled data.
    /// Matches C++ `HasField` + `Get` on the manifest's SdfLayer.
    pub fn get_field_typed<T: Clone + 'static>(&self, path: &Path, field: &Token) -> Option<T> {
        let layer = self.get_layer_for_clip()?;
        let clip_path = self.translate_path_to_clip(path);
        let val = layer.get_field(&clip_path, field)?;
        val.downcast_clone::<T>()
    }

    /// Query a time sample value of type T from the clip layer.
    ///
    /// Matches C++ template `QueryTimeSampleTyped<T>`.
    pub fn query_time_sample_typed<T>(&self, path: &Path, time: f64) -> Option<T>
    where
        T: Clone + 'static,
    {
        if let Some(layer) = self.get_layer_for_clip() {
            let clip_path = self.translate_path_to_clip(path);
            let time_in_clip = self.translate_time_to_internal(TimeCode::new(time));

            if let Some(value) = layer.query_time_sample(&clip_path, time_in_clip) {
                if let Some(typed_value) = value.downcast_clone::<T>() {
                    return Some(typed_value);
                }
            }
        }
        None
    }

    // ========================================================================
    // Internal helpers
    // ========================================================================

    /// Gets the layer for this clip, opening it if necessary.
    pub(crate) fn get_layer_for_clip(&self) -> Option<Arc<Layer>> {
        {
            let has_layer = *self.has_layer.lock().expect("lock poisoned");
            if has_layer {
                let layer = self.layer.lock().expect("lock poisoned");
                return layer.clone();
            }
        }

        // Need to open the layer
        let layer = if let Some(ref source_layer) = self.source_layer {
            // LayerHandle is Arc<Layer>, so we can use it directly
            if let Some(layer_arc) = source_layer.upgrade() {
                Layer::find_or_open_relative_to_layer(&layer_arc, self.asset_path.get_asset_path())
                    .ok()
            } else {
                None
            }
        } else {
            None
        };

        let layer = layer.unwrap_or_else(|| {
            // If we failed to open the specified layer, create a dummy anonymous layer
            // to avoid having to check layer validity everywhere
            Layer::create_anonymous(Some("dummy_clip.usda"))
        });

        {
            let mut cached_layer = self.layer.lock().expect("lock poisoned");
            let mut has_layer_flag = self.has_layer.lock().expect("lock poisoned");
            if cached_layer.is_none() {
                *cached_layer = Some(layer);
                *has_layer_flag = true;
            }
            cached_layer.clone()
        }
    }

    /// Translates a path from stage space to clip space.
    fn translate_path_to_clip(&self, path: &Path) -> Path {
        path.replace_prefix(&self.source_prim_path, &self.prim_path)
            .unwrap_or_else(|| path.clone())
    }

    /// Translates external time to internal time.
    fn translate_time_to_internal(&self, ext_time: TimeCode) -> f64 {
        let time_value = ext_time.value();

        // Find the bracketing time segment
        let (i1, i2) = match self.get_bracketing_time_segment(time_value) {
            Some((i1, i2)) => (i1, i2),
            None => return time_value,
        };

        let m1 = &self.times[i1];
        let m2 = &self.times[i2];

        // Handle jump discontinuities
        if ext_time.is_pre_time() && m1.is_jump_discontinuity {
            return m1.internal_time;
        }

        if m2.is_jump_discontinuity && i2 + 1 < self.times.len() {
            let m3 = &self.times[i2 + 1];
            return self.translate_time_to_internal_helper(
                time_value,
                *m1,
                TimeMapping {
                    external_time: m3.external_time,
                    internal_time: m2.internal_time,
                    is_jump_discontinuity: false,
                },
            );
        }

        self.translate_time_to_internal_helper(time_value, *m1, *m2)
    }

    /// Translates internal time to external time.
    fn translate_time_to_external(&self, int_time: f64, i1: usize, i2: usize) -> f64 {
        let m1 = &self.times[i1];
        let m2 = &self.times[i2];

        // Clients should never be trying to map an internal time through a jump
        // discontinuity
        if m1.is_jump_discontinuity {
            return m1.external_time;
        }

        if m2.is_jump_discontinuity && i2 + 1 < self.times.len() {
            let m3 = &self.times[i2 + 1];
            return self.translate_time_to_external_helper(
                int_time,
                *m1,
                TimeMapping {
                    external_time: m3.external_time,
                    internal_time: m2.internal_time,
                    is_jump_discontinuity: false,
                },
            );
        }

        self.translate_time_to_external_helper(int_time, *m1, *m2)
    }

    /// Helper to translate external time to internal time.
    fn translate_time_to_internal_helper(
        &self,
        ext_time: f64,
        m1: TimeMapping,
        m2: TimeMapping,
    ) -> f64 {
        // Early out in special cases
        if m1.external_time == m2.external_time {
            return m1.internal_time;
        } else if ext_time == m1.external_time {
            return m1.internal_time;
        } else if ext_time == m2.external_time {
            return m2.internal_time;
        }

        // Linear interpolation
        (m2.internal_time - m1.internal_time) / (m2.external_time - m1.external_time)
            * (ext_time - m1.external_time)
            + m1.internal_time
    }

    /// Helper to translate internal time to external time.
    fn translate_time_to_external_helper(
        &self,
        int_time: f64,
        m1: TimeMapping,
        m2: TimeMapping,
    ) -> f64 {
        // Early out in special cases
        if m1.internal_time == m2.internal_time {
            return m1.external_time;
        } else if int_time == m1.internal_time {
            return m1.external_time;
        } else if int_time == m2.internal_time {
            return m2.external_time;
        }

        // Linear interpolation
        (m2.external_time - m1.external_time) / (m2.internal_time - m1.internal_time)
            * (int_time - m1.internal_time)
            + m1.external_time
    }

    /// Gets the bracketing time segment for the given time.
    fn get_bracketing_time_segment(&self, time: f64) -> Option<(usize, usize)> {
        if self.times.is_empty() {
            return None;
        }

        let times = &self.times;

        if time <= times[0].external_time {
            Some((0, 1.min(times.len() - 1)))
        } else if time >= times[times.len() - 1].external_time {
            let len = times.len();
            Some(((len - 2).max(0), len - 1))
        } else {
            // Binary search for the segment
            let i2 = times
                .binary_search_by(|m| {
                    m.external_time
                        .partial_cmp(&time)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .unwrap_or_else(|i| i);

            if i2 == 0 {
                return Some((0, 1));
            }

            let i1 = i2 - 1;
            Some((i1, i2))
        }
    }

    /// Gets bracketing time samples from clip layer.
    fn get_bracketing_time_samples_for_path_from_clip_layer(
        &self,
        path: &Path,
        time: f64,
    ) -> Option<(f64, f64)> {
        let layer = self.get_layer_for_clip()?;
        let clip_path = self.translate_path_to_clip(path);
        let time_in_clip = self.translate_time_to_internal(TimeCode::new(time));

        if let Some((lower_in_clip, upper_in_clip)) =
            layer.get_bracketing_time_samples_for_path(&clip_path, time_in_clip)
        {
            // Translate back to external time domain
            // This is complex because external -> internal mapping is many-to-one.
            // We find the bracketing time segment and then search for the closest
            // translation that maps the internal times back to external times.
            let (i1, _i2) = self.get_bracketing_time_segment(time)?;

            // Try to find the translation that is closest to the requested time
            let mut translated_lower = None;
            let mut translated_upper = None;

            // Walk backwards to find lower translation
            for i in (0..=i1).rev() {
                if i + 1 < self.times.len() {
                    let m1 = &self.times[i];
                    let m2 = &self.times[i + 1];

                    if !m1.is_jump_discontinuity {
                        let lower = m1.internal_time.min(m2.internal_time);
                        let upper = m1.internal_time.max(m2.internal_time);

                        if lower <= lower_in_clip && lower_in_clip <= upper {
                            if m1.internal_time != m2.internal_time {
                                translated_lower =
                                    Some(self.translate_time_to_external(lower_in_clip, i, i + 1));
                            } else {
                                translated_lower = Some(m1.external_time);
                            }
                            break;
                        }
                    }
                }
            }

            // Walk forwards to find upper translation
            for i in i1..self.times.len().saturating_sub(1) {
                let m1 = &self.times[i];
                let m2 = &self.times[i + 1];

                if !m1.is_jump_discontinuity {
                    let lower = m1.internal_time.min(m2.internal_time);
                    let upper = m1.internal_time.max(m2.internal_time);

                    if lower <= upper_in_clip && upper_in_clip <= upper {
                        if m1.internal_time != m2.internal_time {
                            translated_upper =
                                Some(self.translate_time_to_external(upper_in_clip, i, i + 1));
                        } else {
                            translated_upper = Some(m2.external_time);
                        }
                        break;
                    }
                }
            }

            // Fallback: use direct translation if available
            if translated_lower.is_none() {
                translated_lower = Some(lower_in_clip);
            }
            if translated_upper.is_none() {
                translated_upper = Some(upper_in_clip);
            }

            Some((
                translated_lower.expect("fallback set"),
                translated_upper.expect("fallback set"),
            ))
        } else {
            None
        }
    }

    /// Lists time samples from clip layer.
    fn list_time_samples_for_path_from_clip_layer(
        &self,
        path: &Path,
        time_samples: &mut BTreeSet<OrderedFloat<f64>>,
    ) {
        let layer = match self.get_layer_for_clip() {
            Some(l) => l,
            None => return,
        };

        let clip_path = self.translate_path_to_clip(path);
        let time_samples_in_clip = layer.list_time_samples_for_path(&clip_path);

        if self.times.is_empty() {
            // No time mapping - use samples directly, filtered by active range
            for t in time_samples_in_clip {
                if t >= self.start_time && t < self.end_time {
                    time_samples.insert(OrderedFloat::from(t));
                }
            }
            return;
        }

        // Convert internal time samples to external domain using time mappings
        // This is complex because the mapping is many-to-one
        for int_time in time_samples_in_clip {
            for i in 0..self.times.len().saturating_sub(1) {
                let m1 = &self.times[i];
                let m2 = &self.times[i + 1];

                // Check if this segment intersects the clip's active range
                let mapping_start = m1.external_time;
                let mapping_end = m2.external_time;

                if mapping_end < self.start_time || mapping_start >= self.end_time {
                    continue;
                }

                // Ignore jump discontinuities
                if m1.is_jump_discontinuity {
                    continue;
                }

                let lower = m1.internal_time.min(m2.internal_time);
                let upper = m1.internal_time.max(m2.internal_time);

                if lower <= int_time && int_time <= upper {
                    if m1.internal_time == m2.internal_time {
                        if mapping_start >= self.start_time && mapping_start < self.end_time {
                            time_samples.insert(OrderedFloat::from(mapping_start));
                        }
                        if mapping_end >= self.start_time && mapping_end < self.end_time {
                            time_samples.insert(OrderedFloat::from(mapping_end));
                        }
                    } else {
                        let ext_time = self.translate_time_to_external(int_time, i, i + 1);
                        if ext_time >= self.start_time && ext_time < self.end_time {
                            time_samples.insert(OrderedFloat::from(ext_time));
                        }
                    }
                }
            }
        }
    }

    /// Gets bracketing time samples from time mappings.
    fn get_bracketing_time_samples_from_mappings(&self, time: f64) -> Option<(f64, f64)> {
        if self.times.is_empty() {
            return None;
        }

        let times = &self.times;

        if time <= times[0].external_time {
            let t = times[0].external_time;
            Some((t, t))
        } else if time >= times[times.len() - 1].external_time {
            let t = times[times.len() - 1].external_time;
            Some((t, t))
        } else {
            // Binary search
            match times.binary_search_by(|m| {
                m.external_time
                    .partial_cmp(&time)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                Ok(i) => {
                    // Exact match
                    let t = times[i].external_time;
                    Some((t, t))
                }
                Err(i) => {
                    // Between two mappings
                    if i == 0 {
                        let t = times[0].external_time;
                        Some((t, t))
                    } else if i >= times.len() {
                        let t = times[times.len() - 1].external_time;
                        Some((t, t))
                    } else {
                        Some((times[i - 1].external_time, times[i].external_time))
                    }
                }
            }
        }
    }

    /// Gets bracketing time samples from a sorted list.
    fn get_bracketing_time_samples_from_sorted(
        &self,
        sorted_times: &[f64],
        time: f64,
    ) -> Option<(f64, f64)> {
        if sorted_times.is_empty() {
            return None;
        }

        if time <= sorted_times[0] {
            let t = sorted_times[0];
            Some((t, t))
        } else if time >= sorted_times[sorted_times.len() - 1] {
            let t = sorted_times[sorted_times.len() - 1];
            Some((t, t))
        } else {
            // Binary search
            match sorted_times
                .binary_search_by(|t| t.partial_cmp(&time).unwrap_or(std::cmp::Ordering::Equal))
            {
                Ok(i) => {
                    // Exact match
                    let t = sorted_times[i];
                    Some((t, t))
                }
                Err(i) => {
                    // Between two times
                    if i == 0 {
                        let t = sorted_times[0];
                        Some((t, t))
                    } else if i >= sorted_times.len() {
                        let t = sorted_times[sorted_times.len() - 1];
                        Some((t, t))
                    } else {
                        Some((sorted_times[i - 1], sorted_times[i]))
                    }
                }
            }
        }
    }
}

/// Reference-counted pointer to a clip.
pub type ClipRefPtr = Arc<Clip>;

/// Vector of clip references.
pub type ClipRefPtrVector = Vec<ClipRefPtr>;

// ============================================================================
// Helper Functions
// ============================================================================

/// Returns true if the given field name is associated with value clip functionality.
///
/// Matches C++ `UsdIsClipRelatedField(const TfToken& fieldName)`.
pub fn is_clip_related_field(field_name: &Token) -> bool {
    let name = field_name.as_str();
    name == "clips" || name == "clipSets"
}

/// Returns list of all field names associated with value clip functionality.
///
/// Matches C++ `UsdGetClipRelatedFields()`.
pub fn get_clip_related_fields() -> Vec<Token> {
    vec![Token::new("clips"), Token::new("clipSets")]
}
