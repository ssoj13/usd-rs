
//! HdSplitDiopterSchema - Split diopter schema.
//!
//! Corresponds to pxr/imaging/hd/splitDiopterSchema.h.

use super::base::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_tf::Token;

static SPLIT_DIOPTER: Lazy<Token> = Lazy::new(|| Token::new("splitDiopter"));
static COUNT: Lazy<Token> = Lazy::new(|| Token::new("count"));
static ANGLE: Lazy<Token> = Lazy::new(|| Token::new("angle"));
static OFFSET1: Lazy<Token> = Lazy::new(|| Token::new("offset1"));
static WIDTH1: Lazy<Token> = Lazy::new(|| Token::new("width1"));
static FOCUS_DISTANCE1: Lazy<Token> = Lazy::new(|| Token::new("focusDistance1"));
static OFFSET2: Lazy<Token> = Lazy::new(|| Token::new("offset2"));
static WIDTH2: Lazy<Token> = Lazy::new(|| Token::new("width2"));
static FOCUS_DISTANCE2: Lazy<Token> = Lazy::new(|| Token::new("focusDistance2"));

/// Int data source handle.
pub type HdIntDataSourceHandle = Arc<dyn HdTypedSampledDataSource<i32> + Send + Sync>;
/// Float data source handle.
pub type HdFloatDataSourceHandle = Arc<dyn HdTypedSampledDataSource<f32> + Send + Sync>;

/// Schema for split diopter (tilt-shift / miniature effect).
#[derive(Debug, Clone)]
pub struct HdSplitDiopterSchema {
    schema: HdSchema,
}

impl HdSplitDiopterSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "splitDiopter".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        parent
            .get(&SPLIT_DIOPTER)
            .and_then(|h| crate::data_source::cast_to_container(&h).map(Self::new))
    }

    /// Get count.
    pub fn get_count(&self) -> Option<HdIntDataSourceHandle> {
        self.schema.get_typed(&COUNT)
    }

    /// Get angle.
    pub fn get_angle(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&ANGLE)
    }

    /// Get offset1.
    pub fn get_offset1(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&OFFSET1)
    }

    /// Get width1.
    pub fn get_width1(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&WIDTH1)
    }

    /// Get focus distance 1.
    pub fn get_focus_distance1(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&FOCUS_DISTANCE1)
    }

    /// Get offset2.
    pub fn get_offset2(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&OFFSET2)
    }

    /// Get width2.
    pub fn get_width2(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&WIDTH2)
    }

    /// Get focus distance 2.
    pub fn get_focus_distance2(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&FOCUS_DISTANCE2)
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &SPLIT_DIOPTER
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(SPLIT_DIOPTER.clone())
    }
}
