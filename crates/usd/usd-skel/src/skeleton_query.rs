//! UsdSkelSkeletonQuery - primary interface to reading bound skeleton data.
//!
//! Port of pxr/usd/usdSkel/skeletonQuery.h/cpp

use super::anim_mapper::AnimMapper;
use super::anim_query::AnimQuery;
use super::skel_definition::SkelDefinition;
use super::skeleton::Skeleton;
use super::topology::Topology;
use super::utils::concat_joint_transforms;
use usd_core::Prim;
use usd_geom::XformCache;
use usd_gf::Matrix4d;
use usd_sdf::TimeCode;
use usd_tf::Token;

/// Primary interface to reading *bound* skeleton data.
///
/// This is used to query properties such as resolved transforms and animation
/// bindings, as bound through the UsdSkelBindingAPI.
///
/// Matches C++ `UsdSkelSkeletonQuery`.
#[derive(Clone)]
pub struct SkeletonQuery {
    /// The skeleton definition (cached structure).
    definition: Option<SkelDefinition>,
    /// Animation query for bound animation.
    anim_query: AnimQuery,
    /// Mapper from animation joint order to skeleton joint order.
    anim_to_skel_mapper: AnimMapper,
}

impl Default for SkeletonQuery {
    fn default() -> Self {
        Self::new()
    }
}

impl SkeletonQuery {
    /// Create an invalid skeleton query.
    pub fn new() -> Self {
        Self {
            definition: None,
            anim_query: AnimQuery::new(),
            anim_to_skel_mapper: AnimMapper::new(),
        }
    }

    /// Create a skeleton query from a definition and optional animation query.
    pub fn from_definition(definition: SkelDefinition, anim_query: Option<AnimQuery>) -> Self {
        let anim_to_skel_mapper = if let Some(ref anim) = anim_query {
            // Create mapper from animation joint order to skeleton joint order
            let skel_order = definition.get_joint_order();
            let anim_order = anim.get_joint_order();
            AnimMapper::from_orders(anim_order, skel_order)
        } else {
            AnimMapper::new()
        };

        Self {
            definition: Some(definition),
            anim_query: anim_query.unwrap_or_default(),
            anim_to_skel_mapper,
        }
    }

    /// Return true if this query is valid.
    pub fn is_valid(&self) -> bool {
        self.definition
            .as_ref()
            .map(|d| d.is_valid())
            .unwrap_or(false)
    }

    /// Returns true if the skeleton has a valid bind pose.
    pub fn has_bind_pose(&self) -> bool {
        self.definition
            .as_ref()
            .map(|d| d.has_bind_pose())
            .unwrap_or(false)
    }

    /// Returns true if the skeleton has a valid rest pose.
    pub fn has_rest_pose(&self) -> bool {
        self.definition
            .as_ref()
            .map(|d| d.has_rest_pose())
            .unwrap_or(false)
    }

    /// Returns the underlying Skeleton primitive.
    pub fn get_prim(&self) -> Option<Prim> {
        self.definition
            .as_ref()
            .map(|d| d.get_skeleton().prim().clone())
    }

    /// Returns the bound skeleton instance.
    pub fn get_skeleton(&self) -> Option<&Skeleton> {
        self.definition.as_ref().map(|d| d.get_skeleton())
    }

    /// Returns the animation query that provides animation for the bound skeleton.
    pub fn get_anim_query(&self) -> &AnimQuery {
        &self.anim_query
    }

    /// Returns the topology of the bound skeleton.
    pub fn get_topology(&self) -> Option<&Topology> {
        self.definition.as_ref().map(|d| d.get_topology())
    }

    /// Returns a mapper for remapping from the bound animation to the Skeleton.
    pub fn get_mapper(&self) -> &AnimMapper {
        &self.anim_to_skel_mapper
    }

    /// Returns an array of joint paths describing the order and parent-child
    /// relationships of joints in the skeleton.
    pub fn get_joint_order(&self) -> Vec<Token> {
        self.definition
            .as_ref()
            .map(|d| d.get_joint_order().to_vec())
            .unwrap_or_default()
    }

    /// Compute joint transforms in joint-local space at the given time.
    ///
    /// If `at_rest` is false and an animation source is bound, local transforms
    /// defined by the animation are mapped into the skeleton's joint order.
    /// Any transforms not defined by the animation source use the transforms
    /// from the rest pose as a fallback value.
    pub fn compute_joint_local_transforms(
        &self,
        xforms: &mut Vec<Matrix4d>,
        time: &TimeCode,
        at_rest: bool,
    ) -> bool {
        let Some(def) = &self.definition else {
            return false;
        };

        if at_rest || !self.has_mappable_anim() {
            // Use rest pose directly.
            return match def.get_joint_local_rest_transforms() {
                Some(xf) => {
                    *xforms = xf;
                    true
                }
                None => false,
            };
        }

        // Sparse animation: not all target joints are provided by the animation source.
        // Must pre-fill xforms with rest transforms so unmapped joints keep their rest pose.
        // Matches C++ which calls GetJointLocalRestTransforms(xforms) before RemapTransforms.
        if self.anim_to_skel_mapper.is_sparse() {
            match def.get_joint_local_rest_transforms() {
                Some(xf) => *xforms = xf,
                None => {
                    // Sparse anim but no rest transforms - cannot proceed.
                    return false;
                }
            }
        }

        // Compute animation transforms.
        let mut anim_xforms = Vec::new();
        if self
            .anim_query
            .compute_joint_local_transforms(&mut anim_xforms, time)
        {
            // Remap animation transforms into skeleton joint order.
            // Return the result of RemapTransforms, matching C++:
            // `return _animToSkelMapper.RemapTransforms(animXforms, xforms)`
            return self
                .anim_to_skel_mapper
                .remap_transforms_4d(&anim_xforms, xforms, 1);
        } else {
            // Animation failed.
            if self.anim_to_skel_mapper.is_sparse() {
                // xforms already filled with rest transforms above - ok.
                return true;
            } else {
                // Non-sparse mapper: fall back to rest transforms.
                return match def.get_joint_local_rest_transforms() {
                    Some(xf) => {
                        *xforms = xf;
                        true
                    }
                    None => false,
                };
            }
        }
    }

    /// Compute joint transforms in skeleton space at the given time.
    ///
    /// This concatenates joint transforms as computed from ComputeJointLocalTransforms().
    /// If `at_rest` is true, any bound animation source is ignored.
    pub fn compute_joint_skel_transforms(
        &self,
        xforms: &mut Vec<Matrix4d>,
        time: &TimeCode,
        at_rest: bool,
    ) -> bool {
        let Some(def) = &self.definition else {
            return false;
        };

        if at_rest {
            // Use cached rest transforms in skel space
            if let Some(rest) = def.get_joint_skel_rest_transforms() {
                *xforms = rest;
                return true;
            }
            return false;
        }

        // Get local transforms
        let mut local_xforms = Vec::new();
        if !self.compute_joint_local_transforms(&mut local_xforms, time, false) {
            return false;
        }

        // Concatenate to get skel-space transforms
        let num_joints = def.num_joints();
        xforms.clear();
        xforms.resize(num_joints, Matrix4d::identity());

        concat_joint_transforms(def.get_topology(), &local_xforms, xforms, None)
    }

    /// Compute joint transforms which, when concatenated against the rest pose,
    /// produce joint transforms in joint-local space.
    ///
    /// Computes: restRelativeTransform where
    /// `restRelativeTransform * restTransform = jointLocalTransform`
    pub fn compute_joint_rest_relative_transforms(
        &self,
        xforms: &mut Vec<Matrix4d>,
        time: &TimeCode,
    ) -> bool {
        let Some(def) = &self.definition else {
            return false;
        };

        // Get local transforms
        let mut local_xforms = Vec::new();
        if !self.compute_joint_local_transforms(&mut local_xforms, time, false) {
            return false;
        }

        // Get inverse rest transforms
        let inverse_rest = match def.get_joint_local_inverse_rest_transforms() {
            Some(xf) => xf,
            None => return false,
        };

        // Compute: localXform * inv(restXform) = restRelativeXform
        // So: restRelativeXform * restXform = localXform
        xforms.clear();
        xforms.reserve(local_xforms.len());

        for (local, inv_rest) in local_xforms.iter().zip(inverse_rest.iter()) {
            xforms.push(*local * *inv_rest);
        }

        true
    }

    /// Compute skinning transforms.
    ///
    /// These are transforms representing the change in transformation
    /// of a joint from its rest pose, in skeleton space:
    /// `inverse(bindTransform) * jointTransform`
    ///
    /// These are the transforms usually required for skinning.
    pub fn compute_skinning_transforms(&self, xforms: &mut Vec<Matrix4d>, time: &TimeCode) -> bool {
        let Some(def) = &self.definition else {
            return false;
        };

        // Get skel-space transforms
        let mut skel_xforms = Vec::new();
        if !self.compute_joint_skel_transforms(&mut skel_xforms, time, false) {
            return false;
        }

        // Get inverse bind transforms
        let inverse_bind = match def.get_joint_world_inverse_bind_transforms() {
            Some(xf) => xf,
            None => return false,
        };

        // Compute: inv(bindXform) * skelXform
        xforms.clear();
        xforms.reserve(skel_xforms.len());

        for (skel, inv_bind) in skel_xforms.iter().zip(inverse_bind.iter()) {
            xforms.push(*inv_bind * *skel);
        }

        true
    }

    /// Compute joint transforms in world space at the given time.
    ///
    /// Skel-space transforms are post-multiplied by the local-to-world
    /// transform of the Skeleton prim.
    /// If `at_rest` is true, any bound animation source is ignored.
    ///
    /// Matches C++ `ComputeJointWorldTransforms()`.
    pub fn compute_joint_world_transforms(
        &self,
        xforms: &mut Vec<Matrix4d>,
        xf_cache: &mut XformCache,
        at_rest: bool,
    ) -> bool {
        let Some(def) = &self.definition else {
            return false;
        };

        // Compute local transforms at the cache's time
        let mut local_xforms = Vec::new();
        if !self.compute_joint_local_transforms(&mut local_xforms, &xf_cache.get_time(), at_rest) {
            return false;
        }

        // Get root world transform from prim
        let root_xform = if let Some(prim) = self.get_prim() {
            xf_cache.get_local_to_world_transform(&prim)
        } else {
            return false;
        };

        // Concatenate local transforms to skel space, then multiply by root world xform
        let num_joints = def.num_joints();
        xforms.clear();
        xforms.resize(num_joints, Matrix4d::identity());

        concat_joint_transforms(def.get_topology(), &local_xforms, xforms, Some(&root_xform))
    }

    /// Returns the world space joint transforms at bind time.
    pub fn get_joint_world_bind_transforms(&self, xforms: &mut Vec<Matrix4d>) -> bool {
        let Some(def) = &self.definition else {
            return false;
        };

        if let Some(bind) = def.get_joint_world_bind_transforms() {
            *xforms = bind;
            return true;
        }
        false
    }

    /// Get a description string.
    pub fn get_description(&self) -> String {
        if let Some(prim) = self.get_prim() {
            format!("SkeletonQuery for {}", prim.path().get_string())
        } else {
            "Invalid SkeletonQuery".to_string()
        }
    }

    /// Returns true if there is a mappable animation.
    fn has_mappable_anim(&self) -> bool {
        self.anim_query.is_valid() && !self.anim_to_skel_mapper.is_null()
    }
}

impl PartialEq for SkeletonQuery {
    fn eq(&self, other: &Self) -> bool {
        self.definition == other.definition && self.anim_query == other.anim_query
    }
}

impl std::hash::Hash for SkeletonQuery {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        if let Some(prim) = self.get_prim() {
            prim.path().hash(state);
        }
        if let Some(anim_prim) = self.anim_query.get_prim() {
            anim_prim.path().hash(state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_query() {
        let query = SkeletonQuery::new();
        assert!(!query.is_valid());
        assert!(query.get_prim().is_none());
        assert!(!query.has_bind_pose());
        assert!(!query.has_rest_pose());
    }
}
