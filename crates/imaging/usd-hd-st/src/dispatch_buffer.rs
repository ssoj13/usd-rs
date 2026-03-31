#![allow(dead_code)]

//! HdStDispatchBuffer - Buffer for indirect draw/dispatch commands.
//!
//! A uint32 array buffer used for GPU indirect dispatch (MultiDrawIndirect,
//! DispatchComputeIndirect). Supports interleaved buffer resource views
//! over the same underlying data for different shader binding points.
//!
//! For example, a single draw item might have 14 uint32s containing:
//! - MDI draw params (count, instanceCount, first, baseVertex, baseInstance)
//! - Cull params
//! - DrawingCoord0, DrawingCoord1, DrawingCoordI
//!
//! Port of pxr/imaging/hdSt/dispatchBuffer.h

use super::buffer_resource::{
    HdStBufferResource, HdStBufferResourceNamedList, HdStBufferResourceSharedPtr,
};
use std::sync::Arc;
use usd_hd::types::HdTupleType;
use usd_hgi::HgiBufferHandle;
use usd_tf::Token;

/// Shared pointer to dispatch buffer.
pub type HdStDispatchBufferSharedPtr = Arc<HdStDispatchBuffer>;

/// Buffer for indirect draw/dispatch commands.
///
/// Stores an array of uint32 commands with interleaved resource views.
/// Each draw item occupies `command_num_uints` consecutive uint32s.
///
/// # Resource Views
///
/// Multiple named buffer resource views can be defined over the same
/// underlying data with different offsets and strides. This allows
/// different parts of the draw command to be bound to different
/// shader inputs.
///
/// Port of HdStDispatchBuffer from pxr/imaging/hdSt/dispatchBuffer.h
#[derive(Debug)]
pub struct HdStDispatchBuffer {
    /// Role token for this buffer
    role: Token,
    /// Number of draw items (commands)
    count: usize,
    /// Number of uint32s per command
    command_num_uints: u32,
    /// CPU-side command data
    data: Vec<u32>,
    /// The entire buffer as a single resource
    entire_resource: HdStBufferResourceSharedPtr,
    /// Named interleaved resource views
    resource_list: HdStBufferResourceNamedList,
    /// HGI buffer handle for the GPU buffer
    buffer_handle: HgiBufferHandle,
}

impl HdStDispatchBuffer {
    /// Create a new dispatch buffer.
    ///
    /// # Arguments
    /// * `role` - Buffer role token (e.g., "drawDispatch")
    /// * `count` - Number of draw commands
    /// * `command_num_uints` - Number of uint32s per command
    pub fn new(role: Token, count: usize, command_num_uints: u32) -> Self {
        let total_uints = count * command_num_uints as usize;
        let byte_size = total_uints * std::mem::size_of::<u32>();

        // Create the entire buffer resource
        let mut entire = HdStBufferResource::new(
            role.clone(),
            HdTupleType::default(),
            0,
            (command_num_uints as i32) * (std::mem::size_of::<u32>() as i32),
        );
        entire.set_allocation(HgiBufferHandle::default(), byte_size);

        Self {
            role,
            count,
            command_num_uints,
            data: vec![0u32; total_uints],
            entire_resource: Arc::new(entire),
            resource_list: Vec::new(),
            buffer_handle: HgiBufferHandle::default(),
        }
    }

    /// Update entire buffer data from CPU.
    ///
    /// Data length must match count * command_num_uints.
    pub fn copy_data(&mut self, data: &[u32]) {
        assert_eq!(
            data.len(),
            self.count * self.command_num_uints as usize,
            "Data size mismatch: expected {}, got {}",
            self.count * self.command_num_uints as usize,
            data.len()
        );
        self.data.copy_from_slice(data);
        // In real impl: upload to GPU via HGI blit
    }

    /// Add an interleaved buffer resource view.
    ///
    /// Creates a named view into the uint32 array at the given offset
    /// (in uint32s, not bytes) with the buffer's stride.
    ///
    /// # Arguments
    /// * `name` - Resource name (e.g., "drawingCoord0")
    /// * `tuple_type` - Data type description
    /// * `offset` - Offset in uint32s from command start
    pub fn add_resource_view(&mut self, name: Token, tuple_type: HdTupleType, offset: u32) {
        let byte_offset = (offset as i32) * (std::mem::size_of::<u32>() as i32);
        let stride = (self.command_num_uints as i32) * (std::mem::size_of::<u32>() as i32);

        let mut resource = HdStBufferResource::new(name.clone(), tuple_type, byte_offset, stride);

        // Share the same GPU buffer
        let byte_size = self.count * self.command_num_uints as usize * std::mem::size_of::<u32>();
        resource.set_allocation(self.buffer_handle.clone(), byte_size);

        self.resource_list.push((name, Arc::new(resource)));
    }

    /// Get number of draw commands.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Get number of uint32s per command.
    pub fn command_num_uints(&self) -> u32 {
        self.command_num_uints
    }

    /// Get the entire buffer as a single resource.
    pub fn entire_resource(&self) -> &HdStBufferResourceSharedPtr {
        &self.entire_resource
    }

    /// Get all named resource views.
    pub fn resources(&self) -> &HdStBufferResourceNamedList {
        &self.resource_list
    }

    /// Get a named resource view.
    pub fn get_resource(&self, name: &Token) -> Option<&HdStBufferResourceSharedPtr> {
        self.resource_list
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r)
    }

    /// Get the raw CPU data.
    pub fn data(&self) -> &[u32] {
        &self.data
    }

    /// Get mutable CPU data.
    pub fn data_mut(&mut self) -> &mut [u32] {
        &mut self.data
    }

    /// Get the role token.
    pub fn role(&self) -> &Token {
        &self.role
    }

    /// Get total byte size.
    pub fn byte_size(&self) -> usize {
        self.data.len() * std::mem::size_of::<u32>()
    }

    /// Get the HGI buffer handle.
    pub fn buffer_handle(&self) -> &HgiBufferHandle {
        &self.buffer_handle
    }

    /// Set the HGI buffer handle (after GPU allocation).
    pub fn set_buffer_handle(&mut self, handle: HgiBufferHandle) {
        self.buffer_handle = handle;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatch_buffer_creation() {
        let buf = HdStDispatchBuffer::new(Token::new("drawDispatch"), 10, 5);

        assert_eq!(buf.count(), 10);
        assert_eq!(buf.command_num_uints(), 5);
        assert_eq!(buf.data().len(), 50);
        assert_eq!(buf.byte_size(), 200);
    }

    #[test]
    fn test_copy_data() {
        let mut buf = HdStDispatchBuffer::new(Token::new("dispatch"), 2, 3);
        let data = vec![1, 2, 3, 4, 5, 6];
        buf.copy_data(&data);

        assert_eq!(buf.data(), &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_resource_views() {
        let mut buf = HdStDispatchBuffer::new(Token::new("dispatch"), 4, 14);

        // Add interleaved views (like draw + cull + drawing coords)
        buf.add_resource_view(
            Token::new("drawingCoord0"),
            HdTupleType::default(),
            9, // offset in uint32s
        );
        buf.add_resource_view(Token::new("drawingCoord1"), HdTupleType::default(), 13);

        assert_eq!(buf.resources().len(), 2);
        assert!(buf.get_resource(&Token::new("drawingCoord0")).is_some());
        assert!(buf.get_resource(&Token::new("drawingCoord1")).is_some());
        assert!(buf.get_resource(&Token::new("missing")).is_none());
    }

    #[test]
    fn test_resource_view_offsets() {
        let mut buf = HdStDispatchBuffer::new(Token::new("dispatch"), 2, 5);

        buf.add_resource_view(Token::new("mdi"), HdTupleType::default(), 0);
        buf.add_resource_view(Token::new("coords"), HdTupleType::default(), 3);

        let mdi = buf.get_resource(&Token::new("mdi")).unwrap();
        assert_eq!(mdi.get_offset(), 0);
        assert_eq!(mdi.get_stride(), 20); // 5 * 4 bytes

        let coords = buf.get_resource(&Token::new("coords")).unwrap();
        assert_eq!(coords.get_offset(), 12); // 3 * 4 bytes
        assert_eq!(coords.get_stride(), 20);
    }

    #[test]
    #[should_panic(expected = "Data size mismatch")]
    fn test_copy_data_mismatch() {
        let mut buf = HdStDispatchBuffer::new(Token::new("dispatch"), 2, 3);
        buf.copy_data(&[1, 2, 3]); // wrong size
    }
}
