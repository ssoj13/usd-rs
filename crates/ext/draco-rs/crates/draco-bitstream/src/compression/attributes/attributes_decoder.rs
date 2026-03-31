//! Attribute decoder base.
//! Reference: `_ref/draco/src/draco/compression/attributes/attributes_decoder.h|cc`.

use crate::compression::attributes::attributes_decoder_interface::AttributesDecoderInterface;
use crate::compression::config::compression_shared::bitstream_version;
use crate::compression::point_cloud::PointCloudDecoder;
use draco_core::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::draco_types::{data_type_length, DataType};
use draco_core::core::varint_decoding::decode_varint;
use draco_core::point_cloud::point_cloud::PointCloud;

type DecoderRawPtr = (*mut (), *mut ());

#[inline]
unsafe fn decode_raw_ptr(raw: DecoderRawPtr) -> *mut dyn PointCloudDecoder {
    std::mem::transmute::<DecoderRawPtr, *mut dyn PointCloudDecoder>(raw)
}

#[inline]
unsafe fn encode_raw_ptr(decoder: &mut dyn PointCloudDecoder) -> DecoderRawPtr {
    std::mem::transmute::<&mut dyn PointCloudDecoder, DecoderRawPtr>(decoder)
}

pub struct AttributesDecoderBase {
    point_attribute_ids: Vec<i32>,
    point_attribute_to_local_id_map: Vec<i32>,
    // Safety: erased lifetime to allow self-referential decoder graph (matches C++ owner pointer).
    point_cloud_decoder: Option<DecoderRawPtr>,
    point_cloud: *mut PointCloud,
}

fn data_type_from_u8(value: u8) -> Option<DataType> {
    let dt = match value {
        x if x == DataType::Invalid as u8 => DataType::Invalid,
        x if x == DataType::Int8 as u8 => DataType::Int8,
        x if x == DataType::Uint8 as u8 => DataType::Uint8,
        x if x == DataType::Int16 as u8 => DataType::Int16,
        x if x == DataType::Uint16 as u8 => DataType::Uint16,
        x if x == DataType::Int32 as u8 => DataType::Int32,
        x if x == DataType::Uint32 as u8 => DataType::Uint32,
        x if x == DataType::Int64 as u8 => DataType::Int64,
        x if x == DataType::Uint64 as u8 => DataType::Uint64,
        x if x == DataType::Float32 as u8 => DataType::Float32,
        x if x == DataType::Float64 as u8 => DataType::Float64,
        x if x == DataType::Bool as u8 => DataType::Bool,
        x if x == DataType::TypesCount as u8 => DataType::TypesCount,
        _ => return None,
    };
    Some(dt)
}

fn geometry_attribute_type_from_u8(value: u8) -> Option<GeometryAttributeType> {
    let ga = match value {
        x if x == GeometryAttributeType::Position as u8 => GeometryAttributeType::Position,
        x if x == GeometryAttributeType::Normal as u8 => GeometryAttributeType::Normal,
        x if x == GeometryAttributeType::Color as u8 => GeometryAttributeType::Color,
        x if x == GeometryAttributeType::TexCoord as u8 => GeometryAttributeType::TexCoord,
        x if x == GeometryAttributeType::Generic as u8 => GeometryAttributeType::Generic,
        x if x == GeometryAttributeType::NamedAttributesCount as u8 => {
            GeometryAttributeType::NamedAttributesCount
        }
        _ => return None,
    };
    Some(ga)
}

impl AttributesDecoderBase {
    pub fn new() -> Self {
        Self {
            point_attribute_ids: Vec::new(),
            point_attribute_to_local_id_map: Vec::new(),
            point_cloud_decoder: None,
            point_cloud: std::ptr::null_mut(),
        }
    }

    pub fn init(&mut self, decoder: &mut dyn PointCloudDecoder, pc: &mut PointCloud) -> bool {
        self.point_cloud_decoder = Some(unsafe { encode_raw_ptr(decoder) });
        self.point_cloud = pc as *mut PointCloud;
        true
    }

    pub fn decode_attributes_decoder_data(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        let mut num_attributes: u32 = 0;
        let decoder_bitstream_version = match self.decoder() {
            Some(dec) => dec.bitstream_version(),
            None => return false,
        };
        if decoder_bitstream_version < bitstream_version(2, 0) {
            if !in_buffer.decode(&mut num_attributes) {
                return false;
            }
        } else if !decode_varint(&mut num_attributes, in_buffer) {
            return false;
        }

        if num_attributes == 0 {
            return false;
        }
        if num_attributes > (5 * in_buffer.remaining_size() as u32) {
            return false;
        }

        self.point_attribute_ids.resize(num_attributes as usize, -1);
        let pc_ptr = self.point_cloud;
        if pc_ptr.is_null() {
            return false;
        }
        let pc = unsafe { &mut *pc_ptr };
        for i in 0..num_attributes {
            let mut att_type: u8 = 0;
            let mut data_type: u8 = 0;
            let mut num_components: u8 = 0;
            let mut normalized: u8 = 0;
            if !in_buffer.decode(&mut att_type) {
                return false;
            }
            if !in_buffer.decode(&mut data_type) {
                return false;
            }
            if !in_buffer.decode(&mut num_components) {
                return false;
            }
            if !in_buffer.decode(&mut normalized) {
                return false;
            }
            if att_type >= GeometryAttributeType::NamedAttributesCount as u8 {
                return false;
            }
            if data_type == DataType::Invalid as u8 || data_type >= DataType::TypesCount as u8 {
                return false;
            }
            if num_components == 0 {
                return false;
            }

            let draco_dt = match data_type_from_u8(data_type) {
                Some(dt) if dt != DataType::Invalid && dt != DataType::TypesCount => dt,
                _ => return false,
            };
            let ga_type = match geometry_attribute_type_from_u8(att_type) {
                Some(t) => t,
                None => return false,
            };
            let mut ga = GeometryAttribute::new();
            ga.init(
                ga_type,
                None,
                num_components,
                draco_dt,
                normalized > 0,
                (data_type_length(draco_dt) as i64) * (num_components as i64),
                0,
            );

            let mut unique_id: u32 = 0;
            if decoder_bitstream_version < bitstream_version(1, 3) {
                let mut custom_id: u16 = 0;
                if !in_buffer.decode(&mut custom_id) {
                    return false;
                }
                unique_id = custom_id as u32;
                ga.set_unique_id(unique_id);
            } else if !decode_varint(&mut unique_id, in_buffer) {
                return false;
            } else {
                ga.set_unique_id(unique_id);
            }

            let pa = PointAttribute::from_geometry_attribute(ga);
            let att_id = pc.add_attribute(pa);
            if let Some(att) = pc.attribute_mut(att_id) {
                att.set_unique_id(unique_id);
            }
            self.point_attribute_ids[i as usize] = att_id;

            if att_id >= 0 {
                let idx = att_id as usize;
                if idx >= self.point_attribute_to_local_id_map.len() {
                    self.point_attribute_to_local_id_map.resize(idx + 1, -1);
                }
                self.point_attribute_to_local_id_map[idx] = i as i32;
            }
        }
        true
    }

    pub fn get_attribute_id(&self, i: i32) -> i32 {
        self.point_attribute_ids[i as usize]
    }

    pub fn get_num_attributes(&self) -> i32 {
        self.point_attribute_ids.len() as i32
    }

    pub fn decoder(&self) -> Option<&dyn PointCloudDecoder> {
        self.point_cloud_decoder
            .map(|raw| unsafe { &*decode_raw_ptr(raw) })
    }

    pub fn decoder_mut(&mut self) -> Option<&mut dyn PointCloudDecoder> {
        self.point_cloud_decoder
            .map(|raw| unsafe { &mut *decode_raw_ptr(raw) })
    }

    pub fn point_cloud_mut(&mut self) -> Option<&mut PointCloud> {
        unsafe { self.point_cloud.as_mut() }
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

impl Default for AttributesDecoderBase {
    fn default() -> Self {
        Self::new()
    }
}

pub trait AttributesDecoder: AttributesDecoderInterface {
    fn base(&self) -> &AttributesDecoderBase;
    fn base_mut(&mut self) -> &mut AttributesDecoderBase;

    fn init(&mut self, decoder: &mut dyn PointCloudDecoder, pc: &mut PointCloud) -> bool {
        self.base_mut().init(decoder, pc)
    }

    fn decode_attributes_decoder_data(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        self.base_mut().decode_attributes_decoder_data(in_buffer)
    }

    fn get_attribute_id(&self, i: i32) -> i32 {
        self.base().get_attribute_id(i)
    }

    fn get_num_attributes(&self) -> i32 {
        self.base().get_num_attributes()
    }

    fn get_decoder(&self) -> Option<&dyn PointCloudDecoder> {
        self.base().decoder()
    }

    fn decode_attributes(&mut self, in_buffer: &mut DecoderBuffer) -> bool {
        if !self.decode_portable_attributes(in_buffer) {
            return false;
        }
        if !self.decode_data_needed_by_portable_transforms(in_buffer) {
            return false;
        }
        if !self.transform_attributes_to_original_format() {
            return false;
        }
        true
    }

    fn decode_portable_attributes(&mut self, _in_buffer: &mut DecoderBuffer) -> bool;

    fn decode_data_needed_by_portable_transforms(
        &mut self,
        _in_buffer: &mut DecoderBuffer,
    ) -> bool {
        true
    }

    fn transform_attributes_to_original_format(&mut self) -> bool {
        true
    }
}
