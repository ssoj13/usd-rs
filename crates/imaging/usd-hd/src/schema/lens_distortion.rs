
//! HdLensDistortionSchema - Lens distortion schema.
//!
//! Corresponds to pxr/imaging/hd/lensDistortionSchema.h.

use super::base::HdSchema;
use crate::data_source::{
    HdContainerDataSourceHandle, HdDataSourceLocator, HdTypedSampledDataSource,
};
use once_cell::sync::Lazy;
use std::sync::Arc;
use usd_gf::Vec2f;
use usd_tf::Token;

static LENS_DISTORTION: Lazy<Token> = Lazy::new(|| Token::new("lensDistortion"));
static TYPE: Lazy<Token> = Lazy::new(|| Token::new("type"));
static K1: Lazy<Token> = Lazy::new(|| Token::new("k1"));
static K2: Lazy<Token> = Lazy::new(|| Token::new("k2"));
static CENTER: Lazy<Token> = Lazy::new(|| Token::new("center"));
static ANA_SQ: Lazy<Token> = Lazy::new(|| Token::new("anaSq"));
static ASYM: Lazy<Token> = Lazy::new(|| Token::new("asym"));
static SCALE: Lazy<Token> = Lazy::new(|| Token::new("scale"));
static IOR: Lazy<Token> = Lazy::new(|| Token::new("ior"));
static STANDARD: Lazy<Token> = Lazy::new(|| Token::new("standard"));
static FISHEYE: Lazy<Token> = Lazy::new(|| Token::new("fisheye"));

/// Token data source handle.
pub type HdTokenDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Token> + Send + Sync>;
/// Float data source handle.
pub type HdFloatDataSourceHandle = Arc<dyn HdTypedSampledDataSource<f32> + Send + Sync>;
/// Vec2f data source handle.
pub type HdVec2fDataSourceHandle = Arc<dyn HdTypedSampledDataSource<Vec2f> + Send + Sync>;

/// Schema for lens distortion.
#[derive(Debug, Clone)]
pub struct HdLensDistortionSchema {
    schema: HdSchema,
}

impl HdLensDistortionSchema {
    /// Construct from container.
    pub fn new(container: HdContainerDataSourceHandle) -> Self {
        Self {
            schema: HdSchema::new(container),
        }
    }

    /// Get from parent container at "lensDistortion".
    pub fn get_from_parent(parent: &HdContainerDataSourceHandle) -> Option<Self> {
        parent
            .get(&LENS_DISTORTION)
            .and_then(|h| crate::data_source::cast_to_container(&h).map(Self::new))
    }

    /// Get distortion type.
    pub fn get_type(&self) -> Option<HdTokenDataSourceHandle> {
        self.schema.get_typed(&TYPE)
    }

    /// Get k1 coefficient.
    pub fn get_k1(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&K1)
    }

    /// Get k2 coefficient.
    pub fn get_k2(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&K2)
    }

    /// Get center point.
    pub fn get_center(&self) -> Option<HdVec2fDataSourceHandle> {
        self.schema.get_typed(&CENTER)
    }

    /// Get anaSq parameter.
    pub fn get_ana_sq(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&ANA_SQ)
    }

    /// Get asym parameter.
    pub fn get_asym(&self) -> Option<HdVec2fDataSourceHandle> {
        self.schema.get_typed(&ASYM)
    }

    /// Get scale.
    pub fn get_scale(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&SCALE)
    }

    /// Get index of refraction.
    pub fn get_ior(&self) -> Option<HdFloatDataSourceHandle> {
        self.schema.get_typed(&IOR)
    }

    /// Get schema token.
    pub fn get_schema_token() -> &'static Token {
        &LENS_DISTORTION
    }

    /// Get default locator.
    pub fn get_default_locator() -> HdDataSourceLocator {
        HdDataSourceLocator::from_token(LENS_DISTORTION.clone())
    }

    /// Token for standard distortion type.
    pub fn standard_token() -> &'static Token {
        &STANDARD
    }

    /// Token for fisheye distortion type.
    pub fn fisheye_token() -> &'static Token {
        &FISHEYE
    }
}
