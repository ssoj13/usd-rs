//! C API — ABI-compatible exports for FFI consumers.
//!
//! Provides a C-compatible API matching the OSL C interface, allowing
//! this Rust implementation to be used as a drop-in replacement for
//! the C++ OSL library via dynamic linking.
//!
//! All functions use `extern "C"` and `#[unsafe(no_mangle)]` to ensure
//! symbol visibility and calling convention compatibility.

use std::ffi::{CStr, CString, c_char};
use std::os::raw::{c_float, c_int, c_void};
use std::ptr;
use std::sync::Arc;

use crate::math::{Color3, Vec3};
use crate::renderer::NullRenderer;
use crate::shaderglobals::ShaderGlobals;
use crate::shadingsys::{ShaderGroupRef, ShadingSystem};
use crate::typedesc::TypeDesc;
use crate::ustring::UString;

// ---------------------------------------------------------------------------
// Helper: C string conversions
// ---------------------------------------------------------------------------

unsafe fn cstr_to_str<'a>(s: *const c_char) -> &'a str {
    if s.is_null() {
        return "";
    }
    unsafe { CStr::from_ptr(s).to_str().unwrap_or("") }
}

// ---------------------------------------------------------------------------
// ShadingSystem
// ---------------------------------------------------------------------------

/// Opaque handle to a ShadingSystem.
pub type ShadingSystemHandle = *mut c_void;

/// Opaque handle to a ShaderGroup.
pub type ShaderGroupHandle = *mut c_void;

/// Create a new ShadingSystem with a null renderer.
#[unsafe(no_mangle)]
pub extern "C" fn osl_shading_system_create() -> ShadingSystemHandle {
    let ss = Box::new(ShadingSystem::new(Arc::new(NullRenderer), None));
    Box::into_raw(ss) as ShadingSystemHandle
}

/// Destroy a ShadingSystem.
///
/// # Safety
/// `handle` must be a valid ShadingSystem handle created by `osl_shading_system_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shading_system_destroy(handle: ShadingSystemHandle) {
    if !handle.is_null() {
        let _ = unsafe { Box::from_raw(handle as *mut ShadingSystem) };
    }
}

/// Set a string attribute on the ShadingSystem.
///
/// # Safety
/// `handle` must be a valid ShadingSystem handle. `name` and `value` must be valid C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shading_system_attribute_s(
    handle: ShadingSystemHandle,
    name: *const c_char,
    value: *const c_char,
) -> c_int {
    if handle.is_null() {
        return 0;
    }
    let ss = unsafe { &*(handle as *const ShadingSystem) };
    let name = unsafe { cstr_to_str(name) };
    let value = unsafe { cstr_to_str(value) };
    ss.attribute(
        name,
        crate::shadingsys::AttributeValue::String(value.to_string()),
    );
    1
}

/// Set an integer attribute on the ShadingSystem.
///
/// # Safety
/// `handle` must be a valid ShadingSystem handle. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shading_system_attribute_i(
    handle: ShadingSystemHandle,
    name: *const c_char,
    value: c_int,
) -> c_int {
    if handle.is_null() {
        return 0;
    }
    let ss = unsafe { &*(handle as *const ShadingSystem) };
    let name = unsafe { cstr_to_str(name) };
    ss.attribute(name, crate::shadingsys::AttributeValue::Int(value));
    1
}

/// Set a float attribute on the ShadingSystem.
///
/// # Safety
/// `handle` must be a valid ShadingSystem handle. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shading_system_attribute_f(
    handle: ShadingSystemHandle,
    name: *const c_char,
    value: c_float,
) -> c_int {
    if handle.is_null() {
        return 0;
    }
    let ss = unsafe { &*(handle as *const ShadingSystem) };
    let name = unsafe { cstr_to_str(name) };
    ss.attribute(name, crate::shadingsys::AttributeValue::Float(value));
    1
}

/// Begin construction of a new shader group.
///
/// # Safety
/// `handle` must be a valid ShadingSystem handle. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shader_group_begin(
    handle: ShadingSystemHandle,
    name: *const c_char,
) -> ShaderGroupHandle {
    if handle.is_null() {
        return ptr::null_mut();
    }
    let ss = unsafe { &*(handle as *const ShadingSystem) };
    let name = unsafe { cstr_to_str(name) };
    let group = ss.shader_group_begin(name);
    Box::into_raw(Box::new(group)) as ShaderGroupHandle
}

/// End construction of a shader group.
///
/// # Safety
/// `ss_handle` and `group_handle` must be valid handles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shader_group_end(
    ss_handle: ShadingSystemHandle,
    group_handle: ShaderGroupHandle,
) {
    if ss_handle.is_null() || group_handle.is_null() {
        return;
    }
    let ss = unsafe { &*(ss_handle as *const ShadingSystem) };
    let group = unsafe { &*(group_handle as *const ShaderGroupRef) };
    // C API: log validation errors but don't propagate them
    if let Err(e) = ss.shader_group_end(group) {
        ss.error(&e);
    }
}

/// Destroy a shader group handle.
///
/// # Safety
/// `handle` must be a valid ShaderGroup handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_shader_group_destroy(handle: ShaderGroupHandle) {
    if !handle.is_null() {
        let _ = unsafe { Box::from_raw(handle as *mut ShaderGroupRef) };
    }
}

/// Register a closure type.
///
/// # Safety
/// `ss_handle` must be a valid ShadingSystem handle. `name` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_register_closure(
    ss_handle: ShadingSystemHandle,
    name: *const c_char,
    id: c_int,
) -> c_int {
    if ss_handle.is_null() {
        return 0;
    }
    let ss = unsafe { &*(ss_handle as *const ShadingSystem) };
    let name = unsafe { cstr_to_str(name) };
    ss.register_closure(name, id, vec![]);
    1
}

// ---------------------------------------------------------------------------
// TypeDesc
// ---------------------------------------------------------------------------

/// Create a TypeDesc for float.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_float() -> TypeDesc {
    TypeDesc::FLOAT
}

/// Create a TypeDesc for int.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_int() -> TypeDesc {
    TypeDesc::INT
}

/// Create a TypeDesc for string.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_string() -> TypeDesc {
    TypeDesc::STRING
}

/// Create a TypeDesc for color.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_color() -> TypeDesc {
    TypeDesc::COLOR
}

/// Create a TypeDesc for point.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_point() -> TypeDesc {
    TypeDesc::POINT
}

/// Create a TypeDesc for vector.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_vector() -> TypeDesc {
    TypeDesc::VECTOR
}

/// Create a TypeDesc for normal.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_normal() -> TypeDesc {
    TypeDesc::NORMAL
}

/// Create a TypeDesc for matrix44.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_matrix44() -> TypeDesc {
    TypeDesc::MATRIX
}

/// Get the size of a TypeDesc in bytes.
#[unsafe(no_mangle)]
pub extern "C" fn osl_typedesc_size(td: TypeDesc) -> c_int {
    td.size() as c_int
}

// ---------------------------------------------------------------------------
// UString
// ---------------------------------------------------------------------------

/// Create an interned string (UString).
///
/// # Safety
/// `s` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_ustring_create(s: *const c_char) -> u64 {
    let s = unsafe { cstr_to_str(s) };
    let us = UString::new(s);
    us.hash()
}

/// Get the C string pointer for a UString hash.
///
/// # Safety
/// The returned pointer is valid as long as the UString table exists.
#[unsafe(no_mangle)]
pub extern "C" fn osl_ustring_c_str(hash: u64) -> *const c_char {
    if let Some(us) = UString::from_hash(hash) {
        // This is safe because UString's internal string is interned and lives forever
        us.as_str().as_ptr() as *const c_char
    } else {
        ptr::null()
    }
}

/// Compare two UString hashes for equality.
#[unsafe(no_mangle)]
pub extern "C" fn osl_ustring_eq(a: u64, b: u64) -> c_int {
    if a == b { 1 } else { 0 }
}

// ---------------------------------------------------------------------------
// ShaderGlobals
// ---------------------------------------------------------------------------

/// Get the size of ShaderGlobals struct.
#[unsafe(no_mangle)]
pub extern "C" fn osl_shaderglobals_size() -> c_int {
    std::mem::size_of::<ShaderGlobals>() as c_int
}

// ---------------------------------------------------------------------------
// Noise functions
// ---------------------------------------------------------------------------

/// Compute Perlin noise at a 3D point.
#[unsafe(no_mangle)]
pub extern "C" fn osl_noise_perlin3(x: c_float, y: c_float, z: c_float) -> c_float {
    crate::noise::perlin3(Vec3::new(x, y, z))
}

/// Compute unsigned Perlin noise at a 3D point.
#[unsafe(no_mangle)]
pub extern "C" fn osl_noise_uperlin3(x: c_float, y: c_float, z: c_float) -> c_float {
    crate::noise::uperlin3(Vec3::new(x, y, z))
}

/// Compute cell noise at a 3D point.
#[unsafe(no_mangle)]
pub extern "C" fn osl_noise_cellnoise3(x: c_float, y: c_float, z: c_float) -> c_float {
    crate::noise::cellnoise3(Vec3::new(x, y, z))
}

/// Compute simplex noise at a 3D point.
#[unsafe(no_mangle)]
pub extern "C" fn osl_noise_simplex3(x: c_float, y: c_float, z: c_float) -> c_float {
    crate::simplex::simplex3(Vec3::new(x, y, z))
}

// ---------------------------------------------------------------------------
// Color operations
// ---------------------------------------------------------------------------

/// Compute luminance of an RGB color.
#[unsafe(no_mangle)]
pub extern "C" fn osl_luminance(r: c_float, g: c_float, b: c_float) -> c_float {
    crate::color::luminance(Color3::new(r, g, b))
}

/// Compute blackbody color for a given temperature.
///
/// # Safety
/// Each of `r`, `g`, `b` may be null; when non-null, must point to a writable `c_float`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_blackbody(
    temp: c_float,
    r: *mut c_float,
    g: *mut c_float,
    b: *mut c_float,
) {
    let color = crate::color::blackbody(temp);
    unsafe {
        if !r.is_null() {
            *r = color.x;
        }
        if !g.is_null() {
            *g = color.y;
        }
        if !b.is_null() {
            *b = color.z;
        }
    }
}

// ---------------------------------------------------------------------------
// Hash functions
// ---------------------------------------------------------------------------

/// Compute FarmHash Fingerprint64.
///
/// # Safety
/// `data` must point to at least `len` bytes of valid memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_farmhash64(data: *const u8, len: c_int) -> u64 {
    if data.is_null() || len <= 0 {
        return 0;
    }
    let slice = unsafe { std::slice::from_raw_parts(data, len as usize) };
    crate::hashes::fingerprint64(slice)
}

// ---------------------------------------------------------------------------
// Compiler
// ---------------------------------------------------------------------------

/// Compile an OSL source string to OSO bytecode.
/// Returns a newly allocated C string with the OSO text, or NULL on error.
/// The caller must free the returned string with `osl_free_string`.
///
/// # Safety
/// `source` must be a valid C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_compile(source: *const c_char) -> *mut c_char {
    let source = unsafe { cstr_to_str(source) };
    let opts = crate::oslc::CompilerOptions::default();
    let result = crate::oslc::compile_string(source, &opts);
    if result.success {
        match CString::new(result.oso_text) {
            Ok(cs) => cs.into_raw(),
            Err(_) => ptr::null_mut(),
        }
    } else {
        ptr::null_mut()
    }
}

/// Free a string allocated by the osl_* functions.
///
/// # Safety
/// `s` must be a string previously returned by an `osl_*` function, or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn osl_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = unsafe { CString::from_raw(s) };
    }
}

// ---------------------------------------------------------------------------
// Version info
// ---------------------------------------------------------------------------

/// Get the OSL version number (major * 10000 + minor * 100 + patch).
#[unsafe(no_mangle)]
pub extern "C" fn osl_version() -> c_int {
    crate::OSL_VERSION as c_int
}

/// Get a version string.
#[unsafe(no_mangle)]
pub extern "C" fn osl_version_string() -> *const c_char {
    c"osl-rs 1.14.0".as_ptr()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typedesc_exports() {
        let f = osl_typedesc_float();
        assert_eq!(f, TypeDesc::FLOAT);
        assert_eq!(osl_typedesc_size(f), 4);

        let c = osl_typedesc_color();
        assert_eq!(osl_typedesc_size(c), 12);
    }

    #[test]
    fn test_shading_system_lifecycle() {
        unsafe {
            let ss = osl_shading_system_create();
            assert!(!ss.is_null());

            let name = c"test_group".as_ptr().cast::<c_char>();
            let group = osl_shader_group_begin(ss, name);
            assert!(!group.is_null());
            osl_shader_group_end(ss, group);
            osl_shader_group_destroy(group);

            osl_shading_system_destroy(ss);
        }
    }

    #[test]
    fn test_ustring_exports() {
        unsafe {
            let s = c"hello".as_ptr().cast::<c_char>();
            let hash = osl_ustring_create(s);
            assert_ne!(hash, 0);
            assert_eq!(osl_ustring_eq(hash, hash), 1);

            let s2 = c"world".as_ptr().cast::<c_char>();
            let hash2 = osl_ustring_create(s2);
            assert_eq!(osl_ustring_eq(hash, hash2), 0);
        }
    }

    #[test]
    fn test_noise_exports() {
        let v = osl_noise_perlin3(1.0, 2.0, 3.0);
        assert!((-1.0..=1.0).contains(&v));

        let v = osl_noise_uperlin3(1.0, 2.0, 3.0);
        assert!((0.0..=1.0).contains(&v));

        let v = osl_noise_cellnoise3(1.0, 2.0, 3.0);
        assert!((0.0..=1.0).contains(&v));
    }

    #[test]
    fn test_version() {
        assert_eq!(osl_version(), crate::OSL_VERSION as c_int);
    }

    #[test]
    fn test_compile_export() {
        unsafe {
            let src = c"shader test() { float x = 1.0; }"
                .as_ptr()
                .cast::<c_char>();
            let oso = osl_compile(src);
            assert!(!oso.is_null());
            let oso_str = CStr::from_ptr(oso).to_str().unwrap();
            assert!(oso_str.contains("OpenShadingLanguage"));
            osl_free_string(oso);
        }
    }

    #[test]
    fn test_shaderglobals_size() {
        let size = osl_shaderglobals_size();
        assert!(size > 0);
        assert_eq!(size as usize, std::mem::size_of::<ShaderGlobals>());
    }

    #[test]
    fn test_luminance_export() {
        let lum = osl_luminance(1.0, 1.0, 1.0);
        assert!(lum > 0.9 && lum < 1.1);
    }
}
