//! Sequential attribute encoder base.
//! Reference: `_ref/draco/src/draco/compression/attributes/sequential_attribute_encoder.h|cc`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::config::encoder_options::EncoderOptions;
use crate::compression::point_cloud::{PointCloudEncoder, PointCloudEncoderBase};
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::point_cloud::point_cloud::PointCloud;

pub struct SequentialAttributeEncoderBase {
    point_cloud: *const PointCloud,
    options: *const EncoderOptions,
    encoder_base: *mut PointCloudEncoderBase,
    attribute: *const PointAttribute,
    attribute_id: i32,
    parent_attributes: Vec<i32>,
    parent_portable_attributes: Vec<Option<PointAttribute>>,
    is_parent_encoder: bool,
    portable_attribute: Option<PointAttribute>,
}

impl SequentialAttributeEncoderBase {
    pub fn new() -> Self {
        Self {
            point_cloud: std::ptr::null(),
            options: std::ptr::null(),
            encoder_base: std::ptr::null_mut(),
            attribute: std::ptr::null(),
            attribute_id: -1,
            parent_attributes: Vec::new(),
            parent_portable_attributes: Vec::new(),
            is_parent_encoder: false,
            portable_attribute: None,
        }
    }

    pub fn init(&mut self, encoder: &mut dyn PointCloudEncoder, attribute_id: i32) -> bool {
        let encoder_base = encoder.base_mut() as *mut PointCloudEncoderBase;
        let point_cloud = match unsafe { (*encoder_base).point_cloud.as_ref() } {
            Some(pc) => pc,
            None => return false,
        };
        let att = match point_cloud.attribute(attribute_id) {
            Some(att) => att,
            None => return false,
        };
        self.point_cloud = point_cloud as *const PointCloud;
        self.options = unsafe { &*(*encoder_base).options } as *const EncoderOptions;
        self.encoder_base = encoder_base;
        self.attribute = att as *const PointAttribute;
        self.attribute_id = attribute_id;
        true
    }

    pub fn initialize_standalone(&mut self, attribute: &PointAttribute) -> bool {
        self.point_cloud = std::ptr::null();
        self.options = std::ptr::null();
        self.encoder_base = std::ptr::null_mut();
        self.attribute = attribute as *const PointAttribute;
        self.attribute_id = -1;
        true
    }

    pub fn num_parent_attributes(&self) -> i32 {
        self.parent_attributes.len() as i32
    }

    pub fn get_parent_attribute_id(&self, i: i32) -> i32 {
        self.parent_attributes[i as usize]
    }

    pub fn mark_parent_attribute(&mut self) {
        self.is_parent_encoder = true;
    }

    pub fn attribute(&self) -> &PointAttribute {
        unsafe { &*self.attribute }
    }

    pub fn attribute_id(&self) -> i32 {
        self.attribute_id
    }

    pub fn point_cloud(&self) -> Option<&PointCloud> {
        unsafe { self.point_cloud.as_ref() }
    }

    pub fn options(&self) -> Option<&EncoderOptions> {
        unsafe { self.options.as_ref() }
    }

    pub fn encoder_base(&self) -> Option<&mut PointCloudEncoderBase> {
        unsafe { self.encoder_base.as_mut() }
    }

    pub fn is_parent_encoder(&self) -> bool {
        self.is_parent_encoder
    }

    pub fn set_portable_attribute(&mut self, att: PointAttribute) {
        self.portable_attribute = Some(att);
    }

    pub fn portable_attribute(&self) -> Option<&PointAttribute> {
        self.portable_attribute.as_ref()
    }

    pub fn portable_attribute_mut(&mut self) -> Option<&mut PointAttribute> {
        self.portable_attribute.as_mut()
    }

    pub fn set_parent_portable_attributes(&mut self, attributes: Vec<Option<PointAttribute>>) {
        self.parent_portable_attributes = attributes;
    }

    pub fn get_portable_attribute(&self) -> &PointAttribute {
        if let Some(portable) = &self.portable_attribute {
            return portable;
        }
        self.attribute()
    }

    pub fn init_prediction_scheme(&mut self, ps: &mut dyn PredictionSchemeInterface) -> bool {
        self.parent_attributes.clear();
        self.parent_portable_attributes.clear();
        for i in 0..ps.get_num_parent_attributes() {
            let att_id = {
                let point_cloud = match self.point_cloud() {
                    Some(pc) => pc,
                    None => return false,
                };
                point_cloud.get_named_attribute_id(ps.get_parent_attribute_type(i))
            };
            if att_id == -1 {
                return false;
            }
            self.parent_attributes.push(att_id);
        }
        true
    }

    pub fn set_prediction_scheme_parent_attributes(
        &self,
        ps: &mut dyn PredictionSchemeInterface,
    ) -> bool {
        if self.parent_attributes.len() != ps.get_num_parent_attributes() as usize {
            return false;
        }
        let encoder_base = unsafe { self.encoder_base.as_ref() };
        for (parent_index, &att_id) in self.parent_attributes.iter().enumerate() {
            let local_parent = self
                .parent_portable_attributes
                .get(parent_index)
                .and_then(|parent| parent.as_ref());
            let parent = if let Some(parent) = local_parent {
                parent
            } else {
                let encoder_base = match encoder_base {
                    Some(encoder_base) => encoder_base,
                    None => return false,
                };
                if att_id < 0 || att_id as usize >= encoder_base.attribute_to_encoder_map.len() {
                    return false;
                }
                let encoder_id = encoder_base.attribute_to_encoder_map[att_id as usize];
                if encoder_id < 0 {
                    return false;
                }
                match encoder_base.attributes_encoders[encoder_id as usize]
                    .get_portable_attribute(att_id)
                {
                    Some(parent) => parent,
                    None => return false,
                }
            };
            if !ps.set_parent_attribute(parent) {
                return false;
            }
        }
        true
    }
}

impl Default for SequentialAttributeEncoderBase {
    fn default() -> Self {
        Self::new()
    }
}

pub trait SequentialAttributeEncoderInterface {
    fn base(&self) -> &SequentialAttributeEncoderBase;
    fn base_mut(&mut self) -> &mut SequentialAttributeEncoderBase;

    fn init(&mut self, encoder: &mut dyn PointCloudEncoder, attribute_id: i32) -> bool {
        self.base_mut().init(encoder, attribute_id)
    }

    fn initialize_standalone(&mut self, attribute: &PointAttribute) -> bool {
        self.base_mut().initialize_standalone(attribute)
    }

    fn transform_attribute_to_portable_format(&mut self, _point_ids: &[PointIndex]) -> bool {
        true
    }

    fn encode_portable_attribute(
        &mut self,
        point_ids: &[PointIndex],
        out_buffer: &mut EncoderBuffer,
    ) -> bool {
        self.encode_values(point_ids, out_buffer)
    }

    fn encode_data_needed_by_portable_transform(
        &mut self,
        _out_buffer: &mut EncoderBuffer,
    ) -> bool {
        true
    }

    fn is_lossy_encoder(&self) -> bool {
        false
    }

    fn num_parent_attributes(&self) -> i32 {
        self.base().num_parent_attributes()
    }

    fn get_parent_attribute_id(&self, i: i32) -> i32 {
        self.base().get_parent_attribute_id(i)
    }

    fn get_portable_attribute(&self) -> &PointAttribute {
        self.base().get_portable_attribute()
    }

    fn mark_parent_attribute(&mut self) {
        self.base_mut().mark_parent_attribute();
    }

    fn get_unique_id(&self) -> u8 {
        crate::compression::config::compression_shared::SequentialAttributeEncoderType::SequentialAttributeEncoderGeneric
            as u8
    }

    fn attribute(&self) -> &PointAttribute {
        self.base().attribute()
    }

    fn attribute_id(&self) -> i32 {
        self.base().attribute_id()
    }

    fn point_cloud(&self) -> Option<&PointCloud> {
        self.base().point_cloud()
    }

    fn options(&self) -> Option<&EncoderOptions> {
        self.base().options()
    }

    fn is_parent_encoder(&self) -> bool {
        self.base().is_parent_encoder()
    }

    fn set_portable_attribute(&mut self, att: PointAttribute) {
        self.base_mut().set_portable_attribute(att);
    }

    fn set_parent_portable_attributes(&mut self, attributes: Vec<Option<PointAttribute>>) {
        self.base_mut().set_parent_portable_attributes(attributes);
    }

    fn portable_attribute_mut(&mut self) -> Option<&mut PointAttribute> {
        self.base_mut().portable_attribute_mut()
    }

    fn init_prediction_scheme(&mut self, ps: &mut dyn PredictionSchemeInterface) -> bool {
        self.base_mut().init_prediction_scheme(ps)
    }

    fn set_prediction_scheme_parent_attributes(
        &self,
        ps: &mut dyn PredictionSchemeInterface,
    ) -> bool {
        self.base().set_prediction_scheme_parent_attributes(ps)
    }

    fn encode_values(&mut self, point_ids: &[PointIndex], out_buffer: &mut EncoderBuffer) -> bool {
        let entry_size = self.attribute().byte_stride() as usize;
        let mut value_data = vec![0u8; entry_size];
        for &pi in point_ids {
            let entry_id = self.attribute().mapped_index(pi);
            self.attribute().get_value_bytes(entry_id, &mut value_data);
            if !out_buffer.encode_bytes(&value_data) {
                return false;
            }
        }
        true
    }
}

/// Generic sequential attribute encoder with no special transforms.
pub struct SequentialAttributeEncoder {
    base: SequentialAttributeEncoderBase,
}

impl SequentialAttributeEncoder {
    pub fn new() -> Self {
        Self {
            base: SequentialAttributeEncoderBase::new(),
        }
    }
}

impl Default for SequentialAttributeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SequentialAttributeEncoderInterface for SequentialAttributeEncoder {
    fn base(&self) -> &SequentialAttributeEncoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut SequentialAttributeEncoderBase {
        &mut self.base
    }
}
