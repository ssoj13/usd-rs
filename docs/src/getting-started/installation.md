# Installation

## Requirements

- **Rust 1.85+** (edition 2024)
- A GPU driver supporting Vulkan, Metal, or DX12 (for the viewer / Storm renderer)

No C/C++ toolchain is required -- usd-rs is a pure Rust project with zero native
dependencies.

## Adding usd-rs as a Dependency

Add the top-level facade crate to your `Cargo.toml`:

```toml
[dependencies]
usd-rs = { git = "https://github.com/vfx-rs/usd-rs.git", branch = "main" }
```

Or depend on individual sub-crates for finer control:

```toml
[dependencies]
usd-sdf = { git = "https://github.com/vfx-rs/usd-rs.git", branch = "main" }
usd-core = { git = "https://github.com/vfx-rs/usd-rs.git", branch = "main" }
usd-geom = { git = "https://github.com/vfx-rs/usd-rs.git", branch = "main" }
```

## Building the CLI

Clone and build the `usd` binary, which provides all CLI tools (`cat`, `tree`,
`view`, etc.):

```bash
git clone https://github.com/vfx-rs/usd-rs.git
cd usd-rs
cargo build --release
```

The binary is produced at `target/release/usd` (or `usd.exe` on Windows).

## Building Without the Viewer

If you only need the core USD library without the GPU-dependent viewer:

```bash
cargo build --release --no-default-features
```

This disables the `wgpu` feature and skips all GPU/imaging crates.

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `wgpu` | yes | Enable wgpu-based Storm renderer and viewer |
| `mtlx-rs` | no | Enable MaterialX material network support |
| `nightly` | no | Enable nightly Rust optimizations |
| `jemalloc` | no | Use jemalloc allocator |
| `dev_build` | no | Development build with extra diagnostics |

## Verifying the Installation

```bash
# Print version
usd --version

# Parse and print a USD file
usd cat path/to/scene.usda

# Launch the interactive viewer
usd view path/to/scene.usdz
```
