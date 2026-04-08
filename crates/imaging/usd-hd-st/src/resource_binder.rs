//! Resource binder for Storm WGSL pipeline.
//!
//! Ported from C++ resourceBinder.h. Maps buffer/texture resources to
//! @group/@binding slots and builds HgiResourceBindingsDesc.

use crate::binding::{BindingRequest, BindingType, slots};
use crate::draw_item::HdStDrawItem;
use crate::draw_program_key::{BasisCurvesProgramKey, DrawProgramKey, PointsProgramKey};
use crate::mesh_shader_key::MeshShaderKey;
use usd_hgi::{
    HgiBindResourceType, HgiBufferBindDesc, HgiBufferHandle, HgiFormat, HgiResourceBindingsDesc,
    HgiVertexAttributeDesc, HgiVertexBufferDesc,
};

/// Collects binding requests and produces HGI descriptors.
///
/// Manages the mapping between named primvar/uniform resources and
/// their @group(N) @binding(M) slots in the WGSL shader.
#[derive(Debug, Default, Clone)]
pub struct ResourceBinder {
    /// All active binding requests
    bindings: Vec<BindingRequest>,
}

impl ResourceBinder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a binding request.
    pub fn add(&mut self, req: BindingRequest) {
        self.bindings.push(req);
    }

    /// Build standard bindings from a mesh shader key.
    ///
    /// Populates vertex attributes and uniform bindings based on
    /// what primvars the mesh has.
    pub fn from_mesh_key(key: &MeshShaderKey) -> Self {
        Self::from_draw_item_and_mesh_key(key, None)
    }

    /// Build bindings from the shader key plus the head draw item.
    ///
    /// `_ref` resolves bindings from `HdStDrawItem`, not from a coarse shader key
    /// alone. The Rust path is still mid-port, but taking the draw item here keeps
    /// the API aligned with the eventual parity direction and lets pipeline caches
    /// account for retained draw-item metadata.
    pub fn from_draw_item_and_mesh_key(
        key: &MeshShaderKey,
        _draw_item: Option<&HdStDrawItem>,
    ) -> Self {
        let mut binder = Self::new();
        let uses_face_varying =
            key.has_fvar_normals || key.has_fvar_uv || key.has_fvar_color || key.has_fvar_opacity;

        // Vertex attributes
        binder.add(BindingRequest::vertex_attr(
            "position",
            HgiFormat::Float32Vec3,
            slots::POSITION_LOCATION,
        ));
        if key.has_normals && !key.has_fvar_normals {
            binder.add(BindingRequest::vertex_attr(
                "normals",
                HgiFormat::Float32Vec3,
                slots::NORMAL_LOCATION,
            ));
        }
        if key.has_uv && !key.has_fvar_uv {
            binder.add(BindingRequest::vertex_attr(
                "uvs",
                HgiFormat::Float32Vec2,
                slots::UV_LOCATION,
            ));
        }
        if key.has_color && !key.has_fvar_color {
            binder.add(BindingRequest::vertex_attr(
                "displayColor",
                HgiFormat::Float32Vec3,
                slots::COLOR_LOCATION,
            ));
        }

        // Scene uniforms (VP matrix, lights, camera)
        binder.add(BindingRequest::ubo(
            "sceneUniforms",
            slots::SCENE_GROUP,
            slots::SCENE_UNIFORMS_BINDING,
        ));

        // Material uniforms
        binder.add(BindingRequest::ubo(
            "materialParams",
            slots::MATERIAL_GROUP,
            slots::MATERIAL_PARAMS_BINDING,
        ));

        if uses_face_varying {
            binder.add(BindingRequest::ssbo(
                "faceVaryingData",
                slots::face_varying_group(
                    key.has_uv,
                    key.has_ibl,
                    key.use_shadows,
                    key.use_instancing,
                ),
                slots::FACE_VARYING_BINDING,
                false,
            ));
        }

        binder
    }

    /// Build bindings for point rprims.
    pub fn from_points_key(key: &PointsProgramKey) -> Self {
        let mut binder = Self::new();
        binder.add(BindingRequest::vertex_attr(
            "position",
            HgiFormat::Float32Vec3,
            slots::POSITION_LOCATION,
        ));
        if key.has_widths {
            binder.add(BindingRequest::vertex_attr(
                "widths",
                HgiFormat::Float32,
                slots::POINT_WIDTH_LOCATION,
            ));
        }
        if key.has_color {
            binder.add(BindingRequest::vertex_attr(
                "displayColor",
                HgiFormat::Float32Vec3,
                slots::POINT_COLOR_LOCATION,
            ));
        }
        binder.add(BindingRequest::ubo(
            "sceneUniforms",
            slots::SCENE_GROUP,
            slots::SCENE_UNIFORMS_BINDING,
        ));
        binder.add(BindingRequest::ubo(
            "materialParams",
            slots::MATERIAL_GROUP,
            slots::MATERIAL_PARAMS_BINDING,
        ));
        binder
    }

    /// Build bindings for basis-curves rprims.
    pub fn from_basis_curves_key(key: &BasisCurvesProgramKey) -> Self {
        let mut binder = Self::new();
        binder.add(BindingRequest::vertex_attr(
            "position",
            HgiFormat::Float32Vec3,
            slots::POSITION_LOCATION,
        ));
        if key.has_widths {
            binder.add(BindingRequest::vertex_attr(
                "widths",
                HgiFormat::Float32,
                slots::CURVE_WIDTH_LOCATION,
            ));
        }
        if key.has_normals {
            binder.add(BindingRequest::vertex_attr(
                "normals",
                HgiFormat::Float32Vec3,
                slots::CURVE_NORMAL_LOCATION,
            ));
        }
        if key.has_color {
            binder.add(BindingRequest::vertex_attr(
                "displayColor",
                HgiFormat::Float32Vec3,
                slots::CURVE_COLOR_LOCATION,
            ));
        }
        binder.add(BindingRequest::ubo(
            "sceneUniforms",
            slots::SCENE_GROUP,
            slots::SCENE_UNIFORMS_BINDING,
        ));
        binder.add(BindingRequest::ubo(
            "materialParams",
            slots::MATERIAL_GROUP,
            slots::MATERIAL_PARAMS_BINDING,
        ));
        binder
    }

    /// Build bindings from the active program family.
    pub fn from_program_key(
        program_key: &DrawProgramKey,
        sample_item: Option<&HdStDrawItem>,
    ) -> Self {
        match program_key {
            DrawProgramKey::Mesh(key) => Self::from_draw_item_and_mesh_key(key, sample_item),
            DrawProgramKey::Points(key) => Self::from_points_key(key),
            DrawProgramKey::BasisCurves(key) => Self::from_basis_curves_key(key),
        }
    }

    /// Compute a stable hash of the resolved binding requests.
    ///
    /// This is intended for pipeline/codegen cache keys so future draw-item-aware
    /// binding differences do not collapse onto the same cached pipeline.
    pub fn cache_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for req in &self.bindings {
            Hash::hash(&req.name, &mut hasher);
            req.data_type.hash(&mut hasher);
            req.binding_type.hash(&mut hasher);
            req.group.hash(&mut hasher);
            req.binding.hash(&mut hasher);
            req.location.hash(&mut hasher);
            req.writable.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Build HGI vertex buffer descriptors for pipeline creation.
    ///
    /// Each vertex attribute gets its own vertex buffer binding
    /// (non-interleaved layout matching Storm's buffer allocation).
    pub fn build_vertex_descs(&self) -> Vec<HgiVertexBufferDesc> {
        let mut descs = Vec::new();
        let mut binding_idx = 0u32;

        for req in &self.bindings {
            if req.binding_type != BindingType::VertexAttr {
                continue;
            }
            let stride = hgi_format_stride(req.data_type);
            let desc = HgiVertexBufferDesc::new()
                .with_binding_index(binding_idx)
                .with_vertex_stride(stride)
                .with_attribute(HgiVertexAttributeDesc::new(
                    req.data_type,
                    0, // offset within this buffer
                    req.location,
                ));
            descs.push(desc);
            binding_idx += 1;
        }

        descs
    }

    /// Build HgiResourceBindingsDesc from active UBO/SSBO/texture bindings.
    pub fn build_resource_desc(
        &self,
        scene_ubo: Option<&HgiBufferHandle>,
        material_ubo: Option<&HgiBufferHandle>,
    ) -> HgiResourceBindingsDesc {
        let mut desc = HgiResourceBindingsDesc::new();

        for req in &self.bindings {
            match req.binding_type {
                BindingType::Ubo => {
                    let buf = if req.group == slots::SCENE_GROUP {
                        scene_ubo
                    } else if req.group == slots::MATERIAL_GROUP {
                        material_ubo
                    } else {
                        None
                    };
                    if let Some(handle) = buf {
                        desc.buffer_bindings.push(HgiBufferBindDesc {
                            buffers: vec![handle.clone()],
                            offsets: vec![0],
                            sizes: vec![0], // 0 = whole buffer
                            resource_type: HgiBindResourceType::UniformBuffer,
                            binding_index: req.binding,
                            stage_usage: usd_hgi::HgiShaderStage::VERTEX
                                | usd_hgi::HgiShaderStage::FRAGMENT,
                            writable: false,
                        });
                    }
                }
                BindingType::Ssbo => {
                    // SSBO bindings will be added when we have storage buffers
                }
                BindingType::Texture => {
                    // Texture bindings will be added when we have texture support
                }
                _ => {} // vertex attrs and push constants handled elsewhere
            }
        }

        desc
    }

    /// Get all vertex attribute bindings.
    pub fn vertex_attrs(&self) -> impl Iterator<Item = &BindingRequest> {
        self.bindings
            .iter()
            .filter(|b| b.binding_type == BindingType::VertexAttr)
    }

    /// Count of vertex buffer bindings needed.
    pub fn vertex_buffer_count(&self) -> u32 {
        self.vertex_attrs().count() as u32
    }
}

/// Compute stride in bytes for a given HgiFormat.
fn hgi_format_stride(fmt: HgiFormat) -> u32 {
    crate::hgi_conversions::hgi_format_byte_size(fmt) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_mesh_key_lit() {
        let key = MeshShaderKey::default(); // has_normals=true
        let binder = ResourceBinder::from_mesh_key(&key);
        // 2 vertex attrs (pos + normals) + 2 UBOs
        assert_eq!(binder.vertex_buffer_count(), 2);
        let vdescs = binder.build_vertex_descs();
        assert_eq!(vdescs.len(), 2);
    }

    #[test]
    fn test_from_mesh_key_fallback() {
        let key = MeshShaderKey::fallback(); // position only
        let binder = ResourceBinder::from_mesh_key(&key);
        assert_eq!(binder.vertex_buffer_count(), 1);
    }

    // ---------------------------------------------------------------
    // Pipeline/draw_batch consistency: vertex buffer counts
    // ---------------------------------------------------------------
    // If resource_binder says N vertex buffers, draw_batch must bind
    // exactly N. A mismatch causes wgpu validation crash.

    #[test]
    fn test_vertex_buffer_count_matches_key_normals() {
        // has_normals=true -> pipeline needs 2 VBOs (slot 0=pos, slot 1=nrm)
        // draw_batch binds 2 when shader_key.has_normals is true.
        let key = MeshShaderKey {
            has_normals: true,
            ..Default::default()
        };
        let binder = ResourceBinder::from_mesh_key(&key);
        assert_eq!(
            binder.vertex_buffer_count(),
            2,
            "has_normals=true -> 2 vertex buffers (pos + normals)"
        );
    }

    #[test]
    fn test_vertex_buffer_count_no_normals() {
        let key = MeshShaderKey {
            has_normals: false,
            ..Default::default()
        };
        let binder = ResourceBinder::from_mesh_key(&key);
        assert_eq!(
            binder.vertex_buffer_count(),
            1,
            "has_normals=false -> 1 vertex buffer (pos only)"
        );
    }

    #[test]
    fn test_vertex_buffer_count_all_attrs() {
        // pos + normals + uv + color = 4 VBOs
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            has_color: true,
            ..Default::default()
        };
        let binder = ResourceBinder::from_mesh_key(&key);
        assert_eq!(
            binder.vertex_buffer_count(),
            4,
            "all attrs enabled -> 4 vertex buffers"
        );
    }

    #[test]
    fn test_binding_indices_are_sequential() {
        // wgpu requires binding indices 0, 1, 2, ... with no gaps.
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            has_color: true,
            ..Default::default()
        };
        let binder = ResourceBinder::from_mesh_key(&key);
        let descs = binder.build_vertex_descs();

        for (i, desc) in descs.iter().enumerate() {
            assert_eq!(
                desc.binding_index, i as u32,
                "binding index {} must equal sequential index {}",
                desc.binding_index, i
            );
        }
    }

    #[test]
    fn test_vertex_stride_vec3_is_12() {
        let key = MeshShaderKey {
            has_normals: true,
            ..Default::default()
        };
        let binder = ResourceBinder::from_mesh_key(&key);
        let descs = binder.build_vertex_descs();

        // Both pos and normals are Float32Vec3 = 12 bytes
        for desc in &descs {
            assert_eq!(
                desc.vertex_stride, 12,
                "vec3<f32> stride must be 12 bytes, got {}",
                desc.vertex_stride
            );
        }
    }

    #[test]
    fn test_face_varying_channels_do_not_consume_vertex_slots() {
        let key = MeshShaderKey {
            has_normals: true,
            has_uv: true,
            has_color: true,
            has_fvar_normals: true,
            has_fvar_uv: true,
            has_fvar_color: true,
            ..Default::default()
        };
        let binder = ResourceBinder::from_mesh_key(&key);
        assert_eq!(
            binder.vertex_buffer_count(),
            1,
            "position stays as the only vertex attribute when normals/uv/color come from fvar storage"
        );
        assert!(
            binder.cache_hash() != 0,
            "face-varying binding metadata must contribute to cache stability"
        );
    }
}
