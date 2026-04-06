# Python API — Current Status

## PyO3 Crate: crates/usd-pyo3
- 16 modules: Tf, Gf, Vt, Sdf, Pcp, Ar, Kind, Plug, Ts, Work, Usd, UsdGeom, UsdShade, UsdLux, UsdSkel, Cli
- ~16k lines Rust, PyO3 0.28, Python 3.11-3.14
- Package: `pxr-rs` → `import pxr_rs as pxr`
- 465 Python tests copied from OpenUSD reference (imports fixed to pxr_rs)
- sys.modules registration for submodule imports

## Python Test Status (local)
| Module | Pass | Fail | Error | Notes |
|--------|------|------|-------|-------|
| base/gf | 6 | 141 | 0 | Copy constructors, tuple→Vec, hash, math functions |
| base/vt | 2 | 26 | 0 | Array ops, Value conversion |
| base/tf | 0 | 0 | 12 | Missing test-only C++ classes |
| usd/sdf | 0 | 0 | 8 | Missing classes in bindings |
| usd/usd | 0 | 0 | 26 | Missing classes in bindings |
| usd/ar | 0 | 0 | 4 | Missing attributes |
| usd/kind | 0 | 0 | 1 | Collection error |
| usd/pcp | 0 | 0 | 4 | Collection errors |

## Rust Crate Bugs Found (NOT pyo3 — core crates)
1. **LightAPI::get_inputs(onlyAuthored=false)** returns empty — built-in inputs not exposed
2. **Extent computation** returns MAX_FLOAT for lights — ComputeExtentFromPlugins broken
3. **SDR shader node registration** for lights completely non-functional
4. **Render context shader ID attrs** — created when they shouldn't exist
5. **Layer cache pollution** — define_prim mutations visible across Stages sharing layers

## Agents Running
- fix-gf-tests — Gf PyO3 failures
- fix-sdf-usd-tests — Sdf/Usd collection errors
- fix-vt-tests — Vt failures
- fix-tf-ar-kind-pcp — Tf/Ar/Kind/Pcp errors
- verify-lux-tests — DONE, found 4 failing tests (core bugs, not pyo3)

## CI
- macOS: PASSES
- Ubuntu: LightListAPI test fixed, cache removed
- Windows: cache step removed (was hanging on tar)
