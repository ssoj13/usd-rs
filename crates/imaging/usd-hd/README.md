# Hydra Core (hd)

Core infrastructure for the Hydra rendering framework.

## Overview

This module provides the foundational types and utilities that all Hydra components depend on:

- **Version tracking** - API version constants
- **Type system** - Primvar types, tuple types, dirty bits
- **Enumerations** - Interpolation, compare functions, cull styles  
- **Tokens** - Standard identifiers for prims and properties
- **Debug support** - Debug codes for conditional logging
- **Performance** - No-op instrumentation macros (use `tracing` in Rust)

Recent task-graph plumbing also relies on `hd` core traits for render-index queries from HDX tasks, including rprim enumeration and integer prim-ID lookup used by selection and picking contracts.

## Implemented Files

### Core Infrastructure
- `version.rs` - API version constants (HD_API_VERSION = 90)
- `types.rs` - HdType, HdTupleType, HdSamplerParameters, HdDirtyBits, packed vectors
- `enums.rs` - HdInterpolation, HdCullStyle, HdCompareFunction, filtering modes
- `tokens.rs` - Standard tokens (POINTS, NORMALS, TRANSFORM, rprim/sprim types)
- `debug_codes.rs` - Debug codes for TF_DEBUG-style logging
- `perf_log.rs` - Performance instrumentation macros (no-op in Rust)

## Usage

```rust
use usd_rs::imaging::hd;

// Use types
let tuple = hd::HdTupleType::new(hd::HdType::FloatVec3, 100);
println!("Size: {} bytes", tuple.size_in_bytes());

// Access tokens
println!("Points: {}", hd::tokens::POINTS.as_str());

// Configure samplers
let sampler = hd::HdSamplerParameters::default();
assert_eq!(sampler.wrap_s, hd::HdWrap::Black);

// Use enums
let interp = hd::HdInterpolation::Vertex;
println!("Interpolation: {}", interp.as_str());
```

## Test Coverage

All core types have comprehensive unit tests:
- Type size calculations
- Component count queries  
- Tuple type operations
- Packed vector conversions
- Sampler parameter defaults
- Enum inversions
- Token creation
- Debug code strings

Run tests: `cargo test --package usd-rs --lib imaging::hd`

## Next Steps

Future modules will build on this foundation:
- `rprim.rs` - Renderable primitives (meshes, curves)
- `sprim.rs` - State primitives (cameras, lights, materials)
- `bprim.rs` - Buffer primitives  
- `scene_delegate.rs` - Scene data interface
- `render_index.rs` - Scene object registry
- `render_delegate.rs` - Backend rendering interface
- `engine.rs` - Rendering execution
- `task.rs` - Rendering tasks

## Reference

Original C++ implementation: `pxr/imaging/hd/`
