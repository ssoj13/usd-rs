//! Stringify `VtValue` entries from `sdrMetadata` dictionaries for SDR consumers.
//!
//! Matches C++ `UsdShade` / `TfStringify`-style conversion: token and string values
//! must become plain text (not `Value`/`Token` debug formatting), or conditionalVis
//! parsing in [`usd_sdr::shader_metadata_helpers`] will not recognize operators.

use usd_tf::Token;
use usd_vt::Value;

/// Converts a composed `sdrMetadata` dictionary value to the string form SDR expects.
#[must_use]
pub(crate) fn sdr_metadata_value_string(value: &Value) -> String {
    if let Some(s) = value.get::<String>() {
        return s.clone();
    }
    if let Some(t) = value.get::<Token>() {
        return t.as_str().to_string();
    }
    if let Some(b) = value.get::<bool>() {
        return b.to_string();
    }
    if let Some(i) = value.get::<i32>() {
        return i.to_string();
    }
    if let Some(i) = value.get::<i64>() {
        return i.to_string();
    }
    if let Some(f) = value.get::<f32>() {
        return f.to_string();
    }
    if let Some(f) = value.get::<f64>() {
        return f.to_string();
    }
    value.stream_out()
}
