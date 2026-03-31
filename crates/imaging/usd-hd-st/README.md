# usd-hd-st

Storm render delegate for `usd-rs`, backed primarily by `wgpu` through HGI.

## What This Crate Does

- Implements the Storm render delegate (`HdStRenderDelegate`).
- Owns Storm prim implementations, resource registry, draw batching, and render-pass execution.
- Consumes render-pass state and AOV bindings produced by the Hydra task graph.

## Current Render-Pass Model

The active path follows the reference split between draw preparation and draw execution:

```text
HdStRenderPass::execute_with_hgi()
 -> PrepareDraw phase
 -> submit preparation work
 -> ExecuteDraw phase
 -> write requested AOV attachments
```

Important current behavior:

- Render-pass execution respects material-tag ordered passes coming from HDX render tasks.
- AOV bindings are provided by the application-facing engine, including Storm-special outputs such as `Neye`; default AOV descriptors follow the same `GetDefaultAovDescriptor()` semantics as the OpenUSD reference.
- The render delegate is used by `usd-imaging::gl::Engine`, not as a standalone application entry point.

## Relationship To Other Crates

- `usd-hd` defines the core Hydra delegate/render-pass interfaces.
- `usd-hdx` builds the task graph that drives Storm.
- `usd-imaging` owns the engine that translates HDX render-pass intent into concrete Storm targets and post-processing.

## Reference

Compare against `_ref/OpenUSD/pxr/imaging/hdSt/`, especially `renderPass.cpp`, when changing execution order or AOV behavior.
