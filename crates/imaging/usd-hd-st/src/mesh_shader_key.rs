//! MeshShaderKey - drives shader variant selection for mesh rendering.
//!
//! Ported from C++ meshShaderKey.h. Encodes which primvars are present,
//! interpolation mode, shading style, and material features to select
//! the correct shader variant.

use std::hash::{Hash, Hasher};

/// Shading model for fragment shader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ShadingModel {
    /// Flat grey, no lighting (fallback)
    #[default]
    FlatColor,
    /// Flat geometry shading using face normals (dFdx/dFdy in FS), no material
    GeomFlat,
    /// Smooth geometry shading using vertex normals, no material
    GeomSmooth,
    /// Blinn-Phong with diffuse + specular
    BlinnPhong,
    /// PBR metallic-roughness (UsdPreviewSurface)
    Pbr,
}

/// Primitive topology for the draw call.
///
/// Controls HgiPrimitiveType selection in the graphics pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DrawTopology {
    /// Indexed triangle list (normal solid mesh)
    #[default]
    TriangleList,
    /// Line list for wireframe (uses edge indices)
    LineList,
    /// Point list per vertex
    PointList,
}

/// Primvar interpolation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PrimvarInterp {
    /// Per-vertex interpolation (most common)
    #[default]
    Vertex,
    /// Per-face (flat) interpolation
    FaceVarying,
    /// Uniform (per-face, single value)
    Uniform,
}

/// Mesh shader variant key.
///
/// Uniquely identifies which shader variant to compile based on
/// available primvars, shading style, and material features.
/// Used as a cache key for compiled pipelines.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MeshShaderKey {
    /// Mesh has vertex/face normals
    pub has_normals: bool,
    /// Mesh has vertex colors (displayColor)
    pub has_color: bool,
    /// Mesh has texture coordinates
    pub has_uv: bool,
    /// Shading model to use
    pub shading: ShadingModel,
    /// Normal interpolation mode
    pub normal_interp: PrimvarInterp,
    /// Normals are sourced from the face-varying storage BAR.
    pub has_fvar_normals: bool,
    /// UVs are sourced from the face-varying storage BAR.
    pub has_fvar_uv: bool,
    /// displayColor is sourced from the face-varying storage BAR.
    pub has_fvar_color: bool,
    /// displayOpacity is sourced from the face-varying storage BAR.
    pub has_fvar_opacity: bool,
    /// Enable displacement mapping
    pub has_displacement: bool,
    /// Enable alpha cutoff (opacity < threshold discards)
    pub has_alpha_cutoff: bool,
    /// Wireframe overlay
    pub wireframe: bool,
    /// IBL textures are available (irradiance + prefilter + BRDF LUT bound in group 4)
    pub has_ibl: bool,
    /// GPU primitive topology (triangle/line/point)
    pub topology: DrawTopology,
    /// Depth-only pass (no color output, for HiddenSurfaceWireframe prepass)
    pub depth_only: bool,
    /// Use face normals via dpdx/dpdy instead of vertex normals (ShadedFlat mode).
    /// Unlike GeomFlat, this keeps full material/lighting evaluation.
    pub flat_shading: bool,
    /// Shadow mapping enabled (shadow uniforms + depth atlas bound in light group).
    /// Matches C++ `#define USE_SHADOWS 1` in simpleLightingShader.cpp::GetSource().
    pub use_shadows: bool,
    /// GPU instancing: instance transforms in storage buffer, indexed by instance_index.
    /// When true, vertex shader reads model matrix from SSBO instead of scene.model.
    pub use_instancing: bool,
    /// Pick/deep-resolve storage buffer is bound for this shader variant.
    /// Used to route `resolveDeep`-style pick writes through the live wgpu pipeline.
    pub pick_buffer_rw: bool,
}

impl Default for MeshShaderKey {
    fn default() -> Self {
        Self {
            has_normals: true,
            has_color: false,
            has_uv: false,
            shading: ShadingModel::BlinnPhong,
            normal_interp: PrimvarInterp::Vertex,
            has_fvar_normals: false,
            has_fvar_uv: false,
            has_fvar_color: false,
            has_fvar_opacity: false,
            has_displacement: false,
            has_alpha_cutoff: false,
            wireframe: false,
            has_ibl: false,
            topology: DrawTopology::TriangleList,
            depth_only: false,
            flat_shading: false,
            use_shadows: false,
            use_instancing: false,
            pick_buffer_rw: false,
        }
    }
}

impl MeshShaderKey {
    /// Create minimal key for position-only rendering (fallback).
    pub fn fallback() -> Self {
        Self {
            has_normals: false,
            shading: ShadingModel::FlatColor,
            ..Default::default()
        }
    }

    /// Create key for lit mesh with normals.
    pub fn lit() -> Self {
        Self::default()
    }

    /// Create key for UsdPreviewSurface PBR material.
    pub fn pbr() -> Self {
        Self {
            has_normals: true,
            shading: ShadingModel::Pbr,
            ..Default::default()
        }
    }

    /// Create key for point rendering (PointList topology, flat grey).
    pub fn points() -> Self {
        Self {
            has_normals: false,
            shading: ShadingModel::FlatColor,
            topology: DrawTopology::PointList,
            ..Default::default()
        }
    }

    /// Create key for wireframe rendering (LineList topology, flat grey).
    pub fn wireframe() -> Self {
        Self {
            has_normals: false,
            shading: ShadingModel::FlatColor,
            topology: DrawTopology::LineList,
            ..Default::default()
        }
    }

    /// Create key for geometry-only flat shading (face normals, no material).
    pub fn geom_flat() -> Self {
        Self {
            has_normals: false, // face normals computed in FS via dFdx/dFdy
            shading: ShadingModel::GeomFlat,
            ..Default::default()
        }
    }

    /// Create key for geometry-only smooth shading (vertex normals, no material).
    pub fn geom_smooth() -> Self {
        Self {
            has_normals: true,
            shading: ShadingModel::GeomSmooth,
            ..Default::default()
        }
    }

    /// Create depth-only key (no color output, for prepass in HiddenSurfaceWireframe).
    pub fn depth_only() -> Self {
        Self {
            has_normals: false,
            shading: ShadingModel::FlatColor,
            depth_only: true,
            ..Default::default()
        }
    }

    /// Returns the HGI primitive type for pipeline creation.
    pub fn hgi_primitive_type(&self) -> usd_hgi::HgiPrimitiveType {
        match self.topology {
            DrawTopology::TriangleList => usd_hgi::HgiPrimitiveType::TriangleList,
            DrawTopology::LineList => usd_hgi::HgiPrimitiveType::LineList,
            DrawTopology::PointList => usd_hgi::HgiPrimitiveType::PointList,
        }
    }

    /// Returns true when this key uses per-pixel face normals (GeomFlat).
    pub fn uses_face_normals(&self) -> bool {
        self.shading == ShadingModel::GeomFlat
    }

    /// Returns true when materials/textures should be ignored.
    pub fn ignores_materials(&self) -> bool {
        matches!(
            self.shading,
            ShadingModel::FlatColor | ShadingModel::GeomFlat | ShadingModel::GeomSmooth
        )
    }

    /// Compute a u64 hash for use as pipeline cache key.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// Number of vertex attributes this key requires.
    pub fn vertex_attr_count(&self) -> u32 {
        let mut count = 1; // position always present
        if self.has_normals && !self.has_fvar_normals {
            count += 1;
        }
        if self.has_color && !self.has_fvar_color {
            count += 1;
        }
        if self.has_uv && !self.has_fvar_uv {
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_key() {
        let key = MeshShaderKey::default();
        assert!(key.has_normals);
        assert!(!key.has_color);
        assert_eq!(key.shading, ShadingModel::BlinnPhong);
    }

    #[test]
    fn test_cache_hash_differs() {
        let k1 = MeshShaderKey::fallback();
        let k2 = MeshShaderKey::lit();
        assert_ne!(k1.cache_hash(), k2.cache_hash());
    }

    #[test]
    fn test_vertex_attr_count() {
        assert_eq!(MeshShaderKey::fallback().vertex_attr_count(), 1);
        assert_eq!(MeshShaderKey::lit().vertex_attr_count(), 2); // pos + normal
        let full = MeshShaderKey {
            has_normals: true,
            has_color: true,
            has_uv: true,
            ..Default::default()
        };
        assert_eq!(full.vertex_attr_count(), 4);

        let fvar = MeshShaderKey {
            has_normals: true,
            has_color: true,
            has_uv: true,
            has_fvar_normals: true,
            has_fvar_color: true,
            has_fvar_uv: true,
            ..Default::default()
        };
        assert_eq!(fvar.vertex_attr_count(), 1);
    }
}
