//! Accumulator framework for Light Path Expressions — full C++ parity.
//!
//! Port of `accum.h` / `accum.cpp`. The accumulator system maps compiled
//! LPE automata to named AOV buffers, supporting **incremental per-bounce
//! accumulation** as the path tracer advances through the scene.
//!
//! # Architecture
//!
//! ```text
//! Accumulator ─┬─ rules: Vec<AccumRule>     (name + LPEDFA)
//!              └─ buffers: Vec<AccumBuffer>  (named color accumulators)
//!
//! LPEPathState ── states: Vec<usize>        (per-rule DFA state for one path)
//! ```
//!
//! The path tracer holds an `Accumulator` (shared, immutable during render)
//! and creates an [`LPEPathState`] per active light path. At each bounce:
//!
//! 1. `accumulator.begin_path()` → new `LPEPathState` at Camera
//! 2. `accumulator.step(&mut path_state, event)` → advance all DFAs
//! 3. `accumulator.accumulate(&mut path_state, &path_state, contribution)`
//! 4. Repeat 2–3 for each bounce
//! 5. Final `step(Light/Object/Background)` + `accumulate()`

use crate::Float;
use crate::closure::{ClosureRef, DirectionKind, ScatteringKind};
use crate::lpe::{LPEDFA, LPEEvent, compile_lpe};
use crate::math::Color3;

// ---------------------------------------------------------------------------
// Per-path DFA state
// ---------------------------------------------------------------------------

/// Per-path state for all LPE rules in an accumulator.
///
/// Each active light path has its own `LPEPathState`. This is separate from
/// the `Accumulator` so multiple paths can be processed concurrently (e.g.
/// in a multi-threaded path tracer) without locking.
#[derive(Debug, Clone)]
pub struct LPEPathState {
    /// Current DFA state for each rule (indexed by rule index).
    pub states: Vec<usize>,
    /// State stack for push/pop during nested path tracking.
    /// C++ uses `std::stack<int>` in `Accumulator` (see `accum.h` line 227).
    /// We store it per-path since `LPEPathState` is the mutable per-path object.
    state_stack: Vec<Vec<usize>>,
}

impl LPEPathState {
    /// Save the current DFA states onto the stack.
    ///
    /// Used by integrators to speculatively test path continuations
    /// without losing the current state. See C++ `Accumulator::pushState()`.
    pub fn push_state(&mut self) {
        self.state_stack.push(self.states.clone());
    }

    /// Restore the DFA states from the stack.
    ///
    /// Returns `true` if the state was restored, `false` if the stack was empty.
    /// See C++ `Accumulator::popState()`.
    pub fn pop_state(&mut self) -> bool {
        if let Some(saved) = self.state_stack.pop() {
            self.states = saved;
            true
        } else {
            false
        }
    }

    /// Depth of the state stack.
    pub fn stack_depth(&self) -> usize {
        self.state_stack.len()
    }
}

// ---------------------------------------------------------------------------
// Accumulation rule
// ---------------------------------------------------------------------------

/// An accumulation rule mapping an LPE pattern to a named AOV buffer.
#[derive(Debug, Clone)]
pub struct AccumRule {
    /// Human-readable name (e.g. "diffuse", "beauty").
    pub name: String,
    /// The original LPE pattern string.
    pub lpe: String,
    /// Compiled DFA for matching.
    pub dfa: LPEDFA,
    /// Index into the accumulator's buffer list.
    pub buffer_index: usize,
}

// ---------------------------------------------------------------------------
// AOV buffer
// ---------------------------------------------------------------------------

/// Buffer content type for AOV accumulation.
#[derive(Debug, Clone)]
pub enum AccumBufValue {
    /// Color3 accumulator (default, most common).
    ColorBuf(Color3),
    /// Scalar float accumulator (e.g. depth, alpha AOVs).
    FloatBuf(Float),
    /// Closure accumulator (stores the last contributed closure).
    ClosureBuf(Option<ClosureRef>),
}

impl Default for AccumBufValue {
    fn default() -> Self {
        Self::ColorBuf(Color3::ZERO)
    }
}

/// A single AOV buffer that receives accumulated contributions.
#[derive(Debug, Clone)]
pub struct AccumBuffer {
    pub name: String,
    pub value: AccumBufValue,
    /// If true, contributions are subtracted instead of added.
    pub negate: bool,
}

impl AccumBuffer {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: AccumBufValue::ColorBuf(Color3::ZERO),
            negate: false,
        }
    }

    /// Create a float accumulation buffer.
    pub fn new_float(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: AccumBufValue::FloatBuf(0.0),
            negate: false,
        }
    }

    /// Create a closure accumulation buffer.
    pub fn new_closure(name: &str) -> Self {
        Self {
            name: name.to_string(),
            value: AccumBufValue::ClosureBuf(None),
            negate: false,
        }
    }

    #[inline]
    pub fn add(&mut self, contribution: Color3) {
        if let AccumBufValue::ColorBuf(ref mut v) = self.value {
            *v += contribution;
        }
    }

    /// Add a scalar float contribution.
    #[inline]
    pub fn add_float(&mut self, contribution: Float) {
        if let AccumBufValue::FloatBuf(ref mut v) = self.value {
            *v += contribution;
        }
    }

    /// Set a closure contribution.
    #[inline]
    pub fn set_closure(&mut self, closure: ClosureRef) {
        if let AccumBufValue::ClosureBuf(ref mut v) = self.value {
            *v = Some(closure);
        }
    }

    /// Get color value (returns ZERO for non-color buffers).
    /// If `negate` is set, returns `1.0 - accumulated` matching C++ flush semantics.
    #[inline]
    pub fn color_value(&self) -> Color3 {
        match &self.value {
            AccumBufValue::ColorBuf(v) => {
                if self.negate {
                    Color3::new(1.0 - v.x, 1.0 - v.y, 1.0 - v.z)
                } else {
                    *v
                }
            }
            _ => Color3::ZERO,
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        match &mut self.value {
            AccumBufValue::ColorBuf(v) => *v = Color3::ZERO,
            AccumBufValue::FloatBuf(v) => *v = 0.0,
            AccumBufValue::ClosureBuf(v) => *v = None,
        }
    }
}

// ---------------------------------------------------------------------------
// Accumulator
// ---------------------------------------------------------------------------

/// The main LPE accumulator managing rules and AOV buffers.
///
/// Thread-safety: the accumulator itself is immutable during rendering.
/// Mutable state lives in [`LPEPathState`] (per-path) and the buffers
/// (written at accumulation time — caller is responsible for synchronization
/// if multiple threads write to the same accumulator).
#[derive(Debug)]
pub struct Accumulator {
    rules: Vec<AccumRule>,
    buffers: Vec<AccumBuffer>,
}

impl Accumulator {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            buffers: Vec::new(),
        }
    }

    /// Add an LPE-to-AOV mapping rule.
    pub fn add_rule(&mut self, name: &str, lpe_expr: &str) -> Result<(), String> {
        let dfa = compile_lpe(lpe_expr)?;

        // Find or create buffer
        let buffer_index = self
            .buffers
            .iter()
            .position(|b| b.name == name)
            .unwrap_or_else(|| {
                let idx = self.buffers.len();
                self.buffers.push(AccumBuffer::new(name));
                idx
            });

        self.rules.push(AccumRule {
            name: name.to_string(),
            lpe: lpe_expr.to_string(),
            dfa,
            buffer_index,
        });
        Ok(())
    }

    /// Begin a new light path: initializes state to Camera event.
    ///
    /// The returned `LPEPathState` has already been stepped through the
    /// Camera bounce: Camera + wildcard_scatter + STOP.
    pub fn begin_path(&self) -> LPEPathState {
        let states: Vec<usize> = self
            .rules
            .iter()
            .map(|rule| {
                let mut s = rule.dfa.initial_state();
                // Camera bounce: Camera(event) + scatter(wildcard) + STOP
                s = rule.dfa.step(s, LPEEvent::Camera);
                // Feed a scatter event as the wildcard scatter slot.
                // Use Diffuse/Reflect as a stand-in; the NFA wildcard [^STOP]
                // accepts any non-Stop event here.
                s = rule.dfa.step(
                    s,
                    LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Reflect),
                );
                s = rule.dfa.step(s, LPEEvent::Stop);
                s
            })
            .collect();
        LPEPathState {
            states,
            state_stack: Vec::new(),
        }
    }

    /// Step all rules by a scattering event (from closure labels).
    ///
    /// Feeds the full STOP-framed bounce: event(scatter) + scatter + STOP.
    pub fn step_scatter(
        &self,
        path: &mut LPEPathState,
        scattering: ScatteringKind,
        direction: DirectionKind,
    ) {
        let event = LPEEvent::scatter(scattering, direction);
        // Bounce: event_slot(scatter) + scatter_slot(scatter) + STOP
        self.step_raw(path, event);
        self.step_raw(path, event);
        self.step_raw(path, LPEEvent::Stop);
    }

    /// Step all rules by a terminal event (Light, Object, Background, etc.).
    ///
    /// Feeds the full STOP-framed bounce: terminal + wildcard_scatter + STOP.
    pub fn step(&self, path: &mut LPEPathState, event: LPEEvent) {
        // Terminal bounce: event + scatter_wildcard + STOP
        self.step_raw(path, event);
        self.step_raw(
            path,
            LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Reflect),
        );
        self.step_raw(path, LPEEvent::Stop);
    }

    /// Step all rules by a single raw event (no STOP framing).
    fn step_raw(&self, path: &mut LPEPathState, event: LPEEvent) {
        for (i, rule) in self.rules.iter().enumerate() {
            path.states[i] = rule.dfa.step(path.states[i], event);
        }
    }

    /// Accumulate a contribution for all rules currently in an accepting state.
    pub fn accumulate(&mut self, path: &LPEPathState, contribution: Color3) {
        for (i, rule) in self.rules.iter().enumerate() {
            if rule.dfa.is_accept(path.states[i]) {
                self.buffers[rule.buffer_index].add(contribution);
            }
        }
    }

    /// Legacy API: match a complete path at once (non-incremental).
    ///
    /// Interprets `events` as a logical bounce sequence: [Camera, scatter*, terminal].
    /// Internally inserts STOP framing for each bounce before matching against the DFA.
    pub fn accumulate_path(&mut self, events: &[LPEEvent], contribution: Color3) {
        if events.is_empty() {
            return;
        }
        // Build STOP-framed event stream from the logical bounce sequence.
        let wildcard_scatter = LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Reflect);
        let mut framed = Vec::with_capacity(events.len() * 3);
        for (i, &ev) in events.iter().enumerate() {
            if i == 0 {
                // Camera bounce: Camera + wildcard_scatter + STOP
                framed.push(ev);
                framed.push(wildcard_scatter);
                framed.push(LPEEvent::Stop);
            } else if matches!(ev, LPEEvent::Scatter { .. } | LPEEvent::UserDefined(_)) {
                // Scatter bounce: scatter(event_slot) + scatter(scatter_slot) + STOP
                framed.push(ev);
                framed.push(ev);
                framed.push(LPEEvent::Stop);
            } else {
                // Terminal bounce: terminal + wildcard_scatter + STOP
                framed.push(ev);
                framed.push(wildcard_scatter);
                framed.push(LPEEvent::Stop);
            }
        }
        for rule in &self.rules {
            if rule.dfa.matches(&framed) {
                self.buffers[rule.buffer_index].add(contribution);
            }
        }
    }

    /// Get the accumulated color value for a named AOV.
    pub fn get(&self, name: &str) -> Option<Color3> {
        self.buffers
            .iter()
            .find(|b| b.name == name)
            .map(|b| b.color_value())
    }

    /// Get buffer by index.
    pub fn get_buffer(&self, index: usize) -> Option<&AccumBuffer> {
        self.buffers.get(index)
    }

    /// Reset all buffers to zero.
    pub fn reset(&mut self) {
        for buf in &mut self.buffers {
            buf.reset();
        }
    }

    /// Get all buffer names.
    pub fn buffer_names(&self) -> Vec<&str> {
        self.buffers.iter().map(|b| b.name.as_str()).collect()
    }

    /// Number of rules.
    pub fn num_rules(&self) -> usize {
        self.rules.len()
    }

    /// Number of buffers.
    pub fn num_buffers(&self) -> usize {
        self.buffers.len()
    }

    /// Check if any rule is still alive (not dead) in the given path state.
    pub fn any_alive(&self, path: &LPEPathState) -> bool {
        self.rules
            .iter()
            .enumerate()
            .any(|(i, rule)| !rule.dfa.is_dead(path.states[i]))
    }
}

impl Default for Accumulator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Standard AOV rules (production renderer conventions)
// ---------------------------------------------------------------------------

/// Standard AOV rules used by production renderers.
///
/// These follow the conventions from Arnold, RenderMan, Cycles, etc.
pub fn standard_aov_rules() -> Vec<(&'static str, &'static str)> {
    vec![
        ("beauty", "C[DGSW]*[LO]"),
        ("diffuse", "CD[DGSW]*[LO]"),
        ("glossy", "CG[DGSW]*[LO]"),
        ("specular", "CS[DGSW]*[LO]"),
        ("direct_diffuse", "CDL"),
        ("indirect_diffuse", "CD[DGSW]+[LO]"),
        ("direct_glossy", "CGL"),
        ("indirect_glossy", "CG[DGSW]+[LO]"),
        ("direct_specular", "CSL"),
        ("indirect_specular", "CS[DGSW]+[LO]"),
        ("emission", "CO"),
        ("background", "CB"),
        ("volume", "CV[DGSW]*[LO]"),
        ("transmission", "C<T>[DGSW]*[LO]"),
        ("reflection", "C<R>[DGSW]*[LO]"),
        ("diffuse_reflect", "C<RD>[DGSW]*[LO]"),
        ("diffuse_transmit", "C<TD>[DGSW]*[LO]"),
        ("glossy_reflect", "C<RG>[DGSW]*[LO]"),
        ("glossy_transmit", "C<TG>[DGSW]*[LO]"),
    ]
}

/// Create an Accumulator with standard AOV rules pre-loaded.
pub fn create_standard_accumulator() -> Result<Accumulator, String> {
    let mut acc = Accumulator::new();
    for (name, lpe) in standard_aov_rules() {
        acc.add_rule(name, lpe)?;
    }
    Ok(acc)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closure::{DirectionKind, ScatteringKind};
    use crate::lpe::LPEEvent;

    #[test]
    fn test_incremental_accumulation() {
        let mut acc = Accumulator::new();
        acc.add_rule("direct_diffuse", "CDL").unwrap();
        acc.add_rule("beauty", "C[DGSW]*[LO]").unwrap();

        // Path: Camera → Diffuse(Reflect) → Light
        let mut path = acc.begin_path();
        acc.step_scatter(&mut path, ScatteringKind::Diffuse, DirectionKind::Reflect);
        acc.step(&mut path, LPEEvent::Light);
        acc.accumulate(&path, Color3::new(0.5, 0.3, 0.1));

        let dd = acc.get("direct_diffuse").unwrap();
        assert!((dd.x - 0.5).abs() < 1e-6);
        let beauty = acc.get("beauty").unwrap();
        assert!((beauty.x - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_indirect_does_not_match_direct() {
        let mut acc = Accumulator::new();
        acc.add_rule("indirect_diffuse", "CD[DGSW]+[LO]").unwrap();

        // Direct path: Camera → Diffuse → Light (only 1 bounce)
        let mut path = acc.begin_path();
        acc.step_scatter(&mut path, ScatteringKind::Diffuse, DirectionKind::Reflect);
        acc.step(&mut path, LPEEvent::Light);
        acc.accumulate(&path, Color3::new(1.0, 1.0, 1.0));

        let v = acc.get("indirect_diffuse").unwrap();
        assert!(
            (v.x).abs() < 1e-6,
            "direct path should not match indirect rule"
        );
    }

    #[test]
    fn test_indirect_matches_multi_bounce() {
        let mut acc = Accumulator::new();
        acc.add_rule("indirect_diffuse", "CD[DGSW]+[LO]").unwrap();

        // Indirect: Camera → Diffuse → Glossy → Light
        let mut path = acc.begin_path();
        acc.step_scatter(&mut path, ScatteringKind::Diffuse, DirectionKind::Reflect);
        acc.step_scatter(&mut path, ScatteringKind::Glossy, DirectionKind::Reflect);
        acc.step(&mut path, LPEEvent::Light);
        acc.accumulate(&path, Color3::new(1.0, 0.0, 0.0));

        let v = acc.get("indirect_diffuse").unwrap();
        assert!((v.x - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_emission() {
        let mut acc = Accumulator::new();
        acc.add_rule("emission", "CO").unwrap();

        let mut path = acc.begin_path();
        acc.step(&mut path, LPEEvent::Object);
        acc.accumulate(&path, Color3::new(0.0, 1.0, 0.0));

        let v = acc.get("emission").unwrap();
        assert!((v.y - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_directed_accumulation() {
        let mut acc = Accumulator::new();
        acc.add_rule("diffuse_reflect", "C<RD>[DGSW]*[LO]").unwrap();
        acc.add_rule("diffuse_transmit", "C<TD>[DGSW]*[LO]")
            .unwrap();

        // Reflect path
        let mut rpath = acc.begin_path();
        acc.step_scatter(&mut rpath, ScatteringKind::Diffuse, DirectionKind::Reflect);
        acc.step(&mut rpath, LPEEvent::Light);
        acc.accumulate(&rpath, Color3::new(1.0, 0.0, 0.0));

        // Transmit path
        let mut tpath = acc.begin_path();
        acc.step_scatter(&mut tpath, ScatteringKind::Diffuse, DirectionKind::Transmit);
        acc.step(&mut tpath, LPEEvent::Light);
        acc.accumulate(&tpath, Color3::new(0.0, 0.0, 1.0));

        let dr = acc.get("diffuse_reflect").unwrap();
        assert!((dr.x - 1.0).abs() < 1e-6);
        assert!((dr.z).abs() < 1e-6);

        let dt = acc.get("diffuse_transmit").unwrap();
        assert!((dt.z - 1.0).abs() < 1e-6);
        assert!((dt.x).abs() < 1e-6);
    }

    #[test]
    fn test_any_alive() {
        let mut acc = Accumulator::new();
        acc.add_rule("test", "CDL").unwrap();

        let path = acc.begin_path(); // after Camera bounce
        assert!(acc.any_alive(&path));

        // Wrong scatter type → kills the DFA (Glossy doesn't match Diffuse in "CDL")
        let mut dead_path = acc.begin_path();
        acc.step_scatter(
            &mut dead_path,
            ScatteringKind::Glossy,
            DirectionKind::Reflect,
        );
        acc.step(&mut dead_path, LPEEvent::Light);
        assert!(!acc.any_alive(&dead_path));
    }

    #[test]
    fn test_reset() {
        let mut acc = Accumulator::new();
        acc.add_rule("test", "CDL").unwrap();
        acc.accumulate_path(
            &[
                LPEEvent::Camera,
                LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Reflect),
                LPEEvent::Light,
            ],
            Color3::new(1.0, 0.0, 0.0),
        );
        assert!(acc.get("test").unwrap().x > 0.0);
        acc.reset();
        assert!((acc.get("test").unwrap().x).abs() < 1e-6);
    }

    #[test]
    fn test_standard_accumulator() {
        let acc = create_standard_accumulator().unwrap();
        assert!(acc.num_rules() >= 15);
        assert!(acc.buffer_names().contains(&"beauty"));
        assert!(acc.buffer_names().contains(&"emission"));
    }

    #[test]
    fn test_legacy_accumulate_path() {
        let mut acc = Accumulator::new();
        acc.add_rule("specular", "CS[DGSW]*[LO]").unwrap();

        let path = [
            LPEEvent::Camera,
            LPEEvent::scatter(ScatteringKind::Singular, DirectionKind::Reflect),
            LPEEvent::Light,
        ];
        acc.accumulate_path(&path, Color3::new(1.0, 1.0, 1.0));

        let v = acc.get("specular").unwrap();
        assert!((v.x - 1.0).abs() < 1e-6);
    }
}
