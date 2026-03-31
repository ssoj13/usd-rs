//! Integration tests for UsdLux light schemas.
//!
//! Port of `pxr/usd/usdLux/testenv/testUsdLuxLight.py`

use usd_core::{InitialLoadSet, Stage};
use usd_gf::Vec3f;
use usd_lux::{
    CylinderLight, DiskLight, DistantLight, DomeLight, LightAPI, LightFilter, PortalLight,
    RectLight, ShadowAPI, ShapingAPI, SphereLight, blackbody_temperature_as_rgb, tokens,
};
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;

// =============================================================================
// test_BlackbodySpectrum
// Matches Python: test_BlackbodySpectrum
// =============================================================================

#[test]
fn test_blackbody_spectrum() {
    let warm_color = blackbody_temperature_as_rgb(1000.0);
    let whitepoint = blackbody_temperature_as_rgb(6500.0);
    let cool_color = blackbody_temperature_as_rgb(10000.0);

    // Whitepoint is ~= (1,1,1)
    assert!(
        is_close_vec3f(&whitepoint, &Vec3f::new(1.0, 1.0, 1.0), 0.1),
        "whitepoint should be close to (1,1,1), got {:?}",
        whitepoint
    );
    // Warm has more red than green or blue
    assert!(warm_color.x > warm_color.y);
    assert!(warm_color.x > warm_color.z);
    // Cool has more blue than red or green
    assert!(cool_color.z > cool_color.x);
    assert!(cool_color.z > cool_color.y);
}

fn is_close_vec3f(a: &Vec3f, b: &Vec3f, tolerance: f32) -> bool {
    (a.x - b.x).abs() < tolerance && (a.y - b.y).abs() < tolerance && (a.z - b.z).abs() < tolerance
}

// =============================================================================
// test_DomeLight_OrientToStageUpAxis
// Matches Python: test_DomeLight_OrientToStageUpAxis
// =============================================================================

#[test]
fn test_dome_light_orient_to_stage_up_axis() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");

    // Try Y-up first
    usd_geom::set_stage_up_axis(&stage, &usd_geom::tokens::usd_geom_tokens().y);

    // Create a dome
    let light =
        DomeLight::define(&stage, &Path::from("/dome")).expect("failed to define dome light");

    // No xform ops to begin with
    let xformable = usd_geom::Xformable::new(light.get_prim().clone());
    assert_eq!(xformable.get_ordered_xform_ops().len(), 0);

    // Align to up axis
    light.orient_to_stage_up_axis();

    // Since the stage is already Y-up, no additional xform op was required
    assert_eq!(xformable.get_ordered_xform_ops().len(), 0);

    // Now change the stage to Z-up and re-align the dome
    usd_geom::set_stage_up_axis(&stage, &usd_geom::tokens::usd_geom_tokens().z);
    light.orient_to_stage_up_axis();

    // That should require a +90 deg rotate on X
    let ops = xformable.get_ordered_xform_ops();
    assert_eq!(ops.len(), 1);
    let op = &ops[0];
    assert!(
        op.name().as_str().contains("orientToStageUpAxis"),
        "op name should contain orientToStageUpAxis, got: {}",
        op.name().as_str()
    );
    // Verify the value is 90.0
    if let Some(val) = op.get_typed::<f32>(TimeCode::default()) {
        assert!((val - 90.0).abs() < 0.001, "expected 90.0, got {}", val);
    }
}

// =============================================================================
// test_GetShaderId
// Matches Python: test_GetShaderId
// =============================================================================

#[test]
fn test_get_shader_id() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");

    // Create an untyped prim with a LightAPI applied
    let prim = stage
        .define_prim("/PrimLight", "")
        .expect("failed to define prim");
    let light = LightAPI::apply(&prim).expect("failed to apply LightAPI");

    // By default LightAPI shader IDs are empty for all render contexts
    assert_eq!(light.get_shader_id(&[]).as_str(), "");
    assert_eq!(
        light
            .get_shader_id(&[Token::new("other"), Token::new("ri")])
            .as_str(),
        ""
    );

    // The default shader ID attr name is correct
    let t = tokens();
    assert_eq!(t.light_shader_id.as_str(), "light:shaderId");

    // Create a LightFilter prim and test
    let light_filter = LightFilter::define(&stage, &Path::from("/PrimLightFilter"))
        .expect("failed to define light filter");

    // Default shader ID is empty
    assert_eq!(light_filter.get_shader_id(&[]).as_str(), "");
    assert_eq!(
        light_filter
            .get_shader_id(&[Token::new("other"), Token::new("ri")])
            .as_str(),
        ""
    );

    // The filter shader ID attr name is correct
    assert_eq!(t.light_filter_shader_id.as_str(), "lightFilter:shaderId");
}

// =============================================================================
// test_BasicConnectableLights (partial — schema attribute names + LightAPI)
// Matches Python: test_BasicConnectableLights (attribute-name part)
// =============================================================================

#[test]
fn test_basic_light_api_schema_attrs() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");

    let rect_light =
        RectLight::define(&stage, &Path::from("/RectLight")).expect("failed to define RectLight");
    assert!(rect_light.is_valid());

    let light_api = rect_light.light_api();
    assert!(light_api.is_valid());

    // Verify schema attribute names include all expected attributes
    let names = LightAPI::get_schema_attribute_names(true);
    assert!(
        names.len() >= 10,
        "expected at least 10 LightAPI attrs, got {}",
        names.len()
    );

    // Verify collection names
    assert_eq!(
        LightAPI::get_light_link_collection_name().as_str(),
        "lightLink"
    );
    assert_eq!(
        LightAPI::get_shadow_link_collection_name().as_str(),
        "shadowLink"
    );
}

// =============================================================================
// test_LightSchemaAttributeNames (verify each light type)
// =============================================================================

#[test]
fn test_light_schema_attribute_names() {
    // RectLight: width, height, texture:file
    let rect_names = RectLight::get_schema_attribute_names(false);
    assert_eq!(rect_names.len(), 3);

    // DiskLight: radius, texture:file
    let disk_names = DiskLight::get_schema_attribute_names(false);
    assert_eq!(disk_names.len(), 2);

    // SphereLight: radius, treatAsPoint
    let sphere_names = SphereLight::get_schema_attribute_names(false);
    assert_eq!(sphere_names.len(), 2);

    // CylinderLight: length, radius, treatAsLine
    let cyl_names = CylinderLight::get_schema_attribute_names(false);
    assert_eq!(cyl_names.len(), 3);

    // DistantLight: angle
    let dist_names = DistantLight::get_schema_attribute_names(false);
    assert_eq!(dist_names.len(), 1);

    // DomeLight: texture:file, texture:format, guideRadius
    let dome_names = DomeLight::get_schema_attribute_names(false);
    assert_eq!(dome_names.len(), 3);

    // ShadowAPI: 5 attributes
    let shadow_names = ShadowAPI::get_schema_attribute_names(true);
    assert_eq!(shadow_names.len(), 5);

    // ShapingAPI: 7 attributes
    let shaping_names = ShapingAPI::get_schema_attribute_names(true);
    assert_eq!(shaping_names.len(), 7);
}

// =============================================================================
// test_LightDefine (verify all light types can be defined on a stage)
// =============================================================================

#[test]
fn test_light_define_all_types() {
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");

    // Boundable lights
    let rect = RectLight::define(&stage, &Path::from("/RectLight"));
    assert!(rect.is_some());
    let disk = DiskLight::define(&stage, &Path::from("/DiskLight"));
    assert!(disk.is_some());
    let sphere = SphereLight::define(&stage, &Path::from("/SphereLight"));
    assert!(sphere.is_some());
    let cyl = CylinderLight::define(&stage, &Path::from("/CylLight"));
    assert!(cyl.is_some());
    let portal = PortalLight::define(&stage, &Path::from("/PortalLight"));
    assert!(portal.is_some());

    // Non-boundable lights
    let dome = DomeLight::define(&stage, &Path::from("/DomeLight"));
    assert!(dome.is_some());
    let distant = DistantLight::define(&stage, &Path::from("/DistLight"));
    assert!(distant.is_some());

    // Light filter
    let filter = LightFilter::define(&stage, &Path::from("/LightFilter"));
    assert!(filter.is_some());
}

// =============================================================================
// test_TokenValues (verify all critical tokens match C++ values)
// =============================================================================

#[test]
fn test_token_values() {
    let t = tokens();

    // Texture format tokens
    assert_eq!(t.angular.as_str(), "angular");
    assert_eq!(t.automatic.as_str(), "automatic");
    assert_eq!(t.cube_map_vertical_cross.as_str(), "cubeMapVerticalCross");
    assert_eq!(t.latlong.as_str(), "latlong");
    assert_eq!(t.mirrored_ball.as_str(), "mirroredBall");

    // Collection tokens
    assert_eq!(t.filter_link.as_str(), "filterLink");
    assert_eq!(t.light_link.as_str(), "lightLink");
    assert_eq!(t.shadow_link.as_str(), "shadowLink");

    // Cache behavior tokens
    assert_eq!(t.consume_and_continue.as_str(), "consumeAndContinue");
    assert_eq!(t.consume_and_halt.as_str(), "consumeAndHalt");
    assert_eq!(t.ignore.as_str(), "ignore");

    // Material sync mode tokens
    assert_eq!(t.independent.as_str(), "independent");
    assert_eq!(
        t.material_glow_tints_light.as_str(),
        "materialGlowTintsLight"
    );
    assert_eq!(t.no_material_response.as_str(), "noMaterialResponse");

    // Schema identifiers
    assert_eq!(t.rect_light.as_str(), "RectLight");
    assert_eq!(t.disk_light.as_str(), "DiskLight");
    assert_eq!(t.sphere_light.as_str(), "SphereLight");
    assert_eq!(t.cylinder_light.as_str(), "CylinderLight");
    assert_eq!(t.distant_light.as_str(), "DistantLight");
    assert_eq!(t.dome_light.as_str(), "DomeLight");
    assert_eq!(t.dome_light_1.as_str(), "DomeLight_1");
    assert_eq!(t.geometry_light.as_str(), "GeometryLight");
    assert_eq!(t.portal_light.as_str(), "PortalLight");
    assert_eq!(t.light_api.as_str(), "LightAPI");
    assert_eq!(t.light_filter.as_str(), "LightFilter");
    assert_eq!(t.light_list_api.as_str(), "LightListAPI");
    assert_eq!(t.mesh_light_api.as_str(), "MeshLightAPI");
    assert_eq!(t.volume_light_api.as_str(), "VolumeLightAPI");
    assert_eq!(t.shadow_api.as_str(), "ShadowAPI");
    assert_eq!(t.shaping_api.as_str(), "ShapingAPI");
    assert_eq!(t.plugin_light.as_str(), "PluginLight");
    assert_eq!(t.plugin_light_filter.as_str(), "PluginLightFilter");

    // Light input tokens
    assert_eq!(t.inputs_intensity.as_str(), "inputs:intensity");
    assert_eq!(t.inputs_exposure.as_str(), "inputs:exposure");
    assert_eq!(t.inputs_color.as_str(), "inputs:color");
    assert_eq!(t.inputs_diffuse.as_str(), "inputs:diffuse");
    assert_eq!(t.inputs_specular.as_str(), "inputs:specular");
    assert_eq!(t.inputs_normalize.as_str(), "inputs:normalize");
    assert_eq!(
        t.inputs_color_temperature.as_str(),
        "inputs:colorTemperature"
    );
    assert_eq!(
        t.inputs_enable_color_temperature.as_str(),
        "inputs:enableColorTemperature"
    );
    assert_eq!(t.inputs_width.as_str(), "inputs:width");
    assert_eq!(t.inputs_height.as_str(), "inputs:height");
    assert_eq!(t.inputs_radius.as_str(), "inputs:radius");
    assert_eq!(t.inputs_length.as_str(), "inputs:length");
    assert_eq!(t.inputs_angle.as_str(), "inputs:angle");
    assert_eq!(t.inputs_texture_file.as_str(), "inputs:texture:file");
    assert_eq!(t.inputs_texture_format.as_str(), "inputs:texture:format");

    // Shadow input tokens
    assert_eq!(t.inputs_shadow_enable.as_str(), "inputs:shadow:enable");
    assert_eq!(t.inputs_shadow_color.as_str(), "inputs:shadow:color");
    assert_eq!(t.inputs_shadow_distance.as_str(), "inputs:shadow:distance");
    assert_eq!(t.inputs_shadow_falloff.as_str(), "inputs:shadow:falloff");
    assert_eq!(
        t.inputs_shadow_falloff_gamma.as_str(),
        "inputs:shadow:falloffGamma"
    );

    // Shaping input tokens
    assert_eq!(t.inputs_shaping_focus.as_str(), "inputs:shaping:focus");
    assert_eq!(
        t.inputs_shaping_focus_tint.as_str(),
        "inputs:shaping:focusTint"
    );
    assert_eq!(
        t.inputs_shaping_cone_angle.as_str(),
        "inputs:shaping:cone:angle"
    );
    assert_eq!(
        t.inputs_shaping_cone_softness.as_str(),
        "inputs:shaping:cone:softness"
    );
    assert_eq!(
        t.inputs_shaping_ies_file.as_str(),
        "inputs:shaping:ies:file"
    );
    assert_eq!(
        t.inputs_shaping_ies_angle_scale.as_str(),
        "inputs:shaping:ies:angleScale"
    );
    assert_eq!(
        t.inputs_shaping_ies_normalize.as_str(),
        "inputs:shaping:ies:normalize"
    );

    // Pole axis tokens
    assert_eq!(t.scene.as_str(), "scene");
    assert_eq!(t.y_axis.as_str(), "Y");
    assert_eq!(t.z_axis.as_str(), "Z");
    assert_eq!(t.pole_axis.as_str(), "poleAxis");

    // Light property tokens
    assert_eq!(t.light_shader_id.as_str(), "light:shaderId");
    assert_eq!(
        t.light_material_sync_mode.as_str(),
        "light:materialSyncMode"
    );
    assert_eq!(t.light_filter_shader_id.as_str(), "lightFilter:shaderId");
    assert_eq!(t.light_filters.as_str(), "light:filters");
    assert_eq!(t.orient_to_stage_up_axis.as_str(), "orientToStageUpAxis");
    assert_eq!(t.portals.as_str(), "portals");
    assert_eq!(t.treat_as_line.as_str(), "treatAsLine");
    assert_eq!(t.treat_as_point.as_str(), "treatAsPoint");
    assert_eq!(t.geometry.as_str(), "geometry");
    assert_eq!(t.guide_radius.as_str(), "guideRadius");
    assert_eq!(t.light_list.as_str(), "lightList");
    assert_eq!(
        t.light_list_cache_behavior.as_str(),
        "lightList:cacheBehavior"
    );
}

// =============================================================================
// test_LightAPI_DefaultValues (verify schema defaults match C++)
// =============================================================================

#[test]
fn test_light_api_default_values() {
    let light_api = LightAPI::invalid();

    // Default values when no prim exists
    assert_eq!(light_api.get_intensity(TimeCode::default()), 1.0);
    assert_eq!(light_api.get_exposure(TimeCode::default()), 0.0);
    assert_eq!(light_api.get_diffuse(TimeCode::default()), 1.0);
    assert_eq!(light_api.get_specular(TimeCode::default()), 1.0);
    assert!(!light_api.get_normalize_power(TimeCode::default()));
    assert_eq!(light_api.get_color_temperature(TimeCode::default()), 6500.0);
    assert!(!light_api.get_enable_color_temperature(TimeCode::default()));

    let color = light_api.get_color(TimeCode::default());
    assert_eq!(color, Vec3f::new(1.0, 1.0, 1.0));
}

// =============================================================================
// test_LightFilter_SchemaInfo
// =============================================================================

#[test]
fn test_light_filter_schema_info() {
    assert_eq!(LightFilter::SCHEMA_TYPE_NAME, "LightFilter");
    assert_eq!(
        LightFilter::get_filter_link_collection_name().as_str(),
        "filterLink"
    );

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("failed to create stage");
    if let Some(filter) = LightFilter::define(&stage, &Path::from("/TestFilter")) {
        assert_eq!(filter.get_path().as_str(), "/TestFilter");
    }
}

// =============================================================================
// test_RenderContextShaderIdAttrName
// =============================================================================

#[test]
fn test_render_context_shader_id_attr_name() {
    let t = tokens();
    let render_context = "ri";
    let expected = format!("{}:{}", render_context, t.light_shader_id.as_str());
    assert_eq!(expected, "ri:light:shaderId");

    let expected_filter = format!("{}:{}", render_context, t.light_filter_shader_id.as_str());
    assert_eq!(expected_filter, "ri:lightFilter:shaderId");
}

// =============================================================================
// test_SdrShaderNodesForLights
// Matches Python: test_SdrShaderNodesForLights
// =============================================================================

#[test]
fn test_sdr_shader_nodes_for_lights() {
    use std::collections::HashMap;
    use usd_sdr::registry::SdrRegistry;

    // Initialize UsdLux plugins (discovery + parser)
    usd_lux::init();

    let registry = SdrRegistry::get_instance();
    let usd_source = vec![Token::new("USD")];

    // Expected light nodes and their type-specific inputs
    let expected_light_nodes: HashMap<&str, Vec<&str>> = HashMap::from([
        ("CylinderLight", vec!["length", "radius"]),
        ("DiskLight", vec!["radius"]),
        ("DistantLight", vec!["angle"]),
        ("DomeLight", vec!["texture:file", "texture:format"]),
        ("GeometryLight", vec![]),
        ("PortalLight", vec!["width", "height"]),
        ("RectLight", vec!["width", "height", "texture:file"]),
        ("SphereLight", vec!["radius"]),
        ("MeshLight", vec![]),
        ("VolumeLight", vec![]),
    ]);

    // Common inputs from LightAPI + ShadowAPI + ShapingAPI
    let expected_common_inputs = vec![
        "color",
        "colorTemperature",
        "diffuse",
        "enableColorTemperature",
        "exposure",
        "intensity",
        "normalize",
        "specular",
        "shadow:color",
        "shadow:distance",
        "shadow:enable",
        "shadow:falloff",
        "shadow:falloffGamma",
        "shaping:cone:angle",
        "shaping:cone:softness",
        "shaping:focus",
        "shaping:focusTint",
        "shaping:ies:angleScale",
        "shaping:ies:file",
        "shaping:ies:normalize",
    ];

    for (light_name, extra_inputs) in &expected_light_nodes {
        let identifier = Token::new(light_name);
        let node = registry.get_shader_node_by_identifier(&identifier, &usd_source);

        // Node should exist in registry
        assert!(
            node.is_some(),
            "SdrShaderNode not found for '{}'",
            light_name
        );
        let node = node.expect("node");

        // Verify basic node properties
        assert_eq!(
            node.get_context().as_str(),
            "light",
            "{}: context",
            light_name
        );
        assert_eq!(
            node.get_source_type().as_str(),
            "USD",
            "{}: source_type",
            light_name
        );

        // Verify inputs for valid nodes (nodes with properties from generatedSchema)
        if node.is_valid() {
            let node_input_names = node.get_shader_input_names();
            for common_input in &expected_common_inputs {
                assert!(
                    node_input_names.iter().any(|n| n.as_str() == *common_input),
                    "{}: missing common input '{}'. Node has: {:?}",
                    light_name,
                    common_input,
                    node_input_names
                        .iter()
                        .map(|n| n.as_str())
                        .collect::<Vec<_>>()
                );
            }
            for extra_input in extra_inputs {
                assert!(
                    node_input_names.iter().any(|n| n.as_str() == *extra_input),
                    "{}: missing type-specific input '{}'. Node has: {:?}",
                    light_name,
                    extra_input,
                    node_input_names
                        .iter()
                        .map(|n| n.as_str())
                        .collect::<Vec<_>>()
                );
            }

            // No outputs expected
            assert!(
                node.get_shader_output_names().is_empty(),
                "{}: expected no outputs",
                light_name
            );
        }
    }
}
