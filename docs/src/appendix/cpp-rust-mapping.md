# C++ to Rust Mapping

This appendix provides a quick reference for mapping between C++ OpenUSD
concepts and their Rust equivalents in usd-rs.

## Namespace Mapping

| C++ Namespace | Rust Module | Crate |
|--------------|-------------|-------|
| `pxr::Arch*` | `usd::arch` | `usd-arch` |
| `pxr::Tf*` | `usd::tf` | `usd-tf` |
| `pxr::Gf*` | `usd::gf` | `usd-gf` |
| `pxr::Vt*` | `usd::vt` | `usd-vt` |
| `pxr::Js*` | `usd::js` | `usd-js` |
| `pxr::Plug*` | `usd::plug` (via `usd-plug`) | `usd-plug` |
| `pxr::Trace*` | `usd::trace` | `usd-trace` |
| `pxr::Ts*` | `usd::ts` | `usd-ts` |
| `pxr::Work*` | `usd::work` | `usd-work` |
| `pxr::Ar*` | `usd::ar` | `usd-ar` |
| `pxr::Sdf*` | `usd::sdf` | `usd-sdf` |
| `pxr::Pcp*` | `usd::pcp` | `usd-pcp` |
| `pxr::Usd*` | `usd::usd::*` | `usd-core` |
| `pxr::UsdGeom*` | `usd::usd_geom` | `usd-geom` |
| `pxr::UsdShade*` | `usd::usd_shade` | `usd-shade` |
| `pxr::UsdLux*` | `usd::usd_lux` | `usd-lux` |
| `pxr::UsdSkel*` | `usd::usd_skel` | `usd-skel` |
| `pxr::Hd*` | `usd::imaging::hd` | `usd-hd` |
| `pxr::HdSt*` | `usd::imaging::hd_st` | `usd-hd-st` |
| `pxr::Hgi*` | `usd::imaging::hgi` | `usd-hgi` |
| `pxr::Hdx*` | `usd::imaging::hdx` | `usd-hdx` |

## Type Mapping

### Smart Pointers

| C++ | Rust | Notes |
|-----|------|-------|
| `TfRefPtr<T>` | `Arc<T>` | Thread-safe reference counting |
| `TfWeakPtr<T>` | `Weak<T>` | Non-owning weak reference |
| `SdfLayerRefPtr` | `Arc<Layer>` | |
| `SdfLayerHandle` | `Arc<Layer>` | No separate handle type |
| `UsdStageRefPtr` | `Arc<Stage>` | |
| `std::shared_ptr<T>` | `Arc<T>` | |
| `std::unique_ptr<T>` | `Box<T>` | |
| `T*` (raw pointer) | `&T` or `Option<&T>` | Borrow or optional reference |

### Strings

| C++ | Rust | Notes |
|-----|------|-------|
| `std::string` | `String` | Owned string |
| `const std::string&` | `&str` | Borrowed string |
| `TfToken` | `Token` | Interned, O(1) comparison |
| `SdfPath` | `Path` | Scene hierarchy path |
| `SdfAssetPath` | `AssetPath` | Asset reference |

### Containers

| C++ | Rust | Notes |
|-----|------|-------|
| `std::vector<T>` | `Vec<T>` | |
| `VtArray<T>` | `VtArray<T>` or `Vec<T>` | Copy-on-write in C++ |
| `std::map<K,V>` | `BTreeMap<K,V>` | Ordered map |
| `std::unordered_map<K,V>` | `HashMap<K,V>` | Hash map |
| `std::set<T>` | `BTreeSet<T>` | Ordered set |
| `std::pair<A,B>` | `(A, B)` | Tuple |
| `std::optional<T>` | `Option<T>` | |

### Values

| C++ | Rust | Notes |
|-----|------|-------|
| `VtValue` | `Value` | Type-erased value |
| `GfVec2f` | `Vec2f` / `glam::Vec2` | 2D float vector |
| `GfVec3f` | `Vec3f` / `glam::Vec3` | 3D float vector |
| `GfVec4f` | `Vec4f` / `glam::Vec4` | 4D float vector |
| `GfVec3d` | `Vec3d` / `glam::DVec3` | 3D double vector |
| `GfMatrix4d` | `Matrix4d` / `glam::DMat4` | 4x4 double matrix |
| `GfQuatf` | `Quatf` / `glam::Quat` | Float quaternion |
| `GfRange3d` | `Range3d` | 3D bounding box |
| `GfBBox3d` | `BBox3d` | Bbox with transform |

### Error Handling

| C++ | Rust | Notes |
|-----|------|-------|
| `TF_CODING_ERROR(...)` | `return Err(...)` | Error propagation |
| `TF_RUNTIME_ERROR(...)` | `return Err(...)` | Runtime errors |
| `TF_WARN(...)` | `log::warn!(...)` | Warnings |
| `TF_STATUS(...)` | `log::info!(...)` | Status messages |
| `try { } catch { }` | `Result<T, E>` + `?` | No exceptions |
| `bool success = ...` | `Result<(), Error>` | Fallible operations |

### Threading

| C++ | Rust | Notes |
|-----|------|-------|
| `TBB` | `Rayon` | Thread pool |
| `WorkDispatcher` | `rayon::scope` | Scoped parallelism |
| `std::mutex` | `std::sync::Mutex` | |
| `std::shared_mutex` | `std::sync::RwLock` | Reader-writer lock |
| `std::atomic<T>` | `std::sync::atomic::Atomic*` | |
| `tbb::concurrent_hash_map` | `dashmap::DashMap` or `Mutex<HashMap>` | |

## API Pattern Mapping

### Creating Objects

```cpp
// C++
SdfLayerRefPtr layer = SdfLayer::CreateNew("scene.usda");
UsdStageRefPtr stage = UsdStage::Open("scene.usda");
```

```rust
// Rust
let layer = Layer::create_new("scene.usda")?;
let stage = Stage::open("scene.usda", InitialLoadSet::All)?;
```

### Prim Operations

```cpp
// C++
UsdPrim prim = stage->GetPrimAtPath(SdfPath("/World"));
TfToken typeName = prim.GetTypeName();
bool active = prim.IsActive();
UsdAttribute attr = prim.GetAttribute(TfToken("points"));
```

```rust
// Rust
let prim = stage.get_prim_at_path(&Path::from("/World"));
let type_name = prim.type_name();
let active = prim.is_active();
let attr = prim.get_attribute(&"points".into());
```

### Attribute Access

```cpp
// C++
VtValue value;
attr.Get(&value, UsdTimeCode::Default());
VtArray<GfVec3f> points;
attr.Get(&points, UsdTimeCode(24.0));
attr.Set(VtValue(42.0), UsdTimeCode::Default());
```

```rust
// Rust
let value = attr.get(TimeCode::default());
let points: Option<Vec<[f32; 3]>> = attr.get_typed(TimeCode::from(24.0));
attr.set(Value::from(42.0f64), TimeCode::default());
```

### Iteration

```cpp
// C++
for (const UsdPrim& prim : stage->Traverse()) {
    std::cout << prim.GetPath() << std::endl;
}
```

```rust
// Rust
for prim in stage.traverse() {
    println!("{}", prim.path());
}
```

## Method Naming Convention

C++ OpenUSD uses `PascalCase` for method names. usd-rs converts these to
Rust's `snake_case`:

| C++ | Rust |
|-----|------|
| `GetPrimAtPath()` | `get_prim_at_path()` |
| `GetAttribute()` | `get_attribute()` |
| `GetTypeName()` | `type_name()` or `get_type_name()` |
| `IsActive()` | `is_active()` |
| `HasAuthoredValue()` | `has_authored_value()` |
| `SetVariantSelection()` | `set_variant_selection()` |
| `CreateNew()` | `create_new()` |

Simple getters often drop the `Get` prefix when unambiguous (e.g.,
`GetTypeName()` → `type_name()`).
