//! Closure runtime operations — allocation, mul, add. 100% safe Rust.
//!
//! Port of `opclosure.cpp`. Provides runtime functions for creating and
//! combining closure trees from within shader execution.
//!
//! # Why no arena allocator?
//!
//! The C++ OSL uses a bump-allocator arena (`ShadingContext::closure_arena`)
//! to allocate closure nodes within a contiguous byte slab. In this Rust port,
//! closures are `Arc<ClosureNode>` — reference-counted heap values that drop
//! automatically when the last reference goes away.

use std::collections::HashMap;
use std::sync::Arc;

use crate::closure::{ClosureLabels, ClosureNode, ClosureParam, ClosureParams, ClosureRef};
use crate::math::Vec3;
use crate::typedesc::TypeDesc;

// ---------------------------------------------------------------------------
// Default labels for standard closure IDs
// ---------------------------------------------------------------------------

/// Return the standard LPE labels for a known closure ID.
///
/// Maps standard closure IDs (diffuse, reflection, etc.) to their scattering
/// type and direction classification. Custom/unknown IDs get `ClosureLabels::NONE`.
pub fn default_labels_for_closure(id: i32) -> ClosureLabels {
    match id {
        closure_ids::DIFFUSE_ID
        | closure_ids::OREN_NAYAR_ID
        | closure_ids::OREN_NAYAR_DIFFUSE_BSDF_ID
        | closure_ids::BURLEY_DIFFUSE_BSDF_ID => ClosureLabels::DIFFUSE_REFLECT,
        closure_ids::PHONG_ID
        | closure_ids::WARD_ID
        | closure_ids::MICROFACET_ID
        | closure_ids::DIELECTRIC_BSDF_ID
        | closure_ids::CONDUCTOR_BSDF_ID
        | closure_ids::GENERALIZED_SCHLICK_BSDF_ID
        | closure_ids::SHEEN_BSDF_ID
        | closure_ids::CHIANG_HAIR_BSDF_ID => ClosureLabels::GLOSSY_REFLECT,
        closure_ids::REFLECTION_ID | closure_ids::FRESNEL_REFLECTION_ID => {
            ClosureLabels::SINGULAR_REFLECT
        }
        closure_ids::REFRACTION_ID => ClosureLabels::SINGULAR_TRANSMIT,
        closure_ids::TRANSPARENT_ID | closure_ids::TRANSPARENT_BSDF_ID => {
            ClosureLabels::STRAIGHT_TRANSMIT
        }
        closure_ids::TRANSLUCENT_ID
        | closure_ids::TRANSLUCENT_BSDF_ID
        | closure_ids::SUBSURFACE_BSSRDF_ID => ClosureLabels::DIFFUSE_TRANSMIT,
        _ => ClosureLabels::NONE,
    }
}

// ---------------------------------------------------------------------------
// Construction helpers
// ---------------------------------------------------------------------------

/// Create a closure component with unit (white) weight and auto-resolved labels.
pub fn allocate_closure_component(id: i32) -> ClosureRef {
    allocate_closure_component_labeled(id, default_labels_for_closure(id))
}

/// Create a closure component with unit weight and explicit labels.
pub fn allocate_closure_component_labeled(id: i32, labels: ClosureLabels) -> ClosureRef {
    allocate_closure_component_with_params(id, labels, ClosureParams::None)
}

/// Create a closure component with unit weight, explicit labels, and params.
pub fn allocate_closure_component_with_params(
    id: i32,
    labels: ClosureLabels,
    params: ClosureParams,
) -> ClosureRef {
    Arc::new(ClosureNode::Component {
        id,
        weight: Vec3::new(1.0, 1.0, 1.0),
        labels,
        params,
    })
}

/// Create a weighted closure component with auto-resolved labels.
/// Returns `None` if weight is zero (black).
pub fn allocate_weighted_closure_component(id: i32, weight: Vec3) -> Option<ClosureRef> {
    allocate_weighted_closure_component_labeled(id, weight, default_labels_for_closure(id))
}

/// Create a weighted closure component with explicit labels.
/// Returns `None` if weight is zero (black).
pub fn allocate_weighted_closure_component_labeled(
    id: i32,
    weight: Vec3,
    labels: ClosureLabels,
) -> Option<ClosureRef> {
    if weight.x == 0.0 && weight.y == 0.0 && weight.z == 0.0 {
        return None;
    }
    Some(Arc::new(ClosureNode::Component {
        id,
        weight,
        labels,
        params: ClosureParams::None,
    }))
}

/// Add two closures.
pub fn add_closure_closure(a: Option<ClosureRef>, b: Option<ClosureRef>) -> Option<ClosureRef> {
    match (a, b) {
        (None, b) => b,
        (a, None) => a,
        (Some(a), Some(b)) => Some(Arc::new(ClosureNode::Add { a, b })),
    }
}

/// Multiply a closure by a color weight.
pub fn mul_closure_color(closure: Option<ClosureRef>, w: Vec3) -> Option<ClosureRef> {
    let closure = closure?;
    if w.x == 0.0 && w.y == 0.0 && w.z == 0.0 {
        return None;
    }
    if w.x == 1.0 && w.y == 1.0 && w.z == 1.0 {
        return Some(closure);
    }
    Some(Arc::new(ClosureNode::Mul { weight: w, closure }))
}

/// Multiply a closure by a scalar weight.
pub fn mul_closure_float(closure: Option<ClosureRef>, w: f32) -> Option<ClosureRef> {
    mul_closure_color(closure, Vec3::new(w, w, w))
}

// ---------------------------------------------------------------------------
// Flatten — extract weighted components for the integrator
// ---------------------------------------------------------------------------

/// A flattened closure component: weight, ID, and LPE labels.
#[derive(Debug, Clone)]
pub struct FlatComponent {
    pub weight: Vec3,
    pub id: i32,
    pub labels: ClosureLabels,
}

/// Flatten a closure tree into weighted components.
pub fn flatten_closure(root: &ClosureNode) -> Vec<FlatComponent> {
    let mut result = Vec::new();
    flatten_inner(root, Vec3::new(1.0, 1.0, 1.0), &mut result);
    result
}

/// Flatten from an optional closure reference.
pub fn flatten_closure_opt(root: Option<&ClosureRef>) -> Vec<FlatComponent> {
    match root {
        Some(r) => flatten_closure(r),
        None => Vec::new(),
    }
}

fn flatten_inner(node: &ClosureNode, weight: Vec3, result: &mut Vec<FlatComponent>) {
    match node {
        ClosureNode::Component {
            id,
            weight: w,
            labels,
            ..
        } => {
            let final_weight = Vec3::new(weight.x * w.x, weight.y * w.y, weight.z * w.z);
            result.push(FlatComponent {
                weight: final_weight,
                id: *id,
                labels: *labels,
            });
        }
        ClosureNode::Mul { weight: w, closure } => {
            let new_weight = Vec3::new(weight.x * w.x, weight.y * w.y, weight.z * w.z);
            flatten_inner(closure, new_weight, result);
        }
        ClosureNode::Add { a, b } => {
            flatten_inner(a, weight, result);
            flatten_inner(b, weight, result);
        }
    }
}

// ---------------------------------------------------------------------------
// Pretty-print
// ---------------------------------------------------------------------------

/// Convert a closure tree to a human-readable string.
///
/// Output matches the C++ `print_closure` format:
/// - each leaf: `(w.x, w.y, w.z) * name (params...)`
/// - multiple leaves separated by `\n\t+ `
/// - Mul weights are accumulated and propagated down to leaves
pub fn closure_to_string(root: &ClosureNode) -> String {
    let mut out = String::new();
    let mut first = true;
    print_closure_inner(root, Vec3::new(1.0, 1.0, 1.0), &mut out, &mut first);
    out
}

pub fn closure_to_string_opt(root: Option<&ClosureRef>) -> String {
    match root {
        Some(r) => closure_to_string(r),
        None => "null".to_string(),
    }
}

/// Format closure params as `(p1, p2, ...)` matching C++ `print_component`.
fn fmt_closure_params(params: &ClosureParams) -> String {
    match params {
        ClosureParams::None => "()".to_string(),
        ClosureParams::Normal(n) => {
            format!("({}, {}, {})", n.x, n.y, n.z)
        }
        ClosureParams::NormalRoughness { n, roughness } => {
            format!("({}, {}, {}), {}", n.x, n.y, n.z, roughness)
        }
        ClosureParams::NormalIor { n, ior } => {
            format!("({}, {}, {}), {}", n.x, n.y, n.z, ior)
        }
        ClosureParams::NormalRoughnessIor { n, roughness, ior } => {
            format!("({}, {}, {}), {}, {}", n.x, n.y, n.z, roughness, ior)
        }
        ClosureParams::NormalColorRoughness {
            n,
            color,
            roughness,
        } => {
            format!(
                "({}, {}, {}), ({}, {}, {}), {}",
                n.x, n.y, n.z, color.x, color.y, color.z, roughness
            )
        }
        ClosureParams::Hair {
            tangent,
            color,
            melanin,
            roughness,
            ior,
            offset,
        } => {
            format!(
                "({}, {}, {}), ({}, {}, {}), {}, {}, {}, {}",
                tangent.x,
                tangent.y,
                tangent.z,
                color.x,
                color.y,
                color.z,
                melanin,
                roughness,
                ior,
                offset
            )
        }
        ClosureParams::Generic(_) => "(...)".to_string(),
    }
}

/// Recursive printer — accumulates Mul weights before printing leaves.
/// Matches C++ `print_closure(out, closure, ss, accumulated_weight, first, ...)`.
fn print_closure_inner(node: &ClosureNode, w: Vec3, out: &mut String, first: &mut bool) {
    match node {
        ClosureNode::Component {
            id, weight, params, ..
        } => {
            // Combine accumulated weight with component's own weight
            let eff = Vec3::new(w.x * weight.x, w.y * weight.y, w.z * weight.z);
            let name = closure_id_to_name(*id).unwrap_or("unknown");
            let params_str = fmt_closure_params(params);

            if !*first {
                out.push_str("\n\t+ ");
            }
            // params_str already includes outer parens ("()" or "(0, 1, 0)");
            // emit without wrapping again.
            out.push_str(&format!(
                "({}, {}, {}) * {} {}",
                eff.x, eff.y, eff.z, name, params_str
            ));
            *first = false;
        }
        ClosureNode::Mul {
            weight: mw,
            closure,
        } => {
            // Accumulate weight and recurse
            let new_w = Vec3::new(w.x * mw.x, w.y * mw.y, w.z * mw.z);
            print_closure_inner(closure, new_w, out, first);
        }
        ClosureNode::Add { a, b } => {
            print_closure_inner(a, w, out, first);
            print_closure_inner(b, w, out, first);
        }
    }
}

// ---------------------------------------------------------------------------
// Closure ID registry
// ---------------------------------------------------------------------------

/// Closure component IDs matching C++ testrender/shading.h enum.
/// These must stay in sync with C++ for cross-runtime compatibility.
pub mod closure_ids {
    // Base IDs (matching C++ testrender enum order)
    pub const EMISSION_ID: i32 = 1;
    pub const BACKGROUND_ID: i32 = 2;
    pub const DIFFUSE_ID: i32 = 3;
    pub const OREN_NAYAR_ID: i32 = 4;
    pub const TRANSLUCENT_ID: i32 = 5;
    pub const PHONG_ID: i32 = 6;
    pub const WARD_ID: i32 = 7;
    pub const MICROFACET_ID: i32 = 8;
    pub const REFLECTION_ID: i32 = 9;
    pub const FRESNEL_REFLECTION_ID: i32 = 10;
    pub const REFRACTION_ID: i32 = 11;
    pub const TRANSPARENT_ID: i32 = 12;
    pub const DEBUG_ID: i32 = 13;
    pub const HOLDOUT_ID: i32 = 14;
    // MaterialX closure IDs (MX_ prefix in C++)
    pub const OREN_NAYAR_DIFFUSE_BSDF_ID: i32 = 15;
    pub const BURLEY_DIFFUSE_BSDF_ID: i32 = 16;
    pub const DIELECTRIC_BSDF_ID: i32 = 17;
    pub const CONDUCTOR_BSDF_ID: i32 = 18;
    pub const GENERALIZED_SCHLICK_BSDF_ID: i32 = 19;
    pub const TRANSLUCENT_BSDF_ID: i32 = 20;
    pub const TRANSPARENT_BSDF_ID: i32 = 21;
    pub const SUBSURFACE_BSSRDF_ID: i32 = 22;
    pub const SHEEN_BSDF_ID: i32 = 23;
    pub const UNIFORM_EDF_ID: i32 = 24;
    pub const ANISOTROPIC_VDF_ID: i32 = 25;
    pub const MEDIUM_VDF_ID: i32 = 26;
    pub const LAYER_ID: i32 = 27;
    // SPI closures
    pub const SPI_THINLAYER_ID: i32 = 28;
    pub const CHIANG_HAIR_BSDF_ID: i32 = 29;
    pub const EMPTY_ID: i32 = 30;
    // Alias: C++ has no standalone subsurface, uses MX_SUBSURFACE_ID
    pub const SUBSURFACE_ID: i32 = SUBSURFACE_BSSRDF_ID;
}

fn closure_id_to_name(id: i32) -> Option<&'static str> {
    match id {
        closure_ids::EMISSION_ID => Some("emission"),
        closure_ids::BACKGROUND_ID => Some("background"),
        closure_ids::DIFFUSE_ID => Some("diffuse"),
        closure_ids::OREN_NAYAR_ID => Some("oren_nayar"),
        closure_ids::TRANSLUCENT_ID => Some("translucent"),
        closure_ids::PHONG_ID => Some("phong"),
        closure_ids::WARD_ID => Some("ward"),
        closure_ids::MICROFACET_ID => Some("microfacet"),
        closure_ids::REFLECTION_ID => Some("reflection"),
        closure_ids::FRESNEL_REFLECTION_ID => Some("fresnel_reflection"),
        closure_ids::REFRACTION_ID => Some("refraction"),
        closure_ids::TRANSPARENT_ID => Some("transparent"),
        closure_ids::DEBUG_ID => Some("debug"),
        closure_ids::HOLDOUT_ID => Some("holdout"),
        closure_ids::OREN_NAYAR_DIFFUSE_BSDF_ID => Some("oren_nayar_diffuse_bsdf"),
        closure_ids::BURLEY_DIFFUSE_BSDF_ID => Some("burley_diffuse_bsdf"),
        closure_ids::DIELECTRIC_BSDF_ID => Some("dielectric_bsdf"),
        closure_ids::CONDUCTOR_BSDF_ID => Some("conductor_bsdf"),
        closure_ids::GENERALIZED_SCHLICK_BSDF_ID => Some("generalized_schlick_bsdf"),
        closure_ids::TRANSLUCENT_BSDF_ID => Some("translucent_bsdf"),
        closure_ids::TRANSPARENT_BSDF_ID => Some("transparent_bsdf"),
        closure_ids::SUBSURFACE_BSSRDF_ID => Some("subsurface"),
        closure_ids::SHEEN_BSDF_ID => Some("sheen_bsdf"),
        closure_ids::UNIFORM_EDF_ID => Some("uniform_edf"),
        closure_ids::ANISOTROPIC_VDF_ID => Some("anisotropic_vdf"),
        closure_ids::MEDIUM_VDF_ID => Some("medium_vdf"),
        closure_ids::LAYER_ID => Some("layer"),
        closure_ids::SPI_THINLAYER_ID => Some("spi_thinlayer"),
        closure_ids::CHIANG_HAIR_BSDF_ID => Some("chiang_hair_bsdf"),
        _ => None,
    }
}

pub fn closure_name_to_id(name: &str) -> Option<i32> {
    match name {
        "emission" => Some(closure_ids::EMISSION_ID),
        "background" => Some(closure_ids::BACKGROUND_ID),
        "diffuse" => Some(closure_ids::DIFFUSE_ID),
        "oren_nayar" => Some(closure_ids::OREN_NAYAR_ID),
        "translucent" => Some(closure_ids::TRANSLUCENT_ID),
        "phong" => Some(closure_ids::PHONG_ID),
        "ward" => Some(closure_ids::WARD_ID),
        "microfacet" => Some(closure_ids::MICROFACET_ID),
        "reflection" => Some(closure_ids::REFLECTION_ID),
        "fresnel_reflection" => Some(closure_ids::FRESNEL_REFLECTION_ID),
        "refraction" => Some(closure_ids::REFRACTION_ID),
        "transparent" => Some(closure_ids::TRANSPARENT_ID),
        "debug" => Some(closure_ids::DEBUG_ID),
        "holdout" => Some(closure_ids::HOLDOUT_ID),
        "oren_nayar_diffuse_bsdf" => Some(closure_ids::OREN_NAYAR_DIFFUSE_BSDF_ID),
        "burley_diffuse_bsdf" => Some(closure_ids::BURLEY_DIFFUSE_BSDF_ID),
        "dielectric_bsdf" => Some(closure_ids::DIELECTRIC_BSDF_ID),
        "conductor_bsdf" => Some(closure_ids::CONDUCTOR_BSDF_ID),
        "generalized_schlick_bsdf" => Some(closure_ids::GENERALIZED_SCHLICK_BSDF_ID),
        "translucent_bsdf" => Some(closure_ids::TRANSLUCENT_BSDF_ID),
        "transparent_bsdf" => Some(closure_ids::TRANSPARENT_BSDF_ID),
        "subsurface" | "subsurface_bssrdf" => Some(closure_ids::SUBSURFACE_BSSRDF_ID),
        "sheen_bsdf" => Some(closure_ids::SHEEN_BSDF_ID),
        "uniform_edf" => Some(closure_ids::UNIFORM_EDF_ID),
        "anisotropic_vdf" => Some(closure_ids::ANISOTROPIC_VDF_ID),
        "medium_vdf" => Some(closure_ids::MEDIUM_VDF_ID),
        "layer" => Some(closure_ids::LAYER_ID),
        "spi_thinlayer" => Some(closure_ids::SPI_THINLAYER_ID),
        "chiang_hair_bsdf" => Some(closure_ids::CHIANG_HAIR_BSDF_ID),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ClosureRegistry -- dynamic closure ID management
// ---------------------------------------------------------------------------

/// Named parameter descriptor for a closure primitive.
///
/// Stores the human-readable name and type for each positional or keyword
/// argument. Replaces the hard-coded per-closure match arms in interp.rs
/// and lets the interpreter validate argument counts generically.
#[derive(Debug, Clone)]
pub struct ClosureParamDesc {
    /// Parameter name (e.g. "N", "roughness", "eta").
    pub name: &'static str,
    /// OSL type of the parameter.
    pub type_desc: TypeDesc,
    /// Whether this is a keyword argument (name=value style).
    pub is_keyword: bool,
}

impl ClosureParamDesc {
    pub const fn positional(name: &'static str, type_desc: TypeDesc) -> Self {
        Self {
            name,
            type_desc,
            is_keyword: false,
        }
    }
    pub const fn keyword(name: &'static str, type_desc: TypeDesc) -> Self {
        Self {
            name,
            type_desc,
            is_keyword: true,
        }
    }
}

/// Called before executing a closure component to prepare its data.
/// Matches C++ `PrepareClosureFn` in `oslclosure.h`.
pub type PrepareClosureFn = fn(id: i32, data: *mut std::ffi::c_void);

/// Called after executing a closure component to finalize its data.
/// Matches C++ `SetupClosureFn` in `oslclosure.h`.
pub type SetupClosureFn = fn(id: i32, data: *mut std::ffi::c_void);

/// Entry for a registered closure in the registry.
#[derive(Clone)]
pub struct ClosureEntry {
    pub name: String,
    pub id: i32,
    /// Low-level C-layout param descriptors (offset/key, for FFI).
    pub params: Vec<ClosureParam>,
    /// High-level named param descriptors (for the interpreter).
    pub param_descs: Vec<ClosureParamDesc>,
    pub labels: ClosureLabels,
    /// Optional prepare callback — called before executing the closure.
    /// Matches C++ `ClosureRegistry::ClosureEntry::prepare`.
    pub prepare: Option<PrepareClosureFn>,
    /// Optional setup callback — called after executing the closure.
    /// Matches C++ `ClosureRegistry::ClosureEntry::setup`.
    pub setup: Option<SetupClosureFn>,
}

impl std::fmt::Debug for ClosureEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClosureEntry")
            .field("name", &self.name)
            .field("id", &self.id)
            .field("labels", &self.labels)
            .field("prepare", &self.prepare.map(|_| "<fn>"))
            .field("setup", &self.setup.map(|_| "<fn>"))
            .finish()
    }
}

/// Dynamic closure registry. Maps closure names to IDs and vice versa.
///
/// Pre-registers all standard OSL closures (diffuse, emission, etc.).
/// Renderers can register additional custom closures at runtime.
#[derive(Debug, Clone)]
pub struct ClosureRegistry {
    name_to_id: HashMap<String, i32>,
    id_to_entry: HashMap<i32, ClosureEntry>,
    next_id: i32,
}

impl ClosureRegistry {
    /// Create an empty registry (no standard closures).
    pub fn empty() -> Self {
        Self {
            name_to_id: HashMap::new(),
            id_to_entry: HashMap::new(),
            next_id: 100, // custom IDs start at 100
        }
    }

    /// Register a closure with explicit params and labels. Returns assigned ID.
    pub fn register_closure(
        &mut self,
        name: &str,
        params: Vec<ClosureParam>,
        labels: ClosureLabels,
    ) -> i32 {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        let entry = ClosureEntry {
            name: name.to_string(),
            id,
            params,
            param_descs: Vec::new(),
            labels,
            prepare: None,
            setup: None,
        };
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_entry.insert(id, entry);
        id
    }

    /// Register a closure with explicit params, labels, and optional callbacks. Returns assigned ID.
    pub fn register_closure_with_callbacks(
        &mut self,
        name: &str,
        params: Vec<ClosureParam>,
        labels: ClosureLabels,
        prepare: Option<PrepareClosureFn>,
        setup: Option<SetupClosureFn>,
    ) -> i32 {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        let entry = ClosureEntry {
            name: name.to_string(),
            id,
            params,
            param_descs: Vec::new(),
            labels,
            prepare,
            setup,
        };
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_entry.insert(id, entry);
        id
    }

    /// Register a closure with a specific ID and optional parameter descriptors.
    fn register_std(
        &mut self,
        name: &str,
        id: i32,
        labels: ClosureLabels,
        param_descs: Vec<ClosureParamDesc>,
    ) {
        let entry = ClosureEntry {
            name: name.to_string(),
            id,
            params: Vec::new(),
            param_descs,
            labels,
            prepare: None,
            setup: None,
        };
        self.name_to_id.insert(name.to_string(), id);
        self.id_to_entry.insert(id, entry);
    }

    /// Get the full closure entry by name.
    pub fn get_entry_by_name(&self, name: &str) -> Option<&ClosureEntry> {
        self.name_to_id
            .get(name)
            .and_then(|id| self.id_to_entry.get(id))
    }

    /// Named param descriptors for a closure by name. Empty slice for unknown.
    pub fn param_descs_for(&self, name: &str) -> &[ClosureParamDesc] {
        self.name_to_id
            .get(name)
            .and_then(|id| self.id_to_entry.get(id))
            .map(|e| e.param_descs.as_slice())
            .unwrap_or(&[])
    }

    /// Expected positional arg count for a named closure. None if not registered.
    pub fn positional_param_count(&self, name: &str) -> Option<usize> {
        let entry = self.get_entry_by_name(name)?;
        Some(entry.param_descs.iter().filter(|p| !p.is_keyword).count())
    }

    /// Look up a closure ID by name.
    pub fn lookup_id(&self, name: &str) -> Option<i32> {
        self.name_to_id.get(name).copied()
    }

    /// Look up a closure name by ID.
    pub fn lookup_name(&self, id: i32) -> Option<&str> {
        self.id_to_entry.get(&id).map(|e| e.name.as_str())
    }

    /// Get the full closure entry by ID.
    pub fn get_entry(&self, id: i32) -> Option<&ClosureEntry> {
        self.id_to_entry.get(&id)
    }

    /// Get the labels for a closure ID.
    pub fn labels_for(&self, id: i32) -> ClosureLabels {
        self.id_to_entry
            .get(&id)
            .map(|e| e.labels)
            .unwrap_or(ClosureLabels::NONE)
    }

    /// Number of registered closures.
    pub fn len(&self) -> usize {
        self.name_to_id.len()
    }

    /// Returns true if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.name_to_id.is_empty()
    }
}

impl Default for ClosureRegistry {
    /// Create a registry pre-populated with all standard OSL closures,
    /// including named parameter descriptors for each.
    fn default() -> Self {
        use crate::typedesc::TypeDesc as T;
        let mut reg = Self::empty();
        let p = ClosureParamDesc::positional;

        reg.register_std(
            "diffuse",
            closure_ids::DIFFUSE_ID,
            ClosureLabels::DIFFUSE_REFLECT,
            vec![p("N", T::NORMAL)],
        );

        reg.register_std(
            "oren_nayar",
            closure_ids::OREN_NAYAR_ID,
            ClosureLabels::DIFFUSE_REFLECT,
            vec![p("N", T::NORMAL), p("sigma", T::FLOAT)],
        );

        reg.register_std(
            "translucent",
            closure_ids::TRANSLUCENT_ID,
            ClosureLabels::DIFFUSE_TRANSMIT,
            vec![p("N", T::NORMAL)],
        );

        reg.register_std(
            "phong",
            closure_ids::PHONG_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![p("N", T::NORMAL), p("exponent", T::FLOAT)],
        );

        reg.register_std(
            "ward",
            closure_ids::WARD_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("T", T::VECTOR),
                p("ax", T::FLOAT),
                p("ay", T::FLOAT),
            ],
        );

        reg.register_std(
            "microfacet",
            closure_ids::MICROFACET_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("distribution", T::STRING),
                p("N", T::NORMAL),
                p("alpha_x", T::FLOAT),
                p("alpha_y", T::FLOAT),
                p("eta", T::FLOAT),
                p("refract", T::INT),
            ],
        );

        reg.register_std(
            "reflection",
            closure_ids::REFLECTION_ID,
            ClosureLabels::SINGULAR_REFLECT,
            vec![p("N", T::NORMAL), p("eta", T::FLOAT)],
        );

        reg.register_std(
            "fresnel_reflection",
            closure_ids::FRESNEL_REFLECTION_ID,
            ClosureLabels::SINGULAR_REFLECT,
            vec![p("N", T::NORMAL)],
        );

        reg.register_std(
            "refraction",
            closure_ids::REFRACTION_ID,
            ClosureLabels::SINGULAR_TRANSMIT,
            vec![p("N", T::NORMAL), p("eta", T::FLOAT)],
        );

        reg.register_std(
            "transparent",
            closure_ids::TRANSPARENT_ID,
            ClosureLabels::STRAIGHT_TRANSMIT,
            vec![],
        );
        reg.register_std(
            "emission",
            closure_ids::EMISSION_ID,
            ClosureLabels::NONE,
            vec![],
        );
        reg.register_std(
            "background",
            closure_ids::BACKGROUND_ID,
            ClosureLabels::NONE,
            vec![],
        );
        reg.register_std(
            "holdout",
            closure_ids::HOLDOUT_ID,
            ClosureLabels::NONE,
            vec![],
        );
        reg.register_std("debug", closure_ids::DEBUG_ID, ClosureLabels::NONE, vec![]);

        reg.register_std(
            "subsurface",
            closure_ids::SUBSURFACE_ID,
            ClosureLabels::DIFFUSE_TRANSMIT,
            vec![p("N", T::NORMAL), p("scale", T::FLOAT)],
        );

        reg.register_std(
            "oren_nayar_diffuse_bsdf",
            closure_ids::OREN_NAYAR_DIFFUSE_BSDF_ID,
            ClosureLabels::DIFFUSE_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("roughness", T::FLOAT),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "burley_diffuse_bsdf",
            closure_ids::BURLEY_DIFFUSE_BSDF_ID,
            ClosureLabels::DIFFUSE_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("roughness", T::FLOAT),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "dielectric_bsdf",
            closure_ids::DIELECTRIC_BSDF_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("roughness", T::FLOAT),
                p("ior", T::FLOAT),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "conductor_bsdf",
            closure_ids::CONDUCTOR_BSDF_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("roughness", T::FLOAT),
                p("eta", T::COLOR),
                p("k", T::COLOR),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "generalized_schlick_bsdf",
            closure_ids::GENERALIZED_SCHLICK_BSDF_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("roughness", T::FLOAT),
                p("f0", T::COLOR),
                p("f90", T::COLOR),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "translucent_bsdf",
            closure_ids::TRANSLUCENT_BSDF_ID,
            ClosureLabels::DIFFUSE_TRANSMIT,
            vec![p("N", T::NORMAL), p("label", T::STRING)],
        );

        reg.register_std(
            "transparent_bsdf",
            closure_ids::TRANSPARENT_BSDF_ID,
            ClosureLabels::STRAIGHT_TRANSMIT,
            vec![],
        );

        reg.register_std(
            "subsurface_bssrdf",
            closure_ids::SUBSURFACE_BSSRDF_ID,
            ClosureLabels::DIFFUSE_TRANSMIT,
            vec![
                p("N", T::NORMAL),
                p("radius", T::COLOR),
                p("albedo", T::COLOR),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "sheen_bsdf",
            closure_ids::SHEEN_BSDF_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("N", T::NORMAL),
                p("roughness", T::FLOAT),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "uniform_edf",
            closure_ids::UNIFORM_EDF_ID,
            ClosureLabels::NONE,
            vec![p("label", T::STRING)],
        );

        reg.register_std(
            "anisotropic_vdf",
            closure_ids::ANISOTROPIC_VDF_ID,
            ClosureLabels::NONE,
            vec![
                p("albedo", T::COLOR),
                p("anisotropy", T::FLOAT),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "medium_vdf",
            closure_ids::MEDIUM_VDF_ID,
            ClosureLabels::NONE,
            vec![
                p("albedo", T::COLOR),
                p("transmission_depth", T::FLOAT),
                p("transmission_color", T::COLOR),
                p("label", T::STRING),
            ],
        );

        reg.register_std(
            "layer",
            closure_ids::LAYER_ID,
            ClosureLabels::NONE,
            vec![p("top", T::UNKNOWN), p("base", T::UNKNOWN)],
        );

        reg.register_std(
            "chiang_hair_bsdf",
            closure_ids::CHIANG_HAIR_BSDF_ID,
            ClosureLabels::GLOSSY_REFLECT,
            vec![
                p("T", T::VECTOR),
                p("melanin", T::FLOAT),
                p("melanin_redness", T::FLOAT),
                p("tint", T::COLOR),
                p("roughness", T::FLOAT),
                p("ior", T::FLOAT),
                p("offset", T::FLOAT),
            ],
        );

        reg
    }
}
// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_component() {
        let c = allocate_closure_component(closure_ids::DIFFUSE_ID);
        assert!(c.is_component());
        assert_eq!(c.component_id(), Some(closure_ids::DIFFUSE_ID));
        assert_eq!(c.component_weight(), Some(Vec3::new(1.0, 1.0, 1.0)));
        assert_eq!(c.component_labels(), Some(ClosureLabels::DIFFUSE_REFLECT));
    }

    #[test]
    fn test_allocate_component_labels() {
        let c = allocate_closure_component(closure_ids::REFRACTION_ID);
        assert_eq!(c.component_labels(), Some(ClosureLabels::SINGULAR_TRANSMIT));

        let c2 = allocate_closure_component(closure_ids::TRANSLUCENT_ID);
        assert_eq!(c2.component_labels(), Some(ClosureLabels::DIFFUSE_TRANSMIT));
    }

    #[test]
    fn test_weighted_component_zero() {
        let c = allocate_weighted_closure_component(1, Vec3::new(0.0, 0.0, 0.0));
        assert!(c.is_none());
    }

    #[test]
    fn test_add_mul() {
        let c1 = Some(allocate_closure_component(closure_ids::DIFFUSE_ID));
        let c2 = Some(allocate_closure_component(closure_ids::EMISSION_ID));
        let m = mul_closure_color(c1, Vec3::new(0.5, 0.5, 0.5));
        let a = add_closure_closure(m, c2);
        assert!(a.is_some());
        assert!(a.as_ref().unwrap().is_add());
    }

    #[test]
    fn test_add_none_optimization() {
        let c = Some(allocate_closure_component(1));
        let result = add_closure_closure(None, c.clone());
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_component());
    }

    #[test]
    fn test_mul_identity() {
        let c = Some(allocate_closure_component(1));
        let result = mul_closure_color(c, Vec3::new(1.0, 1.0, 1.0));
        assert!(result.is_some());
        assert!(result.as_ref().unwrap().is_component());
    }

    #[test]
    fn test_mul_zero() {
        let c = Some(allocate_closure_component(1));
        let result = mul_closure_color(c, Vec3::new(0.0, 0.0, 0.0));
        assert!(result.is_none());
    }

    #[test]
    fn test_flatten_closure() {
        let c1 = allocate_closure_component(closure_ids::DIFFUSE_ID);
        let c2 = allocate_weighted_closure_component(
            closure_ids::MICROFACET_ID,
            Vec3::new(0.5, 0.5, 0.5),
        )
        .unwrap();
        let mul = mul_closure_color(Some(c1), Vec3::new(0.3, 0.3, 0.3)).unwrap();
        let add = add_closure_closure(Some(mul), Some(c2)).unwrap();

        let flat = flatten_closure(&add);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].id, closure_ids::DIFFUSE_ID);
        assert!((flat[0].weight.x - 0.3).abs() < 1e-6);
        assert_eq!(flat[0].labels, ClosureLabels::DIFFUSE_REFLECT);
        assert_eq!(flat[1].id, closure_ids::MICROFACET_ID);
        assert!((flat[1].weight.x - 0.5).abs() < 1e-6);
        assert_eq!(flat[1].labels, ClosureLabels::GLOSSY_REFLECT);
    }

    #[test]
    fn test_closure_name_to_id() {
        assert_eq!(closure_name_to_id("diffuse"), Some(closure_ids::DIFFUSE_ID));
        assert_eq!(
            closure_name_to_id("emission"),
            Some(closure_ids::EMISSION_ID)
        );
        assert_eq!(closure_name_to_id("unknown"), None);
    }

    #[test]
    fn test_closure_to_string_simple() {
        // Unit-weight component: "(1, 1, 1) * diffuse ()"
        let c = allocate_closure_component(closure_ids::DIFFUSE_ID);
        let s = closure_to_string(&c);
        assert_eq!(s, "(1, 1, 1) * diffuse ()");

        let c2 = allocate_closure_component(closure_ids::EMISSION_ID);
        let s2 = closure_to_string(&c2);
        assert_eq!(s2, "(1, 1, 1) * emission ()");
    }

    #[test]
    fn test_closure_to_string_with_params() {
        use crate::closure::ClosureParams;
        let n = Vec3::new(0.0, 1.0, 0.0);
        let c = allocate_closure_component_with_params(
            closure_ids::DIFFUSE_ID,
            ClosureLabels::DIFFUSE_REFLECT,
            ClosureParams::Normal(n),
        );
        let s = closure_to_string(&c);
        // C++ format: "(w.x, w.y, w.z) * diffuse (n.x, n.y, n.z)"
        assert_eq!(s, "(1, 1, 1) * diffuse (0, 1, 0)");
    }

    #[test]
    fn test_closure_to_string_mul_propagates() {
        // Mul weight should be multiplied into component weight in output
        let c = allocate_closure_component(closure_ids::DIFFUSE_ID);
        let m = mul_closure_color(Some(c), Vec3::new(0.5, 0.5, 0.5)).unwrap();
        let s = closure_to_string(&m);
        // accumulated weight: (0.5*1, 0.5*1, 0.5*1)
        assert_eq!(s, "(0.5, 0.5, 0.5) * diffuse ()");
    }

    #[test]
    fn test_closure_to_string_add_separator() {
        // Add should separate leaves with "\n\t+ "
        let c1 = allocate_closure_component(closure_ids::DIFFUSE_ID);
        let c2 = allocate_closure_component(closure_ids::EMISSION_ID);
        let add = add_closure_closure(Some(c1), Some(c2)).unwrap();
        let s = closure_to_string(&add);
        assert!(
            s.contains("\n\t+ "),
            "Add separator must be '\\n\\t+ ', got: {s:?}"
        );
        assert!(s.contains("diffuse"));
        assert!(s.contains("emission"));
    }

    #[test]
    fn test_closure_to_string_nested_params() {
        use crate::closure::ClosureParams;
        let c = allocate_closure_component_with_params(
            closure_ids::OREN_NAYAR_ID,
            ClosureLabels::DIFFUSE_REFLECT,
            ClosureParams::NormalRoughness {
                n: Vec3::new(0.0, 1.0, 0.0),
                roughness: 0.3,
            },
        );
        let s = closure_to_string(&c);
        assert!(s.contains("oren_nayar"));
        assert!(s.contains("0.3"));
    }

    #[test]
    fn test_default_labels() {
        assert_eq!(
            default_labels_for_closure(closure_ids::DIFFUSE_ID),
            ClosureLabels::DIFFUSE_REFLECT
        );
        assert_eq!(
            default_labels_for_closure(closure_ids::REFLECTION_ID),
            ClosureLabels::SINGULAR_REFLECT
        );
        assert_eq!(
            default_labels_for_closure(closure_ids::REFRACTION_ID),
            ClosureLabels::SINGULAR_TRANSMIT
        );
        assert_eq!(
            default_labels_for_closure(closure_ids::TRANSPARENT_ID),
            ClosureLabels::STRAIGHT_TRANSMIT
        );
        assert_eq!(default_labels_for_closure(999), ClosureLabels::NONE);
    }

    // -- ClosureRegistry tests --

    #[test]
    fn test_registry_default_has_std_closures() {
        let reg = ClosureRegistry::default();
        // 15 std (incl. fresnel_reflection, subsurface) + 14 MaterialX = 29
        assert_eq!(reg.len(), 29);
        assert_eq!(reg.lookup_id("diffuse"), Some(closure_ids::DIFFUSE_ID));
        assert_eq!(reg.lookup_id("emission"), Some(closure_ids::EMISSION_ID));
        assert_eq!(reg.lookup_id("debug"), Some(closure_ids::DEBUG_ID));
        assert_eq!(
            reg.lookup_id("subsurface"),
            Some(closure_ids::SUBSURFACE_ID)
        );
        assert_eq!(reg.lookup_name(closure_ids::DIFFUSE_ID), Some("diffuse"));
        assert_eq!(
            reg.lookup_name(closure_ids::REFRACTION_ID),
            Some("refraction")
        );
    }

    #[test]
    fn test_registry_lookup_unknown() {
        let reg = ClosureRegistry::default();
        assert_eq!(reg.lookup_id("nonexistent"), None);
        assert_eq!(reg.lookup_name(999), None);
    }

    #[test]
    fn test_registry_register_custom() {
        let mut reg = ClosureRegistry::default();
        let id = reg.register_closure("my_bsdf", Vec::new(), ClosureLabels::GLOSSY_REFLECT);
        assert!(id >= 100); // custom IDs start at 100
        assert_eq!(reg.lookup_id("my_bsdf"), Some(id));
        assert_eq!(reg.lookup_name(id), Some("my_bsdf"));
        assert_eq!(reg.labels_for(id), ClosureLabels::GLOSSY_REFLECT);
        assert_eq!(reg.len(), 30); // 29 std + 1 custom
    }

    #[test]
    fn test_registry_register_duplicate_returns_existing() {
        let mut reg = ClosureRegistry::default();
        let id1 = reg.register_closure("custom", Vec::new(), ClosureLabels::NONE);
        let id2 = reg.register_closure("custom", Vec::new(), ClosureLabels::NONE);
        assert_eq!(id1, id2);
        assert_eq!(reg.len(), 30); // 29 std + 1 custom, no duplicate
    }

    #[test]
    fn test_registry_labels_for_std() {
        let reg = ClosureRegistry::default();
        assert_eq!(
            reg.labels_for(closure_ids::DIFFUSE_ID),
            ClosureLabels::DIFFUSE_REFLECT
        );
        assert_eq!(
            reg.labels_for(closure_ids::REFRACTION_ID),
            ClosureLabels::SINGULAR_TRANSMIT
        );
        assert_eq!(
            reg.labels_for(closure_ids::EMISSION_ID),
            ClosureLabels::NONE
        );
    }

    #[test]
    fn test_registry_get_entry() {
        let reg = ClosureRegistry::default();
        let entry = reg.get_entry(closure_ids::MICROFACET_ID).unwrap();
        assert_eq!(entry.name, "microfacet");
        assert_eq!(entry.id, closure_ids::MICROFACET_ID);
        assert_eq!(entry.labels, ClosureLabels::GLOSSY_REFLECT);
    }

    #[test]
    fn test_registry_empty() {
        let reg = ClosureRegistry::empty();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert_eq!(reg.lookup_id("diffuse"), None);
    }

    // -- MaterialX closure tests --

    #[test]
    fn test_materialx_closure_ids() {
        // Verify all MaterialX closures have IDs and names
        let pairs = [
            ("subsurface", closure_ids::SUBSURFACE_BSSRDF_ID),
            (
                "oren_nayar_diffuse_bsdf",
                closure_ids::OREN_NAYAR_DIFFUSE_BSDF_ID,
            ),
            ("burley_diffuse_bsdf", closure_ids::BURLEY_DIFFUSE_BSDF_ID),
            ("dielectric_bsdf", closure_ids::DIELECTRIC_BSDF_ID),
            ("conductor_bsdf", closure_ids::CONDUCTOR_BSDF_ID),
            (
                "generalized_schlick_bsdf",
                closure_ids::GENERALIZED_SCHLICK_BSDF_ID,
            ),
            ("translucent_bsdf", closure_ids::TRANSLUCENT_BSDF_ID),
            ("transparent_bsdf", closure_ids::TRANSPARENT_BSDF_ID),
            ("sheen_bsdf", closure_ids::SHEEN_BSDF_ID),
            ("uniform_edf", closure_ids::UNIFORM_EDF_ID),
            ("anisotropic_vdf", closure_ids::ANISOTROPIC_VDF_ID),
            ("medium_vdf", closure_ids::MEDIUM_VDF_ID),
            ("layer", closure_ids::LAYER_ID),
            ("chiang_hair_bsdf", closure_ids::CHIANG_HAIR_BSDF_ID),
        ];
        for (name, id) in &pairs {
            assert_eq!(
                closure_name_to_id(name),
                Some(*id),
                "name_to_id failed for {name}"
            );
            assert_eq!(
                closure_id_to_name(*id),
                Some(*name),
                "id_to_name failed for {name}"
            );
        }
        // "subsurface_bssrdf" is an alias that maps to the same ID
        assert_eq!(
            closure_name_to_id("subsurface_bssrdf"),
            Some(closure_ids::SUBSURFACE_BSSRDF_ID)
        );
    }

    #[test]
    fn test_materialx_labels() {
        let reg = ClosureRegistry::default();
        // Diffuse BSDFs -> DIFFUSE_REFLECT
        assert_eq!(
            reg.labels_for(closure_ids::OREN_NAYAR_DIFFUSE_BSDF_ID),
            ClosureLabels::DIFFUSE_REFLECT
        );
        assert_eq!(
            reg.labels_for(closure_ids::BURLEY_DIFFUSE_BSDF_ID),
            ClosureLabels::DIFFUSE_REFLECT
        );
        // Glossy BSDFs
        assert_eq!(
            reg.labels_for(closure_ids::DIELECTRIC_BSDF_ID),
            ClosureLabels::GLOSSY_REFLECT
        );
        assert_eq!(
            reg.labels_for(closure_ids::CONDUCTOR_BSDF_ID),
            ClosureLabels::GLOSSY_REFLECT
        );
        assert_eq!(
            reg.labels_for(closure_ids::SHEEN_BSDF_ID),
            ClosureLabels::GLOSSY_REFLECT
        );
        assert_eq!(
            reg.labels_for(closure_ids::CHIANG_HAIR_BSDF_ID),
            ClosureLabels::GLOSSY_REFLECT
        );
        // Transmit
        assert_eq!(
            reg.labels_for(closure_ids::TRANSLUCENT_BSDF_ID),
            ClosureLabels::DIFFUSE_TRANSMIT
        );
        assert_eq!(
            reg.labels_for(closure_ids::TRANSPARENT_BSDF_ID),
            ClosureLabels::STRAIGHT_TRANSMIT
        );
        assert_eq!(
            reg.labels_for(closure_ids::SUBSURFACE_ID),
            ClosureLabels::DIFFUSE_TRANSMIT
        );
        assert_eq!(
            reg.labels_for(closure_ids::SUBSURFACE_BSSRDF_ID),
            ClosureLabels::DIFFUSE_TRANSMIT
        );
        // VDF/EDF -> NONE (volume/emissive, no scattering direction)
        assert_eq!(
            reg.labels_for(closure_ids::UNIFORM_EDF_ID),
            ClosureLabels::NONE
        );
        assert_eq!(
            reg.labels_for(closure_ids::ANISOTROPIC_VDF_ID),
            ClosureLabels::NONE
        );
        assert_eq!(
            reg.labels_for(closure_ids::MEDIUM_VDF_ID),
            ClosureLabels::NONE
        );
        assert_eq!(reg.labels_for(closure_ids::LAYER_ID), ClosureLabels::NONE);
    }

    #[test]
    fn test_materialx_registry_lookup() {
        let reg = ClosureRegistry::default();
        assert_eq!(
            reg.lookup_id("dielectric_bsdf"),
            Some(closure_ids::DIELECTRIC_BSDF_ID)
        );
        assert_eq!(
            reg.lookup_name(closure_ids::DIELECTRIC_BSDF_ID),
            Some("dielectric_bsdf")
        );
        assert_eq!(
            reg.lookup_id("chiang_hair_bsdf"),
            Some(closure_ids::CHIANG_HAIR_BSDF_ID)
        );
        assert_eq!(
            reg.lookup_name(closure_ids::CHIANG_HAIR_BSDF_ID),
            Some("chiang_hair_bsdf")
        );
    }

    #[test]
    fn test_registry_param_descs() {
        let reg = ClosureRegistry::default();

        // diffuse(N) -- 1 positional
        let descs = reg.param_descs_for("diffuse");
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].name, "N");
        assert!(!descs[0].is_keyword);
        assert_eq!(reg.positional_param_count("diffuse"), Some(1));

        // phong(N, exponent) -- 2 positional
        assert_eq!(reg.positional_param_count("phong"), Some(2));
        assert_eq!(reg.param_descs_for("phong")[1].name, "exponent");

        // microfacet(distribution, N, alpha_x, alpha_y, eta, refract) -- 6
        assert_eq!(reg.positional_param_count("microfacet"), Some(6));

        // emission() -- 0 params, but entry is registered
        assert_eq!(reg.positional_param_count("emission"), Some(0));

        // unknown -- None
        assert_eq!(reg.positional_param_count("nonexistent_bsdf"), None);
    }

    #[test]
    fn test_registry_get_entry_by_name() {
        let reg = ClosureRegistry::default();
        let entry = reg.get_entry_by_name("ward").unwrap();
        assert_eq!(entry.id, closure_ids::WARD_ID);
        // ward(N, T_tangent, ax, ay) -- 4 params
        assert_eq!(entry.param_descs.len(), 4);
        assert_eq!(entry.param_descs[0].name, "N");
        assert_eq!(entry.param_descs[2].name, "ax");

        assert!(reg.get_entry_by_name("not_a_real_closure").is_none());
    }
}
