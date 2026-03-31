# CLI Tools

usd-rs ships a unified `usd` binary that mirrors the CLI tools from the C++
OpenUSD distribution. All subcommands work with `.usda`, `.usdc`, and `.usdz`
files.

## Usage

```
usd <command> [options] [args...]
usd --help
usd --version
```

## Commands Overview

| Command | Description |
|---------|-------------|
| `cat` | Print composed or flat stage as USDA text |
| `tree` | Print the prim hierarchy as an indented tree |
| `dump` | Dump raw layer data (specs, fields, values) |
| `meshdump` | Dump composed mesh, xform, and bounds diagnostics |
| `filter` | Filter prims by path pattern or type |
| `diff` | Compare two USD files and show differences |
| `resolve` | Resolve an asset path through the ArResolver |
| `edit` | Open a layer for programmatic editing |
| `stitch` | Stitch multiple layers into one |
| `stitchclips` | Create value clips from a set of layers |
| `dumpcrate` | Dump raw USDC crate structure |
| `compress` | Re-encode a USD file (USDA <-> USDC) |
| `genschemafromsdr` | Generate schema from shader definitions |
| `fixbrokenpixarschemas` | Fix deprecated Pixar schema patterns |
| `zip` | Create a USDZ package from assets |
| `view` | Launch the interactive 3D viewer |

## Examples

### Inspecting a File

```bash
# Print the composed stage as USDA
usd cat scene.usda

# Print the prim tree
usd tree scene.usdz

# Dump raw layer specs
usd dump scene.usdc
```

### Mesh Diagnostics

```bash
# Dump mesh data for a specific prim at time=0
usd meshdump scene.usda /World/Mesh 0
```

### Converting Formats

```bash
# Convert USDA to binary USDC
usd compress scene.usda -o scene.usdc

# Package into USDZ
usd zip scene.usdc textures/ -o package.usdz
```

### Resolving Asset Paths

```bash
# Resolve a relative path through ArResolver
usd resolve ./assets/texture.png
```

### Comparing Files

```bash
# Show differences between two stages
usd diff scene_v1.usda scene_v2.usda
```

### Interactive Viewer

```bash
# Launch the viewer (requires wgpu feature)
usd view scene.usdz

# Short form
usd v scene.usdz

# Verbose logging
usd view -v scene.usdz
```

The viewer provides:
- Orbit / pan / zoom camera controls
- Prim hierarchy browser
- Attribute inspector
- Timeline scrubbing for animated scenes
- Multiple renderer settings and AOV display
- HUD with performance statistics

## Verbose Mode

Most commands accept `-v` / `--verbose` for detailed logging. The viewer
supports additional verbosity levels (`-vv`, `-vvv`).

Logging can also be controlled via environment variables:

```bash
USD_LOG=debug usd cat scene.usda
RUST_LOG=info usd tree scene.usdz
```
