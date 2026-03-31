# Base Crates

The base crates provide fundamental types and utilities used by all other
layers. They correspond to `pxr/base/` in C++ OpenUSD.

## usd-arch

**C++ equivalent:** `pxr/base/arch`

Architecture and platform abstractions.

| Feature | Description |
|---------|-------------|
| OS detection | Platform identification (Windows, Linux, macOS) |
| CPU info | Architecture, features, cache sizes |
| Memory | Alignment, page size, virtual memory info |
| Stack traces | Capture and symbolicate stack traces |
| Demangling | C++ and Rust symbol demangling |
| Timing | High-resolution timers, `ArchGetTickTime` |

```rust
use usd::arch;

// Get system page size
let page_size = arch::get_page_size();

// Capture a stack trace
let trace = arch::get_stack_trace();
```

## usd-tf

**C++ equivalent:** `pxr/base/tf`

Tools Foundation -- the most widely used base crate. Provides token interning,
diagnostics, type system, and notification infrastructure.

### Token

`Token` is an interned string. Comparison is O(1) pointer equality.

```rust
use usd::tf::Token;

let tok = Token::from("Mesh");
let tok2 = Token::from("Mesh");
assert!(tok == tok2);  // pointer comparison, very fast

// Empty/null token
let empty = Token::new();
assert!(empty.is_empty());

// Get the string value
let s: &str = tok.as_str();
```

### Type System

`TfType` provides runtime type information, similar to C++ RTTI but with
USD-specific features like schema type hierarchy.

### Debug Codes

The debug code system allows fine-grained diagnostic output controlled by
environment variables.

### Notifications

`TfNotice` provides a publish-subscribe system for change notifications
between components.

## usd-gf

**C++ equivalent:** `pxr/base/gf`

Graphics Foundation -- math types for 3D graphics. Backed by the `glam` crate
for SIMD-optimized operations.

### Vectors

| Type | Description |
|------|-------------|
| `Vec2f`, `Vec2d`, `Vec2i`, `Vec2h` | 2D vectors (float, double, int, half) |
| `Vec3f`, `Vec3d`, `Vec3i`, `Vec3h` | 3D vectors |
| `Vec4f`, `Vec4d`, `Vec4i`, `Vec4h` | 4D vectors |

### Matrices

| Type | Description |
|------|-------------|
| `Matrix2f`, `Matrix2d` | 2x2 matrices |
| `Matrix3f`, `Matrix3d` | 3x3 matrices |
| `Matrix4f`, `Matrix4d` | 4x4 matrices (transforms) |

### Geometry

| Type | Description |
|------|-------------|
| `Quatf`, `Quatd`, `Quath` | Quaternions |
| `Rotation` | Axis-angle rotation |
| `Range1f`, `Range1d` | 1D ranges (intervals) |
| `Range2f`, `Range2d` | 2D ranges (rectangles) |
| `Range3f`, `Range3d` | 3D ranges (bounding boxes) |
| `BBox3d` | Bounding box with transform |
| `Ray` | 3D ray (origin + direction) |
| `Frustum` | View frustum |
| `Plane` | Half-space plane |

```rust
use usd::gf;

let point = gf::Vec3f::new(1.0, 2.0, 3.0);
let matrix = gf::Matrix4d::identity();
let bbox = gf::Range3d::new(
    gf::Vec3d::new(-1.0, -1.0, -1.0),
    gf::Vec3d::new(1.0, 1.0, 1.0),
);
```

## usd-vt

**C++ equivalent:** `pxr/base/vt`

Value types -- provides a type-erased value container and array type.

### Value

`Value` is a type-erased container that can hold any USD value type (scalars,
vectors, matrices, arrays, tokens, strings, asset paths, etc.).

```rust
use usd::vt::Value;

let int_val = Value::from(42i32);
let float_val = Value::from(3.14f64);
let string_val = Value::from("hello");
let vec_val = Value::from([1.0f32, 2.0, 3.0]);

// Type checking
if let Some(f) = int_val.get::<i32>() {
    println!("Got int: {}", f);
}
```

### VtArray

`VtArray<T>` is the typed array container used for multi-valued attributes
(vertex positions, face indices, etc.). It supports copy-on-write semantics.

## usd-js

**C++ equivalent:** `pxr/base/js`

JSON utilities for reading plugin metadata, plugInfo.json files, and
configuration data.

## usd-plug

**C++ equivalent:** `pxr/base/plug`

Plugin registry for discovering and loading plugins. In usd-rs, plugins are
registered statically at compile time rather than dynamically loaded from
shared libraries.

```rust
use usd::plug;

// Register a file format plugin
plug::Registry::register_plugin(plugin_metadata);
```

## usd-trace

**C++ equivalent:** `pxr/base/trace`

Performance tracing instrumentation. Integrates with the Rust `tracing`
ecosystem.

```rust
use usd::trace;

// Instrumented scope
trace::scope!("MyOperation");
```

## usd-ts

**C++ equivalent:** `pxr/base/ts`

Time splines for smooth animation interpolation. Provides:
- Bezier spline evaluation
- Hermite interpolation
- Knot manipulation (linear, held, bezier)
- Spline looping

## usd-work

**C++ equivalent:** `pxr/base/work`

Work dispatcher for parallel task execution. Wraps Rayon to provide:
- Parallel for-each over ranges
- Parallel reduce
- Task graphs with dependencies
- Thread pool management

```rust
use usd::work;

// Parallel iteration
work::parallel_for_each(&items, |item| {
    process(item);
});
```
