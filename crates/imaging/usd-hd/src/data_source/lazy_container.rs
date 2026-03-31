//! Lazy container data source - deferred evaluation.
//!
//! Port of pxr/imaging/hd/lazyContainerDataSource.h/cpp
//!
//! A container that lazily evaluates a thunk on first access and then
//! delegates all calls to the computed container. Used to break reference
//! cycles (e.g. instance schema holding observer reference).

use super::base::{HdDataSourceBase, HdDataSourceBaseHandle};
use super::container::{HdContainerDataSource, HdContainerDataSourceHandle};
use std::fmt;
use std::sync::Mutex;
use usd_tf::Token;

/// Lazy container data source.
///
/// On first GetNames/Get, calls the thunk, caches the result, and delegates.
/// Port of HdLazyContainerDataSource.
pub struct HdLazyContainerDataSource {
    thunk_and_src: Mutex<Option<ThunkOrSrc>>,
}

enum ThunkOrSrc {
    Thunk(Box<dyn FnOnce() -> Option<HdContainerDataSourceHandle> + Send>),
    Src(HdContainerDataSourceHandle),
}

impl HdLazyContainerDataSource {
    /// Create a lazy container with the given thunk.
    ///
    /// The thunk is invoked on first access. It may return None if the
    /// container cannot be computed (e.g. observer was dropped).
    pub fn new<F>(thunk: F) -> std::sync::Arc<Self>
    where
        F: FnOnce() -> Option<HdContainerDataSourceHandle> + Send + 'static,
    {
        std::sync::Arc::new(Self {
            thunk_and_src: Mutex::new(Some(ThunkOrSrc::Thunk(Box::new(thunk)))),
        })
    }

    fn get_src(&self) -> Option<HdContainerDataSourceHandle> {
        let mut guard = self.thunk_and_src.lock().ok()?;
        match std::mem::take(&mut *guard) {
            Some(ThunkOrSrc::Src(src)) => {
                *guard = Some(ThunkOrSrc::Src(src.clone()));
                Some(src)
            }
            Some(ThunkOrSrc::Thunk(thunk)) => {
                let src = thunk();
                *guard = src.as_ref().map(|s| ThunkOrSrc::Src(s.clone()));
                src
            }
            None => None,
        }
    }
}

impl fmt::Debug for HdLazyContainerDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdLazyContainerDataSource").finish()
    }
}

impl HdDataSourceBase for HdLazyContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        std::sync::Arc::new(HdLazyContainerDataSource {
            thunk_and_src: Mutex::new(self.get_src().map(ThunkOrSrc::Src)),
        }) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        // Build a clone that holds the already-resolved source, mirroring clone_box.
        Some(std::sync::Arc::new(HdLazyContainerDataSource {
            thunk_and_src: Mutex::new(self.get_src().map(ThunkOrSrc::Src)),
        }))
    }
}

impl HdContainerDataSource for HdLazyContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        if let Some(src) = self.get_src() {
            src.get_names()
        } else {
            Vec::new()
        }
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if let Some(src) = self.get_src() {
            src.get(name)
        } else {
            None
        }
    }
}
