//! Labels Query utility.
//!
//! Query utility for computing semantic labels with caching.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdSemantics/labelsQuery.h` and `labelsQuery.cpp`

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use usd_core::Prim;
use usd_gf::Interval;
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;

use super::labels_api::LabelsAPI;

/// Query time specification - either a single time code or a GfInterval.
///
/// Matches C++ `std::variant<GfInterval, UsdTimeCode>`.
#[derive(Debug, Clone)]
pub enum QueryTime {
    /// Single time code query
    TimeCode(TimeCode),
    /// Interval query - computes union of values across the interval
    Interval(Interval),
}

/// Labels query utility with thread-safe caching.
///
/// Queries a prim's labels for a specified taxonomy and time from
/// `UsdSemanticsLabelsAPI`. Caches reads for performance. Discard the
/// query when stage state changes.
///
/// Matches C++ `UsdSemanticsLabelsQuery`.
pub struct LabelsQuery {
    taxonomy: Token,
    time: QueryTime,
    /// Cache: prim path -> set of label tokens
    cached_labels: RwLock<HashMap<Path, HashSet<Token>>>,
}

impl LabelsQuery {
    /// Construct a query for a taxonomy at a single time code.
    ///
    /// Panics (coding error) if `taxonomy` is empty.
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery(taxonomy, timeCode)`.
    pub fn new_at_time(taxonomy: Token, time_code: TimeCode) -> Self {
        if taxonomy.is_empty() {
            // C++ uses TF_CODING_ERROR - log error and return with empty taxonomy
            eprintln!("[TF_CODING_ERROR] UsdSemanticsLabelsQuery created with empty taxonomy.");
        }
        Self {
            taxonomy,
            time: QueryTime::TimeCode(time_code),
            cached_labels: RwLock::new(HashMap::new()),
        }
    }

    /// Construct a query for a taxonomy over an interval.
    ///
    /// Panics (coding error) if `taxonomy` is empty.
    /// Falls back to `TimeCode::DEFAULT` if `interval` is empty.
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery(taxonomy, interval)`.
    pub fn new_over_interval(taxonomy: Token, interval: Interval) -> Self {
        if taxonomy.is_empty() {
            eprintln!("[TF_CODING_ERROR] UsdSemanticsLabelsQuery created with empty taxonomy.");
        }
        // C++ falls back to UsdTimeCode::Default() for empty intervals
        let time = if interval.is_empty() {
            eprintln!("UsdSemanticsLabelsQuery created with empty interval.");
            QueryTime::TimeCode(TimeCode::DEFAULT)
        } else {
            QueryTime::Interval(interval)
        };
        Self {
            taxonomy,
            time,
            cached_labels: RwLock::new(HashMap::new()),
        }
    }

    /// Get the taxonomy being queried.
    ///
    /// Matches C++ `GetTaxonomy()`.
    pub fn get_taxonomy(&self) -> &Token {
        &self.taxonomy
    }

    /// Get the query time.
    ///
    /// Matches C++ `GetTime()`.
    pub fn get_time(&self) -> &QueryTime {
        &self.time
    }

    /// Compute unique labels directly applied to this prim, sorted.
    ///
    /// Returns empty vec if the prim has no applicable labels.
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery::ComputeUniqueDirectLabels(prim)`.
    pub fn compute_unique_direct_labels(&self, prim: &Prim) -> Vec<Token> {
        if !self.populate_labels(prim) {
            return vec![];
        }

        let cache = self.cached_labels.read().expect("rwlock poisoned");
        let path = prim.get_path();
        match cache.get(path) {
            None => vec![],
            Some(set) => {
                let mut result: Vec<Token> = set.iter().cloned().collect();
                result.sort_by(|a, b| a.as_str().cmp(b.as_str()));
                result
            }
        }
    }

    /// Compute unique labels including those inherited from ancestors, sorted.
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery::ComputeUniqueInheritedLabels(prim)`.
    pub fn compute_unique_inherited_labels(&self, prim: &Prim) -> Vec<Token> {
        if !self.populate_inherited_labels(prim) {
            return vec![];
        }

        let mut unique: HashSet<Token> = HashSet::new();
        {
            let cache = self.cached_labels.read().expect("rwlock poisoned");
            for path in prim.get_path().get_ancestors_range() {
                if let Some(set) = cache.get(&path) {
                    unique.extend(set.iter().cloned());
                }
            }
        }

        let mut result: Vec<Token> = unique.into_iter().collect();
        result.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        result
    }

    /// Check if a label is directly applied to this prim.
    ///
    /// Uses the cache directly for O(1) lookup (not linear scan through sorted vec).
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery::HasDirectLabel(prim, label)`.
    pub fn has_direct_label(&self, prim: &Prim, label: &Token) -> bool {
        // Early exit if prim has no labels
        if !self.populate_labels(prim) {
            return false;
        }

        // O(1) HashSet lookup via cache
        let cache = self.cached_labels.read().expect("rwlock poisoned");
        let path = prim.get_path();
        cache
            .get(path)
            .map(|set| set.contains(label))
            .unwrap_or(false)
    }

    /// Check if a label is applied to this prim or any ancestor.
    ///
    /// Uses cached HashSet for O(1) per-ancestor lookup.
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery::HasInheritedLabel(prim, label)`.
    pub fn has_inherited_label(&self, prim: &Prim, label: &Token) -> bool {
        if !self.populate_inherited_labels(prim) {
            return false;
        }

        let cache = self.cached_labels.read().expect("rwlock poisoned");
        prim.get_path()
            .get_ancestors_range()
            .into_iter()
            .any(|path| {
                cache
                    .get(&path)
                    .map(|set| set.contains(label))
                    .unwrap_or(false)
            })
    }

    // =========================================================================
    // Internal - cache population
    // =========================================================================

    /// Populate the cache for a single prim. Returns true if the prim has labels.
    ///
    /// Checks: not pseudo-root, has the API applied, schema is valid.
    /// Uses double-checked locking (read -> compute -> try-insert).
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery::_PopulateLabels(prim)`.
    fn populate_labels(&self, prim: &Prim) -> bool {
        // P0-4: skip pseudo-root
        if prim.is_pseudo_root() {
            return false;
        }

        // P0-4: only query prims that actually have the API applied
        let schema_type = Token::new(LabelsAPI::SCHEMA_TYPE_NAME);
        if !prim.has_api_instance(&schema_type, &self.taxonomy) {
            return false;
        }

        let api = LabelsAPI::new(prim.clone(), self.taxonomy.clone());
        if !api.is_valid() {
            return false;
        }

        // Double-checked locking: check cache first with read lock
        {
            let cache = self.cached_labels.read().expect("rwlock poisoned");
            if cache.contains_key(prim.get_path()) {
                return true;
            }
        }

        // Compute labels without holding any lock
        let labels = self.compute_labels_for_api(&api);

        // Write lock: use entry() to avoid overwriting if another thread inserted first
        // P1-4: use entry().or_insert() instead of insert() to avoid clobbering
        {
            let mut cache = self.cached_labels.write().expect("rwlock poisoned");
            cache.entry(prim.get_path().clone()).or_insert(labels);
        }

        true
    }

    /// Populate the cache for a prim and all its ancestors.
    ///
    /// Returns true if any ancestor (including self) has labels.
    ///
    /// Matches C++ `UsdSemanticsLabelsQuery::_PopulateInheritedLabels(prim)`.
    fn populate_inherited_labels(&self, prim: &Prim) -> bool {
        let Some(stage) = prim.stage() else {
            return false;
        };

        let mut has_any = false;
        // Must populate EVERY ancestor - cannot short-circuit
        for path in prim.get_path().get_ancestors_range() {
            if let Some(ancestor) = stage.get_prim_at_path(&path) {
                if self.populate_labels(&ancestor) {
                    has_any = true;
                }
            }
        }
        has_any
    }

    /// Compute the label set for a LabelsAPI instance based on query time.
    ///
    /// For TimeCode: queries at that single time.
    /// For Interval: unions all values across time samples in the interval.
    fn compute_labels_for_api(&self, api: &LabelsAPI) -> HashSet<Token> {
        let mut labels = HashSet::new();

        let attr = match api.get_labels_attr() {
            Some(a) => a,
            None => {
                eprintln!(
                    "Labels attribute undefined at prim: {:?}",
                    api.get_prim().get_path()
                );
                return labels;
            }
        };

        match &self.time {
            QueryTime::TimeCode(tc) => {
                // Single time code: query once
                if let Some(value) = attr.get(*tc) {
                    if let Some(tokens) = value.as_vec_clone::<Token>() {
                        labels.extend(tokens);
                    }
                }
            }
            QueryTime::Interval(interval) => {
                // P0-6: check finiteness of min before using it
                // C++: IsMinFinite() ? GetMin() : UsdTimeCode::EarliestTime().GetValue()
                // UsdTimeCode::EarliestTime() stores f64::MIN
                let earliest_val = f64::MIN;
                let min_val = if interval.is_min_finite() {
                    interval.get_min()
                } else {
                    earliest_val
                };

                let mut times =
                    attr.get_time_samples_in_interval(interval.get_min(), interval.get_max());

                // Ensure the effective minimum is always queried
                if times.is_empty() || times.first().copied() != Some(min_val) {
                    times.push(min_val);
                }

                for t in &times {
                    let tc = TimeCode::from(*t);
                    if let Some(value) = attr.get(tc) {
                        if let Some(tokens) = value.as_vec_clone::<Token>() {
                            labels.extend(tokens);
                        }
                    }
                }
            }
        }

        labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_creation_at_time() {
        let query = LabelsQuery::new_at_time(Token::new("category"), TimeCode::DEFAULT);
        assert_eq!(query.get_taxonomy().as_str(), "category");
        matches!(query.get_time(), QueryTime::TimeCode(_));
    }

    #[test]
    fn test_interval_query() {
        let interval = Interval::closed(0.0, 100.0);
        let query = LabelsQuery::new_over_interval(Token::new("objects"), interval.clone());
        match query.get_time() {
            QueryTime::Interval(i) => {
                assert_eq!(i.get_min(), 0.0);
                assert_eq!(i.get_max(), 100.0);
            }
            _ => panic!("Expected interval"),
        }
    }

    #[test]
    fn test_empty_interval_falls_back_to_default_time() {
        // P0-5: empty interval should fall back to TimeCode::Default
        // open(5,5) is an empty interval because no x satisfies 5 < x < 5
        let empty = Interval::open(5.0, 5.0);
        assert!(empty.is_empty());
        let query = LabelsQuery::new_over_interval(Token::new("test"), empty);
        matches!(query.get_time(), QueryTime::TimeCode(_));
    }

    #[test]
    fn test_empty_taxonomy_logs_error_at_time() {
        // C++ TF_CODING_ERROR logs error, does not abort
        let query = LabelsQuery::new_at_time(Token::new(""), TimeCode::DEFAULT);
        assert!(query.get_taxonomy().is_empty());
    }

    #[test]
    fn test_empty_taxonomy_logs_error_interval() {
        // C++ TF_CODING_ERROR logs error, does not abort
        let query = LabelsQuery::new_over_interval(Token::new(""), Interval::closed(0.0, 1.0));
        assert!(query.get_taxonomy().is_empty());
    }
}
