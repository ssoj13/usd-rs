# External Library Ports

Independent pure Rust ports of third-party libraries used by OpenUSD. Each crate is a standalone rewrite — no C/C++ dependencies, no FFI.

| Crate | Source | Lines | Description |
|-------|--------|------:|-------------|
| **opensubdiv-rs** | [OpenSubdiv 3.7](https://github.com/PixarAnimationStudios/OpenSubdiv) | 40k | Subdivision surfaces: Catmull-Clark, Loop, Bilinear. Far topology refiner, patch tables, stencil tables. |
| **mtlx-rs** | [MaterialX](https://github.com/AcademySoftwareFoundation/MaterialX) | 55k | Material definitions, node graphs, WGSL shader generation via naga. |
| **osl-rs** | [Open Shading Language](https://github.com/AcademySoftwareFoundation/OpenShadingLanguage) | 78k | OSL parser, AST, type system. Runtime shader execution is partial. |
| **draco-rs** | [Draco](https://github.com/google/draco) | 76k | Mesh compression/decompression. Encoder and decoder for points, meshes, point clouds. |
| **gltf-rs** | [gltf](https://crates.io/crates/gltf) | 15k | glTF 2.0 loader. Fork of the gltf crate (MIT/Apache-2.0), adapted for usd-rs integration. |
| **pxr-lz4** | [LZ4](https://github.com/lz4/lz4) | 0.6k | LZ4 block decompression for USDC crate files. |

All crates except gltf-rs are original Rust ports, not wrappers or bindings.
