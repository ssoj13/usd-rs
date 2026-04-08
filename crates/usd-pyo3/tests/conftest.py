"""pytest conftest for pxr_rs Python binding tests.

Ported from OpenUSD testenv/ — adapted for pxr_rs package.
"""
import os
from pathlib import Path

import pytest


# ---------------------------------------------------------------------------
# Auto-chdir into test data directories
# ---------------------------------------------------------------------------
# OpenUSD tests assume cwd is a testenv directory with data files.
# Convention: testFoo.py  →  testFoo/ or testFoo.testenv/ as data dir.
# This hook changes cwd before each test module is collected and restores it
# afterwards, so module-level code like `Usd.Stage.Open("./Test.usda")` works.

def pytest_collectstart(collector):
    """Change cwd to a test module's data dir right before it is collected.

    OpenUSD tests assume cwd contains their test data. The convention is:
    testFoo.py  ->  testFoo/ or testFoo.testenv/
    We chdir before each module is collected (and thus imported).
    """
    if not hasattr(collector, "path"):
        return
    module_path = collector.path
    if module_path.suffix != ".py":
        return
    stem = module_path.stem
    test_dir = module_path.parent
    for candidate in (test_dir / stem, test_dir / f"{stem}.testenv"):
        if candidate.is_dir():
            os.chdir(candidate)
            return


@pytest.fixture(autouse=True)
def _usd_test_chdir(request, tmp_path):
    """Per-test fixture: run in test data dir or tmpdir (never project root)."""
    test_file = Path(request.fspath)
    stem = test_file.stem
    test_dir = test_file.parent
    saved = os.getcwd()
    # Try test-specific data dir first
    for candidate in (test_dir / stem, test_dir / f"{stem}.testenv"):
        if candidate.is_dir():
            os.chdir(candidate)
            yield
            os.chdir(saved)
            return
    # No data dir — use tmpdir so tests don't pollute project root
    os.chdir(tmp_path)
    yield
    os.chdir(saved)


# Skip files that call sys.exit() or argparse at module level
collect_ignore_glob = [
    # Crash handler tests (sys.exit at import)
    "**/testTfCrashHandler.py",
    # C++ test infrastructure — depends on C++ pybind test classes (_TestBase,
    # Tf_TestPyOptionalStd) that don't exist in the pure-Rust port
    "**/testTfPython.py",
    "**/testTfPyNotice.py",
    "**/testTfPyOptional.py",
    "**/testTfPyDllLink.py",
    # ScriptModuleLoader AAA_RaisesError deliberately raises at import
    "**/testTfScriptModuleLoader_AAA_RaisesError.py",
    # ScriptModuleLoader main test requires Python package __package__ plumbing
    # to dynamically import sibling test modules — C++ test infrastructure
    "**/testTfScriptModuleLoader.py",
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
    # Plug test fixture modules (loaded by testPlug.py, not standalone tests)
    "**/plug/TestPlug*__init__.py",
    # Trace module not yet ported
    "**/trace/**",
    # Module-level variant editing logic needs full API surface (GetVariantEditContext etc.)
    "**/testUsdVariantEditing.py",
    # Scripts that use argparse at module level
    "**/testUsdBakeMtlx.py",
    "**/testSdrCompliance*.py",
    "**/testArOptionalImplementation.py",
    "**/testPcpCompositionResults.py",
    # Requires test env vars not available outside C++ testenv infrastructure
    "**/testPcpDynamicFileFormatPlugin.py",
]

try:
    import pxr_rs
except ImportError:
    def pytest_collection_modifyitems(items):
        skip = pytest.mark.skip(reason="pxr_rs not installed")
        for item in items:
            item.add_marker(skip)
