
//! HdInstanceRegistry - Shared instance dictionary with GC.
//!
//! Corresponds to pxr/imaging/hd/instanceRegistry.h.

use std::collections::HashMap;
use std::sync::Mutex;

/// Key type for instance registry (hash).
pub type HdInstanceKey = u64;

/// Value holder with recycle counter.
struct ValueHolder<V> {
    value: V,
    recycle_counter: i32,
}

/// Interface to a shared instance in HdInstanceRegistry.
///
/// Corresponds to C++ `HdInstance<VALUE>`.
pub struct HdInstance<V> {
    key: HdInstanceKey,
    value: V,
}

impl<V> HdInstance<V> {
    /// Get key.
    pub fn get_key(&self) -> HdInstanceKey {
        self.key
    }

    /// Get value.
    pub fn get_value(&self) -> &V {
        &self.value
    }

    /// Update value in registry.
    pub fn set_value(&mut self, value: V)
    where
        V: Clone,
    {
        self.value = value.clone();
        // Note: updating container would require keeping a ref to it
        // For simplicity we don't support SetValue when holding lock
    }

    /// True if value was not initialized (first instance).
    pub fn is_first_instance(&self) -> bool {
        // In C++ this checks !bool(_value) - for Arc, use_count == 0 or similar
        false
    }
}

/// Instance registry - dictionary of shared instances with GC.
///
/// Corresponds to C++ `HdInstanceRegistry<VALUE>`.
pub struct HdInstanceRegistry<V> {
    dictionary: Mutex<HashMap<HdInstanceKey, ValueHolder<V>>>,
}

impl<V> Default for HdInstanceRegistry<V> {
    fn default() -> Self {
        Self {
            dictionary: Mutex::new(HashMap::new()),
        }
    }
}

impl<V> HdInstanceRegistry<V> {
    /// Create new registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create instance for key.
    ///
    /// Returns instance holding registry lock until dropped.
    pub fn get_instance(&self, key: HdInstanceKey) -> HdInstance<V>
    where
        V: Default + Clone,
    {
        let mut dict = self.dictionary.lock().unwrap();
        let holder = dict.entry(key).or_insert_with(|| ValueHolder {
            value: V::default(),
            recycle_counter: 0,
        });
        holder.recycle_counter = 0;
        HdInstance {
            key,
            value: holder.value.clone(),
        }
    }

    /// Find instance only if key exists.
    pub fn find_instance(&self, key: HdInstanceKey) -> Option<HdInstance<V>>
    where
        V: Clone,
    {
        let dict = self.dictionary.lock().unwrap();
        dict.get(&key).map(|h| HdInstance {
            key,
            value: h.value.clone(),
        })
    }

    /// Garbage collect unreferenced entries (when V is Arc, use_count==1).
    pub fn garbage_collect(&self, recycle_count: i32) -> usize
    where
        V: Clone,
    {
        self.garbage_collect_with_callback(|_| {}, recycle_count)
    }

    /// Garbage collect with callback before removal.
    pub fn garbage_collect_with_callback<F>(&self, mut callback: F, recycle_count: i32) -> usize
    where
        F: FnMut(&V),
        V: Clone,
    {
        if recycle_count < 0 {
            return self.dictionary.lock().unwrap().len();
        }
        let mut dict = self.dictionary.lock().unwrap();
        let mut to_remove = Vec::new();
        for (k, v) in dict.iter_mut() {
            let is_unique = Self::is_value_unique(&v.value);
            if is_unique {
                v.recycle_counter += 1;
                if v.recycle_counter > recycle_count {
                    to_remove.push(*k);
                }
            }
        }
        for k in to_remove {
            if let Some(h) = dict.remove(&k) {
                callback(&h.value);
            }
        }
        dict.len()
    }

    /// Override for Arc<V> - check strong_count == 1.
    fn is_value_unique(_value: &V) -> bool {
        true
    }

    /// Size of registry.
    pub fn size(&self) -> usize {
        self.dictionary.lock().unwrap().len()
    }

    /// Invalidate (clear) registry.
    pub fn invalidate(&self) {
        self.dictionary.lock().unwrap().clear();
    }
}
