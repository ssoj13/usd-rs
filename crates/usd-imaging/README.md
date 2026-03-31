# usd-imaging

USD-to-Hydra translation layer and application-facing rendering engine for `usd-rs`.

## What This Crate Does

- Builds the canonical UsdImaging scene-index chain from a USD stage.
- Translates USD prims into Hydra data sources via adapters.
- Provides `usd_imaging::gl::Engine`, the application-facing renderer used by `usd-view`.
- Bridges Hydra task-graph intent into real Storm/wgpu execution.

## Current Render Path

The modern render path is reference-aligned around `HdxTaskController` and `HdEngine`:

```text
UsdStage
 -> usd-imaging scene indices
 -> HdRenderIndex
 -> HdxTaskController
 -> HdEngine::execute()
 -> Storm geometry passes
 -> engine AOV bridge
 -> deferred post-FX replay
 -> final color target
```

The engine-side post-FX bridge now replays deferred HDX tasks after backend rendering:

- `HdxAovInputTask`
- `HdxColorizeSelectionTask`
- `HdxColorCorrectionTask` (`sRGB` and `OpenColorIO` engine-side replay paths are live)
- `HdxVisualizeAovTask`
- `HdxPresentTask`

## Important Notes

- Main render targets include `color`, `depth`, `primId`, `instanceId`, and `elementId`.
- `colorIntermediate` is backed by a dedicated ping-pong texture, not by aliasing the main color target.
- Selection highlighting follows the HDX post-process path: `HdxSelectionTask` populates the selection buffer contract in task context, and the engine-side compute compositor consumes it after backend draw.
- `visualizeAov` and `colorizeSelection` are no longer log-only placeholders; both now execute engine-side compute passes.
- `get_renderer_aovs()` follows the OpenUSD candidate-filtering rule instead of advertising an engine-local superset.

## Relationship To Other Crates

- `usd-hdx` describes the Hydra task graph and emits backend-facing post-task requests.
- `usd-hd-st` is the Storm render delegate that executes the actual geometry/AOV passes.
- `usd-view` consumes `usd_imaging::gl::Engine` for interactive viewport rendering.

## Reference

Always compare behavior against `_ref/OpenUSD/pxr/usdImaging/` and `_ref/OpenUSD/pxr/imaging/hdx/` when extending this crate.
