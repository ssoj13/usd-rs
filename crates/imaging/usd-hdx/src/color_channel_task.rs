//! Color channel task - post-process color channel selection/visualization.
//!
//! Allows viewing individual color channels (R, G, B, A, luminance) from
//! the rendered image. Useful for debugging and compositing workflows.
//! Port of pxr/imaging/hdx/colorChannelTask.h/cpp

use std::sync::LazyLock;
use usd_hd::prim::HdSceneDelegate;
use usd_hd::render::{HdRenderIndexTrait, HdTask, HdTaskContext, TfTokenVector};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

/// Color channel task tokens.
pub mod color_channel_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Color channel parameter key.
    pub static CHANNEL: LazyLock<Token> = LazyLock::new(|| Token::new("channel"));

    /// RGB channel (default - pass-through).
    pub static COLOR: LazyLock<Token> = LazyLock::new(|| Token::new("color"));

    /// Red channel only.
    pub static RED: LazyLock<Token> = LazyLock::new(|| Token::new("red"));

    /// Green channel only.
    pub static GREEN: LazyLock<Token> = LazyLock::new(|| Token::new("green"));

    /// Blue channel only.
    pub static BLUE: LazyLock<Token> = LazyLock::new(|| Token::new("blue"));

    /// Alpha channel only.
    pub static ALPHA: LazyLock<Token> = LazyLock::new(|| Token::new("alpha"));

    /// Luminance (grayscale).
    pub static LUMINANCE: LazyLock<Token> = LazyLock::new(|| Token::new("luminance"));
}

/// Which channel(s) to display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChannel {
    /// Full RGB color (pass-through).
    Color,
    /// Red channel only.
    Red,
    /// Green channel only.
    Green,
    /// Blue channel only.
    Blue,
    /// Alpha channel only.
    Alpha,
    /// Luminance (rec709 grayscale).
    Luminance,
}

impl Default for ColorChannel {
    fn default() -> Self {
        Self::Color
    }
}

impl ColorChannel {
    /// Convert from token to enum.
    pub fn from_token(tok: &Token) -> Self {
        match tok.as_str() {
            "red" => Self::Red,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "alpha" => Self::Alpha,
            "luminance" => Self::Luminance,
            _ => Self::Color,
        }
    }

    /// Convert to token.
    pub fn to_token(self) -> Token {
        match self {
            Self::Color => color_channel_tokens::COLOR.clone(),
            Self::Red => color_channel_tokens::RED.clone(),
            Self::Green => color_channel_tokens::GREEN.clone(),
            Self::Blue => color_channel_tokens::BLUE.clone(),
            Self::Alpha => color_channel_tokens::ALPHA.clone(),
            Self::Luminance => color_channel_tokens::LUMINANCE.clone(),
        }
    }

    /// Convert to shader channel index for GPU uniform.
    fn as_channel_index(self) -> i32 {
        match self {
            Self::Color => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Blue => 3,
            Self::Alpha => 4,
            Self::Luminance => 5,
        }
    }
}

/// Color channel task parameters.
///
/// Port of HdxColorChannelTaskParams from pxr/imaging/hdx/colorChannelTask.h
#[derive(Debug, Clone)]
pub struct HdxColorChannelTaskParams {
    /// Channel to display. C++ default is HdxColorChannelTokens->color.
    pub channel: ColorChannel,
}

impl Default for HdxColorChannelTaskParams {
    fn default() -> Self {
        Self {
            channel: ColorChannel::Color,
        }
    }
}

/// Token for color channel task in task context.
pub static COLOR_CHANNEL_TASK: LazyLock<Token> = LazyLock::new(|| Token::new("colorChannelTask"));

/// GPU shader parameter buffer for the color channel shader.
///
/// Matches C++ `_ParameterBuffer` in colorChannelTask.h (std430 layout).
/// Must match layout of `ParameterBuffer` in `colorChannel.glslfx`.
/// Used by `HdxFullscreenShader` (`_compositor`) to upload uniforms.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorChannelParameterBuffer {
    /// Screen size in pixels [width, height] (C++ `float screenSize[2]`).
    pub screen_size: [f32; 2],

    /// Channel index for shader (C++ `int channel`):
    /// 0=color(passthrough), 1=red, 2=green, 3=blue, 4=alpha, 5=luminance.
    pub channel: i32,
}

impl Default for ColorChannelParameterBuffer {
    fn default() -> Self {
        Self {
            screen_size: [0.0, 0.0],
            channel: 0, // color (pass-through)
        }
    }
}

/// Color channel visualization task.
///
/// Post-processing task that isolates individual color channels from the
/// rendered image. Applied after main rendering and before presentation.
///
/// Matches C++ `HdxColorChannelTask` fields:
/// - `_compositor` (unique_ptr<HdxFullscreenShader>) — fullscreen shader (lazy init)
/// - `_parameterData` (_ParameterBuffer) — GPU uniform data
/// - `_channel` (TfToken) — active channel
///
/// Port of HdxColorChannelTask from pxr/imaging/hdx/colorChannelTask.h
pub struct HdxColorChannelTask {
    /// Task path.
    id: Path,
    /// Task parameters (C++ `_channel` TfToken is stored here).
    params: HdxColorChannelTaskParams,
    /// Render tags for filtering.
    render_tags: TfTokenVector,
    /// GPU shader parameter data (C++ `_parameterData`).
    parameter_data: ColorChannelParameterBuffer,
}

impl HdxColorChannelTask {
    /// Create new color channel task.
    pub fn new(id: Path) -> Self {
        Self {
            id,
            params: HdxColorChannelTaskParams::default(),
            render_tags: Vec::new(),
            parameter_data: ColorChannelParameterBuffer::default(),
        }
    }

    /// Set color channel parameters.
    pub fn set_params(&mut self, params: HdxColorChannelTaskParams) {
        self.params = params;
    }

    /// Get color channel parameters.
    pub fn get_params(&self) -> &HdxColorChannelTaskParams {
        &self.params
    }

    /// Get the GPU parameter buffer (for inspection/testing).
    pub fn get_parameter_data(&self) -> &ColorChannelParameterBuffer {
        &self.parameter_data
    }

    /// Update the GPU parameter buffer.
    ///
    /// Mirrors C++ `_UpdateParameterBuffer(float screenSizeX, float screenSizeY)`.
    /// Returns true if values changed (triggering a GPU upload in full impl).
    pub fn update_parameter_buffer(&mut self, screen_width: f32, screen_height: f32) -> bool {
        let new_data = ColorChannelParameterBuffer {
            screen_size: [screen_width, screen_height],
            channel: self.params.channel.as_channel_index(),
        };
        if new_data == self.parameter_data {
            return false;
        }
        self.parameter_data = new_data;
        true
    }
}

impl HdTask for HdxColorChannelTask {
    fn id(&self) -> &Path {
        &self.id
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _ctx: &mut HdTaskContext,
        dirty_bits: &mut u32,
    ) {
        // C++ _Sync: pull params from delegate, update _channel token.
        *dirty_bits = 0;
    }

    fn prepare(&mut self, ctx: &mut HdTaskContext, _render_index: &dyn HdRenderIndexTrait) {
        // C++ Prepare: get color AOV texture from ctx, validate it exists.
        ctx.insert(Token::new("colorChannelPrepared"), Value::from(true));
    }

    fn execute(&mut self, _ctx: &mut HdTaskContext) {
        if self.params.channel == ColorChannel::Color {
            return; // Pass-through, no work needed.
        }

        // In full implementation (C++ Execute()):
        // 1. Get color AOV handle from task context
        // 2. _UpdateParameterBuffer(screenSizeX, screenSizeY) — upload if changed
        // 3. _compositor->BindBuffer("colorChannelParams", _parameterData)
        // 4. _compositor->BindTexture("colorIn", colorAovHandle)
        // 5. _compositor->Draw() — colorChannel.glslfx fragment shader:
        //    - Red:      fragColor = vec4(color.r, color.r, color.r, 1)
        //    - Green:    fragColor = vec4(color.g, color.g, color.g, 1)
        //    - Blue:     fragColor = vec4(color.b, color.b, color.b, 1)
        //    - Alpha:    fragColor = vec4(color.a, color.a, color.a, 1)
        //    - Luminance: fragColor = vec4(dot(color.rgb, rec709_luma), ...)
        let _ = self.update_parameter_buffer(0.0, 0.0); // placeholder
    }

    fn get_render_tags(&self) -> &[Token] {
        &self.render_tags
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_channel_task() {
        let task = HdxColorChannelTask::new(Path::from_string("/colorChannel").unwrap());
        assert_eq!(task.get_params().channel, ColorChannel::Color);
    }

    #[test]
    fn test_color_channel_from_token() {
        assert_eq!(
            ColorChannel::from_token(&Token::new("red")),
            ColorChannel::Red
        );
        assert_eq!(
            ColorChannel::from_token(&Token::new("luminance")),
            ColorChannel::Luminance
        );
        assert_eq!(
            ColorChannel::from_token(&Token::new("unknown")),
            ColorChannel::Color
        );
    }

    #[test]
    fn test_color_channel_round_trip() {
        for ch in [
            ColorChannel::Color,
            ColorChannel::Red,
            ColorChannel::Green,
            ColorChannel::Blue,
            ColorChannel::Alpha,
            ColorChannel::Luminance,
        ] {
            let tok = ch.to_token();
            assert_eq!(ColorChannel::from_token(&tok), ch);
        }
    }

    #[test]
    fn test_parameter_buffer_default() {
        let buf = ColorChannelParameterBuffer::default();
        assert_eq!(buf.screen_size, [0.0, 0.0]);
        assert_eq!(buf.channel, 0); // color pass-through
    }

    #[test]
    fn test_update_parameter_buffer() {
        let mut task = HdxColorChannelTask::new(Path::from_string("/ch").unwrap());
        // First update — values differ from default (0,0), returns true.
        let changed = task.update_parameter_buffer(1920.0, 1080.0);
        assert!(changed);
        assert_eq!(task.get_parameter_data().screen_size, [1920.0, 1080.0]);
        // Same values again — no change.
        let changed2 = task.update_parameter_buffer(1920.0, 1080.0);
        assert!(!changed2);
    }

    #[test]
    fn test_channel_index() {
        assert_eq!(ColorChannel::Color.as_channel_index(), 0);
        assert_eq!(ColorChannel::Red.as_channel_index(), 1);
        assert_eq!(ColorChannel::Green.as_channel_index(), 2);
        assert_eq!(ColorChannel::Blue.as_channel_index(), 3);
        assert_eq!(ColorChannel::Alpha.as_channel_index(), 4);
        assert_eq!(ColorChannel::Luminance.as_channel_index(), 5);
    }
}
