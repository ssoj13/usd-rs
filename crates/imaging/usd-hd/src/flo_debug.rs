//! Shared debug helpers for `flo.usdz` dirty-trace instrumentation.

use crate::scene_index::observer::DirtiedPrimEntry;
use std::collections::HashMap;
use std::sync::OnceLock;
use usd_sdf::Path as SdfPath;

static FLO_DEBUG_ENABLED: OnceLock<bool> = OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct DirtiedEntriesSummary {
    pub total: usize,
    pub unique_paths: usize,
    pub duplicate_paths: usize,
    pub duplicate_instances: usize,
    pub first_path: String,
}

pub fn flo_debug_enabled() -> bool {
    *FLO_DEBUG_ENABLED.get_or_init(|| std::env::var_os("USD_RS_DEBUG_FLO_DIRTY").is_some())
}

pub fn summarize_dirtied_entries(entries: &[DirtiedPrimEntry]) -> DirtiedEntriesSummary {
    let mut counts: HashMap<SdfPath, usize> = HashMap::new();
    for entry in entries {
        *counts.entry(entry.prim_path.clone()).or_insert(0) += 1;
    }

    DirtiedEntriesSummary {
        total: entries.len(),
        unique_paths: counts.len(),
        duplicate_paths: counts.values().filter(|&&n| n > 1).count(),
        duplicate_instances: counts.values().map(|&n| n.saturating_sub(1)).sum(),
        first_path: entries
            .first()
            .map(|entry| entry.prim_path.to_string())
            .unwrap_or_else(|| "<none>".to_string()),
    }
}
