//! UsdRenderSettingsSchema - Hydra schema for render settings data.
//!
//! Port of pxr/usdImaging/usdImaging/usdRenderSettingsSchema.h
//!
//! Provides data source schema for render settings configuration in Hydra.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_RENDER_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("__usdRenderSettings"));
    pub static RESOLUTION: LazyLock<Token> = LazyLock::new(|| Token::new("resolution"));
    pub static PIXEL_ASPECT_RATIO: LazyLock<Token> =
        LazyLock::new(|| Token::new("pixelAspectRatio"));
    pub static ASPECT_RATIO_CONFORM_POLICY: LazyLock<Token> =
        LazyLock::new(|| Token::new("aspectRatioConformPolicy"));
    pub static DATA_WINDOW_NDC: LazyLock<Token> = LazyLock::new(|| Token::new("dataWindowNDC"));
    pub static DISABLE_MOTION_BLUR: LazyLock<Token> =
        LazyLock::new(|| Token::new("disableMotionBlur"));
    pub static DISABLE_DEPTH_OF_FIELD: LazyLock<Token> =
        LazyLock::new(|| Token::new("disableDepthOfField"));
    pub static CAMERA: LazyLock<Token> = LazyLock::new(|| Token::new("camera"));
    pub static INCLUDED_PURPOSES: LazyLock<Token> =
        LazyLock::new(|| Token::new("includedPurposes"));
    pub static MATERIAL_BINDING_PURPOSES: LazyLock<Token> =
        LazyLock::new(|| Token::new("materialBindingPurposes"));
    pub static RENDERING_COLOR_SPACE: LazyLock<Token> =
        LazyLock::new(|| Token::new("renderingColorSpace"));
    pub static PRODUCTS: LazyLock<Token> = LazyLock::new(|| Token::new("products"));
    pub static NAMESPACED_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("namespacedSettings"));
}

// ============================================================================
// UsdRenderSettingsSchema
// ============================================================================

/// Schema for render settings data in Hydra.
///
/// Corresponds to UsdRenderSettings. Contains global render configuration
/// including resolution, camera, color space, and render products.
#[derive(Debug, Clone)]
pub struct UsdRenderSettingsSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl UsdRenderSettingsSchema {
    /// Create schema from container.
    pub fn new(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self { container }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.container.is_some()
    }

    /// Get the schema token.
    pub fn get_schema_token() -> Token {
        tokens::USD_RENDER_SETTINGS.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_RENDER_SETTINGS.clone())
    }

    /// Get the resolution locator.
    pub fn get_resolution_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::RESOLUTION.clone(),
        )
    }

    /// Get the pixel aspect ratio locator.
    pub fn get_pixel_aspect_ratio_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::PIXEL_ASPECT_RATIO.clone(),
        )
    }

    /// Get the aspect ratio conform policy locator.
    pub fn get_aspect_ratio_conform_policy_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::ASPECT_RATIO_CONFORM_POLICY.clone(),
        )
    }

    /// Get the data window NDC locator.
    pub fn get_data_window_ndc_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::DATA_WINDOW_NDC.clone(),
        )
    }

    /// Get the disable motion blur locator.
    pub fn get_disable_motion_blur_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::DISABLE_MOTION_BLUR.clone(),
        )
    }

    /// Get the disable depth of field locator.
    pub fn get_disable_depth_of_field_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::DISABLE_DEPTH_OF_FIELD.clone(),
        )
    }

    /// Get the camera locator.
    pub fn get_camera_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::CAMERA.clone(),
        )
    }

    /// Get the included purposes locator.
    pub fn get_included_purposes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::INCLUDED_PURPOSES.clone(),
        )
    }

    /// Get the material binding purposes locator.
    pub fn get_material_binding_purposes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::MATERIAL_BINDING_PURPOSES.clone(),
        )
    }

    /// Get the rendering color space locator.
    pub fn get_rendering_color_space_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::RENDERING_COLOR_SPACE.clone(),
        )
    }

    /// Get the products locator.
    pub fn get_products_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::PRODUCTS.clone(),
        )
    }

    /// Get the namespaced settings locator.
    pub fn get_namespaced_settings_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_SETTINGS.clone(),
            tokens::NAMESPACED_SETTINGS.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::USD_RENDER_SETTINGS)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }
}

// ============================================================================
// UsdRenderSettingsSchemaBuilder
// ============================================================================

/// Builder for UsdRenderSettingsSchema data sources.
#[derive(Debug, Default)]
pub struct UsdRenderSettingsSchemaBuilder {
    resolution: Option<HdDataSourceBaseHandle>,
    pixel_aspect_ratio: Option<HdDataSourceBaseHandle>,
    aspect_ratio_conform_policy: Option<HdDataSourceBaseHandle>,
    data_window_ndc: Option<HdDataSourceBaseHandle>,
    disable_motion_blur: Option<HdDataSourceBaseHandle>,
    disable_depth_of_field: Option<HdDataSourceBaseHandle>,
    camera: Option<HdDataSourceBaseHandle>,
    included_purposes: Option<HdDataSourceBaseHandle>,
    material_binding_purposes: Option<HdDataSourceBaseHandle>,
    rendering_color_space: Option<HdDataSourceBaseHandle>,
    products: Option<HdDataSourceBaseHandle>,
    namespaced_settings: Option<HdDataSourceBaseHandle>,
}

impl UsdRenderSettingsSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the resolution data source.
    pub fn set_resolution(mut self, resolution: HdDataSourceBaseHandle) -> Self {
        self.resolution = Some(resolution);
        self
    }

    /// Set the pixel aspect ratio data source.
    pub fn set_pixel_aspect_ratio(mut self, ratio: HdDataSourceBaseHandle) -> Self {
        self.pixel_aspect_ratio = Some(ratio);
        self
    }

    /// Set the aspect ratio conform policy data source.
    pub fn set_aspect_ratio_conform_policy(mut self, policy: HdDataSourceBaseHandle) -> Self {
        self.aspect_ratio_conform_policy = Some(policy);
        self
    }

    /// Set the data window NDC data source.
    pub fn set_data_window_ndc(mut self, window: HdDataSourceBaseHandle) -> Self {
        self.data_window_ndc = Some(window);
        self
    }

    /// Set the disable motion blur data source.
    pub fn set_disable_motion_blur(mut self, disable: HdDataSourceBaseHandle) -> Self {
        self.disable_motion_blur = Some(disable);
        self
    }

    /// Set the disable depth of field data source.
    pub fn set_disable_depth_of_field(mut self, disable: HdDataSourceBaseHandle) -> Self {
        self.disable_depth_of_field = Some(disable);
        self
    }

    /// Set the camera data source.
    pub fn set_camera(mut self, camera: HdDataSourceBaseHandle) -> Self {
        self.camera = Some(camera);
        self
    }

    /// Set the included purposes data source.
    pub fn set_included_purposes(mut self, purposes: HdDataSourceBaseHandle) -> Self {
        self.included_purposes = Some(purposes);
        self
    }

    /// Set the material binding purposes data source.
    pub fn set_material_binding_purposes(mut self, purposes: HdDataSourceBaseHandle) -> Self {
        self.material_binding_purposes = Some(purposes);
        self
    }

    /// Set the rendering color space data source.
    pub fn set_rendering_color_space(mut self, color_space: HdDataSourceBaseHandle) -> Self {
        self.rendering_color_space = Some(color_space);
        self
    }

    /// Set the products data source.
    pub fn set_products(mut self, products: HdDataSourceBaseHandle) -> Self {
        self.products = Some(products);
        self
    }

    /// Set the namespaced settings data source.
    pub fn set_namespaced_settings(mut self, settings: HdDataSourceBaseHandle) -> Self {
        self.namespaced_settings = Some(settings);
        self
    }

    /// Build the container data source from set fields.
    ///
    /// Matches C++ BuildRetained: only includes non-None fields.
    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(12);
        if let Some(v) = self.resolution {
            entries.push((tokens::RESOLUTION.clone(), v));
        }
        if let Some(v) = self.pixel_aspect_ratio {
            entries.push((tokens::PIXEL_ASPECT_RATIO.clone(), v));
        }
        if let Some(v) = self.aspect_ratio_conform_policy {
            entries.push((tokens::ASPECT_RATIO_CONFORM_POLICY.clone(), v));
        }
        if let Some(v) = self.data_window_ndc {
            entries.push((tokens::DATA_WINDOW_NDC.clone(), v));
        }
        if let Some(v) = self.disable_motion_blur {
            entries.push((tokens::DISABLE_MOTION_BLUR.clone(), v));
        }
        if let Some(v) = self.disable_depth_of_field {
            entries.push((tokens::DISABLE_DEPTH_OF_FIELD.clone(), v));
        }
        if let Some(v) = self.camera {
            entries.push((tokens::CAMERA.clone(), v));
        }
        if let Some(v) = self.included_purposes {
            entries.push((tokens::INCLUDED_PURPOSES.clone(), v));
        }
        if let Some(v) = self.material_binding_purposes {
            entries.push((tokens::MATERIAL_BINDING_PURPOSES.clone(), v));
        }
        if let Some(v) = self.rendering_color_space {
            entries.push((tokens::RENDERING_COLOR_SPACE.clone(), v));
        }
        if let Some(v) = self.products {
            entries.push((tokens::PRODUCTS.clone(), v));
        }
        if let Some(v) = self.namespaced_settings {
            entries.push((tokens::NAMESPACED_SETTINGS.clone(), v));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            UsdRenderSettingsSchema::get_schema_token().as_str(),
            "__usdRenderSettings"
        );
    }

    #[test]
    fn test_resolution_locator() {
        let locator = UsdRenderSettingsSchema::get_resolution_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_camera_locator() {
        let locator = UsdRenderSettingsSchema::get_camera_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_included_purposes_locator() {
        let locator = UsdRenderSettingsSchema::get_included_purposes_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_rendering_color_space_locator() {
        let locator = UsdRenderSettingsSchema::get_rendering_color_space_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_products_locator() {
        let locator = UsdRenderSettingsSchema::get_products_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_namespaced_settings_locator() {
        let locator = UsdRenderSettingsSchema::get_namespaced_settings_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = UsdRenderSettingsSchemaBuilder::new().build();
    }

    #[test]
    fn test_builder_chain() {
        let _schema = UsdRenderSettingsSchemaBuilder::new()
            .set_camera(usd_hd::HdRetainedContainerDataSource::new_empty())
            .set_products(usd_hd::HdRetainedContainerDataSource::new_empty())
            .build();
    }
}
