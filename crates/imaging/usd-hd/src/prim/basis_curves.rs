//! HdBasisCurves - Basis curves primitive.
//!
//! Represents curves (splines) in Hydra. Supports:
//! - Multiple basis types (bezier, bspline, catmullRom, hermite)
//! - Linear curves
//! - Varying widths along curves
//! - Cubic and linear segments
//!
//! # Curve Types
//!
//! - **Cubic**: Smooth curves with 4 CVs per segment
//! - **Linear**: Straight line segments with 2 CVs per segment
//!
//! # Basis Types
//!
//! - **Bezier**: Cubic Bezier curves
//! - **BSpline**: B-spline curves  
//! - **CatmullRom**: Catmull-Rom splines
//! - **Hermite**: Hermite curves

use once_cell::sync::Lazy;

use super::{HdBasisCurvesReprDesc, HdRenderParam, HdRprim, HdSceneDelegate, ReprDescConfigs};
use crate::scene_delegate::HdDisplayStyle;
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Global repr config for basis curves (1 descriptor per repr).
static CURVES_REPR_CONFIGS: Lazy<ReprDescConfigs<HdBasisCurvesReprDesc, 1>> =
    Lazy::new(ReprDescConfigs::new);

/// Curve basis type defining the interpolation method for cubic curves.
///
/// Specifies how control vertices (CVs) are interpreted to form smooth curves.
/// Each basis type has different mathematical properties affecting curve shape.
///
/// # OpenUSD Reference
///
/// Corresponds to `HdBasisCurvesTopology::Basis` in OpenUSD.
/// See [UsdGeomBasisCurves](https://openusd.org/dev/api/class_usd_geom_basis_curves.html)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdCurveBasis {
    /// Cubic Bezier curves with 4 CVs per segment (endpoints + control points).
    Bezier,
    /// B-spline curves with local control and C2 continuity.
    BSpline,
    /// Catmull-Rom splines interpolating through all CVs.
    CatmullRom,
    /// Hermite curves defined by endpoints and tangent vectors.
    Hermite,
}

impl HdCurveBasis {
    /// Return the C++ token string for this basis type.
    pub fn as_token_str(&self) -> &'static str {
        match self {
            Self::Bezier => "bezier",
            Self::BSpline => "bspline",
            Self::CatmullRom => "catmullRom",
            Self::Hermite => "hermite",
        }
    }
}

impl std::fmt::Display for HdCurveBasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_token_str())
    }
}

/// Curve segment type determining interpolation degree.
///
/// Defines whether curves use cubic (4 CVs) or linear (2 CVs) segments.
///
/// # OpenUSD Reference
///
/// Corresponds to `HdBasisCurvesTopology::CurveType` in OpenUSD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdCurveType {
    /// Cubic segments using 4 control vertices with smooth interpolation.
    Cubic,
    /// Linear segments using 2 vertices forming straight lines.
    Linear,
}

impl HdCurveType {
    /// Return the C++ token string for this curve type.
    pub fn as_token_str(&self) -> &'static str {
        match self {
            Self::Cubic => "cubic",
            Self::Linear => "linear",
        }
    }
}

impl std::fmt::Display for HdCurveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_token_str())
    }
}

/// Curve wrap mode controlling endpoint behavior.
///
/// Determines whether curves are open, closed (forming loops), or pinned.
///
/// # OpenUSD Reference
///
/// Corresponds to `HdBasisCurvesTopology::Wrap` in OpenUSD.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HdCurveWrap {
    /// Open curves (not wrapped).
    #[default]
    Nonperiodic,
    /// Closed curves (wrapped).
    Periodic,
    /// Pinned at both ends.
    Pinned,
}

impl HdCurveWrap {
    /// Return the C++ token string for this wrap mode.
    pub fn as_token_str(&self) -> &'static str {
        match self {
            Self::Nonperiodic => "nonperiodic",
            Self::Periodic => "periodic",
            Self::Pinned => "pinned",
        }
    }
}

impl std::fmt::Display for HdCurveWrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_token_str())
    }
}

/// Basis curves topology describing curve structure and properties.
///
/// Contains all topological information needed to define curve geometry:
/// - Vertex counts per curve
/// - Basis type for cubic interpolation
/// - Curve type (cubic vs linear)
/// - Wrap mode for endpoint handling
///
/// # OpenUSD Reference
///
/// Corresponds to `HdBasisCurvesTopology` in OpenUSD.
/// See [HdBasisCurvesTopology](https://openusd.org/dev/api/class_hd_basis_curves_topology.html)
#[derive(Debug, Clone, Default)]
pub struct HdBasisCurvesTopology {
    /// Number of vertices per curve.
    pub curve_vertex_counts: Vec<i32>,

    /// Curve basis type.
    pub basis: Option<HdCurveBasis>,

    /// Curve type (cubic or linear).
    pub curve_type: Option<HdCurveType>,

    /// Curve wrap mode.
    pub wrap: HdCurveWrap,
}

impl HdBasisCurvesTopology {
    /// Creates new empty topology with default values.
    ///
    /// # Returns
    ///
    /// Empty topology with no curves and `Nonperiodic` wrap mode.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the total number of curves in this topology.
    ///
    /// # Returns
    ///
    /// Number of curves, equal to length of `curve_vertex_counts`.
    pub fn num_curves(&self) -> usize {
        self.curve_vertex_counts.len()
    }
}

/// Hydra basis curves renderable primitive (Rprim).
///
/// Represents curves/splines for rendering hair, fur, cables, and other
/// curve-based geometry in Hydra's render pipeline.
///
/// # Curve Rendering
///
/// Supports various rendering styles:
/// - Hair and fur simulation
/// - Cables and wires
/// - Motion paths and trajectories
/// - Decorative curves and splines
///
/// # Data Flow
///
/// 1. Scene delegate populates curve data (positions, widths, topology)
/// 2. `sync()` pulls updated data when dirty bits are set
/// 3. Render delegate converts to GPU-ready representation
///
/// # OpenUSD Reference
///
/// Corresponds to `HdBasisCurves` in OpenUSD.
/// See [HdBasisCurves](https://openusd.org/dev/api/class_hd_basis_curves.html)
#[derive(Debug)]
pub struct HdBasisCurves {
    /// Prim identifier.
    id: SdfPath,

    /// Current dirty bits.
    dirty_bits: HdDirtyBits,

    /// Instancer id if instanced.
    instancer_id: Option<SdfPath>,

    /// Visibility state.
    visible: bool,

    /// Material id.
    material_id: Option<SdfPath>,

    /// Curves topology.
    topology: HdBasisCurvesTopology,
}

impl HdBasisCurves {
    // ------------------------------------------------------------------
    // Static repr configuration
    // ------------------------------------------------------------------

    /// Configure the geometric style for a repr.
    /// Corresponds to C++ `HdBasisCurves::ConfigureRepr`.
    pub fn configure_repr(name: &Token, desc: HdBasisCurvesReprDesc) {
        CURVES_REPR_CONFIGS.add_or_update(name, [desc]);
    }

    /// Look up repr descriptor by name.
    /// Corresponds to C++ `HdBasisCurves::_GetReprDesc`.
    pub fn get_repr_desc(name: &Token) -> Option<[HdBasisCurvesReprDesc; 1]> {
        CURVES_REPR_CONFIGS.find(name)
    }

    /// Returns whether force-refined curves is enabled via env var.
    /// Corresponds to C++ `HdBasisCurves::IsEnabledForceRefinedCurves`.
    pub fn is_enabled_force_refined_curves() -> bool {
        std::env::var("HD_ENABLE_REFINED_CURVES")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Creates a new basis curves primitive.
    ///
    /// # Parameters
    ///
    /// - `id`: Unique scene path identifier for this prim
    /// - `instancer_id`: Optional path to instancer if this prim is instanced
    ///
    /// # Returns
    ///
    /// New curves prim with all dirty bits set for initial sync.
    pub fn new(id: SdfPath, instancer_id: Option<SdfPath>) -> Self {
        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            instancer_id,
            visible: true,
            material_id: None,
            topology: HdBasisCurvesTopology::new(),
        }
    }

    // ------------------------------------------------------------------
    // Delegate convenience wrappers (inline in C++ basisCurves.h)
    // ------------------------------------------------------------------

    /// Convenience: fetch topology from the scene delegate.
    pub fn get_basis_curves_topology(
        &self,
        delegate: &dyn HdSceneDelegate,
    ) -> HdBasisCurvesTopology {
        delegate.get_basis_curves_topology(self.get_id())
    }

    /// Convenience: fetch display style from the scene delegate.
    pub fn get_display_style(&self, delegate: &dyn HdSceneDelegate) -> HdDisplayStyle {
        delegate.get_display_style(self.get_id())
    }

    /// Returns reference to the current curves topology.
    ///
    /// # Returns
    ///
    /// Immutable reference to topology describing curve structure.
    pub fn get_topology(&self) -> &HdBasisCurvesTopology {
        &self.topology
    }

    /// Updates curves topology and marks topology dirty.
    ///
    /// # Parameters
    ///
    /// - `topology`: New topology replacing current configuration
    ///
    /// # Side Effects
    ///
    /// Sets `DIRTY_TOPOLOGY` bit to trigger re-sync with render delegate.
    pub fn set_topology(&mut self, topology: HdBasisCurvesTopology) {
        self.topology = topology;
        self.mark_dirty(Self::DIRTY_TOPOLOGY);
    }
}

impl HdRprim for HdBasisCurves {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn get_instancer_id(&self) -> Option<&SdfPath> {
        self.instancer_id.as_ref()
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        _repr_token: &Token,
    ) {
        // Query delegate for curve data based on dirty bits
        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn get_material_id(&self) -> Option<&SdfPath> {
        self.material_id.as_ref()
    }

    fn get_builtin_primvar_names() -> Vec<Token>
    where
        Self: Sized,
    {
        vec![
            Token::new("points"),
            Token::new("normals"),
            Token::new("widths"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curves_creation() {
        let id = SdfPath::from_string("/Curves").unwrap();
        let curves = HdBasisCurves::new(id.clone(), None);

        assert_eq!(curves.get_id(), &id);
        assert!(curves.is_visible());
    }

    #[test]
    fn test_curves_topology() {
        let mut topology = HdBasisCurvesTopology::new();
        topology.curve_vertex_counts = vec![4, 4, 4]; // 3 cubic curves
        topology.basis = Some(HdCurveBasis::BSpline);
        topology.curve_type = Some(HdCurveType::Cubic);

        assert_eq!(topology.num_curves(), 3);
    }
}
