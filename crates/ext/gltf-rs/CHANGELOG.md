# gltf-rs Changelog

Changes relative to upstream [gltf-rs/gltf v1.4.1](https://github.com/gltf-rs/gltf/tree/v1.4.1).

## Workspace integration

- Package renamed `gltf` -> `gltf-rs` (lib name stays `gltf`).
- `edition`, `version`, `license`, `repository`, `rust-version` pulled from workspace.
- Own `[workspace]` section removed; `gltf-json` and `gltf-derive` are members of the parent usd-rs workspace.
- `gltf-json`: replaced `serde_derive = "1.0"` with `serde = { workspace = true }` (uses `features = ["derive"]`).
- `gltf-derive`: `version`, `edition`, `license`, `repository` from workspace.

## Safety fixes

- **import.rs** `Scheme::parse`: `urlencoding::decode(uri).unwrap()` replaced with `Result`-returning `.map_err(|_| Error::UnsupportedScheme)`. The entire `parse()` signature changed from `-> Scheme` to `-> Result<Scheme>`, and `read()` propagates via `?`. Upstream panics on malformed percent-encoded URIs.
- **mesh/mod.rs**: 5 `unreachable!()` arms in `read_colors`, `read_indices`, `read_joints`, `read_tex_coords`, `read_weights` replaced with `_ => None` -- returns `None` instead of panicking on unexpected accessor types.

## Dependency changes

- Removed direct `byteorder = "1.3"` dependency (unused after edition bump; transitive via `image` stays).

## Typo fixes

- **binary.rs**: `exceeeds` -> `exceeds`, `occured` -> `occurred`.
- **gltf-json/validation.rs**: `occured` -> `occurred`.

## Added

- `examples/gen_box_sparse_glb.rs` -- generates `tests/box_sparse.glb` for roundtrip tests.
- `PLAN.md` -- internal improvement tracker (not shipped).
