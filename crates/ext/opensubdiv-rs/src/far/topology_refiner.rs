// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/topologyRefiner.h + far/topologyRefiner.cpp

use super::error::{ErrorType, far_error};
use super::topology_level::TopologyLevel;
use super::types::{ConstIndexArray, Index};
use crate::sdc::{
    Options,
    crease::Rule,
    types::{SchemeType, SchemeTypeTraits, Split},
};
use crate::vtr::{
    Level, QuadRefinement, Refinement, RefinementOptions, SparseSelector, TriRefinement, VTag,
};

// ---------------------------------------------------------------------------
// Options structs
// ---------------------------------------------------------------------------

/// Options for uniform refinement.
/// Mirrors C++ `Far::TopologyRefiner::UniformOptions`.
#[derive(Clone, Copy)]
pub struct UniformOptions {
    /// Number of refinement iterations (0-15).
    pub refinement_level: u32,
    /// Order child vertices from faces first (instead of from vertices).
    pub order_vertices_from_faces_first: bool,
    /// Generate full topology in the last level (required for limit queries).
    pub full_topology_in_last_level: bool,
}

impl UniformOptions {
    pub fn new(level: u32) -> Self {
        Self {
            refinement_level: level & 0xf,
            order_vertices_from_faces_first: false,
            full_topology_in_last_level: false,
        }
    }
    pub fn set_refinement_level(&mut self, level: u32) {
        self.refinement_level = level & 0xf;
    }
}

impl Default for UniformOptions {
    fn default() -> Self {
        Self::new(0)
    }
}

/// Options for feature-adaptive refinement.
/// Mirrors C++ `Far::TopologyRefiner::AdaptiveOptions`.
#[derive(Clone, Copy)]
pub struct AdaptiveOptions {
    /// Maximum isolation level (0-15).
    pub isolation_level: u32,
    /// Secondary (shallower) isolation level (0-15, default 15).
    pub secondary_level: u32,
    /// Use single-crease patches (Catmark only).
    pub use_single_crease_patch: bool,
    /// Use infinitely-sharp patches.
    pub use_inf_sharp_patch: bool,
    /// Consider face-varying channels when isolating.
    pub consider_fvar_channels: bool,
    /// Order child vertices from faces first.
    pub order_vertices_from_faces_first: bool,
}

impl AdaptiveOptions {
    pub fn new(level: u32) -> Self {
        Self {
            isolation_level: level & 0xf,
            secondary_level: 0xf,
            use_single_crease_patch: false,
            use_inf_sharp_patch: false,
            consider_fvar_channels: false,
            order_vertices_from_faces_first: false,
        }
    }
    pub fn set_isolation_level(&mut self, level: u32) {
        self.isolation_level = level & 0xf;
    }
    pub fn set_secondary_level(&mut self, level: u32) {
        self.secondary_level = level & 0xf;
    }
}

impl Default for AdaptiveOptions {
    fn default() -> Self {
        Self::new(0)
    }
}

// ---------------------------------------------------------------------------
// FeatureMask
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct FeatureMask {
    select_xordinary_interior: bool,
    select_xordinary_boundary: bool,
    select_semi_sharp_single: bool,
    select_semi_sharp_non_single: bool,
    select_inf_sharp_regular_crease: bool,
    select_inf_sharp_regular_corner: bool,
    select_inf_sharp_irregular_dart: bool,
    select_inf_sharp_irregular_crease: bool,
    select_inf_sharp_irregular_corner: bool,
    select_unisolated_interior_edge: bool,
    select_non_manifold: bool,
    select_fvar_features: bool,
}

impl FeatureMask {
    fn initialize(&mut self, opts: &AdaptiveOptions, reg_face_size: i32) {
        let use_single = opts.use_single_crease_patch && (reg_face_size == 4);
        self.select_xordinary_interior = true;
        self.select_xordinary_boundary = true;
        self.select_semi_sharp_single = !use_single;
        self.select_semi_sharp_non_single = true;
        self.select_inf_sharp_regular_crease = !(opts.use_inf_sharp_patch || use_single);
        self.select_inf_sharp_regular_corner = !opts.use_inf_sharp_patch;
        self.select_inf_sharp_irregular_dart = true;
        self.select_inf_sharp_irregular_crease = true;
        self.select_inf_sharp_irregular_corner = true;
        self.select_unisolated_interior_edge = use_single && !opts.use_inf_sharp_patch;
        self.select_non_manifold = true;
        self.select_fvar_features = opts.consider_fvar_channels;
    }

    fn reduce(&mut self, opts: &AdaptiveOptions) {
        self.select_xordinary_interior = false;
        self.select_xordinary_boundary = false;
        if opts.use_inf_sharp_patch {
            self.select_inf_sharp_regular_crease = false;
            self.select_inf_sharp_regular_corner = false;
            self.select_inf_sharp_irregular_dart = false;
            self.select_inf_sharp_irregular_crease = false;
        }
    }
}

// ---------------------------------------------------------------------------
// TopologyRefiner
// ---------------------------------------------------------------------------

/// Stores topology data for a specified set of refinement options.
/// Mirrors C++ `Far::TopologyRefiner`.
pub struct TopologyRefiner {
    subdiv_type: SchemeType,
    subdiv_options: Options,

    is_uniform: bool,
    has_holes: bool,
    has_irreg_faces: bool,
    reg_face_size: i32,
    max_level: u32,

    uniform_options: UniformOptions,
    adaptive_options: AdaptiveOptions,

    total_vertices: i32,
    total_edges: i32,
    total_faces: i32,
    total_face_vertices: i32,
    max_valence: i32,

    #[allow(dead_code)]
    base_level_owned: bool,

    /// Owned topology levels (Box keeps addresses stable for raw-ptr refs).
    levels: Vec<Box<Level>>,
    /// Owned refinements between successive levels.
    refinements: Vec<Box<Refinement>>,

    /// Far-level wrappers (hold raw ptrs into levels/refinements).
    far_levels: Vec<TopologyLevel>,
}

impl TopologyRefiner {
    /// Construct a new refiner sharing only the base level of `source`.
    /// The returned refiner has no refinements yet.
    /// Mirrors C++ `TopologyRefinerFactory<M>::create(source)` shallow-copy path.
    pub fn from_base(source: &TopologyRefiner) -> Self {
        // Deep-clone only level 0; no refinements carried over.
        let base_level = source.levels[0].as_ref().clone();
        let reg_face_size = SchemeTypeTraits::get_regular_face_size(source.subdiv_type);
        let mut me = Self {
            subdiv_type: source.subdiv_type,
            subdiv_options: source.subdiv_options,
            is_uniform: true,
            has_holes: source.has_holes,
            has_irreg_faces: source.has_irreg_faces,
            reg_face_size,
            max_level: 0,
            uniform_options: UniformOptions::default(),
            adaptive_options: AdaptiveOptions::default(),
            total_vertices: 0,
            total_edges: 0,
            total_faces: 0,
            total_face_vertices: 0,
            max_valence: 0,
            base_level_owned: true,
            levels: Vec::with_capacity(10),
            refinements: Vec::with_capacity(10),
            far_levels: Vec::with_capacity(10),
        };
        me.levels.push(Box::new(base_level));
        me.initialize_inventory();
        me.assemble_far_levels();
        me
    }

    /// Construct a new refiner for the given subdivision scheme.
    pub fn new(scheme_type: SchemeType, options: Options) -> Self {
        let reg_face_size = SchemeTypeTraits::get_regular_face_size(scheme_type);
        let mut me = Self {
            subdiv_type: scheme_type,
            subdiv_options: options,
            is_uniform: true,
            has_holes: false,
            has_irreg_faces: false,
            reg_face_size,
            max_level: 0,
            uniform_options: UniformOptions::default(),
            adaptive_options: AdaptiveOptions::default(),
            total_vertices: 0,
            total_edges: 0,
            total_faces: 0,
            total_face_vertices: 0,
            max_valence: 0,
            base_level_owned: true,
            levels: Vec::with_capacity(10),
            refinements: Vec::with_capacity(10),
            far_levels: Vec::with_capacity(10),
        };
        me.levels.push(Box::new(Level::new()));
        me.assemble_far_levels();
        me
    }

    // ---- public accessors ----

    pub fn get_scheme_type(&self) -> SchemeType {
        self.subdiv_type
    }
    pub fn get_scheme_options(&self) -> Options {
        self.subdiv_options
    }
    pub fn is_uniform(&self) -> bool {
        self.is_uniform
    }
    pub fn get_num_levels(&self) -> i32 {
        self.levels.len() as i32
    }
    pub fn get_max_level(&self) -> i32 {
        self.max_level as i32
    }
    pub fn get_max_valence(&self) -> i32 {
        self.max_valence
    }
    pub fn has_holes(&self) -> bool {
        self.has_holes
    }
    pub fn get_num_vertices_total(&self) -> i32 {
        self.total_vertices
    }
    pub fn get_num_edges_total(&self) -> i32 {
        self.total_edges
    }
    pub fn get_num_faces_total(&self) -> i32 {
        self.total_faces
    }
    pub fn get_num_face_vertices_total(&self) -> i32 {
        self.total_face_vertices
    }

    /// Access a read-only level wrapper.
    pub fn get_level(&self, level: i32) -> &TopologyLevel {
        &self.far_levels[level as usize]
    }

    pub fn get_uniform_options(&self) -> UniformOptions {
        self.uniform_options
    }
    pub fn get_adaptive_options(&self) -> AdaptiveOptions {
        self.adaptive_options
    }

    // ---- face-varying queries ----

    pub fn get_num_fvar_channels(&self) -> i32 {
        self.levels[0].get_num_fvar_channels()
    }
    pub fn get_fvar_linear_interpolation(
        &self,
        channel: i32,
    ) -> crate::sdc::options::FVarLinearInterpolation {
        self.levels[0]
            .get_fvar_options(channel)
            .get_fvar_linear_interpolation()
    }
    pub fn get_num_fvar_values_total(&self, channel: i32) -> i32 {
        self.levels
            .iter()
            .map(|l| l.get_num_fvar_values(channel))
            .sum()
    }

    // ---- refinement ----

    /// Apply uniform refinement.
    pub fn refine_uniform(&mut self, options: UniformOptions) {
        if self.levels[0].get_num_vertices() == 0 {
            far_error(
                ErrorType::RuntimeError,
                "TopologyRefiner::refine_uniform: base level is uninitialized",
            );
            return;
        }
        if !self.refinements.is_empty() {
            far_error(
                ErrorType::RuntimeError,
                "TopologyRefiner::refine_uniform: previous refinements already applied",
            );
            return;
        }

        self.uniform_options = options;
        self.is_uniform = true;
        self.max_level = options.refinement_level;

        let split_type = SchemeTypeTraits::get_topological_split_type(self.subdiv_type);

        for i in 1..=(options.refinement_level as usize) {
            let minimal = if options.full_topology_in_last_level {
                false
            } else {
                i == options.refinement_level as usize
            };
            let ref_opts = RefinementOptions {
                sparse: false,
                face_verts_first: options.order_vertices_from_faces_first,
                minimal_topology: minimal,
            };

            let parent_ptr: *const Level = &*self.levels[i - 1];
            let mut child_box = Box::new(Level::new());
            let child_ptr: *mut Level = &mut *child_box;

            let refinement = match split_type {
                Split::ToQuads => {
                    let mut qr =
                        unsafe { QuadRefinement::new(parent_ptr, child_ptr, self.subdiv_options) };
                    qr.refine(ref_opts);
                    Box::new(qr.0)
                }
                _ => {
                    let mut tr =
                        unsafe { TriRefinement::new(parent_ptr, child_ptr, self.subdiv_options) };
                    tr.refine(ref_opts);
                    Box::new(tr.0)
                }
            };

            self.update_inventory(&*child_box);
            self.levels.push(child_box);
            self.refinements.push(refinement);
        }

        self.assemble_far_levels();
    }

    /// Apply feature-adaptive refinement.
    pub fn refine_adaptive(&mut self, options: AdaptiveOptions, selected_faces: ConstIndexArray) {
        if self.levels[0].get_num_vertices() == 0 {
            far_error(
                ErrorType::RuntimeError,
                "TopologyRefiner::refine_adaptive: base level is uninitialized",
            );
            return;
        }
        if !self.refinements.is_empty() {
            far_error(
                ErrorType::RuntimeError,
                "TopologyRefiner::refine_adaptive: previous refinements already applied",
            );
            return;
        }

        self.is_uniform = false;
        self.adaptive_options = options;

        let non_linear = SchemeTypeTraits::get_local_neighborhood_size(self.subdiv_type);
        let shallow = (options.secondary_level as i32).min(options.isolation_level as i32);
        let deeper = options.isolation_level as i32;
        let potential_max = if non_linear != 0 {
            deeper
        } else {
            self.has_irreg_faces as i32
        };

        let mut more = FeatureMask::default();
        more.initialize(&options, self.reg_face_size);
        let mut less = more;
        if shallow < potential_max {
            less.reduce(&options);
        }

        // If no non-linear fvar channels, disable fvar consideration
        if more.select_fvar_features && non_linear != 0 {
            let any_nl = (0..self.levels[0].get_num_fvar_channels())
                .any(|c| !self.levels[0].get_fvar_level(c).is_linear());
            if !any_nl {
                more.select_fvar_features = false;
                less.select_fvar_features = false;
            }
        }

        let split_type = SchemeTypeTraits::get_topological_split_type(self.subdiv_type);
        let ref_opts = RefinementOptions {
            sparse: true,
            minimal_topology: false,
            face_verts_first: options.order_vertices_from_faces_first,
        };

        let sel_faces_owned: Vec<Index> = selected_faces.as_slice().to_vec();

        for i in 1..=(potential_max as usize) {
            let parent_ptr: *const Level = &*self.levels[i - 1];
            let mut child_box = Box::new(Level::new());
            let child_ptr: *mut Level = &mut *child_box;

            let mut refinement_box: Box<Refinement> = match split_type {
                Split::ToQuads => Box::new(
                    unsafe { QuadRefinement::new(parent_ptr, child_ptr, self.subdiv_options) }.0,
                ),
                _ => Box::new(
                    unsafe { TriRefinement::new(parent_ptr, child_ptr, self.subdiv_options) }.0,
                ),
            };

            let level_mask = if i <= shallow as usize { &more } else { &less };
            let parent_level = unsafe { &*parent_ptr };

            let any_selected = {
                let mut selector = SparseSelector::new(&mut *refinement_box);
                let sf = ConstIndexArray::new(&sel_faces_owned);
                if i > 1 {
                    Self::select_features(
                        &mut selector,
                        level_mask,
                        self.reg_face_size,
                        self.has_holes,
                        ConstIndexArray::new(&[]),
                        parent_level,
                    );
                } else if non_linear != 0 {
                    Self::select_features(
                        &mut selector,
                        level_mask,
                        self.reg_face_size,
                        self.has_holes,
                        sf,
                        parent_level,
                    );
                } else {
                    Self::select_linear_irreg(
                        &mut selector,
                        self.reg_face_size,
                        self.has_holes,
                        sf,
                        parent_level,
                    );
                }
                !selector.is_selection_empty()
            };

            if !any_selected {
                break;
            }

            refinement_box.refine(ref_opts);
            self.update_inventory(&*child_box);
            self.levels.push(child_box);
            self.refinements.push(refinement_box);
        }

        self.max_level = self.refinements.len() as u32;
        self.assemble_far_levels();
    }

    /// Remove all refinement levels above the base level.
    pub fn unrefine(&mut self) {
        self.levels.truncate(1);
        self.refinements.clear();
        self.max_level = 0;
        self.initialize_inventory();
        self.assemble_far_levels();
    }

    // ---- internal accessors used by factories and patch builders ----

    pub fn get_level_internal(&self, l: i32) -> &Level {
        &self.levels[l as usize]
    }
    pub fn get_level_internal_mut(&mut self, l: i32) -> &mut Level {
        &mut self.levels[l as usize]
    }
    pub fn get_refinement_internal(&self, l: i32) -> &Refinement {
        &self.refinements[l as usize]
    }

    pub fn has_irreg_faces_flag(&self) -> bool {
        self.has_irreg_faces
    }
    pub fn get_reg_face_size(&self) -> i32 {
        self.reg_face_size
    }
    pub fn set_has_irreg_faces(&mut self, v: bool) {
        self.has_irreg_faces = v;
    }
    pub fn set_has_holes_flag(&mut self, v: bool) {
        self.has_holes = v;
    }

    // ---- inventory management ----

    pub(crate) fn initialize_inventory(&mut self) {
        if !self.levels.is_empty() {
            let lv = &self.levels[0];
            self.total_vertices = lv.get_num_vertices();
            self.total_edges = lv.get_num_edges();
            self.total_faces = lv.get_num_faces();
            self.total_face_vertices = lv.get_num_face_vertices_total();
            self.max_valence = lv.get_max_valence();
        } else {
            self.total_vertices = 0;
            self.total_edges = 0;
            self.total_faces = 0;
            self.total_face_vertices = 0;
            self.max_valence = 0;
        }
    }

    fn update_inventory(&mut self, new_level: &Level) {
        self.total_vertices += new_level.get_num_vertices();
        self.total_edges += new_level.get_num_edges();
        self.total_faces += new_level.get_num_faces();
        self.total_face_vertices += new_level.get_num_face_vertices_total();
        self.max_valence = self.max_valence.max(new_level.get_max_valence());
    }

    pub(crate) fn assemble_far_levels(&mut self) {
        let n = self.levels.len();
        self.far_levels.clear();
        self.far_levels.reserve(n);
        for i in 0..n {
            let level_ptr: *const Level = &*self.levels[i];
            let parent_ptr = if i == 0 || self.refinements.is_empty() {
                std::ptr::null()
            } else {
                &*self.refinements[i - 1] as *const Refinement
            };
            let child_ptr = if i < self.refinements.len() {
                &*self.refinements[i] as *const Refinement
            } else {
                std::ptr::null()
            };
            self.far_levels.push(TopologyLevel {
                level: level_ptr,
                ref_to_parent: parent_ptr,
                ref_to_child: child_ptr,
            });
        }
    }

    // ---- feature selection (static helpers) ----

    fn vtag_inf_sharp_features(comp: VTag, mask: &FeatureMask) -> bool {
        let rule_bits = comp.rule();
        if comp.inf_irregular() {
            if rule_bits & (Rule::Corner as u16) != 0 {
                return mask.select_inf_sharp_irregular_corner;
            } else if rule_bits & (Rule::Crease as u16) != 0 {
                return if comp.boundary() {
                    mask.select_xordinary_boundary
                } else {
                    mask.select_inf_sharp_irregular_crease
                };
            } else if rule_bits & (Rule::Dart as u16) != 0 {
                return mask.select_inf_sharp_irregular_dart;
            }
        } else if comp.boundary() {
            if rule_bits & (Rule::Corner as u16) != 0 {
                return if comp.corner() {
                    false
                } else {
                    mask.select_inf_sharp_regular_corner
                };
            } else {
                return false;
            }
        } else {
            if rule_bits & (Rule::Corner as u16) != 0 {
                return mask.select_inf_sharp_regular_corner;
            } else {
                return mask.select_inf_sharp_regular_crease;
            }
        }
        false
    }

    fn face_inf_sharp_features(comp: VTag, vtags: &[VTag], nv: i32, mask: &FeatureMask) -> bool {
        let smooth_bit = comp.rule() & (Rule::Smooth as u16) != 0;
        if nv == 4 {
            if smooth_bit {
                return Self::vtag_inf_sharp_features(comp, mask);
            } else if mask.select_unisolated_interior_edge {
                for i in 0..4 {
                    if vtags[i].inf_sharp_edges() && !vtags[i].boundary() {
                        return true;
                    }
                }
            }
        } else {
            if smooth_bit && !comp.boundary() {
                return Self::vtag_inf_sharp_features(comp, mask);
            }
        }
        for i in 0..nv as usize {
            if vtags[i].rule() & (Rule::Smooth as u16) == 0 {
                if Self::vtag_inf_sharp_features(vtags[i], mask) {
                    return true;
                }
            }
        }
        false
    }

    fn face_features(level: &Level, face: Index, mask: &FeatureMask, reg_face_size: i32) -> bool {
        let fverts = level.get_face_vertices(face);
        if fverts.size() != reg_face_size {
            return true;
        }

        let mut vtags = [VTag::default(); 4];
        level.get_face_vtags(face, &mut vtags, -1);
        let comp = VTag::bitwise_or(&vtags[..fverts.size() as usize]);

        if comp.incid_irreg_face() {
            return true;
        }
        if comp.incomplete() {
            return false;
        }
        if comp.non_manifold() && mask.select_non_manifold {
            return true;
        }

        if comp.xordinary() && mask.select_xordinary_interior {
            if comp.rule() == (Rule::Smooth as u16) {
                return true;
            }
            if level.get_depth() < 2 {
                for i in 0..fverts.size() as usize {
                    if vtags[i].xordinary() && vtags[i].rule() == (Rule::Smooth as u16) {
                        return true;
                    }
                }
            }
        }

        if comp.rule() == (Rule::Smooth as u16) {
            return false;
        }

        if comp.semi_sharp() || comp.semi_sharp_edges() {
            if mask.select_semi_sharp_single && mask.select_semi_sharp_non_single {
                return true;
            } else if level.is_single_crease_patch(face) {
                return mask.select_semi_sharp_single;
            } else {
                return mask.select_semi_sharp_non_single;
            }
        }

        if comp.inf_sharp() || comp.inf_sharp_edges() {
            return Self::face_inf_sharp_features(comp, &vtags, fverts.size(), mask);
        }
        false
    }

    fn face_fvar_features(level: &Level, face: Index, mask: &FeatureMask, channel: i32) -> bool {
        let fverts = level.get_face_vertices(face);
        let mut vtags = [VTag::default(); 4];
        for i in 0..fverts.size() as usize {
            vtags[i] = level.get_vertex_composite_fvar_vtag(fverts[i], channel);
        }
        let comp = VTag::bitwise_or(&vtags[..fverts.size() as usize]);
        if comp.incomplete() {
            return false;
        }
        if comp.non_manifold() && mask.select_non_manifold {
            return true;
        }
        if comp.xordinary() && mask.select_xordinary_interior {
            return true;
        }
        Self::face_inf_sharp_features(comp, &vtags, fverts.size(), mask)
    }

    fn select_features(
        selector: &mut SparseSelector,
        mask: &FeatureMask,
        reg_face_size: i32,
        has_holes: bool,
        faces: ConstIndexArray,
        level: &Level,
    ) {
        let total = if faces.is_empty() {
            level.get_num_faces()
        } else {
            faces.size()
        };
        let n_fvar = if mask.select_fvar_features {
            level.get_num_fvar_channels()
        } else {
            0
        };
        for fi in 0..total {
            let f: Index = if faces.is_empty() { fi } else { faces[fi] };
            if has_holes && level.is_face_hole(f) {
                continue;
            }
            let mut sel = Self::face_features(level, f, mask, reg_face_size);
            if !sel && mask.select_fvar_features {
                for c in 0..n_fvar {
                    if !level.does_face_fvar_topology_match(f, c) {
                        sel = Self::face_fvar_features(level, f, mask, c);
                        if sel {
                            break;
                        }
                    }
                }
            }
            if sel {
                selector.select_face(f);
            }
        }
    }

    fn select_linear_irreg(
        selector: &mut SparseSelector,
        reg_face_size: i32,
        has_holes: bool,
        faces: ConstIndexArray,
        level: &Level,
    ) {
        let total = if faces.is_empty() {
            level.get_num_faces()
        } else {
            faces.size()
        };
        for fi in 0..total {
            let f: Index = if faces.is_empty() { fi } else { faces[fi] };
            if has_holes && level.is_face_hole(f) {
                continue;
            }
            if level.get_face_vertices(f).size() != reg_face_size {
                selector.select_face(f);
            }
        }
    }
}

// Safety: TopologyRefiner's raw pointers all point to data owned by self.
// Same-thread usage is safe as long as no aliased mutable access occurs.
unsafe impl Send for TopologyRefiner {}
