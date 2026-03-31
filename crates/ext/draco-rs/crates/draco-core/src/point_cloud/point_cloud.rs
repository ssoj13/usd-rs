//! Point cloud implementation.
//! Reference: `_ref/draco/src/draco/point_cloud/point_cloud.h` + `.cc`.

use std::cmp::max;
use std::collections::HashMap;

use crate::attributes::geometry_attribute::{GeometryAttribute, GeometryAttributeType};
use crate::attributes::geometry_indices::{AttributeValueIndex, PointIndex};
use crate::attributes::point_attribute::{PointAttribute, PointAttributeHasher};
use crate::core::bounding_box::BoundingBox;
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::hash_utils::hash_combine_with;
use crate::core::vector_d::Vector3f;
use crate::metadata::geometry_metadata::{
    AttributeMetadata, GeometryMetadata, GeometryMetadataHasher,
};

const NAMED_ATTRIBUTES_COUNT: usize = GeometryAttributeType::NamedAttributesCount as usize;

pub struct PointCloud {
    metadata: Option<GeometryMetadata>,
    // Option is needed because set_attribute() with arbitrary att_id can create
    // gaps via resize_with(). C++ uses vector<unique_ptr> where gaps are nullptr.
    attributes: Vec<Option<PointAttribute>>,
    named_attribute_index: [Vec<i32>; NAMED_ATTRIBUTES_COUNT],
    num_points: u32,
    compression_enabled: bool,
    compression_options: crate::compression::DracoCompressionOptions,
}

impl PointCloud {
    pub fn new() -> Self {
        Self {
            metadata: None,
            attributes: Vec::new(),
            named_attribute_index: std::array::from_fn(|_| Vec::new()),
            num_points: 0,
            compression_enabled: false,
            compression_options: crate::compression::DracoCompressionOptions::default(),
        }
    }

    pub fn copy(&mut self, src: &PointCloud) {
        self.num_points = src.num_points;
        for i in 0..NAMED_ATTRIBUTES_COUNT {
            self.named_attribute_index[i] = src.named_attribute_index[i].clone();
        }
        self.attributes.resize_with(src.attributes.len(), || None);
        for i in 0..src.attributes.len() {
            if let Some(src_att) = &src.attributes[i] {
                let mut att = PointAttribute::new();
                att.copy_from(src_att);
                self.attributes[i] = Some(att);
            } else {
                self.attributes[i] = None;
            }
        }
        self.compression_enabled = src.compression_enabled;
        self.compression_options = src.compression_options.clone();
        self.copy_metadata(src);
    }

    pub fn num_named_attributes(&self, attr_type: GeometryAttributeType) -> i32 {
        if attr_type == GeometryAttributeType::Invalid
            || (attr_type as i32) >= GeometryAttributeType::NamedAttributesCount as i32
        {
            return 0;
        }
        self.named_attribute_index[attr_type as usize].len() as i32
    }

    pub fn get_named_attribute_id(&self, attr_type: GeometryAttributeType) -> i32 {
        self.get_named_attribute_id_by_index(attr_type, 0)
    }

    pub fn get_named_attribute_id_by_index(&self, attr_type: GeometryAttributeType, i: i32) -> i32 {
        if self.num_named_attributes(attr_type) <= i {
            return -1;
        }
        self.named_attribute_index[attr_type as usize][i as usize]
    }

    pub fn get_named_attribute(&self, attr_type: GeometryAttributeType) -> Option<&PointAttribute> {
        self.get_named_attribute_by_index(attr_type, 0)
    }

    pub fn get_named_attribute_by_index(
        &self,
        attr_type: GeometryAttributeType,
        i: i32,
    ) -> Option<&PointAttribute> {
        let att_id = self.get_named_attribute_id_by_index(attr_type, i);
        if att_id == -1 {
            return None;
        }
        self.attribute(att_id)
    }

    pub fn get_named_attribute_by_unique_id(
        &self,
        attr_type: GeometryAttributeType,
        unique_id: u32,
    ) -> Option<&PointAttribute> {
        for att_id in &self.named_attribute_index[attr_type as usize] {
            if let Some(att) = self.attributes[*att_id as usize].as_ref() {
                if att.unique_id() == unique_id {
                    return Some(att);
                }
            }
        }
        None
    }

    pub fn get_attribute_by_unique_id(&self, unique_id: u32) -> Option<&PointAttribute> {
        let att_id = self.get_attribute_id_by_unique_id(unique_id);
        if att_id == -1 {
            return None;
        }
        self.attribute(att_id)
    }

    pub fn get_named_attribute_by_name(
        &self,
        attr_type: GeometryAttributeType,
        name: &str,
    ) -> Option<&PointAttribute> {
        for att_id in &self.named_attribute_index[attr_type as usize] {
            if let Some(att) = self.attributes[*att_id as usize].as_ref() {
                if att.name() == name {
                    return Some(att);
                }
            }
        }
        None
    }

    pub fn get_attribute_id_by_unique_id(&self, unique_id: u32) -> i32 {
        for (att_id, att_opt) in self.attributes.iter().enumerate() {
            if let Some(att) = att_opt {
                if att.unique_id() == unique_id {
                    return att_id as i32;
                }
            }
        }
        -1
    }

    pub fn num_attributes(&self) -> i32 {
        self.attributes.len() as i32
    }

    pub fn attribute(&self, att_id: i32) -> Option<&PointAttribute> {
        if att_id < 0 || att_id as usize >= self.attributes.len() {
            return None;
        }
        self.attributes[att_id as usize].as_ref()
    }

    pub fn attribute_mut(&mut self, att_id: i32) -> Option<&mut PointAttribute> {
        if att_id < 0 || att_id as usize >= self.attributes.len() {
            return None;
        }
        self.attributes[att_id as usize].as_mut()
    }

    pub fn add_attribute(&mut self, pa: PointAttribute) -> i32 {
        let att_id = self.attributes.len() as i32;
        self.set_attribute(att_id, pa);
        (self.attributes.len() as i32) - 1
    }

    pub fn add_attribute_from_geometry(
        &mut self,
        att: &GeometryAttribute,
        identity_mapping: bool,
        num_attribute_values: u32,
    ) -> i32 {
        let pa = self.create_attribute(att, identity_mapping, num_attribute_values);
        match pa {
            Some(pa) => self.add_attribute(pa),
            None => -1,
        }
    }

    pub fn create_attribute(
        &self,
        att: &GeometryAttribute,
        identity_mapping: bool,
        num_attribute_values: u32,
    ) -> Option<PointAttribute> {
        if att.attribute_type() == GeometryAttributeType::Invalid {
            return None;
        }
        let mut pa = PointAttribute::from_geometry_attribute(att.clone());
        if !identity_mapping {
            pa.set_explicit_mapping(self.num_points as usize);
        } else {
            pa.set_identity_mapping();
        }
        let mut num_values = num_attribute_values;
        if identity_mapping {
            num_values = max(self.num_points, num_values);
        }
        if num_values > 0 {
            let _ = pa.reset(num_values as usize);
        }
        Some(pa)
    }

    pub fn set_attribute(&mut self, att_id: i32, mut pa: PointAttribute) {
        if att_id < 0 {
            return;
        }
        if self.attributes.len() <= att_id as usize {
            self.attributes.resize_with(att_id as usize + 1, || None);
        }
        if (pa.attribute_type() as i32) < GeometryAttributeType::NamedAttributesCount as i32 {
            self.named_attribute_index[pa.attribute_type() as usize].push(att_id);
        }
        pa.set_unique_id(att_id as u32);
        self.attributes[att_id as usize] = Some(pa);
    }

    pub fn delete_attribute(&mut self, att_id: i32) {
        if att_id < 0 || att_id as usize >= self.attributes.len() {
            return;
        }
        let att = match self.attributes[att_id as usize].as_ref() {
            Some(att) => att,
            None => return,
        };
        let att_type = att.attribute_type();
        let unique_id = att.unique_id();

        self.attributes.remove(att_id as usize);

        if let Some(metadata) = &mut self.metadata {
            metadata.delete_attribute_metadata_by_unique_id(unique_id as i32);
        }

        if (att_type as i32) < GeometryAttributeType::NamedAttributesCount as i32 {
            let index = &mut self.named_attribute_index[att_type as usize];
            if let Some(pos) = index.iter().position(|id| *id == att_id) {
                index.remove(pos);
            }
        }

        for i in 0..NAMED_ATTRIBUTES_COUNT {
            for entry in &mut self.named_attribute_index[i] {
                if *entry > att_id {
                    *entry -= 1;
                }
            }
        }
    }

    pub fn deduplicate_attribute_values(&mut self) -> bool {
        if self.num_points == 0 {
            return true;
        }
        for att_id in 0..self.num_attributes() {
            let mut_att = match self.attribute_mut(att_id) {
                Some(att) => att,
                None => continue,
            };
            if mut_att.deduplicate_values() < 0 {
                return false;
            }
        }
        true
    }

    pub fn deduplicate_point_ids(&mut self) {
        let (num_unique_points, index_map, unique_points) =
            build_point_deduplication_map(self.num_points, |point| self.point_signature(point));
        if num_unique_points == self.num_points {
            return;
        }

        self.apply_point_id_deduplication(&index_map, &unique_points);
        self.set_num_points(num_unique_points);
    }

    pub fn compute_bounding_box(&self) -> BoundingBox {
        let mut bounding_box = BoundingBox::default();
        let pc_att = self.get_named_attribute(GeometryAttributeType::Position);
        if pc_att.is_none() {
            return bounding_box;
        }
        let pc_att = pc_att.unwrap();
        for i in 0..pc_att.size() {
            let att_index = AttributeValueIndex::from(i as u32);
            let vals = pc_att.get_value_array::<f32, 3>(att_index);
            let p = Vector3f::new3(vals[0], vals[1], vals[2]);
            bounding_box.update_point(&p);
        }
        bounding_box
    }

    pub fn add_metadata(&mut self, metadata: GeometryMetadata) {
        self.metadata = Some(metadata);
    }

    pub fn add_attribute_metadata(&mut self, att_id: i32, mut metadata: AttributeMetadata) {
        if self.metadata.is_none() {
            self.metadata = Some(GeometryMetadata::new());
        }
        let att_unique_id = self
            .attribute(att_id)
            .expect("Invalid attribute id")
            .unique_id();
        metadata.set_att_unique_id(att_unique_id);
        if let Some(meta) = &mut self.metadata {
            let _ = meta.add_attribute_metadata(Some(Box::new(metadata)));
        }
    }

    pub fn get_attribute_metadata_by_attribute_id(
        &self,
        att_id: i32,
    ) -> Option<&AttributeMetadata> {
        let metadata = self.metadata.as_ref()?;
        let unique_id = self.attribute(att_id)?.unique_id();
        metadata.get_attribute_metadata_by_unique_id(unique_id as i32)
    }

    pub fn get_attribute_metadata_by_string_entry(
        &self,
        name: &str,
        value: &str,
    ) -> Option<&AttributeMetadata> {
        let metadata = self.metadata.as_ref()?;
        metadata.get_attribute_metadata_by_string_entry(name, value)
    }

    pub fn get_attribute_id_by_metadata_entry(&self, name: &str, value: &str) -> i32 {
        let metadata = match self.metadata.as_ref() {
            Some(metadata) => metadata,
            None => return -1,
        };
        let att_metadata = metadata.get_attribute_metadata_by_string_entry(name, value);
        let att_metadata = match att_metadata {
            Some(att_metadata) => att_metadata,
            None => return -1,
        };
        self.get_attribute_id_by_unique_id(att_metadata.att_unique_id())
    }

    pub fn get_metadata(&self) -> Option<&GeometryMetadata> {
        self.metadata.as_ref()
    }

    pub fn metadata_mut(&mut self) -> Option<&mut GeometryMetadata> {
        self.metadata.as_mut()
    }

    pub fn num_points(&self) -> u32 {
        self.num_points
    }

    pub fn set_num_points(&mut self, num: u32) {
        self.num_points = num;
    }

    pub fn set_compression_enabled(&mut self, enabled: bool) {
        self.compression_enabled = enabled;
    }

    pub fn is_compression_enabled(&self) -> bool {
        self.compression_enabled
    }

    pub fn set_compression_options(
        &mut self,
        options: crate::compression::DracoCompressionOptions,
    ) {
        self.compression_options = options;
    }

    pub fn compression_options(&self) -> &crate::compression::DracoCompressionOptions {
        &self.compression_options
    }

    pub fn compression_options_mut(&mut self) -> &mut crate::compression::DracoCompressionOptions {
        &mut self.compression_options
    }

    fn copy_metadata(&mut self, src: &PointCloud) {
        if let Some(metadata) = &src.metadata {
            self.metadata = Some(metadata.clone());
        } else {
            self.metadata = None;
        }
    }

    fn apply_point_id_deduplication(
        &mut self,
        id_map: &crate::core::draco_index_type_vector::IndexTypeVector<PointIndex, PointIndex>,
        unique_point_ids: &[PointIndex],
    ) {
        let mut num_unique_points = 0u32;
        for &point_id in unique_point_ids {
            let new_point_id = id_map[point_id];
            if new_point_id.value() >= num_unique_points {
                for a in 0..self.num_attributes() {
                    if let Some(att) = self.attribute_mut(a) {
                        let mapped = att.mapped_index(point_id);
                        att.set_point_map_entry(new_point_id, mapped);
                    }
                }
                num_unique_points = new_point_id.value() + 1;
            }
        }
        for a in 0..self.num_attributes() {
            if let Some(att) = self.attribute_mut(a) {
                att.set_explicit_mapping(num_unique_points as usize);
            }
        }
    }

    pub(crate) fn point_signature(&self, point: PointIndex) -> Vec<u32> {
        let mut signature = Vec::with_capacity(self.num_attributes() as usize);
        for i in 0..self.num_attributes() {
            let att = self.attribute(i).unwrap();
            signature.push(att.mapped_index(point).value());
        }
        signature
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn build_point_deduplication_map(
    num_points: u32,
    mut point_signature: impl FnMut(PointIndex) -> Vec<u32>,
) -> (
    u32,
    IndexTypeVector<PointIndex, PointIndex>,
    Vec<PointIndex>,
) {
    let mut unique_point_map: HashMap<Vec<u32>, PointIndex> =
        HashMap::with_capacity(num_points as usize);
    let mut num_unique_points = 0u32;
    let mut index_map = IndexTypeVector::<PointIndex, PointIndex>::with_size_value(
        num_points as usize,
        0u32.into(),
    );
    let mut unique_points: Vec<PointIndex> = Vec::new();

    for i in 0..num_points {
        let point = PointIndex::from(i);
        let signature = point_signature(point);
        if let Some(&existing) = unique_point_map.get(&signature) {
            index_map[point] = index_map[existing];
            continue;
        }

        let new_id = PointIndex::from(num_unique_points);
        index_map[point] = new_id;
        unique_points.push(point);
        unique_point_map.insert(signature, point);
        num_unique_points += 1;
    }

    (num_unique_points, index_map, unique_points)
}

pub struct PointCloudHasher;

impl PointCloudHasher {
    pub fn hash(&self, pc: &PointCloud) -> u64 {
        let mut hash = pc.num_points as u64;
        hash = hash_combine_with(&(pc.attributes.len() as u64), hash);
        for i in 0..NAMED_ATTRIBUTES_COUNT {
            hash = hash_combine_with(&(pc.named_attribute_index[i].len() as u64), hash);
            for j in 0..pc.named_attribute_index[i].len() {
                hash = hash_combine_with(&(pc.named_attribute_index[i][j] as u64), hash);
            }
        }
        for att in &pc.attributes {
            if let Some(att) = att {
                let att_hasher = PointAttributeHasher;
                let att_hash = att_hasher.hash(att);
                hash = hash_combine_with(&att_hash, hash);
            }
        }
        if let Some(metadata) = &pc.metadata {
            let metadata_hasher = GeometryMetadataHasher;
            let meta_hash = metadata_hasher.hash(metadata);
            hash = hash_combine_with(&meta_hash, hash);
        }
        hash
    }
}
