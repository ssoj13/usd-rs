// Copyright 2015 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/topologyLevel.h

use crate::sdc::crease::Rule;
use crate::vtr::{Level, Refinement};
use crate::vtr::array::ConstArray;
use super::types::{Index, ConstIndexArray, ConstLocalIndexArray};

/// Read-only view into a single level of the topology hierarchy.
///
/// Instances are owned by `TopologyRefiner` and returned as references.
/// Mirrors C++ `Far::TopologyLevel`.
pub struct TopologyLevel {
    pub(crate) level:         *const Level,
    pub(crate) ref_to_parent: *const Refinement,
    pub(crate) ref_to_child:  *const Refinement,
}

impl TopologyLevel {
    /// Create an empty (null) level placeholder.
    pub fn null() -> Self {
        Self {
            level:         std::ptr::null(),
            ref_to_parent: std::ptr::null(),
            ref_to_child:  std::ptr::null(),
        }
    }

    fn lv(&self) -> &Level { unsafe { &*self.level } }

    // ---- component counts ----

    /// Number of vertices in this level.
    pub fn get_num_vertices(&self) -> i32 { self.lv().get_num_vertices() }

    /// Number of faces in this level.
    pub fn get_num_faces(&self) -> i32 { self.lv().get_num_faces() }

    /// Number of edges in this level.
    pub fn get_num_edges(&self) -> i32 { self.lv().get_num_edges() }

    /// Total number of face-vertex entries (sum of face sizes).
    pub fn get_num_face_vertices(&self) -> i32 { self.lv().get_num_face_vertices_total() }

    // ---- topological relations ----

    pub fn get_face_vertices(&self, f: Index) -> ConstIndexArray<'_> { self.lv().get_face_vertices(f) }
    pub fn get_face_edges(&self, f: Index)    -> ConstIndexArray<'_> { self.lv().get_face_edges(f) }
    pub fn get_edge_vertices(&self, e: Index) -> ConstIndexArray<'_> { self.lv().get_edge_vertices(e) }
    pub fn get_edge_faces(&self, e: Index)    -> ConstIndexArray<'_> { self.lv().get_edge_faces(e) }
    pub fn get_vertex_faces(&self, v: Index)  -> ConstIndexArray<'_> { self.lv().get_vertex_faces(v) }
    pub fn get_vertex_edges(&self, v: Index)  -> ConstIndexArray<'_> { self.lv().get_vertex_edges(v) }

    pub fn get_vertex_face_local_indices(&self, v: Index) -> ConstLocalIndexArray<'_> {
        self.lv().get_vertex_face_local_indices(v)
    }
    pub fn get_vertex_edge_local_indices(&self, v: Index) -> ConstLocalIndexArray<'_> {
        self.lv().get_vertex_edge_local_indices(v)
    }
    pub fn get_edge_face_local_indices(&self, e: Index) -> ConstLocalIndexArray<'_> {
        self.lv().get_edge_face_local_indices(e)
    }

    /// Find edge by vertex pair.
    pub fn find_edge(&self, v0: Index, v1: Index) -> Index {
        self.lv().find_edge(v0, v1)
    }

    // ---- topological properties ----

    pub fn is_edge_non_manifold(&self, e: Index) -> bool { self.lv().is_edge_non_manifold(e) }
    pub fn is_vertex_non_manifold(&self, v: Index) -> bool { self.lv().is_vertex_non_manifold(v) }
    pub fn is_edge_boundary(&self, e: Index) -> bool { self.lv().get_edge_tag(e).boundary() }
    pub fn is_vertex_boundary(&self, v: Index) -> bool { self.lv().get_vertex_tag(v).boundary() }
    /// Returns true when vertex `v` is a corner (only one adjacent face).
    ///
    /// Uses face-count `== 1` as the corner criterion, which matches C++
    /// `TopologyLevel::IsVertexCorner()` for the common case.  C++ may also
    /// consult the vertex tag `_corner` bit (set by topology analysis) for
    /// sharpness-driven corners; those rare cases behave identically because
    /// a valence-1 vertex is always topologically a corner.
    pub fn is_vertex_corner(&self, v: Index) -> bool {
        self.lv().get_num_vertex_faces(v) == 1
    }
    pub fn is_vertex_valence_regular(&self, v: Index) -> bool {
        !self.lv().get_vertex_tag(v).xordinary() || self.is_vertex_corner(v)
    }

    // ---- feature tags ----

    pub fn get_edge_sharpness(&self, e: Index) -> f32   { self.lv().get_edge_sharpness(e) }
    pub fn get_vertex_sharpness(&self, v: Index) -> f32 { self.lv().get_vertex_sharpness(v) }
    pub fn is_edge_inf_sharp(&self, e: Index) -> bool   { self.lv().get_edge_tag(e).inf_sharp() }
    pub fn is_vertex_inf_sharp(&self, v: Index) -> bool { self.lv().get_vertex_tag(v).inf_sharp() }
    pub fn is_edge_semi_sharp(&self, e: Index) -> bool  { self.lv().get_edge_tag(e).semi_sharp() }
    pub fn is_vertex_semi_sharp(&self, v: Index) -> bool{ self.lv().get_vertex_tag(v).semi_sharp() }
    pub fn is_face_hole(&self, f: Index) -> bool        { self.lv().is_face_hole(f) }

    pub fn get_vertex_rule(&self, v: Index) -> Rule { self.lv().get_vertex_rule(v) }

    // ---- face-varying data ----

    pub fn get_num_fvar_channels(&self) -> i32 { self.lv().get_num_fvar_channels() }
    pub fn get_num_fvar_values(&self, channel: i32) -> i32 {
        self.lv().get_num_fvar_values(channel)
    }
    pub fn get_face_fvar_values(&self, f: Index, channel: i32) -> ConstIndexArray<'_> {
        self.lv().get_face_fvar_values(f, channel)
    }
    pub fn does_vertex_fvar_topology_match(&self, v: Index, channel: i32) -> bool {
        self.lv().does_vertex_fvar_topology_match(v, channel)
    }
    pub fn does_edge_fvar_topology_match(&self, e: Index, channel: i32) -> bool {
        self.lv().does_edge_fvar_topology_match(e, channel)
    }
    pub fn does_face_fvar_topology_match(&self, f: Index, channel: i32) -> bool {
        self.lv().does_face_fvar_topology_match(f, channel)
    }

    // ---- child/parent component access ----

    fn child_ref(&self) -> &Refinement {
        assert!(!self.ref_to_child.is_null(), "no child refinement");
        unsafe { &*self.ref_to_child }
    }
    fn parent_ref(&self) -> &Refinement {
        assert!(!self.ref_to_parent.is_null(), "no parent refinement");
        unsafe { &*self.ref_to_parent }
    }

    pub fn get_face_child_faces(&self, f: Index) -> ConstIndexArray<'_> {
        ConstArray::new(self.child_ref().get_face_child_faces(f))
    }
    pub fn get_face_child_edges(&self, f: Index) -> ConstIndexArray<'_> {
        ConstArray::new(self.child_ref().get_face_child_edges(f))
    }
    pub fn get_edge_child_edges(&self, e: Index) -> ConstIndexArray<'_> {
        ConstArray::new(self.child_ref().get_edge_child_edges(e).as_ref())
    }
    pub fn get_face_child_vertex(&self, f: Index) -> Index {
        self.child_ref().get_face_child_vertex(f)
    }
    pub fn get_edge_child_vertex(&self, e: Index) -> Index {
        self.child_ref().get_edge_child_vertex(e)
    }
    pub fn get_vertex_child_vertex(&self, v: Index) -> Index {
        self.child_ref().get_vertex_child_vertex(v)
    }
    pub fn get_face_parent_face(&self, f: Index) -> Index {
        self.parent_ref().get_child_face_parent_face(f)
    }

    // ---- debugging ----

    pub fn validate_topology(&self) -> bool { self.lv().validate_topology(None) }
    pub fn print_topology(&self, show_children: bool) {
        let refn = if show_children && !self.ref_to_child.is_null() {
            Some(unsafe { &*self.ref_to_child })
        } else {
            None
        };
        self.lv().print(refn);
    }
}

// Safety: TopologyLevel holds raw pointers to data owned by TopologyRefiner.
// As long as the refiner lives, the level is safe to use from the same thread.
unsafe impl Send for TopologyLevel {}
