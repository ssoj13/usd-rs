//! Metadata encoder.
//! Reference: `_ref/draco/src/draco/metadata/metadata_encoder.h` + `.cc`.

use crate::core::encoder_buffer::EncoderBuffer;
use crate::core::varint_encoding::encode_varint;
use crate::metadata::geometry_metadata::AttributeMetadata;
use crate::metadata::geometry_metadata::GeometryMetadata;
use crate::metadata::metadata::{Metadata, MetadataName};

pub struct MetadataEncoder;

impl MetadataEncoder {
    pub fn new() -> Self {
        Self
    }

    pub fn encode_geometry_metadata(
        &self,
        out_buffer: &mut EncoderBuffer,
        metadata: &GeometryMetadata,
    ) -> bool {
        let att_metadatas = metadata.attribute_metadatas();
        encode_varint(att_metadatas.len() as u32, out_buffer);
        for att_metadata in att_metadatas {
            if !self.encode_attribute_metadata(out_buffer, att_metadata) {
                return false;
            }
        }
        self.encode_metadata(out_buffer, metadata)
    }

    pub fn encode_metadata(&self, out_buffer: &mut EncoderBuffer, metadata: &Metadata) -> bool {
        let entries = metadata.entries();
        encode_varint(entries.len() as u32, out_buffer);
        for (name, entry) in entries {
            if !self.encode_name(out_buffer, name) {
                return false;
            }
            let entry_value = entry.data();
            let data_size = entry_value.len() as u32;
            encode_varint(data_size, out_buffer);
            if !out_buffer.encode_bytes(entry_value) {
                return false;
            }
        }

        let sub_metadatas = metadata.sub_metadatas();
        encode_varint(sub_metadatas.len() as u32, out_buffer);
        for (name, sub) in sub_metadatas {
            if !self.encode_name(out_buffer, name) {
                return false;
            }
            if !self.encode_metadata(out_buffer, sub) {
                return false;
            }
        }
        true
    }

    fn encode_attribute_metadata(
        &self,
        out_buffer: &mut EncoderBuffer,
        metadata: &AttributeMetadata,
    ) -> bool {
        encode_varint(metadata.att_unique_id(), out_buffer);
        self.encode_metadata(out_buffer, metadata)
    }

    fn encode_name(&self, out_buffer: &mut EncoderBuffer, value: &MetadataName) -> bool {
        let bytes = value.as_bytes();
        if bytes.len() > 255 {
            return false;
        }
        if bytes.is_empty() {
            out_buffer.encode(0u8)
        } else {
            if !out_buffer.encode(bytes.len() as u8) {
                return false;
            }
            out_buffer.encode_bytes(bytes)
        }
    }
}

impl Default for MetadataEncoder {
    fn default() -> Self {
        Self::new()
    }
}
