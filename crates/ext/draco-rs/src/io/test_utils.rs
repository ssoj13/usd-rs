//! IO test utilities for draco-rs.
//!
//! What: Shared helpers for test data paths and IO reads.
//! Why: Keeps test modules consistent while using local `crates/draco-rs/test`.
//! Where used: IO tests in `src/io/*_tests.rs`.

use std::env;
use std::path::PathBuf;

use crate::core::status_or::StatusOr;
use crate::io::{mesh_io, point_cloud_io, scene_io};
use crate::mesh::Mesh;
use crate::point_cloud::PointCloud;
use crate::scene::Scene;

/// Returns the root of the testdata directory.
pub(crate) fn test_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test")
}

/// Returns the path to a specific reference test file.
pub(crate) fn get_test_file_full_path(entry_name: &str) -> String {
    test_data_dir()
        .join(entry_name)
        .to_string_lossy()
        .into_owned()
}

/// Returns the temp directory for test outputs.
pub(crate) fn get_test_temp_dir() -> String {
    test_data_dir().to_string_lossy().into_owned()
}

/// Returns the full path to a temp test file.
pub(crate) fn get_test_temp_file_full_path(file_name: &str) -> String {
    PathBuf::from(get_test_temp_dir())
        .join(file_name)
        .to_string_lossy()
        .into_owned()
}

/// Reads a mesh from local testdata.
pub(crate) fn read_mesh_from_test_file(file_name: &str) -> StatusOr<Box<Mesh>> {
    let path = get_test_file_full_path(file_name);
    mesh_io::read_mesh_from_file(&path, None, None)
}

/// Reads a point cloud from local testdata.
// Parity: kept for future IO tests and reference coverage.
#[allow(dead_code)]
pub(crate) fn read_point_cloud_from_test_file(file_name: &str) -> StatusOr<Box<PointCloud>> {
    let path = get_test_file_full_path(file_name);
    point_cloud_io::read_point_cloud_from_file(&path)
}

/// Reads a scene from local testdata.
pub(crate) fn read_scene_from_test_file(file_name: &str) -> StatusOr<Box<Scene>> {
    let path = get_test_file_full_path(file_name);
    scene_io::read_scene_from_file(&path)
}
