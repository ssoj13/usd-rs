//! File utilities.
//! Reference: `_ref/draco/src/draco/io/file_utils.h` + `.cc`.

use crate::io::file_reader_factory::FileReaderFactory;
use crate::io::file_writer_factory::FileWriterFactory;
use crate::io::file_writer_utils::split_path_private;
use crate::io::parser_utils;

pub fn split_path(full_path: &str, out_folder_path: &mut String, out_file_name: &mut String) {
    split_path_private(full_path, out_folder_path, out_file_name);
}

pub fn replace_file_extension(in_file_name: &str, new_extension: &str) -> String {
    if let Some(pos) = in_file_name.rfind('.') {
        let mut out = in_file_name[..pos + 1].to_string();
        out.push_str(new_extension);
        out
    } else {
        format!("{}.{}", in_file_name, new_extension)
    }
}

pub fn lowercase_file_extension(filename: &str) -> String {
    if let Some(pos) = filename.rfind('.') {
        if pos == 0 || pos == filename.len() - 1 {
            return String::new();
        }
        return parser_utils::to_lower(&filename[pos + 1..]);
    }
    String::new()
}

pub fn lowercase_mime_type_extension(mime_type: &str) -> String {
    if let Some(pos) = mime_type.rfind('/') {
        if pos == 0 || pos == mime_type.len() - 1 {
            return String::new();
        }
        return parser_utils::to_lower(&mime_type[pos + 1..]);
    }
    String::new()
}

pub fn remove_file_extension(filename: &str) -> String {
    if let Some(pos) = filename.rfind('.') {
        if pos == 0 || pos == filename.len() - 1 {
            return filename.to_string();
        }
        return filename[..pos].to_string();
    }
    filename.to_string()
}

pub fn get_full_path(input_file_relative_path: &str, sibling_file_full_path: &str) -> String {
    let mut prefix = String::new();
    if let Some(pos) = sibling_file_full_path.rfind(['/', '\\']) {
        prefix = sibling_file_full_path[..pos + 1].to_string();
    }
    format!("{}{}", prefix, input_file_relative_path)
}

pub fn read_file_to_buffer(file_name: &str, buffer: &mut Vec<u8>) -> bool {
    let mut file_reader = match FileReaderFactory::open_reader(file_name) {
        Some(reader) => reader,
        None => return false,
    };
    file_reader.read_file_to_buffer(buffer)
}

pub fn read_file_to_buffer_u8(file_name: &str, buffer: &mut Vec<u8>) -> bool {
    let mut file_reader = match FileReaderFactory::open_reader(file_name) {
        Some(reader) => reader,
        None => return false,
    };
    file_reader.read_file_to_buffer_u8(buffer)
}

pub fn read_file_to_string(file_name: &str, contents: &mut String) -> bool {
    contents.clear();
    let mut buffer: Vec<u8> = Vec::new();
    if !read_file_to_buffer(file_name, &mut buffer) {
        return false;
    }
    *contents = String::from_utf8_lossy(&buffer).into_owned();
    true
}

pub fn write_buffer_to_file(buffer: &[u8], file_name: &str) -> bool {
    let mut file_writer = match FileWriterFactory::open_writer(file_name) {
        Some(writer) => writer,
        None => return false,
    };
    file_writer.write(buffer)
}

pub fn get_file_size(file_name: &str) -> usize {
    let mut file_reader = match FileReaderFactory::open_reader(file_name) {
        Some(reader) => reader,
        None => return 0,
    };
    file_reader.get_file_size()
}
