use crate::far::stencil_table_factory::{
    InterpolationMode, StencilTableFactory, StencilTableOptions,
};
use crate::far::types::ConstArray;
use crate::far::{
    AdaptiveOptions, EndCapType, PatchTable, PatchTableFactory, PatchTableFactoryOptions,
    StencilTable, TopologyRefiner, UniformOptions,
};
use crate::osd::{
    BufferDescriptor,
    cpu_evaluator::CpuEvaluator,
    cpu_patch_table::CpuPatchTable,
    cpu_vertex_buffer::{CpuVertexBuffer, VertexBuffer},
};

/// Bit indices for MeshBitset flags — mirrors Osd::MeshBits.
pub mod mesh_bits {
    pub const ADAPTIVE: u32 = 0;
    pub const INTERLEAVE_VARYING: u32 = 1;
    pub const FVAR_DATA: u32 = 2;
    pub const FVAR_ADAPTIVE: u32 = 3;
    pub const USE_SMOOTH_CORNER_PATCH: u32 = 4;
    pub const USE_SINGLE_CREASE_PATCH: u32 = 5;
    pub const USE_INF_SHARP_PATCH: u32 = 6;
    pub const END_CAP_BILINEAR_BASIS: u32 = 7;
    pub const END_CAP_BSPLINE_BASIS: u32 = 8;
    pub const END_CAP_GREGORY_BASIS: u32 = 9;
    pub const END_CAP_LEGACY_GREGORY: u32 = 10;
    pub const NUM_MESH_BITS: u32 = 11;
}

/// Packed bitset mirroring `std::bitset<11>` MeshBitset in C++.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MeshBitset(pub u32);

impl MeshBitset {
    pub fn new() -> Self {
        Self(0)
    }

    /// Test whether bit `bit` is set.
    pub fn test(&self, bit: u32) -> bool {
        (self.0 >> bit) & 1 != 0
    }

    /// Set bit `bit`.
    pub fn set(&mut self, bit: u32) {
        self.0 |= 1 << bit;
    }

    /// Clear bit `bit`.
    pub fn clear(&mut self, bit: u32) {
        self.0 &= !(1 << bit);
    }
}

/// Abstract mesh interface — mirrors `Osd::MeshInterface<PATCH_TABLE>`.
///
/// In C++ this is a template, parameterised by the patch table type.
/// In Rust we use a trait with an associated type for the binding handle.
pub trait MeshInterface {
    /// The type returned by `bind_vertex_buffer()`.
    type VertexBufferBinding;

    /// Total number of vertices (coarse + refined + end-cap).
    fn get_num_vertices(&self) -> i32;

    /// Maximum ring valence observed in the patch table.
    fn get_max_valence(&self) -> i32;

    /// Upload coarse vertex data into the vertex buffer.
    fn update_vertex_buffer(&mut self, vertex_data: &[f32], start_vertex: i32, num_verts: i32);

    /// Upload coarse varying data into the varying buffer.
    fn update_varying_buffer(&mut self, varying_data: &[f32], start_vertex: i32, num_verts: i32);

    /// Run subdivision stencils, computing refined vertex positions.
    fn refine(&mut self);

    /// Synchronize GPU work (no-op for CPU backends).
    fn synchronize(&mut self);

    /// Return a reference to the device-side patch table.
    fn get_patch_table(&self) -> &dyn std::any::Any;

    /// Return a reference to the Far::PatchTable used during construction.
    ///
    /// Mirrors C++ `MeshInterface::GetFarPatchTable()`.  Returns `None` when the
    /// implementation does not retain the Far patch table after construction.
    fn get_far_patch_table(&self) -> Option<&PatchTable>;

    /// Return the raw vertex buffer binding handle.
    fn bind_vertex_buffer(&mut self) -> Self::VertexBufferBinding;

    /// Return the raw varying buffer binding handle.
    fn bind_varying_buffer(&mut self) -> Self::VertexBufferBinding;
}

/// Helper: build RefinementOptions from MeshBitset.
///
/// Mirrors the protected static `refineMesh()` methods in C++.
pub fn refine_mesh_options(
    adaptive: bool,
    level: i32,
    single_crease: bool,
    inf_sharp: bool,
    fvar_adaptive: bool,
) -> RefinementOptions {
    RefinementOptions {
        adaptive,
        level,
        single_crease_patch: single_crease,
        inf_sharp_patch: inf_sharp,
        consider_fvar_channels: fvar_adaptive,
    }
}

/// Options controlling how a TopologyRefiner is refined.
#[derive(Debug, Clone, Copy)]
pub struct RefinementOptions {
    pub adaptive: bool,
    pub level: i32,
    pub single_crease_patch: bool,
    pub inf_sharp_patch: bool,
    pub consider_fvar_channels: bool,
}

impl Default for RefinementOptions {
    fn default() -> Self {
        Self {
            adaptive: false,
            level: 1,
            single_crease_patch: false,
            inf_sharp_patch: false,
            consider_fvar_channels: false,
        }
    }
}

/// Cache of evaluator instances keyed by (srcDesc, dstDesc, du/dv/duu/duv/dvv).
///
/// Mirrors `Osd::EvaluatorCacheT<EVALUATOR>`.
pub struct EvaluatorCache<E> {
    entries: Vec<EvaluatorEntry<E>>,
}

struct EvaluatorEntry<E> {
    src_desc: BufferDescriptor,
    dst_desc: BufferDescriptor,
    du_desc: BufferDescriptor,
    dv_desc: BufferDescriptor,
    duu_desc: BufferDescriptor,
    duv_desc: BufferDescriptor,
    dvv_desc: BufferDescriptor,
    evaluator: E,
}

impl<E> EvaluatorCache<E> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Look up a cached evaluator by descriptor signatures.
    pub fn get_evaluator(
        &self,
        src: &BufferDescriptor,
        dst: &BufferDescriptor,
        du: &BufferDescriptor,
        dv: &BufferDescriptor,
        duu: &BufferDescriptor,
        duv: &BufferDescriptor,
        dvv: &BufferDescriptor,
    ) -> Option<&E> {
        self.entries
            .iter()
            .find(|e| {
                e.src_desc.matches(src)
                    && e.dst_desc.matches(dst)
                    && e.du_desc.matches(du)
                    && e.dv_desc.matches(dv)
                    && e.duu_desc.matches(duu)
                    && e.duv_desc.matches(duv)
                    && e.dvv_desc.matches(dvv)
            })
            .map(|e| &e.evaluator)
    }

    /// Insert a new evaluator entry.
    pub fn insert(
        &mut self,
        src: BufferDescriptor,
        dst: BufferDescriptor,
        du: BufferDescriptor,
        dv: BufferDescriptor,
        duu: BufferDescriptor,
        duv: BufferDescriptor,
        dvv: BufferDescriptor,
        evaluator: E,
    ) {
        self.entries.push(EvaluatorEntry {
            src_desc: src,
            dst_desc: dst,
            du_desc: du,
            dv_desc: dv,
            duu_desc: duu,
            duv_desc: duv,
            dvv_desc: dvv,
            evaluator,
        });
    }
}

impl<E> Default for EvaluatorCache<E> {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
//  CpuMesh — concrete CPU Mesh implementation
// ---------------------------------------------------------------------------

/// CPU mesh — manages topology refinement, stencil tables, patch tables and
/// vertex buffers for CPU-side OpenSubdiv evaluation.
///
/// Mirrors the `Osd::Mesh<CpuVertexBuffer, Far::StencilTable, CpuEvaluator,
/// CpuPatchTable>` specialisation from mesh.h.
pub struct CpuMesh {
    /// Refined vertex count (coarse + stencils + local/end-cap points).
    num_vertices: i32,
    /// Number of coarse control vertices (level-0 count from the refiner).
    num_ctrl_verts: i32,
    /// Maximum ring valence from the patch table.
    max_valence: i32,
    /// Vertex primvar buffer (interleaved or vertex-only).
    vertex_buffer: CpuVertexBuffer,
    /// Optional separate varying buffer (non-interleaved mode).
    varying_buffer: Option<CpuVertexBuffer>,
    /// Vertex patch table.
    patch_table: CpuPatchTable,
    /// Far::PatchTable retained for callers that need raw Far data.
    ///
    /// Mirrors C++ `Mesh::_farPatchTable` / `GetFarPatchTable()`.
    far_patch_table: Box<PatchTable>,
    /// Vertex stencil table (coarse -> refined, optionally with local points).
    stencil_table: Box<StencilTable>,
    /// Varying stencil table (optional).
    varying_stencil_table: Option<Box<StencilTable>>,
    /// Descriptor for the vertex portion of vertex_buffer.
    vertex_desc: BufferDescriptor,
    /// Descriptor for the varying portion.
    varying_desc: BufferDescriptor,
}

impl CpuMesh {
    /// Build a CpuMesh from a TopologyRefiner.
    ///
    /// Mirrors `Osd::Mesh<CpuVertexBuffer, StencilTable, CpuEvaluator, CpuPatchTable>::Mesh(
    ///           refiner, numVertexElements, numVaryingElements, level, bits)`.
    pub fn new(
        mut refiner: TopologyRefiner,
        num_vertex_elements: i32,
        num_varying_elements: i32,
        level: i32,
        bits: MeshBitset,
    ) -> Self {
        // --- Refine topology ------------------------------------------------
        if bits.test(mesh_bits::ADAPTIVE) {
            let mut opts = AdaptiveOptions::new(level as u32);
            opts.use_single_crease_patch = bits.test(mesh_bits::USE_SINGLE_CREASE_PATCH);
            opts.use_inf_sharp_patch = bits.test(mesh_bits::USE_INF_SHARP_PATCH);
            opts.consider_fvar_channels = bits.test(mesh_bits::FVAR_ADAPTIVE);
            // Pass empty selected_faces slice via ConstArray.
            refiner.refine_adaptive(opts, ConstArray::new(&[]));
        } else {
            let full_topo = refiner.get_num_fvar_channels() > 0;
            let mut opts = UniformOptions::new(level as u32);
            opts.full_topology_in_last_level = full_topo;
            refiner.refine_uniform(opts);
        }

        // Coarse mesh vertex count from level-0 — used for the dst offset in Refine()
        // and matches what C++ reads from `_refiner->GetLevel(0).GetNumVertices()`.
        let num_ctrl_verts = refiner.get_level(0).get_num_vertices();

        // --- Stencil tables -------------------------------------------------
        let mut st_opts = StencilTableOptions::default();
        st_opts.generate_offsets = true;
        st_opts.generate_intermediate_levels = !refiner.is_uniform();

        // `mut` is required so local-point stencils can be appended once
        // PatchTable's stub is replaced with a real StencilTable.
        #[allow(unused_mut)]
        let mut vertex_stencils = if num_vertex_elements > 0 {
            StencilTableFactory::create(&refiner, st_opts)
        } else {
            Box::new(StencilTable::new())
        };

        #[allow(unused_mut)]
        let mut varying_stencils = if num_varying_elements > 0 {
            let mut vo = st_opts;
            vo.interpolation_mode = InterpolationMode::Varying;
            Some(StencilTableFactory::create(&refiner, vo))
        } else {
            None
        };

        // --- Patch table ----------------------------------------------------
        let mut po = PatchTableFactoryOptions::default();
        po.generate_fvar_tables = bits.test(mesh_bits::FVAR_DATA);
        po.generate_fvar_legacy_linear_patches = !bits.test(mesh_bits::FVAR_ADAPTIVE);
        // Propagate sharp-corner / crease / inf-sharp options — mirrors C++ initializeContext.
        po.generate_legacy_sharp_corner_patches = !bits.test(mesh_bits::USE_SMOOTH_CORNER_PATCH);
        po.use_single_crease_patch = bits.test(mesh_bits::USE_SINGLE_CREASE_PATCH);
        po.use_inf_sharp_patch = bits.test(mesh_bits::USE_INF_SHARP_PATCH);
        // Propagate end-cap type from MeshBitset (exclusive bits 7-10).
        if bits.test(mesh_bits::END_CAP_BILINEAR_BASIS) {
            po.end_cap_type = EndCapType::BilinearBasis;
            po.share_end_cap_patch_points = true;
        } else if bits.test(mesh_bits::END_CAP_BSPLINE_BASIS) {
            po.end_cap_type = EndCapType::BSplineBasis;
        } else if bits.test(mesh_bits::END_CAP_GREGORY_BASIS) {
            po.end_cap_type = EndCapType::GregoryBasis;
            po.share_end_cap_patch_points = true;
        } else if bits.test(mesh_bits::END_CAP_LEGACY_GREGORY) {
            po.end_cap_type = EndCapType::LegacyGregory;
        }

        // No selected_faces restriction — use empty slice.
        let far_patch = PatchTableFactory::create(&refiner, po, &[]);
        let max_valence = far_patch.get_max_valence();

        // --- Append local-point (end-cap) stencils --------------------------
        // If the patch table produced local-point stencils (Gregory basis end-caps),
        // merge them into the regular stencil tables.  Matches C++ initializeContext:
        //   if (_farPatchTable->GetLocalPointStencilTable())
        //       vertexStencils = StencilTableFactory::AppendLocalPointStencilTable(...)
        //
        // Append vertex local-point (end-cap) stencils when present.
        if far_patch.get_num_local_points() > 0 {
            if let Some(local_st) = far_patch.get_local_point_stencil_table_as_stencil_table() {
                if let Some(merged) = StencilTableFactory::append_local_point_stencil_table(
                    &refiner,
                    &vertex_stencils,
                    local_st,
                    false,
                ) {
                    vertex_stencils = merged;
                }
            }
        }
        // Append varying local-point stencils when present.
        if far_patch.get_num_local_points_varying() > 0 {
            if let (Some(local_vst), Some(ref base_vst)) = (
                far_patch.get_local_point_varying_stencil_table_as_stencil_table(),
                varying_stencils.as_deref(),
            ) {
                if let Some(merged) = StencilTableFactory::append_local_point_stencil_table_varying(
                    &refiner, base_vst, local_vst, false,
                ) {
                    varying_stencils = Some(merged);
                }
            }
        }

        // Rebuild OSD-side patch table from the Far patch table.
        let patch_table = CpuPatchTable::from_far(&far_patch);

        // Total vertices = coarse control verts + stencil outputs (+ end-cap local points).
        // Uses stencil_table.get_num_control_vertices() which equals num_ctrl_verts for
        // vertex stencils, and stencil count which covers all refined + local-point verts.
        let num_vertices =
            vertex_stencils.get_num_control_vertices() + vertex_stencils.get_num_stencils();

        // --- Vertex buffers -------------------------------------------------
        let vertex_buffer_stride = num_vertex_elements
            + if bits.test(mesh_bits::INTERLEAVE_VARYING) {
                num_varying_elements
            } else {
                0
            };
        let varying_buffer_stride = if bits.test(mesh_bits::INTERLEAVE_VARYING) {
            0
        } else {
            num_varying_elements
        };

        let vertex_buffer = CpuVertexBuffer::new(vertex_buffer_stride, num_vertices);
        let varying_buffer = if varying_buffer_stride > 0 {
            Some(CpuVertexBuffer::new(varying_buffer_stride, num_vertices))
        } else {
            None
        };

        // --- Buffer descriptors ---------------------------------------------
        let vertex_desc = BufferDescriptor::new(0, num_vertex_elements, vertex_buffer_stride);
        let varying_desc = if bits.test(mesh_bits::INTERLEAVE_VARYING) {
            BufferDescriptor::new(
                num_vertex_elements,
                num_varying_elements,
                vertex_buffer_stride,
            )
        } else {
            BufferDescriptor::new(0, num_varying_elements, varying_buffer_stride)
        };

        Self {
            num_vertices,
            num_ctrl_verts,
            max_valence,
            vertex_buffer,
            varying_buffer,
            patch_table,
            far_patch_table: Box::new(far_patch),
            stencil_table: vertex_stencils,
            varying_stencil_table: varying_stencils,
            vertex_desc,
            varying_desc,
        }
    }

    // -----------------------------------------------------------------------
    //  Direct API (also exposed via MeshInterface impl below)
    // -----------------------------------------------------------------------

    pub fn get_num_vertices(&self) -> i32 {
        self.num_vertices
    }
    pub fn get_max_valence(&self) -> i32 {
        self.max_valence
    }

    /// Upload coarse vertex data.
    pub fn update_vertex_buffer(&mut self, data: &[f32], start_vertex: i32, num_verts: i32) {
        VertexBuffer::update_data(&mut self.vertex_buffer, data, start_vertex, num_verts);
    }

    /// Upload coarse varying data.
    pub fn update_varying_buffer(&mut self, data: &[f32], start_vertex: i32, num_verts: i32) {
        if let Some(ref mut buf) = self.varying_buffer {
            VertexBuffer::update_data(buf, data, start_vertex, num_verts);
        }
    }

    /// Return a reference to the Far patch table retained after construction.
    ///
    /// Mirrors C++ `Mesh::GetFarPatchTable()`.
    pub fn get_far_patch_table(&self) -> &PatchTable {
        &self.far_patch_table
    }

    /// Number of coarse control vertices (level-0).
    pub fn get_num_ctrl_verts(&self) -> i32 {
        self.num_ctrl_verts
    }

    /// Evaluate stencils to push coarse vertices into refined positions.
    ///
    /// Mirrors `Osd::Mesh::Refine()` — calls `CpuEvaluator::EvalStencils` for
    /// vertex data and optionally for varying data.
    pub fn refine(&mut self) {
        // Use the stored coarse vertex count (from level-0 of the refiner at
        // construction time), matching C++ `refiner->GetLevel(0).GetNumVertices()`.
        let num_ctrl = self.num_ctrl_verts;

        // Vertex stencils: src = [0, num_ctrl), dst offset past coarse verts.
        let src_desc = self.vertex_desc;
        let mut dst_desc = src_desc;
        dst_desc.offset += num_ctrl * dst_desc.stride;

        // Clone src to satisfy borrow checker (src and dst are the same buffer).
        let src_data = VertexBuffer::bind_cpu_buffer(&self.vertex_buffer).to_vec();
        CpuEvaluator::eval_stencils_raw(
            &src_data,
            &src_desc,
            VertexBuffer::bind_cpu_buffer_mut(&mut self.vertex_buffer),
            &dst_desc,
            self.stencil_table.sizes(),
            self.stencil_table.offsets(),
            self.stencil_table.indices(),
            self.stencil_table.weights(),
            0,
            self.stencil_table.get_num_stencils(),
        );

        // Varying stencils (only if present).
        if self.varying_desc.length > 0 {
            if let Some(ref vst) = self.varying_stencil_table {
                let v_ctrl = vst.get_num_control_vertices();
                let n_st = vst.get_num_stencils();
                let v_src = self.varying_desc;
                let mut v_dst = v_src;
                v_dst.offset += v_ctrl * v_dst.stride;

                // Clone stencil arrays to release borrow on vst before buf borrow.
                let (sizes, offsets, indices, weights) = (
                    vst.sizes().to_vec(),
                    vst.offsets().to_vec(),
                    vst.indices().to_vec(),
                    vst.weights().to_vec(),
                );

                if let Some(ref mut vary_buf) = self.varying_buffer {
                    // Non-interleaved: separate varying buffer — clone src.
                    let v_src_data = VertexBuffer::bind_cpu_buffer(vary_buf).to_vec();
                    CpuEvaluator::eval_stencils_raw(
                        &v_src_data,
                        &v_src,
                        VertexBuffer::bind_cpu_buffer_mut(vary_buf),
                        &v_dst,
                        &sizes,
                        &offsets,
                        &indices,
                        &weights,
                        0,
                        n_st,
                    );
                } else {
                    // Interleaved: varying lives in the vertex_buffer — clone src.
                    let v_src_data = VertexBuffer::bind_cpu_buffer(&self.vertex_buffer).to_vec();
                    CpuEvaluator::eval_stencils_raw(
                        &v_src_data,
                        &v_src,
                        VertexBuffer::bind_cpu_buffer_mut(&mut self.vertex_buffer),
                        &v_dst,
                        &sizes,
                        &offsets,
                        &indices,
                        &weights,
                        0,
                        n_st,
                    );
                }
            }
        }
    }

    /// Synchronize — no-op for CPU backend.
    pub fn synchronize() {
        CpuEvaluator::synchronize();
    }

    pub fn get_patch_table(&self) -> &CpuPatchTable {
        &self.patch_table
    }
    pub fn get_vertex_buffer(&self) -> &CpuVertexBuffer {
        &self.vertex_buffer
    }
    pub fn get_varying_buffer(&self) -> Option<&CpuVertexBuffer> {
        self.varying_buffer.as_ref()
    }

    /// Bind vertex buffer — returns raw f32 slice.
    pub fn bind_vertex_buffer(&self) -> &[f32] {
        VertexBuffer::bind_cpu_buffer(&self.vertex_buffer)
    }

    /// Bind varying buffer — returns raw f32 slice if present.
    pub fn bind_varying_buffer(&self) -> Option<&[f32]> {
        self.varying_buffer
            .as_ref()
            .map(|b| VertexBuffer::bind_cpu_buffer(b))
    }
}

/// Implement the `MeshInterface` trait for `CpuMesh`.
impl MeshInterface for CpuMesh {
    type VertexBufferBinding = *const f32;

    fn get_num_vertices(&self) -> i32 {
        self.num_vertices
    }
    fn get_max_valence(&self) -> i32 {
        self.max_valence
    }
    fn update_vertex_buffer(&mut self, vertex_data: &[f32], start_vertex: i32, num_verts: i32) {
        CpuMesh::update_vertex_buffer(self, vertex_data, start_vertex, num_verts);
    }
    fn update_varying_buffer(&mut self, varying_data: &[f32], start_vertex: i32, num_verts: i32) {
        CpuMesh::update_varying_buffer(self, varying_data, start_vertex, num_verts);
    }
    fn refine(&mut self) {
        CpuMesh::refine(self);
    }
    fn synchronize(&mut self) {
        CpuEvaluator::synchronize();
    }
    fn get_patch_table(&self) -> &dyn std::any::Any {
        &self.patch_table
    }
    fn get_far_patch_table(&self) -> Option<&PatchTable> {
        Some(&self.far_patch_table)
    }
    fn bind_vertex_buffer(&mut self) -> *const f32 {
        VertexBuffer::bind_cpu_buffer(&self.vertex_buffer).as_ptr()
    }
    fn bind_varying_buffer(&mut self) -> *const f32 {
        self.varying_buffer
            .as_ref()
            .map(|b| VertexBuffer::bind_cpu_buffer(b).as_ptr())
            .unwrap_or(std::ptr::null())
    }
}
