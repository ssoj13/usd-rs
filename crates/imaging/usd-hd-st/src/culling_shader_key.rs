
//! CullingShaderKey - shader keys for GPU frustum/backface culling.
//!
//! Port of C++ `HdSt_CullingShaderKey` and `HdSt_CullingComputeShaderKey`.
//! Two variants:
//! - `CullingShaderKey` - vertex shader based culling (transform feedback)
//! - `CullingComputeShaderKey` - compute shader based culling
//!
//! Both encode instancing, tiny-prim culling, and instance counting flags
//! to select the correct culling shader mixins.

use std::hash::{Hash, Hasher};
use usd_tf::Token;

/// Vertex-shader-based frustum culling key.
///
/// Uses a VS-only pipeline with transform feedback to cull draw items.
/// The output marks which instances are visible after frustum testing.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CullingShaderKey {
    /// Enable per-instance culling (vs per-draw-item)
    pub instancing: bool,
    /// Enable tiny-prim culling (reject sub-pixel geometry)
    pub tiny_cull: bool,
    /// Count surviving instances (for indirect dispatch)
    pub counting: bool,
    /// GLSLFX source file
    pub glslfx: Token,
    /// Vertex shader mixins
    pub vs: Vec<Token>,
}

impl CullingShaderKey {
    /// Build a culling shader key with the given flags.
    pub fn new(instancing: bool, tiny_cull: bool, counting: bool) -> Self {
        let glslfx = Token::new("frustumCull.glslfx");

        let mut vs = vec![Token::new("FrustumCull.Vertex.Common")];

        if instancing {
            vs.push(Token::new("FrustumCull.Vertex.Instancing"));
        } else {
            vs.push(Token::new("FrustumCull.Vertex.NoInstancing"));
        }

        if tiny_cull {
            vs.push(Token::new("FrustumCull.Vertex.TinyCull"));
        }

        if counting {
            vs.push(Token::new("FrustumCull.Vertex.Counting"));
        } else {
            vs.push(Token::new("FrustumCull.Vertex.NoCounting"));
        }

        Self {
            instancing,
            tiny_cull,
            counting,
            glslfx,
            vs,
        }
    }

    /// This is always a frustum culling pass.
    pub fn is_frustum_culling_pass(&self) -> bool {
        true
    }

    /// Culling always operates on points (one per draw item / instance).
    pub fn primitive_type_is_points(&self) -> bool {
        true
    }

    /// Compute a u64 hash for cache lookup.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

/// Compute-shader-based frustum culling key.
///
/// Uses a compute pipeline instead of VS + transform feedback.
/// More efficient on modern GPUs with good compute support.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CullingComputeShaderKey {
    /// Enable per-instance culling
    pub instancing: bool,
    /// Enable tiny-prim culling
    pub tiny_cull: bool,
    /// Count surviving instances
    pub counting: bool,
    /// GLSLFX source file
    pub glslfx: Token,
    /// Compute shader mixins
    pub cs: Vec<Token>,
}

impl CullingComputeShaderKey {
    /// Build a compute culling shader key with the given flags.
    pub fn new(instancing: bool, tiny_cull: bool, counting: bool) -> Self {
        let glslfx = Token::new("frustumCull.glslfx");

        let mut cs = vec![Token::new("FrustumCull.Compute.Common")];

        if instancing {
            cs.push(Token::new("FrustumCull.Compute.Instancing"));
        } else {
            cs.push(Token::new("FrustumCull.Compute.NoInstancing"));
        }

        if tiny_cull {
            cs.push(Token::new("FrustumCull.Compute.TinyCull"));
        }

        if counting {
            cs.push(Token::new("FrustumCull.Compute.Counting"));
        } else {
            cs.push(Token::new("FrustumCull.Compute.NoCounting"));
        }

        Self {
            instancing,
            tiny_cull,
            counting,
            glslfx,
            cs,
        }
    }

    /// This is always a frustum culling pass.
    pub fn is_frustum_culling_pass(&self) -> bool {
        true
    }

    /// Compute culling uses PRIM_COMPUTE (no rasterization).
    pub fn primitive_type_is_compute(&self) -> bool {
        true
    }

    /// Compute a u64 hash for cache lookup.
    pub fn cache_hash(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_culling_basic() {
        let key = CullingShaderKey::new(false, false, false);
        assert!(key.is_frustum_culling_pass());
        assert!(key.primitive_type_is_points());
        assert!(key.vs.iter().any(|t| t.as_str().contains("NoInstancing")));
        assert!(key.vs.iter().any(|t| t.as_str().contains("NoCounting")));
    }

    #[test]
    fn test_culling_instanced_tiny() {
        let key = CullingShaderKey::new(true, true, false);
        assert!(
            key.vs
                .iter()
                .any(|t| t.as_str().contains("Instancing") && !t.as_str().contains("No"))
        );
        assert!(key.vs.iter().any(|t| t.as_str().contains("TinyCull")));
    }

    #[test]
    fn test_culling_counting() {
        let key = CullingShaderKey::new(false, false, true);
        assert!(
            key.vs
                .iter()
                .any(|t| t.as_str().contains("Counting") && !t.as_str().contains("No"))
        );
    }

    #[test]
    fn test_compute_culling_basic() {
        let key = CullingComputeShaderKey::new(false, false, false);
        assert!(key.is_frustum_culling_pass());
        assert!(key.primitive_type_is_compute());
        assert!(key.cs.iter().any(|t| t.as_str().contains("Common")));
    }

    #[test]
    fn test_compute_culling_all_flags() {
        let key = CullingComputeShaderKey::new(true, true, true);
        assert!(
            key.cs
                .iter()
                .any(|t| t.as_str().contains("Instancing") && !t.as_str().contains("No"))
        );
        assert!(key.cs.iter().any(|t| t.as_str().contains("TinyCull")));
        assert!(
            key.cs
                .iter()
                .any(|t| t.as_str().contains("Counting") && !t.as_str().contains("No"))
        );
    }

    #[test]
    fn test_hash_differs() {
        let k1 = CullingShaderKey::new(false, false, false);
        let k2 = CullingShaderKey::new(true, true, true);
        assert_ne!(k1.cache_hash(), k2.cache_hash());
    }

    #[test]
    fn test_compute_hash_differs() {
        let k1 = CullingComputeShaderKey::new(false, false, false);
        let k2 = CullingComputeShaderKey::new(true, false, true);
        assert_ne!(k1.cache_hash(), k2.cache_hash());
    }
}
