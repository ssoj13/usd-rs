# mtlx-rs

Pure Rust port of [MaterialX](https://materialx.org/) — an open standard for representing materials and look-development content across DCC tools and renderers.

Ported from the C++ [MaterialX SDK](https://github.com/AcademySoftwareFoundation/MaterialX) as part of the [usd-rs](../../) project.

## Modules

| Module | C++ equivalent | Description |
|--------|---------------|-------------|
| `core` | MaterialXCore | Document, elements, nodes, types, traversal |
| `format` | MaterialXFormat | XML I/O, file search, environment utilities |
| `gen_shader` | MaterialXGenShader | Shader generation infrastructure, graph, type system |
| `gen_glsl` | MaterialXGenGlsl | GLSL / ESSL / Vulkan GLSL / WGSL shader generators |
| `gen_osl` | MaterialXGenOsl | OSL shader generation |
| `gen_msl` | MaterialXGenMsl | Metal Shading Language generation |
| `gen_mdl` | MaterialXGenMdl | MDL (NVIDIA) shader generation |
| `gen_slang` | MaterialXGenSlang | Slang shader generation |
| `gen_hw` | MaterialXGenHw | Hardware shading utilities (lights, transforms, resources) |

## Features

- **`ocio`** (default) — OpenColorIO integration via `vfx-ocio`
- **`wgsl-native`** — native WGSL output via `naga` transpilation (GLSL 450 -> WGSL)

## Standard libraries

The `libraries/` directory contains the MaterialX standard definition libraries (stdlib, pbrlib, bxdf, lights, cmlib, nprlib, targets).

## Usage

```toml
[dependencies]
mtlx-rs = { path = "crates/ext/mtlx-rs" }
```

```rust
use mtlx_rs::core::document::Document;
use mtlx_rs::format::xml_io;

let doc = Document::new();
xml_io::read_from_file(&doc, "material.mtlx", &Default::default())?;
```

## License

See the top-level [LICENSE](../../LICENSE) file.
