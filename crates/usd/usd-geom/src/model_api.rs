//! UsdGeomModelAPI - API schema for model-level geometry concepts.
//!
//! Extends UsdModelAPI with geometry-specific concepts:
//! - Cached extents for entire models (extentsHint)
//! - Constraint targets for rigging
//! - Draw mode control for LOD/proxy representation
//!
//! # Draw Modes
//!
//! Draw modes provide alternate imaging for USD subtrees with kind "model":
//! - `origin` - Draw model-space basis vectors
//! - `bounds` - Draw model-space bounding box
//! - `cards` - Draw textured quads as placeholders
//! - `default` - Draw USD subtree normally
//! - `inherited` - Defer to parent opinion
//!
//! # Cards Geometry
//!
//! When drawMode is "cards", geometry type is controlled by cardGeometry:
//! - `cross` - Quads bisecting model extents
//! - `box` - Quads on faces of model extents
//! - `fromTexture` - Quads generated from texture metadata
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdGeom/modelAPI.h` and `modelAPI.cpp`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_gf::Vec3f;
use usd_sdf::{Path, TimeCode, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

use super::bbox_cache::BBoxCache;
use super::constraint_target::ConstraintTarget;
use super::schema_create_default::apply_optional_default;
use super::tokens::usd_geom_tokens;

/// UsdGeomModelAPI extends UsdModelAPI with geometry-specific concepts.
///
/// Provides:
/// - Cached extents (extentsHint) for fast bbox queries
/// - Constraint targets for rigging
/// - Draw mode control for LOD/proxy imaging
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
#[derive(Debug, Clone)]
pub struct ModelAPI {
    prim: Prim,
}

impl ModelAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "GeomModelAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct a ModelAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a ModelAPI holding the prim at `path` on `stage`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        // GeomModelAPI can be applied to any valid prim
        prim.is_valid()
    }

    /// Applies this single-apply API schema to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.is_valid() {
            // Register the API schema on the prim
            prim.apply_api(&usd_tf::Token::new("GeomModelAPI"));
            Some(Self::new(prim.clone()))
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    /// Create-or-get a GeomModelAPI attribute; optionally set default time sample.
    ///
    /// Matches C++ `Create*Attr(VtValue defaultValue, bool writeSparsely)` pattern.
    fn create_geom_model_schema_attr(
        &self,
        name: &str,
        sdf_typename: &str,
        variability: Variability,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let prim = &self.prim;
        if !prim.is_valid() {
            return Attribute::invalid();
        }
        let attr = if prim.has_authored_attribute(name) {
            prim.get_attribute(name).unwrap_or_else(Attribute::invalid)
        } else {
            let registry = ValueTypeRegistry::instance();
            let ty = registry.find_type_by_token(&Token::new(sdf_typename));
            prim.create_attribute(name, &ty, false, Some(variability))
                .unwrap_or_else(Attribute::invalid)
        };
        apply_optional_default(attr, default_value)
    }

    // =========================================================================
    // ModelDrawMode Attribute
    // =========================================================================

    /// Get the model:drawMode attribute.
    ///
    /// Alternate imaging mode applied to this prim or children where
    /// model:applyDrawMode is true, or where prim has kind "component".
    ///
    /// Allowed values: origin, bounds, cards, default, inherited
    pub fn get_model_draw_mode_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_draw_mode.as_str())
    }

    /// Creates the model:drawMode attribute.
    ///
    /// Matches C++ `CreateModelDrawModeAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_draw_mode_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_draw_mode.as_str(),
            "token",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // ModelApplyDrawMode Attribute
    // =========================================================================

    /// Get the model:applyDrawMode attribute.
    ///
    /// If true and resolved drawMode is non-default, apply alternate imaging.
    pub fn get_model_apply_draw_mode_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_apply_draw_mode.as_str())
    }

    /// Creates the model:applyDrawMode attribute.
    ///
    /// Matches C++ `CreateModelApplyDrawModeAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_apply_draw_mode_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_apply_draw_mode.as_str(),
            "bool",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // ModelDrawModeColor Attribute
    // =========================================================================

    /// Get the model:drawModeColor attribute.
    ///
    /// Base color for imaging prims in alternate modes.
    /// Default: (0.18, 0.18, 0.18)
    pub fn get_model_draw_mode_color_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_draw_mode_color.as_str())
    }

    /// Creates the model:drawModeColor attribute.
    ///
    /// Matches C++ `CreateModelDrawModeColorAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_draw_mode_color_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_draw_mode_color.as_str(),
            "float3",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // ModelCardGeometry Attribute
    // =========================================================================

    /// Get the model:cardGeometry attribute.
    ///
    /// Geometry type for cards imaging mode.
    /// Allowed values: cross, box, fromTexture
    pub fn get_model_card_geometry_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_geometry.as_str())
    }

    /// Creates the model:cardGeometry attribute.
    ///
    /// Matches C++ `CreateModelCardGeometryAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_geometry_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_geometry.as_str(),
            "token",
            Variability::Uniform,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // Card Texture Attributes (6 faces)
    // =========================================================================

    /// Get the model:cardTextureXPos attribute.
    pub fn get_model_card_texture_x_pos_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_texture_x_pos.as_str())
    }

    /// Creates the model:cardTextureXPos attribute.
    ///
    /// Matches C++ `CreateModelCardTextureXPosAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_texture_x_pos_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_texture_x_pos.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    /// Get the model:cardTextureYPos attribute.
    pub fn get_model_card_texture_y_pos_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_texture_y_pos.as_str())
    }

    /// Creates the model:cardTextureYPos attribute.
    ///
    /// Matches C++ `CreateModelCardTextureYPosAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_texture_y_pos_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_texture_y_pos.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    /// Get the model:cardTextureZPos attribute.
    pub fn get_model_card_texture_z_pos_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_texture_z_pos.as_str())
    }

    /// Creates the model:cardTextureZPos attribute.
    ///
    /// Matches C++ `CreateModelCardTextureZPosAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_texture_z_pos_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_texture_z_pos.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    /// Get the model:cardTextureXNeg attribute.
    pub fn get_model_card_texture_x_neg_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_texture_x_neg.as_str())
    }

    /// Creates the model:cardTextureXNeg attribute.
    ///
    /// Matches C++ `CreateModelCardTextureXNegAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_texture_x_neg_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_texture_x_neg.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    /// Get the model:cardTextureYNeg attribute.
    pub fn get_model_card_texture_y_neg_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_texture_y_neg.as_str())
    }

    /// Creates the model:cardTextureYNeg attribute.
    ///
    /// Matches C++ `CreateModelCardTextureYNegAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_texture_y_neg_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_texture_y_neg.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    /// Get the model:cardTextureZNeg attribute.
    pub fn get_model_card_texture_z_neg_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().model_card_texture_z_neg.as_str())
    }

    /// Creates the model:cardTextureZNeg attribute.
    ///
    /// Matches C++ `CreateModelCardTextureZNegAttr(VtValue defaultValue, bool writeSparsely)`.
    pub fn create_model_card_texture_z_neg_attr(
        &self,
        default_value: Option<Value>,
        write_sparsely: bool,
    ) -> Attribute {
        self.create_geom_model_schema_attr(
            usd_geom_tokens().model_card_texture_z_neg.as_str(),
            "asset",
            Variability::Varying,
            default_value,
            write_sparsely,
        )
    }

    // =========================================================================
    // Extents Hint API
    // =========================================================================

    /// Retrieve the authored extentsHint value.
    ///
    /// The extentsHint contains cached extents for various purpose values,
    /// stored as pairs of Vec3f (min, max) in the order specified by
    /// `Imageable::get_ordered_purpose_tokens()`.
    ///
    /// Returns None if no value was authored or prim is not a model root.
    pub fn get_extents_hint(&self, time: TimeCode) -> Option<Vec<Vec3f>> {
        let attr = self.get_extents_hint_attr()?;
        attr.get_typed::<Vec<Vec3f>>(time)
    }

    /// Authors the extentsHint array at the given time.
    ///
    /// The extents should contain pairs of (min, max) Vec3f values
    /// for each purpose in `Imageable::get_ordered_purpose_tokens()`.
    pub fn set_extents_hint(&self, extents: &[Vec3f], time: TimeCode) -> bool {
        let registry = ValueTypeRegistry::instance();
        let float3_array_type = registry.find_type_by_token(&Token::new("float3[]"));

        if let Some(attr) = self.prim.create_attribute(
            usd_geom_tokens().extents_hint.as_str(),
            &float3_array_type,
            false,
            Some(Variability::Varying),
        ) {
            attr.set(Value::from_no_hash(extents.to_vec()), time);
            true
        } else {
            false
        }
    }

    /// Returns the custom 'extentsHint' attribute if it exists.
    pub fn get_extents_hint_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(usd_geom_tokens().extents_hint.as_str())
    }

    /// Compute a value suitable for `set_extents_hint()`.
    ///
    /// For Boundable prims, calls `ComputeExtentFromPlugins`.
    /// For non-Boundable prims, uses BBoxCache to compute untransformed bounds
    /// for each purpose.
    ///
    /// The result contains min/max pairs for each purpose in
    /// `Imageable::get_ordered_purpose_tokens()`, with trailing empty
    /// boxes omitted.
    pub fn compute_extents_hint(&self, bbox_cache: &mut BBoxCache) -> Vec<Vec3f> {
        // Get ordered purposes
        let purposes = super::imageable::Imageable::get_ordered_purpose_tokens();
        let mut result: Vec<Vec3f> = Vec::new();
        let mut last_non_empty = 0usize;

        for (i, purpose) in purposes.iter().enumerate() {
            // Set the purpose for bbox computation
            bbox_cache.set_included_purposes(vec![purpose.clone()]);

            // Compute untransformed bound for this prim
            let bbox = bbox_cache.compute_untransformed_bound(&self.prim);
            let range = bbox.compute_aligned_range();

            if !range.is_empty() {
                let min = range.min();
                let max = range.max();
                // Ensure we have space for all extents up to this point
                while result.len() < (i + 1) * 2 {
                    // Fill with empty range for skipped purposes.
                    // GfRange3f default empty = [FLT_MAX, -FLT_MAX] (matches C++ range3f.h line 59-60)
                    result.push(Vec3f::new(f32::MAX, f32::MAX, f32::MAX));
                    result.push(Vec3f::new(-f32::MAX, -f32::MAX, -f32::MAX));
                }
                result[i * 2] = Vec3f::new(min.x as f32, min.y as f32, min.z as f32);
                result[i * 2 + 1] = Vec3f::new(max.x as f32, max.y as f32, max.z as f32);
                last_non_empty = (i + 1) * 2;
            }
        }

        // Trim trailing empty boxes
        result.truncate(last_non_empty);

        // If all empty, return single empty box.
        // GfRange3f default empty = [FLT_MAX, -FLT_MAX] (matches C++ range3f.h line 59-60)
        if result.is_empty() {
            result.push(Vec3f::new(f32::MAX, f32::MAX, f32::MAX));
            result.push(Vec3f::new(-f32::MAX, -f32::MAX, -f32::MAX));
        }

        result
    }

    // =========================================================================
    // Constraint Targets API
    // =========================================================================

    /// Get the constraint target with the given name.
    ///
    /// Returns an invalid ConstraintTarget if not found.
    pub fn get_constraint_target(&self, constraint_name: &str) -> ConstraintTarget {
        let attr_name = ConstraintTarget::get_constraint_attr_name(constraint_name);
        if let Some(attr) = self.prim.get_attribute(attr_name.as_str()) {
            ConstraintTarget::new(attr)
        } else {
            ConstraintTarget::invalid()
        }
    }

    /// Creates a new constraint target with the given name.
    ///
    /// If it already exists, returns the existing target.
    pub fn create_constraint_target(&self, constraint_name: &str) -> ConstraintTarget {
        let attr_name = ConstraintTarget::get_constraint_attr_name(constraint_name);

        // Check if it already exists (filter by is_valid to skip non-existent attrs)
        if let Some(attr) = self
            .prim
            .get_attribute(attr_name.as_str())
            .filter(|a| a.is_valid())
        {
            return ConstraintTarget::new(attr);
        }

        // Create new attribute with matrix4d type
        let registry = ValueTypeRegistry::instance();
        let matrix4d_type = registry.find_type_by_token(&Token::new("matrix4d"));

        if let Some(attr) = self.prim.create_attribute(
            attr_name.as_str(),
            &matrix4d_type,
            true, // Custom attribute in constraintTargets namespace
            Some(Variability::Varying),
        ) {
            ConstraintTarget::new(attr)
        } else {
            ConstraintTarget::invalid()
        }
    }

    /// Returns all constraint targets belonging to this model.
    pub fn get_constraint_targets(&self) -> Vec<ConstraintTarget> {
        // Get all properties in the constraintTargets namespace
        let namespace = Token::new("constraintTargets");
        let props = self.prim.get_properties_in_namespace(&namespace);

        let mut targets = Vec::new();
        for prop in props {
            if let Some(attr) = prop.as_attribute() {
                let target = ConstraintTarget::new(attr);
                if target.is_defined() {
                    targets.push(target);
                }
            }
        }

        targets
    }

    // =========================================================================
    // Draw Mode Computation
    // =========================================================================

    /// Calculate the effective model:drawMode of this prim.
    ///
    /// If authored on this prim, uses that value. Otherwise "inherited"
    /// defers to parent opinion. Returns "default" if no ancestors have
    /// authored drawMode.
    ///
    /// For efficiency in traversal, pass computed parent drawMode via
    /// `parent_draw_mode` to avoid repeated upward traversal.
    pub fn compute_model_draw_mode(&self, parent_draw_mode: Option<&Token>) -> Token {
        // Check if authored on this prim
        if let Some(attr) = self.get_model_draw_mode_attr() {
            if let Some(mode) = attr.get_typed::<Token>(TimeCode::default()) {
                if mode != usd_geom_tokens().inherited {
                    return mode;
                }
            }
        }

        // If parent mode was provided, use it
        if let Some(parent_mode) = parent_draw_mode {
            if !parent_mode.as_str().is_empty() && *parent_mode != usd_geom_tokens().inherited {
                return parent_mode.clone();
            }
        }

        // Otherwise traverse upward to find first non-inherited ancestor
        let mut current = self.prim.parent();
        while current.is_valid() && !current.is_pseudo_root() {
            let parent_api = ModelAPI::new(current.clone());
            if let Some(attr) = parent_api.get_model_draw_mode_attr() {
                if let Some(mode) = attr.get_typed::<Token>(TimeCode::default()) {
                    if mode != usd_geom_tokens().inherited {
                        return mode;
                    }
                }
            }
            current = current.parent();
        }

        // No ancestor has authored drawMode, return "default"
        usd_geom_tokens().default_.clone()
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            usd_geom_tokens().model_draw_mode.clone(),
            usd_geom_tokens().model_apply_draw_mode.clone(),
            usd_geom_tokens().model_draw_mode_color.clone(),
            usd_geom_tokens().model_card_geometry.clone(),
            usd_geom_tokens().model_card_texture_x_pos.clone(),
            usd_geom_tokens().model_card_texture_y_pos.clone(),
            usd_geom_tokens().model_card_texture_z_pos.clone(),
            usd_geom_tokens().model_card_texture_x_neg.clone(),
            usd_geom_tokens().model_card_texture_y_neg.clone(),
            usd_geom_tokens().model_card_texture_z_neg.clone(),
        ]
    }
}

// ============================================================================
// Trait implementations
// ============================================================================

impl From<Prim> for ModelAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<ModelAPI> for Prim {
    fn from(api: ModelAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for ModelAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use usd_core::{InitialLoadSet, Stage};
    use usd_sdf::TimeCode;

    #[test]
    fn test_schema_kind() {
        assert_eq!(ModelAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(ModelAPI::SCHEMA_TYPE_NAME, "GeomModelAPI");
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = ModelAPI::get_schema_attribute_names(false);
        assert!(names.len() >= 10);
    }

    #[test]
    fn create_model_draw_mode_attr_writes_default_token() {
        let _ = usd_sdf::init();
        let stage = Arc::new(Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap());
        let prim = stage.define_prim("/GmDraw", "Xform").unwrap();
        let api = ModelAPI::new(prim);
        let mode = usd_geom_tokens().cards.clone();
        let attr = api.create_model_draw_mode_attr(Some(Value::new(mode.clone())), false);
        assert!(attr.is_valid());
        assert_eq!(
            attr.get_typed::<Token>(TimeCode::default()).as_ref(),
            Some(&mode)
        );
    }

    #[test]
    fn create_model_apply_draw_mode_attr_writes_default_bool() {
        let _ = usd_sdf::init();
        let stage = Arc::new(Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap());
        let prim = stage.define_prim("/GmApply", "Xform").unwrap();
        let api = ModelAPI::new(prim);
        let attr = api.create_model_apply_draw_mode_attr(Some(Value::new(true)), false);
        assert!(attr.is_valid());
        assert_eq!(attr.get_typed::<bool>(TimeCode::default()), Some(true));
    }

    #[test]
    fn create_model_draw_mode_second_call_reuses_attr() {
        let _ = usd_sdf::init();
        let stage = Arc::new(Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap());
        let prim = stage.define_prim("/GmReuse", "Xform").unwrap();
        let api = ModelAPI::new(prim);
        let a = api.create_model_draw_mode_attr(None, false);
        assert!(a.is_valid());
        let b = api
            .create_model_draw_mode_attr(Some(Value::new(usd_geom_tokens().bounds.clone())), false);
        assert!(b.is_valid());
        assert_eq!(
            b.get_typed::<Token>(TimeCode::default()).as_ref(),
            Some(&usd_geom_tokens().bounds)
        );
    }
}
