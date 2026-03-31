//! Spline sampling utilities.
//!
//! Port of pxr/base/ts/sample.h and sample.cpp
//!
//! Provides functions for sampling splines into piecewise linear
//! polylines for rendering or analysis. Supports:
//! - Adaptive Bezier subdivision using de Casteljau algorithm
//! - Inner loop unrolling
//! - Extrapolation loops (repeat, reset, oscillate)
//! - Baking splines into explicit knot representations

use super::eval::{SampleVertex, SplineSamples, SplineSamplesWithSources};
use super::knot_data::TypedKnotData;
use super::regression_preventer::RegressionPreventerBatch;
use super::spline::Spline;
use super::spline_data::TypedSplineData;
use super::types::{
    AntiRegressionMode, CurveType, ExtrapMode, InterpMode, SplineSampleSource, TsTime,
};
use usd_gf::{Interval, Vec2d};

/// Interface for adding segments to sample data.
pub trait SampleDataInterface {
    /// Adds a segment to the samples.
    fn add_segment(
        &mut self,
        time0: f64,
        value0: f64,
        time1: f64,
        value1: f64,
        source: SplineSampleSource,
    );

    /// Clears existing sample data.
    fn clear(&mut self);
}

impl<V: SampleVertex> SampleDataInterface for SplineSamples<V> {
    fn add_segment(
        &mut self,
        time0: f64,
        value0: f64,
        time1: f64,
        value1: f64,
        _source: SplineSampleSource,
    ) {
        self.add_segment(time0, value0, time1, value1);
    }

    fn clear(&mut self) {
        self.clear();
    }
}

impl<V: SampleVertex> SampleDataInterface for SplineSamplesWithSources<V> {
    fn add_segment(
        &mut self,
        time0: f64,
        value0: f64,
        time1: f64,
        value1: f64,
        source: SplineSampleSource,
    ) {
        self.add_segment_with_source(time0, value0, time1, value1, source);
    }

    fn clear(&mut self) {
        self.clear();
    }
}

/// Source interval for tracking sample regions.
#[derive(Debug, Clone)]
struct SourceInterval {
    source: SplineSampleSource,
    interval: Interval,
}

impl SourceInterval {
    fn new(source: SplineSampleSource, t1: TsTime, t2: TsTime) -> Self {
        Self {
            source,
            interval: Interval::new(t1, t2, true, false),
        }
    }
}

/// Unrolled knot reference with time and value offsets.
#[derive(Debug, Clone)]
struct UnrolledKnot {
    knot_index: usize,
    time_offset: TsTime,
    value_offset: f64,
}

impl UnrolledKnot {
    fn new(index: usize, time_off: TsTime, value_off: f64) -> Self {
        Self {
            knot_index: index,
            time_offset: time_off,
            value_offset: value_off,
        }
    }
}

/// Main sampler with loop support and adaptive subdivision.
pub struct Sampler<'a> {
    data: &'a TypedSplineData<f64>,
    time_interval: Interval,
    time_scale: f64,
    value_scale: f64,
    tolerance: f64,

    have_inner_loops: bool,
    have_multiple_knots: bool,
    first_inner_proto_index: usize,
    have_pre_extrap_loops: bool,
    have_post_extrap_loops: bool,
    first_time: TsTime,
    last_time: TsTime,
    first_inner_loop: TsTime,
    last_inner_loop: TsTime,
    first_inner_proto: TsTime,
    last_inner_proto: TsTime,

    source_intervals: Vec<SourceInterval>,
    unrolled_knots: Vec<UnrolledKnot>,
    internal_knots: Vec<TypedKnotData<f64>>,
    internal_times: Vec<TsTime>,
}

impl<'a> Sampler<'a> {
    /// Creates a new sampler.
    pub fn new(
        data: &'a TypedSplineData<f64>,
        time_interval: Interval,
        time_scale: f64,
        value_scale: f64,
        tolerance: f64,
    ) -> Self {
        let mut sampler = Self {
            data,
            time_interval,
            time_scale,
            value_scale,
            tolerance,
            have_inner_loops: false,
            have_multiple_knots: false,
            first_inner_proto_index: 0,
            have_pre_extrap_loops: false,
            have_post_extrap_loops: false,
            first_time: 0.0,
            last_time: 0.0,
            first_inner_loop: 0.0,
            last_inner_loop: 0.0,
            first_inner_proto: 0.0,
            last_inner_proto: 0.0,
            source_intervals: Vec::new(),
            unrolled_knots: Vec::new(),
            internal_knots: Vec::new(),
            internal_times: Vec::new(),
        };
        sampler.init();
        sampler
    }

    /// Creates a sampler for baking.
    pub fn for_baking(
        data: &'a TypedSplineData<f64>,
        time_interval: Interval,
        _include_extrap_loops: bool,
    ) -> Self {
        Self::new(data, time_interval, 1.0, 1.0, 1.0)
    }

    fn init(&mut self) {
        if self.data.knots.is_empty() {
            return;
        }

        self.have_inner_loops = self.data.base.has_inner_loops();
        if self.have_inner_loops {
            self.first_inner_proto_index = self.data.base.first_inner_proto_index().unwrap_or(0);
        }

        self.have_multiple_knots = self.have_inner_loops || self.data.knots.len() > 1;
        self.have_pre_extrap_loops =
            self.have_multiple_knots && self.data.base.pre_extrapolation.is_looping();
        self.have_post_extrap_loops =
            self.have_multiple_knots && self.data.base.post_extrapolation.is_looping();

        let raw_first = self.data.base.times.first().copied().unwrap_or(0.0);
        let raw_last = self.data.base.times.last().copied().unwrap_or(0.0);

        self.first_time = raw_first;
        self.last_time = raw_last;

        if self.have_inner_loops {
            let lp = &self.data.base.loop_params;
            self.first_inner_proto = lp.proto_start;
            self.last_inner_proto = lp.proto_end;

            let looped = lp.looped_interval();
            self.first_inner_loop = looped.get_min();
            self.last_inner_loop = looped.get_max();

            if self.first_inner_loop < raw_first {
                self.first_time = self.first_inner_loop;
            }
            if self.last_inner_loop > raw_last {
                self.last_time = self.last_inner_loop;
            }
        }

        self.build_source_intervals();
        self.unroll_loops();
        self.convert_unrolled_knots();
    }

    fn build_source_intervals(&mut self) {
        if self.data.base.pre_extrapolation.mode != ExtrapMode::ValueBlock {
            let source = if self.have_pre_extrap_loops {
                SplineSampleSource::PreExtrapLoop
            } else {
                SplineSampleSource::PreExtrap
            };
            self.source_intervals.push(SourceInterval::new(
                source,
                f64::NEG_INFINITY,
                self.first_time,
            ));
        }

        if self.have_inner_loops {
            if self.first_time < self.first_inner_loop {
                self.source_intervals.push(SourceInterval::new(
                    SplineSampleSource::KnotInterp,
                    self.first_time,
                    self.first_inner_loop,
                ));
            }
            if self.first_inner_loop < self.first_inner_proto {
                self.source_intervals.push(SourceInterval::new(
                    SplineSampleSource::InnerLoopPreEcho,
                    self.first_inner_loop,
                    self.first_inner_proto,
                ));
            }
            self.source_intervals.push(SourceInterval::new(
                SplineSampleSource::InnerLoopProto,
                self.first_inner_proto,
                self.last_inner_proto,
            ));
            if self.last_inner_proto < self.last_inner_loop {
                self.source_intervals.push(SourceInterval::new(
                    SplineSampleSource::InnerLoopPostEcho,
                    self.last_inner_proto,
                    self.last_inner_loop,
                ));
            }
            if self.last_inner_loop < self.last_time {
                self.source_intervals.push(SourceInterval::new(
                    SplineSampleSource::KnotInterp,
                    self.last_inner_loop,
                    self.last_time,
                ));
            }
        } else if self.first_time < self.last_time {
            self.source_intervals.push(SourceInterval::new(
                SplineSampleSource::KnotInterp,
                self.first_time,
                self.last_time,
            ));
        }

        if self.data.base.post_extrapolation.mode != ExtrapMode::ValueBlock {
            let source = if self.have_post_extrap_loops {
                SplineSampleSource::PostExtrapLoop
            } else {
                SplineSampleSource::PostExtrap
            };
            self.source_intervals
                .push(SourceInterval::new(source, self.last_time, f64::INFINITY));
        }
    }

    fn unroll_loops(&mut self) {
        if !self.have_inner_loops {
            for i in 0..self.data.knots.len() {
                self.unrolled_knots.push(UnrolledKnot::new(i, 0.0, 0.0));
            }
            return;
        }

        let lp = &self.data.base.loop_params;
        let proto_span = lp.proto_end - lp.proto_start;
        if proto_span <= 0.0 {
            for i in 0..self.data.knots.len() {
                self.unrolled_knots.push(UnrolledKnot::new(i, 0.0, 0.0));
            }
            return;
        }

        let proto_begin = self
            .data
            .base
            .times
            .iter()
            .position(|&t| t >= self.first_inner_proto)
            .unwrap_or(0);
        let proto_end = self
            .data
            .base
            .times
            .iter()
            .position(|&t| t >= self.last_inner_proto)
            .unwrap_or(self.data.base.times.len());

        let looped = lp.looped_interval().intersection(&self.time_interval);
        let pre_offset = self.first_inner_proto - looped.get_min();
        let pre_loops = (pre_offset / proto_span).ceil().max(0.0) as i32;
        let post_offset = looped.get_max() - self.last_inner_proto;
        let post_loops = (post_offset / proto_span).ceil().max(0.0) as i32;

        // Pre-loop knots
        for i in 0..proto_begin {
            self.unrolled_knots.push(UnrolledKnot::new(i, 0.0, 0.0));
        }

        // Looped copies
        for loop_idx in -pre_loops..=post_loops {
            let time_off = proto_span * (loop_idx as f64);
            let val_off = lp.value_offset * (loop_idx as f64);
            for i in proto_begin..proto_end {
                self.unrolled_knots
                    .push(UnrolledKnot::new(i, time_off, val_off));
            }
        }

        // Final boundary
        let final_t = proto_span * ((post_loops + 1) as f64);
        let final_v = lp.value_offset * ((post_loops + 1) as f64);
        self.unrolled_knots.push(UnrolledKnot::new(
            self.first_inner_proto_index,
            final_t,
            final_v,
        ));

        // Post-loop knots
        let post_begin = self
            .data
            .base
            .times
            .iter()
            .position(|&t| t > self.last_inner_loop)
            .unwrap_or(self.data.base.times.len());

        for i in post_begin..self.data.base.times.len() {
            self.unrolled_knots.push(UnrolledKnot::new(i, 0.0, 0.0));
        }
    }

    fn convert_unrolled_knots(&mut self) {
        self.internal_times.reserve(self.unrolled_knots.len());
        self.internal_knots.reserve(self.unrolled_knots.len());

        for uk in &self.unrolled_knots {
            let mut knot = self.data.knots[uk.knot_index];
            knot.base.time += uk.time_offset;
            knot.value += uk.value_offset;
            knot.pre_value += uk.value_offset;

            self.internal_times.push(knot.base.time);
            self.internal_knots.push(knot);
        }
    }

    /// Samples the spline.
    pub fn sample<T: SampleDataInterface>(&self, output: &mut T) -> bool {
        self.sample_interval(&self.time_interval, output)
    }

    /// Samples a sub-interval.
    pub fn sample_interval<T: SampleDataInterface>(
        &self,
        sub_interval: &Interval,
        output: &mut T,
    ) -> bool {
        if self.internal_knots.is_empty() {
            return false;
        }

        for si in &self.source_intervals {
            let region = sub_interval.intersection(&si.interval);
            if region.size() > 0.0 {
                match si.source {
                    SplineSampleSource::PreExtrap | SplineSampleSource::PostExtrap => {
                        self.extrap_linear(&region, si.source, output);
                    }
                    SplineSampleSource::PreExtrapLoop | SplineSampleSource::PostExtrapLoop => {
                        self.extrap_loop(&region, si.source, output);
                    }
                    _ => {
                        self.sample_knots(&region, si.source, 1.0, 0.0, 0.0, output);
                    }
                }
            }
        }

        true
    }

    fn extrap_linear<T: SampleDataInterface>(
        &self,
        region: &Interval,
        source: SplineSampleSource,
        output: &mut T,
    ) {
        let is_pre = source == SplineSampleSource::PreExtrap;
        let extrap = if is_pre {
            &self.data.base.pre_extrapolation
        } else {
            &self.data.base.post_extrapolation
        };

        let knot = if is_pre {
            &self.internal_knots[0]
        } else {
            self.internal_knots.last().expect("value expected")
        };

        let slope = match extrap.mode {
            ExtrapMode::ValueBlock => return,
            ExtrapMode::Held => 0.0,
            ExtrapMode::Sloped => extrap.slope,
            ExtrapMode::Linear => {
                if self.have_multiple_knots {
                    if is_pre {
                        knot.post_tan_slope
                    } else {
                        knot.pre_tan_slope
                    }
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };

        let t1 = region.get_min();
        let t2 = region.get_max();

        let (v1, v2) = if is_pre {
            let v2 = knot.pre_value();
            (v2 - slope * (t2 - t1), v2)
        } else {
            let v1 = knot.value;
            (v1, v1 + slope * (t2 - t1))
        };

        output.add_segment(
            t1 * self.time_scale,
            v1 * self.value_scale,
            t2 * self.time_scale,
            v2 * self.value_scale,
            source,
        );
    }

    fn extrap_loop<T: SampleDataInterface>(
        &self,
        region: &Interval,
        source: SplineSampleSource,
        output: &mut T,
    ) {
        let is_pre = source == SplineSampleSource::PreExtrapLoop;
        let extrap = if is_pre {
            &self.data.base.pre_extrapolation
        } else {
            &self.data.base.post_extrapolation
        };

        let time_delta = self.last_time - self.first_time;
        if time_delta <= 0.0 {
            return;
        }

        let value_delta = if extrap.mode == ExtrapMode::LoopRepeat {
            self.internal_knots.last().map(|k| k.value).unwrap_or(0.0)
                - self.internal_knots.first().map(|k| k.value).unwrap_or(0.0)
        } else {
            0.0
        };

        let oscillate = extrap.mode == ExtrapMode::LoopOscillate;
        let min_iter = ((region.get_min() - self.first_time) / time_delta).floor() as i64;
        let max_iter = ((region.get_max() - self.first_time) / time_delta).ceil() as i64;

        for iter_num in min_iter..max_iter {
            if iter_num == 0 {
                continue;
            }

            let reversed = oscillate && (iter_num % 2 != 0);
            let first_iter_time = self.first_time + (iter_num as f64) * time_delta;
            let last_iter_time = self.first_time + ((iter_num + 1) as f64) * time_delta;

            let (scale, offset) = if reversed {
                (-1.0, self.last_time + first_iter_time)
            } else {
                (1.0, (iter_num as f64) * time_delta)
            };

            let iter_interval = Interval::new(first_iter_time, last_iter_time, true, false);
            let sample_interval = region.intersection(&iter_interval);

            if reversed {
                self.sample_knots_reversed(&sample_interval, source, scale, offset, output);
            } else {
                let val_off = (iter_num as f64) * value_delta;
                self.sample_knots(&sample_interval, source, scale, offset, val_off, output);
            }
        }
    }

    fn sample_knots<T: SampleDataInterface>(
        &self,
        sample_interval: &Interval,
        source: SplineSampleSource,
        knot_scale: f64,
        knot_offset: TsTime,
        value_offset: f64,
        output: &mut T,
    ) {
        let knot_min = (sample_interval.get_min() - knot_offset) / knot_scale;
        let knot_max = (sample_interval.get_max() - knot_offset) / knot_scale;

        let knot_interval =
            Interval::new(knot_min.min(knot_max), knot_min.max(knot_max), true, true)
                .intersection(&Interval::new(self.first_time, self.last_time, true, true));

        if knot_interval.is_empty() {
            return;
        }

        let next_idx = self
            .internal_times
            .iter()
            .position(|&t| t > knot_interval.get_min())
            .unwrap_or(self.internal_times.len());

        if next_idx == 0 {
            return;
        }

        let end_idx = self
            .internal_times
            .iter()
            .position(|&t| t >= knot_interval.get_max())
            .unwrap_or(self.internal_times.len());

        for prev_idx in (next_idx - 1)..end_idx.min(self.internal_knots.len() - 1) {
            let n_idx = prev_idx + 1;
            if n_idx >= self.internal_knots.len() {
                break;
            }

            let prev = &self.internal_knots[prev_idx];
            let next = &self.internal_knots[n_idx];

            let seg = Interval::new(prev.base.time, next.base.time, true, true)
                .intersection(&knot_interval);

            self.sample_segment(
                prev,
                next,
                &seg,
                source,
                knot_scale,
                knot_offset,
                value_offset,
                output,
            );
        }
    }

    fn sample_knots_reversed<T: SampleDataInterface>(
        &self,
        sample_interval: &Interval,
        source: SplineSampleSource,
        knot_scale: f64,
        knot_offset: TsTime,
        output: &mut T,
    ) {
        let knot_time = (sample_interval.get_min() - knot_offset) / knot_scale;
        let knot_begin = (sample_interval.get_max() - knot_offset) / knot_scale;

        let prev_idx = self
            .internal_times
            .iter()
            .rposition(|&t| t <= knot_time)
            .unwrap_or(0);
        let begin_idx = self
            .internal_times
            .iter()
            .rposition(|&t| t <= knot_begin)
            .unwrap_or(0);

        for idx in (begin_idx..prev_idx).rev() {
            if idx + 1 >= self.internal_knots.len() {
                continue;
            }

            let prev = &self.internal_knots[idx + 1];
            let next = &self.internal_knots[idx];

            let mut prev_data = *prev;
            prev_data.base.time = prev_data.base.time * knot_scale + knot_offset;
            prev_data.value = prev_data.pre_value();
            prev_data.base.post_tan_width = prev.base.pre_tan_width;
            prev_data.post_tan_slope = -prev.pre_tan_slope;

            let mut next_data = *next;
            next_data.base.dual_valued = false;
            next_data.base.time = next_data.base.time * knot_scale + knot_offset;
            next_data.base.pre_tan_width = next.base.post_tan_width;
            next_data.pre_tan_slope = -next.post_tan_slope;
            prev_data.base.next_interp = next.base.next_interp;

            let seg = Interval::new(
                prev_data.base.time.min(next_data.base.time),
                prev_data.base.time.max(next_data.base.time),
                true,
                true,
            )
            .intersection(sample_interval);

            self.sample_segment(&prev_data, &next_data, &seg, source, 1.0, 0.0, 0.0, output);
        }
    }

    fn sample_segment<T: SampleDataInterface>(
        &self,
        prev: &TypedKnotData<f64>,
        next: &TypedKnotData<f64>,
        seg: &Interval,
        source: SplineSampleSource,
        knot_scale: f64,
        knot_offset: TsTime,
        value_offset: f64,
        output: &mut T,
    ) {
        if prev.base.next_interp == InterpMode::ValueBlock {
            return;
        }

        if prev.base.next_interp == InterpMode::Curve {
            self.sample_curve_segment(
                prev,
                next,
                seg,
                source,
                knot_scale,
                knot_offset,
                value_offset,
                output,
            );
            return;
        }

        // Linear/held
        let mut t1 = prev.base.time * knot_scale + knot_offset;
        let mut v1 = prev.value + value_offset;
        let t2 = next.base.time * knot_scale + knot_offset;
        let v2 = if prev.base.next_interp == InterpMode::Held {
            prev.value + value_offset
        } else {
            next.pre_value() + value_offset
        };

        // Clip
        if t1 < self.time_interval.get_min() && v1 != v2 {
            let u = (self.time_interval.get_min() - t1) / (t2 - t1);
            v1 = lerp(u, v1, v2);
            t1 = self.time_interval.get_min();
        }

        output.add_segment(
            t1 * self.time_scale,
            v1 * self.value_scale,
            t2 * self.time_scale,
            v2 * self.value_scale,
            source,
        );
    }

    fn sample_curve_segment<T: SampleDataInterface>(
        &self,
        prev: &TypedKnotData<f64>,
        next: &TypedKnotData<f64>,
        seg: &Interval,
        source: SplineSampleSource,
        knot_scale: f64,
        knot_offset: TsTime,
        value_offset: f64,
        output: &mut T,
    ) {
        let cp = match self.data.base.curve_type {
            CurveType::Bezier => {
                let mut p = *prev;
                let mut n = *next;
                RegressionPreventerBatch::process_segment(
                    &mut p.base,
                    &mut n.base,
                    AntiRegressionMode::KeepRatio,
                );

                let p0 = Vec2d::new(p.base.time, p.value);
                let p3 = Vec2d::new(n.base.time, n.pre_value());
                let p1 = p0
                    + Vec2d::new(
                        p.base.post_tan_width,
                        p.post_tan_slope * p.base.post_tan_width,
                    );
                let p2 = p3
                    + Vec2d::new(
                        -n.base.pre_tan_width,
                        n.pre_tan_slope * n.base.pre_tan_width,
                    );
                [p0, p1, p2, p3]
            }
            CurveType::Hermite => {
                let dt = next.base.time - prev.base.time;
                let dt_3 = dt / 3.0;

                let p0 = Vec2d::new(prev.base.time, prev.value);
                let p3 = Vec2d::new(next.base.time, next.pre_value());
                let p1 = p0 + Vec2d::new(dt_3, dt_3 * prev.post_tan_slope);
                let p2 = p3 - Vec2d::new(dt_3, dt_3 * next.pre_tan_slope);
                [p0, p1, p2, p3]
            }
        };

        self.sample_bezier(
            &cp,
            seg,
            source,
            knot_scale,
            knot_offset,
            value_offset,
            output,
        );
    }

    fn sample_bezier<T: SampleDataInterface>(
        &self,
        cp: &[Vec2d; 4],
        seg: &Interval,
        source: SplineSampleSource,
        knot_scale: f64,
        knot_offset: TsTime,
        value_offset: f64,
        output: &mut T,
    ) {
        let scale_vec = Vec2d::new(self.time_scale, self.value_scale);
        let base = comp_mult(&scale_vec, &(cp[3] - cp[0]));
        let v1 = comp_mult(&scale_vec, &(cp[1] - cp[0]));
        let v2 = comp_mult(&scale_vec, &(cp[2] - cp[0]));

        let len_sq = base.length_squared();
        if len_sq < 1e-20 {
            return;
        }

        let t1 = v1.dot(&base) / len_sq;
        let t2 = v2.dot(&base) / len_sq;

        let h1_sq = (v1 - base * t1).length_squared();
        let h2_sq = (v2 - base * t2).length_squared();

        let tol_sq = self.tolerance * self.tolerance;

        if h1_sq.max(h2_sq) <= tol_sq {
            // Flat - output line
            let mut t1 = cp[0][0];
            let mut t2 = cp[3][0];
            let mut val1 = cp[0][1];
            let mut val2 = cp[3][1];

            if t1 < seg.get_min() {
                let u = (seg.get_min() - t1) / (t2 - t1);
                t1 = lerp(u, t1, t2);
                val1 = lerp(u, val1, val2);
            }
            if t2 > seg.get_max() {
                let u = (seg.get_max() - t1) / (t2 - t1);
                t2 = lerp(u, t1, t2);
                val2 = lerp(u, val1, val2);
            }

            let st1 = t1 * knot_scale + knot_offset;
            let st2 = t2 * knot_scale + knot_offset;

            output.add_segment(
                st1 * self.time_scale,
                (val1 + value_offset) * self.value_scale,
                st2 * self.time_scale,
                (val2 + value_offset) * self.value_scale,
                source,
            );
        } else {
            let (left, right) = subdivide_bezier(cp, 0.5);

            let do_left = seg.intersects(&Interval::new(left[0][0], left[3][0], true, true));
            let do_right = seg.intersects(&Interval::new(right[0][0], right[3][0], true, true));

            if knot_scale < 0.0 {
                if do_right {
                    self.sample_bezier(
                        &right,
                        seg,
                        source,
                        knot_scale,
                        knot_offset,
                        value_offset,
                        output,
                    );
                }
                if do_left {
                    self.sample_bezier(
                        &left,
                        seg,
                        source,
                        knot_scale,
                        knot_offset,
                        value_offset,
                        output,
                    );
                }
            } else {
                if do_left {
                    self.sample_bezier(
                        &left,
                        seg,
                        source,
                        knot_scale,
                        knot_offset,
                        value_offset,
                        output,
                    );
                }
                if do_right {
                    self.sample_bezier(
                        &right,
                        seg,
                        source,
                        knot_scale,
                        knot_offset,
                        value_offset,
                        output,
                    );
                }
            }
        }
    }
}

#[inline]
fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline]
fn comp_mult(a: &Vec2d, b: &Vec2d) -> Vec2d {
    Vec2d::new(a[0] * b[0], a[1] * b[1])
}

/// De Casteljau subdivision at parameter u.
fn subdivide_bezier(cp: &[Vec2d; 4], u: f64) -> ([Vec2d; 4], [Vec2d; 4]) {
    let cp01 = lerp_vec(u, &cp[0], &cp[1]);
    let cp12 = lerp_vec(u, &cp[1], &cp[2]);
    let cp23 = lerp_vec(u, &cp[2], &cp[3]);

    let cp012 = lerp_vec(u, &cp01, &cp12);
    let cp123 = lerp_vec(u, &cp12, &cp23);

    let cp0123 = lerp_vec(u, &cp012, &cp123);

    ([cp[0], cp01, cp012, cp0123], [cp0123, cp123, cp23, cp[3]])
}

#[inline]
fn lerp_vec(t: f64, a: &Vec2d, b: &Vec2d) -> Vec2d {
    Vec2d::new(a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]))
}

// Public API

/// Samples a spline into polylines.
pub fn sample_spline<V: SampleVertex>(
    spline: &Spline,
    interval: &Interval,
    time_scale: f64,
    value_scale: f64,
    tolerance: f64,
    output: &mut SplineSamples<V>,
) {
    output.clear();
    if spline.is_empty() {
        return;
    }

    let data = spline_to_typed_data(spline);
    let sampler = Sampler::new(&data, *interval, time_scale, value_scale, tolerance);
    sampler.sample(output);
}

/// Samples a spline with source tracking.
pub fn sample_spline_with_sources<V: SampleVertex>(
    spline: &Spline,
    interval: &Interval,
    time_scale: f64,
    value_scale: f64,
    tolerance: f64,
    output: &mut SplineSamplesWithSources<V>,
) {
    output.clear();
    if spline.is_empty() {
        return;
    }

    let data = spline_to_typed_data(spline);
    let sampler = Sampler::new(&data, *interval, time_scale, value_scale, tolerance);
    sampler.sample(output);
}

pub(crate) fn spline_to_typed_data(spline: &Spline) -> TypedSplineData<f64> {
    let mut data = TypedSplineData::new();
    data.base.curve_type = spline.curve_type();
    data.base.pre_extrapolation = *spline.pre_extrapolation();
    data.base.post_extrapolation = *spline.post_extrapolation();
    data.base.loop_params = *spline.inner_loop_params();

    for knot in spline.knots() {
        let mut kd = TypedKnotData::<f64>::new();
        kd.base.time = knot.time();
        kd.value = knot.value();
        kd.base.next_interp = knot.interp_mode();
        kd.base.dual_valued = knot.is_dual_valued();
        kd.pre_value = if kd.base.dual_valued {
            knot.pre_value()
        } else {
            kd.value
        };
        kd.base.pre_tan_width = knot.pre_tangent().width;
        kd.base.post_tan_width = knot.post_tangent().width;
        kd.pre_tan_slope = knot.pre_tangent().slope;
        kd.post_tan_slope = knot.post_tangent().slope;

        data.base.times.push(kd.base.time);
        data.knots.push(kd);
    }

    data
}

/// Bakes a spline into explicit knots.
pub fn bake_spline<T>(
    spline: &Spline,
    interval: &Interval,
    include_extrap_loops: bool,
) -> Option<TypedSplineData<T>>
where
    T: Clone + Default + PartialEq + num_traits::NumCast + std::ops::DivAssign<T> + 'static,
{
    if spline.is_empty() {
        return None;
    }

    let data = spline_to_typed_data(spline);
    let sampler = Sampler::for_baking(&data, *interval, include_extrap_loops);

    let mut output: TypedSplineData<T> = TypedSplineData::new();
    output.base.curve_type = data.base.curve_type;

    for knot in &sampler.internal_knots {
        let mut kd = TypedKnotData::<T>::new();
        kd.base = knot.base;
        kd.value = num_traits::NumCast::from(knot.value).unwrap_or_default();
        kd.pre_value = num_traits::NumCast::from(knot.pre_value).unwrap_or_default();
        kd.pre_tan_slope = num_traits::NumCast::from(knot.pre_tan_slope).unwrap_or_default();
        kd.post_tan_slope = num_traits::NumCast::from(knot.post_tan_slope).unwrap_or_default();
        output.push_knot(kd, None);
    }

    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InterpMode, Knot};
    use usd_gf::Vec2d;

    #[test]
    fn test_sample_empty_spline() {
        let spline = Spline::new();
        let interval = Interval::new(0.0, 10.0, true, true);
        let mut samples: SplineSamples<Vec2d> = SplineSamples::new();
        sample_spline(&spline, &interval, 1.0, 1.0, 0.1, &mut samples);
        assert!(samples.is_empty());
    }

    #[test]
    fn test_sample_linear_segment() {
        let mut spline = Spline::new();
        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(0.0, 10.0, true, true);
        let mut samples: SplineSamples<Vec2d> = SplineSamples::new();
        sample_spline(&spline, &interval, 1.0, 1.0, 0.1, &mut samples);
        assert!(!samples.is_empty());
    }

    #[test]
    fn test_sample_with_sources() {
        let mut spline = Spline::new();
        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(0.0, 10.0, true, true);
        let mut samples: SplineSamplesWithSources<Vec2d> = SplineSamplesWithSources::new();
        sample_spline_with_sources(&spline, &interval, 1.0, 1.0, 0.1, &mut samples);
        assert!(!samples.is_empty());
        assert!(!samples.sources.is_empty());
    }

    #[test]
    fn test_subdivide_bezier() {
        let cp = [
            Vec2d::new(0.0, 0.0),
            Vec2d::new(1.0, 2.0),
            Vec2d::new(2.0, 2.0),
            Vec2d::new(3.0, 0.0),
        ];
        let (left, right) = subdivide_bezier(&cp, 0.5);

        assert!((left[0][0] - cp[0][0]).abs() < 1e-10);
        assert!((right[3][0] - cp[3][0]).abs() < 1e-10);
        assert!((left[3][0] - right[0][0]).abs() < 1e-10);
    }

    #[test]
    fn test_lerp() {
        assert!((lerp(0.0, 10.0, 20.0) - 10.0).abs() < 1e-10);
        assert!((lerp(1.0, 10.0, 20.0) - 20.0).abs() < 1e-10);
        assert!((lerp(0.5, 10.0, 20.0) - 15.0).abs() < 1e-10);
    }

    #[test]
    fn test_bake_spline() {
        let mut spline = Spline::new();
        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(0.0, 10.0, true, true);
        let baked: Option<TypedSplineData<f64>> = bake_spline(&spline, &interval, false);
        assert!(baked.is_some());
        assert!(!baked.expect("value expected").knots.is_empty());
    }
}
