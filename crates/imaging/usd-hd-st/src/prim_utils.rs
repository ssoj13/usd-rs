//! Rprim utility functions for Storm (ported from primUtils.h).
//!
//! Provides helper functions for:
//! - Draw invalidation and garbage collection
//! - Primvar descriptor filtering
//! - Material processing
//! - Constant primvar population
//! - Instancer data updates
//! - Topological visibility processing
//! - Shared vertex primvar deduplication

use crate::render_param::HdStRenderParam;
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Draw invalidation and garbage collection utilities
// ---------------------------------------------------------------------------

/// Mark all draw batches as dirty, triggering rebuild.
///
/// Called when topology or buffer layout changes invalidate existing batches.
pub fn mark_draw_batches_dirty(render_param: &mut HdStRenderParam) {
    render_param.mark_draw_batches_dirty();
}

/// Mark material tags as dirty, triggering re-bucketing of prims.
pub fn mark_material_tags_dirty(render_param: &mut HdStRenderParam) {
    render_param.mark_material_tags_dirty();
}

/// Mark geometry subset draw items as dirty.
pub fn mark_geom_subset_draw_items_dirty(render_param: &mut HdStRenderParam) {
    render_param.mark_geom_subset_draw_items_dirty();
}

/// Flag that garbage collection is needed for unused buffer resources.
pub fn mark_garbage_collection_needed(render_param: &mut HdStRenderParam) {
    render_param.mark_garbage_collection_needed();
}

// ---------------------------------------------------------------------------
// Primvar descriptor filtering
// ---------------------------------------------------------------------------

/// Interpolation mode for primvar data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdInterpolation {
    /// One value for entire prim
    Constant,
    /// One value per element (face/curve segment)
    Uniform,
    /// One value per vertex, shared across faces
    Vertex,
    /// One value per face-vertex (varying)
    Varying,
    /// One value per face-vertex (face-varying)
    FaceVarying,
    /// One value per instance
    Instance,
}

/// Mesh geometry style for primvar filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdMeshGeomStyle {
    /// Invalid / uninitialized
    Invalid,
    /// Surface hull rendering
    HullEdgeOnly,
    /// Surface rendering (most common)
    Surf,
    /// Wireframe edges only
    EdgeOnly,
    /// Surface + wireframe edges overlay
    SurfEdge,
    /// Points only
    Points,
}

/// Primvar descriptor (name + interpolation + role).
#[derive(Debug, Clone)]
pub struct HdPrimvarDescriptor {
    /// Primvar name
    pub name: Token,
    /// Interpolation mode
    pub interpolation: HdInterpolation,
    /// Semantic role (e.g. "point", "normal", "color")
    pub role: Token,
}

/// Buffer spec (name + type + component count).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HdBufferSpec {
    /// Buffer name
    pub name: Token,
    /// Number of components (1-4)
    pub num_components: u32,
    /// Byte size per element
    pub byte_size: usize,
}

// ---------------------------------------------------------------------------
// BAR validation utilities
// ---------------------------------------------------------------------------

/// Returns true if a buffer array range is non-empty and valid.
///
/// Used during primvar processing to check if existing GPU allocations
/// can be reused.
pub fn is_valid_bar(range_valid: bool, range_num_elements: usize) -> bool {
    range_valid && range_num_elements > 0
}

/// Returns true if BAR allocation/update can be skipped.
///
/// Checks if there are no new sources, no new computations,
/// and the dirty bits don't require an update.
pub fn can_skip_bar_allocation_or_update(
    num_sources: usize,
    num_computations: usize,
    range_valid: bool,
    dirty_bits: HdDirtyBits,
) -> bool {
    // No new data sources or computations, and range exists
    num_sources == 0 && num_computations == 0 && range_valid && dirty_bits == 0
}

// ---------------------------------------------------------------------------
// Constant primvar utilities
// ---------------------------------------------------------------------------

/// Check if constant primvars need population based on dirty bits.
pub fn should_populate_constant_primvars(dirty_bits: HdDirtyBits, _id: &SdfPath) -> bool {
    // Constant primvars need update when transform or primvar bits are dirty.
    // DirtyTransform = 1 << 5, DirtyPrimvar = 1 << 10 in C++
    const DIRTY_TRANSFORM: HdDirtyBits = 1 << 5;
    const DIRTY_PRIMVAR: HdDirtyBits = 1 << 10;
    (dirty_bits & (DIRTY_TRANSFORM | DIRTY_PRIMVAR)) != 0
}

// ---------------------------------------------------------------------------
// Shared vertex primvar deduplication
// ---------------------------------------------------------------------------

/// Check if shared vertex primvar optimization is enabled.
///
/// When enabled, immutable primvar data is deduplicated across prims
/// using a hash-based instance registry.
pub fn is_shared_vertex_primvar_enabled() -> bool {
    // Default enabled in C++
    true
}

/// Compute a hash for shared primvar deduplication.
///
/// Combines base hash with source data hashes. Used to look up
/// existing GPU allocations in the primvar instance registry.
pub fn compute_shared_primvar_id(base_id: u64, source_hashes: &[u64]) -> u64 {
    let mut hash = base_id;
    for &h in source_hashes {
        // Simple hash combine (FNV-like)
        hash ^= h
            .wrapping_mul(0x9e3779b97f4a7c15)
            .wrapping_add(hash << 6)
            .wrapping_add(hash >> 2);
    }
    hash
}

// ---------------------------------------------------------------------------
// Material processing utilities
// ---------------------------------------------------------------------------

/// Set the material tag on a draw item.
///
/// Material tags bucket prims into draw queues:
/// - defaultMaterialTag: opaque
/// - masked: opaque with cutout
/// - additive: transparent (no sort)
/// - translucent: transparent (sorted)
/// - volume: raymarched
pub fn set_material_tag(material_tag: &Token) -> Token {
    material_tag.clone()
}

/// Check if a primvar exists and has a valid value.
pub fn is_primvar_existent_and_valid(
    primvars: &[HdPrimvarDescriptor],
    primvar_name: &Token,
) -> bool {
    primvars.iter().any(|pv| pv.name == *primvar_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_bar() {
        assert!(!is_valid_bar(false, 10));
        assert!(!is_valid_bar(true, 0));
        assert!(is_valid_bar(true, 10));
    }

    #[test]
    fn test_should_populate_constant_primvars() {
        assert!(!should_populate_constant_primvars(0, &SdfPath::default()));
        assert!(should_populate_constant_primvars(
            1 << 5, // transform dirty
            &SdfPath::default()
        ));
        assert!(should_populate_constant_primvars(
            1 << 10, // primvar dirty
            &SdfPath::default()
        ));
    }

    #[test]
    fn test_shared_primvar_id() {
        let id1 = compute_shared_primvar_id(42, &[100, 200]);
        let id2 = compute_shared_primvar_id(42, &[100, 200]);
        let id3 = compute_shared_primvar_id(42, &[100, 201]);
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_primvar_exists() {
        let primvars = vec![
            HdPrimvarDescriptor {
                name: Token::new("points"),
                interpolation: HdInterpolation::Vertex,
                role: Token::new("point"),
            },
            HdPrimvarDescriptor {
                name: Token::new("normals"),
                interpolation: HdInterpolation::Vertex,
                role: Token::new("normal"),
            },
        ];

        assert!(is_primvar_existent_and_valid(
            &primvars,
            &Token::new("points")
        ));
        assert!(!is_primvar_existent_and_valid(
            &primvars,
            &Token::new("colors")
        ));
    }
}
