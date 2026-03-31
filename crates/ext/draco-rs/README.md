# draco-rs

Pure Rust port of the Google Draco geometry compression library.

This workspace mirrors the reference implementation in `draco` and is maintained with explicit parity checks against the C++ codebase.
It supports Draco bitstreams (`.drc`), mesh and point-cloud IO, glTF/GLB scene transcoding, and binding crates for JavaScript, Maya, and Unity.

## Status
- Reference baseline: `_ref/draco`
- Audit status: full requested parity audit completed for the current `draco-rs` scope
- Validation status: green
- Latest validation result: `253 passed`, `0 failed`, `1 ignored`

## What This Workspace Contains

```text
draco-rs/
├── Cargo.toml              # root crate: draco-rs
├── src/                    # high-level IO, scene, animation, tools, wrapper modules
├── crates/
│   ├── draco-core/         # core data model: mesh, point cloud, attributes, metadata
│   ├── draco-bitstream/    # Draco encode/decode codec implementation
│   ├── draco-cli/          # CLI binary: draco encoder|decoder|transcoder
│   ├── draco-js/           # wasm-bindgen / JS bindings
│   ├── draco-maya/         # Maya-facing C ABI wrapper
│   ├── draco-unity/        # Unity-facing C ABI wrapper
│   └── draco-fuzz/         # fuzz support crate
├── fuzz/                   # corpus runners and fuzz targets
├── scripts/                # install-check, fuzz, parity helper scripts
├── test/                   # test assets
├── AGENTS.md               # maintainer/agent knowledge base for this crate
├── DIAGRAMS.md             # mermaid diagrams for core dataflows
└── DRACO_PLAN.md           # consolidated parity audit ledger
```

## Public Surface
The root crate re-exports the core Draco domains and adds higher-level application modules:

- Re-exported from `draco-core`:
  - `attributes`
  - `compression`
  - `core`
  - `mesh`
  - `metadata`
  - `point_cloud`
- High-level modules in `src/`:
  - `io`
  - `scene`
  - `animation`
  - `material`
  - `texture`
  - `tools`
  - `javascript`
  - `maya`
  - `unity`

## Supported Workflows

### Draco bitstream
- Encode mesh or point cloud to `.drc`
- Decode `.drc` back to mesh or point-cloud representations
- Expert encoder surface for fine-grained compression control

### Mesh and point-cloud IO
- Mesh IO:
  - `.drc`
  - `.obj`
  - `.ply`
  - `.stl`
- Point-cloud IO:
  - `.drc`
  - `.obj`
  - `.ply`

### glTF / GLB
- Decode glTF/GLB scenes into the Rust `Scene` model
- Encode `Scene` back to glTF/GLB
- Support Draco-compressed glTF meshes
- Support a reviewed set of glTF extensions already wired into the decoder and encoder paths

### Bindings
- `draco-js`: JavaScript / WASM surface
- `draco-maya`: Maya-oriented C ABI layer
- `draco-unity`: Unity-oriented C ABI layer

## Reference-Aligned Semantics
These points are important if you compare results with `_ref/draco`.

1. Raw `.drc` mesh decode returns the decoded mesh directly.
   - It does not run the OBJ/PLY-style post-decode deduplication pass.
2. OBJ and PLY decode do run post-decode deduplication.
3. `TriangleSoupMeshBuilder::Finalize()` also deduplicates before returning the mesh.
4. Metadata names and metadata string payloads preserve raw bytes in the core API.
5. Exported binding entry points do not intentionally panic across the C ABI boundary.

If a test expectation disagrees with the reference on one of these rules, the test should be fixed rather than forcing Rust-only behavior.

## Build
Run from the workspace root.

```bash
cargo build -p draco-rs --release
cargo build -p draco-cli --release
```

## Test
Recommended validation commands:

```bash
cargo test -p draco-core metadata_ --lib
cargo test -p draco-rs --no-run
cargo test -p draco-rs gltf_decoder_test_ -- --test-threads=1
cargo test -p draco-rs -- --test-threads=1
```

If other workspace jobs are running, isolate this crate's build artifacts:

```powershell
$env:CARGO_TARGET_DIR='C:\projects\projects.rust.cg\usd-rs\crates\ext\draco-rs\target-draco-verify'
cargo test -p draco-rs -- --test-threads=1
```

## CLI
The `draco-cli` crate builds a single `draco` binary with three subcommands:

```bash
cargo run -p draco-cli -- encoder -i input.obj -o output.drc
cargo run -p draco-cli -- decoder -i input.drc -o output.obj
cargo run -p draco-cli -- transcoder -i scene.glb -o scene_draco.glb
```

Subcommands:
- `encoder`: mesh or point cloud to `.drc`
- `decoder`: `.drc` to `.obj`, `.ply`, or `.stl`
- `transcoder`: glTF/GLB scene transcoding with Draco compression options

## Fuzzing and Utility Scripts
- `scripts/install_check.ps1`
- `scripts/fuzz_run.ps1`
- `scripts/fuzz_run.py`
- `scripts/ref_rust_roundtrip.ps1`

The `fuzz/` directory contains corpus runners and fuzz targets for mesh and point-cloud decode paths.

## Project Documents
- [AGENTS.md](AGENTS.md): crate-level maintainer guide, invariants, codepaths, and parity notes
- [DIAGRAMS.md](DIAGRAMS.md): mermaid diagrams for the main dataflows
- [DRACO_PLAN.md](DRACO_PLAN.md): consolidated audit ledger and closure notes

## License
Apache-2.0
