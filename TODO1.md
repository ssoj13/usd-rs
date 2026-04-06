# Python API Full Inventory

## Phase 1: Data Collection — COMPLETE
All 10 research agents finished. Results saved to md/pyapi/:
- [x] md/pyapi/tf.md — base/tf (35 wraps)
- [x] md/pyapi/gf.md — base/gf (60 wraps)
- [x] md/pyapi/vt.md — base/vt (14 wraps)
- [x] md/pyapi/sdf.md — usd/sdf (38 wraps)
- [x] md/pyapi/usd.md — usd/usd (37+ wraps)
- [x] md/pyapi/pcp_ar_sdr.md — pcp (19) + ar (12) + sdr (14) wraps
- [x] md/pyapi/geom.md — usdGeom (38 wraps)
- [x] md/pyapi/shade_lux_skel.md — usdShade (15) + usdLux (22) + usdSkel (17) wraps
- [x] md/pyapi/remaining_schemas.md — usdVol/Physics/Render/UI/Media/Proc/Ri/Semantics/Hydra/Mtlx/Utils (89 wraps)
- [x] md/pyapi/base_imaging.md — trace/plug/work/ts + imaging + usdImaging + validation + plugins (55 wraps)

## Phase 2: Verification — COMPLETE
6 verification agents checking every claim against real code:
- [x] verify-tf — 100% correct
- [x] verify-sdf — CRITICAL: Layer/Path/PrimSpec/Spec truncated → complete-sdf agent running
- [x] verify-usd — 100% correct, all 45 files verified
- [x] verify-gf-vt — 1 DualQuat fix applied. Vt 100% correct
- [x] verify-pcp-ar-geom — AR/SDR counts fixed, PointInstancer 17 methods added
- [x] verify-shade-rest — Systematic: schema wraps missing Create*/Get* attr accessors (60-70% method coverage)

### Known systematic gap:
Generated schema classes (UsdGeom*, UsdLux*, UsdVol*, UsdPhysics*, etc.) all follow same pattern:
- Each attribute has Get*Attr() + Create*Attr(defaultValue, writeSparsely)
- Each relationship has Get*Rel() + Create*Rel()
- Reports list attributes but often omit the Get/Create method pairs
- NOT a real blocker for PyO3 — these are mechanical and can be derived from schema definitions

## Phase 3: Corrections — COMPLETE
- gf.md: DualQuat methods fixed (ExtractTranslation→GetTranslation, +GetConjugate, +Transform)
- sdf.md: Layer/Path/PrimSpec/Spec complete data appended (385+ bindings)
- geom.md: PointInstancer 17 custom methods + 2 enums added
- pcp_ar_sdr.md: AR count 9→12, SDR count 13→14 corrected

## Phase 4: PyO3 Crate Creation — DONE (skeleton)
- [x] crates/usd-pyo3/Cargo.toml — cdylib, all USD crates as deps
- [x] crates/usd-pyo3/pyproject.toml — maturin config, module-name = pxr._usd
- [x] crates/usd-pyo3/pxr/__init__.py — re-exports Tf, Gf, Vt, Sdf, Usd
- [x] src/lib.rs — root module with submodule registration
- [x] src/tf.rs — Token (full API: new, str, repr, hash, eq, ne, bool, len)
- [x] src/gf.rs — Vec3f, Vec3d, Matrix4d (arithmetic, indexing, methods)
- [x] src/vt.rs — stub
- [x] src/sdf.rs — stub
- [x] src/usd.rs — stub
- [x] Added to workspace Cargo.toml
- [x] bootstrap.py b p — builds wheel via maturin
- [x] pip install + `from pxr import Tf, Gf` — TESTED OK
- [x] bootstrap.py encoding fix (utf-8)

## Phase 5: Fill remaining modules — TODO
Priority order (by Python user frequency):
1. Sdf: Path, Layer, PrimSpec, AttributeSpec, Reference, Payload, LayerOffset, TimeCode, ValueTypeName
2. Usd: Stage, Prim, Attribute, Property, Relationship, TimeCode, EditTarget, SchemaBase
3. Gf: remaining Vec2/4, Matrix2/3, Quat, Range, BBox3d, Rotation, Transform, Camera, Frustum
4. Vt: VtValue, VtArray<T>, VtDictionary
5. UsdGeom: Mesh, Xformable, Imageable, BBoxCache, Primvar, PointInstancer, Camera
6. UsdShade: Material, Shader, Input, Output, MaterialBindingAPI
7. Remaining schemas
