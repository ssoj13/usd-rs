
//! HdMaterialOverrideSchema - Material parameter/interface overrides.
//!
//! Corresponds to pxr/imaging/hd/materialOverrideSchema.h

use super::HdSchema;
use super::container_schema::HdContainerSchema;
use super::material_node_parameter::HdMaterialNodeParameterSchema;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use once_cell::sync::Lazy;
use usd_tf::Token;

static MATERIAL_OVERRIDE: Lazy<Token> = Lazy::new(|| Token::new("materialOverride"));
static INTERFACE_VALUES: Lazy<Token> = Lazy::new(|| Token::new("interfaceValues"));
static PARAMETER_VALUES: Lazy<Token> = Lazy::new(|| Token::new("parameterValues"));

/// Schema for material overrides (interface values and parameter values).
#[derive(Debug, Clone)]
pub struct HdMaterialOverrideSchema {
    schema: HdSchema,
}

impl HdMaterialOverrideSchema {
    /// Create from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container (looks up "materialOverride" child).
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&MATERIAL_OVERRIDE) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self {
            schema: HdSchema::empty(),
        }
    }

    /// Get parameter override for a specific shader node and parameter name.
    pub fn get_parameter_override(
        &self,
        shader_node_name: &Token,
        parameter_name: &Token,
    ) -> HdMaterialNodeParameterSchema {
        if let Some(param_vals) = self.schema.get_container() {
            if let Some(node_container) = param_vals.get(shader_node_name) {
                if let Some(container) = cast_to_container(&node_container) {
                    if let Some(param_child) = container.get(parameter_name) {
                        if let Some(param_container) = cast_to_container(&param_child) {
                            return HdMaterialNodeParameterSchema::new(param_container);
                        }
                    }
                }
            }
        }
        HdMaterialNodeParameterSchema::new(
            crate::data_source::HdRetainedContainerDataSource::new_empty(),
        )
    }

    /// Get interface values container (maps interface names to MaterialNodeParameter).
    pub fn get_interface_values(&self) -> HdContainerSchema {
        if let Some(child) = self
            .schema
            .get_container()
            .and_then(|c| c.get(&INTERFACE_VALUES))
        {
            if let Some(container) = cast_to_container(&child) {
                return HdContainerSchema::new(container);
            }
        }
        HdContainerSchema::new(crate::data_source::HdRetainedContainerDataSource::new_empty())
    }

    /// Get parameter values container (maps node names to containers of MaterialNodeParameter).
    pub fn get_parameter_values(&self) -> HdContainerSchema {
        if let Some(child) = self
            .schema
            .get_container()
            .and_then(|c| c.get(&PARAMETER_VALUES))
        {
            if let Some(container) = cast_to_container(&child) {
                return HdContainerSchema::new(container);
            }
        }
        HdContainerSchema::new(crate::data_source::HdRetainedContainerDataSource::new_empty())
    }

    /// Schema token.
    pub fn get_schema_token() -> &'static Lazy<Token> {
        &MATERIAL_OVERRIDE
    }
}
