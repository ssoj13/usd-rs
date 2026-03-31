# UsdMedia Module - Media Schemas

Rust port of OpenUSD `pxr/usd/usdMedia`. Asset previews and spatial audio.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdMedia/*.h` on 2026-02-08.

---

| C++ Header | Rust File | Status |
|---|---|---|
| assetPreviewsAPI.h | asset_previews_api.rs | 100% |
| spatialAudio.h | spatial_audio.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |

---

## Summary

**UsdMedia module: 100% API parity with OpenUSD C++ reference.**

All 3 public C++ headers fully covered. 4 Rust source files. 0 API gaps.

Verified 2026-02-08.
