# UsdShade Module - Shading Schemas

Rust port of OpenUSD `pxr/usd/usdShade`. Material, shader, and shading network system.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdShade/*.h` on 2026-02-08.

---

### Core Types

| C++ Header | Rust File | Status |
|---|---|---|
| material.h | material.rs | 100% |
| shader.h | shader.rs | 100% |
| nodeGraph.h | node_graph.rs | 100% |
| input.h | input.rs | 100% |
| output.h | output.rs | 100% |
| types.h | types.rs | 100% |
| tokens.h | tokens.rs | 100% |

### APIs

| C++ Header | Rust File | Status |
|---|---|---|
| connectableAPI.h | connectable_api.rs | 100% |
| connectableAPIBehavior.h | connectable_api_behavior.rs | 100% |
| materialBindingAPI.h | material_binding_api.rs | 100% |
| coordSysAPI.h | coord_sys_api.rs | 100% |
| nodeDefAPI.h | node_def_api.rs | 100% |

### Utilities

| C++ Header | Rust File | Status |
|---|---|---|
| shaderDefParser.h | shader_def_parser.rs | 100% |
| shaderDefUtils.h | shader_def_utils.rs | 100% |
| udimUtils.h | udim_utils.rs | 100% |
| utils.h | utils.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdShade module: 100% API parity with OpenUSD C++ reference.**

All 16 public C++ headers fully covered. 17 Rust source files. 0 API gaps.

Verified 2026-02-08.
