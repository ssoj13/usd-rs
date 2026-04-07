# usd-rs

![usd-rs viewer](data/usdview.jpg)

A few points setting the right context:

  - A ground-up pure Rust rewrite of Pixar's [OpenUSD](https://github.com/PixarAnimationStudios/OpenUSD) — the industry standard for 3D scene interchange in VFX, animation, and real-time graphics.
  - This is not a binding layer.
  - Every line is Rust, targeting behavioral parity with the C++ reference: same composition semantics, same file format support (USDA/USDC/USDZ), same scene index architecture, same rendering pipeline concepts.
  - This repo is large and still under active work. Sudden changes and API rewrites are to expect at any moment.
  - I built this for myself as a learning exercise and a practical tool. It's public in case it's useful to someone else. Clone it, use it, drop me a line if you want.
  - This project actively uses non-traditional development patterns, AI and agentic development.
  - Personally I see a huge potential in Rust coupled with AI tool to quickly develop monolithic apps with required functionality.
  - Range of applications for this library is much larger than "just VFX": it can be anything of
    - compact digital asset tools both for VFX / AI / robotics / cross-applications
    - cross-platform GIS: real-time apps (maps), navigation apps
    - AI/robotic applications: on-board simulations
    - IoT applications (potentially Jetson or kind, that's still something to test)


## What's here

~1.1M lines of Rust across 2400+ files in 72 crates:

| Layer | Crates | What it does |
|-------|--------|-------------|
| **Base** (9) | usd-tf, usd-gf, usd-vt, usd-plug, usd-trace, usd-ts, usd-work, usd-arch, usd-js | Foundation: tokens, math (glam-backed), value types, plugin system, tracing |
| **USD Core** (6) | usd-sdf, usd-ar, usd-pcp, usd-core, usd-kind, usd-sdr | Scene description, asset resolution, composition engine (LIVRPS), stage API |
| **Schemas** (16) | usd-geom, usd-shade, usd-lux, usd-skel, usd-vol, usd-physics, ... | Geometry, materials, lights, skeletal animation, volumes, physics |
| **Imaging** (22) | usd-hd, usd-hdx, usd-hd-st, usd-hgi, usd-hgi-wgpu, usd-hdsi, ... | Hydra scene index chain, Storm renderer, wgpu GPU backend |
| **Viewer** | usd-view | egui-based viewer (usdview equivalent) with dockable panels |
| **Python** | usd-pyo3 | PyO3 bindings: `import pxr_rs as pxr` — drop-in for Pixar's `pxr` |
| **External** (6) | opensubdiv-rs, mtlx-rs, osl-rs, draco-rs, gltf-rs, pxr-lz4 | Independent Rust ports of OpenSubdiv 3.7, MaterialX, Open Shading Language, Draco mesh compression, and LZ4 decompression. gltf-rs is a fork of the [gltf](https://crates.io/crates/gltf) crate (MIT/Apache-2.0). |

## What works

**File I/O** — USDA parser/writer, USDC binary reader/writer, USDZ package handling, Alembic reader. Open, compose, traverse, export, and round-trip USD files.

**Composition** — Full LIVRPS arcs (Local, Inherits, VariantSets, References, Payloads, Specializes). PcpCache, PrimIndex, layer stack composition, sublayers, layer offsets, variant selections, parallel composition via rayon.

**Stage API** — Stage.Open, Traverse, DefinePrim, attribute authoring, time samples, instancing (native + point instancer), edit targets, population masks, load rules.

**Schemas** — All USD schema domains are ported:
- UsdGeom (Mesh, BasisCurves, Points, NurbsPatch, Xformable, Camera, PointInstancer, BBoxCache, XformCache, Primvar, Subset, MotionAPI)
- UsdShade (Material, Shader, NodeGraph, ConnectableAPI, MaterialBindingAPI, CoordSysAPI)
- UsdLux (all light types, LightListAPI, ShapingAPI, ShadowAPI, MeshLightAPI, VolumeLightAPI)
- UsdSkel (Skeleton, Animation, BindingAPI, BlendShape, Cache, SkinningQuery)
- UsdVol (Volume, VolumeFieldBase, OpenVDBAsset, Field3DAsset)
- UsdPhysics (RigidBodyAPI, CollisionAPI, Joint types, MassAPI, MaterialAPI, Scene)
- UsdRender (Settings, Product, Pass, Var)
- UsdUI (NodeGraphNodeAPI, SceneGraphPrimAPI, Backdrop, AccessibilityAPI)
- UsdMedia (SpatialAudio, AssetPreviewsAPI)
- UsdProc (GenerativeProcedural)
- UsdRi (MaterialAPI, SplineAPI, StatementsAPI)
- UsdSemantics (LabelsAPI, LabelsQuery)
- UsdHydra (GenerativeProceduralAPI)

**Hydra / Rendering** — Scene index chain (flattening, material binding, visibility, instancing), Storm-equivalent renderer via wgpu (DX12/Vulkan/Metal), mesh sync, draw batching, implicit surface synthesis, subdivision surfaces via OpenSubdiv port.

**Validation** — USD scene validation framework (usd-validation crate).

**Viewer** — egui-based GUI with 3D viewport, prim tree, attribute inspector, layer stack, composition arcs, playback, selection, hot-reload, persistence.

**Python bindings** — 16 modules (Tf, Gf, Vt, Sdf, Pcp, Ar, Kind, Usd, UsdGeom, UsdShade, UsdLux, UsdSkel, Plug, Ts, Work, Cli). PyO3 0.28, Python 3.11–3.14.

**CLI tools** — `usd cat/tree/dump/diff/resolve/edit/stitch/zip/view/meshdump/dumpcrate` — all as a single `usd` binary, also callable from Python via `pxr_rs.Cli`.

**mtlx-rs** — MaterialX port (55k LOC, 800+ tests): core document model, node graphs, shader generation backends for GLSL, WGSL (via naga), MSL, MDL, OSL, Slang.

**osl-rs** — Open Shading Language port (78k LOC, 78 tests): lexer, parser, AST, type system, codegen, builtins, closures, BSDF models.

**opensubdiv-rs** — OpenSubdiv 3.7 port (40k LOC): Catmull-Clark, Loop, Bilinear subdivision. Far topology refiner, patch tables, stencil tables.

**draco-rs** — Draco mesh compression port (76k LOC): encoder/decoder for meshes, point clouds, mesh features.

## Work in progress

- **Hydra render delegate** — basic Storm pipeline works, but material networks, AOVs, and advanced shading are incomplete
- **Python API parity** — bindings exist for all major modules but many methods are stubs. Reference test suite (465 tests from OpenUSD) shows ~80 passing, rest need API completion

## Architecture differences from C++

- **glam** for math instead of GfVec/GfMatrix — zero-cost SIMD, same API surface
- **wgpu** instead of OpenGL/Vulkan — cross-platform GPU abstraction (DX12, Vulkan, Metal)
- **egui** instead of Qt — immediate-mode UI, no C++ dependencies
- **Token interning** via lock-free concurrent hash map, not TBB
- **No TfType plugin system** — schema registration is compile-time, not dlopen-based
- **No Boost** — obviously

## Python bindings

```python
import pxr_rs as pxr
from pxr_rs import Tf, Gf, Sdf, Usd, UsdGeom

stage = pxr.Usd.Stage.Open("scene.usda")
for prim in stage.Traverse():
    print(prim.GetPath())

v = Gf.Vec3f(1, 2, 3)
print(v.GetLength(), v.GetNormalized())
```

Build: `python bootstrap.py b p` (requires [maturin](https://www.maturin.rs/) + Rust nightly)

## CLI

```bash
usd cat scene.usda              # print USDA text
usd tree scene.usda             # prim hierarchy
usd view scene.usda             # launch viewer
usd diff a.usda b.usda          # diff two files
usd dump scene.usdc             # layer statistics
usd zip model.usda model.usdz   # create USDZ package
```

## Building

Requires Rust nightly (MSRV 1.85), Python 3.11+ for bindings.

```bash
cargo build --release           # build everything
cargo test --workspace          # run tests
python bootstrap.py b           # same via bootstrap
python bootstrap.py b p         # build Python wheel
python bootstrap.py ch          # clippy + fmt
```

## Known issues

- Layer cache is global — mutations via `Stage::DefinePrim` on one stage can be visible to other stages sharing the same root layer through `Layer::FindOrOpen`. This matches C++ behavior but tests must account for it.
- `is_main_thread()` is unreliable in cargo test (test harness uses worker threads)
- draco-rs has 69 pre-existing test failures (upstream port incomplete)
- Some test fixtures require Git LFS

## License

MIT OR Apache-2.0

gltf-rs is a fork of [gltf](https://crates.io/crates/gltf) (MIT/Apache-2.0). All other code, including the ports of OpenSubdiv, MaterialX, OSL, and Draco, is original work.
