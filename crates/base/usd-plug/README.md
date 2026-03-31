# usd-plug: Port Status

Session: 2026-03-16, branch `dev`

## Status: ALL PHASES COMPLETE, ALL ISSUES RESOLVED, 0 errors, 0 warnings

## Test Coverage: 88 tests (40 unit + 16 integration + 32 testPlug.py port)

## Done

### Phase 1: Core (metadata, no code loading)
- [x] 1.1 plugin.rs — PlugPlugin struct, metadata, types, resources
- [x] 1.2 info.rs — plugInfo.json parser with comment stripping
- [x] 1.3 info.rs — Wildcard/glob support (*, **)
- [x] 1.4 init_config.rs — PXR_PLUGINPATH_NAME bootstrap
- [x] 1.5 registry.rs — PlugRegistry singleton

### Phase 2: Type Integration
- [x] 2.1 notice.rs — DidRegisterPlugins callback
- [x] 2.2 Type declaration from metadata — bases, alias, TfType registration
- [x] 2.3 Type query methods — find/get_derived/demand

### Phase 3: Code Loading
- [x] 3.1 Load() — libloading, dependency resolution, cycle detection, base-type validation
- [x] 3.2 interface_factory.rs — PlugInterfaceFactory + SingletonFactory
- [x] 3.3 static_interface.rs — PlugStaticInterface lazy loading

### Phase 4: Consumer Wiring
- [x] 4.1 KindRegistry — reads "Kinds" from plugin metadata
- [x] 4.2 SdfSchema — reads "SdfMetadata" from plugin metadata
- [x] 4.3 UsdGeom metrics — reads "UsdGeomMetrics.upAxis" from plugins
- [x] 4.4 FileFormatRegistry — plugin discovery with PluginFormatInfo metadata
- [x] 4.5 AR resolver — DEFERRED (no URI resolver plugins; built-in ResolverRegistry sufficient)

### Phase 5: Test Suite (port of testPlug.py)
- [x] 5.1 test_plug.rs — 32 integration tests ported from C++ testPlug.py
- [x] 5.2 Test fixtures — dso_plugins, unloadable, dependencies, dep_bad_*, dep_cycle, empty_info, incomplete
- [x] 5.3 Registration tests — multi-plugin, type hierarchy, TfType integration, notification
- [x] 5.4 Metadata access — vectorInt/String/Double, Int, Double, bases, description, notLoadable
- [x] 5.5 Error cases — nonexistent path, empty/invalid JSON, incomplete, unknown type, demand panics, re-registration
- [x] 5.6 Dependency tests — plugin dependencies, bad base, bad dep, bad dep2, bad load, cycle detection
- [x] 5.7 Alias declaration — TfType alias from plugInfo.json metadata
- [x] 5.8 Resource plugin semantics — always loaded, load() is noop
- [x] 5.9 Library plugin loading — nonexistent DSO fails, empty path (static link) succeeds

### Resolved Issues
- [x] FIXED: Non-recursive Mutex in Load() — moved LOAD_MUTEX to _load() only,
  serializing library loading without blocking dependency resolution. Eliminates
  deadlock risk between LOAD_MUTEX and registry RwLock.
- [x] FIXED: Multi-plugin registration from single plugInfo.json — resource plugins
  sharing a directory path were blocked by path-based dedup. Changed to use
  name-based dedup for resource plugins (path:name compound key).

### C++ Files Not Ported (by design)
- debugCodes.h/cpp — C++ TfDebug symbol registration; Rust uses `log` crate targets instead
- thisPlugin.h — C++ macro for PlugThisPlugin; not applicable in Rust
- module.cpp — C++ Python module init; not applicable
- wrapNotice/Plugin/Registry/TestPlugBase.cpp — Python bindings; not applicable
- testenv/TestPlugDso*.cpp — C++ DSO test implementations; Rust tests use resource plugins
  with identical plugInfo.json metadata to validate the same registration/hierarchy logic
- testenv/TestPlugModule*__init__.py — Python module implementations; Rust tests use
  resource plugins with the same type declarations
