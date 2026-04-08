//! Data source for RenderMan render terminal prims.
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging/dataSourcePxrRenderTerminalPrims.h`.

use std::sync::Arc;

use super::render_terminal_helper::RenderTerminalHelper;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::types::PropertyInvalidationType;
use usd_core::Prim;
use usd_hd::schema::{
    HdDisplayFilterSchema, HdIntegratorSchema, HdMaterialNodeSchema, HdSampleFilterSchema,
};
use usd_hd::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdDataSourceLocatorSet, HdRetainedContainerDataSource, HdRetainedSampledDataSource,
    HdRetainedTypedSampledDataSource,
};
use usd_tf::Token;

fn build_resource_ds(
    prim: &Prim,
    shader_id: &Token,
    prim_type: &Token,
) -> HdContainerDataSourceHandle {
    let node = RenderTerminalHelper::create_hd_material_node2(prim, shader_id, prim_type);
    let mut parameter_entries = Vec::with_capacity(node.parameters.len());
    for (name, value) in node.parameters {
        let param_ds: HdContainerDataSourceHandle =
            HdRetainedContainerDataSource::from_entries(&[(
                Token::new("value"),
                HdRetainedSampledDataSource::new(value) as HdDataSourceBaseHandle,
            )]);
        parameter_entries.push((name, param_ds as HdDataSourceBaseHandle));
    }

    let parameters = if parameter_entries.is_empty() {
        None
    } else {
        Some(
            HdRetainedContainerDataSource::from_entries(&parameter_entries)
                as HdContainerDataSourceHandle,
        )
    };
    HdMaterialNodeSchema::build_retained(
        parameters,
        None,
        Some(HdRetainedTypedSampledDataSource::new(node.node_type_id)),
        None,
        None,
    )
}

#[derive(Debug)]
pub struct DataSourceRenderTerminalPrim {
    schema_token: Token,
    shader_id: Token,
    prim: Prim,
}

impl DataSourceRenderTerminalPrim {
    pub fn new(
        _scene_index_path: &usd_sdf::Path,
        prim: Prim,
        schema_token: Token,
        shader_id: Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            schema_token,
            shader_id,
            prim,
        }
    }

    pub fn get_names(&self) -> Vec<Token> {
        vec![self.schema_token.clone()]
    }

    pub fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name != &self.schema_token {
            return None;
        }

        let resource = build_resource_ds(&self.prim, &self.shader_id, name);
        let terminal = HdRetainedContainerDataSource::from_entries(&[(
            Token::new("resource"),
            resource as HdDataSourceBaseHandle,
        )]);
        Some(terminal as HdDataSourceBaseHandle)
    }

    pub fn invalidate(
        _prim: &Prim,
        _subprim: &Token,
        properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
        resource_locator: &HdDataSourceLocator,
    ) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::empty();
        for property in properties {
            if RenderTerminalHelper::has_input_prefix(property.as_str()) {
                locators.insert(resource_locator.clone());
            }
        }
        locators
    }
}

pub type DataSourceRenderTerminalPrimHandle = Arc<DataSourceRenderTerminalPrim>;
pub type DataSourceIntegratorPrim = DataSourceRenderTerminalPrim;
pub type DataSourceSampleFilterPrim = DataSourceRenderTerminalPrim;
pub type DataSourceDisplayFilterPrim = DataSourceRenderTerminalPrim;

pub fn integrator_resource_locator() -> HdDataSourceLocator {
    HdIntegratorSchema::get_resource_locator()
}

pub fn sample_filter_resource_locator() -> HdDataSourceLocator {
    HdSampleFilterSchema::get_resource_locator()
}

pub fn display_filter_resource_locator() -> HdDataSourceLocator {
    HdDisplayFilterSchema::get_resource_locator()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    #[test]
    fn test_data_source_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());
        let ds = DataSourceRenderTerminalPrim::new(
            &usd_sdf::Path::absolute_root(),
            stage.get_pseudo_root(),
            Token::new("integrator"),
            Token::new("ri:integrator:shaderId"),
            &globals,
        );
        assert_eq!(ds.get_names(), vec![Token::new("integrator")]);
    }

    #[test]
    fn test_invalidate_inputs_marks_resource_locator() {
        let properties = vec![Token::new("inputs:maxSamples")];
        let locators = DataSourceRenderTerminalPrim::invalidate(
            &Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
                .unwrap()
                .get_pseudo_root(),
            &Token::new(""),
            &properties,
            PropertyInvalidationType::PropertyChanged,
            &integrator_resource_locator(),
        );
        assert!(!locators.is_empty());
    }
}
