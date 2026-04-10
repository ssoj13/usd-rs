//! Create-or-get helpers for UsdLux typed and API schemas (`UsdSchemaBase::_CreateAttr` parity).

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim};
use usd_geom::apply_optional_default;
use usd_sdf::ValueTypeRegistry;
use usd_tf::Token;
use usd_vt::Value;

/// Create-or-get a schema attribute; optional default at default time.
///
/// Matches C++ `UsdSchemaBase::_CreateAttr` used across `usdLux` generated schemas.
#[must_use]
pub(crate) fn create_lux_schema_attr(
    prim: &Prim,
    name: &str,
    sdf_typename: &str,
    variability: Variability,
    default_value: Option<Value>,
    _write_sparsely: bool,
) -> Attribute {
    if !prim.is_valid() {
        return Attribute::invalid();
    }
    let attr = if prim.has_authored_attribute(name) {
        prim.get_attribute(name).unwrap_or_else(Attribute::invalid)
    } else {
        let type_registry = ValueTypeRegistry::instance();
        let value_type = type_registry.find_type_by_token(&Token::new(sdf_typename));
        prim
            .create_attribute(name, &value_type, false, Some(variability))
            .unwrap_or_else(Attribute::invalid)
    };
    apply_optional_default(attr, default_value)
}
