//! UsdRenderProductSchema - Hydra schema for render product data.
//!
//! Port of pxr/usdImaging/usdImaging/usdRenderProductSchema.h
//!
//! Provides data source schema for render product settings in Hydra.

use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator, cast_to_container,
};
use usd_tf::Token;

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_RENDER_PRODUCT: LazyLock<Token> =
        LazyLock::new(|| Token::new("__usdRenderProduct"));
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
    pub static PRODUCT_TYPE: LazyLock<Token> = LazyLock::new(|| Token::new("productType"));
    pub static PRODUCT_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("productName"));
    pub static ORDERED_VARS: LazyLock<Token> = LazyLock::new(|| Token::new("orderedVars"));
    pub static NAMESPACED_SETTINGS: LazyLock<Token> =
        LazyLock::new(|| Token::new("namespacedSettings"));
}

// ============================================================================
// UsdRenderProductSchema
// ============================================================================

/// Schema for render product data in Hydra.
///
/// Corresponds to UsdRenderProduct. Contains output product settings
/// including resolution, camera, product type, and render vars.
#[derive(Debug, Clone)]
pub struct UsdRenderProductSchema {
    container: Option<HdContainerDataSourceHandle>,
}

impl UsdRenderProductSchema {
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
        tokens::USD_RENDER_PRODUCT.clone()
    }

    /// Get the default locator for this schema.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_RENDER_PRODUCT.clone())
    }

    /// Get the resolution locator.
    pub fn get_resolution_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::RESOLUTION.clone(),
        )
    }

    /// Get the pixel aspect ratio locator.
    pub fn get_pixel_aspect_ratio_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::PIXEL_ASPECT_RATIO.clone(),
        )
    }

    /// Get the aspect ratio conform policy locator.
    pub fn get_aspect_ratio_conform_policy_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::ASPECT_RATIO_CONFORM_POLICY.clone(),
        )
    }

    /// Get the data window NDC locator.
    pub fn get_data_window_ndc_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::DATA_WINDOW_NDC.clone(),
        )
    }

    /// Get the disable motion blur locator.
    pub fn get_disable_motion_blur_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::DISABLE_MOTION_BLUR.clone(),
        )
    }

    /// Get the disable depth of field locator.
    pub fn get_disable_depth_of_field_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::DISABLE_DEPTH_OF_FIELD.clone(),
        )
    }

    /// Get the camera locator.
    pub fn get_camera_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::CAMERA.clone(),
        )
    }

    /// Get the product type locator.
    pub fn get_product_type_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::PRODUCT_TYPE.clone(),
        )
    }

    /// Get the product name locator.
    pub fn get_product_name_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::PRODUCT_NAME.clone(),
        )
    }

    /// Get the ordered vars locator.
    pub fn get_ordered_vars_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::ORDERED_VARS.clone(),
        )
    }

    /// Get the namespaced settings locator.
    pub fn get_namespaced_settings_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_tokens_2(
            tokens::USD_RENDER_PRODUCT.clone(),
            tokens::NAMESPACED_SETTINGS.clone(),
        )
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        let ds = parent.get(&tokens::USD_RENDER_PRODUCT)?;
        let container = cast_to_container(&ds)?;
        Some(Self {
            container: Some(container),
        })
    }
}

// ============================================================================
// UsdRenderProductSchemaBuilder
// ============================================================================

/// Builder for UsdRenderProductSchema data sources.
#[derive(Debug, Default)]
pub struct UsdRenderProductSchemaBuilder {
    resolution: Option<HdDataSourceBaseHandle>,
    pixel_aspect_ratio: Option<HdDataSourceBaseHandle>,
    aspect_ratio_conform_policy: Option<HdDataSourceBaseHandle>,
    data_window_ndc: Option<HdDataSourceBaseHandle>,
    disable_motion_blur: Option<HdDataSourceBaseHandle>,
    disable_depth_of_field: Option<HdDataSourceBaseHandle>,
    camera: Option<HdDataSourceBaseHandle>,
    product_type: Option<HdDataSourceBaseHandle>,
    product_name: Option<HdDataSourceBaseHandle>,
    ordered_vars: Option<HdDataSourceBaseHandle>,
    namespaced_settings: Option<HdDataSourceBaseHandle>,
}

impl UsdRenderProductSchemaBuilder {
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

    /// Set the product type data source.
    pub fn set_product_type(mut self, product_type: HdDataSourceBaseHandle) -> Self {
        self.product_type = Some(product_type);
        self
    }

    /// Set the product name data source.
    pub fn set_product_name(mut self, name: HdDataSourceBaseHandle) -> Self {
        self.product_name = Some(name);
        self
    }

    /// Set the ordered vars data source.
    pub fn set_ordered_vars(mut self, vars: HdDataSourceBaseHandle) -> Self {
        self.ordered_vars = Some(vars);
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
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::with_capacity(11);
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
        if let Some(v) = self.product_type {
            entries.push((tokens::PRODUCT_TYPE.clone(), v));
        }
        if let Some(v) = self.product_name {
            entries.push((tokens::PRODUCT_NAME.clone(), v));
        }
        if let Some(v) = self.ordered_vars {
            entries.push((tokens::ORDERED_VARS.clone(), v));
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
            UsdRenderProductSchema::get_schema_token().as_str(),
            "__usdRenderProduct"
        );
    }

    #[test]
    fn test_resolution_locator() {
        let locator = UsdRenderProductSchema::get_resolution_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_camera_locator() {
        let locator = UsdRenderProductSchema::get_camera_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_product_type_locator() {
        let locator = UsdRenderProductSchema::get_product_type_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_ordered_vars_locator() {
        let locator = UsdRenderProductSchema::get_ordered_vars_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_namespaced_settings_locator() {
        let locator = UsdRenderProductSchema::get_namespaced_settings_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_builder() {
        let _schema = UsdRenderProductSchemaBuilder::new().build();
    }

    #[test]
    fn test_builder_chain() {
        let _schema = UsdRenderProductSchemaBuilder::new()
            .set_product_name(usd_hd::HdRetainedContainerDataSource::new_empty())
            .build();
    }
}
