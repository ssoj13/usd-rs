//! HdPoints - Point cloud primitive.
//!
//! Represents a point cloud in Hydra. Each point can have:
//! - Position
//! - Width/radius
//! - Color
//! - Custom primvars
//!
//! Points can be rendered as:
//! - Screen-space circles/squares
//! - World-space spheres
//! - Oriented disks

use once_cell::sync::Lazy;

use super::{HdPointsReprDesc, HdRenderParam, HdRprim, HdSceneDelegate, ReprDescConfigs};
use crate::scene_delegate::HdDisplayStyle;
use crate::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Global repr config for points (1 descriptor per repr).
static POINTS_REPR_CONFIGS: Lazy<ReprDescConfigs<HdPointsReprDesc, 1>> =
    Lazy::new(ReprDescConfigs::new);

/// Point cloud primitive.
///
/// Represents a collection of points for rendering point clouds,
/// particle systems, and scatter distributions.
#[derive(Debug)]
pub struct HdPoints {
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

    /// Number of points.
    num_points: usize,
}

impl HdPoints {
    // ------------------------------------------------------------------
    // Static repr configuration
    // ------------------------------------------------------------------

    /// Configure the geometric style for a repr.
    /// Corresponds to C++ `HdPoints::ConfigureRepr`.
    pub fn configure_repr(name: &Token, desc: HdPointsReprDesc) {
        POINTS_REPR_CONFIGS.add_or_update(name, [desc]);
    }

    /// Look up repr descriptor by name.
    /// Corresponds to C++ `HdPoints::_GetReprDesc`.
    pub fn get_repr_desc(name: &Token) -> Option<[HdPointsReprDesc; 1]> {
        POINTS_REPR_CONFIGS.find(name)
    }

    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Create a new points primitive.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this point cloud
    /// * `instancer_id` - Optional instancer id if points are instanced
    pub fn new(id: SdfPath, instancer_id: Option<SdfPath>) -> Self {
        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            instancer_id,
            visible: true,
            material_id: None,
            num_points: 0,
        }
    }

    // ------------------------------------------------------------------
    // Delegate convenience wrappers (inline in C++ points.h)
    // ------------------------------------------------------------------

    /// Convenience: fetch display style from the scene delegate.
    pub fn get_display_style(&self, delegate: &dyn HdSceneDelegate) -> HdDisplayStyle {
        delegate.get_display_style(self.get_id())
    }

    /// Get number of points.
    pub fn get_num_points(&self) -> usize {
        self.num_points
    }

    /// Set number of points.
    pub fn set_num_points(&mut self, count: usize) {
        if self.num_points != count {
            self.num_points = count;
            self.mark_dirty(Self::DIRTY_TOPOLOGY);
        }
    }
}

impl HdRprim for HdPoints {
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
        // Query delegate for point data based on dirty bits

        if (*dirty_bits & Self::DIRTY_TOPOLOGY) != 0 {
            // Query point count from delegate
            // self.num_points = delegate.get_point_count(self.get_id());
        }

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
        // Matches C++ HdPoints::GetBuiltinPrimvarNames (points.cpp:26-31).
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
    fn test_points_creation() {
        let id = SdfPath::from_string("/Points").unwrap();
        let points = HdPoints::new(id.clone(), None);

        assert_eq!(points.get_id(), &id);
        assert!(points.is_visible());
        assert_eq!(points.get_num_points(), 0);
    }

    #[test]
    fn test_points_count() {
        let mut points = HdPoints::new(SdfPath::from_string("/Points").unwrap(), None);

        points.set_num_points(1000);
        assert_eq!(points.get_num_points(), 1000);
        assert!(points.is_dirty_bits(HdPoints::DIRTY_TOPOLOGY));
    }
}
