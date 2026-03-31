use crate::far::{PatchHandle, PatchDescriptor, PatchParam as FarPatchParam};

/// Parametric coordinate on a specific patch — mirrors Osd::PatchCoord.
///
/// A 5-field struct: the three fields of PatchHandle plus (s, t).
#[derive(Debug, Clone, Copy, Default)]
pub struct PatchCoord {
    /// Handle identifying which array and patch this coordinate belongs to.
    pub handle: PatchHandle,
    /// Parametric s coordinate on the patch.
    pub s: f32,
    /// Parametric t coordinate on the patch.
    pub t: f32,
}

impl PatchCoord {
    pub fn new(handle: PatchHandle, s: f32, t: f32) -> Self {
        Self { handle, s, t }
    }
}

/// Describes a contiguous block of patches of (potentially mixed) type.
///
/// Mirrors Osd::PatchArray.  The CPU patch table holds a Vec<PatchArray> and
/// uses it together with the flat index buffer and patch param buffer during
/// limit-surface evaluation.
#[derive(Debug, Clone)]
pub struct PatchArray {
    /// Regular (base) patch descriptor — the type when the patch is regular.
    pub reg_desc: PatchDescriptor,
    /// Irregular (end-cap) descriptor — same as reg_desc for uniform arrays.
    pub desc: PatchDescriptor,
    /// Total number of patches in this array.
    pub num_patches: i32,
    /// First index into the global CV index buffer for this array.
    pub index_base: i32,
    /// Stride between patches in the CV index buffer
    /// (max of reg/irreg control-vertex counts).
    pub stride: i32,
    /// First index into the global patch-param buffer for this array.
    pub primitive_id_base: i32,
}

impl PatchArray {
    /// Construct a uniform (single-type) patch array.
    pub fn new(
        desc: PatchDescriptor,
        num_patches: i32,
        index_base: i32,
        primitive_id_base: i32,
    ) -> Self {
        let stride = desc.get_num_control_vertices();
        Self {
            reg_desc: desc,
            desc,
            num_patches,
            index_base,
            stride,
            primitive_id_base,
        }
    }

    /// Construct a mixed (regular + irregular) patch array.
    pub fn new_mixed(
        reg_desc: PatchDescriptor,
        irreg_desc: PatchDescriptor,
        num_patches: i32,
        index_base: i32,
        primitive_id_base: i32,
    ) -> Self {
        let stride = reg_desc
            .get_num_control_vertices()
            .max(irreg_desc.get_num_control_vertices());
        Self {
            reg_desc,
            desc: irreg_desc,
            num_patches,
            index_base,
            stride,
            primitive_id_base,
        }
    }

    /// Irregular (end-cap) descriptor — same as `get_descriptor()`.
    pub fn get_descriptor(&self) -> PatchDescriptor {
        self.desc
    }

    pub fn get_descriptor_regular(&self) -> PatchDescriptor {
        self.reg_desc
    }

    pub fn get_descriptor_irregular(&self) -> PatchDescriptor {
        self.desc
    }

    /// Patch type integer for regular patches.
    pub fn get_patch_type_regular(&self) -> i32 {
        self.reg_desc.get_type()
    }

    /// Patch type integer for irregular (end-cap) patches.
    pub fn get_patch_type_irregular(&self) -> i32 {
        self.desc.get_type()
    }

    /// Patch type — delegates to the irregular (end-cap) descriptor.
    ///
    /// Mirrors C++ `Osd::PatchArray::GetPatchType()` which returns `desc.GetType()`.
    pub fn get_patch_type(&self) -> i32 {
        self.desc.get_type()
    }

    pub fn get_num_patches(&self) -> i32 {
        self.num_patches
    }

    pub fn get_index_base(&self) -> i32 {
        self.index_base
    }

    pub fn get_stride(&self) -> i32 {
        self.stride
    }

    pub fn get_primitive_id_base(&self) -> i32 {
        self.primitive_id_base
    }
}

/// Osd-side patch param — extends Far::PatchParam with a sharpness field.
///
/// The extra `sharpness` float is spliced in when building CpuPatchTable from
/// a Far::PatchTable (using the sharpness index/value tables).
#[derive(Debug, Clone, Copy, Default)]
pub struct PatchParam {
    /// Inherited bit-packed fields from Far::PatchParam.
    pub field0: i32,
    pub field1: i32,
    /// Per-patch crease sharpness (0.0 if none).
    pub sharpness: f32,
}

impl PatchParam {
    pub fn new(field0: i32, field1: i32, sharpness: f32) -> Self {
        Self { field0, field1, sharpness }
    }

    /// Copy from a Far::PatchParam, setting sharpness explicitly.
    pub fn from_far(far: &FarPatchParam, sharpness: f32) -> Self {
        Self {
            field0: far.field0,
            field1: far.field1,
            sharpness,
        }
    }
}

/// Convenience alias — a patch-array list used by GPU patch table builders.
pub type PatchArrayVector = Vec<PatchArray>;

/// Convenience alias — a param list.
pub type PatchParamVector = Vec<PatchParam>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::far::{PatchDescriptor, PatchType};

    #[test]
    fn patch_coord_default() {
        let c = PatchCoord::default();
        assert_eq!(c.s, 0.0);
        assert_eq!(c.t, 0.0);
        assert_eq!(c.handle.array_index, 0);
    }

    #[test]
    fn patch_array_stride_uniform() {
        let desc = PatchDescriptor::new(PatchType::Regular);
        let pa = PatchArray::new(desc, 4, 0, 0);
        assert_eq!(pa.get_stride(), 16); // Regular = 16 CVs
    }

    #[test]
    fn patch_array_stride_mixed() {
        let reg = PatchDescriptor::new(PatchType::Regular);      // 16 CVs
        let irr = PatchDescriptor::new(PatchType::GregoryBasis); // 20 CVs
        let pa = PatchArray::new_mixed(reg, irr, 2, 0, 0);
        assert_eq!(pa.get_stride(), 20); // max(16, 20)
    }

    #[test]
    fn patch_param_from_far() {
        let fp = crate::far::PatchParam::new(42, 99);
        let op = PatchParam::from_far(&fp, 1.5);
        assert_eq!(op.field0, 42);
        assert_eq!(op.field1, 99);
        assert_eq!(op.sharpness, 1.5);
    }
}
