//! pxr.UsdSkel — Python bindings for the UsdSkel skeletal animation library.
//!
//! Mirrors the C++ API: UsdSkelRoot, UsdSkelSkeleton, UsdSkelAnimation,
//! UsdSkelBindingAPI, UsdSkelBlendShape, UsdSkelCache,
//! UsdSkelSkeletonQuery, UsdSkelSkinningQuery, UsdSkelTopology,
//! UsdSkelAnimMapper, UsdSkelAnimQuery, UsdSkelBinding,
//! UsdSkelInbetweenShape, UsdSkelBlendShapeQuery.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

use usd_core::Stage;
use usd_sdf::{Path, TimeCode};
use usd_skel::{
    anim_mapper::AnimMapper,
    anim_query::AnimQuery,
    animation::SkelAnimation,
    binding::Binding,
    binding_api::BindingAPI,
    blend_shape::BlendShape,
    blend_shape_query::BlendShapeQuery,
    cache::Cache,
    inbetween_shape::InbetweenShape,
    root::SkelRoot,
    skeleton::Skeleton,
    skeleton_query::SkeletonQuery,
    skinning_query::SkinningQuery,
    topology::Topology,
};
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn path_from_str(s: &str) -> PyResult<Path> {
    Path::from_string(s).ok_or_else(|| PyValueError::new_err(format!("Invalid SdfPath: {s}")))
}

// ---------------------------------------------------------------------------
// Shared stage wrapper
// ---------------------------------------------------------------------------

#[pyclass(name = "Stage", module = "pxr_rs.UsdSkel")]
struct PyStage {
    inner: Arc<Stage>,
}

// ---------------------------------------------------------------------------
// PySkelRoot
// ---------------------------------------------------------------------------

#[pyclass(name = "Root", module = "pxr_rs.UsdSkel")]
struct PySkelRoot {
    inner: SkelRoot,
}

#[pymethods]
impl PySkelRoot {
    /// Root.Get(stage, path) -> Root
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: SkelRoot::get(&*stage.inner, &p) })
    }

    /// Root.Define(stage, path) -> Root
    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: SkelRoot::define(&*stage.inner, &p) })
    }

    /// Root.Find(prim) -> Root — find the SkelRoot at or above the given prim
    #[staticmethod]
    fn find(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at path: {path}")))?;
        Ok(Self { inner: SkelRoot::find(&prim) })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.prim().path().to_string()
    }

    fn __repr__(&self) -> String {
        format!("UsdSkel.Root('{}')", self.inner.prim().path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PySkeleton
// ---------------------------------------------------------------------------

#[pyclass(name = "Skeleton", module = "pxr_rs.UsdSkel")]
struct PySkeleton {
    inner: Skeleton,
}

#[pymethods]
impl PySkeleton {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: Skeleton::get(&*stage.inner, &p) })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: Skeleton::define(&*stage.inner, &p) })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.prim().path().to_string()
    }

    /// GetJointsAttr() -> attribute path string or None
    fn get_joints_attr(&self) -> Option<String> {
        let a = self.inner.get_joints_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    /// GetBindTransformsAttr() -> attribute path string or None
    fn get_bind_transforms_attr(&self) -> Option<String> {
        let a = self.inner.get_bind_transforms_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    /// GetRestTransformsAttr() -> attribute path string or None
    fn get_rest_transforms_attr(&self) -> Option<String> {
        let a = self.inner.get_rest_transforms_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn __repr__(&self) -> String {
        format!("UsdSkel.Skeleton('{}')", self.inner.prim().path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PySkelAnimation
// ---------------------------------------------------------------------------

#[pyclass(name = "Animation", module = "pxr_rs.UsdSkel")]
struct PySkelAnimation {
    inner: SkelAnimation,
}

#[pymethods]
impl PySkelAnimation {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: SkelAnimation::get(&*stage.inner, &p) })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: SkelAnimation::define(&*stage.inner, &p) })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.prim().path().to_string()
    }

    fn get_joints_attr(&self) -> Option<String> {
        let a = self.inner.get_joints_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn get_translations_attr(&self) -> Option<String> {
        let a = self.inner.get_translations_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn get_rotations_attr(&self) -> Option<String> {
        let a = self.inner.get_rotations_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn get_scales_attr(&self) -> Option<String> {
        let a = self.inner.get_scales_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn get_blend_shape_weights_attr(&self) -> Option<String> {
        let a = self.inner.get_blend_shape_weights_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn __repr__(&self) -> String {
        format!("UsdSkel.Animation('{}')", self.inner.prim().path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyBindingAPI
// ---------------------------------------------------------------------------

#[pyclass(name = "BindingAPI", module = "pxr_rs.UsdSkel")]
struct PyBindingAPI {
    inner: BindingAPI,
}

#[pymethods]
impl PyBindingAPI {
    /// BindingAPI.Apply(stage, path) -> BindingAPI
    #[staticmethod]
    fn apply(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        let prim = stage
            .inner
            .get_prim_at_path(&p)
            .ok_or_else(|| PyValueError::new_err(format!("No prim at path: {path}")))?;
        Ok(Self { inner: BindingAPI::apply(&prim) })
    }

    /// BindingAPI.Get(stage, path) -> BindingAPI
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: BindingAPI::get(&*stage.inner, &p) })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// GetSkeleton() -> Skeleton or None
    fn get_skeleton(&self) -> Option<PySkeleton> {
        self.inner.get_skeleton().filter(|s| s.is_valid()).map(|s| PySkeleton { inner: s })
    }

    /// GetAnimationSource() -> Animation path string or None
    fn get_animation_source(&self) -> Option<String> {
        self.inner
            .get_animation_source()
            .map(|prim| prim.path().to_string())
    }

    /// GetJointIndicesPrimvar() -> primvar attribute path or None
    fn get_joint_indices_primvar(&self) -> Option<String> {
        let pv = self.inner.get_joint_indices_primvar();
        if pv.is_defined() {
            Some(pv.get_attr().path().to_string())
        } else {
            None
        }
    }

    /// GetJointWeightsPrimvar() -> primvar attribute path or None
    fn get_joint_weights_primvar(&self) -> Option<String> {
        let pv = self.inner.get_joint_weights_primvar();
        if pv.is_defined() {
            Some(pv.get_attr().path().to_string())
        } else {
            None
        }
    }

    fn __repr__(&self) -> String {
        "UsdSkel.BindingAPI".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyBlendShape
// ---------------------------------------------------------------------------

#[pyclass(name = "BlendShape", module = "pxr_rs.UsdSkel")]
struct PyBlendShape {
    inner: BlendShape,
}

#[pymethods]
impl PyBlendShape {
    #[staticmethod]
    fn get(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: BlendShape::get(&*stage.inner, &p) })
    }

    #[staticmethod]
    fn define(stage: &PyStage, path: &str) -> PyResult<Self> {
        let p = path_from_str(path)?;
        Ok(Self { inner: BlendShape::define(&*stage.inner, &p) })
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_path(&self) -> String {
        self.inner.prim().path().to_string()
    }

    fn get_offsets_attr(&self) -> Option<String> {
        let a = self.inner.get_offsets_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn get_normal_offsets_attr(&self) -> Option<String> {
        let a = self.inner.get_normal_offsets_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn get_point_indices_attr(&self) -> Option<String> {
        let a = self.inner.get_point_indices_attr();
        if a.is_valid() { Some(a.path().to_string()) } else { None }
    }

    fn __repr__(&self) -> String {
        format!("UsdSkel.BlendShape('{}')", self.inner.prim().path().to_string())
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyCache
// ---------------------------------------------------------------------------

#[pyclass(name = "Cache", module = "pxr_rs.UsdSkel")]
struct PyCache {
    inner: Cache,
}

#[pymethods]
impl PyCache {
    #[new]
    fn new() -> Self {
        Self { inner: Cache::new() }
    }

    /// Populate(root) -> bool
    fn populate(&self, root: &PySkelRoot) -> bool {
        self.inner.populate_default(&root.inner)
    }

    /// GetSkelQuery(skeleton) -> SkeletonQuery (may be invalid)
    fn get_skel_query(&self, skeleton: &PySkeleton) -> PySkeletonQuery {
        PySkeletonQuery { inner: self.inner.get_skel_query(&skeleton.inner) }
    }

    /// GetAnimQuery(animation) -> AnimQuery (may be invalid)
    fn get_anim_query(&self, animation: &PySkelAnimation) -> PyAnimQuery {
        PyAnimQuery { inner: self.inner.get_anim_query(&animation.inner) }
    }

    fn clear(&self) {
        self.inner.clear();
    }

    fn __repr__(&self) -> String {
        "UsdSkel.Cache".to_string()
    }
}

// ---------------------------------------------------------------------------
// PySkeletonQuery
// ---------------------------------------------------------------------------

#[pyclass(name = "SkeletonQuery", module = "pxr_rs.UsdSkel")]
struct PySkeletonQuery {
    inner: SkeletonQuery,
}

#[pymethods]
impl PySkeletonQuery {
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn has_bind_pose(&self) -> bool {
        self.inner.has_bind_pose()
    }

    fn has_rest_pose(&self) -> bool {
        self.inner.has_rest_pose()
    }

    /// GetSkeleton() -> Skeleton or None
    fn get_skeleton(&self) -> Option<PySkeleton> {
        self.inner.get_skeleton().map(|s| PySkeleton { inner: s.clone() })
    }

    /// ComputeJointLocalTransforms(time=DEFAULT) -> list of flat 4x4 matrix rows (as list[list[float]])
    ///
    /// Returns a flat list of 16-element lists (column-major, matching GfMatrix4d).
    #[pyo3(signature = (time = 0.0, unset_rest_pose = false))]
    fn compute_joint_local_transforms(
        &self,
        time: f64,
        unset_rest_pose: bool,
    ) -> Vec<Vec<f64>> {
        let tc = TimeCode::new(time);
        let mut xforms = Vec::new();
        if self.inner.compute_joint_local_transforms(&mut xforms, &tc, unset_rest_pose) {
            xforms.iter().map(|m| m.as_slice().to_vec()).collect()
        } else {
            Vec::new()
        }
    }

    /// ComputeSkinningTransforms(time=DEFAULT) -> list of flat 4x4 matrices or empty
    #[pyo3(signature = (time = 0.0))]
    fn compute_skinning_transforms(&self, time: f64) -> Vec<Vec<f64>> {
        let tc = TimeCode::new(time);
        let mut xforms = Vec::new();
        if self.inner.compute_skinning_transforms(&mut xforms, &tc) {
            xforms.iter().map(|m| m.as_slice().to_vec()).collect()
        } else {
            Vec::new()
        }
    }

    fn __repr__(&self) -> String {
        "UsdSkel.SkeletonQuery".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PySkinningQuery
// ---------------------------------------------------------------------------

#[pyclass(name = "SkinningQuery", module = "pxr_rs.UsdSkel")]
struct PySkinningQuery {
    inner: SkinningQuery,
}

#[pymethods]
impl PySkinningQuery {
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// GetJointIndicesPrimvar() -> attribute path or None
    fn get_joint_indices_primvar(&self) -> Option<String> {
        self.inner
            .get_joint_indices_attr()
            .map(|a| a.path().to_string())
    }

    /// ComputeSkinnedPoints(xforms, points, time) — stub returning empty
    ///
    /// Full implementation would require passing VtArray<GfVec3f> from Python.
    fn compute_skinned_points(&self) -> bool {
        // Stub: full bridge requires float[] <-> Vec<Vec3f> conversion
        false
    }

    fn __repr__(&self) -> String {
        "UsdSkel.SkinningQuery".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyTopology
// ---------------------------------------------------------------------------

#[pyclass(name = "Topology", module = "pxr_rs.UsdSkel")]
struct PyTopology {
    inner: Topology,
}

#[pymethods]
impl PyTopology {
    /// Topology(joint_paths) — construct from list of joint path strings
    #[new]
    fn new(joint_paths: Vec<String>) -> Self {
        let tokens: Vec<Token> = joint_paths.iter().map(|s| Token::new(s)).collect();
        Self { inner: Topology::from_tokens(&tokens) }
    }

    /// GetNumJoints() -> int
    fn get_num_joints(&self) -> usize {
        self.inner.num_joints()
    }

    /// GetParent(index) -> int  (−1 means root)
    fn get_parent(&self, index: usize) -> i32 {
        self.inner.get_parent(index)
    }

    /// IsRoot(index) -> bool
    fn is_root(&self, index: usize) -> bool {
        self.inner.is_root(index)
    }

    /// Validate() -> str or None  (None = valid)
    fn validate(&self) -> Option<String> {
        self.inner.validate().err()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!("UsdSkel.Topology(num_joints={})", self.inner.num_joints())
    }
}

// ---------------------------------------------------------------------------
// PyAnimMapper
// ---------------------------------------------------------------------------

#[pyclass(name = "AnimMapper", module = "pxr_rs.UsdSkel")]
struct PyAnimMapper {
    inner: AnimMapper,
}

#[pymethods]
impl PyAnimMapper {
    #[new]
    fn new() -> Self {
        Self { inner: AnimMapper::new() }
    }

    fn is_null(&self) -> bool {
        self.inner.is_null()
    }

    fn is_identity(&self) -> bool {
        self.inner.is_identity()
    }

    fn is_sparse(&self) -> bool {
        self.inner.is_sparse()
    }

    fn size(&self) -> usize {
        self.inner.size()
    }

    fn __repr__(&self) -> String {
        "UsdSkel.AnimMapper".to_string()
    }

    fn __bool__(&self) -> bool {
        !self.inner.is_null()
    }
}

// ---------------------------------------------------------------------------
// PyAnimQuery
// ---------------------------------------------------------------------------

#[pyclass(name = "AnimQuery", module = "pxr_rs.UsdSkel")]
struct PyAnimQuery {
    inner: AnimQuery,
}

#[pymethods]
impl PyAnimQuery {
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    /// GetPrim() path string or None
    fn get_prim_path(&self) -> Option<String> {
        // AnimQuery wraps an optional SkelAnimation; path not directly exposed
        None
    }

    fn __repr__(&self) -> String {
        "UsdSkel.AnimQuery".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyBinding
// ---------------------------------------------------------------------------

#[pyclass(name = "Binding", module = "pxr_rs.UsdSkel")]
struct PyBinding {
    inner: Binding,
}

#[pymethods]
impl PyBinding {
    fn is_valid(&self) -> bool {
        self.inner.get_skeleton().is_valid()
    }

    fn get_skeleton_path(&self) -> String {
        self.inner.get_skeleton().prim().path().to_string()
    }

    fn get_skinning_target_count(&self) -> usize {
        self.inner.num_skinning_targets()
    }

    fn __repr__(&self) -> String {
        "UsdSkel.Binding".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.get_skeleton().is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyInbetweenShape
// ---------------------------------------------------------------------------

#[pyclass(name = "InbetweenShape", module = "pxr_rs.UsdSkel")]
struct PyInbetweenShape {
    inner: InbetweenShape,
}

#[pymethods]
impl PyInbetweenShape {
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_weight(&self) -> Option<f32> {
        self.inner.get_weight()
    }

    fn __repr__(&self) -> String {
        "UsdSkel.InbetweenShape".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// PyBlendShapeQuery
// ---------------------------------------------------------------------------

#[pyclass(name = "BlendShapeQuery", module = "pxr_rs.UsdSkel")]
struct PyBlendShapeQuery {
    inner: BlendShapeQuery,
}

#[pymethods]
impl PyBlendShapeQuery {
    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }

    fn get_num_blend_shapes(&self) -> usize {
        self.inner.get_num_blend_shapes()
    }

    fn get_num_sub_shapes(&self) -> usize {
        self.inner.get_num_sub_shapes()
    }

    fn __repr__(&self) -> String {
        "UsdSkel.BlendShapeQuery".to_string()
    }

    fn __bool__(&self) -> bool {
        self.inner.is_valid()
    }
}

// ---------------------------------------------------------------------------
// Tokens — mirrors UsdSkelTokens
// ---------------------------------------------------------------------------

#[pyclass(name = "Tokens", module = "pxr_rs.UsdSkel")]
struct PyTokens;

#[pymethods]
impl PyTokens {
    #[getter] fn bindTransforms(&self) -> &str { "bindTransforms" }
    #[getter] fn blendShapes(&self) -> &str { "blendShapes" }
    #[getter] fn blendShapeWeights(&self) -> &str { "blendShapeWeights" }
    #[getter] fn classicLinear(&self) -> &str { "classicLinear" }
    #[getter] fn dualQuaternion(&self) -> &str { "dualQuaternion" }
    #[getter] fn interpolateBoundary(&self) -> &str { "interpolateBoundary" }
    #[getter] fn joints(&self) -> &str { "joints" }
    #[getter] fn normalOffsets(&self) -> &str { "normalOffsets" }
    #[getter] fn offsets(&self) -> &str { "offsets" }
    #[getter] fn pointIndices(&self) -> &str { "pointIndices" }
    #[getter] fn primvarsSkelGeomBindTransform(&self) -> &str { "primvars:skel:geomBindTransform" }
    #[getter] fn primvarsSkelJointIndices(&self) -> &str { "primvars:skel:jointIndices" }
    #[getter] fn primvarsSkelJointWeights(&self) -> &str { "primvars:skel:jointWeights" }
    #[getter] fn restTransforms(&self) -> &str { "restTransforms" }
    #[getter] fn rotations(&self) -> &str { "rotations" }
    #[getter] fn scales(&self) -> &str { "scales" }
    #[getter] fn skelAnimationSource(&self) -> &str { "skel:animationSource" }
    #[getter] fn skelBindingAPI(&self) -> &str { "SkelBindingAPI" }
    #[getter] fn skelJoints(&self) -> &str { "skel:joints" }
    #[getter] fn skelSkeleton(&self) -> &str { "skel:skeleton" }
    #[getter] fn translations(&self) -> &str { "translations" }
    #[getter] fn weight(&self) -> &str { "weight" }
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStage>()?;
    m.add_class::<PySkelRoot>()?;
    m.add_class::<PySkeleton>()?;
    m.add_class::<PySkelAnimation>()?;
    m.add_class::<PyBindingAPI>()?;
    m.add_class::<PyBlendShape>()?;
    m.add_class::<PyCache>()?;
    m.add_class::<PySkeletonQuery>()?;
    m.add_class::<PySkinningQuery>()?;
    m.add_class::<PyTopology>()?;
    m.add_class::<PyAnimMapper>()?;
    m.add_class::<PyAnimQuery>()?;
    m.add_class::<PyBinding>()?;
    m.add_class::<PyInbetweenShape>()?;
    m.add_class::<PyBlendShapeQuery>()?;
    m.add_class::<PyTokens>()?;

    // Tokens singleton
    m.add("Tokens", PyTokens)?;

    Ok(())
}
