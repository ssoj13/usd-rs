//! OpenGL graphics commands implementation

use usd_gf::Vec4f;
use usd_hgi::*;

#[cfg(feature = "opengl")]
use super::conversions::hgi_primitive_type_to_gl;

/// OpenGL graphics commands buffer
///
/// Records rendering commands that will be executed on submission.
/// On execution the descriptor's attachments are used to build and bind
/// a framebuffer object (FBO) matching C++ `HgiGLOps::BindFramebufferOp()`.
#[derive(Debug)]
#[allow(dead_code)]
pub struct HgiGLGraphicsCmds {
    /// Descriptor with attachment textures/ops used to build the FBO
    desc: HgiGraphicsCmdsDesc,

    /// Recorded commands
    commands: Vec<GraphicsCommand>,

    /// Whether commands have been submitted
    submitted: bool,

    /// Current bound pipeline (for state tracking and primitive type)
    current_pipeline: Option<HgiGraphicsPipelineHandle>,

    /// Cached FBO object name (created on first execute, 0 = no FBO / default)
    fbo: u32,
}

/// Individual graphics command types
#[derive(Debug)]
#[allow(dead_code)]
enum GraphicsCommand {
    BindPipeline(HgiGraphicsPipelineHandle),
    BindResources(HgiResourceBindingsHandle),
    /// Set uniform/push constants via glProgramUniform (P1-2)
    SetConstantValues(HgiGraphicsPipelineHandle, u32, Vec<u8>),
    BindVertexBuffers(Vec<HgiBufferHandle>, Vec<u64>),
    SetViewport(HgiViewport),
    SetScissor(HgiScissor),
    SetBlendConstantColor(Vec4f),
    SetStencilRef(u32),
    Draw(HgiDrawOp),
    DrawIndexed(HgiBufferHandle, HgiDrawIndexedOp),
    DrawIndirect(HgiDrawIndirectOp),
    DrawIndexedIndirect(HgiBufferHandle, HgiDrawIndirectOp),
    PushDebugGroup(String),
    PopDebugGroup,
    InsertDebugMarker(String),
    MemoryBarrier(HgiMemoryBarrier),
}

impl HgiGLGraphicsCmds {
    /// Create a graphics command buffer with no render-target attachments
    /// (renders to the default framebuffer / window surface).
    pub fn new() -> Self {
        Self {
            desc: HgiGraphicsCmdsDesc::new(),
            commands: Vec::new(),
            submitted: false,
            current_pipeline: None,
            fbo: 0,
        }
    }

    /// Create a graphics command buffer that will render into the textures
    /// specified by `desc`. An FBO is created and bound on first execution.
    ///
    /// Matches C++ `HgiGL::CreateGraphicsCmds(HgiGraphicsCmdsDesc const& desc)`.
    pub fn new_with_desc(desc: HgiGraphicsCmdsDesc) -> Self {
        Self {
            desc,
            commands: Vec::new(),
            submitted: false,
            current_pipeline: None,
            fbo: 0,
        }
    }

    /// Build and bind an FBO from the descriptor's attachments.
    ///
    /// Simplified version of C++ `HgiGLDevice::AcquireFramebuffer()` +
    /// `HgiGLOps::BindFramebufferOp()`. We don't cache FBOs here — the
    /// Device/ContextArena cache is a P2 item.
    #[cfg(feature = "opengl")]
    fn bind_framebuffer(&mut self) {
        use gl::types::*;

        if !self.desc.has_attachments() {
            // No attachments — render to default framebuffer (window surface)
            unsafe {
                gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            }
            return;
        }

        if self.fbo == 0 {
            // Create a new FBO for this cmd buffer
            unsafe {
                gl::CreateFramebuffers(1, &mut self.fbo);
            }
        }

        let fbo = self.fbo;
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::Enable(gl::FRAMEBUFFER_SRGB);

            // Attach color textures
            for (i, tex_handle) in self.desc.color_textures.iter().enumerate() {
                if let Some(tex) = tex_handle.get() {
                    let tex_id = tex.raw_resource() as GLuint;
                    if tex_id != 0 {
                        let desc = &self.desc.color_attachment_descs;
                        let layer = desc.get(i).map_or(0, |d| d.layer_index) as i32;
                        let mip = desc.get(i).map_or(0, |d| d.mip_level) as i32;
                        if layer > 0 {
                            gl::NamedFramebufferTextureLayer(
                                fbo,
                                gl::COLOR_ATTACHMENT0 + i as u32,
                                tex_id,
                                mip,
                                layer,
                            );
                        } else {
                            gl::NamedFramebufferTexture(
                                fbo,
                                gl::COLOR_ATTACHMENT0 + i as u32,
                                tex_id,
                                mip,
                            );
                        }
                    }
                }
            }

            // Attach depth texture
            if self.desc.depth_texture.is_valid() {
                if let Some(tex) = self.desc.depth_texture.get() {
                    let tex_id = tex.raw_resource() as GLuint;
                    if tex_id != 0 {
                        let mip = self.desc.depth_attachment_desc.mip_level as i32;
                        let layer = self.desc.depth_attachment_desc.layer_index as i32;
                        // Determine attachment point: depth or depth+stencil
                        let attachment =
                            if tex.descriptor().format == usd_hgi::HgiFormat::Float32UInt8 {
                                gl::DEPTH_STENCIL_ATTACHMENT
                            } else {
                                gl::DEPTH_ATTACHMENT
                            };
                        if layer > 0 {
                            gl::NamedFramebufferTextureLayer(fbo, attachment, tex_id, mip, layer);
                        } else {
                            gl::NamedFramebufferTexture(fbo, attachment, tex_id, mip);
                        }
                    }
                }
            }

            // Setup draw buffers
            let n = self.desc.color_textures.len();
            if n > 0 {
                let bufs: Vec<GLenum> = (0..n as u32).map(|i| gl::COLOR_ATTACHMENT0 + i).collect();
                gl::NamedFramebufferDrawBuffers(fbo, n as i32, bufs.as_ptr());
            }

            // Apply clear (LoadOp)
            for (i, color_desc) in self.desc.color_attachment_descs.iter().enumerate() {
                if color_desc.load_op == HgiAttachmentLoadOp::Clear {
                    let cv = color_desc.clear_value;
                    let cv_arr = [cv[0], cv[1], cv[2], cv[3]];
                    gl::ClearBufferfv(gl::COLOR, i as i32, cv_arr.as_ptr());
                }
            }

            if self.desc.depth_texture.is_valid() {
                let da = &self.desc.depth_attachment_desc;
                if da.load_op == HgiAttachmentLoadOp::Clear {
                    gl::ClearBufferfv(gl::DEPTH, 0, std::ptr::addr_of!(da.clear_value[0]));
                }
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    #[allow(dead_code)]
    fn bind_framebuffer(&mut self) {}

    /// Execute all recorded commands
    #[cfg(feature = "opengl")]
    pub fn execute(&mut self) {
        if self.submitted {
            return;
        }

        // Bind FBO from descriptor (P1-8). Matches C++ BindFramebufferOp.
        self.bind_framebuffer();

        // Track current GL primitive type across bind/draw commands
        let mut gl_prim_type = gl::TRIANGLES;

        for cmd in &self.commands {
            match cmd {
                GraphicsCommand::BindPipeline(pipeline) => {
                    self.execute_bind_pipeline(pipeline);
                    // Update primitive type from newly bound pipeline
                    if let Some(p) = pipeline.get() {
                        gl_prim_type = hgi_primitive_type_to_gl(p.descriptor().primitive_type);
                    }
                }
                GraphicsCommand::BindResources(resources) => {
                    self.execute_bind_resources(resources);
                }
                GraphicsCommand::SetConstantValues(pipeline, bind_index, data) => {
                    self.execute_set_constant_values(pipeline, *bind_index, data);
                }
                GraphicsCommand::BindVertexBuffers(buffers, offsets) => {
                    self.execute_bind_vertex_buffers(buffers, offsets);
                }
                GraphicsCommand::SetViewport(viewport) => {
                    self.execute_set_viewport(viewport);
                }
                GraphicsCommand::SetScissor(scissor) => {
                    self.execute_set_scissor(scissor);
                }
                GraphicsCommand::SetBlendConstantColor(color) => {
                    self.execute_set_blend_color(color);
                }
                GraphicsCommand::SetStencilRef(ref_val) => {
                    self.execute_set_stencil_ref(*ref_val);
                }
                GraphicsCommand::Draw(op) => {
                    Self::execute_draw_with_prim(gl_prim_type, op);
                }
                GraphicsCommand::DrawIndexed(index_buffer, op) => {
                    Self::execute_draw_indexed_with_prim(gl_prim_type, index_buffer, op);
                }
                GraphicsCommand::DrawIndirect(op) => {
                    Self::execute_draw_indirect_with_prim(gl_prim_type, op);
                }
                GraphicsCommand::DrawIndexedIndirect(index_buffer, op) => {
                    Self::execute_draw_indexed_indirect_with_prim(gl_prim_type, index_buffer, op);
                }
                GraphicsCommand::PushDebugGroup(label) => {
                    self.execute_push_debug_group(label);
                }
                GraphicsCommand::PopDebugGroup => {
                    self.execute_pop_debug_group();
                }
                GraphicsCommand::InsertDebugMarker(label) => {
                    self.execute_debug_marker(label);
                }
                GraphicsCommand::MemoryBarrier(barrier) => {
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
    fn execute_bind_pipeline(&self, pipeline: &HgiGraphicsPipelineHandle) {
        if let Some(p) = pipeline.get() {
            // Cast to HgiGLGraphicsPipeline
            let vao = p.raw_resource() as u32;
            if vao != 0 {
                unsafe {
                    gl::BindVertexArray(vao);
                }
            }

            // Also bind the shader program
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

    /// Execute vertex buffer binding
    #[cfg(feature = "opengl")]
    fn execute_bind_vertex_buffers(&self, buffers: &[HgiBufferHandle], offsets: &[u64]) {
        use gl::types::*;

        for (index, buffer) in buffers.iter().enumerate() {
            if let Some(buf) = buffer.get() {
                let buf_id = buf.raw_resource() as GLuint;
                let offset = offsets.get(index).copied().unwrap_or(0) as GLintptr;
                let stride = buf.descriptor().byte_size as GLsizei; // Full buffer as stride for now

                if buf_id != 0 {
                    unsafe {
                        gl::BindVertexBuffer(index as GLuint, buf_id, offset, stride);
                    }
                }
            }
        }
    }

    /// Execute viewport setting
    #[cfg(feature = "opengl")]
    fn execute_set_viewport(&self, viewport: &HgiViewport) {
        unsafe {
            gl::Viewport(
                viewport.x as i32,
                viewport.y as i32,
                viewport.width as i32,
                viewport.height as i32,
            );
            gl::DepthRange(viewport.min_depth as f64, viewport.max_depth as f64);
        }
    }

    /// Execute scissor setting
    #[cfg(feature = "opengl")]
    fn execute_set_scissor(&self, scissor: &HgiScissor) {
        unsafe {
            gl::Scissor(
                scissor.x,
                scissor.y,
                scissor.width as i32,
                scissor.height as i32,
            );
        }
    }

    /// Execute blend color setting
    #[cfg(feature = "opengl")]
    fn execute_set_blend_color(&self, color: &Vec4f) {
        unsafe {
            gl::BlendColor(color.x, color.y, color.z, color.w);
        }
    }

    /// Execute stencil reference setting.
    /// Only updates the reference value, preserving existing compare func and mask.
    #[cfg(feature = "opengl")]
    fn execute_set_stencil_ref(&self, ref_val: u32) {
        unsafe {
            // Query current front stencil func and mask to preserve them
            let mut front_func: i32 = gl::ALWAYS as i32;
            let mut front_mask: i32 = 0xFF;
            gl::GetIntegerv(gl::STENCIL_FUNC, &mut front_func);
            gl::GetIntegerv(gl::STENCIL_VALUE_MASK, &mut front_mask);
            gl::StencilFuncSeparate(
                gl::FRONT,
                front_func as u32,
                ref_val as i32,
                front_mask as u32,
            );

            // Query current back stencil func and mask to preserve them
            let mut back_func: i32 = gl::ALWAYS as i32;
            let mut back_mask: i32 = 0xFF;
            gl::GetIntegerv(gl::STENCIL_BACK_FUNC, &mut back_func);
            gl::GetIntegerv(gl::STENCIL_BACK_VALUE_MASK, &mut back_mask);
            gl::StencilFuncSeparate(gl::BACK, back_func as u32, ref_val as i32, back_mask as u32);
        }
    }

    /// Execute draw command with given GL primitive type
    #[cfg(feature = "opengl")]
    fn execute_draw_with_prim(gl_prim: u32, op: &HgiDrawOp) {
        unsafe {
            gl::DrawArraysInstancedBaseInstance(
                gl_prim,
                op.base_vertex as i32,
                op.vertex_count as i32,
                op.instance_count as i32,
                op.base_instance,
            );
        }
    }

    /// Execute indexed draw command with given GL primitive type
    #[cfg(feature = "opengl")]
    fn execute_draw_indexed_with_prim(
        gl_prim: u32,
        index_buffer: &HgiBufferHandle,
        op: &HgiDrawIndexedOp,
    ) {
        if let Some(buf) = index_buffer.get() {
            let buf_id = buf.raw_resource() as u32;
            if buf_id != 0 {
                unsafe {
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, buf_id);

                    // Determine index type based on buffer usage
                    let index_type = if buf.descriptor().usage.contains(HgiBufferUsage::INDEX16) {
                        gl::UNSIGNED_SHORT
                    } else {
                        gl::UNSIGNED_INT
                    };

                    let offset = (op.base_index as usize
                        * if index_type == gl::UNSIGNED_SHORT {
                            2
                        } else {
                            4
                        }) as *const std::ffi::c_void;

                    gl::DrawElementsInstancedBaseVertexBaseInstance(
                        gl_prim,
                        op.index_count as i32,
                        index_type,
                        offset,
                        op.instance_count as i32,
                        op.base_vertex,
                        op.base_instance,
                    );
                }
            }
        }
    }

    /// Execute indirect draw command with given GL primitive type
    #[cfg(feature = "opengl")]
    fn execute_draw_indirect_with_prim(gl_prim: u32, op: &HgiDrawIndirectOp) {
        if let Some(buf) = op.draw_buffer.get() {
            let buf_id = buf.raw_resource() as u32;
            if buf_id != 0 {
                unsafe {
                    gl::BindBuffer(gl::DRAW_INDIRECT_BUFFER, buf_id);
                    gl::MultiDrawArraysIndirect(
                        gl_prim,
                        op.draw_buffer_byte_offset as *const std::ffi::c_void,
                        op.draw_count as i32,
                        op.stride as i32,
                    );
                }
            }
        }
    }

    /// Execute indexed indirect draw command with given GL primitive type
    #[cfg(feature = "opengl")]
    fn execute_draw_indexed_indirect_with_prim(
        gl_prim: u32,
        index_buffer: &HgiBufferHandle,
        op: &HgiDrawIndirectOp,
    ) {
        let index_buf = index_buffer.get();
        let draw_buf = op.draw_buffer.get();

        if let (Some(idx_buf), Some(drw_buf)) = (index_buf, draw_buf) {
            let idx_id = idx_buf.raw_resource() as u32;
            let drw_id = drw_buf.raw_resource() as u32;

            if idx_id != 0 && drw_id != 0 {
                unsafe {
                    gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, idx_id);
                    gl::BindBuffer(gl::DRAW_INDIRECT_BUFFER, drw_id);

                    let index_type = if idx_buf.descriptor().usage.contains(HgiBufferUsage::INDEX16)
                    {
                        gl::UNSIGNED_SHORT
                    } else {
                        gl::UNSIGNED_INT
                    };

                    gl::MultiDrawElementsIndirect(
                        gl_prim,
                        index_type,
                        op.draw_buffer_byte_offset as *const std::ffi::c_void,
                        op.draw_count as i32,
                        op.stride as i32,
                    );
                }
            }
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
        let gl_barrier = if barrier.is_empty() || barrier == HgiMemoryBarrier::NONE {
            0u32
        } else {
            gl::ALL_BARRIER_BITS
        };
        if gl_barrier != 0 {
            unsafe {
                gl::MemoryBarrier(gl_barrier);
            }
        }
    }
}

impl HgiGLGraphicsCmds {
    /// Set constant values (push constants) via glProgramUniform4fv (P1-2 fix).
    ///
    /// Matches C++ `HgiGLOps::SetConstantValues()` (ops.cpp) which uses
    /// glProgramUniform4fv to set data at `bind_index` uniform location.
    #[allow(dead_code)]
    #[cfg(feature = "opengl")]
    fn execute_set_constant_values(
        &self,
        pipeline: &HgiGraphicsPipelineHandle,
        bind_index: u32,
        data: &[u8],
    ) {
        use gl::types::*;

        if let Some(p) = pipeline.get() {
            if let Some(program) = p.descriptor().shader_program.get() {
                let program_id = program.raw_resource() as GLuint;
                if program_id == 0 {
                    return;
                }
                // Transmit data as vec4 array via glProgramUniform4fv.
                // Each vec4 = 4 floats = 16 bytes.
                let float_count = data.len() / 4;
                let vec4_count = (float_count / 4).max(1);
                let float_data: &[f32] = unsafe {
                    #[allow(clippy::cast_ptr_alignment)]
                    std::slice::from_raw_parts(
                        data.as_ptr() as *const f32,
                        (data.len() / 4).min(vec4_count * 4),
                    )
                };
                unsafe {
                    gl::ProgramUniform4fv(
                        program_id,
                        bind_index as GLint,
                        vec4_count as GLsizei,
                        float_data.as_ptr(),
                    );
                }
            }
        }
    }

    #[allow(dead_code)]
    #[cfg(not(feature = "opengl"))]
    fn execute_set_constant_values(
        &self,
        _pipeline: &HgiGraphicsPipelineHandle,
        _bind_index: u32,
        _data: &[u8],
    ) {
        // stub when opengl feature disabled
    }
}

impl Default for HgiGLGraphicsCmds {
    fn default() -> Self {
        Self::new()
    }
}

impl HgiCmds for HgiGLGraphicsCmds {
    fn is_submitted(&self) -> bool {
        self.submitted
    }

    fn execute_submit(&mut self) {
        self.execute();
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
}

impl HgiGraphicsCmds for HgiGLGraphicsCmds {
    fn bind_pipeline(&mut self, pipeline: &HgiGraphicsPipelineHandle) {
        self.current_pipeline = Some(pipeline.clone());
        self.commands
            .push(GraphicsCommand::BindPipeline(pipeline.clone()));
    }

    fn bind_resources(&mut self, resources: &HgiResourceBindingsHandle) {
        self.commands
            .push(GraphicsCommand::BindResources(resources.clone()));
    }

    fn set_constant_values(
        &mut self,
        pipeline: &HgiGraphicsPipelineHandle,
        _stages: HgiShaderStage,
        bind_index: u32,
        data: &[u8],
    ) {
        self.commands.push(GraphicsCommand::SetConstantValues(
            pipeline.clone(),
            bind_index,
            data.to_vec(),
        ));
    }

    fn bind_vertex_buffers(&mut self, buffers: &[HgiBufferHandle], offsets: &[u64]) {
        self.commands.push(GraphicsCommand::BindVertexBuffers(
            buffers.to_vec(),
            offsets.to_vec(),
        ));
    }

    fn set_viewport(&mut self, viewport: &HgiViewport) {
        self.commands.push(GraphicsCommand::SetViewport(*viewport));
    }

    fn set_scissor(&mut self, scissor: &HgiScissor) {
        self.commands.push(GraphicsCommand::SetScissor(*scissor));
    }

    fn set_blend_constant_color(&mut self, color: &Vec4f) {
        self.commands
            .push(GraphicsCommand::SetBlendConstantColor(*color));
    }

    fn set_stencil_reference_value(&mut self, ref_val: u32) {
        self.commands.push(GraphicsCommand::SetStencilRef(ref_val));
    }

    fn draw(&mut self, op: &HgiDrawOp) {
        self.commands.push(GraphicsCommand::Draw(*op));
    }

    fn draw_indexed(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndexedOp) {
        self.commands
            .push(GraphicsCommand::DrawIndexed(index_buffer.clone(), *op));
    }

    fn draw_indirect(&mut self, op: &HgiDrawIndirectOp) {
        self.commands
            .push(GraphicsCommand::DrawIndirect(op.clone()));
    }

    fn draw_indexed_indirect(&mut self, index_buffer: &HgiBufferHandle, op: &HgiDrawIndirectOp) {
        self.commands.push(GraphicsCommand::DrawIndexedIndirect(
            index_buffer.clone(),
            op.clone(),
        ));
    }

    fn memory_barrier(&mut self, barrier: HgiMemoryBarrier) {
        self.commands.push(GraphicsCommand::MemoryBarrier(barrier));
    }
}

impl Drop for HgiGLGraphicsCmds {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        if self.fbo != 0 {
            unsafe {
                gl::DeleteFramebuffers(1, &self.fbo);
            }
            self.fbo = 0;
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphics_cmds_creation() {
        let cmds = HgiGLGraphicsCmds::new();
        assert!(!cmds.is_submitted());
        assert_eq!(cmds.commands.len(), 0);
    }

    #[test]
    fn test_record_commands() {
        let mut cmds = HgiGLGraphicsCmds::new();

        let viewport = HgiViewport::new(0.0, 0.0, 800.0, 600.0);
        cmds.set_viewport(&viewport);

        let draw_op = HgiDrawOp {
            vertex_count: 3,
            ..Default::default()
        };
        cmds.draw(&draw_op);

        assert_eq!(cmds.commands.len(), 2);
        assert!(!cmds.is_submitted());
    }
}
