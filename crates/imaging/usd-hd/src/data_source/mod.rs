//! Data source infrastructure for Hydra.
//!
//! Data sources provide the foundational data layer for Hydra's scene index
//! architecture. They represent time-sampled, hierarchical scene data that
//! can be queried by render delegates and scene indices.
//!
//! # Architecture
//!
//! - [`HdDataSourceBase`] - Base trait for all data sources
//! - [`HdContainerDataSource`] - Hierarchical named data (like a dictionary)
//! - [`HdSampledDataSource`] - Time-sampled values with motion blur support
//! - [`HdVectorDataSource`] - Indexed arrays of data sources
//! - [`HdTypedSampledDataSource`] - Type-safe sampled data wrapper
//! - [`HdDataSourceLocator`] - Path addressing within nested containers
//!
//! # Retained Implementations
//!
//! The [`retained`] module provides in-memory implementations suitable for
//! storing scene data locally, disconnected from live scene sources.
//!
//! # Examples
//!
//! ```
//! use usd_hd::data_source::*;
//! use usd_vt::Value;
//!
//! // Create a simple retained sampled data source
//! let value = Value::from(42i32);
//! let ds = HdRetainedSampledDataSource::new(value);
//!
//! // Access the value
//! let result = ds.get_value(0.0);
//! ```

mod base;
mod container;
mod container_editor;
mod debug;
mod hash;
mod invalidatable_container;
mod lazy_container;
mod legacy_prim;
mod legacy_task_prim;
mod locator;
mod map_container;
mod retained;
mod sampled;
mod static_copy;
mod type_defs;
mod typed;
mod value_extract;
mod vector;

pub use base::*;
pub use container::*;
pub use container_editor::*;
pub use debug::{hd_debug_print_data_source, hd_debug_print_data_source_stdout};
pub use hash::*;
pub use invalidatable_container::HdInvalidatableContainerDataSource;
pub use lazy_container::*;
pub use legacy_prim::{
    HdDataSourceLegacyPrim, flag_tokens as legacy_flag_tokens, tokens as legacy_prim_tokens,
};
pub(crate) use legacy_prim::{TOK_SCENE_DELEGATE, extract_scene_delegate_handle};
pub use legacy_task_prim::{
    HdDataSourceLegacyTaskPrim, HdLegacyTaskFactory, HdLegacyTaskFactoryHandle,
    HdLegacyTaskFactoryImpl, hd_make_legacy_task_factory,
};
pub use locator::*;
pub use map_container::HdMapContainerDataSource;
pub use retained::*;
pub use sampled::*;
pub use static_copy::hd_make_static_copy;
pub use type_defs::*;
pub use typed::*;
pub use value_extract::*;
pub use vector::*;

// Type aliases for commonly used typed data sources
use usd_ar::ResolverContext;

/// Typed data source for ArResolverContext.
pub type HdResolverContextDataSource = dyn HdTypedSampledDataSource<ResolverContext>;

/// Handle to a resolver context data source.
pub type HdResolverContextDataSourceHandle = std::sync::Arc<HdResolverContextDataSource>;
