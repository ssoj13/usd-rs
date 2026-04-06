#!/usr/bin/env python3
"""
bootstrap.py - Build script for usd-rs.

Commands:
    build, b              Build all (release by default)
    build python, b p     Build Python bindings (usd-pyo3 via maturin)

    test, t               Run tests
    test python, t p      Run Python binding tests
    check, ch             Run clippy + fmt check
    clean                 Clean build artifacts

Options:
    --debug, -d           Build in debug mode (default: release)
    --verbose, -v         Verbose output

Usage:
    python bootstrap.py b             # Build everything (release)
    python bootstrap.py b p           # Build Python bindings
    python bootstrap.py b -d          # Build all in debug
    python bootstrap.py t             # Run tests
    python bootstrap.py ch            # Clippy + fmt
"""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

# ============================================================
# CONSTANTS
# ============================================================

ROOT = Path(__file__).parent.resolve()
PYO3_CRATE = "crates/usd-pyo3"
PYO3_MANIFEST = f"{PYO3_CRATE}/Cargo.toml"

# ============================================================
# COLORS
# ============================================================

class C:
    """ANSI colors."""
    RST = "\033[0m"
    RED = "\033[91m"
    GRN = "\033[92m"
    YLW = "\033[93m"
    CYN = "\033[96m"
    WHT = "\033[97m"

    @classmethod
    def init(cls) -> None:
        if platform.system() == "Windows":
            os.system("")


def fmt_time(ms: float) -> str:
    if ms < 1000:
        return f"{ms:.0f}ms"
    if ms < 60_000:
        return f"{ms / 1000:.1f}s"
    m = int(ms // 60_000)
    s = (ms % 60_000) / 1000
    return f"{m}m{s:.0f}s"


def header(text: str) -> None:
    ln = "=" * 60
    print(f"\n{C.CYN}{ln}\n{text}\n{ln}{C.RST}")


def step(text: str) -> None:
    print(f"  {C.WHT}{text}{C.RST}")


def ok(text: str) -> None:
    print(f"  {C.GRN}{text}{C.RST}")


def err(text: str) -> None:
    print(f"  {C.RED}{text}{C.RST}")


def warn(text: str) -> None:
    print(f"  {C.YLW}{text}{C.RST}")


# ============================================================
# HELPERS
# ============================================================

def run(args: list[str], cwd: Path | None = None,
        capture: bool = False) -> tuple[int, str, float]:
    """Run command, return (exit_code, output, time_ms)."""
    t0 = time.perf_counter()
    r = subprocess.run(args, cwd=cwd or ROOT, capture_output=capture, text=True)
    ms = (time.perf_counter() - t0) * 1000
    out = (r.stdout or "") + (r.stderr or "") if capture else ""
    return r.returncode, out, ms


def is_win() -> bool:
    return platform.system() == "Windows"


def target_dir(debug: bool) -> Path:
    return ROOT / "target" / ("debug" if debug else "release")


def mode_str(debug: bool) -> str:
    return "debug" if debug else "release"


def has_maturin() -> bool:
    return shutil.which("maturin") is not None


# ============================================================
# BUILD COMMANDS
# ============================================================

def build_all(debug: bool) -> int:
    """Build entire workspace."""
    header("BUILD ALL")
    step(f"Mode: {mode_str(debug)}")
    print()
    step("Building workspace...")

    cmd = ["cargo", "build"]
    if not debug:
        cmd.append("--release")

    code, _, ms = run(cmd)
    print()
    if code == 0:
        ok(f"Build OK ({fmt_time(ms)})")
    else:
        err("Build FAILED")
    print()
    return code



def build_python(debug: bool) -> int:
    """Build Python bindings via maturin."""
    header("BUILD PYTHON BINDINGS")

    if not has_maturin():
        err("maturin not found!")
        print()
        step("Install:")
        step("  pip install maturin")
        step("  or: pipx install maturin")
        print()
        return 1

    manifest = ROOT / PYO3_MANIFEST
    if not manifest.exists():
        warn(f"PyO3 crate not yet created: {PYO3_MANIFEST}")
        step("Run bootstrap setup or create the crate first.")
        print()
        return 1

    step(f"Mode: {mode_str(debug)}")
    step(f"Package: usd-pyo3 (via maturin)")
    print()
    step("Building...")

    cmd = ["maturin", "build", "-m", str(manifest)]
    if not debug:
        cmd.append("--release")

    code, output, ms = run(cmd, capture=True)
    print()
    if code == 0:
        ok(f"Build OK ({fmt_time(ms)})")
        # Show wheel location
        wheels = ROOT / "target" / "wheels"
        if wheels.exists():
            whl = sorted(wheels.glob("*.whl"), key=lambda p: p.stat().st_mtime)
            if whl:
                step(f"Wheel: {whl[-1].name}")
    else:
        err("Build FAILED")
        if output:
            for line in output.strip().split("\n")[-10:]:
                step(line)
    print()
    return code


# ============================================================
# TEST / CHECK COMMANDS
# ============================================================

def run_tests(debug: bool, target: str | None = None) -> int:
    """Run tests."""
    header("TEST")

    if target == "python":
        step("Running Python binding tests...")
        manifest = ROOT / PYO3_MANIFEST
        if not manifest.exists():
            warn("PyO3 crate not yet created")
            return 1
        cmd = ["maturin", "develop", "-m", str(manifest)]
        if not debug:
            cmd.append("--release")
        code, _, ms = run(cmd)
        if code != 0:
            err("maturin develop failed")
            return code
        code, _, ms = run([sys.executable, "-m", "pytest", PYO3_CRATE])
        print()
        if code == 0:
            ok(f"Tests OK ({fmt_time(ms)})")
        else:
            err("Tests FAILED")
        return code

    step(f"Mode: {mode_str(debug)}")
    step("Running cargo test...")
    print()

    cmd = ["cargo", "test"]
    if not debug:
        cmd.append("--release")

    code, _, ms = run(cmd)
    print()
    if code == 0:
        ok(f"Tests OK ({fmt_time(ms)})")
    else:
        err("Tests FAILED")
    print()
    return code


def run_check() -> int:
    """Run clippy + fmt check."""
    header("CHECK")

    step("Running cargo fmt --check...")
    code, out, _ = run(["cargo", "fmt", "--check"], capture=True)
    if code != 0:
        err("fmt check failed")
        if out:
            for line in out.strip().split("\n")[:10]:
                step(line)
        return code
    ok("fmt OK")

    print()
    step("Running cargo clippy...")
    code, _, ms = run([
        "cargo", "clippy", "--all-targets", "--all-features",
        "--", "-D", "warnings",
    ])
    print()
    if code == 0:
        ok(f"clippy OK ({fmt_time(ms)})")
    else:
        err("clippy FAILED")
    print()
    return code


def run_clean() -> int:
    """Clean build artifacts."""
    header("CLEAN")
    step("Running cargo clean...")
    code, _, ms = run(["cargo", "clean"])
    if code == 0:
        ok(f"Clean OK ({fmt_time(ms)})")
    else:
        err("Clean FAILED")
    print()
    return code


# ============================================================
# TARGET ALIASES
# ============================================================

BUILD_ALIASES: dict[str, str] = {
    "p": "python",
}

TEST_ALIASES: dict[str, str] = {
    "p": "python",
}


def resolve(aliases: dict[str, str], key: str | None) -> str | None:
    if key is None:
        return None
    return aliases.get(key, key)


# ============================================================
# MAIN
# ============================================================

def main() -> int:
    C.init()

    parser = argparse.ArgumentParser(
        prog="bootstrap",
        description="usd-rs build script",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--debug", "-d", action="store_true",
                        help="Build in debug mode (default: release)")
    parser.add_argument("--verbose", "-v", action="store_true",
                        help="Verbose output")

    sub = parser.add_subparsers(dest="cmd")

    # build
    p_build = sub.add_parser("build", aliases=["b"], help="Build")
    p_build.add_argument("target", nargs="?", help="p=python")

    # test
    p_test = sub.add_parser("test", aliases=["t"], help="Run tests")
    p_test.add_argument("target", nargs="?", help="p=python")

    # check
    sub.add_parser("check", aliases=["ch"], help="Clippy + fmt")

    # clean
    sub.add_parser("clean", help="Clean artifacts")

    args = parser.parse_args()
    debug = args.debug

    if args.cmd in ("build", "b"):
        t = resolve(BUILD_ALIASES, getattr(args, "target", None))
        if t == "python":
            return build_python(debug)
        return build_all(debug)

    if args.cmd in ("test", "t"):
        t = resolve(TEST_ALIASES, getattr(args, "target", None))
        return run_tests(debug, t)

    if args.cmd in ("check", "ch"):
        return run_check()

    if args.cmd == "clean":
        return run_clean()

    parser.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
