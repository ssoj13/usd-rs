//! GL utility functions: format queries, framebuffer status checks.
//!
//! Port of pxr/imaging/glf/utils.h / utils.cpp
//!
//! All GL enum constants used here are standard `gl` crate values.
//! Legacy GL 1.x formats (LUMINANCE, LUMINANCE_ALPHA, COLOR_INDEX) that are
//! absent from the modern `gl` crate are referenced by their raw numeric
//! values from the OpenGL specification.
//!
//! The pure-enum functions are usable without a GL context.
//! `check_gl_framebuffer_status` requires a current context.

// Legacy GL 1.x format tokens absent from the modern `gl` crate.
const GL_LUMINANCE: u32 = 0x1909;
const GL_LUMINANCE_ALPHA: u32 = 0x190A;
const GL_COLOR_INDEX: u32 = 0x1900;
const GL_ALPHA: u32 = 0x1906;

// S3TC compressed format tokens (GL extensions, not in core gl crate)
const GL_COMPRESSED_RGBA_S3TC_DXT1_EXT: u32 = 0x83F1;
const GL_COMPRESSED_RGBA_S3TC_DXT5_EXT: u32 = 0x83F3;

// ---------------------------------------------------------------------------
// GlfGetNumElements — channel count for a GL base format
// ---------------------------------------------------------------------------

/// Return the number of elements (channels) for a GL base format.
///
/// Mirrors `GlfGetNumElements()`.
///
/// | Format                          | Channels |
/// |---------------------------------|----------|
/// | DEPTH_COMPONENT, RED, ALPHA ... |  1       |
/// | RG, LUMINANCE_ALPHA             |  2       |
/// | RGB                             |  3       |
/// | RGBA                            |  4       |
///
/// Returns 1 for unrecognized formats and logs a warning.
pub fn get_num_elements(format: u32) -> u32 {
    match format {
        gl::DEPTH_COMPONENT | gl::RED | GL_ALPHA | GL_LUMINANCE | GL_COLOR_INDEX => 1,
        gl::RG | GL_LUMINANCE_ALPHA => 2,
        gl::RGB => 3,
        gl::RGBA => 4,
        _ => {
            log::warn!("GlfGetNumElements: unsupported format 0x{:04X}", format);
            1
        }
    }
}

// ---------------------------------------------------------------------------
// GlfGetElementSize — byte size for a GL type
// ---------------------------------------------------------------------------

/// Return the byte size of one element of a GL scalar type.
///
/// Mirrors `GlfGetElementSize()`.
///
/// Returns `size_of::<f32>()` for unrecognised types and logs a warning.
pub fn get_element_size(gl_type: u32) -> u32 {
    match gl_type {
        gl::UNSIGNED_BYTE | gl::BYTE => 1,
        gl::UNSIGNED_SHORT | gl::SHORT | gl::HALF_FLOAT => 2,
        gl::UNSIGNED_INT | gl::INT | gl::FLOAT => 4,
        gl::DOUBLE => 8,
        _ => {
            log::warn!("GlfGetElementSize: unsupported type 0x{:04X}", gl_type);
            4
        }
    }
}

// ---------------------------------------------------------------------------
// GlfCheckGLFrameBufferStatus
// ---------------------------------------------------------------------------

/// Check whether the currently-bound GL framebuffer is complete.
///
/// Mirrors `GlfCheckGLFrameBufferStatus()`.
///
/// * `target` — typically `gl::FRAMEBUFFER`, `gl::READ_FRAMEBUFFER`, or
///   `gl::DRAW_FRAMEBUFFER`.
///
/// Returns `Ok(())` when the framebuffer is complete, or `Err(reason)` with
/// a human-readable description of the problem.
///
/// # Safety
/// Requires a current GL context.
#[cfg(feature = "opengl")]
pub fn check_gl_framebuffer_status(target: u32) -> Result<(), String> {
    let status = unsafe { gl::CheckFramebufferStatus(target) };
    match status {
        gl::FRAMEBUFFER_COMPLETE => Ok(()),
        gl::FRAMEBUFFER_UNSUPPORTED => Err("Framebuffer unsupported".to_owned()),
        gl::FRAMEBUFFER_INCOMPLETE_ATTACHMENT => {
            Err("Framebuffer incomplete attachment".to_owned())
        }
        gl::FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT => {
            Err("Framebuffer incomplete: missing attachment".to_owned())
        }
        gl::FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER => {
            Err("Framebuffer incomplete: draw buffer".to_owned())
        }
        gl::FRAMEBUFFER_INCOMPLETE_READ_BUFFER => {
            Err("Framebuffer incomplete: read buffer".to_owned())
        }
        gl::FRAMEBUFFER_INCOMPLETE_MULTISAMPLE => {
            Err("Framebuffer incomplete: multisample".to_owned())
        }
        gl::FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS => {
            Err("Framebuffer incomplete: layer targets".to_owned())
        }
        other => Err(format!("Framebuffer error 0x{:04X}", other)),
    }
}

/// Stub that always returns `Ok(())` when the `opengl` feature is disabled.
#[cfg(not(feature = "opengl"))]
pub fn check_gl_framebuffer_status(_target: u32) -> Result<(), String> {
    Ok(())
}

// ---------------------------------------------------------------------------
// HioFormat conversion
// ---------------------------------------------------------------------------

/// A self-contained HIO format descriptor for format conversion.
///
/// We use a local enum rather than pulling in all of usd-hio so that
/// usd-glf can remain a lighter dependency.  The variant names match
/// those in `usd_hio::HioFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum HioFormat {
    Invalid = -1,
    // 1-channel UNorm8
    UNorm8 = 0,
    UNorm8Vec2,
    UNorm8Vec3,
    UNorm8Vec4,
    // sRGB variants
    UNorm8srgb,
    UNorm8Vec2srgb,
    UNorm8Vec3srgb,
    UNorm8Vec4srgb,
    // SNorm8
    SNorm8,
    SNorm8Vec2,
    SNorm8Vec3,
    SNorm8Vec4,
    // Float16
    Float16,
    Float16Vec2,
    Float16Vec3,
    Float16Vec4,
    // Float32
    Float32,
    Float32Vec2,
    Float32Vec3,
    Float32Vec4,
    // Double64
    Double64,
    Double64Vec2,
    Double64Vec3,
    Double64Vec4,
    // Integer types
    UInt16,
    UInt16Vec2,
    UInt16Vec3,
    UInt16Vec4,
    Int16,
    Int16Vec2,
    Int16Vec3,
    Int16Vec4,
    UInt32,
    UInt32Vec2,
    UInt32Vec3,
    UInt32Vec4,
    Int32,
    Int32Vec2,
    Int32Vec3,
    Int32Vec4,
    // BPTC / S3TC compressed
    BC6UFloatVec3,
    BC6FloatVec3,
    BC7UNorm8Vec4,
    BC7UNorm8Vec4srgb,
    BC1UNorm8Vec4,
    BC3UNorm8Vec4,
}

/// Map a (glFormat, glType, isSRGB) triple to an `HioFormat`.
///
/// Mirrors `GlfGetHioFormat()`.  Returns `HioFormat::Invalid` for
/// unrecognized combinations.
pub fn get_hio_format(gl_format: u32, gl_type: u32, is_srgb: bool) -> HioFormat {
    match gl_format {
        // Single-channel
        gl::DEPTH_COMPONENT | gl::RED | GL_ALPHA | GL_LUMINANCE | GL_COLOR_INDEX => match gl_type {
            gl::UNSIGNED_BYTE => {
                if is_srgb {
                    HioFormat::UNorm8srgb
                } else {
                    HioFormat::UNorm8
                }
            }
            gl::BYTE => HioFormat::SNorm8,
            gl::UNSIGNED_SHORT => HioFormat::UInt16,
            gl::SHORT => HioFormat::Int16,
            gl::UNSIGNED_INT => HioFormat::UInt32,
            gl::INT => HioFormat::Int32,
            gl::HALF_FLOAT => HioFormat::Float16,
            gl::FLOAT => HioFormat::Float32,
            gl::DOUBLE => HioFormat::Double64,
            _ => HioFormat::Invalid,
        },
        // Two-channel
        gl::RG | GL_LUMINANCE_ALPHA => match gl_type {
            gl::UNSIGNED_BYTE => {
                if is_srgb {
                    HioFormat::UNorm8Vec2srgb
                } else {
                    HioFormat::UNorm8Vec2
                }
            }
            gl::BYTE => HioFormat::SNorm8Vec2,
            gl::UNSIGNED_SHORT => HioFormat::UInt16Vec2,
            gl::SHORT => HioFormat::Int16Vec2,
            gl::UNSIGNED_INT => HioFormat::UInt32Vec2,
            gl::INT => HioFormat::Int32Vec2,
            gl::HALF_FLOAT => HioFormat::Float16Vec2,
            gl::FLOAT => HioFormat::Float32Vec2,
            gl::DOUBLE => HioFormat::Double64Vec2,
            _ => HioFormat::Invalid,
        },
        // RGB
        gl::RGB => match gl_type {
            gl::UNSIGNED_BYTE => {
                if is_srgb {
                    HioFormat::UNorm8Vec3srgb
                } else {
                    HioFormat::UNorm8Vec3
                }
            }
            gl::BYTE => HioFormat::SNorm8Vec3,
            gl::UNSIGNED_SHORT => HioFormat::UInt16Vec3,
            gl::SHORT => HioFormat::Int16Vec3,
            gl::UNSIGNED_INT => HioFormat::UInt32Vec3,
            gl::INT => HioFormat::Int32Vec3,
            gl::HALF_FLOAT => HioFormat::Float16Vec3,
            gl::FLOAT => HioFormat::Float32Vec3,
            gl::DOUBLE => HioFormat::Double64Vec3,
            _ => HioFormat::Invalid,
        },
        // RGBA
        gl::RGBA => match gl_type {
            gl::UNSIGNED_BYTE => {
                if is_srgb {
                    HioFormat::UNorm8Vec4srgb
                } else {
                    HioFormat::UNorm8Vec4
                }
            }
            gl::BYTE => HioFormat::SNorm8Vec4,
            gl::UNSIGNED_SHORT => HioFormat::UInt16Vec4,
            gl::SHORT => HioFormat::Int16Vec4,
            gl::UNSIGNED_INT => HioFormat::UInt32Vec4,
            gl::INT => HioFormat::Int32Vec4,
            gl::HALF_FLOAT => HioFormat::Float16Vec4,
            gl::FLOAT => HioFormat::Float32Vec4,
            gl::DOUBLE => HioFormat::Double64Vec4,
            _ => HioFormat::Invalid,
        },
        // Compressed BPTC
        gl::COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT => HioFormat::BC6UFloatVec3,
        gl::COMPRESSED_RGB_BPTC_SIGNED_FLOAT => HioFormat::BC6FloatVec3,
        gl::COMPRESSED_RGBA_BPTC_UNORM => HioFormat::BC7UNorm8Vec4,
        gl::COMPRESSED_SRGB_ALPHA_BPTC_UNORM => HioFormat::BC7UNorm8Vec4srgb,
        // S3TC (DXT)
        GL_COMPRESSED_RGBA_S3TC_DXT1_EXT => HioFormat::BC1UNorm8Vec4,
        GL_COMPRESSED_RGBA_S3TC_DXT5_EXT => HioFormat::BC3UNorm8Vec4,
        _ => {
            log::warn!("get_hio_format: unsupported format 0x{:04X}", gl_format);
            HioFormat::Invalid
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_num_elements() {
        assert_eq!(get_num_elements(gl::RED), 1);
        assert_eq!(get_num_elements(gl::DEPTH_COMPONENT), 1);
        assert_eq!(get_num_elements(gl::RG), 2);
        assert_eq!(get_num_elements(gl::RGB), 3);
        assert_eq!(get_num_elements(gl::RGBA), 4);
        // Unknown falls back to 1
        assert_eq!(get_num_elements(0xDEAD), 1);
    }

    #[test]
    fn test_element_size() {
        assert_eq!(get_element_size(gl::UNSIGNED_BYTE), 1);
        assert_eq!(get_element_size(gl::HALF_FLOAT), 2);
        assert_eq!(get_element_size(gl::FLOAT), 4);
        assert_eq!(get_element_size(gl::DOUBLE), 8);
    }

    #[test]
    fn test_hio_format_rgba_float() {
        assert_eq!(
            get_hio_format(gl::RGBA, gl::FLOAT, false),
            HioFormat::Float32Vec4
        );
    }

    #[test]
    fn test_hio_format_rgb_byte_srgb() {
        assert_eq!(
            get_hio_format(gl::RGB, gl::UNSIGNED_BYTE, true),
            HioFormat::UNorm8Vec3srgb
        );
    }

    #[test]
    fn test_hio_format_depth() {
        assert_eq!(
            get_hio_format(gl::DEPTH_COMPONENT, gl::FLOAT, false),
            HioFormat::Float32
        );
    }

    #[test]
    fn test_hio_format_s3tc_dxt1() {
        assert_eq!(
            get_hio_format(GL_COMPRESSED_RGBA_S3TC_DXT1_EXT, 0, false),
            HioFormat::BC1UNorm8Vec4
        );
    }

    #[test]
    fn test_hio_format_s3tc_dxt5() {
        assert_eq!(
            get_hio_format(GL_COMPRESSED_RGBA_S3TC_DXT5_EXT, 0, false),
            HioFormat::BC3UNorm8Vec4
        );
    }

    #[test]
    fn test_framebuffer_status_stub() {
        // Should not panic; returns Ok without a context when feature disabled
        #[cfg(not(feature = "opengl"))]
        assert!(check_gl_framebuffer_status(gl::FRAMEBUFFER).is_ok());
    }
}
