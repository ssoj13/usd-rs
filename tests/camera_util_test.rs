//! Integration test for camera_util module

use usd::gf::{
    Camera, FOVDirection, Matrix4d, Range2d, Range2f, Rect2i, Vec2d, Vec2f, Vec2i, Vec4d,
};
use usd::imaging::camera_util::{
    ConformWindowPolicy, Framing, ScreenWindowParameters, conform_window_matrix,
    conform_window_range2d, conform_window_vec2, conform_window_vec4,
};

#[test]
fn test_conform_window_vec2_match_vertically() {
    let window = Vec2d::new(100.0, 100.0);
    let result = conform_window_vec2(window, ConformWindowPolicy::MatchVertically, 2.0);
    assert!((result[0] - 200.0).abs() < 1e-10);
    assert!((result[1] - 100.0).abs() < 1e-10);
}

#[test]
fn test_conform_window_vec2_match_horizontally() {
    let window = Vec2d::new(100.0, 100.0);
    let result = conform_window_vec2(window, ConformWindowPolicy::MatchHorizontally, 2.0);
    assert!((result[0] - 100.0).abs() < 1e-10);
    assert!((result[1] - 50.0).abs() < 1e-10);
}

#[test]
fn test_conform_window_vec2_fit() {
    let window = Vec2d::new(100.0, 100.0);
    let result = conform_window_vec2(window, ConformWindowPolicy::Fit, 2.0);
    assert!((result[0] - 200.0).abs() < 1e-10);
    assert!((result[1] - 100.0).abs() < 1e-10);
}

#[test]
fn test_conform_window_vec2_crop() {
    let window = Vec2d::new(100.0, 100.0);
    let result = conform_window_vec2(window, ConformWindowPolicy::Crop, 2.0);
    assert!((result[0] - 100.0).abs() < 1e-10);
    assert!((result[1] - 50.0).abs() < 1e-10);
}

#[test]
fn test_conform_window_range2d() {
    let window = Range2d::new(Vec2d::new(-50.0, -50.0), Vec2d::new(50.0, 50.0));
    let result = conform_window_range2d(window, ConformWindowPolicy::MatchVertically, 2.0);

    assert!((result.min()[0] - (-100.0)).abs() < 1e-10);
    assert!((result.max()[0] - 100.0).abs() < 1e-10);
    assert!((result.min()[1] - (-50.0)).abs() < 1e-10);
    assert!((result.max()[1] - 50.0).abs() < 1e-10);
}

#[test]
fn test_conform_window_vec4() {
    let window = Vec4d::new(-50.0, 50.0, -50.0, 50.0);
    let result = conform_window_vec4(window, ConformWindowPolicy::MatchVertically, 2.0);

    assert!((result[0] - (-100.0)).abs() < 1e-10);
    assert!((result[1] - 100.0).abs() < 1e-10);
    assert!((result[2] - (-50.0)).abs() < 1e-10);
    assert!((result[3] - 50.0).abs() < 1e-10);
}

#[test]
fn test_conform_window_matrix() {
    let mut proj = Matrix4d::identity();
    proj[0][0] = 1.0;
    proj[1][1] = 1.0;

    let result = conform_window_matrix(proj, ConformWindowPolicy::MatchHorizontally, 2.0);

    assert!((result[1][1] - 2.0).abs() < 1e-10);
    assert!((result[0][0] - 1.0).abs() < 1e-10);
}

#[test]
fn test_framing_new_empty() {
    let framing = Framing::new_empty();
    assert!(!framing.is_valid());
}

#[test]
fn test_framing_from_data_window() {
    let data_window = Rect2i::new(Vec2i::new(0, 0), Vec2i::new(1919, 1079));
    let framing = Framing::from_data_window(data_window);

    assert!(framing.is_valid());
    assert_eq!(framing.data_window, data_window);
}

#[test]
fn test_framing_is_valid() {
    let valid = Framing::new(
        Range2f::new(Vec2f::new(0.0, 0.0), Vec2f::new(100.0, 100.0)),
        Rect2i::new(Vec2i::new(0, 0), Vec2i::new(99, 99)),
        1.0,
    );
    assert!(valid.is_valid());

    let invalid = Framing::new_empty();
    assert!(!invalid.is_valid());
}

#[test]
fn test_screen_window_parameters() {
    let camera = Camera::default();
    let params = ScreenWindowParameters::new(&camera, FOVDirection::Horizontal);

    let sw = params.screen_window();
    assert!(sw[0] < sw[1]); // left < right
    assert!(sw[2] < sw[3]); // bottom < top

    assert!(params.field_of_view() > 0.0);
}

#[test]
fn test_conform_window_policy_equality() {
    assert_eq!(ConformWindowPolicy::Fit, ConformWindowPolicy::Fit);
    assert_ne!(ConformWindowPolicy::Fit, ConformWindowPolicy::Crop);
}
