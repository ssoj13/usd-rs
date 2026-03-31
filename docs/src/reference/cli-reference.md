# CLI Reference

The `usd` binary provides command-line tools for working with USD files.
Each subcommand mirrors a corresponding tool from the C++ OpenUSD distribution.

## Global Options

```
usd [global-options] <command> [command-options] [args...]
```

| Option | Description |
|--------|-------------|
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |
| `-v`, `--verbose` | Enable verbose logging |

## usd cat

Print a composed or flat USD stage as USDA text.

```
usd cat [options] <file>
```

Equivalent to C++ `usdcat`.

## usd tree

Print the prim hierarchy as an indented tree.

```
usd tree [options] <file>
```

Equivalent to C++ `usdtree`.

**Example output:**

```
/
+-- World [Xform]
|   +-- Ground [Mesh]
|   +-- Character [Xform]
|   |   +-- Body [Mesh]
|   |   +-- Head [Mesh]
|   +-- Light [DistantLight]
+-- Materials [Scope]
    +-- Default [Material]
```

## usd dump

Dump raw layer data including specs, fields, and values.

```
usd dump [options] <file>
```

Equivalent to C++ `usddump`. Shows the un-composed layer content -- useful for
debugging file format issues.

## usd meshdump

Dump composed mesh diagnostics for a specific prim.

```
usd meshdump <file> <primPath> [time]
```

Outputs:
- Composed mesh topology (face counts, indices)
- Vertex positions
- Transform matrix
- Bounding box
- Subdivision scheme

## usd filter

Filter prims by path pattern or type.

```
usd filter [options] <file>
```

## usd diff

Compare two USD files and show differences.

```
usd diff <file1> <file2>
```

Equivalent to C++ `usddiff`.

## usd resolve

Resolve an asset path through the ArResolver.

```
usd resolve <assetPath>
```

Equivalent to C++ `usdresolve`. Useful for debugging asset resolution issues.

## usd edit

Open a layer for programmatic editing.

```
usd edit [options] <file>
```

Equivalent to C++ `usdedit`.

## usd stitch

Stitch multiple layers into one.

```
usd stitch [options] <file1> <file2> [file3...]
```

Equivalent to C++ `usdstitch`. Combines opinions from multiple layers into a
single output layer.

## usd stitchclips

Create value clips from a set of time-sample layers.

```
usd stitchclips [options] <files...>
```

Equivalent to C++ `usdstitchclips`. Generates the clip manifest and metadata
for per-frame caching workflows.

## usd dumpcrate

Dump the raw binary structure of a USDC crate file.

```
usd dumpcrate <file.usdc>
```

Shows:
- String table
- Token table
- Path table
- Spec table
- Field sets
- Section layout

## usd compress

Re-encode a USD file between formats.

```
usd compress <input> -o <output>
```

The output format is determined by file extension:
- `.usda` → text format
- `.usdc` → binary crate format

## usd genschemafromsdr

Generate USD schema from shader definitions in the SDR registry.

```
usd genschemafromsdr [options]
```

## usd fixbrokenpixarschemas

Fix deprecated Pixar schema patterns in USD files.

```
usd fixbrokenpixarschemas <file>
```

## usd zip

Create a USDZ package from source files.

```
usd zip [options] <files...> -o <output.usdz>
```

Packages a root USD file and its dependencies (textures, references) into a
single `.usdz` archive.

## usd view

Launch the interactive 3D viewer.

```
usd view [options] <file>
usd v <file>
```

### Viewer Options

| Option | Description |
|--------|-------------|
| `-v` | Verbose logging (info level) |
| `-vv` | Debug logging |
| `-vvv` | Trace logging |

### Viewer Controls

| Action | Control |
|--------|---------|
| Orbit | Left mouse drag |
| Pan | Middle mouse drag |
| Zoom | Scroll wheel |
| Select | Left click |
| Frame selection | F key |
| Frame all | A key |
| Toggle wireframe | W key |

### Viewer Panels

- **Viewport** — 3D rendering with Storm
- **Hierarchy** — Prim tree browser with search/filter
- **Attributes** — Attribute inspector for selected prim
- **Layer Stack** — Layer composition view
- **Timeline** — Playback controls and frame scrubbing

## Environment Variables

| Variable | Description |
|----------|-------------|
| `USD_LOG` | Log level filter (e.g., `debug`, `info`, `warn`) |
| `RUST_LOG` | Fallback log level (standard Rust) |
| `USD_RESOLVER_SEARCH_PATHS` | Additional asset search paths |
