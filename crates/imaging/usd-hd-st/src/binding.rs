//! Binding request structures for Storm shader resource binding.
//!
//! Ported from C++ binding.h. Defines binding request types that describe
//! how shader resources (buffers, textures) are connected to shader stages.

use usd_hgi::HgiFormat;
use usd_tf::Token;

/// Type of shader resource binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingType {
    /// Uniform buffer object (small, frequently updated)
    Ubo,
    /// Shader storage buffer object (large, read-write)
    Ssbo,
    /// Combined image sampler (texture + sampler)
    Texture,
    /// Push constant / root constant (fastest, very small)
    PushConstant,
    /// Vertex attribute (per-vertex input)
    VertexAttr,
    /// Instance attribute (per-instance input)
    InstanceAttr,
}

/// A request to bind a named resource to a shader slot.
///
/// Describes what kind of data is expected at a particular binding point.
/// The resource_binder uses these to build the actual bind group layout.
#[derive(Debug, Clone)]
pub struct BindingRequest {
    /// Primvar / uniform / texture name
    pub name: Token,
    /// Data format of the binding
    pub data_type: HgiFormat,
    /// What kind of binding this is
    pub binding_type: BindingType,
    /// Bind group index (wgpu @group(N))
    pub group: u32,
    /// Binding index within the group (wgpu @binding(N))
    pub binding: u32,
    /// Shader location for vertex attributes (@location(N))
    pub location: u32,
    /// Whether this binding is writable (for SSBO / storage textures)
    pub writable: bool,
}

impl BindingRequest {
    /// Create a vertex attribute binding.
    pub fn vertex_attr(name: &str, fmt: HgiFormat, location: u32) -> Self {
        Self {
            name: Token::new(name),
            data_type: fmt,
            binding_type: BindingType::VertexAttr,
            group: 0,
            binding: 0,
            location,
            writable: false,
        }
    }

    /// Create a UBO binding.
    pub fn ubo(name: &str, group: u32, binding: u32) -> Self {
        Self {
            name: Token::new(name),
            data_type: HgiFormat::Invalid,
            binding_type: BindingType::Ubo,
            group,
            binding,
            location: 0,
            writable: false,
        }
    }

    /// Create a texture binding.
    pub fn texture(name: &str, group: u32, binding: u32) -> Self {
        Self {
            name: Token::new(name),
            data_type: HgiFormat::Invalid,
            binding_type: BindingType::Texture,
            group,
            binding,
            location: 0,
            writable: false,
        }
    }

    /// Create a push constant binding.
    pub fn push_constant(name: &str, fmt: HgiFormat) -> Self {
        Self {
            name: Token::new(name),
            data_type: fmt,
            binding_type: BindingType::PushConstant,
            group: 0,
            binding: 0,
            location: 0,
            writable: false,
        }
    }

    /// Create an SSBO binding.
    pub fn ssbo(name: &str, group: u32, binding: u32, writable: bool) -> Self {
        Self {
            name: Token::new(name),
            data_type: HgiFormat::Invalid,
            binding_type: BindingType::Ssbo,
            group,
            binding,
            location: 0,
            writable,
        }
    }
}

/// Well-known binding slots for the Storm WGSL pipeline.
///
/// Bind group layout:
///   Group 0: Scene uniforms  (camera VP, model, ambient)
///   Group 1: Light uniforms  (multi-light array)
///   Group 2: Material params (UsdPreviewSurface values + texture flags)
///   Group 3: Textures        (7 tex+sampler pairs, bindings 0-13)
///
/// set_constant_values(bind_index=N) maps to @group(N) @binding(0).
pub mod slots {
    // Group 0: Scene uniforms
    pub const SCENE_GROUP: u32 = 0;
    pub const SCENE_UNIFORMS_BINDING: u32 = 0;

    // Group 1: Light + shadow uniforms
    pub const LIGHT_GROUP: u32 = 1;
    pub const LIGHT_UNIFORMS_BINDING: u32 = 0;

    // Pick/deep-resolve storage buffer.
    // Reuses group 1 / binding 0 only for the FlatColor pick shader variant,
    // where light uniforms are not emitted at all.
    pub const PICK_BUFFER_GROUP: u32 = LIGHT_GROUP;
    pub const PICK_BUFFER_BINDING: u32 = 0;

    // Shadow bindings (in LIGHT_GROUP, bindings 1-3)
    // Matches C++ simpleLightingShader shadowCompareTextures array binding.
    pub const SHADOW_UNIFORMS_BINDING: u32 = 1;
    pub const SHADOW_ATLAS_BINDING: u32 = 2;
    pub const SHADOW_SAMPLER_BINDING: u32 = 3;

    // Group 2: Material params
    pub const MATERIAL_GROUP: u32 = 2;
    pub const MATERIAL_PARAMS_BINDING: u32 = 0;

    // Group 3: Per-material textures (7 texture+sampler pairs = 14 bindings)
    pub const TEXTURE_GROUP: u32 = 3;

    // Diffuse / base color -- slots 0,1
    pub const DIFFUSE_TEX_BINDING: u32 = 0;
    pub const DIFFUSE_SAMPLER_BINDING: u32 = 1;

    // Normal map -- slots 2,3
    pub const NORMAL_TEX_BINDING: u32 = 2;
    pub const NORMAL_SAMPLER_BINDING: u32 = 3;

    // Roughness -- slots 4,5
    pub const ROUGHNESS_TEX_BINDING: u32 = 4;
    pub const ROUGHNESS_SAMPLER_BINDING: u32 = 5;

    // Metallic -- slots 6,7
    pub const METALLIC_TEX_BINDING: u32 = 6;
    pub const METALLIC_SAMPLER_BINDING: u32 = 7;

    // Opacity -- slots 8,9
    pub const OPACITY_TEX_BINDING: u32 = 8;
    pub const OPACITY_SAMPLER_BINDING: u32 = 9;

    // Emissive -- slots 10,11
    pub const EMISSIVE_TEX_BINDING: u32 = 10;
    pub const EMISSIVE_SAMPLER_BINDING: u32 = 11;

    // Occlusion -- slots 12,13
    pub const OCCLUSION_TEX_BINDING: u32 = 12;
    pub const OCCLUSION_SAMPLER_BINDING: u32 = 13;

    // Displacement (reserved) -- slots 14,15
    pub const DISPLACEMENT_TEX_BINDING: u32 = 14;
    pub const DISPLACEMENT_SAMPLER_BINDING: u32 = 15;

    /// Total number of texture slots in group 3 (7 used + 1 reserved = 8 pairs = 16 bindings).
    pub const TEXTURE_SLOT_COUNT: u32 = 8;

    // Group 4: IBL (Image-Based Lighting) textures (when textures also present).
    // When has_uv=false, IBL occupies group 3 to avoid a gap in the pipeline layout
    // (wgpu requires contiguous bind groups).
    pub const IBL_GROUP: u32 = 4;

    /// Dynamic IBL group index: group 3 without UV, group 4 with UV.
    /// Avoids bind-group gaps that wgpu rejects.
    pub fn ibl_group(has_uv: bool) -> u32 {
        if has_uv { IBL_GROUP } else { TEXTURE_GROUP }
    }

    /// Dynamic shadow atlas group index: placed after the last used group
    /// (IBL or textures or material) to keep bind groups contiguous.
    ///
    /// Layout: group = 3 + has_uv + has_ibl.
    pub fn shadow_group(has_uv: bool, has_ibl: bool) -> u32 {
        3 + (has_uv as u32) + (has_ibl as u32)
    }

    /// Instance transforms SSBO binding within its group.
    pub const INSTANCE_XFORMS_BINDING: u32 = 0;

    /// Dynamic instance transforms group: placed after shadow (or after IBL/textures).
    /// Layout: group = 3 + has_uv + has_ibl + has_shadows.
    pub fn instance_group(has_uv: bool, has_ibl: bool, has_shadows: bool) -> u32 {
        3 + (has_uv as u32) + (has_ibl as u32) + (has_shadows as u32)
    }

    /// Face-varying storage buffer binding within its group.
    pub const FACE_VARYING_BINDING: u32 = 0;

    /// Dynamic face-varying group: placed after instancing when both are present.
    /// This keeps bind groups contiguous for wgpu while preserving a stable
    /// ordering: textures/IBL/shadows -> instancing -> face-varying storage.
    pub fn face_varying_group(
        has_uv: bool,
        has_ibl: bool,
        has_shadows: bool,
        use_instancing: bool,
    ) -> u32 {
        3 + (has_uv as u32) + (has_ibl as u32) + (has_shadows as u32) + (use_instancing as u32)
    }

    // Irradiance cubemap (diffuse IBL) -- texture_cube = texture_2d_array 6-layer
    pub const IBL_IRRADIANCE_TEX_BINDING: u32 = 0;
    pub const IBL_IRRADIANCE_SAMPLER_BINDING: u32 = 1;

    // Prefiltered specular cubemap (specular IBL with mips)
    pub const IBL_PREFILTER_TEX_BINDING: u32 = 2;
    pub const IBL_PREFILTER_SAMPLER_BINDING: u32 = 3;

    // BRDF LUT (split-sum, 2D)
    pub const IBL_BRDF_LUT_TEX_BINDING: u32 = 4;
    pub const IBL_BRDF_LUT_SAMPLER_BINDING: u32 = 5;

    // Vertex attribute locations
    pub const POSITION_LOCATION: u32 = 0;
    pub const NORMAL_LOCATION: u32 = 1;
    pub const UV_LOCATION: u32 = 2;
    pub const COLOR_LOCATION: u32 = 3;

    // Dedicated non-mesh attribute locations.
    //
    // Points/basisCurves do not share mesh packing order, so they need their
    // own explicit locations instead of inheriting the mesh convention.
    pub const POINT_WIDTH_LOCATION: u32 = 1;
    pub const POINT_COLOR_LOCATION: u32 = 2;
    pub const CURVE_WIDTH_LOCATION: u32 = 1;
    pub const CURVE_NORMAL_LOCATION: u32 = 2;
    pub const CURVE_COLOR_LOCATION: u32 = 3;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_attr_binding() {
        let b = BindingRequest::vertex_attr("position", HgiFormat::Float32Vec3, 0);
        assert_eq!(b.binding_type, BindingType::VertexAttr);
        assert_eq!(b.location, 0);
        assert_eq!(b.data_type, HgiFormat::Float32Vec3);
    }

    #[test]
    fn test_ubo_binding() {
        let b = BindingRequest::ubo("sceneUniforms", 0, 0);
        assert_eq!(b.binding_type, BindingType::Ubo);
        assert_eq!(b.group, 0);
        assert_eq!(b.binding, 0);
    }

    #[test]
    fn test_texture_slots_non_overlapping() {
        use slots::*;
        // Each tex and its sampler must be adjacent and not overlap with other slots
        assert_eq!(DIFFUSE_TEX_BINDING + 1, DIFFUSE_SAMPLER_BINDING);
        assert_eq!(NORMAL_TEX_BINDING + 1, NORMAL_SAMPLER_BINDING);
        assert_eq!(ROUGHNESS_TEX_BINDING + 1, ROUGHNESS_SAMPLER_BINDING);
        assert_eq!(METALLIC_TEX_BINDING + 1, METALLIC_SAMPLER_BINDING);
        assert_eq!(OPACITY_TEX_BINDING + 1, OPACITY_SAMPLER_BINDING);
        assert_eq!(EMISSIVE_TEX_BINDING + 1, EMISSIVE_SAMPLER_BINDING);
        assert_eq!(OCCLUSION_TEX_BINDING + 1, OCCLUSION_SAMPLER_BINDING);
        assert_eq!(DISPLACEMENT_TEX_BINDING + 1, DISPLACEMENT_SAMPLER_BINDING);
        // Groups 0-15 are fully covered
        assert_eq!(DISPLACEMENT_SAMPLER_BINDING, 15);
    }
}
