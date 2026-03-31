
//! VolumeShaderKey - shader variant selection for volume rendering.
//!
//! Port of C++ `HdSt_VolumeShaderKey`. Simple key with VS + FS stages
//! for ray-marched volume rendering. The VS sets up the proxy box geometry
//! and the FS performs the actual raymarching through the volume.

use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Shader key for volume rendering.
///
/// Selects the VS and FS mixins for ray-marched volume rendering.
/// Volumes use PRIM_VOLUME topology (a box proxy mesh).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VolumeShaderKey {
    /// GLSLFX/WGSLFX source file
    pub glslfx: Token,
    /// Vertex shader mixins
    pub vs: Vec<Token>,
    /// Fragment shader mixins
    pub fs: Vec<Token>,
}

impl VolumeShaderKey {
    /// Build the volume shader key.
    ///
    /// The C++ version has a fixed set of mixins for volume rendering.
    pub fn new() -> Self {
        let glslfx = Token::new("volume.glslfx");

        let vs = vec![
            Token::new("Instancing.Transform"),
            Token::new("Volume.Vertex.Common"),
        ];

        let fs = vec![
            Token::new("Volume.Fragment.Common"),
            Token::new("Volume.Fragment.Raymarching"),
            Token::new("Volume.Fragment.Compositing"),
        ];

        Self { glslfx, vs, fs }
    }

    /// Volumes use PRIM_VOLUME topology.
    pub fn primitive_type_is_volume(&self) -> bool {
        true
    }

    /// Compute a u64 hash for pipeline cache lookup.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

impl Default for VolumeShaderKey {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_key() {
        let key = VolumeShaderKey::new();
        assert!(key.primitive_type_is_volume());
        assert_eq!(key.glslfx.as_str(), "volume.glslfx");
    }

    #[test]
    fn test_volume_key_stages() {
        let key = VolumeShaderKey::new();
        assert!(!key.vs.is_empty());
        assert!(!key.fs.is_empty());
        assert!(key.vs.iter().any(|t| t.as_str().contains("Volume.Vertex")));
        assert!(key.fs.iter().any(|t| t.as_str().contains("Raymarching")));
    }

    #[test]
    fn test_volume_key_hash_stable() {
        let k1 = VolumeShaderKey::new();
        let k2 = VolumeShaderKey::new();
        assert_eq!(k1.cache_hash(), k2.cache_hash());
    }

    #[test]
    fn test_default() {
        let key = VolumeShaderKey::default();
        assert_eq!(key.glslfx.as_str(), "volume.glslfx");
    }
}
