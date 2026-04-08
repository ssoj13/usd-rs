//! HdSceneDelegate types and structs.
//!
//! Corresponds to pxr/imaging/hd/sceneDelegate.h - data structures used by
//! the HdSceneDelegate interface for synchronizing and querying scene data.

use crate::enums::HdInterpolation;
use crate::types::{HdDirtyBits, HdTupleType};
use std::sync::Arc;
use usd_gf::{Vec3f, Vec3i};
use usd_sdf::{Path as SdfPath, asset_path::AssetPath as SdfAssetPath};
use usd_tf::Token;

// Re-export HdFormat from types.rs where it canonically lives (matches C++ hd/types.h)
pub use crate::types::HdFormat;

// -----------------------------------------------------------------------//
// HdSyncRequestVector
// -----------------------------------------------------------------------//

/// Request vector for delegate synchronization.
///
/// The SceneDelegate is requested to synchronize prims as the result of
/// executing a specific render pass.
///
/// Corresponds to C++ `HdSyncRequestVector`.
#[derive(Debug, Clone, Default)]
pub struct HdSyncRequestVector {
    /// Prims to synchronize in this request.
    pub ids: Vec<SdfPath>,
    /// Dirty bits set for each prim.
    pub dirty_bits: Vec<HdDirtyBits>,
}

// -----------------------------------------------------------------------//
// HdDisplayStyle
// -----------------------------------------------------------------------//

/// Describes how the geometry of a prim should be displayed.
///
/// Corresponds to C++ `HdDisplayStyle`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdDisplayStyle {
    /// Refine level [0, 8] for subdivision.
    pub refine_level: i32,
    /// Flat shading enabled.
    pub flat_shading_enabled: bool,
    /// Displacement enabled.
    pub displacement_enabled: bool,
    /// Display in overlay.
    pub display_in_overlay: bool,
    /// Occluded selection shows through.
    pub occluded_selection_shows_through: bool,
    /// Points shading as surfaces.
    pub points_shading_enabled: bool,
    /// Material is final (exempt from override).
    pub material_is_final: bool,
}

impl Default for HdDisplayStyle {
    fn default() -> Self {
        Self {
            refine_level: 0,
            flat_shading_enabled: false,
            displacement_enabled: true,
            display_in_overlay: false,
            occluded_selection_shows_through: false,
            points_shading_enabled: false,
            material_is_final: false,
        }
    }
}

impl HdDisplayStyle {
    /// Create with parameters.
    pub fn new(
        refine_level: i32,
        flat_shading: bool,
        displacement: bool,
        display_in_overlay: bool,
        occluded_selection_shows_through: bool,
        points_shading_enabled: bool,
        material_is_final: bool,
    ) -> Self {
        Self {
            // C++: clamp to [0, +inf), warn but do not clamp values > 8.
            refine_level: refine_level.max(0),
            flat_shading_enabled: flat_shading,
            displacement_enabled: displacement,
            display_in_overlay,
            occluded_selection_shows_through,
            points_shading_enabled,
            material_is_final,
        }
    }
}

// -----------------------------------------------------------------------//
// HdPrimvarDescriptor
// -----------------------------------------------------------------------//

/// Describes a primvar.
///
/// Corresponds to C++ `HdPrimvarDescriptor`.
/// NOTE: PartialEq is implemented manually to exclude `indexed`, matching C++
/// where HdPrimvarDescriptor::operator== compares only name, interpolation, role.
#[derive(Debug, Clone, Eq)]
pub struct HdPrimvarDescriptor {
    /// Name of the primvar.
    pub name: Token,
    /// Interpolation (data-sampling rate).
    pub interpolation: HdInterpolation,
    /// Optional role (color, vector, point, normal).
    pub role: Token,
    /// True if primvar is indexed.
    pub indexed: bool,
}

impl Default for HdPrimvarDescriptor {
    fn default() -> Self {
        Self {
            name: Token::default(),
            interpolation: HdInterpolation::Constant,
            role: Token::default(),
            indexed: false,
        }
    }
}

impl HdPrimvarDescriptor {
    /// Create with name, interpolation, role, indexed.
    pub fn new(name: Token, interpolation: HdInterpolation, role: Token, indexed: bool) -> Self {
        Self {
            name,
            interpolation,
            role,
            indexed,
        }
    }

    /// Create with name and interpolation only.
    pub fn with_name_and_interp(name: Token, interpolation: HdInterpolation) -> Self {
        Self {
            name,
            interpolation,
            role: Token::default(),
            indexed: false,
        }
    }
}

impl PartialEq for HdPrimvarDescriptor {
    /// Matches C++ `HdPrimvarDescriptor::operator==`: compares name, interpolation, role.
    /// The `indexed` field is deliberately excluded, matching C++ behavior.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.interpolation == other.interpolation
            && self.role == other.role
    }
}

impl std::hash::Hash for HdPrimvarDescriptor {
    /// Hash consistent with PartialEq: excludes `indexed`.
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Token has an inherent hash() that conflicts; use std::hash::Hash explicitly.
        std::hash::Hash::hash(self.name.as_str(), state);
        self.interpolation.hash(state);
        std::hash::Hash::hash(self.role.as_str(), state);
    }
}

/// Vector of primvar descriptors.
pub type HdPrimvarDescriptorVector = Vec<HdPrimvarDescriptor>;

// -----------------------------------------------------------------------//
// HdModelDrawMode tokens (C++ HdModelDrawModeTokens)
// -----------------------------------------------------------------------//

use once_cell::sync::Lazy;
static HD_MODEL_DRAW_MODE_INHERITED: Lazy<Token> = Lazy::new(|| Token::new("inherited"));
static HD_MODEL_DRAW_MODE_CROSS: Lazy<Token> = Lazy::new(|| Token::new("cross"));

// -----------------------------------------------------------------------//
// HdModelDrawMode
// -----------------------------------------------------------------------//

/// Describes optional alternative imaging behavior for prims.
///
/// Corresponds to C++ `HdModelDrawMode`.
#[derive(Debug, Clone, PartialEq)]
pub struct HdModelDrawMode {
    /// Draw mode: origin, bounds, cards, default, inherited.
    pub draw_mode: Token,
    /// Whether to apply the alternative imaging mode.
    pub apply_draw_mode: bool,
    /// Color for draw mode geometry.
    pub draw_mode_color: Vec3f,
    /// Card geometry: cross, box, fromTexture.
    pub card_geometry: Token,
    /// Card textures for each face.
    /// Card texture for +X face.
    pub card_texture_x_pos: SdfAssetPath,
    /// Card texture for +Y face.
    pub card_texture_y_pos: SdfAssetPath,
    /// Card texture for +Z face.
    pub card_texture_z_pos: SdfAssetPath,
    /// Card texture for -X face.
    pub card_texture_x_neg: SdfAssetPath,
    /// Card texture for -Y face.
    pub card_texture_y_neg: SdfAssetPath,
    /// Card texture for -Z face.
    pub card_texture_z_neg: SdfAssetPath,
}

impl Default for HdModelDrawMode {
    fn default() -> Self {
        Self {
            draw_mode: (*HD_MODEL_DRAW_MODE_INHERITED).clone(),
            apply_draw_mode: false,
            draw_mode_color: Vec3f::new(0.18, 0.18, 0.18),
            card_geometry: (*HD_MODEL_DRAW_MODE_CROSS).clone(),
            card_texture_x_pos: SdfAssetPath::default(),
            card_texture_y_pos: SdfAssetPath::default(),
            card_texture_z_pos: SdfAssetPath::default(),
            card_texture_x_neg: SdfAssetPath::default(),
            card_texture_y_neg: SdfAssetPath::default(),
            card_texture_z_neg: SdfAssetPath::default(),
        }
    }
}

// -----------------------------------------------------------------------//
// HdExtComputationPrimvarDescriptor
// -----------------------------------------------------------------------//

/// Extends HdPrimvarDescriptor for primvars from ExtComputation output.
///
/// Corresponds to C++ `HdExtComputationPrimvarDescriptor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdExtComputationPrimvarDescriptor {
    /// Primvar name.
    pub name: Token,
    /// Interpolation (data-sampling rate).
    pub interpolation: HdInterpolation,
    /// Optional role (color, vector, point, normal).
    pub role: Token,
    /// Source computation id in render index.
    pub source_computation_id: SdfPath,
    /// Output name from that computation.
    pub source_computation_output_name: Token,
    /// Value type of the output.
    pub value_type: HdTupleType,
}

impl Default for HdExtComputationPrimvarDescriptor {
    fn default() -> Self {
        Self {
            name: Token::default(),
            interpolation: HdInterpolation::Constant,
            role: Token::default(),
            source_computation_id: SdfPath::default(),
            source_computation_output_name: Token::default(),
            value_type: HdTupleType::default(),
        }
    }
}

impl HdExtComputationPrimvarDescriptor {
    /// Create with all fields.
    pub fn new(
        name: Token,
        interpolation: HdInterpolation,
        role: Token,
        source_computation_id: SdfPath,
        source_computation_output_name: Token,
        value_type: HdTupleType,
    ) -> Self {
        Self {
            name,
            interpolation,
            role,
            source_computation_id,
            source_computation_output_name,
            value_type,
        }
    }
}

/// Vector of ext computation primvar descriptors.
pub type HdExtComputationPrimvarDescriptorVector = Vec<HdExtComputationPrimvarDescriptor>;

// -----------------------------------------------------------------------//
// HdExtComputationInputDescriptor
// -----------------------------------------------------------------------//

/// Describes an input to an ExtComputation from another ExtComputation output.
///
/// Corresponds to C++ `HdExtComputationInputDescriptor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdExtComputationInputDescriptor {
    /// Input name.
    pub name: Token,
    /// Source computation prim path.
    pub source_computation_id: SdfPath,
    /// Output name from the source computation.
    pub source_computation_output_name: Token,
}

impl Default for HdExtComputationInputDescriptor {
    fn default() -> Self {
        Self {
            name: Token::default(),
            source_computation_id: SdfPath::default(),
            source_computation_output_name: Token::default(),
        }
    }
}

/// Vector of ext computation input descriptors.
pub type HdExtComputationInputDescriptorVector = Vec<HdExtComputationInputDescriptor>;

// -----------------------------------------------------------------------//
// HdExtComputationOutputDescriptor
// -----------------------------------------------------------------------//

/// Describes an output of an ExtComputation.
///
/// Corresponds to C++ `HdExtComputationOutputDescriptor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdExtComputationOutputDescriptor {
    /// Output name.
    pub name: Token,
    /// Value type of the output.
    pub value_type: HdTupleType,
}

impl Default for HdExtComputationOutputDescriptor {
    fn default() -> Self {
        Self {
            name: Token::default(),
            value_type: HdTupleType::default(),
        }
    }
}

/// Vector of ext computation output descriptors.
pub type HdExtComputationOutputDescriptorVector = Vec<HdExtComputationOutputDescriptor>;

// -----------------------------------------------------------------------//
// HdVolumeFieldDescriptor
// -----------------------------------------------------------------------//

/// Description of a single field related to a volume primitive.
///
/// Corresponds to C++ `HdVolumeFieldDescriptor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdVolumeFieldDescriptor {
    /// Field name (e.g. "density", "temperature").
    pub field_name: Token,
    /// Field prim type (e.g. OpenVDBAsset).
    pub field_prim_type: Token,
    /// Path to the field prim in the render index.
    pub field_id: SdfPath,
}

impl Default for HdVolumeFieldDescriptor {
    fn default() -> Self {
        Self {
            field_name: Token::default(),
            field_prim_type: Token::default(),
            field_id: SdfPath::default(),
        }
    }
}

impl HdVolumeFieldDescriptor {
    /// Create with all fields.
    pub fn new(field_name: Token, field_prim_type: Token, field_id: SdfPath) -> Self {
        Self {
            field_name,
            field_prim_type,
            field_id,
        }
    }
}

/// Vector of volume field descriptors.
pub type HdVolumeFieldDescriptorVector = Vec<HdVolumeFieldDescriptor>;

// -----------------------------------------------------------------------//
// HdInstancerContext
// -----------------------------------------------------------------------//

/// Instancer context: (instancer path, instance index) pairs.
///
/// Corresponds to C++ `HdInstancerContext` = std::vector<std::pair<SdfPath, int>>.
pub type HdInstancerContext = Vec<(SdfPath, i32)>;

// -----------------------------------------------------------------------//
// HdIdVectorSharedPtr
// -----------------------------------------------------------------------//

/// Shared pointer to a vector of paths (coordinate system bindings).
///
/// Corresponds to C++ `HdIdVectorSharedPtr` = std::shared_ptr<SdfPathVector>.
pub type HdIdVectorSharedPtr = Arc<Vec<SdfPath>>;

// -----------------------------------------------------------------------//
// HdRenderBufferDescriptor
// -----------------------------------------------------------------------//

/// Describes the allocation structure of a render buffer bprim.
///
/// Corresponds to C++ `HdRenderBufferDescriptor` in pxr/imaging/hd/aov.h.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HdRenderBufferDescriptor {
    /// Width, height, depth of the buffer.
    pub dimensions: Vec3i,
    /// Data format.
    pub format: HdFormat,
    /// Whether multi-sampled.
    pub multi_sampled: bool,
}

impl Default for HdRenderBufferDescriptor {
    fn default() -> Self {
        Self {
            dimensions: Vec3i::new(0, 0, 0),
            format: HdFormat::Invalid,
            multi_sampled: false,
        }
    }
}

impl HdRenderBufferDescriptor {
    /// Create a new render buffer descriptor.
    pub fn new(dimensions: Vec3i, format: HdFormat, multi_sampled: bool) -> Self {
        Self {
            dimensions,
            format,
            multi_sampled,
        }
    }
}
