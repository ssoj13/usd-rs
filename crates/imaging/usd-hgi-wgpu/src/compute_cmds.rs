//! wgpu compute command buffer implementation for HGI.
//!
//! Uses a deferred recording model: commands are stored as enums and
//! replayed into a wgpu::ComputePass during execute/submit.

use std::sync::Arc;

use usd_hgi::cmds::HgiCmds;
use usd_hgi::compute_cmds::{HgiComputeCmds, HgiComputeDispatchOp};
use usd_hgi::compute_pipeline::HgiComputePipelineHandle;
use usd_hgi::enums::HgiMemoryBarrier;
use usd_hgi::resource_bindings::HgiResourceBindingsHandle;

use super::compute_pipeline::WgpuComputePipeline;
use super::resource_bindings::WgpuResourceBindings;

/// Deferred compute command.
enum ComputeCmd {
    BindPipeline(HgiComputePipelineHandle),
    BindResources(HgiResourceBindingsHandle),
    /// Upload uniform data for a bind group index (slot, not byte offset).
    SetUniform {
        bind_index: u32,
        data: Vec<u8>,
    },
    Dispatch {
        x: u32,
        y: u32,
        z: u32,
    },
    PushDebugGroup(String),
    PopDebugGroup,
    InsertDebugMarker(String),
}

/// wgpu compute command buffer.
///
/// Records commands into a Vec and replays them into a wgpu::ComputePass
/// when execute() is called. The wgpu::CommandEncoder is created at
/// execute time because compute passes borrow the encoder mutably.
pub struct WgpuComputeCmds {
    commands: Vec<ComputeCmd>,
    submitted: bool,
    /// Encoder and device are stored so we can create the pass at submit time
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
}

impl WgpuComputeCmds {
    /// Create a new compute command buffer.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self {
            commands: Vec::new(),
            submitted: false,
            device,
            queue,
        }
    }

    /// Record a bind-pipeline command using the concrete wgpu type.
    ///
    /// This is a convenience API for direct wgpu backend usage.
    pub fn bind_pipeline_direct(&mut self, pipeline: Arc<WgpuComputePipeline>) {
        let handle = HgiComputePipelineHandle::new(pipeline, 0);
        self.commands.push(ComputeCmd::BindPipeline(handle));
    }

    /// Record a bind-resources command using the concrete wgpu type.
    ///
    /// This is a convenience API for direct wgpu backend usage.
    pub fn bind_resources_direct(&mut self, bindings: Arc<WgpuResourceBindings>) {
        let handle = HgiResourceBindingsHandle::new(bindings, 0);
        self.commands.push(ComputeCmd::BindResources(handle));
    }

    /// Execute all recorded commands by creating a compute pass and
    /// submitting the resulting command buffer.
    pub fn execute(&mut self) {
        if self.submitted {
            return;
        }

        // Pre-locate pipeline handle to use for SetUniform bind group creation.
        let pipeline_handle = self.commands.iter().find_map(|c| {
            if let ComputeCmd::BindPipeline(h) = c {
                Some(h.clone())
            } else {
                None
            }
        });

        // Pre-create bind groups for SetUniform before the pass borrows the encoder.
        let mut prebuilt_uniforms: Vec<(u32, wgpu::BindGroup)> = Vec::new();
        if let Some(ref ph) = pipeline_handle {
            if let Some(wgpu_pipe) = crate::resolve::resolve_compute_pipeline(ph) {
                for cmd in &self.commands {
                    if let ComputeCmd::SetUniform { bind_index, data } = cmd {
                        let aligned = ((data.len() + 15) & !15).max(16) as u64;
                        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("compute_uniform"),
                            size: aligned,
                            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        self.queue.write_buffer(&buf, 0, data);
                        let bgl = wgpu_pipe.get_bind_group_layout(*bind_index);
                        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                            layout: &bgl,
                            entries: &[wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buf.as_entire_binding(),
                            }],
                            label: Some("compute_uniform_bg"),
                        });
                        prebuilt_uniforms.push((*bind_index, bg));
                    }
                }
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("HgiComputeCmds"),
            });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("HgiComputePass"),
                timestamp_writes: None,
            });

            // Replay commands using resolve helpers to downcast handles
            let mut uniform_cursor = 0usize;
            for cmd in &self.commands {
                match cmd {
                    ComputeCmd::BindPipeline(pipeline_handle) => {
                        if let Some(wgpu_pipeline) =
                            crate::resolve::resolve_compute_pipeline(pipeline_handle)
                        {
                            pass.set_pipeline(wgpu_pipeline);
                        } else {
                            log::warn!("Failed to resolve compute pipeline during replay");
                        }
                    }
                    ComputeCmd::BindResources(bindings_handle) => {
                        if let Some(bind_group) =
                            crate::resolve::resolve_bind_group(bindings_handle)
                        {
                            pass.set_bind_group(0, bind_group, &[]);
                        } else {
                            log::warn!("Failed to resolve resource bindings during replay");
                        }
                    }
                    ComputeCmd::SetUniform { .. } => {
                        // Bind pre-built uniform bind group at the correct slot
                        if let Some((bind_index, bg)) = prebuilt_uniforms.get(uniform_cursor) {
                            pass.set_bind_group(*bind_index, bg, &[]);
                            uniform_cursor += 1;
                        }
                    }
                    ComputeCmd::Dispatch { x, y, z } => {
                        pass.dispatch_workgroups(*x, *y, *z);
                    }
                    ComputeCmd::PushDebugGroup(label) => {
                        pass.push_debug_group(label);
                    }
                    ComputeCmd::PopDebugGroup => {
                        pass.pop_debug_group();
                    }
                    ComputeCmd::InsertDebugMarker(label) => {
                        pass.insert_debug_marker(label);
                    }
                }
            }

            // Pipeline and bindings state is tracked via wgpu internally
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.submitted = true;
    }
}

impl HgiCmds for WgpuComputeCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn push_debug_group(&mut self, label: &str) {
        self.commands
            .push(ComputeCmd::PushDebugGroup(label.to_string()));
    }

    fn pop_debug_group(&mut self) {
        self.commands.push(ComputeCmd::PopDebugGroup);
    }

    fn insert_debug_marker(&mut self, label: &str) {
        self.commands
            .push(ComputeCmd::InsertDebugMarker(label.to_string()));
    }

    fn execute_submit(&mut self) {
        self.execute();
    }
}

impl HgiComputeCmds for WgpuComputeCmds {
    fn bind_pipeline(&mut self, pipeline: &HgiComputePipelineHandle) {
        self.commands
            .push(ComputeCmd::BindPipeline(pipeline.clone()));
    }

    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle) {
        self.commands
            .push(ComputeCmd::BindResources(resources.clone()));
    }

    fn set_constant_values(
        &mut self,
        _pipeline: &HgiComputePipelineHandle,
        bind_index: u32,
        data: &[u8],
    ) {
        self.commands.push(ComputeCmd::SetUniform {
            bind_index,
            data: data.to_vec(),
        });
    }

    fn dispatch(&mut self, dispatch: &HgiComputeDispatchOp) {
        self.commands.push(ComputeCmd::Dispatch {
            x: dispatch.work_group_count_x,
            y: dispatch.work_group_count_y,
            z: dispatch.work_group_count_z,
        });
    }

    fn memory_barrier(&mut self, _barrier: HgiMemoryBarrier) {
        // wgpu handles synchronization automatically within a compute pass.
        // Explicit barriers are not needed (and not exposed).
    }

    fn get_dispatch_method(&self) -> usd_hgi::enums::HgiComputeDispatch {
        // wgpu supports concurrent/parallel dispatch by default.
        usd_hgi::enums::HgiComputeDispatch::Concurrent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: actual GPU tests require a wgpu device.
    // These tests verify the recording side only.

    #[test]
    fn test_dispatch_recording() {
        // We can't create a real device in unit tests without pollster/GPU,
        // so just verify the command enum is constructible.
        let dispatch = HgiComputeDispatchOp::new(8, 8, 1);
        assert_eq!(dispatch.work_group_count_x, 8);
    }
}
