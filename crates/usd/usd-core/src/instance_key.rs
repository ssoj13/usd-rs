//! Usd_InstanceKey - key for identifying instanceable prim indexes.
//!
//! Port of pxr/usd/usd/instanceKey.h
//!
//! A key that uniquely identifies an instanceable prim index based on
//! its composition structure.

use crate::clip_set_definition::ClipSetDefinition;
use crate::load_rules::{Rule, StageLoadRules};
use crate::population_mask::StagePopulationMask;
use std::hash::{Hash, Hasher};
use usd_pcp::{InstanceKey as PcpInstanceKey, PrimIndex};
use usd_sdf::Path;

// ============================================================================
// InstanceKey
// ============================================================================

/// Key that uniquely identifies an instanceable prim index.
///
/// Matches C++ `Usd_InstanceKey`.
///
/// This key is based on the composition structure of the prim index.
#[derive(Debug, Clone)]
pub struct InstanceKey {
    /// PCP instance key (from PcpPrimIndex).
    pcp_instance_key: PcpInstanceKey,
    /// Clip set definitions for this instance.
    clip_defs: Vec<ClipSetDefinition>,
    /// Population mask (relative to instance path).
    mask: StagePopulationMask,
    /// Load rules (relative to instance path).
    load_rules: StageLoadRules,
    /// Cached hash value.
    hash: u64,
}

impl InstanceKey {
    /// Creates an empty instance key.
    ///
    /// Matches C++ `Usd_InstanceKey()`.
    pub fn new() -> Self {
        let key = Self {
            pcp_instance_key: PcpInstanceKey::new(),
            clip_defs: Vec::new(),
            mask: StagePopulationMask::all(),
            load_rules: StageLoadRules::new(),
            hash: 0,
        };
        let hash = key.compute_hash();
        Self { hash, ..key }
    }

    /// Creates an instance key from a prim index.
    ///
    /// Matches C++ `Usd_InstanceKey(const PcpPrimIndex& instance, const UsdStagePopulationMask *mask, const UsdStageLoadRules &loadRules)`.
    pub fn from_prim_index(
        index: &PrimIndex,
        mask: Option<&StagePopulationMask>,
        load_rules: &StageLoadRules,
    ) -> Self {
        use crate::clip_set_definition::compute_clip_set_definitions_for_prim_index;

        // Get PCP instance key
        let pcp_instance_key = PcpInstanceKey::from_prim_index(index);

        // Compute clip set definitions
        let mut clip_defs = Vec::new();
        let mut clip_set_names = Vec::new();
        compute_clip_set_definitions_for_prim_index(index, &mut clip_defs, &mut clip_set_names);

        // Make the population mask "relative" to this prim index by removing the
        // index's path prefix from all paths in the mask that it prefixes.
        let instance_path = index.path();
        let mask = if let Some(m) = mask {
            make_mask_relative_to(&instance_path, m)
        } else {
            StagePopulationMask::all()
        };

        // Do the same with the load rules.
        let load_rules = make_load_rules_relative_to(&instance_path, load_rules);

        let key = Self {
            pcp_instance_key,
            clip_defs,
            mask,
            load_rules,
            hash: 0,
        };

        // Compute and cache the hash code.
        let hash = key.compute_hash();
        Self { hash, ..key }
    }

    /// Returns the hash value.
    pub fn get_hash(&self) -> u64 {
        self.hash
    }

    /// Returns the PCP instance key.
    pub fn pcp_instance_key(&self) -> &PcpInstanceKey {
        &self.pcp_instance_key
    }

    /// Returns the clip definitions.
    pub fn clip_defs(&self) -> &[ClipSetDefinition] {
        &self.clip_defs
    }

    /// Returns the population mask.
    pub fn mask(&self) -> &StagePopulationMask {
        &self.mask
    }

    /// Returns the load rules.
    pub fn load_rules(&self) -> &StageLoadRules {
        &self.load_rules
    }

    /// Computes the hash for this instance key.
    ///
    /// Matches C++ `_ComputeHash()`.
    fn compute_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();

        // Hash PCP instance key
        self.pcp_instance_key.hash(&mut hasher);

        // Hash clip definitions
        for clip_def in &self.clip_defs {
            clip_def.get_hash().hash(&mut hasher);
        }

        // Hash mask
        for path in self.mask.get_paths() {
            path.hash(&mut hasher);
        }

        // Hash load rules
        // Get all rules and hash them in a deterministic order
        let mut rules: Vec<_> = self
            .load_rules
            .iter()
            .map(|(p, r)| (p.clone(), *r))
            .collect();
        rules.sort_by(|a, b| a.0.cmp(&b.0));
        for (path, rule) in rules {
            path.hash(&mut hasher);
            format!("{:?}", rule).hash(&mut hasher);
        }

        hasher.finish()
    }
}

impl Default for InstanceKey {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for InstanceKey {
    fn eq(&self, other: &Self) -> bool {
        // First check hash for quick comparison
        if self.hash != other.hash {
            return false;
        }

        // Then check all fields
        self.pcp_instance_key == other.pcp_instance_key &&
        self.clip_defs == other.clip_defs &&
        self.mask.get_paths().len() == other.mask.get_paths().len() &&
        // Compare mask paths
        {
            let self_paths_vec = self.mask.get_paths();
            let other_paths_vec = other.mask.get_paths();
            let mut self_paths: Vec<_> = self_paths_vec.iter().collect();
            let mut other_paths: Vec<_> = other_paths_vec.iter().collect();
            self_paths.sort();
            other_paths.sort();
            self_paths == other_paths
        } &&
        // Compare load rules
        {
            let mut self_rules: Vec<_> = self.load_rules.iter().map(|(p, r)| (p.clone(), *r)).collect();
            let mut other_rules: Vec<_> = other.load_rules.iter().map(|(p, r)| (p.clone(), *r)).collect();
            self_rules.sort_by(|a, b| a.0.cmp(&b.0));
            other_rules.sort_by(|a, b| a.0.cmp(&b.0));
            self_rules == other_rules
        }
    }
}

impl Eq for InstanceKey {}

impl Hash for InstanceKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Makes a population mask relative to the given path.
///
/// Matches C++ `_MakeMaskRelativeTo`.
fn make_mask_relative_to(path: &Path, mask: &StagePopulationMask) -> StagePopulationMask {
    let abs_root = Path::absolute_root();
    let mut mask_paths: Vec<Path> = mask.get_paths().iter().map(|p| (*p).clone()).collect();

    for mask_path in &mut mask_paths {
        if mask_path.has_prefix(path) {
            *mask_path = mask_path
                .replace_prefix(path, &abs_root)
                .unwrap_or_else(Path::empty);
        } else {
            *mask_path = Path::empty();
        }
    }

    // Remove empty paths
    mask_paths.retain(|p| !p.is_empty());

    StagePopulationMask::from_paths(mask_paths)
}

/// Makes load rules relative to the given path.
///
/// Matches C++ `_MakeLoadRulesRelativeTo`.
fn make_load_rules_relative_to(path: &Path, rules: &StageLoadRules) -> StageLoadRules {
    let abs_root = Path::absolute_root();
    let root_rule = rules.get_effective_rule(path);

    let mut elems: Vec<(Path, Rule)> = rules.iter().map(|(p, r)| (p.clone(), *r)).collect();

    for elem in &mut elems {
        if elem.0 == *path {
            elem.0 = abs_root.clone();
            elem.1 = root_rule;
        } else if elem.0.has_prefix(path) {
            elem.0 = elem
                .0
                .replace_prefix(path, &abs_root)
                .unwrap_or_else(Path::empty);
        } else {
            elem.0 = Path::empty();
        }
    }

    // Remove empty paths
    elems.retain(|(p, _)| !p.is_empty());

    // Ensure the first element is the root rule
    elems.sort_by(|a, b| a.0.cmp(&b.0));
    if elems.is_empty() || elems[0].0 != abs_root {
        elems.insert(0, (abs_root, root_rule));
    } else {
        elems[0].1 = root_rule;
    }

    let mut ret = StageLoadRules::new();
    for (p, r) in elems {
        ret.add_rule(p, r);
    }

    ret
}
