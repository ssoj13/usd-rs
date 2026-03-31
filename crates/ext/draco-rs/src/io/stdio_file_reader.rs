//! Stdio file reader.
//! Reference: `_ref/draco/src/draco/io/stdio_file_reader.h` + `.cc`.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::io::file_reader_interface::FileReaderInterface;

pub struct StdioFileReader {
    file: File,
}

impl StdioFileReader {
    pub fn open(file_name: &str) -> Option<Box<dyn FileReaderInterface>> {
        if file_name.is_empty() {
            return None;
        }
        let path = Path::new(file_name);
        let file = File::open(path).ok()?;
        Some(Box::new(Self { file }))
    }
}

impl FileReaderInterface for StdioFileReader {
    fn read_file_to_buffer(&mut self, buffer: &mut Vec<u8>) -> bool {
        buffer.clear();
        let file_size = self.get_file_size();
        if file_size == 0 {
            return false;
        }
        if self.file.seek(SeekFrom::Start(0)).is_err() {
            return false;
        }
        buffer.resize(file_size, 0u8);
        if let Err(_) = self.file.read_exact(buffer) {
            return false;
        }
        true
    }

    fn read_file_to_buffer_u8(&mut self, buffer: &mut Vec<u8>) -> bool {
        self.read_file_to_buffer(buffer)
    }

    fn get_file_size(&mut self) -> usize {
        if self.file.seek(SeekFrom::End(0)).is_err() {
            return 0;
        }
        let file_size = match self.file.stream_position() {
            Ok(pos) => pos as usize,
            Err(_) => 0,
        };
        let _ = self.file.seek(SeekFrom::Start(0));
        file_size
    }
}
