
//! HdExtComputationPrimvarsSchema - Container of ext computation primvars.
//!
//! Port of pxr/imaging/hd/extComputationPrimvarsSchema.h
//!
//! Lists primvars whose values come from ext computations.

use super::HdSchema;
use super::ext_computation_primvar::HdExtComputationPrimvarSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

static EXT_COMPUTATION_PRIMVARS: Lazy<Token> = Lazy::new(|| Token::new("extComputationPrimvars"));

/// Schema for the extComputationPrimvars container.
///
/// Each child is an HdExtComputationPrimvarSchema describing a primvar
/// driven by an ext computation.
#[derive(Debug, Clone)]
pub struct HdExtComputationPrimvarsSchema {
    schema: HdSchema,
}

impl HdExtComputationPrimvarsSchema {
    /// Creates schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves the schema from a parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&EXT_COMPUTATION_PRIMVARS) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Returns true if the schema has valid data.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns all ext computation primvar names.
    pub fn get_ext_computation_primvar_names(&self) -> Vec<Token> {
        if let Some(container) = self.schema.get_container() {
            container.get_names()
        } else {
            Vec::new()
        }
    }

    /// Returns the ext computation primvar schema for the given name.
    pub fn get_ext_computation_primvar(&self, name: &Token) -> HdExtComputationPrimvarSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(name) {
                if let Some(child_container) = cast_to_container(&child) {
                    return HdExtComputationPrimvarSchema::new(child_container);
                }
            }
        }
        HdExtComputationPrimvarSchema::empty()
    }

    /// Returns the schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &EXT_COMPUTATION_PRIMVARS
    }

    /// Returns the default locator for extComputationPrimvars.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(EXT_COMPUTATION_PRIMVARS.clone())
    }
}
