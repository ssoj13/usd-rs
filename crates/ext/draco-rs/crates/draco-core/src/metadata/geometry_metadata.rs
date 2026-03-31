//! Geometry metadata utilities.
//! Reference: `_ref/draco/src/draco/metadata/geometry_metadata.h` + `.cc`.

use std::ops::{Deref, DerefMut};

use crate::core::hash_utils::hash_combine_with;
use crate::metadata::metadata::{Metadata, MetadataHasher, MetadataString};

#[derive(Clone, Debug)]
pub struct AttributeMetadata {
    metadata: Metadata,
    att_unique_id: u32,
}

impl AttributeMetadata {
    pub fn new() -> Self {
        Self {
            metadata: Metadata::new(),
            att_unique_id: 0,
        }
    }

    pub fn from_metadata(metadata: Metadata) -> Self {
        Self {
            metadata,
            att_unique_id: 0,
        }
    }

    pub fn set_att_unique_id(&mut self, att_unique_id: u32) {
        self.att_unique_id = att_unique_id;
    }

    pub fn att_unique_id(&self) -> u32 {
        self.att_unique_id
    }
}

impl Deref for AttributeMetadata {
    type Target = Metadata;

    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

impl DerefMut for AttributeMetadata {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.metadata
    }
}

pub struct AttributeMetadataHasher;

impl AttributeMetadataHasher {
    pub fn hash(&self, metadata: &AttributeMetadata) -> u64 {
        let mut hash = metadata.att_unique_id as u64;
        let metadata_hasher = MetadataHasher;
        let base_hash = metadata_hasher.hash(&metadata.metadata);
        hash = hash_combine_with(&base_hash, hash);
        hash
    }
}

#[derive(Clone, Debug)]
pub struct GeometryMetadata {
    metadata: Metadata,
    att_metadatas: Vec<Box<AttributeMetadata>>,
}

impl GeometryMetadata {
    pub fn new() -> Self {
        Self {
            metadata: Metadata::new(),
            att_metadatas: Vec::new(),
        }
    }

    pub fn from_metadata(metadata: Metadata) -> Self {
        Self {
            metadata,
            att_metadatas: Vec::new(),
        }
    }

    pub fn get_attribute_metadata_by_string_entry<N: AsRef<[u8]>, V: AsRef<[u8]>>(
        &self,
        entry_name: N,
        entry_value: V,
    ) -> Option<&AttributeMetadata> {
        for att_metadata in &self.att_metadatas {
            let mut value = MetadataString::default();
            if !att_metadata.get_entry_string(entry_name.as_ref(), &mut value) {
                continue;
            }
            if value.as_bytes() == entry_value.as_ref() {
                return Some(att_metadata.as_ref());
            }
        }
        None
    }

    pub fn add_attribute_metadata(&mut self, att_metadata: Option<Box<AttributeMetadata>>) -> bool {
        let att_metadata = match att_metadata {
            Some(metadata) => metadata,
            None => return false,
        };
        self.att_metadatas.push(att_metadata);
        true
    }

    pub fn delete_attribute_metadata_by_unique_id(&mut self, att_unique_id: i32) {
        if att_unique_id < 0 {
            return;
        }
        let att_unique_id = att_unique_id as u32;
        if let Some(pos) = self
            .att_metadatas
            .iter()
            .position(|m| m.att_unique_id == att_unique_id)
        {
            self.att_metadatas.remove(pos);
        }
    }

    pub fn get_attribute_metadata_by_unique_id(
        &self,
        att_unique_id: i32,
    ) -> Option<&AttributeMetadata> {
        if att_unique_id < 0 {
            return None;
        }
        let att_unique_id = att_unique_id as u32;
        for att_metadata in &self.att_metadatas {
            if att_metadata.att_unique_id == att_unique_id {
                return Some(att_metadata.as_ref());
            }
        }
        None
    }

    pub fn attribute_metadata(&mut self, att_unique_id: i32) -> Option<&mut AttributeMetadata> {
        if att_unique_id < 0 {
            return None;
        }
        let att_unique_id = att_unique_id as u32;
        for att_metadata in &mut self.att_metadatas {
            if att_metadata.att_unique_id == att_unique_id {
                return Some(att_metadata.as_mut());
            }
        }
        None
    }

    pub fn attribute_metadatas(&self) -> &Vec<Box<AttributeMetadata>> {
        &self.att_metadatas
    }
}

impl Deref for GeometryMetadata {
    type Target = Metadata;

    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}

impl DerefMut for GeometryMetadata {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.metadata
    }
}

pub struct GeometryMetadataHasher;

impl GeometryMetadataHasher {
    pub fn hash(&self, metadata: &GeometryMetadata) -> u64 {
        let mut hash = metadata.att_metadatas.len() as u64;
        let att_metadata_hasher = AttributeMetadataHasher;
        for att_metadata in &metadata.att_metadatas {
            let att_hash = att_metadata_hasher.hash(att_metadata);
            hash = hash_combine_with(&att_hash, hash);
        }
        let metadata_hasher = MetadataHasher;
        let base_hash = metadata_hasher.hash(&metadata.metadata);
        hash = hash_combine_with(&base_hash, hash);
        hash
    }
}
