//! Reporter data sources.
//!
//! Port of pxr/base/trace/reporterDataSourceBase.h,
//! reporterDataSourceCollection.h, reporterDataSourceCollector.h
//!
//! This module provides data source implementations for trace reporters.

use super::collection::Collection;
use super::collection_notice::{CollectionAvailable, CollectionListener, global_registry};
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

// ============================================================================
// Data Source Base Trait
// ============================================================================

/// Base trait for TraceReporter data sources.
///
/// TraceReporterBase uses an instance of a type implementing this trait
/// to access TraceCollections.
pub trait ReporterDataSource: Send + Sync {
    /// Removes all references to TraceCollections.
    fn clear(&self);

    /// Returns the next TraceCollections which need to be processed.
    fn consume_data(&self) -> Vec<Arc<Collection>>;
}

// ============================================================================
// Collection Data Source
// ============================================================================

/// A data source that provides access to a fixed set of TraceCollections.
///
/// This is useful for generating reports from serialized TraceCollections.
pub struct CollectionDataSource {
    /// The stored collections.
    data: Mutex<Vec<Arc<Collection>>>,
}

impl CollectionDataSource {
    /// Creates a new data source with a single collection.
    pub fn new(collection: Arc<Collection>) -> Self {
        Self {
            data: Mutex::new(vec![collection]),
        }
    }

    /// Creates a new data source with multiple collections.
    pub fn from_vec(collections: Vec<Arc<Collection>>) -> Self {
        Self {
            data: Mutex::new(collections),
        }
    }

    /// Creates an empty data source.
    pub fn empty() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
        }
    }

    /// Adds a collection to the data source.
    pub fn add(&self, collection: Arc<Collection>) {
        self.data.lock().push(collection);
    }
}

impl ReporterDataSource for CollectionDataSource {
    fn clear(&self) {
        self.data.lock().clear();
    }

    fn consume_data(&self) -> Vec<Arc<Collection>> {
        std::mem::take(&mut *self.data.lock())
    }
}

// ============================================================================
// Collector Data Source
// ============================================================================

/// A data source that retrieves TraceCollections from the TraceCollector.
///
/// This data source listens for CollectionAvailable notices and queues
/// the collections for processing.
pub struct CollectorDataSource {
    /// The accept function (if any).
    accept: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    /// Pending collections waiting to be processed.
    pending: Mutex<VecDeque<Arc<Collection>>>,
    /// Self-reference for notice handling (weak to avoid cycles).
    self_arc: Mutex<Option<Arc<Self>>>,
}

impl CollectorDataSource {
    /// Creates a new collector data source.
    pub fn new() -> Arc<Self> {
        let source = Arc::new(Self {
            accept: None,
            pending: Mutex::new(VecDeque::new()),
            self_arc: Mutex::new(None),
        });

        // Store self-reference
        *source.self_arc.lock() = Some(Arc::clone(&source));

        // Register for notices
        let listener: Arc<dyn CollectionListener> = source.clone();
        global_registry().register(&listener);

        source
    }

    /// Creates a new collector data source with an accept function.
    ///
    /// The data source will only listen to CollectionAvailable notices
    /// when the accept function returns true.
    pub fn with_accept<F>(accept: F) -> Arc<Self>
    where
        F: Fn() -> bool + Send + Sync + 'static,
    {
        let source = Arc::new(Self {
            accept: Some(Box::new(accept)),
            pending: Mutex::new(VecDeque::new()),
            self_arc: Mutex::new(None),
        });

        *source.self_arc.lock() = Some(Arc::clone(&source));

        let listener: Arc<dyn CollectionListener> = source.clone();
        global_registry().register(&listener);

        source
    }

    /// Returns the number of pending collections.
    pub fn pending_count(&self) -> usize {
        self.pending.lock().len()
    }

    /// Handles a new collection being available.
    fn on_collection(&self, collection: Arc<Collection>) {
        // Check if we should accept this collection
        if let Some(ref accept) = self.accept {
            if !accept() {
                return;
            }
        }

        self.pending.lock().push_back(collection);
    }
}

impl ReporterDataSource for CollectorDataSource {
    fn clear(&self) {
        self.pending.lock().clear();
    }

    fn consume_data(&self) -> Vec<Arc<Collection>> {
        std::mem::take(&mut *self.pending.lock())
            .into_iter()
            .collect()
    }
}

impl CollectionListener for CollectorDataSource {
    fn on_collection_available(&self, notice: &CollectionAvailable) {
        self.on_collection(Arc::clone(notice.collection()));
    }
}

// ============================================================================
// Reporter Base
// ============================================================================

/// Base class for report implementations.
///
/// Handles receiving and processing of TraceCollections.
pub struct ReporterBase {
    /// The data source.
    data_source: Box<dyn ReporterDataSource>,
    /// Processed collections (kept for serialization).
    processed: Mutex<Vec<Arc<Collection>>>,
}

impl ReporterBase {
    /// Creates a new reporter with the given data source.
    pub fn new<D: ReporterDataSource + 'static>(data_source: D) -> Self {
        Self {
            data_source: Box::new(data_source),
            processed: Mutex::new(Vec::new()),
        }
    }

    /// Creates a new reporter with a boxed data source.
    pub fn with_boxed(data_source: Box<dyn ReporterDataSource>) -> Self {
        Self {
            data_source,
            processed: Mutex::new(Vec::new()),
        }
    }

    /// Removes all references to TraceCollections.
    pub fn clear(&self) {
        self.data_source.clear();
        self.processed.lock().clear();
    }

    /// Gets the latest data and processes all new collections.
    ///
    /// Returns the collections that were processed.
    pub fn update(&self) -> Vec<Arc<Collection>> {
        let collections = self.data_source.consume_data();

        // Store for later serialization
        self.processed.lock().extend(collections.iter().cloned());

        collections
    }

    /// Returns all processed collections.
    pub fn processed_collections(&self) -> Vec<Arc<Collection>> {
        self.processed.lock().clone()
    }

    /// Serializes all processed collections to JSON.
    pub fn serialize_processed(&self) -> Result<String, serde_json::Error> {
        let collections = self.processed.lock();
        let data: Vec<_> = collections
            .iter()
            .map(|c| {
                // Convert to serializable format
                let mut threads = Vec::new();
                for (thread_id, events) in c.iter() {
                    let event_data: Vec<_> = events
                        .iter()
                        .map(|e| {
                            serde_json::json!({
                                "key": e.key(),
                                "timestamp": e.timestamp_seconds(),
                                "type": format!("{:?}", e.event_type),
                            })
                        })
                        .collect();

                    threads.push(serde_json::json!({
                        "thread_id": thread_id.to_string(),
                        "events": event_data,
                    }));
                }
                serde_json::json!({ "threads": threads })
            })
            .collect();

        serde_json::to_string_pretty(&data)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::EventList;

    #[test]
    fn test_collection_data_source_single() {
        let collection = Arc::new(Collection::new());
        let source = CollectionDataSource::new(collection.clone());

        let data = source.consume_data();
        assert_eq!(data.len(), 1);
        assert!(Arc::ptr_eq(&data[0], &collection));

        // Should be empty after consuming
        let data2 = source.consume_data();
        assert!(data2.is_empty());
    }

    #[test]
    fn test_collection_data_source_multiple() {
        let c1 = Arc::new(Collection::new());
        let c2 = Arc::new(Collection::new());
        let source = CollectionDataSource::from_vec(vec![c1.clone(), c2.clone()]);

        let data = source.consume_data();
        assert_eq!(data.len(), 2);
    }

    #[test]
    fn test_collection_data_source_clear() {
        let source = CollectionDataSource::new(Arc::new(Collection::new()));
        source.clear();

        let data = source.consume_data();
        assert!(data.is_empty());
    }

    #[test]
    fn test_reporter_base_update() {
        let c1 = Arc::new(Collection::new());
        let c2 = Arc::new(Collection::new());
        let source = CollectionDataSource::from_vec(vec![c1.clone(), c2.clone()]);

        let reporter = ReporterBase::new(source);
        let processed = reporter.update();

        assert_eq!(processed.len(), 2);
        assert_eq!(reporter.processed_collections().len(), 2);
    }

    #[test]
    fn test_reporter_base_clear() {
        let source = CollectionDataSource::new(Arc::new(Collection::new()));
        let reporter = ReporterBase::new(source);

        reporter.update();
        assert_eq!(reporter.processed_collections().len(), 1);

        reporter.clear();
        assert!(reporter.processed_collections().is_empty());
    }
}
