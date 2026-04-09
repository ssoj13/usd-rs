# pxr — Pure Rust OpenUSD Python bindings (usd-rs)
#
# Drop-in for Pixar's `pxr` package: `import pxr` after installing this wheel.

from importlib import import_module

from pxr._usd import (
    Tf,
    Gf,
    Vt,
    Trace,
    Ar,
    Plug,
    Kind,
    Sdf,
    Pcp,
    Ts,
    Usd,
    UsdGeom,
    UsdShade,
    UsdLux,
    UsdSkel,
    Cli,
)

UsdUtils = import_module("pxr.UsdUtils")
Sdr = import_module("pxr.Sdr")

__all__ = [
    "Tf",
    "Gf",
    "Vt",
    "Trace",
    "Ar",
    "Plug",
    "Kind",
    "Sdf",
    "Pcp",
    "Ts",
    "Usd",
    "UsdGeom",
    "UsdShade",
    "UsdLux",
    "UsdSkel",
    "Cli",
    "UsdUtils",
    "Sdr",
]
