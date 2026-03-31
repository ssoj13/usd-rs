//! Metadata decoder.
//! Reference: `_ref/draco/src/draco/metadata/metadata_decoder.h` + `.cc`.

use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::varint_decoding::decode_varint;
use crate::metadata::geometry_metadata::AttributeMetadata;
use crate::metadata::geometry_metadata::GeometryMetadata;
use crate::metadata::metadata::{Metadata, MetadataName};

const MAX_SUBMETADATA_LEVEL: i32 = 1000;

pub struct MetadataDecoder;

impl MetadataDecoder {
    pub fn new() -> Self {
        Self
    }

    pub fn decode_metadata(
        &mut self,
        in_buffer: &mut DecoderBuffer,
        metadata: &mut Metadata,
    ) -> bool {
        self.decode_metadata_internal(in_buffer, metadata)
    }

    pub fn decode_geometry_metadata(
        &mut self,
        in_buffer: &mut DecoderBuffer,
        metadata: &mut GeometryMetadata,
    ) -> bool {
        let mut num_att_metadata: u32 = 0;
        if !decode_varint(&mut num_att_metadata, in_buffer) {
            return false;
        }
        for _ in 0..num_att_metadata {
            let mut att_unique_id: u32 = 0;
            if !decode_varint(&mut att_unique_id, in_buffer) {
                return false;
            }
            let mut att_metadata = AttributeMetadata::new();
            att_metadata.set_att_unique_id(att_unique_id);
            if !self.decode_metadata_internal(in_buffer, &mut att_metadata) {
                return false;
            }
            if !metadata.add_attribute_metadata(Some(Box::new(att_metadata))) {
                return false;
            }
        }
        self.decode_metadata_internal(in_buffer, metadata)
    }

    /// Iterative metadata decoding using an explicit work stack.
    /// Mirrors the C++ `DecodeMetadata(Metadata*)` which also uses a stack
    /// to avoid deep recursion on nested sub-metadata (up to 1000 levels).
    fn decode_metadata_internal(
        &mut self,
        buffer: &mut DecoderBuffer,
        metadata: &mut Metadata,
    ) -> bool {
        // Work item for the explicit stack, mirrors C++ MetadataTuple.
        // Uses raw pointers to parent/current metadata to avoid borrow conflicts.
        struct WorkItem {
            parent: *mut Metadata,
            metadata: *mut Metadata,
            level: i32,
        }

        let mut stack: Vec<WorkItem> = Vec::new();
        // Seed with root metadata (no parent)
        stack.push(WorkItem {
            parent: std::ptr::null_mut(),
            metadata: metadata as *mut Metadata,
            level: 0,
        });

        while let Some(item) = stack.pop() {
            // Resolve current metadata target
            // SAFETY: pointers are derived from valid &mut references or from
            // heap-allocated Box<Metadata> inside BTreeMap that won't move.
            // Only one mutable reference is active at a time within each iteration.
            let current: *mut Metadata = if !item.parent.is_null() {
                // Sub-metadata: enforce nesting depth limit
                if item.level > MAX_SUBMETADATA_LEVEL {
                    return false;
                }
                // Decode sub-metadata name from stream
                let mut name = MetadataName::default();
                if !self.decode_name(buffer, &mut name) {
                    return false;
                }
                // Create empty sub-metadata, insert into parent, get ptr back
                let parent = unsafe { &mut *item.parent };
                if !parent.add_sub_metadata(&name, Metadata::new()) {
                    return false;
                }
                match parent.sub_metadata(&name) {
                    Some(m) => m as *mut Metadata,
                    None => return false,
                }
            } else {
                // Root item: use the provided metadata pointer directly
                item.metadata
            };

            if current.is_null() {
                return false;
            }
            let md = unsafe { &mut *current };

            // Decode key-value entries for this metadata node
            let mut num_entries: u32 = 0;
            if !decode_varint(&mut num_entries, buffer) {
                return false;
            }
            for _ in 0..num_entries {
                if !self.decode_entry(buffer, md) {
                    return false;
                }
            }

            // Decode sub-metadata count and push work items (LIFO = DFS order)
            let mut num_sub_metadata: u32 = 0;
            if !decode_varint(&mut num_sub_metadata, buffer) {
                return false;
            }
            if (num_sub_metadata as i64) > buffer.remaining_size() {
                return false;
            }
            let next_level = if !item.parent.is_null() {
                item.level + 1
            } else {
                item.level
            };
            for _ in 0..num_sub_metadata {
                stack.push(WorkItem {
                    parent: current,
                    metadata: std::ptr::null_mut(),
                    level: next_level,
                });
            }
        }
        true
    }

    fn decode_entry(&mut self, buffer: &mut DecoderBuffer, metadata: &mut Metadata) -> bool {
        let mut entry_name = MetadataName::default();
        if !self.decode_name(buffer, &mut entry_name) {
            return false;
        }
        let mut data_size: u32 = 0;
        if !decode_varint(&mut data_size, buffer) {
            return false;
        }
        if data_size == 0 {
            return false;
        }
        if (data_size as i64) > buffer.remaining_size() {
            return false;
        }
        let mut entry_value = vec![0u8; data_size as usize];
        if !buffer.decode_bytes(&mut entry_value) {
            return false;
        }
        metadata.add_entry_binary(&entry_name, &entry_value);
        true
    }

    fn decode_name(&mut self, buffer: &mut DecoderBuffer, name: &mut MetadataName) -> bool {
        let mut name_len: u8 = 0;
        if !buffer.decode(&mut name_len) {
            return false;
        }
        if name_len == 0 {
            *name = MetadataName::default();
            return true;
        }
        let mut bytes = vec![0u8; name_len as usize];
        if !buffer.decode_bytes(&mut bytes) {
            return false;
        }
        *name = MetadataName::from(bytes);
        true
    }
}

impl Default for MetadataDecoder {
    fn default() -> Self {
        Self::new()
    }
}
