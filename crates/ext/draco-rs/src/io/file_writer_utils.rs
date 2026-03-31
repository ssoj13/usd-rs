//! File writer utilities.
//! Reference: `_ref/draco/src/draco/io/file_writer_utils.h` + `.cc`.

use std::fs;
use std::path::{Path, PathBuf};

pub fn split_path_private(
    full_path: &str,
    out_folder_path: &mut String,
    out_file_name: &mut String,
) {
    if let Some(pos) = full_path.rfind(['/', '\\']) {
        *out_folder_path = full_path[..pos].to_string();
        *out_file_name = full_path[pos + 1..].to_string();
    } else {
        *out_folder_path = ".".to_string();
        *out_file_name = full_path.to_string();
    }
}

pub fn directory_exists(path_arg: &str) -> bool {
    let mut path = PathBuf::from(path_arg);
    if cfg!(windows) {
        if let Some(last) = path_arg.chars().last() {
            if last != '\\' && last != '/' {
                path = PathBuf::from(format!("{}\\", path_arg));
            }
        }
    }
    Path::new(&path)
        .metadata()
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

pub fn check_and_create_path_for_file(filename: &str) -> bool {
    let mut folder = String::new();
    let mut basename = String::new();
    split_path_private(filename, &mut folder, &mut basename);
    // C++ SplitPathPrivate always sets folder to "." when no separator found,
    // so folder is never empty. But guard against edge cases the same way.
    if folder.is_empty() {
        folder = ".".to_string();
    }
    let _ = fs::create_dir_all(&folder);
    directory_exists(&folder)
}
