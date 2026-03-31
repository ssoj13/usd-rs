
//! HDX tokens - Standard identifiers for Hydra extensions

use std::sync::LazyLock;
use usd_tf::Token;

// Buffer tokens
/// Order-independent transparency counter buffer token
pub static HDX_OIT_COUNTER_BUFFER: LazyLock<Token> =
    LazyLock::new(|| Token::new("hdxOitCounterBuffer"));
/// Order-independent transparency data buffer token
pub static HDX_OIT_DATA_BUFFER: LazyLock<Token> = LazyLock::new(|| Token::new("hdxOitDataBuffer"));
/// Order-independent transparency depth buffer token
pub static HDX_OIT_DEPTH_BUFFER: LazyLock<Token> =
    LazyLock::new(|| Token::new("hdxOitDepthBuffer"));
/// Order-independent transparency index buffer token
pub static HDX_OIT_INDEX_BUFFER: LazyLock<Token> =
    LazyLock::new(|| Token::new("hdxOitIndexBuffer"));
/// Selection highlight buffer token
pub static HDX_SELECTION_BUFFER: LazyLock<Token> =
    LazyLock::new(|| Token::new("hdxSelectionBuffer"));

// Context tokens
/// Imager version identifier
pub static IMAGER_VERSION: LazyLock<Token> = LazyLock::new(|| Token::new("imagerVersion"));
/// Lighting computation context
pub static LIGHTING_CONTEXT: LazyLock<Token> = LazyLock::new(|| Token::new("lightingContext"));
/// Lighting shader resource identifier
pub static LIGHTING_SHADER: LazyLock<Token> = LazyLock::new(|| Token::new("lightingShader"));
/// Opacity value for occluded selections
pub static OCCLUDED_SELECTION_OPACITY: LazyLock<Token> =
    LazyLock::new(|| Token::new("occludedSelectionOpacity"));

// OIT tokens
/// OIT fragment counter resource
pub static OIT_COUNTER: LazyLock<Token> = LazyLock::new(|| Token::new("oitCounter"));
/// OIT fragment data storage
pub static OIT_DATA: LazyLock<Token> = LazyLock::new(|| Token::new("oitData"));
/// OIT depth values storage
pub static OIT_DEPTH: LazyLock<Token> = LazyLock::new(|| Token::new("oitDepth"));
/// OIT fragment indices
pub static OIT_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("oitIndices"));
/// OIT shader uniforms
pub static OIT_UNIFORMS: LazyLock<Token> = LazyLock::new(|| Token::new("oitUniforms"));
/// OIT counter buffer barrier sync token
pub static OIT_COUNTER_BUFFER_BAR: LazyLock<Token> =
    LazyLock::new(|| Token::new("oitCounterBufferBar"));
/// OIT data buffer barrier sync token
pub static OIT_DATA_BUFFER_BAR: LazyLock<Token> = LazyLock::new(|| Token::new("oitDataBufferBar"));
/// OIT depth buffer barrier sync token
pub static OIT_DEPTH_BUFFER_BAR: LazyLock<Token> =
    LazyLock::new(|| Token::new("oitDepthBufferBar"));
/// OIT index buffer barrier sync token
pub static OIT_INDEX_BUFFER_BAR: LazyLock<Token> =
    LazyLock::new(|| Token::new("oitIndexBufferBar"));
/// OIT uniform buffer barrier sync token
pub static OIT_UNIFORM_BAR: LazyLock<Token> = LazyLock::new(|| Token::new("oitUniformBar"));
/// OIT render pass state configuration
pub static OIT_RENDER_PASS_STATE: LazyLock<Token> =
    LazyLock::new(|| Token::new("oitRenderPassState"));
/// OIT screen dimensions
pub static OIT_SCREEN_SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("oitScreenSize"));
/// Flag indicating OIT rendering requested
pub static OIT_REQUEST_FLAG: LazyLock<Token> = LazyLock::new(|| Token::new("oitRequestFlag"));
/// Flag indicating OIT buffers cleared
pub static OIT_CLEARED_FLAG: LazyLock<Token> = LazyLock::new(|| Token::new("oitClearedFlag"));

// Render state tokens
/// Render pass state configuration
pub static RENDER_PASS_STATE: LazyLock<Token> = LazyLock::new(|| Token::new("renderPassState"));
/// Render index version tracking
pub static RENDER_INDEX_VERSION: LazyLock<Token> =
    LazyLock::new(|| Token::new("renderIndexVersion"));

// Selection tokens
/// Selection data resource
pub static SELECTION: LazyLock<Token> = LazyLock::new(|| Token::new("selection"));
/// Current selection state
pub static SELECTION_STATE: LazyLock<Token> = LazyLock::new(|| Token::new("selectionState"));
/// Selection buffer offset data
pub static SELECTION_OFFSETS: LazyLock<Token> = LazyLock::new(|| Token::new("selectionOffsets"));
/// Selection shader uniforms
pub static SELECTION_UNIFORMS: LazyLock<Token> = LazyLock::new(|| Token::new("selectionUniforms"));
/// Selected object highlight color
pub static SEL_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("selColor"));
/// Selection location marker color
pub static SEL_LOCATE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("selLocateColor"));
/// Per-point selection colors
pub static SELECTION_POINT_COLORS: LazyLock<Token> =
    LazyLock::new(|| Token::new("selectionPointColors"));

// Draw target tokens
/// Render passes for draw target rendering
pub static DRAW_TARGET_RENDER_PASSES: LazyLock<Token> =
    LazyLock::new(|| Token::new("drawTargetRenderPasses"));

// Light type tokens
/// Positional point light type
pub static LIGHT_TYPE_POSITIONAL: LazyLock<Token> =
    LazyLock::new(|| Token::new("lightTypePositional"));
/// Directional light type
pub static LIGHT_TYPE_DIRECTIONAL: LazyLock<Token> =
    LazyLock::new(|| Token::new("lightTypeDirectional"));
/// Spotlight light type
pub static LIGHT_TYPE_SPOT: LazyLock<Token> = LazyLock::new(|| Token::new("lightTypeSpot"));

// Task primitive tokens
/// AOV input processing task type
pub static AOV_INPUT_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("aovInputTask"));
/// Bounding box visualization task type
pub static BOUNDING_BOX_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("boundingBoxTask"));
/// Color correction processing task type
pub static COLOR_CORRECTION_TASK: LazyLock<Token> =
    LazyLock::new(|| Token::new("colorCorrectionTask"));
/// Selection colorization task type
pub static COLORIZE_SELECTION_TASK: LazyLock<Token> =
    LazyLock::new(|| Token::new("colorizeSelectionTask"));
/// Draw target rendering task type
pub static DRAW_TARGET_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("drawTargetTask"));
/// Draw target resolve task type
pub static DRAW_TARGET_RESOLVE_TASK: LazyLock<Token> =
    LazyLock::new(|| Token::new("drawTargetResolveTask"));
/// OIT rendering task type
pub static OIT_RENDER_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("oitRenderTask"));
/// OIT resolve/composite task type
pub static OIT_RESOLVE_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("oitResolveTask"));
/// OIT volume rendering task type
pub static OIT_VOLUME_RENDER_TASK: LazyLock<Token> =
    LazyLock::new(|| Token::new("oitVolumeRenderTask"));
/// Object picking task type
pub static PICK_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("pickTask"));
/// Pick from render buffer task type
pub static PICK_FROM_RENDER_BUFFER_TASK: LazyLock<Token> =
    LazyLock::new(|| Token::new("pickFromRenderBufferTask"));
/// Final presentation task type
pub static PRESENT_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("presentTask"));
/// Standard rendering task type
pub static RENDER_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("renderTask"));
/// Render setup task type
pub static RENDER_SETUP_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("renderSetupTask"));
/// Simple lighting task type
pub static SIMPLE_LIGHT_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("simpleLightTask"));
/// Shadow map generation task type
pub static SHADOW_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("shadowTask"));

// Render tag tokens
/// Rendering guide visualization tag
pub static RENDERING_GUIDE: LazyLock<Token> = LazyLock::new(|| Token::new("renderingGuide"));
/// Text label geometry tag
pub static LABEL: LazyLock<Token> = LazyLock::new(|| Token::new("label"));
/// Camera visualization guide tag
pub static CAMERA_GUIDE: LazyLock<Token> = LazyLock::new(|| Token::new("cameraGuide"));
/// In-camera guide overlay tag
pub static IN_CAMERA_GUIDE: LazyLock<Token> = LazyLock::new(|| Token::new("inCameraGuide"));
/// Streamline visualization tag
pub static STREAMLINE: LazyLock<Token> = LazyLock::new(|| Token::new("streamline"));
/// Interactive-only geometry tag
pub static INTERACTIVE_ONLY_GEOM: LazyLock<Token> =
    LazyLock::new(|| Token::new("interactiveOnlyGeom"));
/// Path visualization tag
pub static PATH: LazyLock<Token> = LazyLock::new(|| Token::new("path"));

// Color correction tokens
/// Disabled color correction mode
pub static COLOR_CORRECTION_DISABLED: LazyLock<Token> = LazyLock::new(|| Token::new("disabled"));
/// sRGB color correction mode
pub static COLOR_CORRECTION_SRGB: LazyLock<Token> = LazyLock::new(|| Token::new("sRGB"));
/// OpenColorIO color correction mode
pub static COLOR_CORRECTION_OPENCOLORIO: LazyLock<Token> =
    LazyLock::new(|| Token::new("openColorIO"));

// Color channel tokens
/// Full color display channel
pub static COLOR_CHANNEL_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("color"));
/// Red channel only display
pub static COLOR_CHANNEL_RED: LazyLock<Token> = LazyLock::new(|| Token::new("red"));
/// Green channel only display
pub static COLOR_CHANNEL_GREEN: LazyLock<Token> = LazyLock::new(|| Token::new("green"));
/// Blue channel only display
pub static COLOR_CHANNEL_BLUE: LazyLock<Token> = LazyLock::new(|| Token::new("blue"));
/// Alpha channel display
pub static COLOR_CHANNEL_ALPHA: LazyLock<Token> = LazyLock::new(|| Token::new("alpha"));
/// Luminance grayscale display
pub static COLOR_CHANNEL_LUMINANCE: LazyLock<Token> = LazyLock::new(|| Token::new("luminance"));

// AOV tokens
/// Intermediate color AOV buffer
pub static COLOR_INTERMEDIATE: LazyLock<Token> = LazyLock::new(|| Token::new("colorIntermediate"));
/// Intermediate depth AOV buffer
pub static DEPTH_INTERMEDIATE: LazyLock<Token> = LazyLock::new(|| Token::new("depthIntermediate"));

// Simple light task tokens
/// Lighting computation resource
pub static LIGHTING: LazyLock<Token> = LazyLock::new(|| Token::new("lighting"));
/// Enable lighting computation flag
pub static USE_LIGHTING: LazyLock<Token> = LazyLock::new(|| Token::new("useLighting"));
/// Use material color as diffuse flag
pub static USE_COLOR_MATERIAL_DIFFUSE: LazyLock<Token> =
    LazyLock::new(|| Token::new("useColorMaterialDiffuse"));
/// Light source data
pub static LIGHT_SOURCE: LazyLock<Token> = LazyLock::new(|| Token::new("lightSource"));
/// Light position in world space
pub static POSITION: LazyLock<Token> = LazyLock::new(|| Token::new("position"));
/// Ambient light component
pub static AMBIENT: LazyLock<Token> = LazyLock::new(|| Token::new("ambient"));
/// Diffuse light component
pub static DIFFUSE: LazyLock<Token> = LazyLock::new(|| Token::new("diffuse"));
/// Specular light component
pub static SPECULAR: LazyLock<Token> = LazyLock::new(|| Token::new("specular"));
/// Spotlight direction vector
pub static SPOT_DIRECTION: LazyLock<Token> = LazyLock::new(|| Token::new("spotDirection"));
/// Spotlight cutoff angle
pub static SPOT_CUTOFF: LazyLock<Token> = LazyLock::new(|| Token::new("spotCutoff"));
/// Spotlight falloff exponent
pub static SPOT_FALLOFF: LazyLock<Token> = LazyLock::new(|| Token::new("spotFalloff"));
/// Light attenuation coefficients
pub static ATTENUATION: LazyLock<Token> = LazyLock::new(|| Token::new("attenuation"));
/// World to light space transform
pub static WORLD_TO_LIGHT_TRANSFORM: LazyLock<Token> =
    LazyLock::new(|| Token::new("worldToLightTransform"));
/// Shadow map array start index
pub static SHADOW_INDEX_START: LazyLock<Token> = LazyLock::new(|| Token::new("shadowIndexStart"));
/// Shadow map array end index
pub static SHADOW_INDEX_END: LazyLock<Token> = LazyLock::new(|| Token::new("shadowIndexEnd"));
/// Light casts shadows flag
pub static HAS_SHADOW: LazyLock<Token> = LazyLock::new(|| Token::new("hasShadow"));
/// Indirect lighting flag
pub static IS_INDIRECT_LIGHT: LazyLock<Token> = LazyLock::new(|| Token::new("isIndirectLight"));
/// Shadow map data
pub static SHADOW: LazyLock<Token> = LazyLock::new(|| Token::new("shadow"));
/// World to shadow projection matrix
pub static WORLD_TO_SHADOW_MATRIX: LazyLock<Token> =
    LazyLock::new(|| Token::new("worldToShadowMatrix"));
/// Shadow to world projection matrix
pub static SHADOW_TO_WORLD_MATRIX: LazyLock<Token> =
    LazyLock::new(|| Token::new("shadowToWorldMatrix"));
/// Shadow blur amount
pub static BLUR: LazyLock<Token> = LazyLock::new(|| Token::new("blur"));
/// Shadow bias value
pub static BIAS: LazyLock<Token> = LazyLock::new(|| Token::new("bias"));
/// Material properties
pub static MATERIAL: LazyLock<Token> = LazyLock::new(|| Token::new("material"));
/// Material emission component
pub static EMISSION: LazyLock<Token> = LazyLock::new(|| Token::new("emission"));
/// Scene background color
pub static SCENE_COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("sceneColor"));
/// Material shininess/glossiness
pub static SHININESS: LazyLock<Token> = LazyLock::new(|| Token::new("shininess"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens_initialized() {
        assert_eq!(HDX_SELECTION_BUFFER.as_str(), "hdxSelectionBuffer");
        assert_eq!(RENDER_TASK.as_str(), "renderTask");
        assert_eq!(CAMERA_GUIDE.as_str(), "cameraGuide");
        assert_eq!(COLOR_CORRECTION_SRGB.as_str(), "sRGB");
        assert_eq!(COLOR_CHANNEL_RED.as_str(), "red");
        assert_eq!(COLOR_INTERMEDIATE.as_str(), "colorIntermediate");
        assert_eq!(LIGHTING.as_str(), "lighting");
    }

    #[test]
    fn test_token_uniqueness() {
        // Ensure different tokens are distinct
        assert_ne!(*RENDER_TASK, *SHADOW_TASK);
        assert_ne!(*COLOR_CHANNEL_RED, *COLOR_CHANNEL_GREEN);
        assert_ne!(*COLOR_INTERMEDIATE, *DEPTH_INTERMEDIATE);
    }
}
