# HdMtlx - MaterialX Integration for Hydra

## Overview

HdMtlx provides MaterialX integration for Hydra's material system, enabling conversion between Hydra material networks and MaterialX documents. Used by `usd-hd-st`'s `materialx_filter` module to generate WGSL shaders from MaterialX node graphs via the `mtlx-rs` crate.

## Implementation Status

**Current Status: FULL C++ PARITY**

Full parity with C++ `pxr/imaging/hdMtlx/hdMtlx.cpp` (verified 2026-03-17):
- `HdMaterialNetworkInterface` trait abstracts Hydra material networks for MaterialX conversion
- `create_mtlx_document_from_hd_network_interface()` converts Hydra networks to MaterialX `Document`
- Standard library loading via `get_std_libraries()` (loads from mtlx-rs bundled libraries)
- Node definition lookup via `resolve_node_def_from_stdlib()` with SDR registry fallback for custom nodes
- USD-to-MaterialX type mapping (`convert_to_mtlx_type`) for all 20 standard USD types
- Proper input type resolution (`get_mx_input_type`) from USD type name or NodeDef
- `colorSpace:` / `typeName:` prefix parameter filtering and colorSpace propagation
- Multi-output node handling with per-output type resolution
- Swizzle backward compat (synthetic NodeDef for MX >= 1.39)
- Document version setting from `mtlx:version` config (default `"1.38"`)
- Texture node detection via `filename`-type inputs from NodeDef
- Primvar/texcoord node tracking (exact `geompropvalue` match + `_UsesTexcoordNode`)
- Integration with `materialx_filter.rs` in usd-hd-st: network -> MX Document -> NagaWgslShaderGenerator -> WGSL

## Module Structure

### Files

- **lib.rs** - Module exports
- **tokens.rs** - MaterialX shader terminal tokens (Surface, Displacement)
- **debug_codes.rs** - Debug codes for MaterialX operations
- **types.rs** - Core types (`HdMtlxTexturePrimvarData`) for texture/primvar tracking
- **conversion.rs** - Full conversion pipeline: Hydra network -> MaterialX Document
- **network_interface.rs** - `HdMaterialNetworkInterface` trait + adapter types

### Key Types

#### `HdMtlxTexturePrimvarData`

Stores the mapping between MaterialX and Hydra texture/primvar nodes:
- `mx_hd_texture_map` - MaterialX to Hydra texture name mapping (node name -> set of input names)
- `hd_texture_nodes` - Paths to Hydra texture nodes (detected via `filename`-type inputs)
- `hd_primvar_nodes` - Paths to Hydra primvar nodes (geompropvalue + texcoord nodes)

#### `HdMtlxDebugCode`

Debug codes for MaterialX operations:
- `Document` - MaterialX document operations
- `VersionUpgrade` - MaterialX version upgrade
- `WriteDocument` - Document writing (with includes)
- `WriteDocumentWithoutIncludes` - Document writing (without includes)

### Tokens

- `SURFACE_SHADER_NAME` - "Surface"
- `DISPLACEMENT_SHADER_NAME` - "Displacement"
- `VOLUME_SHADER_NAME` - "Volume"
- `SURFACE_SHADER_TYPE` / `DISPLACEMENT_SHADER_TYPE` / `VOLUME_SHADER_TYPE`
- `STANDARD_SURFACE` / `USD_PREVIEW_SURFACE` / `GLTF_PBR` - Common NodeDef names
- `FILE_INPUT` / `TEXCOORD_INPUT` / `DEFAULT_GEOMPROP`

## Key API

### `HdMaterialNetworkInterface` (trait)

Abstracts access to Hydra material networks for MaterialX conversion.
Implemented by `Net2Adapter` in `usd-hd-st/materialx_filter.rs`.

### `create_mtlx_document_from_hd_network_interface()`

Converts a Hydra material network into a MaterialX `Document`:
1. Creates MaterialX Document, imports standard libraries
2. Sets document version from `mtlx:version` config (default `"1.38"`)
3. Resolves terminal NodeDef via `resolve_node_def_from_stdlib()` (stdlib -> SDR registry -> swizzle compat)
4. Adds terminal shader node with `get_mx_node_string()` (namespace-aware)
5. Builds NodeGraph from upstream connections with proper inter-node wiring
6. Handles multi-output nodes (per-output type from NodeDef)
7. Tracks texture/primvar nodes via NodeDef input types
8. Adds material node, upgrades document version, validates

### `convert_to_string(value: &VtValue) -> String`

Converts Hydra parameter values to MaterialX-compatible string format.
Supports bool, int, float, double, string, Token, Vec2f, Vec3f, Vec4f, Matrix3d, Matrix4d, SdfAssetPath.

### `convert_to_mtlx_type(usd_type: &str) -> &str`

Maps USD type names to MaterialX type strings (20 mappings, e.g. `color3f` -> `color3`, `matrix4d` -> `matrix44`).

### `get_std_libraries() -> Document`

Loads MaterialX standard libraries from search paths (env vars + auto-detect).

### `resolve_node_def_from_stdlib(hd_node_type, stdlib) -> Option<ElementPtr>`

Full NodeDef resolution mirroring C++ `_GetNodeDef`:
1. Direct stdlib lookup
2. Version-renamed lookup (e.g. `ND_normalmap` -> `ND_normalmap_float`)
3. SDR registry fallback for custom nodes with external `.mtlx` files
4. Node name matching
5. Swizzle backward compat (synthetic NodeDef for MX >= 1.39)

### `get_mx_terminal_name(terminal_type: &str) -> String`

Maps terminal types to MaterialX terminal names (`surfaceshader` -> `Surface`, `displacementshader` -> `Displacement`).

## Pipeline

```
HdMaterialNetwork2  (Hydra)
    |  Net2Adapter (implements HdMaterialNetworkInterface)
    v
create_mtlx_document_from_hd_network_interface()
    |  Walks upstream nodes, creates MX node graph
    v
mtlx_rs::core::Document  (MaterialX)
    |  find_renderable_element() -> ElementPtr
    v
NagaWgslShaderGenerator::generate()
    |  VkShaderGen (GLSL 450) -> naga -> WGSL
    v
WGSL fragment source  (Storm render pipeline)
```

## Dependencies

- `mtlx-rs` - MaterialX core library (document model, stdlib loading)
- `usd-sdf` - SdfPath
- `usd-tf` - Token
- `usd-vt` - VtValue
- `usd-gf` - GfVec/GfMatrix types
- `usd-sdr` - SdrRegistry for custom node resolution

## Testing

Run tests with:
```bash
cargo test -p usd-hd-mtlx
```

## C++ API Parity

Full functional parity with `pxr/imaging/hdMtlx/` (verified 2026-03-17):

| C++ Function | Rust Equivalent |
|---|---|
| `HdMtlxSearchPaths()` | `get_search_paths()` |
| `HdMtlxStdLibraries()` | `get_std_libraries()` |
| `HdMtlxConvertToString()` | `convert_to_string()` |
| `HdMtlxCreateNameFromPath()` | `create_name_from_path()` |
| `HdMtlxGetNodeDef()` | `get_node_def()` / `resolve_node_def_from_stdlib()` |
| `HdMtlxGetNodeDefName()` | `get_node_def_name()` |
| `HdMtlxGetMxTerminalName()` | `get_mx_terminal_name()` |
| `HdMtlxCreateMtlxDocumentFromHdNetwork()` | `create_mtlx_document_from_hd_network()` |
| `HdMtlxCreateMtlxDocumentFromHdMaterialNetworkInterface()` | `create_mtlx_document_from_hd_network_interface()` |
| `_ConvertToMtlxType()` | `convert_to_mtlx_type()` |
| `_GetMxInputType()` | `get_mx_input_type()` |
| `_GetMxNodeString()` | `get_mx_node_string()` |
| `_UsesTexcoordNode()` | `uses_texcoord_node()` |
| `_AddParameterInputs()` | `add_parameter_inputs()` |
| `_AddMaterialXNode()` | `gather_upstream_nodes()` + node creation |
| `_AddNodeInput()` | `add_node_connections()` (with multi-output) |
| `_GatherUpstreamNodes()` | `gather_upstream_nodes()` |
| `_CreateNodeGraphFromTerminalNodeConnections()` | `create_node_graph_from_terminal_connections()` |

## References

- [MaterialX Specification](https://www.materialx.org/)
- [OpenUSD hdMtlx Documentation](https://openusd.org/dev/api/hd_mtlx_page_front.html)
- [Hydra Material Networks](https://openusd.org/dev/api/hd_page_front.html)

## License

Copyright 2025 Pixar. Licensed under the terms set forth in the LICENSE.txt file available at https://openusd.org/license.
