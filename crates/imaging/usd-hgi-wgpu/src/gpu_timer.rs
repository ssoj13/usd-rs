//! GPU timestamp query timer for frame profiling.
//!
//! Uses wgpu TIMESTAMP_QUERY feature to measure GPU execution time.
//! Falls back gracefully when the feature is unavailable.

/// GPU frame timer using wgpu timestamp queries.
pub struct GpuTimer {
    /// Whether timestamp queries are available on this device.
    available: bool,
    /// Query set for begin/end timestamps (2 queries).
    query_set: Option<wgpu::QuerySet>,
    /// Buffer for resolving query results (2 x u64 = 16 bytes).
    resolve_buf: Option<wgpu::Buffer>,
    /// Buffer for reading results back to CPU.
    readback_buf: Option<wgpu::Buffer>,
    /// Last measured GPU frame time in milliseconds.
    last_gpu_ms: f64,
    /// Timestamp period (nanoseconds per tick).
    timestamp_period: f32,
    /// Whether a readback is pending (map requested but not yet read).
    pending_readback: bool,
}

impl GpuTimer {
    /// Create timer. Checks device features for TIMESTAMP_QUERY support.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let available = device.features().contains(wgpu::Features::TIMESTAMP_QUERY);
        if !available {
            log::info!("[GpuTimer] TIMESTAMP_QUERY not available, GPU timing disabled");
            return Self {
                available: false,
                query_set: None,
                resolve_buf: None,
                readback_buf: None,
                last_gpu_ms: 0.0,
                timestamp_period: 1.0,
                pending_readback: false,
            };
        }

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("gpu_timer_queries"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });

        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_resolve"),
            size: 16, // 2 x u64
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_timer_readback"),
            size: 16,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let timestamp_period = queue.get_timestamp_period();
        log::info!(
            "[GpuTimer] initialized, timestamp_period={:.3}ns/tick",
            timestamp_period
        );

        Self {
            available: true,
            query_set: Some(query_set),
            resolve_buf: Some(resolve_buf),
            readback_buf: Some(readback_buf),
            last_gpu_ms: 0.0,
            timestamp_period,
            pending_readback: false,
        }
    }

    /// Whether GPU timing is available on this device.
    pub fn is_available(&self) -> bool {
        self.available
    }

    /// Write begin timestamp into command encoder.
    pub fn begin_frame(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(qs) = &self.query_set {
            encoder.write_timestamp(qs, 0);
        }
    }

    /// Write end timestamp, resolve queries, and copy to readback buffer.
    pub fn end_frame(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let (Some(qs), Some(resolve)) = (&self.query_set, &self.resolve_buf) {
            encoder.write_timestamp(qs, 1);
            encoder.resolve_query_set(qs, 0..2, resolve, 0);
            if let Some(readback) = &self.readback_buf {
                encoder.copy_buffer_to_buffer(resolve, 0, readback, 0, 16);
            }
            self.pending_readback = true;
        }
    }

    /// Attempt to read GPU time from previous frame's readback buffer.
    ///
    /// Uses a callback-based async map. The result is available on the
    /// NEXT call after the GPU has finished and the map callback fires.
    /// Returns the cached last_gpu_ms (may be from 1-2 frames ago).
    pub fn poll_gpu_time_ms(&mut self, device: &wgpu::Device) -> f64 {
        if !self.available || !self.pending_readback {
            return self.last_gpu_ms;
        }

        if let Some(buf) = &self.readback_buf {
            let slice = buf.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });

            // Poll device once (non-blocking on most backends)
            let _ = device.poll(wgpu::PollType::Poll);

            // Check if mapping completed
            if let Ok(Ok(())) = rx.try_recv() {
                let data = slice.get_mapped_range();
                if data.len() >= 16 {
                    let timestamps: &[u64] = bytemuck::cast_slice(&data);
                    let begin = timestamps[0];
                    let end = timestamps[1];
                    let delta_ticks = end.saturating_sub(begin);
                    self.last_gpu_ms =
                        (delta_ticks as f64 * self.timestamp_period as f64) / 1_000_000.0;
                }
                drop(data);
                buf.unmap();
                self.pending_readback = false;
            }
            // If not ready yet, keep pending_readback=true and return cached value
        }
        self.last_gpu_ms
    }

    /// Last GPU frame time in ms (cached, does not trigger readback).
    pub fn last_gpu_ms(&self) -> f64 {
        self.last_gpu_ms
    }
}
