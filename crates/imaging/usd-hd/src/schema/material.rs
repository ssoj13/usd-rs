//! Material schema for Hydra.
//!
//! Container schema providing material definitions per render context.
//! Each render context (e.g., "", "ri", "glslfx") contains a material network
//! defining the shader graph for that specific renderer.

use super::HdSchema;
use crate::data_source::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

// Schema tokens

/// Schema name token: "material"
pub static MATERIAL: Lazy<Token> = Lazy::new(|| Token::new("material"));

/// Universal render context token (empty string applies to all renderers)
pub static UNIVERSAL_RENDER_CONTEXT: Lazy<Token> = Lazy::new(|| Token::new(""));

// Terminal tokens for material networks
#[allow(dead_code)] // Ready for use when shader network population is needed
/// Terminal outputs token: "terminals"
pub static TERMINALS: Lazy<Token> = Lazy::new(|| Token::new("terminals"));

#[allow(dead_code)]
/// Surface shader token: "surface"
pub static SURFACE: Lazy<Token> = Lazy::new(|| Token::new("surface"));

#[allow(dead_code)]
/// Displacement shader token: "displacement"
pub static DISPLACEMENT: Lazy<Token> = Lazy::new(|| Token::new("displacement"));

#[allow(dead_code)]
/// Volume shader token: "volume"
pub static VOLUME: Lazy<Token> = Lazy::new(|| Token::new("volume"));

/// Schema representing material data.
///
/// The Material schema is a container that provides material definitions
/// organized by render context. For example:
/// - Universal render context ("") applies to all renderers
/// - "ri" context for Renderman
/// - "glslfx" context for Storm/OpenGL
///
/// Each render context contains a MaterialNetwork schema defining the
/// specific shader graph for that renderer.
///
/// # Location
///
/// Default locator: `material`
#[derive(Debug, Clone)]
pub struct HdMaterialSchema {
    schema: HdSchema,
}

impl HdMaterialSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract material schema from parent container
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&MATERIAL) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Check if schema is defined (has valid container)
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get underlying container data source
    pub fn get_container(&self) -> Option<&HdContainerDataSourceHandle> {
        self.schema.get_container()
    }

    /// Get available render contexts in this material
    pub fn get_render_contexts(&self) -> Vec<Token> {
        if let Some(container) = self.get_container() {
            container.get_names()
        } else {
            Vec::new()
        }
    }

    /// Get material network for the universal render context
    pub fn get_material_network(&self) -> Option<HdContainerDataSourceHandle> {
        self.get_material_network_for_context(&UNIVERSAL_RENDER_CONTEXT)
    }

    /// Get material network for a specific render context
    pub fn get_material_network_for_context(
        &self,
        context: &Token,
    ) -> Option<HdContainerDataSourceHandle> {
        if let Some(container) = self.get_container() {
            if let Some(child) = container.get(context) {
                return cast_to_container(&child);
            }
        }
        None
    }

    /// Get material network from list of contexts, falling back to universal
    pub fn get_material_network_for_contexts(
        &self,
        contexts: &[Token],
    ) -> Option<HdContainerDataSourceHandle> {
        // Try each requested context
        for context in contexts {
            if let Some(network) = self.get_material_network_for_context(context) {
                return Some(network);
            }
        }
        // Fallback to universal context
        self.get_material_network()
    }

    /// Get schema name token
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &MATERIAL
    }

    /// Get default locator for material data
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[MATERIAL.clone()])
    }

    /// Build material schema with render context networks
    pub fn build_retained(
        contexts: &[Token],
        networks: &[HdContainerDataSourceHandle],
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let entries: Vec<(Token, HdDataSourceBaseHandle)> = contexts
            .iter()
            .zip(networks.iter())
            .map(|(ctx, net)| (ctx.clone(), net.clone() as HdDataSourceBaseHandle))
            .collect();

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
