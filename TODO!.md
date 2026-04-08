# Python API Parity — Current State (2026-04-07)

## Test counts:
- **Total**: 283 passed / 901 failed / 61 errors (core modules)
- **UsdGeom**: 8 passed / 196 failed / 2 errors (was 0/206)
- **base/gf**: 126/147 (86%)
- **base/vt**: 24/28 (86%)
- **usd/ar**: 36/52 (69%)

## What was done this session:
1. **geom.rs complete CamelCase** — ALL methods annotated with `#[pyo3(name = "CamelCase")]`
2. **Type unification** — removed duplicate PyStage/PyPrim/PyAttribute from geom.rs, use crate::usd::*
3. **extract_prim()** — schema wrappers (Mesh, Xform...) accepted as Prim via GetPrim() duck typing
4. **check_xform_op()** — returns RuntimeError when adding duplicate xform ops (C++ TF_CODING_ERROR parity)
5. **XformOp.Set/Get** — delegates to Attribute.Set/Get via Python bridge
6. **Xform xformable methods** — AddTranslateOp, AddRotateXYZ, AddScale, GetOrderedXformOps, etc. delegated
7. **PointInstancer.GetPath** — added
8. **XformCommonAPI.RotationOrderXYZ** etc. — classattr constants
9. **Tf.Type.Define/Find** — stubs for plugin test infrastructure
10. **Plug._TestPlugBase1..4** — for cross-language inheritance tests
11. **PrimvarsAPI.CreatePrimvar** — optional args signature fix
12. **Stage.DefinePrim** — accepts `typeName` keyword arg
13. **sdf.rs** — fixed ApplyRootPrimOrder mutability and UpdateCompositionAssetDependency Option
14. **Metrics functions** — CamelCase (GetStageUpAxis etc.)
15. **Dummy stage OnceLock** — shared instance instead of per-call creation

## Top remaining failure patterns (UsdGeom):
- 22: XformOp.Set → FIXED but tests expect specific value formats
- 16: ComputeInstanceTransformsAtTimes (plural) — missing
- 15: NotImplementedError stubs
- 13: Gf.IsClose unsupported types
- 7: BBoxCache kwarg includedPurposes
- 7: PointBased.ComputePointsAtTime — missing
- 5: CreatePrimvar type_name_str receives ValueTypeName object

## Top remaining failure patterns (usd/usd):
- 49: "Failed to load expected test plugin" — schema registration tests
- 35: "0 != 1" — attribute set/get value issues
- 6: Path object not accepted as str
- 6: PathListOp.CreateExplicit missing
- 5: Layer.relocates missing
