//! Metadata utilities.
//! Reference: `_ref/draco/src/draco/metadata/metadata.h` + `.cc`.

use std::collections::BTreeMap;
use std::fmt;
use std::mem::size_of;
use std::ptr::copy_nonoverlapping;

use crate::core::hash_utils::{fingerprint_string, hash_combine, hash_combine_with, CppHash};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct MetadataName(Vec<u8>);

impl MetadataName {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_utf8_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
}

impl AsRef<[u8]> for MetadataName {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<&str> for MetadataName {
    fn from(value: &str) -> Self {
        Self::from_bytes(value.as_bytes())
    }
}

impl From<&[u8]> for MetadataName {
    fn from(value: &[u8]) -> Self {
        Self::from_bytes(value)
    }
}

impl From<Vec<u8>> for MetadataName {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl fmt::Display for MetadataName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_utf8_lossy())
    }
}

impl CppHash for MetadataName {
    fn cpp_hash(&self) -> u64 {
        fingerprint_string(self.as_bytes())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub struct MetadataString(Vec<u8>);

impl MetadataString {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_utf8_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
}

impl AsRef<[u8]> for MetadataString {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<&str> for MetadataString {
    fn from(value: &str) -> Self {
        Self::from_bytes(value.as_bytes())
    }
}

impl From<&[u8]> for MetadataString {
    fn from(value: &[u8]) -> Self {
        Self::from_bytes(value)
    }
}

impl From<Vec<u8>> for MetadataString {
    fn from(value: Vec<u8>) -> Self {
        Self(value)
    }
}

impl fmt::Display for MetadataString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_utf8_lossy())
    }
}

impl CppHash for MetadataString {
    fn cpp_hash(&self) -> u64 {
        fingerprint_string(self.as_bytes())
    }
}

#[derive(Clone, Debug)]
pub struct EntryValue {
    data: Vec<u8>,
}

impl EntryValue {
    pub fn from_value<T: Copy>(data: &T) -> Self {
        let data_type_size = size_of::<T>();
        let mut buffer = vec![0u8; data_type_size];
        unsafe {
            copy_nonoverlapping(
                data as *const T as *const u8,
                buffer.as_mut_ptr(),
                data_type_size,
            );
        }
        Self { data: buffer }
    }

    pub fn from_vec<T: Copy>(data: &[T]) -> Self {
        let total_size = size_of::<T>() * data.len();
        let mut buffer = vec![0u8; total_size];
        if !data.is_empty() {
            unsafe {
                copy_nonoverlapping(data.as_ptr() as *const u8, buffer.as_mut_ptr(), total_size);
            }
        }
        Self { data: buffer }
    }

    pub fn from_string<V: AsRef<[u8]>>(value: V) -> Self {
        Self {
            data: value.as_ref().to_vec(),
        }
    }

    pub fn from_bytes(value: &[u8]) -> Self {
        Self {
            data: value.to_vec(),
        }
    }

    pub fn get_value<T: Copy>(&self, value: &mut T) -> bool {
        let data_type_size = size_of::<T>();
        if data_type_size != self.data.len() {
            return false;
        }
        unsafe {
            copy_nonoverlapping(
                self.data.as_ptr(),
                value as *mut T as *mut u8,
                data_type_size,
            );
        }
        true
    }

    /// Reads typed array from byte buffer. Caller must ensure data was stored as valid T.
    /// Avoids zeroed() which can be UB for types with invalid bit patterns.
    pub fn get_value_vec<T: Copy>(&self, value: &mut Vec<T>) -> bool {
        if self.data.is_empty() {
            return false;
        }
        let data_type_size = size_of::<T>();
        if self.data.len() % data_type_size != 0 {
            return false;
        }
        let count = self.data.len() / data_type_size;
        value.clear();
        value.reserve_exact(count);
        unsafe {
            copy_nonoverlapping(
                self.data.as_ptr(),
                value.as_mut_ptr() as *mut u8,
                self.data.len(),
            );
            value.set_len(count);
        }
        true
    }

    pub fn get_value_string(&self, value: &mut MetadataString) -> bool {
        if self.data.is_empty() {
            return false;
        }
        value.clear();
        value.0.extend_from_slice(&self.data);
        true
    }

    pub fn get_value_bytes(&self, value: &mut Vec<u8>) -> bool {
        if self.data.is_empty() {
            return false;
        }
        value.clear();
        value.extend_from_slice(&self.data);
        true
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

pub struct EntryValueHasher;

impl EntryValueHasher {
    pub fn hash(&self, value: &EntryValue) -> u64 {
        let mut hash = value.data.len() as u64;
        for byte in &value.data {
            hash = hash_combine_with(byte, hash);
        }
        hash
    }
}

#[derive(Clone, Copy)]
pub struct MetadataEntries<'a>(&'a BTreeMap<MetadataName, EntryValue>);

impl<'a> MetadataEntries<'a> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains_key<N: AsRef<[u8]>>(&self, name: N) -> bool {
        self.0.contains_key(&MetadataName::from(name.as_ref()))
    }

    pub fn get<N: AsRef<[u8]>>(&self, name: N) -> Option<&'a EntryValue> {
        self.0.get(&MetadataName::from(name.as_ref()))
    }

    pub fn keys(self) -> impl Iterator<Item = &'a MetadataName> {
        self.0.keys()
    }
}

impl<'a> IntoIterator for MetadataEntries<'a> {
    type Item = (&'a MetadataName, &'a EntryValue);
    type IntoIter = std::collections::btree_map::Iter<'a, MetadataName, EntryValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Clone, Copy)]
pub struct MetadataSubMetadatas<'a>(&'a BTreeMap<MetadataName, Box<Metadata>>);

impl<'a> MetadataSubMetadatas<'a> {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn contains_key<N: AsRef<[u8]>>(&self, name: N) -> bool {
        self.0.contains_key(&MetadataName::from(name.as_ref()))
    }

    pub fn get<N: AsRef<[u8]>>(&self, name: N) -> Option<&'a Metadata> {
        self.0
            .get(&MetadataName::from(name.as_ref()))
            .map(|metadata| metadata.as_ref())
    }
}

impl<'a> IntoIterator for MetadataSubMetadatas<'a> {
    type Item = (&'a MetadataName, &'a Box<Metadata>);
    type IntoIter = std::collections::btree_map::Iter<'a, MetadataName, Box<Metadata>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Metadata {
    entries: BTreeMap<MetadataName, EntryValue>,
    sub_metadatas: BTreeMap<MetadataName, Box<Metadata>>,
}

impl Metadata {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            sub_metadatas: BTreeMap::new(),
        }
    }

    pub fn add_entry_int(&mut self, name: &str, value: i32) {
        self.add_entry(name, EntryValue::from_value(&value));
    }

    pub fn get_entry_int(&self, name: &str, value: &mut i32) -> bool {
        self.get_entry(name, |entry| entry.get_value(value))
    }

    pub fn add_entry_int_array(&mut self, name: &str, value: &[i32]) {
        self.add_entry(name, EntryValue::from_vec(value));
    }

    pub fn get_entry_int_array(&self, name: &str, value: &mut Vec<i32>) -> bool {
        self.get_entry(name, |entry| entry.get_value_vec(value))
    }

    pub fn add_entry_double(&mut self, name: &str, value: f64) {
        self.add_entry(name, EntryValue::from_value(&value));
    }

    pub fn get_entry_double(&self, name: &str, value: &mut f64) -> bool {
        self.get_entry(name, |entry| entry.get_value(value))
    }

    pub fn add_entry_double_array(&mut self, name: &str, value: &[f64]) {
        self.add_entry(name, EntryValue::from_vec(value));
    }

    pub fn get_entry_double_array(&self, name: &str, value: &mut Vec<f64>) -> bool {
        self.get_entry(name, |entry| entry.get_value_vec(value))
    }

    pub fn add_entry_string<N: AsRef<[u8]>, V: AsRef<[u8]>>(&mut self, name: N, value: V) {
        self.add_entry(name, EntryValue::from_string(value));
    }

    pub fn get_entry_string<N: AsRef<[u8]>>(&self, name: N, value: &mut MetadataString) -> bool {
        self.get_entry(name, |entry| entry.get_value_string(value))
    }

    pub fn add_entry_bytes<N: AsRef<[u8]>>(&mut self, name: N, value: &[u8]) {
        self.add_entry(name, EntryValue::from_bytes(value));
    }

    pub fn get_entry_bytes<N: AsRef<[u8]>>(&self, name: N, value: &mut Vec<u8>) -> bool {
        self.get_entry(name, |entry| entry.get_value_bytes(value))
    }

    pub fn add_entry_binary<N: AsRef<[u8]>>(&mut self, name: N, value: &[u8]) {
        self.add_entry(name, EntryValue::from_vec(value));
    }

    pub fn get_entry_binary<N: AsRef<[u8]>>(&self, name: N, value: &mut Vec<u8>) -> bool {
        self.get_entry(name, |entry| entry.get_value_vec(value))
    }

    pub fn add_sub_metadata<N: AsRef<[u8]>>(&mut self, name: N, sub_metadata: Metadata) -> bool {
        let name = MetadataName::from(name.as_ref());
        if self.sub_metadatas.contains_key(&name) {
            return false;
        }
        self.sub_metadatas.insert(name, Box::new(sub_metadata));
        true
    }

    pub fn get_sub_metadata<N: AsRef<[u8]>>(&self, name: N) -> Option<&Metadata> {
        self.sub_metadatas
            .get(&MetadataName::from(name.as_ref()))
            .map(|m| m.as_ref())
    }

    pub fn sub_metadata<N: AsRef<[u8]>>(&mut self, name: N) -> Option<&mut Metadata> {
        self.sub_metadatas
            .get_mut(&MetadataName::from(name.as_ref()))
            .map(|m| m.as_mut())
    }

    pub fn remove_entry<N: AsRef<[u8]>>(&mut self, name: N) {
        self.entries.remove(&MetadataName::from(name.as_ref()));
    }

    pub fn num_entries(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> MetadataEntries<'_> {
        MetadataEntries(&self.entries)
    }

    pub fn sub_metadatas(&self) -> MetadataSubMetadatas<'_> {
        MetadataSubMetadatas(&self.sub_metadatas)
    }

    fn add_entry<N: AsRef<[u8]>>(&mut self, entry_name: N, entry_value: EntryValue) {
        self.entries
            .insert(MetadataName::from(entry_name.as_ref()), entry_value);
    }

    fn get_entry<N: AsRef<[u8]>, F>(&self, entry_name: N, getter: F) -> bool
    where
        F: FnOnce(&EntryValue) -> bool,
    {
        match self.entries.get(&MetadataName::from(entry_name.as_ref())) {
            Some(entry) => getter(entry),
            None => false,
        }
    }
}

pub struct MetadataHasher;

impl MetadataHasher {
    pub fn hash(&self, metadata: &Metadata) -> u64 {
        let mut hash = hash_combine(
            &(metadata.entries.len() as u64),
            &(metadata.sub_metadatas.len() as u64),
        );
        let entry_value_hasher = EntryValueHasher;
        for (name, value) in metadata.entries.iter() {
            hash = hash_combine_with(name, hash);
            let value_hash = entry_value_hasher.hash(value);
            hash = hash_combine_with(&value_hash, hash);
        }
        let metadata_hasher = MetadataHasher;
        for (name, sub_metadata) in metadata.sub_metadatas.iter() {
            hash = hash_combine_with(name, hash);
            let sub_hash = metadata_hasher.hash(sub_metadata);
            hash = hash_combine_with(&sub_hash, hash);
        }
        hash
    }
}
