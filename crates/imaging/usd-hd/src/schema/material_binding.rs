//! Material binding schema (single purpose binding).
//!
//! Port of pxr/imaging/hd/materialBindingSchema.h

use super::HdSchema;
use crate::data_source::HdContainerDataSourceHandle;
use once_cell::sync::Lazy;
use usd_sdf::Path;

/// Schema token: "path"
pub static PATH: Lazy<usd_tf::Token> = Lazy::new(|| usd_tf::Token::new("path"));

/// Token for all-purpose binding fallback.
pub static ALL_PURPOSE: Lazy<usd_tf::Token> = Lazy::new(|| usd_tf::Token::new("allPurpose"));

/// Schema for a single material binding (one purpose).
#[derive(Debug, Clone)]
pub struct HdMaterialBindingSchema {
    schema: HdSchema,
}

impl HdMaterialBindingSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Create empty schema (no binding).
    pub fn empty() -> Self {
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get the bound material path. Returns None if not set.
    pub fn get_path(&self) -> Option<Path> {
        let ds = self.schema.get_container()?;
        let child = ds.get(&*PATH)?;
        let sampled = child.as_sampled()?;
        let value = sampled.get_value(0.0);
        value.get::<Path>().cloned()
    }
}
