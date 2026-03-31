//! Test utilities.
//! Reference: `_ref/draco/src/draco/core/draco_test_utils.h` + `.cc`.

use std::env;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

#[cfg(feature = "io")]
use crate::core::status_or::StatusOr;
#[cfg(feature = "io")]
use crate::io::{mesh_io, point_cloud_io};

const GOLDEN_BUFFER_SIZE: usize = 1024;

fn test_data_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_TEST_DATA_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("_ref")
        .join("draco")
        .join("testdata")
}

fn test_temp_dir() -> PathBuf {
    if let Ok(dir) = env::var("DRACO_TEST_TEMP_DIR") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("draco_test_temp")
}

pub fn get_test_temp_dir() -> String {
    test_data_dir().to_string_lossy().into_owned()
}

pub fn get_test_file_full_path(entry_name: &str) -> String {
    test_data_dir()
        .join(entry_name)
        .to_string_lossy()
        .into_owned()
}

pub fn get_test_temp_file_full_path(file_name: &str) -> String {
    test_temp_dir()
        .join(file_name)
        .to_string_lossy()
        .into_owned()
}

pub fn generate_golden_file(golden_file_name: &str, data: &[u8]) -> bool {
    let path = get_test_file_full_path(golden_file_name);
    if let Some(parent) = PathBuf::from(&path).parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "Failed to create golden file dir {}: {}",
                parent.display(),
                err
            );
            return false;
        }
    }
    fs::write(path, data).is_ok()
}

pub fn compare_golden_file(golden_file_name: &str, data: &[u8]) -> bool {
    let golden_path = get_test_file_full_path(golden_file_name);
    let mut in_file = match File::open(&golden_path) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut buffer = [0u8; GOLDEN_BUFFER_SIZE];
    let mut remaining_data_size = data.len();
    let mut offset: usize = 0;
    let mut extracted_size: usize = 0;
    loop {
        match in_file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => extracted_size = n,
            Err(_) => return false,
        }
        if remaining_data_size == 0 {
            break;
        }
        let mut size_to_check = extracted_size;
        if remaining_data_size < size_to_check {
            size_to_check = remaining_data_size;
        }
        for i in 0..size_to_check {
            if buffer[i] != data[offset] {
                eprintln!("Test output differed from golden file at byte {}", offset);
                return false;
            }
            offset += 1;
        }
        remaining_data_size = remaining_data_size.wrapping_sub(extracted_size);
    }
    if remaining_data_size != extracted_size {
        eprintln!("Test output size differed from golden file size");
        return false;
    }
    true
}

#[cfg(feature = "io")]
pub fn read_mesh_from_test_file(file_name: &str) -> StatusOr<Box<crate::mesh::Mesh>> {
    let path = get_test_file_full_path(file_name);
    mesh_io::read_mesh_from_file(&path, None, None)
}

#[cfg(feature = "io")]
pub fn read_point_cloud_from_test_file(
    file_name: &str,
) -> StatusOr<Box<crate::point_cloud::PointCloud>> {
    let path = get_test_file_full_path(file_name);
    point_cloud_io::read_point_cloud_from_file(&path)
}

#[derive(Default)]
pub struct CaptureStream {
    buffer: Vec<u8>,
}

impl CaptureStream {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn get_string_and_release(&mut self) -> String {
        let out = String::from_utf8_lossy(&self.buffer).into_owned();
        self.reset();
        out
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
    }
}

impl Write for CaptureStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[macro_export]
macro_rules! draco_assert_ok {
    ($expression:expr) => {{
        let _local_status = $expression;
        assert!(
            _local_status.is_ok(),
            "{}",
            _local_status.error_msg_string()
        );
    }};
}

#[macro_export]
macro_rules! draco_assign_or_assert {
    ($lhs:expr, $expression:expr) => {{
        let _statusor = $expression;
        assert!(
            _statusor.is_ok(),
            "{}",
            _statusor.status().error_msg_string()
        );
        $lhs = _statusor.into_value();
    }};
}
