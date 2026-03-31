# JS Module - JSON Support

Rust port of OpenUSD `pxr/base/js`. JSON parsing and serialization using serde_json backend.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/base/js/*.h`.

---

### Core Types

| C++ Header | Rust File | Status |
|---|---|---|
| types.h | value.rs | JsObject, JsArray |
| value.h | value.rs | 100% — Get*, Is*, GetArrayOf, IsArrayOf |
| utils.h | utils.rs | 100% — JsFindValue → find_value |
| json.h | mod.rs | 100% — parse/write, JsWriter, JsParseError |
| converter.h | converter.rs | 100% — JsValueTypeConverter, JsConvertToContainerType |
| N/A | error.rs | JsParseError |

### Intentional Differences

| C++ | Rust |
|-----|------|
| Parse returns null on failure | `Result<JsValue, JsParseError>` |
| `JsFindValue` empty key: TF_CODING_ERROR + nullopt | `find_value` empty key: returns default (no error log) |
| `JsOptionalValue` typedef | `pub type JsOptionalValue = Option<JsValue>` |

### GetArrayOf Support

C++ `GetArrayOf<std::string>()` and similar require `T: From<JsValue>`. Implemented for: `String`, `i64`, `u64`, `f64`, `bool`, `i32`, `JsObject`, `JsArray`.

### Backend

Uses **serde_json** (replaces C++ vendored RapidJSON):
- Parse JSON strings
- Serialize to JSON
- JSON value manipulation
- Pretty printing

### Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| module.cpp / pch.h | Build infrastructure |
| rapidjson/* | Replaced by serde_json |

---

## Summary

**JS module: 100% API parity with OpenUSD C++ reference.**

All 5 public C++ headers (types, value, utils, json, converter) fully covered. 5 Rust source files. 0 API gaps.

Verified 2026-02-15.
