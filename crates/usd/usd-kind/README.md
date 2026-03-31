# Kind Module - Model Kinds

Rust port of OpenUSD `pxr/usd/kind`. Model hierarchy classification system.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/kind/*.h` on 2026-02-08.

---

### Core Types

| C++ Header | Rust File | Status |
|---|---|---|
| registry.h | registry.rs | 100% — kind registry with inheritance |
| tokens.h | tokens.rs | 100% — model, group, assembly, component, subcomponent |

### Built-in Kinds

- `model` — base model kind
- `group` — container model
- `assembly` — assembled models
- `component` — leaf components
- `subcomponent` — sub-components

### Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp | Build infrastructure |

---

## Summary

**Kind module: 100% API parity with OpenUSD C++ reference.**

All 2 public C++ headers fully covered. 31 tests passing.

Verified 2026-02-08.
