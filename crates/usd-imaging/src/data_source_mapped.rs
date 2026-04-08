//! DataSourceMapped - Property mapping data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourceMapped.h/.cpp
//!
//! A data source that is a (potentially nested) container for USD attributes
//! and relationships of a prim, mapped to Hydra locator paths.

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use std::sync::Arc;
use usd_core::{Attribute, Prim, Relationship};
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

pub type DataSourceAttributeFactoryFn = fn(
    Attribute,
    DataSourceStageGlobalsHandle,
    Path,
    HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle>;
pub type DataSourceRelationshipFactoryFn = fn(
    Relationship,
    DataSourceStageGlobalsHandle,
    Path,
    HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle>;

fn default_attribute_factory(
    attr: Attribute,
    stage_globals: DataSourceStageGlobalsHandle,
    scene_index_path: Path,
    locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    Some(DataSourceAttribute::<Value>::new_with_locator(
        attr,
        stage_globals,
        scene_index_path,
        locator,
    ) as HdDataSourceBaseHandle)
}

fn path_from_relationship_factory(
    rel: Relationship,
    _stage_globals: DataSourceStageGlobalsHandle,
    _scene_index_path: Path,
    _locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    rel.get_forwarded_targets()
        .into_iter()
        .next()
        .map(|path| HdRetainedTypedSampledDataSource::<Path>::new(path) as HdDataSourceBaseHandle)
}

fn path_array_from_relationship_factory(
    rel: Relationship,
    _stage_globals: DataSourceStageGlobalsHandle,
    _scene_index_path: Path,
    _locator: HdDataSourceLocator,
) -> Option<HdDataSourceBaseHandle> {
    Some(
        HdRetainedTypedSampledDataSource::<Vec<Path>>::new(rel.get_forwarded_targets())
            as HdDataSourceBaseHandle,
    )
}

#[derive(Debug, Clone)]
pub struct PropertyMappingBase {
    pub usd_name: Token,
    pub hd_locator: HdDataSourceLocator,
}

impl PropertyMappingBase {
    pub fn new(usd_name: Token, hd_locator: HdDataSourceLocator) -> Self {
        Self {
            usd_name,
            hd_locator,
        }
    }
}

#[derive(Clone)]
pub struct AttributeMapping {
    pub base: PropertyMappingBase,
    pub factory: DataSourceAttributeFactoryFn,
}

impl std::fmt::Debug for AttributeMapping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttributeMapping")
            .field("usd_name", &self.base.usd_name)
            .field("hd_locator", &self.base.hd_locator)
            .finish()
    }
}

impl AttributeMapping {
    pub fn new(usd_name: Token, hd_locator: HdDataSourceLocator) -> Self {
        Self::new_with_factory(usd_name, hd_locator, default_attribute_factory)
    }

    pub fn new_with_factory(
        usd_name: Token,
        hd_locator: HdDataSourceLocator,
        factory: DataSourceAttributeFactoryFn,
    ) -> Self {
        Self {
            base: PropertyMappingBase::new(usd_name, hd_locator),
            factory,
        }
    }
}

#[derive(Clone)]
pub struct RelationshipMapping {
    pub base: PropertyMappingBase,
    pub factory: DataSourceRelationshipFactoryFn,
}

impl std::fmt::Debug for RelationshipMapping {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelationshipMapping")
            .field("usd_name", &self.base.usd_name)
            .field("hd_locator", &self.base.hd_locator)
            .finish()
    }
}

impl RelationshipMapping {
    pub fn new(usd_name: Token, hd_locator: HdDataSourceLocator) -> Self {
        Self::new_with_factory(usd_name, hd_locator, path_from_relationship_factory)
    }

    pub fn new_path_array(usd_name: Token, hd_locator: HdDataSourceLocator) -> Self {
        Self::new_with_factory(usd_name, hd_locator, path_array_from_relationship_factory)
    }

    pub fn new_with_factory(
        usd_name: Token,
        hd_locator: HdDataSourceLocator,
        factory: DataSourceRelationshipFactoryFn,
    ) -> Self {
        Self {
            base: PropertyMappingBase::new(usd_name, hd_locator),
            factory,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PropertyMapping {
    Attribute(AttributeMapping),
    Relationship(RelationshipMapping),
}

impl PropertyMapping {
    pub fn base(&self) -> &PropertyMappingBase {
        match self {
            Self::Attribute(m) => &m.base,
            Self::Relationship(m) => &m.base,
        }
    }
}

#[derive(Debug, Clone)]
enum DataSourceInfo {
    Attribute(AttributeMapping),
    Relationship(RelationshipMapping),
    Container(Box<ContainerMappings>),
}

#[derive(Debug, Clone, Default)]
struct ContainerMappings {
    hd_names: Vec<Token>,
    data_source_infos: Vec<DataSourceInfo>,
}

fn find_or_create_child<'a>(
    name: &Token,
    container: &'a mut ContainerMappings,
) -> Option<&'a mut ContainerMappings> {
    match container.hd_names.binary_search(name) {
        Ok(index) => match container.data_source_infos.get_mut(index) {
            Some(DataSourceInfo::Container(child)) => Some(child.as_mut()),
            _ => None,
        },
        Err(index) => {
            container.hd_names.insert(index, name.clone());
            container
                .data_source_infos
                .insert(index, DataSourceInfo::Container(Box::default()));
            match container.data_source_infos.get_mut(index) {
                Some(DataSourceInfo::Container(child)) => Some(child.as_mut()),
                _ => None,
            }
        }
    }
}

fn add_mapping(
    locator: &HdDataSourceLocator,
    info: DataSourceInfo,
    container: &mut ContainerMappings,
) {
    if locator.is_empty() {
        return;
    }

    if locator.len() == 1 {
        let Some(name) = locator.first_element().cloned() else {
            return;
        };
        match container.hd_names.binary_search(&name) {
            Ok(index) => container.data_source_infos[index] = info,
            Err(index) => {
                container.hd_names.insert(index, name);
                container.data_source_infos.insert(index, info);
            }
        }
        return;
    }

    let Some(first) = locator.first_element().cloned() else {
        return;
    };
    let child = match find_or_create_child(&first, container) {
        Some(child) => child,
        None => return,
    };
    add_mapping(&locator.remove_first(), info, child);
}

#[derive(Debug, Clone)]
pub struct PropertyMappings {
    absolute_mappings: Vec<PropertyMappingBase>,
    container_mappings: Arc<ContainerMappings>,
}

impl PropertyMappings {
    pub fn new(mappings: Vec<PropertyMapping>, data_source_prefix: HdDataSourceLocator) -> Self {
        let mut absolute_mappings = Vec::with_capacity(mappings.len());
        let mut container_mappings = ContainerMappings::default();

        for mapping in mappings {
            match mapping {
                PropertyMapping::Attribute(attr_mapping) => {
                    let locator = data_source_prefix.append_locator(&attr_mapping.base.hd_locator);
                    absolute_mappings.push(PropertyMappingBase::new(
                        attr_mapping.base.usd_name.clone(),
                        locator,
                    ));
                    add_mapping(
                        &attr_mapping.base.hd_locator,
                        DataSourceInfo::Attribute(AttributeMapping {
                            base: PropertyMappingBase::new(
                                attr_mapping.base.usd_name,
                                data_source_prefix.append_locator(&attr_mapping.base.hd_locator),
                            ),
                            factory: attr_mapping.factory,
                        }),
                        &mut container_mappings,
                    );
                }
                PropertyMapping::Relationship(rel_mapping) => {
                    let locator = data_source_prefix.append_locator(&rel_mapping.base.hd_locator);
                    absolute_mappings.push(PropertyMappingBase::new(
                        rel_mapping.base.usd_name.clone(),
                        locator,
                    ));
                    add_mapping(
                        &rel_mapping.base.hd_locator,
                        DataSourceInfo::Relationship(RelationshipMapping {
                            base: PropertyMappingBase::new(
                                rel_mapping.base.usd_name,
                                data_source_prefix.append_locator(&rel_mapping.base.hd_locator),
                            ),
                            factory: rel_mapping.factory,
                        }),
                        &mut container_mappings,
                    );
                }
            }
        }

        Self {
            absolute_mappings,
            container_mappings: Arc::new(container_mappings),
        }
    }

    pub fn get_absolute_mappings(&self) -> &[PropertyMappingBase] {
        &self.absolute_mappings
    }

    fn root_container(&self) -> Arc<ContainerMappings> {
        self.container_mappings.clone()
    }
}

#[derive(Clone)]
pub struct DataSourceMapped {
    prim: Prim,
    scene_index_path: Path,
    container_mappings: Arc<ContainerMappings>,
    stage_globals: DataSourceStageGlobalsHandle,
}

impl DataSourceMapped {
    pub fn new(
        prim: Prim,
        scene_index_path: Path,
        mappings: PropertyMappings,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            prim,
            scene_index_path,
            container_mappings: mappings.root_container(),
            stage_globals,
        }
    }

    fn from_container_mappings(
        prim: Prim,
        scene_index_path: Path,
        container_mappings: Arc<ContainerMappings>,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            prim,
            scene_index_path,
            container_mappings,
            stage_globals,
        }
    }

    pub fn invalidate(usd_names: &[Token], mappings: &PropertyMappings) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::new();
        for usd_name in usd_names {
            for mapping in mappings.get_absolute_mappings() {
                if usd_name == &mapping.usd_name {
                    locators.insert(mapping.hd_locator.clone());
                }
            }
        }
        locators
    }
}

impl std::fmt::Debug for DataSourceMapped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceMapped")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceMapped {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceMapped {
    fn get_names(&self) -> Vec<Token> {
        self.container_mappings.hd_names.clone()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if !self.prim.is_valid() {
            return None;
        }

        let index = self.container_mappings.hd_names.binary_search(name).ok()?;
        let info = self.container_mappings.data_source_infos.get(index)?;

        match info {
            DataSourceInfo::Attribute(mapping) => {
                let attr = self.prim.get_attribute(mapping.base.usd_name.as_str())?;
                (mapping.factory)(
                    attr,
                    self.stage_globals.clone(),
                    self.scene_index_path.clone(),
                    mapping.base.hd_locator.clone(),
                )
            }
            DataSourceInfo::Relationship(mapping) => {
                let rel = self.prim.get_relationship(mapping.base.usd_name.as_str())?;
                (mapping.factory)(
                    rel,
                    self.stage_globals.clone(),
                    self.scene_index_path.clone(),
                    mapping.base.hd_locator.clone(),
                )
            }
            DataSourceInfo::Container(child) => Some(Arc::new(Self::from_container_mappings(
                self.prim.clone(),
                self.scene_index_path.clone(),
                Arc::new((**child).clone()),
                self.stage_globals.clone(),
            )) as HdDataSourceBaseHandle),
        }
    }
}

pub type DataSourceMappedHandle = Arc<DataSourceMapped>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_property_mapping() {
        let mapping = AttributeMapping::new(
            Token::new("radius"),
            HdDataSourceLocator::from_token(Token::new("radius")),
        );

        assert_eq!(mapping.base.usd_name.as_str(), "radius");
    }

    #[test]
    fn test_property_mappings_invalidation() {
        let mappings = PropertyMappings::new(
            vec![PropertyMapping::Attribute(AttributeMapping::new(
                Token::new("widths"),
                HdDataSourceLocator::from_token(Token::new("primvars")),
            ))],
            HdDataSourceLocator::empty(),
        );

        let usd_names = vec![Token::new("widths")];
        let locators = DataSourceMapped::invalidate(&usd_names, &mappings);

        assert!(!locators.is_empty());
    }

    #[test]
    fn test_data_source_mapped_creation() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();
        let mappings = PropertyMappings::new(vec![], HdDataSourceLocator::empty());

        let _ds = DataSourceMapped::new(prim, Path::absolute_root(), mappings, globals);
    }
}
