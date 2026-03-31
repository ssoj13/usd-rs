/// CPU-side patch table — a flat representation extracted from `Far::PatchTable`.
///
/// Mirrors `Osd::CpuPatchTable` from OpenSubdiv 3.7.0 `osd/cpuPatchTable.h/.cpp`.
///
/// `CpuPatchTable` exists primarily to:
/// 1. Provide a uniform Osd-level type that satisfies the `PATCH_TABLE` template
///    parameter expected by `Osd::Mesh<…>`.
/// 2. Splice per-patch sharpness values into `PatchParam` (which Far doesn't
///    store inline).
/// 3. Separate varying and face-varying patch data into independent arrays.
use crate::far::{PatchTable, Index};
use crate::osd::types::{PatchArray, PatchParam, PatchArrayVector, PatchParamVector};

/// CPU patch table — holds flat contiguous buffers for all patch data.
///
/// The type-name alias used in Mesh templates: `CpuPatchTable::VertexBufferBinding = *const f32`.
#[derive(Debug, Default)]
pub struct CpuPatchTable {
    // ----- Vertex patches -----
    pub patch_arrays:       PatchArrayVector,
    pub index_buffer:       Vec<Index>,
    pub patch_param_buffer: PatchParamVector,

    // ----- Varying patches -----
    pub varying_patch_arrays: PatchArrayVector,
    pub varying_index_buffer: Vec<Index>,

    // ----- Face-varying patches (one entry per channel) -----
    pub fvar_patch_arrays:  Vec<PatchArrayVector>,
    pub fvar_index_buffers: Vec<Vec<Index>>,
    pub fvar_param_buffers: Vec<PatchParamVector>,
}

impl CpuPatchTable {
    /// Build a CPU patch table from a `Far::PatchTable`.
    ///
    /// Mirrors the `CpuPatchTable(const Far::PatchTable *)` constructor.
    pub fn from_far(far: &PatchTable) -> Self {
        let n_arrays = far.get_num_patch_arrays() as usize;
        let n_fvar   = far.get_num_f_var_channels() as usize;

        let mut table = Self::default();

        // Count totals to pre-reserve
        let mut total_patches  = 0i32;
        let mut total_indices  = 0i32;
        for j in 0..n_arrays as i32 {
            let np = far.get_num_patches(j);
            let nv = far.get_patch_array_descriptor(j).get_num_control_vertices();
            total_patches += np;
            total_indices += np * nv;
        }
        table.patch_arrays.reserve(n_arrays);
        table.index_buffer.reserve(total_indices as usize);

        let varying_nv = far.get_varying_patch_descriptor().get_num_control_vertices();
        table.varying_patch_arrays.reserve(n_arrays);
        table.varying_index_buffer.reserve((total_patches * varying_nv) as usize);

        table.fvar_patch_arrays.resize(n_fvar, Vec::new());
        table.fvar_index_buffers.resize(n_fvar, Vec::new());
        table.fvar_param_buffers.resize(n_fvar, Vec::new());
        for fvc in 0..n_fvar as i32 {
            let stride = far.get_f_var_value_stride(fvc);
            table.fvar_patch_arrays[fvc as usize].reserve(n_arrays);
            table.fvar_index_buffers[fvc as usize].reserve((total_patches * stride) as usize);
            table.fvar_param_buffers[fvc as usize].reserve(total_patches as usize);
        }
        table.patch_param_buffer.reserve(total_patches as usize);

        let param_table      = far.get_patch_param_table();
        let sharpness_idx    = far.get_sharpness_index_table();
        let sharpness_vals   = far.get_sharpness_values();

        // Build per-array data
        for j in 0..n_arrays as i32 {
            let num_p = far.get_num_patches(j);
            let desc  = far.get_patch_array_descriptor(j);

            // Vertex patch array
            let pa = PatchArray::new(
                desc,
                num_p,
                table.index_buffer.len() as i32,
                table.patch_param_buffer.len() as i32,
            );
            table.patch_arrays.push(pa);

            // Append vertex indices
            let verts = far.get_patch_array_vertices(j);
            table.index_buffer.extend_from_slice(verts);

            // Varying patch array
            let vary_desc = far.get_varying_patch_descriptor();
            let vary_pa = PatchArray::new(
                vary_desc,
                num_p,
                table.varying_index_buffer.len() as i32,
                table.patch_param_buffer.len() as i32,
            );
            table.varying_patch_arrays.push(vary_pa);
            let vary_verts = far.get_patch_array_varying_vertices(j);
            table.varying_index_buffer.extend_from_slice(vary_verts);

            // Face-varying arrays per channel
            for fvc in 0..n_fvar as i32 {
                let fvc_idx = fvc as usize;
                let reg_desc  = far.get_f_var_patch_descriptor_regular(fvc);
                let irr_desc  = far.get_f_var_patch_descriptor_irregular(fvc);
                let fvar_pa = PatchArray::new_mixed(
                    reg_desc, irr_desc, num_p,
                    table.fvar_index_buffers[fvc_idx].len() as i32,
                    table.fvar_param_buffers[fvc_idx].len() as i32,
                );
                table.fvar_patch_arrays[fvc_idx].push(fvar_pa);

                let fvar_idx = far.get_patch_array_f_var_values(j, fvc);
                table.fvar_index_buffers[fvc_idx].extend_from_slice(fvar_idx);

                // FVar params (sharpness always 0 for fvar)
                let fvar_params = far.get_patch_array_f_var_patch_params(j, fvc);
                for fp in fvar_params {
                    table.fvar_param_buffers[fvc_idx].push(PatchParam::from_far(fp, 0.0));
                }
            }

            // Vertex patch params with sharpness
            let patch_base = table.patch_param_buffer.len();
            for k in 0..num_p as usize {
                let patch_idx = patch_base + k;
                let mut sharpness = 0.0_f32;
                if patch_idx < sharpness_idx.len() {
                    let si = sharpness_idx[patch_idx];
                    if si >= 0 && (si as usize) < sharpness_vals.len() {
                        sharpness = sharpness_vals[si as usize];
                    }
                }
                let fp = &param_table[patch_idx];
                table.patch_param_buffer.push(PatchParam::from_far(fp, sharpness));
            }
        }

        table
    }

    /// Factory method matching the C++ static `CpuPatchTable::Create`.
    pub fn create(far: &PatchTable) -> Self {
        Self::from_far(far)
    }

    /// Populate this table from a Far::PatchTable, clearing existing data first.
    ///
    /// Mirrors the C++ constructor `CpuPatchTable(const Far::PatchTable *)`.
    /// Useful when the table was default-constructed and must be filled later,
    /// e.g. after `AppendLocalPointStencilTable` re-creates the patch table.
    pub fn populate_from_patch_table(&mut self, far: &PatchTable) {
        *self = Self::from_far(far);
    }

    // -----------------------------------------------------------------------
    //  Accessors (mirrors C++ API)
    // -----------------------------------------------------------------------

    pub fn get_patch_array_buffer(&self) -> &[PatchArray] {
        &self.patch_arrays
    }

    pub fn get_patch_index_buffer(&self) -> &[Index] {
        &self.index_buffer
    }

    pub fn get_patch_param_buffer(&self) -> &[PatchParam] {
        &self.patch_param_buffer
    }

    pub fn get_num_patch_arrays(&self) -> usize {
        self.patch_arrays.len()
    }

    pub fn get_patch_index_size(&self) -> usize {
        self.index_buffer.len()
    }

    pub fn get_patch_param_size(&self) -> usize {
        self.patch_param_buffer.len()
    }

    pub fn get_varying_patch_array_buffer(&self) -> Option<&[PatchArray]> {
        if self.varying_patch_arrays.is_empty() { None }
        else { Some(&self.varying_patch_arrays) }
    }

    pub fn get_varying_patch_index_buffer(&self) -> Option<&[Index]> {
        if self.varying_index_buffer.is_empty() { None }
        else { Some(&self.varying_index_buffer) }
    }

    pub fn get_varying_patch_index_size(&self) -> usize {
        self.varying_index_buffer.len()
    }

    pub fn get_num_f_var_channels(&self) -> i32 {
        self.fvar_patch_arrays.len() as i32
    }

    pub fn get_f_var_patch_array_buffer(&self, channel: i32) -> &[PatchArray] {
        &self.fvar_patch_arrays[channel as usize]
    }

    pub fn get_f_var_patch_index_buffer(&self, channel: i32) -> &[Index] {
        &self.fvar_index_buffers[channel as usize]
    }

    pub fn get_f_var_patch_index_size(&self, channel: i32) -> usize {
        self.fvar_index_buffers[channel as usize].len()
    }

    pub fn get_f_var_patch_param_buffer(&self, channel: i32) -> &[PatchParam] {
        &self.fvar_param_buffers[channel as usize]
    }

    pub fn get_f_var_patch_param_size(&self, channel: i32) -> usize {
        self.fvar_param_buffers[channel as usize].len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let t = CpuPatchTable::default();
        assert_eq!(t.get_num_patch_arrays(), 0);
        assert_eq!(t.get_patch_index_size(), 0);
        assert_eq!(t.get_patch_param_size(), 0);
    }
}
