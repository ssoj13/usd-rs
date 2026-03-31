# USD Imaging Crates

Rust port of Pixar's OpenUSD imaging layer (`pxr/imaging/`). These crates implement the Hydra rendering architecture -- an abstract, pluggable rendering framework that sits between USD scene data and GPU backends.

## Crate Overview

### Hydra Core

| Crate | C++ Origin | Role |
|-------|-----------|------|
| **usd-hd** | `pxr/imaging/hd` | **Hydra core.** Abstract rendering framework: Render Index (central registry of all scene objects), Scene Delegate (interface to scene data), Render Delegate (backend-specific renderer), Change Tracker (incremental dirty-state updates), Data Sources (hierarchical time-sampled data), schemas. Prims are split into three categories: **Rprim** (geometry: meshes, curves, points), **Sprim** (state: cameras, lights, materials), **Bprim** (buffers, render targets). |
| **usd-hf** | `pxr/imaging/hf` | **Hydra Foundation.** Base plugin system for Hydra: `HfPluginBase` trait, `HfPluginRegistry`, plugin descriptors. Render delegates (Storm, Embree, etc.) register through this layer. |
| **usd-hdsi** | `pxr/imaging/hdsi` | **Scene Index filters.** A chain of filters that transform scene data before rendering: implicit surface to mesh conversion, type/path pruning, material binding resolution, light linking, NURBS to mesh, coordinate systems, etc. |
| **usd-hdx** | `pxr/imaging/hdx` | **Hydra Extensions.** High-level task-based rendering pipeline: `TaskController` orchestrates render tasks (main render, shadows, selection highlighting, picking, color correction, AOV, skydome, bounding boxes, present to screen). |

### GPU Abstraction Layer (HGI)

| Crate | C++ Origin | Role |
|-------|-----------|------|
| **usd-hgi** | `pxr/imaging/hgi` | **Hydra Graphics Interface.** Abstract GPU API: traits for buffers, textures, shaders, pipelines, command buffers. Backend-agnostic layer on top of concrete GPU APIs. |
| **usd-hgi-wgpu** | *(no C++ equivalent)* | **wgpu backend for HGI.** Primary working backend in this project. Implements all HGI traits via wgpu (Vulkan/DX12/Metal through a single safe Rust API). Buffers, textures, pipelines, mipmap generation, staged readback. |
| **usd-hgi-vulkan** | `pxr/imaging/hgiVulkan` | **Vulkan backend for HGI** (port). Full implementation via ash + gpu-allocator + shaderc. |
| **usd-hgi-metal** | `pxr/imaging/hgiMetal` | **Metal backend for HGI** (stub). Compiles on all platforms but only functional on macOS/iOS. |
| **usd-hgi-interop** | `pxr/imaging/hgiInterop` | **Cross-backend presentation.** Composites HGI render results (color + optional depth textures) onto a wgpu surface via fullscreen-triangle blit with alpha blending. |

### Storm Renderer

| Crate | C++ Origin | Role |
|-------|-----------|------|
| **usd-hd-st** | `pxr/imaging/hdSt` | **Storm** -- Hydra's default rasterizer. Render Delegate supporting meshes, curves, points, subdivision surfaces (via OpenSubdiv), GPU instancing, MaterialX materials, and OIT (order-independent transparency). Manages GPU resources through `ResourceRegistry`, organizes draw calls via `DrawBatch`. |

### Utilities and Support

| Crate | C++ Origin | Role |
|-------|-----------|------|
| **usd-glf** | `pxr/imaging/glf` | **GL Foundations.** OpenGL utilities: contexts, textures, FBOs, lighting structures (`SimpleLight`, `SimpleMaterial`, `SimpleLightingContext`). Partially stubbed since the primary backend is wgpu. |
| **usd-hio** | `pxr/imaging/hio` | **Image I/O.** Texture loading/writing: PNG, JPEG, EXR, HDR, BMP, TGA, GIF. Also parses GLSLFX shader packages. |
| **usd-hdar** | `pxr/imaging/hdar` | **Hydra Asset Resolution.** Bridge between the Scene Index system and the Asset Resolver (usd-ar). Allows different parts of the scene to carry different asset resolution contexts. |
| **usd-hd-gp** | `pxr/imaging/hdGp` | **Generative Procedurals.** Procedural prim generation in Hydra: `HdGpGenerativeProcedural` trait, plugin registry, Scene Index for resolving procedurals with dependency tracking and async support. |
| **usd-hd-mtlx** | `pxr/imaging/hdMtlx` | **MaterialX integration.** Converts between Hydra material networks and MaterialX documents. Tracks texture and primvar node usage. |
| **usd-camera-util** | `pxr/imaging/cameraUtil` | **Camera utilities.** Window conforming (adjusting frustum to match target aspect ratio), Framing (filmback-to-pixel mapping), RenderMan-compatible screen window parameters. |
| **usd-geom-util** | `pxr/imaging/geomUtil` | **Primitive mesh generators.** Procedural mesh generation for implicit surfaces: cube, sphere, cylinder, cone, capsule, plane, disk. Produces topology, normals, and UVs. |
| **usd-px-osd** | `pxr/imaging/pxOsd` | **OpenSubdiv integration.** Subdivision surface types: `MeshTopology`, interpolation rules, crease data. Bridge between USD and OpenSubdiv. |
| **usd-app-utils** | `pxr/usdImaging/usdAppUtils` | **Application utilities.** Camera lookup on a Stage, `FrameRecorder` for playblasts and offline rendering. |

## Architecture

### Data Flow

```
+-----------------------------------------------------+
|                    USD Stage                         |
|              (usd-core, usd-sdf, usd-pcp)           |
+---------------------------+--------------------------+
                            |
                            v
+-----------------------------------------------------+
|              USD Imaging (Scene Delegate)            |
|                  crates/usd-imaging/                 |
|   Translates USD Prims -> Hydra Rprims/Sprims/Bprims|
|   Manages instancing, materials, cameras             |
+---------------------------+--------------------------+
                            |
                            v
+-----------------------------------------------------+
|           Scene Index Chain (usd-hdsi)               |
|  Implicit->Mesh | MaterialResolve | Pruning | etc.  |
|  Filter chain transforming scene data for rendering  |
+---------------------------+--------------------------+
                            |
                            v
+-----------------------------------------------------+
|            Hydra Render Index (usd-hd)               |
|  Central registry of all scene objects               |
|  Change Tracker: tracks dirty state incrementally    |
|  Rprim (mesh,curves) | Sprim (cam,light) | Bprim    |
+------------+-----------------------------+----------+
             |                             |
             v                             v
+------------------+           +-----------------------+
|  Task Controller |           |   Render Delegate     |
|    (usd-hdx)     |           |   (usd-hd-st/Storm)  |
|                  |           |                       |
| RenderTask       |---------->| ResourceRegistry      |
| ShadowTask       |           | DrawBatch             |
| SelectionTask    |           | RenderPass            |
| PickTask         |           | Mesh/Material sync    |
| ColorCorrection  |           |                       |
| PresentTask      |           +-----------+-----------+
+------------------+                       |
                                           v
                              +------------------------+
                              |     HGI (usd-hgi)      |
                              |  Abstract GPU API       |
                              |  Buffer|Texture|Pipeline|
                              |  GraphicsCmds|BlitCmds  |
                              +----------+-------------+
                                         |
                          +--------------+--------------+
                          v              v              v
                    +----------+  +----------+  +----------+
                    | HGI-wgpu |  |HGI-Vulkan|  |HGI-Metal |
                    |(primary) |  |  (port)  |  |  (stub)  |
                    +----------+  +----------+  +----------+
                          |
                          v
                    +--------------------------+
                    |  HGI Interop             |
                    |  (usd-hgi-interop)       |
                    |  Composite -> screen/egui|
                    +--------------------------+
```

### Render Frame Pipeline

1. **Stage** holds USD data (prims, attributes, time samples)
2. **UsdImaging** (Scene Delegate) translates USD prims into Hydra representation -- Rprims (meshes, curves), Sprims (cameras, lights, materials), Bprims (render buffers)
3. **Scene Index Chain** (hdsi) -- filter chain: implicit surfaces are converted to meshes, material bindings are resolved, unwanted prims are pruned
4. **Render Index** (hd) -- central registry, tracks dirty bits for incremental updates
5. **Task Controller** (hdx) -- orchestrates tasks: setup -> render -> shadows -> selection -> color correction -> present
6. **Storm** (hd-st) -- sync phase (updates GPU buffers from scene data) -> draw list build -> command recording -> submission
7. **HGI** -- abstract GPU layer, Storm records commands through HGI traits
8. **HGI-wgpu** -- concrete implementation, translates HGI calls to wgpu API -> Vulkan/DX12/Metal
9. **HGI Interop** -- final blit of the rendered result to screen

### Support Crate Roles

- **usd-glf** -- lighting/material structs used by hdx to pass illumination parameters
- **usd-hio** -- texture loading (PNG/EXR/HDR), GLSLFX shader parsing
- **usd-geom-util** -- mesh generation for implicit primitives (Cube, Sphere, etc.)
- **usd-px-osd** -- subdivision surface types used by hd-st for Catmull-Clark
- **usd-camera-util** -- camera frustum fitting to viewport dimensions
- **usd-hdar** -- propagates asset resolution context through the scene index
- **usd-hd-gp** -- procedural generation (deferred evaluation inside scene index)
- **usd-hd-mtlx** -- MaterialX material networks for Storm

## Dependency Graph

```
usd-px-osd  <--  usd-tf
usd-hf      <--  usd-tf
usd-camera-util  <--  usd-gf

usd-geom-util  <--  usd-gf, usd-px-osd

usd-hgi  <--  usd-gf, usd-tf, usd-vt
  +-- usd-hgi-wgpu   <--  usd-hgi, usd-gf, wgpu
  +-- usd-hgi-vulkan <--  usd-hgi, usd-gf, ash
  +-- usd-hgi-metal  <--  usd-hgi, usd-gf
  +-- usd-hgi-interop  <--  wgpu

usd-hio  <--  usd-gf, usd-tf, usd-vt, usd-ar, image, exr
usd-glf  <--  usd-gf, usd-tf, usd-vt, usd-sdf

usd-hd  <--  usd-ar, usd-camera-util, usd-gf, usd-hf, usd-px-osd,
             usd-sdf, usd-sdr, usd-tf, usd-vt

usd-hdsi  <--  usd-hd, usd-geom-util, usd-gf, usd-px-osd,
               usd-sdf, usd-tf, usd-vt

usd-hdar  <--  usd-hd, usd-ar, usd-sdf, usd-tf

usd-hd-gp   <--  usd-hd, usd-hf, usd-sdf, usd-tf
usd-hd-mtlx <--  usd-sdf, usd-tf, usd-vt, usd-gf, usd-sdr, mtlx-rs

usd-hd-st  <--  usd-hd, usd-hdsi, usd-hgi, usd-glf, usd-hio,
                usd-px-osd, usd-gf, usd-sdf, usd-tf, usd-vt, usd-trace
                [optional: usd-hgi-wgpu, opensubdiv-rs, usd-hd-mtlx, mtlx-rs]

usd-hdx  <--  usd-hd, usd-hgi, usd-glf, usd-hio, usd-camera-util,
              usd-gf, usd-sdf, usd-tf, usd-vt

usd-app-utils  <--  usd-core, usd-geom, usd-sdf, usd-tf
```
