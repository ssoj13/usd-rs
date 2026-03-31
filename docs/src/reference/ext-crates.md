# External Crates

The `crates/ext/` directory contains pure Rust ports of external libraries
used by the USD ecosystem. These are standalone crates that can be used
independently.

## draco-rs

**Original:** [Google Draco](https://github.com/google/draco)

Pure Rust port of Google's Draco mesh compression library. Provides encoding
and decoding of 3D geometry data (meshes, point clouds) with significant size
reduction.

### Sub-crates

| Crate | Description |
|-------|-------------|
| `draco-core` | Core compression/decompression algorithms |
| `draco-bitstream` | Bitstream reader/writer for Draco format |
| `draco-js` | JavaScript/WASM interop utilities |
| `draco-cli` | Command-line encoding/decoding tool |
| `draco-maya` | Maya integration utilities |
| `draco-unity` | Unity integration utilities |
| `draco-fuzz` | Fuzzing harness |

### Usage

```rust
use draco_core::{Decoder, Encoder};

// Decode a Draco-compressed mesh
let decoder = Decoder::new();
let mesh = decoder.decode(&compressed_data)?;

println!("Vertices: {}", mesh.num_points());
println!("Faces: {}", mesh.num_faces());
```

## gltf-rs

**Original:** [glTF 2.0 specification](https://www.khronos.org/gltf/)

glTF 2.0 loader and data model. Parses `.gltf` (JSON) and `.glb` (binary)
files into a strongly typed Rust data structure.

### Sub-crates

| Crate | Description |
|-------|-------------|
| `gltf-rs` | Main loader and data model |
| `gltf-json` | JSON schema types |
| `gltf-derive` | Derive macros for glTF types |

### Usage

```rust
use gltf_crate::Gltf;

let gltf = Gltf::open("model.gltf")?;
for mesh in gltf.meshes() {
    println!("Mesh: {:?}", mesh.name());
    for primitive in mesh.primitives() {
        println!("  Mode: {:?}", primitive.mode());
    }
}
```

## mtlx-rs

**Original:** [MaterialX](https://materialx.org/)

Pure Rust port of the MaterialX library. Provides:
- MaterialX document parsing (`.mtlx` XML)
- Node graph evaluation
- Shader code generation
- Material definition management

MaterialX integration with Hydra is handled by `usd-hd-mtlx`.

## opensubdiv-rs

**Original:** [OpenSubdiv](https://graphics.pixar.com/opensubdiv/)

Pure Rust port of Pixar's OpenSubdiv library for subdivision surface
evaluation.

### Features

| Feature | Description |
|---------|-------------|
| Catmull-Clark | Quad subdivision scheme |
| Loop | Triangle subdivision scheme |
| Bilinear | Simple bilinear subdivision |
| Adaptive refinement | Feature-adaptive tessellation |
| Limit evaluation | Evaluate limit surface positions and normals |
| Face-varying | Per-face UV and color interpolation |
| Creases and corners | Semi-sharp and infinitely sharp features |

Used internally by `usd-px-osd` and Storm for mesh subdivision.

## osl-rs

**Original:** [OpenShadingLanguage](https://github.com/AcademySoftwareFoundation/OpenShadingLanguage)

Pure Rust port of OSL (OpenShadingLanguage). Provides:
- OSL shader parsing
- Shader compilation
- Shader node metadata extraction

Used by the shader definition registry (`usd-sdr`) for discovering OSL-based
shader nodes.

## pxr-lz4

LZ4 decompression implementation compatible with OpenUSD's `TfFastCompression`
format. Used by the USDC reader to decompress binary crate data.

This is *not* a general-purpose LZ4 implementation -- it specifically handles
the framing format used by OpenUSD's crate files, which differs slightly from
standard LZ4 framing.

```rust
use pxr_lz4::decompress;

let decompressed = decompress(&compressed_data, expected_size)?;
```
