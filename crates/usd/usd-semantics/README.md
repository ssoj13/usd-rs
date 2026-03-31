# UsdSemantics Module - Semantic Labels

Rust port of OpenUSD `pxr/usd/usdSemantics`. Semantic labeling for scene elements.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdSemantics/*.h` on 2026-02-08.

---

| C++ Header | Rust File | Status |
|---|---|---|
| labelsAPI.h | labels_api.rs | 100% |
| labelsQuery.h | labels_query.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |

---

## Summary

**UsdSemantics module: 100% API parity with OpenUSD C++ reference.**

All 3 public C++ headers fully covered. 4 Rust source files. 0 API gaps.

Verified 2026-02-08.
