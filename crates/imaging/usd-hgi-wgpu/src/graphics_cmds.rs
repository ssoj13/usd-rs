//! wgpu graphics command buffer with deferred command model
//!
//! ARCHITECTURE: wgpu requires render pass attachments upfront when creating
//! a render pass. But HGI API records commands first and submits later.
//! Solution: defer commands as enum variants, then replay into wgpu::RenderPass
//! at submit time.

use std::sync::Arc;
use usd_gf::Vec4f;
use usd_hgi::buffer::{HgiBuffer, HgiBufferHandle};
use usd_hgi::cmds::HgiCmds;
use usd_hgi::enums::{HgiBufferUsage, HgiMemoryBarrier, HgiShaderStage};
use usd_hgi::graphics_cmds::*;
use usd_hgi::graphics_cmds_desc::HgiGraphicsCmdsDesc;
use usd_hgi::graphics_pipeline::HgiGraphicsPipelineHandle;
use usd_hgi::resource_bindings::HgiResourceBindingsHandle;
use usd_hgi::sampler::HgiSamplerHandle;
use usd_hgi::texture::HgiTextureHandle;

/// Deferred graphics command enum.
///
/// Each variant represents a recorded rendering operation.
/// Commands are stored in a vec and replayed into wgpu::RenderPass at submit.
#[derive(Clone)]
#[allow(dead_code)]
enum GraphicsCommand {
    /// Bind graphics pipeline state
    BindPipeline(HgiGraphicsPipelineHandle),
    /// Bind resource bindings (buffers, textures, samplers)
    BindResources(HgiResourceBindingsHandle),
    /// Bind vertex buffers with offsets
    BindVertexBuffers {
        buffers: Vec<HgiBufferHandle>,
        offsets: Vec<u64>,
    },
    /// Set viewport region
    SetViewport(HgiViewport),
    /// Set scissor rectangle
    SetScissor(HgiScissor),
    /// Set blend constant color
    SetBlendConstant(Vec4f),
    /// Set stencil reference value
    SetStencilRef(u32),
    /// Draw non-indexed primitives
    Draw(HgiDrawOp),
    /// Draw indexed primitives
    DrawIndexed {
        index_buffer: HgiBufferHandle,
        op: HgiDrawIndexedOp,
    },
    /// Draw using indirect buffer
    DrawIndirect(HgiDrawIndirectOp),
    /// Draw indexed using indirect buffer
    DrawIndexedIndirect {
        index_buffer: HgiBufferHandle,
        op: HgiDrawIndirectOp,
    },
    /// Set uniform buffer data for a bind index.
    /// A wgpu buffer+bind_group is created per command at submit time.
    SetUniform { bind_index: u32, data: Vec<u8> },
    /// Bind texture+sampler pairs as a bind group at the given group index.
    ///
    /// At submit time, resolves handles to wgpu TextureView + Sampler and
    /// creates a BindGroup using the pipeline's layout for `group_index`.
    /// Entries with null handles are filled with a 1x1 white fallback.
    BindTextureGroup {
        group_index: u32,
        textures: Vec<HgiTextureHandle>,
        samplers: Vec<HgiSamplerHandle>,
    },
    /// Bind a storage buffer (SSBO) at a given group + binding.
    /// At submit time, resolves to wgpu::Buffer and creates a storage bind group.
    BindStorageBuffer {
        group_index: u32,
        binding: u32,
        buffer: HgiBufferHandle,
    },
    /// Push debug marker group
    PushDebugGroup(String),
    /// Pop debug marker group
    PopDebugGroup,
    /// Insert single debug marker
    InsertDebugMarker(String),
}

/// Lazily-created 1x1 white fallback texture for unbound material slots.
struct FallbackTexture {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

/// wgpu graphics command buffer with deferred execution.
///
/// Records rendering commands as enum variants, then creates a render pass
/// and replays all commands at submit time.
#[allow(dead_code)]
pub struct WgpuGraphicsCmds {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    encoder: Option<wgpu::CommandEncoder>,
    commands: Vec<GraphicsCommand>,
    descriptor: HgiGraphicsCmdsDesc,
    submitted: bool,
    /// Cached 1x1 white fallback texture (created once per cmd buffer)
    fallback: Option<FallbackTexture>,
}

impl WgpuGraphicsCmds {
    /// Create a new graphics command buffer with render pass descriptor.
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        descriptor: HgiGraphicsCmdsDesc,
    ) -> Self {
        let encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("HgiWgpu GraphicsCmds"),
        });
        Self {
            device,
            queue,
            encoder: Some(encoder),
            commands: Vec::new(),
            descriptor,
            submitted: false,
            fallback: None,
        }
    }

    /// Determine index buffer format from buffer descriptor usage flags.
    fn get_index_format(index_buffer: &HgiBufferHandle) -> wgpu::IndexFormat {
        use crate::resolve::resolve_wgpu_buffer;

        if let Some(buffer) = resolve_wgpu_buffer(index_buffer) {
            let usage = buffer.descriptor().usage;
            if usage.contains(HgiBufferUsage::INDEX16) {
                wgpu::IndexFormat::Uint16
            } else if usage.contains(HgiBufferUsage::INDEX32) {
                wgpu::IndexFormat::Uint32
            } else {
                log::warn!(
                    "Index buffer has neither INDEX16 nor INDEX32 usage flag, defaulting to Uint32"
                );
                wgpu::IndexFormat::Uint32
            }
        } else {
            log::warn!("Failed to resolve index buffer, defaulting to Uint32 format");
            wgpu::IndexFormat::Uint32
        }
    }

    /// Ensure the 1x1 white fallback texture is created (cached after first call).
    fn ensure_fallback(&mut self) {
        if self.fallback.is_some() {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fallback_white_1x1"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            &[255u8, 255, 255, 255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fallback_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        self.fallback = Some(FallbackTexture {
            texture,
            view,
            sampler,
        });
    }

    /// Replay recorded commands into a wgpu::RenderPass.
    fn replay_commands<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        bind_groups: &'a [(usize, u32, wgpu::BindGroup)],
    ) {
        use crate::resolve::*;

        let mut bg_cursor = 0usize;

        for (cmd_idx, cmd) in self.commands.iter().enumerate() {
            match cmd {
                GraphicsCommand::BindPipeline(handle) => {
                    if let Some(pipeline) = resolve_graphics_pipeline(handle) {
                        rpass.set_pipeline(pipeline);
                    }
                }
                GraphicsCommand::BindResources(handle) => {
                    if let Some(bind_group) = resolve_bind_group(handle) {
                        rpass.set_bind_group(0, bind_group, &[]);
                    }
                }
                GraphicsCommand::BindVertexBuffers { buffers, offsets } => {
                    for (i, buf_handle) in buffers.iter().enumerate() {
                        if let Some(buffer) = resolve_buffer(buf_handle) {
                            let offset = offsets.get(i).copied().unwrap_or(0);
                            rpass.set_vertex_buffer(i as u32, buffer.slice(offset..));
                        } else {
                            log::warn!("Failed to resolve vertex buffer slot={}", i);
                        }
                    }
                }
                GraphicsCommand::SetViewport(vp) => {
                    rpass.set_viewport(vp.x, vp.y, vp.width, vp.height, vp.min_depth, vp.max_depth);
                }
                GraphicsCommand::SetScissor(sc) => {
                    rpass.set_scissor_rect(sc.x as u32, sc.y as u32, sc.width, sc.height);
                }
                GraphicsCommand::SetBlendConstant(color) => {
                    rpass.set_blend_constant(wgpu::Color {
                        r: color[0] as f64,
                        g: color[1] as f64,
                        b: color[2] as f64,
                        a: color[3] as f64,
                    });
                }
                GraphicsCommand::SetStencilRef(val) => {
                    rpass.set_stencil_reference(*val);
                }
                GraphicsCommand::Draw(op) => {
                    rpass.draw(
                        op.base_vertex..(op.base_vertex + op.vertex_count),
                        op.base_instance..(op.base_instance + op.instance_count),
                    );
                }
                GraphicsCommand::DrawIndexed { index_buffer, op } => {
                    if let Some(buffer) = resolve_buffer(index_buffer) {
                        let index_format = Self::get_index_format(index_buffer);
                        rpass.set_index_buffer(buffer.slice(..), index_format);
                        rpass.draw_indexed(
                            op.base_index..(op.base_index + op.index_count),
                            op.base_vertex,
                            op.base_instance..(op.base_instance + op.instance_count),
                        );
                    } else {
                        log::warn!("Failed to resolve index buffer for draw_indexed");
                    }
                }
                GraphicsCommand::DrawIndirect(op) => {
                    if let Some(buffer) = resolve_buffer(&op.draw_buffer) {
                        rpass.draw_indirect(buffer, op.draw_buffer_byte_offset as u64);
                    }
                }
                GraphicsCommand::DrawIndexedIndirect { index_buffer, op } => {
                    if let Some(idx_buf) = resolve_buffer(index_buffer) {
                        let index_format = Self::get_index_format(index_buffer);
                        rpass.set_index_buffer(idx_buf.slice(..), index_format);
                        if let Some(indirect_buf) = resolve_buffer(&op.draw_buffer) {
                            rpass.draw_indexed_indirect(
                                indirect_buf,
                                op.draw_buffer_byte_offset as u64,
                            );
                        }
                    }
                }
                // SetUniform, BindTextureGroup, BindStorageBuffer are pre-baked into bind_groups
                GraphicsCommand::SetUniform { .. }
                | GraphicsCommand::BindTextureGroup { .. }
                | GraphicsCommand::BindStorageBuffer { .. } => {
                    while bg_cursor < bind_groups.len() && bind_groups[bg_cursor].0 == cmd_idx {
                        let (_, group_idx, bg) = &bind_groups[bg_cursor];
                        rpass.set_bind_group(*group_idx, bg, &[]);
                        bg_cursor += 1;
                    }
                }
                GraphicsCommand::PushDebugGroup(label) => {
                    rpass.push_debug_group(label);
                }
                GraphicsCommand::PopDebugGroup => {
                    rpass.pop_debug_group();
                }
                GraphicsCommand::InsertDebugMarker(label) => {
                    rpass.insert_debug_marker(label);
                }
            }
        }
    }

    /// Pre-create all wgpu bind groups for SetUniform and BindTextureGroup commands.
    ///
    /// Returns Vec of (command_index, group_index, BindGroup) tuples ordered by
    /// command_index so replay_commands can apply them at the correct time.
    fn create_all_bind_groups(&mut self) -> Vec<(usize, u32, wgpu::BindGroup)> {
        use crate::resolve::{
            resolve_buffer, resolve_graphics_pipeline, resolve_sampler, resolve_texture_view,
        };

        // Locate the pipeline handle (cloned to release borrow on self.commands
        // before calling ensure_fallback() which needs &mut self).
        let pipeline_handle = self.commands.iter().find_map(|cmd| {
            if let GraphicsCommand::BindPipeline(h) = cmd {
                Some(h.clone())
            } else {
                None
            }
        });
        let Some(ph) = pipeline_handle else {
            return Vec::new();
        };

        // Ensure fallback is cached before borrowing pipeline (avoids &mut conflict)
        self.ensure_fallback();
        // Extract fallback references directly (safe because ensure_fallback() guarantees Some())
        let fallback = self.fallback.as_ref().unwrap();
        let fb_view = &fallback.view;
        let fb_sampler = &fallback.sampler;

        let Some(pipeline) = resolve_graphics_pipeline(&ph) else {
            log::warn!("Failed to resolve pipeline for bind groups");
            return Vec::new();
        };

        let mut result = Vec::new();

        for (cmd_idx, cmd) in self.commands.iter().enumerate() {
            match cmd {
                GraphicsCommand::SetUniform { bind_index, data } => {
                    // Uniform buffer bind group: one UBO entry at binding 0
                    let aligned = ((data.len() + 15) & !15).max(16) as u64;
                    let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("uniform"),
                        size: aligned,
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                    self.queue.write_buffer(&buf, 0, data);
                    device_push_scope(&self.device);
                    let bgl = pipeline.get_bind_group_layout(*bind_index);
                    let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &bgl,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: buf.as_entire_binding(),
                        }],
                        label: Some("uniform_bg"),
                    });
                    if device_pop_scope_has_error(&self.device) {
                        log::debug!(
                            "SetUniform: group {} rejected UBO payload, skipping bind",
                            bind_index
                        );
                        continue;
                    }
                    result.push((cmd_idx, *bind_index, bg));
                }
                GraphicsCommand::BindTextureGroup {
                    group_index,
                    textures,
                    samplers,
                } => {
                    // Texture bind group: alternating texture_view (even) + sampler (odd) entries.
                    // Number of slots = max(textures.len(), samplers.len()), paired by index.
                    // When empty (no textures bound), use 7 fallback slots to satisfy the
                    // pipeline layout (group 3 always has 7 tex+sampler pairs in lit shaders).
                    // Use actual count when textures/samplers provided.
                    // Only fall back to 7 slots for group 3 when arrays are empty
                    // (material texture group with no loaded textures needs 7 fallbacks).
                    let provided = textures.len().max(samplers.len());
                    let n_slots = if provided > 0 {
                        provided
                    } else if *group_index == 3 {
                        7 // fallback for empty material texture slots
                    } else {
                        0
                    };
                    if n_slots == 0 {
                        continue;
                    }

                    // Get BGL from pipeline (may fail if group not used in shader)
                    device_push_scope(&self.device);
                    let bgl = pipeline.get_bind_group_layout(*group_index);
                    if device_pop_scope_has_error(&self.device) {
                        // Group index not declared in shader -- skip silently
                        log::debug!(
                            "BindTextureGroup: group {} not in pipeline layout, skipping",
                            group_index
                        );
                        continue;
                    }

                    // Build entries: binding 2*i = texture_view, binding 2*i+1 = sampler.
                    // Resolved views/samplers are either from HGI handles or fb_view/fb_sampler,
                    // both of which live for the duration of this function scope.
                    let mut entries: Vec<wgpu::BindGroupEntry> = Vec::with_capacity(n_slots * 2);

                    for i in 0..n_slots {
                        // Texture view: resolve handle or use fallback
                        let tex_view: &wgpu::TextureView = if let Some(h) = textures.get(i) {
                            resolve_texture_view(h).unwrap_or(&fb_view)
                        } else {
                            &fb_view
                        };

                        // Sampler: resolve handle or use fallback
                        let sampler: &wgpu::Sampler = if let Some(h) = samplers.get(i) {
                            resolve_sampler(h).unwrap_or(&fb_sampler)
                        } else {
                            &fb_sampler
                        };

                        entries.push(wgpu::BindGroupEntry {
                            binding: (i * 2) as u32,
                            resource: wgpu::BindingResource::TextureView(tex_view),
                        });
                        entries.push(wgpu::BindGroupEntry {
                            binding: (i * 2 + 1) as u32,
                            resource: wgpu::BindingResource::Sampler(sampler),
                        });
                    }

                    let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        layout: &bgl,
                        entries: &entries,
                        label: Some("texture_group_bg"),
                    });
                    result.push((cmd_idx, *group_index, bg));
                }
                GraphicsCommand::BindStorageBuffer {
                    group_index,
                    binding,
                    buffer,
                } => {
                    // Storage buffer bind group: read-only vs read-write is defined by
                    // the pipeline layout recovered from shader reflection.
                    if let Some(wgpu_buf) = resolve_buffer(buffer) {
                        device_push_scope(&self.device);
                        let bgl = pipeline.get_bind_group_layout(*group_index);
                        if device_pop_scope_has_error(&self.device) {
                            log::debug!(
                                "BindStorageBuffer: group {} not in pipeline layout, skipping",
                                group_index
                            );
                            continue;
                        }
                        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &bgl,
                            entries: &[wgpu::BindGroupEntry {
                                binding: *binding,
                                resource: wgpu_buf.as_entire_binding(),
                            }],
                            label: Some("storage_bg"),
                        });
                        result.push((cmd_idx, *group_index, bg));
                    }
                }
                _ => {}
            }
        }

        result
    }
}

// --- wgpu error scope helpers (pollster-based) ---

/// Push a validation error scope for silent BGL validity check.
fn device_push_scope(device: &wgpu::Device) {
    device.push_error_scope(wgpu::ErrorFilter::Validation);
}

/// Pop error scope and return true if a validation error occurred.
fn device_pop_scope_has_error(device: &wgpu::Device) -> bool {
    pollster::block_on(device.pop_error_scope()).is_some()
}

impl HgiCmds for WgpuGraphicsCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn push_debug_group(&mut self, label: &str) {
        self.commands
            .push(GraphicsCommand::PushDebugGroup(label.to_string()));
    }

    fn pop_debug_group(&mut self) {
        self.commands.push(GraphicsCommand::PopDebugGroup);
    }

    fn insert_debug_marker(&mut self, label: &str) {
        self.commands
            .push(GraphicsCommand::InsertDebugMarker(label.to_string()));
    }

    fn execute_submit(&mut self) {
        use crate::resolve::resolve_texture_view;
        use usd_hgi::HgiFormat;
        use usd_hgi::enums::{HgiAttachmentLoadOp, HgiAttachmentStoreOp};

        let Some(mut encoder) = self.encoder.take() else {
            log::warn!("Graphics cmds already submitted");
            return;
        };

        log::trace!(
            "Submitting graphics cmds with {} commands, {} color attachments",
            self.commands.len(),
            self.descriptor.color_textures.len()
        );

        // Pre-create all bind groups (uniforms + textures) BEFORE building attachments.
        // Must be done before any immutable borrows via resolve_texture_view to avoid
        // borrow checker conflict with the &mut self needed by ensure_fallback().
        let bind_groups = self.create_all_bind_groups();

        // Build color attachments from descriptor
        let mut color_attachments = Vec::new();
        for (i, tex_handle) in self.descriptor.color_textures.iter().enumerate() {
            if let Some(view) = resolve_texture_view(tex_handle) {
                let desc = self
                    .descriptor
                    .color_attachment_descs
                    .get(i)
                    .cloned()
                    .unwrap_or_default();

                let load_op = match desc.load_op {
                    HgiAttachmentLoadOp::Clear => wgpu::LoadOp::Clear(wgpu::Color {
                        r: desc.clear_value[0] as f64,
                        g: desc.clear_value[1] as f64,
                        b: desc.clear_value[2] as f64,
                        a: desc.clear_value[3] as f64,
                    }),
                    HgiAttachmentLoadOp::Load => wgpu::LoadOp::Load,
                    HgiAttachmentLoadOp::DontCare => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                };
                let store_op = match desc.store_op {
                    HgiAttachmentStoreOp::Store => wgpu::StoreOp::Store,
                    HgiAttachmentStoreOp::DontCare => wgpu::StoreOp::Discard,
                };
                let resolve_target = self
                    .descriptor
                    .color_resolve_textures
                    .get(i)
                    .and_then(|h| resolve_texture_view(h));

                color_attachments.push(Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: store_op,
                    },
                    depth_slice: None,
                }));
            }
        }

        // Build depth-stencil attachment if present
        let depth_stencil_attachment = if self.descriptor.depth_texture.is_valid() {
            resolve_texture_view(&self.descriptor.depth_texture).map(|view| {
                let desc = &self.descriptor.depth_attachment_desc;

                let depth_load_op = match desc.load_op {
                    HgiAttachmentLoadOp::Clear => wgpu::LoadOp::Clear(desc.clear_value[0]),
                    HgiAttachmentLoadOp::Load => wgpu::LoadOp::Load,
                    HgiAttachmentLoadOp::DontCare => wgpu::LoadOp::Clear(1.0),
                };
                let depth_store_op = match desc.store_op {
                    HgiAttachmentStoreOp::Store => wgpu::StoreOp::Store,
                    HgiAttachmentStoreOp::DontCare => wgpu::StoreOp::Discard,
                };
                // Only set stencil_ops when format actually has a stencil component.
                // Depth32Float has no stencil; setting Some(stencil_ops) for it is invalid.
                let has_stencil = matches!(desc.format, HgiFormat::Float32UInt8);
                let stencil_ops = if has_stencil {
                    let stencil_load_op = match desc.load_op {
                        HgiAttachmentLoadOp::Clear => wgpu::LoadOp::Clear(0),
                        HgiAttachmentLoadOp::Load => wgpu::LoadOp::Load,
                        HgiAttachmentLoadOp::DontCare => wgpu::LoadOp::Clear(0),
                    };
                    let stencil_store_op = match desc.store_op {
                        HgiAttachmentStoreOp::Store => wgpu::StoreOp::Store,
                        HgiAttachmentStoreOp::DontCare => wgpu::StoreOp::Discard,
                    };
                    Some(wgpu::Operations {
                        load: stencil_load_op,
                        store: stencil_store_op,
                    })
                } else {
                    None
                };

                wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: depth_load_op,
                        store: depth_store_op,
                    }),
                    stencil_ops,
                }
            })
        } else {
            None
        };

        let has_any_attachment =
            !color_attachments.is_empty() || depth_stencil_attachment.is_some();
        if has_any_attachment {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("HgiWgpu Graphics Commands"),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            self.replay_commands(&mut rpass, &bind_groups);
        } else {
            // OpenUSD allows submitting graphics cmds without attachments (e.g. vertex-only setup).
            let had_draw_calls = self.commands.iter().any(|cmd| {
                matches!(
                    cmd,
                    GraphicsCommand::Draw(_)
                        | GraphicsCommand::DrawIndexed { .. }
                        | GraphicsCommand::DrawIndirect(_)
                        | GraphicsCommand::DrawIndexedIndirect { .. }
                )
            });
            if had_draw_calls {
                log::warn!(
                    "Skipping graphics pass submit: no resolved color/depth attachments for {} recorded commands",
                    self.commands.len()
                );
            } else {
                log::trace!(
                    "Skipping graphics render pass: no resolved color/depth attachments ({} recorded commands)",
                    self.commands.len()
                );
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.submitted = true;
    }
}

impl HgiGraphicsCmds for WgpuGraphicsCmds {
    fn bind_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle) {
        self.commands
            .push(GraphicsCommand::BindPipeline(pipeline.clone()));
    }

    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle) {
        self.commands
            .push(GraphicsCommand::BindResources(resources.clone()));
    }

    fn set_constant_values(
        &mut self,
        _pipeline: &HgiGraphicsPipelineHandle,
        _stages: HgiShaderStage,
        bind_index: u32,
        data: &[u8],
    ) {
        self.commands.push(GraphicsCommand::SetUniform {
            bind_index,
            data: data.to_vec(),
        });
    }

    fn bind_vertex_buffers(&mut self, buffers: &[HgiBufferHandle], offsets: &[u64]) {
        self.commands.push(GraphicsCommand::BindVertexBuffers {
            buffers: buffers.to_vec(),
            offsets: offsets.to_vec(),
        });
    }

    fn set_viewport(&mut self, viewport: &HgiViewport) {
        self.commands.push(GraphicsCommand::SetViewport(*viewport));
    }

    fn set_scissor(&mut self, scissor: &HgiScissor) {
        self.commands.push(GraphicsCommand::SetScissor(*scissor));
    }

    fn set_blend_constant_color(&mut self, color: &Vec4f) {
        self.commands
            .push(GraphicsCommand::SetBlendConstant(*color));
    }

    fn set_stencil_reference_value(&mut self, value: u32) {
        self.commands.push(GraphicsCommand::SetStencilRef(value));
    }

    fn draw(&mut self, op: &HgiDrawOp) {
        self.commands.push(GraphicsCommand::Draw(*op));
    }

    fn draw_indexed(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndexedOp) {
        self.commands.push(GraphicsCommand::DrawIndexed {
            index_buffer: index_buffer.clone(),
            op: *op,
        });
    }

    fn draw_indirect(&mut self, op: &HgiDrawIndirectOp) {
        self.commands
            .push(GraphicsCommand::DrawIndirect(op.clone()));
    }

    fn draw_indexed_indirect(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndirectOp) {
        self.commands.push(GraphicsCommand::DrawIndexedIndirect {
            index_buffer: index_buffer.clone(),
            op: op.clone(),
        });
    }

    fn memory_barrier(&mut self, _barrier: HgiMemoryBarrier) {
        // wgpu handles barriers automatically via resource tracking.
    }

    fn bind_texture_group(
        &mut self,
        group_index: u32,
        textures: &[HgiTextureHandle],
        samplers: &[HgiSamplerHandle],
    ) {
        self.commands.push(GraphicsCommand::BindTextureGroup {
            group_index,
            textures: textures.to_vec(),
            samplers: samplers.to_vec(),
        });
    }

    fn bind_storage_buffer(&mut self, group_index: u32, binding: u32, buffer: &HgiBufferHandle) {
        self.commands.push(GraphicsCommand::BindStorageBuffer {
            group_index,
            binding,
            buffer: buffer.clone(),
        });
    }
}
