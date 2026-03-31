
//! HdDataSourceLegacyTaskPrim - Task prim data source for legacy emulation.
//!
//! Corresponds to pxr/imaging/hd/dataSourceLegacyTaskPrim.h and
//! pxr/imaging/hd/legacyTaskFactory.h.

use super::base::HdDataSourceBase;
use super::{HdContainerDataSource, HdDataSourceBaseHandle};
use crate::prim::HdSceneDelegate;
use crate::render::HdTaskSharedPtr;
use std::fmt;
use std::sync::Arc;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

/// Abstract factory for creating HdTask instances from a scene delegate.
///
/// Port of C++ `HdLegacyTaskFactory` from pxr/imaging/hd/legacyTaskFactory.h.
pub trait HdLegacyTaskFactory: Send + Sync {
    /// Create a task from a scene delegate and prim id.
    fn create(&self, delegate: &dyn HdSceneDelegate, id: &SdfPath) -> Option<HdTaskSharedPtr>;
}

/// Handle to an opaque legacy task factory.
pub type HdLegacyTaskFactoryHandle = Arc<dyn HdLegacyTaskFactory>;

/// Generic task factory for a specific task type.
///
/// Port of C++ `HdLegacyTaskFactory_Impl<T>`.
/// Use `hd_make_legacy_task_factory::<T>()` to create.
pub struct HdLegacyTaskFactoryImpl<F>
where
    F: Fn(&dyn HdSceneDelegate, &SdfPath) -> Option<HdTaskSharedPtr> + Send + Sync,
{
    create_fn: F,
}

impl<F> HdLegacyTaskFactory for HdLegacyTaskFactoryImpl<F>
where
    F: Fn(&dyn HdSceneDelegate, &SdfPath) -> Option<HdTaskSharedPtr> + Send + Sync,
{
    fn create(&self, delegate: &dyn HdSceneDelegate, id: &SdfPath) -> Option<HdTaskSharedPtr> {
        (self.create_fn)(delegate, id)
    }
}

/// Create a legacy task factory from a constructor function.
///
/// Port of C++ `HdMakeLegacyTaskFactory<T>()`.
pub fn hd_make_legacy_task_factory<F>(create_fn: F) -> HdLegacyTaskFactoryHandle
where
    F: Fn(&dyn HdSceneDelegate, &SdfPath) -> Option<HdTaskSharedPtr> + Send + Sync + 'static,
{
    Arc::new(HdLegacyTaskFactoryImpl { create_fn })
}

/// Container data source for task prim from legacy scene delegate.
///
/// Corresponds to C++ `HdDataSourceLegacyTaskPrim`.
pub struct HdDataSourceLegacyTaskPrim {
    id: SdfPath,
    _scene_delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
    _factory: Option<HdLegacyTaskFactoryHandle>,
}

impl HdDataSourceLegacyTaskPrim {
    /// Create new legacy task prim data source.
    pub fn new(
        id: SdfPath,
        _scene_delegate: Option<Arc<dyn HdSceneDelegate + Send + Sync>>,
        _factory: Option<HdLegacyTaskFactoryHandle>,
    ) -> Arc<Self> {
        Arc::new(Self {
            id,
            _scene_delegate,
            _factory,
        })
    }
}

impl fmt::Debug for HdDataSourceLegacyTaskPrim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HdDataSourceLegacyTaskPrim")
            .field("id", &self.id)
            .finish()
    }
}

impl HdDataSourceBase for HdDataSourceLegacyTaskPrim {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(HdDataSourceLegacyTaskPrim {
            id: self.id.clone(),
            _scene_delegate: self._scene_delegate.clone(),
            _factory: self._factory.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<super::container::HdContainerDataSourceHandle> {
        Some(Arc::new(HdDataSourceLegacyTaskPrim {
            id: self.id.clone(),
            _scene_delegate: self._scene_delegate.clone(),
            _factory: self._factory.clone(),
        }))
    }
}

impl HdContainerDataSource for HdDataSourceLegacyTaskPrim {
    fn get_names(&self) -> Vec<Token> {
        vec![Token::new("task"), Token::new("params")]
    }

    fn get(&self, _name: &Token) -> Option<HdDataSourceBaseHandle> {
        None
    }
}
