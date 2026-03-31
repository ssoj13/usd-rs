//! MaterialXFormat -- XML I/O, File, FilePath, Environ.

pub mod environ;
pub mod file;
pub mod util;
pub mod xml_io;

pub use environ::{
    MATERIALX_SEARCH_PATH_ENV_VAR, get_environ, get_environ_opt, remove_environ, set_environ,
};
pub use file::{
    FilePath, FileSearchPath, PATH_LIST_SEPARATOR, PathFormat, get_environment_path,
    get_environment_path_with_sep, read_file,
};
pub use util::{
    flatten_filenames, get_default_data_search_path, get_source_search_path, get_subdirectories,
    load_documents, load_documents_with_options, load_libraries, load_libraries_with_options,
    load_library, load_library_with_options,
};
pub use xml_io::{
    ElementPredicate, MAX_XML_TREE_DEPTH, MTLX_EXTENSION, XmlError, XmlReadOptions,
    XmlWriteOptions, prepend_xinclude, read_from_xml_buffer, read_from_xml_file,
    read_from_xml_file_path, read_from_xml_str, read_from_xml_str_with_options,
    read_from_xml_stream, write_to_xml_file, write_to_xml_file_with_options, write_to_xml_stream,
    write_to_xml_string, write_to_xml_string_with_options,
};
