# UsdSkel Module - Skeletal Animation

Rust port of OpenUSD `pxr/usd/usdSkel`. Skeletal animation, skinning, and blend shapes.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdSkel/*.h` on 2026-02-08.

---

### Schema Types

| C++ Header | Rust File | Status |
|---|---|---|
| skeleton.h | skeleton.rs | 100% |
| animation.h | animation.rs | 100% |
| root.h | root.rs | 100% |
| blendShape.h | blend_shape.rs | 100% |
| inbetweenShape.h | inbetween_shape.rs | 100% |
| bindingAPI.h | binding_api.rs | 100% |

### Query System

| C++ Header | Rust File | Status |
|---|---|---|
| skeletonQuery.h | skeleton_query.rs | 100% |
| animQuery.h | anim_query.rs | 100% |
| skinningQuery.h | skinning_query.rs | 100% |
| blendShapeQuery.h | blend_shape_query.rs | 100% |
| cache.h | cache.rs | 100% |

### Utilities

| C++ Header | Rust File | Status |
|---|---|---|
| animMapper.h | anim_mapper.rs | 100% |
| binding.h | binding.rs | 100% |
| topology.h | topology.rs | 100% |
| skelDefinition.h | skel_definition.rs | 100% |
| bakeSkinning.h | bake_skinning.rs | 100% |
| utils.h | utils.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| animQueryImpl.h / cacheImpl.h | Internal implementation |
| debugCodes.h | Trivial |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Summary

**UsdSkel module: 100% API parity with OpenUSD C++ reference.**

All 18 public C++ headers fully covered. 19 Rust source files. 0 API gaps.

Verified 2026-02-08.
