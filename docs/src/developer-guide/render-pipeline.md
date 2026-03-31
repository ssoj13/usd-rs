# Render Pipeline (Storm / wgpu)

This chapter describes how geometry flows from Hydra through Storm to the GPU
via the wgpu backend.

## Full Render Frame

```mermaid
sequenceDiagram
    participant App as Application / Viewer
    participant Engine as Engine (usd-imaging)
    participant HDX as TaskController (usd-hdx)
    participant HD as HdEngine (usd-hd)
    participant Storm as Storm RenderDelegate
    participant HGI as HGI (usd-hgi)
    participant WGPU as wgpu Backend

    App->>Engine: render(params)
    Engine->>HDX: Build task list
    HDX-->>HD: [RenderTask, AOVInput, Selection, ColorCorrect, Present]

    HD->>Storm: Sync dirty rprims
    Storm->>Storm: Update vertex/index buffers
    Storm->>Storm: Batch compatible draw items
    Storm->>Storm: Generate/compile shaders

    HD->>HD: Execute task list
    loop Each render task
        HD->>Storm: Execute(renderPass)
        Storm->>HGI: Begin graphics commands
        Storm->>HGI: Bind pipeline + resources
        Storm->>HGI: Draw indexed (batched)
        Storm->>HGI: End commands
        HGI->>WGPU: Submit to GPU
    end

    HD->>HDX: Post-processing tasks
    HDX->>Engine: AOV results
    Engine->>App: Frame complete
```

## Storm Draw Pipeline

### Rprim Sync

When the render index has dirty rprims, Storm syncs them to GPU:

```mermaid
graph LR
    Dirty["Dirty Rprim<br/>(mesh, curves, points)"]
    Topo["Topology<br/>Computation"]
    Subdiv["Subdivision<br/>(OpenSubdiv)"]
    Normals["Normal<br/>Computation"]
    Upload["GPU Buffer<br/>Upload"]
    DrawItem["Draw Item<br/>Creation"]

    Dirty --> Topo
    Topo --> Subdiv
    Topo --> Normals
    Subdiv --> Upload
    Normals --> Upload
    Upload --> DrawItem
```

For each mesh:
1. **Topology** -- face vertex counts, indices, subdivision scheme
2. **Subdivision** -- if scheme is `catmullClark`, refine via OpenSubdiv
3. **Normals** -- compute smooth or flat normals if not authored
4. **Upload** -- transfer vertex/index data to GPU buffers via HGI
5. **Draw Item** -- create a draw item with material binding, transform, visibility

### Draw Batching

Storm groups draw items by compatible state to minimize GPU state changes:

| Batch key | Components |
|-----------|-----------|
| Shader program | Material network hash |
| Geometry | Buffer array range |
| Pipeline state | Blend mode, depth test, cull face |
| Render pass | AOV target set |

Batched items are drawn with a single `draw_indexed` call using instanced
rendering or multi-draw-indirect where supported.

### Shader Generation

Storm generates shaders dynamically based on:
- Material network (UsdPreviewSurface parameters)
- Geometry type (mesh, curves, points)
- Lighting configuration
- AOV requirements (color, depth, ID, normals)
- Selection highlighting

## HGI Abstraction Layer

HGI provides a portable GPU API. Key abstractions:

```mermaid
graph TB
    subgraph "HGI Resources"
        Buffer["HgiBuffer<br/>Vertex, Index, Uniform"]
        Texture["HgiTexture<br/>2D, 3D, Cube, Array"]
        Sampler["HgiSampler<br/>Filter, Wrap, Compare"]
        ShaderProg["HgiShaderProgram<br/>Vertex + Fragment"]
        Pipeline["HgiGraphicsPipeline<br/>Raster state + shaders"]
    end

    subgraph "HGI Commands"
        Blit["HgiBlitCmds<br/>Copy, Generate Mipmaps"]
        Graphics["HgiGraphicsCmds<br/>Draw, Bind, Set Viewport"]
        Compute["HgiComputeCmds<br/>Dispatch"]
    end

    subgraph "wgpu Implementation"
        WDevice["wgpu::Device"]
        WQueue["wgpu::Queue"]
        WEncoder["wgpu::CommandEncoder"]
    end

    Buffer --> WDevice
    Texture --> WDevice
    Pipeline --> WDevice
    Graphics --> WEncoder
    Compute --> WEncoder
    Blit --> WQueue
```

### wgpu Backend (`usd-hgi-wgpu`)

The wgpu backend maps HGI operations to the wgpu API:

| HGI Operation | wgpu Equivalent |
|--------------|-----------------|
| `CreateBuffer` | `device.create_buffer()` |
| `CreateTexture` | `device.create_texture()` |
| `CreateGraphicsPipeline` | `device.create_render_pipeline()` |
| `GraphicsCmds::Draw` | `render_pass.draw_indexed()` |
| `BlitCmds::CopyBufferToBuffer` | `encoder.copy_buffer_to_buffer()` |
| `SubmitCmds` | `queue.submit()` |

wgpu automatically selects the best available GPU backend:
- **Windows**: Vulkan or DX12
- **macOS**: Metal
- **Linux**: Vulkan
- **Web**: WebGPU

## AOV (Arbitrary Output Variable) Pipeline

Storm renders to multiple output targets simultaneously:

| AOV | Content | Usage |
|-----|---------|-------|
| `color` | Final shaded color | Display |
| `depth` | Z-buffer depth | Post-effects, compositing |
| `primId` | Prim identifier | GPU picking |
| `instanceId` | Instance identifier | Instance picking |
| `elementId` | Face/edge identifier | Sub-element picking |
| `normal` | World-space normals | Visualization |

The task controller (`usd-hdx`) manages AOV resolution and post-processing:

```mermaid
graph LR
    Render["Render Pass<br/>(geometry)"]
    AOV["AOV Resolve"]
    Select["Selection<br/>Colorize"]
    CC["Color<br/>Correction"]
    Present["Present<br/>(to screen)"]

    Render --> AOV --> Select --> CC --> Present
```

## Viewer Integration (`usd-view`)

The viewer uses egui for the UI and wgpu for rendering:

```mermaid
graph TB
    subgraph "usd-view"
        UI["egui UI<br/>(panels, menus, toolbar)"]
        Viewport["Viewport Panel"]
        DataModel["DataModel<br/>(stage, selection, settings)"]
    end

    subgraph "Rendering"
        Engine["Engine<br/>(usd-imaging)"]
        Storm["Storm"]
        WGPU["wgpu surface"]
    end

    UI --> DataModel
    Viewport --> Engine
    DataModel --> Engine
    Engine --> Storm
    Storm --> WGPU
    WGPU --> Viewport
```

The viewer provides:
- Interactive orbit/pan/zoom camera
- Prim hierarchy browser with filtering
- Attribute inspector with time-sample editing
- Timeline with playback controls
- Renderer settings (complexity, AOV display, material mode)
- GPU picking for selection
- HUD overlay with performance statistics
