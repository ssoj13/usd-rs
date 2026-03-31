#!/usr/bin/env python3
"""
Run cargo fuzz with MSVC env (Windows, via vcv-rs submodule).
Cross-platform: on Windows by default builds without sanitizers (avoids
sanitizer_cov_trace_pc_guard_cleanup / ASan DLL ABI mismatch) and runs the
harness on the corpus; set DRACO_FUZZ_USE_CARGO_FUZZ=1 to use cargo fuzz run
(requires matching ASan/compiler-rt). On other platforms runs cargo fuzz directly.

Usage: python scripts/fuzz_run.py [target] [-- fuzz-args...]
Example: python scripts/fuzz_run.py mesh_decode -- -runs=100

Run from crates/draco-rs or workspace root.
"""

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Optional

# Sample .drc from test/ to seed corpus when empty (per target type).
# mesh: skip cube_att.drc (different encoding); use edgebreaker/ref-format files.
_CORPUS_SEED_MESH = [
    "cube_att.obj.edgebreaker.cl4.2.2.drc",
    "bunny_gltf.drc",
    "car.drc",
    "octagon_preserved.drc",
]
_CORPUS_SEED_POINT_CLOUD = [
    "cube_pc.drc",
    "pc_color.drc",
    "pc_kd_color.drc",
    "point_cloud_no_qp.drc",
]


def script_dir() -> Path:
    return Path(__file__).resolve().parent


def crate_root() -> Path:
    return script_dir().parent


def workspace_root() -> Path:
    return crate_root().resolve().parent.parent


def is_windows() -> bool:
    return sys.platform == "win32"


def apply_vcv_rs_env(workspace: Path) -> bool:
    """Run vcv-rs -f json and apply PATH, INCLUDE, LIB, LIBPATH to os.environ. Return True if applied."""
    manifest = workspace / "Cargo.toml"
    if not manifest.exists():
        return False
    result = subprocess.run(
        [
            "cargo", "run", "-p", "vcv-rs", "--release",
            "--manifest-path", str(manifest),
            "--", "-q", "-f", "json",
        ],
        capture_output=True,
        text=True,
        cwd=str(workspace),
    )
    if result.returncode != 0 or not result.stdout.strip():
        return False
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return False
    path_list = data.get("PATH", [])
    if path_list:
        prefix = os.pathsep.join(path_list)
        os.environ["PATH"] = prefix + os.pathsep + os.environ.get("PATH", "")
    for var, key in [("INCLUDE", "INCLUDE"), ("LIB", "LIB"), ("LIBPATH", "LIBPATH")]:
        lst = data.get(key, [])
        if lst:
            prefix = os.pathsep.join(lst)
            os.environ[var] = prefix + os.pathsep + os.environ.get(var, "")
    for key, value in data.items():
        if key not in ("PATH", "INCLUDE", "LIB", "LIBPATH") and isinstance(value, str):
            os.environ[key] = value
    return True


def find_asan_dll_dir() -> Optional[Path]:
    """Return directory containing clang_rt.asan_dynamic-x86_64.dll, or None."""
    dll_name = "clang_rt.asan_dynamic-x86_64.dll"
    if os.environ.get("ASAN_DLL_DIR"):
        d = Path(os.environ["ASAN_DLL_DIR"])
        if (d / dll_name).exists():
            return d
        return None
    # Rust sysroot
    try:
        out = subprocess.run(
            ["rustc", "+nightly", "--print", "sysroot"],
            capture_output=True, text=True, check=True,
        )
        sysroot = Path(out.stdout.strip())
        try_path = sysroot / "lib" / "rustlib" / "x86_64-pc-windows-msvc" / "lib"
        if (try_path / dll_name).exists():
            return try_path
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    # Program Files LLVM
    llvm_base = Path(os.environ.get("ProgramFiles", "C:\\Program Files")) / "LLVM" / "lib" / "clang"
    if llvm_base.exists():
        for ver_dir in sorted(llvm_base.iterdir(), key=lambda p: p.name, reverse=True):
            if ver_dir.is_dir():
                try_path = ver_dir / "lib" / "windows"
                if (try_path / dll_name).exists():
                    return try_path
    return None


def seed_corpus_if_empty(crate: Path, target: str) -> None:
    """If corpus dir is empty, copy sample .drc from test/ into it (mesh vs point cloud)."""
    corpus_dir = crate / "fuzz" / "corpus" / target
    corpus_dir.mkdir(parents=True, exist_ok=True)
    if any(corpus_dir.iterdir()):
        return
    test_dir = crate / "test"
    if not test_dir.is_dir():
        return
    names = _CORPUS_SEED_POINT_CLOUD if target == "point_cloud_decode" else _CORPUS_SEED_MESH
    copied = 0
    for name in names:
        src = test_dir / name
        if src.is_file():
            shutil.copy2(src, corpus_dir / name)
            copied += 1
    if copied:
        print(f"Seeded corpus with {copied} .drc from test/", file=sys.stderr)


def parse_args(argv: list[str]) -> tuple[str, list[str]]:
    target = "mesh_decode"
    extra: list[str] = []
    i = 0
    while i < len(argv):
        if argv[i] == "--":
            extra = argv[i + 1:]
            break
        if i == 0 and not argv[i].startswith("-"):
            target = argv[i]
        i += 1
    return target, extra


def main() -> int:
    crate = crate_root()
    workspace = workspace_root()
    target, extra = parse_args(sys.argv[1:])

    # On Windows, cargo fuzz injects -Z sanitizer=address and coverage; the resulting
    # exe expects sanitizer_cov_* from a DLL. A different LLVM's clang_rt.asan_dynamic
    # often lacks that symbol (ABI mismatch). So by default we build without cargo fuzz
    # (no sanitizers) and run the harness on the corpus.
    use_cargo_fuzz_on_windows = os.environ.get("DRACO_FUZZ_USE_CARGO_FUZZ", "").strip() == "1"

    if is_windows():
        if (workspace / "crates" / "vcv-rs" / "Cargo.toml").exists():
            if apply_vcv_rs_env(workspace):
                print("MSVC env applied (vcv-rs from crates/vcv-rs).", file=sys.stderr)
        if use_cargo_fuzz_on_windows:
            asan_dir = find_asan_dll_dir()
            if asan_dir is not None:
                os.environ["PATH"] = str(asan_dir) + os.pathsep + os.environ.get("PATH", "")
                print(f"Using ASan runtime: {asan_dir}", file=sys.stderr)
            else:
                print(
                    "Note: DRACO_FUZZ_USE_CARGO_FUZZ=1 but clang_rt.asan_dynamic-x86_64.dll not found. "
                    "You may get STATUS_DLL_NOT_FOUND or entry point errors.",
                    file=sys.stderr,
                )
        else:
            # Build standalone corpus runner (no libFuzzer entry point) and run on corpus dir.
            corpus_bin = f"{target}_corpus"
            manifest = workspace / "Cargo.toml"
            build_cmd = [
                "cargo", "build", "--release",
                "-p", "draco-rs-fuzz", "--bin", corpus_bin,
                "--manifest-path", str(manifest),
            ]
            print(" ".join(build_cmd), file=sys.stderr)
            r = subprocess.run(build_cmd, cwd=str(workspace))
            if r.returncode != 0:
                return r.returncode
            exe = workspace / "target" / "release" / f"{corpus_bin}.exe"
            if not exe.exists():
                exe = workspace / "target" / "x86_64-pc-windows-msvc" / "release" / f"{corpus_bin}.exe"
            if not exe.exists():
                print(f"Error: {corpus_bin}.exe not found after build.", file=sys.stderr)
                return 1
            corpus = crate / "fuzz" / "corpus" / target
            seed_corpus_if_empty(crate, target)
            run_cmd = [str(exe), str(corpus)] + extra
            print(" ".join(run_cmd), file=sys.stderr)
            return subprocess.run(run_cmd).returncode

    seed_corpus_if_empty(crate, target)
    cmd = ["cargo", "fuzz", "run", target] + extra
    print(" ".join(cmd), file=sys.stderr)
    result = subprocess.run(cmd, cwd=str(crate))
    return result.returncode


if __name__ == "__main__":
    sys.exit(main())
