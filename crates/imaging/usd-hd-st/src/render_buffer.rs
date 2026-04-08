//! HdStRenderBuffer - Storm render buffer implementation.
//!
//! Implements render buffer (render target) for the Storm backend.
//! Render buffers are used for offscreen rendering and post-processing.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use usd_gf::Vec3i;
use usd_sdf::Path as SdfPath;

/// Render buffer format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HdFormat {
    /// 8-bit unsigned normalized RGBA
    #[default]
    UNorm8Vec4,
    /// 8-bit unsigned normalized RGB
    UNorm8Vec3,
    /// 16-bit float RGBA
    Float16Vec4,
    /// 32-bit float RGBA
    Float32Vec4,
    /// 32-bit float (depth)
    Float32,
    /// 32-bit integer
    Int32,
}

impl HdFormat {
    /// Get bytes per pixel for this format.
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            HdFormat::UNorm8Vec4 => 4,
            HdFormat::UNorm8Vec3 => 3,
            HdFormat::Float16Vec4 => 8,
            HdFormat::Float32Vec4 => 16,
            HdFormat::Float32 => 4,
            HdFormat::Int32 => 4,
        }
    }
}

/// Storm render buffer.
///
/// Represents a GPU texture resource that can be rendered into.
/// Supports multi-sampling for anti-aliasing.
#[derive(Debug)]
pub struct HdStRenderBuffer {
    /// Prim path
    path: SdfPath,
    /// Dimensions (width, height, depth)
    dimensions: Vec3i,
    /// Pixel format
    format: HdFormat,
    /// Multi-sampled flag
    multi_sampled: bool,
    /// MSAA sample count (1 = no MSAA)
    msaa_sample_count: u32,
    /// Number of current mappers
    mappers: AtomicI32,
    /// Mapped buffer data (for CPU access)
    mapped_buffer: Vec<u8>,
    /// Whether the buffer is allocated
    allocated: bool,
}

impl HdStRenderBuffer {
    /// Create a new render buffer.
    pub fn new(path: SdfPath) -> Self {
        Self {
            path,
            dimensions: Vec3i::new(0, 0, 1),
            format: HdFormat::UNorm8Vec4,
            multi_sampled: false,
            msaa_sample_count: 1,
            mappers: AtomicI32::new(0),
            mapped_buffer: Vec::new(),
            allocated: false,
        }
    }

    /// Get the prim path.
    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }

    /// Allocate the render buffer with given dimensions and format.
    ///
    /// # Arguments
    ///
    /// * `dimensions` - Width, height, depth (depth usually 1)
    /// * `format` - Pixel format
    /// * `multi_sampled` - Whether to use multi-sampling
    ///
    /// # Returns
    ///
    /// True if allocation succeeded.
    pub fn allocate(&mut self, dimensions: Vec3i, format: HdFormat, multi_sampled: bool) -> bool {
        self.dimensions = dimensions;
        self.format = format;
        self.multi_sampled = multi_sampled;

        if multi_sampled {
            // Default to 4x MSAA
            self.msaa_sample_count = 4;
        } else {
            self.msaa_sample_count = 1;
        }

        // Calculate buffer size for CPU mapping
        let bytes_per_pixel = format.bytes_per_pixel();
        let size = (dimensions.x * dimensions.y * dimensions.z) as usize * bytes_per_pixel;
        self.mapped_buffer = vec![0u8; size];
        self.allocated = true;

        true
    }

    /// Deallocate the render buffer.
    pub fn deallocate(&mut self) {
        self.dimensions = Vec3i::new(0, 0, 1);
        self.mapped_buffer.clear();
        self.allocated = false;
    }

    /// Get width.
    pub fn get_width(&self) -> u32 {
        self.dimensions.x as u32
    }

    /// Get height.
    pub fn get_height(&self) -> u32 {
        self.dimensions.y as u32
    }

    /// Get depth.
    pub fn get_depth(&self) -> u32 {
        self.dimensions.z as u32
    }

    /// Get format.
    pub fn get_format(&self) -> HdFormat {
        self.format
    }

    /// Check if multi-sampled.
    pub fn is_multi_sampled(&self) -> bool {
        self.multi_sampled
    }

    /// Get MSAA sample count.
    pub fn get_msaa_sample_count(&self) -> u32 {
        self.msaa_sample_count
    }

    /// Set MSAA sample count.
    pub fn set_msaa_sample_count(&mut self, count: u32) {
        self.msaa_sample_count = count.max(1);
    }

    /// Map the buffer for reading.
    ///
    /// Returns a pointer to the buffer data.
    /// Call `unmap()` when done.
    pub fn map(&self) -> *const u8 {
        self.mappers.fetch_add(1, Ordering::SeqCst);
        self.mapped_buffer.as_ptr()
    }

    /// Map the buffer for writing.
    ///
    /// Returns a mutable pointer to the buffer data.
    /// Call `unmap()` when done.
    pub fn map_mut(&mut self) -> *mut u8 {
        self.mappers.fetch_add(1, Ordering::SeqCst);
        self.mapped_buffer.as_mut_ptr()
    }

    /// Unmap the buffer.
    pub fn unmap(&self) {
        let prev = self.mappers.fetch_sub(1, Ordering::SeqCst);
        debug_assert!(prev > 0, "Unmap called more times than Map");
    }

    /// Check if the buffer is currently mapped.
    pub fn is_mapped(&self) -> bool {
        self.mappers.load(Ordering::SeqCst) != 0
    }

    /// Check if the buffer is converged (not being rendered to).
    ///
    /// For Storm, render buffers are always considered converged.
    pub fn is_converged(&self) -> bool {
        true
    }

    /// Resolve multi-sample buffer to final values.
    ///
    /// This would copy MSAA texture to resolved texture.
    pub fn resolve(&mut self) {
        // In full implementation, would use HGI to resolve MSAA texture
    }

    /// Check if the buffer is allocated.
    pub fn is_allocated(&self) -> bool {
        self.allocated
    }

    /// Get the dimensions.
    pub fn get_dimensions(&self) -> Vec3i {
        self.dimensions
    }

    /// Sync with scene delegate.
    pub fn sync(&mut self) {
        // Would pull dimensions, format, MSAA settings from scene delegate
    }
}

/// Shared pointer to Storm render buffer.
pub type HdStRenderBufferSharedPtr = Arc<HdStRenderBuffer>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_buffer_creation() {
        let path = SdfPath::from_string("/renderBuffers/color").unwrap();
        let buffer = HdStRenderBuffer::new(path.clone());

        assert_eq!(buffer.get_path(), &path);
        assert!(!buffer.is_allocated());
        assert!(!buffer.is_mapped());
    }

    #[test]
    fn test_allocate() {
        let path = SdfPath::from_string("/renderBuffers/color").unwrap();
        let mut buffer = HdStRenderBuffer::new(path);

        let dims = Vec3i::new(1920, 1080, 1);
        let result = buffer.allocate(dims, HdFormat::UNorm8Vec4, false);

        assert!(result);
        assert!(buffer.is_allocated());
        assert_eq!(buffer.get_width(), 1920);
        assert_eq!(buffer.get_height(), 1080);
        assert_eq!(buffer.get_depth(), 1);
        assert_eq!(buffer.get_format(), HdFormat::UNorm8Vec4);
        assert!(!buffer.is_multi_sampled());
    }

    #[test]
    fn test_allocate_msaa() {
        let path = SdfPath::from_string("/renderBuffers/color").unwrap();
        let mut buffer = HdStRenderBuffer::new(path);

        let dims = Vec3i::new(1920, 1080, 1);
        buffer.allocate(dims, HdFormat::UNorm8Vec4, true);

        assert!(buffer.is_multi_sampled());
        assert_eq!(buffer.get_msaa_sample_count(), 4);
    }

    #[test]
    fn test_map_unmap() {
        let path = SdfPath::from_string("/renderBuffers/color").unwrap();
        let mut buffer = HdStRenderBuffer::new(path);

        buffer.allocate(Vec3i::new(100, 100, 1), HdFormat::UNorm8Vec4, false);

        assert!(!buffer.is_mapped());

        let _ptr = buffer.map();
        assert!(buffer.is_mapped());

        buffer.unmap();
        assert!(!buffer.is_mapped());
    }

    #[test]
    fn test_deallocate() {
        let path = SdfPath::from_string("/renderBuffers/color").unwrap();
        let mut buffer = HdStRenderBuffer::new(path);

        buffer.allocate(Vec3i::new(1920, 1080, 1), HdFormat::UNorm8Vec4, false);
        assert!(buffer.is_allocated());

        buffer.deallocate();
        assert!(!buffer.is_allocated());
        assert_eq!(buffer.get_width(), 0);
    }

    #[test]
    fn test_is_converged() {
        let path = SdfPath::from_string("/renderBuffers/color").unwrap();
        let buffer = HdStRenderBuffer::new(path);

        // Storm render buffers are always converged
        assert!(buffer.is_converged());
    }
}
