//! Render settings schema for Hydra.
//!
//! Defines render settings including resolution, output products, purposes,
//! and shutter interval for motion blur.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource,
    HdVectorDataSourceHandle, cast_to_container, cast_to_vector,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::Vec2d;
use usd_tf::Token;

// Schema tokens
/// Render settings schema token
pub static RENDER_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("renderSettings"));
/// Renderer-specific settings token
pub static NAMESPACED_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("namespacedSettings"));
/// Active flag token
pub static ACTIVE: Lazy<Token> = Lazy::new(|| Token::new("active"));
/// Render products array token
pub static RENDER_PRODUCTS: Lazy<Token> = Lazy::new(|| Token::new("renderProducts"));
/// Included purposes array token
pub static INCLUDED_PURPOSES: Lazy<Token> = Lazy::new(|| Token::new("includedPurposes"));
/// Material binding purposes token
pub static MATERIAL_BINDING_PURPOSES: Lazy<Token> =
    Lazy::new(|| Token::new("materialBindingPurposes"));
/// Output color space token
pub static RENDERING_COLOR_SPACE: Lazy<Token> = Lazy::new(|| Token::new("renderingColorSpace"));
/// Shutter interval for motion blur token
pub static SHUTTER_INTERVAL: Lazy<Token> = Lazy::new(|| Token::new("shutterInterval"));
/// Frame token (for frame locator)
pub static FRAME: Lazy<Token> = Lazy::new(|| Token::new("frame"));

// Typed data sources
/// Data source for bool values
pub type HdBoolDataSource = dyn HdTypedSampledDataSource<bool>;
/// Arc handle to bool data source
pub type HdBoolDataSourceHandle = Arc<HdBoolDataSource>;

/// Data source for Token values
pub type HdTokenDataSource = dyn HdTypedSampledDataSource<Token>;
/// Arc handle to Token data source
pub type HdTokenDataSourceHandle = Arc<HdTokenDataSource>;

/// Data source for Token arrays
pub type HdTokenArrayDataSource = dyn HdTypedSampledDataSource<Vec<Token>>;
/// Arc handle to Token array data source
pub type HdTokenArrayDataSourceHandle = Arc<HdTokenArrayDataSource>;

/// Data source for Vec2d values
pub type HdVec2dDataSource = dyn HdTypedSampledDataSource<Vec2d>;
/// Arc handle to Vec2d data source
pub type HdVec2dDataSourceHandle = Arc<HdVec2dDataSource>;

/// Schema representing render settings.
///
/// Provides access to:
/// - `namespacedSettings` - Renderer-specific settings
/// - `active` - Whether these settings are active
/// - `renderProducts` - Vector of RenderProduct schemas (output AOVs)
/// - `includedPurposes` - Array of purposes to include in render
/// - `materialBindingPurposes` - Array of material binding purposes
/// - `renderingColorSpace` - Output color space
/// - `shutterInterval` - Frame-relative shutter interval for motion blur
///
/// # Location
///
/// Default locator: `renderSettings`
#[derive(Debug, Clone)]
pub struct HdRenderSettingsSchema {
    schema: HdSchema,
}

impl HdRenderSettingsSchema {
    /// Constructs a render settings schema from a container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves render settings schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&RENDER_SETTINGS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema is non-empty.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Gets the underlying container data source.
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Gets renderer-specific settings container.
    pub fn get_namespaced_settings(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&NAMESPACED_SETTINGS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Gets active flag indicating if these settings are active.
    pub fn get_active(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema.get_typed(&ACTIVE)
    }

    /// Gets render products container (output AOVs).
    pub fn get_render_products(&self) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&RENDER_PRODUCTS) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Gets render products as a vector data source.
    ///
    /// The C++ HdRenderProductVectorSchema wraps HdVectorDataSource; each
    /// element is a container for HdRenderProductSchema.
    pub fn get_render_products_vector(&self) -> Option<HdVectorDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(&RENDER_PRODUCTS) {
                return cast_to_vector(&child);
            }
        }
        None
    }

    /// Gets array of purposes to include in render.
    pub fn get_included_purposes(&self) -> Option<HdTokenArrayDataSourceHandle> {
        self.schema.get_typed(&INCLUDED_PURPOSES)
    }

    /// Gets array of material binding purposes.
    pub fn get_material_binding_purposes(&self) -> Option<HdTokenArrayDataSourceHandle> {
        self.schema.get_typed(&MATERIAL_BINDING_PURPOSES)
    }

    /// Gets output color space token.
    pub fn get_rendering_color_space(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&RENDERING_COLOR_SPACE)
    }

    /// Gets frame-relative shutter interval for motion blur as Vec2d.
    pub fn get_shutter_interval(&self) -> Option<HdVec2dDataSourceHandle> {
        self.schema.get_typed(&SHUTTER_INTERVAL)
    }

    /// Returns the schema token for render settings.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &RENDER_SETTINGS
    }

    /// Returns the default locator for render settings schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone()])
    }

    /// Locator for active field.
    pub fn get_active_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), ACTIVE.clone()])
    }

    /// Locator for frame field (pass-through from scene globals).
    pub fn get_frame_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), FRAME.clone()])
    }

    /// Locator for render products.
    pub fn get_render_products_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), RENDER_PRODUCTS.clone()])
    }

    /// Locator for shutter interval.
    pub fn get_shutter_interval_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), SHUTTER_INTERVAL.clone()])
    }

    /// Locator for includedPurposes field.
    pub fn get_included_purposes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), INCLUDED_PURPOSES.clone()])
    }

    /// Locator for materialBindingPurposes field.
    pub fn get_material_binding_purposes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), MATERIAL_BINDING_PURPOSES.clone()])
    }

    /// Locator for namespacedSettings container.
    pub fn get_namespaced_settings_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), NAMESPACED_SETTINGS.clone()])
    }

    /// Locator for renderingColorSpace field.
    pub fn get_rendering_color_space_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[RENDER_SETTINGS.clone(), RENDERING_COLOR_SPACE.clone()])
    }

    /// Builds a retained container with render settings parameters.
    ///
    /// # Parameters
    /// All render settings as optional data source handles.
    pub fn build_retained(
        namespaced_settings: Option<HdContainerDataSourceHandle>,
        active: Option<HdBoolDataSourceHandle>,
        render_products: Option<HdContainerDataSourceHandle>,
        included_purposes: Option<HdTokenArrayDataSourceHandle>,
        material_binding_purposes: Option<HdTokenArrayDataSourceHandle>,
        rendering_color_space: Option<HdTokenDataSourceHandle>,
        shutter_interval: Option<HdVec2dDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(v) = namespaced_settings {
            entries.push((NAMESPACED_SETTINGS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = active {
            entries.push((ACTIVE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = render_products {
            entries.push((RENDER_PRODUCTS.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = included_purposes {
            entries.push((INCLUDED_PURPOSES.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = material_binding_purposes {
            entries.push((
                MATERIAL_BINDING_PURPOSES.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = rendering_color_space {
            entries.push((RENDERING_COLOR_SPACE.clone(), v as HdDataSourceBaseHandle));
        }
        if let Some(v) = shutter_interval {
            entries.push((SHUTTER_INTERVAL.clone(), v as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for constructing HdRenderSettingsSchema containers.
///
/// Provides a fluent interface for setting render settings parameters.
#[allow(dead_code)]
#[derive(Default)]
pub struct HdRenderSettingsSchemaBuilder {
    namespaced_settings: Option<HdContainerDataSourceHandle>,
    active: Option<HdBoolDataSourceHandle>,
    render_products: Option<HdContainerDataSourceHandle>,
    included_purposes: Option<HdTokenArrayDataSourceHandle>,
    material_binding_purposes: Option<HdTokenArrayDataSourceHandle>,
    rendering_color_space: Option<HdTokenDataSourceHandle>,
    shutter_interval: Option<HdVec2dDataSourceHandle>,
}

#[allow(dead_code)]
impl HdRenderSettingsSchemaBuilder {
    /// Creates a new empty render settings schema builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets renderer-specific settings.
    pub fn set_namespaced_settings(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.namespaced_settings = Some(v);
        self
    }

    /// Sets active flag.
    pub fn set_active(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.active = Some(v);
        self
    }

    /// Sets render products.
    pub fn set_render_products(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.render_products = Some(v);
        self
    }

    /// Sets included purposes array.
    pub fn set_included_purposes(mut self, v: HdTokenArrayDataSourceHandle) -> Self {
        self.included_purposes = Some(v);
        self
    }

    /// Sets material binding purposes.
    pub fn set_material_binding_purposes(mut self, v: HdTokenArrayDataSourceHandle) -> Self {
        self.material_binding_purposes = Some(v);
        self
    }

    /// Sets output color space.
    pub fn set_rendering_color_space(mut self, v: HdTokenDataSourceHandle) -> Self {
        self.rendering_color_space = Some(v);
        self
    }

    /// Sets shutter interval for motion blur.
    pub fn set_shutter_interval(mut self, v: HdVec2dDataSourceHandle) -> Self {
        self.shutter_interval = Some(v);
        self
    }

    /// Builds the container with all set render settings parameters.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdRenderSettingsSchema::build_retained(
            self.namespaced_settings,
            self.active,
            self.render_products,
            self.included_purposes,
            self.material_binding_purposes,
            self.rendering_color_space,
            self.shutter_interval,
        )
    }
}
