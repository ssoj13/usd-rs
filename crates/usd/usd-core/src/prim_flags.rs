//! Prim flags and predicates for filtering prims.
//!
//! Prim flags describe boolean properties of prims, such as whether they
//! are active, defined, loaded, etc. Predicates combine flags for filtering.
//!
//! # Examples
//!
//! ```ignore
//! // Get only loaded model children.
//! prim.get_filtered_children(UsdPrimIsModel.and(UsdPrimIsLoaded))
//!
//! // Get all deactivated or undefined children.
//! prim.get_filtered_children(UsdPrimIsActive.not().or(UsdPrimIsDefined.not()))
//! ```

use std::hash::{Hash, Hasher};

// ============================================================================
// Prim Flag Enum
// ============================================================================

/// Individual prim flags that can be tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum PrimFlag {
    /// Prim is active (not deactivated).
    Active = 0,
    /// Prim is loaded (payload is loaded if present).
    Loaded = 1,
    /// Prim is a model (has model kind).
    Model = 2,
    /// Prim is a group (has group model kind).
    Group = 3,
    /// Prim is a component model.
    Component = 4,
    /// Prim is abstract (cannot be instantiated).
    Abstract = 5,
    /// Prim is defined (has a def, not just over).
    Defined = 6,
    /// Prim has a defining specifier (def or class).
    HasDefiningSpecifier = 7,
    /// Prim is an instance (part of an instance).
    Instance = 8,
    // Internal flags
    /// Prim has payloads.
    HasPayload = 9,
    /// Prim may have opinions in clips.
    Clips = 10,
    /// Prim is dead (removed from stage).
    Dead = 11,
    /// Prim is a prototype.
    Prototype = 12,
    /// Prim is an instance proxy.
    InstanceProxy = 13,
    /// Prim is the pseudo-root.
    PseudoRoot = 14,
}

impl PrimFlag {
    /// Total number of flags.
    pub const NUM_FLAGS: usize = 15;

    /// Convert to bit mask.
    pub const fn to_mask(self) -> u32 {
        1u32 << (self as u8)
    }
}

// ============================================================================
// PrimFlagBits - Bitset for flags
// ============================================================================

/// Bitset storing prim flags.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrimFlagBits(u32);

impl PrimFlagBits {
    /// Creates empty flag bits.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Creates flag bits with a single flag set.
    pub const fn from_flag(flag: PrimFlag) -> Self {
        Self(flag.to_mask())
    }

    /// Sets a flag to the given value.
    pub fn set(&mut self, flag: PrimFlag, value: bool) {
        if value {
            self.0 |= flag.to_mask();
        } else {
            self.0 &= !flag.to_mask();
        }
    }

    /// Returns whether a flag is set.
    pub const fn get(&self, flag: PrimFlag) -> bool {
        (self.0 & flag.to_mask()) != 0
    }

    /// Returns raw bits.
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Creates from raw bits.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Bitwise AND.
    pub const fn and(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Bitwise OR.
    pub const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns true if all bits in `other` are set in `self`.
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl Hash for PrimFlagBits {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

// ============================================================================
// Term - A single flag term (possibly negated)
// ============================================================================

/// A single predicate term consisting of a flag and whether it's negated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Term {
    /// The flag being tested.
    pub flag: PrimFlag,
    /// Whether the term is negated.
    pub negated: bool,
}

impl Term {
    /// Creates a new term.
    pub const fn new(flag: PrimFlag) -> Self {
        Self {
            flag,
            negated: false,
        }
    }

    /// Creates a negated term.
    pub const fn negated(flag: PrimFlag) -> Self {
        Self {
            flag,
            negated: true,
        }
    }

    /// Negates this term.
    pub const fn not(self) -> Self {
        Self {
            flag: self.flag,
            negated: !self.negated,
        }
    }

    /// Combines with another term into a conjunction.
    pub fn and(self, other: Term) -> PrimFlagsConjunction {
        let mut conj = PrimFlagsConjunction::tautology();
        conj.add_term(self);
        conj.add_term(other);
        conj
    }

    /// Combines with another term into a disjunction.
    pub fn or(self, other: Term) -> PrimFlagsDisjunction {
        let mut disj = PrimFlagsDisjunction::contradiction();
        disj.add_term(self);
        disj.add_term(other);
        disj
    }
}

impl From<PrimFlag> for Term {
    fn from(flag: PrimFlag) -> Self {
        Term::new(flag)
    }
}

// ============================================================================
// PrimFlagsPredicate - Base predicate
// ============================================================================

/// Predicate functor class that tests a prim's flags against desired values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrimFlagsPredicate {
    /// Mask indicating which flags are of interest.
    mask: PrimFlagBits,
    /// Desired values for prim flags.
    values: PrimFlagBits,
    /// Whether to negate the predicate's result.
    negate: bool,
}

impl PrimFlagsPredicate {
    /// Creates a tautology (always true) predicate.
    pub const fn tautology() -> Self {
        Self {
            mask: PrimFlagBits::empty(),
            values: PrimFlagBits::empty(),
            negate: false,
        }
    }

    /// Creates a contradiction (always false) predicate.
    pub const fn contradiction() -> Self {
        Self {
            mask: PrimFlagBits::empty(),
            values: PrimFlagBits::empty(),
            negate: true,
        }
    }

    /// Creates a predicate from a single flag.
    pub const fn from_flag(flag: PrimFlag) -> Self {
        Self {
            mask: PrimFlagBits::from_flag(flag),
            values: PrimFlagBits::from_flag(flag),
            negate: false,
        }
    }

    /// Creates a predicate from a term.
    pub const fn from_term(term: Term) -> Self {
        let mask = PrimFlagBits::from_flag(term.flag);
        let values = if term.negated {
            PrimFlagBits::empty()
        } else {
            mask
        };
        Self {
            mask,
            values,
            negate: false,
        }
    }

    /// Returns true if this predicate is a tautology.
    pub fn is_tautology(&self) -> bool {
        *self == Self::tautology()
    }

    /// Returns true if this predicate is a contradiction.
    pub fn is_contradiction(&self) -> bool {
        *self == Self::contradiction()
    }

    /// Negates this predicate.
    pub fn negate(&mut self) {
        self.negate = !self.negate;
    }

    /// Returns a negated copy of this predicate.
    pub fn negated(&self) -> Self {
        Self {
            mask: self.mask,
            values: self.values,
            negate: !self.negate,
        }
    }

    /// Set flag to indicate whether prim traversal functions should traverse
    /// beneath instances and return descendants as instance proxy prims.
    pub fn traverse_instance_proxies(&mut self, traverse: bool) {
        if traverse {
            // Don't test instance proxy flag (allow both values)
            self.mask.set(PrimFlag::InstanceProxy, false);
            self.values.set(PrimFlag::InstanceProxy, true);
        } else {
            // Test instance proxy flag, require it to be false
            self.mask.set(PrimFlag::InstanceProxy, true);
            self.values.set(PrimFlag::InstanceProxy, false);
        }
    }

    /// Returns true if this predicate was explicitly set to include instance proxies.
    pub fn include_instance_proxies_in_traversal(&self) -> bool {
        !self.mask.get(PrimFlag::InstanceProxy) && self.values.get(PrimFlag::InstanceProxy)
    }

    /// Evaluates the predicate against the given flags.
    pub fn eval(&self, flags: PrimFlagBits) -> bool {
        let masked_flags = flags.and(self.mask);
        let masked_values = self.values.and(self.mask);
        (masked_flags == masked_values) ^ self.negate
    }

    /// Evaluates the predicate with explicit instance proxy state.
    pub fn eval_with_instance_proxy(
        &self,
        mut flags: PrimFlagBits,
        is_instance_proxy: bool,
    ) -> bool {
        flags.set(PrimFlag::InstanceProxy, is_instance_proxy);
        self.eval(flags)
    }

    /// Legacy compatibility: evaluates the predicate against PrimFlags.
    pub fn matches(&self, flags: PrimFlags) -> bool {
        self.eval(flags.to_flag_bits())
    }

    /// Legacy alias for tautology (all prims pass).
    pub fn all() -> Self {
        Self::tautology()
    }

    /// Returns the mask bits.
    pub fn mask(&self) -> PrimFlagBits {
        self.mask
    }

    /// Returns the value bits.
    pub fn values(&self) -> PrimFlagBits {
        self.values
    }
}

impl Default for PrimFlagsPredicate {
    fn default() -> Self {
        Self::tautology()
    }
}

impl Hash for PrimFlagsPredicate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.mask.hash(state);
        self.values.hash(state);
        self.negate.hash(state);
    }
}

impl From<Term> for PrimFlagsPredicate {
    fn from(term: Term) -> Self {
        Self::from_term(term)
    }
}

impl From<PrimFlag> for PrimFlagsPredicate {
    fn from(flag: PrimFlag) -> Self {
        Self::from_flag(flag)
    }
}

// ============================================================================
// PrimFlagsConjunction - AND of terms
// ============================================================================

/// Conjunction of prim flag predicate terms (AND).
///
/// Usually clients will implicitly create conjunctions by using the `and` method.
/// For example:
/// ```ignore
/// // Get all loaded model children.
/// prim.get_filtered_children(UsdPrimIsModel.and(UsdPrimIsLoaded))
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrimFlagsConjunction(PrimFlagsPredicate);

impl PrimFlagsConjunction {
    /// Creates a tautology (always true) conjunction.
    pub const fn tautology() -> Self {
        Self(PrimFlagsPredicate::tautology())
    }

    /// Creates a conjunction from a term.
    pub fn from_term(term: Term) -> Self {
        let mut conj = Self::tautology();
        conj.add_term(term);
        conj
    }

    /// Adds a term to this conjunction.
    pub fn add_term(&mut self, term: Term) {
        // If this conjunction is a contradiction, do nothing.
        if self.0.is_contradiction() {
            return;
        }

        // If we don't have the bit, set it in mask and values (if needed).
        if !self.0.mask.get(term.flag) {
            self.0.mask.set(term.flag, true);
            self.0.values.set(term.flag, !term.negated);
        } else if self.0.values.get(term.flag) == term.negated {
            // If we do have the bit and the values disagree, then this entire
            // conjunction becomes a contradiction.
            self.0 = PrimFlagsPredicate::contradiction();
        }
        // If the values agree, it's redundant and we do nothing.
    }

    /// Combines with another term.
    pub fn and(mut self, term: Term) -> Self {
        self.add_term(term);
        self
    }

    /// Negates this conjunction, producing a disjunction by De Morgan's law.
    pub fn not(self) -> PrimFlagsDisjunction {
        PrimFlagsDisjunction(self.0.negated())
    }

    /// Returns the underlying predicate.
    pub fn as_predicate(&self) -> &PrimFlagsPredicate {
        &self.0
    }

    /// Converts to the underlying predicate.
    pub fn into_predicate(self) -> PrimFlagsPredicate {
        self.0
    }

    /// Set flag to indicate whether to traverse instance proxies.
    pub fn traverse_instance_proxies(mut self, traverse: bool) -> Self {
        self.0.traverse_instance_proxies(traverse);
        self
    }
}

impl Default for PrimFlagsConjunction {
    fn default() -> Self {
        Self::tautology()
    }
}

impl From<Term> for PrimFlagsConjunction {
    fn from(term: Term) -> Self {
        Self::from_term(term)
    }
}

impl From<PrimFlag> for PrimFlagsConjunction {
    fn from(flag: PrimFlag) -> Self {
        Self::from_term(Term::new(flag))
    }
}

impl From<PrimFlagsConjunction> for PrimFlagsPredicate {
    fn from(conj: PrimFlagsConjunction) -> Self {
        conj.0
    }
}

// ============================================================================
// PrimFlagsDisjunction - OR of terms
// ============================================================================

/// Disjunction of prim flag predicate terms (OR).
///
/// Usually clients will implicitly create disjunctions by using the `or` method.
/// For example:
/// ```ignore
/// // Get all deactivated or undefined children.
/// prim.get_filtered_children(UsdPrimIsActive.not().or(UsdPrimIsDefined.not()))
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrimFlagsDisjunction(PrimFlagsPredicate);

impl PrimFlagsDisjunction {
    /// Creates a contradiction (always false) disjunction.
    pub fn contradiction() -> Self {
        Self(PrimFlagsPredicate::contradiction())
    }

    /// Creates a disjunction from a term.
    pub fn from_term(term: Term) -> Self {
        let mut disj = Self::contradiction();
        disj.add_term(term);
        disj
    }

    /// Adds a term to this disjunction.
    pub fn add_term(&mut self, term: Term) {
        // If this disjunction is a tautology, do nothing.
        if self.0.is_tautology() {
            return;
        }

        // If we don't have the bit, set it in mask and values (if needed).
        // Note: For disjunction, we store negated values because the base predicate
        // is negated (De Morgan's law: A || B == !(!A && !B))
        if !self.0.mask.get(term.flag) {
            self.0.mask.set(term.flag, true);
            self.0.values.set(term.flag, term.negated);
        } else if self.0.values.get(term.flag) != term.negated {
            // If we do have the bit and the values disagree, then this entire
            // disjunction becomes a tautology.
            self.0 = PrimFlagsPredicate::tautology();
        }
        // If the values agree, it's redundant and we do nothing.
    }

    /// Combines with another term.
    pub fn or(mut self, term: Term) -> Self {
        self.add_term(term);
        self
    }

    /// Negates this disjunction, producing a conjunction by De Morgan's law.
    pub fn not(self) -> PrimFlagsConjunction {
        PrimFlagsConjunction(self.0.negated())
    }

    /// Returns the underlying predicate.
    pub fn as_predicate(&self) -> &PrimFlagsPredicate {
        &self.0
    }

    /// Converts to the underlying predicate.
    pub fn into_predicate(self) -> PrimFlagsPredicate {
        self.0
    }

    /// Set flag to indicate whether to traverse instance proxies.
    pub fn traverse_instance_proxies(mut self, traverse: bool) -> Self {
        self.0.traverse_instance_proxies(traverse);
        self
    }
}

impl Default for PrimFlagsDisjunction {
    fn default() -> Self {
        Self::contradiction()
    }
}

impl From<Term> for PrimFlagsDisjunction {
    fn from(term: Term) -> Self {
        Self::from_term(term)
    }
}

impl From<PrimFlag> for PrimFlagsDisjunction {
    fn from(flag: PrimFlag) -> Self {
        Self::from_term(Term::new(flag))
    }
}

impl From<PrimFlagsDisjunction> for PrimFlagsPredicate {
    fn from(disj: PrimFlagsDisjunction) -> Self {
        disj.0
    }
}

// ============================================================================
// Global flag constants (matching C++ UsdPrimIsActive, etc.)
// ============================================================================

/// Tests UsdPrim::is_active()
pub const USD_PRIM_IS_ACTIVE: Term = Term::new(PrimFlag::Active);

/// Tests UsdPrim::is_loaded()
pub const USD_PRIM_IS_LOADED: Term = Term::new(PrimFlag::Loaded);

/// Tests UsdPrim::is_model()
pub const USD_PRIM_IS_MODEL: Term = Term::new(PrimFlag::Model);

/// Tests UsdPrim::is_group()
pub const USD_PRIM_IS_GROUP: Term = Term::new(PrimFlag::Group);

/// Tests UsdPrim::is_abstract()
pub const USD_PRIM_IS_ABSTRACT: Term = Term::new(PrimFlag::Abstract);

/// Tests UsdPrim::is_defined()
pub const USD_PRIM_IS_DEFINED: Term = Term::new(PrimFlag::Defined);

/// Tests UsdPrim::is_instance()
pub const USD_PRIM_IS_INSTANCE: Term = Term::new(PrimFlag::Instance);

/// Tests UsdPrim::has_defining_specifier()
pub const USD_PRIM_HAS_DEFINING_SPECIFIER: Term = Term::new(PrimFlag::HasDefiningSpecifier);

// ============================================================================
// Standard predicates
// ============================================================================

/// The default predicate used for prim traversals.
///
/// This is a conjunction that includes all active, loaded, defined,
/// non-abstract prims, equivalent to:
/// ```ignore
/// UsdPrimIsActive && UsdPrimIsDefined && UsdPrimIsLoaded && !UsdPrimIsAbstract
/// ```
pub fn default_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::tautology()
        .and(USD_PRIM_IS_ACTIVE)
        .and(USD_PRIM_IS_DEFINED)
        .and(USD_PRIM_IS_LOADED)
        .and(USD_PRIM_IS_ABSTRACT.not())
}

/// Predicate that includes all prims.
pub fn all_prims_predicate() -> PrimFlagsPredicate {
    PrimFlagsPredicate::tautology()
}

/// Predicate for active prims only.
pub fn active_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_ACTIVE)
}

/// Predicate for defined prims only.
pub fn defined_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_DEFINED)
}

/// Predicate for loaded prims only.
pub fn loaded_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_LOADED)
}

/// Predicate for model prims only.
pub fn model_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_MODEL)
}

/// Predicate for group prims only.
pub fn group_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_GROUP)
}

/// Predicate for abstract prims only.
pub fn abstract_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_ABSTRACT)
}

/// Predicate for instance prims only.
pub fn instance_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(USD_PRIM_IS_INSTANCE)
}

/// Predicate for component model prims only.
pub fn component_predicate() -> PrimFlagsConjunction {
    PrimFlagsConjunction::from_term(Term::new(PrimFlag::Component))
}

// ============================================================================
// Helper function for instance proxy traversal
// ============================================================================

/// Returns a predicate that allows traversal beneath instances.
///
/// This function is used to allow prim traversal functions to traverse beneath
/// instance prims and return descendants that pass the specified predicate
/// as instance proxy prims.
///
/// # Examples
///
/// ```ignore
/// // Return all children of the specified prim.
/// // If prim is an instance, return all children as instance proxy prims.
/// prim.get_filtered_children(traverse_instance_proxies(all_prims_predicate()))
///
/// // Return children that pass the default predicate.
/// prim.get_filtered_children(traverse_instance_proxies_default())
/// ```
pub fn traverse_instance_proxies(mut predicate: PrimFlagsPredicate) -> PrimFlagsPredicate {
    predicate.traverse_instance_proxies(true);
    predicate
}

/// Returns default predicate with instance proxy traversal enabled.
pub fn traverse_instance_proxies_default() -> PrimFlagsPredicate {
    let mut pred = default_predicate().into_predicate();
    pred.traverse_instance_proxies(true);
    pred
}

// ============================================================================
// Legacy API compatibility
// ============================================================================

/// Flags describing prim properties (legacy bitflags API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PrimFlags(u32);

impl PrimFlags {
    /// Prim is active.
    pub const ACTIVE: PrimFlags = PrimFlags(PrimFlag::Active.to_mask());
    /// Prim is loaded.
    pub const LOADED: PrimFlags = PrimFlags(PrimFlag::Loaded.to_mask());
    /// Prim is a model.
    pub const MODEL: PrimFlags = PrimFlags(PrimFlag::Model.to_mask());
    /// Prim is a group.
    pub const GROUP: PrimFlags = PrimFlags(PrimFlag::Group.to_mask());
    /// Prim is abstract.
    pub const ABSTRACT: PrimFlags = PrimFlags(PrimFlag::Abstract.to_mask());
    /// Prim is defined.
    pub const DEFINED: PrimFlags = PrimFlags(PrimFlag::Defined.to_mask());
    /// Prim is an instance.
    pub const INSTANCE: PrimFlags = PrimFlags(PrimFlag::Instance.to_mask());
    /// Prim has payloads.
    pub const HAS_PAYLOAD: PrimFlags = PrimFlags(PrimFlag::HasPayload.to_mask());
    /// Prim is pseudo-root.
    pub const PSEUDO_ROOT: PrimFlags = PrimFlags(PrimFlag::PseudoRoot.to_mask());
    /// Prim has defining specifier.
    pub const HAS_DEFINING_SPECIFIER: PrimFlags =
        PrimFlags(PrimFlag::HasDefiningSpecifier.to_mask());
    /// Prim is a component.
    pub const COMPONENT: PrimFlags = PrimFlags(PrimFlag::Component.to_mask());
    /// Prim has clips.
    pub const CLIPS: PrimFlags = PrimFlags(PrimFlag::Clips.to_mask());
    /// Prim is a prototype (root of shared subtree for instancing).
    pub const PROTOTYPE: PrimFlags = PrimFlags(PrimFlag::Prototype.to_mask());
    /// Prim is an instance proxy (virtual prim representing prototype content through an instance).
    pub const INSTANCE_PROXY: PrimFlags = PrimFlags(PrimFlag::InstanceProxy.to_mask());

    /// Creates empty flags.
    pub const fn empty() -> Self {
        PrimFlags(0)
    }

    /// Returns true if flags contain all the given flags.
    pub const fn contains(&self, other: PrimFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns true if flags are empty.
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Inserts flags.
    pub fn insert(&mut self, other: PrimFlags) {
        self.0 |= other.0;
    }

    /// Removes flags.
    pub fn remove(&mut self, other: PrimFlags) {
        self.0 &= !other.0;
    }

    /// Returns raw bits.
    pub const fn bits(&self) -> u32 {
        self.0
    }

    /// Converts to PrimFlagBits.
    pub const fn to_flag_bits(&self) -> PrimFlagBits {
        PrimFlagBits(self.0)
    }
}

impl std::ops::BitOr for PrimFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        PrimFlags(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for PrimFlags {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        PrimFlags(self.0 & rhs.0)
    }
}

impl std::ops::BitOrAssign for PrimFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAndAssign for PrimFlags {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl std::ops::Not for PrimFlags {
    type Output = Self;
    fn not(self) -> Self {
        PrimFlags(!self.0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_term_negation() {
        let term = Term::new(PrimFlag::Active);
        assert!(!term.negated);

        let neg = term.not();
        assert!(neg.negated);
        assert_eq!(neg.flag, PrimFlag::Active);

        let double_neg = neg.not();
        assert!(!double_neg.negated);
    }

    #[test]
    fn test_predicate_from_term() {
        let term = Term::new(PrimFlag::Active);
        let pred = PrimFlagsPredicate::from_term(term);

        let mut flags = PrimFlagBits::empty();
        flags.set(PrimFlag::Active, true);
        assert!(pred.eval(flags));

        let empty_flags = PrimFlagBits::empty();
        assert!(!pred.eval(empty_flags));
    }

    #[test]
    fn test_predicate_from_negated_term() {
        let term = Term::negated(PrimFlag::Abstract);
        let pred = PrimFlagsPredicate::from_term(term);

        let empty_flags = PrimFlagBits::empty();
        assert!(pred.eval(empty_flags));

        let mut abstract_flags = PrimFlagBits::empty();
        abstract_flags.set(PrimFlag::Abstract, true);
        assert!(!pred.eval(abstract_flags));
    }

    #[test]
    fn test_conjunction() {
        let conj = USD_PRIM_IS_ACTIVE.and(USD_PRIM_IS_DEFINED);
        let pred = conj.into_predicate();

        let mut both_flags = PrimFlagBits::empty();
        both_flags.set(PrimFlag::Active, true);
        both_flags.set(PrimFlag::Defined, true);
        assert!(pred.eval(both_flags));

        let mut active_only = PrimFlagBits::empty();
        active_only.set(PrimFlag::Active, true);
        assert!(!pred.eval(active_only));

        let mut defined_only = PrimFlagBits::empty();
        defined_only.set(PrimFlag::Defined, true);
        assert!(!pred.eval(defined_only));
    }

    #[test]
    fn test_conjunction_with_negation() {
        // Active AND NOT Abstract
        let conj = USD_PRIM_IS_ACTIVE.and(USD_PRIM_IS_ABSTRACT.not());
        let pred = conj.into_predicate();

        let mut active_flags = PrimFlagBits::empty();
        active_flags.set(PrimFlag::Active, true);
        assert!(pred.eval(active_flags));

        let mut active_abstract = PrimFlagBits::empty();
        active_abstract.set(PrimFlag::Active, true);
        active_abstract.set(PrimFlag::Abstract, true);
        assert!(!pred.eval(active_abstract));
    }

    #[test]
    fn test_disjunction() {
        let disj = USD_PRIM_IS_MODEL.or(USD_PRIM_IS_GROUP);
        let pred = disj.into_predicate();

        let mut model_flags = PrimFlagBits::empty();
        model_flags.set(PrimFlag::Model, true);
        assert!(pred.eval(model_flags));

        let mut group_flags = PrimFlagBits::empty();
        group_flags.set(PrimFlag::Group, true);
        assert!(pred.eval(group_flags));

        let mut both_flags = PrimFlagBits::empty();
        both_flags.set(PrimFlag::Model, true);
        both_flags.set(PrimFlag::Group, true);
        assert!(pred.eval(both_flags));

        let empty_flags = PrimFlagBits::empty();
        assert!(!pred.eval(empty_flags));
    }

    #[test]
    fn test_default_predicate() {
        let pred = default_predicate().into_predicate();

        let mut valid_flags = PrimFlagBits::empty();
        valid_flags.set(PrimFlag::Active, true);
        valid_flags.set(PrimFlag::Defined, true);
        valid_flags.set(PrimFlag::Loaded, true);
        assert!(pred.eval(valid_flags));

        // Add abstract - should fail
        let mut abstract_flags = valid_flags;
        abstract_flags.set(PrimFlag::Abstract, true);
        assert!(!pred.eval(abstract_flags));
    }

    #[test]
    fn test_tautology_contradiction() {
        let taut = PrimFlagsPredicate::tautology();
        let cont = PrimFlagsPredicate::contradiction();

        let any_flags = PrimFlagBits::from_bits(0xFFFF);
        let no_flags = PrimFlagBits::empty();

        assert!(taut.eval(any_flags));
        assert!(taut.eval(no_flags));

        assert!(!cont.eval(any_flags));
        assert!(!cont.eval(no_flags));
    }

    #[test]
    fn test_conjunction_contradiction() {
        // Active AND NOT Active should be contradiction
        let conj = USD_PRIM_IS_ACTIVE.and(USD_PRIM_IS_ACTIVE.not());
        assert!(conj.as_predicate().is_contradiction());
    }

    #[test]
    fn test_disjunction_tautology() {
        // Active OR NOT Active should be tautology
        let disj = USD_PRIM_IS_ACTIVE.or(USD_PRIM_IS_ACTIVE.not());
        assert!(disj.as_predicate().is_tautology());
    }

    #[test]
    fn test_de_morgan() {
        // !(A && B) == !A || !B
        let conj = USD_PRIM_IS_ACTIVE.and(USD_PRIM_IS_LOADED);
        let negated_conj = conj.not();

        let disj = USD_PRIM_IS_ACTIVE.not().or(USD_PRIM_IS_LOADED.not());

        // Test both evaluate same for various flags
        let test_cases = [(false, false), (false, true), (true, false), (true, true)];

        for (active, loaded) in test_cases {
            let mut flags = PrimFlagBits::empty();
            flags.set(PrimFlag::Active, active);
            flags.set(PrimFlag::Loaded, loaded);

            assert_eq!(
                negated_conj.as_predicate().eval(flags),
                disj.as_predicate().eval(flags),
                "De Morgan failed for active={}, loaded={}",
                active,
                loaded
            );
        }
    }

    #[test]
    fn test_traverse_instance_proxies() {
        let mut pred = default_predicate().into_predicate();
        assert!(!pred.include_instance_proxies_in_traversal());

        pred.traverse_instance_proxies(true);
        assert!(pred.include_instance_proxies_in_traversal());

        pred.traverse_instance_proxies(false);
        assert!(!pred.include_instance_proxies_in_traversal());
    }

    #[test]
    fn test_legacy_prim_flags() {
        let flags = PrimFlags::ACTIVE | PrimFlags::DEFINED;
        assert!(flags.contains(PrimFlags::ACTIVE));
        assert!(flags.contains(PrimFlags::DEFINED));
        assert!(!flags.contains(PrimFlags::MODEL));
    }
}
