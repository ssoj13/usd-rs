//! Stdio file writer.
//! Reference: `_ref/draco/src/draco/io/stdio_file_writer.h` + `.cc`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::io::file_writer_interface::FileWriterInterface;
use crate::io::file_writer_utils::check_and_create_path_for_file;

pub struct StdioFileWriter {
    file: File,
}

impl StdioFileWriter {
    pub fn open(file_name: &str) -> Option<Box<dyn FileWriterInterface>> {
        if file_name.is_empty() {
            return None;
        }
        if !check_and_create_path_for_file(file_name) {
            return None;
        }
        let path = Path::new(file_name);
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)
            .ok()?;
        Some(Box::new(Self { file }))
    }
}

impl FileWriterInterface for StdioFileWriter {
    fn write(&mut self, buffer: &[u8]) -> bool {
        self.file.write_all(buffer).is_ok()
    }
}
