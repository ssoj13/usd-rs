//! File system watcher for stage hot-reload.
//!
//! Watches the loaded USD file (and sublayers) for changes on disk.
//! When a modification is detected, signals the app to reload via mpsc channel.

use std::path::PathBuf;
use std::sync::mpsc;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

/// File watcher state — held by ViewerApp.
pub struct FileWatcher {
    /// The underlying OS watcher (kept alive for the duration of the watch).
    _watcher: RecommendedWatcher,
    /// Receiver for file-change events.
    rx: mpsc::Receiver<PathBuf>,
    /// Paths currently being watched.
    watched: Vec<PathBuf>,
}

impl FileWatcher {
    /// Start watching `paths` for modifications.
    /// Returns None if the watcher cannot be created (e.g. unsupported OS).
    pub fn new(paths: &[PathBuf]) -> Option<Self> {
        let (tx, rx) = mpsc::channel();

        let sender = tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only trigger on actual content modifications
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    for p in &event.paths {
                        let _ = sender.send(p.clone());
                    }
                }
            }
        })
        .ok()?;

        let mut watched = Vec::new();
        for path in paths {
            if path.exists() {
                if watcher.watch(path, RecursiveMode::NonRecursive).is_ok() {
                    watched.push(path.clone());
                }
            }
        }

        if watched.is_empty() {
            return None;
        }

        Some(Self {
            _watcher: watcher,
            rx,
            watched,
        })
    }

    /// Non-blocking poll: returns true if any watched file was modified.
    pub fn poll_changed(&self) -> bool {
        // Drain all pending events; return true if any received
        let mut changed = false;
        while self.rx.try_recv().is_ok() {
            changed = true;
        }
        changed
    }

    /// Paths currently watched (for debug / logging).
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched
    }
}
