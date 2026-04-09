"""pxr.UsdUtils — merges `pxr._usd.UsdUtilsNative` with pure-Python helpers."""

from pxr._usd import UsdUtilsNative as _native

_g = globals()
for _name in dir(_native):
    if _name.startswith("_"):
        continue
    _g[_name] = getattr(_native, _name)

from . import constantsGroup

__all__ = [n for n in _g if not n.startswith("_")]
