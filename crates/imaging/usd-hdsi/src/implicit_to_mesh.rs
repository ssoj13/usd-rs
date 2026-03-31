//! Implicit surface to mesh conversion.
//!
//! Converts cube, sphere, cone, cylinder, capsule, plane primitives to mesh
//! representation with topology and points primvars. Port of C++ _CubeToMesh,
//! _SphereToMesh, etc. in implicitSurfaceSceneIndex.cpp.

use std::sync::Arc;
use usd_geom_util::{CapsuleMeshGenerator, ConeMeshGenerator, CuboidMeshGenerator};
use usd_geom_util::{CylinderMeshGenerator, PlaneMeshGenerator, SphereMeshGenerator};
use usd_gf::{Matrix4d, Vec3f};
use usd_hd::data_source::{
    HdBlockDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
    HdSampledDataSource, HdSampledDataSourceTime, HdTypedSampledDataSource,
};
use usd_hd::schema::mesh::{
    HdBoolDataSourceHandle, HdTokenDataSourceHandle as HdMeshTokenDataSourceHandle,
};
use usd_hd::schema::mesh_topology::{
    HdIntArrayDataSourceHandle, HdTokenDataSourceHandle as HdMeshTopologyTokenDataSourceHandle,
};
use usd_hd::schema::{
    HdCapsuleSchema, HdConeSchema, HdCubeSchema, HdCylinderSchema, HdDependenciesSchema,
    HdDependencySchemaBuilder, HdMeshSchema, HdMeshTopologySchema, HdPlaneSchema, HdPrimvarsSchema,
    HdSphereSchema, PRIMVAR_VALUE,
};
use usd_px_osd::tokens as px_osd_tokens;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;
use usd_vt::Array;
use usd_vt::Value;

const NUM_RADIAL: usize = 10;
const NUM_AXIAL: usize = 10;
const NUM_CAP_AXIAL: usize = 4;

use once_cell::sync::Lazy;

static POINT: Lazy<TfToken> = Lazy::new(|| TfToken::new("point"));
static IMPLICIT_TO_MESH: Lazy<TfToken> = Lazy::new(|| TfToken::new("implicitToMesh"));
static VERTEX: Lazy<TfToken> = Lazy::new(|| TfToken::new("vertex"));
static POINTS: Lazy<TfToken> = Lazy::new(|| TfToken::new("points"));

/// Returns the axis adjustment matrix for implicit surfaces (cone, cylinder).
/// Maps from axis-aligned space (X/Y/Z spine) to canonical Z-up. Used by
/// axisToTransform mode in implicit surface scene index.
pub fn get_axis_adjustment_matrix(axis: &TfToken) -> Matrix4d {
    let (u, v, spine) = if axis == "X" {
        (
            usd_gf::Vec4d::new(0.0, 1.0, 0.0, 0.0),
            usd_gf::Vec4d::new(0.0, 0.0, 1.0, 0.0),
            usd_gf::Vec4d::new(1.0, 0.0, 0.0, 0.0),
        )
    } else if axis == "Y" {
        (
            usd_gf::Vec4d::new(0.0, 0.0, 1.0, 0.0),
            usd_gf::Vec4d::new(1.0, 0.0, 0.0, 0.0),
            usd_gf::Vec4d::new(0.0, 1.0, 0.0, 0.0),
        )
    } else {
        (
            usd_gf::Vec4d::new(1.0, 0.0, 0.0, 0.0),
            usd_gf::Vec4d::new(0.0, 1.0, 0.0, 0.0),
            usd_gf::Vec4d::new(0.0, 0.0, 1.0, 0.0),
        )
    };
    let mut m = Matrix4d::identity();
    m.set_row(0, &u);
    m.set_row(1, &v);
    m.set_row(2, &spine);
    m
}

fn compute_points_dependencies(
    prim_path: &SdfPath,
    schema_locator: usd_hd::data_source::HdDataSourceLocator,
) -> HdContainerDataSourceHandle {
    let points_locator = HdPrimvarsSchema::get_points_locator().append(&*PRIMVAR_VALUE);
    let dep = HdDependencySchemaBuilder::default()
        .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::new(prim_path.clone())
            as usd_hd::schema::HdPathDataSourceHandle)
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(schema_locator)
            as usd_hd::schema::HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(points_locator)
            as usd_hd::schema::HdLocatorDataSourceHandle)
        .build();
    HdDependenciesSchema::build_retained(
        &[(*IMPLICIT_TO_MESH).clone()],
        &[dep as HdDataSourceBaseHandle],
    )
}

fn build_primvar_container(
    role: TfToken,
    interpolation: TfToken,
    value: HdDataSourceBaseHandle,
) -> HdContainerDataSourceHandle {
    use usd_hd::data_source::HdRetainedContainerDataSource;
    let role_ds = HdRetainedTypedSampledDataSource::new(role) as HdDataSourceBaseHandle;
    let interp_ds = HdRetainedTypedSampledDataSource::new(interpolation) as HdDataSourceBaseHandle;
    HdRetainedContainerDataSource::from_entries(&[
        (TfToken::new("role"), role_ds),
        (TfToken::new("interpolation"), interp_ds),
        ((*PRIMVAR_VALUE).clone(), value),
    ])
}

/// Points data source for cube - generates vertices from size.
#[derive(Debug)]
struct CubePointsDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl CubePointsDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }
}

impl HdDataSourceBase for CubePointsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_ds: self.prim_ds.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for CubePointsDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(<Self as HdTypedSampledDataSource<Vec<Vec3f>>>::get_typed_value(self, time))
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
impl HdTypedSampledDataSource<Vec<Vec3f>> for CubePointsDataSource {
    fn get_typed_value(&self, _time: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let schema = HdCubeSchema::get_from_parent(&self.prim_ds);
        let size = schema
            .get_size()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(2.0) as f32;
        CuboidMeshGenerator::generate_points_f32(size, size, size, None)
    }
}

/// Points data source for sphere.
#[derive(Debug)]
struct SpherePointsDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl SpherePointsDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }
}

impl HdDataSourceBase for SpherePointsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_ds: self.prim_ds.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for SpherePointsDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(<Self as HdTypedSampledDataSource<Vec<Vec3f>>>::get_typed_value(self, time))
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
impl HdTypedSampledDataSource<Vec<Vec3f>> for SpherePointsDataSource {
    fn get_typed_value(&self, _time: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let schema = HdSphereSchema::get_from_parent(&self.prim_ds);
        let radius = schema
            .get_radius()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        SphereMeshGenerator::generate_points_f32(NUM_RADIAL, NUM_AXIAL, radius, 360.0, None)
    }
}

/// Points data source for cone.
#[derive(Debug)]
struct ConePointsDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl ConePointsDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }
}

impl HdDataSourceBase for ConePointsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_ds: self.prim_ds.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for ConePointsDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(<Self as HdTypedSampledDataSource<Vec<Vec3f>>>::get_typed_value(self, time))
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
impl HdTypedSampledDataSource<Vec<Vec3f>> for ConePointsDataSource {
    fn get_typed_value(&self, _time: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let schema = HdConeSchema::get_from_parent(&self.prim_ds);
        let height = schema
            .get_height()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        let radius = schema
            .get_radius()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        let axis = schema
            .get_axis()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("Z"));
        let basis = get_axis_adjustment_matrix(&axis);
        ConeMeshGenerator::generate_points_f32(NUM_RADIAL, radius, height, 360.0, Some(&basis))
    }
}

/// Points data source for cylinder.
#[derive(Debug)]
struct CylinderPointsDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl CylinderPointsDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }
}

impl HdDataSourceBase for CylinderPointsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_ds: self.prim_ds.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for CylinderPointsDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(<Self as HdTypedSampledDataSource<Vec<Vec3f>>>::get_typed_value(self, time))
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
impl HdTypedSampledDataSource<Vec<Vec3f>> for CylinderPointsDataSource {
    fn get_typed_value(&self, _time: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let schema = HdCylinderSchema::get_from_parent(&self.prim_ds);
        let height = schema
            .get_height()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(2.0) as f32;
        let radius = schema
            .get_radius()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        let axis = schema
            .get_axis()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("Z"));
        let basis = get_axis_adjustment_matrix(&axis);
        CylinderMeshGenerator::generate_points_f32(
            NUM_RADIAL,
            radius,
            radius,
            height,
            360.0,
            Some(&basis),
        )
    }
}

/// Points data source for capsule.
#[derive(Debug)]
struct CapsulePointsDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl CapsulePointsDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }
}

impl HdDataSourceBase for CapsulePointsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_ds: self.prim_ds.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for CapsulePointsDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(<Self as HdTypedSampledDataSource<Vec<Vec3f>>>::get_typed_value(self, time))
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
impl HdTypedSampledDataSource<Vec<Vec3f>> for CapsulePointsDataSource {
    fn get_typed_value(&self, _time: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let schema = HdCapsuleSchema::get_from_parent(&self.prim_ds);
        let height = schema
            .get_height()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        let radius = schema
            .get_radius()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(0.5) as f32;
        let axis = schema
            .get_axis()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("Z"));
        let basis = get_axis_adjustment_matrix(&axis);
        CapsuleMeshGenerator::generate_points_f32(
            NUM_RADIAL,
            NUM_CAP_AXIAL,
            radius,
            radius,
            height,
            360.0,
            Some(&basis),
        )
    }
}

/// Points data source for plane.
#[derive(Debug)]
struct PlanePointsDataSource {
    prim_ds: HdContainerDataSourceHandle,
}

impl PlanePointsDataSource {
    fn new(prim_ds: HdContainerDataSourceHandle) -> Arc<Self> {
        Arc::new(Self { prim_ds })
    }
}

impl HdDataSourceBase for PlanePointsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            prim_ds: self.prim_ds.clone(),
        }) as HdDataSourceBaseHandle
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for PlanePointsDataSource {
    fn get_value(&self, time: HdSampledDataSourceTime) -> Value {
        Value::from(<Self as HdTypedSampledDataSource<Vec<Vec3f>>>::get_typed_value(self, time))
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
impl HdTypedSampledDataSource<Vec<Vec3f>> for PlanePointsDataSource {
    fn get_typed_value(&self, _time: HdSampledDataSourceTime) -> Vec<Vec3f> {
        let schema = HdPlaneSchema::get_from_parent(&self.prim_ds);
        let width = schema
            .get_width()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        let length = schema
            .get_length()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or(1.0) as f32;
        let axis = schema
            .get_axis()
            .map(|ds| ds.get_typed_value(0.0))
            .unwrap_or_else(|| TfToken::new("Z"));
        let basis = get_axis_adjustment_matrix(&axis);
        PlaneMeshGenerator::generate_points_f32(width, length, Some(&basis))
    }
}

fn mesh_topology_to_container(
    face_vertex_counts: Vec<i32>,
    face_vertex_indices: Vec<i32>,
    scheme: TfToken,
) -> HdContainerDataSourceHandle {
    let topology = HdMeshTopologySchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(
            face_vertex_counts.into_iter().collect::<Array<i32>>(),
        ) as HdIntArrayDataSourceHandle),
        Some(HdRetainedTypedSampledDataSource::new(
            face_vertex_indices.into_iter().collect::<Array<i32>>(),
        ) as HdIntArrayDataSourceHandle),
        None,
        Some(
            HdRetainedTypedSampledDataSource::new((*px_osd_tokens::RIGHT_HANDED).clone())
                as HdMeshTopologyTokenDataSourceHandle,
        ),
    );
    HdMeshSchema::build_retained(
        Some(topology),
        Some(HdRetainedTypedSampledDataSource::new(scheme) as HdMeshTokenDataSourceHandle),
        None,
        Some(HdRetainedTypedSampledDataSource::new(false) as HdBoolDataSourceHandle),
    )
}

// Use points data source directly so points update when schema params change

/// Convert cube implicit surface to mesh data source with topology and points.
pub fn compute_cube_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topo = CuboidMeshGenerator::generate_topology();
    let mesh_ds = mesh_topology_to_container(
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        (*px_osd_tokens::BILINEAR).clone(),
    );
    let points_ds = CubePointsDataSource::new(prim_ds.clone());
    let primvar = build_primvar_container(
        (*POINT).clone(),
        (*VERTEX).clone(),
        points_ds as HdDataSourceBaseHandle,
    );
    let primvars = HdRetainedContainerDataSource::from_entries(&[(
        (*POINTS).clone(),
        primvar as HdDataSourceBaseHandle,
    )]);
    let deps = compute_points_dependencies(prim_path, HdCubeSchema::get_default_locator());

    let block = HdBlockDataSource::new();
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdCubeSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_ds as HdDataSourceBaseHandle,
        ),
        (
            (*HdPrimvarsSchema::get_schema_token()).clone(),
            primvars as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

/// Convert sphere implicit surface to mesh data source with topology and points.
pub fn compute_sphere_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topo = SphereMeshGenerator::generate_topology(NUM_RADIAL, NUM_AXIAL, true);
    let mesh_ds = mesh_topology_to_container(
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        (*px_osd_tokens::CATMULL_CLARK).clone(),
    );
    let points_ds = SpherePointsDataSource::new(prim_ds.clone());
    let primvar = build_primvar_container(
        (*POINT).clone(),
        (*VERTEX).clone(),
        points_ds as HdDataSourceBaseHandle,
    );
    let primvars = HdRetainedContainerDataSource::from_entries(&[(
        (*POINTS).clone(),
        primvar as HdDataSourceBaseHandle,
    )]);
    let deps = compute_points_dependencies(prim_path, HdSphereSchema::get_default_locator());

    let block = HdBlockDataSource::new();
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdSphereSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_ds as HdDataSourceBaseHandle,
        ),
        (
            (*HdPrimvarsSchema::get_schema_token()).clone(),
            primvars as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

/// Convert cone implicit surface to mesh data source with topology and points.
pub fn compute_cone_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topo = ConeMeshGenerator::generate_topology(NUM_RADIAL, true);
    let mesh_ds = mesh_topology_to_container(
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        (*px_osd_tokens::CATMULL_CLARK).clone(),
    );
    let points_ds = ConePointsDataSource::new(prim_ds.clone());
    let primvar = build_primvar_container(
        (*POINT).clone(),
        (*VERTEX).clone(),
        points_ds as HdDataSourceBaseHandle,
    );
    let primvars = HdRetainedContainerDataSource::from_entries(&[(
        (*POINTS).clone(),
        primvar as HdDataSourceBaseHandle,
    )]);
    let deps = compute_points_dependencies(prim_path, HdConeSchema::get_default_locator());

    let block = HdBlockDataSource::new();
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdConeSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_ds as HdDataSourceBaseHandle,
        ),
        (
            (*HdPrimvarsSchema::get_schema_token()).clone(),
            primvars as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

/// Convert cylinder implicit surface to mesh data source with topology and points.
pub fn compute_cylinder_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topo = CylinderMeshGenerator::generate_topology(NUM_RADIAL, true);
    let mesh_ds = mesh_topology_to_container(
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        (*px_osd_tokens::CATMULL_CLARK).clone(),
    );
    let points_ds = CylinderPointsDataSource::new(prim_ds.clone());
    let primvar = build_primvar_container(
        (*POINT).clone(),
        (*VERTEX).clone(),
        points_ds as HdDataSourceBaseHandle,
    );
    let primvars = HdRetainedContainerDataSource::from_entries(&[(
        (*POINTS).clone(),
        primvar as HdDataSourceBaseHandle,
    )]);
    let deps = compute_points_dependencies(prim_path, HdCylinderSchema::get_default_locator());

    let block = HdBlockDataSource::new();
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdCylinderSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_ds as HdDataSourceBaseHandle,
        ),
        (
            (*HdPrimvarsSchema::get_schema_token()).clone(),
            primvars as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

/// Convert capsule implicit surface to mesh data source with topology and points.
pub fn compute_capsule_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topo = CapsuleMeshGenerator::generate_topology(NUM_RADIAL, NUM_CAP_AXIAL, true);
    let mesh_ds = mesh_topology_to_container(
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        (*px_osd_tokens::CATMULL_CLARK).clone(),
    );
    let points_ds = CapsulePointsDataSource::new(prim_ds.clone());
    let primvar = build_primvar_container(
        (*POINT).clone(),
        (*VERTEX).clone(),
        points_ds as HdDataSourceBaseHandle,
    );
    let primvars = HdRetainedContainerDataSource::from_entries(&[(
        (*POINTS).clone(),
        primvar as HdDataSourceBaseHandle,
    )]);
    let deps = compute_points_dependencies(prim_path, HdCapsuleSchema::get_default_locator());

    let block = HdBlockDataSource::new();
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdCapsuleSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_ds as HdDataSourceBaseHandle,
        ),
        (
            (*HdPrimvarsSchema::get_schema_token()).clone(),
            primvars as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}

/// Convert plane implicit surface to mesh data source with topology and points.
pub fn compute_plane_to_mesh_prim_data_source(
    prim_path: &SdfPath,
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let topo = PlaneMeshGenerator::generate_topology();
    let mesh_ds = mesh_topology_to_container(
        topo.face_vertex_counts().to_vec(),
        topo.face_vertex_indices().to_vec(),
        (*px_osd_tokens::BILINEAR).clone(),
    );
    let points_ds = PlanePointsDataSource::new(prim_ds.clone());
    let primvar = build_primvar_container(
        (*POINT).clone(),
        (*VERTEX).clone(),
        points_ds as HdDataSourceBaseHandle,
    );
    let primvars = HdRetainedContainerDataSource::from_entries(&[(
        (*POINTS).clone(),
        primvar as HdDataSourceBaseHandle,
    )]);
    let deps = compute_points_dependencies(prim_path, HdPlaneSchema::get_default_locator());

    let block = HdBlockDataSource::new();
    let overlay = HdRetainedContainerDataSource::from_entries(&[
        (
            (*HdPlaneSchema::get_schema_token()).clone(),
            block as HdDataSourceBaseHandle,
        ),
        (
            (*HdMeshSchema::get_schema_token()).clone(),
            mesh_ds as HdDataSourceBaseHandle,
        ),
        (
            (*HdPrimvarsSchema::get_schema_token()).clone(),
            primvars as HdDataSourceBaseHandle,
        ),
        (
            (*HdDependenciesSchema::get_schema_token()).clone(),
            deps as HdDataSourceBaseHandle,
        ),
    ]);
    HdOverlayContainerDataSource::new_2(overlay, prim_ds.clone())
}
