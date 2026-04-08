//! SurfaceFactoryMeshAdapter — abstract interface between SurfaceFactory and a mesh.
//!
//! Mirrors `Bfr::SurfaceFactoryMeshAdapter` from `surfaceFactoryMeshAdapter.h`.

use super::vertex_descriptor::VertexDescriptor;

/// Integer type representing a mesh index.
pub type Index = i32;

/// Type used to identify face-varying primvar channels.
///
/// May be interpreted as a channel index (integer) or a pointer cast
/// to `intptr_t` depending on the subclass.
pub type FVarId = isize;

/// Abstract interface adapting `SurfaceFactory` to a connected mesh.
///
/// Mirrors `Bfr::SurfaceFactoryMeshAdapter`.
///
/// Implementors (subclasses of `SurfaceFactory`) must implement all required
/// methods.  The two optional "if-regular" methods have default no-op impls.
pub trait SurfaceFactoryMeshAdapter {
    // -----------------------------------------------------------------------
    // Required: simple face properties
    // -----------------------------------------------------------------------

    /// Returns `true` if the face is tagged as a hole.
    fn is_face_hole(&self, face_index: Index) -> bool;

    /// Returns the number of vertices of the face.
    fn get_face_size(&self, face_index: Index) -> i32;

    // -----------------------------------------------------------------------
    // Required: face-level vertex indices
    // -----------------------------------------------------------------------

    /// Gather vertex indices of the face's vertices.
    ///
    /// Returns the number of indices written.
    fn get_face_vertex_indices(&self, face_index: Index, vertex_indices: &mut [Index]) -> i32;

    /// Gather face-varying value indices of the face's vertices for the
    /// given `fvar_id`.
    fn get_face_fvar_value_indices(
        &self,
        face_index: Index,
        fvar_id: FVarId,
        fvar_value_indices: &mut [Index],
    ) -> i32;

    // -----------------------------------------------------------------------
    // Required: per-face-vertex neighbourhood
    // -----------------------------------------------------------------------

    /// Describe the topology of all incident faces around a corner vertex.
    ///
    /// Returns the position of the base face in the ordered sequence of
    /// incident faces (i.e. the "face-in-ring" value for that corner).
    fn populate_face_vertex_descriptor(
        &self,
        face_index: Index,
        face_vertex: i32,
        descriptor: &mut VertexDescriptor,
    ) -> i32;

    /// Gather vertex indices of all incident faces around a corner vertex.
    ///
    /// Indices must be ordered consistently with `populate_face_vertex_descriptor`
    /// and oriented relative to the corner vertex.
    fn get_face_vertex_incident_face_vertex_indices(
        &self,
        face_index: Index,
        face_vertex: i32,
        vertex_indices: &mut [Index],
    ) -> i32;

    /// Gather face-varying value indices of all incident faces around a corner
    /// vertex for the given `fvar_id`.
    fn get_face_vertex_incident_face_fvar_value_indices(
        &self,
        face_index: Index,
        face_vertex: i32,
        fvar_id: FVarId,
        fvar_value_indices: &mut [Index],
    ) -> i32;

    // -----------------------------------------------------------------------
    // Optional: fast path for regular faces
    // -----------------------------------------------------------------------

    /// If the neighbourhood around `face_index` is purely regular, populate
    /// `vertex_indices` with the indices of the 16-point (Catmark) or 12-point
    /// (Loop) regular patch and return `true`.
    ///
    /// Pass `None` for `vertex_indices` to just query the regularity.
    ///
    /// Default implementation returns `false`.
    fn get_face_neighborhood_vertex_indices_if_regular(
        &self,
        _face_index: Index,
        _vertex_indices: Option<&mut [Index]>,
    ) -> bool {
        false
    }

    /// Same as above, for face-varying indices.
    fn get_face_neighborhood_fvar_value_indices_if_regular(
        &self,
        _face_index: Index,
        _fvar_id: FVarId,
        _fvar_value_indices: Option<&mut [Index]>,
    ) -> bool {
        false
    }
}
