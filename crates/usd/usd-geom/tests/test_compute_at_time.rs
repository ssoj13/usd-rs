//! Port of testUsdGeomComputeAtTime.py
//!
//! Tests for PointInstancer::compute_instance_transforms_at_time /
//! compute_instance_transforms_at_times and PointBased::compute_points_at_time /
//! compute_points_at_times.

use std::path::PathBuf;
use std::sync::Arc;
use usd_core::{InitialLoadSet, Stage};
use usd_geom::point_instancer::{MaskApplication, ProtoXformInclusion};
use usd_geom::*;
use usd_gf::Rotation;
use usd_gf::matrix4::Matrix4d;
use usd_gf::vec3::{Vec3d, Vec3f};
use usd_sdf::TimeCode;

// ============================================================================
// Constants
// ============================================================================

const MATRIX_TOLERANCE: f64 = 0.01;
const EXTENT_TOLERANCE: f64 = 0.0001;
const VECTOR_TOLERANCE: f64 = 0.0001;

// ============================================================================
// Helpers
// ============================================================================

fn testenv_path(subdir: &str, file: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("testenv");
    path.push(subdir);
    path.push(file);
    path.to_string_lossy().to_string()
}

fn open_stage() -> Arc<Stage> {
    usd_sdf::init();
    Stage::open(
        testenv_path("testUsdGeomComputeAtTime", "test.usda"),
        InitialLoadSet::LoadAll,
    )
    .expect("Failed to open test stage")
}

/// Iterate over all times within 1 unit of a given time in increments of 0.1.
/// Returns Vec<(time, delta)>.
fn time_range(time: f64) -> Vec<(f64, f64)> {
    let mut result = Vec::new();
    for i in -10..=10 {
        let delta = i as f64 * 0.1;
        result.push((time + delta, delta));
    }
    result
}

/// Build a Matrix4d from Rotation(axis, angle_degrees) + translation,
/// matching Python `Gf.Matrix4d(Gf.Rotation(axis, angle), Gf.Vec3d(tx, ty, tz))`.
fn matrix_from_rotation_translation(axis: Vec3d, angle_degrees: f64, translate: Vec3d) -> Matrix4d {
    let rotation = Rotation::from_axis_angle(axis, angle_degrees);
    let quat = rotation.get_quat();
    let mut mat = Matrix4d::identity();
    mat.set_rotate(&quat);
    mat.set_translate_only(&translate);
    mat
}

fn assert_matrix_lists_equal(list1: &[Matrix4d], list2: &[Matrix4d]) {
    assert_eq!(
        list1.len(),
        list2.len(),
        "Matrix list lengths differ: {} vs {}",
        list1.len(),
        list2.len()
    );
    for (i, (m1, m2)) in list1.iter().zip(list2.iter()).enumerate() {
        for row in 0..4 {
            for col in 0..4 {
                let diff = (m1[row][col] - m2[row][col]).abs();
                assert!(
                    diff <= MATRIX_TOLERANCE,
                    "Matrix [{i}][{row}][{col}] differs: {} vs {} (diff={diff})",
                    m1[row][col],
                    m2[row][col],
                );
            }
        }
    }
}

fn assert_all_matrix_lists_equal(lists1: &[Vec<Matrix4d>], lists2: &[Vec<Matrix4d>]) {
    assert_eq!(lists1.len(), lists2.len());
    for (l1, l2) in lists1.iter().zip(lists2.iter()) {
        assert_matrix_lists_equal(l1, l2);
    }
}

fn assert_extents_equal(ext1: &[Vec3f], ext2: &[(f64, f64, f64)]) {
    assert!(ext1.len() >= 2);
    assert!(ext2.len() >= 2);
    for i in 0..2 {
        let e1 = &ext1[i];
        let e2 = &ext2[i];
        assert!(
            (e1.x as f64 - e2.0).abs() <= EXTENT_TOLERANCE,
            "Extent[{i}].x: {} vs {}",
            e1.x,
            e2.0
        );
        assert!(
            (e1.y as f64 - e2.1).abs() <= EXTENT_TOLERANCE,
            "Extent[{i}].y: {} vs {}",
            e1.y,
            e2.1
        );
        assert!(
            (e1.z as f64 - e2.2).abs() <= EXTENT_TOLERANCE,
            "Extent[{i}].z: {} vs {}",
            e1.z,
            e2.2
        );
    }
}

fn assert_vector_lists_equal(list1: &[Vec3f], list2: &[Vec3f]) {
    assert_eq!(
        list1.len(),
        list2.len(),
        "Vector list lengths differ: {} vs {}",
        list1.len(),
        list2.len()
    );
    for (i, (v1, v2)) in list1.iter().zip(list2.iter()).enumerate() {
        assert!(
            (v1.x as f64 - v2.x as f64).abs() <= VECTOR_TOLERANCE
                && (v1.y as f64 - v2.y as f64).abs() <= VECTOR_TOLERANCE
                && (v1.z as f64 - v2.z as f64).abs() <= VECTOR_TOLERANCE,
            "Vectors [{i}] not equal: ({},{},{}) vs ({},{},{})",
            v1.x,
            v1.y,
            v1.z,
            v2.x,
            v2.y,
            v2.z,
        );
    }
}

fn assert_all_vector_lists_equal(lists1: &[Vec<Vec3f>], lists2: &[Vec<Vec3f>]) {
    assert_eq!(lists1.len(), lists2.len());
    for (l1, l2) in lists1.iter().zip(lists2.iter()) {
        assert_vector_lists_equal(l1, l2);
    }
}

// ============================================================================
// Single-sample instance transform computation helpers
// ============================================================================

/// Compute instance transforms at each time in `tr` independently (single-sample API).
fn compute_instance_transforms_single(
    pi: &PointInstancer,
    tr: &[(f64, f64)],
    base_time: f64,
    xform_inclusion: ProtoXformInclusion,
) -> Vec<Vec<Matrix4d>> {
    let mut result = Vec::new();
    for &(time, _delta) in tr {
        let mut xforms = Vec::new();
        pi.compute_instance_transforms_at_time(
            &mut xforms,
            TimeCode::new(time),
            TimeCode::new(base_time),
            xform_inclusion,
            MaskApplication::ApplyMask,
        );
        result.push(xforms);
    }
    result
}

/// Compute instance transforms using the multi-sample API.
fn compute_instance_transforms_multi(
    pi: &PointInstancer,
    tr: &[(f64, f64)],
    base_time: f64,
    xform_inclusion: ProtoXformInclusion,
) -> Vec<Vec<Matrix4d>> {
    let times: Vec<TimeCode> = tr.iter().map(|&(time, _)| TimeCode::new(time)).collect();
    let mut xforms_array = Vec::new();
    pi.compute_instance_transforms_at_times(
        &mut xforms_array,
        &times,
        TimeCode::new(base_time),
        xform_inclusion,
        MaskApplication::ApplyMask,
    );
    xforms_array
}

/// Compute points at each time in `tr` independently (single-sample API).
fn compute_points_single(pb: &PointBased, tr: &[(f64, f64)], base_time: f64) -> Vec<Vec<Vec3f>> {
    let mut result = Vec::new();
    for &(time, _delta) in tr {
        let mut points = Vec::new();
        pb.compute_points_at_time(&mut points, TimeCode::new(time), TimeCode::new(base_time));
        result.push(points);
    }
    result
}

/// Compute points using the multi-sample API.
fn compute_points_multi(pb: &PointBased, tr: &[(f64, f64)], base_time: f64) -> Vec<Vec<Vec3f>> {
    let times: Vec<TimeCode> = tr.iter().map(|&(time, _)| TimeCode::new(time)).collect();
    let mut points_array = Vec::new();
    pb.compute_points_at_times(&mut points_array, &times, TimeCode::new(base_time));
    points_array
}

// Z-axis unit vector
fn z_axis() -> Vec3d {
    Vec3d::new(0.0, 0.0, 1.0)
}

// ============================================================================
// Macro for generating single + multi variants of each test
// ============================================================================

/// Generate single-sample and multi-sample test variants.
/// Usage: gen_instance_tests!(test_name, body_fn)
/// body_fn takes a function pointer for computing transforms.
macro_rules! gen_instance_tests {
    ($test_name:ident, $body:expr) => {
        paste::paste! {
            #[test]
            fn [<$test_name _single>]() {
                $body(compute_instance_transforms_single);
            }

            #[test]
            fn [<$test_name _multi>]() {
                $body(compute_instance_transforms_multi);
            }
        }
    };
}

macro_rules! gen_points_tests {
    ($test_name:ident, $body:expr) => {
        paste::paste! {
            #[test]
            fn [<$test_name _single>]() {
                $body(compute_points_single);
            }

            #[test]
            fn [<$test_name _multi>]() {
                $body(compute_points_multi);
            }
        }
    };
}

// ============================================================================
// PointInstancer transform tests (base class tests shared by both variants)
// ============================================================================

type ComputeXformsFn =
    fn(&PointInstancer, &[(f64, f64)], f64, ProtoXformInclusion) -> Vec<Vec<Matrix4d>>;

type ComputePointsFn = fn(&PointBased, &[(f64, f64)], f64) -> Vec<Vec<Vec3f>>;

gen_instance_tests!(test_no_instances, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/NoInstances").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    for (base_time, _) in time_range(0.0) {
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        for xforms in &xforms_array {
            assert_eq!(xforms.len(), 0);
        }
    }
});

gen_instance_tests!(test_one_instance_no_samples, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OneInstanceNoSamples").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let base_time = 1.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr.iter().map(|_| vec![Matrix4d::identity()]).collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);
});

gen_instance_tests!(
    test_one_instance_no_velocities,
    |compute: ComputeXformsFn| {
        let stage = open_stage();
        let path = usd_sdf::Path::from_string("/OneInstanceNoVelocities").unwrap();
        let pi = PointInstancer::get(&stage, &path);

        // Test directly on sample.
        let base_time = 0.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let mut compares = Vec::new();
        for &(time, _delta) in &tr {
            if time < 0.0 {
                // Samples at times less than 0 should clamp to first sample.
                compares.push(vec![Matrix4d::identity()]);
            } else {
                compares.push(vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                )]);
            }
        }
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test in-between samples.
        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime before and after natural sample. Since we are
        // interpolating, these should always be the same.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array_before = compute(
            &pi,
            &tr,
            base_time - 1.0,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_array_after =
            compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        assert_all_matrix_lists_equal(&xforms_array_before, &xforms_array_after);
    }
);

gen_instance_tests!(test_one_instance, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OneInstance").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    // Test directly on sample.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test in-between samples.
    let base_time = 2.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test with basetime before natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let xforms_array = compute(
        &pi,
        &tr,
        base_time - 1.0,
        ProtoXformInclusion::IncludeProtoXform,
    );
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test with basetime on natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(_time, delta)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                180.0 - delta * 36.0,
                Vec3d::new(
                    25.0 - delta * 5.0,
                    50.0 - delta * 10.0,
                    100.0 - delta * 20.0,
                ),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);
});

gen_instance_tests!(test_pref_orientationsf, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/PrefOrientationsf").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    // Test directly on sample.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test in-between samples.
    let base_time = 2.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test with basetime before natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let xforms_array = compute(
        &pi,
        &tr,
        base_time - 1.0,
        ProtoXformInclusion::IncludeProtoXform,
    );
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test with basetime on natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(_time, delta)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                180.0 - delta * 36.0,
                Vec3d::new(
                    25.0 - delta * 5.0,
                    50.0 - delta * 10.0,
                    100.0 - delta * 20.0,
                ),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);
});

gen_instance_tests!(
    test_pref_halves_over_floats_time_samples,
    |compute: ComputeXformsFn| {
        let stage = open_stage();
        let path = usd_sdf::Path::from_string("/PrefHalvesOverFloatsTimeSamples").unwrap();
        let pi = PointInstancer::get(&stage, &path);

        // Test directly on sample.
        let base_time = 0.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test in-between samples.
        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime before natural sample.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array = compute(
            &pi,
            &tr,
            base_time - 1.0,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime on natural sample.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(_time, delta)| {
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    180.0 - delta * 36.0,
                    Vec3d::new(
                        25.0 - delta * 5.0,
                        50.0 - delta * 10.0,
                        100.0 - delta * 20.0,
                    ),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);
    }
);

gen_instance_tests!(
    test_pref_halves_over_floats_no_samples,
    |compute: ComputeXformsFn| {
        let stage = open_stage();
        let path = usd_sdf::Path::from_string("/PrefHalvesOverFloatsNoSamples").unwrap();
        let pi = PointInstancer::get(&stage, &path);

        let base_time = 1.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr.iter().map(|_| vec![Matrix4d::identity()]).collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);
    }
);

gen_instance_tests!(
    test_one_instance_acceleration,
    |compute: ComputeXformsFn| {
        let stage = open_stage();
        let path = usd_sdf::Path::from_string("/OneInstanceAcceleration").unwrap();
        let pi = PointInstancer::get(&stage, &path);

        // Test directly on sample.
        let base_time = 0.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                let p = (time / 24.0) * (120.0 + (time * 1.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(p, p, p),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test in-between samples.
        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                let p = (time / 24.0) * (120.0 + (time * 1.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(p, p, p),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime before natural sample.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array = compute(
            &pi,
            &tr,
            base_time - 1.0,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                let p = (time / 24.0) * (120.0 + (time * 1.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(p, p, p),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime on natural sample.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(_time, delta)| {
                let px = 25.0 + (delta / 24.0) * (120.0 + (delta * 1.0 * 0.5));
                let py = 50.0 + (delta / 24.0) * (240.0 + (delta * 2.0 * 0.5));
                let pz = 100.0 + (delta / 24.0) * (480.0 + (delta * 3.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    180.0 - delta * 36.0,
                    Vec3d::new(px, py, pz),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);
    }
);

gen_instance_tests!(
    test_one_instance_accelerationf,
    |compute: ComputeXformsFn| {
        let stage = open_stage();
        let path = usd_sdf::Path::from_string("/OneInstanceAccelerationf").unwrap();
        let pi = PointInstancer::get(&stage, &path);

        // Test directly on sample.
        let base_time = 0.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                let p = (time / 24.0) * (120.0 + (time * 1.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(p, p, p),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test in-between samples.
        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                let p = (time / 24.0) * (120.0 + (time * 1.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(p, p, p),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime before natural sample.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array = compute(
            &pi,
            &tr,
            base_time - 1.0,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(time, _)| {
                let p = (time / 24.0) * (120.0 + (time * 1.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(p, p, p),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);

        // Test with basetime on natural sample.
        let base_time = 5.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let compares: Vec<Vec<Matrix4d>> = tr
            .iter()
            .map(|&(_time, delta)| {
                let px = 25.0 + (delta / 24.0) * (120.0 + (delta * 1.0 * 0.5));
                let py = 50.0 + (delta / 24.0) * (240.0 + (delta * 2.0 * 0.5));
                let pz = 100.0 + (delta / 24.0) * (480.0 + (delta * 3.0 * 0.5));
                vec![matrix_from_rotation_translation(
                    z_axis(),
                    180.0 - delta * 36.0,
                    Vec3d::new(px, py, pz),
                )]
            })
            .collect();
        assert_all_matrix_lists_equal(&xforms_array, &compares);
    }
);

gen_instance_tests!(
    test_one_instance_unaligned_data,
    |compute: ComputeXformsFn| {
        let stage = open_stage();
        let path = usd_sdf::Path::from_string("/OneInstanceUnalignedData").unwrap();
        let pi = PointInstancer::get(&stage, &path);

        // Test that unaligned positions/orientations are handled properly.
        let base_time = 3.0;
        let tr = time_range(base_time);
        let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
        let mut compares = Vec::new();
        for &(time, _delta) in &tr {
            let rotation_time = time - 2.0;
            let velocity_time = time - 1.0;
            compares.push(vec![matrix_from_rotation_translation(
                z_axis(),
                rotation_time * 36.0,
                Vec3d::new(
                    velocity_time * 5.0,
                    velocity_time * 10.0,
                    velocity_time * 20.0,
                ),
            )]);
        }
        assert_all_matrix_lists_equal(&xforms_array, &compares);
    }
);

gen_instance_tests!(
    test_one_instance_time_sample_correspondence_validation,
    |compute: ComputeXformsFn| {
        let stage = open_stage();

        let pi_diff_pos_vel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceDifferingNumberPositionsAndVelocities")
                .unwrap(),
        );
        let pi_unaligned_pos_vel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceUnalignedPositionsAndVelocities").unwrap(),
        );
        let pi_pos_only = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceUnalignedPositionsOnly").unwrap(),
        );

        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_diff_pos_vel = compute(
            &pi_diff_pos_vel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_unaligned_pos_vel = compute(
            &pi_unaligned_pos_vel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_pos_only = compute(
            &pi_pos_only,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );

        assert_all_matrix_lists_equal(&xforms_diff_pos_vel, &xforms_pos_only);
        assert_all_matrix_lists_equal(&xforms_unaligned_pos_vel, &xforms_pos_only);

        // Orientations and angular velocities
        let pi_diff_orient_angvel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string(
                "/OneInstanceDifferingNumberOrientationsAndAngularVelocities",
            )
            .unwrap(),
        );
        let pi_unaligned_orient_angvel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceUnalignedOrientationsAndAngularVelocities")
                .unwrap(),
        );
        let pi_orient_only = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceUnalignedOrientationsOnly").unwrap(),
        );

        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_diff_orient_angvel = compute(
            &pi_diff_orient_angvel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_unaligned_orient_angvel = compute(
            &pi_unaligned_orient_angvel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_orient_only = compute(
            &pi_orient_only,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );

        assert_all_matrix_lists_equal(&xforms_diff_orient_angvel, &xforms_orient_only);
        assert_all_matrix_lists_equal(&xforms_unaligned_orient_angvel, &xforms_orient_only);

        // Velocities and accelerations
        let pi_diff_vel_accel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceDiffNumberVelocitiesAndAccelerations")
                .unwrap(),
        );
        let pi_unaligned_vel_accel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstanceUnalignedVelocitiesAndAccelerations").unwrap(),
        );
        let pi_diff_pos_vel_accel = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string(
                "/OneInstanceDiffNumberPositionsAndVelocitiesAndAccelerations",
            )
            .unwrap(),
        );
        let pi_pos_vel_only = PointInstancer::get(
            &stage,
            &usd_sdf::Path::from_string("/OneInstancePositionsAndVelocitiesOnly").unwrap(),
        );

        let base_time = 2.0;
        let tr = time_range(base_time);
        let xforms_diff_vel_accel = compute(
            &pi_diff_vel_accel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_unaligned_vel_accel = compute(
            &pi_unaligned_vel_accel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_diff_pos_vel_accel = compute(
            &pi_diff_pos_vel_accel,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );
        let xforms_pos_vel_only = compute(
            &pi_pos_vel_only,
            &tr,
            base_time,
            ProtoXformInclusion::IncludeProtoXform,
        );

        assert_all_matrix_lists_equal(&xforms_diff_vel_accel, &xforms_pos_vel_only);
        assert_all_matrix_lists_equal(&xforms_unaligned_vel_accel, &xforms_pos_vel_only);
        assert_all_matrix_lists_equal(&xforms_diff_pos_vel_accel, &xforms_pos_only);
    }
);

gen_instance_tests!(test_one_instance_proto_xform, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OneInstanceProtoXform").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    // Test with prototype xforms (default).
    let base_time = 0.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                180.0 + time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);

    // Test without prototype xforms.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::ExcludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![matrix_from_rotation_translation(
                z_axis(),
                time * 36.0,
                Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
            )]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);
});

gen_instance_tests!(test_multi_instance, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/MultiInstance").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    // Test with 3 instances.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![
                matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                ),
                matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, 1.0 + time * 20.0),
                ),
                matrix_from_rotation_translation(
                    z_axis(),
                    180.0 - time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, 2.0 + time * 20.0),
                ),
            ]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);
});

gen_instance_tests!(test_mask, |compute: ComputeXformsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/MultiInstanceMask").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    // Test with 3 instances with the second masked out.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let xforms_array = compute(&pi, &tr, base_time, ProtoXformInclusion::IncludeProtoXform);
    let compares: Vec<Vec<Matrix4d>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![
                matrix_from_rotation_translation(
                    z_axis(),
                    time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, time * 20.0),
                ),
                matrix_from_rotation_translation(
                    z_axis(),
                    180.0 - time * 36.0,
                    Vec3d::new(time * 5.0, time * 10.0, 2.0 + time * 20.0),
                ),
            ]
        })
        .collect();
    assert_all_matrix_lists_equal(&xforms_array, &compares);
});

// ============================================================================
// Extent tests
// ============================================================================

#[test]
fn test_extent_single() {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/MultiInstanceForExtents").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let times = [0.0, 1.0, 2.0];
    let expected_extents: Vec<Vec<(f64, f64, f64)>> = vec![
        vec![(-1.0, -1.0, -1.0), (1.0, 1.0, 1.0)],
        vec![(-3.7600734, 1.2399265, -1.0), (3.7600734, 6.2600737, 3.5)],
        vec![(-6.3968024, 3.6031978, -1.0), (6.3968024, 11.396802, 6.0)],
    ];

    for (time, expected_extent) in times.iter().zip(expected_extents.iter()) {
        let mut extent = usd_vt::Array::new();
        pi.compute_extent_at_time(&mut extent, TimeCode::new(*time), TimeCode::new(0.0));
        let extent_slice: Vec<Vec3f> = extent.iter().cloned().collect();
        assert_extents_equal(&extent_slice, expected_extent);
    }
}

#[test]
fn test_extent_multi() {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/MultiInstanceForExtents").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let times: Vec<TimeCode> = vec![TimeCode::new(0.0), TimeCode::new(1.0), TimeCode::new(2.0)];
    let expected_extents: Vec<Vec<(f64, f64, f64)>> = vec![
        vec![(-1.0, -1.0, -1.0), (1.0, 1.0, 1.0)],
        vec![(-3.7600734, 1.2399265, -1.0), (3.7600734, 6.2600737, 3.5)],
        vec![(-6.3968024, 3.6031978, -1.0), (6.3968024, 11.396802, 6.0)],
    ];

    let mut extents = Vec::new();
    pi.compute_extent_at_times(&mut extents, &times, TimeCode::new(0.0));
    for (computed_extent, expected_extent) in extents.iter().zip(expected_extents.iter()) {
        let extent_slice: Vec<Vec3f> = computed_extent.iter().cloned().collect();
        assert_extents_equal(&extent_slice, expected_extent);
    }
}

// ============================================================================
// Single-sample specific tests (TestUsdGeomComputeAtTime only)
// ============================================================================

#[test]
fn test_no_instances_default_single() {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/NoInstances").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let mut xforms = Vec::new();
    pi.compute_instance_transforms_at_time(
        &mut xforms,
        TimeCode::default_time(),
        TimeCode::default_time(),
        ProtoXformInclusion::IncludeProtoXform,
        MaskApplication::ApplyMask,
    );
    assert_eq!(xforms.len(), 0);
}

#[test]
fn test_one_instance_no_samples_default_single() {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OneInstanceNoSamples").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let mut xforms = Vec::new();
    pi.compute_instance_transforms_at_time(
        &mut xforms,
        TimeCode::default_time(),
        TimeCode::default_time(),
        ProtoXformInclusion::IncludeProtoXform,
        MaskApplication::ApplyMask,
    );
    assert_matrix_lists_equal(&xforms, &[Matrix4d::identity()]);
}

// ============================================================================
// Multi-sample specific tests (TestUsdGeomComputeAtTimeMultisampled only)
// ============================================================================

#[test]
fn test_no_instances_default_multi() {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/NoInstances").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let mut xforms_array = Vec::new();
    pi.compute_instance_transforms_at_times(
        &mut xforms_array,
        &[TimeCode::default_time()],
        TimeCode::default_time(),
        ProtoXformInclusion::IncludeProtoXform,
        MaskApplication::ApplyMask,
    );
    assert_eq!(xforms_array.len(), 1);
    assert_eq!(xforms_array[0].len(), 0);
}

#[test]
fn test_one_instance_no_samples_default_multi() {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OneInstanceNoSamples").unwrap();
    let pi = PointInstancer::get(&stage, &path);

    let mut xforms_array = Vec::new();
    pi.compute_instance_transforms_at_times(
        &mut xforms_array,
        &[TimeCode::default_time()],
        TimeCode::default_time(),
        ProtoXformInclusion::IncludeProtoXform,
        MaskApplication::ApplyMask,
    );
    let compares = vec![vec![Matrix4d::identity()]];
    assert_all_matrix_lists_equal(&xforms_array, &compares);
}

// ============================================================================
// PointBased compute points tests
// ============================================================================

gen_points_tests!(test_no_points, |compute: ComputePointsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/NoPoints").unwrap();
    let pb = PointBased::get(&stage, &path);

    for (base_time, _) in time_range(0.0) {
        let tr = time_range(base_time);
        let points_array = compute(&pb, &tr, base_time);
        for points in &points_array {
            assert_eq!(points.len(), 0);
        }
    }
});

gen_points_tests!(test_one_point_no_samples, |compute: ComputePointsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OnePointNoSamples").unwrap();
    let pb = PointBased::get(&stage, &path);

    let base_time = 1.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr.iter().map(|_| vec![Vec3f::new(0.0, 0.0, 0.0)]).collect();
    assert_all_vector_lists_equal(&points_array, &compares);
});

gen_points_tests!(test_one_point_no_velocities, |compute: ComputePointsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OnePointNoVelocities").unwrap();
    let pb = PointBased::get(&stage, &path);

    // Test directly on sample.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let mut compares = Vec::new();
    for &(time, _delta) in &tr {
        if time < 0.0 {
            // Samples at times less than 0 should clamp to first sample.
            compares.push(vec![Vec3f::new(0.0, 0.0, 0.0)]);
        } else {
            compares.push(vec![Vec3f::new(
                time as f32 * 5.0,
                time as f32 * 10.0,
                time as f32 * 20.0,
            )]);
        }
    }
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test in-between samples.
    let base_time = 2.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![Vec3f::new(
                time as f32 * 5.0,
                time as f32 * 10.0,
                time as f32 * 20.0,
            )]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test with basetime before and after natural sample. Since we are
    // interpolating, these should always be the same.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let points_array_before = compute(&pb, &tr, base_time - 1.0);
    let points_array_after = compute(&pb, &tr, base_time);
    assert_all_vector_lists_equal(&points_array_before, &points_array_after);
});

gen_points_tests!(test_one_point, |compute: ComputePointsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OnePoint").unwrap();
    let pb = PointBased::get(&stage, &path);

    // Test directly on sample.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![Vec3f::new(
                time as f32 * 5.0,
                time as f32 * 10.0,
                time as f32 * 20.0,
            )]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test in-between samples.
    let base_time = 2.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![Vec3f::new(
                time as f32 * 5.0,
                time as f32 * 10.0,
                time as f32 * 20.0,
            )]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test with basetime before natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time - 1.0);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![Vec3f::new(
                time as f32 * 5.0,
                time as f32 * 10.0,
                time as f32 * 20.0,
            )]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test with basetime on natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(_time, delta)| {
            vec![Vec3f::new(
                (25.0 - delta * 5.0) as f32,
                (50.0 - delta * 10.0) as f32,
                (100.0 - delta * 20.0) as f32,
            )]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);
});

gen_points_tests!(test_one_point_acceleration, |compute: ComputePointsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/OnePointAcceleration").unwrap();
    let pb = PointBased::get(&stage, &path);

    // Test directly on sample.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            let p = ((time / 24.0) * (120.0 + (time * 1.0 * 0.5))) as f32;
            vec![Vec3f::new(p, p, p)]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test in-between samples.
    let base_time = 2.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            let p = ((time / 24.0) * (120.0 + (time * 1.0 * 0.5))) as f32;
            vec![Vec3f::new(p, p, p)]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test with basetime before natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time - 1.0);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            let p = ((time / 24.0) * (120.0 + (time * 1.0 * 0.5))) as f32;
            vec![Vec3f::new(p, p, p)]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);

    // Test with basetime on natural sample.
    let base_time = 5.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(_time, delta)| {
            let px = (25.0 + (delta / 24.0) * (120.0 + (delta * 1.0 * 0.5))) as f32;
            let py = (50.0 + (delta / 24.0) * (240.0 + (delta * 2.0 * 0.5))) as f32;
            let pz = (100.0 + (delta / 24.0) * (480.0 + (delta * 3.0 * 0.5))) as f32;
            vec![Vec3f::new(px, py, pz)]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);
});

gen_points_tests!(
    test_one_point_time_sample_correspondence_validation,
    |compute: ComputePointsFn| {
        let stage = open_stage();

        let pb_diff_pos_vel = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string("/OnePointDifferingNumberPositionsAndVelocities").unwrap(),
        );
        let pb_unaligned_pos_vel = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string("/OnePointUnalignedPositionsAndVelocities").unwrap(),
        );
        let pb_pos_only = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string("/OnePointUnalignedPositionsOnly").unwrap(),
        );

        let base_time = 2.0;
        let tr = time_range(base_time);
        let points_diff_pos_vel = compute(&pb_diff_pos_vel, &tr, base_time);
        let points_unaligned_pos_vel = compute(&pb_unaligned_pos_vel, &tr, base_time);
        let points_pos_only = compute(&pb_pos_only, &tr, base_time);

        assert_all_vector_lists_equal(&points_diff_pos_vel, &points_pos_only);
        assert_all_vector_lists_equal(&points_unaligned_pos_vel, &points_pos_only);

        // Velocities and accelerations
        let pb_diff_vel_accel = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string("/OnePointDiffNumberVelocitiesAndAccelerations").unwrap(),
        );
        let pb_unaligned_vel_accel = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string("/OnePointUnalignedVelocitiesAndAccelerations").unwrap(),
        );
        let pb_diff_pos_vel_accel = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string(
                "/OnePointDiffNumberPositionsAndVelocitiesAndAccelerations",
            )
            .unwrap(),
        );
        let pb_pos_vel_only = PointBased::get(
            &stage,
            &usd_sdf::Path::from_string("/OnePointPositionsAndVelocitiesOnly").unwrap(),
        );

        let base_time = 2.0;
        let tr = time_range(base_time);
        let points_diff_vel_accel = compute(&pb_diff_vel_accel, &tr, base_time);
        let points_unaligned_vel_accel = compute(&pb_unaligned_vel_accel, &tr, base_time);
        let points_diff_pos_vel_accel = compute(&pb_diff_pos_vel_accel, &tr, base_time);
        let points_pos_vel_only = compute(&pb_pos_vel_only, &tr, base_time);

        assert_all_vector_lists_equal(&points_diff_vel_accel, &points_pos_vel_only);
        assert_all_vector_lists_equal(&points_unaligned_vel_accel, &points_pos_vel_only);
        assert_all_vector_lists_equal(&points_diff_pos_vel_accel, &points_pos_only);
    }
);

gen_points_tests!(test_multi_points, |compute: ComputePointsFn| {
    let stage = open_stage();
    let path = usd_sdf::Path::from_string("/MultiPoints").unwrap();
    let pb = PointBased::get(&stage, &path);

    // Test with 3 points.
    let base_time = 0.0;
    let tr = time_range(base_time);
    let points_array = compute(&pb, &tr, base_time);
    let compares: Vec<Vec<Vec3f>> = tr
        .iter()
        .map(|&(time, _)| {
            vec![
                Vec3f::new(time as f32 * 5.0, time as f32 * 10.0, time as f32 * 20.0),
                Vec3f::new(
                    time as f32 * 5.0,
                    time as f32 * 10.0,
                    1.0 + time as f32 * 20.0,
                ),
                Vec3f::new(
                    time as f32 * 5.0,
                    time as f32 * 10.0,
                    2.0 + time as f32 * 20.0,
                ),
            ]
        })
        .collect();
    assert_all_vector_lists_equal(&points_array, &compares);
});
