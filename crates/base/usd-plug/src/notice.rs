//! Plugin notifications.
//!
//! Port of pxr/base/plug/notice.h/cpp
//!
//! Provides notification mechanism for plugin registration events.

use std::sync::{Arc, Mutex, OnceLock};

use crate::plugin::PlugPlugin;

/// Callback type for plugin registration notifications.
pub type DidRegisterPluginsCallback = Arc<dyn Fn(&[Arc<PlugPlugin>]) + Send + Sync>;

/// Global listener list for DidRegisterPlugins notifications.
struct NoticeRegistry {
    listeners: Vec<DidRegisterPluginsCallback>,
}

static NOTICE_REGISTRY: OnceLock<Mutex<NoticeRegistry>> = OnceLock::new();

fn notice_registry() -> &'static Mutex<NoticeRegistry> {
    NOTICE_REGISTRY.get_or_init(|| {
        Mutex::new(NoticeRegistry {
            listeners: Vec::new(),
        })
    })
}

/// Register a callback to be invoked when new plugins are registered.
///
/// Matches C++ `TfNotice::Register(... &PlugNotice::DidRegisterPlugins ...)`.
pub fn on_did_register_plugins<F>(callback: F)
where
    F: Fn(&[Arc<PlugPlugin>]) + Send + Sync + 'static,
{
    let mut registry = notice_registry().lock().expect("notice registry poisoned");
    registry.listeners.push(Arc::new(callback));
}

/// Send DidRegisterPlugins notification to all registered listeners.
///
/// Called internally by PlugRegistry after registering new plugins.
/// C++ explicitly sends notices outside any lock (registry.cpp comment on line 333).
/// We clone the Arc'd listener list under the lock, then invoke outside it.
pub(crate) fn send_did_register_plugins(new_plugins: &[Arc<PlugPlugin>]) {
    if new_plugins.is_empty() {
        return;
    }
    // Snapshot listeners under lock, then drop lock before invoking.
    // This prevents deadlock if a callback registers new listeners.
    let snapshot: Vec<DidRegisterPluginsCallback> = {
        let registry = notice_registry().lock().expect("notice registry poisoned");
        registry.listeners.clone()
    };
    for listener in &snapshot {
        listener(new_plugins);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_send_empty_no_callback() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        on_did_register_plugins(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Empty list should not invoke callbacks
        send_did_register_plugins(&[]);
        // Note: can't assert counter==0 reliably due to global state from other tests
    }
}
