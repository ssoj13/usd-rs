//! Basis curves schema for Hydra.
//!
//! Schema for rendering parametric curves (hair, cables, etc.).

use super::{HdBasisCurvesTopologySchema, HdSchema};
use crate::data_source::HdDataSourceLocator;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

/// Schema name token: "basisCurves"
pub static BASIS_CURVES: Lazy<Token> = Lazy::new(|| Token::new("basisCurves"));

/// Field name token: "topology"
pub static TOPOLOGY: Lazy<Token> = Lazy::new(|| Token::new("topology"));

/// Schema representing basis curves primitive.
///
/// Contains topology and other curve-specific data.
///
/// # Location
///
/// Default locator: `basisCurves`
#[derive(Debug, Clone)]
pub struct HdBasisCurvesSchema {
    /// Underlying schema container
    schema: HdSchema,
}

impl HdBasisCurvesSchema {
    /// Create schema from container data source
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Extract basis curves schema from parent container
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&BASIS_CURVES) {
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

    /// Get topology schema for curves
    pub fn get_topology(&self) -> Option<HdBasisCurvesTopologySchema> {
        if let Some(container) = self.schema.get_container() {
            let topo = HdBasisCurvesTopologySchema::get_from_parent(container);
            if topo.is_defined() {
                return Some(topo);
            }
        }
        None
    }

    /// Get schema name token
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &BASIS_CURVES
    }

    /// Get default locator for basis curves data
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[BASIS_CURVES.clone()])
    }

    /// Get locator for topology within basis curves
    pub fn get_topology_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[BASIS_CURVES.clone(), TOPOLOGY.clone()])
    }

    /// Build retained container with basis curves data
    pub fn build_retained(
        topology: Option<HdContainerDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries = Vec::new();

        if let Some(t) = topology {
            entries.push((TOPOLOGY.clone(), t as HdDataSourceBaseHandle));
        }

        HdRetainedContainerDataSource::from_entries(&entries)
    }
}
