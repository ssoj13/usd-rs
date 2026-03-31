//! Test utility functions for MaterialX.
//!
//! Port of `pxr/usd/usdMtlx/backdoor.cpp` - provides test utilities for
//! converting MaterialX documents to USD stages from strings and files.
//!
//! # Overview
//!
//! These functions are intended for testing and allow:
//! - Parsing MaterialX from XML strings
//! - Parsing MaterialX from files
//! - Converting to USD stages
//! - Optionally reading only node graphs
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMtlx/backdoor.{h,cpp}`

use std::sync::Arc;
use usd_core::Stage;
use usd_core::common::InitialLoadSet;

/// Parse MaterialX from string and convert to USD stage.
///
/// This is a test utility that:
/// 1. Parses MaterialX XML from string
/// 2. Creates in-memory USD stage
/// 3. Converts MaterialX to USD
///
/// # Arguments
///
/// * `xml` - MaterialX XML content
/// * `node_graphs` - If true, only read node graphs; otherwise read everything
///
/// # Returns
///
/// USD stage with converted MaterialX content, or None on error.
///
/// # Examples
///
/// ```ignore
/// use usd::schema::mtlx::backdoor::test_string;
///
/// let xml = r#"
/// <?xml version="1.0"?>
/// <materialx>
///   <nodedef name="ND_test" node="test"/>
/// </materialx>
/// "#;
///
/// let stage = test_string(xml, false).unwrap();
/// ```
pub fn test_string(xml: &str, node_graphs: bool) -> Option<Arc<Stage>> {
    // Parse MaterialX document from string
    let doc = super::xml_io::read_from_xml_string(xml).ok()?;

    // Create in-memory stage
    let stage =
        Stage::create_in_memory_with_identifier("tmp.usda", InitialLoadSet::LoadAll).ok()?;

    // Convert MaterialX to USD
    if node_graphs {
        super::reader::usd_mtlx_read_node_graphs(&doc, &stage, None);
    } else {
        super::reader::usd_mtlx_read(&doc, &stage, None, None);
    }

    Some(stage)
}

/// Parse MaterialX from file and convert to USD stage.
///
/// This is a test utility that:
/// 1. Parses MaterialX XML from file
/// 2. Creates in-memory USD stage
/// 3. Converts MaterialX to USD
///
/// # Arguments
///
/// * `path` - Path to MaterialX file
/// * `node_graphs` - If true, only read node graphs; otherwise read everything
///
/// # Returns
///
/// USD stage with converted MaterialX content, or None on error.
///
/// # Examples
///
/// ```ignore
/// use usd::schema::mtlx::backdoor::test_file;
///
/// let stage = test_file("test.mtlx", false).unwrap();
/// ```
pub fn test_file(path: &str, node_graphs: bool) -> Option<Arc<Stage>> {
    // Parse MaterialX document from file
    let doc = super::xml_io::read_from_xml_file(path).ok()?;

    // Create in-memory stage
    let stage =
        Stage::create_in_memory_with_identifier("tmp.usda", InitialLoadSet::LoadAll).ok()?;

    // Convert MaterialX to USD
    if node_graphs {
        super::reader::usd_mtlx_read_node_graphs(&doc, &stage, None);
    } else {
        super::reader::usd_mtlx_read(&doc, &stage, None, None);
    }

    Some(stage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_simple() {
        let xml = r#"
<?xml version="1.0"?>
<materialx version="1.38">
  <nodedef name="ND_test" node="test" type="color3">
    <input name="amount" type="float" value="0.5"/>
  </nodedef>
</materialx>
"#;

        // Reader is implemented; parsing a valid .mtlx string must succeed.
        let result = test_string(xml, false);
        assert!(
            result.is_some(),
            "test_string should succeed for valid MaterialX"
        );
    }

    #[test]
    fn test_string_node_graphs() {
        let xml = r#"
<?xml version="1.0"?>
<materialx>
  <nodegraph name="NG_test">
    <input name="in1" type="color3"/>
    <output name="out" type="color3"/>
  </nodegraph>
</materialx>
"#;

        // Reader is implemented; node-graph-only read must also succeed.
        let result = test_string(xml, true);
        assert!(
            result.is_some(),
            "test_string should succeed for node graph MaterialX"
        );
    }

    #[test]
    fn test_file_nonexistent() {
        let result = test_file("/nonexistent/path.mtlx", false);
        assert!(result.is_none());
    }

    #[test]
    fn test_string_invalid_xml() {
        let xml = "<invalid xml";
        let result = test_string(xml, false);
        assert!(result.is_none());
    }

    #[test]
    fn test_string_empty() {
        let xml = "";
        let result = test_string(xml, false);
        assert!(result.is_none());
    }
}
