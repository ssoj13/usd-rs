//! Spline regression prevention.
//!
//! Port of pxr/base/ts/regressionPreventer.h and regressionPreventer.cpp
//!
//! Provides utilities to prevent "regression" in splines - situations where
//! the curve goes backwards in time due to tangent configurations that
//! create loops or cusps.
//!
//! # Overview
//!
//! Beziers are parametric: the time function is x(t), where t is the parameter
//! value, and x is what we call time to avoid confusion with t.
//!
//! When the time function has two zero derivatives, there are two vertical
//! tangents in the segment, and the curve goes backward between them. When the
//! time function has a single zero derivative, there is one vertical tangent in
//! the segment, and the curve never goes backward. When the time function has
//! no zero derivatives, it is monotonically increasing, and the curve never goes
//! backward.
//!
//! We can detect regression by the presence of double verticals. We can also
//! minimally fix regression by shortening knot tangents, collapsing the double
//! vertical to a single vertical.
//!
//! # The Ellipse
//!
//! The zero-discriminant equation in L1/L2 space forms an ellipse with:
//! - A: L1 minimum at (0, 1)
//! - B: L2 maximum at (1/3, 4/3)
//! - C: L1/L2 balance at (1, 1)
//! - D: L1 maximum at (4/3, 1/3)
//! - E: L2 minimum at (1, 0)

use super::knot::Knot;
use super::knot_data::KnotData;
use super::segment::{Segment, SegmentInterp};
use super::spline::Spline;
use super::types::{AntiRegressionMode, CurveType, InterpMode, TsTime};
use usd_gf::Vec2d;

// ============================================================================
// Constants
// ============================================================================

/// Amount by which we over-fix. Each tangent will be made shorter than the
/// exact solution by this fraction of the unit interval. This ensures the curve
/// is definitely non-regressive.
const WRITE_PADDING: TsTime = 1e-5;

/// Amount by which we insist that the curve be over-fixed when deciding whether
/// there is regression. Smaller than WRITE_PADDING to ensure our output passes.
const READ_PADDING: TsTime = 1e-6;

/// Maximum tangent width when contained within interval.
const CONTAINED_MAX: TsTime = 1.0;

/// Maximum tangent width for single vertical (at 4/3).
const VERT_MAX: TsTime = 4.0 / 3.0;

/// Minimum tangent width for single vertical (at 1/3).
const VERT_MIN: TsTime = 1.0 / 3.0;

// ============================================================================
// Public Types
// ============================================================================

/// Interactive anti-regression modes.
///
/// These modes differentiate between 'active' (being edited) and
/// 'opposite' (neighbor) knots, favoring one over the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractiveMode {
    /// Shorten the active knot's tangents, leaving neighbors alone.
    LimitActive = 100,
    /// Shorten the opposite tangent first (up to 1/3), then cap active at 4/3.
    LimitOpposite = 101,
}

impl From<AntiRegressionMode> for InteractiveMode {
    fn from(mode: AntiRegressionMode) -> Self {
        match mode {
            AntiRegressionMode::KeepStart => InteractiveMode::LimitActive,
            _ => InteractiveMode::LimitOpposite,
        }
    }
}

/// Result details from a Set operation.
#[derive(Debug, Clone, Default)]
pub struct SetResult {
    /// Whether any adjustments were made.
    pub adjusted: bool,

    /// Pre-segment adjustments.
    /// Whether a pre-segment exists.
    pub have_pre_segment: bool,
    /// Whether the pre-segment active tangent was adjusted.
    pub pre_active_adjusted: bool,
    /// The adjusted width of the pre-segment active tangent.
    pub pre_active_adjusted_width: TsTime,
    /// Whether the pre-segment opposite tangent was adjusted.
    pub pre_opposite_adjusted: bool,
    /// The adjusted width of the pre-segment opposite tangent.
    pub pre_opposite_adjusted_width: TsTime,

    /// Post-segment adjustments.
    /// Whether a post-segment exists.
    pub have_post_segment: bool,
    /// Whether the post-segment active tangent was adjusted.
    pub post_active_adjusted: bool,
    /// The adjusted width of the post-segment active tangent.
    pub post_active_adjusted_width: TsTime,
    /// Whether the post-segment opposite tangent was adjusted.
    pub post_opposite_adjusted: bool,
    /// The adjusted width of the post-segment opposite tangent.
    pub post_opposite_adjusted_width: TsTime,
}

impl SetResult {
    /// Returns a debug description of the result.
    pub fn debug_description(&self, precision: usize) -> String {
        let mut s = String::new();

        s.push_str("TsRegressionPreventer::SetResult:\n");
        s.push_str(&format!("  adjusted: {}\n", self.adjusted));
        s.push_str(&format!("  havePreSegment: {}\n", self.have_pre_segment));
        s.push_str(&format!(
            "  preActiveAdjusted: {}\n",
            self.pre_active_adjusted
        ));
        s.push_str(&format!(
            "  preActiveAdjustedWidth: {:.prec$}\n",
            self.pre_active_adjusted_width,
            prec = precision
        ));
        s.push_str(&format!(
            "  preOppositeAdjusted: {}\n",
            self.pre_opposite_adjusted
        ));
        s.push_str(&format!(
            "  preOppositeAdjustedWidth: {:.prec$}\n",
            self.pre_opposite_adjusted_width,
            prec = precision
        ));
        s.push_str(&format!("  havePostSegment: {}\n", self.have_post_segment));
        s.push_str(&format!(
            "  postActiveAdjusted: {}\n",
            self.post_active_adjusted
        ));
        s.push_str(&format!(
            "  postActiveAdjustedWidth: {:.prec$}\n",
            self.post_active_adjusted_width,
            prec = precision
        ));
        s.push_str(&format!(
            "  postOppositeAdjusted: {}\n",
            self.post_opposite_adjusted
        ));
        s.push_str(&format!(
            "  postOppositeAdjustedWidth: {:.prec$}\n",
            self.post_opposite_adjusted_width,
            prec = precision
        ));

        s
    }
}

// ============================================================================
// Internal Mode Enum
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    None,
    Contain,
    KeepRatio,
    KeepStart,
    LimitActive,
    LimitOpposite,
}

impl From<AntiRegressionMode> for Mode {
    fn from(mode: AntiRegressionMode) -> Self {
        match mode {
            AntiRegressionMode::None => Mode::None,
            AntiRegressionMode::Contain => Mode::Contain,
            AntiRegressionMode::KeepRatio => Mode::KeepRatio,
            AntiRegressionMode::KeepStart => Mode::KeepStart,
        }
    }
}

impl From<InteractiveMode> for Mode {
    fn from(mode: InteractiveMode) -> Self {
        match mode {
            InteractiveMode::LimitActive => Mode::LimitActive,
            InteractiveMode::LimitOpposite => Mode::LimitOpposite,
        }
    }
}

// ============================================================================
// Math Helpers
// ============================================================================

/// Checks if tangent widths are regressive.
///
/// Uses the ellipse equation to determine if the (width1, width2) point
/// lies outside the non-regressive region.
///
/// The ellipse equation is: L1^2 + L2^2 + L1*L2 - 2*L1 - 2*L2 + 1 = 0
fn are_tan_widths_regressive(width1: TsTime, width2: TsTime) -> bool {
    // If contained, then not regressive. There are non-regressive (w1, w2)
    // points outside the ellipse but inside the contained square.
    if width1 <= CONTAINED_MAX && width2 <= CONTAINED_MAX {
        return false;
    }

    // Consider both widths with padding.
    let w1 = width1 + READ_PADDING;
    let w2 = width2 + READ_PADDING;

    // Determine whether (w1, w2) lies outside the ellipse.
    (w1 * w1) + (w2 * w2) - 2.0 * (w1 + w2) + (w1 * w2) + 1.0 > 0.0
}

/// Computes the other width for a single vertical.
///
/// Given one tangent width, finds the corresponding width that creates
/// a single vertical on the ellipse. Uses the quadratic formula.
///
/// The ellipse equation in power form with L1 as constant:
///   L2^2 + (L1 - 2) L2 + (L1 - 1)^2 = 0
fn compute_other_width_for_vert(width: TsTime, hint: TsTime) -> TsTime {
    // Clamp to longest given width / shortest other width.
    if width > VERT_MAX {
        return VERT_MIN;
    }

    // Solve for the two ellipse points that have the given width.
    let b = width - 2.0;
    let c = (width - 1.0).powi(2);
    let discriminant = b * b - 4.0 * c;

    // Should always be non-negative for valid inputs
    if discriminant < 0.0 {
        return VERT_MIN;
    }

    let root_base = -b / 2.0;
    let root_offset = discriminant.sqrt() / 2.0;

    // Choose the solution closer to the hint.
    if hint > root_base {
        root_base + root_offset
    } else {
        root_base - root_offset
    }
}

// ============================================================================
// Knot Params - Simplified working data
// ============================================================================

/// Working parameters for a knot during regression prevention.
#[derive(Debug, Clone)]
struct KnotParams {
    time: TsTime,
    pre_tan_width: TsTime,
    post_tan_width: TsTime,
}

impl KnotParams {
    fn from_knot(knot: &Knot) -> Self {
        Self {
            time: knot.time(),
            pre_tan_width: knot.pre_tangent().width,
            post_tan_width: knot.post_tangent().width,
        }
    }

    fn from_knot_data(data: &KnotData) -> Self {
        Self {
            time: data.time,
            pre_tan_width: data.pre_tan_width,
            post_tan_width: data.post_tan_width,
        }
    }

    fn set_pre_tan_width(&mut self, width: TsTime) {
        self.pre_tan_width = width;
    }

    fn set_post_tan_width(&mut self, width: TsTime) {
        self.post_tan_width = width;
    }
}

// ============================================================================
// Knot State
// ============================================================================

/// Tracks original and current state of a knot.
struct KnotState {
    original_knot: Knot,
    current_params: KnotParams,
}

impl KnotState {
    fn new(knot: Knot) -> Self {
        let params = KnotParams::from_knot(&knot);
        Self {
            original_knot: knot,
            current_params: params,
        }
    }
}

/// Working state during regression adjustment.
struct WorkingKnotState {
    proposed_knot: Option<Knot>,
    proposed_params: KnotParams,
    working_params: KnotParams,
}

impl WorkingKnotState {
    fn from_proposed(proposed: Knot) -> Self {
        let params = KnotParams::from_knot(&proposed);
        Self {
            proposed_knot: Some(proposed),
            proposed_params: params.clone(),
            working_params: params,
        }
    }

    fn from_original(state: &KnotState) -> Self {
        let params = KnotParams::from_knot(&state.original_knot);
        Self {
            proposed_knot: Some(state.original_knot.clone()),
            proposed_params: params.clone(),
            working_params: params,
        }
    }

    fn from_knot_data(data: &KnotData) -> Self {
        let params = KnotParams::from_knot_data(data);
        Self {
            proposed_knot: None,
            proposed_params: params.clone(),
            working_params: params,
        }
    }

    /// Returns the proposed knot with working params applied.
    fn to_working_knot(&self) -> Option<Knot> {
        let mut knot = self.proposed_knot.clone()?;
        let mut pre_tan = *knot.pre_tangent();
        let mut post_tan = *knot.post_tangent();
        pre_tan.width = self.working_params.pre_tan_width;
        post_tan.width = self.working_params.post_tan_width;
        knot.set_pre_tangent(pre_tan);
        knot.set_post_tangent(post_tan);
        Some(knot)
    }

    /// Returns the proposed knot unchanged.
    fn get_proposed_knot(&self) -> Option<&Knot> {
        self.proposed_knot.as_ref()
    }
}

// ============================================================================
// Segment Solver
// ============================================================================

/// Which segment relative to the active knot.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WhichSegment {
    Pre,
    Post,
}

/// Solver for a single segment's regression prevention.
struct SegmentSolver<'a> {
    which_segment: WhichSegment,
    mode: Mode,
    segment: Option<&'a mut Segment>,
    active_state: &'a mut WorkingKnotState,
    opposite_state: Option<&'a mut WorkingKnotState>,
    segment_width: TsTime,
    result: Option<&'a mut SetResult>,
}

impl<'a> SegmentSolver<'a> {
    fn new_with_knots(
        which_segment: WhichSegment,
        mode: Mode,
        active_state: &'a mut WorkingKnotState,
        opposite_state: &'a mut WorkingKnotState,
        segment_width: TsTime,
        result: Option<&'a mut SetResult>,
    ) -> Self {
        Self {
            which_segment,
            mode,
            segment: None,
            active_state,
            opposite_state: Some(opposite_state),
            segment_width,
            result,
        }
    }

    fn new_with_segment(
        mode: Mode,
        segment: &'a mut Segment,
        active_state: &'a mut WorkingKnotState,
        result: Option<&'a mut SetResult>,
    ) -> Self {
        let segment_width = segment.time_span();
        Self {
            which_segment: WhichSegment::Post,
            mode,
            segment: Some(segment),
            active_state,
            opposite_state: None,
            segment_width,
            result,
        }
    }

    /// Main adjustment entry point.
    fn adjust(&mut self) -> bool {
        // Contain mode adjusts tangents even when non-regressive.
        if self.mode == Mode::Contain {
            return self.adjust_with_contain();
        }

        // If no regression, nothing to do.
        if !are_tan_widths_regressive(
            self.get_proposed_active_width(),
            self.get_proposed_opposite_width(),
        ) {
            return true;
        }

        // Other modes.
        match self.mode {
            Mode::KeepRatio => self.adjust_with_keep_ratio(),
            Mode::KeepStart => self.adjust_with_keep_start(),
            Mode::LimitActive => self.adjust_with_limit_active(),
            Mode::LimitOpposite => self.adjust_with_limit_opposite(),
            _ => false,
        }
    }

    /// Contain mode: clamp both tangents to interval.
    fn adjust_with_contain(&mut self) -> bool {
        // Don't use write padding for Contain. We want the maximum to exactly
        // equal the interval.
        if self.get_proposed_active_width() > CONTAINED_MAX {
            self.set_active_width(CONTAINED_MAX);
        }

        if self.get_proposed_opposite_width() > CONTAINED_MAX {
            self.set_opposite_width(CONTAINED_MAX);
        }

        true
    }

    /// KeepRatio mode: maintain ratio between tangents.
    fn adjust_with_keep_ratio(&mut self) -> bool {
        let active_width = self.get_proposed_active_width();
        let opposite_width = self.get_proposed_opposite_width();

        if active_width < READ_PADDING {
            // Zero active width. Clamp opposite to 1.
            self.set_opposite_width(CONTAINED_MAX - WRITE_PADDING);
        } else if opposite_width < READ_PADDING {
            // Zero opposite width. Clamp active to 1.
            self.set_active_width(CONTAINED_MAX - WRITE_PADDING);
        } else {
            // Find ratio of proposed active to opposite width.
            let ratio = active_width / opposite_width;

            // Solve for line / ellipse intersection.
            // L1 = (sqrt(k) + k + 1) / (k^2 + k + 1)
            let adjusted_opposite = (ratio.sqrt() + ratio + 1.0) / (ratio * ratio + ratio + 1.0);
            self.set_active_width(ratio * adjusted_opposite - WRITE_PADDING);
            self.set_opposite_width(adjusted_opposite - WRITE_PADDING);
        }

        true
    }

    /// KeepStart mode: favor the start tangent.
    fn adjust_with_keep_start(&mut self) -> bool {
        let start_width = self.get_proposed_start_width();

        if start_width >= VERT_MAX {
            // Clamp to longest start width.
            self.set_start_width(VERT_MAX - WRITE_PADDING);
            self.set_end_width(VERT_MIN - WRITE_PADDING);
        } else {
            // Keep start width; solve for end width.
            let adjusted_width =
                compute_other_width_for_vert(start_width, self.get_proposed_end_width());
            self.set_end_width(adjusted_width - WRITE_PADDING);
        }

        true
    }

    /// LimitActive mode: only adjust the active tangent.
    fn adjust_with_limit_active(&mut self) -> bool {
        let opposite_width = self.get_proposed_opposite_width();

        if opposite_width >= VERT_MAX {
            // Clamp to longest opposite width.
            self.set_opposite_width(VERT_MAX - WRITE_PADDING);
            self.set_active_width((VERT_MIN - WRITE_PADDING).min(self.get_proposed_active_width()));
        } else {
            // Keep opposite width; solve for active width.
            let adjusted_width =
                compute_other_width_for_vert(opposite_width, self.get_proposed_active_width());
            self.set_active_width(adjusted_width - WRITE_PADDING);
        }

        true
    }

    /// LimitOpposite mode: adjust opposite first, then active.
    fn adjust_with_limit_opposite(&mut self) -> bool {
        let active_width = self.get_proposed_active_width();
        let opposite_width = self.get_proposed_opposite_width();

        if opposite_width <= VERT_MIN {
            // Non-regressive limit will be in fringe.
            // Don't adjust opposite; just clamp active.
            let adjusted_width = compute_other_width_for_vert(opposite_width, active_width);
            self.set_active_width(adjusted_width - WRITE_PADDING);
        } else if active_width >= VERT_MAX {
            // Clamp to longest active width.
            self.set_active_width(VERT_MAX - WRITE_PADDING);
            self.set_opposite_width(VERT_MIN - WRITE_PADDING);
        } else {
            // Keep active width; solve for opposite width.
            let adjusted_width = compute_other_width_for_vert(active_width, opposite_width);
            self.set_opposite_width(adjusted_width - WRITE_PADDING);
        }

        true
    }

    // Width accessors

    fn get_proposed_active_width(&self) -> TsTime {
        let raw_width = if let Some(seg) = &self.segment {
            seg.t0[0] - seg.p0[0]
        } else {
            match self.which_segment {
                WhichSegment::Pre => self.active_state.proposed_params.pre_tan_width,
                WhichSegment::Post => self.active_state.proposed_params.post_tan_width,
            }
        };
        raw_width / self.segment_width
    }

    fn get_proposed_opposite_width(&self) -> TsTime {
        let raw_width = if let Some(seg) = &self.segment {
            seg.p1[0] - seg.t1[0]
        } else if let Some(opposite) = &self.opposite_state {
            match self.which_segment {
                WhichSegment::Pre => opposite.proposed_params.post_tan_width,
                WhichSegment::Post => opposite.proposed_params.pre_tan_width,
            }
        } else {
            0.0
        };
        raw_width / self.segment_width
    }

    fn get_proposed_start_width(&self) -> TsTime {
        match self.which_segment {
            WhichSegment::Pre => self.get_proposed_opposite_width(),
            WhichSegment::Post => self.get_proposed_active_width(),
        }
    }

    fn get_proposed_end_width(&self) -> TsTime {
        match self.which_segment {
            WhichSegment::Pre => self.get_proposed_active_width(),
            WhichSegment::Post => self.get_proposed_opposite_width(),
        }
    }

    fn set_active_width(&mut self, width: TsTime) {
        let adjusted = (width - self.get_proposed_active_width()).abs() > 1e-12;
        let raw_width = width * self.segment_width;

        if let Some(seg) = &mut self.segment {
            // Scale the tangent to achieve the new width
            let tangent = [seg.t0[0] - seg.p0[0], seg.t0[1] - seg.p0[1]];
            let scale = if tangent[0].abs() < 1e-12 {
                1.0
            } else {
                raw_width / tangent[0]
            };
            seg.t0 = Vec2d::new(
                seg.p0[0] + tangent[0] * scale,
                seg.p0[1] + tangent[1] * scale,
            );

            if let Some(result) = &mut self.result {
                result.adjusted |= adjusted;
                result.post_active_adjusted |= adjusted;
                result.post_active_adjusted_width = raw_width;
            }
        } else {
            match self.which_segment {
                WhichSegment::Pre => {
                    self.active_state
                        .working_params
                        .set_pre_tan_width(raw_width);
                    if let Some(result) = &mut self.result {
                        result.adjusted |= adjusted;
                        result.pre_active_adjusted |= adjusted;
                        result.pre_active_adjusted_width = raw_width;
                    }
                }
                WhichSegment::Post => {
                    self.active_state
                        .working_params
                        .set_post_tan_width(raw_width);
                    if let Some(result) = &mut self.result {
                        result.adjusted |= adjusted;
                        result.post_active_adjusted |= adjusted;
                        result.post_active_adjusted_width = raw_width;
                    }
                }
            }
        }
    }

    fn set_opposite_width(&mut self, width: TsTime) {
        let adjusted = (width - self.get_proposed_opposite_width()).abs() > 1e-12;
        let raw_width = width * self.segment_width;

        if let Some(seg) = &mut self.segment {
            // Scale the tangent (note: negative direction)
            let tangent = [seg.t1[0] - seg.p1[0], seg.t1[1] - seg.p1[1]];
            let scale = if tangent[0].abs() < 1e-12 {
                1.0
            } else {
                -raw_width / tangent[0]
            };
            seg.t1 = Vec2d::new(
                seg.p1[0] + tangent[0] * scale,
                seg.p1[1] + tangent[1] * scale,
            );

            if let Some(result) = &mut self.result {
                result.adjusted |= adjusted;
                result.post_opposite_adjusted |= adjusted;
                result.post_opposite_adjusted_width = raw_width;
            }
        } else if let Some(opposite) = &mut self.opposite_state {
            match self.which_segment {
                WhichSegment::Pre => {
                    opposite.working_params.set_post_tan_width(raw_width);
                    if let Some(result) = &mut self.result {
                        result.adjusted |= adjusted;
                        result.pre_opposite_adjusted |= adjusted;
                        result.pre_opposite_adjusted_width = raw_width;
                    }
                }
                WhichSegment::Post => {
                    opposite.working_params.set_pre_tan_width(raw_width);
                    if let Some(result) = &mut self.result {
                        result.adjusted |= adjusted;
                        result.post_opposite_adjusted |= adjusted;
                        result.post_opposite_adjusted_width = raw_width;
                    }
                }
            }
        }
    }

    fn set_start_width(&mut self, width: TsTime) {
        match self.which_segment {
            WhichSegment::Pre => self.set_opposite_width(width),
            WhichSegment::Post => self.set_active_width(width),
        }
    }

    fn set_end_width(&mut self, width: TsTime) {
        match self.which_segment {
            WhichSegment::Pre => self.set_active_width(width),
            WhichSegment::Post => self.set_opposite_width(width),
        }
    }
}

// ============================================================================
// RegressionPreventer
// ============================================================================

/// Authoring helper that enforces non-regression in splines.
///
/// Construct when a knot is being interactively edited.
/// Call `set` for each change to apply anti-regression.
pub struct RegressionPreventer<'a> {
    spline: &'a mut Spline,
    active_knot_time: TsTime,
    mode: Mode,
    limit: bool,
    valid: bool,
    initial_adjustment_done: bool,
    active_knot_state: Option<KnotState>,
    pre_knot_state: Option<KnotState>,
    post_knot_state: Option<KnotState>,
    overwritten_knot_state: Option<KnotState>,
}

impl<'a> RegressionPreventer<'a> {
    /// Creates a new RegressionPreventer with the default authoring mode.
    ///
    /// The mode is determined by the current authoring mode (defaults to Contain).
    /// If `limit` is true, adjustments are enforced.
    pub fn new(spline: &'a mut Spline, active_knot_time: TsTime, limit: bool) -> Self {
        Self::with_internal_mode(spline, active_knot_time, Mode::Contain, limit)
    }

    /// Creates with a specific anti-regression mode.
    pub fn with_anti_regression_mode(
        spline: &'a mut Spline,
        active_knot_time: TsTime,
        mode: AntiRegressionMode,
        limit: bool,
    ) -> Self {
        Self::with_internal_mode(spline, active_knot_time, mode.into(), limit)
    }

    /// Creates with a specific interactive mode.
    pub fn with_mode(
        spline: &'a mut Spline,
        active_knot_time: TsTime,
        mode: InteractiveMode,
        limit: bool,
    ) -> Self {
        Self::with_internal_mode(spline, active_knot_time, mode.into(), limit)
    }

    fn with_internal_mode(
        spline: &'a mut Spline,
        active_knot_time: TsTime,
        mode: Mode,
        limit: bool,
    ) -> Self {
        // Check for Bezier curve type
        if spline.curve_type() != CurveType::Bezier {
            return Self {
                spline,
                active_knot_time,
                mode,
                limit,
                valid: false,
                initial_adjustment_done: false,
                active_knot_state: None,
                pre_knot_state: None,
                post_knot_state: None,
                overwritten_knot_state: None,
            };
        }

        // Find the active knot
        let active_knot = spline.get_knot(active_knot_time).cloned();

        if active_knot.is_none() {
            return Self {
                spline,
                active_knot_time,
                mode,
                limit,
                valid: false,
                initial_adjustment_done: false,
                active_knot_state: None,
                pre_knot_state: None,
                post_knot_state: None,
                overwritten_knot_state: None,
            };
        }

        let active_knot = active_knot.expect("value expected");

        // Check for inner loops - cannot edit echoed knots
        if spline.has_inner_loops() {
            let lp = spline.inner_loop_params();
            let looped_interval = lp.looped_interval();
            let proto_interval = lp.prototype_interval();

            if looped_interval.contains(active_knot_time)
                && !proto_interval.contains(active_knot_time)
            {
                return Self {
                    spline,
                    active_knot_time,
                    mode,
                    limit,
                    valid: false,
                    initial_adjustment_done: false,
                    active_knot_state: None,
                    pre_knot_state: None,
                    post_knot_state: None,
                    overwritten_knot_state: None,
                };
            }
        }

        // Find neighbor knots
        let knots: Vec<_> = spline.knots().cloned().collect();
        let active_idx = knots
            .iter()
            .position(|k| (k.time() - active_knot_time).abs() < 1e-10);

        let active_idx = match active_idx {
            Some(idx) => idx,
            None => {
                return Self {
                    spline,
                    active_knot_time,
                    mode,
                    limit,
                    valid: false,
                    initial_adjustment_done: false,
                    active_knot_state: None,
                    pre_knot_state: None,
                    post_knot_state: None,
                    overwritten_knot_state: None,
                };
            }
        };

        let active_knot_state = Some(KnotState::new(active_knot.clone()));

        // Pre-neighbor: check if previous knot's next interpolation is curve
        let pre_knot_state = if active_idx > 0 {
            let pre_knot = &knots[active_idx - 1];
            if pre_knot.interp_mode() == InterpMode::Curve {
                Some(KnotState::new(pre_knot.clone()))
            } else {
                None
            }
        } else {
            None
        };

        // Post-neighbor: check if active knot's next interpolation is curve
        let post_knot_state = if active_idx + 1 < knots.len() {
            if active_knot.interp_mode() == InterpMode::Curve {
                Some(KnotState::new(knots[active_idx + 1].clone()))
            } else {
                None
            }
        } else {
            None
        };

        Self {
            spline,
            active_knot_time,
            mode,
            limit,
            valid: true,
            initial_adjustment_done: false,
            active_knot_state,
            pre_knot_state,
            post_knot_state,
            overwritten_knot_state: None,
        }
    }

    /// Returns whether the preventer is valid.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Sets an edited version of the active knot.
    ///
    /// Adjusts tangent widths if needed based on the mode.
    /// Returns true on success.
    pub fn set(&mut self, proposed: &Knot, result: &mut SetResult) -> bool {
        // Initialize result
        self.init_set_result(proposed, result);

        if !self.valid {
            return false;
        }

        // If anti-regression is disabled, just write the knot as proposed
        if self.mode == Mode::None {
            self.spline.set_knot(proposed.clone());
            return true;
        }

        // Perform initial anti-regression if needed
        self.handle_initial_adjustment(proposed, result);

        // If the active knot's time has changed, update state
        self.handle_time_change(proposed.time());

        // Solve the segments
        self.do_set(proposed, self.mode, result);
        true
    }

    fn init_set_result(&self, proposed: &Knot, result: &mut SetResult) {
        result.have_pre_segment = self.pre_knot_state.is_some();
        result.have_post_segment = self.post_knot_state.is_some();

        result.pre_active_adjusted_width = proposed.pre_tangent().width;
        result.post_active_adjusted_width = proposed.post_tangent().width;

        if let Some(ref pre) = self.pre_knot_state {
            result.pre_opposite_adjusted_width = pre.original_knot.post_tangent().width;
        }

        if let Some(ref post) = self.post_knot_state {
            result.post_opposite_adjusted_width = post.original_knot.pre_tangent().width;
        }
    }

    fn handle_initial_adjustment(&mut self, _proposed: &Knot, result: &mut SetResult) {
        if self.initial_adjustment_done {
            return;
        }

        self.initial_adjustment_done = true;

        // Perform a no-op change using Contain or Limit Opposite to fix any
        // initial regression.
        if let Some(ref active_state) = self.active_knot_state {
            let initial_mode = if self.mode == Mode::Contain {
                Mode::Contain
            } else {
                Mode::LimitOpposite
            };

            let original = active_state.original_knot.clone();
            self.do_set(&original, initial_mode, result);
        }

        // Note: In C++, they latch edits to track as original.
        // We simplify by keeping the original states as-is.
    }

    fn handle_time_change(&mut self, proposed_time: TsTime) {
        let current_time = if let Some(ref state) = self.active_knot_state {
            state.current_params.time
        } else {
            return;
        };

        if (proposed_time - current_time).abs() < 1e-10 {
            return;
        }

        // Time has changed - need to update neighbor tracking
        // Check if we haven't crossed any neighbors and no overwrite
        let crossed_pre = self
            .pre_knot_state
            .as_ref()
            .map(|s| proposed_time <= s.original_knot.time())
            .unwrap_or(false);
        let crossed_post = self
            .post_knot_state
            .as_ref()
            .map(|s| proposed_time >= s.original_knot.time())
            .unwrap_or(false);

        if self.overwritten_knot_state.is_none() && !crossed_pre && !crossed_post {
            self.active_knot_time = proposed_time;
            return;
        }

        // Restore overwritten knot if any
        if let Some(ref state) = self.overwritten_knot_state {
            self.spline.set_knot(state.original_knot.clone());
        }
        self.overwritten_knot_state = None;

        // Restore original neighbors
        if let Some(ref state) = self.pre_knot_state {
            self.spline.set_knot(state.original_knot.clone());
        }
        self.pre_knot_state = None;

        if let Some(ref state) = self.post_knot_state {
            self.spline.set_knot(state.original_knot.clone());
        }
        self.post_knot_state = None;

        // Find knots around the new proposed time
        let knots: Vec<_> = self.spline.knots().cloned().collect();
        let lb_idx = knots.iter().position(|k| k.time() >= proposed_time);

        // Check if we're overwriting an existing knot at this time
        if let Some(idx) = lb_idx {
            if (knots[idx].time() - proposed_time).abs() < 1e-10 {
                // Store state of knot being overwritten
                self.overwritten_knot_state = Some(KnotState::new(knots[idx].clone()));
            }
        }

        // Set up pre-knot state
        let pre_idx = lb_idx.and_then(|idx| if idx > 0 { Some(idx - 1) } else { None });
        if let Some(idx) = pre_idx {
            if knots[idx].time() < proposed_time {
                self.pre_knot_state = Some(KnotState::new(knots[idx].clone()));
            }
        }

        // Set up post-knot state (accounting for overwritten knot offset)
        let post_offset = if self.overwritten_knot_state.is_some() {
            1
        } else {
            0
        };
        let post_idx = lb_idx.map(|idx| idx + post_offset);
        if let Some(idx) = post_idx {
            if idx < knots.len() && knots[idx].time() > proposed_time {
                self.post_knot_state = Some(KnotState::new(knots[idx].clone()));
            }
        }

        self.active_knot_time = proposed_time;
    }

    fn do_set(&mut self, proposed: &Knot, mode: Mode, result: &mut SetResult) {
        // Create working states
        let mut active_working = WorkingKnotState::from_proposed(proposed.clone());

        // Adjust pre-segment if it exists
        if let Some(ref pre_state) = self.pre_knot_state {
            let mut pre_working = WorkingKnotState::from_original(pre_state);
            let segment_width = proposed.time() - pre_state.original_knot.time();

            if segment_width > 0.0 {
                let mut solver = SegmentSolver::new_with_knots(
                    WhichSegment::Pre,
                    mode,
                    &mut active_working,
                    &mut pre_working,
                    segment_width,
                    Some(result),
                );
                solver.adjust();

                // Apply pre-knot adjustments if limiting
                if self.limit && result.pre_opposite_adjusted {
                    let mut knot = pre_state.original_knot.clone();
                    let mut tan = *knot.post_tangent();
                    tan.width = pre_working.working_params.post_tan_width;
                    knot.set_post_tangent(tan);
                    self.spline.set_knot(knot);
                }
            }
        }

        // Adjust post-segment if it exists
        if let Some(ref post_state) = self.post_knot_state {
            let mut post_working = WorkingKnotState::from_original(post_state);
            let segment_width = post_state.original_knot.time() - proposed.time();

            if segment_width > 0.0 {
                let mut solver = SegmentSolver::new_with_knots(
                    WhichSegment::Post,
                    mode,
                    &mut active_working,
                    &mut post_working,
                    segment_width,
                    Some(result),
                );
                solver.adjust();

                // Apply post-knot adjustments if limiting
                if self.limit && result.post_opposite_adjusted {
                    let mut knot = post_state.original_knot.clone();
                    let mut tan = *knot.pre_tangent();
                    tan.width = post_working.working_params.pre_tan_width;
                    knot.set_pre_tangent(tan);
                    self.spline.set_knot(knot);
                }
            }
        }

        // Write the active knot
        if self.limit {
            // Write adjusted knot using working params
            if let Some(knot) = active_working.to_working_knot() {
                self.spline.set_knot(knot);
            }
        } else {
            // Write proposed knot as-is
            if let Some(knot) = active_working.get_proposed_knot() {
                self.spline.set_knot(knot.clone());
            }
        }
    }
}

// ============================================================================
// Batch Processing
// ============================================================================

/// Batch access for segment regression checking.
pub struct RegressionPreventerBatch;

impl RegressionPreventerBatch {
    /// Checks if a segment is regressive (using Segment).
    pub fn is_segment_regressive_seg(segment: &Segment, mode: AntiRegressionMode) -> bool {
        if mode == AntiRegressionMode::None {
            return false;
        }

        // Only check Bezier segments
        if segment.interp != SegmentInterp::Bezier {
            return false;
        }

        let interval = segment.p1[0] - segment.p0[0];
        if interval <= 0.0 {
            return false;
        }

        let start_width = (segment.t0[0] - segment.p0[0]) / interval;
        let end_width = (segment.p1[0] - segment.t1[0]) / interval;

        // In Contain mode, check simple max
        if mode == AntiRegressionMode::Contain {
            return start_width > CONTAINED_MAX || end_width > CONTAINED_MAX;
        }

        // Call math helper
        are_tan_widths_regressive(start_width, end_width)
    }

    /// Processes a segment to remove regression.
    ///
    /// Returns true if adjustments were made.
    pub fn process_segment(
        start: &mut KnotData,
        end: &mut KnotData,
        mode: AntiRegressionMode,
    ) -> bool {
        // If anti-regression is disabled, nothing to do
        if mode == AntiRegressionMode::None {
            return false;
        }

        // Only process Bezier segments
        if start.next_interp != InterpMode::Curve {
            return false;
        }

        // Create working states
        let mut start_working = WorkingKnotState::from_knot_data(start);
        let mut end_working = WorkingKnotState::from_knot_data(end);

        let segment_width = end.time - start.time;
        if segment_width <= 0.0 {
            return false;
        }

        let mut result = SetResult::default();
        let internal_mode: Mode = mode.into();

        // Create solver and adjust
        {
            let mut solver = SegmentSolver::new_with_knots(
                WhichSegment::Post,
                internal_mode,
                &mut start_working,
                &mut end_working,
                segment_width,
                Some(&mut result),
            );
            solver.adjust();
        }

        // Write adjusted widths back
        if result.post_active_adjusted {
            start.post_tan_width = start_working.working_params.post_tan_width;
        }
        if result.post_opposite_adjusted {
            end.pre_tan_width = end_working.working_params.pre_tan_width;
        }

        result.adjusted
    }

    /// Checks if a segment is regressive using raw widths.
    ///
    /// Takes absolute tangent widths and segment interval.
    pub fn is_segment_regressive(
        post_width: TsTime,
        pre_width: TsTime,
        segment_width: TsTime,
        mode: AntiRegressionMode,
    ) -> bool {
        if mode == AntiRegressionMode::None {
            return false;
        }

        if segment_width <= 0.0 {
            return false;
        }

        let start_norm = post_width / segment_width;
        let end_norm = pre_width / segment_width;

        // In Contain mode, check simple max
        if mode == AntiRegressionMode::Contain {
            return start_norm > CONTAINED_MAX || end_norm > CONTAINED_MAX;
        }

        // Call math helper
        are_tan_widths_regressive(start_norm, end_norm)
    }

    /// Adjusts widths to remove regression.
    ///
    /// Returns adjusted (post_width, pre_width).
    pub fn adjust_widths(
        post_width: TsTime,
        pre_width: TsTime,
        segment_width: TsTime,
        mode: AntiRegressionMode,
    ) -> (TsTime, TsTime) {
        if mode == AntiRegressionMode::None || segment_width <= 0.0 {
            return (post_width, pre_width);
        }

        let start_norm = post_width / segment_width;
        let end_norm = pre_width / segment_width;

        // Check if regressive
        let is_regressive = if mode == AntiRegressionMode::Contain {
            start_norm > CONTAINED_MAX || end_norm > CONTAINED_MAX
        } else {
            are_tan_widths_regressive(start_norm, end_norm)
        };

        if !is_regressive {
            return (post_width, pre_width);
        }

        // Adjust based on mode
        match mode {
            AntiRegressionMode::Contain => {
                let new_start = start_norm.min(CONTAINED_MAX);
                let new_end = end_norm.min(CONTAINED_MAX);
                (new_start * segment_width, new_end * segment_width)
            }
            AntiRegressionMode::KeepRatio => {
                // Scale both proportionally
                let total = start_norm + end_norm;
                if total > 0.0 {
                    let scale = 1.0 / total.max(1.0);
                    (post_width * scale, pre_width * scale)
                } else {
                    (post_width, pre_width)
                }
            }
            AntiRegressionMode::KeepStart => {
                // Keep start, adjust end
                let new_end = compute_other_width_for_vert(start_norm, end_norm);
                (post_width, (new_end * segment_width).min(pre_width))
            }
            _ => (post_width, pre_width),
        }
    }

    /// Checks if a segment is regressive (using KnotData).
    pub fn is_segment_regressive_knots(
        start: &KnotData,
        end: &KnotData,
        mode: AntiRegressionMode,
    ) -> bool {
        if mode == AntiRegressionMode::None {
            return false;
        }

        // Only check Bezier segments (curve interpolation)
        if start.next_interp != InterpMode::Curve {
            return false;
        }

        let interval = end.time - start.time;
        if interval <= 0.0 {
            return false;
        }

        let start_width = start.post_tan_width / interval;
        let end_width = end.pre_tan_width / interval;

        // In Contain mode, check simple max
        if mode == AntiRegressionMode::Contain {
            return start_width > CONTAINED_MAX || end_width > CONTAINED_MAX;
        }

        // Call math helper
        are_tan_widths_regressive(start_width, end_width)
    }

    /// Processes a segment in-place.
    ///
    /// Returns true if adjustments were made.
    pub fn process_segment_in_place(segment: &mut Segment, mode: AntiRegressionMode) -> bool {
        // If anti-regression is disabled, nothing to do
        if mode == AntiRegressionMode::None {
            return false;
        }

        // Only process Bezier segments
        if segment.interp != SegmentInterp::Bezier {
            return false;
        }

        let segment_width = segment.p1[0] - segment.p0[0];
        if segment_width <= 0.0 {
            return false;
        }

        // Create dummy state for segment-based solver
        let dummy_params = KnotParams {
            time: segment.p0[0],
            pre_tan_width: 0.0,
            post_tan_width: segment.t0[0] - segment.p0[0],
        };
        let mut active_state = WorkingKnotState {
            proposed_knot: None,
            proposed_params: dummy_params.clone(),
            working_params: dummy_params,
        };

        let mut result = SetResult::default();
        let internal_mode: Mode = mode.into();

        // Create solver with segment
        {
            let mut solver = SegmentSolver::new_with_segment(
                internal_mode,
                segment,
                &mut active_state,
                Some(&mut result),
            );
            solver.adjust();
        }

        result.adjusted
    }
}

// ============================================================================
// Public Math Functions
// ============================================================================

/// Checks if the given tangent widths would cause regression.
///
/// Both widths should be normalized to the segment interval [0, 1].
pub fn are_widths_regressive(width1: TsTime, width2: TsTime) -> bool {
    are_tan_widths_regressive(width1, width2)
}

/// Computes the other tangent width to create a single vertical.
///
/// Given one normalized width, returns the corresponding width that
/// will create exactly one vertical tangent (the edge of regression).
pub fn compute_width_for_vertical(width: TsTime, hint: TsTime) -> TsTime {
    compute_other_width_for_vert(width, hint)
}

/// Returns the maximum tangent width for the contained region.
pub fn contained_max() -> TsTime {
    CONTAINED_MAX
}

/// Returns the maximum tangent width for a single vertical (4/3).
pub fn vert_max() -> TsTime {
    VERT_MAX
}

/// Returns the minimum tangent width for a single vertical (1/3).
pub fn vert_min() -> TsTime {
    VERT_MIN
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert!((VERT_MAX - 4.0 / 3.0).abs() < 1e-10);
        assert!((VERT_MIN - 1.0 / 3.0).abs() < 1e-10);
        assert!((CONTAINED_MAX - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_set_result_debug() {
        let mut result = SetResult::default();
        let desc = result.debug_description(2);
        assert!(desc.contains("SetResult"));

        result.adjusted = true;
        result.have_pre_segment = true;
        result.pre_active_adjusted = true;
        result.pre_active_adjusted_width = 0.5;

        let desc = result.debug_description(2);
        assert!(desc.contains("preActiveAdjusted: true"));
    }

    #[test]
    fn test_are_tan_widths_regressive_contained() {
        // Contained widths are not regressive
        assert!(!are_tan_widths_regressive(0.5, 0.5));
        assert!(!are_tan_widths_regressive(0.3, 0.6));
        assert!(!are_tan_widths_regressive(1.0, 0.0));
        assert!(!are_tan_widths_regressive(0.0, 1.0));
    }

    #[test]
    fn test_are_tan_widths_regressive_outside() {
        // Widths that exceed contained region OR are outside ellipse are regressive
        // Note: (0.7, 0.7) is CONTAINED (both <= 1.0) so not regressive
        assert!(!are_tan_widths_regressive(0.7, 0.7)); // Contained
        // Widths where at least one exceeds 1.0 AND they're outside ellipse
        assert!(are_tan_widths_regressive(1.5, 0.5));
        assert!(are_tan_widths_regressive(0.5, 1.5));
        // Both exceed 1.0
        assert!(are_tan_widths_regressive(1.1, 1.1));
    }

    #[test]
    fn test_compute_other_width_for_vert() {
        // At VERT_MAX (4/3), the other width should be VERT_MIN (1/3)
        let result = compute_other_width_for_vert(VERT_MAX, 0.5);
        assert!((result - VERT_MIN).abs() < 0.01);

        // At width=1.0, the ellipse equation L2^2 + (1-2)L2 + (1-1)^2 = 0
        // becomes L2^2 - L2 = 0, so L2 = 0 or L2 = 1
        let result = compute_other_width_for_vert(1.0, 0.5);
        // With hint 0.5, should pick solution closer to 0.5
        assert!(result >= 0.0 && result <= 1.0);

        // At width=0.5, verify we get a valid result
        let result = compute_other_width_for_vert(0.5, 1.0);
        assert!(result >= 0.0);
    }

    #[test]
    fn test_batch_is_regressive() {
        let mut start = KnotData::new();
        start.time = 0.0;
        start.post_tan_width = 12.0; // Normalized: 12/10 = 1.2 > 1.0
        start.next_interp = InterpMode::Curve;

        let mut end = KnotData::new();
        end.time = 10.0;
        end.pre_tan_width = 6.0;

        // Width 1.2 > contained_max (1.0), so regressive in Contain mode
        assert!(RegressionPreventerBatch::is_segment_regressive_knots(
            &start,
            &end,
            AntiRegressionMode::Contain
        ));
    }

    #[test]
    fn test_batch_not_regressive() {
        let mut start = KnotData::new();
        start.time = 0.0;
        start.post_tan_width = 3.0;
        start.next_interp = InterpMode::Curve;

        let mut end = KnotData::new();
        end.time = 10.0;
        end.pre_tan_width = 3.0;

        // Combined = 0.6, not regressive
        assert!(!RegressionPreventerBatch::is_segment_regressive_knots(
            &start,
            &end,
            AntiRegressionMode::Contain
        ));
    }

    #[test]
    fn test_batch_process() {
        let mut start = KnotData::new();
        start.time = 0.0;
        start.post_tan_width = 15.0; // Normalized: 15/10 = 1.5 > 1.0
        start.next_interp = InterpMode::Curve;

        let mut end = KnotData::new();
        end.time = 10.0;
        end.pre_tan_width = 12.0; // Normalized: 12/10 = 1.2 > 1.0

        let adjusted = RegressionPreventerBatch::process_segment(
            &mut start,
            &mut end,
            AntiRegressionMode::Contain,
        );

        assert!(adjusted);
        // After processing, widths should be clamped to interval (10.0)
        assert!(start.post_tan_width <= 10.0 + 1e-10);
        assert!(end.pre_tan_width <= 10.0 + 1e-10);
    }

    #[test]
    fn test_batch_no_process_held() {
        let mut start = KnotData::new();
        start.time = 0.0;
        start.post_tan_width = 6.0;
        start.next_interp = InterpMode::Held; // Not curve

        let mut end = KnotData::new();
        end.time = 10.0;
        end.pre_tan_width = 6.0;

        // Should not process non-curve segments
        assert!(!RegressionPreventerBatch::is_segment_regressive_knots(
            &start,
            &end,
            AntiRegressionMode::Contain
        ));
    }

    #[test]
    fn test_ellipse_boundary() {
        // Points on the ellipse boundary should not be regressive
        // At (1, 1), both widths are at the balance point
        assert!(!are_tan_widths_regressive(1.0, 0.0));
        assert!(!are_tan_widths_regressive(0.0, 1.0));

        // Just outside should be regressive (with some tolerance)
        assert!(are_tan_widths_regressive(1.1, 1.1));
    }

    #[test]
    fn test_mode_conversion() {
        assert_eq!(Mode::from(AntiRegressionMode::None), Mode::None);
        assert_eq!(Mode::from(AntiRegressionMode::Contain), Mode::Contain);
        assert_eq!(Mode::from(AntiRegressionMode::KeepRatio), Mode::KeepRatio);
        assert_eq!(Mode::from(AntiRegressionMode::KeepStart), Mode::KeepStart);
    }

    #[test]
    fn test_interactive_mode_conversion() {
        assert_eq!(Mode::from(InteractiveMode::LimitActive), Mode::LimitActive);
        assert_eq!(
            Mode::from(InteractiveMode::LimitOpposite),
            Mode::LimitOpposite
        );
    }

    #[test]
    fn test_public_math_functions() {
        assert_eq!(contained_max(), CONTAINED_MAX);
        assert_eq!(vert_max(), VERT_MAX);
        assert_eq!(vert_min(), VERT_MIN);

        assert!(are_widths_regressive(1.2, 1.2));
        assert!(!are_widths_regressive(0.5, 0.5));
    }
}
