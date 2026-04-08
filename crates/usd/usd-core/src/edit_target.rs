//! Edit target for directing edits to a specific layer.
//!
//! An EditTarget specifies which layer in a stage's layer stack should
//! receive edits to prims and properties.

use std::collections::BTreeMap;
use std::sync::Arc;
use usd_pcp::MapFunction;
use usd_sdf::{Layer, LayerHandle, Path};

// ============================================================================
// EditTarget
// ============================================================================

/// Target for directing stage edits to a specific layer.
///
/// When making changes to a stage (creating prims, setting attributes, etc.),
/// the EditTarget determines which layer receives those changes.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::{UsdStage, EditTarget};
///
/// let stage = UsdStage::open("scene.usda")?;
///
/// // Get the session layer as edit target
/// let session = stage.get_session_layer();
/// let target = EditTarget::for_local_layer(session);
///
/// // Set the edit target
/// stage.set_edit_target(&target);
///
/// // Now edits go to the session layer
/// stage.define_prim("/Temp")?;
/// ```
/// Note: Debug is not derived because Layer does not implement Debug.
#[derive(Clone)]
pub struct EditTarget {
    /// The layer to receive edits (optional for invalid targets).
    layer: Option<Arc<Layer>>,
    /// Mapping from authored layer namespace/time to stage namespace/time.
    map_function: Option<MapFunction>,
}

impl EditTarget {
    /// Creates an invalid edit target.
    ///
    /// Matches C++ `UsdEditTarget()` default constructor.
    pub fn invalid() -> Self {
        Self {
            layer: None,
            map_function: None,
        }
    }

    /// Creates an edit target for a local layer (no path remapping).
    ///
    /// Matches C++ `UsdEditTarget(const SdfLayerHandle &layer)`.
    pub fn for_local_layer(layer: Arc<Layer>) -> Self {
        Self::for_local_layer_with_offset(layer, usd_sdf::LayerOffset::identity())
    }

    /// Creates an edit target for a local layer with a layer-to-stage offset.
    pub fn for_local_layer_with_offset(layer: Arc<Layer>, offset: usd_sdf::LayerOffset) -> Self {
        Self {
            layer: Some(layer),
            map_function: Some(MapFunction::identity().compose_offset(&offset)),
        }
    }

    /// Creates an edit target with path mapping.
    ///
    /// Matches C++ `UsdEditTarget(const SdfLayerHandle &layer, const SdfLayerOffset &offset)`.
    pub fn for_layer_at_path(layer: Arc<Layer>, source: Path, target: Path) -> Self {
        Self::for_layer_at_path_with_offset(layer, source, target, usd_sdf::LayerOffset::identity())
    }

    /// Creates an edit target with path mapping and layer offset.
    pub fn for_layer_at_path_with_offset(
        layer: Arc<Layer>,
        source: Path,
        target: Path,
        offset: usd_sdf::LayerOffset,
    ) -> Self {
        let mut path_map = BTreeMap::new();
        path_map.insert(source, target);
        Self {
            layer: Some(layer),
            map_function: Some(
                MapFunction::create(path_map, offset)
                    .unwrap_or_else(|| MapFunction::identity().compose_offset(&offset)),
            ),
        }
    }

    /// Creates an edit target directly from a PCP map function.
    pub fn for_layer_with_map_function(layer: Arc<Layer>, map_function: MapFunction) -> Self {
        Self {
            layer: Some(layer),
            map_function: Some(map_function),
        }
    }

    /// Creates an edit target for editing inside a variant.
    ///
    /// Matches C++ `UsdEditTarget::ForLocalDirectVariant(const SdfLayerHandle &layer, const SdfPath &variantPath)`.
    pub fn for_local_direct_variant(layer: LayerHandle, variant_path: Path) -> Self {
        // Convert LayerHandle to Arc<Layer> if possible
        let layer_arc = layer.upgrade();
        Self {
            layer: layer_arc,
            map_function: Some({
                let mut path_map = BTreeMap::new();
                path_map.insert(variant_path.clone(), variant_path);
                MapFunction::create(path_map, usd_sdf::LayerOffset::identity())
                    .unwrap_or_else(|| MapFunction::identity().clone())
            }),
        }
    }

    /// Returns the layer that receives edits.
    ///
    /// Matches C++ `GetLayer() const`.
    pub fn layer(&self) -> Option<&Arc<Layer>> {
        self.layer.as_ref()
    }

    /// Returns the layer handle (clone of the layer Arc).
    ///
    /// Matches C++ `GetLayer() const`.
    pub fn get_layer(&self) -> Option<LayerHandle> {
        self.layer.as_ref().map(LayerHandle::from_layer)
    }

    /// Returns true if this target has a local (non-remapped) layer.
    pub fn is_local_layer(&self) -> bool {
        self.map_function
            .as_ref()
            .is_some_and(|map| map.is_identity_path_mapping())
    }

    /// Maps a path from the stage namespace to the layer namespace.
    ///
    /// Matches C++ `MapToSpecPath(const SdfPath &path) const`.
    pub fn map_to_spec_path(&self, path: &Path) -> Path {
        if let Some(map) = &self.map_function {
            if path.is_property_path() {
                let prim_path = path.get_prim_path();
                let prop_name = path.get_name();
                if let Some(mapped_prim) = map.map_target_to_source(&prim_path) {
                    return mapped_prim
                        .append_property(prop_name)
                        .unwrap_or_else(|| path.clone());
                }
                return path.clone();
            }
            return map
                .map_target_to_source(path)
                .unwrap_or_else(|| path.clone());
        }
        path.clone()
    }

    /// Maps a stage time into the target layer's local time domain.
    pub fn map_time_to_spec_time(&self, time: f64) -> f64 {
        self.layer_to_stage_offset().inverse().apply(time)
    }

    /// Returns the cumulative layer-to-stage offset carried by this target.
    pub fn layer_to_stage_offset(&self) -> usd_sdf::LayerOffset {
        self.map_function
            .as_ref()
            .map(|map| *map.time_offset())
            .unwrap_or_else(usd_sdf::LayerOffset::identity)
    }

    /// Returns true if this edit target is valid (has a valid layer).
    ///
    /// Matches C++ `IsValid() const`.
    pub fn is_valid(&self) -> bool {
        self.layer.is_some()
    }

    /// Returns true if this is a null edit target.
    ///
    /// Null EditTargets map paths unchanged and have no layer.
    /// Matches C++ `IsNull() const`.
    pub fn is_null(&self) -> bool {
        self.layer.is_none() && self.map_function.is_none()
    }

    // ========================================================================
    // Spec Convenience Methods (matches C++ GetPrimSpecForScenePath etc.)
    // ========================================================================

    /// Returns the PrimSpec in the edit target's layer for the given scene path.
    ///
    /// Equivalent to `target.layer().get_prim_at_path(target.map_to_spec_path(path))`.
    /// Returns None if the target is null or no valid mapping exists.
    ///
    /// Matches C++ `GetPrimSpecForScenePath(const SdfPath&) const`.
    pub fn get_prim_spec_for_scene_path(&self, scene_path: &Path) -> Option<usd_sdf::PrimSpec> {
        let layer = self.layer.as_ref()?;
        let spec_path = self.map_to_spec_path(scene_path);
        layer.get_prim_at_path(&spec_path)
    }

    /// Returns the PropertySpec in the edit target's layer for the given scene path.
    ///
    /// Checks for attribute spec first, then relationship spec.
    /// Returns None if the target is null or no valid mapping exists.
    ///
    /// Matches C++ `GetPropertySpecForScenePath(const SdfPath&) const`.
    pub fn get_property_spec_for_scene_path(
        &self,
        scene_path: &Path,
    ) -> Option<usd_sdf::PropertySpec> {
        let layer = self.layer.as_ref()?;
        let spec_path = self.map_to_spec_path(scene_path);
        // Try attribute first, then relationship
        if let Some(attr) = layer.get_attribute_at_path(&spec_path) {
            return Some(usd_sdf::PropertySpec::new(attr.as_spec().clone()));
        }
        if let Some(rel) = layer.get_relationship_at_path(&spec_path) {
            return Some(usd_sdf::PropertySpec::new(rel.spec().clone()));
        }
        None
    }

    /// Returns the AttributeSpec in the edit target's layer for the given scene path.
    ///
    /// Returns None if the target is null, no mapping exists, or the spec is not an attribute.
    ///
    /// Matches C++ `GetAttributeSpecForScenePath(const SdfPath&) const`.
    pub fn get_attribute_spec_for_scene_path(
        &self,
        scene_path: &Path,
    ) -> Option<usd_sdf::AttributeSpec> {
        let layer = self.layer.as_ref()?;
        let spec_path = self.map_to_spec_path(scene_path);
        layer.get_attribute_at_path(&spec_path)
    }

    /// Returns the RelationshipSpec in the edit target's layer for the given scene path.
    ///
    /// Returns None if the target is null, no mapping exists, or the spec is not a relationship.
    ///
    /// Matches C++ `GetRelationshipSpecForScenePath(const SdfPath&) const`.
    pub fn get_relationship_spec_for_scene_path(
        &self,
        scene_path: &Path,
    ) -> Option<usd_sdf::RelationshipSpec> {
        let layer = self.layer.as_ref()?;
        let spec_path = self.map_to_spec_path(scene_path);
        layer.get_relationship_at_path(&spec_path)
    }

    /// Returns the path mapping (our equivalent of C++ PcpMapFunction).
    ///
    /// Matches C++ `GetMapFunction() const`.
    pub fn get_map_function(&self) -> Option<&MapFunction> {
        self.map_function.as_ref()
    }

    /// Returns a new EditTarget composed over `weaker`.
    ///
    /// This takes the layer from `self` and composes the path mappings.
    /// If `self` has no layer, uses `weaker`'s layer. If `self` has no mapping,
    /// uses `weaker`'s mapping.
    ///
    /// Matches C++ `ComposeOver(const UsdEditTarget&) const`.
    pub fn compose_over(&self, weaker: &EditTarget) -> EditTarget {
        // Use our layer if we have one, else the weaker's
        let layer = self.layer.clone().or_else(|| weaker.layer.clone());
        // Use our mapping if we have one, else the weaker's
        let map_function = self
            .map_function
            .clone()
            .or_else(|| weaker.map_function.clone());
        EditTarget {
            layer,
            map_function,
        }
    }
}

impl Default for EditTarget {
    /// Creates an invalid edit target.
    fn default() -> Self {
        Self::invalid()
    }
}

impl PartialEq for EditTarget {
    fn eq(&self, other: &Self) -> bool {
        match (&self.layer, &other.layer) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for EditTarget {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_target() {
        let target = EditTarget::invalid();
        assert!(!target.is_valid());
    }

    #[test]
    fn test_default_is_invalid() {
        let target = EditTarget::default();
        assert!(!target.is_valid());
    }

    #[test]
    fn test_local_layer_target() {
        let layer = Layer::create_anonymous(Some("test"));
        let target = EditTarget::for_local_layer(layer);

        assert!(target.is_local_layer());
        assert!(target.is_valid());
    }

    #[test]
    fn test_map_path_no_remapping() {
        let layer = Layer::create_anonymous(Some("test"));
        let target = EditTarget::for_local_layer(layer);

        let path = Path::from_string("/World/Cube").unwrap();
        assert_eq!(target.map_to_spec_path(&path), path);
    }

    #[test]
    fn test_map_path_with_remapping() {
        let layer = Layer::create_anonymous(Some("test"));
        let source = Path::from_string("/Root").unwrap();
        let target_path = Path::from_string("/World").unwrap();
        let target = EditTarget::for_layer_at_path(layer, source, target_path);

        let path = Path::from_string("/World/Cube").unwrap();
        let mapped = target.map_to_spec_path(&path);
        assert_eq!(mapped.get_string(), "/Root/Cube");
    }
}
