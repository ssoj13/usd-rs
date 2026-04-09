"""pxr.Sdr — merges `pxr._usd.SdrNative` with pure-Python test utilities."""

from __future__ import annotations

from pxr._usd import SdrNative as _native

_g = globals()
for _name in dir(_native):
    if _name.startswith("_"):
        continue
    _g[_name] = getattr(_native, _name)


def __getattr__(name: str):
    if name == "shaderParserTestUtils":
        from . import shaderParserTestUtils as m

        return m
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


__all__ = [n for n in _g if not n.startswith("_")] + ["shaderParserTestUtils"]
