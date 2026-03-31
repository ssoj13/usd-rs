# UsdRender Module - Render Settings Schemas

Rust port of OpenUSD `pxr/usd/usdRender`. Render settings, passes, products, and variables.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdRender/*.h` on 2026-02-08.

---

| C++ Header | Rust File | Status |
|---|---|---|
| settings.h | settings.rs | 100% |
| settingsBase.h | settings_base.rs | 100% |
| pass.h | pass.rs | 100% |
| product.h | product.rs | 100% |
| spec.h | spec.rs | 100% |
| var.h | var.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |

---

## Summary

**UsdRender module: 100% API parity with OpenUSD C++ reference.**

All 7 public C++ headers fully covered. 8 Rust source files. 0 API gaps.

Verified 2026-02-08.
