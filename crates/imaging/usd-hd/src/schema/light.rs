//! Light schema for Hydra.
//!
//! Container schema for light parameters. Light-specific parameters
//! are typically stored as primvars or in renderer-specific namespaced
//! settings rather than directly in this schema.

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

// Schema token
pub static LIGHT: Lazy<Token> = Lazy::new(|| Token::new("light"));

/// Schema representing light data.
///
/// This is a container schema that serves as a placeholder for light data.
/// Actual light parameters (color, intensity, exposure, etc.) are typically
/// stored as primvars or in renderer-specific namespaced settings.
///
/// # Location
///
/// Default locator: `light`
#[derive(Debug, Clone)]
pub struct HdLightSchema {
    schema: HdSchema,
}

impl HdLightSchema {
    /// Creates a new light schema from a container data source.
    ///
    /// # Arguments
    ///
    /// * `container` - Container data source holding light data
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdLightSchema::HdLightSchema(HdContainerDataSourceHandle)`
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Retrieves light schema from a parent container.
    ///
    /// Looks up the `light` locator in the parent container and constructs
    /// a schema from it. Returns an empty schema if not found.
    ///
    /// # Arguments
    ///
    /// * `parent` - Parent container to search in
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdLightSchema::GetFromParent(HdContainerDataSourceHandle)`
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&LIGHT) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Checks if the schema is defined (has a valid container).
    ///
    /// # Returns
    ///
    /// `true` if the schema has a valid container, `false` otherwise
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the underlying container data source.
    ///
    /// # Returns
    ///
    /// Reference to the container if defined, `None` otherwise
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Returns the schema token (`light`).
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdLightSchema::GetSchemaToken()`
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &LIGHT
    }

    /// Returns the default locator for this schema.
    ///
    /// # Returns
    ///
    /// Locator with path `["light"]`
    ///
    /// # Reference
    ///
    /// OpenUSD: `HdLightSchema::GetDefaultLocator()`
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[LIGHT.clone()])
    }
}
