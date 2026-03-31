# draco-maya

C ABI bridge for the Draco Rust port, matching `_ref/draco/src/draco/maya/draco_maya_plugin.*`.

## Build

```bash
cargo build -p draco-maya --release
```

The shared library is produced at:

- `target/release/draco_maya.dll` (Windows)
- `target/release/libdraco_maya.so` (Linux)
- `target/release/libdraco_maya.dylib` (macOS)

## C ABI

Exports:
- `drc2py_decode(char *data, unsigned int length, Drc2PyMesh **res_mesh)`
- `drc2py_free(Drc2PyMesh **res_mesh)`
- `drc2py_encode(Drc2PyMesh *in_mesh, char *file_path)`

Struct layout (must match the C++ reference):

```c
struct Drc2PyMesh {
  int faces_num;
  int *faces;
  int vertices_num;
  float *vertices;
  int normals_num;
  float *normals;
  int uvs_num;
  int uvs_real_num;
  float *uvs;
};
```

## Python ctypes example

See `examples/maya_ctypes_example.py` for a minimal encode/decode round-trip using `ctypes`.
