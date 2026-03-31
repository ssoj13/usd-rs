# Testing

usd-rs uses a multi-layered testing strategy: unit tests, integration tests,
doc tests, and real-file validation.

## Test Organization

| Level | Location | Purpose |
|-------|----------|---------|
| Unit tests | `#[cfg(test)] mod tests` in source files | Per-function correctness |
| Integration tests | `tests/` directories in crates | Cross-module behavior |
| Doc tests | `///` doc comments | API usage examples |
| Real-file tests | `data/` directory | End-to-end with production files |

## Running Tests

```bash
# All workspace tests
cargo test --workspace

# Specific crate
cargo test -p usd-sdf
cargo test -p usd-pcp
cargo test -p usd-core

# Specific test function
cargo test -p usd-sdf test_layer_create

# With output visible
cargo test -p usd-core -- --nocapture
```

## Test Data

Sample USD files for testing live in the `data/` directory. These include:
- Simple USDA/USDC files for parser testing
- Multi-layer compositions for PCP testing
- Animated scenes for time-sample testing
- USDZ packages for packaging testing

## Writing Tests

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_from_string() {
        let path = Path::from("/World/Mesh");
        assert_eq!(path.get_name(), "Mesh");
        assert_eq!(path.get_parent_path(), Path::from("/World"));
        assert!(path.is_absolute_path());
    }

    #[test]
    fn layer_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let layer = Layer::create_anonymous(Some("test"));
        // ... test layer operations ...
        Ok(())
    }
}
```

### Integration Test Example

```rust
// tests/composition_test.rs
use usd::{Stage, InitialLoadSet, Path};

#[test]
fn test_sublayer_composition() -> Result<(), Box<dyn std::error::Error>> {
    let stage = Stage::open("data/sublayer_test.usda", InitialLoadSet::All)?;
    let prim = stage.get_prim_at_path(&Path::from("/Root"));
    assert!(prim.is_valid());
    // Verify composed values from multiple sublayers
    Ok(())
}
```

## Test Utilities

The `test_utils` module provides helpers for testing:

```rust
use usd::test_utils;

// Run a test with a timeout to detect deadlocks
test_utils::with_timeout(Duration::from_secs(10), || {
    // test code that might deadlock
});
```

## Validation Tests

The `usd-validation` crate provides a framework for testing USD file
compliance:

- Schema validation (required attributes, correct types)
- Composition validation (resolvable references, valid arcs)
- Naming convention validation (prim names, path patterns)

## Performance Profiling

For performance-sensitive code, use the built-in tracing:

```bash
# Run with performance instrumentation
RUST_LOG=perf=info cargo run --release -- view data/scene.usdz

# Use the profile binary for parsing benchmarks
cargo run --release --bin profile_parse -- data/large_scene.usdc
```

Key instrumented areas:
- USDC read/parse/populate phases
- Mesh sync timing
- Draw item sync
- HGI command execution
- Stage composition timing
