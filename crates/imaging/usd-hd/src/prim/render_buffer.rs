
//! HdRenderBuffer - Buffer primitive for render targets.
//!
//! Represents a 2D or 3D buffer for rendering output. Used for:
//! - Color buffers (beauty, albedo, etc)
//! - Depth buffers
//! - ID buffers (primId, instanceId, elementId)
//! - AOVs (arbitrary output variables)
//!
//! # Buffer Types
//!
//! Buffers can store various data types:
//! - Color (RGBA, float or uint8)
//! - Depth (float32)
//! - Integer IDs (int32)
//! - Custom formats

use super::{HdBprim, HdRenderParam, HdSceneDelegate};
use crate::types::{HdDirtyBits, HdType};
use usd_sdf::Path as SdfPath;

/// Render buffer format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdRenderBufferFormat {
    /// RGBA 8-bit unsigned normalized.
    UNorm8,

    /// RGBA 8-bit signed normalized.
    SNorm8,

    /// RGBA 16-bit float.
    Float16,

    /// RGBA 32-bit float.
    Float32,

    /// Single channel 32-bit int.
    Int32,

    /// Single channel 32-bit uint.
    UInt32,
}

impl HdRenderBufferFormat {
    /// Get component type.
    pub fn component_type(self) -> HdType {
        match self {
            Self::UNorm8 | Self::SNorm8 => HdType::UInt8,
            Self::Float16 => HdType::HalfFloat,
            Self::Float32 => HdType::Float,
            Self::Int32 => HdType::Int32,
            Self::UInt32 => HdType::UInt32,
        }
    }

    /// Get bytes per pixel.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Self::UNorm8 | Self::SNorm8 => 4,
            Self::Float16 => 8,
            Self::Float32 => 16,
            Self::Int32 | Self::UInt32 => 4,
        }
    }
}

/// Render buffer dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HdRenderBufferDimensions {
    /// Buffer width in pixels.
    pub width: u32,
    /// Buffer height in pixels.
    pub height: u32,
    /// Buffer depth (1 for 2D).
    pub depth: u32,
}

impl HdRenderBufferDimensions {
    /// Create 2D buffer dimensions.
    pub fn new_2d(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            depth: 1,
        }
    }

    /// Create 3D buffer dimensions.
    pub fn new_3d(width: u32, height: u32, depth: u32) -> Self {
        Self {
            width,
            height,
            depth,
        }
    }

    /// Get total pixel count.
    pub fn pixel_count(&self) -> usize {
        (self.width as usize) * (self.height as usize) * (self.depth as usize)
    }
}

/// Render buffer primitive.
///
/// Represents a buffer for render output (color, depth, AOVs).
#[derive(Debug)]
pub struct HdRenderBuffer {
    /// Prim identifier.
    id: SdfPath,

    /// Current dirty bits.
    dirty_bits: HdDirtyBits,

    /// Buffer dimensions.
    dimensions: HdRenderBufferDimensions,

    /// Buffer format.
    format: HdRenderBufferFormat,

    /// Multi-sample count.
    multi_sample_count: u32,
}

impl HdRenderBuffer {
    /// Create a new render buffer.
    pub fn new(
        id: SdfPath,
        dimensions: HdRenderBufferDimensions,
        format: HdRenderBufferFormat,
    ) -> Self {
        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            dimensions,
            format,
            multi_sample_count: 1,
        }
    }

    /// Get buffer dimensions.
    pub fn get_dimensions(&self) -> HdRenderBufferDimensions {
        self.dimensions
    }

    /// Set buffer dimensions.
    pub fn set_dimensions(&mut self, dimensions: HdRenderBufferDimensions) {
        if self.dimensions != dimensions {
            self.dimensions = dimensions;
            self.mark_dirty(Self::DIRTY_PARAMS);
        }
    }

    /// Get buffer format.
    pub fn get_format(&self) -> HdRenderBufferFormat {
        self.format
    }

    /// Get buffer size in bytes.
    pub fn get_size_bytes(&self) -> usize {
        self.dimensions.pixel_count() * self.format.bytes_per_pixel()
    }

    /// Get multi-sample count.
    pub fn get_multi_sample_count(&self) -> u32 {
        self.multi_sample_count
    }

    /// Set multi-sample count.
    pub fn set_multi_sample_count(&mut self, count: u32) {
        if self.multi_sample_count != count {
            self.multi_sample_count = count;
            self.mark_dirty(Self::DIRTY_PARAMS);
        }
    }
}

impl HdBprim for HdRenderBuffer {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn sync(
        &mut self,
        _delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        if (*dirty_bits & Self::DIRTY_PARAMS) != 0 {
            // Reallocate buffer if size/format changed
        }

        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_buffer_creation() {
        let id = SdfPath::from_string("/RenderBuffer").unwrap();
        let dims = HdRenderBufferDimensions::new_2d(1920, 1080);
        let buffer = HdRenderBuffer::new(id.clone(), dims, HdRenderBufferFormat::Float32);

        assert_eq!(buffer.get_id(), &id);
        assert_eq!(buffer.get_dimensions().width, 1920);
        assert_eq!(buffer.get_dimensions().height, 1080);
    }

    #[test]
    fn test_buffer_dimensions() {
        let dims_2d = HdRenderBufferDimensions::new_2d(1920, 1080);
        assert_eq!(dims_2d.pixel_count(), 1920 * 1080);

        let dims_3d = HdRenderBufferDimensions::new_3d(512, 512, 256);
        assert_eq!(dims_3d.pixel_count(), 512 * 512 * 256);
    }

    #[test]
    fn test_buffer_format() {
        assert_eq!(HdRenderBufferFormat::Float32.bytes_per_pixel(), 16);
        assert_eq!(HdRenderBufferFormat::UNorm8.bytes_per_pixel(), 4);
    }

    #[test]
    fn test_buffer_size() {
        let dims = HdRenderBufferDimensions::new_2d(1920, 1080);
        let buffer = HdRenderBuffer::new(
            SdfPath::from_string("/Buffer").unwrap(),
            dims,
            HdRenderBufferFormat::Float32,
        );

        let expected_size = 1920 * 1080 * 16; // width * height * bytes_per_pixel
        assert_eq!(buffer.get_size_bytes(), expected_size);
    }

    #[test]
    fn test_multisampling() {
        let mut buffer = HdRenderBuffer::new(
            SdfPath::from_string("/Buffer").unwrap(),
            HdRenderBufferDimensions::new_2d(1920, 1080),
            HdRenderBufferFormat::Float32,
        );

        assert_eq!(buffer.get_multi_sample_count(), 1);

        buffer.set_multi_sample_count(4);
        assert_eq!(buffer.get_multi_sample_count(), 4);
        assert!(buffer.is_dirty_bits(HdRenderBuffer::DIRTY_PARAMS));
    }
}
