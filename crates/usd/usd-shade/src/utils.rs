//! USD Shade Utils - utility functions for shading networks.
//!
//! Port of pxr/usd/usdShade/utils.h and utils.cpp

use super::connectable_api::ConnectionSourceInfo;
use super::input::Input;
use super::output::Output;
use super::tokens::tokens;
use super::types::{AttributeType, AttributeVector};
use usd_sdf::Path;
use usd_tf::Token;

/// Utility functions for shading networks.
pub struct Utils;

impl Utils {
    /// Returns the namespace prefix of the USD attribute associated with the
    /// given shading attribute type.
    pub fn get_prefix_for_attribute_type(source_type: AttributeType) -> String {
        match source_type {
            AttributeType::Input => tokens().inputs.as_str().to_string(),
            AttributeType::Output => tokens().outputs.as_str().to_string(),
            AttributeType::Invalid => String::new(),
        }
    }

    /// Given the full name of a shading attribute, returns its base name and
    /// shading attribute type.
    pub fn get_base_name_and_type(full_name: &Token) -> (Token, AttributeType) {
        let full_name_str = full_name.as_str();
        let inputs_prefix = tokens().inputs.as_str();
        let outputs_prefix = tokens().outputs.as_str();

        if full_name_str.starts_with(inputs_prefix) {
            let base_name = Token::new(&full_name_str[inputs_prefix.len()..]);
            return (base_name, AttributeType::Input);
        }

        if full_name_str.starts_with(outputs_prefix) {
            let base_name = Token::new(&full_name_str[outputs_prefix.len()..]);
            return (base_name, AttributeType::Output);
        }

        (full_name.clone(), AttributeType::Invalid)
    }

    /// Given the full name of a shading attribute, returns its shading
    /// attribute type.
    pub fn get_type(full_name: &Token) -> AttributeType {
        Self::get_base_name_and_type(full_name).1
    }

    /// Returns the full shading attribute name given the basename and the
    /// shading attribute type.
    pub fn get_full_name(base_name: &Token, attr_type: AttributeType) -> Token {
        let prefix = Self::get_prefix_for_attribute_type(attr_type);
        Token::new(&format!("{}{}", prefix, base_name.as_str()))
    }

    /// For a valid UsdShadeConnectionSourceInfo, return the complete path
    /// to the source property; otherwise the empty path.
    pub fn get_connected_source_path(source_info: &ConnectionSourceInfo) -> Path {
        if !source_info.is_valid() {
            return Path::empty();
        }

        let prefix = Self::get_prefix_for_attribute_type(source_info.source_type);
        let prop_name = format!("{}{}", prefix, source_info.source_name.as_str());

        source_info
            .source
            .get_prim()
            .path()
            .append_property(&prop_name)
            .unwrap_or_else(Path::empty)
    }

    /// Find what is connected to an Input or Output recursively.
    ///
    /// GetValueProducingAttributes implements the UsdShade connectivity rules
    /// described in UsdShadeAttributeResolution.
    ///
    /// When tracing connections within networks that contain containers like
    /// UsdShadeNodeGraph nodes, the actual output(s) or value(s) at the end of
    /// an input or output might be multiple connections removed. This method
    /// resolves this across multiple physical connections.
    ///
    /// An UsdShadeInput is getting its value from one of these sources:
    /// - If the input is not connected the UsdAttribute for this input is
    /// returned, but only if it has an authored value. The input attribute
    /// itself carries the value for this input.
    /// - If the input is connected we follow the connection(s) until we reach
    /// a valid output of a UsdShadeShader node or if we reach a valid
    /// UsdShadeInput attribute of a UsdShadeNodeGraph or UsdShadeMaterial that
    /// has an authored value.
    ///
    /// An UsdShadeOutput on a container can get its value from the same
    /// type of sources as a UsdShadeInput on either a UsdShadeShader or
    /// UsdShadeNodeGraph. Outputs on non-containers (UsdShadeShaders) cannot be
    /// connected.
    ///
    /// If `shader_outputs_only` is true, it will only report attributes that are
    /// outputs of non-containers (UsdShadeShaders). This is a bit faster and
    /// what is needed when determining the connections for Material terminals.
    pub fn get_value_producing_attributes(
        input: &Input,
        shader_outputs_only: bool,
    ) -> AttributeVector {
        // We track which attributes we've visited so far to avoid getting caught
        // in an infinite loop, if the network contains a cycle.
        let mut found_attributes = Vec::new();
        let mut value_attributes = Vec::new();

        Self::_get_value_producing_attributes_recursive(
            input,
            &mut found_attributes,
            &mut value_attributes,
            shader_outputs_only,
        );

        value_attributes
    }

    /// \overload
    pub fn get_value_producing_attributes_output(
        output: &Output,
        shader_outputs_only: bool,
    ) -> AttributeVector {
        // We track which attributes we've visited so far to avoid getting caught
        // in an infinite loop, if the network contains a cycle.
        let mut found_attributes = Vec::new();
        let mut value_attributes = Vec::new();

        Self::_get_value_producing_attributes_recursive_output(
            output,
            &mut found_attributes,
            &mut value_attributes,
            shader_outputs_only,
        );

        value_attributes
    }

    /// Recursive helper for Input.
    fn _get_value_producing_attributes_recursive(
        inoutput: &Input,
        found_attributes: &mut Vec<Path>,
        attrs: &mut AttributeVector,
        shader_outputs_only: bool,
    ) -> bool {
        if !inoutput.is_defined() {
            return false;
        }

        // Check if we've visited this attribute before and if so abort with an
        // error, since this means we have a loop in the chain
        // Input derefs to Attribute, so we can use it directly
        let this_attr_path = inoutput.as_attribute().path().clone();
        if !found_attributes.is_empty() && found_attributes.contains(&this_attr_path) {
            eprintln!(
                "GetValueProducingAttributes: Found cycle with attribute {}",
                this_attr_path.get_string()
            );
            return false;
        }

        // Retrieve all valid connections
        let mut invalid_paths = Vec::new();
        let source_infos = inoutput.get_connected_sources(&mut invalid_paths);

        if !source_infos.is_empty() {
            // Remember the path of this attribute, so that we do not visit it again
            found_attributes.push(this_attr_path);
        }

        let mut found_valid_attr = false;

        if source_infos.len() > 1 {
            // Follow each connection until we reach an output attribute on an
            // actual shader node or an input attribute with a value
            for source_info in source_infos {
                // To handle cycle detection in the case of multiple connection we
                // have to copy the found attributes vector
                let mut local_found_attrs = found_attributes.clone();

                found_valid_attr |= Self::_follow_connection_source_recursive(
                    &source_info,
                    &mut local_found_attrs,
                    attrs,
                    shader_outputs_only,
                );
            }
        } else if !source_infos.is_empty() {
            // Follow the one connection until we reach an output attribute on an
            // actual shader node or an input attribute with a value
            found_valid_attr = Self::_follow_connection_source_recursive(
                &source_infos[0],
                found_attributes,
                attrs,
                shader_outputs_only,
            );
        }

        // If our trace should accept attributes with authored values, check if this
        // input doesn't have any valid attributes from connections, but has an
        // authored value. Return this attribute.
        if !shader_outputs_only && !found_valid_attr {
            let attr = inoutput.as_attribute();
            if attr.has_authored_value() {
                attrs.push(attr.clone());
                found_valid_attr = true;
            }
        }

        found_valid_attr
    }

    /// Recursive helper for Output.
    fn _get_value_producing_attributes_recursive_output(
        inoutput: &Output,
        found_attributes: &mut Vec<Path>,
        attrs: &mut AttributeVector,
        shader_outputs_only: bool,
    ) -> bool {
        if !inoutput.is_defined() {
            return false;
        }

        // Check if we've visited this attribute before
        let Some(attr_ref) = inoutput.get_attr() else {
            return false;
        };
        let this_attr_path = attr_ref.path().clone();
        if !found_attributes.is_empty() && found_attributes.contains(&this_attr_path) {
            eprintln!(
                "GetValueProducingAttributes: Found cycle with attribute {}",
                this_attr_path.get_string()
            );
            return false;
        }

        // Retrieve all valid connections
        let mut invalid_paths = Vec::new();
        let source_infos = inoutput.get_connected_sources(&mut invalid_paths);

        if !source_infos.is_empty() {
            found_attributes.push(this_attr_path);
        }

        let mut found_valid_attr = false;

        if source_infos.len() > 1 {
            for source_info in source_infos {
                let mut local_found_attrs = found_attributes.clone();
                found_valid_attr |= Self::_follow_connection_source_recursive(
                    &source_info,
                    &mut local_found_attrs,
                    attrs,
                    shader_outputs_only,
                );
            }
        } else if !source_infos.is_empty() {
            found_valid_attr = Self::_follow_connection_source_recursive(
                &source_infos[0],
                found_attributes,
                attrs,
                shader_outputs_only,
            );
        }

        // If our trace should accept attributes with authored values
        if !shader_outputs_only && !found_valid_attr {
            if let Some(attr) = inoutput.get_attr() {
                if attr.has_authored_value() {
                    attrs.push(attr);
                    found_valid_attr = true;
                }
            }
        }

        found_valid_attr
    }

    /// Follow a connection source recursively.
    fn _follow_connection_source_recursive(
        source_info: &ConnectionSourceInfo,
        found_attributes: &mut Vec<Path>,
        attrs: &mut AttributeVector,
        shader_outputs_only: bool,
    ) -> bool {
        if source_info.source_type == AttributeType::Output {
            let connected_output = source_info.source.get_output(&source_info.source_name);
            if !connected_output.is_defined() {
                return false;
            }

            if !source_info.source.is_container() {
                // Non-container (Shader) output - this is a terminal
                if let Some(attr) = connected_output.get_attr() {
                    attrs.push(attr);
                    return true;
                }
                return false;
            } else {
                // Container output - recurse
                return Self::_get_value_producing_attributes_recursive_output(
                    &connected_output,
                    found_attributes,
                    attrs,
                    shader_outputs_only,
                );
            }
        } else {
            // sourceType == AttributeType::Input
            let connected_input = source_info.source.get_input(&source_info.source_name);
            if !connected_input.is_defined() {
                return false;
            }

            if !source_info.source.is_container() {
                // Note, this is an invalid situation for a connected chain.
                // Since we started on an input to either a Shader or a container
                // we cannot legally connect to an input on a non-container.
                return false;
            } else {
                // Container input - recurse
                return Self::_get_value_producing_attributes_recursive(
                    &connected_input,
                    found_attributes,
                    attrs,
                    shader_outputs_only,
                );
            }
        }
    }
}
