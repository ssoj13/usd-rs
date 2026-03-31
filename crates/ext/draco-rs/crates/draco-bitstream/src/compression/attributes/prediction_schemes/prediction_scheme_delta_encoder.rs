//! Delta prediction scheme encoder.
//! Reference: `_ref/draco/src/draco/compression/attributes/prediction_schemes/prediction_scheme_delta_encoder.h`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder::PredictionSchemeEncoder;
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoder_interface::{
    PredictionSchemeEncoderInterface, PredictionSchemeTypedEncoderInterface,
};
use crate::compression::attributes::prediction_schemes::prediction_scheme_encoding_transform::EncodingTransform;
use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::config::compression_shared::PredictionSchemeMethod;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct PredictionSchemeDeltaEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    base: PredictionSchemeEncoder<DataTypeT, TransformT>,
}

impl<DataTypeT, TransformT> PredictionSchemeDeltaEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    pub fn new(attribute: &PointAttribute, transform: TransformT) -> Self {
        Self {
            base: PredictionSchemeEncoder::new(attribute, transform),
        }
    }

    pub fn base(&self) -> &PredictionSchemeEncoder<DataTypeT, TransformT> {
        &self.base
    }

    pub fn base_mut(&mut self) -> &mut PredictionSchemeEncoder<DataTypeT, TransformT> {
        &mut self.base
    }
}

impl<DataTypeT, TransformT> PredictionSchemeInterface
    for PredictionSchemeDeltaEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
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

impl<DataTypeT, TransformT> PredictionSchemeEncoderInterface
    for PredictionSchemeDeltaEncoder<DataTypeT, TransformT>
where
    TransformT: EncodingTransform<DataTypeT>,
{
    fn encode_prediction_data(&self, buffer: &mut EncoderBuffer) -> bool {
        self.base.transform().encode_transform_data(buffer)
    }
}

impl<DataTypeT, TransformT> PredictionSchemeTypedEncoderInterface<DataTypeT, TransformT::CorrType>
    for PredictionSchemeDeltaEncoder<DataTypeT, TransformT>
where
    DataTypeT: Copy + Default,
    TransformT: EncodingTransform<DataTypeT> + Clone,
    TransformT::CorrType: Default + Copy,
{
    fn compute_correction_values(
        &mut self,
        in_data: &[DataTypeT],
        out_corr: &mut [TransformT::CorrType],
        num_components: i32,
        _entry_to_point_id_map: &[PointIndex],
    ) -> bool {
        let size = in_data.len() as i32;
        self.base_mut()
            .transform_mut()
            .init(in_data, size, num_components);
        let num_components_usize = num_components as usize;
        if size <= 0 {
            return true;
        }
        let transform = self.base.transform();
        let mut i = num_components_usize;
        while i < size as usize {
            let original = &in_data[i..i + num_components_usize];
            let predicted = &in_data[i - num_components_usize..i];
            let out_slice = &mut out_corr[i..i + num_components_usize];
            transform.compute_correction(original, predicted, out_slice);
            i += num_components_usize;
        }
        let zero_vals = vec![DataTypeT::default(); num_components_usize];
        transform.compute_correction(
            &in_data[0..num_components_usize],
            &zero_vals,
            &mut out_corr[0..num_components_usize],
        );
        true
    }
}
