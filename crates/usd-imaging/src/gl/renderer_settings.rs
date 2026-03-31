//! Renderer settings for UsdImagingGL.
//!
//! This module defines renderer-specific settings that can be queried and configured.

use usd_tf::Token;
use usd_vt::Value;

/// Type of renderer setting value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum RendererSettingType {
    /// Boolean flag setting
    Flag,
    /// Integer value setting
    Int,
    /// Floating point value setting
    Float,
    /// String value setting
    String,
}

/// A single renderer setting with its metadata.
///
/// Renderer settings allow configuration of renderer-specific options
/// such as sampling rates, quality levels, etc.
#[derive(Debug, Clone, PartialEq)]
pub struct RendererSetting {
    /// Human-readable name of the setting
    pub name: String,

    /// Token key used to identify the setting
    pub key: Token,

    /// Type of the setting value
    pub setting_type: RendererSettingType,

    /// Default value for the setting
    pub default_value: Value,
}

impl RendererSetting {
    /// Creates a new renderer setting.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name
    /// * `key` - Token key for identification
    /// * `setting_type` - Type of the setting value
    /// * `default_value` - Default value
    pub fn new(
        name: impl Into<String>,
        key: Token,
        setting_type: RendererSettingType,
        default_value: Value,
    ) -> Self {
        Self {
            name: name.into(),
            key,
            setting_type,
            default_value,
        }
    }

    /// Creates a boolean flag setting.
    pub fn flag(name: impl Into<String>, key: Token, default: bool) -> Self {
        Self::new(name, key, RendererSettingType::Flag, Value::from(default))
    }

    /// Creates an integer setting.
    pub fn int(name: impl Into<String>, key: Token, default: i32) -> Self {
        Self::new(name, key, RendererSettingType::Int, Value::from(default))
    }

    /// Creates a float setting.
    pub fn float(name: impl Into<String>, key: Token, default: f32) -> Self {
        Self::new(name, key, RendererSettingType::Float, Value::from(default))
    }

    /// Creates a string setting.
    pub fn string(name: impl Into<String>, key: Token, default: impl Into<String>) -> Self {
        Self::new(
            name,
            key,
            RendererSettingType::String,
            Value::from(default.into()),
        )
    }
}

/// List of renderer settings.
pub type RendererSettingsList = Vec<RendererSetting>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_setting_flag() {
        let setting = RendererSetting::flag("Enable Shadows", Token::new("enableShadows"), true);

        assert_eq!(setting.name, "Enable Shadows");
        assert_eq!(setting.key, Token::new("enableShadows"));
        assert_eq!(setting.setting_type, RendererSettingType::Flag);
    }

    #[test]
    fn test_renderer_setting_int() {
        let setting = RendererSetting::int("Max Samples", Token::new("maxSamples"), 128);

        assert_eq!(setting.name, "Max Samples");
        assert_eq!(setting.key, Token::new("maxSamples"));
        assert_eq!(setting.setting_type, RendererSettingType::Int);
    }

    #[test]
    fn test_renderer_setting_float() {
        let setting = RendererSetting::float("Quality", Token::new("quality"), 1.0);

        assert_eq!(setting.name, "Quality");
        assert_eq!(setting.setting_type, RendererSettingType::Float);
    }

    #[test]
    fn test_renderer_setting_string() {
        let setting = RendererSetting::string("Output Format", Token::new("outputFormat"), "png");

        assert_eq!(setting.name, "Output Format");
        assert_eq!(setting.setting_type, RendererSettingType::String);
    }
}
