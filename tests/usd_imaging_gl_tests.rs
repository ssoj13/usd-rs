//! Integration tests for UsdImagingGL module.

#[cfg(test)]
mod usd_imaging_gl_tests {
    // Note: These are placeholder tests that verify the module structure compiles.
    // Full rendering tests would require OpenGL context setup and actual USD scenes.

    #[test]
    fn test_module_exists() {
        // This test simply verifies the module structure is correct
        // and all types are accessible
        let _ = "usd_imaging_gl module compiles successfully";
    }

    #[test]
    fn test_version_constant() {
        use usd::usd_imaging::gl::API_VERSION;
        assert_eq!(API_VERSION, 11, "API version should be 11");
    }

    #[test]
    fn test_draw_mode_variants() {
        use usd::usd_imaging::gl::DrawMode;

        // Verify all DrawMode variants compile
        let modes = vec![
            DrawMode::Points,
            DrawMode::Wireframe,
            DrawMode::WireframeOnSurface,
            DrawMode::ShadedFlat,
            DrawMode::ShadedSmooth,
            DrawMode::GeomOnly,
            DrawMode::GeomFlat,
            DrawMode::GeomSmooth,
        ];

        assert_eq!(modes.len(), 8);
        assert_eq!(DrawMode::default(), DrawMode::ShadedSmooth);
    }

    #[test]
    fn test_cull_style_variants() {
        use usd::usd_imaging::gl::CullStyle;

        let styles = vec![
            CullStyle::NoOpinion,
            CullStyle::Nothing,
            CullStyle::Back,
            CullStyle::Front,
            CullStyle::BackUnlessDoubleSided,
        ];

        assert_eq!(styles.len(), 5);
        assert_eq!(CullStyle::default(), CullStyle::Nothing);
    }

    #[test]
    fn test_render_params_default() {
        use usd::usd_imaging::gl::RenderParams;

        let params = RenderParams::default();

        assert_eq!(params.complexity, 1.0);
        assert!(params.enable_lighting);
        assert!(params.enable_scene_materials);
        assert!(params.enable_scene_lights);
        assert!(!params.show_guides);
        assert!(params.show_proxy);
    }

    #[test]
    fn test_render_params_builder() {
        use usd::usd_imaging::gl::{DrawMode, RenderParams};

        let params = RenderParams::new()
            .with_complexity(2.0)
            .with_draw_mode(DrawMode::Wireframe)
            .with_lighting(false)
            .with_scene_materials(false);

        assert_eq!(params.complexity, 2.0);
        assert_eq!(params.draw_mode, DrawMode::Wireframe);
        assert!(!params.enable_lighting);
        assert!(!params.enable_scene_materials);
    }

    #[test]
    fn test_engine_parameters_default() {
        use usd::usd_imaging::gl::EngineParameters;

        let params = EngineParameters::default();

        assert!(params.gpu_enabled);
        assert!(params.enable_usd_draw_modes);
        assert!(!params.display_unloaded_prims_with_bounds);
        assert!(!params.allow_asynchronous_scene_processing);
    }

    #[test]
    fn test_engine_parameters_builder() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::EngineParameters;

        let params = EngineParameters::new()
            .with_gpu_enabled(false)
            .with_renderer_plugin_id(Token::new("GL"));

        assert!(!params.gpu_enabled);
        assert_eq!(params.renderer_plugin_id, Token::new("GL"));
    }

    #[test]
    fn test_engine_creation() {
        use usd::usd_imaging::gl::{Engine, EngineParameters};

        let params = EngineParameters::default();
        let engine = Engine::new(params);

        assert!(engine.is_root_visible());
        assert_eq!(engine.render_buffer_size().x, 1920);
        assert_eq!(engine.render_buffer_size().y, 1080);
    }

    #[test]
    fn test_engine_with_defaults() {
        use usd::usd_imaging::gl::Engine;

        let engine = Engine::with_defaults();

        assert!(engine.is_root_visible());
        assert_eq!(engine.selected_paths().len(), 0);
    }

    #[test]
    fn test_engine_root_transform() {
        use usd::gf::Matrix4d;
        use usd::usd_imaging::gl::Engine;

        let mut engine = Engine::with_defaults();
        let transform = Matrix4d::identity();

        engine.set_root_transform(transform);
        assert_eq!(engine.root_transform(), &Matrix4d::identity());
    }

    #[test]
    fn test_engine_root_visibility() {
        use usd::usd_imaging::gl::Engine;

        let mut engine = Engine::with_defaults();

        assert!(engine.is_root_visible());

        engine.set_root_visibility(false);
        assert!(!engine.is_root_visible());

        engine.set_root_visibility(true);
        assert!(engine.is_root_visible());
    }

    #[test]
    fn test_engine_camera_path() {
        use usd::sdf::Path;
        use usd::usd_imaging::gl::Engine;

        let mut engine = Engine::with_defaults();

        assert!(engine.camera_path().is_none());

        let camera_path = Path::from_string("/World/Camera").expect("valid path");
        engine.set_camera_path(camera_path.clone());

        assert_eq!(engine.camera_path(), Some(&camera_path));
    }

    #[test]
    fn test_engine_render_buffer_size() {
        use usd::gf::Vec2i;
        use usd::usd_imaging::gl::Engine;

        let mut engine = Engine::with_defaults();

        let new_size = Vec2i::new(1280, 720);
        engine.set_render_buffer_size(new_size);

        assert_eq!(engine.render_buffer_size().x, 1280);
        assert_eq!(engine.render_buffer_size().y, 720);
    }

    #[test]
    fn test_engine_selection() {
        use usd::sdf::Path;
        use usd::usd_imaging::gl::Engine;

        let mut engine = Engine::with_defaults();

        assert_eq!(engine.selected_paths().len(), 0);

        let path1 = Path::from_string("/World/Cube").expect("valid path");
        let path2 = Path::from_string("/World/Sphere").expect("valid path");

        engine.set_selected(vec![path1.clone()]);
        assert_eq!(engine.selected_paths().len(), 1);

        engine.add_selected(path2.clone(), -1);
        assert_eq!(engine.selected_paths().len(), 2);

        engine.clear_selected();
        assert_eq!(engine.selected_paths().len(), 0);
    }

    #[test]
    fn test_engine_selection_color() {
        use usd::gf::Vec4f;
        use usd::usd_imaging::gl::Engine;

        let mut engine = Engine::with_defaults();

        let yellow = Vec4f::new(1.0, 1.0, 0.0, 1.0);
        assert_eq!(engine.selection_color(), &yellow);

        let red = Vec4f::new(1.0, 0.0, 0.0, 1.0);
        engine.set_selection_color(red);
        assert_eq!(engine.selection_color(), &red);
    }

    #[test]
    fn test_pick_params_default() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::PickParams;

        let params = PickParams::default();
        assert_eq!(params.resolve_mode, Token::new("resolveNearestToCenter"));
    }

    #[test]
    fn test_pick_params_builder() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::PickParams;

        let params = PickParams::new().with_resolve_mode(Token::new("resolveDeep"));

        assert_eq!(params.resolve_mode, Token::new("resolveDeep"));
    }

    #[test]
    fn test_renderer_setting_type_variants() {
        use usd::usd_imaging::gl::RendererSettingType;

        let types = vec![
            RendererSettingType::Flag,
            RendererSettingType::Int,
            RendererSettingType::Float,
            RendererSettingType::String,
        ];

        assert_eq!(types.len(), 4);
    }

    #[test]
    fn test_renderer_setting_flag() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::{RendererSetting, RendererSettingType};

        let setting = RendererSetting::flag("Enable Shadows", Token::new("enableShadows"), true);

        assert_eq!(setting.name, "Enable Shadows");
        assert_eq!(setting.key, Token::new("enableShadows"));
        assert_eq!(setting.setting_type, RendererSettingType::Flag);
    }

    #[test]
    fn test_renderer_setting_int() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::{RendererSetting, RendererSettingType};

        let setting = RendererSetting::int("Max Samples", Token::new("maxSamples"), 128);

        assert_eq!(setting.name, "Max Samples");
        assert_eq!(setting.key, Token::new("maxSamples"));
        assert_eq!(setting.setting_type, RendererSettingType::Int);
    }

    #[test]
    fn test_renderer_setting_float() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::{RendererSetting, RendererSettingType};

        let setting = RendererSetting::float("Quality", Token::new("quality"), 1.0);

        assert_eq!(setting.name, "Quality");
        assert_eq!(setting.setting_type, RendererSettingType::Float);
    }

    #[test]
    fn test_renderer_setting_string() {
        use usd::tf::Token;
        use usd::usd_imaging::gl::{RendererSetting, RendererSettingType};

        let setting = RendererSetting::string("Output Format", Token::new("outputFormat"), "png");

        assert_eq!(setting.name, "Output Format");
        assert_eq!(setting.setting_type, RendererSettingType::String);
    }

    #[test]
    fn test_intersection_result_struct() {
        use usd::gf::Vec3d;
        use usd::sdf::Path;
        use usd::usd_imaging::gl::IntersectionResult;

        let result = IntersectionResult {
            hit_point: Vec3d::new(1.0, 2.0, 3.0),
            hit_normal: Vec3d::new(0.0, 1.0, 0.0),
            hit_prim_path: Path::from_string("/World/Cube").expect("valid path"),
            hit_instancer_path: Path::empty(),
            hit_instance_index: 0,
        };

        assert_eq!(result.hit_point.x, 1.0);
        assert_eq!(result.hit_normal.y, 1.0);
        assert_eq!(result.hit_instance_index, 0);
    }

    #[test]
    fn test_engine_is_converged() {
        use usd::usd_imaging::gl::Engine;

        let engine = Engine::with_defaults();

        // Default implementation returns true (non-progressive)
        assert!(engine.is_converged());
    }
}
