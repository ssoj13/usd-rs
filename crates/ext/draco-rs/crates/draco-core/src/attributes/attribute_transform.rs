//! Attribute transform base interface.
//! Reference: `_ref/draco/src/draco/attributes/attribute_transform.h` + `.cc`.

use crate::attributes::attribute_transform_data::AttributeTransformData;
use crate::attributes::attribute_transform_type::AttributeTransformType;
use crate::attributes::geometry_attribute::GeometryAttribute;
use crate::attributes::geometry_indices::PointIndex;
use crate::attributes::point_attribute::PointAttribute;
use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::{data_type_length, DataType};
use crate::core::encoder_buffer::EncoderBuffer;

pub trait AttributeTransform {
    fn transform_type(&self) -> AttributeTransformType;
    fn init_from_attribute(&mut self, attribute: &PointAttribute) -> bool;
    fn copy_to_attribute_transform_data(&self, out_data: &mut AttributeTransformData);

    fn transform_attribute(
        &self,
        attribute: &PointAttribute,
        point_ids: &[PointIndex],
        target_attribute: &mut PointAttribute,
    ) -> bool;

    fn inverse_transform_attribute(
        &self,
        attribute: &PointAttribute,
        target_attribute: &mut PointAttribute,
    ) -> bool;

    fn encode_parameters(&self, encoder_buffer: &mut EncoderBuffer) -> bool;
    fn decode_parameters(
        &mut self,
        attribute: &PointAttribute,
        decoder_buffer: &mut DecoderBuffer,
    ) -> bool;

    fn get_transformed_data_type(&self, attribute: &PointAttribute) -> DataType;
    fn get_transformed_num_components(&self, attribute: &PointAttribute) -> i32;

    fn transfer_to_attribute(&self, attribute: &mut PointAttribute) -> bool {
        let mut transform_data = AttributeTransformData::new();
        self.copy_to_attribute_transform_data(&mut transform_data);
        attribute.set_attribute_transform_data(Some(transform_data));
        true
    }

    fn init_transformed_attribute(
        &self,
        src_attribute: &PointAttribute,
        num_entries: i32,
    ) -> PointAttribute {
        let num_components = self.get_transformed_num_components(src_attribute);
        let data_type = self.get_transformed_data_type(src_attribute);
        let mut ga = GeometryAttribute::new();
        let stride = (num_components as i64) * (data_type_length(data_type) as i64);
        ga.init(
            src_attribute.attribute_type(),
            None,
            num_components as u8,
            data_type,
            false,
            stride,
            0,
        );
        let mut transformed = PointAttribute::from_geometry_attribute(ga);
        let entries = if num_entries < 0 {
            0
        } else {
            num_entries as usize
        };
        transformed.reset(entries);
        transformed.set_identity_mapping();
        transformed.set_unique_id(src_attribute.unique_id());
        transformed
    }
}
