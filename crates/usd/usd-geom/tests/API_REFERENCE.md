# API Reference for Test Writers

## Stage/Path/TimeCode
```rust
use usd_core::{InitialLoadSet, Stage};
let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
usd_sdf::init(); // MUST call before Stage::open()
let stage = Stage::open(path_string, InitialLoadSet::LoadAll).unwrap();
let path = usd_sdf::Path::from_string("/Foo").unwrap();
let tc = TimeCode::default_time();   // Default sentinel
let tc = TimeCode::new(1.0);         // Numeric time
let tc = TimeCode::new(f64::MIN);    // Earliest time
```

## XformOp (NOT a schema, a transform operation)
```rust
// Methods (called on instance):
op.op_name() -> Token           // NOT get_op_name()
op.op_type() -> XformOpType     // NOT get_op_type()
op.set(value, time) -> bool     // Generic: accepts Vec3d, Vec3f, f32, f64, Matrix4d, Quatf, etc.
op.get(time) -> Option<Value>
op.get_typed::<T>(time) -> Option<T>
op.is_valid() -> bool           // NOT is_defined()
op.attr() -> &Attribute
op.precision() -> XformOpPrecision
op.is_inverse_op() -> bool
op.might_be_time_varying() -> bool
op.get_time_samples() -> Vec<f64>

// Static functions:
XformOp::get_op_name(op_type: XformOpType, suffix: Option<&Token>, inverse: bool) -> Token  // STATIC
XformOp::get_op_type_token(XformOpType) -> Token
XformOp::get_op_type_enum(&Token) -> XformOpType
```

## Xformable
```rust
let xf = Xformable::new(prim);  // or xform.xformable()

// add_*_op: precision is XformOpPrecision (NOT Option!)
xf.add_translate_op(XformOpPrecision::PrecisionDouble, None, false) -> XformOp
xf.add_translate_x_op(XformOpPrecision::PrecisionDouble, None, false) -> XformOp
xf.add_scale_op(XformOpPrecision::PrecisionFloat, None, false) -> XformOp
xf.add_rotate_x_op(XformOpPrecision::PrecisionFloat, None, false) -> XformOp
xf.add_rotate_xyz_op(XformOpPrecision::PrecisionFloat, None, false) -> XformOp
xf.add_orient_op(XformOpPrecision::PrecisionFloat, None, false) -> XformOp
xf.add_transform_op(XformOpPrecision::PrecisionDouble, None, false) -> XformOp

// get_*_op:
xf.get_translate_op(suffix: Option<&Token>, is_inverse: bool) -> XformOp
// Same pattern for all get_* methods

// Other:
xf.get_local_transformation(TimeCode) -> Matrix4d
xf.get_ordered_xform_ops() -> Vec<XformOp>
xf.clear_xform_op_order() -> bool
xf.set_reset_xform_stack(bool) -> bool
xf.get_reset_xform_stack() -> bool
xf.make_matrix_xform() -> XformOp
xf.transform_might_be_time_varying() -> bool
xf.get_time_samples() -> Vec<f64>
xf.set_xform_op_order(&[XformOp]) -> bool
xf.set_xform_op_order_with_reset(&[XformOp], bool) -> bool
```

## Matrix4d
```rust
Matrix4d::identity()
Matrix4d::from_array([[f64; 4]; 4])  // NOT from_rows!
m[row][col]                          // Index impl, NOT m.data[r][c]
m.set_translate(&Vec3d) -> Matrix4d  // returns new matrix
m.set_scale(&Vec3d) -> Matrix4d
usd_gf::matrix4::is_close(&a, &b, 1e-4) -> bool  // for comparison
```

## XformOpPrecision
```rust
XformOpPrecision::PrecisionDouble  // for translate, transform
XformOpPrecision::PrecisionFloat   // for scale, rotate, orient (default)
XformOpPrecision::PrecisionHalf
```

## XformOpType
```rust
Translate, TranslateX, TranslateY, TranslateZ,
Scale, ScaleX, ScaleY, ScaleZ,
RotateX, RotateY, RotateZ,
RotateXYZ, RotateXZY, RotateYXZ, RotateYZX, RotateZXY, RotateZYX,
Orient, Transform, Invalid
```

## Geom Schema Pattern
```rust
let cube = Cube::define(&stage, &path);
let cube = Cube::get(&stage, &path);
cube.is_valid()
cube.prim() -> &Prim
cube.get_size_attr() -> Attribute
cube.create_size_attr(Option<Value>, bool) -> Attribute  // some take args, some don't

// Imageable base:
imageable.get_visibility_attr() -> Attribute
imageable.create_visibility_attr() -> Attribute  // NO args
imageable.compute_visibility(TimeCode) -> Token
imageable.make_visible(TimeCode)
imageable.make_invisible(TimeCode)
```

## Primvar
```rust
let pv_api = PrimvarsAPI::new(prim);  // or PrimvarsAPI(prim)
pv_api.create_primvar(name: &str, type_name: &ValueTypeName) -> Primvar
pv_api.get_primvar(name: &str) -> Primvar
pv_api.has_primvar(name: &str) -> bool
pv_api.get_primvars() -> Vec<Primvar>

primvar.set(value, TimeCode) -> bool
primvar.get(TimeCode) -> Option<Value>
primvar.set_interpolation(&Token) -> bool
primvar.get_interpolation() -> Token
primvar.set_element_size(usize) -> bool
primvar.get_element_size() -> usize
```

## Subset
```rust
Subset::create_geom_subset(geom, name, element_type, &indices, family_name, family_type) -> Subset
Subset::create_unique_geom_subset(...) -> Subset
Subset::get_all_geom_subsets(geom) -> Vec<Subset>
Subset::get_geom_subsets(geom, element_type, family_name) -> Vec<Subset>
Subset::set_family_type(geom, family_name, family_type) -> bool
Subset::get_family_type(geom, family_name) -> Token
Subset::validate_family(geom, element_type, family_name) -> (bool, String)  // check actual sig
Subset::get_unassigned_indices(...) -> Vec<i32>
```

## PointInstancer
```rust
pi.activate_id(i64)
pi.deactivate_id(i64)
pi.vis_id(i64, TimeCode)
pi.invis_id(i64, TimeCode)
pi.compute_mask_at_time(TimeCode, Option<&[i64]>) -> Vec<bool>
pi.get_instance_count(TimeCode) -> usize
```
