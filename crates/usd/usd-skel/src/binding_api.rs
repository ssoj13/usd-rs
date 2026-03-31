//! UsdSkelBindingAPI - provides API for authoring and extracting all the
//! skinning-related data that lives in the "geometry hierarchy" of prims.
//!
//! Port of pxr/usd/usdSkel/bindingAPI.h/cpp

use super::skeleton::Skeleton;
use super::tokens::tokens;
use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, Relationship, Stage};
use usd_geom::{Primvar, PrimvarsAPI, tokens::usd_geom_tokens};
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// BindingAPI
// ============================================================================

/// Provides API for authoring and extracting all the skinning-related
/// data that lives in the "geometry hierarchy" of prims and models that want
/// to be skeletally deformed.
///
/// Matches C++ `UsdSkelBindingAPI`.
#[derive(Debug, Clone)]
pub struct BindingAPI {
    /// The wrapped prim.
    prim: Prim,
}

impl BindingAPI {
    /// Creates a BindingAPI schema from a prim.
    ///
    /// Matches C++ `UsdSkelBindingAPI(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Creates an invalid BindingAPI schema.
    pub fn invalid() -> Self {
        Self {
            prim: Prim::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        &self.prim
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        tokens().skel_binding_api.clone()
    }

    /// Return a BindingAPI holding the prim adhering to this schema at path on stage.
    ///
    /// Matches C++ `UsdSkelBindingAPI::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    ///
    /// Matches C++ `CanApply(const UsdPrim &prim, std::string *whyNot)`.
    pub fn can_apply(prim: &Prim) -> Result<(), String> {
        if !prim.is_valid() {
            return Err("Invalid prim".to_string());
        }
        // SkelBindingAPI can be applied to any valid prim
        Ok(())
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// Matches C++ `Apply(const UsdPrim &prim)`.
    pub fn apply(prim: &Prim) -> Self {
        if !prim.is_valid() {
            return Self::invalid();
        }
        // Register the API schema on the prim
        prim.apply_api(&usd_tf::Token::new("SkelBindingAPI"));
        Self::new(prim.clone())
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            tokens().primvars_skel_skinning_method.clone(),
            tokens().primvars_skel_geom_bind_transform.clone(),
            tokens().skel_joints.clone(),
            tokens().primvars_skel_joint_indices.clone(),
            tokens().primvars_skel_joint_weights.clone(),
            tokens().skel_blend_shapes.clone(),
        ]
    }

    // ========================================================================
    // SKINNINGMETHOD
    // ========================================================================

    /// Returns the skinningMethod attribute.
    ///
    /// Matches C++ `GetSkinningMethodAttr()`.
    pub fn get_skinning_method_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().primvars_skel_skinning_method.as_str())
    }

    // ========================================================================
    // GEOMBINDTRANSFORM
    // ========================================================================

    /// Returns the geomBindTransform attribute.
    ///
    /// Matches C++ `GetGeomBindTransformAttr()`.
    pub fn get_geom_bind_transform_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().primvars_skel_geom_bind_transform.as_str())
    }

    // ========================================================================
    // JOINTS
    // ========================================================================

    /// Returns the joints attribute.
    ///
    /// Matches C++ `GetJointsAttr()`.
    pub fn get_joints_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().skel_joints.as_str())
    }

    // ========================================================================
    // JOINTINDICES
    // ========================================================================

    /// Returns the jointIndices attribute.
    ///
    /// Matches C++ `GetJointIndicesAttr()`.
    pub fn get_joint_indices_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().primvars_skel_joint_indices.as_str())
    }

    // ========================================================================
    // JOINTWEIGHTS
    // ========================================================================

    /// Returns the jointWeights attribute.
    ///
    /// Matches C++ `GetJointWeightsAttr()`.
    pub fn get_joint_weights_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(tokens().primvars_skel_joint_weights.as_str())
    }

    // ========================================================================
    // BLENDSHAPES
    // ========================================================================

    /// Returns the blendShapes attribute.
    ///
    /// Matches C++ `GetBlendShapesAttr()`.
    pub fn get_blend_shapes_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(tokens().skel_blend_shapes.as_str())
    }

    // ========================================================================
    // RELATIONSHIPS
    // ========================================================================

    /// Returns the animationSource relationship.
    ///
    /// Matches C++ `GetAnimationSourceRel()`.
    pub fn get_animation_source_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(tokens().skel_animation_source.as_str())
    }

    /// Returns the skeleton relationship.
    ///
    /// Matches C++ `GetSkeletonRel()`.
    pub fn get_skeleton_rel(&self) -> Option<Relationship> {
        self.prim.get_relationship(tokens().skel_skeleton.as_str())
    }

    /// Returns the blendShapeTargets relationship.
    ///
    /// Matches C++ `GetBlendShapeTargetsRel()`.
    pub fn get_blend_shape_targets_rel(&self) -> Option<Relationship> {
        self.prim
            .get_relationship(tokens().skel_blend_shape_targets.as_str())
    }

    // ========================================================================
    // CREATE METHODS - Attribute authoring
    // ========================================================================

    /// Create the skinningMethod attribute.
    ///
    /// Matches C++ `CreateSkinningMethodAttr()`.
    pub fn create_skinning_method_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }
        if let Some(attr) = self.get_skinning_method_attr() {
            return attr;
        }
        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));
        self.prim
            .create_attribute(
                tokens().primvars_skel_skinning_method.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the geomBindTransform attribute.
    ///
    /// Matches C++ `CreateGeomBindTransformAttr()`.
    pub fn create_geom_bind_transform_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }
        if let Some(attr) = self.get_geom_bind_transform_attr() {
            return attr;
        }
        let registry = ValueTypeRegistry::instance();
        let matrix_type = registry.find_type_by_token(&Token::new("matrix4d"));
        self.prim
            .create_attribute(
                tokens().primvars_skel_geom_bind_transform.as_str(),
                &matrix_type,
                false,
                None, // Varying (default)
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the joints attribute.
    ///
    /// Matches C++ `CreateJointsAttr()`.
    pub fn create_joints_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }
        if let Some(attr) = self.get_joints_attr() {
            return attr;
        }
        let registry = ValueTypeRegistry::instance();
        let token_array_type = registry.find_type_by_token(&Token::new("token[]"));
        self.prim
            .create_attribute(
                tokens().skel_joints.as_str(),
                &token_array_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the jointIndices attribute.
    ///
    /// Matches C++ `CreateJointIndicesAttr()`.
    pub fn create_joint_indices_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }
        if let Some(attr) = self.get_joint_indices_attr() {
            return attr;
        }
        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));
        self.prim
            .create_attribute(
                tokens().primvars_skel_joint_indices.as_str(),
                &int_array_type,
                false,
                None, // Varying
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the jointWeights attribute.
    ///
    /// Matches C++ `CreateJointWeightsAttr()`.
    pub fn create_joint_weights_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }
        if let Some(attr) = self.get_joint_weights_attr() {
            return attr;
        }
        let registry = ValueTypeRegistry::instance();
        let float_array_type = registry.find_type_by_token(&Token::new("float[]"));
        self.prim
            .create_attribute(
                tokens().primvars_skel_joint_weights.as_str(),
                &float_array_type,
                false,
                None, // Varying
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the blendShapes attribute.
    ///
    /// Matches C++ `CreateBlendShapesAttr()`.
    pub fn create_blend_shapes_attr(&self) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }
        if let Some(attr) = self.get_blend_shapes_attr() {
            return attr;
        }
        let registry = ValueTypeRegistry::instance();
        let token_array_type = registry.find_type_by_token(&Token::new("token[]"));
        self.prim
            .create_attribute(
                tokens().skel_blend_shapes.as_str(),
                &token_array_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Create the animationSource relationship.
    ///
    /// Matches C++ `CreateAnimationSourceRel()`.
    pub fn create_animation_source_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(tokens().skel_animation_source.as_str(), false)
    }

    /// Create the skeleton relationship.
    ///
    /// Matches C++ `CreateSkeletonRel()`.
    pub fn create_skeleton_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(tokens().skel_skeleton.as_str(), false)
    }

    /// Create the blendShapeTargets relationship.
    ///
    /// Matches C++ `CreateBlendShapeTargetsRel()`.
    pub fn create_blend_shape_targets_rel(&self) -> Option<Relationship> {
        self.prim
            .create_relationship(tokens().skel_blend_shape_targets.as_str(), false)
    }

    // ========================================================================
    // PRIMVAR convenience methods
    // ========================================================================

    /// Get the jointIndices attribute as a Primvar.
    ///
    /// Matches C++ `GetJointIndicesPrimvar()`.
    pub fn get_joint_indices_primvar(&self) -> Primvar {
        let attr = self
            .prim
            .get_attribute(tokens().primvars_skel_joint_indices.as_str())
            .unwrap_or_else(Attribute::invalid);
        Primvar::new(attr)
    }

    /// Create the jointIndices primvar with the given interpolation and element size.
    ///
    /// If `constant` is true, uses 'constant' interpolation (rigid deformation).
    /// Otherwise uses 'vertex' interpolation (per-point influences).
    ///
    /// Matches C++ `CreateJointIndicesPrimvar(bool constant, int elementSize)`.
    pub fn create_joint_indices_primvar(&self, constant: bool, element_size: i32) -> Primvar {
        let geom_tokens = usd_geom_tokens();
        let interp = if constant {
            &geom_tokens.constant
        } else {
            &geom_tokens.vertex
        };
        let registry = ValueTypeRegistry::instance();
        let int_array_type = registry.find_type_by_token(&Token::new("int[]"));
        let api = PrimvarsAPI::new(self.prim.clone());
        api.create_primvar(
            &tokens().primvars_skel_joint_indices,
            &int_array_type,
            Some(interp),
            element_size,
        )
    }

    /// Get the jointWeights attribute as a Primvar.
    ///
    /// Matches C++ `GetJointWeightsPrimvar()`.
    pub fn get_joint_weights_primvar(&self) -> Primvar {
        let attr = self
            .prim
            .get_attribute(tokens().primvars_skel_joint_weights.as_str())
            .unwrap_or_else(Attribute::invalid);
        Primvar::new(attr)
    }

    /// Create the jointWeights primvar with the given interpolation and element size.
    ///
    /// If `constant` is true, uses 'constant' interpolation (rigid deformation).
    /// Otherwise uses 'vertex' interpolation (per-point influences).
    ///
    /// Matches C++ `CreateJointWeightsPrimvar(bool constant, int elementSize)`.
    pub fn create_joint_weights_primvar(&self, constant: bool, element_size: i32) -> Primvar {
        let geom_tokens = usd_geom_tokens();
        let interp = if constant {
            &geom_tokens.constant
        } else {
            &geom_tokens.vertex
        };
        let registry = ValueTypeRegistry::instance();
        let float_array_type = registry.find_type_by_token(&Token::new("float[]"));
        let api = PrimvarsAPI::new(self.prim.clone());
        api.create_primvar(
            &tokens().primvars_skel_joint_weights,
            &float_array_type,
            Some(interp),
            element_size,
        )
    }

    /// Set rigid joint influence: creates constant primvars with a single
    /// joint index and weight for rigid skinning.
    ///
    /// Matches C++ `SetRigidJointInfluence(int jointIndex, float weight)`.
    pub fn set_rigid_joint_influence(&self, joint_index: i32, weight: f32) -> bool {
        let indices_pv = self.create_joint_indices_primvar(true, 1);
        let weights_pv = self.create_joint_weights_primvar(true, 1);

        if joint_index < 0 {
            eprintln!("Warning: Invalid jointIndex '{}'", joint_index);
            return false;
        }

        let indices_attr = indices_pv.get_attr();
        let weights_attr = weights_pv.get_attr();

        indices_attr.set(Value::from(vec![joint_index]), usd_sdf::TimeCode::default())
            && weights_attr.set(Value::from(vec![weight]), usd_sdf::TimeCode::default())
    }

    // ========================================================================
    // CUSTOM CODE
    // ========================================================================

    /// Convenience method to query the Skeleton bound on this prim.
    ///
    /// Matches C++ `GetSkeleton(UsdSkelSkeleton* skel)`.
    pub fn get_skeleton(&self) -> Option<Skeleton> {
        let rel = self.get_skeleton_rel()?;
        let targets = rel.get_targets();
        if targets.is_empty() {
            return None;
        }

        if let Some(stage) = self.prim.stage() {
            let skel = Skeleton::get(&stage, &targets[0]);
            if skel.is_valid() {
                return Some(skel);
            }
        }

        None
    }

    /// Convenience method to query the animation source bound on this prim.
    ///
    /// Matches C++ `GetAnimationSource(UsdPrim* prim)`.
    pub fn get_animation_source(&self) -> Option<Prim> {
        let rel = self.get_animation_source_rel()?;
        let targets = rel.get_targets();
        if targets.is_empty() {
            return None;
        }

        if let Some(stage) = self.prim.stage() {
            return stage.get_prim_at_path(&targets[0]);
        }

        None
    }

    /// Returns the skeleton bound at this prim, or one of its ancestors.
    ///
    /// Matches C++ `GetInheritedSkeleton()`.
    pub fn get_inherited_skeleton(&self) -> Option<Skeleton> {
        let mut prim = self.prim.clone();
        // C++: walk up until pseudo-root, only check prims that have the API applied
        while prim.is_valid() && !prim.is_pseudo_root() {
            if prim.has_api(&tokens().skel_binding_api) {
                let binding = BindingAPI::new(prim.clone());
                if let Some(skel) = binding.get_skeleton() {
                    return Some(skel);
                }
            }
            prim = prim.parent();
        }
        None
    }

    /// Returns the animation source bound at this prim, or one of its ancestors.
    ///
    /// Matches C++ `GetInheritedAnimationSource()`.
    pub fn get_inherited_animation_source(&self) -> Option<Prim> {
        let mut prim = self.prim.clone();
        // C++: walk up until pseudo-root, only check prims that have the API applied
        while prim.is_valid() && !prim.is_pseudo_root() {
            if prim.has_api(&tokens().skel_binding_api) {
                let binding = BindingAPI::new(prim.clone());
                if let Some(anim_source) = binding.get_animation_source() {
                    return Some(anim_source);
                }
            }
            prim = prim.parent();
        }
        None
    }

    /// Validate an array of joint indices.
    ///
    /// Matches C++ `ValidateJointIndices(TfSpan<const int> indices, size_t numJoints, std::string* reason)`.
    pub fn validate_joint_indices(indices: &[i32], num_joints: usize) -> Result<(), String> {
        for (i, &idx) in indices.iter().enumerate() {
            if idx < 0 {
                return Err(format!(
                    "Joint index at position {} is negative ({})",
                    i, idx
                ));
            }
            if (idx as usize) >= num_joints {
                return Err(format!(
                    "Joint index at position {} ({}) exceeds joint count ({})",
                    i, idx, num_joints
                ));
            }
        }
        Ok(())
    }
}

impl PartialEq for BindingAPI {
    fn eq(&self, other: &Self) -> bool {
        self.prim == other.prim
    }
}

impl Eq for BindingAPI {}

impl std::hash::Hash for BindingAPI {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prim.hash(state);
    }
}

impl Default for BindingAPI {
    fn default() -> Self {
        Self::invalid()
    }
}
