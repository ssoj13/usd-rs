/// Port of testUsdPhysicsRigidBodyAPI.py
///
/// Tests ComputeMassProperties with various density/mass/scale configurations.
use std::sync::Arc;

use usd_core::{InitialLoadSet, Prim, Stage};
use usd_gf::{Matrix3f, Matrix4d, Quatf, Vec3f};
use usd_physics::{
    CollisionAPI, MassAPI, MassInformation, MassInformationFn, MaterialAPI, RigidBodyAPI,
    set_stage_kilograms_per_unit,
};
use usd_sdf::Path;
use usd_shade::{Material, MaterialBindingAPI, tokens::tokens as shade_tokens};
use usd_tf::Token;
use usd_vt::Value;

const TOLERANCE: f32 = 0.01;

fn new_stage() -> Arc<Stage> {
    usd_sdf::init();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create_in_memory")
}

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap()
}

fn tcd() -> usd_sdf::TimeCode {
    usd_sdf::TimeCode::default()
}

fn setup_scene(stage: &Arc<Stage>) {
    usd_geom::set_stage_up_axis(&*stage, &Token::new("Z"));
    usd_geom::set_stage_meters_per_unit(&*stage, 1.0);
    set_stage_kilograms_per_unit(stage, 1.0);
}

/// Build a MassInformationFn for cube geometry at known world transform.
fn cube_mass_info_fn(
    rigid_body_prim_path: Path,
    rigid_body_world_transform: Matrix4d,
) -> MassInformationFn {
    Box::new(move |prim: &Prim| {
        let mut mass_info = MassInformation::default();

        if prim.get_type_name().as_str() != "Cube" {
            mass_info.volume = -1.0;
            return mass_info;
        }

        // For cube: get local transform to compute scale (extents)
        let xformable = usd_geom::xformable::Xformable::new(prim.clone());
        let local_matrix = xformable.get_local_transformation(tcd());
        let xform = usd_gf::Transform::from_matrix(&local_matrix);
        let scale_d = xform.scale();

        let cube = usd_geom::Cube::new(prim.clone());
        let size_val: f64 = cube
            .get_size_attr()
            .get(tcd())
            .and_then(|v| v.downcast_clone::<f64>())
            .unwrap_or(1.0)
            .abs();

        let extents = Vec3f::new(
            (scale_d.x * size_val) as f32,
            (scale_d.y * size_val) as f32,
            (scale_d.z * size_val) as f32,
        );

        mass_info.volume = extents.x * extents.y * extents.z;

        let ix = (1.0 / 12.0) * (extents.y * extents.y + extents.z * extents.z);
        let iy = (1.0 / 12.0) * (extents.x * extents.x + extents.z * extents.z);
        let iz = (1.0 / 12.0) * (extents.x * extents.x + extents.y * extents.y);
        mass_info.inertia = Matrix3f::identity();
        mass_info.inertia.set_diagonal(ix, iy, iz);

        mass_info.center_of_mass = Vec3f::new(0.0, 0.0, 0.0);

        if prim.get_path().to_string() == rigid_body_prim_path.to_string() {
            mass_info.local_pos = Vec3f::new(0.0, 0.0, 0.0);
            mass_info.local_rot = Quatf::identity();
        } else {
            // Relative transform: collision local_to_world * body_world_inverse
            let body_inv = rigid_body_world_transform
                .inverse()
                .unwrap_or(Matrix4d::identity());
            let rel = local_matrix * body_inv;
            let rel_xform = usd_gf::Transform::from_matrix(&rel);

            let trans = rel_xform.translation();
            mass_info.local_pos = Vec3f::new(trans.x as f32, trans.y as f32, trans.z as f32);

            let quat_d = rel_xform.rotation().get_quat();
            mass_info.local_rot = Quatf::new(
                quat_d.real() as f32,
                Vec3f::new(
                    quat_d.imaginary().x as f32,
                    quat_d.imaginary().y as f32,
                    quat_d.imaginary().z as f32,
                ),
            );

            // Bake body scale into localPos
            let body_xform = usd_gf::Transform::from_matrix(&rigid_body_world_transform);
            let sc = body_xform.scale();
            mass_info.local_pos.x *= sc.x as f32;
            mass_info.local_pos.y *= sc.y as f32;
            mass_info.local_pos.z *= sc.z as f32;
        }

        mass_info
    })
}

fn close_f32(a: f32, b: f32) -> bool {
    (a - b).abs() < TOLERANCE
}

fn close_vec3(a: &Vec3f, b: &Vec3f) -> bool {
    close_f32(a.x, b.x) && close_f32(a.y, b.y) && close_f32(a.z, b.z)
}

// ===========================================================================
// test_mass_rigid_body_cube
// Default density 1000, unit cube -> mass=1000, inertia=(166.667,166.667,166.667)
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube() {
    let stage = new_stage();
    setup_scene(&stage);

    let cube = usd_geom::Cube::define(&*stage, &p("/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let rigid_body_api = RigidBodyAPI::apply(cube.prim()).unwrap();

    // Unit cube at origin: world transform = identity
    let world_xform = Matrix4d::identity();
    let rb_path = cube.prim().get_path().clone();

    let mass_fn = cube_mass_info_fn(rb_path, world_xform);
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 1000.0), "mass={} expected=1000", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)),
        "com={:?}",
        com
    );
    assert!(
        close_vec3(&inertia, &Vec3f::new(166.667, 166.667, 166.667)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_rigid_body_density
// Density 500 on rigid body -> mass=500
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_rigid_body_density() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_density_attr()
        .unwrap()
        .set(Value::from(500.0_f32), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    let world_xform = Matrix4d::identity();
    let rb_path = xform.prim().get_path().clone();

    let mass_fn = cube_mass_info_fn(rb_path, world_xform);
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 500.0), "mass={} expected=500", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)),
        "com={:?}",
        com
    );
    assert!(
        close_vec3(&inertia, &Vec3f::new(83.333, 83.333, 83.333)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_density_precedence
// Collider density 500 overrides body density 5000
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_density_precedence() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let body_mass_api = MassAPI::apply(xform.prim()).unwrap();
    body_mass_api
        .get_density_attr()
        .unwrap()
        .set(Value::from(5000.0_f32), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let col_mass_api = MassAPI::apply(cube.prim()).unwrap();
    col_mass_api
        .get_density_attr()
        .unwrap()
        .set(Value::from(500.0_f32), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    // Collider density has precedence over body density
    assert!(close_f32(mass, 500.0), "mass={} expected=500", mass);
    assert!(close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(&inertia, &Vec3f::new(83.333, 83.333, 83.333)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_collider_com
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_collider_com() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let mass_api = MassAPI::apply(cube.prim()).unwrap();
    mass_api
        .get_center_of_mass_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(2.0, 2.0, 2.0)), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, _inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 1000.0), "mass={}", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(2.0, 2.0, 2.0)),
        "com={:?}",
        com
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_collider_inertia
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_collider_inertia() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let mass_api = MassAPI::apply(cube.prim()).unwrap();
    mass_api
        .get_diagonal_inertia_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(2.0, 2.0, 2.0)), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, _com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 1000.0), "mass={}", mass);
    assert!(
        close_vec3(&inertia, &Vec3f::new(2.0, 2.0, 2.0)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_rigid_body_mass
// Explicit mass 2000 on rigid body -> mass=2000, inertia scaled
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_rigid_body_mass() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_mass_attr()
        .unwrap()
        .set(Value::from(2000.0_f32), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 2000.0), "mass={}", mass);
    assert!(close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(&inertia, &Vec3f::new(333.334, 333.334, 333.334)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_collider_mass
// Explicit mass 2000 on collider -> mass=2000
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_collider_mass() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let mass_api = MassAPI::apply(cube.prim()).unwrap();
    mass_api
        .get_mass_attr()
        .unwrap()
        .set(Value::from(2000.0_f32), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 2000.0), "mass={}", mass);
    assert!(close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(&inertia, &Vec3f::new(333.334, 333.334, 333.334)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_mass_precedence
// Body mass 2000 vs collider mass 500 -> body wins (2000)
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_mass_precedence() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_mass_attr()
        .unwrap()
        .set(Value::from(2000.0_f32), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let col_mass_api = MassAPI::apply(cube.prim()).unwrap();
    col_mass_api
        .get_mass_attr()
        .unwrap()
        .set(Value::from(500.0_f32), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, _inertia, _com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(
        close_f32(mass, 2000.0),
        "mass={} expected=2000 (body has precedence)",
        mass
    );
}

// ===========================================================================
// CoM tests
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_rigid_body_com() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_center_of_mass_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(2.0, 2.0, 2.0)), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, _inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 1000.0), "mass={}", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(2.0, 2.0, 2.0)),
        "com={:?}",
        com
    );
}

#[test]
fn test_mass_rigid_body_cube_com_precedence() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_center_of_mass_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(2.0, 2.0, 2.0)), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let col_mass_api = MassAPI::apply(cube.prim()).unwrap();
    col_mass_api
        .get_center_of_mass_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(1.0, 1.0, 1.0)), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (_mass, _inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(
        close_vec3(&com, &Vec3f::new(2.0, 2.0, 2.0)),
        "com={:?} (body has precedence)",
        com
    );
}

// ===========================================================================
// Inertia tests
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_rigid_body_inertia() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_diagonal_inertia_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(2.0, 2.0, 2.0)), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, _com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 1000.0), "mass={}", mass);
    assert!(
        close_vec3(&inertia, &Vec3f::new(2.0, 2.0, 2.0)),
        "inertia={:?}",
        inertia
    );
}

#[test]
fn test_mass_rigid_body_cube_inertia_precedence() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_diagonal_inertia_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(2.0, 2.0, 2.0)), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let col_mass_api = MassAPI::apply(cube.prim()).unwrap();
    col_mass_api
        .get_diagonal_inertia_attr()
        .unwrap()
        .set(Value::from(Vec3f::new(1.0, 1.0, 1.0)), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (_mass, inertia, _com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(
        close_vec3(&inertia, &Vec3f::new(2.0, 2.0, 2.0)),
        "inertia={:?} (body has precedence)",
        inertia
    );
}

// ===========================================================================
// Units tests
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_cm_units() {
    let stage = new_stage();
    usd_geom::set_stage_up_axis(&*stage, &Token::new("Z"));
    usd_geom::set_stage_meters_per_unit(&*stage, 0.01);
    set_stage_kilograms_per_unit(&stage, 1.0);

    let cube = usd_geom::Cube::define(&*stage, &p("/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let rigid_body_api = RigidBodyAPI::apply(cube.prim()).unwrap();

    let mass_fn = cube_mass_info_fn(cube.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    let mass_scale: f32 = 0.01 * 0.01 * 0.01;
    assert!(
        close_f32(mass, 1000.0 * mass_scale),
        "mass={} expected={}",
        mass,
        1000.0 * mass_scale
    );
    assert!(close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(
            &inertia,
            &Vec3f::new(
                166.667 * mass_scale,
                166.667 * mass_scale,
                166.667 * mass_scale
            )
        ),
        "inertia={:?}",
        inertia
    );
}

#[test]
fn test_mass_rigid_body_cube_decagram_units() {
    let stage = new_stage();
    usd_geom::set_stage_up_axis(&*stage, &Token::new("Z"));
    usd_geom::set_stage_meters_per_unit(&*stage, 1.0);
    set_stage_kilograms_per_unit(&stage, 0.1);

    let cube = usd_geom::Cube::define(&*stage, &p("/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let rigid_body_api = RigidBodyAPI::apply(cube.prim()).unwrap();

    let mass_fn = cube_mass_info_fn(cube.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    let mass_scale: f32 = 1.0 / 0.1;
    assert!(
        close_f32(mass, 1000.0 * mass_scale),
        "mass={} expected={}",
        mass,
        1000.0 * mass_scale
    );
    assert!(close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(
            &inertia,
            &Vec3f::new(
                166.667 * mass_scale,
                166.667 * mass_scale,
                166.667 * mass_scale
            )
        ),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// Compound body test (two cubes offset along Z)
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_compound() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();

    let cube0 = usd_geom::Cube::define(&*stage, &p("/xform/cube0"));
    cube0.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube0.prim()).unwrap();
    cube0
        .gprim()
        .boundable()
        .xformable()
        .add_translate_op(usd_geom::XformOpPrecision::Float, None, false)
        .set(Value::from(Vec3f::new(0.0, 0.0, -2.0)), tcd());

    let cube1 = usd_geom::Cube::define(&*stage, &p("/xform/cube1"));
    cube1.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube1.prim()).unwrap();
    cube1
        .gprim()
        .boundable()
        .xformable()
        .add_translate_op(usd_geom::XformOpPrecision::Float, None, false)
        .set(Value::from(Vec3f::new(0.0, 0.0, 2.0)), tcd());

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, _inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 2000.0), "mass={} expected=2000", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)),
        "com={:?}",
        com
    );
}

// ===========================================================================
// Nested rigid bodies
// ===========================================================================

#[test]
fn test_mass_rigid_body_nested() {
    let stage = new_stage();
    setup_scene(&stage);

    // rbo0 with size=1 cube -> mass=1000
    let rbo0 = usd_geom::Xform::define(&*stage, &p("/rbo0"));
    let rbo0_api = RigidBodyAPI::apply(rbo0.prim()).unwrap();

    let cube0 = usd_geom::Cube::define(&*stage, &p("/rbo0/cube"));
    cube0.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube0.prim()).unwrap();

    // rbo1 nested under rbo0, with size=2 cube -> mass=8000
    let rbo1 = usd_geom::Xform::define(&*stage, &p("/rbo0/rbo1"));
    let rbo1_api = RigidBodyAPI::apply(rbo1.prim()).unwrap();

    let cube1 = usd_geom::Cube::define(&*stage, &p("/rbo0/rbo1/cube"));
    cube1.get_size_attr().set(Value::from(2.0_f64), tcd());
    CollisionAPI::apply(cube1.prim()).unwrap();

    // rbo0 should only see its own cube (size=1 -> mass=1000)
    let mass_fn0 = cube_mass_info_fn(rbo0.prim().get_path().clone(), Matrix4d::identity());
    let (mass0, inertia0, com0, _) = rbo0_api.compute_mass_properties(&mass_fn0);
    assert!(
        close_f32(mass0, 1000.0),
        "rbo0 mass={} expected=1000",
        mass0
    );
    assert!(close_vec3(&com0, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(&inertia0, &Vec3f::new(166.667, 166.667, 166.667)),
        "inertia0={:?}",
        inertia0
    );

    // rbo1 should see its own cube (size=2 -> volume=8 -> mass=8000)
    let mass_fn1 = cube_mass_info_fn(rbo1.prim().get_path().clone(), Matrix4d::identity());
    let (mass1, _inertia1, com1, _) = rbo1_api.compute_mass_properties(&mass_fn1);
    assert!(
        close_f32(mass1, 8000.0),
        "rbo1 mass={} expected=8000",
        mass1
    );
    assert!(close_vec3(&com1, &Vec3f::new(0.0, 0.0, 0.0)));
}

// ===========================================================================
// test_mass_rigid_body_cube_material_density
// Density 500 on physics material bound to collider -> mass=500
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_material_density() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    // Create physics material with density 500
    let mat = Material::define(&stage, &p("/basePhysicsMaterial"));
    let mat_api = MaterialAPI::apply(&mat.get_prim()).unwrap();
    mat_api
        .get_density_attr()
        .unwrap()
        .set(Value::from(500.0_f32), tcd());
    let binding_api = MaterialBindingAPI::apply(cube.prim());
    binding_api.bind(
        &mat,
        &shade_tokens().weaker_than_descendants,
        &Token::new("physics"),
    );

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 500.0), "mass={} expected=500", mass);
    assert!(close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)));
    assert!(
        close_vec3(&inertia, &Vec3f::new(83.333, 83.333, 83.333)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_principal_axes
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_principal_axes() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api.get_principal_axes_attr().unwrap().set(
        Value::from(Quatf::new(0.707, Vec3f::new(0.0, 0.707, 0.0))),
        tcd(),
    );

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    let mass_fn = cube_mass_info_fn(xform.prim().get_path().clone(), Matrix4d::identity());
    let (mass, _inertia, _com, pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 1000.0), "mass={}", mass);
    assert!(
        close_f32(pa.real(), 0.707),
        "pa.real={} expected=0.707",
        pa.real()
    );
    assert!(
        close_vec3(&pa.imaginary(), &Vec3f::new(0.0, 0.707, 0.0)),
        "pa.imag={:?}",
        pa.imaginary()
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_collider_density
// Density 500 on collider MassAPI -> mass=500
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_collider_density() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();
    let mass_api = MassAPI::apply(cube.prim()).unwrap();
    mass_api
        .get_density_attr()
        .unwrap()
        .set(Value::from(500.0_f32), tcd());

    let world_xform = Matrix4d::identity();
    let rb_path = xform.prim().get_path().clone();

    let mass_fn = cube_mass_info_fn(rb_path, world_xform);
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 500.0), "mass={} expected=500", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)),
        "com={:?}",
        com
    );
    assert!(
        close_vec3(&inertia, &Vec3f::new(83.333, 83.333, 83.333)),
        "inertia={:?}",
        inertia
    );
}

// ===========================================================================
// test_mass_rigid_body_cube_mass
// Explicit mass 500 on rigid body -> mass=500
// ===========================================================================

#[test]
fn test_mass_rigid_body_cube_mass() {
    let stage = new_stage();
    setup_scene(&stage);

    let xform = usd_geom::Xform::define(&*stage, &p("/xform"));
    let rigid_body_api = RigidBodyAPI::apply(xform.prim()).unwrap();
    let mass_api = MassAPI::apply(xform.prim()).unwrap();
    mass_api
        .get_mass_attr()
        .unwrap()
        .set(Value::from(500.0_f32), tcd());

    let cube = usd_geom::Cube::define(&*stage, &p("/xform/cube"));
    cube.get_size_attr().set(Value::from(1.0_f64), tcd());
    CollisionAPI::apply(cube.prim()).unwrap();

    let world_xform = Matrix4d::identity();
    let rb_path = xform.prim().get_path().clone();

    let mass_fn = cube_mass_info_fn(rb_path, world_xform);
    let (mass, inertia, com, _pa) = rigid_body_api.compute_mass_properties(&mass_fn);

    assert!(close_f32(mass, 500.0), "mass={} expected=500", mass);
    assert!(
        close_vec3(&com, &Vec3f::new(0.0, 0.0, 0.0)),
        "com={:?}",
        com
    );
    assert!(
        close_vec3(&inertia, &Vec3f::new(83.333, 83.333, 83.333)),
        "inertia={:?}",
        inertia
    );
}
