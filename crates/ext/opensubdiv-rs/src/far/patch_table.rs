use crate::far::{Index, PatchDescriptor, PatchParam};
use crate::far::patch_descriptor::PatchType;
use crate::far::stencil_table::StencilTable;
use crate::sdc::options::FVarLinearInterpolation;
use crate::osd::patch_basis::{OsdPatchParam, evaluate_patch_basis as osd_evaluate_patch_basis};

/// Handle for locating a specific patch within a PatchTable.
///
/// Mirrors Far::PatchTable::PatchHandle (3 packed indices).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PatchHandle {
    /// Index of the patch array containing this patch.
    pub array_index: i32,
    /// Absolute index of this patch across all arrays.
    pub patch_index: i32,
    /// Relative offset to the first CV *within the array's* vertex buffer.
    /// (Not an absolute offset — `index_base` from the array must be added.)
    pub vert_index: i32,
}

/// A single patch array entry — the Far-side equivalent of Osd::PatchArray.
///
/// Mirrors C++ `PatchTable::PatchArray`:
///   desc / numPatches / vertIndex / patchIndex / quadOffsetIndex
#[derive(Debug, Clone)]
pub struct PatchArrayDescriptorInfo {
    pub descriptor: PatchDescriptor,
    pub num_patches: i32,
    /// Absolute offset into `patch_vertices` for the first CV of this array.
    /// Mirrors C++ `vertIndex`.
    pub index_base: i32,
    /// Absolute index of the first patch in the global `patch_params` table.
    /// Mirrors C++ `patchIndex`.
    pub patch_index_base: i32,
    /// Index of the first quad-offset entry in `quad_offsets_table`.
    /// Mirrors C++ `quadOffsetIndex`. Zero for non-Gregory and uniform patches.
    pub quad_offset_index: i32,
}

/// Per-channel face-varying patch data.
///
/// Mirrors C++ `PatchTable::FVarPatchChannel`:
///   regDesc / irregDesc / stride / patchValues / patchParam — all per-channel.
#[derive(Debug, Default)]
pub struct FVarChannel {
    /// Descriptor for regular fvar patches in this channel.
    pub regular_desc: PatchDescriptor,
    /// Descriptor for irregular fvar patches in this channel.
    pub irregular_desc: PatchDescriptor,
    /// Stride between consecutive patches in the `vertices` array
    /// (= max(regular_desc.num_cvs, irregular_desc.num_cvs)).
    pub stride: i32,
    /// Linear interpolation mode for this fvar channel.
    /// Mirrors C++ `FVarPatchChannel::interpolation`.
    pub interpolation: FVarLinearInterpolation,
    /// Flat array of fvar CV indices: `numPatches * stride` entries.
    pub vertices: Vec<Index>,
    /// PatchParam for each patch (one per patch, across all arrays).
    pub params: Vec<PatchParam>,
}

/// The compiled patch table produced by PatchTableFactory.
///
/// Holds all patch arrays, their control vertex indices, and per-patch params.
/// Mirrors C++ `Far::PatchTable`.
#[derive(Debug)]
pub struct PatchTable {
    /// One entry per patch array (one per patch type present in the mesh).
    pub(crate) patch_arrays: Vec<PatchArrayDescriptorInfo>,
    /// Packed control-vertex indices for all patches, all arrays.
    pub(crate) patch_vertices: Vec<Index>,
    /// Per-patch parametrization data (one per patch, across all arrays).
    pub(crate) patch_params: Vec<PatchParam>,
    /// Varying patch descriptor (for separate varying evaluation).
    pub(crate) varying_desc: PatchDescriptor,
    /// Varying CV indices per patch (`numPatches * varying_desc.num_cvs`).
    pub(crate) varying_vertices: Vec<Index>,
    /// FVar channels: one per channel, each holding per-channel flat arrays.
    pub(crate) fvar_channels: Vec<FVarChannel>,
    /// Sharpness index table (one per patch, INDEX_INVALID if no crease).
    pub(crate) sharpness_indices: Vec<Index>,
    /// Per-crease sharpness values referenced by sharpness_indices.
    pub(crate) sharpness_values: Vec<f32>,
    /// Legacy Gregory quad-offsets table (one u32 per CV in Gregory patches).
    pub(crate) quad_offsets_table: Vec<u32>,
    /// Vertex valence table (for Gregory patches).
    pub(crate) vertex_valence_table: Vec<Index>,
    /// Local-point stencil table for vertex primvars (endcap patches).
    /// Populated by PatchTableFactory when end-cap type != LegacyGregory.
    /// Mirrors C++ `PatchTable::_localPointStencils`.
    pub(crate) local_point_stencil_table: Option<Box<StencilTable>>,
    /// Local-point stencil table for varying primvars.
    /// Mirrors C++ `PatchTable::_localPointVaryingStencils`.
    pub(crate) local_point_varying_stencil_table: Option<Box<StencilTable>>,
    /// Per-channel local-point stencil tables for face-varying primvars.
    /// Mirrors C++ `PatchTable::_localPointFaceVaryingStencils`.
    pub(crate) local_point_fvar_stencil_tables: Vec<Option<Box<StencilTable>>>,
    /// Maximum ring valence encountered during patch table construction.
    pub(crate) max_valence: i32,
    /// Total number of ptex faces (populated by factory).
    pub(crate) num_ptex_faces: i32,
    /// True when patches are of uniform linear types.
    pub(crate) is_uniform_linear: bool,
}

impl Default for PatchTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PatchTable {
    /// Construct a new empty PatchTable.
    ///
    /// Mirrors C++ `PatchTable::PatchTable()` which initializes
    /// `_varyingDesc(Far::PatchDescriptor::QUADS)` explicitly.
    pub fn new() -> Self {
        Self {
            patch_arrays:                        Vec::new(),
            patch_vertices:                      Vec::new(),
            patch_params:                        Vec::new(),
            // C++ PatchTable() constructor sets _varyingDesc(PatchDescriptor::QUADS)
            varying_desc:                        PatchDescriptor::new(PatchType::Quads),
            varying_vertices:                    Vec::new(),
            fvar_channels:                       Vec::new(),
            sharpness_indices:                   Vec::new(),
            sharpness_values:                    Vec::new(),
            quad_offsets_table:                  Vec::new(),
            vertex_valence_table:                Vec::new(),
            local_point_stencil_table:           None,
            local_point_varying_stencil_table:   None,
            local_point_fvar_stencil_tables:     Vec::new(),
            max_valence:                         0,
            num_ptex_faces:                      0,
            is_uniform_linear:                   false,
        }
    }

    // -----------------------------------------------------------------------
    //  Global queries
    // -----------------------------------------------------------------------

    /// True when the patches are of feature-adaptive (non-linear) types.
    pub fn is_feature_adaptive(&self) -> bool {
        !self.is_uniform_linear
    }

    /// Total number of control vertex indices in the table.
    pub fn get_num_control_vertices_total(&self) -> i32 {
        self.patch_vertices.len() as i32
    }

    /// Total number of patches across all arrays.
    ///
    /// Mirrors C++ `PatchTable::GetNumPatchesTotal()` which returns `_paramTable.size()`.
    #[doc(alias = "GetNumPatchesTotal")]
    pub fn get_num_patches_total(&self) -> i32 {
        self.patch_params.len() as i32
    }

    /// Maximum vertex valence found in the mesh.
    pub fn get_max_valence(&self) -> i32 {
        self.max_valence
    }

    /// Total number of ptex faces in the mesh.
    pub fn get_num_ptex_faces(&self) -> i32 {
        self.num_ptex_faces
    }

    // -----------------------------------------------------------------------
    //  Per-handle accessors
    // -----------------------------------------------------------------------

    /// PatchDescriptor for the patch identified by `handle`.
    #[doc(alias = "GetPatchDescriptor")]
    pub fn get_patch_descriptor(&self, handle: &PatchHandle) -> PatchDescriptor {
        self.patch_arrays[handle.array_index as usize].descriptor
    }

    /// Control-vertex indices for the patch identified by `handle`.
    ///
    /// Mirrors C++: `pa.vertIndex + handle.vertIndex` is the absolute start.
    pub fn get_patch_vertices(&self, handle: &PatchHandle) -> &[Index] {
        let info  = &self.patch_arrays[handle.array_index as usize];
        let nv    = info.descriptor.get_num_control_vertices() as usize;
        let start = (info.index_base + handle.vert_index) as usize;
        &self.patch_vertices[start..start + nv]
    }

    /// PatchParam for the patch identified by `handle`.
    pub fn get_patch_param(&self, handle: &PatchHandle) -> PatchParam {
        self.patch_params[handle.patch_index as usize]
    }

    /// Control-vertex indices for `patch` within `array`.
    pub fn get_patch_vertices_by_array(&self, array: i32, patch: i32) -> &[Index] {
        let info  = &self.patch_arrays[array as usize];
        let nv    = info.descriptor.get_num_control_vertices() as usize;
        let start = info.index_base as usize + patch as usize * nv;
        &self.patch_vertices[start..start + nv]
    }

    /// PatchParam for `patch` within `array`.
    pub fn get_patch_param_by_array(&self, array: i32, patch: i32) -> PatchParam {
        let info = &self.patch_arrays[array as usize];
        self.patch_params[(info.patch_index_base + patch) as usize]
    }

    // -----------------------------------------------------------------------
    //  Patch array accessors
    // -----------------------------------------------------------------------

    /// Number of patch arrays in the table.
    pub fn get_num_patch_arrays(&self) -> i32 {
        self.patch_arrays.len() as i32
    }

    /// Number of patches in `array`.
    pub fn get_num_patches(&self, array: i32) -> i32 {
        self.patch_arrays[array as usize].num_patches
    }

    /// Number of control vertices in `array`.
    pub fn get_num_control_vertices(&self, array: i32) -> i32 {
        let info = &self.patch_arrays[array as usize];
        info.num_patches * info.descriptor.get_num_control_vertices()
    }

    /// PatchDescriptor for the patches in `array`.
    pub fn get_patch_array_descriptor(&self, array: i32) -> PatchDescriptor {
        self.patch_arrays[array as usize].descriptor
    }

    /// All control-vertex indices for the patches in `array`.
    pub fn get_patch_array_vertices(&self, array: i32) -> &[Index] {
        let info  = &self.patch_arrays[array as usize];
        let nv    = info.descriptor.get_num_control_vertices() as usize;
        let start = info.index_base as usize;
        let end   = start + info.num_patches as usize * nv;
        &self.patch_vertices[start..end]
    }

    /// All PatchParams for the patches in `array`.
    pub fn get_patch_params_by_array(&self, array: i32) -> &[PatchParam] {
        let info  = &self.patch_arrays[array as usize];
        let start = info.patch_index_base as usize;
        let end   = start + info.num_patches as usize;
        &self.patch_params[start..end]
    }

    // -----------------------------------------------------------------------
    //  Single-crease patch sharpness
    // -----------------------------------------------------------------------

    /// Sharpness for the patch identified by `handle`, or 0 if not a
    /// single-crease patch.
    ///
    /// Mirrors C++: checks `INDEX_INVALID` (which is -1 as i32).
    pub fn get_single_crease_patch_sharpness_value(&self, handle: &PatchHandle) -> f32 {
        if self.sharpness_indices.is_empty() { return 0.0; }
        let idx = self.sharpness_indices[handle.patch_index as usize];
        // INDEX_INVALID == -1 (or 0xFFFFFFFF as unsigned, stored as i32 -1)
        if idx == -1 || (idx as usize) >= self.sharpness_values.len() {
            0.0
        } else {
            self.sharpness_values[idx as usize]
        }
    }

    /// Sharpness for `patch` within `array`, or 0 if not a single-crease patch.
    pub fn get_single_crease_patch_sharpness_value_by_array(&self, array: i32, patch: i32) -> f32 {
        if self.sharpness_indices.is_empty() { return 0.0; }
        let info      = &self.patch_arrays[array as usize];
        let patch_idx = (info.patch_index_base + patch) as usize;
        let idx       = self.sharpness_indices[patch_idx];
        if idx == -1 || (idx as usize) >= self.sharpness_values.len() {
            0.0
        } else {
            self.sharpness_values[idx as usize]
        }
    }

    // -----------------------------------------------------------------------
    //  Legacy Gregory patch tables
    // -----------------------------------------------------------------------

    /// Quad-offsets for the Gregory patch identified by `handle` (4 entries).
    ///
    /// Mirrors C++ `PatchTable::GetPatchQuadOffsets(PatchHandle)`:
    ///   `return ConstIndexArray(&_quadOffsetsTable[pa.quadOffsetIndex + handle.vertIndex], 4)`
    pub fn get_patch_quad_offsets(&self, handle: &PatchHandle) -> &[u32] {
        if self.quad_offsets_table.is_empty() {
            return &[];
        }
        let info  = &self.patch_arrays[handle.array_index as usize];
        // C++: pa.quadOffsetIndex + handle.vertIndex
        let start = (info.quad_offset_index + handle.vert_index) as usize;
        let end   = start + 4;
        if end <= self.quad_offsets_table.len() {
            &self.quad_offsets_table[start..end]
        } else {
            &[]
        }
    }

    /// Vertex valence table (for Gregory patches).
    pub fn get_vertex_valence_table(&self) -> &[Index] {
        &self.vertex_valence_table
    }

    // -----------------------------------------------------------------------
    //  Varying patch accessors
    // -----------------------------------------------------------------------

    pub fn get_varying_patch_descriptor(&self) -> PatchDescriptor {
        self.varying_desc
    }

    /// Varying CV indices for the patch identified by `handle`.
    ///
    /// Mirrors C++: `start = handle.patchIndex * numVaryingCVs`.
    pub fn get_patch_varying_vertices(&self, handle: &PatchHandle) -> &[Index] {
        if self.varying_vertices.is_empty() { return &[]; }
        let nv    = self.varying_desc.get_num_control_vertices() as usize;
        if nv == 0 { return &[]; }
        let start = handle.patch_index as usize * nv;
        let end   = start + nv;
        if end <= self.varying_vertices.len() { &self.varying_vertices[start..end] } else { &[] }
    }

    /// Varying CV indices for `patch` within `array`.
    ///
    /// Mirrors C++: `start = (pa.patchIndex + patch) * numVaryingCVs`.
    pub fn get_patch_varying_vertices_by_array(&self, array: i32, patch: i32) -> &[Index] {
        if self.varying_vertices.is_empty() { return &[]; }
        let nv   = self.varying_desc.get_num_control_vertices() as usize;
        if nv == 0 { return &[]; }
        let info  = &self.patch_arrays[array as usize];
        let start = (info.patch_index_base + patch) as usize * nv;
        let end   = start + nv;
        if end <= self.varying_vertices.len() { &self.varying_vertices[start..end] } else { &[] }
    }

    /// All varying CV indices for the patches in `array`.
    ///
    /// Mirrors C++: `start = pa.patchIndex * numVaryingCVs`.
    pub fn get_patch_array_varying_vertices(&self, array: i32) -> &[Index] {
        if self.varying_vertices.is_empty() { return &[]; }
        let info = &self.patch_arrays[array as usize];
        let nv   = self.varying_desc.get_num_control_vertices() as usize;
        if nv == 0 { return &[]; }
        let start = info.patch_index_base as usize * nv;
        let end   = start + info.num_patches as usize * nv;
        if end <= self.varying_vertices.len() { &self.varying_vertices[start..end] } else { &[] }
    }

    pub fn get_varying_vertices(&self) -> &[Index] {
        &self.varying_vertices
    }

    // -----------------------------------------------------------------------
    //  FVar accessors
    // -----------------------------------------------------------------------

    pub fn get_num_f_var_channels(&self) -> i32 {
        self.fvar_channels.len() as i32
    }

    /// Linear interpolation mode for `channel`.
    ///
    /// Mirrors C++ `GetFVarChannelLinearInterpolation(channel)`.
    pub fn get_f_var_channel_linear_interpolation(&self, channel: i32) -> FVarLinearInterpolation {
        self.fvar_channels[channel as usize].interpolation
    }

    /// Stride between patches in the fvar value index array of `channel`.
    ///
    /// Mirrors C++ `GetFVarValueStride(channel)`.
    pub fn get_f_var_value_stride(&self, channel: i32) -> i32 {
        self.fvar_channels[channel as usize].stride
    }

    /// Regular patch descriptor for `channel`.
    ///
    /// Mirrors C++ `GetFVarPatchDescriptorRegular(channel)`.
    pub fn get_f_var_patch_descriptor_regular(&self, channel: i32) -> PatchDescriptor {
        self.fvar_channels[channel as usize].regular_desc
    }

    /// Irregular patch descriptor for `channel`.
    ///
    /// Mirrors C++ `GetFVarPatchDescriptorIrregular(channel)`.
    pub fn get_f_var_patch_descriptor_irregular(&self, channel: i32) -> PatchDescriptor {
        self.fvar_channels[channel as usize].irregular_desc
    }

    /// Default/irregular patch descriptor for `channel`.
    ///
    /// Mirrors C++ `GetFVarPatchDescriptor(channel)` which returns `irregDesc`.
    pub fn get_f_var_patch_descriptor(&self, channel: i32) -> PatchDescriptor {
        self.fvar_channels[channel as usize].irregular_desc
    }

    /// All fvar value indices for `channel`.
    ///
    /// Mirrors C++ `GetFVarValues(channel)`.
    pub fn get_f_var_values(&self, channel: i32) -> &[Index] {
        &self.fvar_channels[channel as usize].vertices
    }

    /// Fvar value indices for the patch identified by `handle` in `channel`.
    ///
    /// Mirrors C++: size depends on whether the patch is regular or irregular.
    pub fn get_patch_f_var_values(&self, handle: &PatchHandle, channel: i32) -> &[Index] {
        let ch    = &self.fvar_channels[channel as usize];
        let patch = handle.patch_index as usize;
        let ncvs  = if patch < ch.params.len() && ch.params[patch].is_regular() {
            ch.regular_desc.get_num_control_vertices() as usize
        } else {
            ch.irregular_desc.get_num_control_vertices() as usize
        };
        let start = patch * ch.stride as usize;
        let end   = start + ncvs;
        if end <= ch.vertices.len() { &ch.vertices[start..end] } else { &[] }
    }

    /// Fvar value indices for `patch` within `array` in `channel`.
    ///
    /// Mirrors C++: delegates to global patch index lookup.
    pub fn get_patch_f_var_values_by_array(&self, array: i32, patch: i32, channel: i32) -> &[Index] {
        let info       = &self.patch_arrays[array as usize];
        let global_idx = info.patch_index_base + patch;
        let ch         = &self.fvar_channels[channel as usize];
        let ncvs       = if (global_idx as usize) < ch.params.len()
                            && ch.params[global_idx as usize].is_regular() {
            ch.regular_desc.get_num_control_vertices() as usize
        } else {
            ch.irregular_desc.get_num_control_vertices() as usize
        };
        let start = global_idx as usize * ch.stride as usize;
        let end   = start + ncvs;
        if end <= ch.vertices.len() { &ch.vertices[start..end] } else { &[] }
    }

    /// All fvar value indices for the patches in `array` in `channel`.
    ///
    /// Mirrors C++: `start = pa.patchIndex * stride`.
    pub fn get_patch_array_f_var_values(&self, array: i32, channel: i32) -> &[Index] {
        let ch    = &self.fvar_channels[channel as usize];
        let info  = &self.patch_arrays[array as usize];
        let start = info.patch_index_base as usize * ch.stride as usize;
        let count = info.num_patches as usize * ch.stride as usize;
        let end   = start + count;
        if end <= ch.vertices.len() { &ch.vertices[start..end] } else { &[] }
    }

    /// FVar PatchParam for the patch identified by `handle` in `channel`.
    pub fn get_patch_f_var_patch_param(&self, handle: &PatchHandle, channel: i32) -> PatchParam {
        self.fvar_channels[channel as usize].params[handle.patch_index as usize]
    }

    /// FVar PatchParam for `patch` within `array` in `channel`.
    pub fn get_patch_f_var_patch_param_by_array(&self, array: i32, patch: i32, channel: i32) -> PatchParam {
        let info  = &self.patch_arrays[array as usize];
        let idx   = (info.patch_index_base + patch) as usize;
        self.fvar_channels[channel as usize].params[idx]
    }

    /// All FVar PatchParams for the patches in `array` in `channel`.
    ///
    /// Mirrors C++: `&c.patchParam[pa.patchIndex]`.
    pub fn get_patch_array_f_var_patch_params(&self, array: i32, channel: i32) -> &[PatchParam] {
        let ch    = &self.fvar_channels[channel as usize];
        let info  = &self.patch_arrays[array as usize];
        let start = info.patch_index_base as usize;
        let end   = start + info.num_patches as usize;
        if end <= ch.params.len() { &ch.params[start..end] } else { &[] }
    }

    /// All FVar PatchParams for `channel`.
    ///
    /// Mirrors C++ `GetFVarPatchParams(channel)`.
    pub fn get_f_var_patch_params(&self, channel: i32) -> &[PatchParam] {
        &self.fvar_channels[channel as usize].params
    }

    // -----------------------------------------------------------------------
    //  Direct/legacy table accessors
    // -----------------------------------------------------------------------

    /// Entire flat PatchParam table.
    pub fn get_patch_param_table(&self) -> &[PatchParam] {
        &self.patch_params
    }

    /// Entire flat patch-vertex index table.
    pub fn get_patch_control_vertices_table(&self) -> &[Index] {
        &self.patch_vertices
    }

    pub fn get_sharpness_index_table(&self) -> &[Index] {
        &self.sharpness_indices
    }

    pub fn get_sharpness_values(&self) -> &[f32] {
        &self.sharpness_values
    }

    pub fn get_quad_offsets_table(&self) -> &[u32] {
        &self.quad_offsets_table
    }

    // -----------------------------------------------------------------------
    //  Local point stencil tables
    // -----------------------------------------------------------------------

    /// Number of local (endcap) vertex points.
    ///
    /// Equals `get_local_point_stencil_table().map_or(0, |st| st.get_num_stencils())`.
    /// Returns 0 for uniform or LegacyGregory meshes.
    pub fn get_num_local_points(&self) -> i32 {
        self.local_point_stencil_table
            .as_ref()
            .map_or(0, |st| st.get_num_stencils())
    }

    /// Number of local varying points.
    pub fn get_num_local_points_varying(&self) -> i32 {
        self.local_point_varying_stencil_table
            .as_ref()
            .map_or(0, |st| st.get_num_stencils())
    }

    /// Number of local face-varying points for `channel`.
    pub fn get_num_local_points_face_varying(&self, channel: i32) -> i32 {
        self.local_point_fvar_stencil_tables
            .get(channel as usize)
            .and_then(|o| o.as_ref())
            .map_or(0, |st| st.get_num_stencils())
    }

    /// Local-point stencil table for vertex primvars.
    ///
    /// Returns `None` for uniform or LegacyGregory meshes.
    /// Mirrors C++ `PatchTable::GetLocalPointStencilTable()`.
    pub fn get_local_point_stencil_table(&self) -> Option<&StencilTable> {
        self.local_point_stencil_table.as_deref()
    }

    /// Local-point stencil table for varying primvars.
    ///
    /// Mirrors C++ `PatchTable::GetLocalPointVaryingStencilTable()`.
    pub fn get_local_point_varying_stencil_table(&self) -> Option<&StencilTable> {
        self.local_point_varying_stencil_table.as_deref()
    }

    /// Local-point stencil table for face-varying primvars in `channel`.
    ///
    /// Mirrors C++ `PatchTable::GetLocalPointFaceVaryingStencilTable(channel)`.
    pub fn get_local_point_face_varying_stencil_table(&self, channel: i32) -> Option<&StencilTable> {
        self.local_point_fvar_stencil_tables
            .get(channel as usize)
            .and_then(|o| o.as_deref())
    }

    // Compatibility aliases — kept for call-sites in mesh.rs that use the
    // "_as_stencil_table" suffix.  They delegate to the primary getters.
    pub fn get_local_point_stencil_table_as_stencil_table(&self) -> Option<&StencilTable> {
        self.get_local_point_stencil_table()
    }

    pub fn get_local_point_varying_stencil_table_as_stencil_table(&self) -> Option<&StencilTable> {
        self.get_local_point_varying_stencil_table()
    }

    /// Store the vertex local-point stencil table.
    ///
    /// Called by PatchTableFactory once end-cap stencils are computed.
    #[doc(hidden)]
    pub fn set_local_point_stencil_table(&mut self, st: Box<StencilTable>) {
        self.local_point_stencil_table = Some(st);
    }

    /// Store the varying local-point stencil table.
    #[doc(hidden)]
    pub fn set_local_point_varying_stencil_table(&mut self, st: Box<StencilTable>) {
        self.local_point_varying_stencil_table = Some(st);
    }

    // -----------------------------------------------------------------------
    //  ComputeLocalPointValues — stencil-based local point evaluation
    // -----------------------------------------------------------------------

    /// Compute local (endcap) point values for vertex primvars.
    ///
    /// Applies the local-point stencil table to transform refined source
    /// points into the patch-local points needed for Gregory endcap evaluation.
    ///
    /// Mirrors C++ `PatchTable::ComputeLocalPointValues<T>(src, dst)`.
    ///
    /// Each stencil produces one output point:
    ///   `dst[i] = sum_j(weight_j * src[index_j])`
    ///
    /// When no stencil table is present (uniform or LegacyGregory), `dst` is
    /// not modified.
    pub fn compute_local_point_values<T>(&self, src: &[T], dst: &mut [T])
    where
        T: Default + Clone,
        T: crate::far::primvar_refiner::Interpolatable,
    {
        let st = match self.local_point_stencil_table.as_ref() {
            Some(st) => st,
            None     => return,
        };
        apply_stencil_table(st, src, dst);
    }

    /// Compute local varying point values.
    ///
    /// Mirrors C++ `PatchTable::ComputeLocalPointValuesVarying<T>(src, dst)`.
    pub fn compute_local_point_values_varying<T>(&self, src: &[T], dst: &mut [T])
    where
        T: Default + Clone,
        T: crate::far::primvar_refiner::Interpolatable,
    {
        let st = match self.local_point_varying_stencil_table.as_ref() {
            Some(st) => st,
            None     => return,
        };
        apply_stencil_table(st, src, dst);
    }

    /// Compute local face-varying point values for `channel`.
    ///
    /// Mirrors C++ `PatchTable::ComputeLocalPointValuesFaceVarying<T>(src, dst, channel)`.
    pub fn compute_local_point_values_face_varying<T>(
        &self, src: &[T], dst: &mut [T], channel: i32,
    )
    where
        T: Default + Clone,
        T: crate::far::primvar_refiner::Interpolatable,
    {
        let st = match self.local_point_fvar_stencil_tables
            .get(channel as usize)
            .and_then(|o| o.as_ref())
        {
            Some(st) => st,
            None     => return,
        };
        apply_stencil_table(st, src, dst);
    }

    // -----------------------------------------------------------------------
    //  Builder helpers (used by PatchTableFactory)
    // -----------------------------------------------------------------------

    #[doc(hidden)]
    pub fn push_patch_array(&mut self, info: PatchArrayDescriptorInfo) {
        self.patch_arrays.push(info);
    }

    #[doc(hidden)]
    pub fn push_vertices(&mut self, verts: &[Index]) {
        self.patch_vertices.extend_from_slice(verts);
    }

    #[doc(hidden)]
    pub fn push_patch_params(&mut self, params: &[PatchParam]) {
        self.patch_params.extend_from_slice(params);
    }

    #[doc(hidden)]
    pub fn set_max_valence(&mut self, v: i32) {
        self.max_valence = v;
    }

    #[doc(hidden)]
    pub fn set_varying_desc(&mut self, desc: PatchDescriptor) {
        self.varying_desc = desc;
    }

    #[doc(hidden)]
    pub fn set_num_ptex_faces(&mut self, n: i32) {
        self.num_ptex_faces = n;
    }

    #[doc(hidden)]
    pub fn set_is_uniform_linear(&mut self, v: bool) {
        self.is_uniform_linear = v;
    }

    // -----------------------------------------------------------------------
    //  Basis evaluation — mirrors C++ PatchTable::EvaluateBasis<REAL>
    // -----------------------------------------------------------------------

    /// Evaluate vertex patch basis weights (and optional derivatives) at (s, t).
    ///
    /// Mirrors C++ `PatchTable::EvaluateBasis<float>(handle, s, t, wP, wDs, wDt, ...)`.
    /// The arrays must be large enough for the patch type (up to 20 elements).
    #[doc(alias = "EvaluateBasis")]
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_basis(
        &self,
        handle: &PatchHandle,
        s: f32, t: f32,
        w_p:   &mut [f32],
        w_ds:  Option<&mut [f32]>,
        w_dt:  Option<&mut [f32]>,
        w_dss: Option<&mut [f32]>,
        w_dst: Option<&mut [f32]>,
        w_dtt: Option<&mut [f32]>,
    ) -> i32 {
        let param     = self.patch_params[handle.patch_index as usize];
        let type_id   = patch_type_to_osd_id(self.get_patch_descriptor(handle).patch_type);
        let osd_param = OsdPatchParam::new(param.field0, param.field1, 0.0);
        osd_evaluate_patch_basis(type_id, &osd_param, s, t, w_p, w_ds, w_dt, w_dss, w_dst, w_dtt)
    }

    /// Evaluate varying patch basis weights at (s, t).
    ///
    /// Mirrors C++ `PatchTable::EvaluateBasisVarying<float>`.
    /// Uses the varying patch descriptor type (linear / bilinear).
    #[doc(alias = "EvaluateBasisVarying")]
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_basis_varying(
        &self,
        handle: &PatchHandle,
        s: f32, t: f32,
        w_p:   &mut [f32],
        w_ds:  Option<&mut [f32]>,
        w_dt:  Option<&mut [f32]>,
        w_dss: Option<&mut [f32]>,
        w_dst: Option<&mut [f32]>,
        w_dtt: Option<&mut [f32]>,
    ) -> i32 {
        let param     = self.patch_params[handle.patch_index as usize];
        let type_id   = patch_type_to_osd_id(self.varying_desc.patch_type);
        let osd_param = OsdPatchParam::new(param.field0, param.field1, 0.0);
        osd_evaluate_patch_basis(type_id, &osd_param, s, t, w_p, w_ds, w_dt, w_dss, w_dst, w_dtt)
    }

    /// Evaluate face-varying patch basis weights at (s, t).
    ///
    /// Mirrors C++ `PatchTable::EvaluateBasisFaceVarying<float>`.
    /// The fvar param and patch type are looked up per-channel.
    #[doc(alias = "EvaluateBasisFaceVarying")]
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_basis_face_varying(
        &self,
        handle:  &PatchHandle,
        s: f32, t: f32,
        w_p:   &mut [f32],
        w_ds:  Option<&mut [f32]>,
        w_dt:  Option<&mut [f32]>,
        w_dss: Option<&mut [f32]>,
        w_dst: Option<&mut [f32]>,
        w_dtt: Option<&mut [f32]>,
        channel: i32,
    ) -> i32 {
        // C++: param = getPatchFVarPatchParam(handle.patchIndex, channel);
        //      patchType = param.IsRegular() ? GetFVarPatchDescriptorRegular(...)
        //                                    : GetFVarPatchDescriptorIrregular(...);
        let fvar_param = if channel >= 0 && channel < self.fvar_channels.len() as i32 {
            let ch = &self.fvar_channels[channel as usize];
            let idx = handle.patch_index as usize;
            if idx < ch.params.len() { ch.params[idx] } else { PatchParam::default() }
        } else {
            PatchParam::default()
        };
        let patch_type = if fvar_param.is_regular() {
            self.get_f_var_patch_descriptor_regular(channel).patch_type
        } else {
            self.get_f_var_patch_descriptor_irregular(channel).patch_type
        };
        let type_id   = patch_type_to_osd_id(patch_type);
        let osd_param = OsdPatchParam::new(fvar_param.field0, fvar_param.field1, 0.0);
        osd_evaluate_patch_basis(type_id, &osd_param, s, t, w_p, w_ds, w_dt, w_dss, w_dst, w_dtt)
    }
}

// ---------------------------------------------------------------------------
// Helper: apply a StencilTable to compute local point values.
//
// For each stencil i: dst[i] = sum_j(weight_j * src[index_j])
// Mirrors C++ StencilTable::UpdateValues<T>.
// ---------------------------------------------------------------------------

fn apply_stencil_table<T>(st: &StencilTable, src: &[T], dst: &mut [T])
where
    T: Default + Clone,
    T: crate::far::primvar_refiner::Interpolatable,
{
    let sizes   = st.sizes();
    let indices = st.indices();
    let weights = st.weights();

    let mut elem = 0usize;
    for (i, &sz) in sizes.iter().enumerate() {
        let mut val = T::default();
        for _ in 0..sz as usize {
            val.add_with_weight(&src[indices[elem] as usize], weights[elem]);
            elem += 1;
        }
        dst[i] = val;
    }
}

// ---------------------------------------------------------------------------
// Helper: map Far PatchType to OSD integer patch-type code.
// Delegates to the canonical implementation in far::patch_basis to avoid
// duplication — both functions perform an identical match.
// ---------------------------------------------------------------------------

#[inline]
fn patch_type_to_osd_id(pt: PatchType) -> i32 {
    crate::far::patch_basis::far_patch_type_to_osd_code(pt)
}
