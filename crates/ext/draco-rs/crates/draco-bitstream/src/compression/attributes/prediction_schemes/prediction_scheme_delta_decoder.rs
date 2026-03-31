//! Delta prediction scheme decoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_delta_decoder.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder::PredictionSchemeDecoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoder_interface::{
    PredictionSchemeDecoderInterface, PredictionSchemeTypedDecoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_decoding_transform::DecodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::config::compression_shared::PredictionSchemeMethod;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub struct PredictionSchemeDeltaDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    base: PredictionSchemeDecoder<DataTypeT, TransformT>,
}

impl<DataTypeT, TransformT> PredictionSchemeDeltaDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT) -> Self {
        Self {
            base: PredictionSchemeDecoder::new(attribute, transform),
        }
    }

    pub fn base(&self) -> &PredictionSchemeDecoder<DataTypeT, TransformT> {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut PredictionSchemeDecoder<DataTypeT, TransformT> {
        &mut self.base
    }
}

impl<DataTypeT, TransformT> PredictionSchemeInterface
    for PredictionSchemeDeltaDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    fn get_prediction_method(&self) -> PredictionSchemeMethod {
        PredictionSchemeMethod::PredictionDifference
    }

    fn get_attribute(&self) -> &PointAttribute {
        self.base.attribute()
    }

    fn is_initialized(&self) -> bool {
        true
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
        self.base.transform().are_corrections_positive()
    }

    fn get_transform_type(
        &self,
    ) -> crate::compression::config::compression_shared::PredictionSchemeTransformType {
        self.base.transform().get_type()
    }
}

impl<DataTypeT, TransformT> PredictionSchemeDecoderInterface
    for PredictionSchemeDeltaDecoder<DataTypeT, TransformT>
where
    TransformT: DecodingTransform<DataTypeT>,
{
    fn decode_prediction_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        self.base.transform_mut().decode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT> PredictionSchemeTypedDecoderInterface<DataTypeT, TransformT::CorrType>
    for PredictionSchemeDeltaDecoder<DataTypeT, TransformT>
where
    DataTypeT: Copy + Default,
    TransformT: DecodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Copy,
{
    fn compute_original_values(
        &self,
        in_corr: &[TransformT::CorrType],
        out_data: &mut [DataTypeT],
        num_components: i32,
        _entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        let size = in_corr.len();
        let mut transform = self.base.transform().clone();
        transform.init(num_components);
        let num_components_usize = num_components as usize;
        if size == 0 {
            return true;
        }
        let zero_vals = vec![DataTypeT::default(); num_components_usize];
        transform.compute_original_value(
            &zero_vals,
            &in_corr[0..num_components_usize],
            &mut out_data[0..num_components_usize],
        );
        let mut i = num_components_usize;
        while i < size {
            // Split to avoid overlapping borrows when predicted and output ranges touch.
            let (prefix, rest) = out_data.split_at_mut(i);
            let predicted = &prefix[i - num_components_usize..i];
            let corr = &in_corr[i..i + num_components_usize];
            let out_slice = &mut rest[..num_components_usize];
            transform.compute_original_value(predicted, corr, out_slice);
            i += num_components_usize;
        }
        true
    }
}
