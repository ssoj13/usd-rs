# UsdUI Module - User Interface Schemas

Rust port of OpenUSD `pxr/usd/usdUI`. Node graph layout, accessibility, and UI hints.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdUI/*.h` on 2026-02-08.

All UsdUI content (schema types and UI hint classes) is in `schema::ui`.

---

### Schema Types (schema/ui/)

| C++ Header | Rust File | Status |
|---|---|---|
| backdrop.h | backdrop.rs | 100% |
| nodeGraphNodeAPI.h | node_graph_node_api.rs | 100% |
| sceneGraphPrimAPI.h | scene_graph_prim_api.rs | 100% |
| accessibilityAPI.h | accessibility_api.rs | 100% |
| tokens.h | tokens.rs | 100% |

### UI Hints (schema::ui)

| C++ Header | Rust File | Status |
|---|---|---|
| attributeHints.h | attribute_hints.rs | 100% |
| objectHints.h | object_hints.rs | 100% |
| primHints.h | prim_hints.rs | 100% |
| propertyHints.h | property_hints.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdUI module: 100% API parity with OpenUSD C++ reference.**

All 9 public C++ headers fully covered across 2 Rust modules. 0 API gaps.

Verified 2026-02-08.
