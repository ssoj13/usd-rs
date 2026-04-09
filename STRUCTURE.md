# USD-RS Project Structure

> **FOR AGENTS**: If you discover new architectural details, key APIs, patterns, or
> gotchas while working on this project — **ADD THEM HERE**. This file is the shared
> knowledge base for all agents. Keep it factual, machine-readable, and up to date.
> Delete stale info rather than leaving it to mislead others.

## Project Overview

Pure Rust port of Pixar's OpenUSD. Not bindings — a ground-up rewrite preserving
the same architecture, APIs, and behavior as the C++ reference at `_ref/OpenUSD/`.

- **Platform**: Windows 11, MSYS2/bash shell
- **Renderer**: wgpu (not OpenGL/Vulkan directly)
- **UI**: egui (not Qt)
- **Build**: Cargo workspace (71 members), vcpkg at `c:/vcpkg`
- **Python**: PyO3/maturin bindings (`pxr` package)
- **Build tool**: `bootstrap.py` (build, test, check, Python bindings)
- **Reference**: `_ref/OpenUSD/pxr/` — always consult before implementing
- **Stats**: ~2480 `.rs` files, ~130k lines of Rust (crates + src + vendor)

## Repository Layout

```
usd-rs/
  _ref/OpenUSD/pxr/         # C++ reference (read-only, DO NOT MODIFY)
    base/                    # arch, gf, js, plug, tf, trace, ts, vt, work
    usd/                     # ar, kind, sdf, pcp, sdr, usd, usdGeom, usdShade, ...
    imaging/                 # hd, hgi, hdSt, glf, hdx, ...
    usdImaging/              # usdImaging (scene delegate)
  crates/
    base/                    # Foundation crates (no USD dependencies)
      usd-arch/              # Platform abstractions (C++ pxr/base/arch)
      usd-gf/                # Graphics foundations: Vec, Matrix, Quat, BBox, Ray, etc.
      usd-js/                # JSON value system (JsValue, JsObject)
      usd-tf/                # Type system, tokens, diagnostics, notices
      usd-plug/              # Plugin registry, plugInfo.json discovery
      usd-trace/             # Performance tracing
      usd-ts/                # Time splines
      usd-vt/                # Value types (VtValue, VtArray, VtDictionary)
      usd-work/              # Task parallelism (thread pool, dispatcher)
    usd/                     # USD core + schema crates
      usd-ar/                # Asset resolution
      usd-kind/              # Kind registry
      usd-sdf/               # Scene description foundations (layers, specs, paths)
      usd-pcp/               # Prim cache population (composition)
      usd-sdr/               # Shader definition registry
      usd-core/              # USD core (stage, prim, attribute, queries)
      usd-geom/              # Geometry schemas
      usd-shade/             # Shading schemas
      usd-lux/               # Lighting schemas
      usd-skel/              # Skeleton/skinning schemas
      usd-hydra/             # Hydra integration schemas
      usd-media/             # Media schemas
      usd-mtlx/              # MaterialX schemas
      usd-physics/           # Physics schemas
      usd-proc/              # Procedural schemas
      usd-render/            # Render settings schemas
      usd-ri/                # RenderMan schemas
      usd-semantics/         # Semantic labels
      usd-ui/                # UI schemas
      usd-utils/             # USD utilities
      usd-vol/               # Volume schemas
      usd-derive-macros/     # Proc macros for USD schema definitions
    imaging/                 # Hydra rendering pipeline
      usd-hd/                # Hydra core (render delegate, scene delegate, tasks)
      usd-hgi/               # Hardware graphics interface (abstract GPU API)
      usd-hgi-wgpu/          # HGI backend: wgpu
      usd-hd-st/             # Storm renderer (Hydra's default rasterizer)
      usd-hdx/               # Hydra extensions (selection, shadow, AOV)
      usd-glf/               # GL foundations (texture, lighting)
      usd-hio/               # Image I/O for Hydra
      usd-hf/                # Hydra field (volumes)
      usd-hdsi/              # Hydra scene index
      usd-hdar/              # Hydra asset resolution
      usd-hd-gp/             # Hydra generative procedurals
      usd-hd-mtlx/           # Hydra MaterialX
      usd-hgi-interop/       # HGI interop layer
      usd-hgi-metal/         # HGI backend: Metal (stub)
      usd-hgi-vulkan/        # HGI backend: Vulkan (stub)
      usd-camera-util/       # Camera utilities
      usd-geom-util/         # Geometry utilities (subdivision, triangulation)
      usd-px-osd/            # OpenSubdiv integration
      usd-app-utils/         # Application utilities
      _usd-garch/            # (placeholder, not in workspace)
      _usd-hgi-gl/           # (placeholder, not in workspace)
    usd-imaging/             # USD scene delegate for Hydra
    usd-validation/          # USD validation framework
    usd-view/                # Viewer application (egui + wgpu)
    usd-pyo3/                # Python bindings (PyO3/maturin)
      src/                   # 18 .rs files: lib, tf, gf/*, sdf, pcp, ar, vt, usd, ...
      pxr/                # Python package root (pxr._usd)
      pyproject.toml         # maturin build config
    ext/                     # External/vendored libraries
      draco-rs/              # Google Draco mesh compression (7 sub-crates + fuzz)
      gltf-rs/               # glTF I/O (+ gltf-derive, gltf-json)
      mtlx-rs/               # MaterialX
      opensubdiv-rs/         # OpenSubdiv (CPU subdivision)
      osl-rs/                # Open Shading Language
      pxr-lz4/               # LZ4 compression (Pixar variant)
  src/                       # Workspace root lib (re-exports all crates)
    lib.rs                   # Facade crate (usd)
    test_utils.rs            # Shared test utilities
    bin/                     # Binary targets
      usd/                   # CLI subcommands (cat, diff, tree, view, dump, etc.)
        main.rs              # Entry point
        cat.rs, compress.rs, diff.rs, dump.rs, dumpcrate.rs, edit.rs,
        filter.rs, fixbrokenpixarschemas.rs, genschemafromsdr.rs,
        meshdump.rs, resolve.rs, stitch.rs, stitchclips.rs, tree.rs,
        view.rs, zip.rs
      debug_parse.rs         # Debug USDA parsing
      profile_parse.rs       # Profile USDA parsing
      profile_render.rs      # Profile rendering
  vendor/                    # Vendored patches
    egui-wgpu-0.33.3/        # Patched egui-wgpu for wgpu 27
  bootstrap.py               # Build script (build, test, check, Python bindings)
  md/                        # Plans, reports, agent analysis docs
```

## Workspace Members (71 total)

### Base crates (9)
`usd-arch`, `usd-gf`, `usd-js`, `usd-tf`, `usd-plug`, `usd-trace`, `usd-ts`, `usd-vt`, `usd-work`

### USD core crates (6)
`usd-kind`, `usd-ar`, `usd-sdf`, `usd-pcp`, `usd-sdr`, `usd-core`

### USD schema crates (16)
`usd-geom`, `usd-shade`, `usd-lux`, `usd-skel`, `usd-hydra`, `usd-media`, `usd-mtlx`, `usd-physics`, `usd-proc`, `usd-render`, `usd-ri`, `usd-semantics`, `usd-ui`, `usd-utils`, `usd-vol`, `usd-derive-macros`

### Imaging crates (19)
`usd-camera-util`, `usd-geom-util`, `usd-px-osd`, `usd-hio`, `usd-hf`, `usd-hgi`, `usd-glf`, `usd-hd`, `usd-hdsi`, `usd-hdar`, `usd-hd-gp`, `usd-hd-mtlx`, `usd-hd-st`, `usd-hdx`, `usd-hgi-wgpu`, `usd-hgi-interop`, `usd-hgi-metal`, `usd-hgi-vulkan`, `usd-app-utils`

### Top-level crates (4)
`usd-imaging`, `usd-validation`, `usd-view`, `usd-pyo3`

### External crates (16)
`draco-rs` (+ `draco-bitstream`, `draco-core`, `draco-js`, `draco-cli`, `draco-maya`, `draco-unity`, `draco-fuzz`, `draco-rs/fuzz`), `osl-rs`, `pxr-lz4`, `mtlx-rs`, `opensubdiv-rs`, `gltf-rs` (+ `gltf-derive`, `gltf-json`)

### Root (1)
`.` — the `usd-rs` facade crate

## Python Bindings (usd-pyo3)

PyO3/maturin-based Python bindings exposing USD-RS as a `pxr` Python package.

- **Crate**: `crates/usd-pyo3/` — builds native extension `_usd` (cdylib)
- **Python package**: `pxr` — importable as `from pxr import _usd`
- **PyO3 version**: 0.28
- **Build**: `maturin develop` or `python bootstrap.py b p`
- **Module structure** (18 source files):
  - `lib.rs` — PyO3 module registration
  - `tf.rs` — Token, TfType bindings
  - `gf/` — Vec2/3/4, Matrix, Quat, BBox, Ray, Range, Frustum (`vec.rs`, `matrix.rs`, `quat.rs`, `geo.rs`)
  - `vt.rs` — VtValue, VtArray, VtDictionary
  - `sdf.rs` — SdfPath, SdfLayer, SdfValueTypeName
  - `pcp.rs` — PcpCache, PcpPrimIndex
  - `ar.rs` — Asset resolution
  - `kind.rs` — Kind registry
  - `usd.rs` — Stage, Prim, Attribute, Relationship
  - `geom.rs` — Geometry schemas
  - `shade.rs` — Shading schemas
  - `lux.rs` — Lighting schemas
  - `skel.rs` — Skeleton/skinning schemas
  - `cli.rs` — CLI integration helpers
- **Dependencies**: all base + core + schema crates, glam, half

## bootstrap.py Build Tool

Python build script (366 lines) for the project.

```
python bootstrap.py b             # Build everything (release)
python bootstrap.py b p           # Build Python bindings (maturin)
python bootstrap.py b -d          # Build all in debug
python bootstrap.py t             # Run tests
python bootstrap.py t p           # Run Python binding tests
python bootstrap.py ch            # Clippy + fmt check
python bootstrap.py clean         # Clean build artifacts
```

## C++ to Rust Mapping

| C++ module | Rust crate | C++ path |
|---|---|---|
| `arch` | `usd-arch` | `pxr/base/arch` |
| `gf` | `usd-gf` | `pxr/base/gf` |
| `js` | `usd-js` | `pxr/base/js` |
| `tf` | `usd-tf` | `pxr/base/tf` |
| `plug` | `usd-plug` | `pxr/base/plug` |
| `trace` | `usd-trace` | `pxr/base/trace` |
| `ts` | `usd-ts` | `pxr/base/ts` |
| `vt` | `usd-vt` | `pxr/base/vt` |
| `work` | `usd-work` | `pxr/base/work` |
| `ar` | `usd-ar` | `pxr/usd/ar` |
| `kind` | `usd-kind` | `pxr/usd/kind` |
| `sdf` | `usd-sdf` | `pxr/usd/sdf` |
| `pcp` | `usd-pcp` | `pxr/usd/pcp` |
| `sdr` | `usd-sdr` | `pxr/usd/sdr` |
| `usd` | `usd-core` | `pxr/usd/usd` |
| `usdGeom` | `usd-geom` | `pxr/usd/usdGeom` |
| `usdShade` | `usd-shade` | `pxr/usd/usdShade` |
| `hd` | `usd-hd` | `pxr/imaging/hd` |
| `hgi` | `usd-hgi` | `pxr/imaging/hgi` |
| `hdSt` | `usd-hd-st` | `pxr/imaging/hdSt` |
| `hdx` | `usd-hdx` | `pxr/imaging/hdx` |
| `usdImaging` | `usd-imaging` | `pxr/usdImaging/usdImaging` |

## Key Infrastructure APIs

### usd-tf: Type System & Tokens

**TfType** — full type registry with hierarchy, factories, aliases.
```rust
// Declare types (plugin registration uses these)
TfType::declare::<MyType>("MyType");
TfType::declare_with_bases::<MyType>("MyType", &[base_type_id]);
declare_by_name("TypeName");                          // name-only (plugin types)
declare_by_name_with_bases("TypeName", &["Base1"]);   // name-only with bases

// Query
TfType::find::<MyType>();                // by Rust TypeId
TfType::find_by_name("TypeName");        // by string name
tf_type.is_unknown();                    // not found
tf_type.is_a(other);                     // inheritance check
tf_type.type_name();                     // get name
tf_type.base_types();                    // direct bases
tf_type.get_all_derived_types();         // transitive children
tf_type.get_all_ancestor_types();        // transitive parents
tf_type.find_derived_by_name("Child");   // find child by name

// Factory pattern
tf_type.set_factory(Arc::new(MyFactory));
tf_type.get_factory();                   // -> Option<Arc<dyn FactoryBase>>
tf_type.get_factory_as::<MyFactory>();   // downcast
tf_type.has_factory();

// Aliases
tf_type.add_alias(base_type, "AliasName");
tf_type.aliases();
```

**Token** — interned string, cheap clone/compare.
```rust
Token::new("myToken");       // intern a string
Token::from("myToken");      // same
token.as_str();              // &str
token.is_empty();
to_token_vec(&strings);     // Vec<String> -> Vec<Token>
to_string_vec(&tokens);     // Vec<Token> -> Vec<String>
```

**FactoryBase** — trait for type factories:
```rust
pub trait FactoryBase: Send + Sync + Any {
    fn as_any(&self) -> &dyn Any;
}
```

### usd-js: JSON Value System

Used for plugInfo.json metadata throughout the plugin system.

```rust
pub type JsObject = BTreeMap<String, JsValue>;  // ordered map
pub type JsArray = Vec<JsValue>;

pub enum JsValue {
    Null, Bool(bool), Int(i64), Real(f64),
    String(String), Array(JsArray), Object(JsObject),
}

// Accessor methods on JsValue:
value.as_string() -> Option<&str>
value.as_i64()    -> Option<i64>
value.as_f64()    -> Option<f64>
value.as_bool()   -> Option<bool>
value.as_array()  -> Option<&JsArray>
value.as_object() -> Option<&JsObject>
value.is_int(), value.is_real(), value.is_string(), ...

// Parsing
usd_js::parse_string(json_str) -> Result<JsValue, JsParseError>

// Utils (operating on JsObject)
usd_js::utils::get_string(&obj, "key") -> Option<&str>
usd_js::utils::get_int(&obj, "key")    -> Option<i64>
usd_js::utils::get_real(&obj, "key")   -> Option<f64>
```

### usd-plug: Plugin Registry

**PlugRegistry** — singleton, discovers plugins via plugInfo.json.
```rust
let reg = PlugRegistry::get_instance();
reg.register_plugins("/path/to/plugins");           // discover + register
reg.get_all_plugins();                              // Vec<Arc<PlugPlugin>>
reg.get_plugin_with_name("Name");                   // Option<Arc<PlugPlugin>>
reg.get_plugin_for_type("TypeName");                // by declared type
reg.demand_plugin_for_type("TypeName");             // panics if missing
reg.find_type_by_name("TypeName");                  // Option<String>
reg.get_directly_derived_types("Base");             // Vec<String>
reg.get_all_derived_types("Base");                  // transitive
reg.find_derived_type_by_name("Base", "Child");     // Option<String>
reg.get_string_from_plugin_metadata("Type", "key"); // shorthand
reg.get_data_from_plugin_metadata("Type", "key");   // -> Option<JsValue>
```

**PlugPlugin** — single registered plugin:
```rust
plugin.get_name();                        // &str
plugin.get_path();                        // &str (library path)
plugin.get_resource_path();               // &str
plugin.get_type();                        // PluginType::{Library, Resource}
plugin.is_loaded();                       // bool
plugin.is_resource();                     // bool
plugin.load();                            // Result<(), String>
plugin.get_metadata();                    // &JsObject (the "Info" dict)
plugin.get_metadata_for_type("TypeName"); // Option<JsObject>
plugin.get_dependencies();                // JsObject (PluginDependencies)
plugin.declares_type("Type", incl_sub);   // bool
plugin.get_declared_types();              // Vec<String>
plugin.make_resource_path("rel/path");    // String
plugin.find_plugin_resource("path", verify); // String
```

**Notice system**:
```rust
on_did_register_plugins(|plugins: &[Arc<PlugPlugin>]| { ... });
```

### usd-vt: Value Types

- `VtValue` — type-erased value container (like `std::any::Any` but with USD semantics)
- `VtArray<T>` — copy-on-write array
- `VtDictionary` — string-keyed heterogeneous map

## Imaging / Scene Index Notes

- `crates/usd-imaging/src/skel/skeleton_resolving_scene_index.rs` must store live `DataSourceResolvedSkeletonPrim` overlays, not retained placeholder containers. Dependency refresh is driven by:
  - direct skeleton dirties,
  - bound or instance-resolved animation source prims,
  - instancer xform / animation-source primvars.

- `crates/usd-imaging/src/skel/data_source_resolved_skeleton_prim.rs` is now the single source of truth for:
  - `ResolvedSkeletonSchema`,
  - guide mesh topology,
  - guide points primvars,
  - dirty propagation from skeleton / skelAnimation / instancer changes.

- `crates/usd-imaging/src/data_source_mapped.rs` is a shared infrastructure layer, not a leaf helper.
  If it is stubbed, every prim type using mapped schema wrappers becomes structurally incomplete.
  Confirmed users include `skel` schema wrappers and future NURBS / field-style adapters.

- `crates/usd-imaging/src/material_adapter.rs` must not carry a second local material datasource implementation.
  The live path is `crates/usd-imaging/src/data_source_material.rs`; the adapter should just expose `DataSourceMaterialPrim` and its invalidation. A duplicated stub version in the adapter silently regresses material parity.

- `crates/usd-imaging/src/data_source_material.rs::DataSourceMaterialPrim` needs to implement `HdDataSourceBase` and `HdContainerDataSource` directly.
  The file already contains the real material-network logic; without those trait impls, adapters cannot actually publish that prim container and downstream code falls back to stale or stub paths.

- `crates/usd-imaging/src/material_binding_api_adapter.rs`, `crates/usd-imaging/src/light_api_adapter.rs`, and `crates/usd-imaging/src/geom_subset_adapter.rs` are now live scene-index contributors rather than invalidation-only shells.
  They must build real datasource containers:
  - `usdMaterialBindings` for direct + collection bindings,
  - `material` + `light` for `UsdLuxLightAPI`,
  - `geomSubset` overlaid with `DataSourcePrim` for subset material-binding support.

- For `crates/usd-imaging/src/curves_adapter.rs`, `crates/usd-imaging/src/implicit_surface_adapter.rs`, and `crates/usd-imaging/src/coord_sys_adapter.rs`, the immediate failure mode was empty `get()` implementations.
  Even when richer specialized datasources exist elsewhere, these adapter-local containers must not return `None` for all fields or the prim type becomes structurally invisible to Hydra.

- Explicit non-Storm placeholder modules still exist under `crates/usd-imaging/src/ri_pxr/`.
  Treat them as intentional stubs unless the task is specifically HdPrman parity.

### usd-gf: Graphics Foundations

- `GfVec2f/3f/4f/2d/3d/4d` — vector types
- `GfMatrix2f/3f/4f/2d/3d/4d` — matrix types
- `GfQuatf/d/h` — quaternions
- `GfBBox3d` — bounding box
- `GfRay` — ray
- `GfRange1f/2f/3f/1d/2d/3d` — ranges
- `GfFrustum` — view frustum

### usd-sdf: Scene Description

- `SdfPath` — scene graph path (`/Root/Child.attr`)
- `SdfLayer` — single layer of scene data
- `SdfSpec` — abstract spec (prim, property, attribute, relationship)
- `SdfValueTypeName` — typed attribute value names
- USDA reader/writer, USDC reader (crate format)

### usd-pcp: Prim Cache Population

- Composition engine (LIVRPS: Local, Inherits, Variants, References, Payloads, Specializes)
- `PcpCache` — composed prim index cache
- `PcpPrimIndex` — composed index for a single prim

### usd-core: USD Core

- `UsdStage` — the central stage object
- `UsdPrim` — a prim on a stage
- `UsdAttribute` — typed attribute
- `UsdRelationship` — relationship
- Instance proxies, prototype population

## Imaging / Scene Index Notes

- `crates/imaging/usd-hd/src/flattened_xform_data_source_provider.rs` is on the critical animated-xform path. The `xform.matrix` result for flattened prims must stay live; returning a retained matrix snapshot at `t=0` freezes transforms after `Engine::set_time()`. The safe behavior is a sampled combiner that recomputes `local * parent` on read and merges sample times from both inputs.

- `crates/imaging/usd-hd/src/scene_index_adapter_scene_delegate.rs` uses a per-thread last-prim cache. Matching the C++ reference means the cache must be owned by the adapter instance, keyed by thread id, and fully cleared on `prims_added`, `prims_removed`, `prims_dirtied`, and `prims_renamed`. A process-global `thread_local!` cache is wrong because it only invalidates the current thread.

- `crates/usd-imaging/src/ni_prototype_propagating_scene_index.rs` must observe both the instance-aggregation scene index and the internal `HdMergingSceneIndex`. Dirties from the merging layer are required for native-instancing propagation. The per-prototype scene-index cache should also reuse `(prototype_si, instance_agg_si)` by `(prototype_name, overlay_hash)` to match `_ref/OpenUSD/pxr/usdImaging/usdImaging/niPrototypePropagatingSceneIndex.cpp`.

- `crates/usd-imaging/src/root_overrides_scene_index.rs` should keep one persistent overlay datasource backed by shared mutable state for root transform/visibility. Rebuilding retained snapshot datasources on each setter call diverges from `_ref` and makes downstream overlays less faithful.

- `crates/usd-imaging/src/app_utils/frame_recorder.rs` is no longer the authoritative placeholder story for recording. The current recording/export path routes through the engine-backed implementation described in the `Current State` section below, so older notes that treat `FrameRecorder::record()` as a hard stub are stale.

- `crates/imaging/usd-hdx/src/render_task.rs` now emits ordered `renderTaskRequests` (material tag + render-pass-state handle) into `HdTaskContext`, and `crates/usd-imaging/src/gl/engine/mod.rs` consumes them to execute Storm geometry passes in task order. Collapsing all Hydra render tasks back into a single generic geometry pass regresses material-tag ordering parity (`default/masked/additive/translucent/volume`).

- `crates/imaging/usd-hdx/src/aov_input_task.rs`, `color_correction_task.rs`, and `present_task.rs` must not pretend post-processing already happened during `HdEngine::execute()`. In this port, real backend draw still happens later in `crates/usd-imaging/src/gl/engine/mod.rs`, so those tasks now emit deferred request records into `HdTaskContext`, and `Engine` performs the post-backend bridge (publishing real AOV handles, replaying post-FX passes, then ending the HGI frame) after geometry/skydome execution.

- `crates/imaging/usd-hdx/src/colorize_selection_task.rs` is in the same bucket: it is a post-AOV fullscreen pass in `_ref`, so it must not mutate task context as if selection compositing already happened before backend draw. Keep it as a deferred request until the engine-side post-FX compositor is brought up.

- Deferred post tasks now also append `postTaskOrder` in `HdTaskContext`. `crates/usd-imaging/src/gl/engine/mod.rs` replays post-backend work in that exact task order instead of consuming request vectors by fixed hard-coded grouping. This matters once `aovInput`, `colorizeSelection`, `colorCorrection`, `visualizeAov`, and `present` all become live fullscreen passes.

- `crates/usd-imaging/src/gl/engine/mod.rs` now owns a dedicated `wgpu_post_color_texture` for post-FX ping-pong. Publishing `color` and `colorIntermediate` as the same texture was structurally wrong; fullscreen tasks like color correction / visualize-AOV require distinct source and destination handles.

- `crates/usd-imaging/src/gl/engine/mod.rs` now executes both `HdxColorCorrectionTaskRequest(mode=sRGB)` and `HdxColorCorrectionTaskRequest(mode=openColorIO)` as real engine-side compute passes. The post-FX bridge samples the current `wgpu_color_texture`, writes into `wgpu_post_color_texture`, swaps the handles, and republishes `color`, `colorIntermediate`, and `aov_color` in `HdTaskContext` so later deferred tasks see the post-corrected result.

- The engine-side AOV bridge must publish depth separately from color. `aov_<name>` cannot blindly alias `wgpu_color_texture` for every request; `aov_depth` needs the depth handle, and the depth render target must be created with `HgiTextureUsage::SHADER_READ` so post-FX / visualize passes can legally sample it.

- `crates/usd-imaging/src/gl/engine/mod.rs::apply_hdx_render_task_request_state()` must translate `HdxRenderPassState::get_aov_bindings()` into real `HdStAovBinding`s backed by engine-owned textures. Without that bridge, the task graph can request `color/depth/primId/instanceId/elementId`, but Storm still renders only into the fallback color/depth pair and every downstream AOV task observes the wrong source.

- Unmatched/non-color AOV requests must no longer alias the main color target. `crates/usd-imaging/src/gl/engine/mod.rs` now allocates per-name auxiliary AOV textures on demand and publishes them back through `aov_<name>` instead of silently routing those requests to `wgpu_color_texture`.

- The engine now allocates main-pass ID AOV textures (`wgpu_prim_id_texture`, `wgpu_instance_id_texture`, `wgpu_element_id_texture`) alongside color/depth/post targets. This is distinct from the older pick-only side path and is required for reference-style `color + depth + id` multi-AOV render outputs from `HdxTaskController::set_render_outputs()`.

- Deferred `visualizeAov` is no longer a log-only placeholder. The engine now runs real post-FX compute kernels for fallback/raw, depth renormalization, ID hashing, and normal visualization, using `wgpu_post_color_texture` as the intermediate output and then swapping the display target.

- Deferred `colorizeSelection` now has a real engine-side compute pass too. Selection data is no longer synthesized ad hoc in `usd-imaging::gl::Engine`; `crates/imaging/usd-hdx/src/selection_task.rs` now populates `selectionState`, `selectionBuffer`, `selectionOffsets`, and `selectionUniforms` into `HdTaskContext`, and the engine-side compositor consumes those buffers after backend draw.

- The engine-side selection compositor now decodes the same two highlight modes (`select`, `locate`) and the same hierarchical selection-buffer structure (`prim -> instance -> element`) that the C++ `HdxColorizeSelectionTask::_ColorizeSelection()` loop expects, instead of treating selection as a flat primId-only mask.

- `crates/usd-view/src/panels/viewport/mod.rs` now feeds hover/rollover picks into `usd_imaging::gl::Engine::set_located(...)`, so the locate highlight path is no longer just a dormant contract. Click selection still uses `set_selected(...)`; rollover uses the parallel locate channel.

- `crates/imaging/usd-hdx/src/task_controller.rs::resolve_render_outputs()` now distinguishes the Storm ordering/augmentation rules from the non-Storm path. For Storm, the resolved AOV list follows `_ref/OpenUSD/pxr/imaging/hdx/taskController.cpp::_ResolvedRenderOutputs(...)`: `color`, `primId+instanceId`, optional `Neye`, then `depth`.

- `crates/usd-imaging/src/gl/engine/mod.rs::get_renderer_aovs()` should not advertise a hand-picked superset. It now follows `_ref/OpenUSD/pxr/usdImaging/usdImagingGL/engine.cpp::GetRendererAovs()` by filtering a fixed candidate set (`primId`, `depth`, `normal`, `Neye`, `primvars:st`) through the active render delegate's `GetDefaultAovDescriptor(...)`.

- `crates/imaging/usd-hdx/src/pick_task.rs` now has real GPU texture readback helpers driven from `HdTaskContext` (`aov_<name>` textures + `renderDriver`/HGI driver), so the CPU-side resolve path no longer hardcodes empty arrays for non-deep picking. For `resolveDeep`, the task now also emits explicit backend-facing `pickTaskRequests` (occluder / pickable / overlay) when no `pickBuffer` has been published yet, then resolves from the SSBO on a second execute once the engine has replayed those requests.

- `crates/imaging/usd-hgi-wgpu/src/blit_cmds.rs` already has a generic GPU-buffer-to-CPU staging path (`HgiBufferGpuToCpuOp` -> staging buffer -> `map_async`). The remaining `resolveDeep` backend gap was not raw buffer readback support; it was the missing writable-storage-buffer shader/bind wiring on the live Storm/wgpu render path.

- `crates/imaging/usd-hd-st/src/draw_batch.rs` and `crates/imaging/usd-hd-st/src/wgsl_code_gen.rs` now treat deep-pick as a real shader variant: when a pick pass has a bound `pick_buffer`, the generated flat-color WGSL declares a `PickBuffer`-style `var<storage, read_write>` binding, the draw batch binds it through `HgiGraphicsCmds::bind_storage_buffer(...)`, and the fragment shader emits reference-style hashed deep-pick writes instead of leaving `pick_buffer_rw` as a dead flag.

- `crates/imaging/usd-hdx/src/pick_task.rs` now provides reusable `PickBuffer` helpers instead of keeping deep-pick buffer layout/readback logic implicit: one helper builds the exact reference-style header/sub-buffer initialization array, and another performs generic HGI buffer readback to `Vec<i32>`.

- `crates/usd-imaging/src/gl/engine/picking.rs` now drives `resolveDeep` through a two-phase task path that is much closer to the C++ `HdxPickTask::Execute()` flow: the first `HdEngine::execute()` lets `HdxPickTask` publish `pickTaskRequests`, the engine replays those requests against dedicated 1x1 pick targets while binding the writable `PickBuffer`, and a second `HdEngine::execute()` lets `HdxPickTask` consume the SSBO and populate hits. The old direct render/readback escape hatch was removed so deep pick now depends only on the task-driven path.

- `crates/usd-imaging/src/gl/engine/picking.rs` replays those `pickTaskRequests` with a dedicated pick-only attachment mapping instead of routing them through the main viewport AOV bridge. That keeps the deep-pick producer path isolated to the 1x1 pick targets and avoids accidentally binding full-resolution engine AOV textures during task-driven picking. The replay loop also performs the C++ `_UpdateUseOverlayPass()` equivalent: after setting a request's `material_tag` on the render pass it calls `has_draw_items()` and skips execution when no draw items match (e.g. `displayInOverlay` with no overlay geometry).

- `crates/usd-imaging/src/gl/engine/mod.rs::PickParams` is no longer just a resolve-mode shim. It now carries `pick_target` as well, and both HDX-driven pick producers (`try_pick_from_render_buffer_task()` and the deep-pick request/replay path) forward that target into `HdxPickTaskContextParams` instead of hardcoding `pickPrimsAndInstances`.

- `crates/imaging/usd-hdx/src/pick_from_render_buffer_task.rs` and `crates/imaging/usd-hdx/src/task_controller.rs` now wire `edgeId` and `pointId` alongside `primId` / `instanceId` / `elementId` / `depth` / `normal`. That means the deferred pick-from-render-buffer path can resolve edge and point subprims whenever those AOVs are present, rather than structurally dropping them before `HdxPickResult`.

- `crates/usd-imaging/src/gl/engine/mod.rs::get_aov_render_buffer()` no longer returns a placeholder `None` type. The Rust equivalent returns an owned `EngineAovRenderBuffer` snapshot (`AOV name`, task-controller render-buffer path, dimensions, format, multisample flag, texture handle), which is the closest stable analogue to the C++ raw `HdRenderBuffer*`.

- Legacy in-pass selection highlighting in `update_render_pass_state()` is intentionally disabled now that the reference-style `HdxSelectionTask -> HdxColorizeSelectionTask` post-FX path is live. Keeping both paths enabled double-applies highlight colors.

- Useful regression coverage for this area:
  - `crates/imaging/usd-hd/tests/test_scene_index.rs::test_flattening_scene_index_preserves_time_samples`
  - `crates/usd-imaging/src/gl/engine/mod.rs::test_render_batch_refreshes_transforms_when_time_changes_and_paths_do_not`
  - `crates/usd-imaging/src/gl/engine/mod.rs::test_sync_render_index_state_uses_synced_mesh_data_without_stage_access`

## usd-view Notes

- `crates/usd-view/src/event_bus.rs` is already a port of Playa's `core/event_bus.rs`, but `usd-view` currently uses it only as a deferred queue for background stage loading. There are no non-test subscribers in `crates/usd-view/src`, so `EventBus` is not the main runtime bottleneck.

- The startup load split is:
  `ViewerApp::load_file()` -> background thread `RootDataModel::open_stage_detached()` + `RootDataModel::collect_stage_time_samples()` -> `EventBus` queue -> `ViewerApp::handle_bus_events()` -> `ViewerApp::on_stage_loaded()`.
  This removes `Stage::open()` and time-sample collection from the UI thread, but the UI thread still performs post-load work (`apply_loaded_stage`, `invalidate_scene(SceneSwitch)`, `apply_post_load_config`) before the first real scene render.

- The dominant playback/render hot path is in `crates/usd-view/src/panels/viewport/mod.rs`: `eng.render()` -> `read_render_pixels_staged()` or `read_render_pixels()` -> CPU `color_correct()` -> `update_viewport_texture()`. This is a full GPU->CPU->GPU roundtrip every frame. `EventBus` does not address that cost.

- `crates/usd-view/src/panels/viewport/color_correction.rs::update_viewport_texture()` reuses the egui texture handle, but still uploads a full `ColorImage` every frame via `handle.set(...)`. The current design avoids texture recreation, not texture upload.

- `crates/usd-view/src/app/sync.rs::apply_view_settings_to_menu_state()` now follows the `_ref/OpenUSD/pxr/usdImaging/usdviewq/appController.py` camera-list pattern: `menu_state.scene_cameras` is cached behind `ViewerApp::scene_cameras_dirty`, refreshed only on scene invalidation / stage notices, and no longer rebuilt every frame.

- `crates/usd-view/src/data_model.rs` now uses reference-style timeline samples instead of traversing every attribute in the stage. `RootDataModel::collect_stage_time_samples()` derives samples from authored stage range plus `timeCodesPerSecond / framesPerSecond` via a Rust port of usdviewq `Drange`, and `rebuild_stage_time_samples()` re-applies that logic when the effective playback range changes.

- `crates/usd-view/src/app/mod.rs` now drains `RootDataModel::change_state` each frame via `handle_stage_notices()`. Real `ObjectsChanged` notices trigger targeted timeline rebuild + `InvalidateLevel::Reload`, which closes the previously unfinished notice-driven invalidation path.

- `crates/usd-view/src/playback.rs`, `crates/usd-view/src/app/actions.rs`, `crates/usd-view/src/app/toolbar.rs`, `crates/usd-view/src/app/mod.rs`, and `crates/usd-view/src/panels/viewport/mod.rs` no longer emit `[DIAG]` stderr spam on the hot playback/render path. Remaining frame-time cost is dominated by the viewport GPU->CPU->GPU roundtrip, not by logging.

## Test Patterns

### Integration tests
- Place in `crates/<layer>/<crate>/tests/*.rs`
- Test data in `crates/<layer>/<crate>/testenv/` (copied from C++ reference)
- Test fixtures in `crates/<layer>/<crate>/tests/fixtures/` (Rust-specific)
- Port Python tests from `_ref/OpenUSD/.../testPlug.py` etc. to Rust `#[test]` functions

### Unit tests
- `#[cfg(test)] mod tests { ... }` at the bottom of source files
- Use unique plugin/type names to avoid singleton conflicts across tests

### Test data
- testenv/ directories contain plugInfo.json files and test fixtures from C++ reference
- Adapt `Type: "library"` to `Type: "resource"` when actual DSO loading isn't needed
- Python test modules (TestPlugModule*) map to Rust test structs/factories

## File Naming Conventions

- Rust source: `snake_case.rs` (e.g., `type_info.rs` for C++ `typeInfo.h/cpp`)
- No `mod.rs` files — use `module_name.rs` at the parent level
- Crate lib entry: named after the crate in Cargo.toml `[lib] path = "..."`
- Test files: `test_<feature>.rs` in `tests/` dir

## Dependency Graph (simplified)

```
usd-arch (no deps)
  <- usd-tf (tokens, types, diagnostics)
  <- usd-js (JSON values)
  <- usd-gf (math)
  <- usd-trace (profiling)
  <- usd-vt (value types) <- usd-gf, usd-tf
  <- usd-plug (plugins) <- usd-tf, usd-js
  <- usd-work (threading) <- usd-trace
  <- usd-ts (splines) <- usd-gf, usd-tf

usd-sdf <- usd-tf, usd-vt, usd-plug, usd-ar, usd-js
usd-pcp <- usd-sdf, usd-ar
usd-core <- usd-pcp, usd-sdf, usd-ar, usd-plug, usd-kind

usd-geom, usd-shade, ... <- usd-core
usd-hd <- usd-sdf, usd-vt, usd-gf, usd-tf
usd-hgi <- usd-gf, usd-tf
usd-hd-st <- usd-hd, usd-hgi, usd-glf
usd-imaging <- usd-core, usd-hd, usd-geom, ...
usd-pyo3 <- usd-tf, usd-gf, usd-vt, usd-sdf, usd-pcp, usd-ar, usd-core, usd-geom, usd-shade, ...
```

## Key Architectural Notes

1. **Singletons**: PlugRegistry, TfType registry — use `OnceLock` pattern.
   Tests sharing a process share singleton state. Use unique names per test.

2. **plugInfo.json**: Central to plugin discovery. Supports:
   - `Includes` directives for recursive discovery
   - Glob patterns (`*`, `**`) in search paths
   - Comment stripping (C/C++ style comments in JSON)
   - Plugin types: `library`, `resource`, `python` (python → resource in Rust)

3. **Token interning**: `Token::new()` interns; `Token::clone()` is cheap.
   Prefer `clone()` over `new()` for already-interned tokens (perf).

4. **Error handling**: Never `unwrap()`. Use `?` propagation.
   Never `let _ =` on fallible ops. Log or handle explicitly.

5. **No mod.rs**: Always `module_name.rs`, never `module_name/mod.rs`.

6. **C++ reference is authoritative**: When behavior is unclear, read
   `_ref/OpenUSD/pxr/...` C++ source. Match semantics, not syntax.

7. **Build targets**: `cargo check -p usd-plug --message-format=short`
   for quick validation. Use short message format to save context.

## What NOT to Do

- Don't compare Rust vs C++ by line count. Compare function-by-function.
- Don't claim "complex/needs refactor" without reading the code first.
- Don't use `unwrap()`, unchecked indexing, or `let _ =` on fallible ops.
- Don't create `mod.rs` files.
- Don't use Unicode in build files.
- Don't use git to "restore" code by discarding changes.
- Don't skip test porting — testenv files and Python tests MUST be ported.
- Don't build tests repeatedly to debug — analyze code statically first.
- Don't use agents excessively — max 1-3 for research, then verify.

## Current State (2026-04-06)

- Branch: `main`, build: 0 errors, 0 warnings
- 71 workspace members, ~2480 `.rs` files, ~130k LOC Rust
- Imaging pipeline functional (wgpu backend) with GPU display color path and HDR-capable present preference
- `usd-pyo3` Python bindings added (PyO3 0.28, maturin, `pxr` package) — covers Tf, Gf, Vt, Sdf, Pcp, Ar, Kind, Usd, Geom, Shade, Lux, Skel
- `bootstrap.py` build tool added — unified build/test/check/Python commands
- `usd-imaging` scene-index parity tightened for xform flattening, render settings/products, root overrides, `ri_pxr`, and skel legacy adapters
- `usd-app-utils::FrameRecorder` placeholder removed; recording now routes through `usd-imaging` engine and supports `.exr`
- `usd-view` no longer relies on CPU color correction for normal viewport present; export path keeps HDR `.exr`
- See `md/` for detailed parity check reports

## Imaging Parity Notes

- `HdFlattenedXformDataSourceProvider` must stay live/time-sampled; retained snapshot matrices break animated xforms after `set_time()`
- `HdSceneIndexAdapterSceneDelegate` input-prim cache must be owned by the delegate and fully clearable; global `thread_local!` cache semantics drift from `_ref`
- `UsdImagingNiPrototypePropagatingSceneIndex` needs observer wiring on the merged prototype scene index to propagate dirties correctly
- `ri_pxr` adapters are no longer structural stubs: camera API, camera projection API, integrator, sample/display filters, projection schema, and render terminals now produce real Hydra-facing data
- Legacy skel adapters (`Skeleton`, `BlendShape`, `SkelRoot`) should delegate to `DataSourceSkeletonPrim`, `DataSourceBlendShapePrim`, and `DataSourcePrim` overlays rather than returning `None`
- `AdapterRegistry::find_for_prim()` must walk schema inheritance (`prim.is_a(...)`) before falling back to `NoOpAdapter`
