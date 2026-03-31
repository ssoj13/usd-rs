# SDR Module - Shader Definition Registry

Rust port of OpenUSD `pxr/usd/sdr`. Shader node discovery, parsing, and registry.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Method-by-method audit in [SDR_PARITY_REPORT.md](SDR_PARITY_REPORT.md) (2026-02-15).

---

### Core Types

| C++ Header | Rust File | Status |
|---|---|---|
| registry.h | registry.rs | 100% — singleton shader registry |
| shaderNode.h | shader_node.rs | 100% — complete shader definition |
| shaderProperty.h | shader_property.rs | 100% — input/output properties |
| declare.h | declare.rs | 100% |

### Discovery System

| C++ Header | Rust File | Status |
|---|---|---|
| discoveryPlugin.h | discovery_plugin.rs | 100% |
| filesystemDiscovery.h | filesystem_discovery.rs | 100% |
| filesystemDiscoveryHelpers.h | filesystem_discovery_helpers.rs | 100% |
| shaderNodeDiscoveryResult.h | discovery_result.rs | 100% |

### Parser Plugins

| C++ Header | Rust File | Status |
|---|---|---|
| parserPlugin.h | parser_plugin.rs | 100% |
| N/A | args_parser.rs | RenderMan .args XML parser |
| N/A | sdrosl_parser.rs | SdrOsl JSON parser |
| N/A | osl_parser.rs | OSL parser |
| N/A | usd_shaders.rs | UsdPreviewSurface built-in |

### Metadata & Utilities

| C++ Header | Rust File | Status |
|---|---|---|
| shaderMetadataHelpers.h | shader_metadata_helpers.rs | 100% |
| shaderNodeMetadata.h | shader_node_metadata.rs | 100% |
| shaderPropertyMetadata.h | shader_property_metadata.rs | 100% |
| shaderNodeQuery.h | shader_node_query.rs | 100% |
| shaderNodeQueryUtils.h | shader_node_query_utils.rs | 100% |
| sdfTypeIndicator.h | sdf_type_indicator.rs | 100% |

### Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| debugCodes.h | Trivial debug flags |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |
| overview.dox | Doxygen docs |

---

## Summary

**SDR module: 100% API parity with OpenUSD C++ reference.**

All 16 public C++ headers fully covered. 22 Rust source files. 0 API gaps.

Verified 2026-02-15. See [SDR_PARITY_REPORT.md](SDR_PARITY_REPORT.md).
