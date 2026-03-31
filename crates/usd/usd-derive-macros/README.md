# usd-derive-macros

Procedural derive macros for defining USD schemas in Rust.

## Overview

This crate provides derive macros that generate USD schema boilerplate code at compile time, making it easy to define custom USD prim types in Rust.

## Usage

```rust
use usd_derive_macros::UsdSchema;

#[derive(UsdSchema)]
#[usd_prim_type("MyMesh")]
#[usd_schema_base("UsdGeomGprim")]
pub struct MyMesh {
    prim: usd::Prim,  // Required: underlying prim
    
    #[usd_attr(type = "point3f[]", interpolation = "vertex")]
    pub points: Vec<Vec3f>,
    
    #[usd_attr(type = "normal3f[]", interpolation = "faceVarying")]
    pub normals: Option<Vec<Vec3f>>,
    
    #[usd_attr(type = "int[]")]
    pub face_vertex_counts: Vec<i32>,
    
    #[usd_attr(type = "float", default = "1.0")]
    pub intensity: f32,
    
    #[usd_rel]
    pub material: Option<Path>,
}
```

## Generated Code

The macro generates:

### Type Implementation
```rust
impl UsdTyped for MyMesh {
    fn get_schema_type_name() -> &'static str { "MyMesh" }
    fn is_typed() -> bool { true }
}
```

### Schema Implementation
```rust
impl UsdSchemaBase for MyMesh {
    fn get_schema_kind() -> &'static str { "concreteTyped" }
    fn get_schema_base_type() -> &'static str { "UsdGeomGprim" }
    fn get_prim(&self) -> &Prim { &self.prim }
}
```

### Factory Methods
```rust
impl MyMesh {
    pub fn define(stage: &Stage, path: &Path) -> Option<Self>;
    pub fn get(stage: &Stage, path: &Path) -> Option<Self>;
    pub fn from_prim(prim: Prim) -> Self;
}
```

### Attribute Accessors
```rust
impl MyMesh {
    // For each #[usd_attr] field:
    pub fn get_points(&self) -> Option<Vec<Vec3f>>;
    pub fn set_points(&self, value: Vec<Vec3f>) -> bool;
    pub fn has_points(&self) -> bool;
    pub fn clear_points(&self) -> bool;
    pub fn create_points_attr(&self) -> Option<Attribute>;
    
    // For each #[usd_rel] field:
    pub fn get_material_rel(&self) -> Option<Relationship>;
    pub fn get_material_targets(&self) -> Vec<Path>;
    pub fn set_material_targets(&self, targets: &[Path]) -> bool;
    pub fn add_material_target(&self, target: &Path) -> bool;
}
```

## Attributes Reference

### Struct-Level

| Attribute | Required | Description |
|-----------|----------|-------------|
| `#[usd_prim_type("Name")]` | Yes | USD prim type name |
| `#[usd_schema_kind("kind")]` | No | Schema kind (default: "concreteTyped") |
| `#[usd_schema_base("Base")]` | No | Base schema (default: "UsdTyped") |
| `#[usd_doc("...")]` | No | Schema documentation |

### Field-Level

| Attribute | Description |
|-----------|-------------|
| `#[usd_attr(type = "...")]` | USD type name (required for attributes) |
| `#[usd_attr(default = "...")]` | Default value |
| `#[usd_attr(interpolation = "...")]` | vertex, faceVarying, uniform, constant |
| `#[usd_attr(doc = "...")]` | Attribute documentation |
| `#[usd_rel]` | Mark field as relationship |

## USD Types

Common USD types for `#[usd_attr(type = "...")]`:

| USD Type | Rust Type |
|----------|-----------|
| `bool` | `bool` |
| `int` | `i32` |
| `float` | `f32` |
| `double` | `f64` |
| `string` | `String` |
| `token` | `Token` |
| `asset` | `AssetPath` |
| `float3` | `Vec3f` |
| `point3f` | `Vec3f` |
| `normal3f` | `Vec3f` |
| `color3f` | `Vec3f` |
| `matrix4d` | `Matrix4d` |
| `float[]` | `Vec<f32>` |
| `point3f[]` | `Vec<Vec3f>` |

## Debugging

Use `cargo-expand` to see generated code:

```bash
cargo install cargo-expand
cargo expand --package my-crate my_schema
```

## Comparison with usdGenSchema

| usdGenSchema (C++) | usd-derive-macros (Rust) |
|-------------------|-------------------------|
| External codegen tool | Compile-time macros |
| Generates .cpp/.h files | Generates in-memory |
| Requires build step | Part of cargo build |
| Reads schema.usda | Uses Rust attributes |

## TODO

- [ ] Support for API schemas
- [ ] Automatic schema.usda generation
- [ ] plugInfo.json generation
- [ ] Validation of USD type names
- [ ] Support for namespaced attributes
