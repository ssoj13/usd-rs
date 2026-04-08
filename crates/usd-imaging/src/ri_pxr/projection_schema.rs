//! Projection schema for RenderMan camera projections.
//!
//! Port of `pxr/usdImaging/usdRiPxrImaging/projectionSchema.h`.

use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocator, cast_to_container};
use usd_tf::Token;

mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static PROJECTION: LazyLock<Token> = LazyLock::new(|| Token::new("projection"));
    pub static RESOURCE: LazyLock<Token> = LazyLock::new(|| Token::new("resource"));
}

#[derive(Debug, Clone)]
pub struct ProjectionSchema {
    container: HdContainerDataSourceHandle,
}

impl ProjectionSchema {
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self { container }
    }

    pub fn is_defined(&self) -> bool {
        true
    }

    pub fn get_schema_token() -> Token {
        tokens::PROJECTION.clone()
    }

    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::PROJECTION.clone())
    }

    pub fn get_resource_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::new(&[tokens::PROJECTION.clone(), tokens::RESOURCE.clone()])
    }

    pub fn get_resource(&self) -> Option<HdContainerDataSourceHandle> {
        self.container
            .get(&tokens::RESOURCE)
            .as_ref()
            .and_then(cast_to_container)
    }

    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        parent
            .get(&tokens::PROJECTION)
            .as_ref()
            .and_then(cast_to_container)
            .map(Self::new)
    }
}

#[derive(Debug, Default)]
pub struct ProjectionSchemaBuilder {
    resource: Option<HdContainerDataSourceHandle>,
}

impl ProjectionSchemaBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_resource(mut self, resource: HdContainerDataSourceHandle) -> Self {
        self.resource = Some(resource);
        self
    }

    pub fn build(self) -> HdContainerDataSourceHandle {
        let mut entries = Vec::new();
        if let Some(resource) = self.resource {
            entries.push((
                tokens::RESOURCE.clone(),
                resource as usd_hd::HdDataSourceBaseHandle,
            ));
        }
        usd_hd::HdRetainedContainerDataSource::from_entries(&entries) as HdContainerDataSourceHandle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::HdRetainedContainerDataSource;

    #[test]
    fn test_schema_token() {
        assert_eq!(ProjectionSchema::get_schema_token().as_str(), "projection");
    }

    #[test]
    fn test_builder_round_trip() {
        let resource: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        let projection = ProjectionSchemaBuilder::new()
            .set_resource(resource.clone())
            .build();
        let parent: HdContainerDataSourceHandle = HdRetainedContainerDataSource::from_entries(&[(
            Token::new("projection"),
            projection as usd_hd::HdDataSourceBaseHandle,
        )]);
        let schema = ProjectionSchema::get_from_parent(&parent).expect("schema");
        assert!(schema.get_resource().is_some());
    }
}
