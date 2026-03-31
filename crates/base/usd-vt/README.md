# usd-vt -- Value Types

Rust port of OpenUSD `pxr/base/vt`.

VT is the value storage layer of OpenUSD. It provides type-erased containers that hold any USD data type at runtime:

- **Value** -- type-erased container (like `Any`) with inline storage for small types (<16 bytes), casting registry (`cast_to_typeid`, `can_cast`), visitor dispatch, and composition semantics
- **Array\<T\>** -- copy-on-write generic array with reshape/multidimensional support, zero-copy foreign data (`ForeignDataSource` with detach callbacks), and COW via `Arc`
- **Dictionary** -- sorted string-keyed map (`BTreeMap<String, Value>`) with recursive composition (`dictionary_over`), path-based access (`set_value_at_path`/`erase_value_at_path`)
- **ArrayEdit** -- monoid-based array editing: Write, Insert, Erase, Append, SetSize with `compose_over` forming a proper monoid
- **ValueTransform** -- composable value transformations: Scale, Offset, Clamp, Identity, Chain, Map
- **TimeCode** -- animation time representation (moved from sdf to break circular deps)
- **AssetPath** -- asset reference with authored/resolved paths (moved from sdf to break circular deps)

Reference: `_ref/OpenUSD/pxr/base/vt`

## Parity Status

All public C++ APIs have Rust equivalents. Verified header-by-header against the reference.

Python bindings (`valueFromPython.h`, `pyOperators.h`, `arrayPyBuffer.h`, `wrapArray.h`, `wrapArrayEdit.h`) excluded by design -- not applicable to Rust.

---

## Module Map

### Core Types

| C++ Header | Rust Module | Notes |
|---|---|---|
| value.h | value.rs | Type-erased Value: inline storage, `get::<T>()`, `get_or::<T>(default)`, `cast::<T>()`, `cast_to_typeid(TypeId)`, `can_cast_from_typeid_to_typeid()`, proxy support |
| array.h | array.rs | COW `Array<T>`: `Arc<Vec<T>>` or `ForeignDataSource` zero-copy storage, reshape, ShapeData, detach callback on mutation |
| dictionary.h | dictionary.rs | BTreeMap-backed Dictionary, recursive `dictionary_over`, path-based get/set/erase, iterators |

### Value Operations

| C++ Header | Rust Module | Notes |
|---|---|---|
| streamOut.h | stream_out.rs | `stream_out_value`, generic/bool/float/double/array formatting via `Display` |
| valueCommon.h | value_common.rs | `Vt_DefaultValueFactory`, `Vt_ValueStoredType` |
| valueComposeOver.h | value_compose_over.rs | Composition semantics, monoid identity detection |
| valueRef.h | value_ref.rs | Non-owning `ValueRef` with type introspection |
| valueTransform.h | value_transform.rs | Composable transforms: Scale, Offset, Clamp, Identity, Chain, Map |
| visitValue.h | visit_value.rs | Visitor pattern with dispatch for all known types including AssetPath and TimeCode |

### Array Editing

| C++ Header | Rust Module | Notes |
|---|---|---|
| arrayEdit.h | array_edit.rs | `ArrayEdit<T>` monoid: IsIdentity, ComposeOver |
| arrayEditBuilder.h | array_edit_builder.rs | Fluent builder: Write, Insert, Erase, Append, SetSize |
| arrayEditOps.h | array_edit_ops.rs | EditOp enum, arity checks, END_INDEX constant |

### Utilities

| C++ Header | Rust Module | Notes |
|---|---|---|
| types.h | types.rs | Scalar type arrays, VT_BUILTIN_VALUE_TYPES, TypeId registrations for AssetPath (14) and TimeCode (15) |
| typeHeaders.h | type_headers.rs | Re-exports of gf types (Vec*, Matrix*, Range*, Quat*, DualQuat*) |
| traits.h | traits.rs | `VtIsArray`, `VtValueTypeHasCheapCopy`, proxy traits |
| hash.h | hash.rs | `hash_value`, `hash_combine`, float hashing via bit representation |
| debugCodes.h | debug_codes.rs | `VT_ARRAY_EDIT_BOUNDS` debug code |

### Rust Extensions (no C++ equivalent in vt/)

| Rust Module | Notes |
|---|---|
| time_code.rs | TimeCode (f64 wrapper with arithmetic). Moved from usd-sdf to break circular dependency |
| asset_path.rs | AssetPath + AssetPathParams. Moved from usd-sdf to break circular dependency |
| spline.rs | Spline animation curves (Bezier/Hermite, extrapolation, tangents) |
| mutable_value_ref.rs | Rust-specific mutable non-owning value reference |

---

## Value Type Support

All USD value types supported:
- Primitives: `bool`, `i32`, `i64`, `u32`, `u64`, `f32`, `f64`
- Vectors: `Vec2/3/4` `d/f/i/h`
- Matrices: `Matrix2/3/4` `d/f`
- Quaternions: `Quatd/f/h`, `DualQuatd/f/h`
- Ranges: `Range1d/f`, `Range2d/f`, `Range3d/f`
- Strings: `String`, `Token`
- Paths: `AssetPath`, `TimeCode`
- Arrays: `Array<T>` for all above types

---

## Not Ported (not needed in Rust)

| C++ File | Reason |
|---|---|
| api.h | `pub` visibility in Rust |
| module.cpp / pch.h | Module init / precompiled headers |
| valueFromPython.h | Python value extraction registry |
| pyOperators.h | Python operator overloads |
| arrayPyBuffer.h | Python buffer protocol |
| wrapArray.h / wrapArrayEdit.h | Python bindings |
| functions.h | C++ template helpers |
| overview.dox | Doxygen docs |

---

## API Differences from C++

| C++ API | Rust Equivalent | Rationale |
|---|---|---|
| `VtValue` void pointer union | TypeId-based dispatch | Type-safe, no `unsafe` needed |
| `VtArray` intrusive refcount | `ArrayStorage::Owned(Arc<Vec<T>>)` / `Foreign(Arc<ForeignSlice<T>>)` | Explicit ownership model |
| `VtDictionary` (std::map) | `BTreeMap<String, Value>` | Same sorted semantics, better cache locality |
| `VtArray::_ForeignDataSource` C function pointer | `ForeignDataSource` with `Box<dyn Fn()>` callback | More idiomatic Rust |
| `SdfAssetPath` (in sdf/) | `AssetPath` (in vt/) | Moved to break circular dep vt->sdf->vt |
| `SdfTimeCode` (in sdf/) | `TimeCode` (in vt/) | Moved to break circular dep vt->sdf->vt |

---

## Implementation Notes

### Array Copy-on-Write
`ArrayStorage` enum: `Owned(Arc<Vec<T>>)` for normal data, `Foreign(Arc<ForeignSlice<T>>)` for zero-copy external data. Any mutation calls `make_unique()` which detaches foreign data (copies to owned, triggers callback when last reference drops).

### Value Inline Storage
Types smaller than 16 bytes are stored inline without heap allocation. Larger types use `Box`. Cast registry supports runtime type conversion via `cast_to_typeid(TypeId)` and `can_cast_from_typeid_to_typeid(TypeId, TypeId)`.

### Dictionary Composition
`dictionary_over(strong, weak)` merges recursively: nested dictionaries are composed, scalar values from strong override weak. Path-based API (`set_value_at_path`) creates intermediate dictionaries as `Dictionary` type (not `HashMap`).

### Visitor Dispatch
`visit_value` dispatches to type-specific visitor methods for all built-in types including `AssetPath` and `TimeCode` (previously blocked by circular deps, now resolved).

Verified 2026-02-22.
