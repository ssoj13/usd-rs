//! ShaderGlobals — the per-shading-point state passed to shaders.
//!
//! This struct is binary-compatible with the C++ `OSL::ShaderGlobals`.
//! Renderers fill it in before calling `ShadingSystem::execute()`.
//!
//! All points, vectors, and normals are in "common" space.
//!
//! # Raw pointers in this struct
//!
//! `ShaderGlobals` is `#[repr(C)]` — its field layout must match the C++
//! struct byte-for-byte so that C renderers can populate it at known offsets.
//! This means several fields are raw pointers (`*mut c_void`, etc.) even
//! though the safe Rust API never exposes them:
//!
//! - **`renderstate`, `tracedata`, `objdata`**: opaque renderer-specific
//!   state. Only the renderer's own code dereferences these.
//! - **`context`, `renderer`**: back-pointers set by the OSL runtime.
//! - **`ci`**: closure output placeholder. In practice, the safe closure
//!   tree lives in [`crate::context::ShadingContext::ci`] as
//!   `Option<ClosureRef>`. This raw pointer field exists only to preserve
//!   the C struct layout.
//!
//! All other fields (positions, normals, UVs, derivatives, etc.) are plain
//! `pub` value types — no unsafe needed to access them.
//!
//! # Accessor functions
//!
//! The `get_*()` functions at the bottom match the C++ `OSL::get_*()` API
//! for use from C FFI callers who have an `OpaqueExecContextPtr`. **Rust
//! code should access fields directly** — `sg.p`, `sg.n`, etc. — which
//! is completely safe.

use std::ffi::c_void;

use crate::Float;
use crate::math::Vec3;

/// Opaque pointer to whatever the renderer uses to represent a
/// (potentially motion-blurred) coordinate transformation.
///
/// This stays as a raw pointer because transformations are renderer-defined
/// and opaque to OSL. The renderer's `get_matrix()` implementation knows
/// how to interpret it.
pub type TransformationPtr = *const c_void;

/// Opaque pointer to the shading state uniform data.
pub type OpaqueShadingStateUniformPtr = *const c_void;

/// Opaque execution context pointer (for C API compatibility).
///
/// In the C++ API, many accessor functions take an `OpaqueExecContextPtr`
/// instead of a typed `ShaderGlobals&`. We preserve this for FFI parity.
/// **Rust code should use `&ShaderGlobals` directly instead.**
pub type OpaqueExecContextPtr = *mut c_void;

/// Forward declaration — actual ShadingContext lives in the context module.
/// Kept as an opaque `#[repr(C)]` type for the back-pointer in ShaderGlobals.
#[repr(C)]
pub struct ShadingContextOpaque {
    _opaque: [u8; 0],
}

/// Forward declaration — actual RendererServices lives in the renderer module.
/// Kept as an opaque `#[repr(C)]` type for the pointer in ShaderGlobals.
#[repr(C)]
pub struct RendererServicesOpaque {
    _opaque: [u8; 0],
}

/// The per-shading-point state, binary-compatible with C++ `ShaderGlobals`.
///
/// Fields are laid out in the exact same order as the C++ struct to ensure
/// ABI compatibility when passing this to C renderers or receiving it from
/// C++ code.
///
/// **For Rust consumers**: access fields directly (`sg.p`, `sg.n`, etc.).
/// The `get_*()` functions exist only for C FFI parity.
#[repr(C)]
#[derive(Clone)]
pub struct ShaderGlobals {
    // -- Surface position and derivatives --
    /// Surface position.
    pub p: Vec3,
    /// dP/dx derivative.
    pub dp_dx: Vec3,
    /// dP/dy derivative.
    pub dp_dy: Vec3,
    /// dP/dz derivative (volume shading only).
    pub dp_dz: Vec3,

    // -- Incident ray and derivatives --
    /// Incident ray direction.
    pub i: Vec3,
    /// dI/dx derivative.
    pub di_dx: Vec3,
    /// dI/dy derivative.
    pub di_dy: Vec3,

    // -- Normals --
    /// Shading normal (already front-facing).
    pub n: Vec3,
    /// True geometric normal.
    pub ng: Vec3,

    // -- UV parameters --
    /// Surface parameter u.
    pub u: Float,
    /// du/dx.
    pub dudx: Float,
    /// du/dy.
    pub dudy: Float,
    /// Surface parameter v.
    pub v: Float,
    /// dv/dx.
    pub dvdx: Float,
    /// dv/dy.
    pub dvdy: Float,

    // -- Surface tangents --
    /// dP/du.
    pub dp_du: Vec3,
    /// dP/dv.
    pub dp_dv: Vec3,

    // -- Time --
    /// Shading sample time.
    pub time: Float,
    /// Time interval for the frame.
    pub dtime: Float,
    /// Velocity: dP/dt.
    pub dp_dtime: Vec3,

    // -- Light point (for light shaders) --
    /// Point being illuminated.
    pub ps: Vec3,
    /// dPs/dx.
    pub dps_dx: Vec3,
    /// dPs/dy.
    pub dps_dy: Vec3,

    // -- Opaque renderer pointers --
    // These stay as raw pointers: only the renderer dereferences them.
    /// Renderer-specific state (opaque, set by renderer).
    pub renderstate: *mut c_void,
    /// Trace data (opaque, set by renderer).
    pub tracedata: *mut c_void,
    /// Object data (opaque, set by renderer).
    pub objdata: *mut c_void,

    // -- OSL internal pointers --
    /// Back-pointer to the ShadingContext (set by OSL runtime).
    pub context: *mut ShadingContextOpaque,
    /// Shading state uniform pointer.
    pub shading_state_uniform: OpaqueShadingStateUniformPtr,

    // -- Indices --
    /// 0-based thread index (set by shading system).
    pub thread_index: i32,
    /// Shade index (set by shading system).
    pub shade_index: i32,

    // -- Renderer services --
    /// Pointer to the RendererServices (set by shading system).
    pub renderer: *mut RendererServicesOpaque,

    // -- Transformation matrices --
    /// Object-to-common transform (opaque, set by renderer).
    pub object2common: TransformationPtr,
    /// Shader-to-common transform (opaque, set by renderer).
    pub shader2common: TransformationPtr,

    // -- Output closure --
    /// Closure output slot (raw pointer for C layout compatibility).
    ///
    /// **Do not use this field directly.** The safe closure output is
    /// stored in [`crate::context::ShadingContext::ci`] as
    /// `Option<ClosureRef>`. This raw pointer exists solely to keep
    /// the `#[repr(C)]` layout identical to the C++ `ShaderGlobals`.
    pub ci: *mut c_void,

    // -- Miscellaneous --
    /// Surface area of the emissive object.
    pub surfacearea: Float,
    /// Bit field of ray type flags.
    pub raytype: i32,
    /// If nonzero, flip the result of `calculatenormal()`.
    pub flip_handedness: i32,
    /// If nonzero, we are shading the back side of a surface.
    pub backfacing: i32,
}

impl ShaderGlobals {
    /// Create a default-initialized ShaderGlobals.
    ///
    /// All numeric fields are zero, all pointers are null. This is the safe
    /// equivalent of the old `mem::zeroed()` approach — no `unsafe` needed.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ShaderGlobals {
    fn default() -> Self {
        Self {
            p: Vec3::ZERO,
            dp_dx: Vec3::ZERO,
            dp_dy: Vec3::ZERO,
            dp_dz: Vec3::ZERO,
            i: Vec3::ZERO,
            di_dx: Vec3::ZERO,
            di_dy: Vec3::ZERO,
            n: Vec3::ZERO,
            ng: Vec3::ZERO,
            u: 0.0,
            dudx: 0.0,
            dudy: 0.0,
            v: 0.0,
            dvdx: 0.0,
            dvdy: 0.0,
            dp_du: Vec3::ZERO,
            dp_dv: Vec3::ZERO,
            time: 0.0,
            dtime: 0.0,
            dp_dtime: Vec3::ZERO,
            ps: Vec3::ZERO,
            dps_dx: Vec3::ZERO,
            dps_dy: Vec3::ZERO,
            renderstate: std::ptr::null_mut(),
            tracedata: std::ptr::null_mut(),
            objdata: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
            shading_state_uniform: std::ptr::null(),
            thread_index: 0,
            shade_index: 0,
            renderer: std::ptr::null_mut(),
            object2common: std::ptr::null(),
            shader2common: std::ptr::null(),
            ci: std::ptr::null_mut(),
            surfacearea: 0.0,
            raytype: 0,
            flip_handedness: 0,
            backfacing: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// SGBits — bitmask for which globals are read/written
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    /// Bitmask of which shader global variables are needed or written.
    ///
    /// Used by the optimizer to track which globals a shader group actually
    /// reads, allowing the renderer to skip computing unused derivatives.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SGBits: u32 {
        const NONE    = 0;
        const P       = 1 << 0;
        const I       = 1 << 1;
        const N       = 1 << 2;
        const NG      = 1 << 3;
        const U       = 1 << 4;
        const V       = 1 << 5;
        const DPDU    = 1 << 6;
        const DPDV    = 1 << 7;
        const TIME    = 1 << 8;
        const DTIME   = 1 << 9;
        const DPDTIME = 1 << 10;
        const PS      = 1 << 11;
        const CI      = 1 << 12;
    }
}

// ---------------------------------------------------------------------------
// Accessor functions (OpaqueExecContextPtr API — for C FFI only)
// ---------------------------------------------------------------------------
//
// These exist to match the C++ `OSL::get_P()`, `OSL::get_N()`, etc. API.
// They take an opaque void pointer and cast it to `&ShaderGlobals`.
//
// **Rust code should NOT use these.** Access fields directly instead:
//   `sg.p`, `sg.n`, `sg.u`, etc.
//
// Gated behind `feature = "capi"` because they require `unsafe` and are
// only needed by C FFI consumers.

#[cfg(feature = "capi")]
mod capi_getters {
    use super::*;

    /// Get a reference to the `ShaderGlobals` from an opaque context pointer.
    ///
    /// # Safety
    /// `oec` must be a valid `*mut ShaderGlobals`.
    #[inline]
    pub unsafe fn sg_from_exec_ctx(oec: OpaqueExecContextPtr) -> &'static ShaderGlobals {
        unsafe { &*(oec as *const ShaderGlobals) }
    }

    /// Get a mutable reference to the `ShaderGlobals` from an opaque context pointer.
    ///
    /// # Safety
    /// `oec` must be a valid `*mut ShaderGlobals`.
    #[inline]
    pub unsafe fn sg_from_exec_ctx_mut(oec: OpaqueExecContextPtr) -> &'static mut ShaderGlobals {
        unsafe { &mut *(oec as *mut ShaderGlobals) }
    }

    macro_rules! sg_getter_ref {
        ($(#[doc = $doc:expr])* $name:ident -> $ty:ty, $field:ident) => {
            $(#[doc = $doc])*
            /// # Safety
            /// `oec` must point to valid ShaderGlobals.
            #[inline]
            pub unsafe fn $name(oec: OpaqueExecContextPtr) -> &'static $ty {
                unsafe { &sg_from_exec_ctx(oec).$field }
            }
        };
    }

    macro_rules! sg_getter_val {
        ($(#[doc = $doc:expr])* $name:ident -> $ty:ty, $field:ident) => {
            $(#[doc = $doc])*
            /// # Safety
            /// `oec` must point to valid ShaderGlobals.
            #[inline]
            pub unsafe fn $name(oec: OpaqueExecContextPtr) -> $ty {
                unsafe { sg_from_exec_ctx(oec).$field }
            }
        };
    }

    sg_getter_ref! { #[doc = "Get surface position P."]           get_p      -> Vec3, p }
    sg_getter_ref! { #[doc = "Get shading normal N."]             get_n      -> Vec3, n }
    sg_getter_ref! { #[doc = "Get geometric normal Ng."]          get_ng     -> Vec3, ng }
    sg_getter_ref! { #[doc = "Get incident ray direction I."]     get_i      -> Vec3, i }
    sg_getter_val! { #[doc = "Get u parameter."]                  get_u      -> Float, u }
    sg_getter_val! { #[doc = "Get v parameter."]                  get_v      -> Float, v }
    sg_getter_val! { #[doc = "Get time."]                         get_time   -> Float, time }
    sg_getter_val! { #[doc = "Get shade index."]                  get_shade_index   -> i32, shade_index }
    sg_getter_val! { #[doc = "Get thread index."]                 get_thread_index  -> i32, thread_index }
    sg_getter_val! { #[doc = "Get backfacing flag."]              get_backfacing    -> i32, backfacing }
    sg_getter_val! { #[doc = "Get raytype bit field."]            get_raytype       -> i32, raytype }
    sg_getter_val! { #[doc = "Get surface area."]                 get_surfacearea   -> Float, surfacearea }
    sg_getter_ref! { #[doc = "Get dP/dx."]                        get_dpdx   -> Vec3, dp_dx }
    sg_getter_ref! { #[doc = "Get dP/dy."]                        get_dpdy   -> Vec3, dp_dy }
    sg_getter_ref! { #[doc = "Get dP/dz (volume only)."]          get_dpdz   -> Vec3, dp_dz }
    sg_getter_ref! { #[doc = "Get dI/dx."]                        get_didx   -> Vec3, di_dx }
    sg_getter_ref! { #[doc = "Get dI/dy."]                        get_didy   -> Vec3, di_dy }
    sg_getter_val! { #[doc = "Get du/dx."]                        get_dudx   -> Float, dudx }
    sg_getter_val! { #[doc = "Get du/dy."]                        get_dudy   -> Float, dudy }
    sg_getter_val! { #[doc = "Get dv/dx."]                        get_dvdx   -> Float, dvdx }
    sg_getter_val! { #[doc = "Get dv/dy."]                        get_dvdy   -> Float, dvdy }
    sg_getter_ref! { #[doc = "Get dP/du."]                        get_dpdu   -> Vec3, dp_du }
    sg_getter_ref! { #[doc = "Get dP/dv."]                        get_dpdv   -> Vec3, dp_dv }
    sg_getter_val! { #[doc = "Get dtime."]                        get_dtime  -> Float, dtime }
    sg_getter_ref! { #[doc = "Get dP/dtime."]                     get_dpdtime -> Vec3, dp_dtime }
    sg_getter_ref! { #[doc = "Get Ps (light shader)."]            get_ps     -> Vec3, ps }
    sg_getter_ref! { #[doc = "Get dPs/dx."]                       get_dpsdx  -> Vec3, dps_dx }
    sg_getter_ref! { #[doc = "Get dPs/dy."]                       get_dpsdy  -> Vec3, dps_dy }
    sg_getter_val! { #[doc = "Get flip_handedness flag."]          get_flip_handedness -> i32, flip_handedness }
    sg_getter_val! { #[doc = "Get object2common transform pointer."] get_object2common -> TransformationPtr, object2common }
    sg_getter_val! { #[doc = "Get shader2common transform pointer."] get_shader2common -> TransformationPtr, shader2common }

    /// Get closure output pointer Ci.
    /// Matches C++ `get_Ci(ec)` from shaderglobals.h.
    /// # Safety
    /// `oec` must point to valid ShaderGlobals.
    #[inline]
    pub unsafe fn get_ci(oec: OpaqueExecContextPtr) -> *mut c_void {
        unsafe { sg_from_exec_ctx(oec).ci }
    }
}

#[cfg(feature = "capi")]
pub use capi_getters::*;

// ---------------------------------------------------------------------------
// C API getters (osl_sg_* — take *const ShaderGlobals directly)
// ---------------------------------------------------------------------------

#[cfg(feature = "capi")]
mod capi_sg_getters {
    use super::*;

    macro_rules! sg_field_ref {
        ($(#[doc = $doc:expr])* $name:ident, $field:ident, $ty:ty) => {
            $(#[doc = $doc])*
            /// # Safety
            /// `sg` must point to a valid `ShaderGlobals`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(sg: *const ShaderGlobals) -> *const $ty {
                unsafe { &(*sg).$field }
            }
        };
    }

    macro_rules! sg_field_val {
        ($(#[doc = $doc:expr])* $name:ident, $field:ident, $ty:ty) => {
            $(#[doc = $doc])*
            /// # Safety
            /// `sg` must point to a valid `ShaderGlobals`.
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn $name(sg: *const ShaderGlobals) -> $ty {
                unsafe { (*sg).$field }
            }
        };
    }

    sg_field_ref! { #[doc = "Get surface position P."]       osl_sg_P,       p,       Vec3 }
    sg_field_ref! { #[doc = "Get dP/dx."]                    osl_sg_dPdx,    dp_dx,   Vec3 }
    sg_field_ref! { #[doc = "Get dP/dy."]                    osl_sg_dPdy,    dp_dy,   Vec3 }
    sg_field_ref! { #[doc = "Get incident ray I."]            osl_sg_I,       i,       Vec3 }
    sg_field_ref! { #[doc = "Get shading normal N."]          osl_sg_N,       n,       Vec3 }
    sg_field_ref! { #[doc = "Get geometric normal Ng."]       osl_sg_Ng,      ng,      Vec3 }
    sg_field_val! { #[doc = "Get u parameter."]               osl_sg_u,       u,       Float }
    sg_field_val! { #[doc = "Get v parameter."]               osl_sg_v,       v,       Float }
    sg_field_ref! { #[doc = "Get dP/du."]                    osl_sg_dPdu,    dp_du,   Vec3 }
    sg_field_ref! { #[doc = "Get dP/dv."]                    osl_sg_dPdv,    dp_dv,   Vec3 }
    sg_field_val! { #[doc = "Get shading time."]              osl_sg_time,    time,    Float }
    sg_field_val! { #[doc = "Get frame time interval."]       osl_sg_dtime,   dtime,   Float }
    sg_field_ref! { #[doc = "Get velocity dP/dtime."]         osl_sg_dPdtime, dp_dtime, Vec3 }
    sg_field_ref! { #[doc = "Get light point Ps."]            osl_sg_Ps,      ps,      Vec3 }
    sg_field_val! { #[doc = "Get backfacing flag."]           osl_sg_backfacing, backfacing, i32 }
    sg_field_val! { #[doc = "Get surface area."]              osl_sg_surfacearea, surfacearea, Float }
}

#[cfg(feature = "capi")]
pub use capi_sg_getters::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_globals_default() {
        let sg = ShaderGlobals::new();
        assert_eq!(sg.p.x, 0.0);
        assert_eq!(sg.raytype, 0);
        assert!(sg.ci.is_null());
        assert!(sg.renderer.is_null());
    }

    #[test]
    fn test_shader_globals_layout() {
        // Verify key field offsets match C++ layout expectations.
        use std::mem;

        let size = mem::size_of::<ShaderGlobals>();
        // Expected ~312–336 bytes on 64-bit (exact size depends on alignment/padding).
        assert!(
            size >= 300 && size <= 360,
            "ShaderGlobals size {size} is outside expected range 300–360"
        );

        // Verify #[repr(C)] by checking that P is at offset 0.
        let sg = ShaderGlobals::new();
        let base = &sg as *const ShaderGlobals as usize;
        let p_offset = &sg.p as *const Vec3 as usize - base;
        assert_eq!(p_offset, 0, "P must be at offset 0");
    }

    #[test]
    fn test_sg_bits() {
        let bits = SGBits::P | SGBits::N | SGBits::CI;
        assert!(bits.contains(SGBits::P));
        assert!(bits.contains(SGBits::N));
        assert!(!bits.contains(SGBits::I));
    }
}
