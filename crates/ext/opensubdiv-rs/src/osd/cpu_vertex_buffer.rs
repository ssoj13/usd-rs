/// CPU vertex buffer backed by a flat `Vec<f32>`.
///
/// Mirrors `CpuVertexBuffer` from OpenSubdiv 3.7.0 `osd/cpuVertexBuffer.h/.cpp`.
/// Implements the `VertexBuffer` trait required by `CpuEvaluator`.

// ---------------------------------------------------------------------------
//  VertexBuffer trait
// ---------------------------------------------------------------------------

/// Trait replacing C++ duck-typing for vertex buffer objects accepted by
/// evaluators.  Any type that can bind a CPU float slice satisfies this.
pub trait VertexBuffer {
    /// Returns a shared slice over the raw float data.
    fn bind_cpu_buffer(&self) -> &[f32];

    /// Returns a mutable slice over the raw float data.
    fn bind_cpu_buffer_mut(&mut self) -> &mut [f32];

    /// Number of float elements per vertex.
    fn num_elements(&self) -> i32;

    /// Total number of vertices in this buffer.
    fn num_vertices(&self) -> i32;

    /// Upload `src` data starting at `start_vertex`.
    ///
    /// Mirrors `CpuVertexBuffer::UpdateData(src, startVertex, numVerts)`.
    fn update_data(&mut self, src: &[f32], start_vertex: i32, num_verts: i32);
}

// ---------------------------------------------------------------------------
//  CpuVertexBuffer
// ---------------------------------------------------------------------------

/// Concrete CPU vertex buffer backed by a `Vec<f32>`.
///
/// Mirrors `Osd::CpuVertexBuffer`.  Layout: tightly packed, no padding.
///
/// ```text
/// [ elem0_v0 elem1_v0 … elemN_v0 | elem0_v1 … ]
/// ```
#[derive(Debug, Clone)]
pub struct CpuVertexBuffer {
    num_elements: i32,
    num_vertices: i32,
    data: Vec<f32>,
}

impl CpuVertexBuffer {
    /// Allocate a buffer for `num_vertices` vertices each with `num_elements`
    /// floats.  All values initialised to zero.
    ///
    /// Returns `None` if either dimension is zero or negative.
    pub fn create(num_elements: i32, num_vertices: i32) -> Option<Self> {
        if num_elements <= 0 || num_vertices <= 0 {
            return None;
        }
        Some(Self {
            num_elements,
            num_vertices,
            data: vec![0.0; (num_elements * num_vertices) as usize],
        })
    }

    /// Same as `create` but panics on invalid dimensions (convenience).
    pub fn new(num_elements: i32, num_vertices: i32) -> Self {
        Self::create(num_elements, num_vertices).expect("CpuVertexBuffer: dimensions must be > 0")
    }

    /// Return a raw pointer to the data slice (CPU binding).
    pub fn bind_cpu_buffer_raw(&self) -> *const f32 {
        self.data.as_ptr()
    }
}

impl VertexBuffer for CpuVertexBuffer {
    fn bind_cpu_buffer(&self) -> &[f32] {
        &self.data
    }

    fn bind_cpu_buffer_mut(&mut self) -> &mut [f32] {
        &mut self.data
    }

    fn num_elements(&self) -> i32 {
        self.num_elements
    }

    fn num_vertices(&self) -> i32 {
        self.num_vertices
    }

    /// Copy `src` into the buffer at `start_vertex`.
    ///
    /// `src` must contain exactly `num_verts * num_elements` floats.
    fn update_data(&mut self, src: &[f32], start_vertex: i32, num_verts: i32) {
        let elem = self.num_elements as usize;
        let start = start_vertex as usize * elem;
        let len = num_verts as usize * elem;
        debug_assert!(src.len() >= len, "update_data: src too short");
        debug_assert!(start + len <= self.data.len(), "update_data: out of range");
        self.data[start..start + len].copy_from_slice(&src[..len]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_update() {
        let mut vb = CpuVertexBuffer::new(3, 4); // 4 verts, XYZ each
        assert_eq!(vb.num_elements(), 3);
        assert_eq!(vb.num_vertices(), 4);
        assert_eq!(vb.bind_cpu_buffer().len(), 12);

        // Upload first two vertices
        vb.update_data(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 0, 2);
        let buf = vb.bind_cpu_buffer();
        assert_eq!(buf[0], 1.0);
        assert_eq!(buf[4], 5.0);
    }

    #[test]
    fn update_middle_vertex() {
        let mut vb = CpuVertexBuffer::new(2, 3);
        vb.update_data(&[7.0, 8.0], 1, 1);
        let buf = vb.bind_cpu_buffer();
        assert_eq!(buf[2], 7.0);
        assert_eq!(buf[3], 8.0);
    }

    #[test]
    fn create_invalid_returns_none() {
        assert!(CpuVertexBuffer::create(0, 10).is_none());
        assert!(CpuVertexBuffer::create(3, -1).is_none());
    }
}
