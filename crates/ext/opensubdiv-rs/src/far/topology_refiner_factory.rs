// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/topologyRefinerFactory.h + topologyRefinerFactory.cpp

use crate::sdc::{Options, types::SchemeType};
use crate::vtr::level::TopologyError;
use super::types::{Index, IndexArray, LocalIndexArray};
use super::topology_refiner::TopologyRefiner;
use super::error::{far_error, far_warning, ErrorType};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Options for TopologyRefinerFactory::create().
/// Mirrors C++ `Far::TopologyRefinerFactory<MESH>::Options`.
#[derive(Clone, Copy)]
pub struct FactoryOptions {
    pub scheme_type:           SchemeType,
    pub scheme_options:        Options,
    pub validate_full_topology: bool,
}

impl FactoryOptions {
    pub fn new(scheme_type: SchemeType, scheme_options: Options) -> Self {
        Self { scheme_type, scheme_options, validate_full_topology: false }
    }
}

impl Default for FactoryOptions {
    fn default() -> Self {
        Self {
            scheme_type:            SchemeType::Catmark,
            scheme_options:         Options::new(),
            validate_full_topology: false,
        }
    }
}

// ---------------------------------------------------------------------------
// TopologyRefinerFactory trait
// ---------------------------------------------------------------------------

/// Factory trait for constructing TopologyRefiners from arbitrary mesh types.
///
/// Users implement this trait for their mesh type, providing:
///   - `resize_component_topology`    (required)
///   - `assign_component_topology`    (required)
///   - `assign_component_tags`        (optional, default: no-op / true)
///   - `assign_face_varying_topology` (optional, default: no-op / true)
///   - `report_invalid_topology`      (optional, default: no-op)
///
/// Mirrors C++ `Far::TopologyRefinerFactory<MESH>`.
pub trait TopologyRefinerFactory {
    type Mesh;

    /// Specify the number of vertices, faces, and per-face vertex counts.
    fn resize_component_topology(
        refiner: &mut TopologyRefiner,
        mesh:    &Self::Mesh,
    ) -> bool;

    /// Assign vertex, edge, and face index relationships.
    fn assign_component_topology(
        refiner: &mut TopologyRefiner,
        mesh:    &Self::Mesh,
    ) -> bool;

    /// Assign edge/vertex sharpness and face holes.  Default: no-op, returns true.
    fn assign_component_tags(
        _refiner: &mut TopologyRefiner,
        _mesh:    &Self::Mesh,
    ) -> bool { true }

    /// Assign face-varying topology.  Default: no-op, returns true.
    fn assign_face_varying_topology(
        _refiner: &mut TopologyRefiner,
        _mesh:    &Self::Mesh,
    ) -> bool { true }

    /// Report a topology validation error.  Default: no-op.
    fn report_invalid_topology(
        _err_code: TopologyError,
        _msg:      &str,
        _mesh:     &Self::Mesh,
    ) {}

    // ---- high-level entry points ----

    /// Construct a `TopologyRefiner` from the given mesh.
    fn create(mesh: &Self::Mesh, options: FactoryOptions) -> Option<Box<TopologyRefiner>> {
        let mut refiner = Box::new(TopologyRefiner::new(
            options.scheme_type, options.scheme_options));

        if !Self::populate_base_level(&mut refiner, mesh, &options) {
            return None;
        }
        Some(refiner)
    }

    /// Construct a new refiner that shares the base level of `source`.
    fn create_from_base(source: &TopologyRefiner) -> Box<TopologyRefiner> {
        Box::new(TopologyRefiner::from_base(source))
    }

    // ---- internal assembly (not to be specialized) ----

    fn populate_base_level(
        refiner: &mut TopologyRefiner,
        mesh:    &Self::Mesh,
        options: &FactoryOptions,
    ) -> bool {
        // Step 1: size the topology
        if !Self::resize_component_topology(refiner, mesh) { return false; }
        if !prepare_topology_sizing(refiner) { return false; }

        // Step 2: assign topology
        if !Self::assign_component_topology(refiner, mesh) { return false; }
        if !prepare_topology_assignment(refiner, options.validate_full_topology) { return false; }

        // Step 3: assign tags
        if !Self::assign_component_tags(refiner, mesh) { return false; }
        if !prepare_tags_and_sharpness(refiner) { return false; }

        // Step 4: face-varying
        if !Self::assign_face_varying_topology(refiner, mesh) { return false; }
        if !prepare_fvar_channels(refiner) { return false; }

        true
    }

    // ---- sizing helpers (forwarded to Level) ----

    fn set_num_base_vertices(refiner: &mut TopologyRefiner, count: i32) {
        refiner.get_level_internal_mut(0).resize_vertices(count);
    }
    fn set_num_base_faces(refiner: &mut TopologyRefiner, count: i32) {
        refiner.get_level_internal_mut(0).resize_faces(count);
    }
    fn set_num_base_edges(refiner: &mut TopologyRefiner, count: i32) {
        refiner.get_level_internal_mut(0).resize_edges(count);
    }
    fn set_num_base_face_vertices(refiner: &mut TopologyRefiner, f: Index, count: i32) {
        let reg = refiner.get_reg_face_size();
        refiner.get_level_internal_mut(0).resize_face_vertices(f, count);
        if count != reg { refiner.set_has_irreg_faces(true); }
    }
    fn set_num_base_edge_faces(refiner: &mut TopologyRefiner, e: Index, count: i32) {
        refiner.get_level_internal_mut(0).resize_edge_faces(e, count);
    }
    fn set_num_base_vertex_faces(refiner: &mut TopologyRefiner, v: Index, count: i32) {
        refiner.get_level_internal_mut(0).resize_vertex_faces(v, count);
    }
    fn set_num_base_vertex_edges(refiner: &mut TopologyRefiner, v: Index, count: i32) {
        refiner.get_level_internal_mut(0).resize_vertex_edges(v, count);
    }

    fn get_num_base_vertices(refiner: &TopologyRefiner) -> i32 {
        refiner.get_level_internal(0).get_num_vertices()
    }
    fn get_num_base_faces(refiner: &TopologyRefiner) -> i32 {
        refiner.get_level_internal(0).get_num_faces()
    }
    fn get_num_base_edges(refiner: &TopologyRefiner) -> i32 {
        refiner.get_level_internal(0).get_num_edges()
    }

    // ---- topology assignment helpers ----

    fn get_base_face_vertices<'a>(refiner: &'a mut TopologyRefiner, f: Index) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_face_vertices_mut(f)
    }
    fn get_base_face_edges<'a>(refiner: &'a mut TopologyRefiner, f: Index) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_face_edges_mut(f)
    }
    fn get_base_edge_vertices<'a>(refiner: &'a mut TopologyRefiner, e: Index) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_edge_vertices_mut(e)
    }
    fn get_base_edge_faces<'a>(refiner: &'a mut TopologyRefiner, e: Index) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_edge_faces_mut(e)
    }
    fn get_base_vertex_faces<'a>(refiner: &'a mut TopologyRefiner, v: Index) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_vertex_faces_mut(v)
    }
    fn get_base_vertex_edges<'a>(refiner: &'a mut TopologyRefiner, v: Index) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_vertex_edges_mut(v)
    }

    fn get_base_edge_face_local_indices<'a>(refiner: &'a mut TopologyRefiner, e: Index)
        -> LocalIndexArray<'a>
    {
        refiner.get_level_internal_mut(0).get_edge_face_local_indices_mut(e)
    }
    fn get_base_vertex_face_local_indices<'a>(refiner: &'a mut TopologyRefiner, v: Index)
        -> LocalIndexArray<'a>
    {
        refiner.get_level_internal_mut(0).get_vertex_face_local_indices_mut(v)
    }
    fn get_base_vertex_edge_local_indices<'a>(refiner: &'a mut TopologyRefiner, v: Index)
        -> LocalIndexArray<'a>
    {
        refiner.get_level_internal_mut(0).get_vertex_edge_local_indices_mut(v)
    }

    /// Populate local indices automatically (manifold meshes only).
    fn populate_base_local_indices(refiner: &mut TopologyRefiner) {
        refiner.get_level_internal_mut(0).populate_local_indices();
    }

    fn set_base_edge_non_manifold(refiner: &mut TopologyRefiner, e: Index, b: bool) {
        refiner.get_level_internal_mut(0).set_edge_non_manifold(e, b);
    }
    fn set_base_vertex_non_manifold(refiner: &mut TopologyRefiner, v: Index, b: bool) {
        refiner.get_level_internal_mut(0).set_vertex_non_manifold(v, b);
    }

    // ---- tag assignment helpers ----

    fn find_base_edge(refiner: &TopologyRefiner, v0: Index, v1: Index) -> Index {
        refiner.get_level_internal(0).find_edge(v0, v1)
    }
    fn set_base_edge_sharpness(refiner: &mut TopologyRefiner, e: Index, s: f32) {
        *refiner.get_level_internal_mut(0).get_edge_sharpness_mut(e) = s;
    }
    fn set_base_vertex_sharpness(refiner: &mut TopologyRefiner, v: Index, s: f32) {
        *refiner.get_level_internal_mut(0).get_vertex_sharpness_mut(v) = s;
    }
    fn set_base_face_hole(refiner: &mut TopologyRefiner, f: Index, is_hole: bool) {
        refiner.get_level_internal_mut(0).set_face_hole(f, is_hole);
        if is_hole { refiner.set_has_holes_flag(true); }
    }

    // ---- face-varying helpers ----

    fn create_base_fvar_channel(refiner: &mut TopologyRefiner, num_values: i32) -> i32 {
        let opts = refiner.get_scheme_options();
        refiner.get_level_internal_mut(0).create_fvar_channel(num_values, opts)
    }
    fn create_base_fvar_channel_with_options(
        refiner: &mut TopologyRefiner, num_values: i32, fvar_opts: Options
    ) -> i32 {
        let mut opts = refiner.get_scheme_options();
        opts.set_fvar_linear_interpolation(fvar_opts.get_fvar_linear_interpolation());
        refiner.get_level_internal_mut(0).create_fvar_channel(num_values, opts)
    }
    fn get_base_face_fvar_values<'a>(
        refiner: &'a mut TopologyRefiner, face: Index, channel: i32
    ) -> IndexArray<'a> {
        refiner.get_level_internal_mut(0).get_face_fvar_values_mut(face, channel)
    }
}

// ---------------------------------------------------------------------------
// Internal prepare functions (common post-processing)
// ---------------------------------------------------------------------------

/// Called after resize_component_topology: compute offsets for variable-arity
/// relations that were given only counts.
fn prepare_topology_sizing(refiner: &mut TopologyRefiner) -> bool {
    let lv = refiner.get_level_internal_mut(0);
    if lv.get_num_vertices() == 0 {
        far_error(ErrorType::CodingError,
            "TopologyRefinerFactory: no vertices specified");
        return false;
    }

    // If edges were explicitly sized, allocate the edge-vertex index array now.
    // Mirrors C++ topologyRefinerFactory.cpp:99 `baseLevel.resizeEdgeVertices()`
    // which is called after all resize_* sizing helpers have run.
    if lv.get_num_edges() > 0 {
        lv.resize_edge_vertices();
    }

    true
}

/// Called after assign_component_topology: complete the topology if only face-verts
/// were given, then optionally validate.
fn prepare_topology_assignment(refiner: &mut TopologyRefiner, validate: bool) -> bool {
    let lv = refiner.get_level_internal_mut(0);
    // If edges were not explicitly assigned, derive them from face-vert data.
    if lv.get_num_edges() == 0 {
        if !lv.complete_topology_from_face_vertices() {
            far_error(ErrorType::RuntimeError,
                "TopologyRefinerFactory: failed to complete topology from face-vertices");
            return false;
        }
    }
    if validate && !lv.validate_topology(None) {
        return false;
    }
    // Initialize inventory counters on the refiner from the now-complete base level.
    refiner.initialize_inventory();
    true
}

/// Complete edge and vertex component tags and sharpness for the base level.
///
/// Full port of C++ `TopologyRefinerFactoryBase::prepareComponentTagsAndSharpness()`
/// (lines 164-405). Covers:
///  - Boundary-faces-as-holes for VTX_BOUNDARY_NONE mode
///  - Edge tags: `_boundary`, non-manifold sharpening, `_infSharp`, `_semiSharp`
///  - Vertex tags: sharpness, corner sharpening, non-manifold sharpening,
///    `_semiSharp`, `_semiSharpEdges`, `_rule`, `_boundary`, `_corner`,
///    `_xordinary`, `_incomplete`, `_infSharpEdges`, `_infSharpCrease`,
///    `_infIrregular`, `_incidIrregFace`
fn prepare_tags_and_sharpness(refiner: &mut TopologyRefiner) -> bool {
    use crate::sdc::{
        crease::{Crease, Rule, is_infinite, is_sharp, is_semi_sharp, SHARPNESS_INFINITE},
        options::VtxBoundaryInterpolation,
        types::SchemeTypeTraits,
    };

    let options = refiner.get_scheme_options();
    let scheme  = refiner.get_scheme_type();
    let crease  = Crease::with_options(options);

    // Whether to tag incident boundary faces as holes (VTX_BOUNDARY_NONE mode).
    let make_boundary_holes =
        options.get_vtx_boundary_interpolation() == VtxBoundaryInterpolation::None
        && SchemeTypeTraits::get_local_neighborhood_size(scheme) > 0;

    // Whether to inf-sharpen topological corner vertices.
    let sharpen_corner_verts =
        options.get_vtx_boundary_interpolation() == VtxBoundaryInterpolation::EdgeAndCorner;

    // Always sharpen non-manifold features.
    let sharpen_non_man = true;

    // Regular interior and boundary valences for xordinary detection.
    let reg_interior_valence = SchemeTypeTraits::get_regular_vertex_valence(scheme);
    let reg_boundary_valence = reg_interior_valence / 2;

    // -----------------------------------------------------------------------
    // Phase 0: Tag boundary faces as holes (VTX_BOUNDARY_NONE mode).
    //
    // Faces are excluded if they contain a vertex on a boundary that did not
    // have all its incident boundary edges already sharpened (i.e. vertex has
    // an unsharpened boundary edge).
    // -----------------------------------------------------------------------
    if make_boundary_holes {
        // Collect faces to hole-tag in a first pass (read), then write in a second
        // pass to avoid mixed borrow conflicts.
        let hole_faces: Vec<crate::vtr::types::Index> = {
            let lv = refiner.get_level_internal(0);
            let vert_count = lv.get_num_vertices();
            let mut faces = Vec::new();
            for v in 0..vert_count {
                let v_edges = lv.get_vertex_edges(v);
                let v_faces = lv.get_vertex_faces(v);

                // Ignore manifold interior vertices
                if v_edges.size() == v_faces.size() && !lv.get_vertex_tag(v).non_manifold() {
                    continue;
                }

                // Check if any incident boundary edge is NOT infinite-sharp
                let mut exclude = false;
                for i in 0..v_edges.size() {
                    let e = v_edges[i];
                    if lv.get_num_edge_faces(e) == 1 && !is_infinite(lv.get_edge_sharpness(e)) {
                        exclude = true;
                        break;
                    }
                }
                if exclude {
                    for i in 0..v_faces.size() {
                        faces.push(v_faces[i]);
                    }
                }
            }
            faces
        };
        if !hole_faces.is_empty() {
            let lv = refiner.get_level_internal_mut(0);
            for f in hole_faces {
                lv.get_face_tag_mut(f).set_hole(true);
            }
            refiner.set_has_holes_flag(true);
        }
    }

    // -----------------------------------------------------------------------
    // Phase 1: Edge tags.
    //   - Set _boundary from edge-face count < 2
    //   - Sharpen boundary and non-manifold edges to INFINITE
    //   - Set _infSharp, _semiSharp
    // -----------------------------------------------------------------------
    {
        let lv = refiner.get_level_internal_mut(0);
        let edge_count = lv.get_num_edges();
        for e in 0..edge_count {
            let is_boundary = lv.get_num_edge_faces(e) < 2;
            let is_non_man  = lv.get_edge_tag(e).non_manifold();

            lv.get_edge_tag_mut(e).set_boundary(is_boundary);

            if is_boundary || (is_non_man && sharpen_non_man) {
                *lv.get_edge_sharpness_mut(e) = SHARPNESS_INFINITE;
            }

            let s = lv.get_edge_sharpness(e);
            lv.get_edge_tag_mut(e).set_inf_sharp(is_infinite(s));
            lv.get_edge_tag_mut(e).set_semi_sharp(is_sharp(s) && !is_infinite(s));
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2: Vertex tags.
    //   Per-vertex: take edge inventory, sharpen if needed, then set all tags.
    // -----------------------------------------------------------------------
    let has_irreg_faces = refiner.has_irreg_faces_flag();
    let reg_face_size   = refiner.get_reg_face_size();

    let lv = refiner.get_level_internal_mut(0);
    let vert_count = lv.get_num_vertices();

    for v in 0..vert_count {
        // Copy edge/face index lists into owned Vecs to avoid lifetime conflicts
        // between immutable ConstIndexArray slices and mutable tag writes below.
        let v_edges: Vec<crate::vtr::types::Index> = {
            let arr = lv.get_vertex_edges(v);
            (0..arr.size()).map(|i| arr[i]).collect()
        };
        let v_faces: Vec<crate::vtr::types::Index> = {
            let arr = lv.get_vertex_faces(v);
            (0..arr.size()).map(|i| arr[i]).collect()
        };

        // Edge inventory
        let mut boundary_edge_count    = 0i32;
        let mut inf_sharp_edge_count   = 0i32;
        let mut semi_sharp_edge_count  = 0i32;
        let mut non_manifold_edge_count = 0i32;
        for &e in &v_edges {
            let et = lv.get_edge_tag(e);
            if et.boundary()     { boundary_edge_count    += 1; }
            if et.inf_sharp()    { inf_sharp_edge_count   += 1; }
            if et.semi_sharp()   { semi_sharp_edge_count  += 1; }
            if et.non_manifold() { non_manifold_edge_count += 1; }
        }
        let sharp_edge_count = inf_sharp_edge_count + semi_sharp_edge_count;

        // Determine if vertex is a topological corner (1 face, 2 edges)
        let is_topo_corner = v_faces.len() == 1 && v_edges.len() == 2;
        let is_non_man     = lv.get_vertex_tag(v).non_manifold();

        // Sharpen vertex if needed
        let v_sharp_before = lv.get_vertex_sharpness(v);
        if is_topo_corner && sharpen_corner_verts {
            *lv.get_vertex_sharpness_mut(v) = SHARPNESS_INFINITE;
        } else if is_non_man && sharpen_non_man && !is_infinite(v_sharp_before) {
            // Avoid sharpening non-manifold interior crease vertices
            // (non-manifold crease: exactly 2 non-man edges, no boundary, more faces than edges)
            let is_non_man_crease =
                non_manifold_edge_count == 2
                && boundary_edge_count == 0
                && v_faces.len() > v_edges.len()
                && lv.test_vertex_non_manifold_crease(v);
            if !is_non_man_crease {
                *lv.get_vertex_sharpness_mut(v) = SHARPNESS_INFINITE;
            }
        }

        let v_sharp = lv.get_vertex_sharpness(v);

        // Basic sharpness tags
        lv.get_vertex_tag_mut(v).set_inf_sharp(is_infinite(v_sharp));
        lv.get_vertex_tag_mut(v).set_semi_sharp(is_semi_sharp(v_sharp));
        lv.get_vertex_tag_mut(v).set_semi_sharp_edges(semi_sharp_edge_count > 0);

        // Vertex-vertex subdivision rule
        let rule = crease.determine_vertex_vertex_rule_from_count(v_sharp, sharp_edge_count);
        lv.get_vertex_tag_mut(v).set_rule(rule as u16);

        // Topological tags
        lv.get_vertex_tag_mut(v).set_boundary(boundary_edge_count > 0);
        let is_inf = lv.get_vertex_tag(v).inf_sharp();
        let is_bnd = lv.get_vertex_tag(v).boundary();
        lv.get_vertex_tag_mut(v).set_corner(is_topo_corner && is_inf);
        let is_corner = lv.get_vertex_tag(v).corner();

        if is_non_man {
            lv.get_vertex_tag_mut(v).set_xordinary(false);
        } else if is_corner {
            lv.get_vertex_tag_mut(v).set_xordinary(false);
        } else if is_bnd {
            let xord = v_faces.len() as i32 != reg_boundary_valence;
            lv.get_vertex_tag_mut(v).set_xordinary(xord);
        } else {
            let xord = v_faces.len() as i32 != reg_interior_valence;
            lv.get_vertex_tag_mut(v).set_xordinary(xord);
        }
        lv.get_vertex_tag_mut(v).set_incomplete(false);

        // Inf-sharp feature tags
        lv.get_vertex_tag_mut(v).set_inf_sharp_edges(inf_sharp_edge_count > 0);
        lv.get_vertex_tag_mut(v).set_inf_sharp_crease(false);
        let inf_irreg_base = is_infinite(v_sharp) || inf_sharp_edge_count > 0;
        lv.get_vertex_tag_mut(v).set_inf_irregular(inf_irreg_base);

        if inf_sharp_edge_count > 0 {
            // Determine rule ignoring semi-sharp vertex sharpness
            let v_sharp_for_inf = if is_infinite(v_sharp) { v_sharp } else { 0.0 };
            let inf_rule = crease.determine_vertex_vertex_rule_from_count(
                v_sharp_for_inf, inf_sharp_edge_count);

            if inf_rule == Rule::Crease {
                lv.get_vertex_tag_mut(v).set_inf_sharp_crease(true);

                // Regular inf-crease can only be along a manifold regular boundary
                // or by bisecting a manifold interior region.
                let xord    = lv.get_vertex_tag(v).xordinary();
                let non_man = lv.get_vertex_tag(v).non_manifold();
                if !xord && !non_man {
                    if is_bnd {
                        lv.get_vertex_tag_mut(v).set_inf_irregular(false);
                    } else {
                        // Check bisection: opposing edges must be either both sharp or
                        // both smooth (valence-4: edges[0] vs [2], valence-6: [0] vs [3]
                        // and [1] vs [4]).
                        let irreg = if reg_interior_valence == 4 && v_edges.len() >= 3 {
                            lv.get_edge_tag(v_edges[0]).inf_sharp()
                            != lv.get_edge_tag(v_edges[2]).inf_sharp()
                        } else if reg_interior_valence == 6 && v_edges.len() >= 5 {
                            (lv.get_edge_tag(v_edges[0]).inf_sharp()
                             != lv.get_edge_tag(v_edges[3]).inf_sharp())
                            || (lv.get_edge_tag(v_edges[1]).inf_sharp()
                                != lv.get_edge_tag(v_edges[4]).inf_sharp())
                        } else {
                            false
                        };
                        lv.get_vertex_tag_mut(v).set_inf_irregular(irreg);
                    }
                }
            } else if inf_rule == Rule::Corner {
                // Regular set of inf-corners: all edges sharp and not a smooth corner
                let all_sharp = inf_sharp_edge_count == v_edges.len() as i32;
                if all_sharp && (v_edges.len() > 2 || is_infinite(v_sharp)) {
                    lv.get_vertex_tag_mut(v).set_inf_irregular(false);
                }
            }
        }

        // Mark vertices incident to irregular faces
        if has_irreg_faces {
            for &f in &v_faces {
                let fv = lv.get_face_vertices(f);
                if fv.size() as i32 != reg_face_size {
                    lv.get_vertex_tag_mut(v).set_incid_irreg_face(true);
                    break;
                }
            }
        }
    }

    true
}

/// Called after assign_face_varying_topology: finalize fvar channels.
fn prepare_fvar_channels(refiner: &mut TopologyRefiner) -> bool {
    let num_channels = refiner.get_level_internal(0).get_num_fvar_channels();
    for c in 0..num_channels {
        // reg_boundary_valence depends on scheme; use 4 for Catmark.
        let reg_bv = match refiner.get_scheme_type() {
            crate::sdc::types::SchemeType::Loop => 6,
            _                                   => 4,
        };
        refiner.get_level_internal_mut(0).complete_fvar_channel_topology(c, reg_bv);
    }
    true
}

// ---------------------------------------------------------------------------
// TopologyDescriptor factory implementation
// ---------------------------------------------------------------------------

use super::topology_descriptor::TopologyDescriptor;

/// Factory implementation for `TopologyDescriptor`.
/// Mirrors C++ template specialization `TopologyRefinerFactory<TopologyDescriptor>`.
pub struct TopologyDescriptorFactory;

impl TopologyRefinerFactory for TopologyDescriptorFactory {
    type Mesh = TopologyDescriptor;

    fn resize_component_topology(refiner: &mut TopologyRefiner, desc: &TopologyDescriptor) -> bool {
        Self::set_num_base_vertices(refiner, desc.num_vertices);
        Self::set_num_base_faces(refiner, desc.num_faces);

        let mut total_fv = 0i32;
        for f in 0..desc.num_faces as usize {
            let count = desc.num_verts_per_face[f];
            Self::set_num_base_face_vertices(refiner, f as Index, count);
            total_fv += count;
        }

        // Pre-allocate flat face-vert storage
        refiner.get_level_internal_mut(0).resize_face_vertices_total(total_fv);
        true
    }

    fn assign_component_topology(refiner: &mut TopologyRefiner, desc: &TopologyDescriptor) -> bool {
        let mut idx = 0usize;
        for f in 0..desc.num_faces {
            let mut dst = Self::get_base_face_vertices(refiner, f as Index);
            let n = dst.size() as usize;
            if desc.is_left_handed {
                dst[0] = desc.vert_indices_per_face[idx];
                for vi in (1..n).rev() {
                    dst[vi] = desc.vert_indices_per_face[idx + (n - vi)];
                }
                idx += n;
            } else {
                for vi in 0..n {
                    dst[vi] = desc.vert_indices_per_face[idx + vi];
                }
                idx += n;
            }
        }
        true
    }

    fn assign_component_tags(refiner: &mut TopologyRefiner, desc: &TopologyDescriptor) -> bool {
        // Creases
        if desc.num_creases > 0 {
            let pairs = &desc.crease_vertex_index_pairs;
            let weights = &desc.crease_weights;
            for edge_i in 0..desc.num_creases as usize {
                let v0 = pairs[2 * edge_i];
                let v1 = pairs[2 * edge_i + 1];
                let idx = Self::find_base_edge(refiner, v0, v1);
                if idx >= 0 {
                    Self::set_base_edge_sharpness(refiner, idx, weights[edge_i]);
                } else {
                    let msg = format!(
                        "Edge {} specified to be sharp does not exist ({}, {})",
                        edge_i, v0, v1);
                    Self::report_invalid_topology(
                        TopologyError::InvalidCreaseEdge, &msg, desc);
                }
            }
        }

        // Corners
        if desc.num_corners > 0 {
            let total_verts = Self::get_num_base_vertices(refiner);
            for vi in 0..desc.num_corners as usize {
                let idx = desc.corner_vertex_indices[vi];
                if idx >= 0 && idx < total_verts {
                    Self::set_base_vertex_sharpness(refiner, idx, desc.corner_weights[vi]);
                } else {
                    let msg = format!(
                        "Vertex {} specified to be sharp does not exist", idx);
                    Self::report_invalid_topology(
                        TopologyError::InvalidCreaseVert, &msg, desc);
                }
            }
        }

        // Holes
        for &h in &desc.hole_indices {
            Self::set_base_face_hole(refiner, h, true);
        }

        true
    }

    fn assign_face_varying_topology(
        refiner: &mut TopologyRefiner, desc: &TopologyDescriptor
    ) -> bool {
        for ch in &desc.fvar_channels {
            let channel = Self::create_base_fvar_channel(refiner, ch.num_values);
            let mut src_next = 0usize;
            for f in 0..desc.num_faces {
                let mut dst = Self::get_base_face_fvar_values(refiner, f as Index, channel);
                let n = dst.size() as usize;
                if desc.is_left_handed {
                    dst[0] = ch.value_indices[src_next];
                    for vi in (1..n).rev() {
                        dst[vi] = ch.value_indices[src_next + (n - vi)];
                    }
                    src_next += n;
                } else {
                    for vi in 0..n {
                        dst[vi] = ch.value_indices[src_next + vi];
                    }
                    src_next += n;
                }
            }
        }
        true
    }

    fn report_invalid_topology(_err_code: TopologyError, msg: &str, _mesh: &TopologyDescriptor) {
        far_warning(msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdc::types::SchemeType;

    fn make_quad_desc() -> TopologyDescriptor {
        // A simple 2-face mesh: two quads sharing an edge.
        //  0---1---2
        //  |   |   |
        //  3---4---5
        TopologyDescriptor {
            num_vertices: 6,
            num_faces:    2,
            num_verts_per_face:  vec![4, 4],
            vert_indices_per_face: vec![0,1,4,3, 1,2,5,4],
            ..Default::default()
        }
    }

    #[test]
    fn test_create_from_descriptor() {
        let desc = make_quad_desc();
        let opts = FactoryOptions {
            scheme_type:   SchemeType::Catmark,
            scheme_options: Options::new(),
            validate_full_topology: false,
        };
        let refiner = TopologyDescriptorFactory::create(&desc, opts);
        assert!(refiner.is_some());
        let refiner = refiner.unwrap();
        assert_eq!(refiner.get_level_internal(0).get_num_vertices(), 6);
        assert_eq!(refiner.get_level_internal(0).get_num_faces(),    2);
    }

    #[test]
    fn test_uniform_refine() {
        let desc = make_quad_desc();
        let opts = FactoryOptions::default();
        let mut refiner = TopologyDescriptorFactory::create(&desc, opts).unwrap();
        let ref_opts = super::super::topology_refiner::UniformOptions::new(2);
        refiner.refine_uniform(ref_opts);
        assert_eq!(refiner.get_max_level(), 2);
        assert_eq!(refiner.get_num_levels(), 3);
    }
}
