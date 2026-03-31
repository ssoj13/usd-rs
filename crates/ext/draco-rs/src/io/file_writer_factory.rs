//! File writer factory.
//! Reference: `_ref/draco/src/draco/io/file_writer_factory.h` + `.cc`.

use std::sync::{Mutex, Once, OnceLock};

use crate::io::file_writer_interface::FileWriterInterface;
use crate::io::stdio_file_writer::StdioFileWriter;

pub type OpenFunction = fn(&str) -> Option<Box<dyn FileWriterInterface>>;

fn open_functions() -> &'static Mutex<Vec<OpenFunction>> {
    static OPEN_FUNCTIONS: OnceLock<Mutex<Vec<OpenFunction>>> = OnceLock::new();
    OPEN_FUNCTIONS.get_or_init(|| Mutex::new(Vec::new()))
}

fn ensure_default_writers() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut funcs = open_functions()
            .lock()
            .expect("file writer factory poisoned");
        funcs.push(StdioFileWriter::open);
    });
}

pub struct FileWriterFactory;

impl FileWriterFactory {
    pub fn register_writer(open_function: Option<OpenFunction>) -> bool {
        let Some(open_function) = open_function else {
            return false;
        };
        ensure_default_writers();
        let mut funcs = open_functions()
            .lock()
            .expect("file writer factory poisoned");
        let num = funcs.len();
        funcs.push(open_function);
        funcs.len() == num + 1
    }

    pub fn open_writer(file_name: &str) -> Option<Box<dyn FileWriterInterface>> {
        ensure_default_writers();
        let funcs = open_functions()
            .lock()
            .expect("file writer factory poisoned");
        for open_fn in funcs.iter() {
            if let Some(writer) = open_fn(file_name) {
                return Some(writer);
            }
        }
        None
    }
}
