//! UsdLux_DiscoveryPlugin - light shader node discovery.
//!
//! Port of pxr/usd/usdLux/discoveryPlugin.h/cpp
//!
//! Discovers shader nodes for concrete UsdLux light types so they
//! appear in the shader registry. Implements the SdrDiscoveryPlugin trait.

use usd_sdr::declare::SdrVersion;
use usd_sdr::discovery_plugin::{SdrDiscoveryPlugin, SdrDiscoveryPluginContext};
use usd_sdr::discovery_result::SdrShaderNodeDiscoveryResult;
use usd_sdr::{SdrShaderNodeDiscoveryResultVec, SdrStringVec, SdrTokenMap};
use usd_tf::Token;

/// Known concrete UsdLux light type names.
static CONCRETE_LIGHT_TYPES: &[&str] = &[
    "CylinderLight",
    "DiskLight",
    "DistantLight",
    "DomeLight",
    "DomeLight_1",
    "GeometryLight",
    "PluginLight",
    "PortalLight",
    "RectLight",
    "SphereLight",
];

/// API schema shader IDs that also need Sdr representation.
static API_SHADER_IDS: &[&str] = &["MeshLight", "VolumeLight"];

/// Discovery plugin for UsdLux light shader nodes.
///
/// Matches C++ `UsdLux_DiscoveryPlugin`.
pub struct DiscoveryPlugin;

impl DiscoveryPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiscoveryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SdrDiscoveryPlugin for DiscoveryPlugin {
    fn discover_shader_nodes(
        &self,
        _context: &dyn SdrDiscoveryPluginContext,
    ) -> SdrShaderNodeDiscoveryResultVec {
        let discovery_type = Token::new("usd-schema-gen");
        let source_type = Token::new("USD");
        let empty_metadata = SdrTokenMap::new();

        let mut results = Vec::new();

        for &type_name in CONCRETE_LIGHT_TYPES {
            results.push(SdrShaderNodeDiscoveryResult::new(
                Token::new(type_name),
                SdrVersion::invalid(),
                type_name.to_string(),
                Token::new(""),
                discovery_type.clone(),
                source_type.clone(),
                String::new(),
                String::new(),
                String::new(),
                empty_metadata.clone(),
                String::new(),
                Token::new(""),
            ));
        }

        for &shader_id in API_SHADER_IDS {
            results.push(SdrShaderNodeDiscoveryResult::new(
                Token::new(shader_id),
                SdrVersion::invalid(),
                shader_id.to_string(),
                Token::new(""),
                discovery_type.clone(),
                source_type.clone(),
                String::new(),
                String::new(),
                String::new(),
                empty_metadata.clone(),
                String::new(),
                Token::new(""),
            ));
        }

        results
    }

    fn get_search_uris(&self) -> SdrStringVec {
        Vec::new()
    }

    fn get_name(&self) -> &str {
        "UsdLux_DiscoveryPlugin"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_sdr::discovery_plugin::DefaultDiscoveryPluginContext;

    #[test]
    fn test_discover_count() {
        let plugin = DiscoveryPlugin::new();
        let ctx = DefaultDiscoveryPluginContext;
        let results = plugin.discover_shader_nodes(&ctx);
        assert_eq!(results.len(), 12);
    }

    #[test]
    fn test_discover_source_type() {
        let plugin = DiscoveryPlugin::new();
        let ctx = DefaultDiscoveryPluginContext;
        let results = plugin.discover_shader_nodes(&ctx);
        for r in &results {
            assert_eq!(r.source_type.as_str(), "USD");
        }
    }
}
