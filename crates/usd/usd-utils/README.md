# UsdUtils Module - USD Utilities

Rust port of OpenUSD `pxr/usd/usdUtils`. Authoring, localization, and pipeline utilities.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdUtils/*.h` on 2026-02-08.

---

### Asset Localization

| C++ Header | Rust File | Status |
|---|---|---|
| assetLocalization.h | asset_localization.rs | 100% |
| assetLocalizationDelegate.h | asset_localization_delegate.rs | 100% |
| assetLocalizationPackage.h | asset_localization_package.rs | 100% |
| localizeAsset.h | localize_asset.rs | 100% |
| usdzPackage.h | usdz_package.rs | 100% |

### Layer Operations

| C++ Header | Rust File | Status |
|---|---|---|
| flattenLayerStack.h | flatten_layer_stack.rs | 100% |
| stitch.h | stitch.rs | 100% |
| stitchClips.h | stitch_clips.rs | 100% |
| dependencies.h | dependencies.rs | 100% |

### Authoring & Pipeline

| C++ Header | Rust File | Status |
|---|---|---|
| authoring.h | authoring.rs | 100% |
| pipeline.h | pipeline.rs | 100% |
| registeredVariantSet.h | registered_variant_set.rs | 100% |
| sparseValueWriter.h | sparse_value_writer.rs | 100% |
| userProcessingFunc.h | user_processing_func.rs | 100% |

### Cache & Diagnostics

| C++ Header | Rust File | Status |
|---|---|---|
| stageCache.h | stage_cache.rs | 100% |
| coalescingDiagnosticDelegate.h | coalescing_diagnostic_delegate.rs | 100% |
| conditionalAbortDiagnosticDelegate.h | conditional_abort_diagnostic_delegate.rs | 100% |
| introspection.h | introspection.rs | 100% |
| timeCodeRange.h | time_code_range.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| debugCodes.h | Trivial debug flags |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdUtils module: 100% API parity with OpenUSD C++ reference.**

All 20 public C++ headers fully covered. 21 Rust source files. 0 API gaps.

Verified 2026-02-08.
