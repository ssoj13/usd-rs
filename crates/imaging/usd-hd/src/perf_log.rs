//! Performance logging for Hydra.
//!
//! Port of pxr/imaging/hd/perfLog.h/cpp
//!
//! Provides HdPerfLog singleton for tracking cache hits/misses and
//! named performance counters. Controlled by enable/disable flag.

use std::collections::HashMap;
use std::sync::Mutex;
use usd_sdf::Path;
use usd_tf::Token;

/// Cache entry tracking hits and misses.
#[derive(Debug, Default)]
struct CacheEntry {
    hits: usize,
    misses: usize,
}

impl CacheEntry {
    fn add_hit(&mut self) {
        self.hits += 1;
    }
    fn add_miss(&mut self) {
        self.misses += 1;
    }
    fn get_hits(&self) -> usize {
        self.hits
    }
    fn get_misses(&self) -> usize {
        self.misses
    }
    fn get_total(&self) -> usize {
        self.hits + self.misses
    }
    fn get_hit_ratio(&self) -> f64 {
        let total = self.get_total();
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
    fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }
}

/// Inner state of HdPerfLog, protected by mutex.
#[derive(Debug, Default)]
struct PerfLogInner {
    cache_map: HashMap<Token, CacheEntry>,
    counter_map: HashMap<Token, f64>,
    resource_registries: Vec<usize>,
    enabled: bool,
}

/// Performance counter monitoring singleton.
///
/// Port of C++ HdPerfLog. Tracks cache hits/misses and named counters.
/// When disabled, mutation operations are no-ops but reads still return
/// last known values.
pub struct HdPerfLog {
    inner: Mutex<PerfLogInner>,
}

impl HdPerfLog {
    /// Get the singleton instance.
    pub fn get_instance() -> &'static HdPerfLog {
        use once_cell::sync::Lazy;
        static INSTANCE: Lazy<HdPerfLog> = Lazy::new(|| {
            let enabled = std::env::var("HD_ENABLE_PERFLOG")
                .map(|v| v == "1")
                .unwrap_or(false);
            HdPerfLog {
                inner: Mutex::new(PerfLogInner {
                    enabled,
                    ..Default::default()
                }),
            }
        });
        &INSTANCE
    }

    /// Enable performance logging.
    pub fn enable(&self) {
        self.inner.lock().unwrap().enabled = true;
    }

    /// Disable performance logging.
    pub fn disable(&self) {
        self.inner.lock().unwrap().enabled = false;
    }

    /// Track a cache hit.
    pub fn add_cache_hit(&self, name: &Token, _id: &Path) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        inner.cache_map.entry(name.clone()).or_default().add_hit();
    }

    /// Track a cache miss.
    pub fn add_cache_miss(&self, name: &Token, _id: &Path) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        inner.cache_map.entry(name.clone()).or_default().add_miss();
    }

    /// Reset a named cache.
    pub fn reset_cache(&self, name: &Token) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        if let Some(entry) = inner.cache_map.get_mut(name) {
            entry.reset();
        }
    }

    /// Get cache hit ratio (hits / total). Returns 0.0 if cache not found.
    pub fn get_cache_hit_ratio(&self, name: &Token) -> f64 {
        let inner = self.inner.lock().unwrap();
        inner
            .cache_map
            .get(name)
            .map(|e| e.get_hit_ratio())
            .unwrap_or(0.0)
    }

    /// Get number of cache hits.
    pub fn get_cache_hits(&self, name: &Token) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.cache_map.get(name).map(|e| e.get_hits()).unwrap_or(0)
    }

    /// Get number of cache misses.
    pub fn get_cache_misses(&self, name: &Token) -> usize {
        let inner = self.inner.lock().unwrap();
        inner
            .cache_map
            .get(name)
            .map(|e| e.get_misses())
            .unwrap_or(0)
    }

    /// Get sorted list of cache names.
    pub fn get_cache_names(&self) -> Vec<Token> {
        let inner = self.inner.lock().unwrap();
        let mut names: Vec<Token> = inner.cache_map.keys().cloned().collect();
        names.sort();
        names
    }

    /// Get sorted list of counter names.
    pub fn get_counter_names(&self) -> Vec<Token> {
        let inner = self.inner.lock().unwrap();
        let mut names: Vec<Token> = inner.counter_map.keys().cloned().collect();
        names.sort();
        names
    }

    /// Increment a named counter by 1.0.
    pub fn increment_counter(&self, name: &Token) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        *inner.counter_map.entry(name.clone()).or_insert(0.0) += 1.0;
    }

    /// Decrement a named counter by 1.0.
    pub fn decrement_counter(&self, name: &Token) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        *inner.counter_map.entry(name.clone()).or_insert(0.0) -= 1.0;
    }

    /// Set a named counter to a value.
    pub fn set_counter(&self, name: &Token, value: f64) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        inner.counter_map.insert(name.clone(), value);
    }

    /// Add value to a named counter.
    pub fn add_counter(&self, name: &Token, value: f64) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        *inner.counter_map.entry(name.clone()).or_insert(0.0) += value;
    }

    /// Subtract value from a named counter.
    pub fn subtract_counter(&self, name: &Token, value: f64) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        *inner.counter_map.entry(name.clone()).or_insert(0.0) -= value;
    }

    /// Get current value of a named counter. Returns 0.0 if not found.
    pub fn get_counter(&self, name: &Token) -> f64 {
        let inner = self.inner.lock().unwrap();
        inner.counter_map.get(name).copied().unwrap_or(0.0)
    }

    /// Reset all counter values to 0.0 (does not reset cache counters).
    pub fn reset_counters(&self) {
        let mut inner = self.inner.lock().unwrap();
        if !inner.enabled {
            return;
        }
        for value in inner.counter_map.values_mut() {
            *value = 0.0;
        }
    }

    /// Add a resource registry for tracking.
    pub fn add_resource_registry(&self, registry_id: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.resource_registries.push(registry_id);
    }

    /// Remove a resource registry from tracking.
    pub fn remove_resource_registry(&self, registry_id: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.resource_registries.retain(|&id| id != registry_id);
    }

    /// Get tracked resource registries.
    pub fn get_resource_registries(&self) -> Vec<usize> {
        let inner = self.inner.lock().unwrap();
        inner.resource_registries.clone()
    }
}

// --- Macros that delegate to singleton ---

/// Trace function scope (no-op, use `tracing` crate for real instrumentation).
#[macro_export]
macro_rules! hd_trace_function {
    () => {};
}

/// Trace scope with tag (no-op).
#[macro_export]
macro_rules! hd_trace_scope {
    ($tag:expr) => {};
}

/// Track cache hit.
#[macro_export]
macro_rules! hd_perf_cache_hit {
    ($name:expr, $id:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().add_cache_hit($name, $id);
    };
    ($name:expr, $id:expr, $tag:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().add_cache_hit($name, $id);
    };
}

/// Track cache miss.
#[macro_export]
macro_rules! hd_perf_cache_miss {
    ($name:expr, $id:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().add_cache_miss($name, $id);
    };
    ($name:expr, $id:expr, $tag:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().add_cache_miss($name, $id);
    };
}

/// Increment performance counter.
#[macro_export]
macro_rules! hd_perf_counter_incr {
    ($name:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().increment_counter($name);
    };
}

/// Decrement performance counter.
#[macro_export]
macro_rules! hd_perf_counter_decr {
    ($name:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().decrement_counter($name);
    };
}

/// Set performance counter.
#[macro_export]
macro_rules! hd_perf_counter_set {
    ($name:expr, $value:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().set_counter($name, $value as f64);
    };
}

/// Add to performance counter.
#[macro_export]
macro_rules! hd_perf_counter_add {
    ($name:expr, $value:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().add_counter($name, $value as f64);
    };
}

/// Subtract from performance counter.
#[macro_export]
macro_rules! hd_perf_counter_subtract {
    ($name:expr, $value:expr) => {
        $crate::perf_log::HdPerfLog::get_instance().subtract_counter($name, $value as f64);
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_perf_macros_compile() {
        hd_trace_function!();
        hd_trace_scope!("test");
    }
}
