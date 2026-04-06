# Diagrams

## usd-view Stage Load

```mermaid
flowchart TD
    A[UI action / CLI initial file] --> B[ViewerApp.load_file]
    B --> C[Spawn background thread]
    C --> D[RootDataModel.open_stage_detached]
    D --> E[RootDataModel.collect_stage_time_samples]
    E --> F[EventBus.emit StageLoaded]
    B --> G[ViewerApp.update]
    G --> H[handle_bus_events]
    H --> I[on_stage_loaded]
    I --> J[apply_loaded_stage]
    I --> K[invalidate_scene SceneSwitch]
    I --> L[apply_post_load_config]
    L --> M[first viewport render]
```

## usd-view Playback Hot Path

```mermaid
flowchart TD
    A[eframe update] --> B[PlaybackState.advance]
    B --> C[RootDataModel.current_time]
    C --> D[DataModel.update_cache_time]
    D --> E[ui_viewport]
    E --> F[Engine.set_time]
    F --> G[Engine.render]
    G --> H[GPU readback]
    H --> I[CPU color_correct]
    I --> J[egui texture upload]
    J --> K[UI presents frame]
```

## Camera List Drift vs Reference

```mermaid
flowchart LR
    A[Stage change] --> B[_ref: clear cached _allSceneCameras]
    B --> C[_ref: rebuild cameras only on demand]

    A --> D[usd-view: ObjectsChanged stored in change_state]
    D --> E[no drain_changes caller]
    E --> F[toolbar sync traverses stage every frame]
```

## usdSkel Resolving Path

```mermaid
flowchart TD
    A[StageSceneIndex skeleton prim] --> B[DataSourceSkeletonPrim]
    B --> C[SkeletonSchema via DataSourceMapped]
    C --> D[SkeletonResolvingSceneIndex]
    D --> E[DataSourceResolvedSkeletonPrim]
    E --> F[ResolvedSkeletonSchema sampled fields]
    E --> G[Guide mesh topology]
    E --> H[Guide points primvars]
    F --> I[PointsResolvingSceneIndex]
    G --> I
    H --> I
```

## DataSourceMapped Tree

```mermaid
flowchart TD
    A[USD prim] --> B[PropertyMappings]
    B --> C[absolute invalidation locators]
    B --> D[nested container tree]
    D --> E[attribute leaf factory]
    D --> F[relationship leaf factory]
    E --> G[DataSourceAttribute]
    F --> H[retained Path or Path array datasource]
```

## Legacy Skel Adapter Parity

```mermaid
flowchart TD
    A[UsdSkel prim adapter call] --> B{prim type}
    B -->|Skeleton| C[DataSourceSkeletonPrim]
    B -->|BlendShape| D[DataSourceBlendShapePrim]
    B -->|SkelRoot| E[DataSourcePrim]
    E --> F[overlay skelBinding.hasSkelRoot = true]
    C --> G[Hydra container]
    D --> G
    F --> G
```

## Adapter Resolution Fallback

```mermaid
flowchart TD
    A[AdapterRegistry.find_for_prim] --> B{direct type hit}
    B -->|yes| C[return registered adapter]
    B -->|no| D[scan registered adapter types]
    D --> E[filter prim.is_a(type)]
    E --> F[pick deepest schema match]
    F --> G{found}
    G -->|yes| H[return inherited adapter]
    G -->|no| I[NoOpAdapter]
```

## Flo Animation Rust Path

```mermaid
flowchart LR
    engineSetTime[Engine.set_time] --> stageSetTime[StageSceneIndex.set_time]
    stageSetTime --> stageGlobals[StageGlobalsImpl.set_time]
    stageGlobals --> dirtiedEntries[DirtiedPrimEntry x tracked path]
    dirtiedEntries --> adapterDirty[HdSceneIndexAdapterSceneDelegate.prims_dirtied]
    dirtiedEntries --> flatteningDirty[HdFlatteningSceneIndex.on_prims_dirtied]
    flatteningDirty --> extraDirty[descendant dirty fanout]
    adapterDirty --> renderIndexDirty[HdRenderIndex.mark_rprim_dirty]
    extraDirty --> renderIndexDirty
    renderIndexDirty --> renderBatch[Engine.render_batch]
    renderBatch --> syncAll[HdEngine.execute and HdRenderIndex.sync_all]
    syncAll --> viewerRefresh[Engine.sync_render_index_state]
    viewerRefresh --> manualDrawGather[Engine calls get_draw_items and downcasts]
    manualDrawGather --> renderPassSet[HdStRenderPass.set_draw_items]
    renderPassSet --> hgiExecute[HdStRenderPass.execute_with_hgi]
```

## Flo Animation Reference Path

```mermaid
flowchart LR
    prepareBatch[UsdImagingGLEngine.PrepareBatch] --> preSetTime[_PreSetTime]
    preSetTime --> refSetTime[StageSceneIndex.SetTime]
    refSetTime --> postSetTime[_PostSetTime]
    postSetTime --> hdExecute[HdEngine.Execute]
    hdExecute --> syncAllRef[HdRenderIndex.SyncAll]
    syncAllRef --> renderTask[HdxRenderTask.Execute]
    renderTask --> renderPassExec[HdSt_RenderPass._Execute]
    renderPassExec --> updateCmd[_UpdateCommandBuffer]
    updateCmd --> updateDraw[_UpdateDrawItems]
    updateDraw -->|state changed| drawItemsFetch[RenderIndex.GetDrawItems]
    updateDraw -->|state unchanged| drawItemsReuse[reuse cached draw-item vector]
```

## Python Bindings (usd-pyo3) Layer

```mermaid
flowchart TD
    A["Python: import pxr_rs"] --> B["pxr_rs._usd (native extension)"]
    B --> C["register_sub per module"]
    C --> D["pxr.Tf → usd-tf"]
    C --> E["pxr.Gf → usd-gf"]
    C --> F["pxr.Vt → usd-vt"]
    C --> G["pxr.Sdf → usd-sdf"]
    C --> H["pxr.Pcp → usd-pcp"]
    C --> I["pxr.Ar → usd-ar"]
    C --> J["pxr.Usd → usd-core"]
    C --> K["pxr.UsdGeom → usd-geom"]
    C --> L["pxr.UsdShade → usd-shade"]
    C --> M["pxr.UsdLux → usd-lux"]
    C --> N["pxr.UsdSkel → usd-skel"]
    C --> O["pxr.Kind → usd-kind"]
    C --> P["pxr.Cli → CLI tools"]

    subgraph build ["Build: maturin + pyo3"]
        Q["pyproject.toml"] --> R["maturin build"]
        R --> S["pxr_rs wheel (CPython ≥3.9)"]
    end
```

## Flo Divergence Map

```mermaid
flowchart TD
    xformTick[Xform-only time tick] --> flatteningFanout[Flattening descendant dirties]
    flatteningFanout --> expectedRef[Expected in reference]
    xformTick --> rustMeshSync[mesh_sync_dirty]
    rustMeshSync --> forcedDrawDirty[render_batch promotes draw_items_dirty]
    forcedDrawDirty --> fullViewerPass[sync_render_index_state full rprim walk]
    forcedDrawDirty --> manualDrawItems[manual get_draw_items and downcast]
    fullViewerPass --> driftRef[Reference drift]
    manualDrawItems --> driftRef
    drawCache[HdStDrawItemsCache and render_pass version checks exist] --> bypassed[bypassed by Engine path]
    bypassed --> driftRef
```
