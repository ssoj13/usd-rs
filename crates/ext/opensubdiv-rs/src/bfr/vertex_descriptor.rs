//! VertexDescriptor — user-facing struct for specifying corner topology.
//!
//! Mirrors `Bfr::VertexDescriptor` from `vertexDescriptor.h/cpp`.

use super::limits::Limits;

/// Describes the complete topological neighbourhood of a vertex that is a
/// corner of a face.
///
/// Instances are partially initialised by `SurfaceFactory` before being
/// passed to subclasses to be fully populated between calls to
/// `initialize()` and `finalize()`.
///
/// Public construction is not intended — the factory manages instances.
/// We mirror the full C++ interface including all inline methods.
#[derive(Clone, Debug)]
pub struct VertexDescriptor {
    // Status flags.
    pub(crate) is_valid:       bool,
    pub(crate) is_initialized: bool,
    pub(crate) is_finalized:   bool,

    pub(crate) is_manifold: bool,
    pub(crate) is_boundary: bool,

    pub(crate) has_face_sizes:     bool,
    pub(crate) has_edge_sharpness: bool,

    /// Number of incident faces.
    pub(crate) num_faces: i16,

    /// Explicit vertex sharpness.
    pub(crate) vert_sharpness: f32,

    /// Per-face-edge sharpness; `2 * num_faces` entries (leading, trailing).
    pub(crate) face_edge_sharpness: Vec<f32>,

    /// Face-size information, stored as cumulative offsets after `finalize()`,
    /// or as raw sizes before `finalize()`.
    pub(crate) face_size_offsets: Vec<i32>,
}

impl VertexDescriptor {
    /// Create an empty (uninitialised) descriptor.
    pub fn new() -> Self {
        VertexDescriptor {
            is_valid:       false,
            is_initialized: false,
            is_finalized:   false,
            is_manifold:    false,
            is_boundary:    false,
            has_face_sizes:     false,
            has_edge_sharpness: false,
            num_faces:      0,
            vert_sharpness: 0.0,
            face_edge_sharpness: Vec::new(),
            face_size_offsets:   Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Initialize / Finalize
    // -----------------------------------------------------------------------

    /// Begin specification with the number of incident faces.
    ///
    /// Returns `false` when `num_incident_faces` is out of range.
    pub fn initialize(&mut self, num_incident_faces: i32) -> bool {
        self.is_valid  = num_incident_faces > 0
            && num_incident_faces <= Limits::max_valence();
        self.num_faces = if self.is_valid { num_incident_faces as i16 } else { 0 };

        // Reset everything else regardless.
        self.vert_sharpness    = 0.0;
        self.is_manifold       = false;
        self.is_boundary       = false;
        self.has_face_sizes     = false;
        self.has_edge_sharpness = false;

        self.is_initialized = self.is_valid;
        self.is_finalized   = false;

        self.is_initialized
    }

    /// Terminate the specification.
    ///
    /// Returns `false` on failure (invalid face sizes).
    pub fn finalize(&mut self) -> bool {
        if !self.is_valid {
            return false;
        }

        if self.has_face_sizes {
            let size0 = self.face_size_offsets[0];
            let mut same_sizes = true;
            let mut sum = 0i32;

            for i in 0..self.num_faces as usize {
                let fs = self.face_size_offsets[i];
                if fs < 3 || fs > Limits::max_face_size() {
                    self.is_valid = false;
                    return false;
                }
                same_sizes &= fs == size0;
                self.face_size_offsets[i] = sum;
                sum += fs;
            }
            let n = self.num_faces as usize;
            self.face_size_offsets[n] = sum;

            // No need to keep explicit sizes if all the same:
            if same_sizes {
                self.has_face_sizes = false;
            }
        }

        self.is_finalized = true;
        true
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    #[inline] pub fn is_valid(&self)      -> bool { self.is_valid }
    #[inline] pub fn is_manifold(&self)   -> bool { self.is_manifold }
    #[inline] pub fn is_boundary(&self)   -> bool { self.is_boundary }

    #[inline] pub fn has_incident_face_sizes(&self)  -> bool { self.has_face_sizes }
    #[inline] pub fn has_vertex_sharpness(&self)     -> bool { self.vert_sharpness > 0.0 }
    #[inline] pub fn has_edge_sharpness(&self)       -> bool { self.has_edge_sharpness }

    #[inline]
    pub fn get_incident_face_size(&self, face_idx: i32) -> i32 {
        if self.is_finalized {
            self.face_size_offsets[face_idx as usize + 1]
                - self.face_size_offsets[face_idx as usize]
        } else {
            self.face_size_offsets[face_idx as usize]
        }
    }

    #[inline] pub fn get_vertex_sharpness(&self)   -> f32  { self.vert_sharpness }

    pub fn get_manifold_edge_sharpness(&self, edge_idx: i32) -> f32 {
        // Compact storage: 2*N entries (leading, trailing sharpness per face).
        // For interior vertices: edge i (i<N) is the leading edge of face i -> index 2*i.
        // For boundary vertices: edge N is the trailing edge of face N-1 -> index 2*N-1.
        // Formula: 2*edge_idx - (edge_idx==N) maps:
        //   edge_idx < N -> 2*edge_idx  (leading of face edge_idx)
        //   edge_idx = N -> 2*N-1       (trailing of face N-1, boundary-only)
        let n = self.num_faces as i32;
        self.face_edge_sharpness[(2 * edge_idx - (edge_idx == n) as i32) as usize]
    }

    pub fn get_incident_face_edge_sharpness(
        &self,
        face_idx: i32,
    ) -> (f32, f32) {
        let base = (2 * face_idx) as usize;
        (
            self.face_edge_sharpness[base],
            self.face_edge_sharpness[base + 1],
        )
    }

    // -----------------------------------------------------------------------
    // Setters
    // -----------------------------------------------------------------------

    #[inline] pub fn set_manifold(&mut self, v: bool) { self.is_manifold = v; }
    #[inline] pub fn set_boundary(&mut self, v: bool) { self.is_boundary = v; }

    pub fn set_incident_face_size(&mut self, face_idx: i32, face_size: i32) {
        if !self.has_face_sizes {
            self.init_face_sizes();
        }
        self.face_size_offsets[face_idx as usize] = face_size;
    }

    #[inline] pub fn clear_incident_face_sizes(&mut self) { self.has_face_sizes = false; }

    #[inline] pub fn set_vertex_sharpness(&mut self, s: f32) { self.vert_sharpness = s; }
    #[inline] pub fn clear_vertex_sharpness(&mut self)       { self.vert_sharpness = 0.0; }

    pub fn set_manifold_edge_sharpness(&mut self, edge_idx: i32, sharpness: f32) {
        if !self.has_edge_sharpness {
            self.init_edge_sharpness();
        }
        let n = self.num_faces as i32;
        // Leading edge of the face AFTER this edge:
        if edge_idx < n {
            self.face_edge_sharpness[(2 * edge_idx) as usize] = sharpness;
        }
        // Trailing edge of the face BEFORE this edge:
        if edge_idx > 0 {
            self.face_edge_sharpness[(2 * edge_idx - 1) as usize] = sharpness;
        } else if !self.is_boundary {
            self.face_edge_sharpness[(2 * n - 1) as usize] = sharpness;
        }
    }

    pub fn set_incident_face_edge_sharpness(
        &mut self,
        face_idx: i32,
        leading: f32,
        trailing: f32,
    ) {
        if !self.has_edge_sharpness {
            self.init_edge_sharpness();
        }
        let base = (2 * face_idx) as usize;
        self.face_edge_sharpness[base]     = leading;
        self.face_edge_sharpness[base + 1] = trailing;
    }

    #[inline] pub fn clear_edge_sharpness(&mut self) { self.has_edge_sharpness = false; }

    // -----------------------------------------------------------------------
    // Internal helpers (called lazily)
    // -----------------------------------------------------------------------

    pub(crate) fn init_face_sizes(&mut self) {
        let n = self.num_faces as usize + 1;
        self.face_size_offsets = vec![0i32; n];
        self.has_face_sizes    = true;
    }

    pub(crate) fn init_edge_sharpness(&mut self) {
        let n = self.num_faces as usize * 2;
        self.face_edge_sharpness = vec![0.0f32; n];
        self.has_edge_sharpness  = true;
    }
}

impl Default for VertexDescriptor {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_invalid_zero() {
        let mut vd = VertexDescriptor::new();
        assert!(!vd.initialize(0));
        assert!(!vd.is_valid());
    }

    #[test]
    fn initialize_and_finalize_simple() {
        let mut vd = VertexDescriptor::new();
        assert!(vd.initialize(4));
        vd.set_manifold(true);
        vd.set_boundary(false);
        vd.clear_incident_face_sizes();
        assert!(vd.finalize());
        assert!(vd.is_valid());
    }

    #[test]
    fn face_sizes_converted_to_offsets() {
        let mut vd = VertexDescriptor::new();
        vd.initialize(3);
        vd.set_incident_face_size(0, 4);
        vd.set_incident_face_size(1, 5);
        vd.set_incident_face_size(2, 4);
        vd.finalize();

        // After finalize: sizes are stored as cumulative offsets.
        // Face 0 = offset 0..4 -> size 4.
        assert_eq!(vd.get_incident_face_size(0), 4);
        assert_eq!(vd.get_incident_face_size(1), 5);
        assert_eq!(vd.get_incident_face_size(2), 4);
    }

    #[test]
    fn edge_sharpness() {
        let mut vd = VertexDescriptor::new();
        vd.initialize(4);
        vd.set_manifold(true);
        vd.set_boundary(false);
        vd.set_incident_face_edge_sharpness(0, 1.5, 0.0);
        vd.set_incident_face_edge_sharpness(1, 0.0, 0.0);
        vd.set_incident_face_edge_sharpness(2, 0.0, 0.0);
        vd.set_incident_face_edge_sharpness(3, 0.0, 0.0);
        vd.finalize();

        let (lead, trail) = vd.get_incident_face_edge_sharpness(0);
        assert!((lead - 1.5).abs() < 1e-6);
        assert!(trail.abs() < 1e-6);
    }
}
