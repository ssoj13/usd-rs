//! USD Attribute - typed property values.

use super::object::Stage;
use super::property::Property;
use std::sync::{Arc, Weak};
use usd_sdf::{AnimationBlock, LayerOffset, Path, TimeCode, ValueBlock};
use usd_tf::Token;
use usd_vt::{Value, spline::SplineValue, value_type_can_compose_over};

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static DEFAULT: LazyLock<Token> = LazyLock::new(|| Token::new("default"));
    pub static SPLINE: LazyLock<Token> = LazyLock::new(|| Token::new("spline"));
}

// ============================================================================
// Value composition helpers (C++ MetadataValueComposer pattern)
// ============================================================================

/// Result of consuming an authored value during layer walk.
enum ConsumeResult {
    /// Composition is complete, return this value.
    Done(Value),
    /// Value is composable, accumulated in partial — keep walking layers.
    Continue,
}

/// Consume an authored value during layer walk.
/// Matches C++ MetadataValueComposer::ConsumeAuthored pattern.
fn consume_authored(partial: &mut Option<Value>, value: Value) -> ConsumeResult {
    // If we have a partial composable value, try composing it over the new value
    if let Some(p) = partial.as_ref() {
        if let Some(composed) = usd_vt::value_try_compose_over(p, &value) {
            // Composed successfully. If result is still composable, keep going.
            if composed.is_array_edit_valued() {
                *partial = Some(composed);
                return ConsumeResult::Continue;
            }
            return ConsumeResult::Done(composed);
        }
    }

    // If value can compose over other values (e.g. ArrayEdit, Dict), accumulate
    if value.is_array_edit_valued() {
        *partial = Some(value);
        return ConsumeResult::Continue;
    }

    // Non-composable value: if we have partial, compose over it; else return as-is
    if let Some(p) = partial.as_ref() {
        if let Some(composed) = usd_vt::value_try_compose_over(p, &value) {
            return ConsumeResult::Done(composed);
        }
    }
    ConsumeResult::Done(value)
}

/// Finalize a partial composable value by composing over an empty background.
/// Matches C++ MetadataValueComposer::Finalize pattern.
fn finalize_partial(partial: Value) -> Value {
    if let Some(composed) = usd_vt::value_try_compose_over(&partial, &Value::empty()) {
        composed
    } else {
        partial
    }
}

/// Apply layer offset to time-mappable values (C++ Usd_ApplyLayerOffsetToValue).
///
/// Handles: SdfTimeCode (scalar), Vec<SdfTimeCode> (array).
/// Non-timecode values pass through unchanged.
pub(crate) fn apply_layer_offset_to_value(value: &mut Value, offset: &LayerOffset) {
    if offset.is_identity() {
        return;
    }
    if value.is::<TimeCode>() {
        if let Some(tc) = value.downcast_clone::<TimeCode>() {
            *value = Value::from(offset.apply_to_time_code(tc));
        }
    } else if value.is::<Vec<TimeCode>>() {
        if let Some(mut arr) = value.downcast_clone::<Vec<TimeCode>>() {
            for tc in arr.iter_mut() {
                *tc = offset.apply_to_time_code(*tc);
            }
            *value = Value::new(arr);
        }
    }
}

/// Apply a layer offset to spline knot times and tangent widths/slopes.
fn apply_layer_offset_to_spline(spline: &mut SplineValue, offset: &LayerOffset) {
    if offset.is_identity() {
        return;
    }

    let scale = offset.scale();
    for knot in spline.knots_mut() {
        knot.time = offset.apply(knot.time);

        if let Some(pre) = knot.pre_tangent.as_mut() {
            if let Some(width) = pre.width.as_mut() {
                *width *= scale;
            }
            if scale != 0.0 {
                pre.slope /= scale;
            }
        }

        if let Some(post) = knot.post_tangent.as_mut() {
            if let Some(width) = post.width.as_mut() {
                *width *= scale;
            }
            if scale != 0.0 {
                post.slope /= scale;
            }
        }
    }

    if let Some(loop_params) = spline.loop_params.as_mut() {
        loop_params.proto_start = offset.apply(loop_params.proto_start);
        loop_params.proto_end = offset.apply(loop_params.proto_end);
    }

    spline.knots_mut().sort_by(|a, b| {
        a.time
            .partial_cmp(&b.time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

pub(crate) fn get_authored_spline(
    layer: &std::sync::Arc<usd_sdf::Layer>,
    path: &Path,
    spline_token: &Token,
) -> Option<SplineValue> {
    let value = layer.get_field(path, spline_token)?;
    if value.is_empty() {
        return None;
    }
    value.downcast_clone::<SplineValue>()
}

fn fill_resolve_info_site(
    info: &mut super::resolve_info::ResolveInfo,
    node: Option<&usd_pcp::NodeRef>,
    layer: &Arc<usd_sdf::Layer>,
    spec_path: &Path,
    layer_to_stage_offset: LayerOffset,
) {
    if let Some(node) = node {
        info.set_node(node.clone());
        if let Some(layer_stack) = node.layer_stack() {
            info.set_layer_stack(layer_stack);
        }
    }
    info.set_layer(usd_sdf::LayerHandle::from_layer(layer));
    info.set_prim_path(spec_path.get_prim_path());
    info.set_layer_to_stage_offset(layer_to_stage_offset);
}

fn resolve_info_set_source(
    info: &mut super::resolve_info::ResolveInfo,
    source: super::resolve_info::ResolveInfoSource,
) -> bool {
    use super::resolve_info::ResolveInfoSource;

    if info.source() == ResolveInfoSource::None {
        info.set_source(source);
        return true;
    }
    if info.source() == ResolveInfoSource::Default
        && matches!(
            source,
            ResolveInfoSource::TimeSamples
                | ResolveInfoSource::ValueClips
                | ResolveInfoSource::Spline
        )
    {
        info.set_default_can_compose_over_weaker_time_varying_sources(true);
    }
    false
}

fn default_value_can_compose(
    value_type: std::any::TypeId,
) -> bool {
    value_type_can_compose_over(value_type)
}

pub(crate) fn resolve_time_sample_value_from_layer(
    layer: &Arc<usd_sdf::Layer>,
    path: &Path,
    time: f64,
    is_pre_time: bool,
    use_linear: bool,
) -> Option<Value> {
    let query_non_block =
        |layer: &Arc<usd_sdf::Layer>, path: &Path, t: f64| -> Option<Value> {
            let val = layer.query_time_sample(path, t)?;
            if val.is::<usd_sdf::ValueBlock>() || val.is_empty() {
                None
            } else {
                Some(val)
            }
        };

    // Use O(log n) BTreeMap::range() via get_bracketing_time_samples_for_path
    // instead of enumerating all time samples + sort + binary search — O(n log n).
    let (lower_t, upper_t) = match layer.get_bracketing_time_samples_for_path(path, time) {
        Some(bracket) => bracket,
        None => {
            // No time samples in BTreeMap — try direct query (e.g. USDC lazy samples)
            if let Some(val) = layer.query_time_sample(path, time) {
                if val.is::<usd_sdf::ValueBlock>() || val.is_empty() {
                    return None;
                }
                return Some(val);
            }
            return None;
        }
    };

    // Handle is_pre_time: when exact match, step back one sample
    let (lower_t, upper_t) = if is_pre_time && lower_t == upper_t && (time - lower_t).abs() < 1e-10 {
        // Need the sample just before this one — use get_previous
        if let Some(prev) = layer.get_previous_time_sample_for_path(path, time) {
            (prev, lower_t)
        } else {
            (lower_t, upper_t)
        }
    } else {
        (lower_t, upper_t)
    };

    if (lower_t - upper_t).abs() < 1e-10 {
        return query_non_block(layer, path, lower_t);
    }

    let lo_val = query_non_block(layer, path, lower_t);
    let hi_val = query_non_block(layer, path, upper_t);
    match (lo_val, hi_val) {
        (Some(lo), Some(hi)) => {
            super::interpolators::interpolate_value(path, time, lower_t, upper_t, &lo, &hi, use_linear)
        }
        (Some(lo), None) => Some(lo),
        _ => None,
    }
}

// ============================================================================
// Attribute
// ============================================================================

/// A typed property that holds values over time.
///
/// Attributes are the primary way to store data on prims. They have a value
/// type and can hold either a single default value or time-sampled values.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::{UsdStage, TimeCode};
///
/// let stage = UsdStage::open("scene.usda")?;
/// let prim = stage.get_prim_at_path("/World/Cube")?;
/// let attr = prim.get_attribute("size")?;
///
/// // Get value at default time
/// let size: f64 = attr.get(TimeCode::default())?;
///
/// // Set value
/// attr.set(2.0, TimeCode::default())?;
/// ```
#[derive(Debug, Clone)]
pub struct Attribute {
    /// Base property data.
    inner: Property,
}

impl Attribute {
    /// Creates a new attribute.
    pub(crate) fn new(stage: Weak<Stage>, path: Path) -> Self {
        Self {
            inner: Property::new_with_type(stage, path, super::object::ObjType::Attribute),
        }
    }

    /// Creates an invalid attribute.
    pub fn invalid() -> Self {
        Self {
            inner: Property::invalid(),
        }
    }

    /// Get resolve info for this attribute.
    ///
    /// Matches C++ `UsdAttribute::GetResolveInfo() const`.
    ///
    /// Returns information about how the attribute's value is resolved.
    pub fn get_resolve_info(&self) -> super::resolve_info::ResolveInfo {
        if !self.is_valid() {
            return super::resolve_info::ResolveInfo::default();
        }
        let Some(stage) = self.inner.stage() else {
            return super::resolve_info::ResolveInfo::default();
        };
        let prim_path = self.prim_path();
        let prim_index_opt = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index_arc());
        match prim_index_opt {
            Some(ref idx) => self.get_resolve_info_with_index(idx),
            None => super::resolve_info::ResolveInfo::default(),
        }
    }

    /// Same as `get_resolve_info` but with a pre-fetched PrimIndex.
    /// Avoids redundant stage.get_prim_at_path + prim_index_arc lookups.
    pub fn get_resolve_info_with_index(
        &self,
        prim_index_arc: &std::sync::Arc<usd_pcp::PrimIndex>,
    ) -> super::resolve_info::ResolveInfo {
        use super::resolve_info::{ResolveInfo, ResolveInfoSource};

        if !self.is_valid() {
            return ResolveInfo::default();
        }

        let Some(stage) = self.inner.stage() else {
            return ResolveInfo::default();
        };

        let attr_name = self.name();
        let prim_path = self.prim_path();
        let default_token = &*tokens::DEFAULT;
        let spline_token = &*tokens::SPLINE;
        let mut info = ResolveInfo::new();
        info.set_prim_path(prim_path.clone());

        let prim_index_opt: Option<std::sync::Arc<usd_pcp::PrimIndex>> = Some(prim_index_arc.clone());

        let mut resolver_visited = false;
        if let Some(ref prim_index) = prim_index_opt {
            let prim_may_have_clips = stage
                .clip_cache()
                .map(|cc| !cc.get_clips_for_prim(&prim_path).is_empty())
                .unwrap_or(false);
            // C++: `Usd_Resolver(..., /*skipEmptyNodes=*/!MayHaveOpinionsInClips)` — when clips
            // may apply, iterate empty nodes too so clip opinions are visible.
            let mut resolver =
                super::resolver::Resolver::new(prim_index, !prim_may_have_clips);
            let mut is_new_node = true;
            let mut spec_path: Option<Path> = None;
            let mut processing_animation_block = false;

            while resolver.is_valid() {
                if is_new_node {
                    spec_path = resolver.get_local_path_for_property(&attr_name);
                    if prim_may_have_clips {
                        if let (Some(ref n), Some(ref sp)) =
                            (resolver.get_node(), spec_path.as_ref())
                        {
                            let clips_all = stage
                                .clip_cache()
                                .map(|cc| cc.get_clips_for_prim(&prim_path))
                                .unwrap_or_default();
                            let relevant =
                                super::clip_set::get_clips_that_apply_to_node(
                                    &clips_all,
                                    n,
                                    sp,
                                );
                            if !n.has_specs() && relevant.is_empty() {
                                resolver.next_node();
                                is_new_node = true;
                                continue;
                            }
                        }
                    }
                }
                if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                    resolver_visited = true;
                    let node = resolver.get_node();
                    let layer_to_stage_offset = resolver.get_layer_to_stage_offset();

                    if !processing_animation_block && layer.get_num_time_samples_for_path(sp) > 0 {
                        let did_set_source =
                            resolve_info_set_source(&mut info, ResolveInfoSource::TimeSamples);
                        if did_set_source {
                            fill_resolve_info_site(
                                &mut info,
                                node.as_ref(),
                                &layer,
                                sp,
                                layer_to_stage_offset,
                            );
                            return info;
                        }
                    }
                    else if !processing_animation_block && layer.has_field(sp, &spline_token) {
                        let did_set_source =
                            resolve_info_set_source(&mut info, ResolveInfoSource::Spline);
                        if did_set_source {
                            fill_resolve_info_site(
                                &mut info,
                                node.as_ref(),
                                &layer,
                                sp,
                                layer_to_stage_offset,
                            );
                            if let Some(spline) = get_authored_spline(&layer, sp, spline_token) {
                                info.set_spline(spline);
                            }
                            return info;
                        }
                    }
                    else if let Some(type_id) = layer.get_field_typeid(sp, &default_token) {
                        if type_id == std::any::TypeId::of::<usd_sdf::ValueBlock>() {
                            if info.source() == ResolveInfoSource::None {
                                info.set_value_is_blocked(true);
                            }
                            if self.has_fallback_value() {
                                let _ = resolve_info_set_source(
                                    &mut info,
                                    ResolveInfoSource::Fallback,
                                );
                            }
                            return info;
                        }
                        else if type_id == std::any::TypeId::of::<usd_sdf::AnimationBlock>() {
                            processing_animation_block = true;
                        }
                        else {
                            let did_set_source =
                                resolve_info_set_source(&mut info, ResolveInfoSource::Default);
                            let default_can_compose = default_value_can_compose(type_id);

                            if did_set_source {
                                fill_resolve_info_site(
                                    &mut info,
                                    node.as_ref(),
                                    &layer,
                                    sp,
                                    layer_to_stage_offset,
                                );
                                info.set_default_can_compose(default_can_compose);
                                if default_can_compose {
                                    is_new_node = resolver.next_layer();
                                    continue;
                                }
                                return info;
                            }

                            if !default_can_compose || info.value_source_might_be_time_varying() {
                                return info;
                            }
                        }
                    }

                    // Value clips: C++ `_GetResolvedValueAtTimeWithClipsImpl` + `ProcessClips`
                    // (GetResolveInfo with time `nullptr` uses `_HasTimeSamples(clipSet, specPath)`
                    // without a query time).
                    if !processing_animation_block {
                        if let Some(ref n) = node {
                            if let Some(clip_cache) = stage.clip_cache() {
                                let clips_all = clip_cache.get_clips_for_prim(&prim_path);
                                if !clips_all.is_empty() {
                                    let candidates = super::clip_set::get_clips_that_apply_to_node(
                                        &clips_all,
                                        n,
                                        sp,
                                    );
                                    for cs in candidates {
                                        if !super::clip_set::clip_source_layer_matches_resolver_layer(
                                            cs.as_ref(),
                                            &layer,
                                        ) {
                                            continue;
                                        }
                                        if !super::clip_set::clip_set_has_time_samples(cs.as_ref(), sp)
                                        {
                                            continue;
                                        }
                                        if resolve_info_set_source(
                                            &mut info,
                                            ResolveInfoSource::ValueClips,
                                        ) {
                                            fill_resolve_info_site(
                                                &mut info,
                                                Some(n),
                                                &layer,
                                                sp,
                                                layer_to_stage_offset,
                                            );
                                            info.set_value_clip_set(Some(cs));
                                            return info;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                is_new_node = resolver.next_layer();
            }
        }

        // Fallback: walk session + root directly (in-memory stages without PCP)
        if !resolver_visited {
            let attr_path = self.path();
            let mut layers_to_check: Vec<std::sync::Arc<usd_sdf::Layer>> = Vec::new();
            if let Some(session) = stage.session_layer() {
                layers_to_check.push(std::sync::Arc::clone(session));
            }
            layers_to_check.push(std::sync::Arc::clone(stage.root_layer()));
            let mut processing_animation_block = false;

            for layer in &layers_to_check {
                if !processing_animation_block && layer.get_num_time_samples_for_path(attr_path) > 0 {
                    let did_set_source =
                        resolve_info_set_source(&mut info, ResolveInfoSource::TimeSamples);
                    if did_set_source {
                        fill_resolve_info_site(
                            &mut info,
                            None,
                            layer,
                            attr_path,
                            LayerOffset::identity(),
                        );
                        return info;
                    }
                }
                else if !processing_animation_block && layer.has_field(attr_path, &spline_token) {
                    let did_set_source =
                        resolve_info_set_source(&mut info, ResolveInfoSource::Spline);
                    if did_set_source {
                        fill_resolve_info_site(
                            &mut info,
                            None,
                            layer,
                            attr_path,
                            LayerOffset::identity(),
                        );
                        if let Some(spline) = get_authored_spline(layer, attr_path, spline_token) {
                            info.set_spline(spline);
                        }
                        return info;
                    }
                }
                else if let Some(type_id) = layer.get_field_typeid(attr_path, &default_token) {
                    if type_id == std::any::TypeId::of::<usd_sdf::ValueBlock>() {
                        if info.source() == ResolveInfoSource::None {
                            info.set_value_is_blocked(true);
                        }
                        if self.has_fallback_value() {
                            let _ = resolve_info_set_source(&mut info, ResolveInfoSource::Fallback);
                        }
                        return info;
                    }
                    else if type_id == std::any::TypeId::of::<usd_sdf::AnimationBlock>() {
                        processing_animation_block = true;
                    }
                    else {
                        let did_set_source =
                            resolve_info_set_source(&mut info, ResolveInfoSource::Default);
                        let default_can_compose = default_value_can_compose(type_id);
                        if did_set_source {
                            fill_resolve_info_site(
                                &mut info,
                                None,
                                layer,
                                attr_path,
                                LayerOffset::identity(),
                            );
                            info.set_default_can_compose(default_can_compose);
                            if default_can_compose {
                                continue;
                            }
                            return info;
                        }

                        if !default_can_compose || info.value_source_might_be_time_varying() {
                            return info;
                        }
                    }
                }
            }
        }

        // Check fallback value
        if self.has_fallback_value() {
            info.set_source(ResolveInfoSource::Fallback);
        }

        info
    }

    /// Get resolve info for this attribute with a resolve target.
    ///
    /// Matches C++ `UsdAttribute::GetResolveInfo(const UsdResolveTarget &resolveTarget) const`.
    pub fn get_resolve_info_with_target(
        &self,
        resolve_target: &super::resolve_target::ResolveTarget,
    ) -> super::resolve_info::ResolveInfo {
        use super::resolve_info::{ResolveInfo, ResolveInfoSource};

        if !self.is_valid() {
            return ResolveInfo::default();
        }

        let Some(stage) = self.inner.stage() else {
            return ResolveInfo::default();
        };

        let attr_name = self.name();
        let prim_path = self.prim_path();
        let default_token = &*tokens::DEFAULT;
        let spline_token = &*tokens::SPLINE;
        let mut info = ResolveInfo::new();

        let prim_may_have_clips = stage
            .clip_cache()
            .map(|cc| !cc.get_clips_for_prim(&prim_path).is_empty())
            .unwrap_or(false);

        // Walk layers constrained by the resolve target.
        let mut resolver = super::resolver::Resolver::new_with_resolve_target(
            resolve_target,
            !prim_may_have_clips,
        );
        let mut is_new_node = true;
        let mut spec_path: Option<Path> = None;
        let mut processing_animation_block = false;

        while resolver.is_valid() {
            if is_new_node {
                spec_path = resolver.get_local_path_for_property(&attr_name);
                if prim_may_have_clips {
                    if let (Some(ref n), Some(ref sp)) = (resolver.get_node(), spec_path.as_ref()) {
                        let clips_all = stage
                            .clip_cache()
                            .map(|cc| cc.get_clips_for_prim(&prim_path))
                            .unwrap_or_default();
                        let relevant = super::clip_set::get_clips_that_apply_to_node(
                            &clips_all,
                            n,
                            sp,
                        );
                        if !n.has_specs() && relevant.is_empty() {
                            resolver.next_node();
                            is_new_node = true;
                            continue;
                        }
                    }
                }
            }
            if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                let node = resolver.get_node();
                let layer_to_stage_offset = resolver.get_layer_to_stage_offset();
                if !processing_animation_block && layer.get_num_time_samples_for_path(sp) > 0 {
                    let did_set_source =
                        resolve_info_set_source(&mut info, ResolveInfoSource::TimeSamples);
                    if did_set_source {
                        fill_resolve_info_site(
                            &mut info,
                            node.as_ref(),
                            &layer,
                            sp,
                            layer_to_stage_offset,
                        );
                        return info;
                    }
                }
                else if !processing_animation_block && layer.has_field(sp, &spline_token) {
                    let did_set_source =
                        resolve_info_set_source(&mut info, ResolveInfoSource::Spline);
                    if did_set_source {
                        fill_resolve_info_site(
                            &mut info,
                            node.as_ref(),
                            &layer,
                            sp,
                            layer_to_stage_offset,
                        );
                        if let Some(spline) = get_authored_spline(&layer, sp, spline_token) {
                            info.set_spline(spline);
                        }
                        return info;
                    }
                }
                else if let Some(type_id) = layer.get_field_typeid(sp, &default_token) {
                    if type_id == std::any::TypeId::of::<ValueBlock>() {
                        if info.source() == ResolveInfoSource::None {
                            info.set_value_is_blocked(true);
                        }
                        if self.has_fallback_value() {
                            let _ = resolve_info_set_source(&mut info, ResolveInfoSource::Fallback);
                        }
                        return info;
                    }
                    if type_id == std::any::TypeId::of::<AnimationBlock>() {
                        processing_animation_block = true;
                        is_new_node = resolver.next_layer();
                        continue;
                    }
                    let did_set_source =
                        resolve_info_set_source(&mut info, ResolveInfoSource::Default);
                    let default_can_compose = default_value_can_compose(type_id);
                    if did_set_source {
                        fill_resolve_info_site(
                            &mut info,
                            node.as_ref(),
                            &layer,
                            sp,
                            layer_to_stage_offset,
                        );
                        info.set_default_can_compose(default_can_compose);
                        if default_can_compose {
                            is_new_node = resolver.next_layer();
                            continue;
                        }
                        return info;
                    }

                    if !default_can_compose || info.value_source_might_be_time_varying() {
                        return info;
                    }
                }

                if !processing_animation_block {
                    if let Some(ref n) = node {
                        if let Some(clip_cache) = stage.clip_cache() {
                            let clips_all = clip_cache.get_clips_for_prim(&prim_path);
                            if !clips_all.is_empty() {
                                let candidates = super::clip_set::get_clips_that_apply_to_node(
                                    &clips_all,
                                    n,
                                    sp,
                                );
                                for cs in candidates {
                                    if !super::clip_set::clip_source_layer_matches_resolver_layer(
                                        cs.as_ref(),
                                        &layer,
                                    ) {
                                        continue;
                                    }
                                    if !super::clip_set::clip_set_has_time_samples(cs.as_ref(), sp)
                                    {
                                        continue;
                                    }
                                    if resolve_info_set_source(
                                        &mut info,
                                        ResolveInfoSource::ValueClips,
                                    ) {
                                        fill_resolve_info_site(
                                            &mut info,
                                            Some(n),
                                            &layer,
                                            sp,
                                            layer_to_stage_offset,
                                        );
                                        info.set_value_clip_set(Some(cs));
                                        return info;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            is_new_node = resolver.next_layer();
        }

        // Check fallback
        if self.has_fallback_value() {
            info.set_source(ResolveInfoSource::Fallback);
        } else {
            info.set_source(ResolveInfoSource::None);
        }
        info.set_prim_path(self.prim_path().clone());
        info
    }

    /// Get resolve info for this attribute at a specific `UsdTimeCode`.
    pub fn get_resolve_info_at_time(
        &self,
        time: &super::time_code::TimeCode,
    ) -> super::resolve_info::ResolveInfo {
        if !time.is_default() {
            return self.get_resolve_info();
        }

        use super::resolve_info::{ResolveInfo, ResolveInfoSource};
        let Some(stage) = self.inner.stage() else {
            return ResolveInfo::default();
        };

        let attr_name = self.name();
        let default_token = &*tokens::DEFAULT;
        let mut info = ResolveInfo::new();
        info.set_prim_path(self.prim_path().clone());
        let mut current_info = &mut info;

        let prim_path = self.prim_path();
        let prim_index_opt = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index_arc());

        let mut resolver_visited = false;
        if let Some(ref prim_index) = prim_index_opt {
            let mut resolver = super::resolver::Resolver::new(prim_index, true);
            let mut is_new_node = true;
            let mut spec_path: Option<Path> = None;

            while resolver.is_valid() {
                if is_new_node {
                    spec_path = resolver.get_local_path_for_property(&attr_name);
                }
                if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                    resolver_visited = true;
                    let node = resolver.get_node();
                    let layer_to_stage_offset = resolver.get_layer_to_stage_offset();
                    if let Some(type_id) = layer.get_field_typeid(sp, &default_token) {
                        if type_id == std::any::TypeId::of::<ValueBlock>() {
                            current_info.set_value_is_blocked(true);
                            if self.has_fallback_value() {
                                current_info.set_source(ResolveInfoSource::Fallback);
                            }
                            return info;
                        }
                        if type_id == std::any::TypeId::of::<AnimationBlock>() {
                            is_new_node = resolver.next_layer();
                            continue;
                        }
                        current_info.set_source(ResolveInfoSource::Default);
                        fill_resolve_info_site(
                            current_info,
                            node.as_ref(),
                            &layer,
                            sp,
                            layer_to_stage_offset,
                        );
                        let default_can_compose =
                            default_value_can_compose(type_id);
                        current_info.set_default_can_compose(default_can_compose);
                        if default_can_compose {
                            current_info = current_info.add_next_weaker_info();
                            is_new_node = resolver.next_layer();
                            continue;
                        }
                        return info;
                    }
                }
                is_new_node = resolver.next_layer();
            }
        }

        if !resolver_visited {
            let attr_path = self.path();
            let mut layers_to_check: Vec<std::sync::Arc<usd_sdf::Layer>> = Vec::new();
            if let Some(session) = stage.session_layer() {
                layers_to_check.push(std::sync::Arc::clone(session));
            }
            layers_to_check.push(std::sync::Arc::clone(stage.root_layer()));

            for layer in &layers_to_check {
                if let Some(type_id) = layer.get_field_typeid(attr_path, &default_token) {
                    if type_id == std::any::TypeId::of::<ValueBlock>() {
                        current_info.set_value_is_blocked(true);
                        if self.has_fallback_value() {
                            current_info.set_source(ResolveInfoSource::Fallback);
                        }
                        return info;
                    }
                    if type_id == std::any::TypeId::of::<AnimationBlock>() {
                        continue;
                    }
                    current_info.set_source(ResolveInfoSource::Default);
                    fill_resolve_info_site(
                        current_info,
                        None,
                        layer,
                        attr_path,
                        LayerOffset::identity(),
                    );
                    let default_can_compose =
                        default_value_can_compose(type_id);
                    current_info.set_default_can_compose(default_can_compose);
                    if default_can_compose {
                        current_info = current_info.add_next_weaker_info();
                        continue;
                    }
                    return info;
                }
            }
        }

        if self.has_fallback_value() {
            current_info.set_source(ResolveInfoSource::Fallback);
        }
        info
    }

    /// Get resolve info for this attribute at a specific `UsdTimeCode`,
    /// constrained by a resolve target.
    pub fn get_resolve_info_at_time_with_target(
        &self,
        time: &super::time_code::TimeCode,
        resolve_target: &super::resolve_target::ResolveTarget,
    ) -> super::resolve_info::ResolveInfo {
        if !time.is_default() {
            return self.get_resolve_info_with_target(resolve_target);
        }

        use super::resolve_info::{ResolveInfo, ResolveInfoSource};
        let attr_name = self.name();
        let default_token = &*tokens::DEFAULT;
        let mut info = ResolveInfo::new();
        info.set_prim_path(self.prim_path().clone());
        let mut current_info = &mut info;

        let mut resolver = super::resolver::Resolver::new_with_resolve_target(resolve_target, true);
        let mut is_new_node = true;
        let mut spec_path: Option<Path> = None;

        while resolver.is_valid() {
            if is_new_node {
                spec_path = resolver.get_local_path_for_property(&attr_name);
            }
            if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                let node = resolver.get_node();
                let layer_to_stage_offset = resolver.get_layer_to_stage_offset();
                if let Some(type_id) = layer.get_field_typeid(sp, &default_token) {
                    if type_id == std::any::TypeId::of::<ValueBlock>() {
                        current_info.set_value_is_blocked(true);
                        if self.has_fallback_value() {
                            current_info.set_source(ResolveInfoSource::Fallback);
                        }
                        return info;
                    }
                    if type_id == std::any::TypeId::of::<AnimationBlock>() {
                        is_new_node = resolver.next_layer();
                        continue;
                    }
                    current_info.set_source(ResolveInfoSource::Default);
                    fill_resolve_info_site(
                        current_info,
                        node.as_ref(),
                        &layer,
                        sp,
                        layer_to_stage_offset,
                    );
                    let default_can_compose =
                        default_value_can_compose(type_id);
                    current_info.set_default_can_compose(default_can_compose);
                    if default_can_compose {
                        current_info = current_info.add_next_weaker_info();
                        is_new_node = resolver.next_layer();
                        continue;
                    }
                    return info;
                }
            }
            is_new_node = resolver.next_layer();
        }

        if self.has_fallback_value() {
            current_info.set_source(ResolveInfoSource::Fallback);
        }
        info.set_prim_path(self.prim_path().clone());
        info
    }

    /// Returns true if this attribute is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// Returns the path to this attribute.
    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    /// Returns the name of this attribute.
    pub fn name(&self) -> Token {
        self.inner.name()
    }

    /// Returns the prim path that owns this attribute.
    pub fn prim_path(&self) -> Path {
        self.inner.prim_path()
    }

    /// Returns the stage that owns this attribute.
    ///
    /// Matches C++ `UsdAttribute::GetStage()`.
    pub fn stage(&self) -> Option<std::sync::Arc<super::stage::Stage>> {
        self.inner.stage()
    }

    /// Returns the prim that owns this attribute.
    ///
    /// Matches C++ `UsdAttribute::GetPrim()`.
    pub fn get_prim(&self) -> super::prim::Prim {
        if let Some(stage) = self.inner.stage() {
            let prim_path = self.inner.prim_path();
            if let Some(prim) = stage.get_prim_at_path(&prim_path) {
                return prim;
            }
        }
        super::prim::Prim::invalid()
    }

    /// Converts this attribute into a property.
    pub fn into_property(self) -> super::property::Property {
        self.inner
    }

    /// Returns a reference to the inner property.
    pub fn as_property(&self) -> &super::property::Property {
        &self.inner
    }

    /// Gets the attribute's value at the specified time.
    ///
    /// Accepts both `SdfTimeCode` and `UsdTimeCode`. When a `UsdTimeCode` with
    /// `is_pre_time()` is passed, returns the value immediately before the
    /// given time (matching C++ PreTime semantics).
    ///
    /// If the resulting value contains an `SdfAssetPath` or `Vec<SdfAssetPath>`,
    /// it is automatically resolved using the stage's resolver context.
    pub fn get(&self, time: impl Into<super::time_code::TimeCode>) -> Option<Value> {
        let usd_time: super::time_code::TimeCode = time.into();
        let sdf_time = if usd_time.is_default() {
            TimeCode::default_time()
        } else {
            TimeCode::new(usd_time.value())
        };
        let stage = self.inner.stage()?;
        let result = self.get_raw(&stage, sdf_time, usd_time.is_pre_time());

        // Apply layer offset to timecode values + resolve asset paths.
        // Filter block sentinels: C++ returns empty VtValue (→ None) for blocked attrs.
        result.and_then(|(mut val, offset)| {
            if val.is_empty() || val.is::<ValueBlock>() || val.is::<AnimationBlock>() {
                return None;
            }
            apply_layer_offset_to_value(&mut val, &offset);
            stage.make_resolved_asset_paths_value(&mut val);
            Some(val)
        })
    }

    /// Gets the raw (unresolved) attribute value at the specified time.
    ///
    /// Returns the value together with the `LayerOffset` from the providing
    /// composition node, so the caller can apply time-code transformations.
    ///
    /// Resolution order matches C++ UsdStage::_GetValueImpl:
    ///   1. Walk full PCP node graph (layer stack) in strong-to-weak order via Usd_Resolver
    ///      - For each layer: check time samples, then spline, then default value (single-pass per layer)
    ///   2. Value clips (after all authored opinions)
    ///   3. Schema fallback value
    ///
    /// IMPORTANT: Per C++ `_GetResolvedValueAtTimeImpl`, for timed queries each layer is
    /// checked in a SINGLE pass: time samples first, then spline, then default value.
    /// This preserves composition strength.
    fn get_raw(
        &self,
        stage: &super::stage::Stage,
        time: TimeCode,
        is_pre_time: bool,
    ) -> Option<(Value, LayerOffset)> {
        let attr_name = self.name();
        let prim_path = self.prim_path();
        let use_linear =
            stage.interpolation_type() == super::interpolation::InterpolationType::Linear;
        let default_token = &*tokens::DEFAULT;
        let spline_token = &*tokens::SPLINE;

        // Get PrimIndex for this prim to drive the resolver walk.
        // Falls back to root-layer-only if no PCP index available.
        let prim_index_opt = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index_arc());

        enum SplineQueryResult {
            Absent,
            Value(Value),
            NoValue,
        }

        // Helper: resolve time sample from a layer at path sp — returns value if found.
        let resolve_time_sample =
            |layer: &std::sync::Arc<usd_sdf::Layer>, sp: &Path, t: f64| -> Option<Value> {
                // Filter ValueBlock from query results
                let query_non_block =
                    |layer: &std::sync::Arc<usd_sdf::Layer>, sp: &Path, t: f64| -> Option<Value> {
                        let val = layer.query_time_sample(sp, t)?;
                        if val.is::<usd_sdf::ValueBlock>() || val.is_empty() {
                            None
                        } else {
                            Some(val)
                        }
                    };

                let mut layer_times = layer.list_time_samples_for_path(sp);
                if layer_times.is_empty() {
                    // No authored time samples — check exact match only
                    if let Some(val) = layer.query_time_sample(sp, t) {
                        if val.is::<usd_sdf::ValueBlock>() || val.is_empty() {
                            return None;
                        }
                        return Some(val);
                    }
                    return None;
                }
                layer_times
                    .sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                // C++ GetBracketingTimeSamples: find lower and upper bracketing t
                let (lower_t, upper_t) = if t <= layer_times[0] {
                    (layer_times[0], layer_times[0])
                } else if t >= *layer_times.last().unwrap() {
                    let last = *layer_times.last().unwrap();
                    (last, last)
                } else {
                    let idx = layer_times.partition_point(|&s| s < t);
                    if idx > 0 && layer_times[idx] == t {
                        (t, t)
                    } else {
                        (layer_times[idx - 1], layer_times[idx])
                    }
                };

                // C++ PreTime: if exact sample hit, shift to previous segment
                // Only shift when t actually lands on the sample, not when clamped to boundary
                let (lower_t, upper_t) = if is_pre_time && lower_t == upper_t && t == lower_t {
                    let exact_idx = layer_times.partition_point(|&s| s < lower_t);
                    if exact_idx > 0 {
                        (layer_times[exact_idx - 1], lower_t)
                    } else {
                        // Before first sample — clamp to first
                        (lower_t, upper_t)
                    }
                } else {
                    (lower_t, upper_t)
                };

                if lower_t == upper_t {
                    // Exact sample (or clamped)
                    return query_non_block(layer, sp, lower_t);
                }
                // Interpolate between lower and upper
                let lo_val = query_non_block(layer, sp, lower_t);
                let hi_val = query_non_block(layer, sp, upper_t);
                match (lo_val, hi_val) {
                    (Some(lo), Some(hi)) => super::interpolators::interpolate_value(
                        sp, t, lower_t, upper_t, &lo, &hi, use_linear,
                    ),
                    // Upper blocked: held fallback (C++ returns lower value)
                    (Some(lo), None) => Some(lo),
                    // Lower blocked or both missing
                    _ => None,
                }
            };

        let resolve_spline_value =
            |layer: &std::sync::Arc<usd_sdf::Layer>, sp: &Path, t: f64| -> SplineQueryResult {
                let Some(spline) = get_authored_spline(layer, sp, &spline_token) else {
                    return SplineQueryResult::Absent;
                };

                let value = if is_pre_time {
                    spline.evaluate_pre(t)
                } else {
                    spline.evaluate(t)
                };

                match value {
                    Some(v) => SplineQueryResult::Value(Value::from(v)),
                    None => SplineQueryResult::NoValue,
                }
            };

        // Try full PCP resolver walk if available.
        let mut resolver_visited = false;
        if let Some(ref prim_index) = prim_index_opt {
            let mut resolver = super::resolver::Resolver::new(prim_index, true);
            let mut is_new_node = true;
            let mut spec_path: Option<Path> = None;

            if time.is_default() {
                // C++ _GetResolvedValueAtDefaultImpl: walk layers with composition.
                // Composable values (ArrayEdit, Dict) accumulate in partial.
                let mut partial: Option<Value> = None;
                let mut found_offset = LayerOffset::identity();
                while resolver.is_valid() {
                    if is_new_node {
                        spec_path = resolver.get_local_path_for_property(&attr_name);
                    }
                    if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref())
                    {
                        resolver_visited = true;
                        if let Some(val) = layer.get_field(sp, &default_token) {
                            if !val.is_empty() {
                                if val.is::<AnimationBlock>() {
                                    is_new_node = resolver.next_layer();
                                    continue;
                                }
                                // Capture offset from the strongest opinion
                                if partial.is_none() {
                                    found_offset = resolver.get_layer_to_stage_offset();
                                }
                                match consume_authored(&mut partial, val) {
                                    ConsumeResult::Done(v) => return Some((v, found_offset)),
                                    ConsumeResult::Continue => {}
                                }
                            }
                        }
                    }
                    is_new_node = resolver.next_layer();
                }
                // Finalize: if partial remains, compose over background
                if let Some(p) = partial {
                    return Some((finalize_partial(p), found_offset));
                }
            } else {
                // C++ _GetResolvedValueAtTimeImpl / ProcessLayerAtTime:
                // Single pass per layer — check time samples, then spline, then default value.
                // Composable values accumulate in partial.
                let t = time.value();
                let mut partial: Option<Value> = None;
                let mut found_offset = LayerOffset::identity();
                let mut processing_animation_block = false;

                while resolver.is_valid() {
                    if is_new_node {
                        spec_path = resolver.get_local_path_for_property(&attr_name);
                    }
                    if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref())
                    {
                        resolver_visited = true;
                        // Convert stage time → layer-local time (C++ ProcessLayerAtTime)
                        let offset = resolver.get_layer_to_stage_offset();
                        let local_t = if offset.is_identity() {
                            t
                        } else {
                            offset.inverse().apply(t)
                        };
                        // 1. Time samples (strongest opinion in this layer)
                        if !processing_animation_block {
                            if let Some(val) = resolve_time_sample(&layer, sp, local_t) {
                                if partial.is_none() {
                                    found_offset = offset;
                                }
                                match consume_authored(&mut partial, val) {
                                    ConsumeResult::Done(v) => return Some((v, found_offset)),
                                    ConsumeResult::Continue => {}
                                }
                                is_new_node = resolver.next_layer();
                                continue;
                            }
                        }
                        // 2. Spline value.
                        if !processing_animation_block {
                            match resolve_spline_value(&layer, sp, local_t) {
                                SplineQueryResult::Value(val) => {
                                    return Some((val, offset));
                                }
                                SplineQueryResult::NoValue => {
                                    return Some((Value::empty(), offset));
                                }
                                SplineQueryResult::Absent => {}
                            }
                        }
                        // 3. Default value (C++ ProcessLayerAtTime falls through to Usd_HasDefault)
                        if let Some(val) = layer.get_field(sp, &default_token) {
                            if !val.is_empty() {
                                if val.is::<AnimationBlock>() {
                                    processing_animation_block = true;
                                    is_new_node = resolver.next_layer();
                                    continue;
                                }
                                if partial.is_none() {
                                    found_offset = offset;
                                }
                                match consume_authored(&mut partial, val) {
                                    ConsumeResult::Done(v) => return Some((v, found_offset)),
                                    ConsumeResult::Continue => {}
                                }
                            }
                        }
                    }
                    is_new_node = resolver.next_layer();
                }

                // After all authored opinions, check value clips.
                if resolver_visited && !processing_animation_block {
                    if let Some(val) = self.get_value_from_clips(stage, time) {
                        match consume_authored(&mut partial, val) {
                            ConsumeResult::Done(v) => return Some((v, found_offset)),
                            ConsumeResult::Continue => {}
                        }
                    }
                }
                // Finalize: if partial remains, compose over background
                if let Some(p) = partial {
                    return Some((finalize_partial(p), found_offset));
                }
            }
        }

        // Fallback to root layer when resolver had no valid nodes (in-memory stages).
        // Walks session layer then root layer with composition support.
        // No composition arcs here, so offset is always identity.
        let id = LayerOffset::identity();
        if !resolver_visited {
            let attr_path = self.path();
            let mut partial: Option<Value> = None;

            // Walk session layer first (stronger), then root layer
            let mut layers_to_check: Vec<std::sync::Arc<usd_sdf::Layer>> = Vec::new();
            if let Some(session) = stage.session_layer() {
                layers_to_check.push(std::sync::Arc::clone(session));
            }
            layers_to_check.push(std::sync::Arc::clone(stage.root_layer()));

            if time.is_default() {
                for layer in &layers_to_check {
                    if let Some(val) = layer.get_field(attr_path, &default_token) {
                        if !val.is_empty() {
                            if val.is::<AnimationBlock>() {
                                continue;
                            }
                            match consume_authored(&mut partial, val) {
                                ConsumeResult::Done(v) => return Some((v, id)),
                                ConsumeResult::Continue => {}
                            }
                        }
                    }
                }
            } else {
                let t = time.value();
                let mut processing_animation_block = false;
                for layer in &layers_to_check {
                    if !processing_animation_block {
                        if let Some(val) = resolve_time_sample(layer, attr_path, t) {
                            match consume_authored(&mut partial, val) {
                                ConsumeResult::Done(v) => return Some((v, id)),
                                ConsumeResult::Continue => {}
                            }
                            continue;
                        }
                    }
                    if !processing_animation_block {
                        match resolve_spline_value(layer, attr_path, t) {
                            SplineQueryResult::Value(val) => return Some((val, id)),
                            SplineQueryResult::NoValue => return Some((Value::empty(), id)),
                            SplineQueryResult::Absent => {}
                        }
                    }
                    if let Some(val) = layer.get_field(attr_path, &default_token) {
                        if !val.is_empty() {
                            if val.is::<AnimationBlock>() {
                                processing_animation_block = true;
                                continue;
                            }
                            match consume_authored(&mut partial, val) {
                                ConsumeResult::Done(v) => return Some((v, id)),
                                ConsumeResult::Continue => {}
                            }
                        }
                    }
                }
                if !processing_animation_block {
                    if let Some(val) = self.get_value_from_clips(stage, time) {
                        match consume_authored(&mut partial, val) {
                            ConsumeResult::Done(v) => return Some((v, id)),
                            ConsumeResult::Continue => {}
                        }
                    }
                }
            }
            // Finalize partial
            if let Some(p) = partial {
                return Some((finalize_partial(p), id));
            }
        }

        // If no authored value found, try schema fallback value.
        self.get_fallback_value().map(|v| (v, id))
    }

    /// Queries value clips for this attribute's prim at the given time.
    ///
    /// Iterates all clip sets affecting this prim (from ClipCache), finds the
    /// first clip set that has authored data for this attribute spec path, and
    /// returns the value. Matches C++ `_GetValueFromResolveInfoImpl` for the
    /// `UsdResolveInfoSourceValueClips` case.
    fn get_value_from_clips(&self, stage: &super::stage::Stage, time: TimeCode) -> Option<Value> {
        let clip_cache = stage.clip_cache()?;
        let prim_path = self.prim_path();
        let clip_sets = clip_cache.get_clips_for_prim(&prim_path);

        if clip_sets.is_empty() {
            return None;
        }

        let attr_path = self.path();

        for clip_set in &clip_sets {
            // Skip clip sets without value clips
            if clip_set.value_clips.is_empty() {
                continue;
            }

            // If a manifest is present, it controls which attributes can have
            // clip values. Only attributes declared as SdfVariabilityVarying in
            // the manifest contribute values from clips.
            // Matches C++ _ClipsContainValueForAttribute.
            if let Some(ref manifest) = clip_set.manifest_clip {
                let variability_token = usd_tf::Token::new("variability");
                if manifest.has_field(attr_path, &variability_token) {
                    // Read variability as a spec FIELD (not a time sample).
                    // SdfVariabilityVarying == 0, SdfVariabilityUniform == 1.
                    // C++ ref: stage.cpp:7229 _ClipsContainValueForAttribute uses HasField + Get.
                    let variability_val =
                        manifest.get_field_typed::<i32>(attr_path, &variability_token);
                    // Varying (0) means clips contribute; Uniform (1) means they don't.
                    let is_varying = variability_val.map(|v| v == 0).unwrap_or(false);
                    if !is_varying {
                        continue; // Uniform — clips don't contribute this attribute
                    }
                } else {
                    // Manifest exists but doesn't declare this attribute — skip.
                    // C++: if manifest doesn't have the variability field, attribute
                    // is not considered Varying, so clips don't contribute.
                    continue;
                }
            }

            // Query the value from this clip set at the requested time
            if let Some(val) = clip_set.get_value(attr_path, time) {
                return Some(val);
            }
        }

        None
    }

    /// Gets the attribute's value at the specified time, with type checking.
    pub fn get_typed<T: Clone + 'static>(&self, time: TimeCode) -> Option<T> {
        self.get(time).and_then(|v| v.downcast_clone::<T>())
    }

    /// Gets the attribute's value as `Vec<T>`, accepting both `Vec<T>` and `Array<T>` storage.
    ///
    /// USDC stores geometry arrays as `Array<T>` (VtArray parity), but USD-A stores them as
    /// `Vec<T>`. This method transparently handles both, so callers don't need to know which
    /// concrete type the layer backend used.
    pub fn get_typed_vec<T: Clone + Send + Sync + 'static>(
        &self,
        time: TimeCode,
    ) -> Option<Vec<T>> {
        self.get(time).and_then(|v| v.as_vec_clone::<T>())
    }

    /// Sets the attribute's value at the specified time.
    ///
    /// Matches C++ `UsdAttribute::Set(const VtValue&, UsdTimeCode)`.
    /// SdfAnimationBlock may only be authored at the default time.
    pub fn set(&self, value: impl Into<Value>, time: TimeCode) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        let attr_path = self.path();
        let spec_attr_path = edit_target.map_to_spec_path(attr_path);
        let val: Value = value.into();

        if val.is::<AnimationBlock>() && !time.is_default() {
            return false;
        }

        // Ensure attribute spec exists in the layer before writing
        if layer.get_prim_at_path(&spec_attr_path.get_prim_path()).is_none() {
            let _ = layer.create_prim_spec(
                &spec_attr_path.get_prim_path(),
                usd_sdf::Specifier::Over,
                "",
            );
        }
        if layer.get_spec_type(&spec_attr_path) == usd_sdf::SpecType::Unknown {
            layer.create_spec(&spec_attr_path, usd_sdf::SpecType::Attribute);
        }

        if time.is_default() {
            layer.set_field(&spec_attr_path, &Token::new("default"), val)
        } else {
            layer.set_time_sample(&spec_attr_path, edit_target.map_time_to_spec_time(time.value()), val)
        }
    }

    /// Clears the attribute's value at the specified time.
    ///
    /// If time is default, clears the default value. Otherwise clears the time sample.
    pub fn clear(&self, time: TimeCode) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return true; // No layer = nothing to clear
        };
        let spec_attr_path = edit_target.map_to_spec_path(self.path());

        let Some(mut attr_spec) = layer.get_attribute_at_path(&spec_attr_path) else {
            return true; // No spec = nothing to clear
        };

        if time.is_default() {
            attr_spec.clear_default_value();
        } else {
            attr_spec.clear_time_sample(edit_target.map_time_to_spec_time(time.value()));
        }
        true
    }

    /// Clears all authored values (default and all time samples).
    pub fn clear_all(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return true; // No layer = nothing to clear
        };
        let spec_attr_path = edit_target.map_to_spec_path(self.path());

        let Some(mut attr_spec) = layer.get_attribute_at_path(&spec_attr_path) else {
            return true; // No spec = nothing to clear
        };

        attr_spec.clear_default_value();
        attr_spec.clear_time_samples();
        true
    }

    /// Blocks the attribute's value.
    ///
    /// Matches C++ `UsdAttribute::Block()`: clears all authored values then
    /// authors an SdfValueBlock at Default time, preventing weaker-layer
    /// values from being seen.
    pub fn block(&self) -> bool {
        // C++: Clear() = ClearDefault() + ClearMetadata(TimeSamples) + ClearMetadata(Spline)
        // then Set(VtValue(SdfValueBlock()), UsdTimeCode::Default())
        self.clear_authored_value();
        self.set(Value::new(ValueBlock), TimeCode::default_time())
    }

    /// Clears all authored values matching C++ `UsdAttribute::Clear()`:
    /// ClearDefault() + ClearMetadata(timeSamples) + ClearMetadata(spline).
    pub fn clear_authored_value(&self) -> bool {
        // C++: ClearDefault() && ClearMetadata(SdfFieldKeys->TimeSamples) && ClearMetadata(SdfFieldKeys->Spline)
        let r1 = self.clear(TimeCode::default_time());
        let r2 = self.clear_metadata(&Token::new("timeSamples"));
        let r3 = self.clear_metadata(&Token::new("spline"));
        r1 && r2 && r3
    }

    /// Returns true if the attribute has a non-blocked value.
    ///
    /// Matches C++ `UsdAttribute::HasValue()` which checks resolve info source.
    /// A blocked attribute (ValueBlock authored) returns false.
    pub fn has_value(&self) -> bool {
        use super::resolve_info::ResolveInfoSource;
        // C++: return resolveInfo._source != UsdResolveInfoSourceNone;
        self.get_resolve_info().source() != ResolveInfoSource::None
    }

    /// Returns true if this attribute has a fallback value from a registered schema.
    ///
    /// Fallback values are defaults defined in USD schema definitions (e.g. UsdGeomMesh).
    /// This checks the schema registry for fallback values defined for this attribute.
    pub fn has_fallback_value(&self) -> bool {
        use super::schema_registry::SchemaRegistry;

        let stage = match self.inner.stage() {
            Some(s) => s,
            None => return false,
        };
        let prim_path = self.prim_path();
        let prim = match stage.get_prim_at_path(&prim_path) {
            Some(p) => p,
            None => return false,
        };
        let type_name = prim.type_name();
        if type_name.is_empty() {
            return false;
        }

        // Try full PrimDefinition first
        let schema_registry = SchemaRegistry::get_instance();
        if let Some(prim_def) = schema_registry.find_concrete_prim_definition(&type_name) {
            let attr_def = prim_def.get_attribute_definition(&self.name());
            if attr_def.is_valid() && attr_def.get_fallback_value_as_value().is_some() {
                return true;
            }
        }

        // Fall back to lightweight schema fallback registry
        super::schema_registry::schema_get_fallback(&type_name, self.name().as_str()).is_some()
    }

    /// Gets the fallback value for this attribute from a registered schema.
    pub fn get_fallback_value(&self) -> Option<Value> {
        use super::schema_registry::SchemaRegistry;

        let stage = self.inner.stage()?;
        let prim_path = self.prim_path();
        let prim = stage.get_prim_at_path(&prim_path)?;
        let type_name = prim.type_name();
        if type_name.is_empty() {
            return None;
        }

        // Try full PrimDefinition first
        let schema_registry = SchemaRegistry::get_instance();
        if let Some(prim_def) = schema_registry.find_concrete_prim_definition(&type_name) {
            let attr_def = prim_def.get_attribute_definition(&self.name());
            if attr_def.is_valid() {
                if let Some(val) = attr_def.get_fallback_value_as_value() {
                    return Some(val);
                }
            }
        }

        // Fall back to lightweight schema fallback registry
        super::schema_registry::schema_get_fallback(&type_name, self.name().as_str())
    }

    /// Returns true if the attribute has an authored value.
    ///
    /// Checks full composed layer stack for default value or time samples.
    pub fn has_authored_value(&self) -> bool {
        // C++: return resolveInfo.HasAuthoredValue();
        // = source is Default/TimeSamples/ValueClips/Spline (excludes blocked & fallback)
        self.get_resolve_info().has_authored_value()
    }

    /// Returns true if the attribute has an authored value at the specified time.
    pub fn has_authored_value_opinion(&self) -> bool {
        self.has_authored_value()
    }

    /// Returns the type name of this attribute.
    ///
    /// Walks the full composed PCP layer stack (sublayers, references, payloads)
    /// to find the strongest opinion, matching C++ `UsdAttribute::GetTypeName()`.
    /// Falls back to root-layer-only when no PCP index is available.
    pub fn type_name(&self) -> Token {
        let Some(stage) = self.inner.stage() else {
            return Token::new("");
        };
        let attr_name = self.name();
        let prim_path = self.prim_path();
        let type_name_token = usd_tf::Token::new("typeName");

        // Walk full PCP layer stack: strongest opinion wins.
        let prim_index_opt = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index_arc());

        if let Some(ref prim_index) = prim_index_opt {
            let mut resolver = super::resolver::Resolver::new(prim_index, true);
            let mut is_new_node = true;
            let mut spec_path: Option<Path> = None;

            while resolver.is_valid() {
                if is_new_node {
                    spec_path = resolver.get_local_path_for_property(&attr_name);
                }
                if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                    if let Some(val) = layer.get_field(sp, &type_name_token) {
                        if let Some(tn) = val.get::<String>() {
                            if !tn.is_empty() {
                                return Token::new(tn);
                            }
                        }
                        // Also try Token type (some layers store typeName as Token)
                        if let Some(tn) = val.get::<usd_tf::Token>() {
                            if !tn.is_empty() {
                                return tn.clone();
                            }
                        }
                    }
                }
                is_new_node = resolver.next_layer();
            }
        }

        // Fallback: root layer only (in-memory stages without PCP)
        let layer = stage.root_layer();
        if let Some(attr_spec) = layer.get_attribute_at_path(self.path()) {
            let tn = attr_spec.type_name();
            if !tn.is_empty() {
                return Token::new(&tn);
            }
        }

        // Infer type from registered schema property types, then fallback value (Vt type names).
        if let Some(prim) = stage.get_prim_at_path(&prim_path) {
            let prim_type = prim.type_name();
            if !prim_type.is_empty() {
                if let Some(sdf_type) = super::schema_registry::schema_get_property_type(
                    &prim_type,
                    attr_name.as_str(),
                ) {
                    return Token::new(&sdf_type);
                }
                if let Some(fallback) =
                    super::schema_registry::schema_get_fallback(&prim_type, attr_name.as_str())
                {
                    if let Some(tn) = fallback.type_name() {
                        return Token::new(tn);
                    }
                }
            }
        }

        Token::new("")
    }

    /// Returns the variability of this attribute (varying or uniform).
    ///
    /// C++ attribute.cpp: variability is a non-time-varying field, resolved
    /// by walking the full composed layer stack (not just root layer).
    /// Variability authored in sublayers/references must be visible.
    pub fn variability(&self) -> Variability {
        let Some(stage) = self.inner.stage() else {
            return Variability::Varying;
        };
        // Walk full layer stack, first opinion wins (strongest to weakest)
        for layer in stage.layer_stack() {
            if let Some(attr_spec) = layer.get_attribute_at_path(self.path()) {
                return match attr_spec.variability() {
                    usd_sdf::Variability::Uniform => Variability::Uniform,
                    usd_sdf::Variability::Varying => Variability::Varying,
                };
            }
        }
        Variability::Varying
    }

    /// Returns all authored time samples for this attribute across the full layer stack.
    ///
    /// Delegates to [`Stage::get_time_samples_in_interval_with_resolve_info`](crate::Stage) with
    /// [`get_resolve_info`](Attribute::get_resolve_info) — same path as [`crate::AttributeQuery`] time APIs.
    pub fn get_time_samples(&self) -> Vec<f64> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };
        let info = self.get_resolve_info();
        stage.get_time_samples_in_interval_with_resolve_info(
            self,
            &info,
            &usd_gf::Interval::get_full_interval(),
            None,
        )
    }

    /// Returns the time samples within the given interval [start, end] (closed).
    pub fn get_time_samples_in_interval(&self, start: f64, end: f64) -> Vec<f64> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };
        let info = self.get_resolve_info();
        let interval = usd_gf::Interval::new(start, end, true, true);
        stage.get_time_samples_in_interval_with_resolve_info(self, &info, &interval, None)
    }

    /// Returns the number of time samples.
    pub fn get_num_time_samples(&self) -> usize {
        self.get_time_samples().len()
    }

    /// Determines whether the attribute's value might vary over time.
    ///
    /// Matches C++ `ValueMightBeTimeVarying()`.
    ///
    /// This checks if the attribute has more than 1 time sample or is spline-valued.
    /// This is more efficient than actually counting time samples.
    pub fn might_be_time_varying(&self) -> bool {
        // Check if we have more than 1 time sample
        let num_samples = self.get_num_time_samples();
        if num_samples > 1 {
            return true;
        }

        // Splines are time-varying by definition.
        if self.has_spline() {
            return true;
        }

        false
    }

    // ========================================================================
    // Spline support
    // ========================================================================

    /// Returns true if this attribute holds a spline value.
    ///
    /// Matches C++ `UsdAttribute::HasSpline()`.
    pub fn has_spline(&self) -> bool {
        self.get_resolve_info().source() == super::resolve_info::ResolveInfoSource::Spline
    }

    /// Returns the spline value if this attribute holds one.
    ///
    /// Matches C++ `UsdAttribute::GetSpline()`.
    pub fn get_spline(&self) -> Option<SplineValue> {
        if !self.has_spline() {
            return None;
        }

        let stage = self.inner.stage()?;
        let attr_name = self.name();
        let prim_path = self.prim_path();
        let default_token = &*tokens::DEFAULT;
        let spline_token = &*tokens::SPLINE;

        let prim_index_opt = stage
            .get_prim_at_path(&prim_path)
            .and_then(|p| p.prim_index_arc());

        if let Some(ref prim_index) = prim_index_opt {
            let mut resolver = super::resolver::Resolver::new(prim_index, true);
            let mut is_new_node = true;
            let mut spec_path: Option<Path> = None;
            let mut processing_animation_block = false;

            while resolver.is_valid() {
                if is_new_node {
                    spec_path = resolver.get_local_path_for_property(&attr_name);
                }
                if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                    if !processing_animation_block && layer.get_num_time_samples_for_path(sp) > 0 {
                        return None;
                    }
                    if !processing_animation_block {
                        if let Some(mut spline) = get_authored_spline(&layer, sp, &spline_token) {
                            apply_layer_offset_to_spline(
                                &mut spline,
                                &resolver.get_layer_to_stage_offset(),
                            );
                            return Some(spline);
                        }
                    }
                    if let Some(val) = layer.get_field(sp, &default_token) {
                        if !val.is_empty() && val.is::<AnimationBlock>() {
                            processing_animation_block = true;
                        }
                    }
                }
                is_new_node = resolver.next_layer();
            }
        }

        let attr_path = self.path();
        let mut layers_to_check: Vec<std::sync::Arc<usd_sdf::Layer>> = Vec::new();
        if let Some(session) = stage.session_layer() {
            layers_to_check.push(std::sync::Arc::clone(session));
        }
        layers_to_check.push(std::sync::Arc::clone(stage.root_layer()));

        let mut processing_animation_block = false;
        for layer in &layers_to_check {
            if !processing_animation_block && layer.get_num_time_samples_for_path(attr_path) > 0 {
                return None;
            }
            if !processing_animation_block {
                if let Some(spline) = get_authored_spline(layer, attr_path, &spline_token) {
                    return Some(spline);
                }
            }
            if let Some(val) = layer.get_field(attr_path, &default_token) {
                if !val.is_empty() && val.is::<AnimationBlock>() {
                    processing_animation_block = true;
                }
            }
        }

        None
    }

    /// Sets a spline value on this attribute.
    ///
    /// Matches C++ `UsdAttribute::SetSpline()`.
    pub fn set_spline(&self, spline: &SplineValue) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        let attr_path = self.path();
        if layer.get_spec_type(attr_path) == usd_sdf::SpecType::Unknown {
            layer.create_spec(attr_path, usd_sdf::SpecType::Attribute);
        }

        layer.set_field(
            attr_path,
            &*tokens::SPLINE,
            Value::from(spline.clone()),
        )
    }

    /// Returns the bracketing time samples around the given time.
    ///
    /// Returns (lower, upper) where lower <= time <= upper.
    pub fn get_bracketing_time_samples(&self, time: f64) -> Option<(f64, f64)> {
        let Some(stage) = self.inner.stage() else {
            return None;
        };
        let info = self.get_resolve_info();
        stage.get_bracketing_time_samples_with_resolve_info(self, &info, time, None)
    }

    /// Returns true if this attribute's value might vary over time.
    ///
    /// If this returns false, the value is definitely constant.
    ///
    /// Matches C++ `UsdAttribute::ValueMightBeTimeVarying()` (delegates to
    /// `UsdStage::_ValueMightBeTimeVaryingFromResolveInfo`).
    pub fn value_might_be_time_varying(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let info = self.get_resolve_info();
        stage.value_might_be_time_varying_from_resolve_info(&info, self)
    }

    /// Returns a description of this attribute.
    pub fn description(&self) -> String {
        if self.is_valid() {
            format!(
                "Attribute '{}' at {}",
                self.name().get_text(),
                self.path().get_string()
            )
        } else {
            "Invalid attribute".to_string()
        }
    }

    /// Returns connections to this attribute.
    ///
    /// These are paths to other attributes that this attribute gets its value from,
    /// commonly used in shader networks.
    pub fn get_connections(&self) -> Vec<Path> {
        self.inner
            .get_composed_targets(usd_pcp::TargetSpecType::Attribute)
    }

    /// Gets connections to this attribute, writing them to the provided vector.
    ///
    /// Matches C++ `GetConnections(SdfPathVector*)`.
    pub fn get_connections_to(&self, paths: &mut Vec<Path>) -> bool {
        *paths = self.get_connections();
        true
    }

    /// Returns true if this attribute has authored connections.
    /// Matches C++ `HasAuthoredMetadata(SdfFieldKeys->ConnectionPaths)` — fast
    /// metadata existence check without full PCP composition.
    pub fn has_authored_connections(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let conn_tok = usd_tf::Token::new("connectionPaths");
        for layer in stage.layer_stack() {
            if layer.has_field(self.path(), &conn_tok) {
                return true;
            }
        }
        false
    }

    /// Adds a connection to this attribute.
    ///
    /// Adds the source path to the connection list in the current edit target.
    pub fn add_connection(&self, source: &Path) -> bool {
        self.add_connection_with_position(source, super::common::ListPosition::BackOfAppendList)
    }

    /// Adds a connection to this attribute at the specified position.
    ///
    /// Matches C++ `AddConnection(SdfPath, UsdListPosition)`.
    pub fn add_connection_with_position(
        &self,
        source: &Path,
        position: super::common::ListPosition,
    ) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        // Ensure attribute spec exists
        let attr_path = self.path();
        if layer.get_attribute_at_path(attr_path).is_none()
            && !layer.create_spec(attr_path, usd_sdf::SpecType::Attribute)
        {
            return false;
        }

        let Some(mut attr_spec) = layer.get_attribute_at_path(attr_path) else {
            return false;
        };

        let mut list_op = attr_spec.connection_paths_list();

        // C++ SdfListOp semantics: if the list is explicit, Prepend/Append
        // modify the explicit items directly (front/back insertion).
        if list_op.is_explicit() {
            let mut items = list_op.get_explicit_items().to_vec();
            match position {
                super::common::ListPosition::FrontOfPrependList
                | super::common::ListPosition::FrontOfAppendList => {
                    items.insert(0, source.clone());
                }
                super::common::ListPosition::BackOfPrependList
                | super::common::ListPosition::BackOfAppendList => {
                    items.push(source.clone());
                }
            }
            list_op.set_explicit_items(items).ok();
        } else {
            match position {
                super::common::ListPosition::FrontOfPrependList => {
                    let mut prepended = list_op.get_prepended_items().to_vec();
                    prepended.insert(0, source.clone());
                    list_op.set_prepended_items(prepended).ok();
                }
                super::common::ListPosition::BackOfPrependList => {
                    let mut prepended = list_op.get_prepended_items().to_vec();
                    prepended.push(source.clone());
                    list_op.set_prepended_items(prepended).ok();
                }
                super::common::ListPosition::FrontOfAppendList => {
                    let mut appended = list_op.get_appended_items().to_vec();
                    appended.insert(0, source.clone());
                    list_op.set_appended_items(appended).ok();
                }
                super::common::ListPosition::BackOfAppendList => {
                    let mut appended = list_op.get_appended_items().to_vec();
                    appended.push(source.clone());
                    list_op.set_appended_items(appended).ok();
                }
            }
        }

        // Write the updated list_op back to the layer
        attr_spec.set_connection_paths_list(list_op);
        true
    }

    /// Removes a connection from this attribute.
    pub fn remove_connection(&self, source: &Path) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        let Some(mut attr_spec) = layer.get_attribute_at_path(self.path()) else {
            return false;
        };

        let mut list_op = attr_spec.connection_paths_list();
        let mut deleted = list_op.get_deleted_items().to_vec();
        deleted.push(source.clone());
        list_op.set_deleted_items(deleted).ok();

        // Write the updated list_op back to the layer
        attr_spec.set_connection_paths_list(list_op);
        true
    }

    /// Clears all connections from this attribute.
    pub fn clear_connections(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return true; // No layer = nothing to clear
        };

        let Some(mut attr_spec) = layer.get_attribute_at_path(self.path()) else {
            return true; // No spec = nothing to clear
        };

        attr_spec.clear_connection_paths();
        true
    }

    /// Sets the connections for this attribute (replaces all existing connections).
    ///
    /// Matches C++ `SetConnections()`.
    pub fn set_connections(&self, connections: Vec<Path>) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        let attr_path = self.path();

        // Ensure attribute spec exists
        if layer.get_attribute_at_path(attr_path).is_none()
            && !layer.create_spec(attr_path, usd_sdf::SpecType::Attribute)
        {
            return false;
        }

        let Some(mut attr_spec) = layer.get_attribute_at_path(attr_path) else {
            return false;
        };

        // Set connections as explicit list
        use usd_sdf::list_op::PathListOp;
        let mut list_op = PathListOp::new();
        list_op.set_explicit_items(connections).ok();
        attr_spec.set_connection_paths_list(list_op);
        true
    }

    /// Sets metadata value for this attribute.
    ///
    /// Matches C++ `SetMetadata()`.
    pub fn set_metadata(&self, key: &Token, value: impl Into<Value>) -> bool {
        self.inner.set_metadata(key, value.into())
    }

    /// Gets metadata value for this attribute.
    ///
    /// Matches C++ `GetMetadata()`.
    pub fn get_metadata(&self, key: &Token) -> Option<Value> {
        self.inner.get_metadata(key)
    }

    /// Composed metadata dictionary (matches C++ `UsdAttribute::GetAllMetadata`).
    pub fn get_all_metadata(&self) -> super::object::MetadataValueMap {
        self.inner.get_all_metadata()
    }

    /// Returns true if metadata is authored for this attribute.
    ///
    /// Matches C++ `HasAuthoredMetadata()`.
    pub fn has_authored_metadata(&self, key: &Token) -> bool {
        self.inner.has_authored_metadata(key)
    }

    /// Gets metadata by dictionary key (matches C++ GetMetadataByDictKey).
    ///
    /// Gets a value from a dictionary-valued metadata field.
    pub fn get_metadata_by_dict_key(&self, dict_key: &Token, key: &Token) -> Option<Value> {
        self.inner.get_metadata_by_dict_key(dict_key, key)
    }

    /// Sets metadata by dictionary key (matches C++ SetMetadataByDictKey).
    ///
    /// Sets a value in a dictionary-valued metadata field.
    pub fn set_metadata_by_dict_key(
        &self,
        dict_key: &Token,
        key: &Token,
        value: impl Into<Value>,
    ) -> bool {
        self.inner.set_metadata_by_dict_key(dict_key, key, value.into())
    }

    /// Returns true if metadata dict key exists.
    pub fn has_metadata_dict_key(&self, dict_key: &Token, key: &Token) -> bool {
        self.inner.has_metadata_dict_key(dict_key, key)
    }

    /// Clears metadata by dictionary key (matches C++ ClearMetadataByDictKey).
    ///
    /// Removes a key from a dictionary-valued metadata field.
    pub fn clear_metadata_by_dict_key(&self, dict_key: &Token, key: &Token) -> bool {
        self.inner.clear_metadata_by_dict_key(dict_key, key)
    }

    /// Clears any metadata value authored on this attribute.
    ///
    /// Matches C++ `ClearMetadata()`.
    pub fn clear_metadata(&self, key: &Token) -> bool {
        self.inner.clear_metadata(key)
    }

    // ========================================================================
    // Additional C++ API methods
    // ========================================================================

    /// Clears the attribute value at the specified time.
    ///
    /// Matches C++ `ClearAtTime(UsdTimeCode time)`.
    pub fn clear_at_time(&self, time: TimeCode) -> bool {
        self.clear(time)
    }

    /// Clears the default value for this attribute.
    ///
    /// Matches C++ `ClearDefault()`.
    pub fn clear_default(&self) -> bool {
        self.clear(TimeCode::default_time())
    }

    /// Blocks animation (time samples and spline) for this attribute.
    ///
    /// Matches C++ `UsdAttribute::BlockAnimation()`: clears all authored values
    /// then authors an `SdfAnimationBlock` at Default time, which preserves
    /// default values from weaker layers while blocking animation.
    pub fn block_animation(&self) -> bool {
        // C++: Clear() = ClearDefault() + ClearMetadata(TimeSamples) + ClearMetadata(Spline)
        // then Set(VtValue(SdfAnimationBlock()), UsdTimeCode::Default())
        self.clear_authored_value();
        self.set(Value::new(AnimationBlock), TimeCode::default_time())
    }

    /// Gets the color space for this attribute.
    ///
    /// Matches C++ `GetColorSpace()`.
    pub fn get_color_space(&self) -> Token {
        self.get_metadata(&Token::new("colorSpace"))
            .and_then(|v| v.downcast_clone::<String>())
            .map(|s| Token::new(&s))
            .unwrap_or_else(|| Token::new(""))
    }

    /// Sets the color space for this attribute.
    ///
    /// Matches C++ `SetColorSpace(const TfToken &colorSpace)`.
    pub fn set_color_space(&self, color_space: &Token) {
        let _ = self.set_metadata(
            &Token::new("colorSpace"),
            Value::from(color_space.get_text().to_string()),
        );
    }

    /// Returns true if this attribute has an authored color space.
    ///
    /// Matches C++ `HasColorSpace()`.
    pub fn has_color_space(&self) -> bool {
        self.has_authored_metadata(&Token::new("colorSpace"))
    }

    /// Clears the authored color space for this attribute.
    ///
    /// Matches C++ `ClearColorSpace()`.
    pub fn clear_color_space(&self) -> bool {
        self.clear_metadata(&Token::new("colorSpace"))
    }

    /// Gets limits dictionary for this attribute.
    ///
    /// Matches C++ `GetLimits()`. Uses the "limits" SDF field.
    pub fn get_limits_dict(&self) -> Option<usd_vt::Dictionary> {
        self.get_metadata(&Token::new("limits"))
            .and_then(|v| v.get::<usd_vt::Dictionary>().cloned())
    }

    /// Sets limits dictionary for this attribute.
    ///
    /// Matches C++ `SetLimits(const VtDictionary &limits)`. Uses the "limits" SDF field.
    pub fn set_limits_dict(&self, limits: usd_vt::Dictionary) -> bool {
        self.set_metadata(&Token::new("limits"), Value::new(limits))
    }

    /// Returns true if this attribute has authored limits.
    ///
    /// Matches C++ `HasAuthoredLimits()`. Uses the "limits" SDF field.
    pub fn has_authored_limits(&self) -> bool {
        self.has_authored_metadata(&Token::new("limits"))
    }

    /// Clears the authored limits for this attribute.
    ///
    /// Matches C++ `ClearLimits()`. Uses the "limits" SDF field.
    pub fn clear_limits(&self) -> bool {
        self.clear_metadata(&Token::new("limits"))
    }

    /// Sets the variability of this attribute.
    ///
    /// Matches C++ `SetVariability(SdfVariability variability)`.
    pub fn set_variability(&self, variability: Variability) -> bool {
        let var_str = match variability {
            Variability::Varying => "varying",
            Variability::Uniform => "uniform",
        };
        self.set_metadata(&Token::new("variability"), Value::from(var_str))
    }

    /// Gets the type name as a SdfValueTypeName.
    ///
    /// Matches C++ `GetTypeName()`.
    pub fn get_type_name(&self) -> usd_sdf::ValueTypeName {
        let type_token = self.type_name();
        usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type(type_token.as_str())
    }

    /// Sets the type name for this attribute.
    ///
    /// Matches C++ `SetTypeName(const SdfValueTypeName &typeName)`.
    pub fn set_type_name(&self, type_name: &usd_sdf::ValueTypeName) -> bool {
        if !type_name.is_valid() {
            return false;
        }
        let type_token = type_name.as_token();
        self.set_metadata(
            &Token::new("typeName"),
            Value::from(type_token.get_text().to_string()),
        )
    }

    /// Gets the role name for this attribute (e.g., "Color", "Point", "Normal").
    ///
    /// Matches C++ `GetRoleName()`.
    pub fn get_role_name(&self) -> Token {
        // Role name is embedded in the type name (e.g., "color3f" has role "Color")
        let type_name = self.get_type_name();
        if type_name.is_valid() {
            type_name.get_role()
        } else {
            Token::new("")
        }
    }

    // =========================================================================
    // ArraySizeConstraint API
    // =========================================================================

    /// Returns the array size constraint for this attribute, or 0 if none is set.
    ///
    /// Matches C++ `UsdAttribute::GetArraySizeConstraint()`.
    pub fn get_array_size_constraint(&self) -> i64 {
        self.get_metadata(&Token::new("arraySizeConstraint"))
            .and_then(|v| v.downcast_clone::<i64>())
            .unwrap_or(0)
    }

    /// Sets the array size constraint for this attribute.
    ///
    /// Matches C++ `UsdAttribute::SetArraySizeConstraint(int64_t size)`.
    pub fn set_array_size_constraint(&self, size: i64) -> bool {
        self.set_metadata(&Token::new("arraySizeConstraint"), Value::from(size))
    }

    /// Returns true if an array size constraint has been authored on this attribute.
    ///
    /// Matches C++ `UsdAttribute::HasAuthoredArraySizeConstraint()`.
    pub fn has_authored_array_size_constraint(&self) -> bool {
        self.has_authored_metadata(&Token::new("arraySizeConstraint"))
    }

    /// Clears the authored array size constraint for this attribute.
    ///
    /// Matches C++ `UsdAttribute::ClearArraySizeConstraint()`.
    pub fn clear_array_size_constraint(&self) -> bool {
        self.clear_metadata(&Token::new("arraySizeConstraint"))
    }

    // =========================================================================
    // Limits API (AttributeLimits)
    // =========================================================================

    /// Returns an AttributeLimits object for the given sub-dictionary key.
    ///
    /// This is the new USD 25.02+ API for limits metadata.
    ///
    /// Matches C++ `GetLimits(const TfToken &subDictKey)`.
    pub fn get_limits_for_subdict(
        &self,
        sub_dict_key: &Token,
    ) -> super::attribute_limits::AttributeLimits {
        super::attribute_limits::AttributeLimits::new(self, sub_dict_key)
    }

    /// Returns the soft limits for this attribute.
    ///
    /// Soft limits indicate the suggested value range.
    ///
    /// Matches C++ `GetSoftLimits()`.
    pub fn get_soft_limits(&self) -> super::attribute_limits::AttributeLimits {
        self.get_limits_for_subdict(&super::attribute_limits::limits_keys::soft())
    }

    /// Returns the hard limits for this attribute.
    ///
    /// Hard limits indicate the enforced value range.
    ///
    /// Matches C++ `GetHardLimits()`.
    pub fn get_hard_limits(&self) -> super::attribute_limits::AttributeLimits {
        self.get_limits_for_subdict(&super::attribute_limits::limits_keys::hard())
    }

    // =========================================================================
    // Static union helpers
    // =========================================================================

    /// Returns the union of all authored time samples across the given attributes.
    ///
    /// Matches C++ `UsdAttribute::GetUnionedTimeSamples(attrs, times)`.
    pub fn get_unioned_time_samples(attrs: &[Attribute]) -> Vec<f64> {
        use super::attribute_query::AttributeQuery;
        let queries: Vec<AttributeQuery> = attrs
            .iter()
            .map(|a| AttributeQuery::new(a.clone()))
            .collect();
        AttributeQuery::get_unioned_time_samples(&queries)
    }

    /// Returns the union of all authored time samples in the given interval
    /// across the given attributes.
    ///
    /// Matches C++ `UsdAttribute::GetUnionedTimeSamplesInInterval(attrs, interval, times)`.
    pub fn get_unioned_time_samples_in_interval(
        attrs: &[Attribute],
        start: f64,
        end: f64,
    ) -> Vec<f64> {
        use super::attribute_query::AttributeQuery;
        use usd_gf::interval::Interval;
        let queries: Vec<AttributeQuery> = attrs
            .iter()
            .map(|a| AttributeQuery::new(a.clone()))
            .collect();
        let interval = Interval::new(start, end, true, true);
        AttributeQuery::get_unioned_time_samples_in_interval(&queries, &interval)
    }
}

/// Variability of an attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Variability {
    /// Value can change over time.
    #[default]
    Varying,
    /// Value is constant.
    Uniform,
}

impl std::fmt::Display for Variability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variability::Varying => write!(f, "Varying"),
            Variability::Uniform => write!(f, "Uniform"),
        }
    }
}

impl PartialEq for Attribute {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Attribute {}

impl std::hash::Hash for Attribute {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl Default for Attribute {
    fn default() -> Self {
        Self::invalid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_attribute() {
        let attr = Attribute::invalid();
        assert!(!attr.is_valid());
    }

    #[test]
    fn test_attribute_path() {
        let path = Path::from_string("/World.size").unwrap();
        let attr = Attribute::new(Weak::new(), path.clone());
        assert_eq!(attr.path(), &path);
    }

    #[test]
    fn test_attribute_name() {
        let path = Path::from_string("/World.size").unwrap();
        let attr = Attribute::new(Weak::new(), path);
        assert_eq!(attr.name().get_text(), "size");
    }

    #[test]
    fn test_variability_default() {
        assert_eq!(Variability::default(), Variability::Varying);
    }

    /// Helper: create a stage with a prim + double attribute with time samples.
    fn make_stage_with_samples(
        samples: &[(f64, f64)],
    ) -> (std::sync::Arc<crate::stage::Stage>, Attribute) {
        use crate::common::InitialLoadSet;
        use crate::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();

        let type_name =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("double");
        let attr = prim
            .create_attribute("val", &type_name, false, None)
            .expect("create attr");

        for &(t, v) in samples {
            let ok = attr.set(Value::from(v), TimeCode::new(t));
            assert!(ok, "Failed to set time sample at t={t}");
        }
        (stage, attr)
    }

    #[test]
    fn test_get_exact_time_sample() {
        let (_stage, attr) = make_stage_with_samples(&[(1.0, 10.0), (3.0, 30.0)]);

        // Verify time samples were authored
        let samples = attr.get_time_samples();
        assert!(!samples.is_empty(), "Expected time samples, got none");
        assert_eq!(
            samples.len(),
            2,
            "Expected 2 time samples, got {}",
            samples.len()
        );

        // Exact sample lookup
        let val = attr.get(TimeCode::new(1.0));
        assert!(val.is_some(), "Expected value at t=1.0");
        let val = val.unwrap();
        assert!(
            val.get::<f64>().is_some(),
            "Expected f64 value, got type: {:?}",
            val
        );
        assert_eq!(*val.get::<f64>().unwrap(), 10.0);

        let val = attr.get(TimeCode::new(3.0)).unwrap();
        assert_eq!(*val.get::<f64>().unwrap(), 30.0);
    }

    #[test]
    fn test_held_interpolation() {
        use crate::interpolation::InterpolationType;

        let (stage, attr) = make_stage_with_samples(&[(1.0, 10.0), (3.0, 30.0)]);
        stage.set_interpolation_type(InterpolationType::Held);

        // Between samples: held returns the earlier sample value
        let val = attr.get(TimeCode::new(2.0)).unwrap();
        assert_eq!(*val.get::<f64>().unwrap(), 10.0);
    }

    #[test]
    fn test_linear_interpolation() {
        use crate::interpolation::InterpolationType;

        let (stage, attr) = make_stage_with_samples(&[(1.0, 10.0), (3.0, 30.0)]);
        stage.set_interpolation_type(InterpolationType::Linear);

        // Midpoint: linear interpolation -> 20.0
        let val = attr.get(TimeCode::new(2.0)).unwrap();
        let v = *val.get::<f64>().unwrap();
        assert!((v - 20.0).abs() < 1e-6, "expected 20.0, got {v}");

        // Quarter: 1.0 + 0.5*(3.0-1.0) = 2.0 => fraction=0.25 => 10 + 0.25*20 = 15
        let val = attr.get(TimeCode::new(1.5)).unwrap();
        let v = *val.get::<f64>().unwrap();
        assert!((v - 15.0).abs() < 1e-6, "expected 15.0, got {v}");
    }

    #[test]
    fn test_interpolation_before_first_sample() {
        let (_stage, attr) = make_stage_with_samples(&[(2.0, 20.0), (4.0, 40.0)]);
        // Before first sample: clamp to first sample
        let val = attr.get(TimeCode::new(0.0)).unwrap();
        assert_eq!(*val.get::<f64>().unwrap(), 20.0);
    }

    #[test]
    fn test_interpolation_after_last_sample() {
        let (_stage, attr) = make_stage_with_samples(&[(2.0, 20.0), (4.0, 40.0)]);
        // After last sample: clamp to last sample
        let val = attr.get(TimeCode::new(10.0)).unwrap();
        assert_eq!(*val.get::<f64>().unwrap(), 40.0);
    }

    #[test]
    fn test_default_value_fallback_no_samples() {
        use crate::common::InitialLoadSet;
        use crate::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();
        let type_name =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("double");
        let attr = prim
            .create_attribute("val", &type_name, false, None)
            .unwrap();

        // Set only default value, no time samples
        attr.set(Value::from(42.0_f64), TimeCode::default_time());

        // Query at a specific time should fall back to default
        let val = attr.get(TimeCode::new(5.0)).unwrap();
        assert_eq!(*val.get::<f64>().unwrap(), 42.0);
    }

    // M10: Spline support tests
    #[test]
    fn test_spline_has_none_by_default() {
        use crate::common::InitialLoadSet;
        use crate::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();
        let type_name =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("double");
        let attr = prim
            .create_attribute("val", &type_name, false, None)
            .unwrap();

        assert!(!attr.has_spline());
        assert!(attr.get_spline().is_none());
    }

    #[test]
    fn test_spline_set_and_get() {
        use crate::common::InitialLoadSet;
        use crate::stage::Stage;
        use usd_vt::spline::{SplineCurveType, SplineKnot, SplineValue};

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/Test", "Xform").unwrap();
        let type_name =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("double");
        let attr = prim
            .create_attribute("anim", &type_name, false, None)
            .unwrap();

        let mut spline = SplineValue::new(SplineCurveType::Bezier);
        spline.add_knot(SplineKnot::new(0.0, 0.0));
        spline.add_knot(SplineKnot::new(1.0, 1.0));

        let ok = attr.set_spline(&spline);
        assert!(ok, "set_spline should succeed");

        assert!(attr.has_spline(), "has_spline should be true after set");
        let got = attr.get_spline().expect("get_spline should return Some");
        assert_eq!(got.knots.len(), 2);
        assert_eq!(got.curve_type, SplineCurveType::Bezier);
    }

    #[test]
    fn test_get_unioned_time_samples() {
        // Two attributes with different sample times -> union of all times
        let (stage, attr1) = make_stage_with_samples(&[(1.0, 10.0), (3.0, 30.0)]);
        let prim = stage
            .get_prim_at_path(&Path::from_string("/Test").unwrap())
            .unwrap();
        let type_name =
            usd_sdf::value_type_registry::ValueTypeRegistry::instance().find_type("double");
        let attr2 = prim
            .create_attribute("val2", &type_name, false, None)
            .unwrap();
        attr2.set(Value::from(20.0), TimeCode::new(2.0));
        attr2.set(Value::from(40.0), TimeCode::new(4.0));

        let times = Attribute::get_unioned_time_samples(&[attr1, attr2]);
        // Should contain 1.0, 2.0, 3.0, 4.0 (sorted, deduplicated)
        assert!(times.contains(&1.0));
        assert!(times.contains(&2.0));
        assert!(times.contains(&3.0));
        assert!(times.contains(&4.0));
    }

    #[test]
    fn test_role_name_and_variability() {
        let (_stage, attr) = make_stage_with_samples(&[]);
        // Default variability is Varying
        assert_eq!(attr.variability(), Variability::Varying);
        // set_variability should succeed on a valid attribute
        assert!(attr.set_variability(Variability::Uniform));
    }

    #[test]
    fn test_block_and_block_animation() {
        let (_stage, attr) = make_stage_with_samples(&[(1.0, 10.0)]);
        // block_animation should succeed on a valid attribute
        assert!(attr.block_animation());
        // block() should succeed - sets default value to SdfValueBlock
        assert!(attr.block());
    }
}
