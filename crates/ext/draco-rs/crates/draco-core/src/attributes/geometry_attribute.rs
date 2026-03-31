//! Geometry attribute utilities.
//! Reference: `_ref/draco/src/draco/attributes/geometry_attribute.h` + `.cc`.

use std::cell::RefCell;
use std::cmp::min;
use std::rc::Rc;

use crate::attributes::draco_numeric::DracoNumeric;
use crate::attributes::geometry_indices::AttributeValueIndex;
use crate::core::data_buffer::{DataBuffer, DataBufferDescriptor};
use crate::core::draco_types::DataType;
use crate::core::hash_utils::{hash_combine, hash_combine_with};

/// NOTE: Unlike C++ Draco without DRACO_TRANSCODER_SUPPORTED (which has 5 types:
/// POSITION, NORMAL, COLOR, TEX_COORD, GENERIC), we unconditionally include
/// TANGENT, MATERIAL, JOINTS, WEIGHTS for full transcoder support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum GeometryAttributeType {
    Invalid = -1,
    Position = 0,
    Normal = 1,
    Color = 2,
    TexCoord = 3,
    Generic = 4,
    Tangent = 5,
    Material = 6,
    Joints = 7,
    Weights = 8,
    NamedAttributesCount = 9,
}

impl GeometryAttributeType {
    /// All named attribute types (excluding Invalid and NamedAttributesCount).
    /// Mirrors C++ loop `for (int att_id = 0; att_id < NAMED_ATTRIBUTES_COUNT; ++att_id)`.
    pub const ALL_NAMED: [GeometryAttributeType; 9] = [
        GeometryAttributeType::Position,
        GeometryAttributeType::Normal,
        GeometryAttributeType::Color,
        GeometryAttributeType::TexCoord,
        GeometryAttributeType::Generic,
        GeometryAttributeType::Tangent,
        GeometryAttributeType::Material,
        GeometryAttributeType::Joints,
        GeometryAttributeType::Weights,
    ];
}

#[derive(Clone, Debug)]
pub struct GeometryAttribute {
    buffer: Option<Rc<RefCell<DataBuffer>>>,
    buffer_descriptor: DataBufferDescriptor,
    num_components: u8,
    data_type: DataType,
    normalized: bool,
    byte_stride: i64,
    byte_offset: i64,
    attribute_type: GeometryAttributeType,
    unique_id: u32,
    name: String,
}

impl GeometryAttribute {
    pub fn new() -> Self {
        Self {
            buffer: None,
            buffer_descriptor: DataBufferDescriptor::default(),
            num_components: 1,
            data_type: DataType::Float32,
            normalized: false,
            byte_stride: 0,
            byte_offset: 0,
            attribute_type: GeometryAttributeType::Invalid,
            unique_id: 0,
            name: String::new(),
        }
    }

    pub fn init(
        &mut self,
        attribute_type: GeometryAttributeType,
        buffer: Option<Rc<RefCell<DataBuffer>>>,
        num_components: u8,
        data_type: DataType,
        normalized: bool,
        byte_stride: i64,
        byte_offset: i64,
    ) {
        self.buffer = buffer;
        if let Some(buf) = &self.buffer {
            let buf_ref = buf.borrow();
            self.buffer_descriptor.buffer_id = buf_ref.buffer_id();
            self.buffer_descriptor.buffer_update_count = buf_ref.update_count();
        }
        self.num_components = num_components;
        self.data_type = data_type;
        self.normalized = normalized;
        self.byte_stride = byte_stride;
        self.byte_offset = byte_offset;
        self.attribute_type = attribute_type;
    }

    pub fn is_valid(&self) -> bool {
        self.buffer.is_some()
    }

    pub fn copy_from(&mut self, src: &GeometryAttribute) -> bool {
        self.num_components = src.num_components;
        self.data_type = src.data_type;
        self.normalized = src.normalized;
        self.byte_stride = src.byte_stride;
        self.byte_offset = src.byte_offset;
        self.attribute_type = src.attribute_type;
        self.buffer_descriptor = src.buffer_descriptor;
        self.unique_id = src.unique_id;
        self.name = src.name.clone();
        if src.buffer.is_none() {
            self.buffer = None;
        } else {
            if self.buffer.is_none() {
                return false;
            }
            let src_buf = src.buffer.as_ref().unwrap().borrow();
            let data = src_buf.data().to_vec();
            let size = data.len() as i64;
            self.buffer
                .as_ref()
                .unwrap()
                .borrow_mut()
                .update(Some(&data), size);
        }
        true
    }

    pub fn get_value_array<T: Copy + Default, const N: usize>(
        &self,
        att_index: AttributeValueIndex,
    ) -> [T; N] {
        let mut out: [T; N] = [T::default(); N];
        let _ = self.get_value_array_into(att_index, &mut out);
        out
    }

    pub fn get_value_array_into<T: Copy + Default, const N: usize>(
        &self,
        att_index: AttributeValueIndex,
        out: &mut [T; N],
    ) -> bool {
        let byte_pos = self.get_byte_pos(att_index);
        if let Some(buf) = &self.buffer {
            let buf_ref = buf.borrow();
            let total = std::mem::size_of::<T>() * N;
            if byte_pos < 0 || (byte_pos as usize + total) > buf_ref.data_size() {
                return false;
            }
            let mut tmp = vec![0u8; total];
            buf_ref.read(byte_pos, &mut tmp);
            unsafe {
                std::ptr::copy_nonoverlapping(tmp.as_ptr(), out.as_mut_ptr() as *mut u8, total);
            }
            return true;
        }
        false
    }

    pub fn get_byte_pos(&self, att_index: AttributeValueIndex) -> i64 {
        self.byte_offset + self.byte_stride * att_index.value() as i64
    }

    pub fn get_value_bytes(&self, att_index: AttributeValueIndex, out_data: &mut [u8]) {
        let byte_pos = self.get_byte_pos(att_index);
        if let Some(buf) = &self.buffer {
            let buf_ref = buf.borrow();
            buf_ref.read(byte_pos, out_data);
        }
    }

    pub fn set_attribute_value(&self, entry_index: AttributeValueIndex, value: &[u8]) {
        if let Some(buf) = &self.buffer {
            let byte_pos = entry_index.value() as i64 * self.byte_stride;
            let mut buf_ref = buf.borrow_mut();
            buf_ref.write(byte_pos, value);
        }
    }

    /// Converts input values from InT to the attribute's stored type and writes.
    /// C++ parity: ConvertAndSetAttributeValue (transcoder support).
    pub fn convert_and_set_value<InT: DracoNumeric + Default>(
        &self,
        entry_index: AttributeValueIndex,
        in_values: &[InT],
    ) -> bool {
        if in_values.is_empty() {
            return false;
        }
        let num_comp = self.num_components as usize;
        let count = min(num_comp, in_values.len());
        match self.data_type {
            DataType::Int8 => self.convert_and_set_typed::<InT, i8>(entry_index, in_values, count),
            DataType::Uint8 => self.convert_and_set_typed::<InT, u8>(entry_index, in_values, count),
            DataType::Int16 => {
                self.convert_and_set_typed::<InT, i16>(entry_index, in_values, count)
            }
            DataType::Uint16 => {
                self.convert_and_set_typed::<InT, u16>(entry_index, in_values, count)
            }
            DataType::Int32 => {
                self.convert_and_set_typed::<InT, i32>(entry_index, in_values, count)
            }
            DataType::Uint32 => {
                self.convert_and_set_typed::<InT, u32>(entry_index, in_values, count)
            }
            DataType::Int64 => {
                self.convert_and_set_typed::<InT, i64>(entry_index, in_values, count)
            }
            DataType::Uint64 => {
                self.convert_and_set_typed::<InT, u64>(entry_index, in_values, count)
            }
            DataType::Float32 => {
                self.convert_and_set_typed::<InT, f32>(entry_index, in_values, count)
            }
            DataType::Float64 => {
                self.convert_and_set_typed::<InT, f64>(entry_index, in_values, count)
            }
            DataType::Bool => {
                self.convert_and_set_typed::<InT, bool>(entry_index, in_values, count)
            }
            _ => false,
        }
    }

    fn convert_and_set_typed<InT: DracoNumeric + Default, OutT: DracoNumeric + Default>(
        &self,
        entry_index: AttributeValueIndex,
        in_values: &[InT],
        count: usize,
    ) -> bool {
        let comp_size = std::mem::size_of::<OutT>();
        let num_comp = self.num_components as usize;
        let mut out_buf = vec![0u8; num_comp * comp_size];
        for i in 0..count {
            if let Some(v) = convert_component_value::<InT, OutT>(in_values[i], self.normalized) {
                let bytes = unsafe {
                    std::slice::from_raw_parts(&v as *const OutT as *const u8, comp_size)
                };
                out_buf[i * comp_size..(i + 1) * comp_size].copy_from_slice(bytes);
            } else {
                return false;
            }
        }
        self.set_attribute_value(entry_index, &out_buf);
        true
    }

    /// C++ parity: ConvertValue(att_index, out_num_components, out_val).
    pub fn convert_value<OutT: DracoNumeric + Default>(
        &self,
        att_index: AttributeValueIndex,
        out_num_components: i8,
        out_val: &mut [OutT],
    ) -> bool {
        if out_val.is_empty() {
            return false;
        }
        match self.data_type {
            DataType::Int8 => {
                self.convert_typed_value::<i8, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Uint8 => {
                self.convert_typed_value::<u8, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Int16 => {
                self.convert_typed_value::<i16, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Uint16 => {
                self.convert_typed_value::<u16, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Int32 => {
                self.convert_typed_value::<i32, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Uint32 => {
                self.convert_typed_value::<u32, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Int64 => {
                self.convert_typed_value::<i64, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Uint64 => {
                self.convert_typed_value::<u64, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Float32 => {
                self.convert_typed_value::<f32, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Float64 => {
                self.convert_typed_value::<f64, OutT>(att_index, out_num_components, out_val)
            }
            DataType::Bool => {
                self.convert_typed_value::<bool, OutT>(att_index, out_num_components, out_val)
            }
            _ => false,
        }
    }

    /// Converts attribute value using internal num_components. C++ parity: ConvertValue(att_index, out_value).
    pub fn convert_value_all<OutT: DracoNumeric + Default>(
        &self,
        att_index: AttributeValueIndex,
        out_val: &mut [OutT],
    ) -> bool {
        let n = self.num_components() as i8;
        if out_val.len() < n as usize {
            return false;
        }
        self.convert_value(att_index, n, out_val)
    }

    pub fn type_to_string(attribute_type: GeometryAttributeType) -> &'static str {
        match attribute_type {
            GeometryAttributeType::Invalid => "INVALID",
            GeometryAttributeType::Position => "POSITION",
            GeometryAttributeType::Normal => "NORMAL",
            GeometryAttributeType::Color => "COLOR",
            GeometryAttributeType::TexCoord => "TEX_COORD",
            GeometryAttributeType::Generic => "GENERIC",
            GeometryAttributeType::Tangent => "TANGENT",
            GeometryAttributeType::Material => "MATERIAL",
            GeometryAttributeType::Joints => "JOINTS",
            GeometryAttributeType::Weights => "WEIGHTS",
            GeometryAttributeType::NamedAttributesCount => "UNKNOWN",
        }
    }

    pub fn attribute_type(&self) -> GeometryAttributeType {
        self.attribute_type
    }

    pub fn set_attribute_type(&mut self, t: GeometryAttributeType) {
        self.attribute_type = t;
    }

    pub fn data_type(&self) -> DataType {
        self.data_type
    }

    pub fn num_components(&self) -> u8 {
        self.num_components
    }

    pub fn normalized(&self) -> bool {
        self.normalized
    }

    pub fn set_normalized(&mut self, normalized: bool) {
        self.normalized = normalized;
    }

    pub fn buffer(&self) -> Option<Rc<RefCell<DataBuffer>>> {
        self.buffer.clone()
    }

    pub fn byte_stride(&self) -> i64 {
        self.byte_stride
    }

    pub fn byte_offset(&self) -> i64 {
        self.byte_offset
    }

    pub fn set_byte_offset(&mut self, byte_offset: i64) {
        self.byte_offset = byte_offset;
    }

    pub fn buffer_descriptor(&self) -> DataBufferDescriptor {
        self.buffer_descriptor
    }

    pub fn unique_id(&self) -> u32 {
        self.unique_id
    }

    pub fn set_unique_id(&mut self, id: u32) {
        self.unique_id = id;
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    pub(crate) fn reset_buffer(
        &mut self,
        buffer: Rc<RefCell<DataBuffer>>,
        byte_stride: i64,
        byte_offset: i64,
    ) {
        self.buffer = Some(buffer);
        if let Some(buf) = &self.buffer {
            let buf_ref = buf.borrow();
            self.buffer_descriptor.buffer_id = buf_ref.buffer_id();
            self.buffer_descriptor.buffer_update_count = buf_ref.update_count();
        }
        self.byte_stride = byte_stride;
        self.byte_offset = byte_offset;
    }

    fn read_typed_components<T: DracoNumeric + Default + Copy>(
        &self,
        att_index: AttributeValueIndex,
        count: usize,
    ) -> Option<Vec<T>> {
        let component_size = std::mem::size_of::<T>();
        let byte_len = count.checked_mul(component_size)?;
        let byte_pos = self.get_byte_pos(att_index);
        let buf = self.buffer.as_ref()?;
        let buf_ref = buf.borrow();
        if byte_pos < 0 || (byte_pos as usize).checked_add(byte_len)? > buf_ref.data_size() {
            return None;
        }
        let mut raw = vec![0u8; byte_len];
        buf_ref.read(byte_pos, &mut raw);
        let mut values = Vec::with_capacity(count);
        for chunk in raw.chunks_exact(component_size) {
            let value = unsafe { std::ptr::read_unaligned(chunk.as_ptr() as *const T) };
            values.push(value);
        }
        Some(values)
    }

    fn convert_typed_value<T: DracoNumeric + Default + Copy, OutT: DracoNumeric + Default>(
        &self,
        att_index: AttributeValueIndex,
        out_num_components: i8,
        out_value: &mut [OutT],
    ) -> bool {
        let out_num_components = out_num_components.max(0) as usize;
        let count = min(self.num_components as usize, out_num_components);
        let src_values = match self.read_typed_components::<T>(att_index, count) {
            Some(values) => values,
            None => return false,
        };
        for i in 0..count {
            let in_value = src_values[i];
            if let Some(out) = convert_component_value::<T, OutT>(in_value, self.normalized) {
                out_value[i] = out;
            } else {
                return false;
            }
        }
        for i in count..out_num_components {
            out_value[i] = OutT::default();
        }
        true
    }
}

impl Default for GeometryAttribute {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for GeometryAttribute {
    fn eq(&self, other: &Self) -> bool {
        if self.attribute_type != other.attribute_type {
            return false;
        }
        if self.buffer_descriptor.buffer_id != other.buffer_descriptor.buffer_id {
            return false;
        }
        if self.buffer_descriptor.buffer_update_count != other.buffer_descriptor.buffer_update_count
        {
            return false;
        }
        if self.num_components != other.num_components {
            return false;
        }
        if self.data_type != other.data_type {
            return false;
        }
        if self.byte_stride != other.byte_stride {
            return false;
        }
        if self.byte_offset != other.byte_offset {
            return false;
        }
        true
    }
}

pub struct GeometryAttributeHasher;

impl GeometryAttributeHasher {
    pub fn hash(&self, ga: &GeometryAttribute) -> u64 {
        let mut hash = hash_combine(
            &ga.buffer_descriptor.buffer_id,
            &ga.buffer_descriptor.buffer_update_count,
        );
        hash = hash_combine_with(&ga.num_components, hash);
        hash = hash_combine_with(&(ga.data_type as i8), hash);
        hash = hash_combine_with(&(ga.attribute_type as i8), hash);
        hash = hash_combine_with(&ga.byte_stride, hash);
        hash_combine_with(&ga.byte_offset, hash)
    }
}

pub struct GeometryAttributeTypeHasher;

impl GeometryAttributeTypeHasher {
    pub fn hash(&self, at: &GeometryAttributeType) -> u64 {
        *at as i32 as u64
    }
}

fn convert_component_value<T: DracoNumeric, OutT: DracoNumeric>(
    in_value: T,
    normalized: bool,
) -> Option<OutT> {
    let in_info = T::draco_type_info();
    let out_info = OutT::draco_type_info();

    if out_info.is_integral {
        if in_info.is_integral && !T::draco_is_bool() {
            let in_i = in_value.draco_to_i128()?;
            let min_allowed = if in_info.is_signed {
                out_info.min_i128
            } else {
                0
            };
            if in_i < min_allowed || in_i > out_info.max_i128 {
                return None;
            }
        }
        if in_info.is_float {
            if in_value.draco_is_nan_or_inf() {
                return None;
            }
            let in_f = in_value.draco_to_f64();
            if in_f < out_info.min_f64 || in_f >= out_info.max_f64 {
                return None;
            }
        }
    }

    if in_info.is_integral && out_info.is_float && normalized {
        let max_in = in_info.max_f64;
        let out = in_value.draco_to_f64() / max_in;
        return Some(OutT::draco_from_f64(out));
    }
    if in_info.is_float && out_info.is_integral && normalized {
        let in_f = in_value.draco_to_f64();
        if in_f > 1.0 || in_f < 0.0 {
            return None;
        }
        if out_info.size > 4 {
            return None;
        }
        let out = (in_f * out_info.max_f64 + 0.5).floor();
        return Some(OutT::draco_from_f64(out));
    }

    Some(OutT::draco_from_f64(in_value.draco_to_f64()))
}
