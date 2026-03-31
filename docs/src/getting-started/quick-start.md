# Quick Start

This chapter walks through common usd-rs operations in Rust code.

## Opening a Stage

The `Stage` is the top-level container for scene description. It composes all
layers, references, and payloads into a single resolved view.

```rust
use usd::{Stage, InitialLoadSet, Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Open an existing file (USDA, USDC, or USDZ)
    let stage = Stage::open("scene.usda", InitialLoadSet::All)?;

    // Print the stage's root layer identifier
    println!("Root layer: {}", stage.get_root_layer().identifier());

    // Iterate all prims on the stage
    for prim in stage.traverse() {
        println!("{} ({})", prim.path(), prim.type_name());
    }

    Ok(())
}
```

## Creating a Stage from Scratch

```rust
use usd::{Stage, InitialLoadSet, Path, TimeCode};
use usd::sdf;
use usd::tf::Token;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory stage
    let stage = Stage::create_in_memory(InitialLoadSet::All)?;

    // Define a prim at /World
    let world = stage.define_prim(&Path::from("/World"), &Token::from("Xform"));

    // Define a mesh under /World
    let mesh = stage.define_prim(
        &Path::from("/World/Cube"),
        &Token::from("Mesh"),
    );

    // Export to USDA
    stage.get_root_layer().export("output.usda")?;

    println!("Wrote output.usda");
    Ok(())
}
```

## Reading Attributes

```rust
use usd::{Stage, InitialLoadSet, Path, TimeCode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = Stage::open("scene.usda", InitialLoadSet::All)?;

    // Get a specific prim
    let prim = stage.get_prim_at_path(&Path::from("/World/Cube"));
    if !prim.is_valid() {
        eprintln!("Prim not found");
        return Ok(());
    }

    // Read an attribute at the default time
    if let Some(attr) = prim.get_attribute(&"points".into()) {
        if let Some(value) = attr.get(TimeCode::default()) {
            println!("points = {:?}", value);
        }
    }

    // List all attributes
    for attr in prim.get_attributes() {
        println!("  {}: {}", attr.name(), attr.type_name());
    }

    Ok(())
}
```

## Writing Attributes

```rust
use usd::{Stage, InitialLoadSet, Path, TimeCode};
use usd::tf::Token;
use usd::vt::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = Stage::create_new("cube.usda", InitialLoadSet::All)?;

    let cube = stage.define_prim(&Path::from("/Cube"), &Token::from("Mesh"));

    // Set display name
    if let Some(attr) = cube.get_attribute(&"displayName".into()) {
        attr.set(Value::from("My Cube"), TimeCode::default());
    }

    // Save
    stage.get_root_layer().save()?;

    Ok(())
}
```

## Traversing the Prim Hierarchy

```rust
use usd::{Stage, InitialLoadSet, Prim};

fn print_tree(prim: &Prim, depth: usize) {
    let indent = "  ".repeat(depth);
    println!("{}{} [{}]", indent, prim.name(), prim.type_name());
    for child in prim.get_all_children() {
        print_tree(&child, depth + 1);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = Stage::open("scene.usda", InitialLoadSet::All)?;

    // Start from the pseudo-root
    let root = stage.get_pseudo_root();
    for child in root.get_all_children() {
        print_tree(&child, 0);
    }

    Ok(())
}
```

## Working with Layers

```rust
use usd::sdf::Layer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new layer
    let layer = Layer::create_new("props.usda")?;

    // Open an existing layer without composition
    let layer = Layer::find_or_open("existing.usda")?;

    println!("Layer: {}", layer.identifier());
    println!("Resolved: {:?}", layer.get_resolved_path());

    Ok(())
}
```

## Next Steps

- [Stages and Layers](../user-guide/stages-and-layers.md) -- deep dive into
  the data model
- [CLI Tools](cli-tools.md) -- command-line utilities for inspecting USD files
- [Composition Arcs](../user-guide/composition.md) -- references, payloads,
  inherits, variants
