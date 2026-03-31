//! Sequential attribute decoders controller.
//! Reference: `_ref/draco/src/draco/compression/attributes/sequential_attribute_decoders_controller.h|cc`.
//!
//! Manages per-attribute sequential decoders and a point id sequencer.

use crate::compression::attributes::attributes_decoder::{
    AttributesDecoder, AttributesDecoderBase,
};
use crate::compression::attributes::attributes_decoder_interface::AttributesDecoderInterface;
use crate::compression::attributes::points_sequencer::PointsSequencer;
use crate::compression::attributes::sequential_attribute_decoder::{
    SequentialAttributeDecoder, SequentialAttributeDecoderInterface,
};
use crate::compression::attributes::sequential_integer_attribute_decoder::SequentialIntegerAttributeDecoder;
use crate::compression::attributes::sequential_normal_attribute_decoder::SequentialNormalAttributeDecoder;
use crate::compression::attributes::sequential_quantization_attribute_decoder::SequentialQuantizationAttributeDecoder;
use crate::compression::config::compression_shared::SequentialAttributeEncoderType;
use crate::compression::point_cloud::PointCloudDecoder;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;

pub struct SequentialAttributeDecodersController {
    base: AttributesDecoderBase,
    sequential_decoders: Vec<Box<dyn SequentialAttributeDecoderInterface>>,
    point_ids: Vec<PointIndex>,
    sequencer: Box<dyn PointsSequencer>,
}

impl SequentialAttributeDecodersController {
    pub fn new(sequencer: Box<dyn PointsSequencer>) -> Self {
        Self {
            base: AttributesDecoderBase::new(),
            sequential_decoders: Vec::new(),
            point_ids: Vec::new(),
            sequencer,
        }
    }

    fn create_sequential_decoder(
        &self,
        decoder_type: u8,
    ) -> Option<Box<dyn SequentialAttributeDecoderInterface>> {
        match decoder_type {
            x if x == SequentialAttributeEncoderType::SequentialAttributeEncoderGeneric as u8 => {
                Some(Box::new(SequentialAttributeDecoder::new()))
            }
            x if x == SequentialAttributeEncoderType::SequentialAttributeEncoderInteger as u8 => {
                Some(Box::new(SequentialIntegerAttributeDecoder::new()))
            }
            x if x
                == SequentialAttributeEncoderType::SequentialAttributeEncoderQuantization as u8 =>
            {
                Some(Box::new(SequentialQuantizationAttributeDecoder::new()))
            }
            x if x == SequentialAttributeEncoderType::SequentialAttributeEncoderNormals as u8 => {
                Some(Box::new(SequentialNormalAttributeDecoder::new()))
            }
            _ => None,
        }
    }
}

impl AttributesDecoderInterface for SequentialAttributeDecodersController {
    fn init(
        &mut self,
        decoder: &mut dyn PointCloudDecoder,
        pc: &mut draco_core::point_cloud::point_cloud::PointCloud,
    ) -> bool {
        self.base.init(decoder, pc)
    }

    fn decode_attributes_decoder_data(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        if !self.base.decode_attributes_decoder_data(in_buffer) {
            return false;
        }
        let num_attributes = self.base.get_num_attributes();
        self.sequential_decoders
            .resize_with(num_attributes as usize, || {
                Box::new(SequentialAttributeDecoder::new())
            });
        for i in 0..num_attributes {
            let mut decoder_type: u8 = 0;
            if !in_buffer.decode(&mut decoder_type) {
                return false;
            }
            let decoder = match self.create_sequential_decoder(decoder_type) {
                Some(decoder) => decoder,
                None => return false,
            };
            self.sequential_decoders[i as usize] = decoder;
            let att_id = self.base.get_attribute_id(i);
            let decoder_ref = match self.base.decoder_mut() {
                Some(decoder_ref) => decoder_ref,
                None => return false,
            };
            if !self.sequential_decoders[i as usize].init(decoder_ref, att_id) {
                return false;
            }
        }
        true
    }

    fn decode_attributes(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        if !self.sequencer.generate_sequence(&mut self.point_ids) {
            return false;
        }
        let num_attributes = self.base.get_num_attributes();
        for i in 0..num_attributes {
            let att_id = self.base.get_attribute_id(i);
            let pa = match self
                .base
                .decoder_mut()
                .and_then(|dec| dec.point_cloud_mut())
                .and_then(|pc| pc.attribute_mut(att_id))
            {
                Some(att) => att,
                None => return false,
            };
            if !self.sequencer.update_point_to_attribute_index_mapping(pa) {
                return false;
            }
        }
        AttributesDecoder::decode_attributes(self, in_buffer)
    }

    fn get_attribute_id(&self, i: i32) -> i32 {
        self.base.get_attribute_id(i)
    }

    fn get_num_attributes(&self) -> i32 {
        self.base.get_num_attributes()
    }

    fn get_decoder(&self) -> Option<&dyn PointCloudDecoder> {
        self.base.decoder()
    }

    fn get_portable_attribute(&self, point_attribute_id: i32) -> Option<&PointAttribute> {
        let loc_id = self
            .base
            .get_local_id_for_point_attribute(point_attribute_id);
        if loc_id < 0 {
            return None;
        }
        self.sequential_decoders[loc_id as usize].get_portable_attribute()
    }
}

impl AttributesDecoder for SequentialAttributeDecodersController {
    fn base(&self) -> &AttributesDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut AttributesDecoderBase {
        &mut self.base
    }

    fn decode_portable_attributes(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        let num_attributes = self.base.get_num_attributes();
        for i in 0..num_attributes {
            if !self.sequential_decoders[i as usize]
                .decode_portable_attribute(&self.point_ids, in_buffer)
            {
                return false;
            }
        }
        true
    }

    fn decode_data_needed_by_portable_transforms(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        let num_attributes = self.base.get_num_attributes();
        for i in 0..num_attributes {
            if !self.sequential_decoders[i as usize]
                .decode_data_needed_by_portable_transform(&self.point_ids, in_buffer)
            {
                return false;
            }
        }
        true
    }

    fn transform_attributes_to_original_format(&mut self) -> bool {
        let num_attributes = self.base.get_num_attributes();
        for i in 0..num_attributes {
            if let Some(decoder) = self.base.decoder() {
                if let Some(opts) = decoder.options() {
                    let attr_type = self.sequential_decoders[i as usize]
                        .attribute()
                        .attribute_type();
                    if opts.get_attribute_bool(&attr_type, "skip_attribute_transform", false) {
                        let _ = self.sequential_decoders[i as usize].copy_portable_to_attribute();
                        continue;
                    }
                }
            }
            if !self.sequential_decoders[i as usize]
                .transform_attribute_to_original_format(&self.point_ids)
            {
                return false;
            }
        }
        true
    }
}
