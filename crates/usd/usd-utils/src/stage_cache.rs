//! Stage cache singleton for USD stages.
//!
//! Provides a singleton [`StageCache`] for stage reuse across USD clients.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use usd_core::stage_cache::StageCache as UsdStageCache;
use usd_sdf::layer::Layer;
use usd_sdf::path::Path;
use usd_tf::Token;

/// Global singleton instance.
static STAGE_CACHE: OnceLock<Arc<StageCache>> = OnceLock::new();

/// A singleton stage cache for USD stage reuse.
///
/// The `StageCache` provides a simple interface for handling a singleton
/// USD stage cache for use by all USD clients. This allows code from any
/// location to use the same cache to maximize stage reuse.
/// Matches C++ `UsdUtilsStageCache`.
pub struct StageCache {
    /// The underlying USD stage cache (shared so `UsdStageCacheContext` can bind a stable `&StageCache`).
    cache: Arc<UsdStageCache>,
    /// Cache of session layers for variant selections.
    session_layers: RwLock<HashMap<String, Arc<Layer>>>,
}

impl StageCache {
    /// Creates a new stage cache.
    fn new() -> Self {
        Self {
            cache: Arc::new(UsdStageCache::new()),
            session_layers: RwLock::new(HashMap::new()),
        }
    }

    /// Returns the singleton stage cache.
    ///
    /// This is a true process-wide singleton, matching C++ behavior.
    /// The same Arc is returned on every call.
    pub fn get() -> Arc<Self> {
        STAGE_CACHE
            .get_or_init(|| Arc::new(StageCache::new()))
            .clone()
    }

    /// Reference to the underlying `usd_core::StageCache` (thread-safe).
    #[must_use]
    pub fn usd_cache(&self) -> &UsdStageCache {
        self.cache.as_ref()
    }

    /// Clone of the [`Arc`] holding the process-wide USD stage cache (for `UsdStageCacheContext` / Python).
    #[must_use]
    pub fn usd_cache_arc(&self) -> Arc<UsdStageCache> {
        Arc::clone(&self.cache)
    }

    /// Gets or creates a session layer with variant selections for a model.
    ///
    /// Given variant selections as a vector of pairs, constructs a session
    /// layer with overs on the given root model name with the variant
    /// selections, or returns a cached session layer with those opinions.
    pub fn get_session_layer_for_variant_selections(
        &self,
        model_name: &Token,
        variant_selections: &[(String, String)],
    ) -> Option<Arc<Layer>> {
        let path_str = format!("/{}", model_name.as_str());
        let prim_path = Path::from_string(&path_str)?;
        self.get_session_layer_for_variant_selections_impl(&prim_path, variant_selections)
    }

    /// Gets or creates a session layer with variant selections at a specific path.
    pub fn get_session_layer_for_variant_selections_at_path(
        &self,
        prim_path: &Path,
        variant_selections: &[(String, String)],
    ) -> Option<Arc<Layer>> {
        self.get_session_layer_for_variant_selections_impl(prim_path, variant_selections)
    }

    /// Internal implementation for session layer creation.
    fn get_session_layer_for_variant_selections_impl(
        &self,
        prim_path: &Path,
        variant_selections: &[(String, String)],
    ) -> Option<Arc<Layer>> {
        // Create a cache key from the path and selections
        let key = Self::make_cache_key(prim_path, variant_selections);

        // Check if we already have this session layer cached
        {
            let layers = self.session_layers.read().ok()?;
            if let Some(layer) = layers.get(&key) {
                return Some(Arc::clone(layer));
            }
        }

        // Create a new anonymous session layer
        let layer = Layer::create_anonymous(Some("session"));

        // Author the variant selections on the prim spec
        // First, ensure the prim spec exists at the target path
        if let Some(mut prim_spec) = layer.get_prim_at_path(prim_path) {
            // Prim exists, set variant selections
            for (variant_set, variant_name) in variant_selections {
                prim_spec.set_variant_selection(variant_set, variant_name);
            }
        } else if !variant_selections.is_empty() {
            // Need to create the prim spec first
            if let Some(mut prim_spec) = layer.create_prim_spec(
                prim_path,
                usd_sdf::Specifier::Over, // Use 'over' for session layer overrides
                "",
            ) {
                for (variant_set, variant_name) in variant_selections {
                    prim_spec.set_variant_selection(variant_set, variant_name);
                }
            }
        }

        // Cache the layer
        {
            let mut layers = self.session_layers.write().ok()?;
            layers.insert(key, Arc::clone(&layer));
        }

        Some(layer)
    }

    /// Creates a cache key from path and variant selections.
    fn make_cache_key(prim_path: &Path, variant_selections: &[(String, String)]) -> String {
        let mut key = prim_path.as_str().to_string();
        for (set_name, variant_name) in variant_selections {
            key.push_str(&format!(":{{{set_name}={variant_name}}}"));
        }
        key
    }

    /// Clears all cached session layers.
    pub fn clear_session_layers(&self) {
        if let Ok(mut layers) = self.session_layers.write() {
            layers.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = StageCache::get();
        assert!(cache.usd_cache().is_empty());
    }

    #[test]
    fn test_singleton_identity() {
        let a = StageCache::get();
        let b = StageCache::get();
        // Both point to the same allocation
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn test_make_cache_key() {
        let path = Path::from_string("/World/Model").unwrap();
        let selections = vec![
            ("shadingVariant".to_string(), "red".to_string()),
            ("lodVariant".to_string(), "high".to_string()),
        ];

        let key = StageCache::make_cache_key(&path, &selections);
        assert!(key.contains("/World/Model"));
        assert!(key.contains("shadingVariant=red"));
        assert!(key.contains("lodVariant=high"));
    }
}
