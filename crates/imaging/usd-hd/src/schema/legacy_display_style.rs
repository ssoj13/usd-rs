//! Legacy display style schema.
//!
//! Port of pxr/imaging/hd/legacyDisplayStyleSchema.h
//! Provides typed access to display-style fields: refine level, flat shading,
//! displacement, overlay, shading style, repr selector, cull style, etc.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, cast_to_container,
};
use once_cell::sync::Lazy;
use usd_tf::Token;

// Schema and member tokens matching C++ HdLegacyDisplayStyleSchemaTokens

/// Schema token: "displayStyle"
pub static DISPLAY_STYLE: Lazy<Token> = Lazy::new(|| Token::new("displayStyle"));
/// Member token: "refineLevel"
pub static REFINE_LEVEL: Lazy<Token> = Lazy::new(|| Token::new("refineLevel"));
/// Member token: "flatShadingEnabled"
pub static FLAT_SHADING_ENABLED: Lazy<Token> = Lazy::new(|| Token::new("flatShadingEnabled"));
/// Member token: "displacementEnabled"
pub static DISPLACEMENT_ENABLED: Lazy<Token> = Lazy::new(|| Token::new("displacementEnabled"));
/// Member token: "displayInOverlay"
pub static DISPLAY_IN_OVERLAY: Lazy<Token> = Lazy::new(|| Token::new("displayInOverlay"));
/// Member token: "occludedSelectionShowsThrough"
pub static OCCLUDED_SELECTION_SHOWS_THROUGH: Lazy<Token> =
    Lazy::new(|| Token::new("occludedSelectionShowsThrough"));
/// Member token: "pointsShadingEnabled"
pub static POINTS_SHADING_ENABLED: Lazy<Token> = Lazy::new(|| Token::new("pointsShadingEnabled"));
/// Member token: "materialIsFinal"
pub static MATERIAL_IS_FINAL: Lazy<Token> = Lazy::new(|| Token::new("materialIsFinal"));
/// Member token: "shadingStyle"
pub static SHADING_STYLE: Lazy<Token> = Lazy::new(|| Token::new("shadingStyle"));
/// Member token: "reprSelector"
pub static REPR_SELECTOR: Lazy<Token> = Lazy::new(|| Token::new("reprSelector"));
/// Member token: "cullStyle"
pub static CULL_STYLE: Lazy<Token> = Lazy::new(|| Token::new("cullStyle"));

/// Schema for legacy display style on prims.
///
/// Provides access to all Hydra 1 display style fields: refine level,
/// flat shading, displacement, overlay, material-is-final, cull style, etc.
///
/// Corresponds to C++ `HdLegacyDisplayStyleSchema`.
#[derive(Debug, Clone)]
pub struct HdLegacyDisplayStyleSchema {
    schema: HdSchema,
}

impl HdLegacyDisplayStyleSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Returns true if the schema has a valid container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&*DISPLAY_STYLE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    // Helper: read a bool field from the container.
    fn _get_bool(&self, token: &Token) -> Option<bool> {
        let ds = self.schema.get_container()?;
        let child: HdDataSourceBaseHandle = ds.get(token)?.clone();
        let sampled = child.as_sampled()?;
        sampled.get_value(0.0).get::<bool>().copied()
    }

    // Helper: read an i32 field from the container.
    fn _get_int(&self, token: &Token) -> Option<i32> {
        let ds = self.schema.get_container()?;
        let child: HdDataSourceBaseHandle = ds.get(token)?.clone();
        let sampled = child.as_sampled()?;
        sampled.get_value(0.0).get::<i32>().copied()
    }

    // Helper: read a Token field from the container.
    fn _get_token(&self, token: &Token) -> Option<Token> {
        let ds = self.schema.get_container()?;
        let child: HdDataSourceBaseHandle = ds.get(token)?.clone();
        let sampled = child.as_sampled()?;
        sampled.get_value(0.0).get::<Token>().cloned()
    }

    /// Get refine level (subdivision level, 0..8).
    pub fn get_refine_level(&self) -> Option<i32> {
        self._get_int(&REFINE_LEVEL)
    }

    /// Get flat shading enabled flag.
    pub fn get_flat_shading_enabled(&self) -> Option<bool> {
        self._get_bool(&FLAT_SHADING_ENABLED)
    }

    /// Get displacement enabled flag.
    pub fn get_displacement_enabled(&self) -> Option<bool> {
        self._get_bool(&DISPLACEMENT_ENABLED)
    }

    /// Get display-in-overlay flag.
    pub fn get_display_in_overlay(&self) -> Option<bool> {
        self._get_bool(&DISPLAY_IN_OVERLAY)
    }

    /// Get occluded selection shows through flag.
    pub fn get_occluded_selection_shows_through(&self) -> Option<bool> {
        self._get_bool(&OCCLUDED_SELECTION_SHOWS_THROUGH)
    }

    /// Get points shading enabled flag.
    pub fn get_points_shading_enabled(&self) -> Option<bool> {
        self._get_bool(&POINTS_SHADING_ENABLED)
    }

    /// Get material is final bool. Returns true if set and value is true.
    pub fn get_material_is_final(&self) -> Option<bool> {
        self._get_bool(&MATERIAL_IS_FINAL)
    }

    /// Get shading style token.
    pub fn get_shading_style(&self) -> Option<Token> {
        self._get_token(&SHADING_STYLE)
    }

    /// Get cull style token (nothing, back, front, backUnlessDoubleSided, etc.).
    pub fn get_cull_style(&self) -> Option<Token> {
        self._get_token(&CULL_STYLE)
    }

    /// Get repr selector (array of tokens like ["refined", "hull", ""]).
    pub fn get_repr_selector(&self) -> Option<Vec<Token>> {
        let ds = self.schema.get_container()?;
        let child: HdDataSourceBaseHandle = ds.get(&REPR_SELECTOR)?.clone();
        let sampled = child.as_sampled()?;
        sampled.get_value(0.0).get::<Vec<Token>>().cloned()
    }

    //--------------------------------------------------------------------------
    // Schema location
    //--------------------------------------------------------------------------

    /// Schema token: "displayStyle".
    pub fn get_schema_token() -> &'static Token {
        &DISPLAY_STYLE
    }

    /// Default locator for displayStyle schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone()])
    }

    //--------------------------------------------------------------------------
    // Member locators
    //--------------------------------------------------------------------------

    /// Locator for refineLevel.
    pub fn get_refine_level_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), REFINE_LEVEL.clone()])
    }

    /// Locator for flatShadingEnabled.
    pub fn get_flat_shading_enabled_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), FLAT_SHADING_ENABLED.clone()])
    }

    /// Locator for displacementEnabled.
    pub fn get_displacement_enabled_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), DISPLACEMENT_ENABLED.clone()])
    }

    /// Locator for displayInOverlay.
    pub fn get_display_in_overlay_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), DISPLAY_IN_OVERLAY.clone()])
    }

    /// Locator for occludedSelectionShowsThrough.
    pub fn get_occluded_selection_shows_through_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[
            DISPLAY_STYLE.clone(),
            OCCLUDED_SELECTION_SHOWS_THROUGH.clone(),
        ])
    }

    /// Locator for pointsShadingEnabled.
    pub fn get_points_shading_enabled_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), POINTS_SHADING_ENABLED.clone()])
    }

    /// Locator for materialIsFinal.
    pub fn get_material_is_final_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), MATERIAL_IS_FINAL.clone()])
    }

    /// Locator for shadingStyle.
    pub fn get_shading_style_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), SHADING_STYLE.clone()])
    }

    /// Locator for reprSelector.
    pub fn get_repr_selector_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), REPR_SELECTOR.clone()])
    }

    /// Locator for cullStyle.
    pub fn get_cull_style_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[DISPLAY_STYLE.clone(), CULL_STYLE.clone()])
    }

    //--------------------------------------------------------------------------
    // Schema construction
    //--------------------------------------------------------------------------

    /// Build a retained container data source with all fields.
    ///
    /// Parameters with None values are excluded from the container.
    /// Matches C++ `HdLegacyDisplayStyleSchema::BuildRetained` which takes
    /// data source handles directly.
    #[allow(clippy::too_many_arguments)]
    pub fn build_retained(
        refine_level: Option<HdDataSourceBaseHandle>,
        flat_shading_enabled: Option<HdDataSourceBaseHandle>,
        displacement_enabled: Option<HdDataSourceBaseHandle>,
        display_in_overlay: Option<HdDataSourceBaseHandle>,
        occluded_selection_shows_through: Option<HdDataSourceBaseHandle>,
        points_shading_enabled: Option<HdDataSourceBaseHandle>,
        material_is_final: Option<HdDataSourceBaseHandle>,
        shading_style: Option<HdDataSourceBaseHandle>,
        repr_selector: Option<HdDataSourceBaseHandle>,
        cull_style: Option<HdDataSourceBaseHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();

        let fields: [(Option<HdDataSourceBaseHandle>, &Token); 10] = [
            (refine_level, &REFINE_LEVEL),
            (flat_shading_enabled, &FLAT_SHADING_ENABLED),
            (displacement_enabled, &DISPLACEMENT_ENABLED),
            (display_in_overlay, &DISPLAY_IN_OVERLAY),
            (
                occluded_selection_shows_through,
                &OCCLUDED_SELECTION_SHOWS_THROUGH,
            ),
            (points_shading_enabled, &POINTS_SHADING_ENABLED),
            (material_is_final, &MATERIAL_IS_FINAL),
            (shading_style, &SHADING_STYLE),
            (repr_selector, &REPR_SELECTOR),
            (cull_style, &CULL_STYLE),
        ];

        for (opt_ds, token) in fields {
            if let Some(ds) = opt_ds {
                entries.push((token.clone(), ds));
            }
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdLegacyDisplayStyleSchema container data sources.
///
/// Corresponds to C++ `HdLegacyDisplayStyleSchema::Builder`.
/// All setter methods return `&mut Self` for chaining.
#[derive(Default)]
pub struct HdLegacyDisplayStyleSchemaBuilder {
    refine_level: Option<HdDataSourceBaseHandle>,
    flat_shading_enabled: Option<HdDataSourceBaseHandle>,
    displacement_enabled: Option<HdDataSourceBaseHandle>,
    display_in_overlay: Option<HdDataSourceBaseHandle>,
    occluded_selection_shows_through: Option<HdDataSourceBaseHandle>,
    points_shading_enabled: Option<HdDataSourceBaseHandle>,
    material_is_final: Option<HdDataSourceBaseHandle>,
    shading_style: Option<HdDataSourceBaseHandle>,
    repr_selector: Option<HdDataSourceBaseHandle>,
    cull_style: Option<HdDataSourceBaseHandle>,
}

impl HdLegacyDisplayStyleSchemaBuilder {
    /// Create empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set refine level data source.
    pub fn set_refine_level(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.refine_level = Some(v);
        self
    }

    /// Set flat shading enabled data source.
    pub fn set_flat_shading_enabled(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.flat_shading_enabled = Some(v);
        self
    }

    /// Set displacement enabled data source.
    pub fn set_displacement_enabled(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.displacement_enabled = Some(v);
        self
    }

    /// Set display in overlay data source.
    pub fn set_display_in_overlay(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.display_in_overlay = Some(v);
        self
    }

    /// Set occluded selection shows through data source.
    pub fn set_occluded_selection_shows_through(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.occluded_selection_shows_through = Some(v);
        self
    }

    /// Set points shading enabled data source.
    pub fn set_points_shading_enabled(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.points_shading_enabled = Some(v);
        self
    }

    /// Set material is final data source.
    pub fn set_material_is_final(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.material_is_final = Some(v);
        self
    }

    /// Set shading style data source.
    pub fn set_shading_style(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.shading_style = Some(v);
        self
    }

    /// Set repr selector data source.
    pub fn set_repr_selector(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.repr_selector = Some(v);
        self
    }

    /// Set cull style data source.
    pub fn set_cull_style(&mut self, v: HdDataSourceBaseHandle) -> &mut Self {
        self.cull_style = Some(v);
        self
    }

    /// Build the container data source from fields set so far.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdLegacyDisplayStyleSchema::build_retained(
            self.refine_level,
            self.flat_shading_enabled,
            self.displacement_enabled,
            self.display_in_overlay,
            self.occluded_selection_shows_through,
            self.points_shading_enabled,
            self.material_is_final,
            self.shading_style,
            self.repr_selector,
            self.cull_style,
        )
    }
}
