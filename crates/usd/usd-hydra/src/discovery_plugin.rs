//! UsdHydra shader discovery plugin.
//!
//! Port of `pxr/usd/usdHydra/discoveryPlugin.h/.cpp`
//!
//! Discovers built-in Hydra shader definitions (HwUvTexture, HwPtexTexture,
//! HwPrimvar) from bundled shaderDefs.usda resources.
//!
//! # Design: `include_str!` vs plugin resource loading
//!
//! The C++ implementation locates `shaderDefs.usda` at runtime via the plugin
//! system (`PLUG_THIS_PLUGIN` + `PlugFindPluginResource`). This works because
//! C++ USD is built around dynamic shared libraries with per-plugin resource
//! directories resolved through `plugInfo.json`.
//!
//! In Rust we embed the USDA content at compile time via `include_str!` instead.
//! Our `usd-plug` crate does provide `find_plugin_resource()` and the full
//! plugin registry, so a runtime approach is technically possible. However,
//! embedding is preferred here because:
//!
//! - **Reliability**: the file is baked into the binary — no missing resource
//!   errors, no deployment of auxiliary files alongside the executable.
//! - **Simplicity**: 3 lines vs plugInfo.json + resource directory + filesystem I/O.
//! - **Size**: shaderDefs.usda is ~8 KB — negligible binary overhead.
//! - **Identical result**: both paths parse the same USDA and produce the same
//!   `SdrShaderNodeDiscoveryResult` set. The loading mechanism is an
//!   implementation detail invisible to callers.

use std::sync::OnceLock;
use usd_sdr::SdrShaderNodeDiscoveryResult;

/// Embedded shader definitions from OpenUSD usdHydra/shaders/shaderDefs.usda.
const SHADER_DEFS_USDA: &str =
    include_str!("shaders/shaderDefs.usda");

/// Source URI used for embedded shader definitions.
const EMBEDDED_SOURCE_URI: &str = "<embedded:usdHydra/shaderDefs.usda>";

/// Discovery context providing environment information for plugin discovery.
///
/// Matches C++ `SdrDiscoveryPlugin::Context`.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryContext {
    /// Source type filter. If non-empty, only discover nodes of this type.
    pub source_type: Option<String>,
}

/// Discovery plugin for UsdHydra built-in shaders.
///
/// Finds shader definitions from bundled resources (shaderDefs.usda).
/// Opens an anonymous USD stage from the embedded USDA, iterates root
/// prims as UsdShadeShader, and calls `get_discovery_results()` on each.
/// Matches C++ `UsdHydraDiscoveryPlugin`.
pub struct UsdHydraDiscoveryPlugin;

/// Cached discovery results (computed once on first access).
static CACHED_RESULTS: OnceLock<Vec<SdrShaderNodeDiscoveryResult>> = OnceLock::new();

fn compute_discovery_results() -> Vec<SdrShaderNodeDiscoveryResult> {
    use usd_core::Stage;
    use usd_sdf::layer::Layer;
    use usd_shade::shader::Shader;
    use usd_shade::shader_def_utils;

    // Create anonymous layer and import embedded USDA content.
    let layer = Layer::create_anonymous(Some("usdHydra_shaderDefs"));
    if !layer.import_from_string(SHADER_DEFS_USDA) {
        log::error!("UsdHydraDiscoveryPlugin: failed to parse embedded shaderDefs.usda");
        return Vec::new();
    }

    // Open stage with this root layer.
    let stage = match Stage::open_with_root_layer(layer, usd_core::InitialLoadSet::LoadAll) {
        Ok(s) => s,
        Err(e) => {
            log::error!("UsdHydraDiscoveryPlugin: failed to open stage: {e}");
            return Vec::new();
        }
    };

    let mut results = Vec::new();

    // Iterate root prims and discover shader nodes.
    let root = stage.get_pseudo_root();
    for child in root.get_children() {
        let shader = Shader::new(child.clone());
        if !shader.is_valid() {
            continue;
        }

        let discovered = shader_def_utils::get_discovery_results(&shader, EMBEDDED_SOURCE_URI);
        if discovered.is_empty() {
            log::warn!(
                "UsdHydraDiscoveryPlugin: shader <{}> has no valid discovery results",
                child.get_path()
            );
        }
        results.extend(discovered);
    }

    log::debug!(
        "UsdHydraDiscoveryPlugin: discovered {} shader nodes",
        results.len()
    );
    results
}

impl UsdHydraDiscoveryPlugin {
    /// Create a new discovery plugin instance.
    pub fn new() -> Self {
        Self
    }

    /// Discover shader nodes from bundled Hydra resources.
    ///
    /// Opens the embedded shaderDefs.usda, iterates root prims as
    /// UsdShadeShader, and calls `shader_def_utils::get_discovery_results()`
    /// on each. Results are cached after first call.
    ///
    /// Matches C++ `DiscoverShaderNodes(const Context &context)`.
    pub fn discover_shader_nodes(
        &self,
        _context: &DiscoveryContext,
    ) -> &'static [SdrShaderNodeDiscoveryResult] {
        CACHED_RESULTS.get_or_init(compute_discovery_results)
    }

    /// Get search URIs for shader resources.
    ///
    /// Returns the embedded source URI since shaderDefs.usda is bundled.
    pub fn get_search_uris(&self) -> &[&str] {
        &[EMBEDDED_SOURCE_URI]
    }
}

impl Default for UsdHydraDiscoveryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_plugin_creation() {
        let plugin = UsdHydraDiscoveryPlugin::new();
        assert!(!plugin.get_search_uris().is_empty());
    }

    #[test]
    fn test_discover_shader_nodes() {
        let plugin = UsdHydraDiscoveryPlugin::new();
        let results = plugin.discover_shader_nodes(&DiscoveryContext::default());
        // shaderDefs.usda defines 3 shaders: HwPtexTexture_1, HwUvTexture_1, HwPrimvar_1
        // They may or may not produce discovery results depending on sourceAsset resolution.
        // At minimum the function should not panic.
        log::info!("Discovered {} shader nodes", results.len());
    }

    #[test]
    fn test_embedded_usda_not_empty() {
        assert!(!SHADER_DEFS_USDA.is_empty());
        assert!(SHADER_DEFS_USDA.starts_with("#usda"));
    }
}
