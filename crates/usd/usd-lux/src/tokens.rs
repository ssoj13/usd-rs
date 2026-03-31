//! UsdLux tokens - token definitions for lighting schemas.
//!
//! Port of pxr/usd/usdLux/tokens.h

use std::sync::OnceLock;
use usd_tf::Token;

/// Static token definitions for UsdLux module.
#[derive(Debug)]
pub struct UsdLuxTokens {
    // Texture format tokens
    /// Token for "angular" texture format.
    pub angular: Token,
    /// Token for "automatic" texture format selection.
    pub automatic: Token,
    /// Token for "cubeMapVerticalCross" texture format.
    pub cube_map_vertical_cross: Token,
    /// Token for "latlong" texture format.
    pub latlong: Token,
    /// Token for "mirroredBall" texture format.
    pub mirrored_ball: Token,

    // Collection tokens
    /// Token for "collection:filterLink:includeRoot" attribute.
    pub collection_filter_link_include_root: Token,
    /// Token for "collection:lightLink:includeRoot" attribute.
    pub collection_light_link_include_root: Token,
    /// Token for "collection:shadowLink:includeRoot" attribute.
    pub collection_shadow_link_include_root: Token,
    /// Token for "filterLink" collection.
    pub filter_link: Token,
    /// Token for "lightLink" collection.
    pub light_link: Token,
    /// Token for "shadowLink" collection.
    pub shadow_link: Token,

    // Cache behavior tokens
    /// Token for "consumeAndContinue" cache behavior.
    pub consume_and_continue: Token,
    /// Token for "consumeAndHalt" cache behavior.
    pub consume_and_halt: Token,
    /// Token for "ignore" cache behavior.
    pub ignore: Token,

    // Material sync mode tokens
    /// Token for "independent" material sync mode.
    pub independent: Token,
    /// Token for "materialGlowTintsLight" material sync mode.
    pub material_glow_tints_light: Token,
    /// Token for "noMaterialResponse" material sync mode.
    pub no_material_response: Token,

    // Light input tokens
    /// Token for "inputs:angle" attribute.
    pub inputs_angle: Token,
    /// Token for "inputs:color" attribute.
    pub inputs_color: Token,
    /// Token for "inputs:colorTemperature" attribute.
    pub inputs_color_temperature: Token,
    /// Token for "inputs:diffuse" attribute.
    pub inputs_diffuse: Token,
    /// Token for "inputs:enableColorTemperature" attribute.
    pub inputs_enable_color_temperature: Token,
    /// Token for "inputs:exposure" attribute.
    pub inputs_exposure: Token,
    /// Token for "inputs:height" attribute.
    pub inputs_height: Token,
    /// Token for "inputs:intensity" attribute.
    pub inputs_intensity: Token,
    /// Token for "inputs:length" attribute.
    pub inputs_length: Token,
    /// Token for "inputs:normalize" attribute.
    pub inputs_normalize: Token,
    /// Token for "inputs:radius" attribute.
    pub inputs_radius: Token,
    /// Token for "inputs:specular" attribute.
    pub inputs_specular: Token,
    /// Token for "inputs:texture:file" attribute.
    pub inputs_texture_file: Token,
    /// Token for "inputs:texture:format" attribute.
    pub inputs_texture_format: Token,
    /// Token for "inputs:width" attribute.
    pub inputs_width: Token,

    // Shadow input tokens
    /// Token for "inputs:shadow:color" attribute.
    pub inputs_shadow_color: Token,
    /// Token for "inputs:shadow:distance" attribute.
    pub inputs_shadow_distance: Token,
    /// Token for "inputs:shadow:enable" attribute.
    pub inputs_shadow_enable: Token,
    /// Token for "inputs:shadow:falloff" attribute.
    pub inputs_shadow_falloff: Token,
    /// Token for "inputs:shadow:falloffGamma" attribute.
    pub inputs_shadow_falloff_gamma: Token,

    // Shaping input tokens
    /// Token for "inputs:shaping:cone:angle" attribute.
    pub inputs_shaping_cone_angle: Token,
    /// Token for "inputs:shaping:cone:softness" attribute.
    pub inputs_shaping_cone_softness: Token,
    /// Token for "inputs:shaping:focus" attribute.
    pub inputs_shaping_focus: Token,
    /// Token for "inputs:shaping:focusTint" attribute.
    pub inputs_shaping_focus_tint: Token,
    /// Token for "inputs:shaping:ies:angleScale" attribute.
    pub inputs_shaping_ies_angle_scale: Token,
    /// Token for "inputs:shaping:ies:file" attribute.
    pub inputs_shaping_ies_file: Token,
    /// Token for "inputs:shaping:ies:normalize" attribute.
    pub inputs_shaping_ies_normalize: Token,

    // Light property tokens
    /// Token for "geometry" attribute.
    pub geometry: Token,
    /// Token for "guideRadius" attribute.
    pub guide_radius: Token,
    /// Token for "light:filters" relationship.
    pub light_filters: Token,
    /// Token for "lightFilter:shaderId" attribute.
    pub light_filter_shader_id: Token,
    /// Token for "lightList" relationship.
    pub light_list: Token,
    /// Token for "lightList:cacheBehavior" attribute.
    pub light_list_cache_behavior: Token,
    /// Token for "light:materialSyncMode" attribute.
    pub light_material_sync_mode: Token,
    /// Token for "light:shaderId" attribute.
    pub light_shader_id: Token,
    /// Token for "orientToStageUpAxis" attribute.
    pub orient_to_stage_up_axis: Token,
    /// Token for "poleAxis" attribute.
    pub pole_axis: Token,
    /// Token for "portals" relationship.
    pub portals: Token,
    /// Token for "treatAsLine" attribute.
    pub treat_as_line: Token,
    /// Token for "treatAsPoint" attribute.
    pub treat_as_point: Token,

    // Pole axis values
    /// Token for "scene" pole axis value.
    pub scene: Token,
    /// Token for "Y" pole axis value.
    pub y_axis: Token,
    /// Token for "Z" pole axis value.
    pub z_axis: Token,

    // Schema identifiers
    /// Token for "BoundableLightBase" schema.
    pub boundable_light_base: Token,
    /// Token for "CylinderLight" schema.
    pub cylinder_light: Token,
    /// Token for "DiskLight" schema.
    pub disk_light: Token,
    /// Token for "DistantLight" schema.
    pub distant_light: Token,
    /// Token for "DomeLight" schema.
    pub dome_light: Token,
    /// Token for "DomeLight_1" schema.
    pub dome_light_1: Token,
    /// Token for "GeometryLight" schema.
    pub geometry_light: Token,
    /// Token for "LightAPI" schema.
    pub light_api: Token,
    /// Token for "LightFilter" schema.
    pub light_filter: Token,
    /// Token for "LightListAPI" schema.
    pub light_list_api: Token,
    /// Token for "ListAPI" schema.
    pub list_api: Token,
    /// Token for "MeshLight" schema.
    pub mesh_light: Token,
    /// Token for "MeshLightAPI" schema.
    pub mesh_light_api: Token,
    /// Token for "NonboundableLightBase" schema.
    pub nonboundable_light_base: Token,
    /// Token for "PluginLight" schema.
    pub plugin_light: Token,
    /// Token for "PluginLightFilter" schema.
    pub plugin_light_filter: Token,
    /// Token for "PortalLight" schema.
    pub portal_light: Token,
    /// Token for "RectLight" schema.
    pub rect_light: Token,
    /// Token for "ShadowAPI" schema.
    pub shadow_api: Token,
    /// Token for "ShapingAPI" schema.
    pub shaping_api: Token,
    /// Token for "SphereLight" schema.
    pub sphere_light: Token,
    /// Token for "VolumeLight" schema.
    pub volume_light: Token,
    /// Token for "VolumeLightAPI" schema.
    pub volume_light_api: Token,
}

impl UsdLuxTokens {
    fn new() -> Self {
        Self {
            // Texture format tokens
            angular: Token::new("angular"),
            automatic: Token::new("automatic"),
            cube_map_vertical_cross: Token::new("cubeMapVerticalCross"),
            latlong: Token::new("latlong"),
            mirrored_ball: Token::new("mirroredBall"),

            // Collection tokens
            collection_filter_link_include_root: Token::new("collection:filterLink:includeRoot"),
            collection_light_link_include_root: Token::new("collection:lightLink:includeRoot"),
            collection_shadow_link_include_root: Token::new("collection:shadowLink:includeRoot"),
            filter_link: Token::new("filterLink"),
            light_link: Token::new("lightLink"),
            shadow_link: Token::new("shadowLink"),

            // Cache behavior tokens
            consume_and_continue: Token::new("consumeAndContinue"),
            consume_and_halt: Token::new("consumeAndHalt"),
            ignore: Token::new("ignore"),

            // Material sync mode tokens
            independent: Token::new("independent"),
            material_glow_tints_light: Token::new("materialGlowTintsLight"),
            no_material_response: Token::new("noMaterialResponse"),

            // Light input tokens
            inputs_angle: Token::new("inputs:angle"),
            inputs_color: Token::new("inputs:color"),
            inputs_color_temperature: Token::new("inputs:colorTemperature"),
            inputs_diffuse: Token::new("inputs:diffuse"),
            inputs_enable_color_temperature: Token::new("inputs:enableColorTemperature"),
            inputs_exposure: Token::new("inputs:exposure"),
            inputs_height: Token::new("inputs:height"),
            inputs_intensity: Token::new("inputs:intensity"),
            inputs_length: Token::new("inputs:length"),
            inputs_normalize: Token::new("inputs:normalize"),
            inputs_radius: Token::new("inputs:radius"),
            inputs_specular: Token::new("inputs:specular"),
            inputs_texture_file: Token::new("inputs:texture:file"),
            inputs_texture_format: Token::new("inputs:texture:format"),
            inputs_width: Token::new("inputs:width"),

            // Shadow input tokens
            inputs_shadow_color: Token::new("inputs:shadow:color"),
            inputs_shadow_distance: Token::new("inputs:shadow:distance"),
            inputs_shadow_enable: Token::new("inputs:shadow:enable"),
            inputs_shadow_falloff: Token::new("inputs:shadow:falloff"),
            inputs_shadow_falloff_gamma: Token::new("inputs:shadow:falloffGamma"),

            // Shaping input tokens
            inputs_shaping_cone_angle: Token::new("inputs:shaping:cone:angle"),
            inputs_shaping_cone_softness: Token::new("inputs:shaping:cone:softness"),
            inputs_shaping_focus: Token::new("inputs:shaping:focus"),
            inputs_shaping_focus_tint: Token::new("inputs:shaping:focusTint"),
            inputs_shaping_ies_angle_scale: Token::new("inputs:shaping:ies:angleScale"),
            inputs_shaping_ies_file: Token::new("inputs:shaping:ies:file"),
            inputs_shaping_ies_normalize: Token::new("inputs:shaping:ies:normalize"),

            // Light property tokens
            geometry: Token::new("geometry"),
            guide_radius: Token::new("guideRadius"),
            light_filters: Token::new("light:filters"),
            light_filter_shader_id: Token::new("lightFilter:shaderId"),
            light_list: Token::new("lightList"),
            light_list_cache_behavior: Token::new("lightList:cacheBehavior"),
            light_material_sync_mode: Token::new("light:materialSyncMode"),
            light_shader_id: Token::new("light:shaderId"),
            orient_to_stage_up_axis: Token::new("orientToStageUpAxis"),
            pole_axis: Token::new("poleAxis"),
            portals: Token::new("portals"),
            treat_as_line: Token::new("treatAsLine"),
            treat_as_point: Token::new("treatAsPoint"),

            // Pole axis values
            scene: Token::new("scene"),
            y_axis: Token::new("Y"),
            z_axis: Token::new("Z"),

            // Schema identifiers
            boundable_light_base: Token::new("BoundableLightBase"),
            cylinder_light: Token::new("CylinderLight"),
            disk_light: Token::new("DiskLight"),
            distant_light: Token::new("DistantLight"),
            dome_light: Token::new("DomeLight"),
            dome_light_1: Token::new("DomeLight_1"),
            geometry_light: Token::new("GeometryLight"),
            light_api: Token::new("LightAPI"),
            light_filter: Token::new("LightFilter"),
            light_list_api: Token::new("LightListAPI"),
            list_api: Token::new("ListAPI"),
            mesh_light: Token::new("MeshLight"),
            mesh_light_api: Token::new("MeshLightAPI"),
            nonboundable_light_base: Token::new("NonboundableLightBase"),
            plugin_light: Token::new("PluginLight"),
            plugin_light_filter: Token::new("PluginLightFilter"),
            portal_light: Token::new("PortalLight"),
            rect_light: Token::new("RectLight"),
            shadow_api: Token::new("ShadowAPI"),
            shaping_api: Token::new("ShapingAPI"),
            sphere_light: Token::new("SphereLight"),
            volume_light: Token::new("VolumeLight"),
            volume_light_api: Token::new("VolumeLightAPI"),
        }
    }
}

static TOKENS: OnceLock<UsdLuxTokens> = OnceLock::new();

/// Get the global UsdLux tokens.
pub fn tokens() -> &'static UsdLuxTokens {
    TOKENS.get_or_init(UsdLuxTokens::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        let t = tokens();
        assert_eq!(t.sphere_light.as_str(), "SphereLight");
        assert_eq!(t.dome_light.as_str(), "DomeLight");
        assert_eq!(t.inputs_intensity.as_str(), "inputs:intensity");
    }
}
