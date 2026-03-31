//! OpenGL shader function implementation

#[cfg(feature = "opengl")]
use super::conversions::GLenum;
use usd_hgi::*;

/// OpenGL shader object
///
/// Wraps an OpenGL shader object (vertex, fragment, compute, etc.)
#[derive(Debug)]
pub struct HgiGLShaderFunction {
    /// OpenGL shader object name
    gl_id: u32,

    /// Shader descriptor
    desc: HgiShaderFunctionDesc,

    /// Whether compilation succeeded
    is_valid: bool,

    /// Compilation errors/warnings
    compile_log: String,
}

impl HgiGLShaderFunction {
    /// Create a new OpenGL shader function
    pub fn new(desc: &HgiShaderFunctionDesc) -> Self {
        let (gl_id, is_valid, compile_log) = Self::compile_shader(desc);

        Self {
            gl_id,
            desc: desc.clone(),
            is_valid,
            compile_log,
        }
    }

    /// Compile a shader from source
    #[cfg(feature = "opengl")]
    fn compile_shader(desc: &HgiShaderFunctionDesc) -> (u32, bool, String) {
        use gl::types::*;
        use std::ffi::CString;
        use std::ptr;

        let shader_type = shader_stage_to_gl_type(desc.shader_stage);

        unsafe {
            let shader_id = gl::CreateShader(shader_type);
            if shader_id == 0 {
                return (0, false, "Failed to create shader object".to_string());
            }

            // Set shader source
            let source = CString::new(desc.shader_code.as_bytes()).unwrap_or_default();
            let source_ptr = source.as_ptr();
            gl::ShaderSource(shader_id, 1, &source_ptr, ptr::null());

            // Compile shader
            gl::CompileShader(shader_id);

            // Check compilation status
            let mut success: GLint = 0;
            gl::GetShaderiv(shader_id, gl::COMPILE_STATUS, &mut success);

            // Get info log
            let mut log_length: GLint = 0;
            gl::GetShaderiv(shader_id, gl::INFO_LOG_LENGTH, &mut log_length);

            let compile_log = if log_length > 0 {
                let mut buffer: Vec<u8> = vec![0; log_length as usize];
                gl::GetShaderInfoLog(
                    shader_id,
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
                log::error!("Shader compilation failed: {}", compile_log);
            }

            // Set debug label if provided
            if !desc.debug_name.is_empty() && is_valid {
                gl::ObjectLabel(
                    gl::SHADER,
                    shader_id,
                    desc.debug_name.len() as GLsizei,
                    desc.debug_name.as_ptr() as *const GLchar,
                );
            }

            (shader_id, is_valid, compile_log)
        }
    }

    /// Compile a shader from source (stub when opengl feature disabled)
    #[cfg(not(feature = "opengl"))]
    fn compile_shader(_desc: &HgiShaderFunctionDesc) -> (u32, bool, String) {
        (0, true, String::new())
    }

    /// Get the OpenGL shader object name
    pub fn gl_id(&self) -> u32 {
        self.gl_id
    }

    /// Get the shader descriptor
    pub fn descriptor(&self) -> &HgiShaderFunctionDesc {
        &self.desc
    }

    /// Check if shader compilation succeeded
    pub fn is_valid(&self) -> bool {
        self.is_valid
    }

    /// Get shader compilation log
    pub fn compile_log(&self) -> &str {
        &self.compile_log
    }

    /// Get shader stage
    pub fn shader_stage(&self) -> HgiShaderStage {
        self.desc.shader_stage
    }
}

impl HgiShaderFunction for HgiGLShaderFunction {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn descriptor(&self) -> &HgiShaderFunctionDesc {
        &self.desc
    }

    fn is_valid(&self) -> bool {
        self.is_valid
    }

    fn compile_errors(&self) -> &str {
        &self.compile_log
    }

    fn byte_size_of_resource(&self) -> usize {
        // Note: Returns 0 - shader source size tracking not implemented
        0
    }

    fn raw_resource(&self) -> u64 {
        self.gl_id as u64
    }
}

impl Drop for HgiGLShaderFunction {
    #[cfg(feature = "opengl")]
    fn drop(&mut self) {
        if self.gl_id != 0 {
            unsafe {
                gl::DeleteShader(self.gl_id);
            }
        }
    }

    #[cfg(not(feature = "opengl"))]
    fn drop(&mut self) {}
}

/// Convert HgiShaderStage to OpenGL shader type
#[cfg(feature = "opengl")]
pub fn shader_stage_to_gl_type(stage: HgiShaderStage) -> GLenum {
    if stage.contains(HgiShaderStage::VERTEX) {
        gl::VERTEX_SHADER
    } else if stage.contains(HgiShaderStage::FRAGMENT) {
        gl::FRAGMENT_SHADER
    } else if stage.contains(HgiShaderStage::COMPUTE) {
        gl::COMPUTE_SHADER
    } else if stage.contains(HgiShaderStage::GEOMETRY) {
        gl::GEOMETRY_SHADER
    } else if stage.contains(HgiShaderStage::TESSELLATION_CONTROL) {
        gl::TESS_CONTROL_SHADER
    } else if stage.contains(HgiShaderStage::TESSELLATION_EVAL) {
        gl::TESS_EVALUATION_SHADER
    } else {
        gl::VERTEX_SHADER // Default to vertex shader
    }
}

/// Convert HgiShaderStage to OpenGL shader type (stub when opengl feature disabled)
#[cfg(not(feature = "opengl"))]
pub fn shader_stage_to_gl_type(stage: HgiShaderStage) -> u32 {
    if stage.contains(HgiShaderStage::VERTEX) {
        0x8B31 // GL_VERTEX_SHADER
    } else if stage.contains(HgiShaderStage::FRAGMENT) {
        0x8B30 // GL_FRAGMENT_SHADER
    } else if stage.contains(HgiShaderStage::COMPUTE) {
        0x91B9 // GL_COMPUTE_SHADER
    } else if stage.contains(HgiShaderStage::GEOMETRY) {
        0x8DD9 // GL_GEOMETRY_SHADER
    } else if stage.contains(HgiShaderStage::TESSELLATION_CONTROL) {
        0x8E88 // GL_TESS_CONTROL_SHADER
    } else if stage.contains(HgiShaderStage::TESSELLATION_EVAL) {
        0x8E87 // GL_TESS_EVALUATION_SHADER
    } else {
        0x8B31 // Default to vertex shader
    }
}

#[cfg(all(test, feature = "opengl"))]
pub(crate) fn run_gl_tests() {
    use super::*;

    let desc = HgiShaderFunctionDesc::new()
        .with_shader_stage(HgiShaderStage::VERTEX)
        .with_shader_code("#version 450\nvoid main() {}".to_string())
        .with_debug_name("TestVertexShader".to_string());

    let shader = HgiGLShaderFunction::new(&desc);
    assert_eq!(shader.shader_stage(), HgiShaderStage::VERTEX);
    assert_eq!(shader.descriptor().debug_name, "TestVertexShader");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_stage_conversion() {
        assert_eq!(shader_stage_to_gl_type(HgiShaderStage::VERTEX), 0x8B31);
        assert_eq!(shader_stage_to_gl_type(HgiShaderStage::FRAGMENT), 0x8B30);
        assert_eq!(shader_stage_to_gl_type(HgiShaderStage::COMPUTE), 0x91B9);
    }
}
