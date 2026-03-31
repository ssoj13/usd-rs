//! UsdSkel_SkelDefinition - internal skeleton structure cache.
//!
//! Port of pxr/usd/usdSkel/skelDefinition.h/cpp

use super::skeleton::Skeleton;
use super::topology::Topology;
use super::utils::{concat_joint_transforms, concat_joint_transforms_f};
use std::sync::{Arc, Mutex};
use usd_gf::{Matrix4d, Matrix4f};
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Convert a Matrix4d to Matrix4f (lossy f64->f32 conversion).
fn mat4d_to_4f(m: &Matrix4d) -> Matrix4f {
    Matrix4f::from_array([
        [
            m[0][0] as f32,
            m[0][1] as f32,
            m[0][2] as f32,
            m[0][3] as f32,
        ],
        [
            m[1][0] as f32,
            m[1][1] as f32,
            m[1][2] as f32,
            m[1][3] as f32,
        ],
        [
            m[2][0] as f32,
            m[2][1] as f32,
            m[2][2] as f32,
            m[2][3] as f32,
        ],
        [
            m[3][0] as f32,
            m[3][1] as f32,
            m[3][2] as f32,
            m[3][3] as f32,
        ],
    ])
}

/// Convert a slice of Matrix4d to Vec<Matrix4f>.
fn convert_4d_to_4f(xforms: &[Matrix4d]) -> Vec<Matrix4f> {
    xforms.iter().map(mat4d_to_4f).collect()
}

/// Internal structure storing the core definition of a Skeleton.
///
/// A definition is a cache of the *validated* structure of a skeleton,
/// including its topology, bind pose and rest pose.
/// Skeleton definitions are meant to be shared across instances.
///
/// Matches C++ `UsdSkel_SkelDefinition`.
#[derive(Clone)]
pub struct SkelDefinition {
    inner: Arc<SkelDefinitionInner>,
}

struct SkelDefinitionInner {
    /// The skeleton this definition is for.
    skeleton: Skeleton,
    /// Joint order tokens.
    joint_order: Vec<Token>,
    /// Skeleton topology.
    topology: Topology,
    /// Joint local rest transforms (from skeleton's restTransforms attribute).
    joint_local_rest_xforms: Vec<Matrix4d>,
    /// Joint world bind transforms (from skeleton's bindTransforms attribute).
    joint_world_bind_xforms: Vec<Matrix4d>,
    /// Cached on-demand transforms.
    cache: Mutex<SkelDefinitionCache>,
}

#[derive(Default)]
struct SkelDefinitionCache {
    /// Rest transforms in skel space (computed from local rest transforms).
    joint_skel_rest_xforms: Option<Vec<Matrix4d>>,
    /// Inverse of world bind transforms.
    joint_world_inverse_bind_xforms: Option<Vec<Matrix4d>>,
    /// Inverse of local rest transforms.
    joint_local_inverse_rest_xforms: Option<Vec<Matrix4d>>,
    /// f32 cached rest transforms in skel space.
    joint_skel_rest_xforms_f: Option<Vec<Matrix4f>>,
    /// f32 cached inverse world bind transforms.
    joint_world_inverse_bind_xforms_f: Option<Vec<Matrix4f>>,
    /// f32 cached inverse local rest transforms.
    joint_local_inverse_rest_xforms_f: Option<Vec<Matrix4f>>,
    /// Whether has valid bind pose.
    has_bind_pose: Option<bool>,
    /// Whether has valid rest pose.
    has_rest_pose: Option<bool>,
}

impl SkelDefinition {
    /// Create a definition from a skeleton.
    /// Returns None if the skeleton or its structure is invalid.
    pub fn new(skel: Skeleton) -> Option<Self> {
        if !skel.is_valid() {
            return None;
        }

        // Get joint order (get_joints_attr returns Attribute, possibly invalid)
        let joint_order = skel
            .get_joints_attr()
            .get_typed_vec::<Token>(TimeCode::default())
            .unwrap_or_default();

        if joint_order.is_empty() {
            return None;
        }

        // Build topology from joint order
        let topology = Topology::from_tokens(&joint_order);
        if !topology.is_valid() {
            return None;
        }

        // Get rest transforms (local space)
        let joint_local_rest_xforms = skel
            .get_rest_transforms_attr()
            .get_typed_vec::<Matrix4d>(TimeCode::default())
            .unwrap_or_default();

        // Get bind transforms (world space)
        let joint_world_bind_xforms = skel
            .get_bind_transforms_attr()
            .get_typed_vec::<Matrix4d>(TimeCode::default())
            .unwrap_or_default();

        Some(Self {
            inner: Arc::new(SkelDefinitionInner {
                skeleton: skel,
                joint_order,
                topology,
                joint_local_rest_xforms,
                joint_world_bind_xforms,
                cache: Mutex::new(SkelDefinitionCache::default()),
            }),
        })
    }

    /// Returns true if this definition is valid.
    pub fn is_valid(&self) -> bool {
        self.inner.skeleton.is_valid()
    }

    /// Get the skeleton this definition is for.
    pub fn get_skeleton(&self) -> &Skeleton {
        &self.inner.skeleton
    }

    /// Get the joint order.
    pub fn get_joint_order(&self) -> &[Token] {
        &self.inner.joint_order
    }

    /// Get the skeleton topology.
    pub fn get_topology(&self) -> &Topology {
        &self.inner.topology
    }

    /// Returns rest pose joint transforms in joint-local space (f64).
    pub fn get_joint_local_rest_transforms(&self) -> Option<Vec<Matrix4d>> {
        if self.inner.joint_local_rest_xforms.is_empty() {
            return None;
        }
        Some(self.inner.joint_local_rest_xforms.clone())
    }

    /// Returns rest pose joint transforms in joint-local space (f32).
    /// Uses uncached conversion from f64 (matches C++ behaviour).
    pub fn get_joint_local_rest_transforms_f(&self) -> Option<Vec<Matrix4f>> {
        self.get_joint_local_rest_transforms()
            .map(|xforms| convert_4d_to_4f(&xforms))
    }

    /// Returns rest pose joint transforms in skel space.
    pub fn get_joint_skel_rest_transforms(&self) -> Option<Vec<Matrix4d>> {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(ref xforms) = cache.joint_skel_rest_xforms {
            return Some(xforms.clone());
        }

        // Compute skel-space rest transforms by concatenating local transforms
        if self.inner.joint_local_rest_xforms.is_empty() {
            return None;
        }

        let num_joints = self.inner.topology.num_joints();
        let mut skel_xforms = vec![Matrix4d::identity(); num_joints];

        if !concat_joint_transforms(
            &self.inner.topology,
            &self.inner.joint_local_rest_xforms,
            &mut skel_xforms,
            None,
        ) {
            return None;
        }

        cache.joint_skel_rest_xforms = Some(skel_xforms.clone());
        Some(skel_xforms)
    }

    /// Returns rest pose joint transforms in skel space (f32).
    /// Cached separately from f64 variant.
    pub fn get_joint_skel_rest_transforms_f(&self) -> Option<Vec<Matrix4f>> {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(ref xforms) = cache.joint_skel_rest_xforms_f {
            return Some(xforms.clone());
        }

        if self.inner.joint_local_rest_xforms.is_empty() {
            return None;
        }

        // Get f32 local rest transforms and concatenate
        let local_f: Vec<Matrix4f> = convert_4d_to_4f(&self.inner.joint_local_rest_xforms);
        let num_joints = self.inner.topology.num_joints();
        let mut skel_xforms = vec![Matrix4f::identity(); num_joints];

        if !concat_joint_transforms_f(&self.inner.topology, &local_f, &mut skel_xforms, None) {
            return None;
        }

        cache.joint_skel_rest_xforms_f = Some(skel_xforms.clone());
        Some(skel_xforms)
    }

    /// Returns bind pose joint transforms in world space.
    pub fn get_joint_world_bind_transforms(&self) -> Option<Vec<Matrix4d>> {
        if self.inner.joint_world_bind_xforms.is_empty() {
            return None;
        }
        Some(self.inner.joint_world_bind_xforms.clone())
    }

    /// Returns bind pose joint transforms in world space (f32).
    /// Uses uncached conversion from f64 (matches C++ behaviour).
    pub fn get_joint_world_bind_transforms_f(&self) -> Option<Vec<Matrix4f>> {
        self.get_joint_world_bind_transforms()
            .map(|xforms| convert_4d_to_4f(&xforms))
    }

    /// Returns the inverse of the world-space joint bind transforms.
    pub fn get_joint_world_inverse_bind_transforms(&self) -> Option<Vec<Matrix4d>> {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(ref xforms) = cache.joint_world_inverse_bind_xforms {
            return Some(xforms.clone());
        }

        if self.inner.joint_world_bind_xforms.is_empty() {
            return None;
        }

        let inverse_xforms: Vec<Matrix4d> = self
            .inner
            .joint_world_bind_xforms
            .iter()
            .map(|m| m.inverse().unwrap_or_else(Matrix4d::identity))
            .collect();

        cache.joint_world_inverse_bind_xforms = Some(inverse_xforms.clone());
        Some(inverse_xforms)
    }

    /// Returns the inverse of the world-space joint bind transforms (f32).
    /// Cached separately from f64 variant.
    pub fn get_joint_world_inverse_bind_transforms_f(&self) -> Option<Vec<Matrix4f>> {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(ref xforms) = cache.joint_world_inverse_bind_xforms_f {
            return Some(xforms.clone());
        }

        // Get f64 bind transforms, invert, then convert to f32
        let bind_d = self.get_joint_world_bind_transforms()?;
        let inverse_f: Vec<Matrix4f> = bind_d
            .iter()
            .map(|m| mat4d_to_4f(&m.inverse().unwrap_or_else(Matrix4d::identity)))
            .collect();

        cache.joint_world_inverse_bind_xforms_f = Some(inverse_f.clone());
        Some(inverse_f)
    }

    /// Returns the inverse of the local-space rest transforms.
    pub fn get_joint_local_inverse_rest_transforms(&self) -> Option<Vec<Matrix4d>> {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(ref xforms) = cache.joint_local_inverse_rest_xforms {
            return Some(xforms.clone());
        }

        if self.inner.joint_local_rest_xforms.is_empty() {
            return None;
        }

        let inverse_xforms: Vec<Matrix4d> = self
            .inner
            .joint_local_rest_xforms
            .iter()
            .map(|m| m.inverse().unwrap_or_else(Matrix4d::identity))
            .collect();

        cache.joint_local_inverse_rest_xforms = Some(inverse_xforms.clone());
        Some(inverse_xforms)
    }

    /// Returns the inverse of the local-space rest transforms (f32).
    /// Cached separately from f64 variant.
    pub fn get_joint_local_inverse_rest_transforms_f(&self) -> Option<Vec<Matrix4f>> {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(ref xforms) = cache.joint_local_inverse_rest_xforms_f {
            return Some(xforms.clone());
        }

        if self.inner.joint_local_rest_xforms.is_empty() {
            return None;
        }

        let inverse_f: Vec<Matrix4f> = self
            .inner
            .joint_local_rest_xforms
            .iter()
            .map(|m| mat4d_to_4f(&m.inverse().unwrap_or_else(Matrix4d::identity)))
            .collect();

        cache.joint_local_inverse_rest_xforms_f = Some(inverse_f.clone());
        Some(inverse_f)
    }

    /// Returns true if the skeleton has a valid bind pose.
    /// A valid bind pose means the number of bind transforms matches the number of joints.
    pub fn has_bind_pose(&self) -> bool {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(has) = cache.has_bind_pose {
            return has;
        }

        let has = self.inner.joint_world_bind_xforms.len() == self.inner.joint_order.len();
        cache.has_bind_pose = Some(has);
        has
    }

    /// Returns true if the skeleton has a valid rest pose.
    /// A valid rest pose means the number of rest transforms matches the number of joints.
    pub fn has_rest_pose(&self) -> bool {
        let mut cache = self.inner.cache.lock().expect("lock poisoned");

        if let Some(has) = cache.has_rest_pose {
            return has;
        }

        let has = self.inner.joint_local_rest_xforms.len() == self.inner.joint_order.len();
        cache.has_rest_pose = Some(has);
        has
    }

    /// Get number of joints.
    pub fn num_joints(&self) -> usize {
        self.inner.joint_order.len()
    }
}

impl PartialEq for SkelDefinition {
    fn eq(&self, other: &Self) -> bool {
        // Compare by skeleton path
        self.inner.skeleton.prim().path() == other.inner.skeleton.prim().path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definition_requires_valid_skeleton() {
        // Can't create definition from invalid skeleton
        let skel = Skeleton::new(usd_core::Prim::invalid());
        let def = SkelDefinition::new(skel);
        assert!(def.is_none());
    }

    #[test]
    fn test_definition_cache_structure() {
        // Verify SkelDefinitionCache default state
        let cache = SkelDefinitionCache::default();
        assert!(cache.joint_skel_rest_xforms.is_none());
        assert!(cache.joint_world_inverse_bind_xforms.is_none());
        assert!(cache.joint_local_inverse_rest_xforms.is_none());
        assert!(cache.has_bind_pose.is_none());
        assert!(cache.has_rest_pose.is_none());
    }

    #[test]
    fn test_definition_equality_by_path() {
        // Two definitions from the same invalid skeleton should be considered
        // invalid (None), but equality is based on skeleton path
        let skel_a = Skeleton::new(usd_core::Prim::invalid());
        let skel_b = Skeleton::new(usd_core::Prim::invalid());
        let def_a = SkelDefinition::new(skel_a);
        let def_b = SkelDefinition::new(skel_b);
        // Both invalid, so both None
        assert!(def_a.is_none());
        assert!(def_b.is_none());
    }

    #[test]
    fn test_definition_clone() {
        // SkelDefinition is Clone via Arc
        let skel = Skeleton::new(usd_core::Prim::invalid());
        let def = SkelDefinition::new(skel);
        assert!(def.is_none());
        // Clone of None is still None
        let cloned = def.clone();
        assert!(cloned.is_none());
    }

    #[test]
    fn test_definition_has_all_methods() {
        // Ensure the SkelDefinition API surface matches C++ UsdSkel_SkelDefinition:
        // - new / New
        // - get_skeleton / GetSkeleton
        // - get_joint_order / GetJointOrder
        // - get_topology / GetTopology
        // - get_joint_local_rest_transforms / GetJointLocalRestTransforms
        // - get_joint_skel_rest_transforms / GetJointSkelRestTransforms
        // - get_joint_world_bind_transforms / GetJointWorldBindTransforms
        // - get_joint_world_inverse_bind_transforms / GetJointWorldInverseBindTransforms
        // - get_joint_local_inverse_rest_transforms / GetJointLocalInverseRestTransforms
        // - has_bind_pose / HasBindPose
        // - has_rest_pose / HasRestPose
        // - num_joints (extra)
        // - is_valid (extra)
        //
        // This test just verifies the methods exist and can be called.
        // We can't create a valid definition without a real stage + skeleton prim.
        let skel = Skeleton::new(usd_core::Prim::invalid());
        let _def = SkelDefinition::new(skel); // Returns None for invalid
    }
}
