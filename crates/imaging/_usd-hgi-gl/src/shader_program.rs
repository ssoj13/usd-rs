//! OpenGL shader program implementation

use usd_hgi::*;

/// OpenGL shader program
///
/// Wraps an OpenGL program object linking multiple shader stages
#[derive(Debug)]
pub struct HgiGLShaderProgram {
    /// OpenGL program object name
    gl_id: u32,

    /// Program descriptor
    desc: HgiShaderProgramDesc,

    /// Whether linking succeeded
    is_valid: bool,

    /// Link errors/warnings
    link_log: String,
}

impl HgiGLShaderProgram {
    /// Create a new OpenGL shader program
    pub fn new(desc: &HgiShaderProgramDesc) -> Self {
        let (gl_id, is_valid, link_log) = Self::link_program(desc);

        Self {
            gl_id,
            desc: desc.clone(),
            is_valid,
            link_log,
        }
    }

    /// Link shader program from shader functions
    #[cfg(feature = "opengl")]
    fn link_program(desc: &HgiShaderProgramDesc) -> (u32, bool, String) {
        use gl::types::*;
        use std::ptr;

        unsafe {
            let program_id = gl::CreateProgram();
            if program_id == 0 {
                return (0, false, "Failed to create program object".to_string());
            }

            // Attach all shader functions
            for shader_handle in &desc.shader_functions {
                if let Some(shader) = shader_handle.get() {
                    let shader_id = shader.raw_resource() as GLuint;
                    if shader_id != 0 {
                        gl::AttachShader(program_id, shader_id);
                    }
                }
            }

            // Link program
            gl::LinkProgram(program_id);

            // Check link status
            let mut success: GLint = 0;
            gl::GetProgramiv(program_id, gl::LINK_STATUS, &mut success);

            // Get info log
            let mut log_length: GLint = 0;
            gl::GetProgramiv(program_id, gl::INFO_LOG_LENGTH, &mut log_length);

            let link_log = if log_length > 0 {
                let mut buffer: Vec<u8> = vec![0; log_length as usize];
                gl::GetProgramInfoLog(
                    program_id,
                    log_length,
                    ptr::null_mut(),
                    buffer.as_mut_ptr() as *mut GLchar,
                );
                String::from_utf8_lossy(&buffer)
                    .trim_end_matches('\0')
                    .to_string()
            } else {
                String::new()
            };

            let is_valid = success != 0;

            if !is_valid {
                log::error!("Program link failed: {}", link_log);
            }

            // Detach shaders after linking (they can be deleted independently)
            for shader_handle in &desc.shader_functions {
                if let Some(shader) = shader_handle.get() {
                    let shader_id = shader.raw_resource() as GLuint;
                    if shader_id != 0 {
                        gl::DetachShader(program_id, shader_id);
                    }
                }
            }

            // Set debug label if provided
            if !desc.debug_name.is_empty() && is_valid {
                gl::ObjectLabel(
                    gl::PROGRAM,
                    program_id,
                    desc.debug_name.len() as GLsizei,
                    desc.debug_name.as_ptr() as *const GLchar,
                );
            }

            (program_id, is_valid, link_log)
        }
    }

    /// Link shader program from shader functions (stub when opengl feature disabled)
    #[cfg(not(feature = "opengl"))]
    fn link_program(_desc: &HgiShaderProgramDesc) -> (u32, bool, String) {
        (0, true, String::new())
    }

    /// Get the OpenGL program object name
    pub fn gl_id(&self) -> u32 {
        self.gl_id
    }

    /// Get the program descriptor
    pub fn descriptor(&self) -> &HgiShaderProgramDesc {
        &self.desc
    }

    /// Check if program linking succeeded
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Get program link log
    pub fn link_log(&self) -> &str {
        &self.link_log
    }
}

impl HgiShaderProgram for HgiGLShaderProgram {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiShaderProgramDesc {
        &self.desc
    }

    fn is_valid(&self) -> bool {
        self.is_valid
    }

    fn link_errors(&self) -> &str {
        &self.link_log
    }

    fn byte_size_of_resource(&self) -> usize {
        // Note: Returns 0 - would need GL_PROGRAM_BINARY_LENGTH query for actual size
        0
    }

    fn raw_resource(&self) -> u64 {
        self.gl_id as u64
    }
}

impl Drop for HgiGLShaderProgram {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        if self.gl_id != 0 {
            unsafe {
                gl::DeleteProgram(self.gl_id);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// Bind (use) a shader program
#[cfg(feature = "opengl")]
pub fn use_program(program: &HgiGLShaderProgram) {
    if program.gl_id() != 0 {
        unsafe {
            gl::UseProgram(program.gl_id());
        }
    }
}

/// Bind (use) a shader program (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn use_program(_program: &HgiGLShaderProgram) {}

/// Unbind current shader program
#[cfg(feature = "opengl")]
pub fn unbind_program() {
    unsafe {
        gl::UseProgram(0);
    }
}

/// Unbind current shader program (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn unbind_program() {}

/// Get uniform location by name
#[cfg(feature = "opengl")]
pub fn get_uniform_location(program: &HgiGLShaderProgram, name: &str) -> i32 {
    use std::ffi::CString;

    if program.gl_id() == 0 {
        return -1;
    }

    let c_name = match CString::new(name) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    unsafe { gl::GetUniformLocation(program.gl_id(), c_name.as_ptr()) }
}

/// Get uniform location by name (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn get_uniform_location(_program: &HgiGLShaderProgram, _name: &str) -> i32 {
    -1
}

/// Get attribute location by name
#[cfg(feature = "opengl")]
pub fn get_attrib_location(program: &HgiGLShaderProgram, name: &str) -> i32 {
    use std::ffi::CString;

    if program.gl_id() == 0 {
        return -1;
    }

    let c_name = match CString::new(name) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    unsafe { gl::GetAttribLocation(program.gl_id(), c_name.as_ptr()) }
}

/// Get attribute location by name (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn get_attrib_location(_program: &HgiGLShaderProgram, _name: &str) -> i32 {
    -1
}

/// Get uniform block index by name
#[cfg(feature = "opengl")]
pub fn get_uniform_block_index(program: &HgiGLShaderProgram, name: &str) -> u32 {
    use std::ffi::CString;

    if program.gl_id() == 0 {
        return 0xFFFFFFFF; // GL_INVALID_INDEX
    }

    let c_name = match CString::new(name) {
        Ok(s) => s,
        Err(_) => return 0xFFFFFFFF,
    };

    unsafe { gl::GetUniformBlockIndex(program.gl_id(), c_name.as_ptr()) }
}

/// Get uniform block index by name (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn get_uniform_block_index(_program: &HgiGLShaderProgram, _name: &str) -> u32 {
    0xFFFFFFFF // GL_INVALID_INDEX
}

/// Bind uniform block to a binding point
#[cfg(feature = "opengl")]
pub fn uniform_block_binding(program: &HgiGLShaderProgram, block_index: u32, binding_point: u32) {
    if program.gl_id() != 0 && block_index != 0xFFFFFFFF {
        unsafe {
            gl::UniformBlockBinding(program.gl_id(), block_index, binding_point);
        }
    }
}

/// Bind uniform block to a binding point (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn uniform_block_binding(
    _program: &HgiGLShaderProgram,
    _block_index: u32,
    _binding_point: u32,
) {
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiShaderProgramDesc::new().with_debug_name("TestProgram".to_string());

    let program = HgiGLShaderProgram::new(&desc);
    assert_eq!(program.descriptor().debug_name, "TestProgram");

    let desc = HgiShaderProgramDesc::new();
    let program = HgiGLShaderProgram::new(&desc);

    // These are stubs, just verify they don't crash
    use_program(&program);
    unbind_program();
    assert_eq!(get_uniform_location(&program, "testUniform"), -1);
    assert_eq!(get_attrib_location(&program, "testAttrib"), -1);
    assert_eq!(get_uniform_block_index(&program, "testBlock"), 0xFFFFFFFF);
}
