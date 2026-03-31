//! Light Path Expressions -- parser, NFA, DFA with incremental matching.
//!
//! Full port of `lpeparse.cpp/h` and `automata.cpp/h` from C++ OSL.
//!
//! # LPE Syntax
//!
//! ```text
//! C            Camera event
//! L            Light event
//! O            Object (emission) event
//! B            Background event
//! V            Volume event
//! D            Diffuse scattering (any direction)
//! G            Glossy scattering (any direction)
//! S            Singular/specular scattering (any direction)
//! W            Straight (transparent pass-through)
//! .            Any event except STOP (wildcard)
//! *  +  ?      Quantifiers: zero-or-more, one-or-more, optional
//! [DGS]        Character class (any of the listed)
//! [^DG]        Negated class (anything except listed + STOP)
//! (expr|expr)  Grouping with alternation
//! <TD>         Directed scatter: Transmit + Diffuse
//! <RG>         Directed scatter: Reflect + Glossy
//! <T>          Direction only (any scattering)
//! 'label'      User-defined custom label (registered via LabelRegistry)
//! ```
//!
//! # STOP semantics
//!
//! Following C++ OSL `lpeparse.cpp`, each symbol outside a `<>` group is
//! automatically wrapped with the `buildStop` pattern:
//!
//! ```text
//! event_type scatter_type [custom_labels]* [any_non_builtin]* STOP
//! ```
//!
//! This allows the integrator to feed per-bounce label sequences terminated
//! by a STOP marker, enabling correct matching of multi-lobe surfaces and
//! custom closure labels.
//!
//! # Incremental matching
//!
//! The DFA supports step-by-step matching for use in path tracers:
//! ```ignore
//! let dfa = compile_lpe("CDL").unwrap();
//! let mut state = dfa.initial_state();
//! state = dfa.step(state, LPEEvent::Camera);
//! state = dfa.step(state, LPEEvent::Stop);
//! state = dfa.step(state, LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Reflect));
//! state = dfa.step(state, LPEEvent::Stop);
//! state = dfa.step(state, LPEEvent::Light);
//! state = dfa.step(state, LPEEvent::Stop);
//! assert!(dfa.is_accept(state));
//! ```

use std::collections::{HashMap, HashSet, VecDeque};

use crate::closure::{DirectionKind, ScatteringKind};

// ---------------------------------------------------------------------------
// Concrete events -- what the integrator feeds to the DFA
// ---------------------------------------------------------------------------

/// Number of distinct base concrete events in the LPE alphabet.
/// 6 terminal (Camera, Light, Background, Volume, Object, Stop) +
/// 4 scattering x 2 direction = 14
pub const NUM_BASE_EVENTS: usize = 14;

/// Legacy alias.
pub const NUM_EVENTS: usize = NUM_BASE_EVENTS;

/// Sentinel value: the DFA dead state (no further matches possible).
pub const DEAD_STATE: usize = usize::MAX;

/// A concrete event in a light transport path, fed to the LPE DFA.
///
/// The integrator constructs these at each path vertex:
/// - `Camera` at the path origin
/// - `Scatter { .. }` at each surface bounce (from closure labels)
/// - `Light` / `Background` / `Object` / `Volume` at the path terminus
/// - `Stop` at the end of each bounce's label sequence
/// - `UserDefined(id)` for custom closure labels registered via [`LabelRegistry`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LPEEvent {
    // Terminal / path-level events
    Camera,
    Light,
    Background,
    Volume,
    Object,
    Stop,
    // Scattering events (direction + type from closure labels)
    Scatter {
        scattering: ScatteringKind,
        direction: DirectionKind,
    },
    /// User-defined label (registered via LabelRegistry).
    /// The id maps to ordinal `NUM_BASE_EVENTS + id`.
    UserDefined(u32),
}

impl LPEEvent {
    /// Convenience constructor for scatter events.
    #[inline]
    pub fn scatter(scattering: ScatteringKind, direction: DirectionKind) -> Self {
        Self::Scatter {
            scattering,
            direction,
        }
    }

    /// Map this event to a unique ordinal.
    /// Base events: 0..NUM_BASE_EVENTS, user labels: NUM_BASE_EVENTS+id.
    pub fn ordinal(&self) -> usize {
        match self {
            LPEEvent::Camera => 0,
            LPEEvent::Light => 1,
            LPEEvent::Background => 2,
            LPEEvent::Volume => 3,
            LPEEvent::Object => 4,
            LPEEvent::Stop => 5,
            LPEEvent::Scatter {
                direction,
                scattering,
            } => {
                let dir = match direction {
                    DirectionKind::Reflect => 0,
                    DirectionKind::Transmit => 4,
                    DirectionKind::None => 0, // treat as reflect
                };
                let scat = match scattering {
                    ScatteringKind::Diffuse => 0,
                    ScatteringKind::Glossy => 1,
                    ScatteringKind::Singular => 2,
                    ScatteringKind::Straight => 3,
                    ScatteringKind::None => 0,
                };
                6 + dir + scat
            }
            LPEEvent::UserDefined(id) => NUM_BASE_EVENTS + *id as usize,
        }
    }

    /// Reconstruct a base event from its ordinal.
    /// Panics for ordinals >= NUM_BASE_EVENTS (use UserDefined directly).
    pub fn from_ordinal(ord: usize) -> Self {
        match ord {
            0 => LPEEvent::Camera,
            1 => LPEEvent::Light,
            2 => LPEEvent::Background,
            3 => LPEEvent::Volume,
            4 => LPEEvent::Object,
            5 => LPEEvent::Stop,
            6..=13 => {
                let idx = ord - 6;
                let direction = if idx < 4 {
                    DirectionKind::Reflect
                } else {
                    DirectionKind::Transmit
                };
                let scattering = match idx % 4 {
                    0 => ScatteringKind::Diffuse,
                    1 => ScatteringKind::Glossy,
                    2 => ScatteringKind::Singular,
                    3 => ScatteringKind::Straight,
                    _ => unreachable!(),
                };
                LPEEvent::Scatter {
                    scattering,
                    direction,
                }
            }
            _ => LPEEvent::UserDefined((ord - NUM_BASE_EVENTS) as u32),
        }
    }
}

// ---------------------------------------------------------------------------
// Label registry -- runtime registration of custom labels (C4)
// ---------------------------------------------------------------------------

/// Position in the per-bounce label sequence (matches C++ m_label_position).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelPosition {
    /// Event type slot (position 0): Camera, Light, Background, Volume, etc.
    Event,
    /// Scattering type slot (position 1): Diffuse, Glossy, Singular, Straight.
    Scatter,
    /// Custom label slot (position 2+): user-defined closure labels.
    Custom,
}

/// Registry for user-defined LPE labels.
///
/// Production renderers register custom closure labels here before compiling
/// LPE expressions. Matches C++ `AccumAutomata::addEventType/addScatteringType`
/// and `Parser(user_events, user_scatterings)`.
///
/// # Example
/// ```ignore
/// let mut reg = LabelRegistry::new();
/// reg.register_event("coat");        // custom event label
/// reg.register_scatter("metallic");  // custom scatter label
/// reg.register_custom("albedo");     // custom extra label
/// let dfa = compile_lpe_with_labels("C'coat'L", &reg).unwrap();
/// ```
#[derive(Debug, Clone, Default)]
pub struct LabelRegistry {
    /// label name -> (ordinal offset from NUM_BASE_EVENTS, position)
    labels: HashMap<String, (u32, LabelPosition)>,
    /// Next free user label ID.
    next_id: u32,
}

impl LabelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a user-defined event-type label (position 0).
    /// Returns the label's ordinal for feeding to the DFA.
    pub fn register_event(&mut self, name: &str) -> u32 {
        self.register(name, LabelPosition::Event)
    }

    /// Register a user-defined scattering-type label (position 1).
    pub fn register_scatter(&mut self, name: &str) -> u32 {
        self.register(name, LabelPosition::Scatter)
    }

    /// Register a user-defined custom label (extra position).
    pub fn register_custom(&mut self, name: &str) -> u32 {
        self.register(name, LabelPosition::Custom)
    }

    fn register(&mut self, name: &str, pos: LabelPosition) -> u32 {
        if let Some(&(id, _)) = self.labels.get(name) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.labels.insert(name.to_string(), (id, pos));
        id
    }

    /// Look up a label by name. Returns (id, position) or None.
    pub fn lookup(&self, name: &str) -> Option<(u32, LabelPosition)> {
        self.labels.get(name).copied()
    }

    /// Total number of events (base + user labels).
    pub fn total_events(&self) -> usize {
        NUM_BASE_EVENTS + self.next_id as usize
    }

    /// Number of registered user labels.
    pub fn num_user_labels(&self) -> usize {
        self.next_id as usize
    }

    /// Iterate over all registered labels.
    pub fn iter(&self) -> impl Iterator<Item = (&str, u32, LabelPosition)> {
        self.labels
            .iter()
            .map(|(name, &(id, pos))| (name.as_str(), id, pos))
    }
}

// ---------------------------------------------------------------------------
// Pattern symbols -- used in the NFA
// ---------------------------------------------------------------------------

/// A symbol in an LPE pattern, used for NFA transitions.
///
/// These represent the *pattern* side: what the parser produces.
/// During DFA construction, each symbol is expanded to the set of concrete
/// [`LPEEvent`] ordinals it matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LPESymbol {
    // Terminal events
    Camera,
    Light,
    Background,
    Volume,
    Object,
    Stop,
    /// Scattering type, any direction: `D`, `G`, `S`, `W`
    Scattering(ScatteringKind),
    /// Directed scatter: `<TD>`, `<RG>`, etc.
    DirectedScatter(DirectionKind, ScatteringKind),
    /// Direction only: `<T>`, `<R>` (any scattering type)
    Direction(DirectionKind),
    /// Wildcard `.`: matches any event EXCEPT Stop.
    /// This is C++'s `Wildexp(m_minus_stop)`.
    Any,
    /// User-defined label (from LabelRegistry).
    UserLabel(u32),
}

impl LPESymbol {
    /// Check if this symbol matches a concrete event ordinal.
    fn matches_ordinal(&self, ord: usize, total_events: usize) -> bool {
        match self {
            LPESymbol::Camera => ord == 0,
            LPESymbol::Light => ord == 1,
            LPESymbol::Background => ord == 2,
            LPESymbol::Volume => ord == 3,
            LPESymbol::Object => ord == 4,
            LPESymbol::Stop => ord == 5,
            LPESymbol::Scattering(sk) => {
                if ord < 6 || ord >= 14 {
                    return false;
                }
                (ord - 6) % 4 == scat_offset(*sk)
            }
            LPESymbol::DirectedScatter(dk, sk) => {
                if ord < 6 || ord >= 14 {
                    return false;
                }
                let idx = ord - 6;
                let dir_ok = match dk {
                    DirectionKind::Reflect => idx < 4,
                    DirectionKind::Transmit => idx >= 4,
                    _ => false,
                };
                dir_ok && idx % 4 == scat_offset(*sk)
            }
            LPESymbol::Direction(dk) => {
                if ord < 6 || ord >= 14 {
                    return false;
                }
                let idx = ord - 6;
                match dk {
                    DirectionKind::Reflect => idx < 4,
                    DirectionKind::Transmit => idx >= 4,
                    _ => false,
                }
            }
            // Any matches everything EXCEPT Stop (ordinal 5).
            // This is the key STOP semantic: C++'s Wildexp(m_minus_stop).
            LPESymbol::Any => ord != 5 && ord < total_events,
            LPESymbol::UserLabel(id) => ord == NUM_BASE_EVENTS + *id as usize,
        }
    }
}

fn scat_offset(sk: ScatteringKind) -> usize {
    match sk {
        ScatteringKind::Diffuse => 0,
        ScatteringKind::Glossy => 1,
        ScatteringKind::Singular => 2,
        ScatteringKind::Straight => 3,
        ScatteringKind::None => 0,
    }
}

// ---------------------------------------------------------------------------
// NFA
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct NFAState {
    /// Transitions: (symbol_or_epsilon, target_state)
    transitions: Vec<(Option<LPESymbol>, usize)>,
    is_accept: bool,
}

impl NFAState {
    fn new() -> Self {
        Self {
            transitions: Vec::new(),
            is_accept: false,
        }
    }
}

/// Non-deterministic finite automaton for LPE matching.
#[derive(Debug)]
pub struct LPENFA {
    states: Vec<NFAState>,
    start: usize,
    /// Total event count (base + user labels).
    total_events: usize,
}

impl LPENFA {
    fn new(total_events: usize) -> Self {
        Self {
            states: Vec::new(),
            start: 0,
            total_events,
        }
    }

    fn add_state(&mut self) -> usize {
        let id = self.states.len();
        self.states.push(NFAState::new());
        id
    }

    fn add_transition(&mut self, from: usize, sym: LPESymbol, to: usize) {
        self.states[from].transitions.push((Some(sym), to));
    }

    fn add_epsilon(&mut self, from: usize, to: usize) {
        self.states[from].transitions.push((None, to));
    }
}

// ---------------------------------------------------------------------------
// DFA -- compiled, O(1) transition lookup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DFAState {
    /// transitions[ordinal] = target state index, or DEAD_STATE.
    /// Length = total_events (base + user labels).
    transitions: Vec<usize>,
    is_accept: bool,
}

/// Compiled DFA for efficient LPE matching.
///
/// Supports both batch matching (full path) and incremental step-by-step
/// matching for use in production path tracers.
#[derive(Debug, Clone)]
pub struct LPEDFA {
    states: Vec<DFAState>,
    /// User-defined custom event labels (name -> ordinal).
    custom_labels: HashMap<String, u32>,
    /// Total event alphabet size.
    total_events: usize,
}

impl LPEDFA {
    /// The initial DFA state for a new path.
    #[inline]
    pub fn initial_state(&self) -> usize {
        0
    }

    /// Advance the DFA by one event. Returns the new state.
    ///
    /// If the returned state equals [`DEAD_STATE`], no further events can
    /// lead to an accepting state -- the path tracer can stop early.
    #[inline]
    pub fn step(&self, state: usize, event: LPEEvent) -> usize {
        if state == DEAD_STATE || state >= self.states.len() {
            return DEAD_STATE;
        }
        let ord = event.ordinal();
        if ord >= self.total_events {
            return DEAD_STATE;
        }
        self.states[state].transitions[ord]
    }

    /// Check if a state is accepting (the path matched the LPE).
    #[inline]
    pub fn is_accept(&self, state: usize) -> bool {
        if state == DEAD_STATE || state >= self.states.len() {
            return false;
        }
        self.states[state].is_accept
    }

    /// Check if a state is the dead state (no further matches possible).
    #[inline]
    pub fn is_dead(&self, state: usize) -> bool {
        state == DEAD_STATE
    }

    /// Number of DFA states (for diagnostics).
    pub fn num_states(&self) -> usize {
        self.states.len()
    }

    /// Register a user-defined event label (post-compilation convenience).
    pub fn register_label(&mut self, name: &str, id: u32) {
        self.custom_labels.insert(name.to_string(), id);
    }

    /// Look up a user-defined event label by name.
    pub fn lookup_label(&self, name: &str) -> Option<u32> {
        self.custom_labels.get(name).copied()
    }

    /// Match a complete path (convenience wrapper over step-by-step).
    pub fn matches(&self, events: &[LPEEvent]) -> bool {
        let mut state = self.initial_state();
        for &event in events {
            state = self.step(state, event);
            if state == DEAD_STATE {
                return false;
            }
        }
        self.is_accept(state)
    }
}

// ---------------------------------------------------------------------------
// Parser -- recursive descent, Thompson NFA construction
// ---------------------------------------------------------------------------

/// Parse an LPE string into a compiled DFA (no user labels, with STOP wrapping).
pub fn compile_lpe(expr: &str) -> Result<LPEDFA, String> {
    let reg = LabelRegistry::new();
    compile_lpe_with_labels(expr, &reg)
}

/// Parse an LPE string into a compiled DFA with user label support.
pub fn compile_lpe_with_labels(expr: &str, labels: &LabelRegistry) -> Result<LPEDFA, String> {
    let nfa = parse_lpe_with_labels(expr, labels)?;
    Ok(nfa_to_dfa(&nfa))
}

/// Parse an LPE string into an NFA (no user labels).
pub fn parse_lpe(expr: &str) -> Result<LPENFA, String> {
    let reg = LabelRegistry::new();
    parse_lpe_with_labels(expr, &reg)
}

/// Parse an LPE string into an NFA with user label support.
///
/// Each symbol outside `<>` groups is wrapped with the C++ `buildStop` pattern:
/// `event scatter [custom]* [any_non_builtin]* STOP`
///
/// Inside `<>` groups, symbols are raw (no wrapping).
pub fn parse_lpe_with_labels(expr: &str, labels: &LabelRegistry) -> Result<LPENFA, String> {
    let total = labels.total_events();
    let mut parser = LPEParser {
        chars: expr.chars().collect(),
        pos: 0,
        nfa: LPENFA::new(total),
        in_group: false,
        labels,
    };
    let (start, end) = parser.parse_expr()?;
    parser.nfa.start = start;
    parser.nfa.states[end].is_accept = true;
    Ok(parser.nfa)
}

struct LPEParser<'a> {
    chars: Vec<char>,
    pos: usize,
    nfa: LPENFA,
    /// True when inside a `<>` group (no buildStop wrapping).
    in_group: bool,
    /// Label registry for user-defined labels.
    labels: &'a LabelRegistry,
}

/// Label position for built-in symbols (matches C++ m_label_position).
fn builtin_label_pos(sym: &LPESymbol) -> Option<u8> {
    match sym {
        LPESymbol::Camera
        | LPESymbol::Light
        | LPESymbol::Background
        | LPESymbol::Volume
        | LPESymbol::Object
        | LPESymbol::Direction(_) => Some(0), // event position
        LPESymbol::Scattering(_) | LPESymbol::DirectedScatter(_, _) => Some(1), // scatter position
        _ => None,
    }
}

impl<'a> LPEParser<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> char {
        let c = self.chars[self.pos];
        self.pos += 1;
        c
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        match self.peek() {
            Some(c) if c == expected => {
                self.advance();
                Ok(())
            }
            Some(c) => Err(format!(
                "expected '{expected}', got '{c}' at pos {}",
                self.pos
            )),
            None => Err(format!("expected '{expected}', got end of input")),
        }
    }

    fn has_factor_start(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                'C' | 'L' | 'D' | 'G' | 'S' | 'B' | 'V' | 'O' | 'W' | '.' | '(' | '[' | '<' | '\''
            )
        )
    }

    // expr = seq ('|' seq)*
    fn parse_expr(&mut self) -> Result<(usize, usize), String> {
        let (s1, e1) = self.parse_seq()?;
        if self.peek() != Some('|') {
            return Ok((s1, e1));
        }
        // Alternation
        let union_s = self.nfa.add_state();
        let union_e = self.nfa.add_state();
        self.nfa.add_epsilon(union_s, s1);
        self.nfa.add_epsilon(e1, union_e);

        while self.peek() == Some('|') {
            self.advance();
            let (s, e) = self.parse_seq()?;
            self.nfa.add_epsilon(union_s, s);
            self.nfa.add_epsilon(e, union_e);
        }
        Ok((union_s, union_e))
    }

    // seq = factor+
    fn parse_seq(&mut self) -> Result<(usize, usize), String> {
        if !self.has_factor_start() {
            // Empty sequence -- epsilon
            let s = self.nfa.add_state();
            return Ok((s, s));
        }
        let (start, mut end) = self.parse_factor()?;
        while self.has_factor_start() {
            let (s, e) = self.parse_factor()?;
            self.nfa.add_epsilon(end, s);
            end = e;
        }
        Ok((start, end))
    }

    // factor = atom quantifier?
    fn parse_factor(&mut self) -> Result<(usize, usize), String> {
        let (atom_s, atom_e) = self.parse_atom()?;
        match self.peek() {
            Some('*') => {
                self.advance();
                let q1 = self.nfa.add_state();
                let q2 = self.nfa.add_state();
                self.nfa.add_epsilon(q1, atom_s);
                self.nfa.add_epsilon(atom_e, atom_s); // loop
                self.nfa.add_epsilon(atom_e, q2); // exit
                self.nfa.add_epsilon(q1, q2); // zero
                Ok((q1, q2))
            }
            Some('+') => {
                self.advance();
                let q2 = self.nfa.add_state();
                self.nfa.add_epsilon(atom_e, atom_s); // loop
                self.nfa.add_epsilon(atom_e, q2); // exit
                Ok((atom_s, q2))
            }
            Some('?') => {
                self.advance();
                let q1 = self.nfa.add_state();
                let q2 = self.nfa.add_state();
                self.nfa.add_epsilon(q1, atom_s);
                self.nfa.add_epsilon(atom_e, q2);
                self.nfa.add_epsilon(q1, q2); // zero
                Ok((q1, q2))
            }
            Some('{') => self.parse_bounded_rep(atom_s, atom_e),
            _ => Ok((atom_s, atom_e)),
        }
    }

    // atom = symbol | quoted_label | '(' expr ')' | '[' class ']' | '<' compound '>'
    fn parse_atom(&mut self) -> Result<(usize, usize), String> {
        match self.peek() {
            Some('(') => {
                self.advance();
                let result = self.parse_expr()?;
                self.expect(')')?;
                Ok(result)
            }
            Some('[') => self.parse_class(),
            Some('<') => self.parse_compound(),
            Some('\'') => self.parse_quoted_label(),
            Some(c) if is_symbol_char(c) => {
                self.advance();
                let sym = char_to_symbol(c)?;
                if self.in_group {
                    // Inside <>, symbols are raw (no wrapping)
                    self.make_transition(sym)
                } else {
                    // Outside <>, wrap with buildStop pattern
                    self.build_stop_for_symbol(sym)
                }
            }
            Some(c) => Err(format!("unexpected character '{c}' at pos {}", self.pos)),
            None => Err("unexpected end of input".into()),
        }
    }

    /// Parse a quoted user label: `'label_name'`
    fn parse_quoted_label(&mut self) -> Result<(usize, usize), String> {
        self.expect('\'')?;
        let start_pos = self.pos;
        let mut name = String::new();
        while self.peek() != Some('\'') && self.peek().is_some() {
            name.push(self.advance());
        }
        if self.peek().is_none() {
            return Err(format!(
                "unterminated quoted label starting at pos {}",
                start_pos
            ));
        }
        self.expect('\'')?;

        // Look up in registry
        let (id, _pos) = self
            .labels
            .lookup(&name)
            .ok_or_else(|| format!("unknown user label '{}' at pos {}", name, start_pos))?;

        let sym = LPESymbol::UserLabel(id);
        if self.in_group {
            self.make_transition(sym)
        } else {
            // Custom labels go through buildStop with the label as a custom slot
            self.build_stop_custom(sym)
        }
    }

    /// Build the C++ `buildStop` pattern for a built-in symbol.
    ///
    /// For a position-0 label (event): `sym [^STOP] [any]* STOP`
    /// For a position-1 label (scatter): `[^STOP] sym [any]* STOP`
    /// For wildcard `.`: `[^STOP] [^STOP] [any]* STOP`
    fn build_stop_for_symbol(&mut self, sym: LPESymbol) -> Result<(usize, usize), String> {
        if sym == LPESymbol::Any {
            // Wildcard: both event and scatter are wildcards
            let cat_s = self.nfa.add_state();
            let (w1_s, w1_e) = self.make_transition(LPESymbol::Any)?;
            let (w2_s, w2_e) = self.make_transition(LPESymbol::Any)?;
            let (tail_s, tail_e) = self.build_stop_tail()?;
            self.nfa.add_epsilon(cat_s, w1_s);
            self.nfa.add_epsilon(w1_e, w2_s);
            self.nfa.add_epsilon(w2_e, tail_s);
            return Ok((cat_s, tail_e));
        }

        let pos = builtin_label_pos(&sym);
        let cat_s = self.nfa.add_state();

        match pos {
            Some(0) => {
                // Event position: sym + [^STOP] + tail
                let (sym_s, sym_e) = self.make_transition(sym)?;
                let (w_s, w_e) = self.make_transition(LPESymbol::Any)?;
                let (tail_s, tail_e) = self.build_stop_tail()?;
                self.nfa.add_epsilon(cat_s, sym_s);
                self.nfa.add_epsilon(sym_e, w_s);
                self.nfa.add_epsilon(w_e, tail_s);
                Ok((cat_s, tail_e))
            }
            Some(1) => {
                // Scatter position: [^STOP] + sym + tail
                let (w_s, w_e) = self.make_transition(LPESymbol::Any)?;
                let (sym_s, sym_e) = self.make_transition(sym)?;
                let (tail_s, tail_e) = self.build_stop_tail()?;
                self.nfa.add_epsilon(cat_s, w_s);
                self.nfa.add_epsilon(w_e, sym_s);
                self.nfa.add_epsilon(sym_e, tail_s);
                Ok((cat_s, tail_e))
            }
            _ => {
                // Unknown position or Stop itself: plain transition
                self.make_transition(sym)
            }
        }
    }

    /// Build buildStop for a custom (user) label.
    /// Pattern: `[^STOP] [^STOP] sym [any]* STOP`
    fn build_stop_custom(&mut self, sym: LPESymbol) -> Result<(usize, usize), String> {
        let cat_s = self.nfa.add_state();
        let (w1_s, w1_e) = self.make_transition(LPESymbol::Any)?;
        let (w2_s, w2_e) = self.make_transition(LPESymbol::Any)?;
        let (sym_s, sym_e) = self.make_transition(sym)?;
        let (tail_s, tail_e) = self.build_stop_tail()?;
        self.nfa.add_epsilon(cat_s, w1_s);
        self.nfa.add_epsilon(w1_e, w2_s);
        self.nfa.add_epsilon(w2_e, sym_s);
        self.nfa.add_epsilon(sym_e, tail_s);
        Ok((cat_s, tail_e))
    }

    /// Build the `[any]* STOP` tail part of buildStop.
    ///
    /// C++ uses `Repeat(Wildexp(m_basic_labels))` which matches only custom labels
    /// (anything not in the basic set). In our simplified model, we use `[^STOP]*`
    /// (Any*) which matches any non-STOP event, then require STOP.
    fn build_stop_tail(&mut self) -> Result<(usize, usize), String> {
        // [^STOP]* = Any*
        let repeat_s = self.nfa.add_state();
        let repeat_mid = self.nfa.add_state();
        let (any_s, any_e) = self.make_transition(LPESymbol::Any)?;
        self.nfa.add_epsilon(repeat_s, any_s);
        self.nfa.add_epsilon(any_e, any_s); // loop
        self.nfa.add_epsilon(any_e, repeat_mid); // exit
        self.nfa.add_epsilon(repeat_s, repeat_mid); // zero

        // STOP
        let (stop_s, stop_e) = self.make_transition(LPESymbol::Stop)?;
        self.nfa.add_epsilon(repeat_mid, stop_s);

        Ok((repeat_s, stop_e))
    }

    /// Parse bounded repetition: `{n}`, `{n,}`, `{n,m}`.
    fn parse_bounded_rep(
        &mut self,
        atom_s: usize,
        atom_e: usize,
    ) -> Result<(usize, usize), String> {
        self.expect('{')?;

        // Parse min count
        let n = self.parse_uint()?;

        let (has_comma, m) = if self.peek() == Some(',') {
            self.advance();
            if self.peek() == Some('}') {
                (true, None) // {n,} = n or more
            } else {
                (true, Some(self.parse_uint()?)) // {n,m}
            }
        } else {
            (false, None) // {n} = exactly n
        };
        self.expect('}')?;

        if !has_comma {
            // {n} = exactly n repetitions
            return self.repeat_fragment(atom_s, atom_e, n, n);
        }

        match m {
            Some(max) => {
                if max < n {
                    return Err(format!("invalid repetition: {{{},{}}}", n, max));
                }
                self.repeat_fragment(atom_s, atom_e, n, max)
            }
            None => {
                // {n,} = at least n
                if n == 0 {
                    let q1 = self.nfa.add_state();
                    let q2 = self.nfa.add_state();
                    self.nfa.add_epsilon(q1, atom_s);
                    self.nfa.add_epsilon(atom_e, atom_s);
                    self.nfa.add_epsilon(atom_e, q2);
                    self.nfa.add_epsilon(q1, q2);
                    return Ok((q1, q2));
                }
                let chain_s = atom_s;
                let mut chain_e = atom_e;
                for _ in 1..n {
                    let copy = self.copy_fragment(atom_s, atom_e);
                    self.nfa.add_epsilon(chain_e, copy.0);
                    chain_e = copy.1;
                }
                let last_copy = self.copy_fragment(atom_s, atom_e);
                let q2 = self.nfa.add_state();
                self.nfa.add_epsilon(chain_e, last_copy.0);
                self.nfa.add_epsilon(last_copy.1, last_copy.0); // loop
                self.nfa.add_epsilon(last_copy.1, q2);
                self.nfa.add_epsilon(chain_e, q2); // can skip extra
                Ok((chain_s, q2))
            }
        }
    }

    /// Parse an unsigned integer from the input.
    fn parse_uint(&mut self) -> Result<usize, String> {
        let start = self.pos;
        while self.peek().map_or(false, |c| c.is_ascii_digit()) {
            self.advance();
        }
        if self.pos == start {
            return Err(format!("expected integer at pos {}", self.pos));
        }
        let s: String = self.chars[start..self.pos].iter().collect();
        s.parse::<usize>()
            .map_err(|e| format!("invalid integer '{}': {}", s, e))
    }

    /// Build NFA for repeating a fragment exactly `min..=max` times.
    fn repeat_fragment(
        &mut self,
        atom_s: usize,
        atom_e: usize,
        min: usize,
        max: usize,
    ) -> Result<(usize, usize), String> {
        if min == 0 && max == 0 {
            let s = self.nfa.add_state();
            return Ok((s, s));
        }

        let mut chain_s = atom_s;
        let mut chain_e = atom_e;

        // Add (min-1) more required copies
        for _ in 1..min {
            let copy = self.copy_fragment(atom_s, atom_e);
            self.nfa.add_epsilon(chain_e, copy.0);
            chain_e = copy.1;
        }

        if min == 0 {
            let skip_s = self.nfa.add_state();
            let skip_e = self.nfa.add_state();
            self.nfa.add_epsilon(skip_s, atom_s);
            self.nfa.add_epsilon(atom_e, skip_e);
            self.nfa.add_epsilon(skip_s, skip_e);
            chain_s = skip_s;
            chain_e = skip_e;
            for _ in 1..max {
                let copy = self.copy_fragment(atom_s, atom_e);
                let opt_s = self.nfa.add_state();
                let opt_e = self.nfa.add_state();
                self.nfa.add_epsilon(opt_s, copy.0);
                self.nfa.add_epsilon(copy.1, opt_e);
                self.nfa.add_epsilon(opt_s, opt_e);
                self.nfa.add_epsilon(chain_e, opt_s);
                chain_e = opt_e;
            }
        } else {
            for _ in min..max {
                let copy = self.copy_fragment(atom_s, atom_e);
                let opt_s = self.nfa.add_state();
                let opt_e = self.nfa.add_state();
                self.nfa.add_epsilon(opt_s, copy.0);
                self.nfa.add_epsilon(copy.1, opt_e);
                self.nfa.add_epsilon(opt_s, opt_e);
                self.nfa.add_epsilon(chain_e, opt_s);
                chain_e = opt_e;
            }
        }

        Ok((chain_s, chain_e))
    }

    /// Copy an NFA fragment by duplicating all states and transitions.
    fn copy_fragment(&mut self, atom_s: usize, atom_e: usize) -> (usize, usize) {
        let frag_states: Vec<usize> = (atom_s..=atom_e).collect();
        let mut remap = std::collections::HashMap::new();

        for &sid in &frag_states {
            let new_sid = self.nfa.add_state();
            remap.insert(sid, new_sid);
        }

        for &sid in &frag_states {
            let new_sid = remap[&sid];
            let transitions = self.nfa.states[sid].transitions.clone();
            for (sym, target) in transitions {
                let new_target = remap.get(&target).copied().unwrap_or(target);
                self.nfa.states[new_sid].transitions.push((sym, new_target));
            }
        }

        (remap[&atom_s], remap[&atom_e])
    }

    // '[' '^'? symbol+ ']'
    fn parse_class(&mut self) -> Result<(usize, usize), String> {
        self.expect('[')?;
        let negate = self.peek() == Some('^');
        if negate {
            self.advance();
        }

        let mut syms = Vec::new();
        while self.peek() != Some(']') && self.peek().is_some() {
            if self.peek() == Some('\'') {
                // Quoted user label in character class
                self.expect('\'')?;
                let mut name = String::new();
                while self.peek() != Some('\'') && self.peek().is_some() {
                    name.push(self.advance());
                }
                self.expect('\'')?;
                let (id, _) = self
                    .labels
                    .lookup(&name)
                    .ok_or_else(|| format!("unknown user label '{}' in character class", name))?;
                syms.push(LPESymbol::UserLabel(id));
            } else {
                let c = self.advance();
                syms.push(char_to_symbol(c)?);
            }
        }
        self.expect(']')?;

        // For negated classes, determine position from ORIGINAL symbols
        // (before negation), matching C++ parseNegor behavior.
        let neg_pos = if negate {
            let positions: Vec<Option<u8>> = syms.iter().map(|s| builtin_label_pos(s)).collect();
            if positions.iter().all(|p| *p == Some(0)) {
                Some(0u8)
            } else if positions.iter().all(|p| *p == Some(1)) {
                Some(1u8)
            } else {
                None // mixed or custom
            }
        } else {
            None
        };

        if negate {
            // Negated class: everything except listed symbols (and always exclude Stop)
            let mut all = vec![
                LPESymbol::Camera,
                LPESymbol::Light,
                LPESymbol::Background,
                LPESymbol::Volume,
                LPESymbol::Object,
                LPESymbol::Scattering(ScatteringKind::Diffuse),
                LPESymbol::Scattering(ScatteringKind::Glossy),
                LPESymbol::Scattering(ScatteringKind::Singular),
                LPESymbol::Scattering(ScatteringKind::Straight),
            ];
            // Add user labels to the negation universe
            for (_name, id, _pos) in self.labels.iter() {
                all.push(LPESymbol::UserLabel(id));
            }
            syms = all.iter().filter(|s| !syms.contains(s)).copied().collect();
        }

        if self.in_group {
            // Inside <>, return raw class
            let start = self.nfa.add_state();
            let end = self.nfa.add_state();
            for sym in syms {
                self.nfa.add_transition(start, sym, end);
            }
            Ok((start, end))
        } else {
            // Outside <>, wrap with buildStop.
            let class_s = self.nfa.add_state();
            let class_e = self.nfa.add_state();
            for sym in &syms {
                self.nfa.add_transition(class_s, *sym, class_e);
            }

            // Determine position: for negated classes, use the position of the
            // ORIGINAL negated symbols (C++ parseNegor). For positive classes,
            // check the actual symbols.
            let (all_pos0, all_pos1) = if let Some(np) = neg_pos {
                // Negated class: position is from the original symbols.
                // If originals were pos 0, the negated result acts at pos 0.
                // If originals were pos 1, the negated result acts at pos 1.
                (np == 0, np == 1)
            } else {
                let positions: Vec<Option<u8>> =
                    syms.iter().map(|s| builtin_label_pos(s)).collect();
                (
                    positions.iter().all(|p| *p == Some(0)),
                    positions.iter().all(|p| *p == Some(1)),
                )
            };

            let outer_s = self.nfa.add_state();

            if all_pos0 {
                // All event-type: [class] [^STOP] [any]* STOP
                let (w_s, w_e) = self.make_transition(LPESymbol::Any)?;
                let (tail_s, tail_e) = self.build_stop_tail()?;
                self.nfa.add_epsilon(outer_s, class_s);
                self.nfa.add_epsilon(class_e, w_s);
                self.nfa.add_epsilon(w_e, tail_s);
                Ok((outer_s, tail_e))
            } else if all_pos1 {
                // All scatter-type: [^STOP] [class] [any]* STOP
                let (w_s, w_e) = self.make_transition(LPESymbol::Any)?;
                let (tail_s, tail_e) = self.build_stop_tail()?;
                self.nfa.add_epsilon(outer_s, w_s);
                self.nfa.add_epsilon(w_e, class_s);
                self.nfa.add_epsilon(class_e, tail_s);
                Ok((outer_s, tail_e))
            } else {
                // Mixed or custom: [^STOP] [^STOP] [class] [any]* STOP
                let (w1_s, w1_e) = self.make_transition(LPESymbol::Any)?;
                let (w2_s, w2_e) = self.make_transition(LPESymbol::Any)?;
                let (tail_s, tail_e) = self.build_stop_tail()?;
                self.nfa.add_epsilon(outer_s, w1_s);
                self.nfa.add_epsilon(w1_e, w2_s);
                self.nfa.add_epsilon(w2_e, class_s);
                self.nfa.add_epsilon(class_e, tail_s);
                Ok((outer_s, tail_e))
            }
        }
    }

    // '<' (direction? scattering? custom*) '>'
    fn parse_compound(&mut self) -> Result<(usize, usize), String> {
        self.expect('<')?;
        let was_in_group = self.in_group;
        self.in_group = true;

        let mut items: Vec<LPESymbol> = Vec::new();
        while self.peek() != Some('>') && self.peek().is_some() {
            if self.peek() == Some('\'') {
                // User label in group
                self.expect('\'')?;
                let mut name = String::new();
                while self.peek() != Some('\'') && self.peek().is_some() {
                    name.push(self.advance());
                }
                self.expect('\'')?;
                let (id, _) = self
                    .labels
                    .lookup(&name)
                    .ok_or_else(|| format!("unknown user label '{}' in group", name))?;
                items.push(LPESymbol::UserLabel(id));
            } else if self.peek() == Some('.') {
                self.advance();
                items.push(LPESymbol::Any);
            } else {
                let c = self.advance();
                match c {
                    'T' | 'R' | 'D' | 'G' | 'S' | 'W' | 'C' | 'L' | 'B' | 'V' | 'O' => {
                        items.push(char_to_compound_symbol(c)?);
                    }
                    _ => {
                        self.in_group = was_in_group;
                        return Err(format!(
                            "unexpected '{c}' in <> compound at pos {}",
                            self.pos
                        ));
                    }
                }
            }
        }
        self.expect('>')?;
        self.in_group = was_in_group;

        if items.is_empty() {
            return Err("empty <> compound".into());
        }

        // Build the group: slot 0 = event, slot 1 = scatter, rest = custom.
        // Fill missing slots with Any wildcard, then wrap with buildStop.
        let mut basics: [Option<LPESymbol>; 2] = [None, None];
        let mut customs: Vec<LPESymbol> = Vec::new();
        let mut basic_idx = 0;

        for sym in &items {
            if basic_idx < 2 {
                basics[basic_idx] = Some(*sym);
                basic_idx += 1;
            } else {
                customs.push(*sym);
            }
        }

        // Fill unfilled slots with Any
        for slot in basics.iter_mut() {
            if slot.is_none() {
                *slot = Some(LPESymbol::Any);
            }
        }

        // Build: basics[0] basics[1] customs* [any]* STOP
        let cat_s = self.nfa.add_state();
        let (s0, e0) = self.make_transition(basics[0].unwrap())?;
        let (s1, e1) = self.make_transition(basics[1].unwrap())?;
        self.nfa.add_epsilon(cat_s, s0);
        self.nfa.add_epsilon(e0, s1);

        let mut prev_e = e1;
        for csym in &customs {
            let (cs, ce) = self.make_transition(*csym)?;
            self.nfa.add_epsilon(prev_e, cs);
            prev_e = ce;
        }

        // Add [any]* STOP tail (only if fewer than 5 custom labels, matching C++)
        if customs.len() < 5 {
            let (tail_s, tail_e) = self.build_stop_tail()?;
            self.nfa.add_epsilon(prev_e, tail_s);
            Ok((cat_s, tail_e))
        } else {
            let (stop_s, stop_e) = self.make_transition(LPESymbol::Stop)?;
            self.nfa.add_epsilon(prev_e, stop_s);
            Ok((cat_s, stop_e))
        }
    }

    /// Create an NFA fragment: start --sym--> end
    fn make_transition(&mut self, sym: LPESymbol) -> Result<(usize, usize), String> {
        let s = self.nfa.add_state();
        let e = self.nfa.add_state();
        self.nfa.add_transition(s, sym, e);
        Ok((s, e))
    }
}

fn is_symbol_char(c: char) -> bool {
    matches!(c, 'C' | 'L' | 'D' | 'G' | 'S' | 'B' | 'V' | 'O' | 'W' | '.')
}

fn char_to_symbol(c: char) -> Result<LPESymbol, String> {
    match c {
        'C' => Ok(LPESymbol::Camera),
        'L' => Ok(LPESymbol::Light),
        'B' => Ok(LPESymbol::Background),
        'V' => Ok(LPESymbol::Volume),
        'O' => Ok(LPESymbol::Object),
        'D' => Ok(LPESymbol::Scattering(ScatteringKind::Diffuse)),
        'G' => Ok(LPESymbol::Scattering(ScatteringKind::Glossy)),
        'S' => Ok(LPESymbol::Scattering(ScatteringKind::Singular)),
        'W' => Ok(LPESymbol::Scattering(ScatteringKind::Straight)),
        '.' => Ok(LPESymbol::Any),
        _ => Err(format!("unknown LPE symbol '{c}'")),
    }
}

/// Map a character inside `<>` to the appropriate symbol.
/// T/R map to Direction, D/G/S/W to Scattering, C/L/B/V/O to terminals.
fn char_to_compound_symbol(c: char) -> Result<LPESymbol, String> {
    match c {
        'T' => Ok(LPESymbol::Direction(DirectionKind::Transmit)),
        'R' => Ok(LPESymbol::Direction(DirectionKind::Reflect)),
        'D' => Ok(LPESymbol::Scattering(ScatteringKind::Diffuse)),
        'G' => Ok(LPESymbol::Scattering(ScatteringKind::Glossy)),
        'S' => Ok(LPESymbol::Scattering(ScatteringKind::Singular)),
        'W' => Ok(LPESymbol::Scattering(ScatteringKind::Straight)),
        'C' => Ok(LPESymbol::Camera),
        'L' => Ok(LPESymbol::Light),
        'B' => Ok(LPESymbol::Background),
        'V' => Ok(LPESymbol::Volume),
        'O' => Ok(LPESymbol::Object),
        '.' => Ok(LPESymbol::Any),
        _ => Err(format!("unknown compound symbol '{c}'")),
    }
}

// ---------------------------------------------------------------------------
// NFA -> DFA powerset construction
// ---------------------------------------------------------------------------

/// Convert an NFA to a DFA using the powerset (subset) construction.
pub fn nfa_to_dfa(nfa: &LPENFA) -> LPEDFA {
    let total = nfa.total_events;
    let mut dfa_states: Vec<DFAState> = Vec::new();
    let mut state_map: HashMap<Vec<usize>, usize> = HashMap::new();
    let mut worklist: VecDeque<usize> = VecDeque::new();

    // Parallel vec storing NFA state sets for each DFA state
    let mut nfa_sets: Vec<HashSet<usize>> = Vec::new();

    // Initial DFA state
    let initial_nfa = epsilon_closure(nfa, &HashSet::from([nfa.start]));
    let initial_key = sorted_key(&initial_nfa);
    let is_accept = initial_nfa.iter().any(|&s| nfa.states[s].is_accept);

    dfa_states.push(DFAState {
        transitions: vec![DEAD_STATE; total],
        is_accept,
    });
    nfa_sets.push(initial_nfa);
    state_map.insert(initial_key, 0);
    worklist.push_back(0);

    while let Some(dfa_idx) = worklist.pop_front() {
        let current_nfa = nfa_sets[dfa_idx].clone();

        for event_ord in 0..total {
            // Compute move: all NFA states reachable on this event
            let mut targets = HashSet::new();
            for &ns in &current_nfa {
                for &(ref sym, target) in &nfa.states[ns].transitions {
                    if let Some(sym) = sym {
                        if sym.matches_ordinal(event_ord, total) {
                            targets.insert(target);
                        }
                    }
                }
            }

            if targets.is_empty() {
                continue; // transition stays DEAD_STATE
            }

            let closed = epsilon_closure(nfa, &targets);
            let key = sorted_key(&closed);

            let target_dfa = if let Some(&existing) = state_map.get(&key) {
                existing
            } else {
                let id = dfa_states.len();
                let is_accept = closed.iter().any(|&s| nfa.states[s].is_accept);
                dfa_states.push(DFAState {
                    transitions: vec![DEAD_STATE; total],
                    is_accept,
                });
                nfa_sets.push(closed);
                state_map.insert(key, id);
                worklist.push_back(id);
                id
            };

            dfa_states[dfa_idx].transitions[event_ord] = target_dfa;
        }
    }

    LPEDFA {
        states: dfa_states,
        custom_labels: HashMap::new(),
        total_events: total,
    }
}

fn epsilon_closure(nfa: &LPENFA, states: &HashSet<usize>) -> HashSet<usize> {
    let mut result = states.clone();
    let mut stack: Vec<usize> = states.iter().copied().collect();

    while let Some(s) = stack.pop() {
        for &(ref sym, target) in &nfa.states[s].transitions {
            if sym.is_none() && result.insert(target) {
                stack.push(target);
            }
        }
    }

    result
}

fn sorted_key(set: &HashSet<usize>) -> Vec<usize> {
    let mut v: Vec<usize> = set.iter().copied().collect();
    v.sort();
    v
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: scatter event shorthand
    fn rd() -> LPEEvent {
        LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Reflect)
    }
    fn rg() -> LPEEvent {
        LPEEvent::scatter(ScatteringKind::Glossy, DirectionKind::Reflect)
    }
    fn rs() -> LPEEvent {
        LPEEvent::scatter(ScatteringKind::Singular, DirectionKind::Reflect)
    }
    fn td() -> LPEEvent {
        LPEEvent::scatter(ScatteringKind::Diffuse, DirectionKind::Transmit)
    }
    fn tg() -> LPEEvent {
        LPEEvent::scatter(ScatteringKind::Glossy, DirectionKind::Transmit)
    }
    fn ts() -> LPEEvent {
        LPEEvent::scatter(ScatteringKind::Singular, DirectionKind::Transmit)
    }
    fn stop() -> LPEEvent {
        LPEEvent::Stop
    }

    // With STOP semantics, each symbol outside <> is wrapped as:
    // event scatter [any]* STOP
    //
    // For 'C': Camera [^STOP] [^STOP]* STOP
    // For 'D': [^STOP] Diffuse [^STOP]* STOP
    // For 'L': Light [^STOP] [^STOP]* STOP
    // For '.': [^STOP] [^STOP] [^STOP]* STOP
    //
    // So "CDL" = Camera wildcard Stop, wildcard Diffuse Stop, Light wildcard Stop

    #[test]
    fn test_simple_cdl() {
        let dfa = compile_lpe("CDL").unwrap();
        // CDL with STOP: C matches Camera, [^STOP] matches any scatter, STOP
        // then [^STOP] matches any event, D matches Diffuse, STOP
        // then L matches Light, [^STOP] matches any scatter, STOP
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            td(),
            stop(),
            rg(),
            td(),
            stop(),
            LPEEvent::Light,
            rs(),
            stop(),
        ]));
        // rg is Glossy, not Diffuse -- middle bounce scatter slot must be Diffuse
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(), // scatter slot = Glossy, not Diffuse
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_directed_scatter() {
        // <TD> inside group: T(direction) D(scatter) [any]* STOP
        let dfa = compile_lpe("C<TD>L").unwrap();
        // <TD> => Direction(Transmit) at slot0, Scattering(Diffuse) at slot1, then tail
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            td(),
            td(),
            stop(), // slot0=td matches Transmit direction, slot1=td matches Diffuse
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_star() {
        let dfa = compile_lpe("CD*L").unwrap();
        // C then zero or more D bounces then L, all with STOP framing
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_plus() {
        let dfa = compile_lpe("CD+L").unwrap();
        // Must have at least one D bounce
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_optional() {
        let dfa = compile_lpe("CD?L").unwrap();
        // Zero or one D
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        // Two D bounces should NOT match
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_character_class() {
        let dfa = compile_lpe("C[DG]+L").unwrap();
        // [DG] is scatter position, so: [^STOP] [DG] [any]* STOP
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        // Must have at least one [DG] bounce
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        // Singular not in [DG]
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rs(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_negated_class() {
        // [^D] = anything except Diffuse (at scatter position)
        let dfa = compile_lpe("C[^D]L").unwrap();
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(), // Diffuse -- should NOT match
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(), // Glossy -- should match
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rs(),
            stop(), // Singular -- should match
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_alternation() {
        let dfa = compile_lpe("CDL|CGL").unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rs(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_grouping() {
        let dfa = compile_lpe("C(D|G)L").unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rs(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_wildcard_any() {
        // `.` = [^STOP][^STOP] [any]* STOP -- matches any single bounce
        let dfa = compile_lpe("C.L").unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rs(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            td(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_dot_star() {
        let dfa = compile_lpe("C.*L").unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            rd(),
            rs(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_incremental_step() {
        let dfa = compile_lpe("CDL").unwrap();
        let mut state = dfa.initial_state();
        assert!(!dfa.is_accept(state));
        assert!(!dfa.is_dead(state));

        // Camera bounce: Camera, [^STOP], STOP
        state = dfa.step(state, LPEEvent::Camera);
        assert!(!dfa.is_dead(state));
        state = dfa.step(state, rd());
        assert!(!dfa.is_dead(state));
        state = dfa.step(state, stop());
        assert!(!dfa.is_accept(state));

        // Diffuse bounce: [^STOP], Diffuse, STOP
        state = dfa.step(state, rd());
        state = dfa.step(state, rd());
        state = dfa.step(state, stop());
        assert!(!dfa.is_accept(state));

        // Light bounce: Light, [^STOP], STOP
        state = dfa.step(state, LPEEvent::Light);
        state = dfa.step(state, rd());
        state = dfa.step(state, stop());
        assert!(dfa.is_accept(state));
    }

    #[test]
    fn test_dead_state() {
        let dfa = compile_lpe("CDL").unwrap();
        let mut state = dfa.initial_state();
        // Light as first event -- should eventually die because pattern expects Camera first
        state = dfa.step(state, LPEEvent::Light);
        // Light doesn't match Camera at position 0, so it may survive as [^STOP] wildcard
        // but after STOP, the next bounce won't work
        // Let's just verify the DFA handles garbage input
        for _ in 0..10 {
            state = dfa.step(state, LPEEvent::Light);
            if dfa.is_dead(state) {
                break;
            }
        }
        // After enough wrong events, should be dead
        assert!(dfa.is_dead(state));

        // Once dead, stays dead
        state = dfa.step(state, LPEEvent::Camera);
        assert!(dfa.is_dead(state));
    }

    #[test]
    fn test_direction_only() {
        // <T> = Direction(Transmit) at slot0, Any at slot1, tail
        let dfa = compile_lpe("C<T>L").unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            td(),
            td(),
            stop(), // Transmit direction
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            tg(),
            tg(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            ts(),
            ts(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        // Reflect direction should not match <T>
        assert!(!dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(), // Reflect, not Transmit
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_standard_beauty() {
        let dfa = compile_lpe("C[DGSW]*[LO]").unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            rd(),
            rs(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            stop(),
            LPEEvent::Object,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_event_ordinals() {
        for ord in 0..NUM_BASE_EVENTS {
            let event = LPEEvent::from_ordinal(ord);
            assert_eq!(event.ordinal(), ord, "roundtrip failed for ordinal {ord}");
        }
    }

    #[test]
    fn test_any_does_not_match_stop() {
        // Verify that Any (`.`) does NOT match Stop
        let sym = LPESymbol::Any;
        assert!(!sym.matches_ordinal(5, NUM_BASE_EVENTS)); // 5 = Stop ordinal
        assert!(sym.matches_ordinal(0, NUM_BASE_EVENTS)); // Camera
        assert!(sym.matches_ordinal(6, NUM_BASE_EVENTS)); // Scatter
    }

    #[test]
    fn test_stop_prevents_wildcard_eating() {
        // Key STOP semantic test: `.*` must not eat STOP markers
        let dfa = compile_lpe("C.*L").unwrap();
        // Without STOP markers between bounces, wildcards would eat everything.
        // With STOP, each bounce is properly delimited.
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rg(),
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    // -----------------------------------------------------------------------
    // C4: User-defined label tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_user_label_registration() {
        let mut reg = LabelRegistry::new();
        let coat_id = reg.register_custom("coat");
        let metal_id = reg.register_scatter("metallic");
        assert_eq!(coat_id, 0);
        assert_eq!(metal_id, 1);
        assert_eq!(reg.total_events(), NUM_BASE_EVENTS + 2);
        assert_eq!(reg.lookup("coat"), Some((0, LabelPosition::Custom)));
        assert_eq!(reg.lookup("metallic"), Some((1, LabelPosition::Scatter)));
        assert_eq!(reg.lookup("nonexistent"), None);
    }

    #[test]
    fn test_user_label_in_lpe() {
        let mut reg = LabelRegistry::new();
        let coat_id = reg.register_custom("coat");
        let coat_event = LPEEvent::UserDefined(coat_id);

        // 'coat' as custom label: [^STOP] [^STOP] coat [any]* STOP
        let dfa = compile_lpe_with_labels("C'coat'L", &reg).unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            coat_event,
            stop(), // custom label bounce
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_user_label_in_group() {
        let mut reg = LabelRegistry::new();
        let coat_id = reg.register_custom("coat");
        let coat_event = LPEEvent::UserDefined(coat_id);

        // <D.'coat'> = Scattering(Diffuse) at slot0, Any at slot1, coat at custom, tail
        let dfa = compile_lpe_with_labels("C<D.'coat'>L", &reg).unwrap();
        assert!(dfa.matches(&[
            LPEEvent::Camera,
            rd(),
            stop(),
            rd(),
            rd(),
            coat_event,
            stop(),
            LPEEvent::Light,
            rd(),
            stop(),
        ]));
    }

    #[test]
    fn test_unknown_user_label_error() {
        let reg = LabelRegistry::new();
        let result = compile_lpe_with_labels("C'unknown'L", &reg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown user label"));
    }
}
