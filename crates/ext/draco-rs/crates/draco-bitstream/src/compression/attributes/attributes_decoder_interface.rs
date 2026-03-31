//! Attribute decoder interface.
//! Reference: `_ref/draco/src/draco/compression/attributes/attributes_decoder_interface.h`.

use crate::compression::point_cloud::PointCloudDecoder;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::point_cloud::point_cloud::PointCloud;

pub trait AttributesDecoderInterface {
    fn init(&mut self, decoder: &mut dyn PointCloudDecoder, pc: &mut PointCloud) -> bool;
    fn decode_attributes_decoder_data(&mut self, in_buffer: &mut DecoderBuffer) -> bool;
    fn decode_attributes(&mut self, in_buffer: &mut DecoderBuffer) -> bool;

    fn get_attribute_id(&self, i: i32) -> i32;
    fn get_num_attributes(&self) -> i32;
    fn get_decoder(&self) -> Option<&dyn PointCloudDecoder>;

    fn get_portable_attribute(&self, _point_attribute_id: i32) -> Option<&PointAttribute> {
        None
    }
}
