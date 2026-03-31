//! Point attribute utilities.
//! Reference: `_ref/draco/src/draco/attributes/point_attribute.h` + `.cc`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use crate::attributes::attribute_transform_data::AttributeTransformData;
use crate::attributes::geometry_attribute::{
    GeometryAttribute, GeometryAttributeHasher, GeometryAttributeType,
};
use crate::attributes::geometry_indices::{
    AttributeValueIndex, PointIndex, INVALID_ATTRIBUTE_VALUE_INDEX,
};
use crate::core::data_buffer::DataBuffer;
use crate::core::draco_index_type_vector::IndexTypeVector;
use crate::core::draco_types::{data_type_length, DataType};
use crate::core::hash_utils::{fingerprint_string, hash_combine_with, HashArray};
use crate::draco_dcheck;

#[derive(Debug)]
pub struct PointAttribute {
    geometry_attribute: GeometryAttribute,
    attribute_buffer: Option<Rc<RefCell<DataBuffer>>>,
    indices_map: IndexTypeVector<PointIndex, AttributeValueIndex>,
    num_unique_entries: u32,
    identity_mapping: bool,
    attribute_transform_data: Option<AttributeTransformData>,
}

impl PointAttribute {
    pub fn new() -> Self {
        Self {
            geometry_attribute: GeometryAttribute::new(),
            attribute_buffer: None,
            indices_map: IndexTypeVector::new(),
            num_unique_entries: 0,
            identity_mapping: false,
            attribute_transform_data: None,
        }
    }

    pub fn from_geometry_attribute(att: GeometryAttribute) -> Self {
        Self {
            geometry_attribute: att,
            attribute_buffer: None,
            indices_map: IndexTypeVector::new(),
            num_unique_entries: 0,
            identity_mapping: false,
            attribute_transform_data: None,
        }
    }

    pub fn init(
        &mut self,
        attribute_type: GeometryAttributeType,
        num_components: i8,
        data_type: DataType,
        normalized: bool,
        num_attribute_values: usize,
    ) {
        let buffer = Rc::new(RefCell::new(DataBuffer::new()));
        self.attribute_buffer = Some(buffer.clone());
        let stride = (data_type_length(data_type) as i64) * (num_components as i64);
        self.geometry_attribute.init(
            attribute_type,
            Some(buffer),
            num_components as u8,
            data_type,
            normalized,
            stride,
            0,
        );
        let _ = self.reset(num_attribute_values);
        self.set_identity_mapping();
    }

    pub fn copy_from(&mut self, src: &PointAttribute) {
        if self.geometry_attribute.buffer().is_none() {
            let buffer = Rc::new(RefCell::new(DataBuffer::new()));
            self.attribute_buffer = Some(buffer.clone());
            self.geometry_attribute.reset_buffer(buffer, 0, 0);
        }
        if !self.geometry_attribute.copy_from(&src.geometry_attribute) {
            return;
        }
        self.identity_mapping = src.identity_mapping;
        self.num_unique_entries = src.num_unique_entries;
        self.indices_map = src.indices_map.clone();
        if let Some(data) = &src.attribute_transform_data {
            self.attribute_transform_data = Some(data.clone());
        } else {
            self.attribute_transform_data = None;
        }
    }

    pub fn reset(&mut self, num_attribute_values: usize) -> bool {
        if self.attribute_buffer.is_none() {
            self.attribute_buffer = Some(Rc::new(RefCell::new(DataBuffer::new())));
        }
        let entry_size =
            (data_type_length(self.data_type()) as i64) * (self.num_components() as i64);
        let size = (num_attribute_values as i64) * entry_size;
        let mut needs_new_buffer = false;
        if let Some(buf) = self.attribute_buffer.as_ref() {
            match buf.try_borrow_mut() {
                Ok(mut buf_ref) => {
                    if !buf_ref.update(None, size) {
                        return false;
                    }
                    drop(buf_ref);
                    self.geometry_attribute
                        .reset_buffer(buf.clone(), entry_size, 0);
                }
                Err(_) => {
                    needs_new_buffer = true;
                }
            }
        }
        if needs_new_buffer {
            // If the buffer is temporarily borrowed, replace it with a fresh buffer.
            // This mirrors a reset semantics and avoids RefCell panics during decoding.
            let new_buf = Rc::new(RefCell::new(DataBuffer::new()));
            if !new_buf.borrow_mut().update(None, size) {
                return false;
            }
            self.attribute_buffer = Some(new_buf.clone());
            self.geometry_attribute.reset_buffer(new_buf, entry_size, 0);
        }
        self.num_unique_entries = num_attribute_values as u32;
        true
    }

    pub fn size(&self) -> usize {
        self.num_unique_entries as usize
    }

    pub fn mapped_index(&self, point_index: PointIndex) -> AttributeValueIndex {
        if self.identity_mapping {
            return AttributeValueIndex::from(point_index.value());
        }
        let idx = point_index.value() as usize;
        if idx >= self.indices_map.size() {
            return INVALID_ATTRIBUTE_VALUE_INDEX;
        }
        self.indices_map[point_index]
    }

    pub fn buffer(&self) -> Option<Rc<RefCell<DataBuffer>>> {
        self.attribute_buffer.clone()
    }

    pub fn is_mapping_identity(&self) -> bool {
        self.identity_mapping
    }

    pub fn indices_map_size(&self) -> usize {
        if self.is_mapping_identity() {
            0
        } else {
            self.indices_map.size()
        }
    }

    pub fn get_value_bytes_by_point(&self, point_index: PointIndex, out_data: &mut [u8]) {
        self.get_value_bytes(self.mapped_index(point_index), out_data);
    }

    pub fn resize(&mut self, new_num_unique_entries: usize) {
        self.num_unique_entries = new_num_unique_entries as u32;
        if let Some(buf) = &self.attribute_buffer {
            let new_size = (new_num_unique_entries as i64) * self.byte_stride();
            buf.borrow_mut().resize(new_size);
        }
    }

    pub fn set_identity_mapping(&mut self) {
        self.identity_mapping = true;
        self.indices_map.clear();
    }

    pub fn set_explicit_mapping(&mut self, num_points: usize) {
        self.identity_mapping = false;
        if self.indices_map.size() == 0 {
            self.indices_map =
                IndexTypeVector::with_size_value(num_points, INVALID_ATTRIBUTE_VALUE_INDEX);
        } else {
            self.indices_map
                .resize_with_value(num_points, INVALID_ATTRIBUTE_VALUE_INDEX);
        }
    }

    pub fn set_point_map_entry(
        &mut self,
        point_index: PointIndex,
        entry_index: AttributeValueIndex,
    ) {
        draco_dcheck!(!self.identity_mapping);
        self.indices_map[point_index] = entry_index;
    }

    pub fn get_mapped_value(&self, point_index: PointIndex, out_data: &mut [u8]) {
        self.get_value_bytes(self.mapped_index(point_index), out_data);
    }

    pub fn set_attribute_transform_data(&mut self, transform_data: Option<AttributeTransformData>) {
        self.attribute_transform_data = transform_data;
    }

    pub fn get_attribute_transform_data(&self) -> Option<&AttributeTransformData> {
        self.attribute_transform_data.as_ref()
    }

    // GeometryAttribute passthroughs
    pub fn geometry_attribute(&self) -> &GeometryAttribute {
        &self.geometry_attribute
    }

    pub fn set_normalized(&mut self, normalized: bool) {
        self.geometry_attribute.set_normalized(normalized);
    }

    pub fn attribute_type(&self) -> GeometryAttributeType {
        self.geometry_attribute.attribute_type()
    }

    pub fn name(&self) -> &str {
        self.geometry_attribute.name()
    }

    pub fn set_name(&mut self, name: &str) {
        self.geometry_attribute.set_name(name);
    }

    pub fn set_attribute_type(&mut self, t: GeometryAttributeType) {
        self.geometry_attribute.set_attribute_type(t);
    }

    pub fn data_type(&self) -> DataType {
        self.geometry_attribute.data_type()
    }

    pub fn num_components(&self) -> u8 {
        self.geometry_attribute.num_components()
    }

    pub fn normalized(&self) -> bool {
        self.geometry_attribute.normalized()
    }

    pub fn byte_stride(&self) -> i64 {
        self.geometry_attribute.byte_stride()
    }

    pub fn byte_offset(&self) -> i64 {
        self.geometry_attribute.byte_offset()
    }

    pub fn buffer_descriptor(&self) -> crate::core::data_buffer::DataBufferDescriptor {
        self.geometry_attribute.buffer_descriptor()
    }

    pub fn unique_id(&self) -> u32 {
        self.geometry_attribute.unique_id()
    }

    pub fn set_unique_id(&mut self, id: u32) {
        self.geometry_attribute.set_unique_id(id);
    }

    pub fn get_value_bytes(&self, att_index: AttributeValueIndex, out_data: &mut [u8]) {
        self.geometry_attribute.get_value_bytes(att_index, out_data);
    }

    pub fn get_value_array<T: Copy + Default, const N: usize>(
        &self,
        att_index: AttributeValueIndex,
    ) -> [T; N] {
        self.geometry_attribute.get_value_array(att_index)
    }

    pub fn get_value_array_into<T: Copy + Default, const N: usize>(
        &self,
        att_index: AttributeValueIndex,
        out: &mut [T; N],
    ) -> bool {
        self.geometry_attribute.get_value_array_into(att_index, out)
    }

    pub fn set_attribute_value_bytes(&self, entry_index: AttributeValueIndex, bytes: &[u8]) {
        self.geometry_attribute
            .set_attribute_value(entry_index, bytes);
    }

    pub fn set_attribute_value<T: Copy>(&self, entry_index: AttributeValueIndex, value: &T) {
        let bytes = unsafe {
            std::slice::from_raw_parts(value as *const T as *const u8, std::mem::size_of::<T>())
        };
        self.set_attribute_value_bytes(entry_index, bytes);
    }

    pub fn set_attribute_value_array<T: Copy, const N: usize>(
        &self,
        entry_index: AttributeValueIndex,
        value: &[T; N],
    ) {
        let bytes = unsafe {
            std::slice::from_raw_parts(value.as_ptr() as *const u8, std::mem::size_of::<T>() * N)
        };
        self.geometry_attribute
            .set_attribute_value(entry_index, bytes);
    }

    pub fn convert_value<OutT: crate::attributes::draco_numeric::DracoNumeric + Default>(
        &self,
        att_index: AttributeValueIndex,
        out_num_components: i8,
        out_val: &mut [OutT],
    ) -> bool {
        self.geometry_attribute
            .convert_value(att_index, out_num_components, out_val)
    }

    /// Converts attribute value using internal num_components. C++ parity: ConvertValue(att_index, out_value).
    pub fn convert_value_all<OutT: crate::attributes::draco_numeric::DracoNumeric + Default>(
        &self,
        att_index: AttributeValueIndex,
        out_val: &mut [OutT],
    ) -> bool {
        self.geometry_attribute
            .convert_value_all(att_index, out_val)
    }

    /// Converts input values from InT to the attribute's stored type and writes.
    /// C++ parity: ConvertAndSetAttributeValue (transcoder support).
    pub fn convert_and_set_value<InT: crate::attributes::draco_numeric::DracoNumeric + Default>(
        &self,
        entry_index: AttributeValueIndex,
        in_values: &[InT],
    ) -> bool {
        self.geometry_attribute
            .convert_and_set_value(entry_index, in_values)
    }

    /// Deduplicate attribute values reading from self. C++ parity: DeduplicateValues(*this).
    pub fn deduplicate_values(&mut self) -> i32 {
        self.deduplicate_values_with_offset(AttributeValueIndex::from(0u32))
    }

    /// Deduplicate values reading from an external attribute. C++ parity:
    /// DeduplicateValues(const GeometryAttribute &in_att).
    pub fn deduplicate_values_with_att(&mut self, in_att: &GeometryAttribute) -> i32 {
        self.deduplicate_values_with_att_offset(in_att, AttributeValueIndex::from(0u32))
    }

    /// Deduplicate values reading from an external attribute with offset. C++ parity:
    /// DeduplicateValues(const GeometryAttribute &in_att, AttributeValueIndex in_att_offset).
    pub fn deduplicate_values_with_att_offset(
        &mut self,
        in_att: &GeometryAttribute,
        in_att_offset: AttributeValueIndex,
    ) -> i32 {
        let unique_vals = match in_att.data_type() {
            DataType::Float32 => self.deduplicate_typed_values_ext::<f32>(in_att, in_att_offset),
            DataType::Int8 => self.deduplicate_typed_values_ext::<i8>(in_att, in_att_offset),
            DataType::Uint8 | DataType::Bool => {
                self.deduplicate_typed_values_ext::<u8>(in_att, in_att_offset)
            }
            DataType::Uint16 => self.deduplicate_typed_values_ext::<u16>(in_att, in_att_offset),
            DataType::Int16 => self.deduplicate_typed_values_ext::<i16>(in_att, in_att_offset),
            DataType::Uint32 => self.deduplicate_typed_values_ext::<u32>(in_att, in_att_offset),
            DataType::Int32 => self.deduplicate_typed_values_ext::<i32>(in_att, in_att_offset),
            _ => 0,
        };
        if unique_vals == 0 {
            return -1;
        }
        unique_vals as i32
    }

    fn deduplicate_values_with_offset(&mut self, in_att_offset: AttributeValueIndex) -> i32 {
        let unique_vals = match self.geometry_attribute.data_type() {
            DataType::Float32 => self.deduplicate_typed_values::<f32>(in_att_offset),
            DataType::Int8 => self.deduplicate_typed_values::<i8>(in_att_offset),
            DataType::Uint8 | DataType::Bool => self.deduplicate_typed_values::<u8>(in_att_offset),
            DataType::Uint16 => self.deduplicate_typed_values::<u16>(in_att_offset),
            DataType::Int16 => self.deduplicate_typed_values::<i16>(in_att_offset),
            DataType::Uint32 => self.deduplicate_typed_values::<u32>(in_att_offset),
            DataType::Int32 => self.deduplicate_typed_values::<i32>(in_att_offset),
            _ => 0,
        };
        if unique_vals == 0 {
            return -1;
        }
        unique_vals as i32
    }

    /// Removes attribute values that are not referenced by any point mapping.
    pub fn remove_unused_values(&mut self) {
        if self.is_mapping_identity() {
            return;
        }
        let mut is_value_used =
            IndexTypeVector::<AttributeValueIndex, bool>::with_size_value(self.size(), false);
        let mut num_used_values = 0usize;
        for pi in 0..self.indices_map.size() {
            let avi = self.indices_map[PointIndex::from(pi as u32)];
            if !is_value_used[avi] {
                is_value_used[avi] = true;
                num_used_values += 1;
            }
        }
        if num_used_values == self.size() {
            return;
        }

        let mut old_to_new_value_map =
            IndexTypeVector::<AttributeValueIndex, AttributeValueIndex>::with_size_value(
                self.size(),
                INVALID_ATTRIBUTE_VALUE_INDEX,
            );
        let mut new_avi = 0u32;
        for avi in 0..self.size() {
            let avi_index = AttributeValueIndex::from(avi as u32);
            if !is_value_used[avi_index] {
                continue;
            }
            let new_index = AttributeValueIndex::from(new_avi);
            if avi_index != new_index {
                let mut bytes = vec![0u8; self.byte_stride() as usize];
                self.get_value_bytes(avi_index, &mut bytes);
                self.set_attribute_value_bytes(new_index, &bytes);
            }
            old_to_new_value_map[avi_index] = new_index;
            new_avi += 1;
        }

        for pi in 0..self.indices_map.size() {
            let point_index = PointIndex::from(pi as u32);
            let avi = self.indices_map[point_index];
            self.indices_map[point_index] = old_to_new_value_map[avi];
        }

        self.num_unique_entries = num_used_values as u32;
    }

    fn deduplicate_typed_values<T: HashableComponent + Copy + Default>(
        &mut self,
        in_att_offset: AttributeValueIndex,
    ) -> u32 {
        match self.geometry_attribute.num_components() {
            1 => self.deduplicate_formatted_values::<T, 1>(in_att_offset),
            2 => self.deduplicate_formatted_values::<T, 2>(in_att_offset),
            3 => self.deduplicate_formatted_values::<T, 3>(in_att_offset),
            4 => self.deduplicate_formatted_values::<T, 4>(in_att_offset),
            _ => 0,
        }
    }

    /// Typed dispatch for cross-attribute deduplication, keyed on in_att.num_components().
    fn deduplicate_typed_values_ext<T: HashableComponent + Copy + Default>(
        &mut self,
        in_att: &GeometryAttribute,
        in_att_offset: AttributeValueIndex,
    ) -> u32 {
        match in_att.num_components() {
            1 => self.deduplicate_formatted_values_ext::<T, 1>(in_att, in_att_offset),
            2 => self.deduplicate_formatted_values_ext::<T, 2>(in_att, in_att_offset),
            3 => self.deduplicate_formatted_values_ext::<T, 3>(in_att, in_att_offset),
            4 => self.deduplicate_formatted_values_ext::<T, 4>(in_att, in_att_offset),
            _ => 0,
        }
    }

    /// Cross-attribute deduplication: reads values from in_att, writes unique values to self.
    fn deduplicate_formatted_values_ext<T: HashableComponent + Copy + Default, const N: usize>(
        &mut self,
        in_att: &GeometryAttribute,
        in_att_offset: AttributeValueIndex,
    ) -> u32 {
        let mut unique_vals = AttributeValueIndex::from(0u32);
        let mut value_to_index_map: HashMap<HashableValue<T::HashType, N>, AttributeValueIndex> =
            HashMap::new();
        let mut value_map =
            IndexTypeVector::<AttributeValueIndex, AttributeValueIndex>::with_size_value(
                self.num_unique_entries as usize,
                INVALID_ATTRIBUTE_VALUE_INDEX,
            );

        for i in 0..self.num_unique_entries {
            let att_pos = in_att_offset + AttributeValueIndex::from(i);
            let mut att_value = [T::default(); N];
            in_att.get_value_array_into::<T, N>(att_pos, &mut att_value);
            let hashable = HashableValue::from_value(&att_value);
            if let Some(existing) = value_to_index_map.get(&hashable) {
                value_map[AttributeValueIndex::from(i)] = *existing;
            } else {
                value_to_index_map.insert(hashable, unique_vals);
                self.set_attribute_value_array(unique_vals, &att_value);
                value_map[AttributeValueIndex::from(i)] = unique_vals;
                unique_vals += 1u32;
            }
        }

        if unique_vals.value() == self.num_unique_entries {
            return unique_vals.value();
        }

        if self.is_mapping_identity() {
            self.set_explicit_mapping(self.num_unique_entries as usize);
            for i in 0..self.num_unique_entries {
                let v = value_map[AttributeValueIndex::from(i)];
                self.set_point_map_entry(PointIndex::from(i), v);
            }
        } else {
            for pi in 0..self.indices_map.size() {
                let p = PointIndex::from(pi as u32);
                let old = self.indices_map[p];
                self.indices_map[p] = value_map[old];
            }
        }

        self.num_unique_entries = unique_vals.value();
        self.num_unique_entries
    }

    fn deduplicate_formatted_values<T: HashableComponent + Copy + Default, const N: usize>(
        &mut self,
        in_att_offset: AttributeValueIndex,
    ) -> u32 {
        let mut unique_vals = AttributeValueIndex::from(0u32);
        let mut value_to_index_map: HashMap<HashableValue<T::HashType, N>, AttributeValueIndex> =
            HashMap::new();
        let mut value_map =
            IndexTypeVector::<AttributeValueIndex, AttributeValueIndex>::with_size_value(
                self.num_unique_entries as usize,
                INVALID_ATTRIBUTE_VALUE_INDEX,
            );

        for i in 0..self.num_unique_entries {
            let att_pos = in_att_offset + AttributeValueIndex::from(i);
            let mut att_value = [T::default(); N];
            self.geometry_attribute
                .get_value_array_into::<T, N>(att_pos, &mut att_value);
            let hashable = HashableValue::from_value(&att_value);
            if let Some(existing) = value_to_index_map.get(&hashable) {
                value_map[AttributeValueIndex::from(i)] = *existing;
            } else {
                value_to_index_map.insert(hashable, unique_vals);
                self.set_attribute_value_array(unique_vals, &att_value);
                value_map[AttributeValueIndex::from(i)] = unique_vals;
                unique_vals += 1u32;
            }
        }

        if unique_vals.value() == self.num_unique_entries {
            return unique_vals.value();
        }

        if self.is_mapping_identity() {
            self.set_explicit_mapping(self.num_unique_entries as usize);
            for i in 0..self.num_unique_entries {
                let v = value_map[AttributeValueIndex::from(i)];
                self.set_point_map_entry(PointIndex::from(i), v);
            }
        } else {
            for pi in 0..self.indices_map.size() {
                let p = PointIndex::from(pi as u32);
                let old = self.indices_map[p];
                self.indices_map[p] = value_map[old];
            }
        }

        self.num_unique_entries = unique_vals.value();
        self.num_unique_entries
    }
}

impl Default for PointAttribute {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PointAttributeHasher;

impl PointAttributeHasher {
    pub fn hash(&self, attribute: &PointAttribute) -> u64 {
        let base_hasher = GeometryAttributeHasher;
        let mut hash = base_hasher.hash(&attribute.geometry_attribute);
        hash = hash_combine_with(&attribute.identity_mapping, hash);
        hash = hash_combine_with(&attribute.num_unique_entries, hash);
        hash = hash_combine_with(&attribute.indices_map.size(), hash);
        if !attribute.indices_map.empty() {
            let data = attribute.indices_map.data();
            let len = attribute.indices_map.size();
            // NOTE: This passes element count instead of byte count to fingerprint_string.
            // This is a bug-for-bug match with C++ (point_attribute.h:181) to ensure
            // identical hash values. Fixing this would break hash compatibility.
            let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, len) };
            let indices_hash = fingerprint_string(bytes);
            hash = hash_combine_with(&indices_hash, hash);
        }
        if let Some(buf) = &attribute.attribute_buffer {
            let buf_ref = buf.borrow();
            let data = buf_ref.data();
            let buffer_hash = fingerprint_string(data);
            hash = hash_combine_with(&buffer_hash, hash);
        }
        hash
    }
}

trait HashableComponent {
    type HashType: Copy + Default + Eq + Hash + crate::core::hash_utils::CppHash;
    fn to_hash_type(self) -> Self::HashType;
}

impl HashableComponent for i8 {
    type HashType = u8;
    fn to_hash_type(self) -> Self::HashType {
        self as u8
    }
}
impl HashableComponent for u8 {
    type HashType = u8;
    fn to_hash_type(self) -> Self::HashType {
        self
    }
}
impl HashableComponent for i16 {
    type HashType = u16;
    fn to_hash_type(self) -> Self::HashType {
        self as u16
    }
}
impl HashableComponent for u16 {
    type HashType = u16;
    fn to_hash_type(self) -> Self::HashType {
        self
    }
}
impl HashableComponent for i32 {
    type HashType = u32;
    fn to_hash_type(self) -> Self::HashType {
        self as u32
    }
}
impl HashableComponent for u32 {
    type HashType = u32;
    fn to_hash_type(self) -> Self::HashType {
        self
    }
}
impl HashableComponent for i64 {
    type HashType = u64;
    fn to_hash_type(self) -> Self::HashType {
        self as u64
    }
}
impl HashableComponent for u64 {
    type HashType = u64;
    fn to_hash_type(self) -> Self::HashType {
        self
    }
}
impl HashableComponent for f32 {
    type HashType = u32;
    fn to_hash_type(self) -> Self::HashType {
        self.to_bits()
    }
}
impl HashableComponent for f64 {
    type HashType = u64;
    fn to_hash_type(self) -> Self::HashType {
        self.to_bits()
    }
}
impl HashableComponent for bool {
    type HashType = u8;
    fn to_hash_type(self) -> Self::HashType {
        if self {
            1
        } else {
            0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HashableValue<T: Copy + Default + Eq + Hash, const N: usize> {
    data: [T; N],
}

impl<T: Copy + Default + Eq + Hash, const N: usize> HashableValue<T, N> {
    fn from_value<U>(value: &[U; N]) -> Self
    where
        U: HashableComponent<HashType = T> + Copy,
    {
        let mut data = [T::default(); N];
        for i in 0..N {
            data[i] = value[i].to_hash_type();
        }
        Self { data }
    }
}

impl<T: Copy + Default + Eq + Hash + crate::core::hash_utils::CppHash, const N: usize> Hash
    for HashableValue<T, N>
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        let hash = HashArray::hash(&self.data);
        state.write_u64(hash);
    }
}
