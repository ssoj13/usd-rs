//! Prediction scheme encoder base.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_encoder.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_interface::PredictionSchemeEncoderInterface;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::config::compression_shared::{
    PredictionSchemeMethod, PredictionSchemeTransformType,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct PredictionSchemeEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    attribute: *const PointAttribute,
    transform: TransformT,
    _phantom: std::marker::PhantomData<DataTypeT>,
}

impl<DataTypeT, TransformT> PredictionSchemeEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT) -> Self {
        Self {
            attribute: attribute as *const PointAttribute,
            transform,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn attribute(&self) -> &PointAttribute {
        unsafe { &*self.attribute }
    }

    pub fn transform(&self) -> &TransformT {
        &self.transform
    }

    pub fn transform_mut(&mut self) -> &mut TransformT {
        &mut self.transform
    }
}

impl<DataTypeT, TransformT> PredictionSchemeInterface
    for PredictionSchemeEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::PredictionDifference
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.attribute()
    }

    fn is_initialized(&self) -> bool {
        !self.attribute.is_null()
    }

    fn get_num_parent_attributes(&self) -> i32 {
        0
    }

    fn get_parent_attribute_type(&self, _i: i32) -> GeometryAttributeType {
        GeometryAttributeType::Invalid
    }

    fn set_parent_attribute(&mut self, _att: &PointAttribute) -> bool {
        false
    }

    fn are_corrections_positive(&self) -> bool {
        self.transform.are_corrections_positive()
    }

    fn get_transform_type(&self) -> PredictionSchemeTransformType {
        self.transform.get_type()
    }
}

impl<DataTypeT, TransformT> PredictionSchemeEncoderInterface
    for PredictionSchemeEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    fn encode_prediction_data(&self, buffer: &mut EncoderBuffer) -> bool {
        self.transform.encode_transform_data(buffer)
    }
}
