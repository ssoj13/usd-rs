//! PCP fundamental types.
//!
//! Types used throughout the Prim Cache Population (composition) system.

use std::fmt;

/// Describes the type of arc connecting two nodes in the prim index.
///
/// Arcs are listed in strength order (from strongest to weakest).
///
/// # Strength Order
///
/// 1. Root (special, no parent)
/// 2. Inherit
/// 3. Variant
/// 4. Relocate
/// 5. Reference
/// 6. Payload
/// 7. Specialize
///
/// # Examples
///
/// ```
/// use usd_pcp::ArcType;
///
/// let arc = ArcType::Reference;
/// assert!(arc.is_composition_arc());
/// assert!(!arc.is_class_based());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum ArcType {
    /// The root arc is a special value used for the root node.
    /// Unlike other arcs, it has no parent node.
    #[default]
    Root = 0,

    /// Inherit arc - class-based composition.
    Inherit = 1,

    /// Variant arc - variant selection.
    Variant = 2,

    /// Relocate arc - namespace relocation.
    Relocate = 3,

    /// Reference arc - external file reference.
    Reference = 4,

    /// Payload arc - deferred loading reference.
    Payload = 5,

    /// Specialize arc - class-based composition (weaker than inherit).
    Specialize = 6,
}

impl ArcType {
    /// Returns the number of arc types.
    pub const COUNT: usize = 7;

    /// Returns true if this is an inherit arc.
    #[inline]
    pub fn is_inherit(self) -> bool {
        self == Self::Inherit
    }

    /// Returns true if this is a specialize arc.
    #[inline]
    pub fn is_specialize(self) -> bool {
        self == Self::Specialize
    }

    /// Returns true if this is a class-based composition arc.
    ///
    /// Class-based arcs (inherit and specialize) imply additional sources
    /// of opinions outside of the site where the arc is introduced.
    #[inline]
    pub fn is_class_based(self) -> bool {
        self.is_inherit() || self.is_specialize()
    }

    /// Returns true if this is a composition arc (not root).
    #[inline]
    pub fn is_composition_arc(self) -> bool {
        self != Self::Root
    }

    /// Returns the strength index (lower = stronger).
    #[inline]
    pub fn strength_index(self) -> u8 {
        self as u8
    }

    /// Returns the display name for this arc type.
    #[inline]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Inherit => "inherit",
            Self::Variant => "variant",
            Self::Relocate => "relocate",
            Self::Reference => "reference",
            Self::Payload => "payload",
            Self::Specialize => "specialize",
        }
    }

    /// Returns all arc types in strength order.
    pub fn all() -> &'static [ArcType] {
        &[
            Self::Root,
            Self::Inherit,
            Self::Variant,
            Self::Relocate,
            Self::Reference,
            Self::Payload,
            Self::Specialize,
        ]
    }
}

impl fmt::Display for ArcType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "root"),
            Self::Inherit => write!(f, "inherit"),
            Self::Variant => write!(f, "variant"),
            Self::Relocate => write!(f, "relocate"),
            Self::Reference => write!(f, "reference"),
            Self::Payload => write!(f, "payload"),
            Self::Specialize => write!(f, "specialize"),
        }
    }
}

/// Describes a range of nodes in the prim index based on arc type.
///
/// Used to filter or select portions of the composition graph.
///
/// # Examples
///
/// ```
/// use usd_pcp::RangeType;
///
/// let range = RangeType::All;
/// assert!(range.is_valid());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RangeType {
    /// Range including just the root node.
    Root = 0,

    /// Range including inherit arcs and their descendants.
    Inherit = 1,

    /// Range including variant arcs and their descendants.
    Variant = 2,

    /// Range including reference arcs and their descendants.
    Reference = 3,

    /// Range including payload arcs and their descendants.
    Payload = 4,

    /// Range including specialize arcs and their descendants.
    Specialize = 5,

    /// Range including all nodes.
    #[default]
    All = 6,

    /// Range including all nodes weaker than the root node.
    WeakerThanRoot = 7,

    /// Range including all nodes stronger than the payload node.
    StrongerThanPayload = 8,

    /// Invalid range.
    Invalid = 9,
}

impl RangeType {
    /// Returns true if this is a valid range type.
    #[inline]
    pub fn is_valid(self) -> bool {
        self != Self::Invalid
    }

    /// Returns the corresponding arc type, if this range is arc-based.
    pub fn arc_type(self) -> Option<ArcType> {
        match self {
            Self::Root => Some(ArcType::Root),
            Self::Inherit => Some(ArcType::Inherit),
            Self::Variant => Some(ArcType::Variant),
            Self::Reference => Some(ArcType::Reference),
            Self::Payload => Some(ArcType::Payload),
            Self::Specialize => Some(ArcType::Specialize),
            _ => None,
        }
    }
}

impl fmt::Display for RangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root => write!(f, "root"),
            Self::Inherit => write!(f, "inherit"),
            Self::Variant => write!(f, "variant"),
            Self::Reference => write!(f, "reference"),
            Self::Payload => write!(f, "payload"),
            Self::Specialize => write!(f, "specialize"),
            Self::All => write!(f, "all"),
            Self::WeakerThanRoot => write!(f, "weaker than root"),
            Self::StrongerThanPayload => write!(f, "stronger than payload"),
            Self::Invalid => write!(f, "invalid"),
        }
    }
}

/// A value which indicates an invalid index.
pub const INVALID_INDEX: usize = usize::MAX;

// ============================================================================
// Site Tracking
// ============================================================================

use crate::Site;

/// Used to keep track of which sites have been visited and through
/// what type of arcs.
///
/// As the composition tree is being built, we add segments to the tracker.
/// If we encounter a site that we've already visited, we've found a cycle.
///
/// # Examples
///
/// ```
/// use usd_pcp::{SiteTrackerSegment, ArcType, Site, LayerStackIdentifier};
/// use usd_sdf::Path;
///
/// let site = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
/// let segment = SiteTrackerSegment::new(site, ArcType::Reference);
/// assert_eq!(segment.arc_type, ArcType::Reference);
/// ```
#[derive(Clone, Debug)]
pub struct SiteTrackerSegment {
    /// The site that was visited.
    pub site: Site,
    /// The type of arc used to reach this site.
    pub arc_type: ArcType,
}

impl SiteTrackerSegment {
    /// Creates a new site tracker segment.
    #[inline]
    pub fn new(site: Site, arc_type: ArcType) -> Self {
        Self { site, arc_type }
    }
}

/// Represents a single path through the composition tree.
///
/// As the tree is being built, we add segments to the tracker. If we
/// encounter a site that we've already visited, we've found a cycle.
pub type SiteTracker = Vec<SiteTrackerSegment>;

// ============================================================================
// Type Aliases
// ============================================================================

/// A map of lists of fallback variant selections.
///
/// This maps a variant set name (e.g., "shadingComplexity") to an ordered
/// list of variant selection names to try. If there is no variant selection
/// authored in the scene description, PCP will check for each listed fallback
/// in sequence, using the first one that exists.
///
/// # Examples
///
/// ```
/// use usd_pcp::VariantFallbackMap;
///
/// let mut fallbacks = VariantFallbackMap::new();
/// fallbacks.insert(
///     "shadingComplexity".to_string(),
///     vec!["full".to_string(), "preview".to_string()]
/// );
/// ```
pub type VariantFallbackMap = std::collections::HashMap<String, Vec<String>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_type_strength_order() {
        // Verify strength ordering
        assert!(ArcType::Root.strength_index() < ArcType::Inherit.strength_index());
        assert!(ArcType::Inherit.strength_index() < ArcType::Variant.strength_index());
        assert!(ArcType::Variant.strength_index() < ArcType::Relocate.strength_index());
        assert!(ArcType::Relocate.strength_index() < ArcType::Reference.strength_index());
        assert!(ArcType::Reference.strength_index() < ArcType::Payload.strength_index());
        assert!(ArcType::Payload.strength_index() < ArcType::Specialize.strength_index());
    }

    #[test]
    fn test_arc_type_is_inherit() {
        assert!(ArcType::Inherit.is_inherit());
        assert!(!ArcType::Reference.is_inherit());
    }

    #[test]
    fn test_arc_type_is_specialize() {
        assert!(ArcType::Specialize.is_specialize());
        assert!(!ArcType::Inherit.is_specialize());
    }

    #[test]
    fn test_arc_type_is_class_based() {
        assert!(ArcType::Inherit.is_class_based());
        assert!(ArcType::Specialize.is_class_based());
        assert!(!ArcType::Reference.is_class_based());
        assert!(!ArcType::Payload.is_class_based());
        assert!(!ArcType::Root.is_class_based());
    }

    #[test]
    fn test_arc_type_is_composition_arc() {
        assert!(!ArcType::Root.is_composition_arc());
        assert!(ArcType::Inherit.is_composition_arc());
        assert!(ArcType::Reference.is_composition_arc());
        assert!(ArcType::Payload.is_composition_arc());
    }

    #[test]
    fn test_arc_type_all() {
        let all = ArcType::all();
        assert_eq!(all.len(), ArcType::COUNT);
        assert_eq!(all[0], ArcType::Root);
        assert_eq!(all[6], ArcType::Specialize);
    }

    #[test]
    fn test_arc_type_display() {
        assert_eq!(format!("{}", ArcType::Root), "root");
        assert_eq!(format!("{}", ArcType::Inherit), "inherit");
        assert_eq!(format!("{}", ArcType::Reference), "reference");
    }

    #[test]
    fn test_range_type_default() {
        assert_eq!(RangeType::default(), RangeType::All);
    }

    #[test]
    fn test_range_type_is_valid() {
        assert!(RangeType::All.is_valid());
        assert!(RangeType::Root.is_valid());
        assert!(!RangeType::Invalid.is_valid());
    }

    #[test]
    fn test_range_type_arc_type() {
        assert_eq!(RangeType::Root.arc_type(), Some(ArcType::Root));
        assert_eq!(RangeType::Inherit.arc_type(), Some(ArcType::Inherit));
        assert_eq!(RangeType::Reference.arc_type(), Some(ArcType::Reference));
        assert_eq!(RangeType::All.arc_type(), None);
        assert_eq!(RangeType::Invalid.arc_type(), None);
    }

    #[test]
    fn test_range_type_display() {
        assert_eq!(format!("{}", RangeType::All), "all");
        assert_eq!(format!("{}", RangeType::Root), "root");
        assert_eq!(format!("{}", RangeType::Invalid), "invalid");
    }

    #[test]
    fn test_invalid_index() {
        assert_eq!(INVALID_INDEX, usize::MAX);
    }

    #[test]
    fn test_variant_fallback_map() {
        let mut fallbacks = VariantFallbackMap::new();
        fallbacks.insert(
            "shadingComplexity".to_string(),
            vec!["full".to_string(), "preview".to_string()],
        );
        fallbacks.insert(
            "renderVariant".to_string(),
            vec!["high".to_string(), "medium".to_string(), "low".to_string()],
        );

        assert_eq!(fallbacks.len(), 2);
        assert_eq!(
            fallbacks.get("shadingComplexity"),
            Some(&vec!["full".to_string(), "preview".to_string()])
        );
        assert!(fallbacks.contains_key("renderVariant"));
    }

    #[test]
    fn test_site_tracker_segment() {
        use crate::LayerStackIdentifier;
        use usd_sdf::Path;

        let site = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        let segment = SiteTrackerSegment::new(site.clone(), ArcType::Reference);

        assert_eq!(segment.arc_type, ArcType::Reference);
        assert_eq!(segment.site.path.as_str(), "/World");
    }

    #[test]
    fn test_site_tracker() {
        use crate::LayerStackIdentifier;
        use usd_sdf::Path;

        let mut tracker: SiteTracker = Vec::new();

        let site1 = Site::new(LayerStackIdentifier::new("root.usda"), Path::from("/World"));
        tracker.push(SiteTrackerSegment::new(site1, ArcType::Root));

        let site2 = Site::new(LayerStackIdentifier::new("ref.usda"), Path::from("/Model"));
        tracker.push(SiteTrackerSegment::new(site2, ArcType::Reference));

        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker[0].arc_type, ArcType::Root);
        assert_eq!(tracker[1].arc_type, ArcType::Reference);
    }
}
