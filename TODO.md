# usd-rs PyO3 Python API Parity — Task Snapshot

**Date:** 2026-04-07
**Goal:** Full parity with C++ OpenUSD Python API (`pxr`). Package: `pxr_rs` (`import pxr_rs as pxr`).
**Ref tests:** `crates/usd-pyo3/tests/` — ported from `C:\projects\projects.rust.cg\usd-refs\OpenUSD`

**Parity memory (checklist + changelog):** `md/PYTHON_API_PARITY.md`  
**Полный реестр отклонений от OpenUSD (что ещё чинить):** `md/PYTHON_API_DEVIATIONS.md`

---

## Current Test Counts

| Module | Passed | Failed | Errors | Total | % |
|--------|--------|--------|--------|-------|---|
| base/gf | 126 | 21 | 0 | 147 | 86% |
| base/vt | 24 | 4 | 0 | 28 | 86% |
| base/tf | 3 | ~41 | 0 | 44 | 7% |
| usd/ar | 36 | 16 | 0 | 52 | 69% |
| usd/sdf | 38 | 157 | 1 | 196 | 19% |
| usd/usd | ~36 | ~430 | ~61 | 527 | 7% |
| usd/usdGeom | 8 | 196 | 2 | 206 | 4% |
| usd/usdSkel | 4 | 29 | 0 | 33 | 12% |
| **TOTAL** | **283** | **887** | **61** | ~1233 | **23%** |

Run command: `python -m pytest crates/usd-pyo3/tests/base/gf crates/usd-pyo3/tests/base/vt crates/usd-pyo3/tests/base/tf crates/usd-pyo3/tests/usd/sdf crates/usd-pyo3/tests/usd/usd crates/usd-pyo3/tests/usd/ar crates/usd-pyo3/tests/usd/usdGeom crates/usd-pyo3/tests/usd/usdSkel --tb=no -q`

Build: `python -m maturin develop -m crates/usd-pyo3/Cargo.toml --release`
Check: `cargo check -p usd-pyo3` — 0 errors, 0 warnings

---

## Source Files

| File | LOC | Role |
|------|-----|------|
| `crates/usd-pyo3/src/lib.rs` | ~80 | Root module, registers 16 submodules |
| `crates/usd-pyo3/src/usd.rs` | ~4600 | Stage, Prim, Attribute, Relationship, VariantSets, References, Payloads, etc. |
| `crates/usd-pyo3/src/sdf.rs` | ~3800 | Path, Layer, PrimSpec, AttributeSpec, ListOps, ValueTypeNames, etc. |
| `crates/usd-pyo3/src/geom.rs` | ~2000 | 39 UsdGeom schemas, BBoxCache, XformCache, Primvar, XformOp, Metrics |
| `crates/usd-pyo3/src/gf/` | ~3000 | Vec, Matrix, Quat, BBox3d, Frustum, Camera, Rotation, etc. |
| `crates/usd-pyo3/src/vt.rs` | ~1400 | VtValue, 31 VtArray types, VtDictionary |
| `crates/usd-pyo3/src/tf.rs` | ~900 | Token, Type, Notice, Stopwatch, Debug |
| `crates/usd-pyo3/src/plug.rs` | ~120 | Registry, _TestPlugBase1..4, Plugin |
| `crates/usd-pyo3/src/ar.rs` | ~200 | Resolver, ResolvedPath |
| `crates/usd-pyo3/src/shade.rs` | ~400 | Material, Shader, ConnectableAPI |
| `crates/usd-pyo3/src/skel.rs` | ~200 | Skeleton, SkelRoot, SkelAnimation |
| `crates/usd-pyo3/tests/conftest.py` | ~90 | Auto-chdir, skip list, tmpdir fixture |

---

## What Was Done (This Session)

### geom.rs — Complete CamelCase + Type Unification
- ALL ~300+ methods annotated with `#[pyo3(name = "CamelCase")]`
- Removed duplicate PyStage/PyPrim/PyAttribute definitions — now uses `crate::usd::*`
- Added `PyPrim::from_prim_auto()` and `PyAttribute::from_attr()` constructors (OnceLock dummy stage)
- `extract_prim()` — accepts both PyPrim and schema wrappers (Mesh, Xform etc.) via GetPrim() duck typing
- `check_xform_op()` — raises RuntimeError on duplicate ops (C++ TF_CODING_ERROR parity)
- XformOp.Set/Get — delegates to Attribute.Set/Get via Python bridge
- Xform delegates all Xformable + Imageable methods (AddTranslateOp, etc.)
- PointInstancer.GetPath added
- XformCommonAPI rotation order classattrs
- PrimvarsAPI.CreatePrimvar signature fix (optional interpolation/element_size)
- Mesh.GetFaceCount, GetPath added; removed _cc duplicate methods
- Metrics functions CamelCase (GetStageUpAxis etc.)
- Deleted duplicate `_cc` methods in Mesh that conflicted with real CamelCase methods

### usd.rs Fixes
- `Stage.DefinePrim` accepts `typeName` keyword arg
- `_instance_name` unused var warnings fixed
- `from_prim_auto` / `from_attr` use shared OnceLock dummy stage (no panic)

### sdf.rs Fixes
- `ApplyRootPrimOrder` — fixed mutability (`&mut Vec<Token>`)
- `UpdateCompositionAssetDependency` — fixed `Option<&str>` arg

### tf.rs Additions
- `Tf.Type.Define(py_class)` — stub for plugin cross-language inheritance
- `Tf.Type.Find(name_or_class)` — alias for FindByName

### plug.rs Additions
- `_TestPlugBase1..4` — subclassable pyclass stubs for Plug test infrastructure
- `Plugin` class with GetName

### conftest.py
- Added `**/plug/TestPlug*__init__.py` to skip (fixture modules, not tests)
- Added `**/trace/**` to skip (module not ported)
- Renamed `TestSdfAttributeBase` → `_SdfAttributeBase` (pytest was collecting abstract base)

---

## What Needs To Be Done

### HIGH IMPACT (by test count)

#### UsdGeom (196 failed → target 150)
- [ ] `ComputeInstanceTransformsAtTimes` (plural, batch version) — 16 tests
- [ ] `PointBased.ComputePointsAtTime/AtTimes` — 14 tests
- [ ] `Gf.IsClose` support for more types — 13 tests
- [ ] `BBoxCache.__new__` accept `includedPurposes` kwarg — 7 tests
- [ ] `CreatePrimvar` accept ValueTypeName object (not just str) — 5 tests
- [ ] `Boundable.ComputeExtentFromPlugins` — 4 tests
- [ ] `HermiteCurves.PointAndTangentArrays` — 3 tests
- [ ] Schema inheritance: all concrete types (Mesh, Camera, etc.) need Xformable/Imageable delegate methods

#### usd/usd (430 failed → target 300)
- [ ] Many tests use script-style (not unittest) — need adaptation or conftest fixtures
- [ ] `Attribute.Set/Get` value conversion for more types (timeSamples, metadata) — 35+ tests
- [ ] Accept `Sdf.Path` objects where `str` is expected — 6 tests
- [ ] `Sdf.PathListOp.CreateExplicit` — 6 tests
- [ ] `Stage.OpenMasked` accept Layer argument — 5 tests
- [ ] `Layer.relocates` property — 5 tests
- [ ] `Sdf.FileFormat` class — 12 tests
- [ ] `Sdf.CopySpec` function — 5 tests
- [ ] `SchemaRegistry.GetTypeFromSchemaTypeName` — 3 tests

#### usd/sdf (157 failed → target 100)
- [ ] `Sdf.FileFormat` — 12 tests
- [ ] `tuple.errors` → need named result type — 9 tests
- [ ] `Sdf.CopySpec` — 5 tests
- [ ] `Sdf.ZipFileWriter` — 4 tests
- [ ] `VariableExpression.MakeLiteral/MakeVariable` — 5 tests
- [ ] `Layer.SetDetachedLayerRules` — 3 tests
- [ ] `BatchNamespaceEdit` — 3 tests
- [ ] `PathListOp.explicitItems` writable — 3 tests

#### base/tf (41 failed → target 20)
- [ ] Missing Notice subclasses — many tests depend on TfNotice hierarchy
- [ ] Token hash/comparison operators
- [ ] Type hierarchy traversal

### MEDIUM IMPACT
- [ ] UsdShade: testUsdShadeShaderDef collection error (missing Sdr module)
- [ ] UsdLux: testUsdLuxLight collection error (missing imports)
- [ ] Not-yet-ported modules: UsdUtils, UsdVol, UsdPhysics, usdImaging, usdValidation, Trace, Ndr, Sdr

### ARCHITECTURE NOTES
- PyStage/PyPrim/PyAttribute defined in `usd.rs`, other modules import via `crate::usd::*`
- `from_prim_auto()` creates a dummy stage — works but incorrect for stage-dependent operations
  - Better fix: store `Arc<Stage>` in Rust schema types, or extract from Prim's internal data
- `extract_prim()` uses `call_method0("GetPrim")` — works but overhead per call
  - Better fix: implement `FromPyObject` trait for Prim that handles schema wrappers
- Schema inheritance not modeled in PyO3 — each concrete type must manually delegate parent methods
  - All geom types need Imageable methods (ComputeVisibility, MakeVisible, etc.)
  - All xformable types need Xformable methods (AddTranslateOp, etc.)
  - Currently only Xform has these delegated; need for Mesh, Sphere, Camera, etc.

### RULES (from operator)
- **NEVER** run tests — operator runs them. Write code, `cargo check` only.
- **NEVER** use sed — use Edit tool or bulk_edits
- **NEVER** launch long-running agents
- **NEVER** compare Rust vs C++ by LOC
- **ALWAYS** port from C++ reference first — it's the whole project mandate
- Python with `python` (not `py`) — `py` resolves to wrong interpreter
- Build: `python -m maturin develop -m crates/usd-pyo3/Cargo.toml --release`
- C++ reference at: `C:\projects\projects.rust.cg\usd-refs\OpenUSD`
