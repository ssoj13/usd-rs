//! One-off diagnostic for comparing `flo.usda` / `flo.usdc` / `flo.usdz`
//! at Stage and UsdGeom transform level before Hydra enters the picture.

use std::collections::{BTreeMap, BTreeSet};

use usd_core::{InitialLoadSet, Stage};
use usd_vt::TimeCode;

use usd_geom::{imageable::Imageable, xformable::Xformable};

#[derive(Debug, Clone)]
struct StageSummary {
    path: String,
    start: f64,
    end: f64,
    prim_count: usize,
    mesh_count: usize,
    xformable_count: usize,
    changed_mesh_world_count: usize,
    changed_xformable_local_count: usize,
    changed_mesh_paths: Vec<String>,
    mesh_world_at_1: BTreeMap<String, [[f64; 4]; 4]>,
    mesh_world_at_50: BTreeMap<String, [[f64; 4]; 4]>,
}

fn matrix_changed(a: &[[f64; 4]; 4], b: &[[f64; 4]; 4]) -> bool {
    a.iter()
        .flatten()
        .zip(b.iter().flatten())
        .any(|(lhs, rhs)| (lhs - rhs).abs() > 1e-9)
}

fn summarize_stage(path: &str) -> Result<StageSummary, String> {
    let stage = Stage::open(path, InitialLoadSet::LoadAll)
        .map_err(|e| format!("open stage failed for {path}: {e}"))?;

    let mut prim_count = 0usize;
    let mut mesh_count = 0usize;
    let mut xformable_count = 0usize;
    let mut changed_mesh_world_count = 0usize;
    let mut changed_xformable_local_count = 0usize;
    let mut changed_mesh_paths = Vec::new();
    let mut mesh_world_at_1 = BTreeMap::new();
    let mut mesh_world_at_50 = BTreeMap::new();

    for prim in stage.traverse() {
        prim_count += 1;
        let prim_path = prim.path().as_str().to_string();
        let type_name = prim.type_name().as_str().to_string();

        let local_1 = Xformable::new(prim.clone()).get_local_transformation(TimeCode::new(1.0));
        let local_50 = Xformable::new(prim.clone()).get_local_transformation(TimeCode::new(50.0));
        if matrix_changed(&local_1.to_array(), &local_50.to_array()) {
            changed_xformable_local_count += 1;
        }
        xformable_count += 1;

        if type_name == "Mesh" {
            mesh_count += 1;
            let imageable = Imageable::new(prim.clone());
            let world_1 = imageable
                .compute_local_to_world_transform(TimeCode::new(1.0))
                .to_array();
            let world_50 = imageable
                .compute_local_to_world_transform(TimeCode::new(50.0))
                .to_array();
            if matrix_changed(&world_1, &world_50) {
                changed_mesh_world_count += 1;
                if changed_mesh_paths.len() < 20 {
                    changed_mesh_paths.push(prim_path.clone());
                }
            }
            mesh_world_at_1.insert(prim_path.clone(), world_1);
            mesh_world_at_50.insert(prim_path, world_50);
        }
    }

    Ok(StageSummary {
        path: path.to_string(),
        start: stage.get_start_time_code(),
        end: stage.get_end_time_code(),
        prim_count,
        mesh_count,
        xformable_count,
        changed_mesh_world_count,
        changed_xformable_local_count,
        changed_mesh_paths,
        mesh_world_at_1,
        mesh_world_at_50,
    })
}

fn print_summary(summary: &StageSummary) {
    println!("== {}", summary.path);
    println!(
        "start={} end={} prims={} meshes={} xformables={}",
        summary.start, summary.end, summary.prim_count, summary.mesh_count, summary.xformable_count
    );
    println!(
        "changed_mesh_world_count={} changed_xformable_local_count={}",
        summary.changed_mesh_world_count, summary.changed_xformable_local_count
    );
    println!(
        "sample_changed_mesh_paths={}",
        summary.changed_mesh_paths.len()
    );
    for path in &summary.changed_mesh_paths {
        println!("  {path}");
    }
}

fn compare_pair(label: &str, a: &StageSummary, b: &StageSummary) {
    println!("== compare {label}");
    println!(
        "mesh_count_equal={} prim_count_equal={}",
        a.mesh_count == b.mesh_count,
        a.prim_count == b.prim_count
    );

    let a_paths: BTreeSet<_> = a.mesh_world_at_1.keys().cloned().collect();
    let b_paths: BTreeSet<_> = b.mesh_world_at_1.keys().cloned().collect();
    println!("mesh_path_set_equal={}", a_paths == b_paths);

    let mut world_t1_diff = Vec::new();
    let mut world_t50_diff = Vec::new();
    for path in a_paths.intersection(&b_paths) {
        let a1 = a.mesh_world_at_1.get(path).unwrap();
        let b1 = b.mesh_world_at_1.get(path).unwrap();
        if matrix_changed(a1, b1) && world_t1_diff.len() < 20 {
            world_t1_diff.push(path.clone());
        }
        let a50 = a.mesh_world_at_50.get(path).unwrap();
        let b50 = b.mesh_world_at_50.get(path).unwrap();
        if matrix_changed(a50, b50) && world_t50_diff.len() < 20 {
            world_t50_diff.push(path.clone());
        }
    }

    println!("sample_world_t1_diffs={}", world_t1_diff.len());
    for path in &world_t1_diff {
        println!("  t1 {path}");
    }
    println!("sample_world_t50_diffs={}", world_t50_diff.len());
    for path in &world_t50_diff {
        println!("  t50 {path}");
    }
}

fn main() -> Result<(), String> {
    let base = std::env::current_dir()
        .map_err(|e| format!("current_dir failed: {e}"))?
        .join("data");
    let usda = summarize_stage(base.join("flo.usda").to_string_lossy().as_ref())?;
    let usdc = summarize_stage(base.join("flo.usdc").to_string_lossy().as_ref())?;
    let usdz = summarize_stage(base.join("flo.usdz").to_string_lossy().as_ref())?;

    print_summary(&usda);
    print_summary(&usdc);
    print_summary(&usdz);

    compare_pair("usda vs usdc", &usda, &usdc);
    compare_pair("usda vs usdz", &usda, &usdz);
    compare_pair("usdc vs usdz", &usdc, &usdz);

    Ok(())
}
