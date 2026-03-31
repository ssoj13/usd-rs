//! Sequential attribute decoder base.
//! Reference: `_ref/draco/src/draco/compression/attributes/sequential_attribute_decoder.h|cc`.

use crate::compression::attributes::prediction_schemes::prediction_scheme_interface::PredictionSchemeInterface;
use crate::compression::point_cloud::PointCloudDecoder;
use draco_core::attributes::geometry_indices::PointIndex;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use std::cell::UnsafeCell;

type DecoderRawPtr = (*mut (), *mut ());

#[inline]
unsafe fn encode_raw_ptr(decoder: &mut dyn PointCloudDecoder) -> DecoderRawPtr {
    std::mem::transmute::<&mut dyn PointCloudDecoder, DecoderRawPtr>(decoder)
}

#[inline]
unsafe fn decode_raw_ptr(raw: DecoderRawPtr) -> *mut dyn PointCloudDecoder {
    std::mem::transmute::<DecoderRawPtr, *mut dyn PointCloudDecoder>(raw)
}

pub struct SequentialAttributeDecoderBase {
    // Safety: erased lifetime to allow self-referential decoder graph (matches C++ owner pointer).
    decoder: Option<DecoderRawPtr>,
    attribute: *mut PointAttribute,
    attribute_id: i32,
    // Safety: portable attributes are mutated through interior mutability to match C++ GetPortableAttribute().
    portable_attribute: Option<UnsafeCell<PointAttribute>>,
}

impl SequentialAttributeDecoderBase {
    pub fn new() -> Self {
        Self {
            decoder: None,
            attribute: std::ptr::null_mut(),
            attribute_id: -1,
            portable_attribute: None,
        }
    }

    pub fn init(&mut self, decoder: &mut dyn PointCloudDecoder, attribute_id: i32) -> bool {
        self.decoder = Some(unsafe { encode_raw_ptr(decoder) });
        let att = match decoder
            .point_cloud_mut()
            .and_then(|pc| pc.attribute_mut(attribute_id))
        {
            Some(att) => att as *mut PointAttribute,
            None => return false,
        };
        self.attribute = att;
        self.attribute_id = attribute_id;
        true
    }

    pub fn initialize_standalone(&mut self, attribute: &mut PointAttribute) -> bool {
        self.attribute = attribute as *mut PointAttribute;
        self.attribute_id = -1;
        true
    }

    pub fn attribute(&self) -> &PointAttribute {
        let attribute_id = self.attribute_id;
        if let Some(decoder) = self.decoder() {
            if let Some(pc) = decoder.point_cloud() {
                if let Some(att) = pc.attribute(attribute_id) {
                    return att;
                }
            }
        }
        unsafe { &*self.attribute }
    }

    pub fn attribute_mut(&mut self) -> &mut PointAttribute {
        let attribute_id = self.attribute_id;
        let attribute_ptr = self.attribute;
        if let Some(decoder) = self.decoder_mut() {
            if let Some(pc) = decoder.point_cloud_mut() {
                if let Some(att) = pc.attribute_mut(attribute_id) {
                    return att;
                }
            }
        }
        unsafe { &mut *attribute_ptr }
    }

    pub fn attribute_id(&self) -> i32 {
        self.attribute_id
    }

    pub fn decoder(&self) -> Option<&dyn PointCloudDecoder> {
        self.decoder.map(|raw| unsafe { &*decode_raw_ptr(raw) })
    }

    pub fn decoder_mut(&mut self) -> Option<&mut dyn PointCloudDecoder> {
        self.decoder.map(|raw| unsafe { &mut *decode_raw_ptr(raw) })
    }

    pub fn set_portable_attribute(&mut self, att: PointAttribute) {
        self.portable_attribute = Some(UnsafeCell::new(att));
    }

    pub fn portable_attribute(&self) -> Option<&PointAttribute> {
        self.portable_attribute
            .as_ref()
            .map(|cell| unsafe { &*cell.get() })
    }

    pub fn portable_attribute_mut(&mut self) -> Option<&mut PointAttribute> {
        self.portable_attribute
            .as_ref()
            .map(|cell| unsafe { &mut *cell.get() })
    }

    pub fn copy_portable_to_attribute(&mut self) -> bool {
        // Copying via raw pointer avoids aliasing the same decoder immutably/mutably.
        let portable_ptr = match self.portable_attribute.as_ref() {
            Some(att) => att.get() as *const PointAttribute,
            None => return false,
        };
        let portable = unsafe { &*portable_ptr };
        self.attribute_mut().copy_from(portable);
        true
    }

    pub fn get_portable_attribute(&self) -> Option<&PointAttribute> {
        let portable_cell = self.portable_attribute.as_ref()?;
        let portable = unsafe { &*portable_cell.get() };
        if !self.attribute().is_mapping_identity() && portable.is_mapping_identity() {
            let map_size = self.attribute().indices_map_size();
            if map_size > 0 {
                // Safety: portable_attribute is owned by this decoder; mapping update mirrors C++ behavior.
                let portable_mut = unsafe { &mut *portable_cell.get() };
                portable_mut.set_explicit_mapping(map_size);
                for i in 0..map_size {
                    let pi = PointIndex::from(i as u32);
                    portable_mut.set_point_map_entry(pi, self.attribute().mapped_index(pi));
                }
            }
        }
        Some(portable)
    }

    pub fn init_prediction_scheme(&mut self, ps: &mut dyn PredictionSchemeInterface) -> bool {
        let decoder = match self.decoder() {
            Some(dec) => dec,
            None => return false,
        };
        for i in 0..ps.get_num_parent_attributes() {
            let att_id = decoder
                .point_cloud()
                .map(|pc| pc.get_named_attribute_id(ps.get_parent_attribute_type(i)))
                .unwrap_or(-1);
            if att_id == -1 {
                return false;
            }
            if decoder.bitstream_version()
                < crate::compression::config::compression_shared::bitstream_version(2, 0)
            {
                let parent = decoder.point_cloud().and_then(|pc| pc.attribute(att_id));
                if parent.is_none() || !ps.set_parent_attribute(parent.unwrap()) {
                    return false;
                }
            } else {
                let parent = decoder.get_portable_attribute(att_id);
                if parent.is_none() || !ps.set_parent_attribute(parent.unwrap()) {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for SequentialAttributeDecoderBase {
    fn default() -> Self {
        Self::new()
    }
}

pub trait SequentialAttributeDecoderInterface {
    fn base(&self) -> &SequentialAttributeDecoderBase;
    fn base_mut(&mut self) -> &mut SequentialAttributeDecoderBase;

    fn init(&mut self, decoder: &mut dyn PointCloudDecoder, attribute_id: i32) -> bool {
        self.base_mut().init(decoder, attribute_id)
    }

    fn initialize_standalone(&mut self, attribute: &mut PointAttribute) -> bool {
        self.base_mut().initialize_standalone(attribute)
    }

    fn decode_portable_attribute(
        &mut self,
        point_ids: &[PointIndex],
        in_buffer: &mut DecoderBuffer,
    ) -> bool {
        let att = self.base_mut().attribute_mut();
        if att.num_components() <= 0 {
            return false;
        }
        if !att.reset(point_ids.len()) {
            return false;
        }
        if !self.decode_values(point_ids, in_buffer) {
            return false;
        }
        true
    }

    fn decode_data_needed_by_portable_transform(
        &mut self,
        _point_ids: &[PointIndex],
        _in_buffer: &mut DecoderBuffer,
    ) -> bool {
        true
    }

    fn transform_attribute_to_original_format(&mut self, _point_ids: &[PointIndex]) -> bool {
        true
    }

    fn get_portable_attribute(&self) -> Option<&PointAttribute> {
        self.base().get_portable_attribute()
    }

    fn copy_portable_to_attribute(&mut self) -> bool {
        self.base_mut().copy_portable_to_attribute()
    }

    fn attribute(&self) -> &PointAttribute {
        self.base().attribute()
    }

    fn attribute_mut(&mut self) -> &mut PointAttribute {
        self.base_mut().attribute_mut()
    }

    fn decoder(&self) -> Option<&dyn PointCloudDecoder> {
        self.base().decoder()
    }

    fn init_prediction_scheme(&mut self, ps: &mut dyn PredictionSchemeInterface) -> bool {
        self.base_mut().init_prediction_scheme(ps)
    }

    fn set_portable_attribute(&mut self, att: PointAttribute) {
        self.base_mut().set_portable_attribute(att)
    }

    fn portable_attribute_mut(&mut self) -> Option<&mut PointAttribute> {
        self.base_mut().portable_attribute_mut()
    }

    fn decode_values(&mut self, point_ids: &[PointIndex], in_buffer: &mut DecoderBuffer) -> bool {
        let num_values = point_ids.len();
        let entry_size = self.attribute().byte_stride() as usize;
        let mut value_data = vec![0u8; entry_size];
        let mut out_byte_pos = 0i64;
        for _ in 0..num_values {
            if !in_buffer.decode_bytes(&mut value_data) {
                return false;
            }
            if let Some(buf) = self.attribute().buffer() {
                buf.borrow_mut().write(out_byte_pos, &value_data);
            }
            out_byte_pos += entry_size as i64;
        }
        true
    }
}

/// Generic sequential attribute decoder with no special transforms.
pub struct SequentialAttributeDecoder {
    base: SequentialAttributeDecoderBase,
}

impl SequentialAttributeDecoder {
    pub fn new() -> Self {
        Self {
            base: SequentialAttributeDecoderBase::new(),
        }
    }
}

impl Default for SequentialAttributeDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl SequentialAttributeDecoderInterface for SequentialAttributeDecoder {
    fn base(&self) -> &SequentialAttributeDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut SequentialAttributeDecoderBase {
        &mut self.base
    }
}
