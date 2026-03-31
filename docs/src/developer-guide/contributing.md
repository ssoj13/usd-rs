# Contributing

This guide covers how to set up a development environment, navigate the
codebase, and contribute to usd-rs.

## Development Setup

### Prerequisites

- Rust 1.85+ (install via [rustup](https://rustup.rs/))
- Git
- A GPU driver supporting Vulkan, Metal, or DX12 (for viewer/imaging work)

### Clone and Build

```bash
git clone https://github.com/vfx-rs/usd-rs.git
cd usd-rs
cargo build
```

### Quick Validation

```bash
# Check specific crates (faster than full build)
cargo check --quiet -p usd-view
cargo check --quiet -p usd-imaging --lib
cargo check --quiet -p usd-hd-st --lib

# Full workspace check
cargo check --workspace
```

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p usd-sdf
cargo test -p usd-core
cargo test -p usd-pcp
```

## Project Layout

```
usd-rs/
+-- Cargo.toml          # Workspace root + facade crate
+-- src/
|   +-- lib.rs           # Facade crate (re-exports all sub-crates)
|   +-- bin/usd/         # CLI binary (cat, tree, view, etc.)
+-- crates/
|   +-- base/            # Foundation crates (tf, gf, vt, arch, ...)
|   +-- usd/             # Core USD + schema crates
|   +-- imaging/          # Hydra, Storm, HGI, scene index plugins
|   +-- usd-imaging/     # USD-to-Hydra bridge
|   +-- usd-validation/  # Validation framework
|   +-- usd-view/        # Interactive viewer (egui)
|   +-- ext/             # External libraries (draco, gltf, mtlx, osd, osl)
+-- data/                # Test data and sample USD files
+-- examples/            # Example programs
+-- tests/               # Integration tests
+-- docs/                # This book (mdbook)
+-- vendor/              # Vendored dependencies (patched)
```

## C++ Reference

The C++ OpenUSD source is the behavioral target. It lives at
`../usd-refs/OpenUSD/` (sibling directory) or can be cloned from
`git@github.com:ssoj13/usd-refs.git`.

When implementing or fixing behavior, always verify against the C++ reference
for correctness.

## Coding Guidelines

### Error Handling

- Use `Result<T, E>` and `?` propagation, never `unwrap()` or `expect()` in
  library code
- Never silently discard errors with `let _ =`
- Propagate errors to the caller; use `log::error!()` only when swallowing
  is intentional and documented

### Naming

- Use full words for variable names (no abbreviations like `q` for `queue`)
- Function names should be concise but descriptive: `get_xform`, not
  `extract_xform_matrix_from_prim`
- Follow C++ OpenUSD naming for public API (e.g., `get_prim_at_path`,
  `get_attribute`, `get_time_samples`)

### Style

- Prefer `Arc<T>` for shared ownership
- Use variable shadowing to scope clones in async contexts
- Comments explain *why*, not *what* -- avoid restating the code
- No doc comments on trivially obvious functions

### Safety

- No `unsafe` in core USD code; document and isolate any `unsafe` in imaging
- Avoid panicking operations: bounds-check before indexing, use `get()` instead
  of `[]` where fallibility is possible

### Organization

- Prefer adding to existing files over creating many small files
- One module per logical C++ header equivalent
- Keep schema crate structure parallel to C++ module layout

## Pull Request Workflow

1. Create a feature branch from `main`
2. Make focused commits with descriptive messages
3. Ensure `cargo check --workspace` passes with zero warnings
4. Ensure `cargo test --workspace` passes
5. Open a PR with a description of what changed and why
6. Reference the C++ behavior you're matching, if applicable

## Parity Checking

The project maintains parity with C++ OpenUSD through systematic checking:
- Function-by-function comparison against the C++ reference
- Runtime testing with real-world USD files
- Automated parity reports (see `md/` directory for reports)

When fixing a parity issue:
1. Identify the C++ reference behavior
2. Read the C++ implementation
3. Implement the fix in Rust
4. Verify with a test case or real file
5. Document the fix in the commit message
