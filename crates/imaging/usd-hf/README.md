# HF (Hydra Foundation) Module

Plugin system foundation for Hydra render delegates and other extensible components.

## Architecture

The HF module provides the base plugin infrastructure used throughout Hydra:

```
HfPluginBase (trait)
    ↓
HfPluginDesc (metadata)
    ↓
HfPluginEntry (lifecycle management)
    ↓
HfPluginRegistry (discovery & access)
```

## Key Features

- **Type-safe plugin system** - TypeId-based identification with Any trait downcasting
- **Ref counting** - Automatic lifecycle management via Arc<RwLock<>>
- **Priority ordering** - Plugins sorted by priority for default selection
- **Thread-safe** - All operations are thread-safe via RwLock
- **Extensible** - Base for render delegates, image handlers, etc.

## Usage Example

```rust
use usd::imaging::hf::{HfPluginBase, HfPluginRegistryImpl};
use std::any::Any;

// 1. Define your plugin type
struct MyRenderDelegate {
    name: String,
}

// 2. Implement HfPluginBase
impl HfPluginBase for MyRenderDelegate {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
    
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

// 3. Create registry and register plugin
let registry = HfPluginRegistryImpl::new();

let id = registry.register::<MyRenderDelegate>(
    "My Render Delegate",  // Display name
    100,                    // Priority (higher = preferred)
    Box::new(|| {          // Factory function
        Box::new(MyRenderDelegate { 
            name: "MyDelegate".into() 
        })
    }),
);

// 4. Get plugin instance
let plugin_lock = registry.get_plugin(&id).unwrap();
let plugin_guard = plugin_lock.read().unwrap();
let plugin = plugin_guard.as_ref().unwrap();

// 5. Downcast to concrete type
let concrete = plugin.as_any().downcast_ref::<MyRenderDelegate>().unwrap();
println!("Plugin name: {}", concrete.name);

// 6. Release when done
drop(plugin_guard);
drop(plugin_lock);
registry.release_plugin(&id);
```

## Components

### HfPluginBase

Base trait for all Hydra plugins. Provides:
- `type_name()` - Runtime type identification
- `as_any()` - Downcasting support
- Thread safety (Send + Sync)

### HfPluginDesc

Plugin descriptor with metadata:
- `id: Token` - Unique plugin identifier
- `display_name: String` - Human-readable name
- `priority: i32` - Ordering priority

Implements `Ord` for automatic sorting by priority (descending) then name (ascending).

### HfPluginEntry

Internal plugin lifecycle manager:
- Lazy instantiation via factory function
- Reference counting for automatic cleanup
- Thread-safe instance access via Arc<RwLock<>>

### HfPluginRegistry

Plugin registry trait with concrete implementation `HfPluginRegistryImpl`:
- `register<T>()` - Register plugin type
- `get_plugin()` - Get/create instance (increments ref count)
- `release_plugin()` - Release instance (decrements ref count)
- `get_plugin_descs()` - List all registered plugins
- `is_registered()` - Check if plugin exists

## Performance Macros

No-op macros for compatibility with C++ codebase:
- `hf_malloc_tag_function!()` - Function-level memory tagging
- `hf_malloc_tag!(tag)` - Named memory tagging
- `hf_trace_function_scope!(tag)` - Function tracing

For actual profiling, use standard Rust tools:
- `cargo-flamegraph` - Flamegraphs
- `perf` / `instruments` - System profilers
- `tracing` crate - Structured logging

## Diagnostic Macros

- `hf_validation_warn!(id, fmt, args...)` - Validation warnings

Example:
```rust
let path = SdfPath::new("/World/InvalidPrim");
hf_validation_warn!(path, "Missing required attribute: {}", "points");
```

## Implementation Notes

### Differences from C++ Version

1. **No TfType** - Uses `std::any::TypeId` instead
2. **No PlugRegistry** - Manual registration instead of JSON discovery
3. **Trait objects** - Instead of class inheritance
4. **Arc<RwLock<>>** - Instead of raw pointers and manual ref counting
5. **No-op macros** - Performance macros compile to nothing

### Thread Safety

All registry operations are thread-safe:
- Plugin registration uses write locks
- Plugin access uses read locks
- Instance creation is atomic via ref counting

### Memory Management

Plugins are ref-counted:
- First `get_plugin()` creates instance
- Each `get_plugin()` increments count
- Each `release_plugin()` decrements count
- Zero count destroys instance

## Testing

The module includes 31 unit tests covering:
- Plugin base trait functionality
- Plugin descriptor ordering
- Plugin entry lifecycle
- Registry operations
- Ref counting behavior
- Downcasting
- Thread safety (implicit via Arc<RwLock<>>)

Run tests:
```bash
cargo test --lib imaging::hf
```

## Future Enhancements

Potential improvements:
- JSON-based plugin discovery (via serde)
- Weak references for non-owning access
- Plugin dependencies/requirements
- Plugin versioning
- Hot reloading support
- Async plugin loading
- Plugin metadata querying
