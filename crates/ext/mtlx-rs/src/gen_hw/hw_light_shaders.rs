//! HwLightShaders — collection of bound light shader implementations for HW generators.
//! Based on MaterialX HwLightShaders.h (header-only in C++).
//!
//! Maps light type IDs (unsigned integers matching renderer light type enums)
//! to their ShaderNode implementations. Passed through GenContext as user data
//! under the key `user_data::USER_DATA_LIGHT_SHADERS`.

use std::collections::HashMap;

use crate::gen_shader::ShaderNode;

/// Manages bound light shader implementations for a HW shader generator session.
///
/// C++ equivalent: HwLightShaders (extends GenUserData).
/// Usage: create, bind light shaders to type IDs, pass to GenContext user data,
/// then the HwShaderGenerator reads them to emit the light loop.
#[derive(Debug, Default)]
pub struct HwLightShaders {
    /// Map from light type ID to ShaderNode holding the light implementation.
    /// C++: std::unordered_map<unsigned int, ShaderNodePtr> _shaders
    shaders: HashMap<u32, Box<ShaderNode>>,
}

impl HwLightShaders {
    /// Create a new empty HwLightShaders collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a light shader to a light type ID.
    ///
    /// `type_id` should be a unique renderer-side light type identifier (e.g., 1 = point, 2 = spot).
    /// The same ID must be used when setting light parameters on generated surface shaders.
    pub fn bind(&mut self, type_id: u32, shader: Box<ShaderNode>) {
        self.shaders.insert(type_id, shader);
    }

    /// Unbind the light shader for the given type ID (no-op if not bound).
    pub fn unbind(&mut self, type_id: u32) {
        self.shaders.remove(&type_id);
    }

    /// Remove all bound light shaders.
    pub fn clear(&mut self) {
        self.shaders.clear();
    }

    /// Return the ShaderNode bound to the given type ID, or None.
    pub fn get(&self, type_id: u32) -> Option<&ShaderNode> {
        self.shaders.get(&type_id).map(|b| b.as_ref())
    }

    /// Return the full map of bound type IDs to ShaderNodes.
    pub fn get_all(&self) -> &HashMap<u32, Box<ShaderNode>> {
        &self.shaders
    }

    /// Return true if any light shaders are bound.
    pub fn is_empty(&self) -> bool {
        self.shaders.is_empty()
    }

    /// Return the number of bound light shaders.
    pub fn len(&self) -> usize {
        self.shaders.len()
    }
}
