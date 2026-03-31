# usd-view — USD Scene Viewer

Rust/egui alternative to usdview (usdviewq). Full-featured USD scene viewer and inspector.

## Features

- **3D Viewport** — Storm-on-wgpu rendering via UsdImagingGL Engine
  - Orbit/pan/zoom camera controls, GPU picking, click selection highlight, rollover locate highlight
  - 10 draw modes (Shaded Smooth/Flat, Wireframe, WireframeOnSurface, HiddenSurfaceWireframe, Points, GeomOnly/Flat/Smooth, Bounds)
  - PBR materials (UsdPreviewSurface, GGX/Smith/Fresnel)
  - HDR/IBL environment lighting (DomeLight + HDRI)
  - Camera mask, reticles, grid, bbox overlays, HUD
- **Viewport Presentation** — direct native egui texture path on wgpu
  - Engine-backed color target presentation without mandatory CPU readback
  - GPU sRGB / OCIO viewport presentation path on wgpu
  - CPU fallback path retained for non-wgpu backends and explicit capture/export
- **Color Correction** — sRGB and OpenColorIO (via `vfx-ocio` crate)
  - OCIO display/view/looks selection from config
  - Builtin ACES 1.3 fallback when `$OCIO` not set
- **Prim Tree** — scene hierarchy with search, filtering by type/purpose
- **Attributes** — property inspector with time-sampled values, array browser
- **Layer Stack** — composition arc visualization
- **Timeline** — playback with scrubbing, loop modes, FPS control
- **Preferences** — camera, rendering, display settings (persisted)
- **Dockable panels** — customizable layout via egui_dock
- **File I/O** — .usd, .usda, .usdc, .usdz; drag & drop; save overrides/flattened/image

## Usage

```bash
# From repository
cargo run -p usd-view -- path/to/scene.usda

# With OCIO config
OCIO=/path/to/config.ocio cargo run -p usd-view -- scene.usd

# After cargo install
usdview scene.usda
```

## Dependencies

- `usd-rs` — Rust port of OpenUSD (sdf, usd, imaging, hydra)
- `egui` / `eframe` — immediate mode GUI
- `egui_dock` — dockable panel layout
- `vfx-ocio` — OpenColorIO color management (pure Rust)
- `wgpu` — GPU backend (primary, replaces OpenGL/Vulkan/Metal)

## Architecture

### wgpu viewport fast path

```text
Stage
 -> usd-imaging::gl::Engine
 -> Storm render targets
 -> engine-side post-FX (selection / visualizeAov / task-driven sRGB)
 -> native egui texture presentation
 -> optional viewport GPU sRGB / OCIO pass
```

### CPU fallback / export path

```text
Stage
 -> usd-imaging::gl::Engine
 -> Storm render target
 -> GPU readback
 -> CPU color correction / export processing
 -> egui texture upload or image write
```

The readback path is still important for non-wgpu backends and explicit capture/export, but it is no longer the only viewport presentation path.

## Status

Feature-complete USD viewer with Storm-on-wgpu rendering, OCIO color management,
full scene inspection, and C++ usdviewq parity (~99%).
