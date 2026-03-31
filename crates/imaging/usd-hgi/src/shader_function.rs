//! Shader function resources, descriptors, and sub-descriptors
//!
//! Mirrors C++ HgiShaderFunctionDesc and its nested descriptor types for
//! textures, buffers, params, param blocks, compute, tessellation, geometry,
//! and fragment stage metadata.

use super::enums::{
    HgiBindingType, HgiInterpolationType, HgiSamplingType, HgiShaderStage, HgiShaderTextureType,
    HgiStorageType,
};
use super::handle::HgiHandle;
use super::types::HgiFormat;

// ---------------------------------------------------------------------------
// HgiShaderFunctionTextureDesc
// ---------------------------------------------------------------------------

/// Describes a texture to be passed into a shader
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionTextureDesc {
    /// The name written from the codegen into shader file for the texture
    pub name_in_shader: String,
    /// 1d, 2d or 3d texture declaration
    pub dimensions: u32,
    /// Format of the texture (required where sampler types depend on texture, e.g. GL)
    pub format: HgiFormat,
    /// Type of the texture (regular, shadow, array, cubemap)
    pub texture_type: HgiShaderTextureType,
    /// The index of the resource
    pub bind_index: u32,
    /// If > 0, indicates the size of the array
    pub array_size: usize,
    /// Whether the texture is writable
    pub writable: bool,
}

impl Default for HgiShaderFunctionTextureDesc {
    fn default() -> Self {
        Self {
            name_in_shader: String::new(),
            dimensions: 2,
            format: HgiFormat::Float32Vec4,
            texture_type: HgiShaderTextureType::Texture,
            bind_index: 0,
            array_size: 0,
            writable: false,
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionBufferDesc
// ---------------------------------------------------------------------------

/// Describes a buffer to be passed into a shader
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionBufferDesc {
    /// The name written from the codegen into shader file for the buffer
    pub name_in_shader: String,
    /// Type of the param within the shader file
    pub type_name: String,
    /// The index of the resource
    pub bind_index: u32,
    /// The size of the array when binding is HgiBindingTypeArray
    pub array_size: u32,
    /// The binding model to use to expose the buffer to the shader
    pub binding: HgiBindingType,
    /// Whether the resource is writable
    pub writable: bool,
}

impl Default for HgiShaderFunctionBufferDesc {
    fn default() -> Self {
        Self {
            name_in_shader: String::new(),
            type_name: String::new(),
            bind_index: 0,
            array_size: 0,
            binding: HgiBindingType::Value,
            writable: false,
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionParamDesc
// ---------------------------------------------------------------------------

/// Describes a param passed into a shader or between shader stages
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionParamDesc {
    /// The name written from the codegen into the shader file for the param
    pub name_in_shader: String,
    /// Type of the param within the shader file
    pub type_name: String,
    /// Layout location (GL) or role generator (Metal). -1 = unset
    pub location: i32,
    /// Index for interstage parameters. -1 = unset
    pub interstage_slot: i32,
    /// Interpolation qualifier: Default, Flat, or NoPerspective
    pub interpolation: HgiInterpolationType,
    /// Sampling qualifier: Default, Centroid, or Sample
    pub sampling: HgiSamplingType,
    /// Storage qualifier: Default or Patch
    pub storage: HgiStorageType,
    /// Optional role (e.g. "position", "uv", "color")
    pub role: String,
    /// If specified, generates an array type parameter with given size
    pub array_size: String,
}

impl Default for HgiShaderFunctionParamDesc {
    fn default() -> Self {
        Self {
            name_in_shader: String::new(),
            type_name: String::new(),
            location: -1,
            interstage_slot: -1,
            interpolation: HgiInterpolationType::Default,
            sampling: HgiSamplingType::Default,
            storage: HgiStorageType::Default,
            role: String::new(),
            array_size: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionParamBlockDesc
// ---------------------------------------------------------------------------

/// A member of a param block
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionParamBlockMember {
    /// Member name
    pub name: String,
    /// Member type
    pub type_name: String,
    /// Interpolation qualifier
    pub interpolation: HgiInterpolationType,
    /// Sampling qualifier
    pub sampling: HgiSamplingType,
}

impl Default for HgiShaderFunctionParamBlockMember {
    fn default() -> Self {
        Self {
            name: String::new(),
            type_name: String::new(),
            interpolation: HgiInterpolationType::Default,
            sampling: HgiSamplingType::Default,
        }
    }
}

/// Describes an interstage param block between shader stages
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionParamBlockDesc {
    /// The name used to match blocks between shader stages
    pub block_name: String,
    /// The name used to scope access to block members
    pub instance_name: String,
    /// The members of the block
    pub members: Vec<HgiShaderFunctionParamBlockMember>,
    /// If specified, generates a block with given size
    pub array_size: String,
    /// The interstage slot index of the first member (sequential for rest)
    pub interstage_slot: i32,
}

impl Default for HgiShaderFunctionParamBlockDesc {
    fn default() -> Self {
        Self {
            block_name: String::new(),
            instance_name: String::new(),
            members: Vec::new(),
            array_size: String::new(),
            interstage_slot: -1,
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionComputeDesc
// ---------------------------------------------------------------------------

/// Describes a compute function's local thread group size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiShaderFunctionComputeDesc {
    /// 3D size of the local thread grouping.
    /// When x > 0, y and z must also be > 0.
    /// Default [0,0,0] means not set.
    pub local_size: [i32; 3],
}

impl Default for HgiShaderFunctionComputeDesc {
    fn default() -> Self {
        Self {
            local_size: [0, 0, 0],
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionTessellationDesc
// ---------------------------------------------------------------------------

/// Patch type for tessellation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TessellationPatchType {
    Triangles,
    Quads,
    Isolines,
}

/// Spacing for tessellation primitive generator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TessellationSpacing {
    Equal,
    FractionalEven,
    FractionalOdd,
}

/// Ordering for tessellation primitive generator
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TessellationOrdering {
    CW,
    CCW,
}

/// Describes a tessellation function's properties
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionTessellationDesc {
    /// The type of patch
    pub patch_type: TessellationPatchType,
    /// The spacing used by the tessellation primitive generator
    pub spacing: TessellationSpacing,
    /// The ordering used by the tessellation primitive generator
    pub ordering: TessellationOrdering,
    /// The number of vertices in per patch
    pub num_verts_per_patch_in: String,
    /// The number of vertices out per patch
    pub num_verts_per_patch_out: String,
}

impl Default for HgiShaderFunctionTessellationDesc {
    fn default() -> Self {
        Self {
            patch_type: TessellationPatchType::Triangles,
            spacing: TessellationSpacing::Equal,
            ordering: TessellationOrdering::CCW,
            num_verts_per_patch_in: String::new(),
            num_verts_per_patch_out: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionGeometryDesc
// ---------------------------------------------------------------------------

/// Input primitive type for geometry shaders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometryInPrimitiveType {
    Points,
    Lines,
    LinesAdjacency,
    Triangles,
    TrianglesAdjacency,
}

/// Output primitive type for geometry shaders
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometryOutPrimitiveType {
    Points,
    LineStrip,
    TriangleStrip,
}

/// Describes a geometry function's properties
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionGeometryDesc {
    /// The input primitive type
    pub in_primitive_type: GeometryInPrimitiveType,
    /// The output primitive type
    pub out_primitive_type: GeometryOutPrimitiveType,
    /// The maximum number of vertices written by a single invocation
    pub out_max_vertices: String,
}

impl Default for HgiShaderFunctionGeometryDesc {
    fn default() -> Self {
        Self {
            in_primitive_type: GeometryInPrimitiveType::Triangles,
            out_primitive_type: GeometryOutPrimitiveType::TriangleStrip,
            out_max_vertices: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionFragmentDesc
// ---------------------------------------------------------------------------

/// Describes a fragment function's properties
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HgiShaderFunctionFragmentDesc {
    /// Fragment shader tests performed before fragment shader execution
    pub early_fragment_tests: bool,
}

impl Default for HgiShaderFunctionFragmentDesc {
    fn default() -> Self {
        Self {
            early_fragment_tests: false,
        }
    }
}

// ---------------------------------------------------------------------------
// HgiShaderFunctionDesc
// ---------------------------------------------------------------------------

/// Describes the properties needed to create a GPU shader function.
///
/// Contains all metadata for shader reflection: textures, buffers,
/// stage inputs/outputs, param blocks, and stage-specific descriptors.
#[derive(Debug, Clone, PartialEq)]
pub struct HgiShaderFunctionDesc {
    /// Debug label for GPU debugging
    pub debug_name: String,

    /// Shader stage this function is for
    pub shader_stage: HgiShaderStage,

    /// Optional shader code declarations (defines/types emitted before resource bindings)
    pub shader_code_declarations: String,

    /// The shader source code (GLSL, HLSL, MSL, or SPIRV)
    pub shader_code: String,

    /// Entry point function name (e.g., "main", "VSMain")
    pub entry_point: String,

    /// Additional shader compilation flags/macros
    pub constants: Vec<(String, String)>,

    /// Textures passed into the shader
    pub textures: Vec<HgiShaderFunctionTextureDesc>,

    /// Buffers passed into the shader
    pub buffers: Vec<HgiShaderFunctionBufferDesc>,

    /// Constant params passed into the shader
    pub constant_params: Vec<HgiShaderFunctionParamDesc>,

    /// Params declared at global scope
    pub stage_global_members: Vec<HgiShaderFunctionParamDesc>,

    /// Stage inputs
    pub stage_inputs: Vec<HgiShaderFunctionParamDesc>,

    /// Stage outputs
    pub stage_outputs: Vec<HgiShaderFunctionParamDesc>,

    /// Input blocks
    pub stage_input_blocks: Vec<HgiShaderFunctionParamBlockDesc>,

    /// Output blocks
    pub stage_output_blocks: Vec<HgiShaderFunctionParamBlockDesc>,

    /// Compute shader descriptor (local workgroup size)
    pub compute_descriptor: HgiShaderFunctionComputeDesc,

    /// Tessellation shader descriptor
    pub tessellation_descriptor: HgiShaderFunctionTessellationDesc,

    /// Geometry shader descriptor
    pub geometry_descriptor: HgiShaderFunctionGeometryDesc,

    /// Fragment shader descriptor
    pub fragment_descriptor: HgiShaderFunctionFragmentDesc,
}

impl Default for HgiShaderFunctionDesc {
    fn default() -> Self {
        Self {
            debug_name: String::new(),
            shader_stage: HgiShaderStage::VERTEX,
            shader_code_declarations: String::new(),
            shader_code: String::new(),
            entry_point: String::from("main"),
            constants: Vec::new(),
            textures: Vec::new(),
            buffers: Vec::new(),
            constant_params: Vec::new(),
            stage_global_members: Vec::new(),
            stage_inputs: Vec::new(),
            stage_outputs: Vec::new(),
            stage_input_blocks: Vec::new(),
            stage_output_blocks: Vec::new(),
            compute_descriptor: HgiShaderFunctionComputeDesc::default(),
            tessellation_descriptor: HgiShaderFunctionTessellationDesc::default(),
            geometry_descriptor: HgiShaderFunctionGeometryDesc::default(),
            fragment_descriptor: HgiShaderFunctionFragmentDesc::default(),
        }
    }
}

impl HgiShaderFunctionDesc {
    /// Create a new shader function descriptor
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the debug name
    pub fn with_debug_name(mut self, name: impl Into<String>) -> Self {
        self.debug_name = name.into();
        self
    }

    /// Set the shader stage
    pub fn with_shader_stage(mut self, stage: HgiShaderStage) -> Self {
        self.shader_stage = stage;
        self
    }

    /// Set the shader code declarations (emitted before resource bindings)
    pub fn with_shader_code_declarations(mut self, decl: impl Into<String>) -> Self {
        self.shader_code_declarations = decl.into();
        self
    }

    /// Set the shader code
    pub fn with_shader_code(mut self, code: impl Into<String>) -> Self {
        self.shader_code = code.into();
        self
    }

    /// Set the entry point
    pub fn with_entry_point(mut self, entry: impl Into<String>) -> Self {
        self.entry_point = entry.into();
        self
    }

    /// Add a shader constant/macro
    pub fn with_constant(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.constants.push((name.into(), value.into()));
        self
    }

    /// Set all constants at once
    pub fn with_constants(mut self, constants: Vec<(String, String)>) -> Self {
        self.constants = constants;
        self
    }

    /// Check if this is a valid descriptor
    pub fn is_valid(&self) -> bool {
        !self.shader_code.is_empty() && !self.entry_point.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Helper functions matching C++ free functions
// ---------------------------------------------------------------------------

/// Add a texture descriptor to a shader function descriptor
pub fn add_texture(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    bind_index: u32,
    dimensions: u32,
    format: HgiFormat,
    texture_type: HgiShaderTextureType,
) {
    desc.textures.push(HgiShaderFunctionTextureDesc {
        name_in_shader: name.into(),
        dimensions,
        format,
        texture_type,
        bind_index,
        array_size: 0,
        writable: false,
    });
}

/// Add an array of textures descriptor to a shader function descriptor
pub fn add_array_of_textures(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    array_size: usize,
    bind_index: u32,
    dimensions: u32,
    format: HgiFormat,
    texture_type: HgiShaderTextureType,
) {
    desc.textures.push(HgiShaderFunctionTextureDesc {
        name_in_shader: name.into(),
        dimensions,
        format,
        texture_type,
        bind_index,
        array_size,
        writable: false,
    });
}

/// Add a writable texture descriptor to a shader function descriptor
pub fn add_writable_texture(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    bind_index: u32,
    dimensions: u32,
    format: HgiFormat,
    texture_type: HgiShaderTextureType,
) {
    desc.textures.push(HgiShaderFunctionTextureDesc {
        name_in_shader: name.into(),
        dimensions,
        format,
        texture_type,
        bind_index,
        array_size: 0,
        writable: true,
    });
}

/// Add a buffer descriptor to a shader function descriptor
pub fn add_buffer(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    bind_index: u32,
    binding: HgiBindingType,
    array_size: u32,
) {
    desc.buffers.push(HgiShaderFunctionBufferDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        bind_index,
        array_size,
        binding,
        writable: false,
    });
}

/// Add a writable buffer descriptor to a shader function descriptor
pub fn add_writable_buffer(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    bind_index: u32,
) {
    desc.buffers.push(HgiShaderFunctionBufferDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        bind_index,
        array_size: 0,
        binding: HgiBindingType::Value,
        writable: true,
    });
}

/// Add a constant param to a shader function descriptor
pub fn add_constant_param(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    role: impl Into<String>,
) {
    desc.constant_params.push(HgiShaderFunctionParamDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        role: role.into(),
        ..Default::default()
    });
}

/// Add a stage input to a shader function descriptor (auto-increments location)
pub fn add_stage_input(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    role: impl Into<String>,
) {
    let location = desc.stage_inputs.len() as i32;
    desc.stage_inputs.push(HgiShaderFunctionParamDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        location,
        role: role.into(),
        ..Default::default()
    });
}

/// Add a stage input from a full param descriptor
pub fn add_stage_input_desc(desc: &mut HgiShaderFunctionDesc, param: HgiShaderFunctionParamDesc) {
    desc.stage_inputs.push(param);
}

/// Add a global variable to a shader function descriptor
pub fn add_global_variable(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    array_size: impl Into<String>,
) {
    desc.stage_global_members.push(HgiShaderFunctionParamDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        array_size: array_size.into(),
        ..Default::default()
    });
}

/// Add a stage output to a shader function descriptor
pub fn add_stage_output(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    role: impl Into<String>,
    array_size: impl Into<String>,
) {
    desc.stage_outputs.push(HgiShaderFunctionParamDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        role: role.into(),
        array_size: array_size.into(),
        ..Default::default()
    });
}

/// Add a stage output with explicit location
pub fn add_stage_output_at(
    desc: &mut HgiShaderFunctionDesc,
    name: impl Into<String>,
    type_name: impl Into<String>,
    location: u32,
) {
    desc.stage_outputs.push(HgiShaderFunctionParamDesc {
        name_in_shader: name.into(),
        type_name: type_name.into(),
        location: location as i32,
        ..Default::default()
    });
}

/// Add a stage output from a full param descriptor
pub fn add_stage_output_desc(desc: &mut HgiShaderFunctionDesc, param: HgiShaderFunctionParamDesc) {
    desc.stage_outputs.push(param);
}

// ---------------------------------------------------------------------------
// HgiShaderFunction trait
// ---------------------------------------------------------------------------

/// GPU shader function resource (abstract interface)
///
/// Represents a graphics platform independent GPU shader function.
/// Shader functions should be created via Hgi::create_shader_function().
pub trait HgiShaderFunction: Send + Sync {
    /// Downcast to concrete type (for backend-specific operations)
    fn as_any(&self) -> &dyn std::any::Any;

    /// Get the descriptor that was used to create this shader function
    fn descriptor(&self) -> &HgiShaderFunctionDesc;

    /// Returns whether the shader compiled successfully
    fn is_valid(&self) -> bool;

    /// Returns the shader compilation errors (if any)
    fn compile_errors(&self) -> &str;

    /// Returns the byte size of the compiled shader
    fn byte_size_of_resource(&self) -> usize;

    /// Returns the backend's raw GPU resource handle
    fn raw_resource(&self) -> u64;
}

/// Type alias for shader function handle
pub type HgiShaderFunctionHandle = HgiHandle<dyn HgiShaderFunction>;

/// Vector of shader function handles
pub type HgiShaderFunctionHandleVector = Vec<HgiShaderFunctionHandle>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_function_desc_default() {
        let desc = HgiShaderFunctionDesc::default();
        assert_eq!(desc.entry_point, "main");
        assert!(desc.constants.is_empty());
        assert!(desc.textures.is_empty());
        assert!(desc.buffers.is_empty());
        assert!(desc.stage_inputs.is_empty());
        assert!(desc.stage_outputs.is_empty());
        assert!(!desc.is_valid());
    }

    #[test]
    fn test_shader_function_desc_builder() {
        let desc = HgiShaderFunctionDesc::new()
            .with_debug_name("VertexShader")
            .with_shader_stage(HgiShaderStage::VERTEX)
            .with_shader_code("#version 450\nvoid main() {}")
            .with_entry_point("main")
            .with_constant("USE_LIGHTING", "1");

        assert_eq!(desc.debug_name, "VertexShader");
        assert_eq!(desc.shader_stage, HgiShaderStage::VERTEX);
        assert!(desc.is_valid());
    }

    #[test]
    fn test_texture_desc() {
        let mut desc = HgiShaderFunctionDesc::new()
            .with_shader_code("code")
            .with_shader_stage(HgiShaderStage::FRAGMENT);

        add_texture(
            &mut desc,
            "diffuseTexture",
            0,
            2,
            HgiFormat::Float32Vec4,
            HgiShaderTextureType::Texture,
        );
        add_writable_texture(
            &mut desc,
            "outputImage",
            1,
            2,
            HgiFormat::Float32Vec4,
            HgiShaderTextureType::Texture,
        );

        assert_eq!(desc.textures.len(), 2);
        assert!(!desc.textures[0].writable);
        assert!(desc.textures[1].writable);
    }

    #[test]
    fn test_buffer_desc() {
        let mut desc = HgiShaderFunctionDesc::new().with_shader_code("code");

        add_buffer(
            &mut desc,
            "myBuffer",
            "float",
            0,
            HgiBindingType::Pointer,
            0,
        );
        add_writable_buffer(&mut desc, "outBuffer", "int", 1);

        assert_eq!(desc.buffers.len(), 2);
        assert!(!desc.buffers[0].writable);
        assert!(desc.buffers[1].writable);
    }

    #[test]
    fn test_param_desc() {
        let mut desc = HgiShaderFunctionDesc::new().with_shader_code("code");

        add_stage_input(&mut desc, "position", "vec3", "position");
        add_stage_input(&mut desc, "normal", "vec3", "");
        add_stage_output(&mut desc, "outColor", "vec4", "color", "");

        assert_eq!(desc.stage_inputs.len(), 2);
        assert_eq!(desc.stage_inputs[0].location, 0);
        assert_eq!(desc.stage_inputs[1].location, 1);
        assert_eq!(desc.stage_outputs.len(), 1);
    }

    #[test]
    fn test_param_block_desc() {
        let block = HgiShaderFunctionParamBlockDesc {
            block_name: "InterstageData".to_string(),
            instance_name: "outData".to_string(),
            members: vec![
                HgiShaderFunctionParamBlockMember {
                    name: "position".to_string(),
                    type_name: "vec4".to_string(),
                    ..Default::default()
                },
                HgiShaderFunctionParamBlockMember {
                    name: "normal".to_string(),
                    type_name: "vec3".to_string(),
                    interpolation: HgiInterpolationType::Flat,
                    ..Default::default()
                },
            ],
            interstage_slot: 0,
            ..Default::default()
        };

        assert_eq!(block.members.len(), 2);
        assert_eq!(block.members[1].interpolation, HgiInterpolationType::Flat);
    }

    #[test]
    fn test_compute_desc() {
        let cd = HgiShaderFunctionComputeDesc {
            local_size: [16, 16, 1],
        };
        assert_eq!(cd.local_size[0], 16);
    }

    #[test]
    fn test_tessellation_desc() {
        let td = HgiShaderFunctionTessellationDesc {
            patch_type: TessellationPatchType::Quads,
            spacing: TessellationSpacing::FractionalEven,
            ordering: TessellationOrdering::CW,
            num_verts_per_patch_in: "4".to_string(),
            num_verts_per_patch_out: "4".to_string(),
        };
        assert_eq!(td.patch_type, TessellationPatchType::Quads);
    }

    #[test]
    fn test_geometry_desc() {
        let gd = HgiShaderFunctionGeometryDesc {
            in_primitive_type: GeometryInPrimitiveType::Triangles,
            out_primitive_type: GeometryOutPrimitiveType::TriangleStrip,
            out_max_vertices: "3".to_string(),
        };
        assert_eq!(gd.in_primitive_type, GeometryInPrimitiveType::Triangles);
    }

    #[test]
    fn test_fragment_desc() {
        let fd = HgiShaderFunctionFragmentDesc {
            early_fragment_tests: true,
        };
        assert!(fd.early_fragment_tests);
    }

    // Mock implementation for testing
    struct MockShaderFunction {
        desc: HgiShaderFunctionDesc,
        valid: bool,
        errors: String,
    }

    impl HgiShaderFunction for MockShaderFunction {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn descriptor(&self) -> &HgiShaderFunctionDesc {
            &self.desc
        }

        fn is_valid(&self) -> bool {
            self.valid
        }

        fn compile_errors(&self) -> &str {
            &self.errors
        }

        fn byte_size_of_resource(&self) -> usize {
            self.desc.shader_code.len()
        }

        fn raw_resource(&self) -> u64 {
            0
        }
    }

    #[test]
    fn test_shader_function_trait() {
        let desc = HgiShaderFunctionDesc::new().with_shader_code("shader code");

        let shader = MockShaderFunction {
            desc: desc.clone(),
            valid: true,
            errors: String::new(),
        };

        assert!(shader.is_valid());
        assert_eq!(shader.compile_errors(), "");
        assert_eq!(shader.byte_size_of_resource(), "shader code".len());
    }
}
