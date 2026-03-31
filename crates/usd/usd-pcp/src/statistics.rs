//! PCP Statistics.
//!
//! Provides functions for printing statistics about PCP caches and prim indices.
//! Useful for debugging and performance analysis.
//!
//! # C++ Parity
//!
//! This is a port of `pxr/usd/pcp/statistics.h` and `statistics.cpp`.

use std::io::Write;

use crate::{ArcType, Cache, NodeRef, PrimIndex};

/// Statistics about a prim index.
#[derive(Clone, Debug, Default)]
pub struct PrimIndexStatistics {
    /// Total number of nodes in the index.
    pub node_count: usize,
    /// Number of culled nodes.
    pub culled_node_count: usize,
    /// Number of nodes with specs.
    pub nodes_with_specs: usize,
    /// Counts by arc type.
    pub arc_type_counts: ArcTypeCounts,
    /// Maximum depth of the graph.
    pub max_depth: usize,
}

/// Counts of nodes by arc type.
#[derive(Clone, Debug, Default)]
pub struct ArcTypeCounts {
    /// Number of root arcs.
    pub root: usize,
    /// Number of inherit arcs.
    pub inherit: usize,
    /// Number of specialize arcs.
    pub specialize: usize,
    /// Number of reference arcs.
    pub reference: usize,
    /// Number of payload arcs.
    pub payload: usize,
    /// Number of variant arcs.
    pub variant: usize,
    /// Number of relocate arcs.
    pub relocate: usize,
}

impl ArcTypeCounts {
    /// Increments the count for the given arc type.
    pub fn increment(&mut self, arc_type: ArcType) {
        match arc_type {
            ArcType::Root => self.root += 1,
            ArcType::Inherit => self.inherit += 1,
            ArcType::Specialize => self.specialize += 1,
            ArcType::Reference => self.reference += 1,
            ArcType::Payload => self.payload += 1,
            ArcType::Variant => self.variant += 1,
            ArcType::Relocate => self.relocate += 1,
        }
    }
}

/// Statistics about a cache.
#[derive(Clone, Debug, Default)]
pub struct CacheStatistics {
    /// Number of prim indices in the cache.
    pub prim_index_count: usize,
    /// Number of layer stacks in the cache.
    pub layer_stack_count: usize,
    /// Total number of nodes across all prim indices.
    pub total_node_count: usize,
    /// Total number of culled nodes.
    pub total_culled_node_count: usize,
    /// Aggregate arc type counts.
    pub arc_type_counts: ArcTypeCounts,
}

// ============================================================================
// Public API
// ============================================================================

/// Prints statistics about the cache to the writer.
pub fn print_cache_statistics<W: Write>(cache: &Cache, out: &mut W) -> std::io::Result<()> {
    let stats = collect_cache_statistics(cache);

    writeln!(out, "PCP Cache Statistics")?;
    writeln!(out, "====================")?;
    writeln!(out)?;
    writeln!(out, "Prim Indices:     {}", stats.prim_index_count)?;
    writeln!(out, "Layer Stacks:     {}", stats.layer_stack_count)?;
    writeln!(out)?;
    writeln!(out, "Total Nodes:      {}", stats.total_node_count)?;
    writeln!(out, "Culled Nodes:     {}", stats.total_culled_node_count)?;
    writeln!(out)?;
    writeln!(out, "Arc Type Breakdown:")?;
    writeln!(out, "  Root:       {}", stats.arc_type_counts.root)?;
    writeln!(out, "  Inherit:    {}", stats.arc_type_counts.inherit)?;
    writeln!(out, "  Specialize: {}", stats.arc_type_counts.specialize)?;
    writeln!(out, "  Reference:  {}", stats.arc_type_counts.reference)?;
    writeln!(out, "  Payload:    {}", stats.arc_type_counts.payload)?;
    writeln!(out, "  Variant:    {}", stats.arc_type_counts.variant)?;
    writeln!(out, "  Relocate:   {}", stats.arc_type_counts.relocate)?;

    Ok(())
}

/// Prints statistics about the prim index to the writer.
pub fn print_prim_index_statistics<W: Write>(
    prim_index: &PrimIndex,
    out: &mut W,
) -> std::io::Result<()> {
    let stats = collect_prim_index_statistics(prim_index);

    writeln!(out, "PCP PrimIndex Statistics")?;
    writeln!(out, "========================")?;
    writeln!(out)?;
    writeln!(out, "Path: {}", prim_index.path().as_str())?;
    writeln!(out)?;
    writeln!(out, "Nodes:            {}", stats.node_count)?;
    writeln!(out, "Culled Nodes:     {}", stats.culled_node_count)?;
    writeln!(out, "Nodes with Specs: {}", stats.nodes_with_specs)?;
    writeln!(out, "Max Depth:        {}", stats.max_depth)?;
    writeln!(out)?;
    writeln!(out, "Arc Type Breakdown:")?;
    writeln!(out, "  Root:       {}", stats.arc_type_counts.root)?;
    writeln!(out, "  Inherit:    {}", stats.arc_type_counts.inherit)?;
    writeln!(out, "  Specialize: {}", stats.arc_type_counts.specialize)?;
    writeln!(out, "  Reference:  {}", stats.arc_type_counts.reference)?;
    writeln!(out, "  Payload:    {}", stats.arc_type_counts.payload)?;
    writeln!(out, "  Variant:    {}", stats.arc_type_counts.variant)?;
    writeln!(out, "  Relocate:   {}", stats.arc_type_counts.relocate)?;

    Ok(())
}

/// Returns statistics as a formatted string.
pub fn cache_statistics_string(cache: &Cache) -> String {
    let mut output = Vec::new();
    if print_cache_statistics(cache, &mut output).is_ok() {
        String::from_utf8_lossy(&output).into_owned()
    } else {
        String::new()
    }
}

/// Returns prim index statistics as a formatted string.
pub fn prim_index_statistics_string(prim_index: &PrimIndex) -> String {
    let mut output = Vec::new();
    if print_prim_index_statistics(prim_index, &mut output).is_ok() {
        String::from_utf8_lossy(&output).into_owned()
    } else {
        String::new()
    }
}

// ============================================================================
// Statistics Collection
// ============================================================================

/// Collects statistics about a cache.
pub fn collect_cache_statistics(cache: &Cache) -> CacheStatistics {
    let mut stats = CacheStatistics::default();

    // Count layer stacks
    stats.layer_stack_count = cache.layer_stack_cache_size();

    // Count prim indices and aggregate stats
    cache.for_each_prim_index(|prim_index| {
        stats.prim_index_count += 1;
        let prim_stats = collect_prim_index_statistics(prim_index);

        stats.total_node_count += prim_stats.node_count;
        stats.total_culled_node_count += prim_stats.culled_node_count;

        stats.arc_type_counts.root += prim_stats.arc_type_counts.root;
        stats.arc_type_counts.inherit += prim_stats.arc_type_counts.inherit;
        stats.arc_type_counts.specialize += prim_stats.arc_type_counts.specialize;
        stats.arc_type_counts.reference += prim_stats.arc_type_counts.reference;
        stats.arc_type_counts.payload += prim_stats.arc_type_counts.payload;
        stats.arc_type_counts.variant += prim_stats.arc_type_counts.variant;
        stats.arc_type_counts.relocate += prim_stats.arc_type_counts.relocate;
    });

    stats
}

/// Collects statistics about a prim index.
pub fn collect_prim_index_statistics(prim_index: &PrimIndex) -> PrimIndexStatistics {
    let mut stats = PrimIndexStatistics::default();

    if !prim_index.is_valid() {
        return stats;
    }

    let root = prim_index.root_node();
    if !root.is_valid() {
        return stats;
    }

    collect_node_statistics(&root, 0, &mut stats);

    stats
}

/// Recursively collects statistics from a node.
fn collect_node_statistics(node: &NodeRef, depth: usize, stats: &mut PrimIndexStatistics) {
    stats.node_count += 1;
    stats.max_depth = stats.max_depth.max(depth);

    if node.is_culled() {
        stats.culled_node_count += 1;
    }

    if node.has_specs() {
        stats.nodes_with_specs += 1;
    }

    stats.arc_type_counts.increment(node.arc_type());

    // Recurse to children
    for child in node.children() {
        collect_node_statistics(&child, depth + 1, stats);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_type_counts_increment() {
        let mut counts = ArcTypeCounts::default();
        assert_eq!(counts.root, 0);

        counts.increment(ArcType::Root);
        assert_eq!(counts.root, 1);

        counts.increment(ArcType::Inherit);
        counts.increment(ArcType::Inherit);
        assert_eq!(counts.inherit, 2);
    }

    #[test]
    fn test_prim_index_statistics_invalid() {
        let prim_index = PrimIndex::new();
        let stats = collect_prim_index_statistics(&prim_index);
        assert_eq!(stats.node_count, 0);
    }

    #[test]
    fn test_prim_index_statistics_string_invalid() {
        let prim_index = PrimIndex::new();
        let s = prim_index_statistics_string(&prim_index);
        assert!(s.contains("PrimIndex Statistics"));
    }

    #[test]
    fn test_cache_statistics_default() {
        let stats = CacheStatistics::default();
        assert_eq!(stats.prim_index_count, 0);
        assert_eq!(stats.layer_stack_count, 0);
    }
}
