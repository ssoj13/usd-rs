//! Runtime program-key selection for Storm draw batches.
//!
//! OpenUSD does not route `HdStPoints` or `HdStBasisCurves` through the mesh
//! shader key. Each rprim family contributes its own shader-key type and
//! resource-binding contract. The earlier Rust batch path collapsed all live
//! draw items onto `MeshShaderKey`, which silently reintroduced mesh-centric
//! assumptions into points/curves rendering.
//!
//! This module keeps the active draw-program choice explicit so the batch layer
//! can dispatch codegen, binding, and pipeline creation based on the actual
//! rprim kind instead of inferring "non-triangle means flat mesh".

use crate::basis_curves_shader_key::{BasisCurvesShaderKey, CurveDrawStyle, CurveNormalStyle};
use crate::mesh_shader_key::MeshShaderKey;
use crate::points_shader_key::PointsShaderKey;
use std::hash::{Hash, Hasher};

/// Runtime shader key for point rprims.
///
/// The embedded `PointsShaderKey` mirrors the `_ref` shader family selection,
/// while the extra booleans capture retained draw-item state that affects the
/// live wgpu program layout.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PointsProgramKey {
    pub shader_key: PointsShaderKey,
    pub has_color: bool,
    pub has_widths: bool,
    pub depth_only: bool,
    pub pick_buffer_rw: bool,
    pub use_instancing: bool,
}

impl PointsProgramKey {
    /// Compute a stable cache hash for pipeline/program reuse.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

/// Runtime shader key for basis-curves rprims.
///
/// `_ref` carries rich draw-style / normal-style state through
/// `HdSt_BasisCurvesShaderKey`; the Rust runtime keeps that same split and adds
/// retained attribute presence bits needed by WGSL/binder generation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BasisCurvesProgramKey {
    pub shader_key: BasisCurvesShaderKey,
    pub has_color: bool,
    pub has_normals: bool,
    pub has_widths: bool,
    pub depth_only: bool,
    pub pick_buffer_rw: bool,
    pub use_instancing: bool,
}

impl BasisCurvesProgramKey {
    /// Compute a stable cache hash for pipeline/program reuse.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }

    /// True when the active curve draw style is point rasterization.
    pub fn draws_points(&self) -> bool {
        self.shader_key.draw_style == CurveDrawStyle::Points
    }

    /// True when the active curve draw style is the retained wire path.
    pub fn draws_lines(&self) -> bool {
        self.shader_key.draw_style == CurveDrawStyle::Wire
    }
}

/// Active Storm draw-program family for a batch.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DrawProgramKey {
    Mesh(MeshShaderKey),
    Points(PointsProgramKey),
    BasisCurves(BasisCurvesProgramKey),
}

impl DrawProgramKey {
    /// Compute a stable cache hash across all program families.
    pub fn cache_hash(&self) -> u64 {
        match self {
            Self::Mesh(key) => key.cache_hash(),
            Self::Points(key) => key.cache_hash(),
            Self::BasisCurves(key) => key.cache_hash(),
        }
    }

    /// Whether this program needs the deep-pick storage buffer.
    pub fn pick_buffer_rw(&self) -> bool {
        match self {
            Self::Mesh(key) => key.pick_buffer_rw,
            Self::Points(key) => key.pick_buffer_rw,
            Self::BasisCurves(key) => key.pick_buffer_rw,
        }
    }

    /// Whether this program needs per-instance transforms from the instance SSBO.
    pub fn use_instancing(&self) -> bool {
        match self {
            Self::Mesh(key) => key.use_instancing,
            Self::Points(key) => key.use_instancing,
            Self::BasisCurves(key) => key.use_instancing,
        }
    }

    /// Whether this program needs material uniforms bound.
    ///
    /// Points and wire curves do not use the full lighting stack yet, but they
    /// still use the material fallback color when no vertex color is authored.
    pub fn needs_material_uniforms(&self) -> bool {
        !matches!(
            self,
            Self::Mesh(MeshShaderKey {
                depth_only: true,
                ..
            })
        )
    }

    /// Whether this program needs light uniforms bound.
    ///
    /// The dedicated points/curves WGSL path intentionally stays unlit for now;
    /// `_ref` eventually feeds richer shading through specialized GLSLFX paths,
    /// but the important parity step here is to stop inheriting mesh shader
    /// assumptions. Lighting can be layered on top of the dedicated program
    /// family once the binding/layout split is correct.
    pub fn needs_lighting_uniforms(&self) -> bool {
        matches!(self, Self::Mesh(MeshShaderKey { shading, .. }) if *shading != crate::mesh_shader_key::ShadingModel::FlatColor)
    }

    /// Primitive type consumed by HGI pipeline creation.
    pub fn hgi_primitive_type(&self) -> usd_hgi::HgiPrimitiveType {
        match self {
            Self::Mesh(key) => key.hgi_primitive_type(),
            Self::Points(_) => usd_hgi::HgiPrimitiveType::PointList,
            Self::BasisCurves(key) => {
                if key.draws_points() {
                    usd_hgi::HgiPrimitiveType::PointList
                } else {
                    usd_hgi::HgiPrimitiveType::LineList
                }
            }
        }
    }

    /// Human-readable label used in debug names/logging.
    pub fn debug_label(&self) -> &'static str {
        match self {
            Self::Mesh(_) => "mesh",
            Self::Points(_) => "points",
            Self::BasisCurves(key) if key.draws_points() => "basisCurves.points",
            Self::BasisCurves(_) => "basisCurves.wire",
        }
    }
}

/// Default normal style for the currently supported retained curves path.
pub const BASIS_CURVES_DEFAULT_NORMAL_STYLE: CurveNormalStyle = CurveNormalStyle::Hair;
