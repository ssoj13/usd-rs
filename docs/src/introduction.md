# Introduction

**usd-rs** is a pure Rust, ground-up rewrite of Pixar's
[OpenUSD](https://openusd.org) (Universal Scene Description). It is *not* a
binding layer around the C++ library -- every module has been re-implemented in
idiomatic Rust, providing memory safety, fearless concurrency, and a modern
developer experience while preserving behavioral parity with the C++ reference.

## What is OpenUSD?

Universal Scene Description is an open-source framework for interchange of 3D
computer graphics data. Originally developed by Pixar Animation Studios, it is
the industry standard for:

- **Scene composition** -- assembling complex scenes from modular, layered
  descriptions using references, payloads, inherits, specializes, and variant
  sets.
- **Schema-driven data model** -- strongly typed prims (primitives) with
  well-defined attribute schemas for geometry, materials, lighting, cameras,
  skeletons, volumes, and more.
- **Hydra rendering architecture** -- a pluggable render abstraction that
  decouples scene description from rendering backends (Storm, RenderMan,
  third-party renderers).

## Why Rust?

| Concern | C++ OpenUSD | usd-rs |
|---------|-------------|--------|
| Memory safety | Manual (`new`/`delete`, raw pointers) | Ownership + borrowing, `Arc`/`Mutex` |
| Thread safety | TBB + manual locking | `Send`/`Sync` by construction, Rayon |
| Build system | CMake + Boost + Python | Cargo, zero external C/C++ deps |
| Error handling | Exceptions + `TF_CODING_ERROR` | `Result<T, E>` / `?` propagation |
| Package management | Manual vendoring | crates.io ecosystem |
| GPU backend | OpenGL (Storm) | wgpu (Vulkan / Metal / DX12 / WebGPU) |

## Project Status

usd-rs is under active development. The core composition engine (SDF, PCP, USD),
the full schema suite, and the Hydra imaging pipeline are implemented.
A built-in viewer (`usd view`) provides interactive 3D viewport with the Storm
render delegate running on wgpu.

Current focus areas:

- Functional parity with the C++ reference on real-world production files
- Performance optimization (USDC parsing, Hydra sync, GPU draw batching)
- Instancing, skeletal animation, and advanced material networks
- Cross-platform support (Windows, Linux, macOS)

## How This Book is Organized

| Section | Audience | Content |
|---------|----------|---------|
| **Getting Started** | Everyone | Installation, quick start, CLI tools |
| **User Guide** | Library consumers | Stage/Layer/Prim API, composition, schemas, materials, animation |
| **Developer Guide** | Contributors | Architecture, crate map, composition engine internals, Hydra pipeline |
| **Reference** | Everyone | Per-crate reference, CLI reference, C++ mapping |

## License

usd-rs is dual-licensed under MIT and Apache-2.0. See
[LICENSE](https://github.com/vfx-rs/usd-rs/blob/main/LICENSE) for details.
