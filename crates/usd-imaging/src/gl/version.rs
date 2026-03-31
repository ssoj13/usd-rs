//! Version information for UsdImagingGL module.
//!
//! This module tracks API version compatibility with OpenUSD's usdImagingGL.

/// UsdImagingGL API version number.
///
/// Version history:
/// - 0 -> 1: added IDRenderColor decode and direct Rprim path fetching
/// - 1 -> 2: added RenderParams::enable_usd_draw_modes
/// - 2 -> 3: refactor picking API
/// - 3 -> 4: Add "instancerContext" to new picking API
/// - 4 -> 5: Use UsdImagingGLEngine::get_scene_delegate() instead of _delegate
/// - 5 -> 6: Use UsdImagingGLEngine::get_hd_engine() instead of _engine
/// - 6 -> 7: Added UsdImagingGLEngine::get_task_controller() and is_using_legacy_impl()
/// - 7 -> 8: Added out_hit_normal parameter to UsdImagingGLEngine::test_intersection()
/// - 8 -> 9: Removed the "HydraDisabled" renderer (i.e. LegacyEngine)
/// - 9 -> 10: Added new UsdImagingGLEngine::test_intersection() method with resolve mode
/// - 10 -> 11: Removed UsdImagingGLRenderParams::enable_id_render
pub const API_VERSION: u32 = 11;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_version() {
        assert_eq!(API_VERSION, 11);
    }
}
