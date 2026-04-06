"""pytest conftest for pxr_rs Python binding tests.

Ported from OpenUSD testenv/ — adapted for pxr_rs package.
"""
import pytest

# Skip files that call sys.exit() or argparse at module level
collect_ignore_glob = [
    # Crash handler tests (sys.exit at import)
    "**/testTfCrashHandler.py",
    # CLI scripts with argparse (not unittest)
    "**/testUsdFlatten*.py",
    "**/testUsdGenSchema*.py",
    "**/testUsdStitch*.py",
    "**/testUsdRecord*.py",
    # usdview integration tests (require Qt/GUI)
    "**/testusdview/**",
    # Helper/utility scripts (not tests)
    "**/create_symlinks.py",
    "**/__init__.py",
    # Scripts that use argparse at module level
    "**/testUsdBakeMtlx.py",
    "**/testSdrCompliance*.py",
]

try:
    import pxr_rs
except ImportError:
    def pytest_collection_modifyitems(items):
        skip = pytest.mark.skip(reason="pxr_rs not installed")
        for item in items:
            item.add_marker(skip)
