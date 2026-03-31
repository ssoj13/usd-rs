# usd-hdx

Hydra task-graph utilities and high-level rendering tasks for `usd-rs`.

## What This Crate Does

- Implements `HdxTaskController` and the HDX task family.
- Owns task-graph policy such as render outputs, selection, presentation, and AOV visualization.
- Defines the backend request contracts consumed by `usd-imaging::gl::Engine`.

## Execution Model

This crate follows a split execution model:

```text
HdxTaskController
 -> HdEngine::execute()
 -> state / geometry tasks run immediately
 -> post tasks emit deferred requests into HdTaskContext
 -> usd-imaging::gl::Engine replays them after backend draw
```

The deferred post-task contract currently covers:

- `HdxAovInputTask`
- `HdxColorCorrectionTask`
- `HdxColorizeSelectionTask`
- `HdxVisualizeAovTask`
- `HdxPresentTask`

Ordering is preserved through `postTaskOrder`, so the engine can replay post-processing in the same sequence requested by the task graph.

## Current Notes

- `set_render_outputs()` now follows the split reference behavior: Storm uses the `_ResolvedRenderOutputs(...)` ordering/augmentation (`color`, `primId+instanceId`, optional `Neye`, `depth`), while the non-Storm path keeps the broader color-implies-picking expansion.
- `set_viewport_render_output()` wires those outputs into AOV input, selection colorize, pick-from-render-buffer, color correction, and visualize-AOV tasks.
- The selection tracker is shared across tasks, and `HdxSelectionTask` is the source of the task-context `selectionBuffer` / `selectionOffsets` contract consumed later by deferred colorize selection replay.

## Relationship To Other Crates

- `usd-imaging` consumes the deferred request structs and executes the real backend work.
- `usd-hd` provides the core Hydra task interfaces and render index.
- `usd-hd-st` provides the Storm render delegate used by the task graph.

## Reference

Compare against `_ref/OpenUSD/pxr/imaging/hdx/` and `_ref/OpenUSD/pxr/imaging/hdx/taskController.cpp` when changing task semantics.
