//! Material interface schema for Hydra.
//!
//! Describes a material's interface parameters (public UI parameters).
//! Corresponds to pxr/imaging/hd/materialInterfaceSchema.h

use super::HdSchema;
use super::material_interface_parameter_schema::HdMaterialInterfaceParameterSchema;
use crate::data_source::{HdContainerDataSourceHandle, cast_to_container};
use crate::schema::render_settings::HdTokenArrayDataSourceHandle;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use usd_tf::Token;

/// Member token: "parameters".
pub static PARAMETERS: Lazy<Token> = Lazy::new(|| Token::new("parameters"));
/// Member token: "parameterOrder".
pub static PARAMETER_ORDER: Lazy<Token> = Lazy::new(|| Token::new("parameterOrder"));

/// Container schema for interface parameters (HdMaterialInterfaceParameterContainerSchema).
/// Each child is an HdMaterialInterfaceParameterSchema keyed by public UI name.
#[derive(Debug, Clone)]
pub struct HdMaterialInterfaceParameterContainerSchema {
    schema: HdSchema,
}

impl HdMaterialInterfaceParameterContainerSchema {
    /// Create from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Returns true if the schema has a valid container.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Returns the parameter names (public UI names).
    pub fn get_names(&self) -> Vec<Token> {
        if let Some(container) = self.schema.get_container() {
            container.get_names()
        } else {
            Vec::new()
        }
    }

    /// Get interface parameter schema by public UI name.
    pub fn get(&self, name: &Token) -> HdMaterialInterfaceParameterSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(name) {
                if let Some(param_container) = cast_to_container(&child) {
                    return HdMaterialInterfaceParameterSchema::new(param_container);
                }
            }
        }
        HdMaterialInterfaceParameterSchema::new(
            crate::data_source::HdRetainedContainerDataSource::new_empty(),
        )
    }
}

/// Nested map: nodePath -> (inputName -> publicUIName).
pub type NestedTokenMap = HashMap<Token, HashMap<Token, Token>>;

/// Schema for material interface (public parameters).
///
/// Corresponds to C++ HdMaterialInterfaceSchema.
#[derive(Debug, Clone)]
pub struct HdMaterialInterfaceSchema {
    schema: HdSchema,
}

impl HdMaterialInterfaceSchema {
    /// Create from container data source.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Builds and returns a map of reversed interface mappings.
    /// Interface mappings: publicUIName -> [(nodePath, inputName),...]
    /// Reversed map: nodePath -> (inputName -> publicUIName)
    pub fn get_reverse_interface_mappings(&self) -> NestedTokenMap {
        let mut reverse: NestedTokenMap = HashMap::new();

        let params = self.get_parameters();
        if !params.is_defined() {
            return reverse;
        }

        for public_ui_name in params.get_names() {
            let param_schema = params.get(&public_ui_name);
            let mappings = param_schema.get_mappings();

            for i in 0..mappings.get_num_elements() {
                let mapping_schema = param_schema.get_mapping_element(i);
                if let Some(node_path_ds) = mapping_schema.get_node_path() {
                    if let Some(input_name_ds) = mapping_schema.get_input_name() {
                        let node_path = node_path_ds.get_typed_value(0.0);
                        let input_name = input_name_ds.get_typed_value(0.0);
                        reverse
                            .entry(node_path)
                            .or_default()
                            .insert(input_name, public_ui_name.clone());
                    }
                }
            }
        }

        reverse
    }

    /// Container for all the material's interface parameters.
    pub fn get_parameters(&self) -> HdMaterialInterfaceParameterContainerSchema {
        if let Some(container) = self.schema.get_container() {
            if let Some(child) = container.get(&PARAMETERS) {
                if let Some(param_container) = cast_to_container(&child) {
                    return HdMaterialInterfaceParameterContainerSchema::new(param_container);
                }
            }
        }
        HdMaterialInterfaceParameterContainerSchema::new(
            crate::data_source::HdRetainedContainerDataSource::new_empty(),
        )
    }

    /// Intended order of interface parameters for UI.
    pub fn get_parameter_order(&self) -> Option<HdTokenArrayDataSourceHandle> {
        self.schema.get_typed(&PARAMETER_ORDER)
    }

    /// Build retained container with provided fields.
    pub fn build_retained(
        parameters: Option<HdContainerDataSourceHandle>,
        parameter_order: Option<HdTokenArrayDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        use crate::data_source::{HdDataSourceBaseHandle, HdRetainedContainerDataSource};

        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(p) = parameters {
            entries.push((PARAMETERS.clone(), p as HdDataSourceBaseHandle));
        }
        if let Some(po) = parameter_order {
            entries.push((PARAMETER_ORDER.clone(), po as HdDataSourceBaseHandle));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

/// Builder for HdMaterialInterfaceSchema.
#[derive(Default)]
pub struct HdMaterialInterfaceSchemaBuilder {
    parameters: Option<HdContainerDataSourceHandle>,
    parameter_order: Option<HdTokenArrayDataSourceHandle>,
}

impl HdMaterialInterfaceSchemaBuilder {
    /// Create empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set parameters container.
    pub fn set_parameters(mut self, v: HdContainerDataSourceHandle) -> Self {
        self.parameters = Some(v);
        self
    }

    /// Set parameter ordering for UI.
    pub fn set_parameter_order(mut self, v: HdTokenArrayDataSourceHandle) -> Self {
        self.parameter_order = Some(v);
        self
    }

    /// Build container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        HdMaterialInterfaceSchema::build_retained(self.parameters, self.parameter_order)
    }
}
