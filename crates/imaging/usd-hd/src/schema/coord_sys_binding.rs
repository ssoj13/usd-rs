
//! Coord sys binding schema for Hydra.
//!
//! Port of pxr/imaging/hd/coordSysBindingSchema.
//!
//! Container mapping coord sys names to target prim paths.

use super::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    cast_to_container,
};
use crate::schema::ext_computation_input_computation::HdPathDataSourceHandle;
use once_cell::sync::Lazy;
use usd_tf::Token;

static COORD_SYS_BINDING: Lazy<Token> = Lazy::new(|| Token::new("coordSysBinding"));

/// Schema for coord sys bindings on a prim.
///
/// Container where each child name is a coord sys name (e.g. "modelSpace")
/// and the value is HdPathDataSource with the target prim path.
#[derive(Debug, Clone)]
pub struct HdCoordSysBindingSchema {
    schema: HdSchema,
}

impl HdCoordSysBindingSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get schema from parent container (prim data source).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&COORD_SYS_BINDING) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &COORD_SYS_BINDING
    }

    /// Get default locator for coordSysBinding.
    pub fn get_default_locator() -> crate::data_source::HdDataSourceLocator {
        crate::data_source::HdDataSourceLocator::from_token(COORD_SYS_BINDING.clone())
    }

    /// Get names of all coord sys bindings.
    pub fn get_coord_sys_binding_names(&self) -> Vec<Token> {
        if let Some(container) = self.schema.get_container() {
            return container.get_names();
        }
        Vec::new()
    }

    /// Get the target path for a coord sys binding by name.
    pub fn get_coord_sys_binding(&self, name: &Token) -> Option<HdPathDataSourceHandle> {
        self.schema.get_typed(name)
    }

    /// Build retained container from name-value pairs.
    pub fn build_retained(
        names: &[Token],
        values: &[HdPathDataSourceHandle],
    ) -> HdContainerDataSourceHandle {
        assert_eq!(names.len(), values.len());
        let entries: Vec<_> = names
            .iter()
            .zip(values.iter())
            .map(|(n, v)| (n.clone(), v.clone() as HdDataSourceBaseHandle))
            .collect();
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
