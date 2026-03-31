//! Round-trip test: read .mtlx -> write -> read again, compare.

use std::path::Path;

use mtlx_rs::format::{read_from_xml_file_path, read_from_xml_str, write_to_xml_string};

const SAMPLE_MTLX: &str = r#"<?xml version="1.0"?>
<materialx version="1.39" colorspace="lin_rec709">
  <standard_surface name="SR_default" type="surfaceshader">
    <input name="base" type="float" value="1.0" />
    <input name="base_color" type="color3" value="0.8, 0.8, 0.8" />
  </standard_surface>
  <surfacematerial name="Default" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="SR_default" />
  </surfacematerial>
</materialx>"#;

#[test]
fn round_trip_read_write() {
    let doc = read_from_xml_str(SAMPLE_MTLX).expect("parse");
    let xml2 = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml2).expect("parse again");

    // Root attributes preserved
    let root1 = doc.get_root();
    let root2 = doc2.get_root();
    assert_eq!(
        root1.borrow().get_attribute("version"),
        root2.borrow().get_attribute("version")
    );
    assert_eq!(
        root1.borrow().get_attribute("colorspace"),
        root2.borrow().get_attribute("colorspace")
    );

    // Children count
    assert_eq!(doc.get_children().len(), doc2.get_children().len());
}

#[test]
fn load_stdlib_defs() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("libraries/stdlib/stdlib_defs.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read and parse stdlib_defs");

    assert_eq!(
        doc.get_root().borrow().get_attribute("version"),
        Some("1.39")
    );
    assert!(
        !doc.get_children().is_empty(),
        "stdlib_defs should have typedefs, nodedefs, etc."
    );
}

#[test]
fn xinclude_import() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/xinclude/root.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read with XInclude");

    assert!(
        doc.get_node_def("ND_IncludedTest").is_some(),
        "nodedef from lib.mtlx should be imported"
    );
    assert!(
        doc.get_material("Main").is_some(),
        "root material should be present"
    );
}
