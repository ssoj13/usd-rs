
//! Implicit surface scene index.
//!
//! Can be configured to either generate mesh for implicit primitives
//! (for renderers that don't natively support them) or overload the
//! transform to account for different spine axis (for cones, capsules,
//! cylinders).

use crate::implicit_to_mesh;
use once_cell::sync::Lazy;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_gf::Matrix4d;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
    HdSampledDataSource, HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, RemovedPrimEntry, RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref,
};
use usd_hd::schema::{
    HdConeSchema, HdCylinderSchema, HdDependenciesSchema, HdDependencySchemaBuilder,
    HdLocatorDataSourceHandle, HdPathDataSourceHandle, HdXformSchema,
};
use usd_hd::tokens as hd_tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Value;

/// Tokens for implicit surface types and configuration.
pub mod tokens {
    use once_cell::sync::Lazy;
    use usd_tf::Token;

    /// Capsule implicit surface type token.
    pub static CAPSULE: Lazy<Token> = Lazy::new(|| Token::new("Capsule"));
    /// Cone implicit surface type token.
    pub static CONE: Lazy<Token> = Lazy::new(|| Token::new("Cone"));
    /// Cube implicit surface type token.
    pub static CUBE: Lazy<Token> = Lazy::new(|| Token::new("Cube"));
    /// Cylinder implicit surface type token.
    pub static CYLINDER: Lazy<Token> = Lazy::new(|| Token::new("Cylinder"));
    /// Plane implicit surface type token.
    pub static PLANE: Lazy<Token> = Lazy::new(|| Token::new("Plane"));
    /// Sphere implicit surface type token.
    pub static SPHERE: Lazy<Token> = Lazy::new(|| Token::new("Sphere"));

    // Mode values
    /// Keep as-is, renderers handle implicit surface directly.
    pub static AS_IS: Lazy<Token> = Lazy::new(|| Token::new("asIs"));
    /// Convert to mesh representation.
    pub static TO_MESH: Lazy<Token> = Lazy::new(|| Token::new("toMesh"));
    /// Apply transform correction for spine axis.
    pub static TRANSFORM: Lazy<Token> = Lazy::new(|| Token::new("transform"));
    /// Axis-to-transform mode (alias for transform, matches C++ token).
    pub static AXIS_TO_TRANSFORM: Lazy<Token> = Lazy::new(|| Token::new("axisToTransform"));

    // Configuration argument tokens
    /// Configuration argument token for capsule mode.
    pub static CAPSULE_MODE: Lazy<Token> = Lazy::new(|| Token::new("capsuleMode"));
    /// Configuration argument token for cone mode.
    pub static CONE_MODE: Lazy<Token> = Lazy::new(|| Token::new("coneMode"));
    /// Configuration argument token for cube mode.
    pub static CUBE_MODE: Lazy<Token> = Lazy::new(|| Token::new("cubeMode"));
    /// Configuration argument token for cylinder mode.
    pub static CYLINDER_MODE: Lazy<Token> = Lazy::new(|| Token::new("cylinderMode"));
    /// Configuration argument token for plane mode.
    pub static PLANE_MODE: Lazy<Token> = Lazy::new(|| Token::new("planeMode"));
    /// Configuration argument token for sphere mode.
    pub static SPHERE_MODE: Lazy<Token> = Lazy::new(|| Token::new("sphereMode"));
}

static IMPLICIT_TO_XFORM: Lazy<TfToken> = Lazy::new(|| TfToken::new("implicitToXform"));

fn compute_matrix_dependencies_data_source(
    prim_path: &SdfPath,
    schema_locator: usd_hd::data_source::HdDataSourceLocator,
) -> HdContainerDataSourceHandle {
    let matrix_token = TfToken::new("matrix");
    let xform_matrix_locator = HdXformSchema::get_default_locator().append(&matrix_token);
    let dep = HdDependencySchemaBuilder::default()
        .set_depended_on_prim_path(
            HdRetainedTypedSampledDataSource::new(prim_path.clone()) as HdPathDataSourceHandle
        )
        .set_depended_on_data_source_locator(
            HdRetainedTypedSampledDataSource::new(schema_locator) as HdLocatorDataSourceHandle
        )
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            xform_matrix_locator,
        ) as HdLocatorDataSourceHandle)
        .build();
    HdDependenciesSchema::build_retained(
        &[(*IMPLICIT_TO_XFORM).clone()],
        &[dep as HdDataSourceBaseHandle],
    )
}

/// Matrix data source for cylinder axis-to-transform overlay.
#[derive(Clone, Debug)]
struct CylinderAxisTransformMatrixDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl CylinderAxisTransformMatrixDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }

    fn get_matrix(&self, time: HdSampledDataSourceTime) -> Matrix4d {
        let xform = HdXformSchema::get_from_parent(&self.prim_ds);
        let base = xform
            .get_matrix()
            .map(|m| m.get_typed_value(time))
            .unwrap_or_else(Matrix4d::identity);
        let cyl = HdCylinderSchema::get_from_parent(&self.prim_ds);
        let axis = cyl
            .get_axis()
            .map(|a| a.get_typed_value(time))
            .unwrap_or_default();
        let adjustment = implicit_to_mesh::get_axis_adjustment_matrix(&axis);
        adjustment * base
    }
}

impl HdDataSourceBase for CylinderAxisTransformMatrixDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone()) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for CylinderAxisTransformMatrixDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(self.get_matrix(time))
    }
    fn get_contributing_sample_times(
        &self,
        _start: HdSampledDataSourceTime,
        _end: HdSampledDataSourceTime,
        _out: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        false
    }
}

impl HdTypedSampledDataSource<Matrix4d> for CylinderAxisTransformMatrixDataSource {
    fn get_typed_value(&self, time: HdSampledDataSourceTime) -> Matrix4d {
        self.get_matrix(time)
    }
}

/// Matrix data source for cone axis-to-transform overlay.
/// Includes height offset (translate 0,0,-0.5*height) before adjustment.
#[derive(Clone, Debug)]
struct ConeAxisTransformMatrixDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl ConeAxisTransformMatrixDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }

    fn get_matrix(&self, time: HdSampledDataSourceTime) -> Matrix4d {
        let xform = HdXformSchema::get_from_parent(&self.prim_ds);
        let base = xform
            .get_matrix()
            .map(|m| m.get_typed_value(time))
            .unwrap_or_else(Matrix4d::identity);
        let cone = HdConeSchema::get_from_parent(&self.prim_ds);
        let axis = cone
            .get_axis()
            .map(|a| a.get_typed_value(time))
            .unwrap_or_default();
        let height = cone
            .get_height()
            .map(|h| h.get_typed_value(time))
            .unwrap_or(1.0);
        let adjustment = implicit_to_mesh::get_axis_adjustment_matrix(&axis);
        let mut height_offset = Matrix4d::identity();
        height_offset.set_translate(&usd_gf::Vec3d::new(0.0, 0.0, -0.5 * height));
        height_offset * adjustment * base
    }
}

impl HdDataSourceBase for ConeAxisTransformMatrixDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone()) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for ConeAxisTransformMatrixDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(self.get_matrix(time))
    }
    fn get_contributing_sample_times(
        &self,
        _start: HdSampledDataSourceTime,
        _end: HdSampledDataSourceTime,
        _out: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        false
    }
}

impl HdTypedSampledDataSource<Matrix4d> for ConeAxisTransformMatrixDataSource {
    fn get_typed_value(&self, time: HdSampledDataSourceTime) -> Matrix4d {
        self.get_matrix(time)
    }
}

fn compute_cylinder_to_transformed_cylinder_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    use usd_hd::data_source::HdDataSourceBaseHandle;
    let matrix_ds = CylinderAxisTransformMatrixDataSource::new(prim_ds.clone());
    let xform_src = HdXformSchema::build_retained(
        Some(matrix_ds as Arc<dyn HdTypedSampledDataSource<Matrix4d>>),
        None,
    );
    let deps =
        compute_matrix_dependencies_data_source(prim_path, HdCylinderSchema::get_default_locator());
    let xform_token = TfToken::new("xform");
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (xform_token.clone(), xform_src as HdDataSourceBaseHandle),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

fn compute_cone_to_transformed_cone_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    use usd_hd::data_source::HdDataSourceBaseHandle;
    let matrix_ds = ConeAxisTransformMatrixDataSource::new(prim_ds.clone());
    let xform_src = HdXformSchema::build_retained(
        Some(matrix_ds as Arc<dyn HdTypedSampledDataSource<Matrix4d>>),
        None,
    );
    let deps =
        compute_matrix_dependencies_data_source(prim_path, HdConeSchema::get_default_locator());
    let xform_token = TfToken::new("xform");
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (xform_token.clone(), xform_src as HdDataSourceBaseHandle),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

/// Set of all implicit surface prim types for quick lookup.
static IMPLICIT_SURFACE_TYPES: Lazy<[&'static TfToken; 6]> = Lazy::new(|| {
    [
        &*usd_hd::tokens::RPRIM_CAPSULE,
        &*usd_hd::tokens::RPRIM_CONE,
        &*usd_hd::tokens::RPRIM_CUBE,
        &*usd_hd::tokens::RPRIM_CYLINDER,
        &*usd_hd::tokens::RPRIM_PLANE,
        &*usd_hd::tokens::RPRIM_SPHERE,
    ]
});

/// Implicit surface scene index.
///
/// Handles conversion of implicit surfaces (sphere, cube, cone, cylinder,
/// capsule, plane) to mesh or transforms them for different spine axes.
pub struct HdsiImplicitSurfaceSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    /// Mode for capsule primitives (toMesh, asIs, etc.)
    capsule_mode: TfToken,
    /// Mode for cone primitives
    cone_mode: TfToken,
    /// Mode for cube primitives
    cube_mode: TfToken,
    /// Mode for cylinder primitives
    cylinder_mode: TfToken,
    /// Mode for plane primitives
    plane_mode: TfToken,
    /// Mode for sphere primitives
    sphere_mode: TfToken,
}

impl HdsiImplicitSurfaceSceneIndex {
    /// Create a new implicit surface scene index.
    ///
    /// # Arguments
    /// * `input_scene` - Input scene index to filter
    /// * `input_args` - Configuration data source with mode tokens keyed by prim type
    ///   (capsule, cone, cube, cylinder, plane, sphere). Values: "asIs", "toMesh", "axisToTransform"
    pub fn new(
        input_scene: HdSceneIndexHandle,
        input_args: Option<HdContainerDataSourceHandle>,
    ) -> Arc<RwLock<Self>> {
        let get_mode = |key: &TfToken| {
            let args = match &input_args {
                Some(a) => a,
                None => return tokens::AS_IS.clone(),
            };
            if let Some(ds) = args.get(key) {
                if let Some(sampled) = ds.as_sampled() {
                    let v = sampled.get_value(0.0);
                    if let Some(t) = v.get::<TfToken>() {
                        return t.clone();
                    }
                }
            }
            tokens::AS_IS.clone()
        };

        let capsule_mode = get_mode(&*hd_tokens::RPRIM_CAPSULE);
        let cone_mode = get_mode(&*hd_tokens::RPRIM_CONE);
        let cube_mode = get_mode(&*hd_tokens::RPRIM_CUBE);
        let cylinder_mode = get_mode(&*hd_tokens::RPRIM_CYLINDER);
        let plane_mode = get_mode(&*hd_tokens::RPRIM_PLANE);
        let sphere_mode = get_mode(&*hd_tokens::RPRIM_SPHERE);

        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            capsule_mode,
            cone_mode,
            cube_mode,
            cylinder_mode,
            plane_mode,
            sphere_mode,
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    /// Create with explicit mode configuration.
    ///
    /// # Arguments
    /// * `input_scene` - Input scene index to filter
    /// * `default_mode` - Default mode for all implicit surfaces
    pub fn new_with_mode(
        input_scene: HdSceneIndexHandle,
        default_mode: TfToken,
    ) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            capsule_mode: default_mode.clone(),
            cone_mode: default_mode.clone(),
            cube_mode: default_mode.clone(),
            cylinder_mode: default_mode.clone(),
            plane_mode: default_mode.clone(),
            sphere_mode: default_mode,
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }

    /// Check if prim type is an implicit surface.
    fn is_implicit_surface(&self, prim_type: &TfToken) -> bool {
        IMPLICIT_SURFACE_TYPES.iter().any(|t| *t == prim_type)
    }

    /// Get mode for prim type.
    fn get_mode_for_type(&self, prim_type: &TfToken) -> &TfToken {
        if *prim_type == *usd_hd::tokens::RPRIM_CAPSULE {
            &self.capsule_mode
        } else if *prim_type == *usd_hd::tokens::RPRIM_CONE {
            &self.cone_mode
        } else if *prim_type == *usd_hd::tokens::RPRIM_CUBE {
            &self.cube_mode
        } else if *prim_type == *usd_hd::tokens::RPRIM_CYLINDER {
            &self.cylinder_mode
        } else if *prim_type == *usd_hd::tokens::RPRIM_PLANE {
            &self.plane_mode
        } else if *prim_type == *usd_hd::tokens::RPRIM_SPHERE {
            &self.sphere_mode
        } else {
            &tokens::AS_IS
        }
    }

    /// Check if mode requires mesh conversion.
    fn mode_requires_mesh(&self, mode: &TfToken) -> bool {
        *mode == *tokens::TO_MESH
    }

    /// Check if mode requires transform adjustment (axisToTransform).
    fn mode_requires_transform(&self, mode: &TfToken) -> bool {
        *mode == *tokens::AXIS_TO_TRANSFORM || *mode == *tokens::TRANSFORM
    }
}

impl HdSceneIndexBase for HdsiImplicitSurfaceSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        if let Some(input) = self.base.get_input_scene() {
            {
                let input_locked = input.read();
                let mut prim = input_locked.get_prim(prim_path);

                if !self.is_implicit_surface(&prim.prim_type) {
                    return prim;
                }

                let mode = self.get_mode_for_type(&prim.prim_type);

                if self.mode_requires_mesh(mode) {
                    let ds = prim.data_source.as_ref().cloned();
                    if let Some(prim_ds) = ds {
                        let overlay = if prim.prim_type == *hd_tokens::RPRIM_CUBE {
                            implicit_to_mesh::compute_cube_to_mesh_prim_data_source(
                                prim_path, &prim_ds,
                            )
                        } else if prim.prim_type == *hd_tokens::RPRIM_SPHERE {
                            implicit_to_mesh::compute_sphere_to_mesh_prim_data_source(
                                prim_path, &prim_ds,
                            )
                        } else if prim.prim_type == *hd_tokens::RPRIM_CONE {
                            implicit_to_mesh::compute_cone_to_mesh_prim_data_source(
                                prim_path, &prim_ds,
                            )
                        } else if prim.prim_type == *hd_tokens::RPRIM_CYLINDER {
                            implicit_to_mesh::compute_cylinder_to_mesh_prim_data_source(
                                prim_path, &prim_ds,
                            )
                        } else if prim.prim_type == *hd_tokens::RPRIM_CAPSULE {
                            implicit_to_mesh::compute_capsule_to_mesh_prim_data_source(
                                prim_path, &prim_ds,
                            )
                        } else if prim.prim_type == *hd_tokens::RPRIM_PLANE {
                            implicit_to_mesh::compute_plane_to_mesh_prim_data_source(
                                prim_path, &prim_ds,
                            )
                        } else {
                            prim_ds
                        };
                        prim.prim_type = (*hd_tokens::RPRIM_MESH).clone();
                        prim.data_source = Some(overlay);
                    }
                } else if self.mode_requires_transform(mode) {
                    if prim.prim_type == *hd_tokens::RPRIM_CONE {
                        if let Some(ref prim_ds) = prim.data_source {
                            prim.data_source =
                                Some(compute_cone_to_transformed_cone_prim_data_source(
                                    prim_path, prim_ds,
                                ));
                        }
                    } else if prim.prim_type == *hd_tokens::RPRIM_CYLINDER {
                        if let Some(ref prim_ds) = prim.data_source {
                            prim.data_source =
                                Some(compute_cylinder_to_transformed_cylinder_prim_data_source(
                                    prim_path, prim_ds,
                                ));
                        }
                    }
                }
                return prim;
            }
        }

        HdSceneIndexPrim::default()
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &usd_hd::scene_index::HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {
        // Forward to input
    }

    fn get_display_name(&self) -> String {
        "HdsiImplicitSurfaceSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiImplicitSurfaceSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        if !self.base.base().is_observed() {
            return;
        }
        let mut new_entries: Vec<AddedPrimEntry> = entries.to_vec();
        for entry in &mut new_entries {
            if self.is_implicit_surface(&entry.prim_type)
                && self.mode_requires_mesh(self.get_mode_for_type(&entry.prim_type))
            {
                entry.prim_type = (*hd_tokens::RPRIM_MESH).clone();
            }
        }
        self.base.forward_prims_added(self, &new_entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        if entries.len() >= 1000 {
            let first = entries
                .first()
                .map(|e| e.prim_path.to_string())
                .unwrap_or_default();
            eprintln!(
                "[implicit_surface] on_prims_dirtied in={} sender={} first={}",
                entries.len(),
                sender.get_display_name(),
                first,
            );
        }
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
