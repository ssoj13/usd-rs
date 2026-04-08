//! HdRprimSharedData - Data shared across HdReprs, owned by HdRprim.
//!
//! Corresponds to pxr/imaging/hd/rprimSharedData.h.
//! HdDrawItem holds a reference to this; HdRprim owns it.

use super::resource::HdBufferArrayRangeContainer;
use usd_gf::BBox3d;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Face-varying topology and associated primvar names (mesh only).
pub type TopologyToPrimvarVector = Vec<(Vec<i32>, Vec<Token>)>;

/// Data shared across HdReprs, owned by HdRprim.
///
/// Corresponds to C++ `HdRprimSharedData`.
/// HdDrawItem holds a reference; HdRprim owns this.
pub struct HdRprimSharedData {
    /// Buffer array range container.
    pub bar_container: HdBufferArrayRangeContainer,
    /// Bounds for CPU frustum culling.
    pub bounds: BBox3d,
    /// Number of instancer levels applied.
    pub instancer_levels: i32,
    /// Authored/delegate visibility.
    pub visible: bool,
    /// Owning rprim identifier.
    pub rprim_id: SdfPath,
    /// Face-varying topologies and primvar names (mesh only).
    pub fvar_topology_to_primvar_vector: TopologyToPrimvarVector,
}

impl HdRprimSharedData {
    /// Create with BAR container size.
    pub fn new(bar_container_size: usize) -> Self {
        Self {
            bar_container: HdBufferArrayRangeContainer::new(bar_container_size),
            bounds: BBox3d::default(),
            instancer_levels: 0,
            visible: true,
            rprim_id: SdfPath::default(),
            fvar_topology_to_primvar_vector: Vec::new(),
        }
    }

    /// Create with visibility override.
    pub fn with_visibility(bar_container_size: usize, visible: bool) -> Self {
        Self {
            bar_container: HdBufferArrayRangeContainer::new(bar_container_size),
            bounds: BBox3d::default(),
            instancer_levels: 0,
            visible,
            rprim_id: SdfPath::default(),
            fvar_topology_to_primvar_vector: Vec::new(),
        }
    }
}
