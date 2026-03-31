# draco-unity

C ABI bridge for the Draco Rust port, matching `_ref/draco/src/draco/unity/draco_unity_plugin.*`.

## Build

```bash
cargo build -p draco-unity --release
```

The shared library is produced at:

- `target/release/draco_unity.dll` (Windows)
- `target/release/libdraco_unity.so` (Linux)
- `target/release/libdraco_unity.dylib` (macOS)

## C ABI (Exports)

- `DecodeDracoMesh`
- `GetAttribute`
- `GetAttributeByType`
- `GetAttributeByUniqueId`
- `GetMeshIndices`
- `GetAttributeData`
- `ReleaseDracoMesh`
- `ReleaseDracoAttribute`
- `ReleaseDracoData`
- `DecodeMeshForUnity` (deprecated)
- `ReleaseUnityMesh` (deprecated)

## ABI Structs

`DracoMesh`, `DracoAttribute`, and `DracoData` mirror the reference plugin and are
intended for use from Unity native interop (C# P/Invoke).
