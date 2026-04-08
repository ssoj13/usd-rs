//! HdTopology - Base trait for topology types.
//!
//! Corresponds to pxr/imaging/hd/topology.h.
//! Mesh, curves, and other geometry topology types implement this trait.

/// Topology identifier type (hash value for instancing).
pub type HdTopologyId = u64;

/// Base trait for geometry topology.
///
/// Corresponds to C++ `HdTopology`.
/// Returns a hash value used for instancing and change detection.
pub trait HdTopology: Send + Sync {
    /// Compute hash value of this topology for instancing.
    fn compute_hash(&self) -> HdTopologyId;
}
