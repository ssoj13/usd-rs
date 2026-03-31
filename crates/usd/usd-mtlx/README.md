# MaterialX Document Model for USD-RS

This module provides a pure Rust implementation of the MaterialX document model, enabling parsing, traversal, and manipulation of MaterialX (.mtlx) files.

## Architecture

The implementation uses an **arena-based approach** with `Arc<DocumentData>` for efficient cloning and element indices for parent/child relationships. Documents are **immutable after parsing**, making them thread-safe and cacheable.

### Key Components

- **Document** - The main document container with Arc-based sharing
- **Element** - Generic element handle with index into the document arena
- **Typed Wrappers** - Type-safe wrappers: NodeDef, Node, Input, Output, NodeGraph, Material, Look, etc.
- **XML Parser** - Quick-XML based parser with XInclude support
- **Value Parser** - Type-safe parsing for all MaterialX data types

## Features

✅ **Complete MaterialX 1.38 support**
- All element types (NodeDef, Node, Input, Output, NodeGraph, Material, Look, etc.)
- Full attribute parsing and navigation
- Type-safe element wrappers
- Parent/child relationships and path navigation

✅ **Arena-based memory model**
- Efficient cloning with Arc
- Index-based relationships (no lifetimes)
- Immutable after construction

✅ **XML I/O**
- Parse from files or strings
- XInclude support for library composition
- Attribute preservation
- Source URI tracking

✅ **Value Parsing**
- All MaterialX types: bool, int, float, string, color3/4, vector2/3/4, matrix33/44
- Array types with proper separators
- Type-safe enum variants

## Usage Examples

### Basic Parsing

```rust
use usd::schema::mtlx::*;

// Parse from file
let doc = read_from_xml_file("materials/standard_surface.mtlx")?;

// Parse from string
let xml = r#"
<?xml version="1.0"?>
<materialx version="1.38">
  <nodedef name="ND_standard_surface" node="standard_surface" type="surfaceshader">
    <input name="base_color" type="color3" value="0.8, 0.8, 0.8"/>
    <input name="metalness" type="float" value="0.0"/>
    <output name="out" type="surfaceshader"/>
  </nodedef>
</materialx>
"#;

let doc = read_from_xml_string(xml)?;
```

### Document Navigation

```rust
// Get root element
let root = doc.get_root();
println!("Document version: {}", root.get_attribute("version"));

// Get all NodeDefs
for nodedef in doc.get_node_defs() {
    println!("NodeDef: {} (node={})", 
        nodedef.0.name(), 
        nodedef.get_node_string()
    );
    
    // Get inputs
    for input in nodedef.get_inputs() {
        println!("  Input: {} type={} value={}", 
            input.0.name(),
            input.get_type(),
            input.get_value_string()
        );
    }
}

// Find specific NodeDef
if let Some(nd) = doc.get_node_def("ND_standard_surface") {
    println!("Found nodedef: {}", nd.0.name());
}
```

### Material and Look Navigation

```rust
// Get all looks
for look in doc.get_looks() {
    println!("Look: {}", look.get_name());
    
    // Get material assignments
    for assign in look.get_material_assigns() {
        println!("  Material: {} -> geom: {}",
            assign.get_material(),
            assign.0.get_attribute("geom")
        );
    }
}

// Get materials
let materials: Vec<_> = root
    .get_children_of_type("material")
    .into_iter()
    .map(Material)
    .collect();
```

### NodeGraph Traversal

```rust
let nodegraphs: Vec<_> = root
    .get_children_of_type("nodegraph")
    .into_iter()
    .map(NodeGraph)
    .collect();

for ng in nodegraphs {
    println!("NodeGraph: {}", ng.0.name());
    
    // Get all nodes
    for node in ng.get_nodes("") {
        println!("  Node: {} (category={})",
            node.0.name(),
            node.get_category_name()
        );
        
        // Check connections
        for input in node.get_inputs() {
            if !input.get_node_name().is_empty() {
                println!("    Connected: {} <- {}.{}",
                    input.0.name(),
                    input.get_node_name(),
                    input.get_output()
                );
            }
        }
    }
}
```

### Value Parsing

```rust
let nodedef = doc.get_node_def("ND_test").unwrap();
for input in nodedef.get_inputs() {
    let value = create_value_from_strings(
        input.get_value_string(),
        input.get_type()
    );
    
    match value {
        Some(MtlxValue::Float(f)) => println!("Float: {}", f),
        Some(MtlxValue::Color3(c)) => println!("Color: {:?}", c),
        Some(MtlxValue::Vector3(v)) => println!("Vector: {:?}", v),
        _ => {}
    }
}
```

### NodeDef Inheritance

```rust
let derived = doc.get_node_def("ND_derived").unwrap();

// Get direct inputs
let direct = derived.get_inputs();
println!("Direct inputs: {}", direct.len());

// Get all inputs including inherited
let active = derived.get_active_inputs();
println!("Active inputs (with inherited): {}", active.len());

// Check inheritance
if derived.has_inherit_string() {
    println!("Inherits from: {}", derived.get_inherit_string());
}
```

### Element Path Navigation

```rust
// Get full path
let input = /* some input element */;
println!("Path: {}", input.get_name_path());
// Output: "nodegraph1/node1/input1"

// Navigate up
if let Some(parent) = input.get_parent() {
    println!("Parent: {}", parent.name());
}

// Get root
let root = input.get_root();
```

### Color Space Management

```rust
// Document-level color space
let doc_colorspace = doc.get_active_color_space();

// Element-level color space
let input: Input = /* ... */;
let cs = input.get_active_color_space();
// Walks up parent hierarchy to find colorspace attribute
```

### Library Composition

```rust
// Load main document
let mut main_doc = read_from_xml_file("main.mtlx")?;

// Load and import library
let lib_doc = read_from_xml_file("stdlib.mtlx")?;
main_doc.import_library(&lib_doc);

// Now main_doc contains all elements from both documents
```

## Type Reference

### Core Types

- `Document` - Main document container (Arc-based, cheap to clone)
- `Element` - Generic element handle
- `MtlxError` - Error type for operations
- `MtlxValue` - Typed value enum for all MaterialX types

### Element Wrappers

- `NodeDef` - Node definition with inputs/outputs
- `Node` - Node instance with connections
- `Input` - Input port with value/connection
- `Output` - Output port
- `NodeGraph` - Container for nodes and connections
- `Material` - Material definition
- `Look` - Look with material assignments
- `MaterialAssign` - Material assignment to geometry
- `Collection` - Geometry collection
- `TypeDef` - Type definition

### Traits

- `TypedElement` - Elements with type attribute
- `ValueElement` - Elements with value and colorspace
- `InterfaceElement` - Elements with inputs/outputs (NodeDef, NodeGraph)

## Constants

```rust
pub const SURFACE_SHADER_TYPE_STRING: &str = "surfaceshader";
pub const DISPLACEMENT_SHADER_TYPE_STRING: &str = "displacementshader";
pub const VOLUME_SHADER_TYPE_STRING: &str = "volumeshader";
pub const LIGHT_SHADER_TYPE_STRING: &str = "lightshader";
pub const SHADER_SEMANTIC: &str = "shader";
pub const ARRAY_PREFERRED_SEPARATOR: &str = ", ";
pub const EMPTY_STRING: &str = "";
```

## Testing

The module includes comprehensive tests:

```bash
# Run all MaterialX tests
cargo test --lib mtlx

# Run specific test
cargo test --lib schema::mtlx::tests::test_full_materialx_document
```

## Implementation Notes

### Arena-Based Design

Elements are stored in a Vec within DocumentData. Each Element holds an Arc to the DocumentData and an index. This approach:
- Eliminates lifetime issues
- Enables efficient cloning (Arc increment)
- Provides stable element references
- Supports concurrent read access

### Immutability

Documents are immutable after parsing. This design:
- Simplifies thread safety (no locks needed for reading)
- Enables aggressive caching
- Matches USD's layer composition model
- Prevents accidental modification

### XML Parsing Strategy

The parser:
1. Treats the first XML element as the document root
2. Updates the pre-created root element with attributes
3. Creates child elements for all nested tags
4. Handles XInclude directives recursively
5. Tracks source URIs for debugging

### Value Parsing

The value parser handles:
- Scalar types (bool, int, float, string)
- Vector types (color3/4, vector2/3/4)
- Matrix types (matrix33, matrix44)
- Array types (comma-separated for scalars, semicolon-separated groups for vectors)
- Whitespace trimming and flexible parsing

## Limitations

1. **No dynamic modification** - Documents are immutable after parsing
2. **No validation** - The parser doesn't validate MaterialX semantics
3. **Limited XInclude** - Only basic file includes, no XPointer support
4. **No writing** - XML writing is not implemented (read-only)

## Future Enhancements

Potential additions for full MaterialX support:
- [ ] XML writing/serialization
- [ ] Schema validation
- [ ] Node graph evaluation
- [ ] Color management integration
- [ ] Shader code generation
- [ ] Advanced XInclude features

## C++ Reference

This implementation is inspired by MaterialX C++ library but adapted for Rust idioms:
- Uses Result<T, E> instead of exceptions
- Leverages Rust's type system for safety
- Eliminates raw pointers with Arc and indices
- Provides functional iterator APIs

## License

This code is part of the USD-RS project and follows the same license as the parent project.
