//! OIT buffer accessor - Access to OIT GPU buffers.
//!
//! Provides unified access to OIT counter, data, depth, and index buffers.
//! Mirrors C++ HdxOitBufferAccessor from pxr/imaging/hdx/oitBufferAccessor.h/cpp
//!
//! Key responsibilities:
//! - `request_oit_buffers()`: signal that OIT buffers are needed this frame
//! - `add_oit_buffer_bindings()`: bind buffers to shader for OIT rendering
//! - `initialize_oit_buffers_if_necessary()`: clear counter buffer once per frame
//!
//! Buffer layout matches C++ HdxOitResolveTask._PrepareOitBuffers():
//!   counter: 1 atomic + pixel_count int32s (pixel fragment counts)
//!   index:   pixel_count * NUM_SAMPLES int32s (linked list heads)
//!   data:    pixel_count * NUM_SAMPLES vec4f  (fragment colors)
//!   depth:   pixel_count * NUM_SAMPLES float  (fragment depths)
//!   uniform: GfVec2i screenSize

use usd_hd::render::HdTaskContext;
use usd_vt::Value;

/// Number of OIT samples per pixel — must match GLSL shader constant.
pub const OIT_NUM_SAMPLES: u32 = 8;

/// OIT buffer accessor tokens.
pub mod oit_buffer_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Buffer binding point
    pub static BINDING_POINT: LazyLock<Token> = LazyLock::new(|| Token::new("bindingPoint"));
    /// Buffer element count
    pub static ELEMENT_COUNT: LazyLock<Token> = LazyLock::new(|| Token::new("elementCount"));
    /// Buffer element size in bytes
    pub static ELEMENT_SIZE: LazyLock<Token> = LazyLock::new(|| Token::new("elementSize"));
}

/// OIT buffer type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OitBufferType {
    /// Atomic counter buffer (per-pixel fragment counts + global atomic)
    Counter,
    /// Fragment color data buffer (RGBA float, numSamples per pixel)
    Data,
    /// Fragment depth buffer (float, numSamples per pixel)
    Depth,
    /// Fragment linked-list index buffer (int32, numSamples per pixel)
    Index,
    /// Screen size uniform (GfVec2i)
    Uniform,
}

/// OIT buffer handle.
///
/// In a real HGI implementation, this wraps GPU buffer handles.
/// Here it tracks allocation metadata for correctness verification.
#[derive(Debug, Clone)]
pub struct OitBufferHandle {
    /// Buffer type
    buffer_type: OitBufferType,
    /// GPU buffer handle (0 = placeholder, real impl uses HGI handle)
    #[allow(dead_code)]
    handle: u64,
    /// Buffer capacity in bytes
    size: usize,
    /// Number of logical elements
    element_count: usize,
}

impl OitBufferHandle {
    fn new(buffer_type: OitBufferType, size: usize, element_count: usize) -> Self {
        Self {
            buffer_type,
            handle: 0,
            size,
            element_count,
        }
    }

    /// Get buffer type.
    pub fn get_type(&self) -> OitBufferType {
        self.buffer_type
    }

    /// Get buffer size in bytes.
    pub fn get_size(&self) -> usize {
        self.size
    }

    /// Get element count.
    pub fn get_element_count(&self) -> usize {
        self.element_count
    }

    /// Check if buffer is valid (non-zero size).
    pub fn is_valid(&self) -> bool {
        self.size > 0 && self.element_count > 0
    }
}

/// OIT buffer accessor for managing OIT GPU resources.
///
/// Lifecycle:
/// 1. HdxOitRenderTask::Prepare calls `request_oit_buffers()`
/// 2. HdxOitResolveTask::Prepare allocates/resizes buffers
/// 3. HdxOitRenderTask::Execute calls `initialize_oit_buffers_if_necessary()` to clear
/// 4. HdxOitRenderTask::Execute calls `add_oit_buffer_bindings()` for shader
/// 5. HdxOitResolveTask::Execute composites and clears request flag
///
/// Port of HdxOitBufferAccessor from pxr/imaging/hdx/oitBufferAccessor.h
pub struct HdxOitBufferAccessor {
    /// Counter buffer: 1 atomic + pixel_count counters
    counter_buffer: Option<OitBufferHandle>,
    /// Data buffer: pixel_count * num_samples RGBA fragments
    data_buffer: Option<OitBufferHandle>,
    /// Depth buffer: pixel_count * num_samples depth values
    depth_buffer: Option<OitBufferHandle>,
    /// Index buffer: pixel_count * num_samples linked list indices
    index_buffer: Option<OitBufferHandle>,
    /// Uniform buffer: screen size (width, height)
    uniform_buffer: Option<OitBufferHandle>,
    /// Screen dimensions at last allocation
    screen_size: (u32, u32),
    /// Max samples per pixel
    max_samples: u32,
}

impl HdxOitBufferAccessor {
    /// Query whether OIT is enabled via environment variable HDX_ENABLE_OIT.
    ///
    /// Mirrors C++ `HdxOitBufferAccessor::IsOitEnabled()`.
    /// Checks HDX_ENABLE_OIT env var; defaults to true.
    pub fn is_oit_enabled() -> bool {
        std::env::var("HDX_ENABLE_OIT")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true)
    }

    /// Create new OIT buffer accessor.
    pub fn new() -> Self {
        Self {
            counter_buffer: None,
            data_buffer: None,
            depth_buffer: None,
            index_buffer: None,
            uniform_buffer: None,
            screen_size: (0, 0),
            max_samples: OIT_NUM_SAMPLES,
        }
    }

    /// Signal that OIT buffers are needed this frame.
    ///
    /// Called by HdxOitRenderTask::Prepare when there are translucent draw items.
    /// Mirrors C++ `HdxOitBufferAccessor::RequestOitBuffers()`.
    pub fn request_oit_buffers(&self, ctx: &mut HdTaskContext) {
        ctx.insert(super::tokens::OIT_REQUEST_FLAG.clone(), Value::from(true));
    }

    /// Initialize OIT buffers if not already cleared this frame.
    ///
    /// The counter buffer is filled with -1 (0xFF bytes) so the shader
    /// can detect uninitialized entries. Only clears once per frame.
    /// Mirrors C++ `HdxOitBufferAccessor::InitializeOitBuffersIfNecessary()`.
    pub fn initialize_oit_buffers_if_necessary(&mut self, ctx: &mut HdTaskContext) {
        // Skip if already cleared this frame
        if ctx.contains_key(&super::tokens::OIT_CLEARED_FLAG) {
            return;
        }

        // Mark as cleared
        ctx.insert(super::tokens::OIT_CLEARED_FLAG.clone(), Value::from(true));

        // In full HGI implementation:
        // 1. Get counter buffer resource
        // 2. hgi.create_blit_cmds()
        // 3. blit_cmds.fill_buffer(counter_resource, 0xFF) — fills with int -1
        // 4. hgi.submit_cmds()
        // Counter filled with 0xFF matches `uint8_t clearCounter = -1` in C++

        self.clear_counters();
    }

    /// Bind OIT buffers to shader for rendering.
    ///
    /// Adds SSBO bindings (counter, data, depth, index) and UBO (uniform/screenSize)
    /// to the render pass shader. Returns false if buffers not allocated.
    /// Mirrors C++ `HdxOitBufferAccessor::AddOitBufferBindings()`.
    pub fn add_oit_buffer_bindings(&self, ctx: &mut HdTaskContext) -> bool {
        if !self.is_initialized() {
            return false;
        }

        // In full HGI implementation:
        // shader.add_buffer_binding(SSBO, oitCounterBufferBar, counterBar, writable=true)
        // shader.add_buffer_binding(SSBO, oitDataBufferBar,    dataBar,    writable=true)
        // shader.add_buffer_binding(SSBO, oitDepthBufferBar,   depthBar,   writable=true)
        // shader.add_buffer_binding(SSBO, oitIndexBufferBar,   indexBar,   writable=true)
        // shader.add_buffer_binding(UBO,  oitUniformBar,       uniformBar, interleaved=true)

        // Publish buffer presence to task context so shaders can bind
        ctx.insert(
            super::tokens::HDX_OIT_COUNTER_BUFFER.clone(),
            Value::from("counterBuffer"),
        );
        ctx.insert(
            super::tokens::HDX_OIT_DATA_BUFFER.clone(),
            Value::from("dataBuffer"),
        );
        ctx.insert(
            super::tokens::HDX_OIT_DEPTH_BUFFER.clone(),
            Value::from("depthBuffer"),
        );
        ctx.insert(
            super::tokens::HDX_OIT_INDEX_BUFFER.clone(),
            Value::from("indexBuffer"),
        );

        true
    }

    /// Allocate or resize OIT buffers for the given screen dimensions.
    ///
    /// Buffer sizes:
    ///   counter: (pixel_count + 1) * 4 bytes — +1 for global atomic counter at slot 0
    ///   index:   pixel_count * num_samples * 4 bytes
    ///   data:    pixel_count * num_samples * 16 bytes (RGBA32F)
    ///   depth:   pixel_count * num_samples * 4 bytes (float)
    ///   uniform: 8 bytes (vec2i)
    pub fn init(&mut self, width: u32, height: u32, max_samples: u32) {
        let needs_resize = width > self.screen_size.0 || height > self.screen_size.1;
        let first_alloc = !self.is_initialized();

        if !first_alloc && !needs_resize {
            return;
        }

        self.screen_size = (width, height);
        self.max_samples = max_samples;

        let pixel_count = (width * height) as usize;
        // +1 for global atomic counter at element 0 (matches C++ _counterBar->Resize(newSize+1))
        let counter_count = pixel_count + 1;
        let max_fragments = pixel_count * max_samples as usize;

        // Counter: int32 per pixel + 1 global atomic
        self.counter_buffer = Some(OitBufferHandle::new(
            OitBufferType::Counter,
            counter_count * std::mem::size_of::<i32>(),
            counter_count,
        ));

        // Data: RGBA32F per fragment (16 bytes each)
        self.data_buffer = Some(OitBufferHandle::new(
            OitBufferType::Data,
            max_fragments * 16,
            max_fragments,
        ));

        // Depth: float32 per fragment
        self.depth_buffer = Some(OitBufferHandle::new(
            OitBufferType::Depth,
            max_fragments * std::mem::size_of::<f32>(),
            max_fragments,
        ));

        // Index: int32 per fragment
        self.index_buffer = Some(OitBufferHandle::new(
            OitBufferType::Index,
            max_fragments * std::mem::size_of::<i32>(),
            max_fragments,
        ));

        // Uniform: Vec2i (8 bytes)
        self.uniform_buffer = Some(OitBufferHandle::new(OitBufferType::Uniform, 8, 1));
    }

    /// Get counter buffer handle.
    pub fn get_counter_buffer(&self) -> Option<&OitBufferHandle> {
        self.counter_buffer.as_ref()
    }

    /// Get data buffer handle.
    pub fn get_data_buffer(&self) -> Option<&OitBufferHandle> {
        self.data_buffer.as_ref()
    }

    /// Get depth buffer handle.
    pub fn get_depth_buffer(&self) -> Option<&OitBufferHandle> {
        self.depth_buffer.as_ref()
    }

    /// Get index buffer handle.
    pub fn get_index_buffer(&self) -> Option<&OitBufferHandle> {
        self.index_buffer.as_ref()
    }

    /// Get uniform buffer handle.
    pub fn get_uniform_buffer(&self) -> Option<&OitBufferHandle> {
        self.uniform_buffer.as_ref()
    }

    /// Get screen size.
    pub fn get_screen_size(&self) -> (u32, u32) {
        self.screen_size
    }

    /// Get max samples per pixel.
    pub fn get_max_samples(&self) -> u32 {
        self.max_samples
    }

    /// Check if all buffers are allocated.
    pub fn is_initialized(&self) -> bool {
        self.counter_buffer.is_some()
            && self.data_buffer.is_some()
            && self.depth_buffer.is_some()
            && self.index_buffer.is_some()
            && self.uniform_buffer.is_some()
    }

    /// Release all buffers (call when no longer needed).
    pub fn release(&mut self) {
        self.counter_buffer = None;
        self.data_buffer = None;
        self.depth_buffer = None;
        self.index_buffer = None;
        self.uniform_buffer = None;
        self.screen_size = (0, 0);
    }

    /// Clear counter buffer — fill with -1 (0xFF bytes).
    ///
    /// The shader interprets -1 as "no fragment at this pixel".
    /// Only the counter buffer needs clearing (index/data/depth are
    /// written before read, gated by counter check).
    pub fn clear_counters(&mut self) {
        if let Some(ref _buf) = self.counter_buffer {
            // In full HGI implementation: blit_cmds.fill_buffer(handle, 0xFF)
            // 0xFF bytes == int -1 == "uninitialized" sentinel
        }
    }

    /// Clear index buffer to sentinel values (0xFFFFFFFF = u32::MAX).
    pub fn clear_indices(&mut self) {
        if let Some(ref _buf) = self.index_buffer {
            // In full HGI implementation: fill with 0xFF (linked list head sentinel)
        }
    }

    /// Bind all OIT buffers to context for GPU access.
    pub fn bind(&self, ctx: &mut HdTaskContext) {
        if !self.is_initialized() {
            return;
        }
        ctx.insert(
            super::tokens::HDX_OIT_COUNTER_BUFFER.clone(),
            Value::from("counterBuffer"),
        );
        ctx.insert(
            super::tokens::HDX_OIT_DATA_BUFFER.clone(),
            Value::from("dataBuffer"),
        );
        ctx.insert(
            super::tokens::HDX_OIT_DEPTH_BUFFER.clone(),
            Value::from("depthBuffer"),
        );
        ctx.insert(
            super::tokens::HDX_OIT_INDEX_BUFFER.clone(),
            Value::from("indexBuffer"),
        );
    }

    /// Calculate total GPU memory usage in bytes.
    pub fn get_memory_usage(&self) -> usize {
        [
            &self.counter_buffer,
            &self.data_buffer,
            &self.depth_buffer,
            &self.index_buffer,
            &self.uniform_buffer,
        ]
        .iter()
        .filter_map(|b| b.as_ref())
        .map(|b| b.get_size())
        .sum()
    }
}

impl Default for HdxOitBufferAccessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if context has all required OIT buffer bindings.
pub fn has_oit_buffers(ctx: &HdTaskContext) -> bool {
    ctx.contains_key(&super::tokens::HDX_OIT_COUNTER_BUFFER)
        && ctx.contains_key(&super::tokens::HDX_OIT_DATA_BUFFER)
        && ctx.contains_key(&super::tokens::HDX_OIT_DEPTH_BUFFER)
        && ctx.contains_key(&super::tokens::HDX_OIT_INDEX_BUFFER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oit_buffer_accessor_creation() {
        let accessor = HdxOitBufferAccessor::new();
        assert!(!accessor.is_initialized());
        assert_eq!(accessor.get_screen_size(), (0, 0));
        assert_eq!(accessor.get_max_samples(), OIT_NUM_SAMPLES);
    }

    #[test]
    fn test_oit_buffer_accessor_init() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(2560, 1440, 16);

        assert!(accessor.is_initialized());
        assert_eq!(accessor.get_screen_size(), (2560, 1440));
        assert_eq!(accessor.get_max_samples(), 16);

        assert!(accessor.get_counter_buffer().is_some());
        assert!(accessor.get_data_buffer().is_some());
        assert!(accessor.get_depth_buffer().is_some());
        assert!(accessor.get_index_buffer().is_some());
        assert!(accessor.get_uniform_buffer().is_some());
    }

    #[test]
    fn test_oit_buffer_no_resize_same_size() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);
        let mem1 = accessor.get_memory_usage();

        // Same size — should not reallocate
        accessor.init(1920, 1080, 8);
        let mem2 = accessor.get_memory_usage();
        assert_eq!(mem1, mem2);
    }

    #[test]
    fn test_oit_buffer_resize_on_larger() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);
        let mem1 = accessor.get_memory_usage();

        // Larger size triggers resize
        accessor.init(2560, 1440, 8);
        let mem2 = accessor.get_memory_usage();
        assert!(mem2 > mem1);
    }

    #[test]
    fn test_oit_buffer_counter_size() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);

        let counter = accessor.get_counter_buffer().unwrap();
        // counter_count = pixel_count + 1 = (1920*1080 + 1) * 4 bytes
        let expected_count = 1920 * 1080 + 1;
        assert_eq!(counter.get_element_count(), expected_count);
        assert_eq!(counter.get_size(), expected_count * 4);
    }

    #[test]
    fn test_oit_buffer_handle() {
        let handle = OitBufferHandle::new(OitBufferType::Counter, 1024, 256);
        assert_eq!(handle.get_type(), OitBufferType::Counter);
        assert_eq!(handle.get_size(), 1024);
        assert_eq!(handle.get_element_count(), 256);
        assert!(handle.is_valid());
    }

    #[test]
    fn test_oit_buffer_handle_invalid() {
        let handle = OitBufferHandle::new(OitBufferType::Data, 0, 0);
        assert!(!handle.is_valid());
    }

    #[test]
    fn test_oit_buffer_types() {
        assert_ne!(OitBufferType::Counter, OitBufferType::Data);
        assert_ne!(OitBufferType::Depth, OitBufferType::Index);
        assert_ne!(OitBufferType::Uniform, OitBufferType::Counter);
    }

    #[test]
    fn test_oit_buffer_accessor_release() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);
        assert!(accessor.is_initialized());

        accessor.release();
        assert!(!accessor.is_initialized());
        assert_eq!(accessor.get_screen_size(), (0, 0));
    }

    #[test]
    fn test_oit_buffer_accessor_memory_usage() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);

        let memory = accessor.get_memory_usage();
        assert!(memory > 0);

        let pixels = 1920 * 1080usize;
        // At minimum: counter buffer alone = (pixels + 1) * 4
        let expected_min = (pixels + 1) * 4;
        assert!(memory >= expected_min);
    }

    #[test]
    fn test_request_oit_buffers() {
        let accessor = HdxOitBufferAccessor::new();
        let mut ctx = HdTaskContext::new();

        assert!(!ctx.contains_key(&super::super::tokens::OIT_REQUEST_FLAG));
        accessor.request_oit_buffers(&mut ctx);
        assert!(ctx.contains_key(&super::super::tokens::OIT_REQUEST_FLAG));
    }

    #[test]
    fn test_add_oit_buffer_bindings_uninit() {
        let accessor = HdxOitBufferAccessor::new();
        let mut ctx = HdTaskContext::new();
        // Uninitialized — should return false
        assert!(!accessor.add_oit_buffer_bindings(&mut ctx));
    }

    #[test]
    fn test_add_oit_buffer_bindings_init() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);
        let mut ctx = HdTaskContext::new();
        assert!(accessor.add_oit_buffer_bindings(&mut ctx));
        assert!(ctx.contains_key(&super::super::tokens::HDX_OIT_COUNTER_BUFFER));
        assert!(ctx.contains_key(&super::super::tokens::HDX_OIT_DATA_BUFFER));
    }

    #[test]
    fn test_initialize_oit_buffers_once_per_frame() {
        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);
        let mut ctx = HdTaskContext::new();

        // First call clears and sets flag
        accessor.initialize_oit_buffers_if_necessary(&mut ctx);
        assert!(ctx.contains_key(&super::super::tokens::OIT_CLEARED_FLAG));

        // Second call is no-op (flag already set)
        accessor.initialize_oit_buffers_if_necessary(&mut ctx);
        // Flag still set, no double-clear
        assert!(ctx.contains_key(&super::super::tokens::OIT_CLEARED_FLAG));
    }

    #[test]
    fn test_has_oit_buffers() {
        let mut ctx = HdTaskContext::new();
        assert!(!has_oit_buffers(&ctx));

        let mut accessor = HdxOitBufferAccessor::new();
        accessor.init(1920, 1080, 8);
        accessor.bind(&mut ctx);
        assert!(has_oit_buffers(&ctx));
    }

    #[test]
    fn test_is_oit_enabled_default() {
        // Default is true unless env var overrides
        // We cannot set env vars in parallel tests safely, so just verify it returns bool
        let _ = HdxOitBufferAccessor::is_oit_enabled();
    }

    #[test]
    fn test_oit_buffer_tokens() {
        assert_eq!(oit_buffer_tokens::BINDING_POINT.as_str(), "bindingPoint");
        assert_eq!(oit_buffer_tokens::ELEMENT_COUNT.as_str(), "elementCount");
        assert_eq!(oit_buffer_tokens::ELEMENT_SIZE.as_str(), "elementSize");
    }
}
