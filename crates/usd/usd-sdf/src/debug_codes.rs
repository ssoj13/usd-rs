//! SdfDebugCodes - debug codes for SDF module.
//!
//! Port of pxr/usd/sdf/debugCodes.h
//!
//! Defines debug codes used throughout the SDF module for tracing and
//! diagnostic output.

use usd_tf::debug::DebugCode;

/// Debug codes for SDF.
pub struct SdfDebugCodes;

impl SdfDebugCodes {
    /// Debug code for asset path resolution.
    pub const ASSET_PATH_RESOLUTION: DebugCode = DebugCode::new("SDF_ASSET");

    /// Debug code for change processing.
    pub const CHANGES: DebugCode = DebugCode::new("SDF_CHANGES");

    /// Debug code for file format operations.
    pub const FILE_FORMAT: DebugCode = DebugCode::new("SDF_FILE_FORMAT");

    /// Debug code for layer operations.
    pub const LAYER: DebugCode = DebugCode::new("SDF_LAYER");

    /// Debug code for layer change notifications.
    pub const LAYER_CHANGES: DebugCode = DebugCode::new("SDF_LAYER_CHANGES");

    /// Debug code for layer loading.
    pub const LAYER_LOADING: DebugCode = DebugCode::new("SDF_LAYER_LOADING");

    /// Debug code for layer saving.
    pub const LAYER_SAVING: DebugCode = DebugCode::new("SDF_LAYER_SAVING");

    /// Debug code for layer registry.
    pub const LAYER_REGISTRY: DebugCode = DebugCode::new("SDF_LAYER_REGISTRY");

    /// Debug code for path operations.
    pub const PATH: DebugCode = DebugCode::new("SDF_PATH");

    /// Debug code for spec operations.
    pub const SPEC: DebugCode = DebugCode::new("SDF_SPEC");

    /// Debug code for text parser.
    pub const TEXT_PARSER: DebugCode = DebugCode::new("SDF_TEXT_PARSER");

    /// Debug code for variable expressions.
    pub const VARIABLE_EXPRESSION: DebugCode = DebugCode::new("SDF_VARIABLE_EXPRESSION");

    /// Debug code for value types.
    pub const VALUE_TYPES: DebugCode = DebugCode::new("SDF_VALUE_TYPES");

    /// Returns all debug codes.
    pub fn all() -> Vec<&'static DebugCode> {
        vec![
            &Self::ASSET_PATH_RESOLUTION,
            &Self::CHANGES,
            &Self::FILE_FORMAT,
            &Self::LAYER,
            &Self::LAYER_CHANGES,
            &Self::LAYER_LOADING,
            &Self::LAYER_SAVING,
            &Self::LAYER_REGISTRY,
            &Self::PATH,
            &Self::SPEC,
            &Self::TEXT_PARSER,
            &Self::VARIABLE_EXPRESSION,
            &Self::VALUE_TYPES,
        ]
    }

    /// Enables a debug code by name.
    pub fn enable(name: &str) -> bool {
        for code in Self::all() {
            if code.name() == name {
                code.enable();
                return true;
            }
        }
        false
    }

    /// Disables a debug code by name.
    pub fn disable(name: &str) -> bool {
        for code in Self::all() {
            if code.name() == name {
                code.disable();
                return true;
            }
        }
        false
    }

    /// Returns whether a debug code is enabled.
    pub fn is_enabled(name: &str) -> bool {
        for code in Self::all() {
            if code.name() == name {
                return code.is_enabled();
            }
        }
        false
    }
}

/// Macro to log debug messages for SDF.
#[macro_export]
macro_rules! sdf_debug {
    ($code:expr, $($arg:tt)*) => {
        if $code.is_enabled() {
            eprintln!("[{}] {}", $code.name(), format!($($arg)*));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_codes() {
        assert!(!SdfDebugCodes::LAYER.is_enabled());
        SdfDebugCodes::LAYER.enable();
        assert!(SdfDebugCodes::LAYER.is_enabled());
        SdfDebugCodes::LAYER.disable();
        assert!(!SdfDebugCodes::LAYER.is_enabled());
    }

    #[test]
    fn test_all_codes() {
        let codes = SdfDebugCodes::all();
        assert!(codes.len() >= 10);
    }
}
