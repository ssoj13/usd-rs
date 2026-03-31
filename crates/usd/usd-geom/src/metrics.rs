//! UsdGeom metrics - utilities for encoding spatial and geometric metrics.
//!
//! Port of pxr/usd/usdGeom/metrics.h/cpp
//!
//! Schema and utilities for encoding various spatial and geometric metrics of
//! a UsdStage and its contents, particularly the stage up axis.

use super::tokens::usd_geom_tokens;
use usd_core::Stage;
use usd_tf::Token;

// ============================================================================
// Stage Up Axis
// ============================================================================

/// Fetch and return stage's upAxis.
///
/// If unauthored, will return the value provided by `get_fallback_up_axis()`.
/// Exporters, however, are strongly encouraged to always set the upAxis for
/// every USD file they create.
///
/// Returns one of: `usd_geom_tokens().y` or `usd_geom_tokens().z`, unless there
/// was an error, in which case returns an empty Token.
///
/// Matches C++ `UsdGeomGetStageUpAxis(const UsdStageWeakPtr &stage)`.
pub fn get_stage_up_axis(stage: &Stage) -> Token {
    // Stage is always valid if it exists (it's an Arc<Stage>)

    // Check if upAxis metadata is authored
    let up_axis_token = usd_geom_tokens().up_axis.clone();
    if let Some(metadata_value) = stage.get_metadata(&up_axis_token) {
        if let Some(token_val) = metadata_value.downcast::<Token>() {
            return token_val.clone();
        }
        // Try string conversion
        if let Some(str_val) = metadata_value.downcast::<String>() {
            let axis_token = Token::new(str_val);
            // Validate it's Y or Z
            if axis_token == usd_geom_tokens().y || axis_token == usd_geom_tokens().z {
                return axis_token;
            }
        }
    }

    // Return fallback
    get_fallback_up_axis()
}

/// Set stage's upAxis to `axis`, which must be one of `usd_geom_tokens().y`
/// or `usd_geom_tokens().z`.
///
/// UpAxis is stage-level metadata, therefore see `Stage::set_metadata()`.
///
/// Returns true if upAxis was successfully set. The stage's edit target
/// must be either its root layer or session layer.
///
/// Matches C++ `UsdGeomSetStageUpAxis(const UsdStageWeakPtr &stage, const TfToken &axis)`.
pub fn set_stage_up_axis(stage: &Stage, axis: &Token) -> bool {
    // Validate axis is Y or Z
    let tokens = usd_geom_tokens();
    if *axis != tokens.y && *axis != tokens.z {
        return false;
    }

    // Set metadata
    let up_axis_token = tokens.up_axis.clone();
    use usd_vt::Value;
    stage.set_metadata(&up_axis_token, Value::from_no_hash(axis.clone()))
}

// ============================================================================
// Linear Units
// ============================================================================

/// Container class for static double-precision symbols representing common
/// units of measure expressed in meters.
///
/// Matches C++ `UsdGeomLinearUnits`.
pub struct LinearUnits;

impl LinearUnits {
    /// Nanometers (1e-9 meters).
    pub const NANOMETERS: f64 = 1e-9;
    /// Micrometers (1e-6 meters).
    pub const MICROMETERS: f64 = 1e-6;
    /// Millimeters (0.001 meters).
    pub const MILLIMETERS: f64 = 0.001;
    /// Centimeters (0.01 meters).
    pub const CENTIMETERS: f64 = 0.01;
    /// Meters (1.0).
    pub const METERS: f64 = 1.0;
    /// Kilometers (1000 meters).
    pub const KILOMETERS: f64 = 1000.0;
    /// Light years (measured for one year = 365.25 days).
    pub const LIGHT_YEARS: f64 = 9.4607304725808e15;
    /// Inches (0.0254 meters).
    pub const INCHES: f64 = 0.0254;
    /// Feet (0.3048 meters).
    pub const FEET: f64 = 0.3048;
    /// Yards (0.9144 meters).
    pub const YARDS: f64 = 0.9144;
    /// Miles (1609.344 meters).
    pub const MILES: f64 = 1609.344;
}

/// Return stage's authored metersPerUnit, or 0.01 (centimeters) if unauthored.
///
/// Matches C++ `UsdGeomGetStageMetersPerUnit(const UsdStageWeakPtr &stage)`.
pub fn get_stage_meters_per_unit(stage: &Stage) -> f64 {
    let meters_per_unit_token = usd_geom_tokens().meters_per_unit.clone();
    if let Some(metadata_value) = stage.get_metadata(&meters_per_unit_token) {
        if let Some(double_val) = metadata_value.downcast::<f64>() {
            return *double_val;
        }
    }

    // Default to centimeters
    LinearUnits::CENTIMETERS
}

/// Return whether stage has an authored metersPerUnit.
///
/// Matches C++ `UsdGeomStageHasAuthoredMetersPerUnit(const UsdStageWeakPtr &stage)`.
pub fn stage_has_authored_meters_per_unit(stage: &Stage) -> bool {
    let meters_per_unit_token = usd_geom_tokens().meters_per_unit.clone();
    stage.has_authored_metadata(&meters_per_unit_token)
}

/// Author stage's metersPerUnit.
///
/// Returns true if metersPerUnit was successfully set. The stage's edit target
/// must be either its root layer or session layer.
///
/// Matches C++ `UsdGeomSetStageMetersPerUnit(const UsdStageWeakPtr &stage, double metersPerUnit)`.
pub fn set_stage_meters_per_unit(stage: &Stage, meters_per_unit: f64) -> bool {
    let meters_per_unit_token = usd_geom_tokens().meters_per_unit.clone();
    use usd_vt::Value;
    stage.set_metadata(&meters_per_unit_token, Value::from_no_hash(meters_per_unit))
}

/// Return true if the two given metrics are within the provided relative
/// epsilon of each other, when you need to know an absolute metric rather
/// than a scaling factor.
///
/// Returns false if either input is zero or negative, otherwise relative
/// floating-point comparison between the two inputs.
///
/// Matches C++ `UsdGeomLinearUnitsAre(double authoredUnits, double standardUnits, double epsilon)`.
pub fn linear_units_are(authored_units: f64, standard_units: f64, epsilon: f64) -> bool {
    if authored_units <= 0.0 || standard_units <= 0.0 {
        return false;
    }

    let diff = (authored_units - standard_units).abs();
    (diff / authored_units < epsilon) && (diff / standard_units < epsilon)
}

/// Return the site-level fallback up axis as a Token.
///
/// In a generic installation of USD, the fallback will be "Y". This can be
/// overridden by:
/// 1. Setting the `USD_GEOM_FALLBACK_UP_AXIS` environment variable ("Y" or "Z")
/// 2. A plugin providing "UsdGeomMetrics" metadata with an "upAxis" key
///    (in its plugInfo.json "Info" dict)
///
/// If multiple plugins disagree on the axis, the schema default "Y" is used
/// and an error is logged, matching C++ behavior.
///
/// Matches C++ `UsdGeomGetFallbackUpAxis()`.
pub fn get_fallback_up_axis() -> Token {
    use std::sync::OnceLock;
    use usd_plug::PlugRegistry;

    static FALLBACK_UP_AXIS: OnceLock<Token> = OnceLock::new();

    FALLBACK_UP_AXIS
        .get_or_init(|| {
            // Env var takes priority (dev convenience, matches common USD conventions).
            if let Ok(env_value) = std::env::var("USD_GEOM_FALLBACK_UP_AXIS") {
                let upper = env_value.to_uppercase();
                if upper == "Y" {
                    return usd_geom_tokens().y.clone();
                } else if upper == "Z" {
                    return usd_geom_tokens().z.clone();
                }
                // Invalid value -- fall through to plugin scan.
                log::warn!(
                    "USD_GEOM_FALLBACK_UP_AXIS has invalid value '{}'; ignoring.",
                    env_value
                );
            }

            // Scan all registered plugins for "UsdGeomMetrics" -> "upAxis".
            // Matches C++ metrics.cpp GetFallbackUpAxis() plugin scan logic.
            let registry = PlugRegistry::get_instance();
            let plugins = registry.get_all_plugins();

            let tokens = usd_geom_tokens();
            let schema_default = tokens.y.clone();
            let mut plugin_axis: Option<Token> = None;
            let mut conflict = false;

            for plugin in &plugins {
                let meta = plugin.get_metadata();
                // Look for "UsdGeomMetrics" dict inside the plugin's Info.
                let geom_metrics = match meta.get("UsdGeomMetrics").and_then(|v| v.as_object()) {
                    Some(obj) => obj,
                    None => continue,
                };
                let up_axis_str = match geom_metrics.get("upAxis").and_then(|v| v.as_string()) {
                    Some(s) => s,
                    None => continue,
                };

                let upper = up_axis_str.to_uppercase();
                let candidate = Token::new(&upper);
                // Only "Y" and "Z" are valid axis values.
                if candidate != tokens.y && candidate != tokens.z {
                    log::warn!(
                        "Plugin '{}' has invalid UsdGeomMetrics.upAxis '{}'; ignoring.",
                        plugin.get_name(),
                        up_axis_str
                    );
                    continue;
                }

                match &plugin_axis {
                    None => {
                        plugin_axis = Some(candidate);
                    }
                    Some(existing) if *existing != candidate => {
                        // Two plugins disagree -- use schema default and log error.
                        log::error!(
                            "Plugins disagree on UsdGeomMetrics.upAxis: '{}' vs '{}'. \
                             Using schema fallback '{}'.",
                            existing,
                            candidate,
                            schema_default
                        );
                        conflict = true;
                        break;
                    }
                    _ => {} // Same value as already seen.
                }
            }

            if conflict {
                schema_default
            } else {
                plugin_axis.unwrap_or(schema_default)
            }
        })
        .clone()
}
