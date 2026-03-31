# HDGP - Hydra Generative Procedurals

**Status**: Full parity with C++ `pxr/imaging/hdGp`

Port of `pxr/imaging/hdGp` from OpenUSD.

## Overview

HdGp provides the framework for generative procedural prims in Hydra. Generative procedurals are scene prims that, when evaluated, produce child prims dynamically. This is used for deferred geometry generation, instancing expansions, and procedural content pipelines.

The system identifies prims typed as `hydraGenerativeProcedural`, evaluates them via registered plugins, and exposes their generated children through the scene index chain.

## Module Structure

```
usd-hd-gp/
  src/
    lib.rs                                             # Module exports and re-exports
    generative_procedural.rs                           # Core trait + tokens + types
    generative_procedural_filtering_scene_index.rs     # Filtering by procedural type
    generative_procedural_plugin.rs                    # Plugin trait (abstract base)
    generative_procedural_plugin_registry.rs           # Singleton plugin registry
    generative_procedural_resolving_scene_index.rs     # Main evaluation engine
    scene_index_plugin.rs                              # HdSceneIndexPlugin integration
```

## Components

### HdGpGenerativeProcedural (trait)

Core trait for implementing generative procedurals. Matches C++ `HdGpGenerativeProcedural` abstract base class.

- `update_dependencies()` -- declare input dependencies
- `update()` -- evaluate procedural, produce child prim types
- `get_child_prim()` -- return data for a generated child prim
- `async_begin()` / `async_update()` -- optional async evaluation

### HdGpGenerativeProceduralFilteringSceneIndex

Scene index filter that re-types procedural prims based on their `hdGp:proceduralType` primvar. Prims matching an allowed type list are re-typed to `_allowedPrimTypeName`; non-matching prims are re-typed to `_skippedPrimTypeName`. Non-procedural prims pass through unchanged.

Reads `hdGp:proceduralType` from `HdPrimvarsSchema`.

### HdGpGenerativeProceduralResolvingSceneIndex

Main evaluation engine. Identifies procedural prims, constructs plugin instances via the registry, evaluates them, and exposes generated children.

Key features:
- **Dependency tracking** -- procedurals declare dependencies; dirtying a dependency re-cooks the procedural
- **Parallel cooking** -- uses `rayon` with threshold=2 (matching C++ `WorkParallelForEach`)
- **Async support** -- `system_message()` handles `asyncAllow`/`asyncPoll` for non-blocking evaluation
- **Hierarchy management** -- intermediate namespace prims are automatically created/removed
- **Re-typing** -- evaluated procedurals become `resolvedHydraGenerativeProcedural` to prevent double-evaluation

### HdGpGenerativeProceduralPlugin (trait)

Plugin interface for constructing procedural instances. Extends `HfPluginBase`.

### HdGpGenerativeProceduralPluginRegistry

Singleton registry managing plugin discovery and instantiation. Supports:
- `register<T>()` -- type-safe registration with automatic factory wiring
- `register_with_factory()` -- explicit factory closure registration
- `construct_procedural()` -- lookup by plugin ID or display name (C++ parity)

### HdGpSceneIndexPlugin

Integration with `HdSceneIndexPluginRegistry`. Reads `proceduralPrimTypeName` from `inputArgs` to support custom procedural type configuration per plugin instance.

## Tokens

| Token | Value | Purpose |
|-------|-------|---------|
| `GENERATIVE_PROCEDURAL` | `hydraGenerativeProcedural` | Default procedural prim type |
| `RESOLVED_GENERATIVE_PROCEDURAL` | `resolvedHydraGenerativeProcedural` | Post-evaluation type |
| `SKIPPED_GENERATIVE_PROCEDURAL` | `skippedHydraGenerativeProcedural` | Filtered-out type |
| `PROCEDURAL_TYPE` | `hdGp:proceduralType` | Primvar key for proc type |
| `ANY_PROCEDURAL_TYPE` | `*` | Wildcard for filtering |

## Type Aliases

- `DependencyMap` = `HashMap<SdfPath, HdDataSourceLocatorSet>` -- input dependencies
- `ChildPrimTypeMap` = `HashMap<SdfPath, TfToken>` -- generated child paths and types

## Tests

38 unit tests covering:
- Token values and enum variants
- Filtering scene index (skip/allow/ignore logic, fast-path optimization)
- Plugin registry (singleton, register, construct by ID/display name)
- Resolving scene index (creation, get_prim fallthrough, child paths, path dedup)
- Scene index plugin (creation, insertion phase, default)

## Dependencies

- `usd-hd` -- Hydra scene index, data source, schema infrastructure
- `usd-hf` -- Plugin framework (HfPluginBase, HfPluginRegistry)
- `usd-sdf` -- SdfPath
- `usd-tf` -- TfToken
- `once_cell` -- Lazy statics for cached tokens and locator sets
- `rayon` -- Parallel procedural cooking (threshold=2)
