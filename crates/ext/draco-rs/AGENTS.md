# AGENTS.md

## Purpose
This file is the maintainer and agent knowledge base for `draco-rs`.

Use it when you need to resume work on this crate after context loss, audit parity against `_ref/draco`, or make changes without regressing the reference-aligned behavior already established in this workspace.

## Scope
- Workspace root: `C:\projects\projects.rust.cg\usd-rs\crates\ext\draco-rs`
- Reference codebase: `C:\projects\projects.rust.cg\usd-rs\_ref\draco`
- Main consolidated audit ledger: `DRACO_PLAN.md`
- Diagrams: `DIAGRAMS.md`

## Current State
- The requested parity audit for `draco-rs` is complete for the reviewed scope.
- The current workspace snapshot has no open confirmed critical, high, or medium defects in the audited areas.
- Validation is green.
- Latest verified result:
  - `253 passed`
  - `0 failed`
  - `1 ignored`

## Crate Map

```text
draco-rs/
  Cargo.toml               root crate
  README.md                user-facing crate overview
  AGENTS.md                this file
  DRACO_PLAN.md            audit ledger and closure summary
  DIAGRAMS.md              mermaid architecture diagrams
  src/
    io/                    file IO, glTF, scene IO
    scene/                 scene graph and scene helpers
    animation/             animation data model and helpers
    tools/                 transcoder support
    javascript/            JS wrapper module
    maya/                  Maya wrapper module
    unity/                 Unity wrapper module
  crates/
    draco-core/            mesh, point cloud, attributes, metadata, material, texture
    draco-bitstream/       encoder/decoder implementation
    draco-cli/             draco binary: encoder / decoder / transcoder
    draco-js/              wasm-bindgen bindings
    draco-maya/            Maya-facing C ABI
    draco-unity/           Unity-facing C ABI
    draco-fuzz/            fuzz support
  fuzz/                    corpus runners and fuzz targets
  scripts/                 validation and parity helpers
  test/                    fixtures
```

## Public Surface Summary
Root crate exports:
- `attributes`
- `compression`
- `core`
- `mesh`
- `metadata`
- `point_cloud`
- `io`
- `scene`
- `animation`
- `material`
- `texture`
- `tools`
- `javascript`
- `maya`
- `unity`

## Architecture Overview

### Layering
```text
Application layer
  src/io + src/scene + src/animation + src/tools
        |
Binding layer
  crates/draco-js + crates/draco-maya + crates/draco-unity
        |
Codec layer
  crates/draco-bitstream
        |
Core data model
  crates/draco-core
```

### Decode path
```text
.drc bytes
  -> DecoderBuffer
  -> draco-bitstream::compression::decode
  -> connectivity reconstruction + attribute decoding
  -> draco-core::{Mesh | PointCloud}
  -> app IO / glTF / bindings
```

### glTF path
```text
.gltf / .glb
  -> src/io/gltf_decoder.rs
  -> Scene / Mesh / PointCloud / materials / textures / animations / metadata
  -> src/io/gltf_encoder.rs
  -> JSON + BIN / GLB
```

## Reference-Aligned Invariants
These are the rules most likely to be broken by well-meaning cleanup. Do not change them without re-checking `_ref/draco`.

1. Raw `.drc` mesh decode is direct.
- Rust must return the decoded mesh without an OBJ/PLY-style post-decode deduplication pass.
- Reference evidence: `_ref/draco/src/draco/io/mesh_io.cc:100-107`.

2. OBJ and PLY decode are not equivalent to raw `.drc` decode.
- OBJ decode performs post-decode deduplication.
- PLY decode performs post-decode deduplication.
- Reference evidence:
  - `_ref/draco/src/draco/io/obj_decoder.cc:272-279`
  - `_ref/draco/src/draco/io/ply_decoder.cc:80-89`

3. `TriangleSoupMeshBuilder::Finalize()` deduplicates.
- This is a separate path and must stay distinct from raw `.drc` decode semantics.
- Reference evidence: `_ref/draco/src/draco/mesh/triangle_soup_mesh_builder.cc:80-90`.

4. Point-id equality is defined by mapped attribute ids.
- Do not reintroduce ad hoc dedup paths for `PointCloud` and `Mesh`.
- Both must use the same equality rule through a singular code path.
- Current shared implementation:
  - `crates/draco-core/src/point_cloud/point_cloud.rs`
  - `build_point_deduplication_map(...)`

5. Metadata names and string payloads are raw bytes in the core API.
- Do not silently force UTF-8 inside core metadata structures.
- Lossy conversion belongs only at explicit text-facing boundaries such as JS / glTF / OBJ emitters.

6. `StatusOr<T>` must behave like an upstream value container.
- Non-OK plus value is invalid.
- Shared extraction should go through one result path.
- Public callers should not have to reason about impossible internal states.

7. FFI entry points must not panic across the ABI boundary.
- Maya and Unity decode/export paths were explicitly fixed to return errors instead of panicking.
- If you add new `extern "C"` entry points, audit for `unwrap()` / `expect()` before shipping.

8. Exported Unity attribute handles own snapshots.
- They must not borrow mesh-owned attribute storage whose lifetime can end earlier than the handle.

9. Material id reads must be stride-aware.
- Do not use fixed-width raw reads for preserved material attributes.
- Use conversion helpers that respect attribute storage width.

10. `Scene::remove_material()` must validate before mutating.
- Do not reintroduce partial mutation before reference checks complete.

## Hot Files
These files carry the highest parity and regression risk.

### Core storage and attributes
- `crates/draco-core/src/core/data_buffer.rs`
- `crates/draco-core/src/attributes/geometry_attribute.rs`
- `crates/draco-core/src/attributes/point_attribute.rs`
- `crates/draco-core/src/core/status_or.rs`
- `crates/draco-core/src/core/options.rs`

### Point-cloud and mesh deduplication
- `crates/draco-core/src/point_cloud/point_cloud.rs`
- `crates/draco-core/src/point_cloud/mod.rs`
- `crates/draco-core/src/mesh/mesh.rs`
- `crates/draco-core/src/mesh/triangle_soup_mesh_builder.rs`
- `src/mesh_tests.rs`

### IO and glTF
- `src/io/mesh_io.rs`
- `src/io/point_cloud_io.rs`
- `src/io/gltf_decoder.rs`
- `src/io/gltf_encoder.rs`
- `src/io/scene_io.rs`

### Bindings
- `crates/draco-maya/src/lib.rs`
- `crates/draco-unity/src/lib.rs`
- `crates/draco-js/src/metadata.rs`

## Final Audit Closure Summary
The following defect classes were closed during the parity pass and should be treated as known historical risk areas.

- Raw pointer escape from borrowed attribute storage
- Misaligned typed reinterpretation over `Vec<u8>` storage
- Point-cloud encoder parent/child aliasing through back-pointers
- Wrong-width material-id reads in mesh splitting
- Rust-only post-decode mutation in raw `.drc` mesh IO
- Local/global attribute-id mix during point-cloud encoder reorder
- Stale glTF required-extension gate
- Maya / Unity FFI panic hazards
- Metadata raw-byte contract drift
- `StatusOr<T>` contract drift
- `Options` parse drift
- Point-cloud-only IO drift
- `Scene::remove_material()` partial-mutation risk
- Unity attribute lifetime hazard
- Point-id deduplication parity/performance drift

## Validation Workflow
Run from the workspace root.

Preferred commands:

```bash
cargo test -p draco-core metadata_ --lib
cargo test -p draco-rs --no-run
cargo test -p draco-rs gltf_decoder_test_ -- --test-threads=1
cargo test -p draco-rs -- --test-threads=1
```

If the broader workspace is busy, isolate the target dir:

```powershell
$env:CARGO_TARGET_DIR='C:\projects\projects.rust.cg\usd-rs\crates\ext\draco-rs\target-draco-verify'
cargo test -p draco-rs -- --test-threads=1
```

Reason:
- this workspace is often used concurrently by other agents
- a dedicated target dir avoids stale locks and misleading timeout symptoms

## File and Tooling Notes
- Build from the workspace root, not from inside nested crates unless you intentionally want a narrower scope.
- Use `DRACO_PLAN.md` for the full issue ledger and closure history.
- Use `DIAGRAMS.md` for visual codepath orientation.
- The `md/` directory contains narrower audit slices and verification reports.
- `plan1.md`, `plan2.md`, and `plan3.md` are checkpoint summaries, not the primary source of truth.

## Change Policy For Future Work
1. Prefer one shared code path over parallel Rust-only helpers.
2. Before changing semantics, check `_ref/draco` first.
3. If a Rust test disagrees with the reference, verify whether the test is wrong before touching production code.
4. Keep byte-level behavior explicit in metadata, attribute buffers, and FFI handoff paths.
5. Document any newly verified divergence immediately in `DRACO_PLAN.md` and this file.

## Recovery Checklist
If you return after context loss:
1. Read `README.md` for the crate overview.
2. Read `DRACO_PLAN.md` for the consolidated audit outcome.
3. Read this file for invariants and hot files.
4. If working on glTF or runtime parity, re-check the `.drc` vs OBJ/PLY dedup distinction before making changes.
5. Use the isolated `CARGO_TARGET_DIR` if test runs look blocked by workspace activity.
