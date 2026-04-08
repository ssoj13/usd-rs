//! Reference: `UsdStage::_SamplesInIntervalResolver`, `_BracketingSamplesResolver`,
//! `_GetResolvedValueAtTimeWithClipsImpl`, `_GetResolvedValueAtTimeNoClipsImpl`,
//! `_GetBracketingTimeSamples` in `pxr/usd/usd/stage.cpp`.

use std::sync::Arc;

use usd_gf::Interval;
use usd_sdf::{Layer, Path};
use usd_tf::Token;
use usd_vt::value_type_can_compose_over;

use crate::attribute::Attribute;
use crate::clip_cache::ClipCache;
use crate::clip_set::{
    ClipSetRefPtr, clip_source_layer_matches_resolver_layer, clips_contain_value_for_attribute,
    get_clips_that_apply_to_node,
};
use crate::compose_time_sample_series::sdf_compose_time_sample_series_can_compose;
use crate::resolve_info::ResolveInfo;
use crate::resolve_target::ResolveTarget;
use crate::resolver::Resolver;
use crate::stage::Stage;
use crate::value_utils::{
    DefaultValueResult, usd_copy_time_samples_in_interval, value_contains_animation_block,
    value_contains_block,
};

fn spline_token() -> Token {
    Token::new("spline")
}

fn default_token() -> Token {
    Token::new("default")
}

fn stage_interval_to_layer_interval(
    interval: &Interval,
    layer_to_stage: &usd_sdf::LayerOffset,
) -> Interval {
    let inv = layer_to_stage.inverse();
    let a = inv.apply(interval.get_min());
    let b = inv.apply(interval.get_max());
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    Interval::new(lo, hi, interval.is_min_closed(), interval.is_max_closed())
}

/// Matches `layer->QueryTimeSampleTypeid` + `VtValueTypeCanComposeOver` (`stage.cpp`).
fn layer_sample_type_can_compose(layer: &Arc<Layer>, spec_path: &Path, layer_time: f64) -> bool {
    layer
        .query_time_sample(spec_path, layer_time)
        .and_then(|v| v.held_type_id())
        .map(|id| value_type_can_compose_over(id))
        .unwrap_or(false)
}

/// Matches `clipSet->QueryTimeSampleTypeid` + `VtValueTypeCanComposeOver`.
fn clip_sample_type_can_compose(
    clip_set: &crate::clip_set::ClipSet,
    spec_path: &Path,
    stage_time: f64,
) -> bool {
    clip_set.query_time_sample_typeid_can_compose(spec_path, stage_time)
}

/// Matches `UsdStage::_BracketingSamplesResolver` (`stage.cpp`).
pub(crate) struct BracketingSamplesResolver {
    stage_time: f64,
    lower: Option<f64>,
    upper: Option<f64>,
    has_any_value: bool,
}

impl BracketingSamplesResolver {
    pub fn new(stage_time: f64) -> Self {
        Self {
            stage_time,
            lower: None,
            upper: None,
            has_any_value: false,
        }
    }

    /// Matches `_BracketingSamplesResolver::_UpdateBounds`.
    fn update_bounds(&mut self, lower: f64, upper: f64, stage_time: f64) -> bool {
        let mut updated_lower = false;
        let should_update_lower = match self.lower {
            None => true,
            Some(l0) => (l0 > stage_time && lower < l0) || (lower <= stage_time && lower > l0),
        };
        if should_update_lower {
            self.lower = Some(lower);
            updated_lower = true;
        }

        let should_update_upper = match self.upper {
            None => true,
            Some(u0) => (u0 < stage_time && upper > u0) || (upper >= stage_time && upper < u0),
        };
        if should_update_upper {
            self.upper = Some(upper);
        }
        updated_lower
    }

    /// Matches `_BracketingSamplesResolver::ProcessFallback`.
    pub fn process_fallback(&mut self) -> bool {
        self.has_any_value = true;
        true
    }

    /// Matches `_BracketingSamplesResolver::ProcessLayerAtTime`.
    pub fn process_layer_at_time(
        &mut self,
        layer: &Arc<Layer>,
        spec_path: &Path,
        layer_to_stage: &usd_sdf::LayerOffset,
        found_opinion: &mut bool,
    ) -> bool {
        let stage_time = self.stage_time;
        let layer_time = layer_to_stage.inverse().apply(stage_time);

        if let Some((layer_lower, layer_upper)) =
            layer.get_bracketing_time_samples_for_path(spec_path, layer_time)
        {
            *found_opinion = true;
            self.has_any_value = true;
            let lower_st = layer_to_stage.apply(layer_lower);
            let upper_st = layer_to_stage.apply(layer_upper);
            let lower_updated = self.update_bounds(lower_st, upper_st, stage_time);
            return lower_updated && !layer_sample_type_can_compose(layer, spec_path, layer_lower);
        } else if layer.has_field(spec_path, &spline_token()) {
            return true;
        } else {
            match layer_default_value_bracketing(layer, spec_path) {
                DefaultValueResult::Found => {
                    *found_opinion = true;
                    self.has_any_value = true;
                    if !layer_default_type_id_can_compose(layer, spec_path) {
                        return true;
                    }
                }
                DefaultValueResult::Blocked | DefaultValueResult::BlockedAnimation => {
                    return true;
                }
                DefaultValueResult::None => {}
            }
        }
        false
    }

    /// Matches `_BracketingSamplesResolver::ProcessClips`.
    pub fn process_clips(&mut self, clip_set: &crate::clip_set::ClipSet, spec_path: &Path) -> bool {
        let stage_time = self.stage_time;
        if !clips_contain_value_for_attribute(clip_set, spec_path) {
            return false;
        }
        let mut lower = 0.0;
        let mut upper = 0.0;
        if !clip_set
            .get_bracketing_time_samples_for_path(spec_path, stage_time, &mut lower, &mut upper)
        {
            return false;
        }
        self.has_any_value = true;
        let lower_updated = self.update_bounds(lower, upper, stage_time);
        lower_updated && !clip_sample_type_can_compose(clip_set, spec_path, lower)
    }

    /// When both bracket times were found; matches `bsr._lower && bsr._upper` in `_GetBracketingTimeSamples`.
    pub fn bracketing_pair(self) -> Option<(f64, f64)> {
        match (self.lower, self.upper) {
            (Some(l), Some(u)) => Some((l, u)),
            _ => None,
        }
    }
}

fn layer_default_value_bracketing(layer: &Arc<Layer>, spec_path: &Path) -> DefaultValueResult {
    let dt = default_token();
    if !layer.has_field(spec_path, &dt) {
        return DefaultValueResult::None;
    }
    let Some(val) = layer.get_field(spec_path, &dt) else {
        return DefaultValueResult::None;
    };
    if val.is_empty() {
        return DefaultValueResult::None;
    }
    if value_contains_block(&val) {
        return DefaultValueResult::Blocked;
    }
    if value_contains_animation_block(&val) {
        return DefaultValueResult::BlockedAnimation;
    }
    DefaultValueResult::Found
}

fn layer_default_type_id_can_compose(layer: &Arc<Layer>, spec_path: &Path) -> bool {
    let val = match layer.get_field(spec_path, &default_token()) {
        Some(v) => v,
        None => return false,
    };
    val.held_type_id()
        .map(|id| value_type_can_compose_over(id))
        .unwrap_or(false)
}

/// Matches `UsdStage::_SamplesInIntervalResolver`.
pub(crate) struct SamplesInIntervalResolver {
    pub interval: Interval,
    pub partial: Vec<(f64, bool)>,
    overrode_interval: bool,
}

impl SamplesInIntervalResolver {
    pub fn new(interval: Interval) -> Self {
        Self {
            interval,
            partial: Vec::new(),
            overrode_interval: false,
        }
    }

    fn compose_partial_over(&mut self, weaker: Vec<(f64, bool)>) {
        if weaker.is_empty() {
            return;
        }
        if self.partial.is_empty() {
            self.partial = weaker;
        } else {
            self.partial = sdf_compose_time_sample_series_can_compose(&self.partial, &weaker);
        }
    }

    pub fn process_fallback(&mut self) -> bool {
        true
    }

    /// Returns `true` if the outer walk should stop.
    pub fn process_layer_at_time(
        &mut self,
        layer: &Arc<Layer>,
        spec_path: &Path,
        layer_to_stage: &usd_sdf::LayerOffset,
        found_opinion: &mut bool,
    ) -> bool {
        let mut sample_times = layer.list_time_samples_for_path(spec_path);
        sample_times.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if !sample_times.is_empty() {
            *found_opinion = true;
            let layer_interval = stage_interval_to_layer_interval(&self.interval, layer_to_stage);
            let layer_times = usd_copy_time_samples_in_interval(&sample_times, &layer_interval);

            if layer_times.is_empty() {
                let Some((low, _up)) =
                    layer.get_bracketing_time_samples_for_path(spec_path, layer_interval.get_min())
                else {
                    return true;
                };
                let can = layer_sample_type_can_compose(layer, spec_path, low);
                return !can;
            }

            let mut weaker: Vec<(f64, bool)> = Vec::with_capacity(layer_times.len());
            for t in layer_times {
                let stage_t = layer_to_stage.apply(t);
                let can = layer_sample_type_can_compose(layer, spec_path, t);
                weaker.push((stage_t, can));
            }
            self.compose_partial_over(weaker);

            if !self.overrode_interval
                && !self.partial.is_empty()
                && self.partial[0].1
                && self.interval.get_min() != self.partial[0].0
            {
                let layer_time = layer_to_stage.inverse().apply(self.partial[0].0);
                if let Some(prev) = layer.get_previous_time_sample_for_path(spec_path, layer_time) {
                    if !layer_sample_type_can_compose(layer, spec_path, prev) {
                        let new_min = layer_to_stage.apply(prev);
                        self.interval.set_min(new_min);
                        self.overrode_interval = true;
                    }
                }
            }
        } else if layer.has_field(spec_path, &spline_token()) {
            return true;
        } else {
            match layer_default_value_bracketing(layer, spec_path) {
                DefaultValueResult::Found => {
                    *found_opinion = true;
                    if !layer_default_type_id_can_compose(layer, spec_path) {
                        return true;
                    }
                }
                DefaultValueResult::Blocked | DefaultValueResult::BlockedAnimation => {
                    return true;
                }
                DefaultValueResult::None => {}
            }
        }

        for s in &self.partial {
            if s.1 {
                return false;
            }
        }
        !self.partial.is_empty()
    }

    pub fn process_clips(&mut self, clip_set: &crate::clip_set::ClipSet, spec_path: &Path) -> bool {
        if !clips_contain_value_for_attribute(clip_set, spec_path) {
            return false;
        }

        let clip_times = clip_set.get_time_samples_in_interval(
            spec_path,
            self.interval.get_min(),
            self.interval.get_max(),
        );

        if clip_times.is_empty() {
            let mut low = 0.0;
            let mut up = 0.0;
            if !clip_set.get_bracketing_time_samples_for_path(
                spec_path,
                self.interval.get_min(),
                &mut low,
                &mut up,
            ) {
                return true;
            }
            let can = clip_sample_type_can_compose(clip_set, spec_path, low);
            return !can;
        }

        let mut weaker: Vec<(f64, bool)> = Vec::with_capacity(clip_times.len());
        for t in clip_times {
            let can = clip_sample_type_can_compose(clip_set, spec_path, t);
            weaker.push((t, can));
        }
        self.compose_partial_over(weaker);

        if !self.overrode_interval
            && !self.partial.is_empty()
            && self.partial[0].1
            && self.interval.get_min() != self.partial[0].0
        {
            let clip_time = self.partial[0].0;
            let mut prev_clip_time = 0.0;
            if clip_set.get_previous_time_sample_for_path(spec_path, clip_time, &mut prev_clip_time)
                && !clip_sample_type_can_compose(clip_set, spec_path, prev_clip_time)
            {
                self.interval.set_min(prev_clip_time);
                self.overrode_interval = true;
            }
        }

        for s in &self.partial {
            if s.1 {
                return false;
            }
        }
        !self.partial.is_empty()
    }
}

fn walk_no_clips(resolver: &mut Resolver, attr: &Attribute, sir: &mut SamplesInIntervalResolver) {
    let prop_name = attr.name();
    let mut is_new_node = true;
    let mut spec_path: Option<Path> = None;
    while resolver.is_valid() {
        if is_new_node {
            spec_path = resolver.get_local_path_for_property(&prop_name);
        }
        if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
            let offset = resolver.get_layer_to_stage_offset();
            let mut found = false;
            if sir.process_layer_at_time(&layer, sp, &offset, &mut found) {
                return;
            }
        }
        is_new_node = resolver.next_layer();
    }
    sir.process_fallback();
}

fn walk_with_clips(
    resolver: &mut Resolver,
    attr: &Attribute,
    sir: &mut SamplesInIntervalResolver,
    clips_affecting_prim: &[ClipSetRefPtr],
) {
    let prop_name = attr.name();
    let mut is_new_node = true;
    let mut spec_path: Option<Path> = None;
    let mut node_has_specs = false;
    let mut clips: Vec<ClipSetRefPtr> = Vec::new();

    while resolver.is_valid() {
        if is_new_node {
            spec_path = resolver.get_local_path_for_property(&prop_name);
            node_has_specs = resolver.get_node().map(|n| n.has_specs()).unwrap_or(false);
        }

        let mut found_opinion = false;
        if node_has_specs {
            if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                let offset = resolver.get_layer_to_stage_offset();
                if sir.process_layer_at_time(&layer, sp, &offset, &mut found_opinion) {
                    return;
                }
            }
        }

        if is_new_node {
            if let (Some(ref n), Some(ref sp)) = (resolver.get_node(), spec_path.as_ref()) {
                clips = get_clips_that_apply_to_node(clips_affecting_prim, n, sp);
                if !node_has_specs && clips.is_empty() {
                    resolver.next_node();
                    is_new_node = true;
                    continue;
                }
            }
        }

        if !found_opinion {
            if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                for clip_set in &clips {
                    if clip_source_layer_matches_resolver_layer(clip_set.as_ref(), &layer) {
                        if sir.process_clips(clip_set.as_ref(), sp) {
                            return;
                        }
                    }
                }
            }
        }

        is_new_node = resolver.next_layer();
    }
    sir.process_fallback();
}

fn walk_bracketing_no_clips(
    resolver: &mut Resolver,
    attr: &Attribute,
    bsr: &mut BracketingSamplesResolver,
) {
    let prop_name = attr.name();
    let mut is_new_node = true;
    let mut spec_path: Option<Path> = None;
    while resolver.is_valid() {
        if is_new_node {
            spec_path = resolver.get_local_path_for_property(&prop_name);
        }
        if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
            let offset = resolver.get_layer_to_stage_offset();
            let mut found = false;
            if bsr.process_layer_at_time(&layer, sp, &offset, &mut found) {
                return;
            }
        }
        is_new_node = resolver.next_layer();
    }
    bsr.process_fallback();
}

fn walk_bracketing_with_clips(
    resolver: &mut Resolver,
    attr: &Attribute,
    bsr: &mut BracketingSamplesResolver,
    clips_affecting_prim: &[ClipSetRefPtr],
) {
    let prop_name = attr.name();
    let mut is_new_node = true;
    let mut spec_path: Option<Path> = None;
    let mut node_has_specs = false;
    let mut clips: Vec<ClipSetRefPtr> = Vec::new();

    while resolver.is_valid() {
        if is_new_node {
            spec_path = resolver.get_local_path_for_property(&prop_name);
            node_has_specs = resolver.get_node().map(|n| n.has_specs()).unwrap_or(false);
        }

        let mut found_opinion = false;
        if node_has_specs {
            if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                let offset = resolver.get_layer_to_stage_offset();
                if bsr.process_layer_at_time(&layer, sp, &offset, &mut found_opinion) {
                    return;
                }
            }
        }

        if is_new_node {
            if let (Some(ref n), Some(ref sp)) = (resolver.get_node(), spec_path.as_ref()) {
                clips = get_clips_that_apply_to_node(clips_affecting_prim, n, sp);
                if !node_has_specs && clips.is_empty() {
                    resolver.next_node();
                    is_new_node = true;
                    continue;
                }
            }
        }

        if !found_opinion {
            if let (Some(layer), Some(ref sp)) = (resolver.get_layer(), spec_path.as_ref()) {
                for clip_set in &clips {
                    if clip_source_layer_matches_resolver_layer(clip_set.as_ref(), &layer) {
                        if bsr.process_clips(clip_set.as_ref(), sp) {
                            return;
                        }
                    }
                }
            }
        }

        is_new_node = resolver.next_layer();
    }
    bsr.process_fallback();
}

/// Matches `UsdStage::_GetBracketingTimeSamples`.
pub(crate) fn get_bracketing_time_samples_resolved(
    stage: &Stage,
    attr: &Attribute,
    resolve_info: Option<&ResolveInfo>,
    resolve_target: Option<&ResolveTarget>,
    stage_time: f64,
) -> Option<(f64, f64)> {
    let prim_path = attr.prim_path();
    let prim_index_opt = stage
        .get_prim_at_path(&prim_path)
        .and_then(|p| p.prim_index())
        .map(Arc::new);

    let Some(prim_index) = prim_index_opt else {
        let mut bsr = BracketingSamplesResolver::new(stage_time);
        let layer = stage.root_layer();
        let spec_path = attr.path();
        let mut fo = false;
        let _ = bsr.process_layer_at_time(
            &layer,
            &spec_path,
            &usd_sdf::LayerOffset::identity(),
            &mut fo,
        );
        bsr.process_fallback();
        return bsr.bracketing_pair();
    };

    let clip_cache: Option<&Arc<ClipCache>> = stage.clip_cache();
    let prim_may_have_clips = clip_cache
        .map(|cc| !cc.get_clips_for_prim(&prim_path).is_empty())
        .unwrap_or(false);

    let mut bsr = BracketingSamplesResolver::new(stage_time);

    if prim_may_have_clips {
        let clips_all = clip_cache
            .map(|cc| cc.get_clips_for_prim(&prim_path))
            .unwrap_or_default();
        let mut resolver = match resolve_target {
            Some(rt) if !rt.is_null() => {
                Resolver::new_with_resolve_target_and_resolve_info(rt, false, resolve_info)
            }
            _ => Resolver::new_with_resolve_info(&prim_index, false, resolve_info),
        };
        if !resolver.is_valid() {
            resolver = Resolver::new_with_resolve_info(&prim_index, false, resolve_info);
        }
        if !resolver.is_valid() {
            resolver = Resolver::new(&prim_index, false);
        }
        walk_bracketing_with_clips(&mut resolver, attr, &mut bsr, &clips_all);
    } else {
        let mut resolver = match resolve_target {
            Some(rt) if !rt.is_null() => {
                Resolver::new_with_resolve_target_and_resolve_info(rt, true, resolve_info)
            }
            _ => Resolver::new_with_resolve_info(&prim_index, true, resolve_info),
        };
        if !resolver.is_valid() {
            resolver = Resolver::new_with_resolve_info(&prim_index, true, resolve_info);
        }
        if !resolver.is_valid() {
            resolver = Resolver::new(&prim_index, true);
        }
        walk_bracketing_no_clips(&mut resolver, attr, &mut bsr);
    }

    bsr.bracketing_pair()
}

/// Matches `UsdStage::_GetTimeSamplesInInterval`.
pub(crate) fn get_time_samples_in_interval_resolved(
    stage: &Stage,
    attr: &Attribute,
    interval: &Interval,
    resolve_info: Option<&ResolveInfo>,
    resolve_target: Option<&ResolveTarget>,
) -> Vec<f64> {
    if interval.is_empty() {
        return Vec::new();
    }

    let prim_path = attr.prim_path();
    let prim_index_opt = stage
        .get_prim_at_path(&prim_path)
        .and_then(|p| p.prim_index())
        .map(Arc::new);

    let Some(prim_index) = prim_index_opt else {
        let mut sir = SamplesInIntervalResolver::new(*interval);
        let layer = stage.root_layer();
        let spec_path = attr.path();
        let mut fo = false;
        let _ = sir.process_layer_at_time(
            &layer,
            &spec_path,
            &usd_sdf::LayerOffset::identity(),
            &mut fo,
        );
        return sir.partial.into_iter().map(|(t, _)| t).collect();
    };

    let clip_cache: Option<&Arc<ClipCache>> = stage.clip_cache();
    let prim_may_have_clips = clip_cache
        .map(|cc| !cc.get_clips_for_prim(&prim_path).is_empty())
        .unwrap_or(false);

    let mut sir = SamplesInIntervalResolver::new(*interval);

    if prim_may_have_clips {
        let clips_all = clip_cache
            .map(|cc| cc.get_clips_for_prim(&prim_path))
            .unwrap_or_default();
        let mut resolver = match resolve_target {
            Some(rt) if !rt.is_null() => {
                Resolver::new_with_resolve_target_and_resolve_info(rt, false, resolve_info)
            }
            _ => Resolver::new_with_resolve_info(&prim_index, false, resolve_info),
        };
        if !resolver.is_valid() {
            resolver = Resolver::new_with_resolve_info(&prim_index, false, resolve_info);
        }
        if !resolver.is_valid() {
            resolver = Resolver::new(&prim_index, false);
        }
        walk_with_clips(&mut resolver, attr, &mut sir, &clips_all);
    } else {
        let mut resolver = match resolve_target {
            Some(rt) if !rt.is_null() => {
                Resolver::new_with_resolve_target_and_resolve_info(rt, true, resolve_info)
            }
            _ => Resolver::new_with_resolve_info(&prim_index, true, resolve_info),
        };
        if !resolver.is_valid() {
            resolver = Resolver::new_with_resolve_info(&prim_index, true, resolve_info);
        }
        if !resolver.is_valid() {
            resolver = Resolver::new(&prim_index, true);
        }
        walk_no_clips(&mut resolver, attr, &mut sir);
    }

    let mut times: Vec<f64> = sir.partial.into_iter().map(|(t, _)| t).collect();
    times.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    times.dedup_by(|a, b| a.to_bits() == b.to_bits());
    times
}
