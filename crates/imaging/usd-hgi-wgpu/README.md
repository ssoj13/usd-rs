# usd-hgi-wgpu

wgpu backend for the USD Hydra Graphics Interface (HGI).

Replaces the separate C++ backends (HgiGL, HgiVulkan, HgiMetal) with a single
cross-platform Rust implementation built on [wgpu](https://wgpu.rs/).
Covers **Vulkan, Metal, DX12, and OpenGL** through one safe API.

## C++ Reference

This crate is a port of two C++ OpenUSD modules:

| C++ module | Path | Role |
|---|---|---|
| **hgi** | `pxr/imaging/hgi/` | Abstract HGI interface, types, enums, descriptors |
| **hgiVulkan** | `pxr/imaging/hgiVulkan/` | Vulkan-specific backend implementation |

In Rust the abstract interface lives in `usd-hgi` (trait `Hgi`), and this crate
(`usd-hgi-wgpu`) provides the concrete backend via `HgiWgpu`.

## Architecture

```
usd-hgi (trait Hgi, descriptors, enums)
    |
    +-- usd-hgi-wgpu (HgiWgpu: wgpu backend)
            |
            +-- wgpu 27 (Vulkan/Metal/DX12/GL)
```

### Module map (C++ -> Rust)

| C++ file (hgi/) | C++ file (hgiVulkan/) | Rust file |
|---|---|---|
| hgi.h/cpp | hgi.h/cpp | `hgi.rs` |
| blitCmds.h/cpp | blitCmds.h/cpp | `blit_cmds.rs` |
| buffer.h/cpp | buffer.h/cpp | `buffer.rs` |
| capabilities.h/cpp | capabilities.h/cpp | `capabilities.rs` |
| computeCmds.h/cpp | computeCmds.h/cpp | `compute_cmds.rs` |
| computePipeline.h/cpp | computePipeline.h/cpp | `compute_pipeline.rs` |
| -- | conversions.h/cpp | `conversions.rs` |
| graphicsCmds.h/cpp | graphicsCmds.h/cpp | `graphics_cmds.rs` |
| graphicsPipeline.h/cpp | graphicsPipeline.h/cpp | `graphics_pipeline.rs` |
| resourceBindings.h/cpp | resourceBindings.h/cpp | `resource_bindings.rs` |
| sampler.h/cpp | sampler.h/cpp | `sampler.rs` |
| shaderFunction.h/cpp | shaderFunction.h/cpp | `shader_function.rs` |
| shaderProgram.h/cpp | shaderProgram.h/cpp | `shader_program.rs` |
| texture.h/cpp | texture.h/cpp | `texture.rs` |
| -- | -- | `surface.rs` (wgpu window presentation) |
| -- | -- | `mipmap.rs` (GPU mipmap generator) |
| -- | -- | `gpu_timer.rs` (timestamp queries) |
| -- | -- | `resolve.rs` (handle downcasting) |

### What is NOT ported (Vulkan-specific internals)

These C++ constructs have no wgpu equivalent and are handled internally by wgpu:

- `HgiVulkanInstance` / `HgiVulkanDevice` -- wgpu Instance/Adapter/Device
- `HgiVulkanCommandBuffer` / `HgiVulkanCommandQueue` -- wgpu CommandEncoder/Queue
- `HgiVulkanGarbageCollector` -- replaced by `deferred_destroy_*` + `process_deferred_deletions()`
- `HgiVulkanShaderCompiler` (glslang) -- wgpu uses naga for WGSL/SPIR-V
- `HgiVulkanPipelineCache` -- wgpu manages pipeline caching internally
- `HgiVulkanDescriptorSetLayouts` -- wgpu uses BindGroupLayouts
- `VkMemoryAllocator` -- wgpu manages GPU memory allocation

## Public types

| Type | Description |
|---|---|
| `HgiWgpu` | Main struct, implements `Hgi` trait |
| `create_hgi_wgpu()` | Factory function, returns `Option<HgiWgpu>` |
| `WgpuBuffer` | Implements `HgiBuffer` |
| `WgpuTexture` | Implements `HgiTexture` |
| `WgpuSampler` | Implements `HgiSampler` |
| `WgpuShaderFunction` | Implements `HgiShaderFunction` |
| `WgpuShaderProgram` | Implements `HgiShaderProgram` |
| `WgpuGraphicsPipeline` | Implements `HgiGraphicsPipeline` |
| `WgpuComputePipeline` | Implements `HgiComputePipeline` |
| `WgpuResourceBindings` | Implements `HgiResourceBindings` |
| `WgpuBlitCmds` | Implements `HgiBlitCmds` (copy/blit operations) |
| `WgpuGraphicsCmds` | Implements `HgiGraphicsCmds` (render pass) |
| `WgpuComputeCmds` | Implements `HgiComputeCmds` (compute dispatch) |
| `WgpuCapabilities` | Device capability queries |
| `MipmapGenerator` | GPU mipmap generation pipeline |
| `GpuTimer` | Timestamp-based GPU profiling |

## Tests

Tests are ported from two C++ test files:

- `hgi/testenv/testHgiCommand.cpp` (basic init, pipeline, draw)
- `hgiVulkan/testenv/testHgiVulkan.cpp` (1713 lines, 15 test functions)

### Test list (22 integration tests)

| # | Test | Ported from |
|---|---|---|
| 1 | `test_wgpu_instance` | TestVulkanInstance |
| 2 | `test_wgpu_device` | TestVulkanDevice |
| 3 | `test_wgpu_capabilities` | (capabilities check) |
| 4 | `test_wgpu_unique_id` | (ID counter) |
| 5 | `test_wgpu_buffer_create` | TestVulkanBuffer (create + size check) |
| 6 | `test_wgpu_buffer_vertex_index` | HgiGfxCmdBfrExecutionTestDriver (VBO + IBO) |
| 7 | `test_wgpu_texture_create` | TestVulkanTexture (create + view) |
| 8 | `test_wgpu_shader_creation` | TestVulkanPipeline (shader part) |
| 9 | `test_wgpu_graphics_pipeline` | TestVulkanPipeline |
| 10 | `test_wgpu_compute_pipeline` | TestVulkanComputeCmds (pipeline part) |
| 11 | `test_wgpu_buffer_readback` | TestVulkanBuffer (GPU -> CPU readback) |
| 12 | `test_wgpu_buffer_cpu_to_gpu_transfer` | TestVulkanBuffer (CPU -> GPU -> readback) |
| 13 | `test_wgpu_texture_readback` | TestVulkanTexture (GPU -> CPU readback) |
| 14 | `test_wgpu_garbage_collection` | TestVulkanGarbageCollection |
| 15 | `test_wgpu_frame_lifecycle` | StartFrame/EndFrame nesting |
| 16 | `test_wgpu_sampler_create` | (sampler creation) |
| 17 | `test_wgpu_resource_bindings` | TestVulkanComputeCmds (UBO + SSBO bindings) |
| 18 | `test_wgpu_texture_to_buffer_copy` | TestHgiTextureToBufferCopy |
| 19 | `test_wgpu_buffer_to_texture_copy` | TestHgiBufferToTextureCopy |
| 20 | `test_wgpu_mipmap_generation` | TestVulkanTexture (mipmap path) |
| 21 | `test_wgpu_wait_for_idle` | device->WaitForIdle() |
| 22 | `test_wgpu_device_identity` | (device identity check) |
| + | `test_wgpu_backend_support` | (static support check) |
| + | `test_wgpu_multi_frame_resources` | (multi-frame stress test) |
| + | `test_wgpu_fill_buffer` | (fill buffer blit) |

Plus 2 unit tests in `hgi.rs`: `test_create_hgi_wgpu`, `test_unique_id`.

### C++ tests NOT ported (Vulkan-specific internals)

| C++ test | Reason |
|---|---|
| TestVulkanShaderCompiler | Tests glslang GLSL->SPIRV; wgpu uses naga internally |
| TestVulkanCommandQueue | Tests Vulkan command pool threading + inflight bits; wgpu abstracts this |
| TestGraphicsCmdsClear | Tests Vulkan render pass clear with image write to disk (needs HioImage) |
| TestCreateSrgbaTexture | Tests sRGB texture + image write to disk (needs HioImage) |
| TestHgiGetMipInitialData | Tests HgiGetMipInfos utility (lives in usd-hgi, not backend) |

### Running tests

```bash
# All tests (requires GPU)
cargo test -p usd-hgi-wgpu

# Single test
cargo test -p usd-hgi-wgpu test_wgpu_buffer_readback

# With logging
RUST_LOG=debug cargo test -p usd-hgi-wgpu -- --nocapture
```

Tests skip gracefully in headless CI (no GPU) via `let Some(hgi) = try_create_hgi() else { return }`.

## Baseline images

Copied from `_ref/OpenUSD/pxr/imaging/hgiVulkan/testenv/`:

```
testenv/baseline/
  copyBufferToTexture.png
  copyTextureToBuffer.png
  graphicsCmdsClear.png
  srgba.png
  testHgiVulkanCommand_triangle.png
```

These are reference images from the C++ Vulkan tests for visual comparison.

## Key differences from C++ API

| C++ | Rust |
|---|---|
| `HgiVulkan` / `HgiGL` / `HgiMetal` | `HgiWgpu` (one backend for all) |
| `Hgi::SubmitCmds(cmds.get())` (borrows) | `hgi.submit_cmds(cmds)` (takes ownership) |
| `initialData` in descriptor | `initial_data: Option<&[u8]>` separate param |
| `pixelsByteSize` in HgiTextureDesc | Not in desc; data size inferred from slice |
| `HgiHandle<T>` (ref-counted ptr) | `Arc<dyn Trait>` behind typed handle |
| GLSL shader code | WGSL shader code (naga handles compilation) |
| `VkShaderModule` / `VkPipeline` | `wgpu::ShaderModule` / `wgpu::RenderPipeline` |
| `GetCPUStagingAddress()` for buffers | `None` (wgpu uses `map_async`) |
| Thread-local command pools | wgpu Device/Queue are Send+Sync |
