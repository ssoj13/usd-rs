# UsdLux Module - Lighting Schemas

Rust port of OpenUSD `pxr/usd/usdLux`. Light sources, shadows, and light linking.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdLux/*.h` on 2026-02-08.

---

### Light Types

| C++ Header | Rust File | Status |
|---|---|---|
| distantLight.h | distant_light.rs | 100% |
| domeLight.h / domeLight_1.h | dome_light.rs + dome_light_1.rs | 100% |
| rectLight.h | rect_light.rs | 100% |
| diskLight.h | disk_light.rs | 100% |
| sphereLight.h | sphere_light.rs | 100% |
| cylinderLight.h | cylinder_light.rs | 100% |
| geometryLight.h | geometry_light.rs | 100% |
| portalLight.h | portal_light.rs | 100% |
| pluginLight.h | plugin_light.rs | 100% |
| pluginLightFilter.h | plugin_light_filter.rs | 100% |

### Base Classes

| C++ Header | Rust File | Status |
|---|---|---|
| boundableLightBase.h | boundable_light_base.rs | 100% |
| nonboundableLightBase.h | nonboundable_light_base.rs | 100% |
| lightFilter.h | light_filter.rs | 100% |

### APIs

| C++ Header | Rust File | Status |
|---|---|---|
| lightAPI.h | light_api.rs | 100% |
| shadowAPI.h | shadow_api.rs | 100% |
| shapingAPI.h | shaping_api.rs | 100% |
| lightListAPI.h | light_list_api.rs | 100% |
| listAPI.h | list_api.rs | 100% |
| meshLightAPI.h | mesh_light_api.rs | 100% |
| volumeLightAPI.h | volume_light_api.rs | 100% |

### Utilities

| C++ Header | Rust File | Status |
|---|---|---|
| blackbody.h | blackbody.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| discoveryPlugin.h | Plugin discovery — handled via static registration |
| lightDefParser.h | Plugin-based parser — integrated into SDR |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdLux module: 100% API parity with OpenUSD C++ reference.**

All 23 public C++ headers fully covered. 24 Rust source files. 0 API gaps.

Verified 2026-02-08.
