//! Prediction scheme decoder base.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_decoder.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::PredictionSchemeDecoderInterface;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::config::compression_shared::{
    PredictionSchemeMethod, PredictionSchemeTransformType,
};
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub struct PredictionSchemeDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    attribute: *const PointAttribute,
    transform: TransformT,
    _phantom: std::marker::PhantomData<DataTypeT>,
}

impl<DataTypeT, TransformT> PredictionSchemeDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
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
    for PredictionSchemeDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
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

impl<DataTypeT, TransformT> PredictionSchemeDecoderInterface
    for PredictionSchemeDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        self.transform.decode_transform_data(buffer)
    }
}
