# UsdVol Module - Volumetric Schemas

Rust port of OpenUSD `pxr/usd/usdVol`. Volumetric data sources.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdVol/*.h` on 2026-02-08.

---

| C++ Header | Rust File | Status |
|---|---|---|
| volume.h | volume.rs | 100% |
| fieldBase.h | field_base.rs | 100% |
| fieldAsset.h | field_asset.rs | 100% |
| field3DAsset.h | field_3d_asset.rs | 100% |
| openVDBAsset.h | open_vdb_asset.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdVol module: 100% API parity with OpenUSD C++ reference.**

All 6 public C++ headers fully covered. 7 Rust source files. 0 API gaps.

Verified 2026-02-08.
