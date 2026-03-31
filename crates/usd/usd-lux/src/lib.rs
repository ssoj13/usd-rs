//! USD Lux module - lighting schemas for USD.
//!
//! This module provides USD lighting schemas, matching C++ `pxr/usd/usdLux`.
//!
//! # Overview
//!
//! UsdLux provides a core set of light types and APIs for scene illumination:
//!
//! ## Concrete Light Types
//!
//! **Boundable lights** (have geometric extent):
//! - [`SphereLight`] - Point light with visible radius
//! - [`DiskLight`] - Circular area light
//! - [`RectLight`] - Rectangular area light
//! - [`CylinderLight`] - Tubular area light
//!
//! **Non-boundable lights** (infinite extent):
//! - [`DistantLight`] - Directional light (sun-like)
//! - [`DomeLight`] - Environment/IBL light
//!
//! ## API Schemas
//!
//! - [`LightAPI`] - Core light properties (intensity, color, exposure)
//! - [`ShadowAPI`] - Shadow controls (enable, color, distance)
//! - [`ShapingAPI`] - Light shaping (cone, focus, IES profiles)
//!
//! ## Base Classes
//!
//! - [`BoundableLightBase`] - Base for lights with geometric extent
//! - [`NonboundableLightBase`] - Base for infinite lights
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdLux`

pub mod blackbody;
pub mod boundable_light_base;
pub mod cylinder_light;
pub mod discovery_plugin;
pub mod disk_light;
pub mod distant_light;
pub mod dome_light;
pub mod dome_light_1;
pub mod geometry_light;
pub mod light_api;
pub mod light_def_parser;
pub mod light_filter;
pub mod light_list_api;
pub mod list_api;
pub mod mesh_light_api;
pub mod nonboundable_light_base;
pub mod plugin_light;
pub mod plugin_light_filter;
pub mod portal_light;
pub mod rect_light;
pub mod shadow_api;
pub mod shaping_api;
pub mod sphere_light;
pub mod tokens;
pub mod volume_light_api;

// Re-export main types
pub use blackbody::blackbody_temperature_as_rgb;
pub use boundable_light_base::BoundableLightBase;
pub use cylinder_light::CylinderLight;
pub use discovery_plugin::DiscoveryPlugin;
pub use disk_light::DiskLight;
pub use distant_light::DistantLight;
pub use dome_light::DomeLight;
pub use dome_light_1::DomeLight1;
#[allow(deprecated)]
pub use geometry_light::GeometryLight;
pub use light_api::LightAPI;
pub use light_def_parser::LightDefParserPlugin;
pub use light_filter::LightFilter;
pub use light_list_api::{ComputeMode, LightListAPI};
pub use list_api::ComputeMode as ListAPIComputeMode;
#[allow(deprecated)]
pub use list_api::ListAPI;
pub use mesh_light_api::MeshLightAPI;
pub use nonboundable_light_base::NonboundableLightBase;
pub use plugin_light::PluginLight;
pub use plugin_light_filter::PluginLightFilter;
pub use portal_light::PortalLight;
pub use rect_light::RectLight;
pub use shadow_api::ShadowAPI;
pub use shaping_api::ShapingAPI;
pub use sphere_light::SphereLight;
pub use tokens::{UsdLuxTokens, tokens};
pub use volume_light_api::VolumeLightAPI;

use std::sync::Once;

static INIT_PLUGINS: Once = Once::new();

/// Initialize UsdLux plugins: register light discovery and parser plugins
/// with the SdrRegistry.
///
/// Matches C++ `SDR_REGISTER_PARSER_PLUGIN(UsdLux_LightDefParserPlugin)` +
/// automatic plugin discovery via plugInfo.json.
///
/// Must be called before using SdrRegistry to query light shader nodes.
pub fn init() {
    INIT_PLUGINS.call_once(|| {
        let registry = usd_sdr::registry::SdrRegistry::get_instance();

        // Register parser plugin for "usd-schema-gen" discovery type
        registry.register_parser_plugin(Box::new(LightDefParserPlugin::new()));

        // Register discovery plugin and run it
        registry.set_extra_discovery_plugins(vec![Box::new(DiscoveryPlugin::new())]);
    });
}
