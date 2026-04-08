//! HdMapContainerDataSource - Apply function to container child data sources.
//!
//! Corresponds to pxr/imaging/hd/mapContainerDataSource.h.

use super::base::HdDataSourceBase;
use super::{HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBaseHandle};
use std::fmt;
use std::sync::Arc;
use usd_tf::Token;

/// Container that applies a function to all child data sources (non-recursive).
///
/// Corresponds to C++ `HdMapContainerDataSource`.
pub struct HdMapContainerDataSource {
    f: Arc<dyn Fn(&HdDataSourceBaseHandle) -> HdDataSourceBaseHandle + Send + Sync>,
    src: HdContainerDataSourceHandle,
}

impl HdMapContainerDataSource {
    /// Create new map container.
    pub fn new<F>(f: F, src: HdContainerDataSourceHandle) -> Arc<Self>
    where
        F: Fn(&HdDataSourceBaseHandle) -> HdDataSourceBaseHandle + Send + Sync + 'static,
    {
        Arc::new(Self {
            f: Arc::new(f),
            src,
        })
    }
}

impl fmt::Debug for HdMapContainerDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdMapContainerDataSource").finish()
    }
}

impl HdDataSourceBase for HdMapContainerDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(HdMapContainerDataSource {
            f: self.f.clone(),
            src: Arc::clone(&self.src),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(HdMapContainerDataSource {
            f: self.f.clone(),
            src: Arc::clone(&self.src),
        }))
    }
}

impl HdContainerDataSource for HdMapContainerDataSource {
    fn get_names(&self) -> Vec<Token> {
        self.src.get_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        self.src.get(name).map(|child| (self.f)(&child))
    }
}
