//! Uniform buffer binding management.
//!
//! Port of pxr/imaging/glf/bindingMap.h

use super::TfToken;
use std::collections::HashMap;

/// Returns true if the given GL type enum is a sampler type.
/// Matches the C++ switch in `_AddActiveUniformBindings`.
#[cfg(feature = "opengl")]
fn is_sampler_type(gl_type: u32) -> bool {
    matches!(
        gl_type,
        gl::SAMPLER_1D
            | gl::SAMPLER_2D
            | gl::SAMPLER_3D
            | gl::SAMPLER_CUBE
            | gl::SAMPLER_1D_SHADOW
            | gl::SAMPLER_2D_SHADOW
            | gl::SAMPLER_1D_ARRAY
            | gl::SAMPLER_2D_ARRAY
            | gl::SAMPLER_1D_ARRAY_SHADOW
            | gl::SAMPLER_2D_ARRAY_SHADOW
            | gl::SAMPLER_2D_MULTISAMPLE
            | gl::SAMPLER_2D_MULTISAMPLE_ARRAY
            | gl::SAMPLER_CUBE_SHADOW
            | gl::SAMPLER_BUFFER
            | gl::SAMPLER_2D_RECT
            | gl::SAMPLER_2D_RECT_SHADOW
            | gl::INT_SAMPLER_1D
            | gl::INT_SAMPLER_2D
            | gl::INT_SAMPLER_3D
            | gl::INT_SAMPLER_CUBE
            | gl::INT_SAMPLER_1D_ARRAY
            | gl::INT_SAMPLER_2D_ARRAY
            | gl::INT_SAMPLER_2D_MULTISAMPLE
            | gl::INT_SAMPLER_2D_MULTISAMPLE_ARRAY
            | gl::INT_SAMPLER_BUFFER
            | gl::INT_SAMPLER_2D_RECT
            | gl::UNSIGNED_INT_SAMPLER_1D
            | gl::UNSIGNED_INT_SAMPLER_2D
            | gl::UNSIGNED_INT_SAMPLER_3D
            | gl::UNSIGNED_INT_SAMPLER_CUBE
            | gl::UNSIGNED_INT_SAMPLER_1D_ARRAY
            | gl::UNSIGNED_INT_SAMPLER_2D_ARRAY
            | gl::UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE
            | gl::UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE_ARRAY
            | gl::UNSIGNED_INT_SAMPLER_BUFFER
            | gl::UNSIGNED_INT_SAMPLER_2D_RECT
    )
}

/// Manages uniform buffer and sampler bindings for GL programs.
///
/// Tracks active bindings for attributes, samplers, and uniform buffers,
/// allowing programs to query and assign binding points consistently.
#[derive(Debug, Clone)]
pub struct GlfBindingMap {
    /// Attribute location bindings
    attrib_bindings: HashMap<TfToken, i32>,
    /// Sampler unit bindings
    sampler_bindings: HashMap<TfToken, i32>,
    /// Uniform block bindings
    uniform_bindings: HashMap<TfToken, i32>,
    /// Base index for sampler units
    sampler_binding_base_index: i32,
    /// Base index for uniform blocks
    uniform_binding_base_index: i32,
}

impl GlfBindingMap {
    /// Creates a new empty binding map.
    pub fn new() -> Self {
        Self {
            attrib_bindings: HashMap::new(),
            sampler_bindings: HashMap::new(),
            uniform_bindings: HashMap::new(),
            sampler_binding_base_index: 0,
            uniform_binding_base_index: 0,
        }
    }

    /// Gets the sampler unit for the given name.
    ///
    /// If the sampler is not yet registered, assigns the next available unit.
    pub fn get_sampler_unit(&mut self, name: &TfToken) -> i32 {
        if let Some(&unit) = self.sampler_bindings.get(name) {
            unit
        } else {
            let unit = self.sampler_binding_base_index + self.sampler_bindings.len() as i32;
            self.sampler_bindings.insert(name.clone(), unit);
            unit
        }
    }

    /// Gets the attribute index for the given name.
    ///
    /// Returns -1 if the attribute is not registered.
    pub fn get_attribute_index(&self, name: &TfToken) -> i32 {
        self.attrib_bindings.get(name).copied().unwrap_or(-1)
    }

    /// Gets the uniform binding for the given name.
    ///
    /// If the uniform block is not yet registered, assigns the next available binding.
    pub fn get_uniform_binding(&mut self, name: &TfToken) -> i32 {
        if let Some(&binding) = self.uniform_bindings.get(name) {
            binding
        } else {
            let binding = self.uniform_binding_base_index + self.uniform_bindings.len() as i32;
            self.uniform_bindings.insert(name.clone(), binding);
            binding
        }
    }

    /// Checks if a uniform binding exists for the given name.
    pub fn has_uniform_binding(&self, name: &TfToken) -> bool {
        self.uniform_bindings.contains_key(name)
    }

    /// Returns the number of sampler bindings.
    pub fn get_num_sampler_bindings(&self) -> usize {
        self.sampler_bindings.len()
    }

    /// Clears all attribute bindings.
    pub fn clear_attrib_bindings(&mut self) {
        self.attrib_bindings.clear();
    }

    /// Resets sampler bindings and sets a new base index.
    ///
    /// Sampler units will be assigned sequentially starting from base_index.
    /// This allows other subsystems to claim sampler units before additional
    /// indices are assigned by this binding map.
    pub fn reset_sampler_bindings(&mut self, base_index: i32) {
        self.sampler_bindings.clear();
        self.sampler_binding_base_index = base_index;
    }

    /// Resets uniform bindings and sets a new base index.
    ///
    /// Uniform block bindings will be assigned sequentially starting from base_index.
    /// This allows other subsystems to claim uniform block bindings before additional
    /// indices are assigned by this binding map.
    pub fn reset_uniform_bindings(&mut self, base_index: i32) {
        self.uniform_bindings.clear();
        self.uniform_binding_base_index = base_index;
    }

    /// Adds an attribute binding at the specified location.
    pub fn add_attrib_binding(&mut self, name: TfToken, location: i32) {
        self.attrib_bindings.insert(name, location);
    }

    /// Returns a reference to all attribute bindings.
    pub fn get_attribute_bindings(&self) -> &HashMap<TfToken, i32> {
        &self.attrib_bindings
    }

    /// Returns a reference to all sampler bindings.
    pub fn get_sampler_bindings(&self) -> &HashMap<TfToken, i32> {
        &self.sampler_bindings
    }

    /// Returns a reference to all uniform bindings.
    pub fn get_uniform_bindings(&self) -> &HashMap<TfToken, i32> {
        &self.uniform_bindings
    }

    /// Assigns sampler units to a GL program.
    ///
    /// Connects sampler uniforms to texture units by calling glUniform1i
    /// for each sampler binding.
    #[cfg(feature = "opengl")]
    pub fn assign_sampler_units_to_program(&self, program: u32) {
        use std::ffi::CString;

        for (name, &unit) in &self.sampler_bindings {
            if let Ok(c_name) = CString::new(name.as_str()) {
                unsafe {
                    let loc = gl::GetUniformLocation(program, c_name.as_ptr());
                    if loc >= 0 {
                        gl::ProgramUniform1i(program, loc, unit);
                    }
                }
            }
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn assign_sampler_units_to_program(&self, _program: u32) {}

    /// Assigns uniform block bindings to a GL program.
    ///
    /// Connects uniform blocks to binding points by calling glUniformBlockBinding.
    #[cfg(feature = "opengl")]
    pub fn assign_uniform_bindings_to_program(&self, program: u32) {
        use std::ffi::CString;

        for (name, &binding) in &self.uniform_bindings {
            if let Ok(c_name) = CString::new(name.as_str()) {
                unsafe {
                    let idx = gl::GetUniformBlockIndex(program, c_name.as_ptr());
                    if idx != gl::INVALID_INDEX {
                        gl::UniformBlockBinding(program, idx, binding as u32);
                    }
                }
            }
        }
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn assign_uniform_bindings_to_program(&self, _program: u32) {}

    /// Adds custom bindings from a GL program.
    ///
    /// Mirrors C++ `AddCustomBindings()`: queries active attributes, uniforms
    /// (sampler types), and uniform blocks, then calls
    /// `assign_uniform_bindings_to_program` and `assign_sampler_units_to_program`.
    #[cfg(feature = "opengl")]
    pub fn add_custom_bindings(&mut self, program: u32) {
        self.add_active_attribute_bindings(program);
        self.add_active_uniform_bindings(program);
        self.add_active_uniform_block_bindings(program);

        self.assign_uniform_bindings_to_program(program);
        self.assign_sampler_units_to_program(program);
    }

    /// No-op when OpenGL feature is disabled.
    #[cfg(not(feature = "opengl"))]
    pub fn add_custom_bindings(&mut self, _program: u32) {}

    /// C++ `_AddActiveAttributeBindings`: queries active attribs and registers them.
    #[cfg(feature = "opengl")]
    fn add_active_attribute_bindings(&mut self, program: u32) {
        use gl::types::*;

        unsafe {
            let mut num_attribs: GLint = 0;
            gl::GetProgramiv(program, gl::ACTIVE_ATTRIBUTES, &mut num_attribs);
            if num_attribs == 0 {
                return;
            }

            let mut max_len: GLint = 0;
            gl::GetProgramiv(program, gl::ACTIVE_ATTRIBUTE_MAX_LENGTH, &mut max_len);
            max_len = max_len.max(100);
            let mut name_buf = vec![0u8; max_len as usize];

            for i in 0..num_attribs as GLuint {
                let mut length: GLsizei = 0;
                let mut size: GLint = 0;
                let mut type_: GLenum = 0;

                gl::GetActiveAttrib(
                    program,
                    i,
                    max_len,
                    &mut length,
                    &mut size,
                    &mut type_,
                    name_buf.as_mut_ptr() as *mut GLchar,
                );

                if length > 0 {
                    let name_str = String::from_utf8_lossy(&name_buf[..length as usize]);
                    let location =
                        gl::GetAttribLocation(program, name_buf.as_ptr() as *const GLchar);
                    let token = TfToken::new(&name_str);

                    if let Some(&existing) = self.attrib_bindings.get(&token) {
                        if existing != location {
                            log::error!("Inconsistent attribute binding detected.");
                        }
                    } else {
                        self.attrib_bindings.insert(token, location);
                    }
                }
            }
        }
    }

    /// C++ `_AddActiveUniformBindings`: queries active uniforms and registers sampler types.
    #[cfg(feature = "opengl")]
    fn add_active_uniform_bindings(&mut self, program: u32) {
        use gl::types::*;

        unsafe {
            let mut num_uniforms: GLint = 0;
            gl::GetProgramiv(program, gl::ACTIVE_UNIFORMS, &mut num_uniforms);
            if num_uniforms == 0 {
                return;
            }

            let mut max_len: GLint = 0;
            gl::GetProgramiv(program, gl::ACTIVE_UNIFORM_MAX_LENGTH, &mut max_len);
            let mut name_buf = vec![0u8; max_len as usize];

            for i in 0..num_uniforms as GLuint {
                let mut length: GLsizei = 0;
                let mut size: GLint = 0;
                let mut type_: GLenum = 0;

                gl::GetActiveUniform(
                    program,
                    i,
                    max_len,
                    &mut length,
                    &mut size,
                    &mut type_,
                    name_buf.as_mut_ptr() as *mut GLchar,
                );

                // Register sampler-type uniforms as sampler bindings
                if is_sampler_type(type_) && length > 0 {
                    let name_str = String::from_utf8_lossy(&name_buf[..length as usize]);
                    self.get_sampler_unit(&TfToken::new(&name_str));
                }
            }
        }
    }

    /// C++ `_AddActiveUniformBlockBindings`: queries active UBOs and registers them.
    #[cfg(feature = "opengl")]
    fn add_active_uniform_block_bindings(&mut self, program: u32) {
        use gl::types::*;

        unsafe {
            let mut num_blocks: GLint = 0;
            gl::GetProgramiv(program, gl::ACTIVE_UNIFORM_BLOCKS, &mut num_blocks);
            if num_blocks == 0 {
                return;
            }

            let mut max_len: GLint = 0;
            gl::GetProgramiv(
                program,
                gl::ACTIVE_UNIFORM_BLOCK_MAX_NAME_LENGTH,
                &mut max_len,
            );
            let mut name_buf = vec![0u8; max_len as usize];

            for i in 0..num_blocks as GLuint {
                let mut length: GLsizei = 0;
                gl::GetActiveUniformBlockName(
                    program,
                    i,
                    max_len,
                    &mut length,
                    name_buf.as_mut_ptr() as *mut GLchar,
                );
                if length > 0 {
                    let name_str = String::from_utf8_lossy(&name_buf[..length as usize]);
                    self.get_uniform_binding(&TfToken::new(&name_str));
                }
            }
        }
    }

    /// Prints debug information about current bindings.
    ///
    /// Mirrors C++ `Debug()`: sorts entries by token for deterministic output.
    pub fn debug(&self) {
        use std::collections::BTreeMap;

        // Sort for deterministic output (C++ uses std::map for this)
        let attribs: BTreeMap<_, _> = self.attrib_bindings.iter().collect();
        let samplers: BTreeMap<_, _> = self.sampler_bindings.iter().collect();
        let uniforms: BTreeMap<_, _> = self.uniform_bindings.iter().collect();

        println!("GlfBindingMap");
        println!(" Attribute bindings");
        for (name, loc) in &attribs {
            println!("  {} : {}", name, loc);
        }
        println!(" Sampler bindings");
        for (name, unit) in &samplers {
            println!("  {} : {}", name, unit);
        }
        println!(" Uniform bindings");
        for (name, binding) in &uniforms {
            println!("  {} : {}", name, binding);
        }
    }
}

impl Default for GlfBindingMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_binding() {
        let mut map = GlfBindingMap::new();
        let diffuse = TfToken::new("diffuse");
        let unit = map.get_sampler_unit(&diffuse);
        assert_eq!(unit, 0);
        assert_eq!(map.get_sampler_unit(&diffuse), 0); // Same unit on second call
    }

    #[test]
    fn test_attribute_binding() {
        let mut map = GlfBindingMap::new();
        let position = TfToken::new("position");
        map.add_attrib_binding(position.clone(), 0);
        assert_eq!(map.get_attribute_index(&position), 0);
        assert_eq!(map.get_attribute_index(&TfToken::new("unknown")), -1);
    }

    #[test]
    fn test_uniform_binding() {
        let mut map = GlfBindingMap::new();
        let transforms = TfToken::new("Transforms");
        let binding = map.get_uniform_binding(&transforms);
        assert_eq!(binding, 0);
        assert!(map.has_uniform_binding(&transforms));
    }

    #[test]
    fn test_reset_bindings() {
        let mut map = GlfBindingMap::new();
        map.get_sampler_unit(&TfToken::new("tex0"));
        map.get_sampler_unit(&TfToken::new("tex1"));
        assert_eq!(map.get_num_sampler_bindings(), 2);

        map.reset_sampler_bindings(10);
        assert_eq!(map.get_num_sampler_bindings(), 0);

        let unit = map.get_sampler_unit(&TfToken::new("tex0"));
        assert_eq!(unit, 10); // Should start from new base
    }
}
