#![allow(dangerous_implicit_autorefs)]
// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/refinement.h/.cpp

use super::fvar_refinement::FVarRefinement;
use super::level::Level;
use super::types::{INDEX_INVALID, Index, index_is_valid};
use crate::sdc::{
    Options,
    crease::{Crease, Rule, SHARPNESS_INFINITE, SHARPNESS_SMOOTH, is_sharp},
    types::Split,
};

// ---------------------------------------------------------------------------
// Refinement options
// ---------------------------------------------------------------------------

/// Options for the refine() call.
/// Mirrors C++ `Vtr::internal::Refinement::Options`.
#[derive(Clone, Copy, Default)]
pub struct RefinementOptions {
    /// Sparse (adaptive) refinement — not all parent components are refined.
    pub sparse: bool,
    /// Order child vertices from faces first (face-verts → edge-verts → vert-verts).
    pub face_verts_first: bool,
    /// Only generate face-vertex relation in the last level (no full topology).
    pub minimal_topology: bool,
}

// ---------------------------------------------------------------------------
// Component tags
// ---------------------------------------------------------------------------

/// Sparse-selection tag for a parent component.
/// Mirrors C++ `Vtr::internal::Refinement::SparseTag`.
#[derive(Clone, Copy, Default)]
pub struct SparseTag {
    /// This component was explicitly selected for refinement.
    pub selected: bool,
    /// Transitional mask: for edges (1 bit), for faces (4 bits, one per edge).
    pub transitional: u8,
}

/// Child-to-parent mapping tag.
/// Mirrors C++ `Vtr::internal::Refinement::ChildTag`.
#[derive(Clone, Copy, Default)]
pub struct ChildTag {
    /// Neighborhood is incomplete (only relevant at the finest level).
    pub incomplete: bool,
    /// Type of parent component: 0=vertex, 1=edge, 2=face.
    pub parent_type: u8,
    /// Index of child within its parent (0-3, or 0 if parent has >4 children).
    pub index_in_parent: u8,
}

impl ChildTag {
    /// Construct a complete child tag with a given index-in-parent.
    /// Mirrors C++ `ChildTag` initialization for complete (non-boundary) children.
    const fn new_complete(index_in_parent: u8) -> Self {
        ChildTag {
            incomplete: false,
            parent_type: 0,
            index_in_parent,
        }
    }
    /// Construct an incomplete child tag (boundary / sparse partial child).
    const fn new_incomplete(index_in_parent: u8) -> Self {
        ChildTag {
            incomplete: true,
            parent_type: 0,
            index_in_parent,
        }
    }
}

// ---------------------------------------------------------------------------
// Relations flags
// ---------------------------------------------------------------------------

/// Which of the 6 topology relations to populate in the child Level.
/// Mirrors C++ `Vtr::internal::Refinement::Relations`.
pub struct Relations {
    pub face_vertices: bool,
    pub face_edges: bool,
    pub edge_vertices: bool,
    pub edge_faces: bool,
    pub vertex_faces: bool,
    pub vertex_edges: bool,
}

impl Relations {
    pub fn all_true() -> Self {
        Relations {
            face_vertices: true,
            face_edges: true,
            edge_vertices: true,
            edge_faces: true,
            vertex_faces: true,
            vertex_edges: true,
        }
    }
    pub fn all_false() -> Self {
        Relations {
            face_vertices: false,
            face_edges: false,
            edge_vertices: false,
            edge_faces: false,
            vertex_faces: false,
            vertex_edges: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Refinement — base struct
// ---------------------------------------------------------------------------

/// Abstract refinement between two topology levels.
///
/// Stores parent-to-child and child-to-parent mapping arrays.
/// Concrete sub-types (`QuadRefinement`, `TriRefinement`) supply the
/// topology-building virtuals (`populate_face_vertex_relation()`, etc.).
///
/// Mirrors C++ `Vtr::internal::Refinement`.
pub struct Refinement {
    // References — stored as raw pointers to avoid borrow-checker conflicts
    // when both parent (immutable) and child (mutable) are needed simultaneously.
    // SAFETY: caller guarantees both levels outlive this refinement.
    pub(crate) parent: *const Level,
    pub(crate) child: *mut Level,

    pub(crate) options: Options,
    pub(crate) split_type: Split,
    pub(crate) reg_face_size: i32,

    /// True when performing uniform (non-sparse) refinement.
    pub(crate) uniform: bool,
    pub(crate) face_verts_first: bool,

    // ---- child component counts ----
    pub(crate) child_face_from_face_count: i32,
    pub(crate) child_edge_from_face_count: i32,
    pub(crate) child_edge_from_edge_count: i32,
    pub(crate) child_vert_from_face_count: i32,
    pub(crate) child_vert_from_edge_count: i32,
    pub(crate) child_vert_from_vert_count: i32,

    pub(crate) first_child_face_from_face: i32,
    pub(crate) first_child_edge_from_face: i32,
    pub(crate) first_child_edge_from_edge: i32,
    pub(crate) first_child_vert_from_face: i32,
    pub(crate) first_child_vert_from_edge: i32,
    pub(crate) first_child_vert_from_vert: i32,

    // ---- parent → child maps ----
    // The face-child-face and face-child-edge counts/offsets are shared with
    // the parent Level's face-vert counts/offsets (same layout), so we store
    // indices into the parent's face_vert_counts_offsets here as a &-reference.
    // To avoid lifetime issues, we keep a flag indicating if they are "shared".
    pub(crate) face_child_face_counts_offsets_shared: bool, // true = same as parent face-vert c/o
    pub(crate) face_child_edge_counts_offsets_shared: bool,

    /// Local counts/offsets vector used when face-child-face c/o is NOT shared
    /// with the parent Level's face-vert array (e.g. TriRefinement stores 4 per face).
    pub(crate) local_face_child_face_counts_offsets: Vec<i32>,

    /// child-face indices per parent face (variable arity, indexed by face c/o).
    pub(crate) face_child_face_indices: Vec<Index>,
    /// child-edge indices per parent face (same count/offset as face-child-faces).
    pub(crate) face_child_edge_indices: Vec<Index>,
    /// one child vertex per parent face.
    pub(crate) face_child_vert_index: Vec<Index>,

    /// two child edge indices per parent edge ([2 * pEdge] and [2 * pEdge + 1]).
    pub(crate) edge_child_edge_indices: Vec<Index>,
    /// one child vertex per parent edge.
    pub(crate) edge_child_vert_index: Vec<Index>,

    /// one child vertex per parent vertex.
    pub(crate) vert_child_vert_index: Vec<Index>,

    // ---- child → parent maps ----
    pub(crate) child_face_parent_index: Vec<Index>,
    pub(crate) child_edge_parent_index: Vec<Index>,
    pub(crate) child_vertex_parent_index: Vec<Index>,

    pub(crate) child_face_tag: Vec<ChildTag>,
    pub(crate) child_edge_tag: Vec<ChildTag>,
    pub(crate) child_vertex_tag: Vec<ChildTag>,

    // ---- sparse selection tags (parent components) ----
    pub(crate) parent_face_tag: Vec<SparseTag>,
    pub(crate) parent_edge_tag: Vec<SparseTag>,
    pub(crate) parent_vertex_tag: Vec<SparseTag>,

    // ---- face-varying channel refinements ----
    pub(crate) fvar_channels: Vec<Box<FVarRefinement>>,

    // ---- virtual-dispatch callbacks (set by QuadRefinement / TriRefinement) ----
    // These replace C++ virtual methods.  QuadRefinement::new() / TriRefinement::new()
    // must set all of them so that Refinement::refine() works correctly even when
    // the concrete wrapper is unwrapped into a bare Box<Refinement>.
    pub(crate) allocate_fn: Option<fn(&mut Refinement)>,
    pub(crate) sparse_face_fn: Option<fn(&mut Refinement)>,
    pub(crate) populate_fv_fn: Option<fn(&mut Refinement)>,
    pub(crate) populate_fe_fn: Option<fn(&mut Refinement)>,
    pub(crate) populate_ev_fn: Option<fn(&mut Refinement)>,
    pub(crate) populate_ef_fn: Option<fn(&mut Refinement)>,
    pub(crate) populate_vf_fn: Option<fn(&mut Refinement)>,
    pub(crate) populate_ve_fn: Option<fn(&mut Refinement)>,
}

// Small helpers mirroring C++ anonymous namespace functions
#[inline]
pub fn is_sparse_index_marked(index: Index) -> bool {
    index != 0
}

pub const SPARSE_MASK_NEIGHBORING: Index = 1 << 0;
pub const SPARSE_MASK_SELECTED: Index = 1 << 1;

#[inline]
pub fn mark_sparse_neighbor(idx: &mut Index) {
    *idx = SPARSE_MASK_NEIGHBORING;
}
#[inline]
pub fn mark_sparse_selected(idx: &mut Index) {
    *idx = SPARSE_MASK_SELECTED;
}
/// Alias so quad_refinement can call it by a consistent name.
#[inline]
pub fn mark_sparse_selected_pub(idx: &mut Index) {
    *idx = SPARSE_MASK_SELECTED;
}

#[inline]
fn sequence_sparse_index_vector(v: &mut Vec<Index>, base: i32) -> i32 {
    let mut count = 0i32;
    for x in v.iter_mut() {
        *x = if is_sparse_index_marked(*x) {
            let r = base + count;
            count += 1;
            r
        } else {
            INDEX_INVALID
        };
    }
    count
}

#[inline]
fn sequence_full_index_vector(v: &mut Vec<Index>, base: i32) -> i32 {
    let n = v.len() as i32;
    for (i, x) in v.iter_mut().enumerate() {
        *x = base + i as i32;
    }
    n
}

impl Refinement {
    /// Create a new (uninitialised) Refinement linking `parent` and `child`.
    ///
    /// # Safety
    /// `parent` and `child` must outlive this struct.  The child's `depth` and
    /// `vert_count` must be 0 on entry (matching the C++ assertion).
    pub unsafe fn new(
        parent: *const Level,
        child: *mut Level,
        options: Options,
        split_type: Split,
    ) -> Self {
        // Set child depth immediately, as C++ does in the ctor
        unsafe {
            (*child).depth = 1 + (*parent).depth;
        }

        let reg_face_size = match split_type {
            Split::ToQuads | Split::Hybrid => 4,
            Split::ToTris => 3,
        };

        Refinement {
            parent,
            child,
            options,
            split_type,
            reg_face_size,
            uniform: false,
            face_verts_first: false,
            child_face_from_face_count: 0,
            child_edge_from_face_count: 0,
            child_edge_from_edge_count: 0,
            child_vert_from_face_count: 0,
            child_vert_from_edge_count: 0,
            child_vert_from_vert_count: 0,
            first_child_face_from_face: 0,
            first_child_edge_from_face: 0,
            first_child_edge_from_edge: 0,
            first_child_vert_from_face: 0,
            first_child_vert_from_edge: 0,
            first_child_vert_from_vert: 0,
            face_child_face_counts_offsets_shared: false,
            face_child_edge_counts_offsets_shared: false,
            local_face_child_face_counts_offsets: Vec::new(),
            face_child_face_indices: Vec::new(),
            face_child_edge_indices: Vec::new(),
            face_child_vert_index: Vec::new(),
            edge_child_edge_indices: Vec::new(),
            edge_child_vert_index: Vec::new(),
            vert_child_vert_index: Vec::new(),
            child_face_parent_index: Vec::new(),
            child_edge_parent_index: Vec::new(),
            child_vertex_parent_index: Vec::new(),
            child_face_tag: Vec::new(),
            child_edge_tag: Vec::new(),
            child_vertex_tag: Vec::new(),
            parent_face_tag: Vec::new(),
            parent_edge_tag: Vec::new(),
            parent_vertex_tag: Vec::new(),
            fvar_channels: Vec::new(),
            allocate_fn: None,
            sparse_face_fn: None,
            populate_fv_fn: None,
            populate_fe_fn: None,
            populate_ev_fn: None,
            populate_ef_fn: None,
            populate_vf_fn: None,
            populate_ve_fn: None,
        }
    }

    // ---- safe reference accessors ----
    #[inline]
    pub fn parent(&self) -> &Level {
        unsafe { &*self.parent }
    }
    #[inline]
    pub fn child(&self) -> &Level {
        unsafe { &*self.child }
    }
    #[inline]
    pub fn child_mut(&mut self) -> &mut Level {
        unsafe { &mut *self.child }
    }

    // ---- basic accessors ----
    #[inline]
    pub fn get_split_type(&self) -> Split {
        self.split_type
    }
    #[inline]
    pub fn get_regular_face_size(&self) -> i32 {
        self.reg_face_size
    }
    #[inline]
    pub fn get_options(&self) -> Options {
        self.options
    }
    #[inline]
    pub fn has_face_vertices_first(&self) -> bool {
        self.face_verts_first
    }

    // ---- child component counts ----
    #[inline]
    pub fn get_num_child_faces_from_faces(&self) -> i32 {
        self.child_face_from_face_count
    }
    #[inline]
    pub fn get_num_child_edges_from_faces(&self) -> i32 {
        self.child_edge_from_face_count
    }
    #[inline]
    pub fn get_num_child_edges_from_edges(&self) -> i32 {
        self.child_edge_from_edge_count
    }
    #[inline]
    pub fn get_num_child_vertices_from_faces(&self) -> i32 {
        self.child_vert_from_face_count
    }
    #[inline]
    pub fn get_num_child_vertices_from_edges(&self) -> i32 {
        self.child_vert_from_edge_count
    }
    #[inline]
    pub fn get_num_child_vertices_from_vertices(&self) -> i32 {
        self.child_vert_from_vert_count
    }

    #[inline]
    pub fn get_first_child_face_from_faces(&self) -> Index {
        self.first_child_face_from_face
    }
    #[inline]
    pub fn get_first_child_edge_from_faces(&self) -> Index {
        self.first_child_edge_from_face
    }
    #[inline]
    pub fn get_first_child_edge_from_edges(&self) -> Index {
        self.first_child_edge_from_edge
    }
    #[inline]
    pub fn get_first_child_vertex_from_faces(&self) -> Index {
        self.first_child_vert_from_face
    }
    #[inline]
    pub fn get_first_child_vertex_from_edges(&self) -> Index {
        self.first_child_vert_from_edge
    }
    #[inline]
    pub fn get_first_child_vertex_from_vertices(&self) -> Index {
        self.first_child_vert_from_vert
    }

    // ---- parent → child vertex accessors ----
    #[inline]
    pub fn get_face_child_vertex(&self, f: Index) -> Index {
        self.face_child_vert_index[f as usize]
    }
    #[inline]
    pub fn get_edge_child_vertex(&self, e: Index) -> Index {
        self.edge_child_vert_index[e as usize]
    }
    #[inline]
    pub fn get_vertex_child_vertex(&self, v: Index) -> Index {
        self.vert_child_vert_index[v as usize]
    }

    // ---- parent → child face/edge arrays (using shared counts/offsets) ----
    pub fn get_face_child_faces(&self, f: Index) -> &[Index] {
        let (count, offset) = self.face_child_face_co(f);
        &self.face_child_face_indices[offset..offset + count]
    }
    pub fn get_face_child_faces_mut(&mut self, f: Index) -> &mut [Index] {
        let (count, offset) = self.face_child_face_co(f);
        &mut self.face_child_face_indices[offset..offset + count]
    }

    pub fn get_face_child_edges(&self, f: Index) -> &[Index] {
        let (count, offset) = self.face_child_edge_co(f);
        &self.face_child_edge_indices[offset..offset + count]
    }
    pub fn get_face_child_edges_mut(&mut self, f: Index) -> &mut [Index] {
        let (count, offset) = self.face_child_edge_co(f);
        &mut self.face_child_edge_indices[offset..offset + count]
    }

    pub fn get_edge_child_edges(&self, e: Index) -> &[Index; 2] {
        let off = (2 * e) as usize;
        self.edge_child_edge_indices[off..off + 2]
            .try_into()
            .unwrap()
    }
    pub fn get_edge_child_edges_mut(&mut self, e: Index) -> &mut [Index] {
        let off = (2 * e) as usize;
        &mut self.edge_child_edge_indices[off..off + 2]
    }

    // ---- child → parent accessors ----
    #[inline]
    pub fn get_child_face_parent_face(&self, f: Index) -> Index {
        self.child_face_parent_index[f as usize]
    }
    #[inline]
    pub fn get_child_face_in_parent_face(&self, f: Index) -> i32 {
        self.child_face_tag[f as usize].index_in_parent as i32
    }
    #[inline]
    pub fn get_child_edge_parent_index(&self, e: Index) -> Index {
        self.child_edge_parent_index[e as usize]
    }
    #[inline]
    pub fn get_child_vertex_parent_index(&self, v: Index) -> Index {
        self.child_vertex_parent_index[v as usize]
    }
    #[inline]
    pub fn is_child_vertex_complete(&self, v: Index) -> bool {
        !self.child_vertex_tag[v as usize].incomplete
    }

    // ---- child tag accessors ----
    #[inline]
    pub fn get_child_face_tag(&self, f: Index) -> &ChildTag {
        &self.child_face_tag[f as usize]
    }
    #[inline]
    pub fn get_child_edge_tag(&self, e: Index) -> &ChildTag {
        &self.child_edge_tag[e as usize]
    }
    #[inline]
    pub fn get_child_vertex_tag(&self, v: Index) -> &ChildTag {
        &self.child_vertex_tag[v as usize]
    }

    #[inline]
    pub fn get_child_face_tag_mut(&mut self, f: Index) -> &mut ChildTag {
        &mut self.child_face_tag[f as usize]
    }
    #[inline]
    pub fn get_child_edge_tag_mut(&mut self, e: Index) -> &mut ChildTag {
        &mut self.child_edge_tag[e as usize]
    }
    #[inline]
    pub fn get_child_vertex_tag_mut(&mut self, v: Index) -> &mut ChildTag {
        &mut self.child_vertex_tag[v as usize]
    }

    // ---- sparse tag accessors ----
    #[inline]
    pub fn get_parent_face_sparse_tag(&self, f: Index) -> &SparseTag {
        &self.parent_face_tag[f as usize]
    }
    #[inline]
    pub fn get_parent_edge_sparse_tag(&self, e: Index) -> &SparseTag {
        &self.parent_edge_tag[e as usize]
    }
    #[inline]
    pub fn get_parent_vertex_sparse_tag(&self, v: Index) -> &SparseTag {
        &self.parent_vertex_tag[v as usize]
    }

    #[inline]
    pub fn get_parent_face_sparse_tag_mut(&mut self, f: Index) -> &mut SparseTag {
        &mut self.parent_face_tag[f as usize]
    }
    #[inline]
    pub fn get_parent_edge_sparse_tag_mut(&mut self, e: Index) -> &mut SparseTag {
        &mut self.parent_edge_tag[e as usize]
    }
    #[inline]
    pub fn get_parent_vertex_sparse_tag_mut(&mut self, v: Index) -> &mut SparseTag {
        &mut self.parent_vertex_tag[v as usize]
    }

    // ---- FVar channel accessors ----
    #[inline]
    pub fn get_num_fvar_channels(&self) -> i32 {
        self.fvar_channels.len() as i32
    }
    #[inline]
    pub fn get_fvar_refinement(&self, c: i32) -> &FVarRefinement {
        &self.fvar_channels[c as usize]
    }

    // =========================================================================
    // Core refinement entry point
    // =========================================================================

    /// Apply refinement, building the child level from the parent level.
    ///
    /// Dispatches to the callbacks stored by QuadRefinement or TriRefinement.
    /// Panics if the topology-populate callbacks have not been wired up, i.e.
    /// if this bare Refinement was constructed without going through
    /// QuadRefinement::new() or TriRefinement::new().
    ///
    /// Mirrors C++ `Refinement::refine()` (which calls virtual methods).
    pub fn refine(&mut self, opts: RefinementOptions) {
        // Topology callbacks must have been set by the concrete sub-type wrapper.
        // If they are None the caller constructed a bare Refinement directly, which
        // is unsupported (mirrors C++ pure-virtual contract).
        assert!(
            self.allocate_fn.is_some() && self.populate_fv_fn.is_some(),
            "Refinement::refine() called without topology callbacks — \
             use QuadRefinement::refine() or TriRefinement::refine() instead"
        );

        self.uniform = !opts.sparse;
        self.face_verts_first = opts.face_verts_first;

        let has_fvar = unsafe { (*self.parent).get_num_fvar_channels() > 0 };

        // Phase 1: parent->child index arrays (scheme-specific allocation via callback)
        (self.allocate_fn.unwrap())(self);
        // Sparse: apply selection marks before sequencing
        if !self.uniform {
            debug_assert!(
                !self.parent_vertex_tag.is_empty(),
                "Sparse tags must be initialized before refine()"
            );
            self.mark_sparse_vertex_children();
            self.mark_sparse_edge_children();
            (self.sparse_face_fn.unwrap())(self);
        }
        // Sequence indices (assign final child-component indices)
        self.populate_parent_child_indices();
        self.initialize_child_component_counts();

        // Phase 2: child->parent maps + component tags
        self.populate_child_to_parent_mapping();
        self.propagate_component_tags();

        // Phase 3: topology relations (dispatch via callbacks, not stubs)
        let mut relations = if opts.minimal_topology {
            let mut r = Relations::all_false();
            r.face_vertices = true;
            r
        } else {
            Relations::all_true()
        };
        if has_fvar {
            relations.vertex_faces = true;
        }

        if relations.face_vertices {
            (self.populate_fv_fn.unwrap())(self);
        }
        if relations.face_edges {
            (self.populate_fe_fn.unwrap())(self);
        }
        if relations.edge_vertices {
            (self.populate_ev_fn.unwrap())(self);
        }
        if relations.edge_faces {
            (self.populate_ef_fn.unwrap())(self);
        }
        if relations.vertex_faces {
            (self.populate_vf_fn.unwrap())(self);
        }
        if relations.vertex_edges {
            (self.populate_ve_fn.unwrap())(self);
        }

        // Post-hoc floor formula for child max_valence (C++ refinement.cpp:810-816).
        unsafe {
            let p = &*self.parent;
            let c = &mut *self.child;
            if self.split_type == crate::sdc::types::Split::ToQuads {
                c.max_valence = c.max_valence.max(p.max_valence).max(4);
                c.max_valence = c.max_valence.max(2 + p.max_edge_faces);
            } else {
                c.max_valence = c.max_valence.max(p.max_valence).max(6);
                c.max_valence = c.max_valence.max(2 + p.max_edge_faces * 2);
            }
        }

        // Phase 4: sharpness + FVar
        self.subdivide_sharpness_values();
        if has_fvar {
            self.subdivide_fvar_channels();
        }
    }
    // =========================================================================
    // Parent → child mapping
    // =========================================================================

    fn populate_parent_child_indices(&mut self) {
        if self.uniform {
            self.first_child_face_from_face = 0;
            self.child_face_from_face_count = sequence_full_index_vector(
                &mut self.face_child_face_indices,
                self.first_child_face_from_face,
            );

            self.first_child_edge_from_face = 0;
            self.child_edge_from_face_count = sequence_full_index_vector(
                &mut self.face_child_edge_indices,
                self.first_child_edge_from_face,
            );

            self.first_child_edge_from_edge = self.child_edge_from_face_count;
            self.child_edge_from_edge_count = sequence_full_index_vector(
                &mut self.edge_child_edge_indices,
                self.first_child_edge_from_edge,
            );

            if self.face_verts_first {
                self.first_child_vert_from_face = 0;
                self.child_vert_from_face_count = sequence_full_index_vector(
                    &mut self.face_child_vert_index,
                    self.first_child_vert_from_face,
                );

                self.first_child_vert_from_edge =
                    self.first_child_vert_from_face + self.child_vert_from_face_count;
                self.child_vert_from_edge_count = sequence_full_index_vector(
                    &mut self.edge_child_vert_index,
                    self.first_child_vert_from_edge,
                );

                self.first_child_vert_from_vert =
                    self.first_child_vert_from_edge + self.child_vert_from_edge_count;
                self.child_vert_from_vert_count = sequence_full_index_vector(
                    &mut self.vert_child_vert_index,
                    self.first_child_vert_from_vert,
                );
            } else {
                self.first_child_vert_from_vert = 0;
                self.child_vert_from_vert_count = sequence_full_index_vector(
                    &mut self.vert_child_vert_index,
                    self.first_child_vert_from_vert,
                );

                self.first_child_vert_from_face =
                    self.first_child_vert_from_vert + self.child_vert_from_vert_count;
                self.child_vert_from_face_count = sequence_full_index_vector(
                    &mut self.face_child_vert_index,
                    self.first_child_vert_from_face,
                );

                self.first_child_vert_from_edge =
                    self.first_child_vert_from_face + self.child_vert_from_face_count;
                self.child_vert_from_edge_count = sequence_full_index_vector(
                    &mut self.edge_child_vert_index,
                    self.first_child_vert_from_edge,
                );
            }
        } else {
            // Sparse
            self.first_child_face_from_face = 0;
            self.child_face_from_face_count = sequence_sparse_index_vector(
                &mut self.face_child_face_indices,
                self.first_child_face_from_face,
            );

            self.first_child_edge_from_face = 0;
            self.child_edge_from_face_count = sequence_sparse_index_vector(
                &mut self.face_child_edge_indices,
                self.first_child_edge_from_face,
            );

            self.first_child_edge_from_edge = self.child_edge_from_face_count;
            self.child_edge_from_edge_count = sequence_sparse_index_vector(
                &mut self.edge_child_edge_indices,
                self.first_child_edge_from_edge,
            );

            if self.face_verts_first {
                self.first_child_vert_from_face = 0;
                self.child_vert_from_face_count = sequence_sparse_index_vector(
                    &mut self.face_child_vert_index,
                    self.first_child_vert_from_face,
                );

                self.first_child_vert_from_edge =
                    self.first_child_vert_from_face + self.child_vert_from_face_count;
                self.child_vert_from_edge_count = sequence_sparse_index_vector(
                    &mut self.edge_child_vert_index,
                    self.first_child_vert_from_edge,
                );

                self.first_child_vert_from_vert =
                    self.first_child_vert_from_edge + self.child_vert_from_edge_count;
                self.child_vert_from_vert_count = sequence_sparse_index_vector(
                    &mut self.vert_child_vert_index,
                    self.first_child_vert_from_vert,
                );
            } else {
                self.first_child_vert_from_vert = 0;
                self.child_vert_from_vert_count = sequence_sparse_index_vector(
                    &mut self.vert_child_vert_index,
                    self.first_child_vert_from_vert,
                );

                self.first_child_vert_from_face =
                    self.first_child_vert_from_vert + self.child_vert_from_vert_count;
                self.child_vert_from_face_count = sequence_sparse_index_vector(
                    &mut self.face_child_vert_index,
                    self.first_child_vert_from_face,
                );

                self.first_child_vert_from_edge =
                    self.first_child_vert_from_face + self.child_vert_from_face_count;
                self.child_vert_from_edge_count = sequence_sparse_index_vector(
                    &mut self.edge_child_vert_index,
                    self.first_child_vert_from_edge,
                );
            }
        }
    }

    /// Assign child component counts to the child Level.
    pub fn initialize_child_component_counts(&mut self) {
        let child = unsafe { &mut *self.child };
        child.face_count = self.child_face_from_face_count;
        child.edge_count = self.child_edge_from_face_count + self.child_edge_from_edge_count;
        child.vert_count = self.child_vert_from_face_count
            + self.child_vert_from_edge_count
            + self.child_vert_from_vert_count;
    }

    /// Initialise the sparse selection tag vectors (called before SparseSelector populates them).
    pub fn initialize_sparse_selection_tags(&mut self) {
        let p = self.parent();
        let nf = p.get_num_faces() as usize;
        let ne = p.get_num_edges() as usize;
        let nv = p.get_num_vertices() as usize;
        self.parent_face_tag.resize(nf, SparseTag::default());
        self.parent_edge_tag.resize(ne, SparseTag::default());
        self.parent_vertex_tag.resize(nv, SparseTag::default());
    }

    // =========================================================================
    // Child → parent mapping
    // =========================================================================

    fn populate_child_to_parent_mapping(&mut self) {
        // Build initial ChildTag templates: [complete=0, incomplete=1][indexInParent 0-3].
        // Uses new_complete / new_incomplete constructors (C++ style initialization).
        let initial: [[ChildTag; 4]; 2] = [
            [
                ChildTag::new_complete(0),
                ChildTag::new_complete(1),
                ChildTag::new_complete(2),
                ChildTag::new_complete(3),
            ],
            [
                ChildTag::new_incomplete(0),
                ChildTag::new_incomplete(1),
                ChildTag::new_incomplete(2),
                ChildTag::new_incomplete(3),
            ],
        ];
        self.populate_face_parent_vectors(&initial);
        self.populate_edge_parent_vectors(&initial);
        self.populate_vertex_parent_vectors(&initial);
    }

    fn populate_face_parent_vectors(&mut self, tags: &[[ChildTag; 4]; 2]) {
        let nfc = self.child().get_num_faces() as usize;
        self.child_face_tag.resize(nfc, ChildTag::default());
        self.child_face_parent_index.resize(nfc, INDEX_INVALID);
        self.populate_face_parent_from_parent_faces(tags);
    }

    fn populate_face_parent_from_parent_faces(&mut self, tags: &[[ChildTag; 4]; 2]) {
        if self.uniform {
            let mut c_face = self.first_child_face_from_face;
            for p_face in 0..self.parent().get_num_faces() {
                // safe copy to avoid simultaneous mutable borrow
                let c_faces: Vec<Index> = self.get_face_child_faces(p_face).to_vec();
                let n = c_faces.len();
                let too_large = n > 4;
                for (i, &cf) in c_faces.iter().enumerate() {
                    if n == 4 {
                        self.child_face_tag[c_face as usize] = tags[0][i];
                    } else {
                        self.child_face_tag[c_face as usize] =
                            tags[0][if too_large { 0 } else { i }];
                    }
                    self.child_face_parent_index[c_face as usize] = p_face;
                    let _ = cf;
                    c_face += 1;
                }
            }
        } else {
            for p_face in 0..self.parent().get_num_faces() {
                let incomplete = !self.parent_face_tag[p_face as usize].selected;
                let c_faces: Vec<Index> = self.get_face_child_faces(p_face).to_vec();
                let n = c_faces.len();
                let too_large = n > 4;
                if !incomplete && n == 4 {
                    for (i, &cf) in c_faces.iter().enumerate() {
                        self.child_face_tag[cf as usize] = tags[0][i];
                        self.child_face_parent_index[cf as usize] = p_face;
                    }
                } else {
                    for (i, &cf) in c_faces.iter().enumerate() {
                        if index_is_valid(cf) {
                            self.child_face_tag[cf as usize] =
                                tags[if incomplete { 1 } else { 0 }][if too_large { 0 } else { i }];
                            self.child_face_parent_index[cf as usize] = p_face;
                        }
                    }
                }
            }
        }
    }

    fn populate_edge_parent_vectors(&mut self, tags: &[[ChildTag; 4]; 2]) {
        let nce = self.child().get_num_edges() as usize;
        self.child_edge_tag.resize(nce, ChildTag::default());
        self.child_edge_parent_index.resize(nce, INDEX_INVALID);
        self.populate_edge_parent_from_parent_faces(tags);
        self.populate_edge_parent_from_parent_edges(tags);
    }

    fn populate_edge_parent_from_parent_faces(&mut self, tags: &[[ChildTag; 4]; 2]) {
        if self.uniform {
            let mut c_edge = self.first_child_edge_from_face;
            for p_face in 0..self.parent().get_num_faces() {
                let c_edges: Vec<Index> = self.get_face_child_edges(p_face).to_vec();
                let n = c_edges.len();
                let too_large = n > 4;
                for (i, &ce) in c_edges.iter().enumerate() {
                    self.child_edge_tag[c_edge as usize] = tags[0][if too_large { 0 } else { i }];
                    self.child_edge_parent_index[c_edge as usize] = p_face;
                    let _ = ce;
                    c_edge += 1;
                }
            }
        } else {
            for p_face in 0..self.parent().get_num_faces() {
                let incomplete = !self.parent_face_tag[p_face as usize].selected;
                let c_edges: Vec<Index> = self.get_face_child_edges(p_face).to_vec();
                let n = c_edges.len();
                let too_large = n > 4;
                if !incomplete && n == 4 {
                    for (i, &ce) in c_edges.iter().enumerate() {
                        self.child_edge_tag[ce as usize] = tags[0][i];
                        self.child_edge_parent_index[ce as usize] = p_face;
                    }
                } else {
                    for (i, &ce) in c_edges.iter().enumerate() {
                        if index_is_valid(ce) {
                            self.child_edge_tag[ce as usize] =
                                tags[if incomplete { 1 } else { 0 }][if too_large { 0 } else { i }];
                            self.child_edge_parent_index[ce as usize] = p_face;
                        }
                    }
                }
            }
        }
    }

    fn populate_edge_parent_from_parent_edges(&mut self, tags: &[[ChildTag; 4]; 2]) {
        if self.uniform {
            let mut c_edge = self.first_child_edge_from_edge;
            for p_edge in 0..self.parent().get_num_edges() {
                self.child_edge_tag[c_edge as usize] = tags[0][0];
                self.child_edge_tag[c_edge as usize + 1] = tags[0][1];
                self.child_edge_parent_index[c_edge as usize] = p_edge;
                self.child_edge_parent_index[c_edge as usize + 1] = p_edge;
                c_edge += 2;
            }
        } else {
            for p_edge in 0..self.parent().get_num_edges() {
                let incomplete = !self.parent_edge_tag[p_edge as usize].selected;
                let c_edges = *self.get_edge_child_edges(p_edge);
                if !incomplete {
                    self.child_edge_tag[c_edges[0] as usize] = tags[0][0];
                    self.child_edge_tag[c_edges[1] as usize] = tags[0][1];
                    self.child_edge_parent_index[c_edges[0] as usize] = p_edge;
                    self.child_edge_parent_index[c_edges[1] as usize] = p_edge;
                } else {
                    for i in 0..2 {
                        if index_is_valid(c_edges[i]) {
                            self.child_edge_tag[c_edges[i] as usize] = tags[1][i];
                            self.child_edge_parent_index[c_edges[i] as usize] = p_edge;
                        }
                    }
                }
            }
        }
    }

    fn populate_vertex_parent_vectors(&mut self, tags: &[[ChildTag; 4]; 2]) {
        let ncv = self.child().get_num_vertices() as usize;
        if self.uniform {
            self.child_vertex_tag.resize(ncv, tags[0][0]);
        } else {
            self.child_vertex_tag.resize(ncv, tags[1][0]);
        }
        self.child_vertex_parent_index.resize(ncv, INDEX_INVALID);

        self.populate_vertex_parent_from_parent_faces(tags);
        self.populate_vertex_parent_from_parent_edges(tags);
        self.populate_vertex_parent_from_parent_vertices(tags);
    }

    fn populate_vertex_parent_from_parent_faces(&mut self, tags: &[[ChildTag; 4]; 2]) {
        if self.child_vert_from_face_count == 0 {
            return;
        }

        if self.uniform {
            let c_vert_start = self.first_child_vert_from_face;
            for p_face in 0..self.parent().get_num_faces() {
                let c_vert = c_vert_start + p_face;
                self.child_vertex_parent_index[c_vert as usize] = p_face;
            }
        } else {
            let complete_tag = tags[0][0];
            for p_face in 0..self.parent().get_num_faces() {
                let c_vert = self.face_child_vert_index[p_face as usize];
                if index_is_valid(c_vert) {
                    if self.parent_face_tag[p_face as usize].selected {
                        self.child_vertex_tag[c_vert as usize] = complete_tag;
                    }
                    self.child_vertex_parent_index[c_vert as usize] = p_face;
                }
            }
        }
    }

    fn populate_vertex_parent_from_parent_edges(&mut self, tags: &[[ChildTag; 4]; 2]) {
        if self.uniform {
            let c_vert_start = self.first_child_vert_from_edge;
            for p_edge in 0..self.parent().get_num_edges() {
                let c_vert = c_vert_start + p_edge;
                self.child_vertex_parent_index[c_vert as usize] = p_edge;
            }
        } else {
            let complete_tag = tags[0][0];
            for p_edge in 0..self.parent().get_num_edges() {
                let c_vert = self.edge_child_vert_index[p_edge as usize];
                if index_is_valid(c_vert) {
                    if self.parent_edge_tag[p_edge as usize].selected {
                        self.child_vertex_tag[c_vert as usize] = complete_tag;
                    }
                    self.child_vertex_parent_index[c_vert as usize] = p_edge;
                }
            }
        }
    }

    fn populate_vertex_parent_from_parent_vertices(&mut self, tags: &[[ChildTag; 4]; 2]) {
        if self.uniform {
            let c_vert_start = self.first_child_vert_from_vert;
            for p_vert in 0..self.parent().get_num_vertices() {
                let c_vert = c_vert_start + p_vert;
                self.child_vertex_parent_index[c_vert as usize] = p_vert;
            }
        } else {
            let complete_tag = tags[0][0];
            for p_vert in 0..self.parent().get_num_vertices() {
                let c_vert = self.vert_child_vert_index[p_vert as usize];
                if index_is_valid(c_vert) {
                    if self.parent_vertex_tag[p_vert as usize].selected {
                        self.child_vertex_tag[c_vert as usize] = complete_tag;
                    }
                    self.child_vertex_parent_index[c_vert as usize] = p_vert;
                }
            }
        }
    }

    // =========================================================================
    // Component tag propagation
    // =========================================================================

    fn propagate_component_tags(&mut self) {
        self.populate_face_tag_vectors();
        self.populate_edge_tag_vectors();
        self.populate_vertex_tag_vectors();
    }

    fn populate_face_tag_vectors(&mut self) {
        let nfc = self.child().get_num_faces() as usize;
        unsafe { (&mut *self.child).face_tags.resize(nfc, Default::default()) };
        self.populate_face_tags_from_parent_faces();
    }

    fn populate_face_tags_from_parent_faces(&mut self) {
        let c_face_start = self.first_child_face_from_face as usize;
        let c_face_end = c_face_start + self.child_face_from_face_count as usize;
        for c_face in c_face_start..c_face_end {
            let p_face = self.child_face_parent_index[c_face] as usize;
            unsafe {
                let child = &mut *self.child;
                let parent = &*self.parent;
                child.face_tags[c_face] = parent.face_tags[p_face];
            }
        }
    }

    fn populate_edge_tag_vectors(&mut self) {
        let nce = self.child().get_num_edges() as usize;
        unsafe { (&mut *self.child).edge_tags.resize(nce, Default::default()) };
        self.populate_edge_tags_from_parent_faces();
        self.populate_edge_tags_from_parent_edges();
    }

    fn populate_edge_tags_from_parent_faces(&mut self) {
        // All edges from parent faces are interior — zero/clear tag
        let c_edge_start = self.first_child_edge_from_face as usize;
        let c_edge_end = c_edge_start + self.child_edge_from_face_count as usize;
        unsafe {
            let child = &mut *self.child;
            for c_edge in c_edge_start..c_edge_end {
                child.edge_tags[c_edge] = Default::default();
            }
        }
    }

    fn populate_edge_tags_from_parent_edges(&mut self) {
        let c_edge_start = self.first_child_edge_from_edge as usize;
        let c_edge_end = c_edge_start + self.child_edge_from_edge_count as usize;
        for c_edge in c_edge_start..c_edge_end {
            let p_edge = self.child_edge_parent_index[c_edge] as usize;
            unsafe {
                let child = &mut *self.child;
                let parent = &*self.parent;
                child.edge_tags[c_edge] = parent.edge_tags[p_edge];
            }
        }
    }

    fn populate_vertex_tag_vectors(&mut self) {
        let ncv = self.child().get_num_vertices() as usize;
        unsafe { (&mut *self.child).vert_tags.resize(ncv, Default::default()) };

        self.populate_vertex_tags_from_parent_faces();
        self.populate_vertex_tags_from_parent_edges();
        self.populate_vertex_tags_from_parent_vertices();

        if !self.uniform {
            let ncv = self.child().get_num_vertices();
            for c_vert in 0..ncv {
                if self.child_vertex_tag[c_vert as usize].incomplete {
                    unsafe {
                        let child = &mut *self.child;
                        child.vert_tags[c_vert as usize].set_incomplete(true);
                    }
                }
            }
        }
    }

    fn populate_vertex_tags_from_parent_faces(&mut self) {
        if self.child_vert_from_face_count == 0 {
            return;
        }

        let c_vert_start = self.first_child_vert_from_face as usize;
        let c_vert_end = c_vert_start + self.child_vert_from_face_count as usize;
        let p_depth = unsafe { (*self.parent).depth };
        let reg = self.reg_face_size;

        unsafe {
            let child = &mut *self.child;
            let parent = &*self.parent;
            let mut vtag = super::level::VTag::default();
            vtag.set_rule(Rule::Smooth as u16);

            if p_depth > 0 {
                for c_vert in c_vert_start..c_vert_end {
                    child.vert_tags[c_vert] = vtag;
                }
            } else {
                for c_vert in c_vert_start..c_vert_end {
                    child.vert_tags[c_vert] = vtag;
                    let p_face = self.child_vertex_parent_index[c_vert] as usize;
                    let pf_size = parent.get_num_face_vertices(p_face as Index);
                    if pf_size != reg {
                        child.vert_tags[c_vert].set_xordinary(true);
                    }
                }
            }
        }
    }

    fn populate_vertex_tags_from_parent_edges(&mut self) {
        unsafe {
            let child = &mut *self.child;
            let parent = &*self.parent;
            for p_edge in 0..parent.get_num_edges() {
                let c_vert = self.edge_child_vert_index[p_edge as usize];
                if !index_is_valid(c_vert) {
                    continue;
                }

                let p_et = parent.edge_tags[p_edge as usize];
                let mut vtag = super::level::VTag::default();

                vtag.set_non_manifold(p_et.non_manifold());
                vtag.set_boundary(p_et.boundary());
                vtag.set_semi_sharp_edges(p_et.semi_sharp());
                vtag.set_inf_sharp_edges(p_et.inf_sharp());
                vtag.set_inf_sharp_crease(p_et.inf_sharp());
                vtag.set_inf_irregular(p_et.inf_sharp() && p_et.non_manifold());

                let rule = if p_et.semi_sharp() || p_et.inf_sharp() {
                    Rule::Crease as u16
                } else {
                    Rule::Smooth as u16
                };
                vtag.set_rule(rule);

                child.vert_tags[c_vert as usize] = vtag;
            }
        }
    }

    fn populate_vertex_tags_from_parent_vertices(&mut self) {
        let c_vert_start = self.first_child_vert_from_vert as usize;
        let c_vert_end = c_vert_start + self.child_vert_from_vert_count as usize;
        for c_vert in c_vert_start..c_vert_end {
            let p_vert = self.child_vertex_parent_index[c_vert] as usize;
            unsafe {
                let child = &mut *self.child;
                let parent = &*self.parent;
                child.vert_tags[c_vert] = parent.vert_tags[p_vert];
                child.vert_tags[c_vert].set_incid_irreg_face(false);
            }
        }
    }

    // =========================================================================
    // Topology subdivision — virtual methods supplied by subclass
    // =========================================================================

    /// Subdivide all requested topology relations via scheme-specific callbacks.
    /// Called internally by refine_with_callbacks(); also accessible directly.
    pub fn subdivide_topology(&mut self, apply_to: Relations) {
        if apply_to.face_vertices {
            if let Some(f) = self.populate_fv_fn {
                f(self);
            }
        }
        if apply_to.face_edges {
            if let Some(f) = self.populate_fe_fn {
                f(self);
            }
        }
        if apply_to.edge_vertices {
            if let Some(f) = self.populate_ev_fn {
                f(self);
            }
        }
        if apply_to.edge_faces {
            if let Some(f) = self.populate_ef_fn {
                f(self);
            }
        }
        if apply_to.vertex_faces {
            if let Some(f) = self.populate_vf_fn {
                f(self);
            }
        }
        if apply_to.vertex_edges {
            if let Some(f) = self.populate_ve_fn {
                f(self);
            }
        }

        // Post-hoc floor formula for child max_valence (C++ refinement.cpp:810-816).
        // Incremental tracking via resize_vertex_edges() may under-count when the
        // parent is trivial (low valence) or has non-manifold edges with many faces.
        unsafe {
            let p = &*self.parent;
            let c = &mut *self.child;
            if self.split_type == crate::sdc::types::Split::ToQuads {
                c.max_valence = c.max_valence.max(p.max_valence).max(4);
                c.max_valence = c.max_valence.max(2 + p.max_edge_faces);
            } else {
                c.max_valence = c.max_valence.max(p.max_valence).max(6);
                c.max_valence = c.max_valence.max(2 + p.max_edge_faces * 2);
            }
        }
    }

    // Allocation stub: calls scheme-specific allocate_fn if set.
    // Called indirectly through the callback mechanism in refine(); exposed here
    // for completeness mirroring C++ Refinement::allocateParentChildIndices().
    #[allow(dead_code)]
    pub(crate) fn allocate_parent_child_indices(&mut self) {
        if let Some(f) = self.allocate_fn {
            f(self);
        }
    }

    // Default sparse face children — calls callback if set, otherwise no-op.
    // Mirrors C++ virtual Refinement::markSparseFaceChildren(); invoked through
    // sparse_face_fn callback in refine(), not called directly.
    #[allow(dead_code)]
    pub(crate) fn mark_sparse_face_children(&mut self) {
        if let Some(f) = self.sparse_face_fn {
            f(self);
        }
    }

    // =========================================================================
    // Sharpness subdivision
    // =========================================================================

    pub fn subdivide_sharpness_values(&mut self) {
        self.subdivide_edge_sharpness();
        self.subdivide_vertex_sharpness();
        self.reclassify_semisharp_vertices();
    }

    fn subdivide_edge_sharpness(&mut self) {
        let crease = Crease::with_options(self.options);

        let nce = self.child().get_num_edges() as usize;
        unsafe { (&mut *self.child).edge_sharpness.clear() };
        unsafe {
            (&mut *self.child)
                .edge_sharpness
                .resize(nce, SHARPNESS_SMOOTH)
        };

        let c_edge_start = self.first_child_edge_from_edge as usize;
        let c_edge_end = c_edge_start + self.child_edge_from_edge_count as usize;

        for c_edge in c_edge_start..c_edge_end {
            unsafe {
                // Read tag and sharpness (Copy types)
                let c_et = (&*self.child).edge_tags[c_edge];

                if c_et.inf_sharp() {
                    (&mut *self.child).edge_sharpness[c_edge] = SHARPNESS_INFINITE;
                } else if c_et.semi_sharp() {
                    let p_edge = self.child_edge_parent_index[c_edge] as usize;
                    let p_sharpness = (&*self.parent).edge_sharpness[p_edge];

                    let new_sharp = if crease.is_uniform() {
                        crease.subdivide_uniform_sharpness(p_sharpness)
                    } else {
                        let p_edge_verts = (&*self.parent).get_edge_vertices(p_edge as Index);
                        let idx_in_parent = self.child_edge_tag[c_edge].index_in_parent as usize;
                        let p_vert = p_edge_verts[idx_in_parent];
                        let p_vert_edges = (&*self.parent).get_vertex_edges(p_vert);
                        let sharp_buf: Vec<f32> = p_vert_edges
                            .as_slice()
                            .iter()
                            .map(|&e| (&*self.parent).edge_sharpness[e as usize])
                            .collect();
                        crease.subdivide_edge_sharpness_at_vertex(p_sharpness, &sharp_buf)
                    };
                    (&mut *self.child).edge_sharpness[c_edge] = new_sharp;
                    if !is_sharp(new_sharp) {
                        (&mut (*self.child).edge_tags)[c_edge].set_semi_sharp(false);
                    }
                }
            }
        }
    }

    fn subdivide_vertex_sharpness(&mut self) {
        let crease = Crease::with_options(self.options);

        let ncv = self.child().get_num_vertices() as usize;
        unsafe { (&mut *self.child).vert_sharpness.clear() };
        unsafe {
            (&mut *self.child)
                .vert_sharpness
                .resize(ncv, SHARPNESS_SMOOTH)
        };

        let c_vert_start = self.first_child_vert_from_vert as usize;
        let c_vert_end = c_vert_start + self.child_vert_from_vert_count as usize;

        for c_vert in c_vert_start..c_vert_end {
            unsafe {
                let c_vt = (&*self.child).vert_tags[c_vert];

                if c_vt.inf_sharp() {
                    (&mut *self.child).vert_sharpness[c_vert] = SHARPNESS_INFINITE;
                } else if c_vt.semi_sharp() {
                    let p_vert = self.child_vertex_parent_index[c_vert] as usize;
                    let p_sharpness = (&*self.parent).vert_sharpness[p_vert];

                    let new_sharp = crease.subdivide_vertex_sharpness(p_sharpness);
                    (&mut *self.child).vert_sharpness[c_vert] = new_sharp;
                    if !is_sharp(new_sharp) {
                        (&mut (*self.child).vert_tags)[c_vert].set_semi_sharp(false);
                    }
                }
            }
        }
    }

    fn reclassify_semisharp_vertices(&mut self) {
        let crease = Crease::with_options(self.options);

        // --- vertices from edges ---
        let from_edge_start = self.first_child_vert_from_edge;
        let from_edge_end = from_edge_start + self.child_vert_from_edge_count;

        for c_vert in from_edge_start..from_edge_end {
            unsafe {
                let c_vt = (&*self.child).vert_tags[c_vert as usize];
                if !c_vt.semi_sharp_edges() {
                    continue;
                }

                let p_edge = self.child_vertex_parent_index[c_vert as usize];
                let c_edges = *self.get_edge_child_edges(p_edge);

                if self.child_vertex_tag[c_vert as usize].incomplete {
                    let semi = (index_is_valid(c_edges[0])
                        && (&*self.child).edge_tags[c_edges[0] as usize].semi_sharp())
                        | (index_is_valid(c_edges[1])
                            && (&*self.child).edge_tags[c_edges[1] as usize].semi_sharp());
                    (&mut (*self.child).vert_tags)[c_vert as usize].set_semi_sharp_edges(semi);
                    (&mut (*self.child).vert_tags)[c_vert as usize].set_rule(if semi {
                        Rule::Crease as u16
                    } else {
                        Rule::Smooth as u16
                    });
                } else {
                    let sharp_count = (&*self.child).edge_tags[c_edges[0] as usize].semi_sharp()
                        as i32
                        + (&*self.child).edge_tags[c_edges[1] as usize].semi_sharp() as i32;
                    (&mut (*self.child).vert_tags)[c_vert as usize]
                        .set_semi_sharp_edges(sharp_count > 0);
                    let rule = crease.determine_vertex_vertex_rule_from_count(0.0, sharp_count);
                    (&mut (*self.child).vert_tags)[c_vert as usize].set_rule(rule as u16);
                }
            }
        }

        // --- vertices from vertices ---
        let from_vert_start = self.first_child_vert_from_vert;
        let from_vert_end = from_vert_start + self.child_vert_from_vert_count;

        for c_vert in from_vert_start..from_vert_end {
            unsafe {
                let p_vert = self.child_vertex_parent_index[c_vert as usize];
                let p_vt = (&*self.parent).vert_tags[p_vert as usize];
                let p_vtag_semi = p_vt.semi_sharp();
                let p_vtag_semi_edges = p_vt.semi_sharp_edges();

                if !p_vtag_semi && !p_vtag_semi_edges {
                    continue;
                }

                let c_vt = (&*self.child).vert_tags[c_vert as usize];
                let sharp_decayed = p_vtag_semi && !c_vt.semi_sharp();

                if p_vtag_semi_edges || sharp_decayed {
                    let mut inf_count = 0i32;
                    let mut semi_count = 0i32;

                    if (&*self.child).get_num_vertex_edges_total() > 0 {
                        // Collect edge tags first (avoid conflicting borrow with write)
                        let c_edges_vec: Vec<Index> =
                            (&*self.child).get_vertex_edges(c_vert).as_slice().to_vec();
                        for ce in c_edges_vec {
                            let ce_tag = (&*self.child).edge_tags[ce as usize];
                            inf_count += ce_tag.inf_sharp() as i32;
                            semi_count += ce_tag.semi_sharp() as i32;
                        }
                    } else {
                        let p_edges_vec: Vec<Index> =
                            (&*self.parent).get_vertex_edges(p_vert).as_slice().to_vec();
                        let p_local_vec: Vec<super::types::LocalIndex> = (&*self.parent)
                            .get_vertex_edge_local_indices(p_vert)
                            .as_slice()
                            .to_vec();
                        for (i, pe) in p_edges_vec.iter().enumerate() {
                            let c_edge_pair = *self.get_edge_child_edges(*pe);
                            let local_idx = p_local_vec[i] as usize;
                            let ce = c_edge_pair[local_idx];
                            let ce_tag = (&*self.child).edge_tags[ce as usize];
                            inf_count += ce_tag.inf_sharp() as i32;
                            semi_count += ce_tag.semi_sharp() as i32;
                        }
                    }

                    (&mut (*self.child).vert_tags)[c_vert as usize]
                        .set_semi_sharp_edges(semi_count > 0);
                    if !c_vt.semi_sharp() && !c_vt.inf_sharp() {
                        let rule = crease
                            .determine_vertex_vertex_rule_from_count(0.0, inf_count + semi_count);
                        (&mut (*self.child).vert_tags)[c_vert as usize].set_rule(rule as u16);
                    }
                }
            }
        }
    }

    // =========================================================================
    // FVar channel subdivision
    // =========================================================================

    fn subdivide_fvar_channels(&mut self) {
        // Mirrors C++ `Refinement::subdivideFVarChannels()`:
        //   for each parent FVarLevel, create a child FVarLevel + FVarRefinement,
        //   call applyRefinement(), then push both into the child Level and self.
        //
        // SAFETY: parent and child raw pointers are valid for the lifetime of
        // this Refinement (guaranteed by TopologyRefiner). We use raw-pointer
        // access to avoid the borrow-checker conflict between &mut self and
        // simultaneous mutable access to parent/child Level channels.
        let n_channels = unsafe { (*self.parent).get_num_fvar_channels() };

        for channel in 0..n_channels {
            unsafe {
                // Get parent FVarLevel (shared ref via raw ptr — safe, read-only).
                let parent_fvar: *const super::fvar_level::FVarLevel =
                    &*(*self.parent).get_fvar_level(channel);

                // Create and push a fresh child FVarLevel into the child Level.
                let child_level_ptr: *mut Level = self.child;
                let child_fvar_channel = {
                    let child_level = &mut *child_level_ptr;
                    let opts = (*parent_fvar).get_options();
                    // Reserve space with 0 values — FVarRefinement::apply_refinement
                    // will populate the actual count via estimate_and_alloc_child_values.
                    let idx = child_level.create_fvar_channel(0, opts);
                    idx
                };

                // Build a FVarRefinement linking parent → child channel.
                // We must reborrow child fvar mutably here — the child level's
                // channel vector was just extended, so the pointer is stable.
                let child_fvar: *mut super::fvar_level::FVarLevel =
                    (*child_level_ptr).get_fvar_level_mut(child_fvar_channel);

                let mut refine_fvar = Box::new(FVarRefinement::with_refs(
                    // SAFETY: transmute lifetime — self outlives this call.
                    &*(self as *const Refinement),
                    &*parent_fvar,
                    &mut *child_fvar,
                    channel,
                ));

                refine_fvar.apply_refinement();

                self.fvar_channels.push(refine_fvar);
            }
        }
    }

    // =========================================================================
    // Sparse child marking
    // =========================================================================

    // Composite sparse-mark driver. Mirrors C++ markSparseChildComponentIndices().
    // In the callback-based Rust port, the three steps are called individually
    // from refine() directly; this method is kept for reference completeness.
    #[allow(dead_code)]
    fn mark_sparse_child_component_indices(&mut self) {
        self.mark_sparse_vertex_children();
        self.mark_sparse_edge_children();
        self.mark_sparse_face_children(); // virtual — overridden in subclass
    }

    fn mark_sparse_vertex_children(&mut self) {
        debug_assert!(!self.parent_vertex_tag.is_empty());
        for p_vert in 0..self.parent().get_num_vertices() {
            if self.parent_vertex_tag[p_vert as usize].selected {
                mark_sparse_selected(&mut self.vert_child_vert_index[p_vert as usize]);
            }
        }
    }

    fn mark_sparse_edge_children(&mut self) {
        debug_assert!(!self.parent_edge_tag.is_empty());
        for p_edge in 0..self.parent().get_num_edges() {
            let e_child_edges_off = (2 * p_edge) as usize;
            let e_verts: [Index; 2] = unsafe {
                let ev = (*self.parent).get_edge_vertices(p_edge);
                [ev[0], ev[1]]
            };

            if self.parent_edge_tag[p_edge as usize].selected {
                mark_sparse_selected(&mut self.edge_child_edge_indices[e_child_edges_off]);
                mark_sparse_selected(&mut self.edge_child_edge_indices[e_child_edges_off + 1]);
                mark_sparse_selected(&mut self.edge_child_vert_index[p_edge as usize]);
            } else {
                if self.parent_vertex_tag[e_verts[0] as usize].selected {
                    mark_sparse_neighbor(&mut self.edge_child_edge_indices[e_child_edges_off]);
                    mark_sparse_neighbor(&mut self.edge_child_vert_index[p_edge as usize]);
                }
                if self.parent_vertex_tag[e_verts[1] as usize].selected {
                    mark_sparse_neighbor(&mut self.edge_child_edge_indices[e_child_edges_off + 1]);
                    mark_sparse_neighbor(&mut self.edge_child_vert_index[p_edge as usize]);
                }
            }

            // Mark transitional bit based on incident face selection
            let e_faces = unsafe { (*self.parent).get_edge_faces(p_edge) };
            let transitional = if e_faces.size() == 2 {
                (self.parent_face_tag[e_faces[0] as usize].selected
                    != self.parent_face_tag[e_faces[1] as usize].selected) as u8
            } else if e_faces.size() < 2 {
                0u8
            } else {
                let first_sel = self.parent_face_tag[e_faces[0] as usize].selected;
                let trans = e_faces.as_slice()[1..]
                    .iter()
                    .any(|&f| self.parent_face_tag[f as usize].selected != first_sel);
                trans as u8
            };
            self.parent_edge_tag[p_edge as usize].transitional = transitional;
        }
    }

    // =========================================================================
    // counts/offsets helpers (shared with parent Level's face-vert arrays)
    // =========================================================================

    /// Return (count, offset) for face child-faces at parent face `f`.
    /// Priority:
    ///   1. shared = true  → read from parent Level's face-vert c/o
    ///   2. local_face_child_face_counts_offsets non-empty → use local vec (TriRefinement)
    ///   3. fall back to parent's get_num/get_offset helpers
    fn face_child_face_co(&self, f: Index) -> (usize, usize) {
        if self.face_child_face_counts_offsets_shared {
            unsafe {
                let co = &(*self.parent).face_vert_counts_offsets;
                let fi = (2 * f) as usize;
                (co[fi] as usize, co[fi + 1] as usize)
            }
        } else if !self.local_face_child_face_counts_offsets.is_empty() {
            // TriRefinement: local fixed-size counts/offsets (4 per face)
            let fi = (2 * f) as usize;
            let co = &self.local_face_child_face_counts_offsets;
            (co[fi] as usize, co[fi + 1] as usize)
        } else {
            // Fallback: read count/offset from parent Level directly
            let count = unsafe { (*self.parent).get_num_face_vertices(f) as usize };
            let offset = unsafe { (*self.parent).get_offset_of_face_vertices(f) as usize };
            (count, offset)
        }
    }

    fn face_child_edge_co(&self, f: Index) -> (usize, usize) {
        // Face-child-edges always share the parent face-vert c/o when the flag is set.
        // (For TriRefinement, face-child-edges are shared but face-child-faces are local.)
        if self.face_child_edge_counts_offsets_shared {
            unsafe {
                let co = &(*self.parent).face_vert_counts_offsets;
                let fi = (2 * f) as usize;
                (co[fi] as usize, co[fi + 1] as usize)
            }
        } else {
            // Fall back to parent Level helpers
            let count = unsafe { (*self.parent).get_num_face_vertices(f) as usize };
            let offset = unsafe { (*self.parent).get_offset_of_face_vertices(f) as usize };
            (count, offset)
        }
    }

    // =========================================================================
    // Debug / print
    // =========================================================================

    pub fn print_parent_to_child_mapping(&self) {
        let p = self.parent();
        for p_face in 0..p.get_num_faces() {
            println!("  Face {}:", p_face);
            println!(
                "    Child vert:  {}",
                self.face_child_vert_index[p_face as usize]
            );
            let cf = self.get_face_child_faces(p_face);
            println!("    Child faces: {:?}", cf);
            let ce = self.get_face_child_edges(p_face);
            println!("    Child edges: {:?}", ce);
        }
        for p_edge in 0..p.get_num_edges() {
            let ce = self.get_edge_child_edges(p_edge);
            println!(
                "  Edge {}: vert={}, edges=[{}, {}]",
                p_edge, self.edge_child_vert_index[p_edge as usize], ce[0], ce[1]
            );
        }
        for p_vert in 0..p.get_num_vertices() {
            println!(
                "  Vert {}: child_vert={}",
                p_vert, self.vert_child_vert_index[p_vert as usize]
            );
        }
    }

    // =========================================================================
    // Public accessors for QuadRefinement / TriRefinement overrides
    // =========================================================================

    /// Public alias for face_child_face_co (count, offset).
    #[inline]
    pub fn face_child_face_co_pub(&self, f: Index) -> (usize, usize) {
        self.face_child_face_co(f)
    }

    /// Public alias for face_child_edge_co (count, offset).
    #[inline]
    pub fn face_child_edge_co_pub(&self, f: Index) -> (usize, usize) {
        self.face_child_edge_co(f)
    }

    /// Virtual-dispatch variant of `refine()`: calls user-supplied callbacks
    /// for scheme-specific topology building methods.
    ///
    /// This mirrors C++ virtual dispatch: `allocate_parent_child_indices`,
    /// `mark_sparse_face_children`, and the six `populate_*_relation` methods
    /// are scheme-specific overrides called through the base `refine()` flow.
    #[allow(clippy::too_many_arguments)]
    pub fn refine_with_callbacks(
        &mut self,
        opts: RefinementOptions,
        allocate_parent_child_indices: fn(&mut Self),
        mark_sparse_face_children: fn(&mut Self),
        populate_fv: fn(&mut Self),
        populate_fe: fn(&mut Self),
        populate_ev: fn(&mut Self),
        populate_ef: fn(&mut Self),
        populate_vf: fn(&mut Self),
        populate_ve: fn(&mut Self),
    ) {
        self.uniform = !opts.sparse;
        self.face_verts_first = opts.face_verts_first;

        let has_fvar = unsafe { (&*self.parent).get_num_fvar_channels() > 0 };

        // Phase 1: parent->child index arrays (scheme-specific allocation)
        allocate_parent_child_indices(self);
        // Sparse: apply selection marks before sequencing
        if !self.uniform {
            debug_assert!(
                !self.parent_vertex_tag.is_empty(),
                "Sparse tags must be initialized before refine_with_callbacks"
            );
            // Mark edge/vert children from parent_vertex_tag
            self.mark_sparse_vertex_children();
            self.mark_sparse_edge_children();
            // Mark face children (scheme-specific)
            mark_sparse_face_children(self);
        }
        // Sequence indices (assign final child-component indices)
        self.populate_parent_child_indices();
        self.initialize_child_component_counts();

        // Phase 2: child->parent maps + component tags
        self.populate_child_to_parent_mapping();
        self.propagate_component_tags();

        // Phase 3: topology relations
        let mut relations = if opts.minimal_topology {
            let mut r = Relations::all_false();
            r.face_vertices = true;
            r
        } else {
            Relations::all_true()
        };
        if has_fvar {
            relations.vertex_faces = true;
        }

        if relations.face_vertices {
            populate_fv(self);
        }
        if relations.face_edges {
            populate_fe(self);
        }
        if relations.edge_vertices {
            populate_ev(self);
        }
        if relations.edge_faces {
            populate_ef(self);
        }
        if relations.vertex_faces {
            populate_vf(self);
        }
        if relations.vertex_edges {
            populate_ve(self);
        }

        // Post-hoc floor formula for child max_valence (C++ refinement.cpp:810-816).
        unsafe {
            let p = &*self.parent;
            let c = &mut *self.child;
            if self.split_type == crate::sdc::types::Split::ToQuads {
                c.max_valence = c.max_valence.max(p.max_valence).max(4);
                c.max_valence = c.max_valence.max(2 + p.max_edge_faces);
            } else {
                c.max_valence = c.max_valence.max(p.max_valence).max(6);
                c.max_valence = c.max_valence.max(2 + p.max_edge_faces * 2);
            }
        }

        // Phase 4: sharpness + FVar
        self.subdivide_sharpness_values();
        if has_fvar {
            self.subdivide_fvar_channels();
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_tag_defaults() {
        let t = SparseTag::default();
        assert!(!t.selected);
        assert_eq!(t.transitional, 0);
    }

    #[test]
    fn child_tag_defaults() {
        let t = ChildTag::default();
        assert!(!t.incomplete);
        assert_eq!(t.parent_type, 0);
        assert_eq!(t.index_in_parent, 0);
    }

    #[test]
    fn sequence_full_vector() {
        let mut v = vec![0i32; 4];
        let n = sequence_full_index_vector(&mut v, 10);
        assert_eq!(n, 4);
        assert_eq!(v, vec![10, 11, 12, 13]);
    }

    #[test]
    fn sequence_sparse_vector_marks_valid() {
        let mut v = vec![0i32, 1, 0, 2, 0];
        let n = sequence_sparse_index_vector(&mut v, 0);
        // positions 1, 3 were marked (non-zero) → become 0, 1
        assert_eq!(n, 2);
        assert_eq!(v[0], INDEX_INVALID);
        assert_eq!(v[1], 0);
        assert_eq!(v[2], INDEX_INVALID);
        assert_eq!(v[3], 1);
        assert_eq!(v[4], INDEX_INVALID);
    }

    #[test]
    fn relations_all_true() {
        let r = Relations::all_true();
        assert!(
            r.face_vertices
                && r.face_edges
                && r.edge_vertices
                && r.edge_faces
                && r.vertex_faces
                && r.vertex_edges
        );
    }
}
