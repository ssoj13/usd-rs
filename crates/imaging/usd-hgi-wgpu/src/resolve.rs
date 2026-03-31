//! Handle resolution utilities for extracting native wgpu types from HGI handles.
//!
//! HGI uses trait objects (Arc<dyn HgiBuffer>) inside HgiHandle<T>. Command buffer
//! implementations need access to the underlying wgpu resources (wgpu::Buffer,
//! wgpu::Texture, etc.) to call wgpu API methods.
//!
//! This module provides helper functions that:
//! 1. Extract the trait object from the handle using handle.get()
//! 2. Downcast to the concrete Wgpu* type using as_any()
//! 3. Access the inner wgpu resource via the accessor method

use usd_hgi::buffer::HgiBufferHandle;
use usd_hgi::compute_pipeline::HgiComputePipelineHandle;
use usd_hgi::graphics_pipeline::HgiGraphicsPipelineHandle;
use usd_hgi::resource_bindings::HgiResourceBindingsHandle;
use usd_hgi::sampler::HgiSamplerHandle;
use usd_hgi::shader_function::HgiShaderFunctionHandle;
use usd_hgi::shader_program::HgiShaderProgramHandle;
use usd_hgi::texture::HgiTextureHandle;

use crate::buffer::WgpuBuffer;
use crate::compute_pipeline::WgpuComputePipeline;
use crate::graphics_pipeline::WgpuGraphicsPipeline;
use crate::resource_bindings::WgpuResourceBindings;
use crate::sampler::WgpuSampler;
use crate::shader_function::WgpuShaderFunction;
use crate::shader_program::WgpuShaderProgram;
use crate::texture::WgpuTexture;

/// Extract the wgpu::Buffer from an HgiBufferHandle.
///
/// Returns None if the handle is null or not a WgpuBuffer.
///
/// # Example
/// ```ignore
/// if let Some(wgpu_buf) = resolve_buffer(&buffer_handle) {
///     render_pass.set_vertex_buffer(0, wgpu_buf.slice(..));
/// }
/// ```
pub fn resolve_buffer(handle: &HgiBufferHandle) -> Option<&wgpu::Buffer> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuBuffer>()
        .map(|b| b.wgpu_buffer())
}

/// Extract the wgpu::Texture from an HgiTextureHandle.
///
/// Returns None if the handle is null or not a WgpuTexture.
pub fn resolve_texture(handle: &HgiTextureHandle) -> Option<&wgpu::Texture> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuTexture>()
        .map(|t| t.wgpu_texture())
}

/// Extract the wgpu::TextureView from an HgiTextureHandle.
///
/// Returns None if the handle is null or not a WgpuTexture.
///
/// # Example
/// ```ignore
/// if let Some(view) = resolve_texture_view(&color_attachment) {
///     let attachment = wgpu::RenderPassColorAttachment {
///         view,
///         resolve_target: None,
///         ops: wgpu::Operations::default(),
///     };
/// }
/// ```
pub fn resolve_texture_view(handle: &HgiTextureHandle) -> Option<&wgpu::TextureView> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuTexture>()
        .map(|t| t.wgpu_view())
}

/// Extract the wgpu::RenderPipeline from an HgiGraphicsPipelineHandle.
///
/// Returns None if the handle is null or not a WgpuGraphicsPipeline.
///
/// # Example
/// ```ignore
/// if let Some(pipeline) = resolve_graphics_pipeline(&pipeline_handle) {
///     render_pass.set_pipeline(pipeline);
/// }
/// ```
pub fn resolve_graphics_pipeline(
    handle: &HgiGraphicsPipelineHandle,
) -> Option<&wgpu::RenderPipeline> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuGraphicsPipeline>()
        .and_then(|p| p.wgpu_pipeline())
}

/// Extract the wgpu::ComputePipeline from an HgiComputePipelineHandle.
///
/// Returns None if the handle is null or not a WgpuComputePipeline.
///
/// # Example
/// ```ignore
/// if let Some(pipeline) = resolve_compute_pipeline(&pipeline_handle) {
///     compute_pass.set_pipeline(pipeline);
/// }
/// ```
pub fn resolve_compute_pipeline(
    handle: &HgiComputePipelineHandle,
) -> Option<&wgpu::ComputePipeline> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuComputePipeline>()
        .and_then(|p| p.wgpu_pipeline())
}

/// Extract the wgpu::BindGroup from an HgiResourceBindingsHandle.
///
/// Returns None if the handle is null or not a WgpuResourceBindings.
///
/// # Example
/// ```ignore
/// if let Some(bind_group) = resolve_bind_group(&bindings_handle) {
///     render_pass.set_bind_group(0, bind_group, &[]);
/// }
/// ```
pub fn resolve_bind_group(handle: &HgiResourceBindingsHandle) -> Option<&wgpu::BindGroup> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuResourceBindings>()
        .map(|b| b.wgpu_bind_group())
}

/// Extract the wgpu::BindGroupLayout from an HgiResourceBindingsHandle.
///
/// Returns None if the handle is null or not a WgpuResourceBindings.
pub fn resolve_bind_group_layout(
    handle: &HgiResourceBindingsHandle,
) -> Option<&wgpu::BindGroupLayout> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuResourceBindings>()
        .map(|b| b.wgpu_layout())
}

/// Extract the wgpu::Sampler from an HgiSamplerHandle.
///
/// Returns None if the handle is null or not a WgpuSampler.
pub fn resolve_sampler(handle: &HgiSamplerHandle) -> Option<&wgpu::Sampler> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuSampler>()
        .map(|s| s.wgpu_sampler())
}

/// Extract the wgpu::ShaderModule from an HgiShaderFunctionHandle.
///
/// Returns None if the handle is null or not a WgpuShaderFunction.
pub fn resolve_shader_module(handle: &HgiShaderFunctionHandle) -> Option<&wgpu::ShaderModule> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuShaderFunction>()
        .map(|s| s.wgpu_module())
}

/// Extract the concrete WgpuBuffer from an HgiBufferHandle.
///
/// Useful when you need to access WgpuBuffer-specific methods beyond
/// the trait interface.
///
/// Returns None if the handle is null or not a WgpuBuffer.
pub fn resolve_wgpu_buffer(handle: &HgiBufferHandle) -> Option<&WgpuBuffer> {
    handle.get()?.as_any().downcast_ref::<WgpuBuffer>()
}

/// Extract the concrete WgpuTexture from an HgiTextureHandle.
///
/// Returns None if the handle is null or not a WgpuTexture.
pub fn resolve_wgpu_texture(handle: &HgiTextureHandle) -> Option<&WgpuTexture> {
    handle.get()?.as_any().downcast_ref::<WgpuTexture>()
}

/// Extract the concrete WgpuGraphicsPipeline from an HgiGraphicsPipelineHandle.
///
/// Returns None if the handle is null or not a WgpuGraphicsPipeline.
pub fn resolve_wgpu_graphics_pipeline(
    handle: &HgiGraphicsPipelineHandle,
) -> Option<&WgpuGraphicsPipeline> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuGraphicsPipeline>()
}

/// Extract the concrete WgpuComputePipeline from an HgiComputePipelineHandle.
///
/// Returns None if the handle is null or not a WgpuComputePipeline.
pub fn resolve_wgpu_compute_pipeline(
    handle: &HgiComputePipelineHandle,
) -> Option<&WgpuComputePipeline> {
    handle.get()?.as_any().downcast_ref::<WgpuComputePipeline>()
}

/// Extract the concrete WgpuResourceBindings from an HgiResourceBindingsHandle.
///
/// Returns None if the handle is null or not a WgpuResourceBindings.
pub fn resolve_wgpu_resource_bindings(
    handle: &HgiResourceBindingsHandle,
) -> Option<&WgpuResourceBindings> {
    handle
        .get()?
        .as_any()
        .downcast_ref::<WgpuResourceBindings>()
}

/// Extract the concrete WgpuSampler from an HgiSamplerHandle.
///
/// Returns None if the handle is null or not a WgpuSampler.
pub fn resolve_wgpu_sampler(handle: &HgiSamplerHandle) -> Option<&WgpuSampler> {
    handle.get()?.as_any().downcast_ref::<WgpuSampler>()
}

/// Extract the concrete WgpuShaderFunction from an HgiShaderFunctionHandle.
///
/// Returns None if the handle is null or not a WgpuShaderFunction.
pub fn resolve_wgpu_shader_function(
    handle: &HgiShaderFunctionHandle,
) -> Option<&WgpuShaderFunction> {
    handle.get()?.as_any().downcast_ref::<WgpuShaderFunction>()
}

/// Extract the concrete WgpuShaderProgram from an HgiShaderProgramHandle.
///
/// Returns None if the handle is null or not a WgpuShaderProgram.
pub fn resolve_wgpu_shader_program(handle: &HgiShaderProgramHandle) -> Option<&WgpuShaderProgram> {
    handle.get()?.as_any().downcast_ref::<WgpuShaderProgram>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hgi::HgiHandle;

    #[test]
    fn test_resolve_null_handle() {
        let null_handle: HgiBufferHandle = HgiHandle::null();
        assert!(resolve_buffer(&null_handle).is_none());
        assert!(resolve_wgpu_buffer(&null_handle).is_none());
    }

    // Note: Full integration tests require wgpu::Device, tested in integration tests
}
