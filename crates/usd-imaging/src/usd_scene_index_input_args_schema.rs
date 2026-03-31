//! UsdSceneIndexInputArgsSchema - Hydra schema for USD scene index input args.
//!
//! Port of pxr/usdImaging/usdImaging/usdSceneIndexInputArgsSchema.h
//!
//! Schema for arguments used when creating a USD-backed scene index.
//! Contains stage, includeUnloadedPrims, displayUnloadedPrimsWithBounds, addDrawModeSceneIndex.
//!
//! Note: Stage data source uses HdDataSourceBaseHandle for flexibility;
//! C++ uses UsdStageRefPtrDataSource (HdTypedSampledDataSource<UsdStageRefPtr>).

use std::sync::Arc;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
    cast_to_container,
};
use usd_hd::schema::HdSchema;
use usd_tf::Token;

// Token constants (USD_IMAGING_USD_SCENE_INDEX_INPUT_ARGS_SCHEMA_TOKENS)
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static USD_SCENE_INDEX: LazyLock<Token> = LazyLock::new(|| Token::new("usdSceneIndex"));
    pub static STAGE: LazyLock<Token> = LazyLock::new(|| Token::new("stage"));
    pub static INCLUDE_UNLOADED_PRIMS: LazyLock<Token> =
        LazyLock::new(|| Token::new("includeUnloadedPrims"));
    pub static DISPLAY_UNLOADED_PRIMS_WITH_BOUNDS: LazyLock<Token> =
        LazyLock::new(|| Token::new("displayUnloadedPrimsWithBounds"));
    pub static ADD_DRAW_MODE_SCENE_INDEX: LazyLock<Token> =
        LazyLock::new(|| Token::new("addDrawModeSceneIndex"));
}

/// Handle to Bool data source.
pub type HdBoolDataSourceHandle = Arc<dyn HdTypedSampledDataSource<bool> + Send + Sync>;

// ============================================================================
// UsdSceneIndexInputArgsSchema
// ============================================================================

/// Schema for USD scene index input arguments.
///
/// Used when constructing a scene index from a USD stage.
/// Contains stage reference and display options.
#[derive(Debug, Clone)]
pub struct UsdSceneIndexInputArgsSchema {
    schema: HdSchema,
}

impl UsdSceneIndexInputArgsSchema {
    /// Create schema from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Create schema from optional container (for undefined case).
    pub fn from_container(container: Option<HdContainerDataSourceHandle>) -> Self {
        Self {
            schema: container.map(HdSchema::new).unwrap_or_else(HdSchema::empty),
        }
    }

    /// Check if this schema is defined.
    pub fn is_defined(&self) -> bool {
        self.schema.is_defined()
    }

    /// Get stage data source (raw handle; stage type is UsdStage in C++).
    pub fn get_stage(&self) -> Option<HdDataSourceBaseHandle> {
        self.schema
            .get_container()
            .and_then(|c| c.get(&tokens::STAGE))
    }

    /// Get stage as Arc<Stage> from schema (for create_scene_indices overload).
    ///
    /// Port of schema.GetStage()->GetTypedValue(0.0f).
    pub fn get_stage_typed_value(&self) -> Option<std::sync::Arc<usd_core::Stage>> {
        use super::usd_stage_data_source::UsdStageRefPtrDataSource;
        self.schema
            .get_typed::<UsdStageRefPtrDataSource>(&tokens::STAGE)
            .map(|ds| ds.get_typed_value(0.0))
    }

    /// Get the container that this schema wraps (usdSceneIndex sub-container).
    ///
    /// Port of schema.GetContainer() used for stageSceneIndexInputArgs.
    pub fn get_container(&self) -> Option<HdContainerDataSourceHandle> {
        self.schema.get_container().cloned()
    }

    /// Get include unloaded prims data source.
    pub fn get_include_unloaded_prims(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<bool>>(&tokens::INCLUDE_UNLOADED_PRIMS)
            .map(|arc| arc as HdBoolDataSourceHandle)
    }

    /// Get display unloaded prims with bounds data source.
    pub fn get_display_unloaded_prims_with_bounds(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<bool>>(
                &tokens::DISPLAY_UNLOADED_PRIMS_WITH_BOUNDS,
            )
            .map(|arc| arc as HdBoolDataSourceHandle)
    }

    /// Get add draw mode scene index data source.
    pub fn get_add_draw_mode_scene_index(&self) -> Option<HdBoolDataSourceHandle> {
        self.schema
            .get_typed::<HdRetainedTypedSampledDataSource<bool>>(&tokens::ADD_DRAW_MODE_SCENE_INDEX)
            .map(|arc| arc as HdBoolDataSourceHandle)
    }

    /// Get schema token.
    pub fn get_schema_token() -> Token {
        tokens::USD_SCENE_INDEX.clone()
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(tokens::USD_SCENE_INDEX.clone())
    }

    /// Get schema from parent container.
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Self {
        if let Some(child) = parent.get(&tokens::USD_SCENE_INDEX) {
            if let Some(container) = cast_to_container(&child) {
                return Self::new(container);
            }
        }
        Self::from_container(None)
    }

    /// Build retained container with optional fields.
    pub fn build_retained(
        stage: Option<HdDataSourceBaseHandle>,
        include_unloaded_prims: Option<HdBoolDataSourceHandle>,
        display_unloaded_prims_with_bounds: Option<HdBoolDataSourceHandle>,
        add_draw_mode_scene_index: Option<HdBoolDataSourceHandle>,
    ) -> HdContainerDataSourceHandle {
        let mut entries: Vec<(Token, HdDataSourceBaseHandle)> = Vec::new();
        if let Some(s) = stage {
            entries.push((tokens::STAGE.clone(), s));
        }
        if let Some(v) = include_unloaded_prims {
            entries.push((
                tokens::INCLUDE_UNLOADED_PRIMS.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = display_unloaded_prims_with_bounds {
            entries.push((
                tokens::DISPLAY_UNLOADED_PRIMS_WITH_BOUNDS.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        if let Some(v) = add_draw_mode_scene_index {
            entries.push((
                tokens::ADD_DRAW_MODE_SCENE_INDEX.clone(),
                v as HdDataSourceBaseHandle,
            ));
        }
        HdRetainedContainerDataSource::from_entries(&entries)
    }
}

// ============================================================================
// UsdSceneIndexInputArgsSchemaBuilder
// ============================================================================

/// Builder for UsdSceneIndexInputArgsSchema data sources.
#[derive(Debug, Default)]
pub struct UsdSceneIndexInputArgsSchemaBuilder {
    stage: Option<HdDataSourceBaseHandle>,
    include_unloaded_prims: Option<HdBoolDataSourceHandle>,
    display_unloaded_prims_with_bounds: Option<HdBoolDataSourceHandle>,
    add_draw_mode_scene_index: Option<HdBoolDataSourceHandle>,
}

impl UsdSceneIndexInputArgsSchemaBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set stage.
    pub fn set_stage(mut self, stage: HdDataSourceBaseHandle) -> Self {
        self.stage = Some(stage);
        self
    }

    /// Set include unloaded prims.
    pub fn set_include_unloaded_prims(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.include_unloaded_prims = Some(v);
        self
    }

    /// Set display unloaded prims with bounds.
    pub fn set_display_unloaded_prims_with_bounds(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.display_unloaded_prims_with_bounds = Some(v);
        self
    }

    /// Set add draw mode scene index.
    pub fn set_add_draw_mode_scene_index(mut self, v: HdBoolDataSourceHandle) -> Self {
        self.add_draw_mode_scene_index = Some(v);
        self
    }

    /// Build the container data source.
    pub fn build(self) -> HdContainerDataSourceHandle {
        UsdSceneIndexInputArgsSchema::build_retained(
            self.stage,
            self.include_unloaded_prims,
            self.display_unloaded_prims_with_bounds,
            self.add_draw_mode_scene_index,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::HdRetainedTypedSampledDataSource;

    #[test]
    fn test_schema_token() {
        assert_eq!(
            UsdSceneIndexInputArgsSchema::get_schema_token().as_str(),
            "usdSceneIndex"
        );
    }

    #[test]
    fn test_default_locator() {
        let locator = UsdSceneIndexInputArgsSchema::get_default_locator();
        assert!(locator.first_element().is_some());
    }

    #[test]
    fn test_build_retained() {
        let include = HdRetainedTypedSampledDataSource::new(true);
        let display = HdRetainedTypedSampledDataSource::new(false);
        let container =
            UsdSceneIndexInputArgsSchema::build_retained(None, Some(include), Some(display), None);
        let schema = UsdSceneIndexInputArgsSchema::new(container);
        assert!(schema.is_defined());
    }

    #[test]
    fn test_builder() {
        let include = HdRetainedTypedSampledDataSource::new(false);
        let _container = UsdSceneIndexInputArgsSchemaBuilder::new()
            .set_include_unloaded_prims(include)
            .build();
    }

    #[test]
    fn test_tokens() {
        assert_eq!(tokens::STAGE.as_str(), "stage");
        assert_eq!(
            tokens::INCLUDE_UNLOADED_PRIMS.as_str(),
            "includeUnloadedPrims"
        );
        assert_eq!(
            tokens::DISPLAY_UNLOADED_PRIMS_WITH_BOUNDS.as_str(),
            "displayUnloadedPrimsWithBounds"
        );
        assert_eq!(
            tokens::ADD_DRAW_MODE_SCENE_INDEX.as_str(),
            "addDrawModeSceneIndex"
        );
    }
}
