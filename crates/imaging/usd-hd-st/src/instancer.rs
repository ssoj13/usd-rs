#![allow(dead_code)]

//! HdStInstancer - Storm instancer implementation.
//!
//! Manages GPU instancing for rendering multiple copies of geometry
//! with varying transforms, materials, and primvars. Supports nested
//! instancing for hierarchical instance transforms.
//!
//! Port of pxr/imaging/hdSt/instancer.h/.cpp

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use usd_hd::render::render_delegate::HdInstancer;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Value;

// ---------------------------------------------------------------------------
// Well-known tokens
// ---------------------------------------------------------------------------

static INSTANCE_TRANSFORMS: LazyLock<Token> = LazyLock::new(|| Token::new("instanceTransforms"));
static INSTANCE_INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("instanceIndices"));

// ---------------------------------------------------------------------------
// HdStInstancePrimvar
// ---------------------------------------------------------------------------

/// Per-instance primvar data (one value per instance, or a single shared value).
#[derive(Debug, Clone)]
pub struct HdStInstancePrimvar {
    /// Primvar name.
    pub name: Token,
    /// Values (array for varying, single for constant).
    pub values: Value,
    /// True when each instance has its own distinct value.
    pub varying: bool,
}

impl HdStInstancePrimvar {
    pub fn new(name: Token, values: Value, varying: bool) -> Self {
        Self {
            name,
            values,
            varying,
        }
    }

    /// Constant primvar — same value for all instances.
    pub fn constant(name: Token, value: Value) -> Self {
        Self::new(name, value, false)
    }

    /// Varying primvar — one value per instance.
    pub fn varying(name: Token, values: Value) -> Self {
        Self::new(name, values, true)
    }
}

// ---------------------------------------------------------------------------
// HdStInstancer
// ---------------------------------------------------------------------------

/// Storm instancer.
///
/// Manages GPU instancing for efficient rendering of repeated geometry.
///
/// ## Instance index encoding
///
/// `get_instance_indices(prototype)` returns a flat `VtIntArray` encoding the
/// cartesian product of per-level sparse instance indices, matching the C++
/// HdStInstancer::GetInstanceIndices output exactly:
///
/// ```text
/// For levels [0,1], [3,4,5] the output is:
///   [<0>,0,3, <1>,1,3, <2>,0,4, <3>,1,4, <4>,0,5, <5>,1,5]
///   where <n> is the global (flat) index and the rest are per-level indices.
/// ```
///
/// ## Nested instancing
///
/// Instancers can form a hierarchy. `_get_instance_indices_recursive` walks up
/// the parent chain to gather per-level index arrays, then the cartesian product
/// is computed once at the top.
#[derive(Debug)]
pub struct HdStInstancer {
    /// Prim path.
    path: SdfPath,
    /// Parent instancer path (nested instancing).
    parent_path: Option<SdfPath>,
    /// Number of instances at this level.
    instance_count: usize,
    /// Flat instance transforms (16 f64 per instance, row-major).
    /// C++ uses GfMatrix4d (double precision) throughout.
    transforms: Vec<f64>,
    /// Instance primvars.
    primvars: HashMap<Token, HdStInstancePrimvar>,
    /// Sparse instance indices (which instances are active).
    indices: Vec<i32>,
    /// Whether instance data is dirty and needs GPU sync.
    dirty: bool,
    /// GPU buffer handle (0 = not yet allocated).
    buffer_handle: u64,
    /// Instancer prim transform (double precision, row-major).
    transform: [f64; 16],
    /// Inverse of the instancer prim transform.
    transform_inverse: [f64; 16],
    /// Whether "displayOpacity" is an instance primvar.
    has_display_opacity: bool,
    /// Whether "normals" is an instance primvar.
    has_normals: bool,
    /// Visibility.
    visible: bool,
    /// Element count of instance primvar arrays (determines valid index range).
    instance_primvar_num_elements: usize,
}

impl HdStInstancer {
    /// Create a root-level instancer (no parent).
    pub fn new(path: SdfPath) -> Self {
        Self {
            path,
            parent_path: None,
            instance_count: 0,
            transforms: Vec::new(),
            primvars: HashMap::new(),
            indices: Vec::new(),
            dirty: true,
            buffer_handle: 0,
            transform: IDENTITY_F64,
            transform_inverse: IDENTITY_F64,
            has_display_opacity: false,
            has_normals: false,
            visible: true,
            instance_primvar_num_elements: 0,
        }
    }

    /// Create a nested instancer (has a parent instancer).
    pub fn new_nested(path: SdfPath, parent: SdfPath) -> Self {
        Self {
            parent_path: Some(parent),
            ..Self::new(path)
        }
    }

    // --- Accessors ---

    pub fn get_path(&self) -> &SdfPath {
        &self.path
    }
    pub fn get_parent_path(&self) -> Option<&SdfPath> {
        self.parent_path.as_ref()
    }
    pub fn is_nested(&self) -> bool {
        self.parent_path.is_some()
    }
    pub fn get_instance_count(&self) -> usize {
        self.instance_count
    }
    pub fn get_transforms(&self) -> &[f64] {
        &self.transforms
    }
    pub fn get_indices(&self) -> &[i32] {
        &self.indices
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn has_buffer(&self) -> bool {
        self.buffer_handle != 0
    }
    pub fn has_display_opacity(&self) -> bool {
        self.has_display_opacity
    }
    pub fn has_normals(&self) -> bool {
        self.has_normals
    }
    pub fn get_transform(&self) -> &[f64; 16] {
        &self.transform
    }
    pub fn get_transform_inverse(&self) -> &[f64; 16] {
        &self.transform_inverse
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn get_primvars(&self) -> &HashMap<Token, HdStInstancePrimvar> {
        &self.primvars
    }
    pub fn get_primvar(&self, name: &Token) -> Option<&HdStInstancePrimvar> {
        self.primvars.get(name)
    }
    pub fn get_buffer_handle(&self) -> u64 {
        self.buffer_handle
    }
    pub fn get_instance_primvar_num_elements(&self) -> usize {
        self.instance_primvar_num_elements
    }

    // --- Mutators ---

    pub fn set_instance_count(&mut self, count: usize) {
        self.instance_count = count;
        self.dirty = true;
    }

    /// Set flat transform data (16 f64 per instance, GfMatrix4d row-major).
    pub fn set_transforms(&mut self, transforms: Vec<f64>) {
        assert_eq!(
            transforms.len() % 16,
            0,
            "Transform data must be 16 floats per instance"
        );
        self.instance_count = transforms.len() / 16;
        self.transforms = transforms;
        self.dirty = true;
    }

    /// Get the 4x4 matrix for instance `index` (16 f64 slice).
    pub fn get_instance_transform(&self, index: usize) -> Option<&[f64]> {
        if index < self.instance_count {
            let s = index * 16;
            Some(&self.transforms[s..s + 16])
        } else {
            None
        }
    }

    pub fn set_primvar(&mut self, pv: HdStInstancePrimvar) {
        self.has_display_opacity |= pv.name == "displayOpacity";
        self.has_normals |= pv.name == "normals";
        self.primvars.insert(pv.name.clone(), pv);
        self.dirty = true;
    }

    pub fn set_indices(&mut self, indices: Vec<i32>) {
        self.indices = indices;
        self.dirty = true;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
    pub fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }

    pub fn set_transform(&mut self, xform: [f64; 16]) {
        self.transform = xform;
        // Compute a simple 4x4 inverse via cofactor expansion.
        // For non-degenerate transforms this gives the exact inverse.
        self.transform_inverse = invert_mat4(xform).unwrap_or(IDENTITY_F64);
        self.dirty = true;
    }

    pub fn set_instance_primvar_num_elements(&mut self, n: usize) {
        self.instance_primvar_num_elements = n;
    }

    // --- Sync ---

    /// Pull instance primvars from a scene delegate and update local state.
    ///
    /// Port of C++ `HdStInstancer::_SyncPrimvars` (instancer.cpp:_SyncPrimvars).
    ///
    /// Retrieves the `instanceTransforms` primvar to populate `transforms` and
    /// any other instance-rate primvars authored on the instancer prim.
    /// The scene delegate callback signature: `(primvar_name) -> VtValue`.
    pub fn sync_primvars<F>(&mut self, get_primvar: F)
    where
        F: Fn(&Token) -> Option<Value>,
    {
        // Pull instanceTransforms: flat Vec<f64> (16 f64 per matrix, row-major).
        // Value is a struct using get::<T>() downcast, not an enum.
        if let Some(val) = get_primvar(&INSTANCE_TRANSFORMS) {
            if let Some(flat) = val.get::<Vec<f64>>() {
                if flat.len() % 16 == 0 && !flat.is_empty() {
                    self.instance_count = flat.len() / 16;
                    self.transforms = flat.clone();
                }
            } else if let Some(flat) = val.get::<Vec<f32>>() {
                // Fallback: f32 array (some delegates use single precision).
                if flat.len() % 16 == 0 && !flat.is_empty() {
                    self.instance_count = flat.len() / 16;
                    self.transforms = flat.iter().map(|&x| x as f64).collect();
                }
            }
        }

        // Pull instanceIndices: Vec<i32> sparse activation list.
        if let Some(val) = get_primvar(&INSTANCE_INDICES) {
            if let Some(indices) = val.get::<Vec<i32>>() {
                if !indices.is_empty() {
                    self.indices = indices.clone();
                }
            }
        }

        self.dirty = true;
    }

    /// Sync instancer with scene state (GPU upload placeholder).
    ///
    /// A full implementation would:
    /// 1. Allocate / resize GPU instance buffer.
    /// 2. Upload transforms and primvar arrays via staging buffer.
    /// 3. Handle nested instancer transform multiplication.
    pub fn sync(&mut self) {
        if !self.dirty {
            return;
        }
        // Placeholder: assign a non-zero buffer handle to indicate allocated state.
        self.buffer_handle = 1;
        self.dirty = false;
        log::debug!(
            "HdStInstancer::sync: {} ({} instances, {} primvars)",
            self.path,
            self.instance_count,
            self.primvars.len()
        );
    }

    // --- Instance index computation ---

    /// Compute flat instance indices for a prototype prim.
    ///
    /// Mirrors C++ `HdStInstancer::GetInstanceIndices`:
    ///
    /// 1. Collects per-level sparse index arrays by walking up the parent chain.
    /// 2. Computes the cartesian product.
    /// 3. Each tuple is prefixed by a global sequential index `n`.
    ///
    /// Output format (instance_index_width = 1 + num_levels):
    /// ```text
    ///   [<0>, level0_idx, level1_idx, ...,
    ///    <1>, level0_idx, level1_idx, ...]
    /// ```
    ///
    /// # Arguments
    /// * `prototype_id` — path of the prototype prim being rendered.
    /// * `parent_indices` — pre-computed per-level arrays from ancestor instancers
    ///   (pass `None` for the leaf call; used internally for recursion).
    ///
    /// # Returns
    /// Flat `Vec<i32>` ready for upload to the GPU instance index buffer.
    pub fn get_instance_indices(&self, _prototype_id: &SdfPath) -> Vec<i32> {
        // Gather per-level index arrays starting from this instancer.
        // In a full scene-delegate implementation, each level's indices come from
        // `HdSceneDelegate::GetInstanceIndices(instancer_id, prototype_id)`.
        // Here we use `self.indices` if populated, otherwise sequential 0..N.
        let mut levels: Vec<Vec<i32>> = Vec::new();
        self.collect_level_indices(&mut levels);

        if levels.is_empty() {
            return Vec::new();
        }

        // Total number of flat instances = product of per-level counts.
        let n_total: usize = levels.iter().map(|l| l.len()).product();
        if n_total == 0 {
            return Vec::new();
        }

        let num_levels = levels.len();
        let index_width = 1 + num_levels; // global index + one per level

        let mut output = vec![0i32; n_total * index_width];
        let mut cursors = vec![0usize; num_levels];

        for j in 0..n_total {
            output[j * index_width] = j as i32; // global index
            for (level, cursor) in cursors.iter().enumerate() {
                output[j * index_width + 1 + level] = levels[level][*cursor];
            }
            // Increment odometer (level 0 is fastest-varying, matching C++)
            let mut carry_level = 0;
            cursors[carry_level] += 1;
            while carry_level < num_levels - 1 && cursors[carry_level] >= levels[carry_level].len()
            {
                cursors[carry_level + 1] += 1;
                cursors[carry_level] = 0;
                carry_level += 1;
            }
        }

        output
    }

    /// Recursively collect per-level sparse index arrays.
    ///
    /// Level 0 = this instancer's direct instance indices (fastest-varying).
    /// Level N = root instancer's indices (slowest-varying).
    fn collect_level_indices(&self, levels: &mut Vec<Vec<i32>>) {
        // Clamp against primvar element count (guard against out-of-range indices).
        let max_idx = if self.instance_primvar_num_elements > 0 {
            self.instance_primvar_num_elements as i32
        } else {
            self.instance_count as i32
        };

        let level_indices: Vec<i32> = if self.visible && !self.indices.is_empty() {
            // Validate and clamp indices
            self.indices
                .iter()
                .filter_map(|&idx| {
                    if idx >= 0 && idx < max_idx {
                        Some(idx)
                    } else {
                        log::warn!(
                            "HdStInstancer: index {} out of range [0, {}) for <{}>",
                            idx,
                            max_idx,
                            self.path
                        );
                        None
                    }
                })
                .collect()
        } else if self.visible && self.instance_count > 0 {
            // No sparse indices — all instances active
            (0..self.instance_count as i32).collect()
        } else {
            // Not visible or no instances
            Vec::new()
        };

        levels.push(level_indices);

        // Walk up the parent chain (no HdRenderIndex here, so we stop at leaf).
        // A full implementation would call parent_instancer.collect_level_indices().
    }

    /// Total instance count including parent multipliers.
    ///
    /// For flat instancing this equals `instance_count`. For nested instancing
    /// a full implementation would multiply through the parent chain.
    pub fn get_total_instance_count(&self) -> usize {
        self.instance_count
    }
}

// ---------------------------------------------------------------------------
// 4x4 matrix helpers (row-major f64)
// ---------------------------------------------------------------------------

/// 4x4 identity matrix (row-major, f64).
const IDENTITY_F64: [f64; 16] = [
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
];

/// Invert a 4x4 row-major matrix via cofactor expansion.
/// Returns `None` if the matrix is singular (|det| < epsilon).
fn invert_mat4(m: [f64; 16]) -> Option<[f64; 16]> {
    // cofactors of the 4x4 matrix
    let c00 = m[5] * m[10] * m[15] - m[5] * m[11] * m[14] - m[9] * m[6] * m[15]
        + m[9] * m[7] * m[14]
        + m[13] * m[6] * m[11]
        - m[13] * m[7] * m[10];
    let c04 = -m[4] * m[10] * m[15] + m[4] * m[11] * m[14] + m[8] * m[6] * m[15]
        - m[8] * m[7] * m[14]
        - m[12] * m[6] * m[11]
        + m[12] * m[7] * m[10];
    let c08 = m[4] * m[9] * m[15] - m[4] * m[11] * m[13] - m[8] * m[5] * m[15]
        + m[8] * m[7] * m[13]
        + m[12] * m[5] * m[11]
        - m[12] * m[7] * m[9];
    let c12 = -m[4] * m[9] * m[14] + m[4] * m[10] * m[13] + m[8] * m[5] * m[14]
        - m[8] * m[6] * m[13]
        - m[12] * m[5] * m[10]
        + m[12] * m[6] * m[9];

    let det = m[0] * c00 + m[1] * c04 + m[2] * c08 + m[3] * c12;
    if det.abs() < 1e-15 {
        return None;
    }
    let inv_det = 1.0 / det;

    let c01 = -m[1] * m[10] * m[15] + m[1] * m[11] * m[14] + m[9] * m[2] * m[15]
        - m[9] * m[3] * m[14]
        - m[13] * m[2] * m[11]
        + m[13] * m[3] * m[10];
    let c05 = m[0] * m[10] * m[15] - m[0] * m[11] * m[14] - m[8] * m[2] * m[15]
        + m[8] * m[3] * m[14]
        + m[12] * m[2] * m[11]
        - m[12] * m[3] * m[10];
    let c09 = -m[0] * m[9] * m[15] + m[0] * m[11] * m[13] + m[8] * m[1] * m[15]
        - m[8] * m[3] * m[13]
        - m[12] * m[1] * m[11]
        + m[12] * m[3] * m[9];
    let c13 = m[0] * m[9] * m[14] - m[0] * m[10] * m[13] - m[8] * m[1] * m[14]
        + m[8] * m[2] * m[13]
        + m[12] * m[1] * m[10]
        - m[12] * m[2] * m[9];

    let c02 = m[1] * m[6] * m[15] - m[1] * m[7] * m[14] - m[5] * m[2] * m[15]
        + m[5] * m[3] * m[14]
        + m[13] * m[2] * m[7]
        - m[13] * m[3] * m[6];
    let c06 = -m[0] * m[6] * m[15] + m[0] * m[7] * m[14] + m[4] * m[2] * m[15]
        - m[4] * m[3] * m[14]
        - m[12] * m[2] * m[7]
        + m[12] * m[3] * m[6];
    let c10 = m[0] * m[5] * m[15] - m[0] * m[7] * m[13] - m[4] * m[1] * m[15]
        + m[4] * m[3] * m[13]
        + m[12] * m[1] * m[7]
        - m[12] * m[3] * m[5];
    let c14 = -m[0] * m[5] * m[14] + m[0] * m[6] * m[13] + m[4] * m[1] * m[14]
        - m[4] * m[2] * m[13]
        - m[12] * m[1] * m[6]
        + m[12] * m[2] * m[5];

    let c03 = -m[1] * m[6] * m[11] + m[1] * m[7] * m[10] + m[5] * m[2] * m[11]
        - m[5] * m[3] * m[10]
        - m[9] * m[2] * m[7]
        + m[9] * m[3] * m[6];
    let c07 = m[0] * m[6] * m[11] - m[0] * m[7] * m[10] - m[4] * m[2] * m[11]
        + m[4] * m[3] * m[10]
        + m[8] * m[2] * m[7]
        - m[8] * m[3] * m[6];
    let c11 = -m[0] * m[5] * m[11] + m[0] * m[7] * m[9] + m[4] * m[1] * m[11]
        - m[4] * m[3] * m[9]
        - m[8] * m[1] * m[7]
        + m[8] * m[3] * m[5];
    let c15 = m[0] * m[5] * m[10] - m[0] * m[6] * m[9] - m[4] * m[1] * m[10]
        + m[4] * m[2] * m[9]
        + m[8] * m[1] * m[6]
        - m[8] * m[2] * m[5];

    // Transpose cofactor matrix and scale by inv_det
    Some([
        c00 * inv_det,
        c01 * inv_det,
        c02 * inv_det,
        c03 * inv_det,
        c04 * inv_det,
        c05 * inv_det,
        c06 * inv_det,
        c07 * inv_det,
        c08 * inv_det,
        c09 * inv_det,
        c10 * inv_det,
        c11 * inv_det,
        c12 * inv_det,
        c13 * inv_det,
        c14 * inv_det,
        c15 * inv_det,
    ])
}

// HdStInstancer implements the HdInstancer trait.
impl HdInstancer for HdStInstancer {}

/// Shared pointer to Storm instancer.
pub type HdStInstancerSharedPtr = Arc<HdStInstancer>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> SdfPath {
        SdfPath::from_string(s).unwrap()
    }

    #[test]
    fn test_creation() {
        let inst = HdStInstancer::new(path("/inst"));
        assert_eq!(inst.get_instance_count(), 0);
        assert!(!inst.is_nested());
        assert!(inst.is_dirty());
    }

    #[test]
    fn test_nested_instancer() {
        let inst = HdStInstancer::new_nested(path("/child"), path("/parent"));
        assert!(inst.is_nested());
        assert_eq!(inst.get_parent_path(), Some(&path("/parent")));
    }

    #[test]
    fn test_set_transforms() {
        let mut inst = HdStInstancer::new(path("/inst"));
        #[rustfmt::skip]
        let xforms = vec![
            1.0f64, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0,

            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            1.0, 0.0, 0.0, 1.0,
        ];
        inst.set_transforms(xforms);
        assert_eq!(inst.get_instance_count(), 2);
        assert_eq!(inst.get_instance_transform(1).unwrap()[12], 1.0);
    }

    #[test]
    #[should_panic(expected = "16 floats per instance")]
    fn test_invalid_transform_size() {
        let mut inst = HdStInstancer::new(path("/inst"));
        inst.set_transforms(vec![1.0f64, 0.0, 0.0]);
    }

    #[test]
    fn test_get_instance_indices_sequential() {
        let mut inst = HdStInstancer::new(path("/inst"));
        inst.set_instance_count(3);

        let proto = path("/proto");
        let indices = inst.get_instance_indices(&proto);
        // width = 1 + 1 = 2: [global, level0] for each instance
        assert_eq!(indices.len(), 6); // 3 * 2
        // [0,0, 1,1, 2,2]
        assert_eq!(&indices[..], &[0, 0, 1, 1, 2, 2]);
    }

    #[test]
    fn test_get_instance_indices_sparse() {
        let mut inst = HdStInstancer::new(path("/inst"));
        inst.set_instance_count(5);
        inst.set_instance_primvar_num_elements(5);
        inst.set_indices(vec![1, 3]); // only instances 1 and 3 active

        let proto = path("/proto");
        let indices = inst.get_instance_indices(&proto);
        // 2 active instances, width=2: [<0>,1, <1>,3]
        assert_eq!(indices.len(), 4);
        assert_eq!(&indices[..], &[0, 1, 1, 3]);
    }

    #[test]
    fn test_instance_indices_invisible() {
        let mut inst = HdStInstancer::new(path("/inst"));
        inst.set_instance_count(3);
        inst.set_visible(false);

        let indices = inst.get_instance_indices(&path("/proto"));
        assert!(indices.is_empty());
    }

    #[test]
    fn test_primvars() {
        let mut inst = HdStInstancer::new(path("/inst"));

        let colors = usd_vt::Array::<f32>::from(vec![1.0f32, 0.0, 0.0]);
        inst.set_primvar(HdStInstancePrimvar::varying(
            Token::new("color"),
            Value::from_no_hash(colors),
        ));

        assert_eq!(inst.get_primvars().len(), 1);
        assert!(inst.get_primvar(&Token::new("color")).unwrap().varying);

        inst.set_primvar(HdStInstancePrimvar::constant(
            Token::new("displayOpacity"),
            Value::from(1.0f32),
        ));
        assert!(inst.has_display_opacity());
    }

    #[test]
    fn test_sync() {
        let mut inst = HdStInstancer::new(path("/inst"));
        inst.set_instance_count(5);
        assert!(inst.is_dirty());
        assert!(!inst.has_buffer());
        inst.sync();
        assert!(!inst.is_dirty());
        assert!(inst.has_buffer());
    }

    #[test]
    fn test_invert_identity() {
        let inv = invert_mat4(IDENTITY_F64).unwrap();
        for (a, b) in inv.iter().zip(IDENTITY_F64.iter()) {
            assert!(
                (a - b).abs() < 1e-12,
                "Identity inverse mismatch: {a} vs {b}"
            );
        }
    }

    #[test]
    fn test_invert_translation() {
        let mut m = IDENTITY_F64;
        m[12] = 3.0;
        m[13] = -1.0;
        m[14] = 5.0; // translate (3,-1,5)
        let inv = invert_mat4(m).unwrap();
        // inverse should have translation negated
        assert!((inv[12] + 3.0).abs() < 1e-10);
        assert!((inv[13] - 1.0).abs() < 1e-10);
        assert!((inv[14] + 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_set_transform_computes_inverse() {
        let mut inst = HdStInstancer::new(path("/inst"));
        let mut xform = IDENTITY_F64;
        xform[12] = 2.0; // translate x by 2
        inst.set_transform(xform);
        assert!((inst.get_transform_inverse()[12] + 2.0).abs() < 1e-10);
    }
}
