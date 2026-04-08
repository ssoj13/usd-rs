//! HdStGeometricShader - Geometric shader for primitives.
//!
//! Port of C++ `HdSt_GeometricShader`.  Wraps a parsed GLSLFX string, carries
//! pipeline-configuration state (cull style, polygon mode, primitive type, etc.)
//! and provides per-stage source extraction.
//!
//! Storm breaks shader programs into four pieces:
//! 1. geometric shader  (this module)
//! 2. material shader
//! 3. lighting shader
//! 4. render pass shader

use crate::shader_code::{HdStShaderCode, NamedTextureHandle, ShaderParameter, ShaderStage};
use crate::shader_key::{GeometricStyle, HdStShaderKey, PrimitiveType};
use std::sync::Arc;
use usd_hd::enums::{HdCullStyle, HdPolygonMode};
use usd_hgi::enums::{HgiCullMode, HgiPrimitiveType};

// ============================================================================
// GsPrimitiveType — full USD parity
// ============================================================================

/// USD-level primitive type, matching C++ `HdSt_GeometricShader::PrimitiveType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GsPrimitiveType {
    Points,
    BasisCurvesLines,
    BasisCurvesLinearPatches,
    BasisCurvesCubicPatches,
    MeshCoarseTriangles,
    MeshRefinedTriangles,
    MeshCoarseQuads,
    MeshRefinedQuads,
    MeshCoarseTriQuads,
    MeshRefinedTriQuads,
    MeshBspline,
    MeshBoxsplineTriangle,
    Volume,
    Compute,
}

impl GsPrimitiveType {
    pub fn is_points(self) -> bool {
        self == Self::Points
    }

    pub fn is_basis_curves(self) -> bool {
        matches!(
            self,
            Self::BasisCurvesLines | Self::BasisCurvesLinearPatches | Self::BasisCurvesCubicPatches
        )
    }

    pub fn is_mesh(self) -> bool {
        matches!(
            self,
            Self::MeshCoarseTriangles
                | Self::MeshRefinedTriangles
                | Self::MeshCoarseQuads
                | Self::MeshRefinedQuads
                | Self::MeshCoarseTriQuads
                | Self::MeshRefinedTriQuads
                | Self::MeshBspline
                | Self::MeshBoxsplineTriangle
        )
    }

    pub fn is_triangles(self) -> bool {
        matches!(
            self,
            Self::MeshCoarseTriangles | Self::MeshRefinedTriangles | Self::Volume
        )
    }

    pub fn is_quads(self) -> bool {
        matches!(self, Self::MeshCoarseQuads | Self::MeshRefinedQuads)
    }

    pub fn is_triquads(self) -> bool {
        matches!(self, Self::MeshCoarseTriQuads | Self::MeshRefinedTriQuads)
    }

    pub fn is_refined_mesh(self) -> bool {
        matches!(
            self,
            Self::MeshRefinedTriangles
                | Self::MeshRefinedQuads
                | Self::MeshRefinedTriQuads
                | Self::MeshBspline
                | Self::MeshBoxsplineTriangle
        )
    }

    pub fn is_patches(self) -> bool {
        matches!(
            self,
            Self::MeshBspline
                | Self::MeshBoxsplineTriangle
                | Self::BasisCurvesCubicPatches
                | Self::BasisCurvesLinearPatches
        )
    }

    pub fn is_compute(self) -> bool {
        self == Self::Compute
    }

    /// Indices per primitive — matches C++ `GetPrimitiveIndexSize`.
    pub fn primitive_index_size(self) -> i32 {
        match self {
            Self::Points => 1,
            Self::BasisCurvesLines | Self::BasisCurvesLinearPatches => 2,
            Self::MeshCoarseTriangles | Self::MeshRefinedTriangles | Self::Volume => 3,
            Self::BasisCurvesCubicPatches | Self::MeshCoarseQuads | Self::MeshRefinedQuads => 4,
            Self::MeshCoarseTriQuads | Self::MeshRefinedTriQuads => 6,
            Self::MeshBspline => 16,
            Self::MeshBoxsplineTriangle => 12,
            Self::Compute => 0,
        }
    }

    /// Patch-eval verts — matches C++ `GetNumPatchEvalVerts`.
    pub fn num_patch_eval_verts(self) -> i32 {
        match self {
            Self::BasisCurvesLinearPatches => 2,
            Self::BasisCurvesCubicPatches => 4,
            Self::MeshBspline => 16,
            Self::MeshBoxsplineTriangle => 15,
            _ => 0,
        }
    }

    /// Prim verts for geometry shader — matches C++ `GetNumPrimitiveVertsForGeometryShader`.
    pub fn num_prim_verts_for_gs(self) -> i32 {
        match self {
            Self::Points => 1,
            Self::BasisCurvesLines => 2,
            Self::MeshCoarseTriangles
            | Self::MeshRefinedTriangles
            | Self::MeshCoarseTriQuads
            | Self::MeshRefinedTriQuads
            | Self::BasisCurvesLinearPatches
            | Self::BasisCurvesCubicPatches
            | Self::MeshBspline
            | Self::MeshBoxsplineTriangle => 3,
            Self::MeshCoarseQuads | Self::MeshRefinedQuads => 4,
            Self::Volume => 3,
            Self::Compute => 1,
        }
    }

    /// HGI primitive type for pipeline creation — matches C++ `GetHgiPrimitiveType`.
    pub fn hgi_primitive_type(self) -> HgiPrimitiveType {
        match self {
            Self::Points => HgiPrimitiveType::PointList,
            Self::BasisCurvesLines => HgiPrimitiveType::LineList,
            Self::BasisCurvesLinearPatches
            | Self::BasisCurvesCubicPatches
            | Self::MeshBspline
            | Self::MeshBoxsplineTriangle => HgiPrimitiveType::PatchList,
            _ => HgiPrimitiveType::TriangleList,
        }
    }
}

// ============================================================================
// FvarPatchType
// ============================================================================

/// Face-varying patch type.  Matches C++ `FvarPatchType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FvarPatchType {
    CoarseTriangles,
    RefinedTriangles,
    CoarseQuads,
    RefinedQuads,
    Bspline,
    BoxsplineTriangle,
    #[default]
    None,
}

// ============================================================================
// HdStGeometricShader
// ============================================================================

/// Geometric shader for primitive rendering.
///
/// Carries pipeline configuration and provides per-stage shader source.
/// Matches C++ `HdSt_GeometricShader`.
#[derive(Debug)]
pub struct HdStGeometricShader {
    /// Shader key identifying this variant
    key: HdStShaderKey,

    /// Unique shader ID (hash-derived)
    id: u64,

    /// Primitive type (full USD set)
    prim_type: GsPrimitiveType,

    /// Cull style from the rprim
    cull_style: HdCullStyle,

    /// Whether to use hardware face culling (vs. fragment-shader discard)
    use_hw_face_culling: bool,

    /// True when the prim's transform has a negative determinant
    has_mirrored_transform: bool,

    /// Double-sided rendering
    double_sided: bool,

    /// Polygon mode (fill / wireframe)
    polygon_mode: HdPolygonMode,

    /// Line width for edge rendering
    line_width: f32,

    /// True when this shader is the frustum-culling compute pass
    frustum_culling_pass: bool,

    /// Face-varying patch type
    fvar_patch_type: FvarPatchType,

    /// Vertex shader source
    vertex_source: String,

    /// Tessellation control source (optional)
    tess_control_source: Option<String>,

    /// Tessellation evaluation source (optional)
    tess_eval_source: Option<String>,

    /// Geometry shader source (optional)
    geometry_source: Option<String>,
}

impl HdStGeometricShader {
    /// Create from a full set of parameters — mirrors the C++ constructor.
    pub fn new(
        prim_type: GsPrimitiveType,
        cull_style: HdCullStyle,
        use_hw_face_culling: bool,
        has_mirrored_transform: bool,
        double_sided: bool,
        polygon_mode: HdPolygonMode,
        frustum_culling_pass: bool,
        fvar_patch_type: FvarPatchType,
        line_width: f32,
    ) -> Self {
        let geom_style = if polygon_mode == HdPolygonMode::Line {
            GeometricStyle::Edges
        } else if prim_type == GsPrimitiveType::Points {
            GeometricStyle::Points
        } else {
            GeometricStyle::Surface
        };

        let shader_prim_type = match prim_type {
            GsPrimitiveType::BasisCurvesLines | GsPrimitiveType::BasisCurvesLinearPatches => {
                PrimitiveType::Lines
            }
            GsPrimitiveType::MeshBspline
            | GsPrimitiveType::MeshBoxsplineTriangle
            | GsPrimitiveType::BasisCurvesCubicPatches => PrimitiveType::Patches,
            GsPrimitiveType::Points => PrimitiveType::Points,
            _ => PrimitiveType::Triangles,
        };

        let key = HdStShaderKey::new(shader_prim_type, geom_style);
        let id = key.get_hash();

        let mut s = Self {
            key,
            id,
            prim_type,
            cull_style,
            use_hw_face_culling,
            has_mirrored_transform,
            double_sided,
            polygon_mode,
            line_width,
            frustum_culling_pass,
            fvar_patch_type,
            vertex_source: String::new(),
            tess_control_source: None,
            tess_eval_source: None,
            geometry_source: None,
        };
        s.generate_sources();
        s
    }

    /// Convenience constructor from a `HdStShaderKey` (legacy path).
    pub fn from_key(key: HdStShaderKey) -> Self {
        let prim_type = match key.get_primitive_type() {
            PrimitiveType::Points => GsPrimitiveType::Points,
            PrimitiveType::Lines | PrimitiveType::LineStrip => GsPrimitiveType::BasisCurvesLines,
            PrimitiveType::Patches => GsPrimitiveType::MeshBspline,
            _ => GsPrimitiveType::MeshCoarseTriangles,
        };
        let polygon_mode = match key.get_geom_style() {
            GeometricStyle::Edges => HdPolygonMode::Line,
            _ => HdPolygonMode::Fill,
        };
        let id = key.get_hash();
        let mut s = Self {
            id,
            prim_type,
            cull_style: HdCullStyle::DontCare,
            use_hw_face_culling: true,
            has_mirrored_transform: false,
            double_sided: false,
            polygon_mode,
            line_width: 1.0,
            frustum_culling_pass: false,
            fvar_patch_type: FvarPatchType::None,
            vertex_source: String::new(),
            tess_control_source: None,
            tess_eval_source: None,
            geometry_source: None,
            key,
        };
        s.generate_sources();
        s
    }

    // ---- Accessors ----------------------------------------------------------

    pub fn get_key(&self) -> &HdStShaderKey {
        &self.key
    }
    pub fn get_primitive_type(&self) -> PrimitiveType {
        self.key.get_primitive_type()
    }
    pub fn get_gs_primitive_type(&self) -> GsPrimitiveType {
        self.prim_type
    }
    pub fn get_geom_style(&self) -> GeometricStyle {
        self.key.get_geom_style()
    }
    pub fn get_cull_style(&self) -> HdCullStyle {
        self.cull_style
    }
    pub fn is_double_sided(&self) -> bool {
        self.double_sided
    }
    pub fn has_mirrored_transform(&self) -> bool {
        self.has_mirrored_transform
    }
    pub fn get_polygon_mode(&self) -> HdPolygonMode {
        self.polygon_mode
    }
    pub fn get_line_width(&self) -> f32 {
        self.line_width
    }
    pub fn set_line_width(&mut self, w: f32) {
        self.line_width = w;
    }
    pub fn is_frustum_culling_pass(&self) -> bool {
        self.frustum_culling_pass
    }
    pub fn get_fvar_patch_type(&self) -> FvarPatchType {
        self.fvar_patch_type
    }

    /// Number of indices per primitive — matches C++ `GetPrimitiveIndexSize`.
    pub fn get_primitive_index_size(&self) -> i32 {
        self.prim_type.primitive_index_size()
    }

    /// Number of patch-eval verts — matches C++ `GetNumPatchEvalVerts`.
    pub fn get_num_patch_eval_verts(&self) -> i32 {
        self.prim_type.num_patch_eval_verts()
    }

    /// Number of prim verts for geometry shader — matches C++ `GetNumPrimitiveVertsForGeometryShader`.
    pub fn get_num_prim_verts_for_gs(&self) -> i32 {
        self.prim_type.num_prim_verts_for_gs()
    }

    /// HGI primitive type for pipeline state — matches C++ `GetHgiPrimitiveType`.
    pub fn get_hgi_primitive_type(&self) -> HgiPrimitiveType {
        self.prim_type.hgi_primitive_type()
    }

    /// Resolve the HGI cull mode, combining rprim opinion + render-state fallback.
    ///
    /// If `use_hw_face_culling` is false, culling is done in the fragment shader
    /// (discard) so hardware culling is disabled (`HgiCullMode::None`).
    /// Mirrors C++ `ResolveCullMode`.
    pub fn resolve_cull_mode(&self, render_state_cull: HdCullStyle) -> HgiCullMode {
        if !self.use_hw_face_culling {
            return HgiCullMode::None;
        }

        // Rprim's own opinion wins; fall back to render state
        let resolved = if self.cull_style == HdCullStyle::DontCare {
            render_state_cull
        } else {
            self.cull_style
        };

        match resolved {
            HdCullStyle::Front => {
                if self.has_mirrored_transform {
                    HgiCullMode::Back
                } else {
                    HgiCullMode::Front
                }
            }
            HdCullStyle::FrontUnlessDoubleSided => {
                if !self.double_sided {
                    if self.has_mirrored_transform {
                        HgiCullMode::Back
                    } else {
                        HgiCullMode::Front
                    }
                } else {
                    HgiCullMode::None
                }
            }
            HdCullStyle::Back => {
                if self.has_mirrored_transform {
                    HgiCullMode::Front
                } else {
                    HgiCullMode::Back
                }
            }
            HdCullStyle::BackUnlessDoubleSided => {
                if !self.double_sided {
                    if self.has_mirrored_transform {
                        HgiCullMode::Front
                    } else {
                        HgiCullMode::Back
                    }
                } else {
                    HgiCullMode::None
                }
            }
            _ => HgiCullMode::None,
        }
    }

    // ---- Internal source generation -----------------------------------------

    fn generate_sources(&mut self) {
        self.vertex_source = self.generate_vertex_shader();

        if self.prim_type.is_patches() {
            self.tess_control_source = Some(self.generate_tess_control_shader());
            self.tess_eval_source = Some(self.generate_tess_eval_shader());
        }

        if self.key.is_instanced() {
            self.geometry_source = Some(self.generate_geometry_shader());
        }
    }

    fn generate_vertex_shader(&self) -> String {
        let mut src = String::from("#version 450\n\n");

        src.push_str("layout(location = 0) in vec3 position;\n");
        src.push_str("layout(location = 1) in vec3 normal;\n");

        if self.key.has_vertex_colors() {
            src.push_str("layout(location = 2) in vec4 color;\n");
        }
        if self.key.has_texture_coords() {
            src.push_str("layout(location = 3) in vec2 uv;\n");
        }

        src.push_str("\nlayout(binding = 0) uniform Transform {\n");
        src.push_str("    mat4 modelViewProjection;\n");
        src.push_str("    mat4 modelView;\n");
        src.push_str("};\n\n");

        src.push_str("out VertexData {\n");
        src.push_str("    vec3 position;\n");
        src.push_str("    vec3 normal;\n");
        if self.key.has_vertex_colors() {
            src.push_str("    vec4 color;\n");
        }
        if self.key.has_texture_coords() {
            src.push_str("    vec2 uv;\n");
        }
        src.push_str("} outData;\n\n");

        src.push_str("void main() {\n");
        src.push_str("    gl_Position = modelViewProjection * vec4(position, 1.0);\n");
        src.push_str("    outData.position = (modelView * vec4(position, 1.0)).xyz;\n");
        src.push_str("    outData.normal = mat3(modelView) * normal;\n");
        if self.key.has_vertex_colors() {
            src.push_str("    outData.color = color;\n");
        }
        if self.key.has_texture_coords() {
            src.push_str("    outData.uv = uv;\n");
        }
        src.push_str("}\n");

        src
    }

    fn generate_tess_control_shader(&self) -> String {
        let mut src = String::from("#version 450\n\n");
        src.push_str("layout(vertices = 4) out;\n\n");
        src.push_str("void main() {\n");
        src.push_str("    gl_TessLevelOuter[0] = 4.0;\n");
        src.push_str("    gl_TessLevelOuter[1] = 4.0;\n");
        src.push_str("    gl_TessLevelOuter[2] = 4.0;\n");
        src.push_str("    gl_TessLevelOuter[3] = 4.0;\n");
        src.push_str("    gl_TessLevelInner[0] = 4.0;\n");
        src.push_str("    gl_TessLevelInner[1] = 4.0;\n");
        src.push_str("}\n");
        src
    }

    fn generate_tess_eval_shader(&self) -> String {
        let mut src = String::from("#version 450\n\n");
        src.push_str("layout(quads, equal_spacing, ccw) in;\n\n");
        src.push_str("void main() {\n");
        src.push_str("    gl_Position = gl_in[0].gl_Position;\n");
        src.push_str("}\n");
        src
    }

    fn generate_geometry_shader(&self) -> String {
        let mut src = String::from("#version 450\n\n");
        src.push_str("layout(triangles) in;\n");
        src.push_str("layout(triangle_strip, max_vertices = 3) out;\n\n");
        src.push_str("void main() {\n");
        src.push_str("    for (int i = 0; i < 3; i++) {\n");
        src.push_str("        gl_Position = gl_in[i].gl_Position;\n");
        src.push_str("        EmitVertex();\n");
        src.push_str("    }\n");
        src.push_str("    EndPrimitive();\n");
        src.push_str("}\n");
        src
    }
}

// ============================================================================
// HdStShaderCode impl
// ============================================================================

impl HdStShaderCode for HdStGeometricShader {
    fn get_id(&self) -> u64 {
        self.id
    }
    fn get_params(&self) -> Vec<ShaderParameter> {
        Vec::new()
    }
    fn get_textures(&self) -> Vec<NamedTextureHandle> {
        Vec::new()
    }
    fn is_valid(&self) -> bool {
        !self.vertex_source.is_empty()
    }
    fn get_hash(&self) -> u64 {
        self.id
    }

    fn get_source(&self, stage: ShaderStage) -> String {
        match stage {
            ShaderStage::Vertex => self.vertex_source.clone(),
            ShaderStage::TessControl => self.tess_control_source.clone().unwrap_or_default(),
            ShaderStage::TessEval => self.tess_eval_source.clone().unwrap_or_default(),
            ShaderStage::Geometry => self.geometry_source.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }
}

/// Shared pointer to geometric shader.
pub type HdStGeometricShaderSharedPtr = Arc<HdStGeometricShader>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_key::{GeometricStyle, HdStShaderKey, PrimitiveType};

    fn make_tri() -> HdStGeometricShader {
        HdStGeometricShader::new(
            GsPrimitiveType::MeshCoarseTriangles,
            HdCullStyle::Back,
            true,
            false,
            false,
            HdPolygonMode::Fill,
            false,
            FvarPatchType::None,
            1.0,
        )
    }

    #[test]
    fn test_primitive_index_sizes() {
        assert_eq!(GsPrimitiveType::Points.primitive_index_size(), 1);
        assert_eq!(GsPrimitiveType::BasisCurvesLines.primitive_index_size(), 2);
        assert_eq!(
            GsPrimitiveType::MeshCoarseTriangles.primitive_index_size(),
            3
        );
        assert_eq!(GsPrimitiveType::MeshCoarseQuads.primitive_index_size(), 4);
        assert_eq!(
            GsPrimitiveType::MeshCoarseTriQuads.primitive_index_size(),
            6
        );
        assert_eq!(GsPrimitiveType::MeshBspline.primitive_index_size(), 16);
        assert_eq!(
            GsPrimitiveType::MeshBoxsplineTriangle.primitive_index_size(),
            12
        );
        assert_eq!(GsPrimitiveType::Compute.primitive_index_size(), 0);
    }

    #[test]
    fn test_hgi_primitive_types() {
        assert_eq!(
            GsPrimitiveType::Points.hgi_primitive_type(),
            HgiPrimitiveType::PointList
        );
        assert_eq!(
            GsPrimitiveType::BasisCurvesLines.hgi_primitive_type(),
            HgiPrimitiveType::LineList
        );
        assert_eq!(
            GsPrimitiveType::MeshBspline.hgi_primitive_type(),
            HgiPrimitiveType::PatchList
        );
        assert_eq!(
            GsPrimitiveType::MeshCoarseTriangles.hgi_primitive_type(),
            HgiPrimitiveType::TriangleList
        );
    }

    #[test]
    fn test_resolve_cull_no_hw() {
        let mut s = make_tri();
        s.use_hw_face_culling = false;
        assert_eq!(s.resolve_cull_mode(HdCullStyle::Back), HgiCullMode::None);
    }

    #[test]
    fn test_resolve_cull_back() {
        let s = make_tri(); // cull_style=Back, no mirror
        assert_eq!(
            s.resolve_cull_mode(HdCullStyle::DontCare),
            HgiCullMode::Back
        );
    }

    #[test]
    fn test_resolve_cull_mirrored() {
        let mut s = make_tri();
        s.has_mirrored_transform = true;
        // Back + mirror -> Front
        assert_eq!(
            s.resolve_cull_mode(HdCullStyle::DontCare),
            HgiCullMode::Front
        );
    }

    #[test]
    fn test_resolve_cull_double_sided() {
        let mut s = HdStGeometricShader::new(
            GsPrimitiveType::MeshCoarseTriangles,
            HdCullStyle::BackUnlessDoubleSided,
            true,
            false,
            true,
            HdPolygonMode::Fill,
            false,
            FvarPatchType::None,
            1.0,
        );
        // BackUnlessDoubleSided + double_sided -> None
        assert_eq!(
            s.resolve_cull_mode(HdCullStyle::DontCare),
            HgiCullMode::None
        );
        s.double_sided = false;
        assert_eq!(
            s.resolve_cull_mode(HdCullStyle::DontCare),
            HgiCullMode::Back
        );
    }

    #[test]
    fn test_patch_shaders_generated() {
        let s = HdStGeometricShader::new(
            GsPrimitiveType::MeshBspline,
            HdCullStyle::Nothing,
            true,
            false,
            false,
            HdPolygonMode::Fill,
            false,
            FvarPatchType::Bspline,
            1.0,
        );
        assert!(!s.get_source(ShaderStage::TessControl).is_empty());
        assert!(!s.get_source(ShaderStage::TessEval).is_empty());
    }

    #[test]
    fn test_from_key_compat() {
        let key = HdStShaderKey::new(PrimitiveType::Triangles, GeometricStyle::Surface);
        let s = HdStGeometricShader::from_key(key);
        assert_eq!(
            s.get_gs_primitive_type(),
            GsPrimitiveType::MeshCoarseTriangles
        );
        assert!(s.is_valid());
    }

    #[test]
    fn test_wireframe_mode() {
        let s = HdStGeometricShader::new(
            GsPrimitiveType::MeshCoarseTriangles,
            HdCullStyle::Nothing,
            true,
            false,
            false,
            HdPolygonMode::Line,
            false,
            FvarPatchType::None,
            2.0,
        );
        assert_eq!(s.get_polygon_mode(), HdPolygonMode::Line);
        assert_eq!(s.get_line_width(), 2.0);
        assert_eq!(s.get_geom_style(), GeometricStyle::Edges);
    }

    #[test]
    fn test_frustum_culling_pass() {
        let s = HdStGeometricShader::new(
            GsPrimitiveType::Compute,
            HdCullStyle::Nothing,
            false,
            false,
            false,
            HdPolygonMode::Fill,
            true,
            FvarPatchType::None,
            0.0,
        );
        assert!(s.is_frustum_culling_pass());
        assert_eq!(s.get_hgi_primitive_type(), HgiPrimitiveType::TriangleList);
    }

    #[test]
    fn test_prim_type_queries() {
        assert!(GsPrimitiveType::MeshBspline.is_patches());
        assert!(GsPrimitiveType::MeshCoarseTriangles.is_triangles());
        assert!(GsPrimitiveType::MeshCoarseQuads.is_quads());
        assert!(GsPrimitiveType::MeshRefinedQuads.is_refined_mesh());
        assert!(GsPrimitiveType::BasisCurvesLines.is_basis_curves());
        assert!(GsPrimitiveType::Compute.is_compute());
    }
}
