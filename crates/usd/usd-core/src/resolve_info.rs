//! Resolve Info - information about the source of an attribute's value.
//!
//! Port of pxr/usd/usd/resolveInfo.h
//!
//! UsdResolveInfo contains information about where an attribute's value comes from,
//! i.e., the 'resolved' location of the attribute.

use std::fmt;
use std::sync::Arc;

use usd_pcp::{LayerStackRefPtr, NodeRef};
use usd_sdf::{LayerHandle, LayerOffset, Path};
use usd_vt::spline::SplineValue;

use super::clip_set::ClipSet;

/// Describes the various sources of attribute values.
///
/// Matches C++ `UsdResolveInfoSource`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolveInfoSource {
    /// No value
    None,
    /// Built-in fallback value
    Fallback,
    /// Attribute default value
    Default,
    /// Attribute time samples
    TimeSamples,
    /// Value clips
    ValueClips,
    /// Spline value
    Spline,
}

impl fmt::Debug for ResolveInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolveInfo")
            .field("source", &self.source)
            .field("layer_stack", &self.layer_stack.as_ref().map(|_| "Some(LayerStack)"))
            .field("layer", &self.layer)
            .field("node", &self.node)
            .field("layer_to_stage_offset", &self.layer_to_stage_offset)
            .field("prim_path", &self.prim_path)
            .field("spline", &self.spline.is_some())
            .field("next_weaker", &self.next_weaker.is_some())
            .field("value_is_blocked", &self.value_is_blocked)
            .field("default_can_compose", &self.default_can_compose)
            .field(
                "default_can_compose_over_weaker_time_varying_sources",
                &self.default_can_compose_over_weaker_time_varying_sources,
            )
            .field(
                "value_clip_set",
                &self.value_clip_set.as_ref().map(|_| "Some(ClipSet)"),
            )
            .finish()
    }
}

impl fmt::Display for ResolveInfoSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveInfoSource::None => write!(f, "None"),
            ResolveInfoSource::Fallback => write!(f, "Fallback"),
            ResolveInfoSource::Default => write!(f, "Default"),
            ResolveInfoSource::TimeSamples => write!(f, "TimeSamples"),
            ResolveInfoSource::ValueClips => write!(f, "ValueClips"),
            ResolveInfoSource::Spline => write!(f, "Spline"),
        }
    }
}

/// Container for information about the source of an attribute's value.
///
/// Matches C++ `UsdResolveInfo`.
#[derive(Clone)]
pub struct ResolveInfo {
    /// The source of the value.
    source: ResolveInfoSource,
    /// The layer stack that provides the strongest value opinion.
    layer_stack: Option<LayerStackRefPtr>,
    /// The layer in layer_stack that provides the strongest time sample or default opinion.
    layer: Option<LayerHandle>,
    /// The node within the containing PcpPrimIndex that provided the strongest value opinion.
    node: Option<NodeRef>,
    /// The time offset that maps time in the strongest resolved layer to the stage.
    layer_to_stage_offset: LayerOffset,
    /// The path to the prim that owns the attribute.
    prim_path: Path,
    /// The authored spline value when the source is `Spline`.
    spline: Option<SplineValue>,
    /// The next weaker resolve info in a composed-value chain.
    next_weaker: Option<Box<ResolveInfo>>,
    /// Whether the value is blocked.
    value_is_blocked: bool,
    /// Whether a default value can compose over weaker opinions.
    default_can_compose: bool,
    /// Whether a composing default had weaker time-varying sources.
    default_can_compose_over_weaker_time_varying_sources: bool,
    /// When source is `ValueClips`, the clip set that provides the strongest opinion.
    /// Matches C++ `_ExtraResolveInfo::clipSet`.
    value_clip_set: Option<Arc<ClipSet>>,
}

impl Default for ResolveInfo {
    fn default() -> Self {
        Self {
            source: ResolveInfoSource::None,
            layer_stack: None,
            layer: None,
            node: None,
            layer_to_stage_offset: LayerOffset::default(),
            prim_path: Path::empty(),
            spline: None,
            next_weaker: None,
            value_is_blocked: false,
            default_can_compose: false,
            default_can_compose_over_weaker_time_varying_sources: false,
            value_clip_set: None,
        }
    }
}

impl ResolveInfo {
    /// Creates a new ResolveInfo with no source.
    ///
    /// Matches C++ default constructor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the source of the associated attribute's value.
    ///
    /// Matches C++ `GetSource()`.
    pub fn source(&self) -> ResolveInfoSource {
        self.source
    }

    /// Return true if this ResolveInfo represents an attribute that has an
    /// authored value opinion. This will return `true` if there is *any*
    /// authored value opinion, including a block.
    ///
    /// Matches C++ `HasAuthoredValueOpinion()`.
    pub fn has_authored_value_opinion(&self) -> bool {
        matches!(
            self.source,
            ResolveInfoSource::Default
                | ResolveInfoSource::TimeSamples
                | ResolveInfoSource::ValueClips
                | ResolveInfoSource::Spline
        ) || self.value_is_blocked
    }

    /// Return true if this ResolveInfo represents an attribute that has an
    /// authored value that is not blocked.
    ///
    /// Matches C++ `HasAuthoredValue()`.
    pub fn has_authored_value(&self) -> bool {
        matches!(
            self.source,
            ResolveInfoSource::Default
                | ResolveInfoSource::TimeSamples
                | ResolveInfoSource::ValueClips
                | ResolveInfoSource::Spline
        )
    }

    /// Return the node within the containing PcpPrimIndex that provided
    /// the resolved value opinion.
    ///
    /// Matches C++ `GetNode()`.
    pub fn node(&self) -> Option<&NodeRef> {
        self.node.as_ref()
    }

    /// Return true if this ResolveInfo represents an attribute whose
    /// value is blocked.
    ///
    /// Matches C++ `ValueIsBlocked()`.
    pub fn value_is_blocked(&self) -> bool {
        self.value_is_blocked
    }

    /// Returns true if the resolve info source might be time-varying; false
    /// otherwise.
    ///
    /// Matches C++ `ValueSourceMightBeTimeVarying()`.
    pub fn value_source_might_be_time_varying(&self) -> bool {
        if matches!(
            self.source,
            ResolveInfoSource::TimeSamples
                | ResolveInfoSource::Spline
                | ResolveInfoSource::ValueClips
        ) {
            return true;
        }
        if let Some(next_weaker) = self.next_weaker.as_deref() {
            return next_weaker.value_source_might_be_time_varying();
        }
        self.source == ResolveInfoSource::Default
            && self.default_can_compose_over_weaker_time_varying_sources
    }

    /// Get the layer stack that provides the strongest value opinion.
    ///
    /// Returns None if the value source is not resolved from composition.
    /// For internal USD composition use.
    pub fn layer_stack(&self) -> Option<&LayerStackRefPtr> {
        self.layer_stack.as_ref()
    }

    /// Get the layer that provides the strongest time sample or default opinion.
    ///
    /// Returns None if no layer provides the value.
    /// For internal USD composition use.
    pub fn layer(&self) -> Option<&LayerHandle> {
        self.layer.as_ref()
    }

    /// Get the time offset that maps time in the strongest resolved layer to the stage.
    ///
    /// This offset is used to transform time samples from the layer's time coordinate
    /// system to the stage's time coordinate system.
    pub fn layer_to_stage_offset(&self) -> &LayerOffset {
        &self.layer_to_stage_offset
    }

    /// Get the path to the prim that owns the attribute.
    ///
    /// Returns the prim path in the layer stack's coordinate system.
    pub fn prim_path(&self) -> &Path {
        &self.prim_path
    }

    pub fn spline(&self) -> Option<&SplineValue> {
        self.spline.as_ref()
    }

    pub fn next_weaker_info(&self) -> Option<&ResolveInfo> {
        self.next_weaker.as_deref()
    }

    /// When [`ResolveInfoSource::ValueClips`] is the source, returns the winning clip set.
    ///
    /// Matches C++ `_ExtraResolveInfo::clipSet` paired with `UsdResolveInfoSourceValueClips`.
    pub fn value_clip_set(&self) -> Option<&Arc<ClipSet>> {
        self.value_clip_set.as_ref()
    }

    pub fn default_can_compose(&self) -> bool {
        self.default_can_compose
    }

    /// Set the source.
    ///
    /// Used by USD composition system to populate resolve info.
    pub(crate) fn set_source(&mut self, source: ResolveInfoSource) {
        self.source = source;
    }

    /// Set the prim path.
    ///
    /// Used by USD composition system to populate resolve info.
    pub(crate) fn set_prim_path(&mut self, path: Path) {
        self.prim_path = path;
    }

    #[allow(dead_code)]
    pub(crate) fn set_spline(&mut self, spline: SplineValue) {
        self.spline = Some(spline);
    }

    #[allow(dead_code)]
    pub(crate) fn clear_spline(&mut self) {
        self.spline = None;
    }

    /// Set the layer that provides the strongest opinion.
    ///
    /// Used by USD composition system to populate resolve info.
    #[allow(dead_code)]
    pub(crate) fn set_layer(&mut self, layer: LayerHandle) {
        self.layer = Some(layer);
    }

    /// Set the layer stack that provides the strongest opinion.
    ///
    /// Used by USD composition system to populate resolve info.
    #[allow(dead_code)]
    pub(crate) fn set_layer_stack(&mut self, layer_stack: LayerStackRefPtr) {
        self.layer_stack = Some(layer_stack);
    }

    /// Set the PCP node that provided the value.
    ///
    /// Used by USD composition system to populate resolve info.
    #[allow(dead_code)]
    pub(crate) fn set_node(&mut self, node: NodeRef) {
        self.node = Some(node);
    }

    /// Set the layer-to-stage time offset.
    ///
    /// Used by USD composition system to populate resolve info.
    #[allow(dead_code)]
    pub(crate) fn set_layer_to_stage_offset(&mut self, offset: LayerOffset) {
        self.layer_to_stage_offset = offset;
    }

    /// Set whether the value is blocked.
    ///
    /// Used by USD composition system to populate resolve info.
    #[allow(dead_code)]
    pub(crate) fn set_value_is_blocked(&mut self, blocked: bool) {
        self.value_is_blocked = blocked;
    }

    #[allow(dead_code)]
    pub(crate) fn set_default_can_compose(&mut self, can_compose: bool) {
        self.default_can_compose = can_compose;
    }

    #[allow(dead_code)]
    pub(crate) fn set_default_can_compose_over_weaker_time_varying_sources(
        &mut self,
        can_compose: bool,
    ) {
        self.default_can_compose_over_weaker_time_varying_sources = can_compose;
    }

    pub(crate) fn set_value_clip_set(&mut self, clip_set: Option<Arc<ClipSet>>) {
        self.value_clip_set = clip_set;
    }

    #[allow(dead_code)]
    pub(crate) fn add_next_weaker_info(&mut self) -> &mut ResolveInfo {
        self.next_weaker
            .get_or_insert_with(|| Box::new(ResolveInfo::default()))
            .as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let info = ResolveInfo::new();
        assert_eq!(info.source(), ResolveInfoSource::None);
        assert!(!info.has_authored_value_opinion());
        assert!(!info.has_authored_value());
        assert!(!info.value_is_blocked());
        assert!(!info.value_source_might_be_time_varying());
    }

    #[test]
    fn test_time_samples() {
        let mut info = ResolveInfo::new();
        info.set_source(ResolveInfoSource::TimeSamples);
        assert!(info.has_authored_value_opinion());
        assert!(info.has_authored_value());
        assert!(info.value_source_might_be_time_varying());
    }

    #[test]
    fn test_blocked() {
        let info = ResolveInfo::default();
        // value_is_blocked is false by default
        assert!(!info.value_is_blocked());
        // Default source has no authored value
        assert!(!info.has_authored_value_opinion());
        assert!(!info.has_authored_value());
    }

    #[test]
    fn test_accessors() {
        let mut info = ResolveInfo::new();

        // Test prim_path accessor and setter
        let test_path = Path::from("/World/Cube");
        info.set_prim_path(test_path.clone());
        assert_eq!(info.prim_path(), &test_path);

        // Test layer_to_stage_offset accessor (default value)
        assert_eq!(info.layer_to_stage_offset().offset(), 0.0);
        assert_eq!(info.layer_to_stage_offset().scale(), 1.0);

        // Test layer accessor (returns None by default)
        assert!(info.layer().is_none());

        // Test layer_stack accessor (returns None by default)
        assert!(info.layer_stack().is_none());
    }

    #[test]
    fn test_node_accessor() {
        let info = ResolveInfo::new();
        // Node should be None by default
        assert!(info.node().is_none());
    }

    // M9: ResolveInfo setters
    #[test]
    fn test_set_value_is_blocked() {
        let mut info = ResolveInfo::new();
        assert!(!info.value_is_blocked());

        info.set_value_is_blocked(true);
        assert!(info.value_is_blocked());

        info.set_value_is_blocked(false);
        assert!(!info.value_is_blocked());
    }

    #[test]
    fn test_set_layer_to_stage_offset() {
        let mut info = ResolveInfo::new();

        let offset = LayerOffset::new(10.0, 2.0);
        info.set_layer_to_stage_offset(offset);

        assert_eq!(info.layer_to_stage_offset().offset(), 10.0);
        assert_eq!(info.layer_to_stage_offset().scale(), 2.0);
    }
}
