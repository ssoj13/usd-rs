# HDSI - Hydra Scene Index Utilities

**Status**: ✅ IMPLEMENTED — 27/27 scene indices fully ported; `nodeIdentifierResolvingSceneIndex` complete

Port of `pxr/imaging/hdsi` from OpenUSD.

## Overview

HDSI provides scene index filters and utilities for the Hydra rendering framework. Scene indices form a filtering chain that transforms scene data for consumption by render delegates.

## Module Structure

```
hdsi/
├── tokens.rs                                    # Token definitions
├── utils.rs                                     # Utility functions
├── compute_scene_index_diff.rs                 # Diff computation
├── debugging_scene_index.rs                    # Debug logging filter
├── prim_managing_scene_index_observer.rs       # Prim lifecycle observer
│
├── Scene Index Filters (27 total):
│   ├── coord_sys_prim_scene_index.rs           # Coordinate system prims
│   ├── implicit_surface_scene_index.rs         # Implicit surface conversion
│   ├── light_linking_scene_index.rs            # Light linking resolution
│   ├── material_binding_resolving_scene_index.rs
│   ├── prim_type_pruning_scene_index.rs
│   ├── render_settings_filtering_scene_index.rs
│   ├── scene_globals_scene_index.rs
│   ├── dome_light_camera_visibility_scene_index.rs
│   ├── ext_computation_dependency_scene_index.rs
│   ├── ext_computation_primvar_pruning_scene_index.rs
│   ├── legacy_display_style_override_scene_index.rs
│   ├── material_override_resolving_scene_index.rs
│   ├── material_primvar_transfer_scene_index.rs
│   ├── material_render_context_filtering_scene_index.rs
│   ├── node_identifier_resolving_scene_index.rs
│   ├── nurbs_approximating_scene_index.rs
│   ├── pinned_curve_expanding_scene_index.rs
│   ├── prefix_path_pruning_scene_index.rs
│   ├── prim_type_and_path_pruning_scene_index.rs
│   ├── prim_type_notice_batching_scene_index.rs
│   ├── render_pass_prune_scene_index.rs
│   ├── scene_material_pruning_scene_index.rs
│   ├── switching_scene_index.rs
│   ├── tet_mesh_conversion_scene_index.rs
│   ├── unbound_material_pruning_scene_index.rs
│   └── velocity_motion_resolving_scene_index.rs
│
├── mod.rs                                       # Module exports
└── tests.rs                                     # Comprehensive tests
```

## Scene Index Filters

### Core Filters

- **HdsiCoordSysPrimSceneIndex** - Creates coordinate system prims for bindings
- **HdsiImplicitSurfaceSceneIndex** - Converts implicit surfaces (sphere, cube, etc.) to mesh
- **HdsiMaterialBindingResolvingSceneIndex** - Resolves material bindings by walking hierarchy
- **HdsiSceneGlobalsSceneIndex** - Manages scene-level globals

### Pruning Filters

- **HdsiPrimTypePruningSceneIndex** - Prunes prims by type (deprecated)
- **HdsiPrimTypeAndPathPruningSceneIndex** - Prunes by type and path predicate
- **HdsiPrefixPathPruningSceneIndex** - Prunes prims by path prefix
- **HdsiSceneMaterialPruningSceneIndex** - Prunes unused materials
- **HdsiUnboundMaterialPruningSceneIndex** - Prunes materials without bindings
- **HdsiRenderPassPruneSceneIndex** - Prunes render pass prims

### Material Filters

- **HdsiMaterialOverrideResolvingSceneIndex** - Resolves material overrides
- **HdsiMaterialPrimvarTransferSceneIndex** - Transfers primvars from materials to geometry
- **HdsiMaterialRenderContextFilteringSceneIndex** - Filters materials by render context

### Light Filters

- **HdsiLightLinkingSceneIndex** - Resolves light/shadow linking from collections
- **HdsiDomeLightCameraVisibilitySceneIndex** - Controls dome light camera visibility

### Computation Filters

- **HdsiExtComputationDependencySceneIndex** - Manages ext computation dependencies
- **HdsiExtComputationPrimvarPruningSceneIndex** - Prunes computed primvars

### Geometry Filters

- **HdsiNurbsApproximatingSceneIndex** - Converts NURBS to mesh approximation
- **HdsiPinnedCurveExpandingSceneIndex** - Expands pinned curves
- **HdsiTetMeshConversionSceneIndex** - Converts tetrahedral meshes

### Other Filters

- **HdsiLegacyDisplayStyleOverrideSceneIndex** - Legacy display style overrides
- **HdsiNodeIdentifierResolvingSceneIndex** - Resolves node identifiers in materials
- **HdsiPrimTypeNoticeBatchingSceneIndex** - Batches change notices by prim type
- **HdsiRenderSettingsFilteringSceneIndex** - Filters by active render settings
- **HdsiSwitchingSceneIndex** - Switches between multiple input scene indices
- **HdsiVelocityMotionResolvingSceneIndex** - Resolves velocity-based motion blur

### Debugging

- **HdsiDebuggingSceneIndex** - Logs all scene index operations for debugging

### Observers

- **HdsiPrimManagingSceneIndexObserver** - Tracks prim lifecycle (add/remove)

## API Design

All scene index filters follow a consistent pattern:

```rust
pub struct HdsiXxxSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    // Filter-specific state
}

impl HdsiXxxSceneIndex {
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        // Create filter
    }
}

impl HdSceneIndexBase for HdsiXxxSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim { ... }
    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector { ... }
    // ... observer management
}

impl FilteringObserverTarget for HdsiXxxSceneIndex {
    fn on_prims_added(&mut self, ...) { ... }
    fn on_prims_removed(&mut self, ...) { ... }
    fn on_prims_dirtied(&mut self, ...) { ... }
    fn on_prims_renamed(&mut self, ...) { ... }
}
```

## Token Groups

All scene index configuration tokens are defined in `tokens.rs`:

- `IMPLICIT_SURFACE_SCENE_INDEX_TOKENS`
- `PRIM_TYPE_PRUNING_SCENE_INDEX_TOKENS`
- `PRIM_TYPE_AND_PATH_PRUNING_SCENE_INDEX_TOKENS`
- `PRIM_TYPE_NOTICE_BATCHING_SCENE_INDEX_TOKENS`
- `DOME_LIGHT_CAMERA_VISIBILITY_SCENE_INDEX_TOKENS`
- `LIGHT_LINKING_SCENE_INDEX_TOKENS`
- `PREFIX_PATH_PRUNING_SCENE_INDEX_TOKENS`
- `PRIM_MANAGING_SCENE_INDEX_OBSERVER_TOKENS`
- `RENDER_SETTINGS_FILTERING_SCENE_INDEX_TOKENS`
- `UNBOUND_MATERIAL_PRUNING_SCENE_INDEX_TOKENS`
- `VELOCITY_MOTION_RESOLVING_SCENE_INDEX_TOKENS`

## Utilities

### Scene Index Diff

```rust
use usd::imaging::hdsi::compute_scene_index_diff;

let diff = compute_scene_index_diff(old_scene, new_scene);
println!("Added: {:?}", diff.added);
println!("Removed: {:?}", diff.removed);
println!("Modified: {:?}", diff.modified);
```

### Path Utilities

```rust
use usd::imaging::hdsi::utils;

let is_prim = utils::is_prim_path(&path);
let under_prefix = utils::is_path_under_prefix(&path, &prefix);
```

## Testing

Comprehensive tests are provided in `tests.rs`:

```bash
cargo test --package usd-rs --lib imaging::hdsi
```

## Implementation Status

### Fully Implemented (26 modules)

All scene index filters are fully implemented with `FilteringSceneIndexObserver` registration for proper change notification propagation:

| Module | Description |
|--------|-------------|
| `HdsiCoordSysPrimSceneIndex` | Creates coordinate system prims for bindings |
| `HdsiDomeLightCameraVisibilitySceneIndex` | Controls dome light camera visibility |
| `HdsiExtComputationDependencySceneIndex` | Manages ext computation dependencies |
| `HdsiExtComputationPrimvarPruningSceneIndex` | Prunes computed primvars |
| `HdsiImplicitSurfaceSceneIndex` | Converts implicit surfaces to mesh |
| `HdsiLightLinkingSceneIndex` | Resolves light/shadow linking |
| `HdsiMaterialBindingResolvingSceneIndex` | Resolves material bindings |
| `HdsiMaterialOverrideResolvingSceneIndex` | Resolves material overrides |
| `HdsiMaterialPrimvarTransferSceneIndex` | Transfers primvars from materials |
| `HdsiMaterialRenderContextFilteringSceneIndex` | Filters materials by render context |
| `HdsiLegacyDisplayStyleOverrideSceneIndex` | Legacy display style overrides |
| `HdsiNurbsApproximatingSceneIndex` | Converts NURBS to mesh |
| `HdsiPinnedCurveExpandingSceneIndex` | Expands pinned curves |
| `HdsiPrefixPathPruningSceneIndex` | Prunes by path prefix |
| `HdsiPrimTypeAndPathPruningSceneIndex` | Prunes by type and path predicate |
| `HdsiPrimTypeNoticeBatchingSceneIndex` | Batches notices by prim type |
| `HdsiPrimTypePruningSceneIndex` | Prunes prims by type |
| `HdsiRenderPassPruneSceneIndex` | Prunes render pass prims |
| `HdsiRenderSettingsFilteringSceneIndex` | Filters render settings |
| `HdsiSceneGlobalsSceneIndex` | Manages scene-level globals |
| `HdsiSceneMaterialPruningSceneIndex` | Prunes unused materials |
| `HdsiSwitchingSceneIndex` | Switches between input scene indices |
| `HdsiTetMeshConversionSceneIndex` | Converts tet meshes |
| `HdsiUnboundMaterialPruningSceneIndex` | Prunes unbound materials |
| `HdsiVelocityMotionResolvingSceneIndex` | Resolves velocity motion blur |
| `HdsiDebuggingSceneIndex` | Checks for inconsistencies |

### Stub / Blocked (1 module)

- **HdsiNodeIdentifierResolvingSceneIndex** — Stub only. Full implementation requires:
  - `HdMaterialFilteringSceneIndexBase`
  - `HdMaterialNetworkInterface` / `HdDataSourceMaterialNetworkInterface`
  - SdrRegistry (from `usd::sdr` crate)

### Other Components

- ✅ `HdsiPrimManagingSceneIndexObserver` — Prim lifecycle observer
- ✅ `compute_scene_index_diff` — Diff computation between scene indices
- ✅ `utils` — compile_collection, is_pruned, remove_pruned_children, etc.
- ✅ `implicit_to_mesh` — Conversion helpers for implicit surfaces
- ✅ Token definitions complete

## Future Work

1. **nodeIdentifierResolvingSceneIndex** — Full implementation requires SdrRegistry and MaterialNetworkInterface infrastructure from other crates
2. **Integration Tests** — Add end-to-end tests with real scene data

## References

- C++ Reference: `pxr/imaging/hdsi/`
- Hydra Documentation: [OpenUSD Docs](https://openusd.org/release/api/hd_page_front.html)
- Scene Index Overview: [Scene Index Introduction](https://openusd.org/release/api/hd_scene_index.html)

## Related Modules

- [`hd`](../hd/README.md) - Hydra core (scene index base traits)
- [`hdx`](../hdx/README.md) - Hydra extensions (WIP)
- [`hd_st`](../hd_st/README.md) - Storm render delegate (WIP)
