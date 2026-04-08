//! UsdAttributeQuery - Object for efficiently making repeated queries for attribute values.
//!
//! Port of pxr/usd/usd/attributeQuery.h/cpp
//!
//! Retrieving an attribute's value at a particular time requires determining
//! the source of strongest opinion for that value. Often this source does not
//! vary over time. UsdAttributeQuery uses this fact to speed up repeated value
//! queries by caching the source information for an attribute.

use crate::attribute::{
    Attribute, apply_layer_offset_to_value, get_authored_spline,
    resolve_time_sample_value_from_layer,
};
use crate::interpolation::InterpolationType;
use crate::prim::Prim;
use crate::resolve_info::{ResolveInfo, ResolveInfoSource};
use crate::resolve_target::ResolveTarget;
use crate::time_code::TimeCode;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use usd_gf::Interval;
use usd_tf::Token;
use usd_vt::Value;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static DEFAULT: LazyLock<Token> = LazyLock::new(|| Token::new("default"));
    pub static SPLINE: LazyLock<Token> = LazyLock::new(|| Token::new("spline"));
}

static DEBUG_ATTRIBUTE_QUERY_NEW_CALLS: AtomicUsize = AtomicUsize::new(0);
static DEBUG_ATTRIBUTE_QUERY_NEW_TOTAL_NS: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Default)]
pub struct DebugAttributeQueryStats {
    pub new_calls: usize,
    pub new_total_ns: u64,
}

pub fn reset_debug_attribute_query_stats() {
    DEBUG_ATTRIBUTE_QUERY_NEW_CALLS.store(0, Ordering::Relaxed);
    DEBUG_ATTRIBUTE_QUERY_NEW_TOTAL_NS.store(0, Ordering::Relaxed);
}

pub fn read_debug_attribute_query_stats() -> DebugAttributeQueryStats {
    DebugAttributeQueryStats {
        new_calls: DEBUG_ATTRIBUTE_QUERY_NEW_CALLS.load(Ordering::Relaxed),
        new_total_ns: DEBUG_ATTRIBUTE_QUERY_NEW_TOTAL_NS.load(Ordering::Relaxed),
    }
}

fn debug_time_dirty_enabled() -> bool {
    std::env::var_os("USD_RS_DEBUG_TIME_DIRTY").is_some()
}

fn finalize_query_value(
    stage: &crate::Stage,
    mut value: Value,
    offset: &usd_sdf::LayerOffset,
) -> Option<Value> {
    if value.is_empty()
        || value.is::<usd_sdf::ValueBlock>()
        || value.is::<usd_sdf::AnimationBlock>()
    {
        return None;
    }
    apply_layer_offset_to_value(&mut value, offset);
    stage.make_resolved_asset_paths_value(&mut value);
    Some(value)
}

/// Object for efficiently making repeated queries for attribute values.
///
/// Matches C++ `UsdAttributeQuery`.
///
/// Retrieving an attribute's value at a particular time requires determining
/// the source of strongest opinion for that value. Often (i.e. unless the
/// attribute is affected by Value Clips) this source does not vary over time.
/// UsdAttributeQuery uses this fact to speed up repeated value queries by
/// caching the source information for an attribute.
///
/// # Thread safety
///
/// This object provides the basic thread-safety guarantee. Multiple threads
/// may call the value accessor functions simultaneously.
///
/// # Invalidation
///
/// This object does not listen for change notification. If a consumer is
/// holding on to a UsdAttributeQuery, it is their responsibility to dispose
/// of it in response to a resync change to the associated attribute.
pub struct AttributeQuery {
    attr: Attribute,
    resolve_info: ResolveInfo,
    resolve_target: Option<ResolveTarget>,
}

impl AttributeQuery {
    /// Construct an invalid query object.
    ///
    /// Matches C++ `UsdAttributeQuery()`.
    pub fn new_invalid() -> Self {
        Self {
            attr: Attribute::default(),
            resolve_info: ResolveInfo::default(),
            resolve_target: None,
        }
    }

    /// Construct a new query for the attribute attr.
    ///
    /// Matches C++ `UsdAttributeQuery(const UsdAttribute& attr)`.
    pub fn new(attr: Attribute) -> Self {
        let debug_stats = debug_time_dirty_enabled();
        let started = debug_stats.then(std::time::Instant::now);
        if debug_stats {
            DEBUG_ATTRIBUTE_QUERY_NEW_CALLS.fetch_add(1, Ordering::Relaxed);
        }
        let resolve_info = if attr.is_valid() {
            attr.get_resolve_info()
        } else {
            ResolveInfo::default()
        };

        let result = Self {
            attr,
            resolve_info,
            resolve_target: None,
        };
        if debug_stats {
            if let Some(started) = started {
                DEBUG_ATTRIBUTE_QUERY_NEW_TOTAL_NS
                    .fetch_add(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }
        }
        result
    }

    /// Construct with a pre-fetched `Arc<PrimIndex>`, avoiding redundant
    /// `stage.get_prim_at_path` + `prim_index_arc()` lookups inside get_resolve_info.
    ///
    /// Used by `Xformable::get_ordered_xform_ops` where multiple attributes on the
    /// same prim share one PrimIndex.
    pub fn new_with_prim_index(
        attr: Attribute,
        prim_index: &std::sync::Arc<usd_pcp::PrimIndex>,
    ) -> Self {
        let resolve_info = if attr.is_valid() {
            // Build resolve info using the caller's PrimIndex directly.
            // This avoids the stage.get_prim_at_path + prim_index_arc() lookups
            // that attr.get_resolve_info() would do internally.
            attr.get_resolve_info_with_index(prim_index)
        } else {
            ResolveInfo::default()
        };
        Self {
            attr,
            resolve_info,
            resolve_target: None,
        }
    }

    /// Construct a new query for the attribute named attrName under the prim prim.
    ///
    /// Matches C++ `UsdAttributeQuery(const UsdPrim& prim, const TfToken& attrName)`.
    pub fn from_prim(prim: &Prim, attr_name: &Token) -> Self {
        if let Some(attr) = prim.get_attribute(attr_name.as_str()) {
            Self::new(attr)
        } else {
            Self::new_invalid()
        }
    }

    /// Construct a new query for the attribute attr with the given resolve target.
    ///
    /// Matches C++ `UsdAttributeQuery(const UsdAttribute &attr, const UsdResolveTarget &resolveTarget)`.
    ///
    /// Note that a UsdResolveTarget is associated with a particular prim so
    /// only resolve targets for the attribute's owning prim are allowed.
    pub fn with_resolve_target(attr: Attribute, resolve_target: ResolveTarget) -> Self {
        // If resolve target is null, fall back to normal init
        if resolve_target.is_null() {
            return Self::new(attr);
        }

        if !attr.is_valid() {
            return Self::new_invalid();
        }

        // C++: validate that the resolve target's prim index path matches the attr's prim path
        if let Some(prim_index) = resolve_target.prim_index() {
            if prim_index.path() != attr.prim_path() {
                // Invalid resolve target for this attribute
                return Self::new_invalid();
            }
        }

        let resolve_info = attr.get_resolve_info_with_target(&resolve_target);

        Self {
            attr,
            resolve_info,
            resolve_target: Some(resolve_target),
        }
    }

    /// Construct new queries for the attributes named in attrNames under the prim prim.
    ///
    /// Matches C++ `CreateQueries(const UsdPrim& prim, const TfTokenVector& attrNames)`.
    ///
    /// The objects in the returned vector will line up 1-to-1 with attrNames.
    pub fn create_queries(prim: &Prim, attr_names: &[Token]) -> Vec<AttributeQuery> {
        attr_names
            .iter()
            .map(|name| Self::from_prim(prim, name))
            .collect()
    }

    // --------------------------------------------------------------------- //
    // Query information
    // --------------------------------------------------------------------- //

    /// Return the attribute associated with this query.
    ///
    /// Matches C++ `GetAttribute() const`.
    pub fn attribute(&self) -> &Attribute {
        &self.attr
    }

    /// Return true if this query is valid (i.e. it is associated with a
    /// valid attribute), false otherwise.
    ///
    /// Matches C++ `IsValid() const`.
    pub fn is_valid(&self) -> bool {
        self.attr.is_valid()
    }

    // --------------------------------------------------------------------- //
    // Value & Time-Sample Accessors
    // --------------------------------------------------------------------- //

    /// Perform value resolution to fetch the value of the attribute associated
    /// with this query at the requested UsdTimeCode time.
    ///
    /// Matches C++ `Get(T* value, UsdTimeCode time) const`.
    pub fn get(&self, time: impl Into<TimeCode>) -> Option<Value> {
        if !self.is_valid() {
            return None;
        }
        let time: TimeCode = time.into();
        let resolve_info =
            if time.is_default() && self.resolve_info.value_source_might_be_time_varying() {
                if let Some(resolve_target) = &self.resolve_target {
                    self.attr
                        .get_resolve_info_at_time_with_target(&time, resolve_target)
                } else {
                    self.attr.get_resolve_info_at_time(&time)
                }
            } else {
                self.resolve_info.clone()
            };

        let stage = self.attr.stage()?;
        let prop_name = self.attr.name();
        let spec_path = resolve_info
            .prim_path()
            .append_property(prop_name.as_str())?;

        match resolve_info.source() {
            ResolveInfoSource::None => None,
            ResolveInfoSource::Fallback => {
                let value = self.attr.get_fallback_value()?;
                finalize_query_value(&stage, value, resolve_info.layer_to_stage_offset())
            }
            ResolveInfoSource::Default => {
                let layer = resolve_info.layer()?.upgrade()?;
                let value = layer.get_field(&spec_path, &*tokens::DEFAULT)?;
                finalize_query_value(&stage, value, resolve_info.layer_to_stage_offset())
            }
            ResolveInfoSource::TimeSamples => {
                if !time.is_numeric() {
                    return None;
                }
                let layer = resolve_info.layer()?.upgrade()?;
                let local_time = if resolve_info.layer_to_stage_offset().is_identity() {
                    time.value()
                } else {
                    resolve_info
                        .layer_to_stage_offset()
                        .inverse()
                        .apply(time.value())
                };
                let value = resolve_time_sample_value_from_layer(
                    &layer,
                    &spec_path,
                    local_time,
                    time.is_pre_time(),
                    stage.interpolation_type() == InterpolationType::Linear,
                )?;
                finalize_query_value(&stage, value, resolve_info.layer_to_stage_offset())
            }
            ResolveInfoSource::ValueClips => {
                if !time.is_numeric() {
                    return None;
                }
                let sdf_time = usd_sdf::TimeCode::new(time.value());
                if let Some(cs) = resolve_info.value_clip_set() {
                    if let Some(value) = cs.get_value(self.attr.path(), sdf_time) {
                        return finalize_query_value(
                            &stage,
                            value,
                            resolve_info.layer_to_stage_offset(),
                        );
                    }
                }
                let clip_cache = stage.clip_cache()?;
                for clip_set in clip_cache.get_clips_for_prim(&self.attr.prim_path()) {
                    if let Some(value) = clip_set.get_value(self.attr.path(), sdf_time) {
                        if let Some(value) = finalize_query_value(
                            &stage,
                            value,
                            resolve_info.layer_to_stage_offset(),
                        ) {
                            return Some(value);
                        }
                    }
                }
                None
            }
            ResolveInfoSource::Spline => {
                if !time.is_numeric() {
                    return None;
                }
                let layer = resolve_info.layer()?.upgrade()?;
                let local_time = if resolve_info.layer_to_stage_offset().is_identity() {
                    time.value()
                } else {
                    resolve_info
                        .layer_to_stage_offset()
                        .inverse()
                        .apply(time.value())
                };
                let spline = get_authored_spline(&layer, &spec_path, &*tokens::SPLINE)?;
                let sample = if time.is_pre_time() {
                    spline.evaluate_pre(local_time)
                } else {
                    spline.evaluate(local_time)
                }?;
                finalize_query_value(
                    &stage,
                    Value::from(sample),
                    resolve_info.layer_to_stage_offset(),
                )
            }
        }
    }

    /// Typed get.
    ///
    /// Matches C++ `Get<T>(T* value, UsdTimeCode time) const`.
    pub fn get_typed<T: Clone + 'static>(&self, time: impl Into<TimeCode>) -> Option<T> {
        if !self.is_valid() {
            return None;
        }
        self.get(time).and_then(|v| v.downcast_clone::<T>())
    }

    /// Populates a vector with authored sample times.
    ///
    /// Matches C++ `GetTimeSamples(std::vector<double>* times) const`.
    pub fn get_time_samples(&self) -> Vec<f64> {
        if !self.is_valid() {
            return Vec::new();
        }
        let Some(stage) = self.attr.stage() else {
            return Vec::new();
        };
        stage.get_time_samples_in_interval_with_resolve_info(
            &self.attr,
            &self.resolve_info,
            &Interval::get_full_interval(),
            self.resolve_target.as_ref(),
        )
    }

    /// Populates a vector with authored sample times in interval.
    ///
    /// Matches C++ `GetTimeSamplesInInterval(const GfInterval& interval, std::vector<double>* times) const`.
    pub fn get_time_samples_in_interval(&self, interval: &Interval) -> Vec<f64> {
        if !self.is_valid() {
            return Vec::new();
        }
        let Some(stage) = self.attr.stage() else {
            return Vec::new();
        };
        stage.get_time_samples_in_interval_with_resolve_info(
            &self.attr,
            &self.resolve_info,
            interval,
            self.resolve_target.as_ref(),
        )
    }

    /// Populates the given vector with the union of all the authored sample times
    /// on all of the given attribute-query objects.
    ///
    /// Matches C++ `GetUnionedTimeSamples(const std::vector<UsdAttributeQuery> &attrQueries, std::vector<double> *times)`.
    pub fn get_unioned_time_samples(attr_queries: &[AttributeQuery]) -> Vec<f64> {
        Self::get_unioned_time_samples_in_interval(attr_queries, &Interval::get_full_interval())
    }

    /// Populates the given vector with the union of all the authored sample times
    /// in the GfInterval on all of the given attribute-query objects.
    ///
    /// Matches C++ `GetUnionedTimeSamplesInInterval(...)`.
    pub fn get_unioned_time_samples_in_interval(
        attr_queries: &[AttributeQuery],
        interval: &Interval,
    ) -> Vec<f64> {
        let mut all_times: Vec<f64> = Vec::new();

        for query in attr_queries {
            if !query.is_valid() {
                continue;
            }
            let times = query.get_time_samples_in_interval(interval);
            all_times.extend(times);
        }

        // Sort and deduplicate
        all_times.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        all_times.dedup_by(|a, b| (*a - *b).abs() < f64::EPSILON);

        all_times
    }

    /// Returns the number of time samples that have been authored.
    ///
    /// Matches C++ `GetNumTimeSamples() const`.
    pub fn get_num_time_samples(&self) -> usize {
        if !self.is_valid() {
            return 0;
        }
        let Some(stage) = self.attr.stage() else {
            return 0;
        };
        stage.get_num_time_samples_with_resolve_info(
            &self.attr,
            &self.resolve_info,
            self.resolve_target.as_ref(),
        )
    }

    /// Populate lower and upper with the next greater and lesser
    /// value relative to the desiredTime.
    ///
    /// Matches C++ `GetBracketingTimeSamples(...)`.
    pub fn get_bracketing_time_samples(&self, desired_time: f64) -> Option<(f64, f64)> {
        if !self.is_valid() {
            return None;
        }
        let Some(stage) = self.attr.stage() else {
            return None;
        };
        stage.get_bracketing_time_samples_with_resolve_info(
            &self.attr,
            &self.resolve_info,
            desired_time,
            self.resolve_target.as_ref(),
        )
    }

    /// Return true if the attribute associated with this query has an
    /// authored default value, authored time samples, or a fallback value
    /// provided by a registered schema.
    ///
    /// Matches C++ `HasValue() const`.
    pub fn has_value(&self) -> bool {
        self.resolve_info.source() != ResolveInfoSource::None
    }

    /// Return true if the attribute associated with this query has a
    /// spline value as the strongest opinion.
    ///
    /// Matches C++ `HasSpline() const`.
    pub fn has_spline(&self) -> bool {
        self.resolve_info.source() == ResolveInfoSource::Spline
    }

    /// Return true if this attribute has either an authored default value or
    /// authored time samples.
    ///
    /// Matches C++ `HasAuthoredValueOpinion() const`.
    ///
    /// Note: This method is deprecated because it returns true even when an
    /// attribute is blocked. Use has_authored_value() instead.
    #[deprecated(note = "Use has_authored_value() instead")]
    pub fn has_authored_value_opinion(&self) -> bool {
        self.resolve_info.has_authored_value_opinion()
    }

    /// Return true if this attribute has either an authored default value or
    /// authored time samples. If the attribute has been blocked, return false.
    ///
    /// Matches C++ `HasAuthoredValue() const`.
    pub fn has_authored_value(&self) -> bool {
        self.resolve_info.has_authored_value()
    }

    /// Return true if the attribute associated with this query has a
    /// fallback value provided by a registered schema.
    ///
    /// Matches C++ `HasFallbackValue() const`.
    pub fn has_fallback_value(&self) -> bool {
        self.attr.has_fallback_value()
    }

    /// If this attribute is a builtin attribute with a fallback value provided
    /// by a schema, fetch that value and return Some. Otherwise return None.
    ///
    /// Matches C++ `GetFallbackValue(T* value) const`.
    pub fn get_fallback_value(&self) -> Option<Value> {
        self.attr.get_fallback_value()
    }

    /// Return true if it is possible, but not certain, that this attribute's
    /// value changes over time, false otherwise.
    ///
    /// Matches C++ `ValueMightBeTimeVarying() const`.
    pub fn value_might_be_time_varying(&self) -> bool {
        if !self.is_valid() {
            return false;
        }
        let Some(stage) = self.attr.stage() else {
            return false;
        };
        stage.value_might_be_time_varying_from_resolve_info(&self.resolve_info, &self.attr)
    }
}

impl Default for AttributeQuery {
    fn default() -> Self {
        Self::new_invalid()
    }
}

impl Clone for AttributeQuery {
    fn clone(&self) -> Self {
        Self {
            attr: self.attr.clone(),
            resolve_info: self.resolve_info.clone(),
            resolve_target: self.resolve_target.clone(),
        }
    }
}

impl From<Attribute> for AttributeQuery {
    fn from(attr: Attribute) -> Self {
        Self::new(attr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribute_query_invalid() {
        let query = AttributeQuery::new_invalid();
        assert!(!query.is_valid());
        assert!(!query.has_value());
    }
}
