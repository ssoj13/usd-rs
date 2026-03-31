//! Pub/Sub Event Bus for decoupled component communication.
//!
//! Ported from playa/src/core/event_bus.rs for usd-view.
//!
//! - emit() invokes callbacks immediately AND queues for deferred processing
//! - poll() returns queued events for batch processing in main loop

use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

const MAX_QUEUE_SIZE: usize = 1000;

/// Marker trait for events. Events must be Send + Sync + 'static.
pub trait Event: Any + Send + Sync + 'static {
    fn as_any(&self) -> &dyn Any;
    fn type_name(&self) -> &'static str;
}

impl<T: Any + Send + Sync + 'static> Event for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

type Callback = Arc<dyn Fn(&dyn Any) + Send + Sync>;

/// Type-erased boxed event for queue storage.
pub type BoxedEvent = Box<dyn Event>;

/// Pub/Sub Event Bus with deferred processing support.
#[derive(Clone)]
pub struct EventBus {
    subscribers: Arc<RwLock<HashMap<TypeId, Vec<Callback>>>>,
    queue: Arc<Mutex<VecDeque<BoxedEvent>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Subscribe to events of type E (callback invoked immediately on emit).
    pub fn subscribe<E, F>(&self, callback: F)
    where
        E: Event,
        F: Fn(&E) + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<E>();
        let wrapped: Callback = Arc::new(move |any: &dyn Any| {
            if let Some(event) = any.downcast_ref::<E>() {
                callback(event);
            }
        });
        self.subscribers
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .entry(type_id)
            .or_default()
            .push(wrapped);
    }

    /// Emit event: invoke callbacks immediately AND queue for poll().
    pub fn emit<E: Event + Clone>(&self, event: E) {
        let type_id = TypeId::of::<E>();

        let callbacks = {
            let subs = self.subscribers.read().unwrap_or_else(|e| e.into_inner());
            subs.get(&type_id).cloned()
        };
        if let Some(cbs) = callbacks {
            for cb in cbs {
                cb(&event);
            }
        }

        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= MAX_QUEUE_SIZE {
            let evict_count = queue.len() / 2;
            log::warn!(
                "EventBus queue full ({} events), evicting oldest {}",
                queue.len(),
                evict_count
            );
            for _ in 0..evict_count {
                queue.pop_front();
            }
        }
        queue.push_back(Box::new(event));
    }

    /// Poll all queued events for batch processing.
    pub fn poll(&self) -> Vec<BoxedEvent> {
        let mut queue = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.drain(..).collect()
    }

    /// Get an emitter handle for passing to UI components or threads.
    pub fn emitter(&self) -> EventEmitter {
        EventEmitter(Arc::new(self.clone()))
    }

    /// Clear all subscribers and queue.
    pub fn clear(&self) {
        self.subscribers
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Check queue length.
    pub fn queue_len(&self) -> usize {
        self.queue.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

/// Lightweight emitter handle — clone-shares the same bus via Deref.
#[derive(Clone)]
pub struct EventEmitter(Arc<EventBus>);

impl EventEmitter {
    pub fn new(bus: Arc<EventBus>) -> Self {
        Self(bus)
    }
}

impl std::ops::Deref for EventEmitter {
    type Target = EventBus;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Helper: downcast BoxedEvent to concrete type.
///
/// Must explicitly deref to `dyn Event` before calling `as_any()` to avoid
/// the blanket impl `Event for Box<dyn Event>` intercepting the call.
#[inline]
pub fn downcast_event<E: Event>(event: &BoxedEvent) -> Option<&E> {
    (**event).as_any().downcast_ref::<E>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[derive(Clone, Debug)]
    struct TestEvent {
        value: i32,
    }

    #[derive(Clone, Debug)]
    struct OtherEvent;

    #[test]
    fn test_subscribe_emit_immediate() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = Arc::clone(&counter);

        bus.subscribe::<TestEvent, _>(move |event| {
            counter_clone.fetch_add(event.value, Ordering::SeqCst);
        });

        bus.emit(TestEvent { value: 10 });
        assert_eq!(counter.load(Ordering::SeqCst), 10);

        bus.emit(TestEvent { value: 5 });
        assert_eq!(counter.load(Ordering::SeqCst), 15);
    }

    #[test]
    fn test_emit_queues_for_poll() {
        let bus = EventBus::new();

        bus.emit(TestEvent { value: 1 });
        bus.emit(TestEvent { value: 2 });
        bus.emit(OtherEvent);

        let events = bus.poll();
        assert_eq!(events.len(), 3);
        assert_eq!(bus.poll().len(), 0);
    }

    #[test]
    fn test_downcast() {
        let bus = EventBus::new();
        bus.emit(TestEvent { value: 42 });

        for event in bus.poll() {
            if let Some(test_event) = downcast_event::<TestEvent>(&event) {
                assert_eq!(test_event.value, 42);
            }
        }
    }

    #[test]
    fn test_emitter_handle() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicI32::new(0));
        let counter_clone = Arc::clone(&counter);

        bus.subscribe::<TestEvent, _>(move |event| {
            counter_clone.fetch_add(event.value, Ordering::SeqCst);
        });

        let emitter = bus.emitter();
        emitter.emit(TestEvent { value: 42 });
        assert_eq!(counter.load(Ordering::SeqCst), 42);
        assert_eq!(bus.poll().len(), 1);
    }
}
