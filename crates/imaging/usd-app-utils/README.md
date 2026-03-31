# UsdAppUtils - USD Application Utilities

Port of OpenUSD `pxr/usdImaging/usdAppUtils`

## Overview

UsdAppUtils provides utilities and common functionality for applications that view and/or record images of USD stages. This module bridges USD scene description with Hydra rendering.

## Modules

### Camera Utilities (`camera.rs`)

Utilities for finding and working with cameras on USD stages.

**Key Functions:**
- `get_camera_at_path()` - Find camera by absolute path or name

**Example:**
```rust
use usd::imaging::usd_app_utils::get_camera_at_path;
use usd::usd::{Stage, InitialLoadSet};
use usd::sdf::Path;

let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)?;

// Get by absolute path
let camera = get_camera_at_path(&stage, &Path::from_string("/cameras/main")?);

// Get by name only (searches entire stage)
let camera = get_camera_at_path(&stage, &Path::from_string("main")?);
```

**Features:**
- Absolute path lookup
- Name-based search across entire stage
- Automatic path correction with warnings
- Comprehensive error handling

### Frame Recorder (`frame_recorder.rs`)

Utility for rendering USD stages to images using Hydra.

**Key Types:**
- `FrameRecorder` - Main recorder struct
- `FrameRecorderBuilder` - Fluent builder for configuration

**Example:**
```rust
use usd::imaging::usd_app_utils::FrameRecorder;
use usd::usd::{Stage, InitialLoadSet, TimeCode};
use usd::schema::geom::Camera;
use usd::sdf::Path;

let stage = Stage::open("scene.usda", InitialLoadSet::LoadAll)?;
let camera = Camera::get(&stage, &Path::from_string("/cameras/main")?);

let recorder = FrameRecorder::builder()
    .image_width(1920)
    .complexity(2.0)
    .camera_light_enabled(true)
    .gpu_enabled(true)
    .build()?;

recorder.record(&stage, &camera, TimeCode::default(), "output.png")?;
```

**Configurable Properties:**
- Image width (default: 960px)
- Refinement complexity (default: 1.0)
- Renderer plugin selection
- GPU enabled/disabled
- Camera light (headlight) on/off
- Dome light visibility
- Included purposes for rendering
- Color correction mode
- Render pass/settings prim paths

## Implementation Status

### Completed ✓
- Camera utilities with full path resolution
- FrameRecorder API with builder pattern
- Configuration management
- Unit tests (10 tests)
- API parity with C++ OpenUSD

### Pending ⏳
- Full `FrameRecorder::record()` implementation
- Requires complete Hydra imaging engine
- Requires HGI backend (OpenGL/Vulkan/Metal)

## C++ API Mapping

| C++ | Rust |
|-----|------|
| `UsdAppUtilsGetCameraAtPath()` | `get_camera_at_path()` |
| `UsdAppUtilsFrameRecorder` | `FrameRecorder` |
| `SetImageWidth()` | `set_image_width()` / builder |
| `SetComplexity()` | `set_complexity()` / builder |
| `SetRendererPlugin()` | `set_renderer_plugin()` / builder |
| `Record()` | `record()` |

## Testing

Run tests:
```bash
cargo test imaging::usd_app_utils
```

Current test coverage:
- Camera path resolution (5 tests)
- Frame recorder configuration (5 tests)

## Dependencies

- `crate::sdf` - USD path and layer system
- `crate::usd` - USD stage and time code
- `crate::schema::geom` - USD geometry schemas
- `crate::tf` - Token and diagnostic systems
- `anyhow` - Error handling

## Future Work

1. Complete Hydra imaging engine integration
2. Implement HGI backends
3. Add render settings support
4. Add render products support
5. Add color management pipeline
6. Add AOV (arbitrary output variable) support
7. Integration tests with actual rendering

## References

- C++ Source: `pxr/usdImaging/usdAppUtils/`
- OpenUSD Docs: https://openusd.org/release/api/usd_app_utils_page_front.html
