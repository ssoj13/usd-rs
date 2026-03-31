
//! HdDirtyBitsTranslator - Translate dirty bits to/from data source locators.
//!
//! Full port of pxr/imaging/hd/dirtyBitsTranslator.h/cpp
//! Implements type-specific mappings for all built-in rprim, sprim, and bprim types.

use super::data_source::{HdDataSourceLocator, HdDataSourceLocatorSet};
use crate::change_tracker::{HdRprimDirtyBits, HdTaskDirtyBits};
use crate::prim::camera::HdCameraDirtyBits;
use crate::prim::coord_sys::HdCoordSysDirtyBits;
use crate::prim::ext_computation::HdExtComputationDirtyBits;
use crate::schema::{
    HdBasisCurvesTopologySchema, HdCameraSchema, HdCapsuleSchema, HdCategoriesSchema,
    HdCollectionsSchema, HdConeSchema, HdCoordSysSchema, HdCubeSchema, HdCylinderSchema,
    HdDisplayFilterSchema, HdExtComputationPrimvarsSchema, HdExtComputationSchema, HdExtentSchema,
    HdInstanceCategoriesSchema, HdInstancedBySchema, HdInstancerTopologySchema, HdIntegratorSchema,
    HdLegacyDisplayStyleSchema, HdLegacyTaskSchema, HdLightSchema, HdMaterialBindingsSchema,
    HdMaterialSchema, HdMeshSchema, HdMeshTopologySchema, HdPrimvarsSchema, HdPurposeSchema,
    HdRenderBufferSchema, HdRenderSettingsSchema, HdSampleFilterSchema, HdSphereSchema,
    HdSubdivisionTagsSchema, HdVisibilitySchema, HdVolumeFieldBindingSchema, HdVolumeFieldSchema,
    HdXformSchema,
};
use crate::tokens::hd_prim_type_is_light;
use crate::tokens::{
    BPRIM_RENDER_BUFFER, LIGHT_FILTER, RPRIM_BASIS_CURVES, RPRIM_CAPSULE, RPRIM_CONE, RPRIM_CUBE,
    RPRIM_CYLINDER, RPRIM_MESH, RPRIM_SPHERE, SPRIM_CAMERA, SPRIM_COORD_SYS, SPRIM_DISPLAY_FILTER,
    SPRIM_DRAW_TARGET, SPRIM_EXT_COMPUTATION, SPRIM_IMAGE_SHADER, SPRIM_INTEGRATOR, SPRIM_MATERIAL,
    SPRIM_SAMPLE_FILTER,
};
use once_cell::sync::Lazy;

static BPRIM_RENDER_SETTINGS: Lazy<Token> = Lazy::new(|| Token::new("renderSettings"));
use std::collections::HashMap;
use std::sync::Mutex;
use usd_tf::Token;

// Re-export the canonical HdDirtyBits type (u32, matching types.rs).
pub use crate::types::HdDirtyBits;

/// Function type: locator set -> dirty bits (output param).
pub type LocatorSetToDirtyBitsFn =
    Box<dyn Fn(&HdDataSourceLocatorSet, &mut HdDirtyBits) + Send + Sync>;

/// Function type: dirty bits -> locator set (output param).
pub type DirtyBitsToLocatorSetFn =
    Box<dyn Fn(HdDirtyBits, &mut HdDataSourceLocatorSet) + Send + Sync>;

/// Dirty bits that mean "everything dirty" (same as HdRprimDirtyBits::ALL_DIRTY).
pub const HD_DIRTY_ALL: HdDirtyBits = !0u32;

/// Clean (no dirty bits).
pub const HD_CLEAN: HdDirtyBits = 0;

// ---- Material dirty bits (from C++ HdMaterial::DirtyBits) ------------------
// These are sprim-specific bits that differ from HdRprimDirtyBits.
pub mod hd_material_dirty_bits {
    use super::HdDirtyBits;
    pub const DIRTY_PARAMS: HdDirtyBits = 1 << 2;
    pub const DIRTY_RESOURCE: HdDirtyBits = 1 << 3;
    pub const DIRTY_SURFACE: HdDirtyBits = 1 << 4;
    pub const DIRTY_DISPLACEMENT: HdDirtyBits = 1 << 5;
    pub const DIRTY_VOLUME: HdDirtyBits = 1 << 6;
    pub const ALL_DIRTY: HdDirtyBits =
        DIRTY_PARAMS | DIRTY_RESOURCE | DIRTY_SURFACE | DIRTY_DISPLACEMENT | DIRTY_VOLUME;
}

// ---- Light dirty bits (from C++ HdLight::DirtyBits) ------------------------
pub mod hd_light_dirty_bits {
    use super::HdDirtyBits;
    pub const DIRTY_TRANSFORM: HdDirtyBits = 1 << 0;
    pub const DIRTY_PARAMS: HdDirtyBits = 1 << 1;
    pub const DIRTY_SHADOW_PARAMS: HdDirtyBits = 1 << 2;
    pub const DIRTY_COLLECTION: HdDirtyBits = 1 << 3;
    pub const DIRTY_RESOURCE: HdDirtyBits = 1 << 4;
    pub const DIRTY_INSTANCER: HdDirtyBits = 1 << 16;
    pub const ALL_DIRTY: HdDirtyBits = DIRTY_TRANSFORM
        | DIRTY_PARAMS
        | DIRTY_SHADOW_PARAMS
        | DIRTY_COLLECTION
        | DIRTY_RESOURCE
        | DIRTY_INSTANCER;
}

// ---- RenderBuffer dirty bits (from C++ HdRenderBuffer::DirtyBits) ----------
pub mod hd_render_buffer_dirty_bits {
    use super::HdDirtyBits;
    pub const DIRTY_DESCRIPTION: HdDirtyBits = 1 << 0;
    pub const ALL_DIRTY: HdDirtyBits = DIRTY_DESCRIPTION;
}

// ---- RenderSettings dirty bits (from C++ HdRenderSettings::DirtyBits) ------
pub mod hd_render_settings_dirty_bits {
    use super::HdDirtyBits;
    pub const DIRTY_ACTIVE: HdDirtyBits = 1 << 1;
    pub const DIRTY_NAMESPACED_SETTINGS: HdDirtyBits = 1 << 2;
    pub const DIRTY_RENDER_PRODUCTS: HdDirtyBits = 1 << 3;
    pub const DIRTY_INCLUDED_PURPOSES: HdDirtyBits = 1 << 4;
    pub const DIRTY_MATERIAL_BINDING_PURPOSES: HdDirtyBits = 1 << 5;
    pub const DIRTY_RENDERING_COLOR_SPACE: HdDirtyBits = 1 << 6;
    pub const DIRTY_SHUTTER_INTERVAL: HdDirtyBits = 1 << 7;
    pub const DIRTY_FRAME_NUMBER: HdDirtyBits = 1 << 8;
    pub const ALL_DIRTY: HdDirtyBits = DIRTY_ACTIVE
        | DIRTY_NAMESPACED_SETTINGS
        | DIRTY_RENDER_PRODUCTS
        | DIRTY_INCLUDED_PURPOSES
        | DIRTY_MATERIAL_BINDING_PURPOSES
        | DIRTY_RENDERING_COLOR_SPACE
        | DIRTY_SHUTTER_INTERVAL
        | DIRTY_FRAME_NUMBER;
}

// ---- ImageShader dirty bits (from C++ HdImageShader::DirtyBits) -------------
pub mod hd_image_shader_dirty_bits {
    use super::HdDirtyBits;
    pub const DIRTY_ENABLED: HdDirtyBits = 1 << 0;
    pub const DIRTY_PRIORITY: HdDirtyBits = 1 << 1;
    pub const DIRTY_FILE_PATH: HdDirtyBits = 1 << 2;
    pub const DIRTY_CONSTANTS: HdDirtyBits = 1 << 3;
    pub const DIRTY_MATERIAL_NETWORK: HdDirtyBits = 1 << 4;
    pub const ALL_DIRTY: HdDirtyBits =
        DIRTY_ENABLED | DIRTY_PRIORITY | DIRTY_FILE_PATH | DIRTY_CONSTANTS | DIRTY_MATERIAL_NETWORK;
}

// ---- Task dirty bits (from HdChangeTracker::DirtyCollection etc.) -----------
// These reuse rprim-range bits for tasks; see C++ changeTracker.h.
pub mod hd_task_dirty {
    use super::HdDirtyBits;
    pub const DIRTY_COLLECTION: HdDirtyBits = 1 << 3;
    pub const DIRTY_PARAMS: HdDirtyBits = 1 << 2;
    pub const DIRTY_RENDER_TAGS: HdDirtyBits = 1 << 4;
}

// ---- Custom bits locator (matches C++ _GetCustomBitsLocator()) --------------
static CUSTOM_BITS_TOKEN: Lazy<Token> = Lazy::new(|| Token::new("__customBits"));

fn get_custom_bits_locator() -> HdDataSourceLocator {
    HdDataSourceLocator::from_token(CUSTOM_BITS_TOKEN.clone())
}

fn get_custom_bit_locator(index: usize) -> HdDataSourceLocator {
    get_custom_bits_locator().append(&Token::new(&index.to_string()))
}

// ---- Custom translator registries -------------------------------------------

static CUSTOM_SPRIM_TRANSLATORS: Lazy<
    Mutex<HashMap<Token, (LocatorSetToDirtyBitsFn, DirtyBitsToLocatorSetFn)>>,
> = Lazy::new(|| Mutex::new(HashMap::new()));

static CUSTOM_RPRIM_TRANSLATORS: Lazy<
    Mutex<HashMap<Token, (LocatorSetToDirtyBitsFn, DirtyBitsToLocatorSetFn)>>,
> = Lazy::new(|| Mutex::new(HashMap::new()));

// ---- _FindLocator helper ----------------------------------------------------
// Mimics the C++ static _FindLocator() function.
// Searches the sorted locator slice for any element that intersects `target`.
// If advanceToNext is true, the iterator is left pointing past all intersecting
// elements; if false, it is left pointing at the first intersecting element.
// Returns true if any intersecting element was found.
fn find_locator(
    target: &HdDataSourceLocator,
    locators: &[HdDataSourceLocator],
    pos: &mut usize,
    advance_to_next: bool,
) -> bool {
    // Empty locator in set is the universal locator — always matches.
    if *pos < locators.len() && locators[*pos].is_empty() {
        return true;
    }

    let mut found = false;
    let start = *pos;
    let mut i = start;

    while i < locators.len() {
        let l = &locators[i];
        if l.intersects(target) {
            found = true;
            if advance_to_next {
                i += 1;
                // Keep going while still intersecting.
                while i < locators.len() && locators[i].intersects(target) {
                    i += 1;
                }
                *pos = i;
                break;
            } else {
                *pos = i;
                break;
            }
        } else if target < l {
            // target is before this locator, so none can match from here.
            *pos = i;
            break;
        }
        i += 1;
    }

    if i == locators.len() && !found {
        *pos = locators.len();
    }

    found
}

/// Translate dirty bits between prim types and data source locators.
///
/// Full port of C++ HdDirtyBitsTranslator.
pub struct HdDirtyBitsTranslator;

impl HdDirtyBitsTranslator {
    // =========================================================================
    // RprimDirtyBitsToLocatorSet
    // =========================================================================

    /// Rprim: dirty bits -> locator set.
    ///
    /// Maps HdRprimDirtyBits constants to data source locators for each rprim type.
    /// Locators are added in sorted order to keep the set minimal and fast.
    pub fn rprim_dirty_bits_to_locator_set(
        prim_type: &Token,
        bits: HdDirtyBits,
        set: &mut HdDataSourceLocatorSet,
    ) {
        // Check custom translators first (type-specific override).
        {
            let custom = CUSTOM_RPRIM_TRANSLATORS.lock().unwrap();
            if let Some((_, b_to_s)) = custom.get(prim_type) {
                b_to_s(bits, set);
                return;
            }
        }

        if bits == 0 {
            return;
        }

        // AllDirty: insert root locator which covers everything.
        if bits == HdRprimDirtyBits::ALL_DIRTY {
            set.insert(HdDataSourceLocator::empty());
            return;
        }

        // Locators are inserted in alphabetical order by their string representation
        // so that HdDataSourceLocatorSet::insert() can exploit its sorted invariant.

        if prim_type == &*RPRIM_BASIS_CURVES {
            if (bits & HdRprimDirtyBits::DIRTY_TOPOLOGY) != 0 {
                set.insert(HdBasisCurvesTopologySchema::get_default_locator());
            }
        }

        if prim_type == &*RPRIM_CAPSULE {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdCapsuleSchema::get_default_locator());
            }
        }

        if (bits & HdRprimDirtyBits::DIRTY_CATEGORIES) != 0 {
            set.insert(HdCategoriesSchema::get_default_locator());
        }

        if prim_type == &*RPRIM_CONE {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdConeSchema::get_default_locator());
            }
        }

        if prim_type == &*RPRIM_CUBE {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdCubeSchema::get_default_locator());
            }
        }

        if prim_type == &*RPRIM_CYLINDER {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdCylinderSchema::get_default_locator());
            }
        }

        // displayStyle covers cullStyle and reprSelector as sub-locators.
        if (bits & HdRprimDirtyBits::DIRTY_DISPLAY_STYLE) != 0 {
            set.insert(HdLegacyDisplayStyleSchema::get_default_locator());
        } else {
            if (bits & HdRprimDirtyBits::DIRTY_CULL_STYLE) != 0 {
                set.insert(HdLegacyDisplayStyleSchema::get_cull_style_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_REPR) != 0 {
                set.insert(HdLegacyDisplayStyleSchema::get_repr_selector_locator());
            }
        }

        if (bits & HdRprimDirtyBits::DIRTY_EXTENT) != 0 {
            set.insert(HdExtentSchema::get_default_locator());
        }

        if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
            set.insert(HdExtComputationPrimvarsSchema::get_default_locator());
        }

        if (bits & HdRprimDirtyBits::DIRTY_INSTANCER) != 0 {
            set.insert(HdInstancedBySchema::get_default_locator());
        }

        if (bits & HdRprimDirtyBits::DIRTY_INSTANCE_INDEX) != 0 {
            set.insert(HdInstancerTopologySchema::get_default_locator());
        }

        if (bits & HdRprimDirtyBits::DIRTY_MATERIAL_ID) != 0 {
            set.insert(HdMaterialBindingsSchema::get_default_locator());
        }

        if prim_type == &*RPRIM_MESH {
            if (bits & HdRprimDirtyBits::DIRTY_DOUBLE_SIDED) != 0 {
                set.insert(HdMeshSchema::get_double_sided_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_TOPOLOGY) != 0 {
                set.insert(HdMeshSchema::get_subdivision_scheme_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_SUBDIV_TAGS) != 0 {
                set.insert(HdSubdivisionTagsSchema::get_default_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_TOPOLOGY) != 0 {
                set.insert(HdMeshTopologySchema::get_default_locator());
            }
        }

        // primvars covers normals, points, and widths as sub-locators.
        if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
            set.insert(HdPrimvarsSchema::get_default_locator());
        } else {
            if (bits & HdRprimDirtyBits::DIRTY_NORMALS) != 0 {
                set.insert(HdPrimvarsSchema::get_normals_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_POINTS) != 0 {
                set.insert(HdPrimvarsSchema::get_points_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_WIDTHS) != 0 {
                set.insert(HdPrimvarsSchema::get_widths_locator());
            }
        }

        if (bits & HdRprimDirtyBits::DIRTY_RENDER_TAG) != 0 {
            set.insert(HdPurposeSchema::get_default_locator());
        }

        if prim_type == &*RPRIM_SPHERE {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdSphereSchema::get_default_locator());
            }
        }

        if (bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
            set.insert(HdVisibilitySchema::get_default_locator());
        }

        if (bits & HdRprimDirtyBits::DIRTY_VOLUME_FIELD) != 0 {
            set.insert(HdVolumeFieldBindingSchema::get_default_locator());
        }

        if (bits & HdRprimDirtyBits::DIRTY_TRANSFORM) != 0 {
            set.insert(HdXformSchema::get_default_locator());
        }

        // Custom bits (__customBits/0 .. __customBits/6).
        if (bits & HdRprimDirtyBits::CUSTOM_BITS_MASK) != 0 {
            let mut bit = HdRprimDirtyBits::CUSTOM_BITS_BEGIN;
            let mut i = 0usize;
            while bit <= HdRprimDirtyBits::CUSTOM_BITS_END {
                if (bits & bit) != 0 {
                    set.insert(get_custom_bit_locator(i));
                }
                bit <<= 1;
                i += 1;
            }
        }
    }

    // =========================================================================
    // SprimDirtyBitsToLocatorSet
    // =========================================================================

    /// Sprim: dirty bits -> locator set.
    ///
    /// Handles camera, light, material, coordSys, extComputation, imageShader,
    /// integrator, sampleFilter, displayFilter, and drawTarget.
    pub fn sprim_dirty_bits_to_locator_set(
        prim_type: &Token,
        bits: HdDirtyBits,
        set: &mut HdDataSourceLocatorSet,
    ) {
        if bits == 0 {
            return;
        }

        if prim_type == &*SPRIM_MATERIAL {
            if (bits & hd_material_dirty_bits::ALL_DIRTY) != 0 {
                set.insert(HdMaterialSchema::get_default_locator());
            }
        } else if prim_type == &*SPRIM_COORD_SYS {
            if (bits & HdCoordSysDirtyBits::DIRTY_NAME) != 0 {
                // coordSys/name
                let locator = HdCoordSysSchema::get_default_locator().append(&Token::new("name"));
                set.insert(locator);
            }
            if (bits & HdCoordSysDirtyBits::DIRTY_TRANSFORM) != 0 {
                set.insert(HdXformSchema::get_default_locator());
            }
        } else if prim_type == &*SPRIM_CAMERA {
            if (bits
                & (HdCameraDirtyBits::DIRTY_PARAMS
                    | HdCameraDirtyBits::DIRTY_CLIP_PLANES
                    | HdCameraDirtyBits::DIRTY_WINDOW_POLICY))
                != 0
            {
                set.insert(HdCameraSchema::get_default_locator());
            }
            if (bits & HdCameraDirtyBits::DIRTY_TRANSFORM) != 0 {
                set.insert(HdXformSchema::get_default_locator());
            }
        } else if hd_prim_type_is_light(prim_type)
            || prim_type == &*LIGHT_FILTER
            // Mesh lights use sprim-specific dirty bits even though type is "mesh".
            || prim_type == &*RPRIM_MESH
        {
            if (bits
                & (hd_light_dirty_bits::DIRTY_PARAMS
                    | hd_light_dirty_bits::DIRTY_SHADOW_PARAMS
                    | hd_light_dirty_bits::DIRTY_COLLECTION))
                != 0
            {
                set.insert(HdLightSchema::get_default_locator());
            }
            if (bits & hd_light_dirty_bits::DIRTY_RESOURCE) != 0 {
                set.insert(HdMaterialSchema::get_default_locator());
            }
            if (bits & hd_light_dirty_bits::DIRTY_PARAMS) != 0 {
                // For mesh lights, don't invalidate mesh primvars when light params change.
                if prim_type != &*RPRIM_MESH {
                    set.insert(HdPrimvarsSchema::get_default_locator());
                }
                set.insert(HdVisibilitySchema::get_default_locator());
                // Invalidate light-linking collections manufactured in emulation.
                set.insert(HdCollectionsSchema::get_default_locator());
            }
            if (bits & hd_light_dirty_bits::DIRTY_TRANSFORM) != 0 {
                set.insert(HdXformSchema::get_default_locator());
            }
            if (bits & hd_light_dirty_bits::DIRTY_INSTANCER) != 0 {
                set.insert(HdInstancedBySchema::get_default_locator());
            }
        } else if prim_type == &*SPRIM_DRAW_TARGET {
            // drawTarget: any bits dirty -> mark the drawTarget locator.
            let locator = HdDataSourceLocator::from_token(SPRIM_DRAW_TARGET.clone());
            set.insert(locator);
        } else if prim_type == &*SPRIM_EXT_COMPUTATION {
            if (bits & HdExtComputationDirtyBits::DIRTY_DISPATCH_COUNT) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("extComputation"),
                    Token::new("dispatchCount"),
                ));
            }
            if (bits & HdExtComputationDirtyBits::DIRTY_ELEMENT_COUNT) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("extComputation"),
                    Token::new("elementCount"),
                ));
            }
            if (bits & HdExtComputationDirtyBits::DIRTY_KERNEL) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("extComputation"),
                    Token::new("glslKernel"),
                ));
            }
            if (bits
                & (HdExtComputationDirtyBits::DIRTY_INPUT_DESC
                    | HdExtComputationDirtyBits::DIRTY_SCENE_INPUT))
                != 0
            {
                set.insert(HdExtComputationSchema::get_input_computations_locator());
                set.insert(HdExtComputationSchema::get_input_values_locator());
            }
            if (bits & HdExtComputationDirtyBits::DIRTY_OUTPUT_DESC) != 0 {
                set.insert(HdExtComputationSchema::get_outputs_locator());
            }
        } else if prim_type == &*SPRIM_INTEGRATOR {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                // Integrators use DirtyParams which maps to the same bit as
                // HdChangeTracker::DirtyParams (which equals DIRTY_PRIMVAR in the
                // sprim context — sprims define their own dirty bit ranges).
                set.insert(HdIntegratorSchema::get_default_locator());
            }
        } else if prim_type == &*SPRIM_SAMPLE_FILTER {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdSampleFilterSchema::get_default_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
                set.insert(HdVisibilitySchema::get_default_locator());
            }
        } else if prim_type == &*SPRIM_DISPLAY_FILTER {
            if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
                set.insert(HdDisplayFilterSchema::get_default_locator());
            }
            if (bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
                set.insert(HdVisibilitySchema::get_default_locator());
            }
        } else if prim_type == &*SPRIM_IMAGE_SHADER {
            if (bits & hd_image_shader_dirty_bits::DIRTY_ENABLED) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("imageShader"),
                    Token::new("enabled"),
                ));
            }
            if (bits & hd_image_shader_dirty_bits::DIRTY_PRIORITY) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("imageShader"),
                    Token::new("priority"),
                ));
            }
            if (bits & hd_image_shader_dirty_bits::DIRTY_FILE_PATH) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("imageShader"),
                    Token::new("filePath"),
                ));
            }
            if (bits & hd_image_shader_dirty_bits::DIRTY_CONSTANTS) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("imageShader"),
                    Token::new("constants"),
                ));
            }
            if (bits & hd_image_shader_dirty_bits::DIRTY_MATERIAL_NETWORK) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("imageShader"),
                    Token::new("materialNetwork"),
                ));
            }
        } else {
            // Check custom translators.
            let custom = CUSTOM_SPRIM_TRANSLATORS.lock().unwrap();
            if let Some((_, b_to_s)) = custom.get(prim_type) {
                b_to_s(bits, set);
            } else {
                // Unknown prim type: use root locator (AllDirty equivalent).
                set.insert(HdDataSourceLocator::empty());
            }
        }
    }

    // =========================================================================
    // BprimDirtyBitsToLocatorSet
    // =========================================================================

    /// Bprim: dirty bits -> locator set.
    ///
    /// Handles renderBuffer, renderSettings, and volume field types.
    pub fn bprim_dirty_bits_to_locator_set(
        prim_type: &Token,
        bits: HdDirtyBits,
        set: &mut HdDataSourceLocatorSet,
    ) {
        if bits == 0 {
            return;
        }

        if prim_type == &*BPRIM_RENDER_BUFFER {
            if (bits & hd_render_buffer_dirty_bits::DIRTY_DESCRIPTION) != 0 {
                set.insert(HdRenderBufferSchema::get_default_locator());
            }
        } else if prim_type == &*BPRIM_RENDER_SETTINGS {
            if (bits & hd_render_settings_dirty_bits::DIRTY_ACTIVE) != 0 {
                set.insert(HdRenderSettingsSchema::get_active_locator());
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_FRAME_NUMBER) != 0 {
                set.insert(HdRenderSettingsSchema::get_frame_locator());
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_INCLUDED_PURPOSES) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("includedPurposes"),
                ));
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_MATERIAL_BINDING_PURPOSES) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("materialBindingPurposes"),
                ));
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_NAMESPACED_SETTINGS) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("namespacedSettings"),
                ));
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_RENDER_PRODUCTS) != 0 {
                set.insert(HdRenderSettingsSchema::get_render_products_locator());
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_RENDERING_COLOR_SPACE) != 0 {
                set.insert(HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("renderingColorSpace"),
                ));
            }
            if (bits & hd_render_settings_dirty_bits::DIRTY_SHUTTER_INTERVAL) != 0 {
                set.insert(HdRenderSettingsSchema::get_shutter_interval_locator());
            }
        } else if is_volume_field_type(prim_type) {
            if bits != 0 {
                set.insert(HdVolumeFieldSchema::get_default_locator());
            }
        }
    }

    // =========================================================================
    // InstancerDirtyBitsToLocatorSet
    // =========================================================================

    /// Instancer: dirty bits -> locator set.
    pub fn instancer_dirty_bits_to_locator_set(
        _prim_type: &Token,
        bits: HdDirtyBits,
        set: &mut HdDataSourceLocatorSet,
    ) {
        if bits == 0 {
            return;
        }

        if bits == HdRprimDirtyBits::ALL_DIRTY {
            set.insert(HdDataSourceLocator::empty());
            return;
        }

        if (bits & HdRprimDirtyBits::DIRTY_INSTANCER) != 0 {
            set.insert(HdInstancedBySchema::get_default_locator());
        }
        if (bits & HdRprimDirtyBits::DIRTY_INSTANCE_INDEX) != 0 {
            set.insert(HdInstancerTopologySchema::get_default_locator());
        }
        if (bits & HdRprimDirtyBits::DIRTY_PRIMVAR) != 0 {
            set.insert(HdPrimvarsSchema::get_default_locator());
        }
        if (bits & HdRprimDirtyBits::DIRTY_VISIBILITY) != 0 {
            set.insert(HdVisibilitySchema::get_default_locator());
        }
        if (bits & HdRprimDirtyBits::DIRTY_TRANSFORM) != 0 {
            set.insert(HdXformSchema::get_default_locator());
        }
        if (bits & HdRprimDirtyBits::DIRTY_CATEGORIES) != 0 {
            // Point instancers: invalidate both categories and instanceCategories.
            set.insert(HdInstanceCategoriesSchema::get_default_locator());
            set.insert(HdCategoriesSchema::get_default_locator());
        }
    }

    // =========================================================================
    // TaskDirtyBitsToLocatorSet
    // =========================================================================

    /// Task: dirty bits -> locator set.
    pub fn task_dirty_bits_to_locator_set(bits: HdDirtyBits, set: &mut HdDataSourceLocatorSet) {
        if bits == 0 {
            return;
        }

        if (bits & HdTaskDirtyBits::DIRTY_COLLECTION) != 0 {
            set.insert(HdLegacyTaskSchema::get_collection_locator());
        }
        if (bits & HdTaskDirtyBits::DIRTY_PARAMS) != 0 {
            set.insert(HdLegacyTaskSchema::get_parameters_locator());
        }
        if (bits & HdTaskDirtyBits::DIRTY_RENDER_TAGS) != 0 {
            set.insert(HdLegacyTaskSchema::get_render_tags_locator());
        }
    }

    // =========================================================================
    // RprimLocatorSetToDirtyBits
    // =========================================================================

    /// Rprim: locator set -> dirty bits.
    ///
    /// Searches the sorted locator set in locator-name order for efficiency.
    pub fn rprim_locator_set_to_dirty_bits(
        prim_type: &Token,
        set: &HdDataSourceLocatorSet,
    ) -> HdDirtyBits {
        // Check custom translators first.
        {
            let custom = CUSTOM_RPRIM_TRANSLATORS.lock().unwrap();
            if let Some((s_to_b, _)) = custom.get(prim_type) {
                let mut bits: HdDirtyBits = HD_CLEAN;
                s_to_b(set, &mut bits);
                return bits;
            }
        }

        let locators: Vec<HdDataSourceLocator> = set.iter().cloned().collect();

        if locators.is_empty() {
            return HD_CLEAN;
        }

        // Root (empty) locator means AllDirty.
        if locators[0].is_empty() {
            return HdRprimDirtyBits::ALL_DIRTY;
        }

        let mut bits: HdDirtyBits = HD_CLEAN;
        let mut pos = 0usize;

        // Custom bits (__customBits) — checked first because "__" sorts before
        // all schema names in lexicographic order.
        if find_locator(&get_custom_bits_locator(), &locators, &mut pos, false) {
            if locators[pos] == get_custom_bits_locator() {
                // Entire custom bits locator present: set all custom bits.
                bits |= HdRprimDirtyBits::CUSTOM_BITS_MASK;
                pos += 1;
            } else {
                // Check individual custom bit sub-locators.
                let mut bit = HdRprimDirtyBits::CUSTOM_BITS_BEGIN;
                let mut i = 0usize;
                while bit <= HdRprimDirtyBits::CUSTOM_BITS_END {
                    if find_locator(&get_custom_bit_locator(i), &locators, &mut pos, true) {
                        bits |= bit;
                    }
                    bit <<= 1;
                    i += 1;
                }
            }
        }

        if prim_type == &*RPRIM_BASIS_CURVES {
            if find_locator(
                &HdBasisCurvesTopologySchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_TOPOLOGY;
            }
        }

        if prim_type == &*RPRIM_CAPSULE {
            if find_locator(
                &HdCapsuleSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
        }

        if find_locator(
            &HdCategoriesSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_CATEGORIES;
        }

        if prim_type == &*RPRIM_CONE {
            if find_locator(
                &HdConeSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
        }

        if prim_type == &*RPRIM_CUBE {
            if find_locator(
                &HdCubeSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
        }

        if prim_type == &*RPRIM_CYLINDER {
            if find_locator(
                &HdCylinderSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
        }

        // displayStyle: may be a parent (covers all sub-fields) or child.
        {
            let display_style_loc = HdLegacyDisplayStyleSchema::get_default_locator();
            if find_locator(&display_style_loc, &locators, &mut pos, false) {
                if display_style_loc.has_prefix(&locators[pos]) {
                    // Parent or exact match: set all display style bits.
                    bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE
                        | HdRprimDirtyBits::DIRTY_CULL_STYLE
                        | HdRprimDirtyBits::DIRTY_REPR;
                } else {
                    // Check individual sub-locators.
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_cull_style_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_CULL_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_displacement_enabled_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_display_in_overlay_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_flat_shading_enabled_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_material_is_final_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_occluded_selection_shows_through_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_points_shading_enabled_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_refine_level_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_repr_selector_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_REPR;
                    }
                    if find_locator(
                        &HdLegacyDisplayStyleSchema::get_shading_style_locator(),
                        &locators,
                        &mut pos,
                        true,
                    ) {
                        bits |= HdRprimDirtyBits::DIRTY_DISPLAY_STYLE;
                    }
                }
            }
        }

        if find_locator(
            &HdExtentSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_EXTENT;
        }

        if find_locator(
            &HdExtComputationPrimvarsSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
        }

        if find_locator(
            &HdInstancedBySchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_INSTANCER;
        }

        if find_locator(
            &HdInstancerTopologySchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_INSTANCE_INDEX;
        }

        if find_locator(
            &HdMaterialBindingsSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_MATERIAL_ID;
        }

        if prim_type == &*RPRIM_MESH {
            if find_locator(
                &HdMeshSchema::get_double_sided_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_DOUBLE_SIDED;
            }
            if find_locator(
                &HdMeshSchema::get_subdivision_scheme_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_TOPOLOGY;
            }
            if find_locator(
                &HdMeshSchema::get_subdivision_tags_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_SUBDIV_TAGS;
            }
            if find_locator(
                &HdMeshTopologySchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_TOPOLOGY;
            }
        }

        // primvars: may be a parent (DirtyPrimvar | DirtyNormals | DirtyPoints | DirtyWidths)
        // or individual children mapped to specific bits.
        {
            let primvars_loc = HdPrimvarsSchema::get_default_locator();
            if set.intersects_locator(&primvars_loc) {
                let primvar_hits = set.intersection(&primvars_loc);
                if primvar_hits
                    .first()
                    .map(|locator| primvars_loc.has_prefix(locator))
                    .unwrap_or(false)
                {
                    bits |= HdRprimDirtyBits::DIRTY_PRIMVAR
                        | HdRprimDirtyBits::DIRTY_NORMALS
                        | HdRprimDirtyBits::DIRTY_POINTS
                        | HdRprimDirtyBits::DIRTY_WIDTHS;
                } else {
                    for l in &primvar_hits {
                        if l.has_prefix(&HdPrimvarsSchema::get_normals_locator()) {
                            bits |= HdRprimDirtyBits::DIRTY_NORMALS;
                        } else if l.has_prefix(&HdPrimvarsSchema::get_points_locator()) {
                            bits |= HdRprimDirtyBits::DIRTY_POINTS;
                        } else if l.has_prefix(&HdPrimvarsSchema::get_widths_locator()) {
                            bits |= HdRprimDirtyBits::DIRTY_WIDTHS;
                        } else {
                            bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
                        }
                    }
                }
            }
        }

        if find_locator(
            &HdPurposeSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_RENDER_TAG;
        }

        if prim_type == &*RPRIM_SPHERE {
            if find_locator(
                &HdSphereSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
        }

        if find_locator(
            &HdVisibilitySchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_VISIBILITY;
        }

        if find_locator(
            &HdVolumeFieldBindingSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_VOLUME_FIELD;
        }

        if find_locator(
            &HdXformSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_TRANSFORM;
        }

        bits
    }

    // =========================================================================
    // SprimLocatorSetToDirtyBits
    // =========================================================================

    /// Sprim: locator set -> dirty bits.
    ///
    /// Each sprim type defines its own dirty bit enum, so the empty locator
    /// must be translated to the type-specific AllDirty value.
    pub fn sprim_locator_set_to_dirty_bits(
        prim_type: &Token,
        set: &HdDataSourceLocatorSet,
    ) -> HdDirtyBits {
        let locators: Vec<HdDataSourceLocator> = set.iter().cloned().collect();

        if locators.is_empty() {
            return HD_CLEAN;
        }

        let mut bits: HdDirtyBits = HD_CLEAN;
        let mut pos = 0usize;

        if prim_type == &*SPRIM_MATERIAL {
            if locators[0].is_empty() {
                return hd_material_dirty_bits::ALL_DIRTY;
            }
            if find_locator(
                &HdMaterialSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                // Any locator under "material" sets params+resource at minimum.
                bits |=
                    hd_material_dirty_bits::DIRTY_PARAMS | hd_material_dirty_bits::DIRTY_RESOURCE;
                // Walk the full set to check for terminal-specific locators.
                for l in &locators {
                    let mat_loc = HdMaterialSchema::get_default_locator();
                    if *l == mat_loc {
                        bits |= hd_material_dirty_bits::ALL_DIRTY;
                    } else if l.has_prefix(&mat_loc) {
                        // Check terminal by inspecting the token after "material/".
                        let terminal = l.elements().get(1);
                        let terminal_str = terminal.map(|t| t.as_str());
                        match terminal_str {
                            Some("surface") => {
                                bits |= hd_material_dirty_bits::DIRTY_SURFACE;
                            }
                            Some("displacement") => {
                                bits |= hd_material_dirty_bits::DIRTY_DISPLACEMENT;
                            }
                            Some("volume") => {
                                bits |= hd_material_dirty_bits::DIRTY_VOLUME;
                            }
                            _ => {
                                // Unknown terminal (e.g. material/{renderContext}): all dirty.
                                bits |= hd_material_dirty_bits::ALL_DIRTY;
                            }
                        }
                    }
                }
            }
        } else if prim_type == &*SPRIM_COORD_SYS {
            if locators[0].is_empty() {
                return HdCoordSysDirtyBits::ALL_DIRTY;
            }
            let name_locator = HdCoordSysSchema::get_default_locator().append(&Token::new("name"));
            if find_locator(&name_locator, &locators, &mut pos, true) {
                bits |= HdCoordSysDirtyBits::DIRTY_NAME;
            }
            if find_locator(
                &HdXformSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdCoordSysDirtyBits::DIRTY_TRANSFORM;
            }
        } else if prim_type == &*SPRIM_CAMERA {
            if locators[0].is_empty() {
                return HdCameraDirtyBits::ALL_DIRTY;
            }
            if find_locator(
                &HdCameraSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdCameraDirtyBits::DIRTY_WINDOW_POLICY
                    | HdCameraDirtyBits::DIRTY_CLIP_PLANES
                    | HdCameraDirtyBits::DIRTY_PARAMS;
            }
            if find_locator(
                &HdXformSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdCameraDirtyBits::DIRTY_TRANSFORM;
            }
        } else if hd_prim_type_is_light(prim_type) || prim_type == &*LIGHT_FILTER {
            if locators[0].is_empty() {
                return hd_light_dirty_bits::ALL_DIRTY;
            }
            if find_locator(
                &HdInstancedBySchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_light_dirty_bits::DIRTY_INSTANCER;
            }
            if find_locator(
                &HdLightSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_light_dirty_bits::DIRTY_PARAMS
                    | hd_light_dirty_bits::DIRTY_RESOURCE
                    | hd_light_dirty_bits::DIRTY_SHADOW_PARAMS
                    | hd_light_dirty_bits::DIRTY_COLLECTION;
            }
            if find_locator(
                &HdMaterialSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_light_dirty_bits::DIRTY_RESOURCE;
            }
            if find_locator(
                &HdPrimvarsSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_light_dirty_bits::DIRTY_PARAMS;
            }
            if find_locator(
                &HdVisibilitySchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_light_dirty_bits::DIRTY_PARAMS;
            }
            if find_locator(
                &HdXformSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_light_dirty_bits::DIRTY_TRANSFORM;
            }
        } else if prim_type == &*SPRIM_DRAW_TARGET {
            let draw_target_loc = HdDataSourceLocator::from_token(SPRIM_DRAW_TARGET.clone());
            // Handles the empty locator case too via intersects.
            if find_locator(&draw_target_loc, &locators, &mut pos, true) {
                // Use ALL_SCENE_DIRTY_BITS to avoid stomping on HdStDrawTarget's camera bit.
                bits |= HdRprimDirtyBits::ALL_SCENE_DIRTY_BITS;
            }
        } else if prim_type == &*SPRIM_EXT_COMPUTATION {
            // Handles the empty locator case via the parent-check path.
            let ext_comp_loc = HdExtComputationSchema::get_default_locator();
            if find_locator(&ext_comp_loc, &locators, &mut pos, false) {
                if ext_comp_loc.has_prefix(&locators[pos]) {
                    bits |= HdExtComputationDirtyBits::DIRTY_DISPATCH_COUNT
                        | HdExtComputationDirtyBits::DIRTY_ELEMENT_COUNT
                        | HdExtComputationDirtyBits::DIRTY_KERNEL
                        | HdExtComputationDirtyBits::DIRTY_INPUT_DESC
                        | HdExtComputationDirtyBits::DIRTY_SCENE_INPUT
                        | HdExtComputationDirtyBits::DIRTY_OUTPUT_DESC;
                } else {
                    while pos < locators.len() && locators[pos].intersects(&ext_comp_loc) {
                        let l = &locators[pos];
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("extComputation"),
                            Token::new("dispatchCount"),
                        )) {
                            bits |= HdExtComputationDirtyBits::DIRTY_DISPATCH_COUNT;
                        }
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("extComputation"),
                            Token::new("elementCount"),
                        )) {
                            bits |= HdExtComputationDirtyBits::DIRTY_ELEMENT_COUNT;
                        }
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("extComputation"),
                            Token::new("glslKernel"),
                        )) {
                            bits |= HdExtComputationDirtyBits::DIRTY_KERNEL;
                        }
                        if l.has_prefix(&HdExtComputationSchema::get_input_values_locator())
                            || l.has_prefix(
                                &HdExtComputationSchema::get_input_computations_locator(),
                            )
                        {
                            bits |= HdExtComputationDirtyBits::DIRTY_INPUT_DESC
                                | HdExtComputationDirtyBits::DIRTY_SCENE_INPUT;
                        }
                        if l.has_prefix(&HdExtComputationSchema::get_outputs_locator()) {
                            bits |= HdExtComputationDirtyBits::DIRTY_OUTPUT_DESC;
                        }
                        pos += 1;
                    }
                }
            }
        } else if prim_type == &*SPRIM_INTEGRATOR {
            if find_locator(
                &HdIntegratorSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR; // maps to DirtyParams in sprim context
            }
        } else if prim_type == &*SPRIM_SAMPLE_FILTER {
            if find_locator(
                &HdSampleFilterSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
            if find_locator(
                &HdVisibilitySchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_VISIBILITY;
            }
        } else if prim_type == &*SPRIM_DISPLAY_FILTER {
            if find_locator(
                &HdDisplayFilterSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
            }
            if find_locator(
                &HdVisibilitySchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= HdRprimDirtyBits::DIRTY_VISIBILITY;
            }
        } else if prim_type == &*SPRIM_IMAGE_SHADER {
            let image_shader_loc = HdDataSourceLocator::from_token(Token::new("imageShader"));
            if find_locator(&image_shader_loc, &locators, &mut pos, false) {
                if image_shader_loc.has_prefix(&locators[pos]) {
                    bits |= hd_image_shader_dirty_bits::ALL_DIRTY;
                } else {
                    while pos < locators.len() && locators[pos].intersects(&image_shader_loc) {
                        let l = &locators[pos];
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("imageShader"),
                            Token::new("enabled"),
                        )) {
                            bits |= hd_image_shader_dirty_bits::DIRTY_ENABLED;
                        }
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("imageShader"),
                            Token::new("priority"),
                        )) {
                            bits |= hd_image_shader_dirty_bits::DIRTY_PRIORITY;
                        }
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("imageShader"),
                            Token::new("filePath"),
                        )) {
                            bits |= hd_image_shader_dirty_bits::DIRTY_FILE_PATH;
                        }
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("imageShader"),
                            Token::new("constants"),
                        )) {
                            bits |= hd_image_shader_dirty_bits::DIRTY_CONSTANTS;
                        }
                        if l.has_prefix(&HdDataSourceLocator::from_tokens_2(
                            Token::new("imageShader"),
                            Token::new("materialNetwork"),
                        )) {
                            bits |= hd_image_shader_dirty_bits::DIRTY_MATERIAL_NETWORK;
                        }
                        pos += 1;
                    }
                }
            }
        } else {
            let custom = CUSTOM_SPRIM_TRANSLATORS.lock().unwrap();
            if let Some((s_to_b, _)) = custom.get(prim_type) {
                s_to_b(set, &mut bits);
            } else {
                // Unknown prim type: AllDirty for any non-empty locator.
                if !locators.is_empty() {
                    bits |= HD_DIRTY_ALL;
                }
            }
        }

        bits
    }

    // =========================================================================
    // BprimLocatorSetToDirtyBits
    // =========================================================================

    /// Bprim: locator set -> dirty bits.
    pub fn bprim_locator_set_to_dirty_bits(
        prim_type: &Token,
        set: &HdDataSourceLocatorSet,
    ) -> HdDirtyBits {
        let locators: Vec<HdDataSourceLocator> = set.iter().cloned().collect();

        if locators.is_empty() {
            return HD_CLEAN;
        }

        let mut bits: HdDirtyBits = HD_CLEAN;
        let mut pos = 0usize;

        if prim_type == &*BPRIM_RENDER_BUFFER {
            if find_locator(
                &HdRenderBufferSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_buffer_dirty_bits::DIRTY_DESCRIPTION;
            }
        } else if prim_type == &*BPRIM_RENDER_SETTINGS {
            if find_locator(
                &HdRenderSettingsSchema::get_active_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_ACTIVE;
            }
            if find_locator(
                &HdRenderSettingsSchema::get_frame_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_FRAME_NUMBER;
            }
            if find_locator(
                &HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("includedPurposes"),
                ),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_INCLUDED_PURPOSES;
            }
            if find_locator(
                &HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("materialBindingPurposes"),
                ),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_MATERIAL_BINDING_PURPOSES;
            }
            if find_locator(
                &HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("namespacedSettings"),
                ),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_NAMESPACED_SETTINGS;
            }
            // renderProducts sorts before renderingColorSpace (uppercase R < lowercase r).
            if find_locator(
                &HdRenderSettingsSchema::get_render_products_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_RENDER_PRODUCTS;
            }
            if find_locator(
                &HdDataSourceLocator::from_tokens_2(
                    Token::new("renderSettings"),
                    Token::new("renderingColorSpace"),
                ),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_RENDERING_COLOR_SPACE;
            }
            if find_locator(
                &HdRenderSettingsSchema::get_shutter_interval_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                bits |= hd_render_settings_dirty_bits::DIRTY_SHUTTER_INTERVAL;
            }
        } else if is_volume_field_type(prim_type) {
            if find_locator(
                &HdVolumeFieldSchema::get_default_locator(),
                &locators,
                &mut pos,
                true,
            ) {
                // HdField::DirtyParams == bit 1.
                bits |= 1 << 1;
            }
        }

        bits
    }

    // =========================================================================
    // InstancerLocatorSetToDirtyBits
    // =========================================================================

    /// Instancer: locator set -> dirty bits.
    pub fn instancer_locator_set_to_dirty_bits(
        _prim_type: &Token,
        set: &HdDataSourceLocatorSet,
    ) -> HdDirtyBits {
        let locators: Vec<HdDataSourceLocator> = set.iter().cloned().collect();

        if locators.is_empty() {
            return HD_CLEAN;
        }

        if locators[0].is_empty() {
            return HdRprimDirtyBits::ALL_DIRTY;
        }

        let mut bits: HdDirtyBits = HD_CLEAN;
        let mut pos = 0usize;

        if find_locator(
            &HdCategoriesSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_CATEGORIES;
        }

        if find_locator(
            &HdInstanceCategoriesSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            // No dedicated instance-categories dirty bit; map to DirtyCategories.
            bits |= HdRprimDirtyBits::DIRTY_CATEGORIES;
        }

        if find_locator(
            &HdInstancedBySchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_INSTANCER;
        }

        if find_locator(
            &HdInstancerTopologySchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_INSTANCE_INDEX;
        }

        if find_locator(
            &HdPrimvarsSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_PRIMVAR;
        }

        if find_locator(
            &HdVisibilitySchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_VISIBILITY;
        }

        if find_locator(
            &HdXformSchema::get_default_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdRprimDirtyBits::DIRTY_TRANSFORM;
        }

        bits
    }

    // =========================================================================
    // TaskLocatorSetToDirtyBits
    // =========================================================================

    /// Task: locator set -> dirty bits.
    pub fn task_locator_set_to_dirty_bits(set: &HdDataSourceLocatorSet) -> HdDirtyBits {
        let locators: Vec<HdDataSourceLocator> = set.iter().cloned().collect();

        if locators.is_empty() {
            return HD_CLEAN;
        }

        if locators[0].is_empty() {
            return HD_DIRTY_ALL;
        }

        let mut bits: HdDirtyBits = HD_CLEAN;
        let mut pos = 0usize;

        if find_locator(
            &HdLegacyTaskSchema::get_collection_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdTaskDirtyBits::DIRTY_COLLECTION;
        }

        if find_locator(
            &HdLegacyTaskSchema::get_parameters_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdTaskDirtyBits::DIRTY_PARAMS;
        }

        if find_locator(
            &HdLegacyTaskSchema::get_render_tags_locator(),
            &locators,
            &mut pos,
            true,
        ) {
            bits |= HdTaskDirtyBits::DIRTY_RENDER_TAGS;
        }

        bits
    }

    // =========================================================================
    // Custom translator registration
    // =========================================================================

    /// Register custom sprim translators for a given prim type.
    pub fn register_translators_for_custom_sprim_type(
        prim_type: Token,
        s_to_b: LocatorSetToDirtyBitsFn,
        b_to_s: DirtyBitsToLocatorSetFn,
    ) {
        CUSTOM_SPRIM_TRANSLATORS
            .lock()
            .unwrap()
            .insert(prim_type, (s_to_b, b_to_s));
    }

    /// Register custom rprim translators for a given prim type.
    pub fn register_translators_for_custom_rprim_type(
        prim_type: Token,
        s_to_b: LocatorSetToDirtyBitsFn,
        b_to_s: DirtyBitsToLocatorSetFn,
    ) {
        CUSTOM_RPRIM_TRANSLATORS
            .lock()
            .unwrap()
            .insert(prim_type, (s_to_b, b_to_s));
    }
}

// ---- Volume field type detection --------------------------------------------
// Mirrors C++ HdLegacyPrimTypeIsVolumeField() which checks against a set of
// known volume field tokens (openvdb, field3d, etc).
fn is_volume_field_type(prim_type: &Token) -> bool {
    let s = prim_type.as_str();
    matches!(
        s,
        "openvdbAsset"
            | "field3dAsset"
            | "openVDB"
            | "field3d"
            | "brickmap"
            | "fieldTexture"
            | "rawField"
    )
}
