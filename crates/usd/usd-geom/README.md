# UsdGeom Module - Geometry Schemas

Rust port of OpenUSD `pxr/usd/usdGeom`. Geometry primitives, transforms, and spatial queries.

## Parity Status: 100%

Every public C++ API has a Rust equivalent. Verified header-by-header against `_ref/OpenUSD/pxr/usd/usdGeom/*.h`.

Last verified: 2026-03-16 (deep parity check + integration tests).

---

### Geometry Primitives

| C++ Header | Rust File | Status |
|---|---|---|
| mesh.h | mesh.rs | 100% - ValidateTopology, SHARPNESS_INFINITE, GetFaceCount |
| basisCurves.h | basis_curves.rs | 100% |
| nurbsCurves.h | nurbs_curves.rs | 100% |
| nurbsPatch.h | nurbs_patch.rs | 100% |
| hermiteCurves.h | hermite_curves.rs | 100% |
| points.h | points.rs | 100% |
| sphere.h | sphere.rs | 100% + ComputeExtentAtTime |
| cube.h | cube.rs | 100% + ComputeExtentAtTime |
| cone.h | cone.rs | 100% + ComputeExtentAtTime |
| cylinder.h + cylinder_1.h | cylinder.rs | 100% - Cylinder + Cylinder1 |
| capsule.h + capsule_1.h | capsule.rs | 100% - Capsule + Capsule1 |
| plane.h | plane.rs | 100% + ComputeExtentAtTime |
| tetMesh.h | tet_mesh.rs | 100% |

### Base Classes

| C++ Header | Rust File | Status |
|---|---|---|
| gprim.h | gprim.rs | 100% |
| boundable.h | boundable.rs | 100% |
| pointBased.h | point_based.rs | 100% |
| curves.h | curves.rs | 100% |
| imageable.h | imageable.rs | 100% - PurposeInfo, visibility, proxy, bounds |
| scope.h | scope.rs | 100% |

### Transforms

| C++ Header | Rust File | Status |
|---|---|---|
| xform.h | xform.rs | 100% |
| xformable.h | xformable.rs | 100% - XformQuery, 80 methods |
| xformOp.h | xform_op.rs | 100% |
| xformCommonAPI.h | xform_common_api.rs | 100% |
| xformCache.h | xform_cache.rs | 100% |

### APIs

| C++ Header | Rust File | Status |
|---|---|---|
| primvarsAPI.h | primvars_api.rs | 100% |
| primvar.h | primvar.rs | 100% |
| modelAPI.h | model_api.rs | 100% |
| motionAPI.h | motion_api.rs | 100% |
| visibilityAPI.h | visibility_api.rs | 100% |

### Utilities

| C++ Header | Rust File | Status |
|---|---|---|
| bboxCache.h | bbox_cache.rs | 100% - includes PointInstance bounds |
| boundableComputeExtent.h | boundable_compute_extent.rs | 100% |
| constraintTarget.h | constraint_target.rs | 100% |
| metrics.h | metrics.rs | 100% - LinearUnits, upAxis, metersPerUnit |
| subset.h | subset.rs | 100% - Create/Get/Validate/Unassigned |
| pointInstancer.h | point_instancer.rs | 100% - activate/deactivate/vis/invis/compute |
| camera.h | camera.rs | 100% |
| samplingUtils.h | sampling_utils.rs | 100% |
| tokens.h | tokens.rs | 100% |

### Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | Rust `pub` visibility |
| debugCodes.h | Trivial debug flags |
| module.cpp / pch.h | Build infrastructure |
| wrap*.cpp | Python bindings |

---

## Tests

**79 tests total, 0 failures.**

### Unit tests (51) - `cargo test -p usd-geom --lib`

Inline `#[cfg(test)]` modules in capsule.rs, cylinder.rs, xformable.rs, etc.

### Integration tests (28) - `cargo test -p usd-geom --test geom_tests`

Ported from `_ref/OpenUSD/pxr/usd/usdGeom/testenv/`:

| Test | Source | Coverage |
|---|---|---|
| Mesh ValidateTopology | testUsdGeomMesh.py | 5 variants (mismatch, negative, OOR, valid, no-reason) |
| Mesh SHARPNESS_INFINITE | testUsdGeomConsts.py | value + is_sharpness_infinite |
| Mesh GetFaceCount | testUsdGeomMesh.py | 5 prims from mesh.usda (unset/blocked/empty/timeSampled/default) |
| ComputeExtent | testUsdGeomComputeAtTime.py | Cube, Sphere, Cone (Z/X/invalid), Cylinder, Capsule, Plane, Cylinder_1, Capsule_1 |
| Schema type names | testUsdGeomSchemata.py | 19 types including Capsule_1, Cylinder_1 |
| Schema attribute names | testUsdGeomSchemata.py | Cube, Mesh, PointInstancer |
| Metrics | testUsdGeomMetrics.py | LinearUnits constants, linear_units_are |
| Stage define | - | Scope, Cube, Mesh, Xform in-memory |
| Imageable purposes | testUsdGeomPurposeVisibility.py | ordered purpose tokens |
| Geom tokens | - | X/Y/Z, default/render/proxy/guide, etc. |

### Test data

Copied from reference (`_ref/OpenUSD/pxr/usd/usdGeom/`):
- `testenv/` - 30 .usda test files + Python/C++ test sources as reference
- `images/` - 5 documentation images (PNG/SVG)

---

## Summary

**UsdGeom: 100% API parity with OpenUSD C++ reference.**

39 Rust source files covering all 42 public C++ headers. 79 tests, 0 failures.

Verified 2026-03-16.
