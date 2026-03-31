//! File reader factory.
//! Reference: `_ref/draco/src/draco/io/file_reader_factory.h` + `.cc`.

use std::sync::{Mutex, Once, OnceLock};

use crate::io::file_reader_interface::FileReaderInterface;
use crate::io::stdio_file_reader::StdioFileReader;

pub type OpenFunction = fn(&str) -> Option<Box<dyn FileReaderInterface>>;

fn open_functions() -> &'static Mutex<Vec<OpenFunction>> {
    static OPEN_FUNCTIONS: OnceLock<Mutex<Vec<OpenFunction>>> = OnceLock::new();
    OPEN_FUNCTIONS.get_or_init(|| Mutex::new(Vec::new()))
}

fn ensure_default_readers() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut funcs = open_functions()
            .lock()
            .expect("file reader factory poisoned");
        funcs.push(StdioFileReader::open);
    });
}

pub struct FileReaderFactory;

impl FileReaderFactory {
    pub fn register_reader(open_function: Option<OpenFunction>) -> bool {
        let Some(open_function) = open_function else {
            return false;
        };
        ensure_default_readers();
        let mut funcs = open_functions()
            .lock()
            .expect("file reader factory poisoned");
        let num = funcs.len();
        funcs.push(open_function);
        funcs.len() == num + 1
    }

    pub fn open_reader(file_name: &str) -> Option<Box<dyn FileReaderInterface>> {
        ensure_default_readers();
        let funcs = open_functions()
            .lock()
            .expect("file reader factory poisoned");
        for open_fn in funcs.iter() {
            if let Some(reader) = open_fn(file_name) {
                return Some(reader);
            }
        }
        None
    }
}
