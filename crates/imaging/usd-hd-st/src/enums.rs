
//! HdSt enums - Storm-specific enumeration types.
//!
//! Port of pxr/imaging/hdSt/enums.h

/// Storm texture types.
///
/// Port of C++ `HdStTextureType` (enums.h). Enumerates all texture sampling
/// strategies supported by Storm.
///
/// - `Uv`:      Sample 2D UV-mapped texture at given UV coordinates.
/// - `Field`:   Transform coordinates by a matrix before accessing a 3D field
///              texture (e.g. OpenVDB).
/// - `Ptex`:    Use Ptex connectivity to sample a Ptex texture.
/// - `Udim`:    Remap UV into UDIM coordinates (max tile width = 10) and
///              sample all tiles found on disk.
/// - `Cubemap`: Sample a direction in a cubemap texture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum HdStTextureType {
    /// 2D UV texture (most common).
    #[default]
    Uv,
    /// 3D field/volume texture with coordinate transform.
    Field,
    /// Ptex texture with face-based connectivity.
    Ptex,
    /// UDIM tile texture.
    Udim,
    /// Cubemap environment texture.
    Cubemap,
}

impl std::fmt::Display for HdStTextureType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Uv      => write!(f, "Uv"),
            Self::Field   => write!(f, "Field"),
            Self::Ptex    => write!(f, "Ptex"),
            Self::Udim    => write!(f, "Udim"),
            Self::Cubemap => write!(f, "Cubemap"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_type_default() {
        assert_eq!(HdStTextureType::default(), HdStTextureType::Uv);
    }

    #[test]
    fn test_texture_type_display() {
        assert_eq!(HdStTextureType::Uv.to_string(), "Uv");
        assert_eq!(HdStTextureType::Field.to_string(), "Field");
        assert_eq!(HdStTextureType::Ptex.to_string(), "Ptex");
        assert_eq!(HdStTextureType::Udim.to_string(), "Udim");
        assert_eq!(HdStTextureType::Cubemap.to_string(), "Cubemap");
    }

    #[test]
    fn test_texture_type_eq() {
        assert_eq!(HdStTextureType::Uv, HdStTextureType::Uv);
        assert_ne!(HdStTextureType::Uv, HdStTextureType::Cubemap);
    }
}
