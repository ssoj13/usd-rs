# Prims and Properties

## Prims

A `Prim` is a named node in the scene hierarchy. Every prim has a path, a type
name, a specifier, and zero or more properties and children.

### Defining Prims

```rust
use usd::{Stage, InitialLoadSet, Path};
use usd::tf::Token;

let stage = Stage::create_in_memory(InitialLoadSet::All)?;

// Define creates the prim and all ancestors as needed
let world = stage.define_prim(&Path::from("/World"), &Token::from("Xform"));
let mesh = stage.define_prim(&Path::from("/World/Cube"), &Token::from("Mesh"));

// Override creates an "over" opinion (no type, no definition)
let over_prim = stage.override_prim(&Path::from("/World/Extra"));
```

### Prim Identity

```rust
let prim = stage.get_prim_at_path(&Path::from("/World/Cube"));

println!("Path: {}", prim.path());           // /World/Cube
println!("Name: {}", prim.name());           // Cube
println!("Type: {}", prim.type_name());      // Mesh
println!("Valid: {}", prim.is_valid());       // true
println!("Defined: {}", prim.is_defined());  // true
println!("Active: {}", prim.is_active());    // true
```

### Hierarchy Navigation

```rust
// Parent
let parent = prim.parent();  // /World

// Children
for child in prim.get_all_children() {
    println!("  child: {}", child.name());
}

// Check
println!("Has children: {}", prim.has_children());
println!("Is pseudo-root: {}", prim.is_pseudo_root());
```

### Prim Flags

Prims carry several boolean flags that control their behavior:

| Flag | Meaning |
|------|---------|
| `is_active()` | Whether the prim participates in composition |
| `is_loaded()` | Whether payloads are loaded |
| `is_model()` | Whether this prim is a model (has Kind) |
| `is_group()` | Whether this prim is a group model |
| `is_defined()` | Whether a "def" specifier exists |
| `is_abstract()` | Whether the prim is a class |
| `has_payload()` | Whether the prim has payload arcs |

### Instancing

```rust
// Check instance status
println!("Instanceable: {}", prim.is_instanceable());
println!("Is instance: {}", prim.is_instance());
println!("Is prototype: {}", prim.is_prototype());

// Set instanceable
prim.set_instanceable(true);

// Get the prototype prim (for instances)
if prim.is_instance() {
    let proto = prim.get_prototype();
    println!("Prototype: {}", proto.path());
}
```

### Type Checking

```rust
use usd::tf::Token;

// Check if prim is of a specific type (including inheritance)
let mesh_type = Token::from("Mesh");
if prim.is_a(&mesh_type) {
    println!("This is a mesh");
}

// Check for applied API schemas
let geom_model = Token::from("GeomModelAPI");
if prim.has_api(&geom_model) {
    println!("Has GeomModelAPI");
}
```

## Properties

Properties are named data holders on a prim. There are two kinds:
**attributes** (typed values) and **relationships** (links to other prims).

### Listing Properties

```rust
// All properties (attributes + relationships)
for prop in prim.get_properties() {
    println!("{}: {}", prop.name(), prop.path());
}

// Only attributes
for attr in prim.get_attributes() {
    println!("attr {}: {}", attr.name(), attr.type_name());
}

// Only relationships
for rel in prim.get_relationships() {
    println!("rel {}", rel.name());
}
```

## Attributes

Attributes hold typed, potentially time-varying data.

### Reading Values

```rust
use usd::TimeCode;
use usd::vt::Value;

let attr = prim.get_attribute(&"points".into());
if let Some(attr) = attr {
    // Read at default time
    if let Some(val) = attr.get(TimeCode::default()) {
        println!("Value: {:?}", val);
    }

    // Read at a specific time
    if let Some(val) = attr.get(TimeCode::from(24.0)) {
        println!("Value at t=24: {:?}", val);
    }

    // Typed read (returns None if type mismatch)
    if let Some(points) = attr.get_typed::<Vec<[f32; 3]>>(TimeCode::default()) {
        println!("Points count: {}", points.len());
    }
}
```

### Writing Values

```rust
use usd::vt::Value;
use usd::TimeCode;

// Set at default time
attr.set(Value::from(42.0f64), TimeCode::default());

// Set a time sample
attr.set(Value::from(1.0f64), TimeCode::from(1.0));
attr.set(Value::from(2.0f64), TimeCode::from(24.0));

// Clear a specific time sample
attr.clear(TimeCode::from(1.0));

// Clear all authored values
attr.clear_all();

// Block the attribute (makes it appear as if it has no value)
attr.block();
```

### Attribute Metadata

```rust
// Type information
println!("Type: {}", attr.type_name());
println!("Variability: {:?}", attr.variability());

// Value presence
println!("Has value: {}", attr.has_value());
println!("Has authored: {}", attr.has_authored_value());
println!("Has fallback: {}", attr.has_fallback_value());
println!("Time varying: {}", attr.might_be_time_varying());

// Time samples
let samples = attr.get_time_samples();
println!("Samples: {:?}", samples);
println!("Count: {}", attr.get_num_time_samples());
```

## Relationships

Relationships are pointers to other prims or properties.

```rust
// Get targets of a relationship
let rel = prim.get_relationship(&"material:binding".into());
if let Some(rel) = rel {
    let targets = rel.get_targets();
    for target in &targets {
        println!("Target: {}", target);
    }
}
```
