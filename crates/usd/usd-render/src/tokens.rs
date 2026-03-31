//! UsdRender token definitions.
//!
//! Provides efficient static tokens for render settings attributes,
//! relationship names, and allowed values. These tokens are auto-generated
//! from the schema and used throughout the usdRender module.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/tokens.h`

use std::sync::LazyLock;
use usd_tf::Token;

/// Container for all UsdRender tokens.
///
/// Provides static, efficient tokens for use in all public USD API.
/// These are auto-generated from the module's schema, representing
/// property names and allowed values.
#[derive(Debug)]
pub struct UsdRenderTokensType {
    // Aspect ratio conform policy values
    /// "adjustApertureHeight" - Adjust aperture height to match image aspect ratio
    pub adjust_aperture_height: Token,
    /// "adjustApertureWidth" - Adjust aperture width to match image aspect ratio
    pub adjust_aperture_width: Token,
    /// "adjustPixelAspectRatio" - Compute pixelAspectRatio to match aperture
    pub adjust_pixel_aspect_ratio: Token,
    /// "expandAperture" - Expand aperture to fit image (default)
    pub expand_aperture: Token,
    /// "cropAperture" - Crop aperture to fit image
    pub crop_aperture: Token,

    // RenderSettingsBase attributes
    /// "aspectRatioConformPolicy"
    pub aspect_ratio_conform_policy: Token,
    /// "camera"
    pub camera: Token,
    /// "dataWindowNDC"
    pub data_window_ndc: Token,
    /// "disableDepthOfField"
    pub disable_depth_of_field: Token,
    /// "disableMotionBlur"
    pub disable_motion_blur: Token,
    /// "instantaneousShutter" (deprecated, use disableMotionBlur)
    pub instantaneous_shutter: Token,
    /// "pixelAspectRatio"
    pub pixel_aspect_ratio: Token,
    /// "resolution"
    pub resolution: Token,

    // RenderSettings attributes
    /// "includedPurposes"
    pub included_purposes: Token,
    /// "materialBindingPurposes"
    pub material_binding_purposes: Token,
    /// "products"
    pub products: Token,
    /// "renderingColorSpace"
    pub rendering_color_space: Token,
    /// "renderSettingsPrimPath" - Stage metadata key
    pub render_settings_prim_path: Token,

    // RenderProduct attributes
    /// "orderedVars"
    pub ordered_vars: Token,
    /// "productName"
    pub product_name: Token,
    /// "productType"
    pub product_type: Token,

    // Product type values
    /// "raster" - Default product type
    pub raster: Token,
    /// "deepRaster" - Deep image product type
    pub deep_raster: Token,

    // RenderVar attributes
    /// "dataType"
    pub data_type: Token,
    /// "sourceName"
    pub source_name: Token,
    /// "sourceType"
    pub source_type: Token,

    // Source type values
    /// "raw" - Pass name directly to renderer (default)
    pub raw: Token,
    /// "primvar" - Source is a primvar name
    pub primvar: Token,
    /// "lpe" - Light Path Expression
    pub lpe: Token,
    /// "intrinsic" - Future namespace for portable baseline RenderVars
    pub intrinsic: Token,

    // Default data type
    /// "color3f" - Default RenderVar data type
    pub color3f: Token,

    // RenderPass attributes
    /// "passType"
    pub pass_type: Token,
    /// "command"
    pub command: Token,
    /// "fileName"
    pub file_name: Token,
    /// "renderSource"
    pub render_source: Token,
    /// "inputPasses"
    pub input_passes: Token,
    /// "renderVisibility" - Collection name for render visibility
    pub render_visibility: Token,

    // Collection attributes for RenderPass
    /// "collection:renderVisibility:includeRoot"
    pub collection_render_visibility_include_root: Token,
    /// "collection:cameraVisibility:includeRoot"
    pub collection_camera_visibility_include_root: Token,

    // Material binding purpose values
    /// "full"
    pub full: Token,
    /// "preview"
    pub preview: Token,

    // Schema type names
    /// "RenderPass"
    pub render_pass: Token,
    /// "RenderProduct"
    pub render_product: Token,
    /// "RenderSettings"
    pub render_settings: Token,
    /// "RenderSettingsBase"
    pub render_settings_base: Token,
    /// "RenderVar"
    pub render_var: Token,
}

impl UsdRenderTokensType {
    /// Create new token container with all tokens initialized.
    fn new() -> Self {
        Self {
            // Aspect ratio conform policy values
            adjust_aperture_height: Token::new("adjustApertureHeight"),
            adjust_aperture_width: Token::new("adjustApertureWidth"),
            adjust_pixel_aspect_ratio: Token::new("adjustPixelAspectRatio"),
            expand_aperture: Token::new("expandAperture"),
            crop_aperture: Token::new("cropAperture"),

            // RenderSettingsBase attributes
            aspect_ratio_conform_policy: Token::new("aspectRatioConformPolicy"),
            camera: Token::new("camera"),
            data_window_ndc: Token::new("dataWindowNDC"),
            disable_depth_of_field: Token::new("disableDepthOfField"),
            disable_motion_blur: Token::new("disableMotionBlur"),
            instantaneous_shutter: Token::new("instantaneousShutter"),
            pixel_aspect_ratio: Token::new("pixelAspectRatio"),
            resolution: Token::new("resolution"),

            // RenderSettings attributes
            included_purposes: Token::new("includedPurposes"),
            material_binding_purposes: Token::new("materialBindingPurposes"),
            products: Token::new("products"),
            rendering_color_space: Token::new("renderingColorSpace"),
            render_settings_prim_path: Token::new("renderSettingsPrimPath"),

            // RenderProduct attributes
            ordered_vars: Token::new("orderedVars"),
            product_name: Token::new("productName"),
            product_type: Token::new("productType"),

            // Product type values
            raster: Token::new("raster"),
            deep_raster: Token::new("deepRaster"),

            // RenderVar attributes
            data_type: Token::new("dataType"),
            source_name: Token::new("sourceName"),
            source_type: Token::new("sourceType"),

            // Source type values
            raw: Token::new("raw"),
            primvar: Token::new("primvar"),
            lpe: Token::new("lpe"),
            intrinsic: Token::new("intrinsic"),

            // Default data type
            color3f: Token::new("color3f"),

            // RenderPass attributes
            pass_type: Token::new("passType"),
            command: Token::new("command"),
            file_name: Token::new("fileName"),
            render_source: Token::new("renderSource"),
            input_passes: Token::new("inputPasses"),
            render_visibility: Token::new("renderVisibility"),

            // Collection attributes
            collection_render_visibility_include_root: Token::new(
                "collection:renderVisibility:includeRoot",
            ),
            collection_camera_visibility_include_root: Token::new(
                "collection:cameraVisibility:includeRoot",
            ),

            // Material binding purpose values
            full: Token::new("full"),
            preview: Token::new("preview"),

            // Schema type names
            render_pass: Token::new("RenderPass"),
            render_product: Token::new("RenderProduct"),
            render_settings: Token::new("RenderSettings"),
            render_settings_base: Token::new("RenderSettingsBase"),
            render_var: Token::new("RenderVar"),
        }
    }

    /// Get all tokens as a vector.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.adjust_aperture_height.clone(),
            self.adjust_aperture_width.clone(),
            self.adjust_pixel_aspect_ratio.clone(),
            self.expand_aperture.clone(),
            self.crop_aperture.clone(),
            self.aspect_ratio_conform_policy.clone(),
            self.camera.clone(),
            self.data_window_ndc.clone(),
            self.disable_depth_of_field.clone(),
            self.disable_motion_blur.clone(),
            self.instantaneous_shutter.clone(),
            self.pixel_aspect_ratio.clone(),
            self.resolution.clone(),
            self.included_purposes.clone(),
            self.material_binding_purposes.clone(),
            self.products.clone(),
            self.rendering_color_space.clone(),
            self.render_settings_prim_path.clone(),
            self.ordered_vars.clone(),
            self.product_name.clone(),
            self.product_type.clone(),
            self.raster.clone(),
            self.deep_raster.clone(),
            self.data_type.clone(),
            self.source_name.clone(),
            self.source_type.clone(),
            self.raw.clone(),
            self.primvar.clone(),
            self.lpe.clone(),
            self.intrinsic.clone(),
            self.color3f.clone(),
            self.pass_type.clone(),
            self.command.clone(),
            self.file_name.clone(),
            self.render_source.clone(),
            self.input_passes.clone(),
            self.render_visibility.clone(),
            self.collection_render_visibility_include_root.clone(),
            self.collection_camera_visibility_include_root.clone(),
            self.full.clone(),
            self.preview.clone(),
            self.render_pass.clone(),
            self.render_product.clone(),
            self.render_settings.clone(),
            self.render_settings_base.clone(),
            self.render_var.clone(),
        ]
    }
}

/// Global singleton for UsdRender tokens.
///
/// Use like: `USD_RENDER_TOKENS.resolution`
pub static USD_RENDER_TOKENS: LazyLock<UsdRenderTokensType> =
    LazyLock::new(UsdRenderTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens_initialized() {
        assert_eq!(USD_RENDER_TOKENS.resolution.get_text(), "resolution");
        assert_eq!(USD_RENDER_TOKENS.camera.get_text(), "camera");
        assert_eq!(USD_RENDER_TOKENS.raster.get_text(), "raster");
    }

    #[test]
    fn test_all_tokens() {
        let tokens = USD_RENDER_TOKENS.all_tokens();
        assert!(!tokens.is_empty());
        assert!(tokens.iter().any(|t| t.get_text() == "resolution"));
    }
}
