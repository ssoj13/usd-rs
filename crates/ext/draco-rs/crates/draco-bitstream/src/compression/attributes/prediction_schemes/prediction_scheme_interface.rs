//! Prediction scheme interface.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_interface.h`.

use crate::compression::config::compression_shared::{
    PredictionSchemeMethod, PredictionSchemeTransformType,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::point_attribute::PointAttribute;

pub trait PredictionSchemeInterface {
    fn get_prediction_method(&self) -> PredictionSchemeMethod;
    fn get_attribute(&self) -> &PointAttribute;
    fn is_initialized(&self) -> bool;
    fn get_num_parent_attributes(&self) -> i32 {
        0
    }
    fn get_parent_attribute_type(&self, _i: i32) -> GeometryAttributeType {
        GeometryAttributeType::Invalid
    }
    fn set_parent_attribute(&mut self, _att: &PointAttribute) -> bool {
        false
    }
    fn are_corrections_positive(&self) -> bool;
    fn get_transform_type(&self) -> PredictionSchemeTransformType;
}
