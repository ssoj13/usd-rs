
//! HdMesh - Polygonal mesh primitive.
//!
//! Represents a polygonal mesh (triangle/quad/n-gon mesh) in Hydra.
//! Supports:
//! - Arbitrary topology (triangles, quads, n-gons)
//! - Subdivision surfaces
//! - Primvars with multiple interpolation modes
//! - Instancing
//! - Geom subsets for multi-material assignment
//!
//! # Example
//!
//! ```ignore
//! use usd_hd::prim::*;
//! use usd_sdf::Path;
//!
//! let id = Path::from_string("/World/Cube").unwrap();
//! let mesh = HdMesh::new(id, None);
//!
//! // Check if topology is dirty
//! if mesh.is_dirty_bits(HdRprim::DIRTY_TOPOLOGY) {
//!     // Re-sync topology
//! }
//! ```

use once_cell::sync::Lazy;

use super::{HdMeshReprDesc, HdRenderParam, HdRprim, HdSceneDelegate, ReprDescConfigs};
use usd_tf::Token;
type TfToken = Token;
use crate::enums::HdCullStyle;
use crate::scene_delegate::HdDisplayStyle;
use crate::types::HdDirtyBits;
use usd_px_osd::SubdivTags;
use usd_sdf::Path as SdfPath;

/// Global repr config for meshes (up to 2 descriptors per repr).
static MESH_REPR_CONFIGS: Lazy<ReprDescConfigs<HdMeshReprDesc, 2>> =
    Lazy::new(ReprDescConfigs::new);

/// Mesh topology data.
///
/// Contains indices and face vertex counts for a mesh.
/// Matches C++ HdMeshTopology (scheme, orientation, counts, indices, holes).
#[derive(Debug, Clone, Default)]
pub struct HdMeshTopology {
    /// Subdivision scheme (e.g. "none", "catmullClark", "loop").
    pub scheme: TfToken,

    /// Winding orientation ("rightHanded" or "leftHanded").
    pub orientation: TfToken,

    /// Number of vertices per face.
    pub face_vertex_counts: Vec<i32>,

    /// Vertex indices for each face.
    pub face_vertex_indices: Vec<i32>,

    /// Hole indices (faces to be treated as holes).
    pub hole_indices: Vec<i32>,

    /// Subdivision surface tags (creases, corners, interpolation rules).
    pub subdiv_tags: SubdivTags,
}

impl HdMeshTopology {
    /// Create new empty topology.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create topology from counts and indices (scheme=none, rightHanded).
    pub fn from_data(face_vertex_counts: Vec<i32>, face_vertex_indices: Vec<i32>) -> Self {
        Self {
            scheme: TfToken::new("none"),
            orientation: TfToken::new("rightHanded"),
            face_vertex_counts,
            face_vertex_indices,
            hole_indices: Vec::new(),
            subdiv_tags: SubdivTags::default(),
        }
    }

    /// Create full topology matching C++ HdMeshTopology constructor.
    pub fn from_full(
        scheme: TfToken,
        orientation: TfToken,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        hole_indices: Vec<i32>,
    ) -> Self {
        Self {
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices,
            subdiv_tags: SubdivTags::default(),
        }
    }

    /// Create full topology with subdiv tags.
    pub fn from_full_with_tags(
        scheme: TfToken,
        orientation: TfToken,
        face_vertex_counts: Vec<i32>,
        face_vertex_indices: Vec<i32>,
        hole_indices: Vec<i32>,
        subdiv_tags: SubdivTags,
    ) -> Self {
        Self {
            scheme,
            orientation,
            face_vertex_counts,
            face_vertex_indices,
            hole_indices,
            subdiv_tags,
        }
    }

    /// Get number of faces.
    pub fn num_faces(&self) -> usize {
        self.face_vertex_counts.len()
    }

    /// Get total number of face vertices.
    pub fn num_face_vertices(&self) -> usize {
        self.face_vertex_indices.len()
    }
}

/// Polygonal mesh primitive.
///
/// Represents a mesh with arbitrary topology. Supports subdivision,
/// primvars, and instancing.
#[derive(Debug)]
pub struct HdMesh {
    /// Prim identifier.
    id: SdfPath,

    /// Current dirty bits.
    dirty_bits: HdDirtyBits,

    /// Instancer id if instanced.
    instancer_id: Option<SdfPath>,

    /// Visibility state.
    visible: bool,

    /// Material id.
    material_id: Option<SdfPath>,

    /// Mesh topology.
    topology: HdMeshTopology,

    /// Display style (smooth/flat shading, etc).
    smooth_normals: bool,

    /// Double-sided rendering.
    double_sided: bool,
}

impl HdMesh {
    // ------------------------------------------------------------------
    // Static repr configuration (C++ ConfigureRepr / _GetReprDesc)
    // ------------------------------------------------------------------

    /// Configure the geometric style for a repr. Up to 2 descriptors.
    /// Corresponds to C++ `HdMesh::ConfigureRepr`.
    pub fn configure_repr(name: &Token, desc0: HdMeshReprDesc, desc1: HdMeshReprDesc) {
        MESH_REPR_CONFIGS.add_or_update(name, [desc0, desc1]);
    }

    /// Look up repr descriptors by name.
    /// Corresponds to C++ `HdMesh::_GetReprDesc`.
    pub fn get_repr_desc(name: &Token) -> Option<[HdMeshReprDesc; 2]> {
        MESH_REPR_CONFIGS.find(name)
    }

    // ------------------------------------------------------------------
    // Delegate convenience wrappers (inline in C++ mesh.h)
    // ------------------------------------------------------------------

    /// Get cull style from scene delegate.
    pub fn get_cull_style(&self, delegate: &dyn HdSceneDelegate) -> HdCullStyle {
        delegate.get_cull_style(self.get_id())
    }

    /// Get display style from scene delegate.
    pub fn get_display_style(&self, delegate: &dyn HdSceneDelegate) -> HdDisplayStyle {
        delegate.get_display_style(self.get_id())
    }

    /// Get subdivision tags from scene delegate.
    pub fn get_subdiv_tags(&self, delegate: &dyn HdSceneDelegate) -> SubdivTags {
        delegate.get_subdiv_tags(self.get_id())
    }

    /// Convenience: fetch shading style from the scene delegate.
    pub fn get_shading_style(&self, delegate: &dyn HdSceneDelegate) -> usd_vt::Value {
        delegate.get_shading_style(self.get_id())
    }

    /// Get points primvar from scene delegate.
    pub fn get_points_from_delegate(&self, delegate: &dyn HdSceneDelegate) -> usd_vt::Value {
        use crate::tokens::POINTS;
        delegate.get(self.get_id(), &POINTS)
    }

    /// Get normals primvar from scene delegate.
    pub fn get_normals_from_delegate(&self, delegate: &dyn HdSceneDelegate) -> usd_vt::Value {
        use crate::tokens::NORMALS;
        delegate.get(self.get_id(), &NORMALS)
    }

    // ------------------------------------------------------------------
    // Construction
    // ------------------------------------------------------------------

    /// Create a new mesh primitive.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for this mesh
    /// * `instancer_id` - Optional instancer id if mesh is instanced
    pub fn new(id: SdfPath, instancer_id: Option<SdfPath>) -> Self {
        Self {
            id,
            dirty_bits: Self::get_initial_dirty_bits_mask(),
            instancer_id,
            visible: true,
            material_id: None,
            topology: HdMeshTopology::new(),
            smooth_normals: true,
            double_sided: false,
        }
    }

    /// Get mesh topology.
    pub fn get_topology(&self) -> &HdMeshTopology {
        &self.topology
    }

    /// Set mesh topology.
    pub fn set_topology(&mut self, topology: HdMeshTopology) {
        self.topology = topology;
        self.mark_dirty(Self::DIRTY_TOPOLOGY);
    }

    /// Check if mesh uses smooth normals.
    pub fn is_smooth_normals(&self) -> bool {
        self.smooth_normals
    }

    /// Set smooth normals flag.
    pub fn set_smooth_normals(&mut self, smooth: bool) {
        if self.smooth_normals != smooth {
            self.smooth_normals = smooth;
            self.mark_dirty(Self::DIRTY_NORMALS);
        }
    }

    /// Check if mesh is double-sided.
    pub fn is_double_sided(&self) -> bool {
        self.double_sided
    }

    /// Set double-sided flag.
    pub fn set_double_sided(&mut self, double_sided: bool) {
        if self.double_sided != double_sided {
            self.double_sided = double_sided;
            self.mark_dirty(Self::DIRTY_DISPLAY_STYLE);
        }
    }
}

impl HdRprim for HdMesh {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn get_instancer_id(&self) -> Option<&SdfPath> {
        self.instancer_id.as_ref()
    }

    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
        _repr_token: &Token,
    ) {
        // Query delegate for data based on dirty bits

        if (*dirty_bits & Self::DIRTY_VISIBILITY) != 0 {
            self.visible = delegate.get_visible(self.get_id());
        }

        if (*dirty_bits & Self::DIRTY_MATERIAL_ID) != 0 {
            self.material_id = delegate.get_material_id(self.get_id());
        }

        if (*dirty_bits & Self::DIRTY_TOPOLOGY) != 0 {
            self.topology = delegate.get_mesh_topology(self.get_id());
        }

        if (*dirty_bits & Self::DIRTY_DISPLAY_STYLE) != 0 {
            self.double_sided = delegate.get_double_sided(self.get_id());
        }

        // Clear dirty bits
        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn get_material_id(&self) -> Option<&SdfPath> {
        self.material_id.as_ref()
    }

    fn get_builtin_primvar_names() -> Vec<Token>
    where
        Self: Sized,
    {
        vec![Token::new("points"), Token::new("normals")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_creation() {
        let id = SdfPath::from_string("/World/Mesh").unwrap();
        let mesh = HdMesh::new(id.clone(), None);

        assert_eq!(mesh.get_id(), &id);
        assert!(mesh.is_visible());
        assert!(mesh.is_dirty());
    }

    #[test]
    fn test_mesh_topology() {
        let mut mesh = HdMesh::new(SdfPath::from_string("/Mesh").unwrap(), None);

        let topology = HdMeshTopology::from_data(
            vec![4, 4, 4, 4, 4, 4], // 6 quads
            vec![
                0, 1, 2, 3, // face 0
                4, 5, 6, 7, // face 1
                0, 1, 5, 4, // face 2
                2, 3, 7, 6, // face 3
                0, 3, 7, 4, // face 4
                1, 2, 6, 5, // face 5
            ],
        );

        assert_eq!(topology.num_faces(), 6);
        assert_eq!(topology.num_face_vertices(), 24);

        mesh.set_topology(topology);
        assert!(mesh.is_dirty_bits(HdMesh::DIRTY_TOPOLOGY));
    }

    #[test]
    fn test_mesh_properties() {
        let mut mesh = HdMesh::new(SdfPath::from_string("/Mesh").unwrap(), None);

        assert!(mesh.is_smooth_normals());
        mesh.set_smooth_normals(false);
        assert!(!mesh.is_smooth_normals());

        assert!(!mesh.is_double_sided());
        mesh.set_double_sided(true);
        assert!(mesh.is_double_sided());
    }
}
