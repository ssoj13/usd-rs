"""Minimal ctypes usage for draco_maya.dll.

This mirrors the C ABI in `_ref/draco/src/draco/maya/draco_maya_plugin.h`.
"""
from __future__ import annotations

import ctypes
from pathlib import Path


class Drc2PyMesh(ctypes.Structure):
    _fields_ = [
        ("faces_num", ctypes.c_int),
        ("faces", ctypes.POINTER(ctypes.c_int)),
        ("vertices_num", ctypes.c_int),
        ("vertices", ctypes.POINTER(ctypes.c_float)),
        ("normals_num", ctypes.c_int),
        ("normals", ctypes.POINTER(ctypes.c_float)),
        ("uvs_num", ctypes.c_int),
        ("uvs_real_num", ctypes.c_int),
        ("uvs", ctypes.POINTER(ctypes.c_float)),
    ]


def load_lib() -> ctypes.CDLL:
    # Adjust path as needed (default: workspace target/release).
    dll_path = (
        Path(__file__).resolve().parents[3]
        / "target"
        / "release"
        / "draco_maya.dll"
    )
    lib = ctypes.CDLL(str(dll_path))

    lib.drc2py_decode.argtypes = [
        ctypes.c_char_p,
        ctypes.c_uint,
        ctypes.POINTER(ctypes.POINTER(Drc2PyMesh)),
    ]
    lib.drc2py_decode.restype = ctypes.c_int

    lib.drc2py_encode.argtypes = [ctypes.POINTER(Drc2PyMesh), ctypes.c_char_p]
    lib.drc2py_encode.restype = ctypes.c_int

    lib.drc2py_free.argtypes = [ctypes.POINTER(ctypes.POINTER(Drc2PyMesh))]
    lib.drc2py_free.restype = None
    return lib


def encode_triangle(lib: ctypes.CDLL, out_path: Path) -> None:
    faces = (ctypes.c_int * 3)(0, 1, 2)
    vertices = (ctypes.c_float * 9)(
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    )

    mesh = Drc2PyMesh()
    mesh.faces_num = 1
    mesh.faces = ctypes.cast(faces, ctypes.POINTER(ctypes.c_int))
    mesh.vertices_num = 3
    mesh.vertices = ctypes.cast(vertices, ctypes.POINTER(ctypes.c_float))
    mesh.normals_num = 0
    mesh.normals = None
    mesh.uvs_num = 0
    mesh.uvs_real_num = 0
    mesh.uvs = None

    result = lib.drc2py_encode(ctypes.byref(mesh), str(out_path).encode("utf-8"))
    if result != 0:
        raise RuntimeError(f"encode failed: {result}")


def decode_mesh(lib: ctypes.CDLL, in_path: Path) -> None:
    data = in_path.read_bytes()
    buf = ctypes.create_string_buffer(data, len(data))
    mesh_ptr = ctypes.POINTER(Drc2PyMesh)()

    result = lib.drc2py_decode(buf, len(data), ctypes.byref(mesh_ptr))
    if result != 0:
        raise RuntimeError(f"decode failed: {result}")

    mesh = mesh_ptr.contents
    faces_count = mesh.faces_num * 3
    verts_count = mesh.vertices_num * 3

    faces = ctypes.cast(mesh.faces, ctypes.POINTER(ctypes.c_int * faces_count)).contents
    vertices = ctypes.cast(
        mesh.vertices, ctypes.POINTER(ctypes.c_float * verts_count)
    ).contents

    print("faces:", list(faces))
    print("vertices:", list(vertices))

    lib.drc2py_free(ctypes.byref(mesh_ptr))


def main() -> None:
    lib = load_lib()
    out_path = Path("maya_triangle.drc")
    encode_triangle(lib, out_path)
    decode_mesh(lib, out_path)


if __name__ == "__main__":
    main()
