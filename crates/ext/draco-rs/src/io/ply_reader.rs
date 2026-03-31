//! PLY reader.
//! Reference: `_ref/draco/src/draco/io/ply_reader.h` + `.cc`.

use std::collections::BTreeMap;

use crate::core::decoder_buffer::DecoderBuffer;
use crate::core::draco_types::{data_type_length, DataType};
use crate::core::status::{Status, StatusCode};
use crate::io::parser_utils;
use crate::io::ply_property_writer::PlyPropertyWriter;

#[derive(Clone, Debug)]
pub struct PlyProperty {
    name: String,
    data: Vec<u8>,
    list_data: Vec<i64>,
    data_type: DataType,
    data_type_num_bytes: i32,
    list_data_type: DataType,
    list_data_type_num_bytes: i32,
}

impl PlyProperty {
    pub fn new(name: &str, data_type: DataType, list_type: DataType) -> Self {
        Self {
            name: name.to_string(),
            data: Vec::new(),
            list_data: Vec::new(),
            data_type,
            data_type_num_bytes: data_type_length(data_type),
            list_data_type: list_type,
            list_data_type_num_bytes: data_type_length(list_type),
        }
    }

    pub fn reserve_data(&mut self, num_entries: i32) {
        let bytes = self.data_type_num_bytes.saturating_mul(num_entries) as usize;
        self.data.reserve(bytes);
    }

    pub fn get_list_entry_offset(&self, entry_id: i32) -> i64 {
        self.list_data[(entry_id as usize) * 2]
    }

    pub fn get_list_entry_num_values(&self, entry_id: i32) -> i64 {
        self.list_data[(entry_id as usize) * 2 + 1]
    }

    /// Returns the byte slice for the entry's data, or None if out of bounds.
    /// Safe API equivalent to C++ GetDataEntryAddress.
    #[inline]
    pub fn get_data_entry_bytes(&self, entry_id: i32) -> Option<&[u8]> {
        let num_bytes = self.data_type_num_bytes as usize;
        let offset = (entry_id as usize).saturating_mul(num_bytes);
        let end = offset.checked_add(num_bytes)?;
        self.data.get(offset..end)
    }

    pub fn push_back_value_bytes(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_list(&self) -> bool {
        self.list_data_type != DataType::Invalid
    }

    pub fn data_type(&self) -> DataType {
        self.data_type
    }

    pub fn data_type_num_bytes(&self) -> i32 {
        self.data_type_num_bytes
    }

    pub fn list_data_type(&self) -> DataType {
        self.list_data_type
    }

    pub fn list_data_type_num_bytes(&self) -> i32 {
        self.list_data_type_num_bytes
    }

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }
}

#[derive(Clone, Debug)]
pub struct PlyElement {
    name: String,
    num_entries: i64,
    properties: Vec<PlyProperty>,
    property_index: BTreeMap<String, usize>,
}

impl PlyElement {
    pub fn new(name: &str, num_entries: i64) -> Self {
        Self {
            name: name.to_string(),
            num_entries,
            properties: Vec::new(),
            property_index: BTreeMap::new(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_property(&mut self, prop: PlyProperty) {
        let index = self.properties.len();
        self.property_index.insert(prop.name().to_string(), index);
        self.properties.push(prop);
        if !self.properties[index].is_list() {
            self.properties[index].reserve_data(self.num_entries as i32);
        }
    }

    pub fn get_property_by_name(&self, name: &str) -> Option<&PlyProperty> {
        self.property_index
            .get(name)
            .and_then(|idx| self.properties.get(*idx))
    }

    pub fn num_properties(&self) -> i32 {
        self.properties.len() as i32
    }

    pub fn num_entries(&self) -> i32 {
        self.num_entries as i32
    }

    pub fn property(&self, prop_index: i32) -> &PlyProperty {
        &self.properties[prop_index as usize]
    }

    pub fn property_mut(&mut self, prop_index: i32) -> &mut PlyProperty {
        &mut self.properties[prop_index as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    LittleEndian = 0,
    Ascii,
}

pub struct PlyReader {
    elements: Vec<PlyElement>,
    element_index: BTreeMap<String, usize>,
    format: Format,
}

impl PlyReader {
    pub fn new() -> Self {
        Self {
            elements: Vec::new(),
            element_index: BTreeMap::new(),
            format: Format::LittleEndian,
        }
    }

    pub fn read(&mut self, buffer: &mut DecoderBuffer) -> Status {
        let mut value = String::new();
        if !parser_utils::parse_string(buffer, &mut value) || value != "ply" {
            return Status::new(StatusCode::InvalidParameter, "Not a valid ply file");
        }
        parser_utils::skip_line(buffer);

        parser_utils::parse_line(buffer, Some(&mut value));
        let words = self.split_words(&value);
        if words.len() >= 3 && words[0] == "format" {
            let format = &words[1];
            let version = &words[2];
            if version != "1.0" {
                return Status::new(StatusCode::UnsupportedVersion, "Unsupported PLY version");
            }
            if format == "binary_big_endian" {
                return Status::new(
                    StatusCode::UnsupportedVersion,
                    "Unsupported format. Currently we support only ascii and binary_little_endian format.",
                );
            }
            if format == "ascii" {
                self.format = Format::Ascii;
            } else {
                self.format = Format::LittleEndian;
            }
        } else {
            return Status::new(StatusCode::InvalidParameter, "Missing or wrong format line");
        }

        let status = self.parse_header(buffer);
        if !status.is_ok() {
            return status;
        }
        if !self.parse_properties_data(buffer) {
            return Status::new(StatusCode::InvalidParameter, "Couldn't parse properties");
        }
        Status::ok()
    }

    pub fn get_element_by_name(&self, name: &str) -> Option<&PlyElement> {
        self.element_index
            .get(name)
            .and_then(|idx| self.elements.get(*idx))
    }

    pub fn num_elements(&self) -> i32 {
        self.elements.len() as i32
    }

    pub fn element(&self, element_index: i32) -> &PlyElement {
        &self.elements[element_index as usize]
    }

    fn parse_header(&mut self, buffer: &mut DecoderBuffer) -> Status {
        loop {
            let end = match self.parse_end_header(buffer) {
                Ok(v) => v,
                Err(status) => return status,
            };
            if end {
                break;
            }
            if self.parse_element(buffer) {
                continue;
            }
            let property_parsed = match self.parse_property(buffer) {
                Ok(v) => v,
                Err(status) => return status,
            };
            if property_parsed {
                continue;
            }
            parser_utils::skip_line(buffer);
        }
        Status::ok()
    }

    fn parse_end_header(&self, buffer: &mut DecoderBuffer) -> Result<bool, Status> {
        parser_utils::skip_whitespace(buffer);
        let mut c = [0u8; 10];
        if !buffer.peek(&mut c) {
            return Err(Status::new(
                StatusCode::InvalidParameter,
                "End of file reached before the end_header",
            ));
        }
        if &c != b"end_header" {
            return Ok(false);
        }
        parser_utils::skip_line(buffer);
        Ok(true)
    }

    fn parse_element(&mut self, buffer: &mut DecoderBuffer) -> bool {
        let mut line_buffer = clone_decoder_buffer(buffer);
        let mut line = String::new();
        parser_utils::parse_line(&mut line_buffer, Some(&mut line));
        let words = self.split_words(&line);
        if words.len() >= 3 && words[0] == "element" {
            let element_name = &words[1];
            let count = words[2].parse::<i64>().unwrap_or(0);
            let element = PlyElement::new(element_name, count);
            self.element_index
                .insert(element.name().to_string(), self.elements.len());
            self.elements.push(element);
            buffer.start_decoding_from(line_buffer.position() as i64);
            return true;
        }
        false
    }

    fn parse_property(&mut self, buffer: &mut DecoderBuffer) -> Result<bool, Status> {
        if self.elements.is_empty() {
            return Ok(false);
        }
        let mut line_buffer = clone_decoder_buffer(buffer);
        let mut line = String::new();
        parser_utils::parse_line(&mut line_buffer, Some(&mut line));
        let words = self.split_words(&line);

        let mut data_type_str = String::new();
        let mut list_type_str = String::new();
        let mut property_name = String::new();
        let mut property_search = false;
        if words.len() >= 3 && words[0] == "property" && words[1] != "list" {
            property_search = true;
            data_type_str = words[1].clone();
            property_name = words[2].clone();
        }
        let mut property_list_search = false;
        if words.len() >= 5 && words[0] == "property" && words[1] == "list" {
            property_list_search = true;
            list_type_str = words[2].clone();
            data_type_str = words[3].clone();
            property_name = words[4].clone();
        }
        if !property_search && !property_list_search {
            return Ok(false);
        }
        let data_type = self.get_data_type_from_string(&data_type_str);
        if data_type == DataType::Invalid {
            return Err(Status::new(
                StatusCode::InvalidParameter,
                "Wrong property data type",
            ));
        }
        let mut list_type = DataType::Invalid;
        if property_list_search {
            list_type = self.get_data_type_from_string(&list_type_str);
            if list_type == DataType::Invalid {
                return Err(Status::new(
                    StatusCode::InvalidParameter,
                    "Wrong property list type",
                ));
            }
        }
        let last = self.elements.len() - 1;
        self.elements[last].add_property(PlyProperty::new(&property_name, data_type, list_type));
        buffer.start_decoding_from(line_buffer.position() as i64);
        Ok(true)
    }

    fn parse_properties_data(&mut self, buffer: &mut DecoderBuffer) -> bool {
        for i in 0..self.elements.len() {
            let ok = match self.format {
                Format::LittleEndian => self.parse_element_data(buffer, i as i32),
                Format::Ascii => self.parse_element_data_ascii(buffer, i as i32),
            };
            if !ok {
                return false;
            }
        }
        true
    }

    fn parse_element_data(&mut self, buffer: &mut DecoderBuffer, element_index: i32) -> bool {
        let element = &mut self.elements[element_index as usize];
        for _entry in 0..element.num_entries() {
            for prop_index in 0..element.num_properties() {
                let prop = element.property_mut(prop_index);
                if prop.is_list() {
                    let num_entries = match read_list_count(buffer, prop.list_data_type()) {
                        Some(v) => v,
                        None => return false,
                    };
                    if num_entries < 0 {
                        return false;
                    }
                    let offset = (prop.data.len() as i64) / (prop.data_type_num_bytes() as i64);
                    prop.list_data.push(offset);
                    prop.list_data.push(num_entries);
                    let num_bytes = (prop.data_type_num_bytes() as i64) * num_entries;
                    if num_bytes < 0 {
                        return false;
                    }
                    let num_bytes = num_bytes as usize;
                    let head = buffer.data_head();
                    if head.len() < num_bytes {
                        return false;
                    }
                    prop.data.extend_from_slice(&head[..num_bytes]);
                    buffer.advance(num_bytes as i64);
                } else {
                    let num_bytes = prop.data_type_num_bytes() as usize;
                    let head = buffer.data_head();
                    if head.len() < num_bytes {
                        return false;
                    }
                    prop.data.extend_from_slice(&head[..num_bytes]);
                    buffer.advance(num_bytes as i64);
                }
            }
        }
        true
    }

    fn parse_element_data_ascii(&mut self, buffer: &mut DecoderBuffer, element_index: i32) -> bool {
        let element = &mut self.elements[element_index as usize];
        for _entry in 0..element.num_entries() {
            for prop_index in 0..element.num_properties() {
                let prop = element.property_mut(prop_index);
                let mut num_entries: i32 = 1;
                if prop.is_list() {
                    parser_utils::skip_whitespace(buffer);
                    if !parser_utils::parse_signed_int(buffer, &mut num_entries) {
                        return false;
                    }
                    let offset = (prop.data.len() as i64) / (prop.data_type_num_bytes() as i64);
                    prop.list_data.push(offset);
                    prop.list_data.push(num_entries as i64);
                }
                for _v in 0..num_entries {
                    parser_utils::skip_whitespace(buffer);
                    if prop.data_type() == DataType::Float32
                        || prop.data_type() == DataType::Float64
                    {
                        let mut val: f32 = 0.0;
                        if !parser_utils::parse_float(buffer, &mut val) {
                            return false;
                        }
                        let mut writer = PlyPropertyWriter::<f64>::new(prop);
                        writer.push_back_value(val as f64);
                    } else {
                        let mut val: i32 = 0;
                        if !parser_utils::parse_signed_int(buffer, &mut val) {
                            return false;
                        }
                        let mut writer = PlyPropertyWriter::<i32>::new(prop);
                        writer.push_back_value(val);
                    }
                }
            }
        }
        true
    }

    fn split_words(&self, line: &str) -> Vec<String> {
        let mut output = Vec::new();
        let mut start = 0usize;
        while let Some(end) = line[start..].find(|c: char| c.is_ascii_whitespace()) {
            let end_idx = start + end;
            let word = line[start..end_idx].trim();
            if !word.is_empty() {
                output.push(word.to_string());
            }
            start = end_idx + 1;
        }
        let last = line[start..].trim();
        if !last.is_empty() {
            output.push(last.to_string());
        }
        output
    }

    fn get_data_type_from_string(&self, name: &str) -> DataType {
        match name {
            "char" | "int8" => DataType::Int8,
            "uchar" | "uint8" => DataType::Uint8,
            "short" | "int16" => DataType::Int16,
            "ushort" | "uint16" => DataType::Uint16,
            "int" | "int32" => DataType::Int32,
            "uint" | "uint32" => DataType::Uint32,
            "float" | "float32" => DataType::Float32,
            "double" | "float64" => DataType::Float64,
            _ => DataType::Invalid,
        }
    }
}

fn clone_decoder_buffer<'a>(buffer: &DecoderBuffer<'a>) -> DecoderBuffer<'a> {
    let mut tmp = DecoderBuffer::new();
    tmp.init_with_version(buffer.data(), buffer.bitstream_version());
    tmp.start_decoding_from(buffer.position() as i64);
    tmp
}

fn read_list_count(buffer: &mut DecoderBuffer, list_type: DataType) -> Option<i64> {
    let num_bytes = data_type_length(list_type);
    if num_bytes <= 0 || num_bytes > 8 {
        return None;
    }
    let mut bytes = vec![0u8; num_bytes as usize];
    if !buffer.decode_bytes(&mut bytes) {
        return None;
    }
    let value = match list_type {
        DataType::Uint8 => bytes[0] as i64,
        DataType::Int8 => (bytes[0] as i8) as i64,
        DataType::Uint16 => {
            let v = u16::from_le_bytes([bytes[0], bytes[1]]);
            v as i64
        }
        DataType::Int16 => {
            let v = i16::from_le_bytes([bytes[0], bytes[1]]);
            v as i64
        }
        DataType::Uint32 => {
            let v = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            v as i64
        }
        DataType::Int32 => {
            let v = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            v as i64
        }
        _ => return None,
    };
    Some(value)
}
