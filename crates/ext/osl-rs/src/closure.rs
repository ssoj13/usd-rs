//! Closure color tree types — 100% safe Rust.
//!
//! # Why an enum instead of C-style tagged union?
//!
//! The original C++ OSL uses a `ClosureColor` base class with an `id` field
//! to discriminate between `ClosureComponent`, `ClosureMul`, and `ClosureAdd`.
//! Downcasting is done via raw pointer reinterpretation (`reinterpret_cast`),
//! and the tree is arena-allocated with manual lifetime management.
//!
//! That approach requires `unsafe` at every node access: raw pointer casts,
//! manual `Send`/`Sync` impls, and an arena allocator with untyped byte slabs.
//! In our Rust port this was the #1 source of `unsafe` code (over 30 instances).
//!
//! A Rust `enum` gives us the same discriminated-union semantics for free:
//! - **Zero unsafe** — pattern matching replaces pointer casts
//! - **Automatic Send + Sync** — no manual trait impls needed
//! - **No arena needed** — `Arc` reference counting handles lifetimes
//! - **Exhaustive matching** — compiler guarantees we handle all variants
//!
//! Binary compatibility for the `.oso` format is unaffected because closures
//! are a *runtime* construct — they are never serialized. The C API layer
//! (`capi.rs`) handles any FFI bridging separately.

use std::sync::Arc;

use crate::math::Vec3;
use crate::typedesc::TypeDesc;

// ---------------------------------------------------------------------------
// Closure label types — LPE classification for scattering events
// ---------------------------------------------------------------------------

/// Scattering type classification for a closure component.
///
/// Matches the C++ `Labels::DIFFUSE`, `Labels::GLOSSY`, `Labels::SINGULAR`,
/// `Labels::STRAIGHT` constants used by the LPE system to categorize
/// what kind of light scattering a closure represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ScatteringKind {
    /// No scattering classification (emission, holdout, debug).
    #[default]
    None,
    /// Lambertian / Oren-Nayar style wide scattering.
    Diffuse,
    /// Phong / Ward / microfacet style concentrated scattering.
    Glossy,
    /// Perfect mirror / glass (delta distribution).
    Singular,
    /// Transparent pass-through (no actual scattering).
    Straight,
}

/// Scattering direction classification for a closure component.
///
/// Matches C++ `Labels::REFLECT` and `Labels::TRANSMIT`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DirectionKind {
    /// No direction (emission, holdout, debug).
    #[default]
    None,
    /// Reflection (same side as incident ray).
    Reflect,
    /// Transmission (opposite side from incident ray).
    Transmit,
}

/// LPE labels attached to a closure component.
///
/// Every `ClosureNode::Component` carries these labels so the LPE automaton
/// (see [`crate::lpe`]) can classify each surface bounce in the light path.
///
/// Matches the C++ `ClosureComponent::labels` array (typically labels[0] =
/// scattering type, labels[1] = direction).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ClosureLabels {
    /// Primary scattering type (Diffuse, Glossy, Singular, Straight).
    pub scattering: ScatteringKind,
    /// Direction (Reflect, Transmit).
    pub direction: DirectionKind,
}

impl ClosureLabels {
    /// No labels — used for emission, holdout, debug closures.
    pub const NONE: Self = Self {
        scattering: ScatteringKind::None,
        direction: DirectionKind::None,
    };

    pub const DIFFUSE_REFLECT: Self = Self {
        scattering: ScatteringKind::Diffuse,
        direction: DirectionKind::Reflect,
    };

    pub const DIFFUSE_TRANSMIT: Self = Self {
        scattering: ScatteringKind::Diffuse,
        direction: DirectionKind::Transmit,
    };

    pub const GLOSSY_REFLECT: Self = Self {
        scattering: ScatteringKind::Glossy,
        direction: DirectionKind::Reflect,
    };

    pub const GLOSSY_TRANSMIT: Self = Self {
        scattering: ScatteringKind::Glossy,
        direction: DirectionKind::Transmit,
    };

    pub const SINGULAR_REFLECT: Self = Self {
        scattering: ScatteringKind::Singular,
        direction: DirectionKind::Reflect,
    };

    pub const SINGULAR_TRANSMIT: Self = Self {
        scattering: ScatteringKind::Singular,
        direction: DirectionKind::Transmit,
    };

    pub const STRAIGHT_TRANSMIT: Self = Self {
        scattering: ScatteringKind::Straight,
        direction: DirectionKind::Transmit,
    };
}

// ---------------------------------------------------------------------------
// ClosureNode — the safe closure tree
// ---------------------------------------------------------------------------

/// Reference-counted closure node.
pub type ClosureRef = Arc<ClosureNode>;

/// Closure-specific parameter data stored inline with the component.
///
/// In C++ OSL, ClosureComponent allocates inline storage after the struct
/// via `osl_allocate_closure_component(id, size)` and parameters (roughness,
/// IOR, normal, etc.) are written into that memory. In Rust we use a typed
/// enum instead of raw bytes for safety.
///
/// See C++ `oslclosure.h` ClosureComponent::data().
#[derive(Debug, Clone, Default)]
pub enum ClosureParams {
    /// No parameters (e.g. transparent, background closures).
    #[default]
    None,
    /// Single normal vector (e.g. diffuse(N), translucent(N)).
    Normal(Vec3),
    /// Normal + roughness (e.g. oren_nayar(N, roughness)).
    NormalRoughness { n: Vec3, roughness: f32 },
    /// Normal + IOR (e.g. refraction(N, eta)).
    NormalIor { n: Vec3, ior: f32 },
    /// Normal + roughness + IOR (e.g. dielectric_bsdf, microfacet).
    NormalRoughnessIor { n: Vec3, roughness: f32, ior: f32 },
    /// Normal + color + roughness (e.g. conductor_bsdf, sheen_bsdf).
    NormalColorRoughness {
        n: Vec3,
        color: Vec3,
        roughness: f32,
    },
    /// Hair BSDF params (chiang_hair_bsdf).
    Hair {
        tangent: Vec3,
        color: Vec3,
        melanin: f32,
        roughness: f32,
        ior: f32,
        offset: f32,
    },
    /// Generic param storage for custom or complex closures.
    Generic(Vec<u8>),
}

/// A node in the closure color tree.
///
/// OSL closures form a tree where:
/// - **Leaves** are `Component` nodes — weighted BSDF primitives (diffuse,
///   reflection, microfacet, etc.) identified by a registered `id`.
/// - **Internal nodes** are either `Mul` (scale a sub-tree by a color weight)
///   or `Add` (combine two sub-trees).
///
/// Each `Component` also carries [`ClosureLabels`] for LPE classification.
#[derive(Debug, Clone)]
pub enum ClosureNode {
    /// A weighted closure component (e.g. diffuse, reflection).
    Component {
        /// Component ID (matches registered closure IDs, >= 0).
        id: i32,
        /// Color weight of this component.
        weight: Vec3,
        /// LPE labels: scattering type + direction.
        labels: ClosureLabels,
        /// BSDF-specific parameters (normal, roughness, IOR, etc.).
        /// C++ stores these inline after ClosureComponent via data() pointer.
        params: ClosureParams,
    },

    /// Multiply a closure sub-tree by a color weight.
    Mul {
        /// The color weight to apply.
        weight: Vec3,
        /// The child closure sub-tree.
        closure: ClosureRef,
    },

    /// Add two closure sub-trees.
    Add {
        /// First child sub-tree.
        a: ClosureRef,
        /// Second child sub-tree.
        b: ClosureRef,
    },
}

impl ClosureNode {
    #[inline]
    pub fn is_component(&self) -> bool {
        matches!(self, ClosureNode::Component { .. })
    }

    #[inline]
    pub fn is_mul(&self) -> bool {
        matches!(self, ClosureNode::Mul { .. })
    }

    #[inline]
    pub fn is_add(&self) -> bool {
        matches!(self, ClosureNode::Add { .. })
    }

    #[inline]
    pub fn component_id(&self) -> Option<i32> {
        match self {
            ClosureNode::Component { id, .. } => Some(*id),
            _ => None,
        }
    }

    #[inline]
    pub fn component_weight(&self) -> Option<Vec3> {
        match self {
            ClosureNode::Component { weight, .. } => Some(*weight),
            _ => None,
        }
    }

    #[inline]
    pub fn component_labels(&self) -> Option<ClosureLabels> {
        match self {
            ClosureNode::Component { labels, .. } => Some(*labels),
            _ => None,
        }
    }

    /// Get the closure params if this is a Component node.
    #[inline]
    pub fn component_params(&self) -> Option<&ClosureParams> {
        match self {
            ClosureNode::Component { params, .. } => Some(params),
            _ => None,
        }
    }
}

/// Legacy type alias.
pub type ClosureColor = ClosureNode;

// ---------------------------------------------------------------------------
// ClosureParam — descriptor for closure parameter registration
// ---------------------------------------------------------------------------

/// Describes a single parameter of a closure primitive.
///
/// Not repr(C) -- closures are runtime-only, FFI bridging is in capi.rs.
#[derive(Debug, Clone, Copy)]
pub struct ClosureParam {
    pub type_desc: TypeDesc,
    pub offset: i32,
    pub key: Option<&'static str>,
    pub field_size: i32,
}

impl ClosureParam {
    pub const fn positional(type_desc: TypeDesc, offset: i32, field_size: i32) -> Self {
        Self {
            type_desc,
            offset,
            key: None,
            field_size,
        }
    }

    pub const fn keyword(
        type_desc: TypeDesc,
        offset: i32,
        key: &'static str,
        field_size: i32,
    ) -> Self {
        Self {
            type_desc,
            offset,
            key: Some(key),
            field_size,
        }
    }

    pub const fn finish(struct_size: i32, struct_align: i32) -> Self {
        Self {
            type_desc: TypeDesc::UNKNOWN,
            offset: struct_size,
            key: None,
            field_size: struct_align,
        }
    }
}

// ---------------------------------------------------------------------------
// Labels — well-known closure label strings (C++ compat)
// ---------------------------------------------------------------------------

/// Well-known closure label strings for light path expressions (LPE).
///
/// These match the C++ `Labels::` string constants exactly.
pub struct Labels;

impl Labels {
    pub const NONE: &'static str = "";
    pub const CAMERA: &'static str = "C";
    pub const LIGHT: &'static str = "L";
    pub const BACKGROUND: &'static str = "B";
    pub const VOLUME: &'static str = "V";
    pub const OBJECT: &'static str = "O";
    pub const TRANSMIT: &'static str = "T";
    pub const REFLECT: &'static str = "R";
    pub const DIFFUSE: &'static str = "D";
    pub const GLOSSY: &'static str = "G";
    pub const SINGULAR: &'static str = "S";
    pub const STRAIGHT: &'static str = "s";
    pub const STOP: &'static str = "__stop__";
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closure_node_variants() {
        let comp = ClosureNode::Component {
            id: 1,
            weight: Vec3::new(0.8, 0.8, 0.8),
            labels: ClosureLabels::DIFFUSE_REFLECT,
            params: ClosureParams::None,
        };
        assert!(comp.is_component());
        assert!(!comp.is_mul());
        assert!(!comp.is_add());
        assert_eq!(comp.component_id(), Some(1));
        assert_eq!(
            comp.component_labels(),
            Some(ClosureLabels::DIFFUSE_REFLECT)
        );

        let mul = ClosureNode::Mul {
            weight: Vec3::new(0.5, 0.5, 0.5),
            closure: Arc::new(comp.clone()),
        };
        assert!(mul.is_mul());
        assert!(!mul.is_component());

        let comp2 = ClosureNode::Component {
            id: 2,
            weight: Vec3::new(1.0, 1.0, 1.0),
            labels: ClosureLabels::GLOSSY_REFLECT,
            params: ClosureParams::None,
        };
        let add = ClosureNode::Add {
            a: Arc::new(mul),
            b: Arc::new(comp2),
        };
        assert!(add.is_add());
    }

    #[test]
    fn test_closure_param_finish() {
        let finish = ClosureParam::finish(32, 16);
        assert_eq!(finish.type_desc, TypeDesc::UNKNOWN);
        assert_eq!(finish.offset, 32);
        assert_eq!(finish.field_size, 16);
    }

    #[test]
    fn test_closure_ref_clone() {
        let node = Arc::new(ClosureNode::Component {
            id: 5,
            weight: Vec3::new(1.0, 0.0, 0.0),
            labels: ClosureLabels::SINGULAR_REFLECT,
            params: ClosureParams::None,
        });
        let clone = node.clone();
        assert_eq!(Arc::strong_count(&node), 2);
        assert!(clone.is_component());
        assert_eq!(clone.component_id(), Some(5));
    }

    #[test]
    fn test_closure_labels_constants() {
        assert_eq!(ClosureLabels::NONE.scattering, ScatteringKind::None);
        assert_eq!(
            ClosureLabels::DIFFUSE_REFLECT.scattering,
            ScatteringKind::Diffuse
        );
        assert_eq!(
            ClosureLabels::DIFFUSE_REFLECT.direction,
            DirectionKind::Reflect
        );
        assert_eq!(
            ClosureLabels::SINGULAR_TRANSMIT.scattering,
            ScatteringKind::Singular
        );
        assert_eq!(
            ClosureLabels::SINGULAR_TRANSMIT.direction,
            DirectionKind::Transmit
        );
    }
}
