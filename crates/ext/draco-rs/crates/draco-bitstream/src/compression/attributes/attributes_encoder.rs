//! Attribute encoder base + interface.
//! Reference: `_ref/draco/src/draco/compression/attributes/attributes_encoder.h|cc`.

use crate::compression::config::encoder_options::EncoderOptions;
use crate::compression::point_cloud::PointCloudEncoder;
use draco_core::attributes::geometry_attribute::GeometryAttributeType;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::varint_encoding::encode_varint;
use draco_core::point_cloud::point_cloud::PointCloud;

pub struct AttributesEncoderBase {
    point_attribute_ids: Vec<i32>,
    point_attribute_to_local_id_map: Vec<i32>,
    point_cloud: *const PointCloud,
    options: *const EncoderOptions,
}

impl AttributesEncoderBase {
    pub fn new() -> Self {
        Self {
            point_attribute_ids: Vec::new(),
            point_attribute_to_local_id_map: Vec::new(),
            point_cloud: std::ptr::null(),
            options: std::ptr::null(),
        }
    }

    pub fn with_attribute_id(point_attrib_id: i32) -> Self {
        let mut enc = Self::new();
        enc.add_attribute_id(point_attrib_id);
        enc
    }

    pub fn init(&mut self, encoder: &mut dyn PointCloudEncoder, pc: &PointCloud) -> bool {
        self.point_cloud = pc as *const PointCloud;
        self.options = encoder.options() as *const EncoderOptions;
        true
    }

    pub fn encode_attributes_encoder_data(&self, out_buffer: &mut EncoderBuffer) -> bool {
        encode_varint(self.num_attributes() as u32, out_buffer);
        for &att_id in &self.point_attribute_ids {
            let pa = match self.point_cloud().and_then(|pc| pc.attribute(att_id)) {
                Some(pa) => pa,
                None => return false,
            };
            let mut att_type = pa.attribute_type();
            if att_type as i32 > GeometryAttributeType::Generic as i32 {
                att_type = GeometryAttributeType::Generic;
            }
            if !out_buffer.encode(att_type as u8) {
                return false;
            }
            if !out_buffer.encode(pa.data_type() as u8) {
                return false;
            }
            if !out_buffer.encode(pa.num_components() as u8) {
                return false;
            }
            if !out_buffer.encode(pa.normalized() as u8) {
                return false;
            }
            encode_varint(pa.unique_id(), out_buffer);
        }
        true
    }

    pub fn add_attribute_id(&mut self, id: i32) {
        self.point_attribute_ids.push(id);
        if id < 0 {
            return;
        }
        let id_usize = id as usize;
        if id_usize >= self.point_attribute_to_local_id_map.len() {
            self.point_attribute_to_local_id_map
                .resize(id_usize + 1, -1);
        }
        self.point_attribute_to_local_id_map[id_usize] =
            (self.point_attribute_ids.len() as i32) - 1;
    }

    pub fn set_attribute_ids(&mut self, point_attribute_ids: &[i32]) {
        self.point_attribute_ids.clear();
        self.point_attribute_to_local_id_map.clear();
        for &att_id in point_attribute_ids {
            self.add_attribute_id(att_id);
        }
    }

    pub fn get_attribute_id(&self, i: i32) -> i32 {
        self.point_attribute_ids[i as usize]
    }

    pub fn num_attributes(&self) -> u32 {
        self.point_attribute_ids.len() as u32
    }

    pub fn options(&self) -> Option<&EncoderOptions> {
        unsafe { self.options.as_ref() }
    }

    pub fn point_cloud(&self) -> Option<&PointCloud> {
        unsafe { self.point_cloud.as_ref() }
    }

    pub fn get_local_id_for_point_attribute(&self, point_attribute_id: i32) -> i32 {
        if point_attribute_id < 0 {
            return -1;
        }
        let idx = point_attribute_id as usize;
        if idx >= self.point_attribute_to_local_id_map.len() {
            return -1;
        }
        self.point_attribute_to_local_id_map[idx]
    }
}

impl Default for AttributesEncoderBase {
    fn default() -> Self {
        Self::new()
    }
}

pub trait AttributesEncoderInterface {
    fn base(&self) -> &AttributesEncoderBase;
    fn base_mut(&mut self) -> &mut AttributesEncoderBase;

    fn init(&mut self, encoder: &mut dyn PointCloudEncoder, pc: &PointCloud) -> bool {
        self.base_mut().init(encoder, pc)
    }

    fn encode_attributes_encoder_data(&mut self, out_buffer: &mut EncoderBuffer) -> bool {
        self.base().encode_attributes_encoder_data(out_buffer)
    }

    fn get_unique_id(&self) -> u8;

    fn encode_attributes(&mut self, out_buffer: &mut EncoderBuffer) -> bool {
        if !self.transform_attributes_to_portable_format() {
            return false;
        }
        if !self.encode_portable_attributes(out_buffer) {
            return false;
        }
        if !self.encode_data_needed_by_portable_transforms(out_buffer) {
            return false;
        }
        true
    }

    fn num_parent_attributes(&self, _point_attribute_id: i32) -> i32 {
        0
    }

    fn get_parent_attribute_id(&self, _point_attribute_id: i32, _parent_i: i32) -> i32 {
        -1
    }

    fn mark_parent_attribute(&mut self, _point_attribute_id: i32) -> bool {
        false
    }

    fn get_portable_attribute(&self, _point_attribute_id: i32) -> Option<&PointAttribute> {
        None
    }

    fn add_attribute_id(&mut self, id: i32) {
        self.base_mut().add_attribute_id(id);
    }

    fn set_attribute_ids(&mut self, ids: &[i32]) {
        self.base_mut().set_attribute_ids(ids);
    }

    fn get_attribute_id(&self, i: i32) -> i32 {
        self.base().get_attribute_id(i)
    }

    fn num_attributes(&self) -> u32 {
        self.base().num_attributes()
    }

    fn options(&self) -> Option<&EncoderOptions> {
        self.base().options()
    }

    fn point_cloud(&self) -> Option<&PointCloud> {
        self.base().point_cloud()
    }

    fn get_local_id_for_point_attribute(&self, point_attribute_id: i32) -> i32 {
        self.base()
            .get_local_id_for_point_attribute(point_attribute_id)
    }

    fn transform_attributes_to_portable_format(&mut self) -> bool {
        true
    }

    fn encode_portable_attributes(&mut self, _out_buffer: &mut EncoderBuffer) -> bool;

    fn encode_data_needed_by_portable_transforms(
        &mut self,
        _out_buffer: &mut EncoderBuffer,
    ) -> bool {
        true
    }
}
