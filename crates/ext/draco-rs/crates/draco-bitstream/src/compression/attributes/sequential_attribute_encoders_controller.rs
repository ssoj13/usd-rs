//! Sequential attribute encoders controller.
//! Reference: `_ref/draco/src/draco/compression/attributes/sequential_attribute_encoders_controller.h|cc`.
//!
//! Manages per-attribute sequential encoders and a point id sequencer.

use crate::compression::attributes::attributes_encoder::{
    AttributesEncoderBase, AttributesEncoderInterface,
};
use crate::compression::attributes::points_sequencer::PointsSequencer;
use crate::compression::attributes::sequential_attribute_encoder::{
    SequentialAttributeEncoder, SequentialAttributeEncoderInterface,
};
use crate::compression::attributes::sequential_integer_attribute_encoder::SequentialIntegerAttributeEncoder;
use crate::compression::attributes::sequential_normal_attribute_encoder::SequentialNormalAttributeEncoder;
use crate::compression::attributes::sequential_quantization_attribute_encoder::SequentialQuantizationAttributeEncoder;
use crate::compression::config::compression_shared::AttributeEncoderType;
use crate::compression::point_cloud::PointCloudEncoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::core::draco_types::DataType;
use draco_core::core::encoder_buffer::EncoderBuffer;

pub struct SequentialAttributeEncodersController {
    base: AttributesEncoderBase,
    sequential_encoders: Vec<Box<dyn SequentialAttributeEncoderInterface>>,
    sequential_encoder_marked_as_parent: Vec<bool>,
    point_ids: Vec<PointIndex>,
    sequencer: Box<dyn PointsSequencer>,
}

impl SequentialAttributeEncodersController {
    pub fn new(sequencer: Box<dyn PointsSequencer>) -> Self {
        Self {
            base: AttributesEncoderBase::new(),
            sequential_encoders: Vec::new(),
            sequential_encoder_marked_as_parent: Vec::new(),
            point_ids: Vec::new(),
            sequencer,
        }
    }

    pub fn with_attribute_id(sequencer: Box<dyn PointsSequencer>, point_attribute_id: i32) -> Self {
        Self {
            base: AttributesEncoderBase::with_attribute_id(point_attribute_id),
            sequential_encoders: Vec::new(),
            sequential_encoder_marked_as_parent: Vec::new(),
            point_ids: Vec::new(),
            sequencer,
        }
    }

    fn create_sequential_encoders(&mut self) -> bool {
        let num_attributes = self.base.num_attributes() as usize;
        self.sequential_encoders.resize_with(num_attributes, || {
            Box::new(SequentialAttributeEncoder::new())
        });
        for i in 0..num_attributes {
            let encoder = match self.create_sequential_encoder(i) {
                Some(enc) => enc,
                None => return false,
            };
            self.sequential_encoders[i] = encoder;
            if i < self.sequential_encoder_marked_as_parent.len() {
                if self.sequential_encoder_marked_as_parent[i] {
                    self.sequential_encoders[i].mark_parent_attribute();
                }
            }
        }
        true
    }

    fn create_sequential_encoder(
        &self,
        local_id: usize,
    ) -> Option<Box<dyn SequentialAttributeEncoderInterface>> {
        let att_id = self.base.get_attribute_id(local_id as i32);
        let point_cloud = self.base.point_cloud()?;
        let options = self.base.options()?;
        let att = point_cloud.attribute(att_id)?;
        match att.data_type() {
            DataType::Uint8
            | DataType::Int8
            | DataType::Uint16
            | DataType::Int16
            | DataType::Uint32
            | DataType::Int32 => {
                return Some(Box::new(SequentialIntegerAttributeEncoder::new()));
            }
            DataType::Float32 => {
                let quant_bits = options.get_attribute_int(&att_id, "quantization_bits", -1);
                if quant_bits > 0 {
                    if att.attribute_type() == GeometryAttributeType::Normal {
                        return Some(Box::new(SequentialNormalAttributeEncoder::new()));
                    }
                    return Some(Box::new(SequentialQuantizationAttributeEncoder::new()));
                }
            }
            _ => {}
        }
        Some(Box::new(SequentialAttributeEncoder::new()))
    }
}

impl AttributesEncoderInterface for SequentialAttributeEncodersController {
    fn base(&self) -> &AttributesEncoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut AttributesEncoderBase {
        &mut self.base
    }

    fn get_unique_id(&self) -> u8 {
        AttributeEncoderType::BasicAttributeEncoder as u8
    }

    fn init(
        &mut self,
        encoder: &mut dyn PointCloudEncoder,
        pc: &draco_core::point_cloud::point_cloud::PointCloud,
    ) -> bool {
        let debug_attr = std::env::var("DRACO_DEBUG_ATTR").ok().as_deref() == Some("1");
        if !self.base.init(encoder, pc) {
            if debug_attr {
                eprintln!("[attr] controller base.init failed");
            }
            return false;
        }
        if !self.create_sequential_encoders() {
            if debug_attr {
                eprintln!("[attr] create_sequential_encoders failed");
            }
            return false;
        }
        for i in 0..self.base.num_attributes() {
            let att_id = self.base.get_attribute_id(i as i32);
            if !self.sequential_encoders[i as usize].init(encoder, att_id) {
                if debug_attr {
                    eprintln!(
                        "[attr] sequential encoder init failed: local_id={}, att_id={}",
                        i, att_id
                    );
                }
                return false;
            }
        }
        true
    }

    fn encode_attributes_encoder_data(&mut self, out_buffer: &mut EncoderBuffer) -> bool {
        if !self.base.encode_attributes_encoder_data(out_buffer) {
            return false;
        }
        for enc in &self.sequential_encoders {
            if !out_buffer.encode(enc.get_unique_id()) {
                return false;
            }
        }
        true
    }

    fn encode_attributes(&mut self, buffer: &mut EncoderBuffer) -> bool {
        let debug_attr = std::env::var("DRACO_DEBUG_ATTR").ok().as_deref() == Some("1");
        if !self.sequencer.generate_sequence(&mut self.point_ids) {
            if debug_attr {
                eprintln!("[attr] sequencer.generate_sequence failed");
            }
            return false;
        }
        if std::env::var("DRACO_DEBUG_TRAVERSAL").ok().as_deref() == Some("1") {
            let ids: Vec<u32> = self.point_ids.iter().map(|p| p.value()).collect();
            eprintln!(
                "[attr] point_ids ({}): {:?}",
                ids.len(),
                &ids[..ids.len().min(16)]
            );
        }
        if !self.transform_attributes_to_portable_format() {
            if debug_attr {
                eprintln!("[attr] transform_attributes_to_portable_format failed");
            }
            return false;
        }
        if !self.encode_portable_attributes(buffer) {
            if debug_attr {
                eprintln!("[attr] encode_portable_attributes failed");
            }
            return false;
        }
        if !self.encode_data_needed_by_portable_transforms(buffer) {
            if debug_attr {
                eprintln!("[attr] encode_data_needed_by_portable_transforms failed");
            }
            return false;
        }
        true
    }

    fn num_parent_attributes(&self, point_attribute_id: i32) -> i32 {
        let loc_id = self
            .base
            .get_local_id_for_point_attribute(point_attribute_id);
        if loc_id < 0 {
            return 0;
        }
        self.sequential_encoders[loc_id as usize].num_parent_attributes()
    }

    fn get_parent_attribute_id(&self, point_attribute_id: i32, parent_i: i32) -> i32 {
        let loc_id = self
            .base
            .get_local_id_for_point_attribute(point_attribute_id);
        if loc_id < 0 {
            return -1;
        }
        self.sequential_encoders[loc_id as usize].get_parent_attribute_id(parent_i)
    }

    fn mark_parent_attribute(&mut self, point_attribute_id: i32) -> bool {
        let loc_id = self
            .base
            .get_local_id_for_point_attribute(point_attribute_id);
        if loc_id < 0 {
            return false;
        }
        let loc_id = loc_id as usize;
        if self.sequential_encoder_marked_as_parent.len() <= loc_id {
            self.sequential_encoder_marked_as_parent
                .resize(loc_id + 1, false);
        }
        self.sequential_encoder_marked_as_parent[loc_id] = true;
        if self.sequential_encoders.len() <= loc_id {
            return true;
        }
        self.sequential_encoders[loc_id].mark_parent_attribute();
        true
    }

    fn get_portable_attribute(
        &self,
        point_attribute_id: i32,
    ) -> Option<&draco_core::attributes::point_attribute::PointAttribute> {
        let loc_id = self
            .base
            .get_local_id_for_point_attribute(point_attribute_id);
        if loc_id < 0 {
            return None;
        }
        Some(self.sequential_encoders[loc_id as usize].get_portable_attribute())
    }

    fn transform_attributes_to_portable_format(&mut self) -> bool {
        let debug_attr = std::env::var("DRACO_DEBUG_ATTR").ok().as_deref() == Some("1");
        for (i, enc) in self.sequential_encoders.iter_mut().enumerate() {
            if !enc.transform_attribute_to_portable_format(&self.point_ids) {
                if debug_attr {
                    eprintln!(
                        "[attr] transform_attribute_to_portable_format failed for {}",
                        i
                    );
                }
                return false;
            }
        }
        true
    }

    fn encode_portable_attributes(&mut self, out_buffer: &mut EncoderBuffer) -> bool {
        let debug_attr = std::env::var("DRACO_DEBUG_ATTR").ok().as_deref() == Some("1");
        let portable_snapshot: Vec<_> = self
            .sequential_encoders
            .iter()
            .map(|enc| {
                let mut copy = draco_core::attributes::point_attribute::PointAttribute::new();
                copy.copy_from(enc.get_portable_attribute());
                copy
            })
            .collect();
        for i in 0..self.sequential_encoders.len() {
            let parent_attributes = {
                let enc = &self.sequential_encoders[i];
                let mut attributes = Vec::new();
                for parent_index in 0..enc.num_parent_attributes() {
                    let parent_attribute_id = enc.get_parent_attribute_id(parent_index);
                    let parent_local_id = self
                        .base
                        .get_local_id_for_point_attribute(parent_attribute_id);
                    if parent_local_id < 0 {
                        attributes.push(None);
                    } else {
                        let mut copy =
                            draco_core::attributes::point_attribute::PointAttribute::new();
                        copy.copy_from(&portable_snapshot[parent_local_id as usize]);
                        attributes.push(Some(copy));
                    }
                }
                attributes
            };
            let enc = &mut self.sequential_encoders[i];
            enc.set_parent_portable_attributes(parent_attributes);
            if !enc.encode_portable_attribute(&self.point_ids, out_buffer) {
                if debug_attr {
                    eprintln!("[attr] encode_portable_attribute failed for {}", i);
                }
                return false;
            }
        }
        true
    }

    fn encode_data_needed_by_portable_transforms(
        &mut self,
        out_buffer: &mut EncoderBuffer,
    ) -> bool {
        for enc in &mut self.sequential_encoders {
            if !enc.encode_data_needed_by_portable_transform(out_buffer) {
                return false;
            }
        }
        true
    }
}
