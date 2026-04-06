import pytest

try:
    import pxr_rs
except ImportError:
    def pytest_collection_modifyitems(items):
        skip = pytest.mark.skip(reason="pxr_rs not installed")
        for item in items:
            item.add_marker(skip)
