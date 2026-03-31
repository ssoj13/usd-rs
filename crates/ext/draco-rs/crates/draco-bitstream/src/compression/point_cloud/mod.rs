//! Point cloud compression codecs and shared encoder/decoder base.
//! Reference: `_ref/draco/src/draco/compression/point_cloud`.
//!
//! Provides point cloud encoder/decoder interfaces, header parsing, and
//! sequential point cloud codecs.

pub mod algorithms;

use crate::compression::attributes::attributes_decoder_interface::AttributesDecoderInterface;
use crate::compression::attributes::attributes_encoder::AttributesEncoderInterface;
use crate::compression::attributes::kd_tree_attributes_decoder::KdTreeAttributesDecoder;
use crate::compression::attributes::kd_tree_attributes_encoder::KdTreeAttributesEncoder;
use crate::compression::attributes::linear_sequencer::LinearSequencer;
use crate::compression::attributes::sequential_attribute_decoders_controller::SequentialAttributeDecodersController;
use crate::compression::attributes::sequential_attribute_encoders_controller::SequentialAttributeEncodersController;
use crate::compression::config::compression_shared::{
    bitstream_version, DracoHeader, EncodedGeometryType, PointCloudEncodingMethod,
    K_DRACO_MESH_BITSTREAM_VERSION_MAJOR, K_DRACO_MESH_BITSTREAM_VERSION_MINOR,
    K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MAJOR, K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MINOR,
    METADATA_FLAG_MASK,
};
use crate::compression::config::decoder_options::DecoderOptions;
use crate::compression::config::encoder_options::EncoderOptions;
use crate::compression::mesh::MeshDecoder;
use draco_core::attributes::point_attribute::PointAttribute;
use draco_core::core::decoder_buffer::DecoderBuffer;
use draco_core::core::encoder_buffer::EncoderBuffer;
use draco_core::core::status::{ok_status, Status, StatusCode};
use draco_core::metadata::metadata_decoder::MetadataDecoder;
use draco_core::metadata::metadata_encoder::MetadataEncoder;
use draco_core::point_cloud::point_cloud::PointCloud;

pub struct PointCloudEncoderBase {
    pub(crate) point_cloud: *const PointCloud,
    pub(crate) attributes_encoders: Vec<Box<dyn AttributesEncoderInterface>>,
    pub(crate) attribute_to_encoder_map: Vec<i32>,
    pub(crate) attributes_encoder_ids_order: Vec<i32>,
    pub(crate) buffer: *mut EncoderBuffer,
    pub(crate) options: *const EncoderOptions,
    pub(crate) num_encoded_points: usize,
}

impl PointCloudEncoderBase {
    pub fn new() -> Self {
        Self {
            point_cloud: std::ptr::null(),
            attributes_encoders: Vec::new(),
            attribute_to_encoder_map: Vec::new(),
            attributes_encoder_ids_order: Vec::new(),
            buffer: std::ptr::null_mut(),
            options: std::ptr::null(),
            num_encoded_points: 0,
        }
    }

    fn clear_state(&mut self) {
        self.attributes_encoders.clear();
        self.attribute_to_encoder_map.clear();
        self.attributes_encoder_ids_order.clear();
    }
}

impl Default for PointCloudEncoderBase {
    fn default() -> Self {
        Self::new()
    }
}

/// Mesh prediction scheme data for mesh encoders. Returns `None` for point cloud encoders.
pub type MeshPredictionSchemeDataForEncoder =
    Option<crate::compression::attributes::prediction_schemes::mesh_prediction_scheme_data::
        MeshPredictionSchemeData<draco_core::mesh::corner_table::CornerTable>>;

pub trait PointCloudEncoder {
    fn base(&self) -> &PointCloudEncoderBase;
    fn base_mut(&mut self) -> &mut PointCloudEncoderBase;

    fn get_geometry_type(&self) -> EncodedGeometryType {
        EncodedGeometryType::PointCloud
    }

    /// Returns mesh prediction scheme data for the given attribute when encoding a mesh.
    /// Returns `None` for point cloud encoders or when data is not available.
    fn mesh_prediction_scheme_data(&self, _att_id: i32) -> MeshPredictionSchemeDataForEncoder {
        None
    }

    fn get_encoding_method(&self) -> u8;

    fn set_point_cloud(&mut self, pc: &PointCloud) {
        self.base_mut().point_cloud = pc as *const PointCloud;
    }

    fn point_cloud(&self) -> Option<&PointCloud> {
        unsafe { self.base().point_cloud.as_ref() }
    }

    fn buffer(&self) -> Option<&mut EncoderBuffer> {
        unsafe { self.base().buffer.as_mut() }
    }

    fn options(&self) -> &EncoderOptions {
        unsafe { &*self.base().options }
    }

    fn options_mut(&mut self) -> &mut EncoderOptions {
        unsafe { &mut *(self.base().options as *mut EncoderOptions) }
    }

    fn num_encoded_points(&self) -> usize {
        self.base().num_encoded_points
    }

    fn set_num_encoded_points(&mut self, num: usize) {
        self.base_mut().num_encoded_points = num;
    }

    fn add_attributes_encoder(&mut self, att_enc: Box<dyn AttributesEncoderInterface>) -> i32 {
        self.base_mut().attributes_encoders.push(att_enc);
        (self.base().attributes_encoders.len() as i32) - 1
    }

    fn attributes_encoder(&self, i: i32) -> Option<&Box<dyn AttributesEncoderInterface>> {
        self.base().attributes_encoders.get(i as usize)
    }

    fn attributes_encoder_mut(
        &mut self,
        i: i32,
    ) -> Option<&mut Box<dyn AttributesEncoderInterface>> {
        self.base_mut().attributes_encoders.get_mut(i as usize)
    }

    fn initialize_encoder(&mut self) -> bool {
        true
    }

    fn encode_encoder_data(&mut self) -> bool {
        true
    }

    fn encode_geometry_data(&mut self) -> Status {
        ok_status()
    }

    fn generate_attributes_encoder(&mut self, _att_id: i32) -> bool;

    fn encode_attributes_encoder_identifier(&mut self, _att_encoder_id: i32) -> bool {
        true
    }

    fn encode(&mut self, options: &EncoderOptions, out_buffer: &mut EncoderBuffer) -> Status
    where
        Self: Sized,
    {
        self.base_mut().options = options as *const EncoderOptions;
        self.base_mut().buffer = out_buffer as *mut EncoderBuffer;
        self.base_mut().clear_state();

        if self.point_cloud().is_none() {
            return Status::new(StatusCode::DracoError, "Invalid input geometry.");
        }
        let status = self.encode_header();
        if !status.is_ok() {
            return status;
        }
        let status = self.encode_metadata();
        if !status.is_ok() {
            return status;
        }
        if !self.initialize_encoder() {
            return Status::new(StatusCode::DracoError, "Failed to initialize encoder.");
        }
        if !self.encode_encoder_data() {
            return Status::new(StatusCode::DracoError, "Failed to encode internal data.");
        }
        let status = self.encode_geometry_data();
        if !status.is_ok() {
            return status;
        }
        if !self.encode_point_attributes() {
            return Status::new(StatusCode::DracoError, "Failed to encode point attributes.");
        }
        if self
            .options()
            .get_global_bool("store_number_of_encoded_points", false)
        {
            self.compute_number_of_encoded_points();
        }
        ok_status()
    }

    fn encode_header(&mut self) -> Status {
        let buffer = match self.buffer() {
            Some(buf) => buf,
            None => return Status::new(StatusCode::DracoError, "Missing output buffer."),
        };
        let pc = match self.point_cloud() {
            Some(pc) => pc,
            None => return Status::new(StatusCode::DracoError, "Missing point cloud."),
        };
        if !buffer.encode_bytes(b"DRACO") {
            return Status::new(StatusCode::DracoError, "Failed to encode header.");
        }
        let encoder_type = self.get_geometry_type();
        let version_major = if encoder_type == EncodedGeometryType::PointCloud {
            K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MAJOR
        } else {
            K_DRACO_MESH_BITSTREAM_VERSION_MAJOR
        };
        let version_minor = if encoder_type == EncodedGeometryType::PointCloud {
            K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MINOR
        } else {
            K_DRACO_MESH_BITSTREAM_VERSION_MINOR
        };
        if !buffer.encode(version_major)
            || !buffer.encode(version_minor)
            || !buffer.encode(encoder_type as u8)
            || !buffer.encode(self.get_encoding_method())
        {
            return Status::new(StatusCode::DracoError, "Failed to encode header.");
        }
        let mut flags: u16 = 0;
        if pc.get_metadata().is_some() {
            flags |= METADATA_FLAG_MASK;
        }
        if !buffer.encode(flags) {
            return Status::new(StatusCode::DracoError, "Failed to encode header.");
        }
        ok_status()
    }

    fn encode_metadata(&mut self) -> Status {
        let pc = match self.point_cloud() {
            Some(pc) => pc,
            None => return Status::new(StatusCode::DracoError, "Missing point cloud."),
        };
        let metadata = match pc.get_metadata() {
            Some(md) => md,
            None => return ok_status(),
        };
        let buffer = match self.buffer() {
            Some(buf) => buf,
            None => return Status::new(StatusCode::DracoError, "Missing output buffer."),
        };
        let encoder = MetadataEncoder::new();
        if !encoder.encode_geometry_metadata(buffer, metadata) {
            return Status::new(StatusCode::DracoError, "Failed to encode metadata.");
        }
        ok_status()
    }

    fn encode_point_attributes(&mut self) -> bool
    where
        Self: Sized,
    {
        let debug_attr = std::env::var("DRACO_DEBUG_ATTR").ok().as_deref() == Some("1");
        if !self.generate_attributes_encoders() {
            if debug_attr {
                eprintln!("[attr] generate_attributes_encoders failed");
            }
            return false;
        }
        let buffer_ptr = match self.buffer() {
            Some(buf) => buf as *mut EncoderBuffer,
            None => return false,
        };
        let pc_ptr = match self.point_cloud() {
            Some(pc) => pc as *const PointCloud,
            None => return false,
        };
        let buffer = unsafe { &mut *buffer_ptr };
        if !buffer.encode(self.base().attributes_encoders.len() as u8) {
            return false;
        }
        let pc = unsafe { &*pc_ptr };

        let mut encoders = std::mem::take(&mut self.base_mut().attributes_encoders);
        for (encoder_index, enc) in encoders.iter_mut().enumerate() {
            if !enc.init(self, pc) {
                if debug_attr {
                    eprintln!("[attr] init failed for encoder {}", encoder_index);
                }
                self.base_mut().attributes_encoders = encoders;
                return false;
            }
        }
        for i in 0..encoders.len() {
            let mut parent_attribute_ids = Vec::new();
            {
                let enc = &encoders[i];
                for attr_index in 0..enc.num_attributes() {
                    let att_id = enc.get_attribute_id(attr_index as i32);
                    for parent_index in 0..enc.num_parent_attributes(att_id) {
                        parent_attribute_ids
                            .push(enc.get_parent_attribute_id(att_id, parent_index));
                    }
                }
            }
            for parent_att_id in parent_attribute_ids {
                if parent_att_id < 0
                    || parent_att_id as usize >= self.base().attribute_to_encoder_map.len()
                {
                    if debug_attr {
                        eprintln!(
                            "[attr] invalid parent attribute {} for encoder {}",
                            parent_att_id, i
                        );
                    }
                    self.base_mut().attributes_encoders = encoders;
                    return false;
                }
                let parent_encoder_id =
                    self.base().attribute_to_encoder_map[parent_att_id as usize] as usize;
                if parent_encoder_id >= encoders.len()
                    || !encoders[parent_encoder_id].mark_parent_attribute(parent_att_id)
                {
                    if debug_attr {
                        eprintln!(
                            "[attr] mark_parent_attribute failed: parent_att_id={}, parent_encoder_id={}",
                            parent_att_id, parent_encoder_id
                        );
                    }
                    self.base_mut().attributes_encoders = encoders;
                    return false;
                }
            }
        }
        self.base_mut().attributes_encoders = encoders;
        if !self.rearrange_attributes_encoders() {
            if debug_attr {
                eprintln!("[attr] rearrange_attributes_encoders failed");
            }
            return false;
        }

        let encoder_ids = self.base().attributes_encoder_ids_order.clone();
        for &att_encoder_id in &encoder_ids {
            if !self.encode_attributes_encoder_identifier(att_encoder_id) {
                if debug_attr {
                    eprintln!(
                        "[attr] encode_attributes_encoder_identifier failed for {}",
                        att_encoder_id
                    );
                }
                return false;
            }
        }
        for att_encoder_id in encoder_ids {
            let enc = &mut self.base_mut().attributes_encoders[att_encoder_id as usize];
            let buffer = unsafe { &mut *buffer_ptr };
            if !enc.encode_attributes_encoder_data(buffer) {
                if debug_attr {
                    eprintln!(
                        "[attr] encode_attributes_encoder_data failed for {}",
                        att_encoder_id
                    );
                }
                return false;
            }
        }
        if !self.encode_all_attributes() {
            if debug_attr {
                eprintln!("[attr] encode_all_attributes failed");
            }
            return false;
        }
        true
    }

    fn generate_attributes_encoders(&mut self) -> bool {
        let num_attributes = match self.point_cloud() {
            Some(pc) => pc.num_attributes() as usize,
            None => return false,
        };
        for i in 0..num_attributes {
            if !self.generate_attributes_encoder(i as i32) {
                return false;
            }
        }
        self.base_mut()
            .attribute_to_encoder_map
            .resize(num_attributes, -1);
        let num_encoders = self.base().attributes_encoders.len();
        let mut mappings: Vec<(usize, i32)> = Vec::new();
        for i in 0..num_encoders {
            let encoder = &self.base().attributes_encoders[i];
            for j in 0..encoder.num_attributes() {
                let att_id = encoder.get_attribute_id(j as i32);
                if att_id >= 0 {
                    mappings.push((att_id as usize, i as i32));
                }
            }
        }
        let map = &mut self.base_mut().attribute_to_encoder_map;
        for (att_id, enc_id) in mappings {
            if att_id >= map.len() {
                map.resize(att_id + 1, -1);
            }
            map[att_id] = enc_id;
        }
        true
    }

    fn encode_all_attributes(&mut self) -> bool {
        let buffer_ptr = match self.buffer() {
            Some(buf) => buf as *mut EncoderBuffer,
            None => return false,
        };
        let encoder_ids = self.base().attributes_encoder_ids_order.clone();
        for att_encoder_id in encoder_ids {
            let enc = &mut self.base_mut().attributes_encoders[att_encoder_id as usize];
            // Safety: buffer is the shared output stream; encoders write sequentially.
            let buffer = unsafe { &mut *buffer_ptr };
            if !enc.encode_attributes(buffer) {
                return false;
            }
        }
        true
    }

    fn mark_parent_attribute(&mut self, parent_att_id: i32) -> bool {
        let pc = match self.point_cloud() {
            Some(pc) => pc,
            None => return false,
        };
        if parent_att_id < 0 || parent_att_id >= pc.num_attributes() {
            return false;
        }
        let parent_encoder_id = self.base().attribute_to_encoder_map[parent_att_id as usize];
        if parent_encoder_id < 0 {
            return false;
        }
        self.base_mut().attributes_encoders[parent_encoder_id as usize]
            .mark_parent_attribute(parent_att_id)
    }

    fn get_portable_attribute(&self, point_attribute_id: i32) -> Option<&PointAttribute> {
        let pc = self.point_cloud()?;
        if point_attribute_id < 0 || point_attribute_id >= pc.num_attributes() {
            return None;
        }
        let encoder_id = self.base().attribute_to_encoder_map[point_attribute_id as usize];
        if encoder_id < 0 {
            return None;
        }
        self.base().attributes_encoders[encoder_id as usize]
            .get_portable_attribute(point_attribute_id)
    }

    fn rearrange_attributes_encoders(&mut self) -> bool {
        let num_encoders = self.base().attributes_encoders.len();
        self.base_mut()
            .attributes_encoder_ids_order
            .resize(num_encoders, 0);
        let mut is_encoder_processed = vec![false; num_encoders];
        let mut num_processed = 0usize;

        while num_processed < num_encoders {
            let mut encoder_processed = false;
            for i in 0..num_encoders {
                if is_encoder_processed[i] {
                    continue;
                }
                let mut can_be_processed = true;
                let enc = &self.base().attributes_encoders[i];
                for p in 0..enc.num_attributes() {
                    let att_id = enc.get_attribute_id(p as i32);
                    for ap in 0..enc.num_parent_attributes(att_id) {
                        let parent_att_id = enc.get_parent_attribute_id(att_id, ap);
                        let parent_encoder_id =
                            self.base().attribute_to_encoder_map[parent_att_id as usize] as usize;
                        if parent_encoder_id != i && !is_encoder_processed[parent_encoder_id] {
                            can_be_processed = false;
                            break;
                        }
                    }
                    if !can_be_processed {
                        break;
                    }
                }
                if !can_be_processed {
                    continue;
                }
                self.base_mut().attributes_encoder_ids_order[num_processed] = i as i32;
                num_processed += 1;
                is_encoder_processed[i] = true;
                encoder_processed = true;
            }
            if !encoder_processed && num_processed < num_encoders {
                return false;
            }
        }

        let encoder_ids = self.base().attributes_encoder_ids_order.clone();
        for encoder_id in encoder_ids {
            let encoder_id = encoder_id as usize;
            let attribute_order = {
                let enc = &self.base().attributes_encoders[encoder_id];
                let num_encoder_attributes = enc.num_attributes() as usize;
                if num_encoder_attributes < 2 {
                    continue;
                }
                let mut is_attribute_processed = vec![false; num_encoder_attributes];
                let mut num_processed_attrs = 0usize;
                let mut attribute_order = vec![0i32; num_encoder_attributes];
                while num_processed_attrs < num_encoder_attributes {
                    let mut attribute_processed = false;
                    for i in 0..num_encoder_attributes {
                        let att_id = enc.get_attribute_id(i as i32);
                        if is_attribute_processed[i] {
                            continue;
                        }
                        let mut can_be_processed = true;
                        for p in 0..enc.num_parent_attributes(att_id) {
                            let parent_att_id = enc.get_parent_attribute_id(att_id, p);
                            let parent_local_id =
                                enc.get_local_id_for_point_attribute(parent_att_id);
                            if parent_local_id < 0 {
                                return false;
                            }
                            if !is_attribute_processed[parent_local_id as usize] {
                                can_be_processed = false;
                                break;
                            }
                        }
                        if !can_be_processed {
                            continue;
                        }
                        attribute_order[num_processed_attrs] = att_id;
                        num_processed_attrs += 1;
                        is_attribute_processed[i] = true;
                        attribute_processed = true;
                    }
                    if !attribute_processed && num_processed_attrs < num_encoder_attributes {
                        return false;
                    }
                }
                attribute_order
            };
            self.base_mut().attributes_encoders[encoder_id].set_attribute_ids(&attribute_order);
        }
        true
    }

    fn compute_number_of_encoded_points(&mut self);
}

pub struct PointCloudDecoderBase {
    pub(crate) point_cloud: *mut PointCloud,
    pub(crate) attributes_decoders: Vec<Box<dyn AttributesDecoderInterface>>,
    pub(crate) attribute_to_decoder_map: Vec<i32>,
    /// Pointer to decode input buffer. Set at start of decode(), used only during decode().
    /// SAFETY: erased to *mut DecoderBuffer<'static> so base can be stored without a lifetime param.
    /// Invariant: buffer is only dereferenced while decode() is running; in_buffer outlives decode().
    pub(crate) buffer: *mut DecoderBuffer<'static>,
    pub(crate) version_major: u8,
    pub(crate) version_minor: u8,
    pub(crate) options: *const DecoderOptions,
}

/// Erases DecoderBuffer lifetime for storage in PointCloudDecoderBase.
/// SAFETY: caller must ensure the buffer outlives all uses of the returned pointer.
/// Used only in decode(): in_buffer outlives the decode call; pointer is overwritten on next decode().
#[inline]
unsafe fn erase_decoder_buffer_lifetime(buffer: *mut DecoderBuffer) -> *mut DecoderBuffer<'static> {
    std::mem::transmute(buffer)
}

impl PointCloudDecoderBase {
    pub fn new() -> Self {
        Self {
            point_cloud: std::ptr::null_mut(),
            attributes_decoders: Vec::new(),
            attribute_to_decoder_map: Vec::new(),
            buffer: std::ptr::null_mut(),
            version_major: 0,
            version_minor: 0,
            options: std::ptr::null(),
        }
    }
}

impl Default for PointCloudDecoderBase {
    fn default() -> Self {
        Self::new()
    }
}

pub trait PointCloudDecoder {
    fn base(&self) -> &PointCloudDecoderBase;
    fn base_mut(&mut self) -> &mut PointCloudDecoderBase;

    fn as_mesh_decoder(&self) -> Option<&dyn MeshDecoder> {
        None
    }

    fn get_geometry_type(&self) -> EncodedGeometryType {
        EncodedGeometryType::PointCloud
    }

    fn decode(
        &mut self,
        options: &DecoderOptions,
        in_buffer: &mut DecoderBuffer,
        out_point_cloud: &mut PointCloud,
    ) -> Status
    where
        Self: Sized,
    {
        self.base_mut().options = options as *const DecoderOptions;
        // Safety: in_buffer outlives this decode call; stored pointer used only during decode.
        self.base_mut().buffer =
            unsafe { erase_decoder_buffer_lifetime(in_buffer as *mut DecoderBuffer) };
        self.base_mut().point_cloud = out_point_cloud as *mut PointCloud;
        self.base_mut().attributes_decoders.clear();
        self.base_mut().attribute_to_decoder_map.clear();

        let mut header = DracoHeader::default();
        let status = decode_header(in_buffer, &mut header);
        if !status.is_ok() {
            return status;
        }
        if header.encoder_type != self.get_geometry_type() as u8 {
            return Status::new(StatusCode::DracoError, "Using incompatible decoder.");
        }
        self.base_mut().version_major = header.version_major;
        self.base_mut().version_minor = header.version_minor;

        let max_major = if header.encoder_type == EncodedGeometryType::PointCloud as u8 {
            K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MAJOR
        } else {
            K_DRACO_MESH_BITSTREAM_VERSION_MAJOR
        };
        let max_minor = if header.encoder_type == EncodedGeometryType::PointCloud as u8 {
            K_DRACO_POINT_CLOUD_BITSTREAM_VERSION_MINOR
        } else {
            K_DRACO_MESH_BITSTREAM_VERSION_MINOR
        };
        if self.base().version_major < 1 || self.base().version_major > max_major {
            return Status::new(StatusCode::UnknownVersion, "Unknown major version.");
        }
        if self.base().version_major == max_major && self.base().version_minor > max_minor {
            return Status::new(StatusCode::UnknownVersion, "Unknown minor version.");
        }
        in_buffer.set_bitstream_version(bitstream_version(
            self.base().version_major,
            self.base().version_minor,
        ));

        if self.bitstream_version() >= bitstream_version(1, 3)
            && (header.flags & METADATA_FLAG_MASK) != 0
        {
            let status = self.decode_metadata();
            if !status.is_ok() {
                return status;
            }
        }
        if !self.initialize_decoder() {
            return Status::new(StatusCode::DracoError, "Failed to initialize decoder.");
        }
        if !self.decode_geometry_data() {
            return Status::new(StatusCode::DracoError, "Failed to decode geometry data.");
        }
        if !self.decode_point_attributes() {
            return Status::new(StatusCode::DracoError, "Failed to decode point attributes.");
        }
        ok_status()
    }

    fn initialize_decoder(&mut self) -> bool {
        true
    }

    fn create_attributes_decoder(&mut self, _att_decoder_id: i32) -> bool;

    fn decode_geometry_data(&mut self) -> bool {
        true
    }

    fn decode_point_attributes(&mut self) -> bool
    where
        Self: Sized,
    {
        let buffer_ptr = self.base().buffer;
        if buffer_ptr.is_null() {
            return false;
        }
        let mut num_decoders: u8 = 0;
        {
            let buffer = unsafe { &mut *buffer_ptr };
            if !buffer.decode(&mut num_decoders) {
                return false;
            }
        }
        for i in 0..num_decoders {
            if !self.create_attributes_decoder(i as i32) {
                return false;
            }
        }
        let num_decoders = self.base().attributes_decoders.len();
        let pc_ptr = self.base().point_cloud;
        if pc_ptr.is_null() {
            return false;
        }
        let pc = unsafe { &mut *pc_ptr };
        for i in 0..num_decoders {
            let dec_ptr = self.base_mut().attributes_decoders[i].as_mut()
                as *mut dyn AttributesDecoderInterface;
            let dec = unsafe { &mut *dec_ptr };
            if !dec.init(self, pc) {
                return false;
            }
        }
        for i in 0..num_decoders {
            let dec_ptr = self.base_mut().attributes_decoders[i].as_mut()
                as *mut dyn AttributesDecoderInterface;
            let dec = unsafe { &mut *dec_ptr };
            let buffer = unsafe { &mut *buffer_ptr };
            if !dec.decode_attributes_decoder_data(buffer) {
                return false;
            }
        }

        let mut mappings: Vec<(usize, i32)> = Vec::new();
        for i in 0..num_decoders {
            let dec_ptr = self.base().attributes_decoders[i].as_ref()
                as *const dyn AttributesDecoderInterface;
            let dec = unsafe { &*dec_ptr };
            let num_attributes = dec.get_num_attributes();
            for j in 0..num_attributes {
                let att_id = dec.get_attribute_id(j);
                if att_id >= 0 {
                    mappings.push((att_id as usize, i as i32));
                }
            }
        }
        let map = &mut self.base_mut().attribute_to_decoder_map;
        for (att_id, dec_id) in mappings {
            if att_id >= map.len() {
                map.resize(att_id + 1, -1);
            }
            map[att_id] = dec_id;
        }

        if !self.decode_all_attributes() {
            return false;
        }
        if !self.on_attributes_decoded() {
            return false;
        }
        true
    }

    fn decode_all_attributes(&mut self) -> bool {
        let buffer_ptr = self.base().buffer;
        if buffer_ptr.is_null() {
            return false;
        }
        let num_decoders = self.base().attributes_decoders.len();
        for i in 0..num_decoders {
            let dec_ptr = self.base_mut().attributes_decoders[i].as_mut()
                as *mut dyn AttributesDecoderInterface;
            let dec = unsafe { &mut *dec_ptr };
            // Safety: buffer is the shared bitstream; decoders read sequentially.
            let buffer = unsafe { &mut *buffer_ptr };
            if !dec.decode_attributes(buffer) {
                return false;
            }
        }
        true
    }

    fn on_attributes_decoded(&mut self) -> bool {
        true
    }

    fn decode_metadata(&mut self) -> Status {
        let buffer_ptr = self.base().buffer;
        if buffer_ptr.is_null() {
            return Status::new(StatusCode::DracoError, "Missing buffer.");
        }
        let pc_ptr = self.base().point_cloud;
        if pc_ptr.is_null() {
            return Status::new(StatusCode::DracoError, "Missing point cloud.");
        }
        let mut metadata = draco_core::metadata::geometry_metadata::GeometryMetadata::new();
        let mut decoder = MetadataDecoder::new();
        // Safety: buffer and point cloud pointers are disjoint mutable fields.
        let buffer = unsafe { &mut *buffer_ptr };
        if !decoder.decode_geometry_metadata(buffer, &mut metadata) {
            return Status::new(StatusCode::DracoError, "Failed to decode metadata.");
        }
        let pc = unsafe { &mut *pc_ptr };
        pc.add_metadata(metadata);
        ok_status()
    }

    fn buffer(&self) -> Option<&DecoderBuffer<'_>> {
        unsafe { self.base().buffer.as_ref() }
    }

    fn buffer_mut(&mut self) -> Option<&mut DecoderBuffer<'_>> {
        // SAFETY: we stored the buffer with erased 'static; the actual data outlives self's borrow.
        // Transmuting to '_ shortens the lifetime for the return; the reference is valid for the call.
        unsafe {
            self.base().buffer.as_mut().map(|buf| {
                std::mem::transmute::<&mut DecoderBuffer<'static>, &mut DecoderBuffer<'_>>(buf)
            })
        }
    }

    fn point_cloud(&self) -> Option<&PointCloud> {
        unsafe { self.base().point_cloud.as_ref() }
    }

    fn point_cloud_mut(&mut self) -> Option<&mut PointCloud> {
        unsafe { self.base().point_cloud.as_mut() }
    }

    fn options(&self) -> Option<&DecoderOptions> {
        unsafe { self.base().options.as_ref() }
    }

    fn set_attributes_decoder(
        &mut self,
        att_decoder_id: i32,
        decoder: Box<dyn AttributesDecoderInterface>,
    ) -> bool {
        if att_decoder_id < 0 {
            return false;
        }
        if att_decoder_id as usize >= self.base().attributes_decoders.len() {
            self.base_mut()
                .attributes_decoders
                .resize_with(att_decoder_id as usize + 1, || {
                    Box::new(SequentialAttributeDecodersController::new(Box::new(
                        LinearSequencer::new(0),
                    )))
                });
        }
        self.base_mut().attributes_decoders[att_decoder_id as usize] = decoder;
        true
    }

    fn get_portable_attribute(
        &self,
        point_attribute_id: i32,
    ) -> Option<&draco_core::attributes::point_attribute::PointAttribute> {
        let pc = self.point_cloud()?;
        if point_attribute_id < 0 || point_attribute_id >= pc.num_attributes() {
            return None;
        }
        let decoder_id = self.base().attribute_to_decoder_map[point_attribute_id as usize];
        if decoder_id < 0 {
            return None;
        }
        self.base().attributes_decoders[decoder_id as usize]
            .get_portable_attribute(point_attribute_id)
    }

    fn bitstream_version(&self) -> u16 {
        bitstream_version(self.base().version_major, self.base().version_minor)
    }
}

pub struct PointCloudSequentialEncoder {
    base: PointCloudEncoderBase,
}

impl PointCloudSequentialEncoder {
    pub fn new() -> Self {
        Self {
            base: PointCloudEncoderBase::new(),
        }
    }
}

impl Default for PointCloudSequentialEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudEncoder for PointCloudSequentialEncoder {
    fn base(&self) -> &PointCloudEncoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PointCloudEncoderBase {
        &mut self.base
    }

    fn get_encoding_method(&self) -> u8 {
        PointCloudEncodingMethod::PointCloudSequentialEncoding as u8
    }

    fn encode_geometry_data(&mut self) -> Status {
        let pc = match self.point_cloud() {
            Some(pc) => pc,
            None => return Status::new(StatusCode::DracoError, "Missing point cloud."),
        };
        let num_points = pc.num_points() as i32;
        if let Some(buf) = self.buffer() {
            if !buf.encode(num_points) {
                return Status::new(StatusCode::DracoError, "Failed to encode geometry.");
            }
        }
        ok_status()
    }

    fn generate_attributes_encoder(&mut self, att_id: i32) -> bool {
        if att_id == 0 {
            let num_points = self
                .point_cloud()
                .map(|pc| pc.num_points() as i32)
                .unwrap_or(0);
            let sequencer = Box::new(LinearSequencer::new(num_points));
            let enc = SequentialAttributeEncodersController::with_attribute_id(sequencer, att_id);
            self.add_attributes_encoder(Box::new(enc));
        } else if let Some(enc) = self.attributes_encoder_mut(0) {
            enc.add_attribute_id(att_id);
        }
        true
    }

    fn compute_number_of_encoded_points(&mut self) {
        let num_points = self
            .point_cloud()
            .map(|pc| pc.num_points() as usize)
            .unwrap_or(0);
        self.set_num_encoded_points(num_points);
    }
}

pub struct PointCloudSequentialDecoder {
    base: PointCloudDecoderBase,
}

impl PointCloudSequentialDecoder {
    pub fn new() -> Self {
        Self {
            base: PointCloudDecoderBase::new(),
        }
    }
}

impl Default for PointCloudSequentialDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudDecoder for PointCloudSequentialDecoder {
    fn base(&self) -> &PointCloudDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PointCloudDecoderBase {
        &mut self.base
    }

    fn decode_geometry_data(&mut self) -> bool {
        let mut num_points: i32 = 0;
        let buffer = match self.buffer_mut() {
            Some(buf) => buf,
            None => return false,
        };
        if !buffer.decode(&mut num_points) {
            return false;
        }
        if let Some(pc) = self.point_cloud_mut() {
            pc.set_num_points(num_points as u32);
        }
        true
    }

    fn create_attributes_decoder(&mut self, att_decoder_id: i32) -> bool {
        let num_points = self
            .point_cloud()
            .map(|pc| pc.num_points() as i32)
            .unwrap_or(0);
        let sequencer = Box::new(LinearSequencer::new(num_points));
        let dec = SequentialAttributeDecodersController::new(sequencer);
        self.set_attributes_decoder(att_decoder_id, Box::new(dec))
    }
}

pub struct PointCloudKdTreeEncoder {
    base: PointCloudEncoderBase,
}

impl PointCloudKdTreeEncoder {
    pub fn new() -> Self {
        Self {
            base: PointCloudEncoderBase::new(),
        }
    }
}

impl Default for PointCloudKdTreeEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudEncoder for PointCloudKdTreeEncoder {
    fn base(&self) -> &PointCloudEncoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PointCloudEncoderBase {
        &mut self.base
    }

    fn get_encoding_method(&self) -> u8 {
        PointCloudEncodingMethod::PointCloudKdTreeEncoding as u8
    }

    fn encode_geometry_data(&mut self) -> Status {
        let buffer = match self.buffer() {
            Some(buf) => buf,
            None => return Status::new(StatusCode::DracoError, "Missing output buffer."),
        };
        let pc = match self.point_cloud() {
            Some(pc) => pc,
            None => return Status::new(StatusCode::DracoError, "Missing point cloud."),
        };
        if !buffer.encode(pc.num_points() as i32) {
            return Status::new(StatusCode::DracoError, "Failed to encode geometry data.");
        }
        ok_status()
    }

    fn generate_attributes_encoder(&mut self, att_id: i32) -> bool {
        if self.base().attributes_encoders.is_empty() {
            let enc = KdTreeAttributesEncoder::with_attribute_id(att_id);
            self.add_attributes_encoder(Box::new(enc));
            return true;
        }
        if let Some(enc) = self.attributes_encoder_mut(0) {
            enc.add_attribute_id(att_id);
            return true;
        }
        false
    }

    fn compute_number_of_encoded_points(&mut self) {
        if let Some(pc) = self.point_cloud() {
            self.set_num_encoded_points(pc.num_points() as usize);
        }
    }
}

pub struct PointCloudKdTreeDecoder {
    base: PointCloudDecoderBase,
}

impl PointCloudKdTreeDecoder {
    pub fn new() -> Self {
        Self {
            base: PointCloudDecoderBase::new(),
        }
    }
}

impl Default for PointCloudKdTreeDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PointCloudDecoder for PointCloudKdTreeDecoder {
    fn base(&self) -> &PointCloudDecoderBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut PointCloudDecoderBase {
        &mut self.base
    }

    fn decode_geometry_data(&mut self) -> bool {
        let mut num_points: i32 = 0;
        let buffer = match self.buffer_mut() {
            Some(buf) => buf,
            None => return false,
        };
        if !buffer.decode(&mut num_points) {
            return false;
        }
        if num_points < 0 {
            return false;
        }
        if let Some(pc) = self.point_cloud_mut() {
            pc.set_num_points(num_points as u32);
        }
        true
    }

    fn create_attributes_decoder(&mut self, att_decoder_id: i32) -> bool {
        let dec = KdTreeAttributesDecoder::new();
        self.set_attributes_decoder(att_decoder_id, Box::new(dec))
    }
}

pub fn decode_header(buffer: &mut DecoderBuffer, out_header: &mut DracoHeader) -> Status {
    const IO_ERROR_MSG: &str = "Failed to parse Draco header.";
    let mut magic = [0u8; 5];
    if !buffer.decode_bytes(&mut magic) {
        return Status::new(StatusCode::IoError, IO_ERROR_MSG);
    }
    if magic != *b"DRACO" {
        return Status::new(StatusCode::DracoError, "Not a Draco file.");
    }
    out_header.draco_string = [
        magic[0] as i8,
        magic[1] as i8,
        magic[2] as i8,
        magic[3] as i8,
        magic[4] as i8,
    ];
    if !buffer.decode(&mut out_header.version_major) {
        return Status::new(StatusCode::IoError, IO_ERROR_MSG);
    }
    if !buffer.decode(&mut out_header.version_minor) {
        return Status::new(StatusCode::IoError, IO_ERROR_MSG);
    }
    if !buffer.decode(&mut out_header.encoder_type) {
        return Status::new(StatusCode::IoError, IO_ERROR_MSG);
    }
    if !buffer.decode(&mut out_header.encoder_method) {
        return Status::new(StatusCode::IoError, IO_ERROR_MSG);
    }
    if !buffer.decode(&mut out_header.flags) {
        return Status::new(StatusCode::IoError, IO_ERROR_MSG);
    }
    ok_status()
}
