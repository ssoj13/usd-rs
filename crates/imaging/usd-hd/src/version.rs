
//! Hydra API version constants.
//!
//! This module defines version numbers for Hydra's public API to track
//! compatibility and changes across releases.

/// Hydra API version number.
///
/// This version is incremented whenever there's a breaking change to the
/// Hydra API. See the version history in the C++ header for detailed changelog.
pub const HD_API_VERSION: u32 = 90;

/// Shader API version number.
///
/// Tracks changes to shader interfaces:
/// - Version 1: SimpleLighting
/// - Version 2: FallbackLighting (current)
pub const HD_SHADER_API: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constants() {
        assert_eq!(HD_API_VERSION, 90);
        assert_eq!(HD_SHADER_API, 2);
    }
}
