
//! Data source implementation of material network interface.
//!
//! Port of pxr/imaging/hd/dataSourceMaterialNetworkInterface.{h,cpp}

use crate::data_source::{
    HdContainerDataSourceEditor, HdContainerDataSourceHandle, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdRetainedContainerDataSource, HdRetainedSampledDataSource,
    HdRetainedSmallVectorDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
    cast_to_container, cast_to_vector,
};
use crate::material_network_interface::{
    HdMaterialNetworkInterface, InputConnection, InputConnectionVector, NodeParamData,
};
use crate::schema::material_network::{self, HdMaterialNetworkSchema};
use crate::schema::material_node::{
    HdMaterialNodeSchema, INPUT_CONNECTIONS, NODE_IDENTIFIER, NODE_TYPE_INFO, PARAMETERS,
};
use std::collections::{HashMap, HashSet};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

/// Builds a MaterialConnection container (upstreamNodePath, upstreamNodeOutputName).
fn build_material_connection(conn: &InputConnection) -> HdDataSourceBaseHandle {
    let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
    if !conn.upstream_node_name.as_str().is_empty() {
        children.insert(
            Token::new("upstreamNodePath"),
            HdRetainedTypedSampledDataSource::new(conn.upstream_node_name.clone())
                as HdDataSourceBaseHandle,
        );
    }
    if !conn.upstream_output_name.as_str().is_empty() {
        children.insert(
            Token::new("upstreamNodeOutputName"),
            HdRetainedTypedSampledDataSource::new(conn.upstream_output_name.clone())
                as HdDataSourceBaseHandle,
        );
    }
    HdRetainedContainerDataSource::new(children) as HdDataSourceBaseHandle
}

/// Builds a MaterialNodeParameter container (value, colorSpace, typeName).
fn build_material_node_parameter(param: &NodeParamData) -> HdDataSourceBaseHandle {
    let mut children: HashMap<Token, HdDataSourceBaseHandle> = HashMap::new();
    children.insert(
        Token::new("value"),
        HdRetainedSampledDataSource::new(param.value.clone()) as HdDataSourceBaseHandle,
    );
    if !param.color_space.as_str().is_empty() {
        children.insert(
            Token::new("colorSpace"),
            HdRetainedTypedSampledDataSource::new(param.color_space.clone())
                as HdDataSourceBaseHandle,
        );
    }
    if !param.type_name.as_str().is_empty() {
        children.insert(
            Token::new("typeName"),
            HdRetainedTypedSampledDataSource::new(param.type_name.clone())
                as HdDataSourceBaseHandle,
        );
    }
    HdRetainedContainerDataSource::new(children) as HdDataSourceBaseHandle
}

/// Implements HdMaterialNetworkInterface for data sources with overlay support.
pub struct HdDataSourceMaterialNetworkInterface {
    material_prim_path: SdfPath,
    network_schema: HdMaterialNetworkSchema,
    network_editor: HdContainerDataSourceEditor,
    prim_container: Option<HdContainerDataSourceHandle>,
    existing_overrides: HashMap<HdDataSourceLocator, Option<HdDataSourceBaseHandle>>,
    overridden_nodes: HashSet<Token>,
    deleted_nodes: HashSet<Token>,
    terminals_overridden: bool,
    node_type_info_editors: HashMap<Token, HdContainerDataSourceEditor>,
}

impl HdDataSourceMaterialNetworkInterface {
    /// Creates a new interface for the given network container.
    pub fn new(
        material_prim_path: SdfPath,
        network_container: HdContainerDataSourceHandle,
        prim_container: Option<HdContainerDataSourceHandle>,
    ) -> Self {
        Self {
            material_prim_path,
            network_schema: HdMaterialNetworkSchema::new(network_container.clone()),
            network_editor: HdContainerDataSourceEditor::new(Some(network_container)),
            prim_container,
            existing_overrides: HashMap::new(),
            overridden_nodes: HashSet::new(),
            deleted_nodes: HashSet::new(),
            terminals_overridden: false,
            node_type_info_editors: HashMap::new(),
        }
    }

    fn nodes_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(material_network::NODES.clone())
    }

    fn terminals_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(material_network::TERMINALS.clone())
    }

    fn get_node(&self, node_name: &Token) -> Option<HdMaterialNodeSchema> {
        if self.deleted_nodes.contains(node_name) {
            return None;
        }
        let nodes = self.network_schema.get_nodes()?;
        let node_container = nodes.get(node_name)?;
        let cont = cast_to_container(&node_container)?;
        Some(HdMaterialNodeSchema::new(cont))
    }

    fn get_node_parameters(&self, node_name: &Token) -> Option<HdContainerDataSourceHandle> {
        self.get_node(node_name)?.get_parameters()
    }

    fn get_node_input_connections(&self, node_name: &Token) -> Option<HdContainerDataSourceHandle> {
        self.get_node(node_name)?.get_input_connections()
    }

    fn set_override(&mut self, loc: HdDataSourceLocator, ds: Option<HdDataSourceBaseHandle>) {
        self.network_editor.set(&loc, ds.clone());
        self.existing_overrides.insert(loc.clone(), ds);

        let nodes_loc = Self::nodes_locator();
        let terminals_loc = Self::terminals_locator();

        if loc.has_prefix(&nodes_loc) && loc.len() > 1 {
            if let Some(name) = loc.get_element(1) {
                self.overridden_nodes.insert(name.clone());
                self.deleted_nodes.remove(name);
            }
        } else if loc.has_prefix(&terminals_loc) {
            self.terminals_overridden = true;
        }
    }

    fn get_override(&self, loc: &HdDataSourceLocator) -> Option<Option<&HdDataSourceBaseHandle>> {
        self.existing_overrides.get(loc).map(|v| v.as_ref())
    }
}

impl HdMaterialNetworkInterface for HdDataSourceMaterialNetworkInterface {
    fn get_material_prim_path(&self) -> SdfPath {
        self.material_prim_path.clone()
    }

    fn get_material_config_keys(&self) -> Vec<Token> {
        let config = self.network_schema.get_config();
        config.map(|c| c.get_names()).unwrap_or_default()
    }

    fn get_material_config_value(&self, key: &Token) -> Value {
        let config = self.network_schema.get_config();
        let Some(config) = config else {
            return Value::default();
        };
        let Some(key_ds) = config.get(key) else {
            return Value::default();
        };
        if let Some(s) = key_ds.as_sampled() {
            return s.get_value(0.0);
        }
        Value::default()
    }

    fn get_model_asset_name(&self) -> String {
        if let Some(ref prim) = self.prim_container {
            if let Some(model) = prim.get(&Token::new("model")) {
                if let Some(cont) = cast_to_container(&model) {
                    if let Some(asset) = cont.get(&Token::new("assetName")) {
                        if let Some(t) = asset
                            .as_any()
                            .downcast_ref::<HdRetainedTypedSampledDataSource<String>>()
                        {
                            return t.get_typed_value(0.0);
                        }
                    }
                }
            }
        }
        String::new()
    }

    fn get_node_names(&self) -> Vec<Token> {
        let mut result = self
            .network_schema
            .get_nodes()
            .map(|n| n.get_names())
            .unwrap_or_default();
        for deleted in &self.deleted_nodes {
            result.retain(|n| n != deleted);
        }
        result
    }

    fn get_node_type(&self, node_name: &Token) -> Token {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            NODE_IDENTIFIER.clone(),
        ]);
        if let Some(Some(ds)) = self.get_override(&loc) {
            if let Some(t) = ds
                .as_any()
                .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
            {
                return t.get_typed_value(0.0);
            }
            return Token::default(); // blocked
        }
        self.get_node(node_name)
            .and_then(|n| n.get_node_identifier())
            .map(|id| id.get_typed_value(0.0))
            .unwrap_or_default()
    }

    fn get_node_type_info_keys(&self, node_name: &Token) -> Vec<Token> {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            NODE_TYPE_INFO.clone(),
        ]);
        let type_info = if let Some(Some(ds)) = self.get_override(&loc) {
            cast_to_container(&ds)
        } else {
            self.get_node(node_name)
                .and_then(|n| n.get_node_type_info())
        };
        type_info.map(|c| c.get_names()).unwrap_or_default()
    }

    fn get_node_type_info_value(&self, node_name: &Token, key: &Token) -> Value {
        let type_info = {
            let loc = HdDataSourceLocator::new(&[
                material_network::NODES.clone(),
                node_name.clone(),
                NODE_TYPE_INFO.clone(),
            ]);
            if let Some(Some(ds)) = self.get_override(&loc) {
                cast_to_container(&ds)
            } else {
                self.get_node(node_name)
                    .and_then(|n| n.get_node_type_info())
            }
        };
        if let Some(ti) = type_info {
            if let Some(child) = ti.get(key) {
                if let Some(s) = child.as_sampled() {
                    return s.get_value(0.0);
                }
            }
        }
        Value::default()
    }

    fn get_authored_node_parameter_names(&self, node_name: &Token) -> Vec<Token> {
        let mut result = self
            .get_node_parameters(node_name)
            .map(|p| p.get_names())
            .unwrap_or_default();

        if self.overridden_nodes.contains(node_name) {
            let params_loc = HdDataSourceLocator::new(&[
                material_network::NODES.clone(),
                node_name.clone(),
                PARAMETERS.clone(),
            ]);
            let mut name_set: HashSet<Token> = result.into_iter().collect();
            for (loc, ds) in &self.existing_overrides {
                if loc.has_prefix(&params_loc) && loc.len() >= 4 {
                    let param = loc.get_element(3).cloned().unwrap_or_default();
                    if ds.is_some() {
                        name_set.insert(param);
                    } else {
                        name_set.remove(&param);
                    }
                }
            }
            result = name_set.into_iter().collect();
        }
        result
    }

    fn get_node_parameter_value(&self, node_name: &Token, param_name: &Token) -> Value {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            PARAMETERS.clone(),
            param_name.clone(),
        ]);
        if let Some(Some(param)) = self.get_override(&loc) {
            if let Some(cont) = cast_to_container(param) {
                if let Some(value_ds) = cont.get(&Token::new("value")) {
                    if let Some(s) = value_ds.as_sampled() {
                        return s.get_value(0.0);
                    }
                }
            }
            return Value::default();
        }
        let params = self.get_node_parameters(node_name);
        let Some(params) = params else {
            return Value::default();
        };
        let Some(param) = params.get(param_name) else {
            return Value::default();
        };
        let Some(param_cont) = cast_to_container(&param) else {
            return Value::default();
        };
        let Some(value_ds) = param_cont.get(&Token::new("value")) else {
            return Value::default();
        };
        value_ds
            .as_sampled()
            .map(|s| s.get_value(0.0))
            .unwrap_or_default()
    }

    fn get_node_parameter_data(&self, node_name: &Token, param_name: &Token) -> NodeParamData {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            PARAMETERS.clone(),
            param_name.clone(),
        ]);
        let mut data = NodeParamData::default();
        if let Some(Some(param)) = self.get_override(&loc) {
            if let Some(cont) = cast_to_container(param) {
                if let Some(v) = cont.get(&Token::new("value")) {
                    if let Some(s) = v.as_sampled() {
                        data.value = s.get_value(0.0);
                    }
                }
                if let Some(cs) = cont.get(&Token::new("colorSpace")) {
                    if let Some(t) = cs
                        .as_any()
                        .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
                    {
                        data.color_space = t.get_typed_value(0.0);
                    }
                }
                if let Some(tn) = cont.get(&Token::new("typeName")) {
                    if let Some(t) = tn
                        .as_any()
                        .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
                    {
                        data.type_name = t.get_typed_value(0.0);
                    }
                }
                return data;
            }
            return data;
        }
        let params = self.get_node_parameters(node_name);
        let Some(params) = params else { return data };
        let Some(param) = params.get(param_name) else {
            return data;
        };
        let Some(param_cont) = cast_to_container(&param) else {
            return data;
        };
        if let Some(v) = param_cont.get(&Token::new("value")) {
            if let Some(s) = v.as_sampled() {
                data.value = s.get_value(0.0);
            }
        }
        if let Some(cs) = param_cont.get(&Token::new("colorSpace")) {
            if let Some(t) = cs
                .as_any()
                .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
            {
                data.color_space = t.get_typed_value(0.0);
            }
        }
        if let Some(tn) = param_cont.get(&Token::new("typeName")) {
            if let Some(t) = tn
                .as_any()
                .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
            {
                data.type_name = t.get_typed_value(0.0);
            }
        }
        data
    }

    fn get_node_input_connection_names(&self, node_name: &Token) -> Vec<Token> {
        let mut result = self
            .get_node_input_connections(node_name)
            .map(|c| c.get_names())
            .unwrap_or_default();

        if self.overridden_nodes.contains(node_name) {
            let inputs_loc = HdDataSourceLocator::new(&[
                material_network::NODES.clone(),
                node_name.clone(),
                INPUT_CONNECTIONS.clone(),
            ]);
            let mut name_set: HashSet<Token> = result.into_iter().collect();
            for (loc, ds) in &self.existing_overrides {
                if loc.has_prefix(&inputs_loc) && loc.len() >= 4 {
                    let input = loc.get_element(3).cloned().unwrap_or_default();
                    if ds.is_some() {
                        name_set.insert(input);
                    } else {
                        name_set.remove(&input);
                    }
                }
            }
            result = name_set.into_iter().collect();
        }
        result
    }

    fn get_node_input_connection(
        &self,
        node_name: &Token,
        input_name: &Token,
    ) -> InputConnectionVector {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            INPUT_CONNECTIONS.clone(),
            input_name.clone(),
        ]);
        let vector_ds = if let Some(Some(vds)) = self.get_override(&loc) {
            cast_to_vector(vds)
        } else {
            None
        };

        let vector_ds = vector_ds.or_else(|| {
            self.get_node_input_connections(node_name)
                .and_then(|conns| conns.get(input_name))
                .and_then(|v| cast_to_vector(&v))
        });

        let Some(vec_ds) = vector_ds else {
            return Vec::new();
        };

        let n = vec_ds.get_num_elements();
        let mut result = Vec::with_capacity(n);
        for i in 0..n {
            if let Some(elem) = vec_ds.get_element(i) {
                let cont = cast_to_container(&elem);
                if let Some(c) = cont {
                    let mut conn = InputConnection::default();
                    if let Some(np) = c.get(&Token::new("upstreamNodePath")) {
                        if let Some(t) = np
                            .as_any()
                            .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
                        {
                            conn.upstream_node_name = t.get_typed_value(0.0);
                        }
                    }
                    if let Some(no) = c.get(&Token::new("upstreamNodeOutputName")) {
                        if let Some(t) = no
                            .as_any()
                            .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
                        {
                            conn.upstream_output_name = t.get_typed_value(0.0);
                        }
                    }
                    result.push(conn);
                }
            }
        }
        result
    }

    fn delete_node(&mut self, node_name: &Token) {
        let loc = HdDataSourceLocator::new(&[material_network::NODES.clone(), node_name.clone()]);
        self.network_editor.set(&loc, None);
        self.deleted_nodes.insert(node_name.clone());
        self.node_type_info_editors.remove(node_name);
    }

    fn set_node_type(&mut self, node_name: &Token, node_type: Token) {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            NODE_IDENTIFIER.clone(),
        ]);
        if node_type.as_str().is_empty() {
            self.set_override(loc, None);
        } else {
            self.set_override(
                loc,
                Some(HdRetainedTypedSampledDataSource::new(node_type) as HdDataSourceBaseHandle),
            );
        }
    }

    fn set_node_type_info_value(&mut self, node_name: &Token, key: &Token, value: Value) {
        let initial = self
            .get_node(node_name)
            .and_then(|n| n.get_node_type_info());
        let initial = initial.unwrap_or_else(|| {
            HdRetainedContainerDataSource::new_empty() as HdContainerDataSourceHandle
        });
        let finished = {
            let editor = self
                .node_type_info_editors
                .entry(node_name.clone())
                .or_insert_with(|| HdContainerDataSourceEditor::new(Some(initial)));
            editor.set(
                &HdDataSourceLocator::from_token(key.clone()),
                Some(HdRetainedSampledDataSource::new(value) as HdDataSourceBaseHandle),
            );
            editor.finish().map(|c| c as HdDataSourceBaseHandle)
        };
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            NODE_TYPE_INFO.clone(),
        ]);
        self.set_override(loc, finished);
    }

    fn set_node_parameter_value(&mut self, node_name: &Token, param_name: &Token, value: Value) {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            PARAMETERS.clone(),
            param_name.clone(),
        ]);
        let param = NodeParamData {
            value,
            color_space: Token::default(),
            type_name: Token::default(),
        };
        self.set_override(loc, Some(build_material_node_parameter(&param)));
    }

    fn set_node_parameter_data(
        &mut self,
        node_name: &Token,
        param_name: &Token,
        param_data: &NodeParamData,
    ) {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            PARAMETERS.clone(),
            param_name.clone(),
        ]);
        self.set_override(loc, Some(build_material_node_parameter(param_data)));
    }

    fn delete_node_parameter(&mut self, node_name: &Token, param_name: &Token) {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            PARAMETERS.clone(),
            param_name.clone(),
        ]);
        self.set_override(loc, None);
    }

    fn set_node_input_connection(
        &mut self,
        node_name: &Token,
        input_name: &Token,
        connections: &InputConnectionVector,
    ) {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            INPUT_CONNECTIONS.clone(),
            input_name.clone(),
        ]);
        let groups: Vec<HdDataSourceBaseHandle> =
            connections.iter().map(build_material_connection).collect();
        let ds = HdRetainedSmallVectorDataSource::new(&groups);
        self.set_override(loc, Some(ds as HdDataSourceBaseHandle));
    }

    fn delete_node_input_connection(&mut self, node_name: &Token, input_name: &Token) {
        let loc = HdDataSourceLocator::new(&[
            material_network::NODES.clone(),
            node_name.clone(),
            INPUT_CONNECTIONS.clone(),
            input_name.clone(),
        ]);
        self.set_override(loc, None);
    }

    fn get_terminal_names(&self) -> Vec<Token> {
        let mut result = self
            .network_schema
            .get_terminals()
            .map(|t| t.get_names())
            .unwrap_or_default();

        if self.terminals_overridden {
            let terminals_loc = Self::terminals_locator();
            let mut name_set: HashSet<Token> = result.into_iter().collect();
            for (loc, ds) in &self.existing_overrides {
                if loc.has_prefix(&terminals_loc) && loc.len() >= 2 {
                    let term = loc.get_element(1).cloned().unwrap_or_default();
                    if ds.is_some() {
                        name_set.insert(term);
                    } else {
                        name_set.remove(&term);
                    }
                }
            }
            result = name_set.into_iter().collect();
        }
        result
    }

    fn get_terminal_connection(&self, terminal_name: &Token) -> (bool, InputConnection) {
        let loc =
            HdDataSourceLocator::new(&[material_network::TERMINALS.clone(), terminal_name.clone()]);
        let container = if let Some(Some(ds)) = self.get_override(&loc) {
            cast_to_container(ds)
        } else {
            None
        };

        let container = container.or_else(|| {
            self.network_schema
                .get_terminals()
                .and_then(|t| t.get(terminal_name))
                .and_then(|c| cast_to_container(&c))
        });

        if let Some(cont) = container {
            let mut result = InputConnection::default();
            let mut found = true;
            if let Some(np) = cont.get(&Token::new("upstreamNodePath")) {
                if let Some(t) = np
                    .as_any()
                    .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
                {
                    result.upstream_node_name = t.get_typed_value(0.0);
                }
            } else {
                found = false;
            }
            if let Some(no) = cont.get(&Token::new("upstreamNodeOutputName")) {
                if let Some(t) = no
                    .as_any()
                    .downcast_ref::<HdRetainedTypedSampledDataSource<Token>>()
                {
                    result.upstream_output_name = t.get_typed_value(0.0);
                }
            }
            return (found, result);
        }

        if self.get_override(&loc).is_some() {
            return (false, InputConnection::default()); // deleted
        }
        (false, InputConnection::default())
    }

    fn delete_terminal(&mut self, terminal_name: &Token) {
        let loc =
            HdDataSourceLocator::new(&[material_network::TERMINALS.clone(), terminal_name.clone()]);
        self.set_override(loc, None);
    }

    fn set_terminal_connection(&mut self, terminal_name: &Token, connection: &InputConnection) {
        let loc =
            HdDataSourceLocator::new(&[material_network::TERMINALS.clone(), terminal_name.clone()]);
        let ds = build_material_connection(connection);
        self.set_override(loc, Some(ds));
    }
}

impl HdDataSourceMaterialNetworkInterface {
    /// Returns the final container with all edits applied.
    pub fn finish(self) -> HdContainerDataSourceHandle {
        if self.existing_overrides.is_empty() {
            self.network_schema
                .get_container()
                .cloned()
                .unwrap_or_else(|| HdRetainedContainerDataSource::new_empty())
        } else {
            self.network_editor.finish().unwrap_or_else(|| {
                self.network_schema
                    .get_container()
                    .cloned()
                    .unwrap_or_else(|| HdRetainedContainerDataSource::new_empty())
            })
        }
    }
}
