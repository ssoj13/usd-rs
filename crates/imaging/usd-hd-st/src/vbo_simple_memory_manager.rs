#![allow(dead_code)]

//! HdStVBOSimpleMemoryManager - Simple non-aggregated VBO allocation.
//!
//! Unlike HdStVBOMemoryManager which aggregates multiple ranges into
//! shared buffer arrays, this manager creates a dedicated buffer array
//! for each range (1:1 mapping). Simpler but uses more GPU resources.
//!
//! Used for buffers that cannot be aggregated (e.g., topology indices,
//! immutable buffers, or buffers with unique layouts).
//!
//! Port of pxr/imaging/hdSt/vboSimpleMemoryManager.h

use crate::buffer_resource::{
    HdStBufferResource, HdStBufferResourceNamedList, HdStBufferResourceSharedPtr,
};
use crate::strategy_base::{AggregationId, HdStAggregationStrategy};
use usd_hd::resource::{
    HdBufferArray, HdBufferArrayHandle, HdBufferArrayRangeHandle,
    HdBufferArrayRangeWeakHandle, HdBufferArrayUsageHint,
    HdBufferSourceHandle, HdBufferSpec, HdBufferSpecVector,
};
use usd_hgi::HgiBufferUsage;
use usd_tf::Token;
use usd_vt::Value as VtValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak, atomic::{AtomicUsize, Ordering}};

/// Maximum VBO size in bytes (default 32 MB).
const HD_MAX_VBO_SIZE: usize = 32 * 1024 * 1024;

// ---------------------------------------------------------------------------
// SimpleBufferArrayRange
// ---------------------------------------------------------------------------

/// Buffer array range for simple (non-aggregated) buffers.
///
/// Always at offset 0 since the buffer is dedicated to this single range.
///
/// Port of HdStVBOSimpleMemoryManager::_SimpleBufferArrayRange
pub struct SimpleBufferArrayRange {
    /// Back-pointer to owning buffer array (None = invalidated)
    buffer_array: Mutex<Option<HdBufferArrayHandle>>,
    /// Number of elements in this range
    num_elements: AtomicUsize,
}

impl std::fmt::Debug for SimpleBufferArrayRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleBufferArrayRange")
            .field("num_elements", &self.num_elements.load(Ordering::Relaxed))
            .finish()
    }
}

impl SimpleBufferArrayRange {
    /// Create a new simple buffer array range.
    pub fn new() -> Self {
        Self {
            buffer_array: Mutex::new(None),
            num_elements: AtomicUsize::new(0),
        }
    }

    /// Invalidate this range (detach from buffer array).
    pub fn invalidate(&self) {
        *self.buffer_array.lock().unwrap() = None;
    }
}

impl Default for SimpleBufferArrayRange {
    fn default() -> Self {
        Self::new()
    }
}

impl usd_hd::resource::HdBufferArrayRange for SimpleBufferArrayRange {
    fn is_valid(&self) -> bool {
        self.buffer_array.lock().unwrap().is_some()
    }

    fn is_assigned(&self) -> bool {
        self.buffer_array.lock().unwrap().is_some()
    }

    fn is_immutable(&self) -> bool {
        self.buffer_array
            .lock()
            .unwrap()
            .as_ref()
            .map(|ba| ba.is_immutable())
            .unwrap_or(false)
    }

    fn requires_staging(&self) -> bool {
        // Simple buffers typically don't need staging
        false
    }

    fn resize(&self, num_elements: usize) -> bool {
        self.num_elements.store(num_elements, Ordering::Release);
        // Always triggers reallocation for simple buffers
        true
    }

    fn copy_data(&self, _buffer_source: HdBufferSourceHandle) {
        // Placeholder: would copy CPU data to GPU buffer via HGI
    }

    fn read_data(&self, _name: &Token) -> Option<VtValue> {
        None // Placeholder: would read back from GPU via HGI
    }

    /// Element offset is always 0 for simple (non-aggregated) buffers.
    fn get_element_offset(&self) -> usize {
        0
    }

    /// Byte offset is always 0 for simple buffers.
    fn get_byte_offset(&self, _resource_name: &Token) -> usize {
        0
    }

    fn get_num_elements(&self) -> usize {
        self.num_elements.load(Ordering::Acquire)
    }

    fn get_version(&self) -> usize {
        self.buffer_array
            .lock()
            .unwrap()
            .as_ref()
            .map(|ba| ba.get_version())
            .unwrap_or(0)
    }

    fn increment_version(&self) {
        if let Some(ba) = self.buffer_array.lock().unwrap().as_ref() {
            ba.increment_version();
        }
    }

    fn get_max_num_elements(&self) -> usize {
        self.buffer_array
            .lock()
            .unwrap()
            .as_ref()
            .map(|ba| ba.get_max_num_elements())
            .unwrap_or(0)
    }

    fn get_usage_hint(&self) -> HdBufferArrayUsageHint {
        self.buffer_array
            .lock()
            .unwrap()
            .as_ref()
            .map(|ba| ba.get_usage_hint())
            .unwrap_or(0)
    }

    fn set_buffer_array(&self, buffer_array: Option<HdBufferArrayHandle>) {
        *self.buffer_array.lock().unwrap() = buffer_array;
    }

    fn get_buffer_specs(&self) -> HdBufferSpecVector {
        Vec::new() // Simple ranges don't track specs directly
    }

    fn get_aggregation_id(&self) -> Option<usize> {
        // Simple ranges: use pointer-based identity (each buffer array is unique)
        self.buffer_array
            .lock()
            .unwrap()
            .as_ref()
            .map(|ba| Arc::as_ptr(ba) as usize)
    }

    fn debug_dump(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(
            out,
            "SimpleBufferArrayRange(elements={}, valid={})",
            self.get_num_elements(),
            self.is_valid()
        )
    }
}

// ---------------------------------------------------------------------------
// SimpleBufferArray
// ---------------------------------------------------------------------------

/// Simple (non-aggregated) buffer array.
///
/// Holds a single range with dedicated GPU resources. Each resource name
/// gets its own HGI buffer. No sharing between ranges.
///
/// Port of HdStVBOSimpleMemoryManager::_SimpleBufferArray
pub struct SimpleBufferArray {
    /// Role token (e.g., "vertex", "index")
    role: Token,
    /// Buffer usage hint
    usage_hint: HdBufferArrayUsageHint,
    /// Version counter (incremented on data change)
    version: AtomicUsize,
    /// Current capacity in elements
    capacity: AtomicUsize,
    /// Maximum bytes per element (across all resources)
    max_bytes_per_element: usize,
    /// HGI buffer usage flags
    _buffer_usage: HgiBufferUsage,
    /// Named GPU resources
    resource_list: Mutex<HdStBufferResourceNamedList>,
    /// Whether the buffer needs reallocation
    needs_reallocation: Mutex<bool>,
    /// Ranges (simple: at most 1)
    ranges: Mutex<Vec<Weak<dyn usd_hd::resource::HdBufferArrayRange>>>,
}

impl std::fmt::Debug for SimpleBufferArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimpleBufferArray")
            .field("role", &self.role.as_str())
            .field("capacity", &self.capacity.load(Ordering::Relaxed))
            .finish()
    }
}

impl SimpleBufferArray {
    /// Create a new simple buffer array.
    pub fn new(
        role: Token,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> Self {
        let mut max_bytes = 0usize;

        // Create resources from buffer specs
        let mut resources = Vec::with_capacity(buffer_specs.len());
        for spec in buffer_specs {
            let sz = spec.tuple_type.size_in_bytes();
            max_bytes = max_bytes.max(sz);

            let resource = HdStBufferResource::new(
                spec.name.clone(),
                spec.tuple_type,
                0, // offset
                0, // stride (tightly packed)
            );
            resources.push((spec.name.clone(), Arc::new(resource)));
        }

        // Determine HGI buffer usage from hint
        let buffer_usage = if usage_hint & 0x1 != 0 {
            HgiBufferUsage::UNIFORM
        } else {
            HgiBufferUsage::VERTEX
        };

        Self {
            role,
            usage_hint,
            version: AtomicUsize::new(0),
            capacity: AtomicUsize::new(0),
            max_bytes_per_element: max_bytes,
            _buffer_usage: buffer_usage,
            resource_list: Mutex::new(resources),
            needs_reallocation: Mutex::new(false),
            ranges: Mutex::new(Vec::new()),
        }
    }

    /// Get the role token.
    pub fn role(&self) -> &Token {
        &self.role
    }

    /// Get current element capacity.
    pub fn capacity(&self) -> usize {
        self.capacity.load(Ordering::Acquire)
    }

    /// Get the single GPU resource (first one). Logs warning if >1 resources.
    pub fn get_resource(&self) -> Option<HdStBufferResourceSharedPtr> {
        let list = self.resource_list.lock().unwrap();
        if list.len() > 1 {
            log::warn!("SimpleBufferArray has {} resources, expected 1", list.len());
        }
        list.first().map(|(_, r)| r.clone())
    }

    /// Get a named GPU resource.
    pub fn get_resource_by_name(&self, name: &Token) -> Option<HdStBufferResourceSharedPtr> {
        let list = self.resource_list.lock().unwrap();
        list.iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.clone())
    }

    /// Get buffer specs from current resources.
    pub fn get_buffer_specs_from_resources(&self) -> HdBufferSpecVector {
        let list = self.resource_list.lock().unwrap();
        list.iter()
            .map(|(name, res)| HdBufferSpec::new(name.clone(), res.get_tuple_type()))
            .collect()
    }

    /// Deallocate all GPU resources.
    pub fn deallocate_resources(&self) {
        let mut list = self.resource_list.lock().unwrap();
        list.clear();
        self.capacity.store(0, Ordering::Release);
    }
}

impl HdBufferArray for SimpleBufferArray {
    fn get_role(&self) -> &Token {
        &self.role
    }

    fn get_version(&self) -> usize {
        self.version.load(Ordering::Acquire)
    }

    fn increment_version(&self) {
        self.version.fetch_add(1, Ordering::Release);
    }

    fn try_assign_range(&self, range: HdBufferArrayRangeHandle) -> bool {
        let mut ranges = self.ranges.lock().unwrap();
        // Simple: only one range per buffer array
        if ranges.is_empty() {
            ranges.push(Arc::downgrade(&range));
            true
        } else {
            false
        }
    }

    fn garbage_collect(&self) -> bool {
        let mut ranges = self.ranges.lock().unwrap();
        ranges.retain(|w| w.upgrade().is_some());
        ranges.is_empty()
    }

    fn reallocate(
        &self,
        ranges: &[HdBufferArrayRangeHandle],
        _cur_range_owner: Option<HdBufferArrayHandle>,
    ) {
        let mut self_ranges = self.ranges.lock().unwrap();
        *self_ranges = ranges.iter().map(|r| Arc::downgrade(r)).collect();
        *self.needs_reallocation.lock().unwrap() = false;
    }

    fn get_max_num_elements(&self) -> usize {
        if self.max_bytes_per_element == 0 {
            return 0;
        }
        HD_MAX_VBO_SIZE / self.max_bytes_per_element
    }

    fn get_range_count(&self) -> usize {
        self.ranges.lock().unwrap().len()
    }

    fn get_range(&self, idx: usize) -> Option<HdBufferArrayRangeWeakHandle> {
        self.ranges.lock().unwrap().get(idx).cloned()
    }

    fn remove_unused_ranges(&self) -> usize {
        let mut ranges = self.ranges.lock().unwrap();
        ranges.retain(|w| w.upgrade().is_some());
        ranges.len()
    }

    fn needs_reallocation(&self) -> bool {
        *self.needs_reallocation.lock().unwrap()
    }

    fn get_usage_hint(&self) -> HdBufferArrayUsageHint {
        self.usage_hint
    }

    fn debug_dump(&self, out: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(
            out,
            "SimpleBufferArray(role={}, capacity={}, resources={})",
            self.role.as_str(),
            self.capacity.load(Ordering::Relaxed),
            self.resource_list.lock().unwrap().len()
        )
    }
}

// ---------------------------------------------------------------------------
// VBOSimpleMemoryManager (aggregation strategy)
// ---------------------------------------------------------------------------

/// VBO simple memory manager (aggregation strategy).
///
/// Creates dedicated buffer arrays for each range (no aggregation).
/// Simpler than VBOMemoryManager but uses more GPU resources.
///
/// Port of HdStVBOSimpleMemoryManager from pxr/imaging/hdSt/vboSimpleMemoryManager.h
pub struct VboSimpleMemoryManager {}

impl VboSimpleMemoryManager {
    /// Create a new VBO simple memory manager.
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for VboSimpleMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl HdStAggregationStrategy for VboSimpleMemoryManager {
    fn create_buffer_array(
        &self,
        role: &Token,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> HdBufferArrayHandle {
        Arc::new(SimpleBufferArray::new(role.clone(), buffer_specs, usage_hint))
    }

    fn create_buffer_array_range(&self) -> HdBufferArrayRangeHandle {
        Arc::new(SimpleBufferArrayRange::new())
    }

    fn compute_aggregation_id(
        &self,
        buffer_specs: &HdBufferSpecVector,
        usage_hint: HdBufferArrayUsageHint,
    ) -> AggregationId {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Simple manager: unique ID per specs+usage for grouping
        for spec in buffer_specs {
            spec.name.as_str().hash(&mut hasher);
            spec.tuple_type.hash(&mut hasher);
        }
        usage_hint.hash(&mut hasher);

        hasher.finish()
    }

    fn get_buffer_specs(&self, _buffer_array: &HdBufferArrayHandle) -> HdBufferSpecVector {
        Vec::new()
    }

    fn get_resource_allocation(
        &self,
        _buffer_array: &HdBufferArrayHandle,
        _result: &mut HashMap<String, usize>,
    ) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::types::{HdType, HdTupleType};

    #[test]
    fn test_simple_buffer_array_creation() {
        let specs = vec![HdBufferSpec::new(
            Token::new("points"),
            HdTupleType::new(HdType::FloatVec3, 1),
        )];
        let ba = SimpleBufferArray::new(Token::new("vertex"), &specs, 0);
        assert_eq!(ba.role().as_str(), "vertex");
        assert_eq!(ba.capacity(), 0);
        assert!(!ba.is_immutable());
    }

    #[test]
    fn test_simple_range_creation() {
        let range = SimpleBufferArrayRange::new();
        assert!(!range.is_valid());
        assert!(!<SimpleBufferArrayRange as usd_hd::resource::HdBufferArrayRange>::is_assigned(
            &range
        ));
        assert_eq!(
            <SimpleBufferArrayRange as usd_hd::resource::HdBufferArrayRange>::get_element_offset(
                &range
            ),
            0
        );
        assert_eq!(
            <SimpleBufferArrayRange as usd_hd::resource::HdBufferArrayRange>::get_num_elements(
                &range
            ),
            0
        );
    }

    #[test]
    fn test_max_elements() {
        let specs = vec![HdBufferSpec::new(
            Token::new("points"),
            HdTupleType::new(HdType::FloatVec3, 1),
        )];
        let ba = SimpleBufferArray::new(Token::new("vertex"), &specs, 0);

        // max_bytes_per_element should be size of Vec3f = 12
        let max = ba.get_max_num_elements();
        assert!(max > 0);
        assert_eq!(max, HD_MAX_VBO_SIZE / 12);
    }

    #[test]
    fn test_vbo_simple_manager() {
        let mgr = VboSimpleMemoryManager::new();
        let specs = vec![HdBufferSpec::new(
            Token::new("points"),
            HdTupleType::new(HdType::FloatVec3, 1),
        )];

        let id = mgr.compute_aggregation_id(&specs, 0);
        assert!(id > 0);

        // Flush should not panic
        mgr.flush();
    }

    #[test]
    fn test_buffer_specs_from_resources() {
        let specs = vec![
            HdBufferSpec::new(Token::new("points"), HdTupleType::new(HdType::FloatVec3, 1)),
            HdBufferSpec::new(Token::new("normals"), HdTupleType::new(HdType::FloatVec3, 1)),
        ];
        let ba = SimpleBufferArray::new(Token::new("vertex"), &specs, 0);
        let out_specs = ba.get_buffer_specs_from_resources();
        assert_eq!(out_specs.len(), 2);
    }

    #[test]
    fn test_simple_range_invalidate() {
        use usd_hd::resource::HdBufferArrayRange;

        let specs = vec![HdBufferSpec::new(
            Token::new("points"),
            HdTupleType::new(HdType::FloatVec3, 1),
        )];
        let ba: HdBufferArrayHandle =
            Arc::new(SimpleBufferArray::new(Token::new("vertex"), &specs, 0));

        let range = SimpleBufferArrayRange::new();
        range.set_buffer_array(Some(ba));
        assert!(range.is_valid());

        range.invalidate();
        assert!(!range.is_valid());
    }

    #[test]
    fn test_try_assign_range() {
        let specs = vec![HdBufferSpec::new(
            Token::new("points"),
            HdTupleType::new(HdType::FloatVec3, 1),
        )];
        let ba = SimpleBufferArray::new(Token::new("vertex"), &specs, 0);

        let r1: HdBufferArrayRangeHandle = Arc::new(SimpleBufferArrayRange::new());
        let r2: HdBufferArrayRangeHandle = Arc::new(SimpleBufferArrayRange::new());

        // First range should succeed
        assert!(ba.try_assign_range(r1));
        // Second should fail (simple: 1:1)
        assert!(!ba.try_assign_range(r2));
    }
}
