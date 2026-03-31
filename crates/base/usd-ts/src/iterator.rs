//! Spline segment iterators.
//!
//! Port of pxr/base/ts/iterator.h and iterator.cpp
//!
//! Provides iterators for walking spline segments, handling:
//! - Inner looping (duplicate knots with offset times/values)
//! - Extrapolation looping (repeat/oscillate beyond knot range)
//! - Forward and reverse iteration
//!
//! All complexity from inner and extrapolation looping is dealt with by
//! a collection of five classes:
//!
//! - `Segment`: One "segment" of the spline (span between 2 knots or to infinity)
//! - `SegmentPrototypeIterator`: Iterates over inner loop prototype region
//! - `SegmentLoopIterator`: Iterates over all segments from inner looping
//! - `SegmentKnotIterator`: Iterates over knot-defined region (pre/inner/post)
//! - `SegmentIterator`: Full spline iterator including extrapolation

use super::segment::{Segment, SegmentInterp};
use super::spline::Spline;
use super::types::{ExtrapMode, LoopParams, TsTime};
use usd_gf::{Interval, Vec2d};

const INF: f64 = f64::INFINITY;
const NEG_INF: f64 = f64::NEG_INFINITY;

// ============================================================================
// SplineSampleSource - which region we're iterating
// ============================================================================

/// Source region for spline samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplineSampleSource {
    /// Pre-extrapolation (non-looping)
    PreExtrap,
    /// Pre-extrapolation looping
    PreExtrapLoop,
    /// Knot interpolation region
    KnotInterp,
    /// Post-extrapolation looping
    PostExtrapLoop,
    /// Post-extrapolation (non-looping)
    PostExtrap,
}

// ============================================================================
// KnotSection - section within knot-defined region
// ============================================================================

/// Section within the knot-defined region relative to inner looping.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum KnotSection {
    /// Before inner looping region
    PreInnerLooping,
    /// Within inner looping region
    InnerLooping,
    /// After inner looping region
    PostInnerLooping,
}

// ============================================================================
// Helper functions
// ============================================================================

/// Get a looped interval that's open on the right-hand end.
fn get_open_looped_interval(lp: &LoopParams) -> Interval {
    let mut result = lp.looped_interval();
    result.set_max_with_closed(result.get_max(), false);
    result
}

/// Build a segment from two consecutive knots.
fn build_segment_from_knots(
    spline: &Spline,
    k0_idx: usize,
    k1_idx: usize,
    k1_offset: Option<(TsTime, f64)>, // (time_offset, value_offset) for virtual knot
) -> Segment {
    let knots: Vec<_> = spline.knots().collect();
    if k0_idx >= knots.len() || k1_idx >= knots.len() {
        return Segment::default();
    }

    let k0 = &knots[k0_idx];
    let k1 = &knots[k1_idx];

    let k0_time = k0.time();
    let k0_value = k0.value();
    let k1_time;
    let k1_value;

    if let Some((t_off, v_off)) = k1_offset {
        k1_time = k1.time() + t_off;
        k1_value = k1.value() + v_off;
    } else {
        k1_time = k1.time();
        k1_value = k1.value();
    }

    let p0 = Vec2d::new(k0_time, k0_value);
    let p1 = Vec2d::new(k1_time, k1_value);

    // Compute tangent control points
    let post_width = k0.post_tangent().width;
    let post_slope = k0.post_tangent().slope;
    let pre_width = k1.pre_tangent().width;
    let pre_slope = k1.pre_tangent().slope;

    let t0 = Vec2d::new(k0_time + post_width, k0_value + post_width * post_slope);
    let t1 = Vec2d::new(k1_time - pre_width, k1_value - pre_width * pre_slope);

    let interp = SegmentInterp::from_interp_and_curve(k0.interp_mode(), spline.curve_type());

    Segment {
        p0,
        t0,
        t1,
        p1,
        interp,
    }
}

// ============================================================================
// SegmentPrototypeIterator
// ============================================================================

/// Iterator for segments within the inner loop prototype region.
///
/// If inner looping is in effect, iterates over segments that intersect
/// the specified time interval. Supports forward and reverse iteration.
#[derive(Clone)]
pub struct SegmentPrototypeIterator<'a> {
    spline: &'a Spline,
    interval: Interval,
    reversed: bool,
    at_end: bool,
    times_idx: usize,
    first_proto_knot_index: usize,
    current_segment: Segment,
}

impl<'a> SegmentPrototypeIterator<'a> {
    /// Creates a new prototype iterator.
    pub fn new(spline: &'a Spline, interval: Interval, reversed: bool) -> Self {
        let mut iter = Self {
            spline,
            interval,
            reversed,
            at_end: true,
            times_idx: 0,
            first_proto_knot_index: 0,
            current_segment: Segment::default(),
        };
        iter.init();
        iter
    }

    fn init(&mut self) {
        if self.spline.is_empty() {
            return;
        }

        let loop_params = self.spline.inner_loop_params();
        if !loop_params.is_enabled() {
            return;
        }

        // Find first prototype knot index
        let knots: Vec<_> = self.spline.knots().collect();
        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();

        for (i, &t) in times.iter().enumerate() {
            if t >= loop_params.proto_start {
                self.first_proto_knot_index = i;
                break;
            }
        }

        let proto_interval = loop_params.prototype_interval();
        let iter_interval = self.interval.intersection(&proto_interval);

        if iter_interval.is_empty() {
            return;
        }

        if self.reversed {
            // Find the beginning of the segment containing max time
            let max_time = iter_interval.get_max();
            match times.binary_search_by(|t| t.partial_cmp(&max_time).expect("value expected")) {
                Ok(i) => {
                    if self.interval.is_max_closed() {
                        self.times_idx = i;
                    } else if i > 0 {
                        self.times_idx = i - 1;
                    }
                }
                Err(i) => {
                    if i > 0 {
                        self.times_idx = i - 1;
                    }
                }
            }
        } else {
            // Find the beginning of the segment containing min time
            let min_time = iter_interval.get_min();
            match times.binary_search_by(|t| t.partial_cmp(&min_time).expect("value expected")) {
                Ok(i) => self.times_idx = i,
                Err(i) => {
                    if i > 0 {
                        self.times_idx = i - 1;
                    }
                }
            }
        }

        self.at_end = false;
        self.update_segment();
    }

    fn update_segment(&mut self) {
        if self.at_end {
            self.current_segment = Segment::default();
            return;
        }

        let knots: Vec<_> = self.spline.knots().collect();
        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();
        let loop_params = self.spline.inner_loop_params();

        if self.times_idx >= times.len() {
            self.at_end = true;
            return;
        }

        let prev_index = self.times_idx;
        let next_index = prev_index + 1;

        // Check if we've gone past the prototype region
        if next_index < times.len() && times[next_index] < loop_params.proto_end {
            // Still within prototype knots
            self.current_segment =
                build_segment_from_knots(self.spline, prev_index, next_index, None);
        } else {
            // Past last knot in prototype - use virtual copy of first prototype knot
            let time_offset = loop_params.proto_end - loop_params.proto_start;
            let value_offset = loop_params.value_offset;
            self.current_segment = build_segment_from_knots(
                self.spline,
                prev_index,
                self.first_proto_knot_index,
                Some((time_offset, value_offset)),
            );
        }
    }

    /// Returns true if at end of iteration.
    pub fn at_end(&self) -> bool {
        self.at_end
    }

    /// Returns the current segment.
    pub fn segment(&self) -> &Segment {
        &self.current_segment
    }

    /// Advances to the next segment.
    pub fn next(&mut self) {
        if self.at_end {
            return;
        }

        let knots: Vec<_> = self.spline.knots().collect();
        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();
        let loop_params = self.spline.inner_loop_params();

        if self.reversed {
            if self.times_idx == 0
                || times[self.times_idx] <= loop_params.proto_start
                || times[self.times_idx] <= self.interval.get_min()
            {
                self.at_end = true;
            } else {
                self.times_idx -= 1;
            }
        } else {
            self.times_idx += 1;
            if self.times_idx >= times.len()
                || times[self.times_idx] >= loop_params.proto_end
                || !self.interval.contains(times[self.times_idx])
            {
                self.at_end = true;
            }
        }

        self.update_segment();
    }
}

// ============================================================================
// SegmentLoopIterator
// ============================================================================

/// Iterator over all segments generated by inner looping.
///
/// Uses SegmentPrototypeIterator repeatedly to iterate over
/// multiple loop iterations with time/value offsets.
#[derive(Clone)]
pub struct SegmentLoopIterator<'a> {
    spline: &'a Spline,
    interval: Interval,
    reversed: bool,
    at_end: bool,
    proto_iter: Option<SegmentPrototypeIterator<'a>>,
    current_iteration: i32,
    min_iteration: i32,
    max_iteration: i32,
    proto_span: TsTime,
    value_offset: f64,
    current_segment: Segment,
}

impl<'a> SegmentLoopIterator<'a> {
    /// Creates a new loop iterator.
    pub fn new(spline: &'a Spline, interval: Interval, reversed: bool) -> Self {
        let mut iter = Self {
            spline,
            interval,
            reversed,
            at_end: true,
            proto_iter: None,
            current_iteration: 0,
            min_iteration: 0,
            max_iteration: 0,
            proto_span: 0.0,
            value_offset: 0.0,
            current_segment: Segment::default(),
        };
        iter.init();
        iter
    }

    fn init(&mut self) {
        let loop_params = self.spline.inner_loop_params();
        if !loop_params.is_enabled() {
            self.at_end = true;
            return;
        }

        let looped_interval = get_open_looped_interval(loop_params);
        let iter_interval = self.interval.intersection(&looped_interval);

        if iter_interval.is_empty() {
            return;
        }

        let proto_start = loop_params.proto_start;
        let proto_end = loop_params.proto_end;
        self.proto_span = proto_end - proto_start;
        self.value_offset = loop_params.value_offset;

        if self.proto_span <= 0.0 {
            return;
        }

        self.min_iteration = -loop_params.num_pre_loops;
        self.max_iteration = loop_params.num_post_loops;

        // Determine starting iteration
        let initial_time_offset = if self.reversed {
            iter_interval.get_max() - proto_start
        } else {
            iter_interval.get_min() - proto_start
        };

        self.current_iteration = (initial_time_offset / self.proto_span).floor() as i32;

        if self.reversed && self.current_iteration as f64 * self.proto_span == initial_time_offset {
            // Exactly at boundary when reversed - go to previous iteration
            self.current_iteration -= 1;
        }

        self.current_iteration = self
            .current_iteration
            .clamp(self.min_iteration, self.max_iteration);

        self.update_proto_iter();
        self.at_end = self.proto_iter.as_ref().is_none_or(|p| p.at_end());
    }

    fn update_proto_iter(&mut self) {
        let loop_params = self.spline.inner_loop_params();
        let looped_interval = get_open_looped_interval(loop_params);
        let iter_interval = self.interval.intersection(&looped_interval);

        let time_delta = self.current_iteration as f64 * self.proto_span;
        let proto_iter_interval = Interval::new(
            iter_interval.get_min() - time_delta,
            iter_interval.get_max() - time_delta,
            iter_interval.is_min_closed(),
            iter_interval.is_max_closed(),
        );

        let proto = SegmentPrototypeIterator::new(self.spline, proto_iter_interval, self.reversed);
        self.proto_iter = Some(proto);
        self.update_segment();
    }

    fn update_segment(&mut self) {
        if self.at_end {
            self.current_segment = Segment::default();
            return;
        }

        if let Some(ref proto) = self.proto_iter {
            if proto.at_end() {
                self.current_segment = Segment::default();
                return;
            }

            let mut seg = *proto.segment();
            let time_delta = self.current_iteration as f64 * self.proto_span;
            let value_delta = self.current_iteration as f64 * self.value_offset;
            seg.offset(Vec2d::new(time_delta, value_delta));
            self.current_segment = seg;
        }
    }

    /// Returns true if at end of iteration.
    pub fn at_end(&self) -> bool {
        self.at_end
    }

    /// Returns the current segment.
    pub fn segment(&self) -> &Segment {
        &self.current_segment
    }

    /// Advances to the next segment.
    pub fn next(&mut self) {
        if self.at_end {
            return;
        }

        if let Some(ref mut proto) = self.proto_iter {
            proto.next();
            if proto.at_end() {
                // Hit end of a prototype loop
                self.current_iteration += if self.reversed { -1 } else { 1 };

                if self.current_iteration < self.min_iteration
                    || self.current_iteration > self.max_iteration
                {
                    self.at_end = true;
                } else {
                    self.update_proto_iter();
                    if self.proto_iter.as_ref().is_none_or(|p| p.at_end()) {
                        self.at_end = true;
                    }
                }
            }
        } else {
            self.at_end = true;
        }

        self.update_segment();
    }
}

// ============================================================================
// SegmentKnotIterator
// ============================================================================

/// Iterator over all knot-defined segments.
///
/// Iterates over segments before, within, and after inner looping.
/// Handles the transition between these regions.
#[derive(Clone)]
pub struct SegmentKnotIterator<'a> {
    spline: &'a Spline,
    interval: Interval,
    reversed: bool,
    at_end: bool,
    loop_iter: Option<SegmentLoopIterator<'a>>,
    section: KnotSection,
    times_idx: usize,
    current_segment: Segment,
    has_inner_loops: bool,
    looped_interval: Interval,
    first_proto_knot_index: usize,
}

impl<'a> SegmentKnotIterator<'a> {
    /// Creates a new knot iterator.
    pub fn new(spline: &'a Spline, interval: Interval, reversed: bool) -> Self {
        let mut iter = Self {
            spline,
            interval,
            reversed,
            at_end: true,
            loop_iter: None,
            section: KnotSection::PostInnerLooping,
            times_idx: 0,
            current_segment: Segment::default(),
            has_inner_loops: false,
            looped_interval: Interval::new_empty(),
            first_proto_knot_index: 0,
        };
        iter.init();
        iter
    }

    fn init(&mut self) {
        if self.spline.is_empty() {
            return;
        }

        let knots: Vec<_> = self.spline.knots().collect();
        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();

        if times.len() < 2 {
            return;
        }

        let loop_params = self.spline.inner_loop_params();
        self.has_inner_loops = loop_params.is_enabled();

        let mut first_time = times[0];
        let mut last_time = times[times.len() - 1];

        if self.has_inner_loops {
            self.looped_interval = get_open_looped_interval(loop_params);

            // Find first prototype knot
            for (i, &t) in times.iter().enumerate() {
                if t >= loop_params.proto_start {
                    self.first_proto_knot_index = i;
                    break;
                }
            }

            first_time = first_time.min(self.looped_interval.get_min());
            last_time = last_time.max(self.looped_interval.get_max());
        }

        // Constrain interval to knot range (closed at min, open at max)
        self.interval = self
            .interval
            .intersection(&Interval::new(first_time, last_time, true, false));
        if self.interval.is_empty() {
            return;
        }

        // Determine starting section
        if self.has_inner_loops {
            if self.reversed {
                if times[times.len() - 1] > self.looped_interval.get_max()
                    && self.interval.get_max() > self.looped_interval.get_max()
                {
                    self.section = KnotSection::PreInnerLooping;
                } else if self.interval.get_max() > self.looped_interval.get_min() {
                    self.section = KnotSection::InnerLooping;
                } else if self.interval.get_max() > times[0] {
                    self.section = KnotSection::PostInnerLooping;
                } else {
                    return;
                }
            } else if times[0] < self.looped_interval.get_min()
                && self.interval.get_min() < self.looped_interval.get_min()
            {
                self.section = KnotSection::PreInnerLooping;
            } else if self.interval.get_min() < self.looped_interval.get_max() {
                self.section = KnotSection::InnerLooping;
            } else if self.interval.get_min() < times[times.len() - 1] {
                self.section = KnotSection::PostInnerLooping;
            } else {
                return;
            }
        } else {
            self.section = KnotSection::PostInnerLooping;
        }

        // Initialize iterator based on section
        if self.section == KnotSection::InnerLooping {
            self.loop_iter = Some(SegmentLoopIterator::new(
                self.spline,
                self.interval,
                self.reversed,
            ));
        } else {
            // Find starting times index
            if self.reversed {
                let max_time = self.interval.get_max();
                match times.binary_search_by(|t| t.partial_cmp(&max_time).expect("value expected"))
                {
                    Ok(i) => {
                        if self.interval.is_max_closed() {
                            self.times_idx = i;
                        } else if i > 0 {
                            self.times_idx = i - 1;
                        }
                    }
                    Err(i) => {
                        if i > 0 {
                            self.times_idx = i - 1;
                        }
                    }
                }
            } else {
                let min_time = self.interval.get_min();
                match times.binary_search_by(|t| t.partial_cmp(&min_time).expect("value expected"))
                {
                    Ok(i) => self.times_idx = i,
                    Err(i) => {
                        if i > 0 {
                            self.times_idx = i - 1;
                        }
                    }
                }
            }
        }

        self.at_end = false;
        self.update_segment();
    }

    fn update_segment(&mut self) {
        if self.at_end {
            self.current_segment = Segment::default();
            return;
        }

        // If in inner looping, use loop iterator
        if self.section == KnotSection::InnerLooping {
            if let Some(ref loop_iter) = self.loop_iter {
                self.current_segment = *loop_iter.segment();
            }
            return;
        }

        // Build segment from current knot to next
        let knots: Vec<_> = self.spline.knots().collect();
        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();

        if self.times_idx >= times.len() - 1 {
            self.at_end = true;
            return;
        }

        let prev_index = self.times_idx;
        let next_index = prev_index + 1;

        // Check if we're running into the looped interval
        let loop_params = self.spline.inner_loop_params();

        if self.has_inner_loops {
            // Handle boundary knots with offset
            let use_offset_prev = (self.section == KnotSection::PreInnerLooping && self.reversed)
                || (self.section == KnotSection::PostInnerLooping && !self.reversed);
            let use_offset_next = (self.section == KnotSection::PreInnerLooping && !self.reversed)
                || (self.section == KnotSection::PostInnerLooping && self.reversed);

            if use_offset_prev && times[prev_index] <= self.looped_interval.get_max() {
                // Use virtual first proto knot with offset
                let time_offset = (loop_params.proto_end - loop_params.proto_start)
                    * (loop_params.num_post_loops as f64 + 1.0);
                let value_offset =
                    loop_params.value_offset * (loop_params.num_post_loops as f64 + 1.0);

                // Build segment with virtual prev knot
                let mut seg = build_segment_from_knots(
                    self.spline,
                    self.first_proto_knot_index,
                    next_index,
                    None,
                );
                seg.p0[0] += time_offset;
                seg.p0[1] += value_offset;
                seg.t0[0] += time_offset;
                seg.t0[1] += value_offset;
                self.current_segment = seg;
                return;
            }

            if use_offset_next
                && next_index < times.len()
                && times[next_index] >= self.looped_interval.get_min()
            {
                // Use virtual first proto knot with offset
                let time_offset = -(loop_params.proto_end - loop_params.proto_start)
                    * loop_params.num_pre_loops as f64;
                let value_offset = -loop_params.value_offset * loop_params.num_pre_loops as f64;

                // Build segment with virtual next knot
                self.current_segment = build_segment_from_knots(
                    self.spline,
                    prev_index,
                    self.first_proto_knot_index,
                    Some((time_offset, value_offset)),
                );
                return;
            }
        }

        self.current_segment = build_segment_from_knots(self.spline, prev_index, next_index, None);
    }

    /// Returns true if at end of iteration.
    pub fn at_end(&self) -> bool {
        self.at_end
    }

    /// Returns the current segment.
    pub fn segment(&self) -> &Segment {
        &self.current_segment
    }

    /// Advances to the next segment.
    pub fn next(&mut self) {
        if self.at_end {
            return;
        }

        let knots: Vec<_> = self.spline.knots().collect();
        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();

        if self.section == KnotSection::InnerLooping {
            if let Some(ref mut loop_iter) = self.loop_iter {
                loop_iter.next();
                if loop_iter.at_end() {
                    // Done with inner looping
                    let last_seg = &self.current_segment;
                    let done = if self.reversed {
                        last_seg.p0[0] <= self.interval.get_min()
                    } else {
                        last_seg.p1[0] >= self.interval.get_max()
                    };

                    if done {
                        self.at_end = true;
                    } else {
                        // Move to PostInnerLooping
                        self.section = KnotSection::PostInnerLooping;
                        self.loop_iter = None;

                        // Find appropriate times index
                        if self.reversed {
                            let target = self.looped_interval.get_min();
                            match times.binary_search_by(|t| {
                                t.partial_cmp(&target).expect("value expected")
                            }) {
                                Ok(i) | Err(i) => {
                                    if i > 0 {
                                        self.times_idx = i - 1;
                                    } else {
                                        self.at_end = true;
                                    }
                                }
                            }
                        } else {
                            let target = self.looped_interval.get_max();
                            match times.binary_search_by(|t| {
                                t.partial_cmp(&target).expect("value expected")
                            }) {
                                Ok(i) | Err(i) => {
                                    if i < times.len() {
                                        self.times_idx = i.saturating_sub(1);
                                    } else {
                                        self.at_end = true;
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                self.at_end = true;
            }
        } else if self.reversed {
            if self.times_idx == 0 || !self.interval.contains(times[self.times_idx]) {
                self.at_end = true;
            } else {
                self.times_idx -= 1;
                // Check if entering inner looping
                if self.section == KnotSection::PreInnerLooping
                    && times[self.times_idx] < self.looped_interval.get_max()
                {
                    self.section = KnotSection::InnerLooping;
                    self.loop_iter = Some(SegmentLoopIterator::new(
                        self.spline,
                        self.interval,
                        self.reversed,
                    ));
                }
            }
        } else {
            self.times_idx += 1;
            if self.times_idx >= times.len() - 1 || !self.interval.contains(times[self.times_idx]) {
                self.at_end = true;
            } else if self.section == KnotSection::PreInnerLooping
                && times[self.times_idx] >= self.looped_interval.get_min()
            {
                self.section = KnotSection::InnerLooping;
                self.loop_iter = Some(SegmentLoopIterator::new(
                    self.spline,
                    self.interval,
                    self.reversed,
                ));
            }
        }

        self.update_segment();
    }
}

// ============================================================================
// SegmentIterator
// ============================================================================

/// Main iterator for all spline segments.
///
/// Handles the entire spline including:
/// - Pre-extrapolation (before first knot)
/// - Knot-defined region (with inner looping)
/// - Post-extrapolation (after last knot)
/// - Extrapolation looping (repeat/oscillate)
pub struct SegmentIterator<'a> {
    spline: &'a Spline,
    interval: Interval,
    at_end: bool,
    knot_iter: Option<SegmentKnotIterator<'a>>,
    region: SplineSampleSource,
    current_segment: Segment,

    // Knot time boundaries (adjusted for inner looping)
    first_knot_time: TsTime,
    last_knot_time: TsTime,
    first_knot_pre_value: f64,
    first_knot_value: f64,
    last_knot_pre_value: f64,
    last_knot_value: f64,

    // Extrapolation loop state
    current_iteration: i32,
    min_iteration: i32,
    max_iteration: i32,
    pre_extrap_looped: bool,
    post_extrap_looped: bool,

    // For oscillating loops
    shift1: f64,
    shift2: f64,
    value_shift: f64,
    oscillating: bool,
    reversing: bool,
}

impl<'a> SegmentIterator<'a> {
    /// Creates a new segment iterator over the given interval.
    pub fn new(spline: &'a Spline, interval: Interval) -> Self {
        let mut iter = Self {
            spline,
            interval,
            at_end: true,
            knot_iter: None,
            region: SplineSampleSource::PreExtrap,
            current_segment: Segment::default(),
            first_knot_time: 0.0,
            last_knot_time: 0.0,
            first_knot_pre_value: 0.0,
            first_knot_value: 0.0,
            last_knot_pre_value: 0.0,
            last_knot_value: 0.0,
            current_iteration: 0,
            min_iteration: 0,
            max_iteration: 0,
            pre_extrap_looped: false,
            post_extrap_looped: false,
            shift1: 0.0,
            shift2: 0.0,
            value_shift: 0.0,
            oscillating: false,
            reversing: false,
        };
        iter.init();
        iter
    }

    fn init(&mut self) {
        if self.interval.is_empty() || self.spline.is_empty() {
            return;
        }

        let knots: Vec<_> = self.spline.knots().collect();
        if knots.is_empty() {
            return;
        }

        let times: Vec<TsTime> = knots.iter().map(|k| k.time()).collect();

        self.first_knot_time = times[0];
        self.last_knot_time = times[times.len() - 1];
        self.first_knot_pre_value = knots[0].value();
        self.first_knot_value = knots[0].value();
        self.last_knot_pre_value = knots[knots.len() - 1].value();
        self.last_knot_value = knots[knots.len() - 1].value();

        // Adjust for inner looping
        let loop_params = self.spline.inner_loop_params();
        if loop_params.is_enabled() {
            let looped_interval = get_open_looped_interval(loop_params);

            // Find first prototype knot
            let mut first_proto_idx = 0;
            for (i, &t) in times.iter().enumerate() {
                if t >= loop_params.proto_start {
                    first_proto_idx = i;
                    break;
                }
            }

            if looped_interval.get_min() <= self.first_knot_time {
                self.first_knot_time = looped_interval.get_min();
                self.first_knot_pre_value = knots[first_proto_idx].value()
                    - loop_params.value_offset * loop_params.num_pre_loops as f64;
                self.first_knot_value = knots[first_proto_idx].value()
                    - loop_params.value_offset * loop_params.num_pre_loops as f64;
            }

            if looped_interval.get_max() >= self.last_knot_time {
                self.last_knot_time = looped_interval.get_max();
                self.last_knot_pre_value = knots[first_proto_idx].value()
                    + loop_params.value_offset * (loop_params.num_post_loops as f64 + 1.0);
                self.last_knot_value = knots[first_proto_idx].value()
                    + loop_params.value_offset * (loop_params.num_post_loops as f64 + 1.0);
            }
        }

        // Check for extrapolation looping
        let has_knot_span = self.first_knot_time < self.last_knot_time;
        self.pre_extrap_looped = has_knot_span && self.spline.pre_extrapolation().mode.is_looping();
        self.post_extrap_looped =
            has_knot_span && self.spline.post_extrapolation().mode.is_looping();

        // Cannot iterate infinite looping over infinite interval
        if (self.pre_extrap_looped && !self.interval.is_min_finite())
            || (self.post_extrap_looped && !self.interval.is_max_finite())
        {
            // Error case
            return;
        }

        // Compute iteration bounds for extrapolation looping
        if self.pre_extrap_looped || self.post_extrap_looped {
            let knot_span = self.last_knot_time - self.first_knot_time;
            self.min_iteration =
                ((self.interval.get_min() - self.first_knot_time) / knot_span).floor() as i32;
            self.max_iteration =
                ((self.interval.get_max() - self.first_knot_time) / knot_span).floor() as i32;

            if !self.pre_extrap_looped {
                self.min_iteration = self.min_iteration.max(0);
                self.max_iteration = self.max_iteration.max(0);
            }
            if !self.post_extrap_looped {
                self.min_iteration = self.min_iteration.min(0);
                self.max_iteration = self.max_iteration.min(0);
            }
        }

        self.current_iteration = self.min_iteration;

        // Determine starting region
        if self.interval.get_min() < self.first_knot_time {
            self.region = if self.pre_extrap_looped {
                SplineSampleSource::PreExtrapLoop
            } else {
                SplineSampleSource::PreExtrap
            };
        } else if self.interval.get_min() < self.last_knot_time {
            self.region = SplineSampleSource::KnotInterp;
        } else {
            self.region = if self.post_extrap_looped {
                SplineSampleSource::PostExtrapLoop
            } else {
                SplineSampleSource::PostExtrap
            };
        }

        self.at_end = false;
        self.update_knot_iterator();
        self.update_segment();
    }

    fn update_knot_iterator(&mut self) {
        if self.region == SplineSampleSource::PreExtrap
            || self.region == SplineSampleSource::PostExtrap
        {
            return;
        }

        if self.current_iteration > self.max_iteration {
            self.knot_iter = None;
            return;
        }

        let knot_time_span = self.last_knot_time - self.first_knot_time;
        let time_delta = self.current_iteration as f64 * knot_time_span;

        // Determine if oscillating/reversing
        let pre_extrap = self.spline.pre_extrapolation();
        let post_extrap = self.spline.post_extrapolation();

        if self.current_iteration < 0 {
            self.oscillating = pre_extrap.mode == ExtrapMode::LoopOscillate;
            self.value_shift = if pre_extrap.mode == ExtrapMode::LoopRepeat {
                self.current_iteration as f64 * (self.last_knot_value - self.first_knot_value)
            } else {
                0.0
            };
        } else if self.current_iteration > 0 {
            self.oscillating = post_extrap.mode == ExtrapMode::LoopOscillate;
            self.value_shift = if post_extrap.mode == ExtrapMode::LoopRepeat {
                self.current_iteration as f64 * (self.last_knot_value - self.first_knot_value)
            } else {
                0.0
            };
        } else {
            self.oscillating = false;
            self.value_shift = 0.0;
        }

        self.reversing = self.oscillating && (self.current_iteration % 2 != 0);

        let knot_iter_interval;
        if self.reversing {
            self.shift1 = time_delta + self.first_knot_time;
            self.shift2 = self.last_knot_time;
            self.value_shift = 0.0; // Oscillating never has value offset

            let t0 = -(self.interval.get_max() - self.shift1) + self.shift2;
            let t1 = -(self.interval.get_min() - self.shift1) + self.shift2;
            knot_iter_interval = Interval::new(t0, t1, true, false);
        } else {
            self.shift1 = time_delta;
            self.shift2 = 0.0;
            knot_iter_interval = Interval::new(
                self.interval.get_min() - self.shift1,
                self.interval.get_max() - self.shift1,
                self.interval.is_min_closed(),
                self.interval.is_max_closed(),
            );
        }

        self.knot_iter = Some(SegmentKnotIterator::new(
            self.spline,
            knot_iter_interval,
            self.reversing,
        ));
        self.at_end = self.knot_iter.as_ref().is_none_or(|k| k.at_end());
    }

    fn update_segment(&mut self) {
        if self.at_end {
            self.current_segment = Segment::default();
            return;
        }

        match self.region {
            SplineSampleSource::PreExtrap => {
                self.update_pre_extrap_segment();
            }
            SplineSampleSource::PostExtrap => {
                self.update_post_extrap_segment();
            }
            _ => {
                if let Some(ref knot_iter) = self.knot_iter {
                    if knot_iter.at_end() {
                        self.at_end = true;
                        self.current_segment = Segment::default();
                    } else {
                        let mut seg = *knot_iter.segment();
                        if self.reversing {
                            seg = seg.transform_oscillate(self.shift1, self.shift2);
                        } else {
                            seg.offset(Vec2d::new(self.shift1, self.value_shift));
                        }
                        self.current_segment = seg;
                    }
                }
            }
        }
    }

    fn update_pre_extrap_segment(&mut self) {
        let extrap = self.spline.pre_extrapolation();
        let end_pt = Vec2d::new(self.first_knot_time, self.first_knot_pre_value);

        let mut slope = 0.0;
        let interp = if extrap.mode == ExtrapMode::ValueBlock {
            SegmentInterp::ValueBlock
        } else {
            SegmentInterp::PreExtrap
        };

        if extrap.mode == ExtrapMode::Sloped {
            slope = extrap.slope;
        }

        // For linear, we need to compute the slope from the first segment
        if extrap.mode == ExtrapMode::Linear
            && (self.first_knot_pre_value - self.first_knot_value).abs() < 1e-10
        {
            let tmp_knot_iter = SegmentKnotIterator::new(
                self.spline,
                Interval::new(self.first_knot_time, self.last_knot_time, true, false),
                false,
            );
            if !tmp_knot_iter.at_end() {
                slope = tmp_knot_iter.segment().compute_derivative(0.0);
            }
        }

        self.current_segment = Segment {
            p0: Vec2d::new(NEG_INF, slope),
            t0: Vec2d::zero(),
            t1: Vec2d::zero(),
            p1: end_pt,
            interp,
        };
    }

    fn update_post_extrap_segment(&mut self) {
        let extrap = self.spline.post_extrapolation();
        let start_pt = Vec2d::new(self.last_knot_time, self.last_knot_value);

        let mut slope = 0.0;
        let interp = if extrap.mode == ExtrapMode::ValueBlock {
            SegmentInterp::ValueBlock
        } else {
            SegmentInterp::PostExtrap
        };

        if extrap.mode == ExtrapMode::Sloped {
            slope = extrap.slope;
        }

        // For linear, compute slope from last segment
        if extrap.mode == ExtrapMode::Linear
            && (self.last_knot_pre_value - self.last_knot_value).abs() < 1e-10
        {
            let tmp_knot_iter = SegmentKnotIterator::new(
                self.spline,
                Interval::new(self.first_knot_time, self.last_knot_time, true, false),
                true,
            );
            if !tmp_knot_iter.at_end() {
                slope = tmp_knot_iter.segment().compute_derivative(1.0);
            }
        }

        self.current_segment = Segment {
            p0: start_pt,
            t0: Vec2d::zero(),
            t1: Vec2d::zero(),
            p1: Vec2d::new(INF, slope),
            interp,
        };
    }

    /// Returns true if at end of iteration.
    pub fn at_end(&self) -> bool {
        self.at_end
    }

    /// Returns the current segment.
    pub fn segment(&self) -> &Segment {
        &self.current_segment
    }

    /// Returns the current source region.
    pub fn source(&self) -> SplineSampleSource {
        self.region
    }

    /// Advances to the next segment.
    pub fn next(&mut self) {
        if self.at_end {
            return;
        }

        if self.region == SplineSampleSource::PostExtrap {
            self.at_end = true;
        } else {
            if self.region == SplineSampleSource::PreExtrap {
                // Advancing from pre-extrap to knot interp
                self.region = SplineSampleSource::KnotInterp;
                self.current_iteration = 0;
                self.update_knot_iterator();
            } else if let Some(ref mut knot_iter) = self.knot_iter {
                knot_iter.next();
            }

            if self.knot_iter.as_ref().is_none_or(|k| k.at_end()) {
                // End of current loop iteration
                self.current_iteration += 1;
                self.update_knot_iterator();
            }

            if self.knot_iter.as_ref().is_none_or(|k| k.at_end()) {
                // Done with all iterations - check for post-extrap
                if !self.post_extrap_looped && self.interval.get_max() > self.last_knot_time {
                    self.region = SplineSampleSource::PostExtrap;
                } else {
                    self.at_end = true;
                }
            }
        }

        self.update_segment();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InterpMode, Knot};

    #[test]
    fn test_empty_spline_iterator() {
        let spline = Spline::new();
        let interval = Interval::new(-10.0, 10.0, true, true);
        let iter = SegmentIterator::new(&spline, interval);
        assert!(iter.at_end());
    }

    #[test]
    fn test_single_knot_iterator() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 1.0));

        let interval = Interval::new(-1.0, 1.0, true, true);
        let iter = SegmentIterator::new(&spline, interval);

        // Single knot produces extrapolation segments
        assert!(!iter.at_end());
    }

    #[test]
    fn test_two_knots_linear() {
        let mut spline = Spline::new();

        let mut k0 = Knot::at_time(0.0, 0.0);
        k0.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k0);
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(0.0, 10.0, true, true);
        let iter = SegmentIterator::new(&spline, interval);

        assert!(!iter.at_end());
        let seg = iter.segment();
        assert_eq!(seg.interp, SegmentInterp::Linear);
        assert!((seg.start_time() - 0.0).abs() < 1e-10);
        assert!((seg.end_time() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_pre_extrapolation() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(-5.0, 5.0, true, true);
        let iter = SegmentIterator::new(&spline, interval);

        assert!(!iter.at_end());
        assert_eq!(iter.source(), SplineSampleSource::PreExtrap);
    }

    #[test]
    fn test_post_extrapolation() {
        let mut spline = Spline::new();
        spline.set_knot(Knot::at_time(0.0, 0.0));
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(15.0, 20.0, true, true);
        let iter = SegmentIterator::new(&spline, interval);

        assert!(!iter.at_end());
        assert_eq!(iter.source(), SplineSampleSource::PostExtrap);
    }

    #[test]
    fn test_segment_compute_derivative() {
        // Linear segment
        let seg = Segment::linear(0.0, 0.0, 10.0, 10.0);
        assert!((seg.compute_derivative(0.5) - 1.0).abs() < 1e-10);

        // Held segment
        let seg = Segment::held(0.0, 5.0, 10.0, 5.0);
        assert!((seg.compute_derivative(0.5) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_segment_transform_oscillate() {
        let seg = Segment::linear(2.0, 0.0, 8.0, 10.0);
        let transformed = seg.transform_oscillate(5.0, 10.0);

        // -(2 - 10) + 5 = 13, -(8 - 10) + 5 = 7
        assert!((transformed.p0[0] - 13.0).abs() < 1e-10);
        assert!((transformed.p1[0] - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_knot_section_transitions() {
        let mut spline = Spline::new();
        let mut k = Knot::at_time(0.0, 0.0);
        k.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k);
        spline.set_knot(Knot::at_time(5.0, 5.0));
        spline.set_knot(Knot::at_time(10.0, 10.0));

        let interval = Interval::new(0.0, 10.0, true, true);
        let mut iter = SegmentIterator::new(&spline, interval);

        let mut count = 0;
        while !iter.at_end() {
            count += 1;
            iter.next();
        }
        assert!(count >= 2); // At least 2 segments between 3 knots
    }

    #[test]
    fn test_multiple_iterations() {
        let mut spline = Spline::new();
        let mut k = Knot::at_time(0.0, 0.0);
        k.set_interp_mode(InterpMode::Linear);
        spline.set_knot(k);
        spline.set_knot(Knot::at_time(10.0, 10.0));
        spline.set_knot(Knot::at_time(20.0, 20.0));

        let interval = Interval::new(0.0, 20.0, true, true);
        let mut iter = SegmentIterator::new(&spline, interval);

        let mut segments = vec![];
        while !iter.at_end() {
            segments.push(iter.segment().clone());
            iter.next();
        }

        assert_eq!(segments.len(), 2);
    }
}
