
//! GPU frustum culling for Storm indirect draw batches.
//!
//! Port of `HdSt_IndirectDrawBatch::_ExecuteFrustumCull` (non-instanced,
//! compute-shader path) from pxr/imaging/hdSt/indirectDrawBatch.cpp.
//!
//! ## Architecture (matching C++)
//!
//! The dispatch buffer IS the indirect draw argument buffer. The compute shader
//! writes `instance_count = 0` for culled items directly into this buffer.
//! The subsequent `draw_indexed_indirect` reads from the SAME buffer -- no CPU
//! readback needed. This is the core design of C++ Storm's GPU culling.
//!
//! ## Buffer Layout
//!
//! The dispatch buffer contains N entries, each `DRAW_CMD_NUM_UINTS` u32 words:
//!
//! ```text
//! [0]  index_count
//! [1]  instance_count  <-- written by culling shader (0 = culled)
//! [2]  base_index
//! [3]  base_vertex
//! [4]  base_instance
//! [5..14] DrawingCoord (not modified by culling)
//! ```
//!
//! ## GPU Buffers
//!
//! - **Dispatch buffer** (`STORAGE | INDIRECT | COPY_DST`): The indirect draw
//!   command buffer. Compute shader writes instance_count in-place. Same buffer
//!   passed to `draw_indexed_indirect()`.
//!
//! - **Cull input buffer** (`STORAGE | COPY_DST`): Read-only snapshot of the
//!   original instance counts. Copied from dispatch buffer when dirty.
//!
//! - **Item data buffer** (`STORAGE | COPY_DST`): Per-item model matrix + AABB.
//!   Updated each frame via `queue.write_buffer()`.
//!
//! ## Wgpu Binding Layout
//!
//! @group(0)
//!   binding(0) - CullParams uniform         (80 bytes)
//!   binding(1) - DrawCullInput storage RO   (instance count snapshot)
//!   binding(2) - DrawCommands storage RW    (dispatch buffer, in-place)
//!   binding(3) - ItemData storage RO        (per-item model + bbox)

use std::sync::Arc;
use usd_hgi::HgiBufferHandle;
use usd_hgi::HgiBufferUsage;
use usd_hgi::buffer::HgiBuffer;
use usd_hgi::buffer::HgiBufferDesc;
use usd_hgi::handle::HgiHandle;
use usd_hgi_wgpu::WgpuBuffer;
use wgpu::util::DeviceExt;

/// Number of u32 words in one draw command (DrawIndexedCommand + DrawingCoord).
pub const DRAW_CMD_NUM_UINTS: u32 = 15;

/// Per-draw-item data uploaded to the GPU each frame.
///
/// std430/std140 compatible layout: mat4 (64 bytes) + two vec4 (32 bytes) = 96 bytes.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GpuItemData {
    /// Model-to-world transform (row-major, row-vector convention).
    /// Stored column-major for WGSL mat4x4 (transpose of row-major matrix).
    pub model: [[f32; 4]; 4],
    /// Local AABB min corner (w=0 padding).
    pub bbox_min: [f32; 4],
    /// Local AABB max corner (w=0 padding).
    pub bbox_max: [f32; 4],
}

impl GpuItemData {
    /// Build from a row-major f64 model matrix and f32 bbox.
    pub fn new(model_f64: &[[f64; 4]; 4], bbox_min: [f32; 3], bbox_max: [f32; 3]) -> Self {
        // Convert f64 row-major to f32; WGSL mat4x4 is column-major so we transpose.
        let mut col_major = [[0.0f32; 4]; 4];
        for row in 0..4 {
            for col in 0..4 {
                col_major[col][row] = model_f64[row][col] as f32;
            }
        }
        Self {
            model: col_major,
            bbox_min: [bbox_min[0], bbox_min[1], bbox_min[2], 0.0],
            bbox_max: [bbox_max[0], bbox_max[1], bbox_max[2], 0.0],
        }
    }
}

/// Uniform block for the culling shader (std140, 80 bytes).
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct CullParams {
    /// Column-major view-projection cull matrix.
    cull_matrix: [[f32; 4]; 4],
    /// NDC draw range: x = min diagonal size, y = max (-1 = disabled).
    draw_range_ndc: [f32; 2],
    /// Command stride in u32 words.
    draw_cmd_num_uints: u32,
    _pad: u32,
}

/// Convert a row-major f64 4x4 matrix to column-major f32 for WGSL.
fn to_col_major_f32(m: &[[f64; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0f32; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[col][row] = m[row][col] as f32;
        }
    }
    out
}

/// Cast a `#[repr(C)]` struct to a byte slice (for GPU upload).
unsafe fn as_bytes<T: Sized>(val: &T) -> &[u8] {
    unsafe { std::slice::from_raw_parts(val as *const T as *const u8, std::mem::size_of::<T>()) }
}

/// Cast a `#[repr(C)]` slice to a byte slice (for GPU upload).
unsafe fn slice_as_bytes<T: Sized>(s: &[T]) -> &[u8] {
    unsafe {
        std::slice::from_raw_parts(s.as_ptr() as *const u8, s.len() * std::mem::size_of::<T>())
    }
}

// ---------------------------------------------------------------------------
// FrustumCullState -- compute pipeline + bind group layout (long-lived)
// ---------------------------------------------------------------------------

/// Cached wgpu resources for frustum-culling compute dispatches.
///
/// Kept on the `IndirectDrawBatch` to avoid recreating pipelines and bind
/// group layouts every frame. Only the data buffers are updated per-frame.
#[derive(Debug)]
pub struct FrustumCullState {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::ComputePipeline,
    bgl: wgpu::BindGroupLayout,
}

/// WGSL source embedded at compile time.
const CULL_WGSL: &str = include_str!("frustum_cull.wgsl");

impl FrustumCullState {
    /// Create the compute pipeline (done once per batch).
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Option<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("frustum_cull"),
            source: wgpu::ShaderSource::Wgsl(CULL_WGSL.into()),
        });

        // Bind group layout: 1 uniform + 3 storage buffers.
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("frustum_cull_bgl"),
            entries: &[
                // binding(0) - CullParams uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding(1) - DrawCullInput (read-only snapshot of instance counts)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding(2) - DrawCommands (read-write dispatch/indirect buffer)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding(3) - ItemData (per-item model + bbox, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("frustum_cull_layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("frustum_cull_pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        log::debug!("FrustumCullState: compute pipeline created");
        Some(Self {
            device,
            queue,
            pipeline,
            bgl,
        })
    }

    /// Dispatch GPU frustum culling in-place on the dispatch buffer.
    ///
    /// This is the correct C++-matching path: the compute shader writes
    /// `instance_count = 0` directly into the dispatch buffer. The same buffer
    /// is then passed to `draw_indexed_indirect()`. **No CPU readback.**
    ///
    /// # Arguments
    /// - `gpu_bufs`: persistent GPU buffers for this batch
    /// - `item_data`: per-draw-item model + bbox (uploaded to GPU this frame)
    /// - `cull_matrix`: row-major view-projection cull matrix
    /// - `draw_range_ndc`: (min_diag, max_diag) in NDC; (0.0, -1.0) disables tiny-prim
    pub fn dispatch(
        &self,
        gpu_bufs: &mut CullGpuBuffers,
        item_data: &[GpuItemData],
        cull_matrix: &[[f64; 4]; 4],
        draw_range_ndc: [f32; 2],
    ) {
        if item_data.is_empty() {
            return;
        }

        let dev = &self.device;
        let q = &self.queue;
        let item_count = item_data.len() as u32;

        // --- Update item data buffer ---
        let item_bytes = unsafe { slice_as_bytes(item_data) };
        let need_resize =
            gpu_bufs.item_data_buf.is_none() || gpu_bufs.item_data_capacity < item_data.len();
        if need_resize {
            gpu_bufs.item_data_buf =
                Some(dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("cull_item_data"),
                    contents: item_bytes,
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                }));
            gpu_bufs.item_data_capacity = item_data.len();
        } else {
            q.write_buffer(gpu_bufs.item_data_buf.as_ref().unwrap(), 0, item_bytes);
        }

        // --- Build and upload CullParams uniform ---
        let params = CullParams {
            cull_matrix: to_col_major_f32(cull_matrix),
            draw_range_ndc,
            draw_cmd_num_uints: DRAW_CMD_NUM_UINTS,
            _pad: 0,
        };
        let params_bytes = unsafe { as_bytes(&params) };
        if gpu_bufs.params_buf.is_none() {
            gpu_bufs.params_buf = Some(dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cull_params"),
                contents: params_bytes,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }));
        } else {
            q.write_buffer(gpu_bufs.params_buf.as_ref().unwrap(), 0, params_bytes);
        }

        // --- Snapshot: copy dispatch buffer -> cull_input before culling ---
        // This preserves original instance counts so culling can restore them
        // for visible items. The copy happens on the GPU command encoder.
        let Some(dispatch_wgpu) = gpu_bufs.get_wgpu_dispatch_buf() else {
            log::warn!("frustum_cull: dispatch buffer not ready, skipping");
            return;
        };
        let Some(cull_input) = gpu_bufs.cull_input_buf.as_ref() else {
            log::warn!("frustum_cull: cull_input buffer not ready, skipping");
            return;
        };

        let dispatch_size = gpu_bufs.dispatch_buf_size as u64;

        let mut encoder = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frustum_cull_enc"),
        });

        // Copy dispatch -> cull_input (GPU-side snapshot of original commands).
        encoder.copy_buffer_to_buffer(dispatch_wgpu, 0, cull_input, 0, dispatch_size);

        // --- Bind group ---
        let bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frustum_cull_bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu_bufs.params_buf.as_ref().unwrap().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cull_input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: dispatch_wgpu.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: gpu_bufs.item_data_buf.as_ref().unwrap().as_entire_binding(),
                },
            ],
        });

        // --- Dispatch compute ---
        let workgroup_size = 64u32;
        let num_workgroups = item_count.div_ceil(workgroup_size);

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("frustum_cull_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        // Submit. wgpu guarantees compute pass completes before any subsequent
        // render pass within the same command buffer or later submissions.
        q.submit(std::iter::once(encoder.finish()));

        log::debug!(
            "FrustumCullState::dispatch: {} items ({} workgroups), no CPU readback",
            item_count,
            num_workgroups,
        );
    }

    /// Legacy CPU-readback path. Kept as fallback for testing/debugging only.
    ///
    /// After the call, `draw_cmd_data[i * DRAW_CMD_NUM_UINTS + 1]` for culled items is 0.
    #[allow(dead_code)]
    pub fn execute_with_readback(
        &self,
        draw_cmd_data: &mut Vec<u32>,
        item_data: &[GpuItemData],
        cull_matrix_f64: &[[f64; 4]; 4],
        draw_range_ndc: [f32; 2],
    ) {
        if item_data.is_empty() || draw_cmd_data.is_empty() {
            return;
        }

        let dev = &self.device;
        let q = &self.queue;
        let item_count = item_data.len() as u32;

        let cull_col = to_col_major_f32(cull_matrix_f64);

        let params = CullParams {
            cull_matrix: cull_col,
            draw_range_ndc,
            draw_cmd_num_uints: DRAW_CMD_NUM_UINTS,
            _pad: 0,
        };
        let params_bytes = unsafe { as_bytes(&params) };
        let params_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cull_params"),
            contents: params_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let cmd_bytes: &[u8] = unsafe { slice_as_bytes(draw_cmd_data.as_slice()) };
        let cull_input_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cull_input"),
            contents: cmd_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let draw_cmds_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("draw_cmds_rw"),
            contents: cmd_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });

        let item_bytes = unsafe { slice_as_bytes(item_data) };
        let item_buf = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cull_item_data"),
            contents: item_bytes,
            usage: wgpu::BufferUsages::STORAGE,
        });

        let readback_buf = dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cull_readback"),
            size: cmd_bytes.len() as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bg = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("frustum_cull_bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: cull_input_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: draw_cmds_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: item_buf.as_entire_binding(),
                },
            ],
        });

        let workgroup_size = 64u32;
        let num_workgroups = item_count.div_ceil(workgroup_size);

        let mut encoder = dev.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frustum_cull_enc"),
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("frustum_cull_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        encoder.copy_buffer_to_buffer(&draw_cmds_buf, 0, &readback_buf, 0, cmd_bytes.len() as u64);
        q.submit(std::iter::once(encoder.finish()));

        let slice = readback_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = dev.poll(wgpu::PollType::wait_indefinitely());
        if rx.recv().map(|r| r.is_ok()).unwrap_or(false) {
            let mapped = slice.get_mapped_range();
            let result: &[u32] = unsafe {
                std::slice::from_raw_parts(
                    mapped.as_ptr() as *const u32,
                    mapped.len() / std::mem::size_of::<u32>(),
                )
            };
            draw_cmd_data.copy_from_slice(result);
        } else {
            log::warn!("FrustumCullState::execute_with_readback: readback map failed");
        }

        log::debug!(
            "FrustumCullState::execute_with_readback: {} items ({} workgroups)",
            item_count,
            num_workgroups
        );
    }
}

// ---------------------------------------------------------------------------
// CullGpuBuffers -- persistent GPU buffers for one batch
// ---------------------------------------------------------------------------

/// Persistent GPU buffers for frustum culling, owned by `IndirectDrawBatch`.
///
/// Port of the dispatch buffer + cull input buffer from C++
/// `HdSt_IndirectDrawBatch`. Buffers are reused across frames and only
/// recreated when the item count changes.
///
/// The dispatch buffer serves double duty:
/// 1. Compute shader reads/writes `instance_count` in-place
/// 2. `draw_indexed_indirect()` reads draw commands from it
///
/// The dispatch buffer is a `WgpuBuffer` (from usd-hgi-wgpu) so that
/// `resolve_buffer()` can extract the raw `wgpu::Buffer` for indirect draws.
#[derive(Debug)]
pub struct CullGpuBuffers {
    /// GPU dispatch buffer wrapped as HgiBufferHandle (STORAGE|INDIRECT|COPY_DST|COPY_SRC).
    /// Contains packed DrawIndexedCommand + DrawingCoord per item.
    /// Uses WgpuBuffer so resolve_buffer() works for draw_indexed_indirect.
    pub dispatch_handle: Option<HgiBufferHandle>,
    /// Size of the dispatch buffer in bytes.
    pub dispatch_buf_size: usize,
    /// Number of draw items the dispatch buffer was allocated for.
    pub dispatch_item_count: usize,

    /// Read-only snapshot of original instance counts (STORAGE | COPY_DST).
    pub cull_input_buf: Option<wgpu::Buffer>,

    /// Per-item model matrix + AABB (STORAGE | COPY_DST).
    pub item_data_buf: Option<wgpu::Buffer>,
    /// Number of items the item_data buffer was allocated for.
    pub item_data_capacity: usize,

    /// CullParams uniform buffer (UNIFORM | COPY_DST).
    pub params_buf: Option<wgpu::Buffer>,
}

/// Monotonically increasing ID for dispatch buffer handles.
static DISPATCH_BUF_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl CullGpuBuffers {
    /// Create empty (no GPU allocations yet).
    pub fn new() -> Self {
        Self {
            dispatch_handle: None,
            dispatch_buf_size: 0,
            dispatch_item_count: 0,
            cull_input_buf: None,
            item_data_buf: None,
            item_data_capacity: 0,
            params_buf: None,
        }
    }

    /// Allocate or resize the dispatch buffer + cull input buffer.
    ///
    /// Called when `compile_batch()` produces new draw commands. Uploads the
    /// command data to the dispatch buffer and creates a matching cull_input
    /// buffer for snapshotting.
    pub fn upload_dispatch(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        draw_cmd_data: &[u32],
        item_count: usize,
    ) {
        let cmd_bytes = unsafe { slice_as_bytes(draw_cmd_data) };
        let buf_size = cmd_bytes.len();

        // Recreate if item count changed (different buffer size needed).
        let need_recreate =
            self.dispatch_handle.is_none() || self.dispatch_item_count != item_count;

        if need_recreate {
            // Dispatch buffer via WgpuBuffer: the indirect draw command source
            // AND compute shader RW target. WgpuBuffer is used so that
            // resolve_buffer() can extract the wgpu::Buffer for indirect draws.
            let desc = HgiBufferDesc {
                debug_name: "cull_dispatch".to_string(),
                byte_size: buf_size,
                usage: HgiBufferUsage::STORAGE | HgiBufferUsage::INDIRECT,
                ..Default::default()
            };
            let wgpu_buf = WgpuBuffer::new(device, &desc, Some(cmd_bytes));

            // Cull input: same size, used as read-only snapshot during culling.
            self.cull_input_buf = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("cull_input_buf"),
                size: buf_size as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));

            // Wrap in HgiBufferHandle for draw_indexed_indirect compatibility.
            let id = DISPATCH_BUF_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.dispatch_handle =
                Some(HgiHandle::new(Arc::new(wgpu_buf) as Arc<dyn HgiBuffer>, id));

            self.dispatch_buf_size = buf_size;
            self.dispatch_item_count = item_count;

            log::debug!(
                "CullGpuBuffers: allocated dispatch+cull_input ({} items, {} bytes)",
                item_count,
                buf_size,
            );
        } else {
            // Same size: just update the data via write_buffer (no realloc).
            if let Some(wgpu_buf) = self.get_wgpu_dispatch_buf() {
                queue.write_buffer(wgpu_buf, 0, cmd_bytes);
            } else {
                log::warn!("upload_dispatch: dispatch buffer gone (device invalidated?)");
            }
        }
    }

    /// Get the raw wgpu::Buffer for the dispatch buffer (for compute shader binding).
    ///
    /// Downcasts the HgiBufferHandle to WgpuBuffer and returns the inner buffer.
    pub fn get_wgpu_dispatch_buf(&self) -> Option<&wgpu::Buffer> {
        self.dispatch_handle
            .as_ref()
            .and_then(|h| h.get())
            .and_then(|b| b.as_any().downcast_ref::<WgpuBuffer>())
            .map(|wb| wb.wgpu_buffer())
    }

    /// Get the HgiBufferHandle for the dispatch buffer (for draw_indexed_indirect).
    pub fn get_dispatch_handle(&self) -> Option<&HgiBufferHandle> {
        self.dispatch_handle.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_item_data_identity() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0f64],
        ];
        let item = GpuItemData::new(&id, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
        // Column-major: col[i][i] == 1 for identity.
        assert_eq!(item.model[0][0], 1.0);
        assert_eq!(item.model[1][1], 1.0);
        assert_eq!(item.model[2][2], 1.0);
        assert_eq!(item.model[3][3], 1.0);
        // Off-diagonal should be 0.
        assert_eq!(item.model[0][1], 0.0);
    }

    #[test]
    fn test_cull_params_size() {
        // CullParams must be 80 bytes (4x4 f32 matrix = 64 + 2 floats + 2 u32 = 80).
        assert_eq!(std::mem::size_of::<CullParams>(), 80);
    }

    #[test]
    fn test_gpu_item_data_size() {
        // mat4(64) + vec4_min(16) + vec4_max(16) = 96 bytes.
        assert_eq!(std::mem::size_of::<GpuItemData>(), 96);
    }

    #[test]
    fn test_draw_cmd_num_uints() {
        // Must match the stride assumed in draw_batch.rs compile_batch().
        assert_eq!(DRAW_CMD_NUM_UINTS, 15);
    }

    #[test]
    fn test_cull_gpu_buffers_new() {
        let bufs = CullGpuBuffers::new();
        assert!(bufs.dispatch_handle.is_none());
        assert!(bufs.cull_input_buf.is_none());
        assert!(bufs.item_data_buf.is_none());
        assert_eq!(bufs.dispatch_item_count, 0);
    }

    #[test]
    fn test_to_col_major_f32() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0f64],
        ];
        let col = to_col_major_f32(&id);
        for i in 0..4 {
            for j in 0..4 {
                let expected = if i == j { 1.0f32 } else { 0.0 };
                assert_eq!(col[i][j], expected);
            }
        }
    }
}
