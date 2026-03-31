# gltf-rs

glTF 2.0 loader for Rust. Part of the usd-rs / vfx-rs ecosystem.

This crate is a fork of [gltf-rs/gltf](https://github.com/gltf-rs/gltf), adapted for use in usd-rs.

## Requirements

`rustc` version 1.85 or above (edition 2024).

## Reference infographic

![infographic](https://raw.githubusercontent.com/KhronosGroup/glTF/main/specification/2.0/figures/gltfOverview-2.0.0d.png)

<p align="center">From <a href="https://github.com/javagl/gltfOverview">javagl/gltfOverview</a></p>
<p align="center"><a href="https://www.khronos.org/files/gltf20-reference-guide.pdf">PDF version</a></p>

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies.gltf-rs]
path = "path/to/crates/gltf-rs"
# or
# gltf-rs = { git = "..." }
```

The library exposes the `gltf` crate name for API compatibility:

```rust
use gltf::Gltf;

let gltf = Gltf::open("model.gltf")?;
for scene in gltf.scenes() {
    for node in scene.nodes() {
        println!("Node: {:?}", node.name());
    }
}
```

## Features

### Extras and names

By default, `gltf-rs` ignores `extras` and `names`. Enable them:

```toml
[dependencies.gltf-rs]
features = ["extras", "names"]
```

### glTF extensions

Supported extensions (enable via features):

- `KHR_lights_punctual`
- `KHR_materials_pbrSpecularGlossiness`
- `KHR_materials_unlit`
- `KHR_texture_transform`
- `KHR_materials_variants`
- `KHR_materials_volume`
- `KHR_materials_specular`
- `KHR_materials_transmission`
- `KHR_materials_ior`
- `KHR_materials_emissive_strength`
- `EXT_texture_webp`

## Examples

```sh
cargo run -p gltf-rs --example gltf-display path/to/asset.gltf
cargo run -p gltf-rs --example gltf-export
cargo run -p gltf-rs --example gltf-roundtrip path/to/asset.gltf
cargo run -p gltf-rs --example gltf-tree path/to/asset.gltf
```

## Tests

Basic tests run without extra assets:

```sh
cargo test -p gltf-rs
```

For the full `import_sample_models` and `roundtrip_binary_gltf` tests, clone the sample assets:

```sh
git clone https://github.com/KhronosGroup/glTF-Sample-Assets
cargo test -p gltf-rs --test import_sample_models -- --ignored
cargo test -p gltf-rs --test roundtrip_binary_gltf
```

## License

MIT OR Apache-2.0
