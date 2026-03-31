# Stages and Layers

## Layers

A `Layer` is the atomic unit of scene description storage. Each layer
corresponds to a single file on disk (or an in-memory buffer).

### Creating Layers

```rust
use usd::sdf::Layer;

// Create a new layer backed by a file
let layer = Layer::create_new("scene.usda")?;

// Create an anonymous (in-memory) layer
let anon = Layer::create_anonymous(Some("scratch"));

// Open an existing layer
let existing = Layer::find_or_open("assets/prop.usda")?;
```

### Layer Identity

Every layer has a unique **identifier** (typically its file path) and may have
a **resolved path** (the absolute filesystem path after asset resolution).

```rust
println!("Identifier: {}", layer.identifier());
println!("Display name: {}", layer.get_display_name());
if let Some(resolved) = layer.get_resolved_path() {
    println!("Resolved: {}", resolved);
}
```

### Layer Registry

Layers are cached globally. Opening the same identifier twice returns the same
`Arc<Layer>` instance. Use `Layer::find()` to look up an already-loaded layer
without triggering I/O.

```rust
// Returns Some if already loaded, None otherwise
if let Some(layer) = Layer::find("scene.usda") {
    println!("Layer already in memory");
}
```

## Stages

A `Stage` composes one or more layers into a single resolved view.

### Opening and Creating

```rust
use usd::{Stage, InitialLoadSet};

// Open from a file path
let stage = Stage::open("scene.usda", InitialLoadSet::All)?;

// Create a new file-backed stage
let stage = Stage::create_new("output.usda", InitialLoadSet::All)?;

// Create in-memory (no file backing)
let stage = Stage::create_in_memory(InitialLoadSet::All)?;
```

### Load Policy

`InitialLoadSet` controls payload loading:

| Variant | Behavior |
|---------|----------|
| `All` | Load all payloads immediately |
| `None` | Defer payload loading (lazy) |

With `None`, payloads are not loaded until explicitly requested via load rules.
This is useful for large scenes where you only need part of the hierarchy.

### Root and Session Layers

```rust
// The root layer is the primary storage
let root = stage.get_root_layer();

// The session layer holds non-persistent overrides
// (e.g., display settings, selection highlights)
let session = stage.get_session_layer();
```

### Sublayers

Layers can include other layers via sublayering. Sublayers are composed in
order -- later sublayers are weaker than earlier ones.

```rust
// Get the sublayer paths from a layer
let sublayers = root.get_sub_layer_paths();
for path in &sublayers {
    println!("Sublayer: {}", path);
}
```

### Edit Target

The **edit target** determines which layer receives authored opinions when you
modify prims or attributes through the stage API.

```rust
use usd::sdf::Layer;

// By default, edits go to the root layer
// Switch to a different layer:
let overlay = Layer::create_anonymous(Some("overlay"));
stage.set_edit_target(&overlay);
```

### Saving

```rust
// Save the root layer to its file
stage.get_root_layer().save()?;

// Export to a different path
stage.get_root_layer().export("backup.usda")?;
```

## Stage Traversal

```rust
// Iterate all defined, active, loaded prims
for prim in stage.traverse() {
    println!("{}", prim.path());
}

// Get a prim by path
let prim = stage.get_prim_at_path(&"/World/Mesh".into());
if prim.is_valid() {
    println!("Found: {}", prim.type_name());
}

// Get the pseudo-root (parent of all top-level prims)
let root = stage.get_pseudo_root();
```
