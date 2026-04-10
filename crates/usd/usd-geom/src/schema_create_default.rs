//! Optional default time samples for `Create*Attr` — parity with pxr `UsdGeom` wraps.

use usd_core::Attribute;
use usd_sdf::TimeCode;
use usd_vt::Value;

/// If `default_value` is `Some`, set it at default time on `attr` (no-op when `None`).
#[must_use]
pub fn apply_optional_default(attr: Attribute, default_value: Option<Value>) -> Attribute {
    if let Some(val) = default_value {
        let _ = attr.set(val, TimeCode::default());
    }
    attr
}
