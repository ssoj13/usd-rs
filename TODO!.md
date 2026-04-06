# Python API — Session Summary

## DONE

### Phase 1-3: API Inventory + Verification
- 482/482 wrap files documented across 39 modules
- 10 research agents + 6 verification agents + 1 final cross-check
- Results in md/pyapi/*.md (10 files, 1234 lines)

### Phase 4: usd-pyo3 Crate
- 13 Python modules: Tf, Gf, Vt, Sdf, Pcp, Ar, Kind, Usd, UsdGeom, UsdShade, UsdLux, UsdSkel, Cli
- 15,687 lines Rust across 18 files
- PyO3 0.28 (Python 3.13/3.14 support)
- Package: `pxr-rs` → `import pxr_rs as pxr`
- 0 errors, 0 warnings
- bootstrap.py: `b p` builds wheel via maturin

### Phase 5: Infrastructure
- bootstrap.py — unified build script (b, b p, t, ch, clean)
- CI: .github/workflows/ci.yml updated with setup-python 3.13
- Standalone usdview binary removed → `usd view` only entry point
- README.md, STRUCTURE.md, CLAUDE.md, DIAGRAMS.md updated

## WAITING
- CI run #69 on GitHub (PyO3 0.28 + setup-python fix)

## NEXT
- Fill remaining schema module methods (UsdVol, UsdPhysics, UsdRender, UsdUI, etc.)
- Migrate vt.rs from into_py → into_pyobject (remove #![allow(deprecated)] properly)
- Quatd constructor: accept Vec3d as imaginary
- VtValue alias fix (Vt.Value = Vt._ValueWrapper)
- Add numpy buffer protocol support to VtArray types
- Integration tests with real USD files
