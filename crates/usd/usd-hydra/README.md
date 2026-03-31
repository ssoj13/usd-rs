# UsdHydra Module - Hydra Integration Schemas

Rust port of OpenUSD `pxr/usd/usdHydra`. Hydra-specific schemas.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified function-by-function against
`_ref/OpenUSD/pxr/usd/usdHydra/*.h` on 2026-03-17.

| C++ Header | Rust File | Status |
|---|---|---|
| generativeProceduralAPI.h | generative_procedural_api.rs | 100% |
| tokens.h | tokens.rs | 100% (30/30 tokens) |
| discoveryPlugin.h/.cpp | discovery_plugin.rs | 100% |

### Not Ported (not applicable to Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| wrapGenerativeProceduralAPI.cpp / wrapTokens.cpp | Python bindings |

## Design: shader resource loading

C++ loads `shaderDefs.usda` at runtime via the plugin system
(`PLUG_THIS_PLUGIN` + `PlugFindPluginResource`), resolving the file relative to
the shared library's resource directory through `plugInfo.json`.

In Rust we embed the USDA at compile time via `include_str!` instead. Our
`usd-plug` crate does provide the full plugin registry and
`find_plugin_resource()`, so a runtime approach is possible. Embedding is
preferred because:

- **Reliability** -- baked into binary, no missing-resource errors at runtime.
- **Simplicity** -- no plugInfo.json, no resource directory deployment.
- **Size** -- shaderDefs.usda is ~8 KB, negligible binary overhead.
- **Identical result** -- both paths parse the same USDA and produce the same
  `SdrShaderNodeDiscoveryResult` set.

## Summary

**100% API parity.** 3 C++ headers, 3 Rust files, 30 tokens, 0 API gaps.
No testenv in the C++ reference -- nothing to port.

Verified 2026-03-17.
