//! OpenGL compute commands implementation

use usd_hgi::*;

/// OpenGL compute commands buffer
///
/// Records compute shader dispatch commands.
#[derive(Debug)]
pub struct HgiGLComputeCmds {
    /// Recorded commands
    commands: Vec<ComputeCommand>,

    /// Whether commands have been submitted
    submitted: bool,

    /// Current bound pipeline
    current_pipeline: Option<HgiComputePipelineHandle>,
}

/// Individual compute command types
#[derive(Debug)]
#[allow(dead_code)]
enum ComputeCommand {
    BindPipeline(HgiComputePipelineHandle),
    BindResources(HgiResourceBindingsHandle),
    Dispatch(HgiComputeDispatchOp),
    PushDebugGroup(String),
    PopDebugGroup,
    InsertDebugMarker(String),
    MemoryBarrier(HgiMemoryBarrier),
}

impl HgiGLComputeCmds {
    /// Create new compute commands buffer
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            submitted: false,
            current_pipeline: None,
        }
    }

    /// Execute all recorded commands
    #[cfg(feature = "opengl")]
    pub fn execute(&mut self) {
        if self.submitted {
            return;
        }

        for cmd in &self.commands {
            match cmd {
                ComputeCommand::BindPipeline(pipeline) => {
                    self.execute_bind_pipeline(pipeline);
                }
                ComputeCommand::BindResources(resources) => {
                    self.execute_bind_resources(resources);
                }
                ComputeCommand::Dispatch(dispatch) => {
                    self.execute_dispatch(dispatch);
                }
                ComputeCommand::PushDebugGroup(label) => {
                    self.execute_push_debug_group(label);
                }
                ComputeCommand::PopDebugGroup => {
                    self.execute_pop_debug_group();
                }
                ComputeCommand::InsertDebugMarker(label) => {
                    self.execute_debug_marker(label);
                }
                ComputeCommand::MemoryBarrier(barrier) => {
                    self.execute_memory_barrier(*barrier);
                }
            }
        }

        self.submitted = true;
    }

    /// Execute all recorded commands (stub when opengl feature disabled)
    #[cfg(not(feature = "opengl"))]
    pub fn execute(&mut self) {
        self.submitted = true;
    }

    /// Execute pipeline binding
    #[cfg(feature = "opengl")]
    fn execute_bind_pipeline(&self, pipeline: &HgiComputePipelineHandle) {
        if let Some(p) = pipeline.get() {
            if let Some(program) = p.descriptor().shader_program.get() {
                let program_id = program.raw_resource() as u32;
                if program_id != 0 {
                    unsafe {
                        gl::UseProgram(program_id);
                    }
                }
            }
        }
    }

    /// Execute resource bindings
    #[cfg(feature = "opengl")]
    fn execute_bind_resources(&self, resources: &HgiResourceBindingsHandle) {
        if let Some(bindings) = resources.get() {
            let desc = bindings.descriptor();

            for binding in &desc.buffer_bindings {
                if let Some(buf) = binding.buffers.first().and_then(|h| h.get()) {
                    let buf_id = buf.raw_resource() as u32;
                    if buf_id != 0 {
                        unsafe {
                            gl::BindBufferBase(gl::UNIFORM_BUFFER, binding.binding_index, buf_id);
                        }
                    }
                }
            }

            for binding in &desc.texture_bindings {
                if let Some(tex) = binding.textures.first().and_then(|h| h.get()) {
                    let tex_id = tex.raw_resource() as u32;
                    if tex_id != 0 {
                        unsafe {
                            gl::BindTextureUnit(binding.binding_index, tex_id);
                        }
                    }
                }
                if let Some(smp) = binding.samplers.first().and_then(|h| h.get()) {
                    let smp_id = smp.raw_resource() as u32;
                    if smp_id != 0 {
                        unsafe {
                            gl::BindSampler(binding.binding_index, smp_id);
                        }
                    }
                }
            }
        }
    }

    /// Execute compute dispatch
    #[cfg(feature = "opengl")]
    fn execute_dispatch(&self, dispatch: &HgiComputeDispatchOp) {
        unsafe {
            gl::DispatchCompute(
                dispatch.work_group_count_x,
                dispatch.work_group_count_y,
                dispatch.work_group_count_z,
            );
        }
    }

    /// Execute push debug group
    #[cfg(feature = "opengl")]
    fn execute_push_debug_group(&self, label: &str) {
        use std::ffi::CString;
        if let Ok(c_label) = CString::new(label) {
            unsafe {
                gl::PushDebugGroup(
                    gl::DEBUG_SOURCE_APPLICATION,
                    0,
                    label.len() as i32,
                    c_label.as_ptr(),
                );
            }
        }
    }

    /// Execute pop debug group
    #[cfg(feature = "opengl")]
    fn execute_pop_debug_group(&self) {
        unsafe {
            gl::PopDebugGroup();
        }
    }

    /// Execute debug marker
    #[cfg(feature = "opengl")]
    fn execute_debug_marker(&self, label: &str) {
        use std::ffi::CString;
        if let Ok(c_label) = CString::new(label) {
            unsafe {
                gl::DebugMessageInsert(
                    gl::DEBUG_SOURCE_APPLICATION,
                    gl::DEBUG_TYPE_MARKER,
                    0,
                    gl::DEBUG_SEVERITY_NOTIFICATION,
                    label.len() as i32,
                    c_label.as_ptr(),
                );
            }
        }
    }

    /// Execute memory barrier
    #[cfg(feature = "opengl")]
    fn execute_memory_barrier(&self, barrier: HgiMemoryBarrier) {
        let gl_barrier = hgi_memory_barrier_to_gl(barrier);
        if gl_barrier != 0 {
            unsafe {
                gl::MemoryBarrier(gl_barrier);
            }
        }
    }
}

/// Convert HGI memory barrier to GL barrier bits
#[cfg(feature = "opengl")]
fn hgi_memory_barrier_to_gl(barrier: HgiMemoryBarrier) -> u32 {
    if barrier.is_empty() || barrier == HgiMemoryBarrier::NONE {
        0
    } else {
        gl::ALL_BARRIER_BITS
    }
}

impl Default for HgiGLComputeCmds {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiCmds for HgiGLComputeCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn execute_submit(&mut self) {
        self.execute();
    }

    fn push_debug_group(&mut self, label: &str) {
        self.commands
            .push(ComputeCommand::PushDebugGroup(label.to_string()));
    }

    fn pop_debug_group(&mut self) {
        self.commands.push(ComputeCommand::PopDebugGroup);
    }

    fn insert_debug_marker(&mut self, label: &str) {
        self.commands
            .push(ComputeCommand::InsertDebugMarker(label.to_string()));
    }
}

impl HgiComputeCmds for HgiGLComputeCmds {
    fn bind_pipeline(&mut self, pipeline: &HgiComputePipelineHandle) {
        self.current_pipeline = Some(pipeline.clone());
        self.commands
            .push(ComputeCommand::BindPipeline(pipeline.clone()));
    }

    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle) {
        self.commands
            .push(ComputeCommand::BindResources(resources.clone()));
    }

    fn dispatch(&mut self, dispatch: &HgiComputeDispatchOp) {
        self.commands.push(ComputeCommand::Dispatch(*dispatch));
    }

    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier) {
        self.commands.push(ComputeCommand::MemoryBarrier(barrier));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_cmds_creation() {
        let cmds = HgiGLComputeCmds::new();
        assert!(!cmds.is_submitted());
        assert_eq!(cmds.commands.len(), 0);
    }

    #[test]
    fn test_record_commands() {
        let mut cmds = HgiGLComputeCmds::new();

        cmds.dispatch(&HgiComputeDispatchOp::new(16, 16, 1));
        cmds.insert_debug_marker("Test compute dispatch");

        assert_eq!(cmds.commands.len(), 2);
        assert!(!cmds.is_submitted());
    }
}
