//! Stage introspection utilities.
//!
//! Provides utilities for collecting statistics and introspecting USD stages.

use std::collections::HashMap;
use std::sync::Arc;
use usd_core::common::InitialLoadSet;
use usd_core::prim::Prim;
use usd_core::stage::Stage;
use usd_tf::Token;
use usd_vt::dictionary::Dictionary;

/// Token definitions for stage stats keys.
#[derive(Debug, Clone)]
pub struct UsdStageStatsKeys {
    /// Approximate memory usage in MB.
    pub approx_memory_in_mb: Token,
    /// Total prim count.
    pub total_prim_count: Token,
    /// Model count.
    pub model_count: Token,
    /// Instanced model count.
    pub instanced_model_count: Token,
    /// Asset count.
    pub asset_count: Token,
    /// Prototype count.
    pub prototype_count: Token,
    /// Total instance count.
    pub total_instance_count: Token,
    /// Used layer count.
    pub used_layer_count: Token,
    /// Primary stats subdictionary key.
    pub primary: Token,
    /// Prototypes stats subdictionary key.
    pub prototypes: Token,
    /// Prim counts subdictionary key.
    pub prim_counts: Token,
    /// Active prim count.
    pub active_prim_count: Token,
    /// Inactive prim count.
    pub inactive_prim_count: Token,
    /// Pure over count.
    pub pure_over_count: Token,
    /// Instance count.
    pub instance_count: Token,
    /// Prim counts by type subdictionary key.
    pub prim_counts_by_type: Token,
    /// Untyped prim key.
    pub untyped: Token,
}

impl Default for UsdStageStatsKeys {
    fn default() -> Self {
        Self {
            approx_memory_in_mb: Token::from("approxMemoryInMb"),
            total_prim_count: Token::from("totalPrimCount"),
            model_count: Token::from("modelCount"),
            instanced_model_count: Token::from("instancedModelCount"),
            asset_count: Token::from("assetCount"),
            prototype_count: Token::from("prototypeCount"),
            total_instance_count: Token::from("totalInstanceCount"),
            used_layer_count: Token::from("usedLayerCount"),
            primary: Token::from("primary"),
            prototypes: Token::from("prototypes"),
            prim_counts: Token::from("primCounts"),
            active_prim_count: Token::from("activePrimCount"),
            inactive_prim_count: Token::from("inactivePrimCount"),
            pure_over_count: Token::from("pureOverCount"),
            instance_count: Token::from("instanceCount"),
            prim_counts_by_type: Token::from("primCountsByType"),
            untyped: Token::from("untyped"),
        }
    }
}

/// Statistics about a prim subtree.
#[derive(Debug, Clone, Default)]
pub struct PrimSubtreeStats {
    /// Total number of prims.
    pub total_prim_count: usize,
    /// Number of active prims.
    pub active_prim_count: usize,
    /// Number of inactive prims.
    pub inactive_prim_count: usize,
    /// Number of pure overs.
    pub pure_over_count: usize,
    /// Number of instances.
    pub instance_count: usize,
    /// Prim counts by type name.
    pub prim_counts_by_type: HashMap<String, usize>,
}

impl PrimSubtreeStats {
    /// Creates a new empty stats object.
    pub fn new() -> Self {
        Self::default()
    }

    /// Converts to a dictionary for output.
    pub fn to_dictionary(&self) -> Dictionary {
        let mut dict = Dictionary::new();

        let mut prim_counts = Dictionary::new();
        prim_counts.insert("totalPrimCount", self.total_prim_count as i64);
        prim_counts.insert("activePrimCount", self.active_prim_count as i64);
        prim_counts.insert("inactivePrimCount", self.inactive_prim_count as i64);
        prim_counts.insert("pureOverCount", self.pure_over_count as i64);
        prim_counts.insert("instanceCount", self.instance_count as i64);

        dict.insert("primCounts", prim_counts);

        let mut by_type = Dictionary::new();
        for (type_name, count) in &self.prim_counts_by_type {
            by_type.insert(type_name, *count as i64);
        }
        dict.insert("primCountsByType", by_type);

        dict
    }
}

/// Opens a USD stage and computes various statistics.
pub fn compute_usd_stage_stats(root_layer_path: &str) -> Option<(Arc<Stage>, Dictionary)> {
    let stage = Stage::open(root_layer_path, InitialLoadSet::LoadAll).ok()?;

    let mut stats = Dictionary::new();
    let total_prims = compute_usd_stage_stats_from_stage(&stage, &mut stats);

    stats.insert("totalPrimCount", total_prims as i64);

    Some((stage, stats))
}

/// Computes stats on an already-opened USD stage.
pub fn compute_usd_stage_stats_from_stage(stage: &Arc<Stage>, stats: &mut Dictionary) -> usize {
    let mut total_prims = 0;
    let mut model_count = 0;
    let mut instanced_model_count = 0;
    let mut total_instance_count = 0;
    let mut asset_count = 0;

    let mut primary_stats = PrimSubtreeStats::new();

    let pseudo_root = stage.get_pseudo_root();
    traverse_for_stats(&pseudo_root, &mut primary_stats);

    count_special_prims(
        &pseudo_root,
        &mut model_count,
        &mut instanced_model_count,
        &mut total_instance_count,
        &mut asset_count,
    );

    total_prims += primary_stats.total_prim_count;
    stats.insert("primary", primary_stats.to_dictionary());

    let prototypes = stage.get_prototypes();
    if !prototypes.is_empty() {
        stats.insert("prototypeCount", prototypes.len() as i64);

        let mut prototype_stats = PrimSubtreeStats::new();
        for prototype in &prototypes {
            traverse_for_stats(prototype, &mut prototype_stats);
        }

        total_prims += prototype_stats.total_prim_count;
        stats.insert("prototypes", prototype_stats.to_dictionary());
    }

    stats.insert("modelCount", model_count as i64);
    stats.insert("instancedModelCount", instanced_model_count as i64);
    stats.insert("assetCount", asset_count as i64);
    stats.insert("totalInstanceCount", total_instance_count as i64);

    let used_layers = stage.get_used_layers(true);
    stats.insert("usedLayerCount", used_layers.len() as i64);

    total_prims
}

fn traverse_for_stats(root: &Prim, stats: &mut PrimSubtreeStats) {
    stats.total_prim_count += 1;

    if root.is_active() {
        stats.active_prim_count += 1;
    } else {
        stats.inactive_prim_count += 1;
    }

    // Note: is_pure_over would need to check if the prim has only over specifier
    // For now, we skip this stat

    if root.is_instance() {
        stats.instance_count += 1;
    }

    let type_name = root.type_name();
    let type_key = if type_name.is_empty() {
        "untyped".to_string()
    } else {
        type_name.as_str().to_string()
    };

    *stats.prim_counts_by_type.entry(type_key).or_insert(0) += 1;

    for child in root.children() {
        traverse_for_stats(&child, stats);
    }
}

fn count_special_prims(
    root: &Prim,
    model_count: &mut usize,
    instanced_model_count: &mut usize,
    total_instance_count: &mut usize,
    asset_count: &mut usize,
) {
    if root.is_model() {
        *model_count += 1;

        if root.is_instance() {
            *instanced_model_count += 1;
        }
    }

    // Check for assetInfo metadata - indicates this prim has asset info defined
    if root.has_authored_metadata(&usd_tf::Token::new("assetInfo")) {
        *asset_count += 1;
    }

    if root.is_instance() {
        *total_instance_count += 1;
    }

    for child in root.children() {
        count_special_prims(
            &child,
            model_count,
            instanced_model_count,
            total_instance_count,
            asset_count,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prim_subtree_stats_new() {
        let stats = PrimSubtreeStats::new();
        assert_eq!(stats.total_prim_count, 0);
        assert_eq!(stats.active_prim_count, 0);
    }

    #[test]
    fn test_usd_stage_stats_keys() {
        let keys = UsdStageStatsKeys::default();
        assert_eq!(keys.total_prim_count.as_str(), "totalPrimCount");
        assert_eq!(keys.model_count.as_str(), "modelCount");
    }
}
