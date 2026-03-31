# UsdRi Module - RenderMan Integration

Rust port of OpenUSD `pxr/usd/usdRi`. RenderMan-specific schemas and utilities.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdRi/*.h` on 2026-02-08.

---

| C++ Header | Rust File | Status |
|---|---|---|
| materialAPI.h | material_api.rs | 100% |
| rmanUtilities.h | rman_utilities.rs | 100% |
| splineAPI.h | spline_api.rs | 100% |
| statementsAPI.h | statements_api.rs | 100% |
| typeUtils.h | type_utils.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdRi module: 100% API parity with OpenUSD C++ reference.**

All 6 public C++ headers fully covered. 7 Rust source files. 0 API gaps.

Verified 2026-02-08.
