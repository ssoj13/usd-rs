//! UsdMtlx - MaterialX integration for USD.
//!
//! Port of `pxr/usd/usdMtlx/` module. Provides:
//!
//! - **MaterialXConfigAPI** - API schema for MaterialX environment configuration
//! - **Document model** - Arena-based MaterialX document representation
//! - **XML I/O** - Parse .mtlx XML files with XInclude support
//! - **Reader** - Convert MaterialX documents to USD Stage prims
//! - **Utils** - Type conversion, value parsing, search paths, document caching
//! - **SDR plugins** - Parser and Discovery plugins for shader registry
//! - **File format** - SdfFileFormat for .mtlx files
//!
//! ## Optional: mtlx-rs feature
//!
//! Enable the `mtlx-rs` feature for parsing via the full MaterialX library:
//! - XInclude with `FileSearchPath` and cycle detection
//! - `document_from_mtlx_rs()` — convert mtlx-rs Document to schema::mtlx Document
//! - `read_document_mtlx_rs()` — read .mtlx files via mtlx-rs
//! - `read_document()` automatically tries mtlx-rs first when the feature is enabled

mod config_api;
mod tokens;

// MaterialX document model
pub mod document;
pub mod value;
pub mod xml_io;

// MaterialX utilities and reader
pub mod reader;
pub mod utils;

// SDR plugins
pub mod discovery_plugin;
pub mod parser_plugin;

// File format and test utilities
pub mod backdoor;
pub mod file_format;

#[cfg(feature = "mtlx-rs")]
mod mtlx_rs_bridge;

#[cfg(test)]
mod tests;

// Public re-exports
pub use config_api::MaterialXConfigAPI;
pub use tokens::{USD_MTLX_TOKENS, UsdMtlxTokensType};

pub use document::{
    ARRAY_PREFERRED_SEPARATOR, DISPLACEMENT_SHADER_TYPE_STRING, EMPTY_STRING,
    LIGHT_SHADER_TYPE_STRING, SHADER_SEMANTIC, SURFACE_SHADER_TYPE_STRING,
    VOLUME_SHADER_TYPE_STRING,
};
pub use document::{
    Collection, Document, Element, GeomInfo, Input, InterfaceElement, Look, Material,
    MaterialAssign, MtlxError, Node, NodeDef, NodeGraph, Output, TypeDef, TypedElement,
    ValueElement, Variant, VariantSet,
};
pub use value::{MtlxValue, create_value_from_strings, split_string, trim_spaces};
pub use xml_io::{read_from_xml_file, read_from_xml_string};

pub use utils::{
    UsdTypeInfo, custom_search_paths, get_document, get_document_from_string,
    get_packed_usd_values, get_source_uri, get_usd_type, get_usd_value, get_version, read_document,
    search_paths, split_string_array, standard_file_extensions, standard_library_paths,
};

pub use backdoor::{test_file, test_string};
pub use discovery_plugin::MtlxDiscoveryPlugin;
pub use file_format::{MtlxFileFormat, file_format_tokens, register_format};
pub use parser_plugin::MtlxParserPlugin;
pub use reader::{usd_mtlx_read, usd_mtlx_read_node_graphs};
#[cfg(feature = "mtlx-rs")]
pub use utils::read_document_mtlx_rs;

#[cfg(feature = "mtlx-rs")]
pub use mtlx_rs_bridge::document_from_mtlx_rs;
