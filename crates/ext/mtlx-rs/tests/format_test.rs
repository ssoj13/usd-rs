//! Comprehensive tests for the Format module.
//! Ported from MaterialXTest/MaterialXFormat (File.cpp, XmlIo.cpp).

use std::path::PathBuf;

use mtlx_rs::format::{
    FilePath, FileSearchPath, XmlReadOptions, XmlWriteOptions, load_libraries, load_library,
    read_from_xml_file, read_from_xml_file_path, read_from_xml_str, read_from_xml_str_with_options,
    write_to_xml_string, write_to_xml_string_with_options,
};
use mtlx_rs::format::{
    MATERIALX_SEARCH_PATH_ENV_VAR, get_environ, read_file, remove_environ, set_environ,
};

/// Helper: crate root directory (where Cargo.toml lives).
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Helper: path to libraries/ directory inside the crate.
fn libraries_dir() -> PathBuf {
    crate_root().join("libraries")
}

// ═══════════════════════════════════════════════════════════════════════════
// FilePath — basic properties
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn filepath_new_and_as_str() {
    let fp = FilePath::new("foo/bar/baz.mtlx");
    assert_eq!(fp.as_str(), "foo/bar/baz.mtlx");
}

#[test]
fn filepath_empty() {
    let fp = FilePath::default();
    assert!(fp.is_empty());
}

#[test]
fn filepath_not_empty() {
    let fp = FilePath::new("something");
    assert!(!fp.is_empty());
}

#[test]
fn filepath_from_str() {
    let fp: FilePath = "hello/world".into();
    assert_eq!(fp.as_str(), "hello/world");
}

#[test]
fn filepath_as_path() {
    let fp = FilePath::new("foo/bar");
    let p = fp.as_path();
    assert_eq!(p.to_str().unwrap(), "foo/bar");
}

#[test]
fn filepath_is_absolute() {
    if cfg!(windows) {
        let fp = FilePath::new("C:\\absolute\\path");
        assert!(fp.is_absolute());
    } else {
        let fp = FilePath::new("/absolute/path");
        assert!(fp.is_absolute());
    }
    let rel = FilePath::new("relative/path");
    assert!(!rel.is_absolute());
}

#[test]
fn filepath_parent_path() {
    let fp = FilePath::new("a/b/c.txt");
    let parent = fp.get_parent_path();
    assert_eq!(parent.as_str(), "a/b");
}

#[test]
fn filepath_parent_of_single_component() {
    let fp = FilePath::new("file.txt");
    let parent = fp.get_parent_path();
    assert!(parent.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// FilePath — as_string format conversion (ported from C++ "Syntactic operations")
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn filepath_as_str_roundtrip() {
    let paths = [
        "Assets/Materials/Robot.mtlx",
        "Materials/Robot.mtlx",
        "a/b/c.mtlx",
    ];
    for input in &paths {
        let fp = FilePath::new(input);
        assert_eq!(fp.as_str(), *input);
    }
}

#[test]
fn filepath_equality() {
    let a = FilePath::new("foo/bar");
    let b = FilePath::new("foo/bar");
    let c = FilePath::new("baz/qux");
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn filepath_clone() {
    let a = FilePath::new("test");
    let b = a.clone();
    assert_eq!(a, b);
}

// ═══════════════════════════════════════════════════════════════════════════
// FileSearchPath — basic operations
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn search_path_new_is_empty() {
    let sp = FileSearchPath::new();
    assert!(sp.is_empty());
}

#[test]
fn search_path_append() {
    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new("path1"));
    sp.append(FilePath::new("path2"));
    assert!(!sp.is_empty());

    let collected: Vec<&str> = sp.paths_iter().map(|p| p.as_str()).collect();
    assert_eq!(collected, vec!["path1", "path2"]);
}

#[test]
fn search_path_prepend() {
    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new("b"));
    sp.append(FilePath::new("c"));
    sp.prepend(FilePath::new("a"));

    let collected: Vec<&str> = sp.paths_iter().map(|p| p.as_str()).collect();
    assert_eq!(collected, vec!["a", "b", "c"]);
}

#[test]
fn search_path_append_search_path() {
    let mut sp1 = FileSearchPath::new();
    sp1.append(FilePath::new("a"));

    let mut sp2 = FileSearchPath::new();
    sp2.append(FilePath::new("b"));
    sp2.append(FilePath::new("c"));

    sp1.append_search_path(&sp2);
    let collected: Vec<&str> = sp1.paths_iter().map(|p| p.as_str()).collect();
    assert_eq!(collected, vec!["a", "b", "c"]);
}

#[test]
fn search_path_iteration() {
    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new("x"));
    sp.append(FilePath::new("y"));

    // paths_iter
    let mut count = 0;
    for _p in sp.paths_iter() {
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn search_path_find_real_file() {
    // Add libraries/stdlib as a search path, then find stdlib_defs.mtlx
    let stdlib_dir = libraries_dir().join("stdlib");
    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new(&stdlib_dir));

    let found = sp.find("stdlib_defs.mtlx");
    assert!(found.is_some(), "Should find stdlib_defs.mtlx");
    // The found file should actually exist on disk
    let found_path = found.unwrap();
    assert!(
        found_path.as_path().exists(),
        "Found path should exist on disk"
    );
}

#[test]
fn search_path_find_nonexistent() {
    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new(libraries_dir()));
    let found = sp.find("__absolutely_nonexistent__.mtlx");
    assert!(found.is_none(), "Should not find non-existent file");
}

#[test]
fn search_path_find_empty_search_path() {
    let sp = FileSearchPath::new();
    let found = sp.find("some_file.mtlx");
    // Empty search path: nothing to search
    assert!(found.is_none());
}

#[test]
fn search_path_find_in_multiple_paths() {
    // First path has nothing, second has the file
    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new("__fake_dir__"));
    sp.append(FilePath::new(libraries_dir().join("stdlib")));

    let found = sp.find("stdlib_defs.mtlx");
    assert!(found.is_some(), "Should find in second search path");
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — Read/Write round-trip (ported from C++ "Load content")
// ═══════════════════════════════════════════════════════════════════════════

const MINIMAL_MTLX: &str = r#"<?xml version="1.0"?>
<materialx version="1.39" colorspace="lin_rec709">
  <standard_surface name="SR_brass" type="surfaceshader">
    <input name="base" type="float" value="1.0" />
    <input name="base_color" type="color3" value="0.944, 0.776, 0.373" />
    <input name="specular" type="float" value="1.0" />
    <input name="specular_roughness" type="float" value="0.2" />
    <input name="metalness" type="float" value="1.0" />
  </standard_surface>
  <surfacematerial name="Brass" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="SR_brass" />
  </surfacematerial>
</materialx>"#;

#[test]
fn xml_read_from_string() {
    let doc = read_from_xml_str(MINIMAL_MTLX).expect("parse");
    let root = doc.get_root();
    assert_eq!(root.borrow().get_attribute("version"), Some("1.39"));
    assert_eq!(
        root.borrow().get_attribute("colorspace"),
        Some("lin_rec709")
    );
    assert!(!doc.get_children().is_empty());
}

#[test]
fn xml_read_write_round_trip() {
    // Read from string
    let doc = read_from_xml_str(MINIMAL_MTLX).expect("parse");

    // Write to XML string
    let xml_out = write_to_xml_string(&doc).expect("write");

    // Read back
    let doc2 = read_from_xml_str(&xml_out).expect("re-parse");

    // Verify root attributes match
    let r1 = doc.get_root();
    let r2 = doc2.get_root();
    assert_eq!(
        r1.borrow().get_attribute("version"),
        r2.borrow().get_attribute("version"),
    );
    assert_eq!(
        r1.borrow().get_attribute("colorspace"),
        r2.borrow().get_attribute("colorspace"),
    );

    // Same number of children
    assert_eq!(doc.get_children().len(), doc2.get_children().len());
}

#[test]
fn xml_read_newlines_when_requested() {
    use mtlx_rs::core::element::category;

    let opts = XmlReadOptions {
        read_xinclude: true,
        search_path: None,
        parent_xincludes: vec![],
        read_comments: false,
        read_newlines: true,
        upgrade_version: true,
    };
    let doc = read_from_xml_str_with_options(MINIMAL_MTLX, &opts).expect("parse with newlines");
    let root = doc.get_root();
    assert!(
        root.borrow()
            .get_children()
            .iter()
            .any(|child| child.borrow().get_category() == category::NEWLINE),
        "newline nodes should be preserved when read_newlines is enabled"
    );
}

#[test]
fn xml_write_options_xinclude_disabled() {
    let doc = read_from_xml_str(MINIMAL_MTLX).expect("parse");

    let mut opts = XmlWriteOptions::default();
    opts.write_xinclude_enable = false;
    let xml = write_to_xml_string_with_options(&doc, &opts).expect("write");

    assert!(
        !xml.contains("xi:include"),
        "Output should not contain xi:include when disabled"
    );
}

#[test]
fn xml_write_xml_declaration() {
    let doc = read_from_xml_str(MINIMAL_MTLX).expect("parse");
    let xml = write_to_xml_string(&doc).expect("write");
    assert!(
        xml.starts_with("<?xml version=\"1.0\"?>"),
        "Output should start with XML declaration"
    );
}

#[test]
fn xml_read_from_file() {
    // Read a real .mtlx library file
    let path = libraries_dir().join("stdlib").join("stdlib_defs.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read stdlib_defs");

    let root = doc.get_root();
    assert_eq!(root.borrow().get_attribute("version"), Some("1.39"));
    assert!(!doc.get_children().is_empty());
}

#[test]
fn xml_read_with_search_path() {
    // Read using a search path
    let sp = {
        let mut s = FileSearchPath::new();
        s.append(FilePath::new(libraries_dir().join("stdlib")));
        s
    };
    let doc = read_from_xml_file("stdlib_defs.mtlx", sp, None).expect("read via search path");

    assert!(
        !doc.get_children().is_empty(),
        "should have read content via search path"
    );
}

#[test]
fn xml_read_nonexistent_file() {
    let sp = FileSearchPath::new();
    let result = read_from_xml_file("__nonexistent__.mtlx", sp, None);
    assert!(result.is_err(), "Reading non-existent file should fail");
}

#[test]
fn xml_read_invalid_root() {
    let result = read_from_xml_str("<not_materialx />");
    assert!(result.is_err(), "Non-materialx root should fail");
}

#[test]
fn xml_read_empty_string() {
    let result = read_from_xml_str("");
    assert!(result.is_err(), "Empty XML should fail");
}

#[test]
fn xml_read_write_verify_children() {
    let doc = read_from_xml_str(MINIMAL_MTLX).expect("parse");

    // Verify specific children
    assert!(doc.get_child("SR_brass").is_some());
    assert!(doc.get_child("Brass").is_some());

    // Write and re-read
    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");
    assert!(doc2.get_child("SR_brass").is_some());
    assert!(doc2.get_child("Brass").is_some());
}

#[test]
fn xml_read_preserves_input_attributes() {
    let doc = read_from_xml_str(MINIMAL_MTLX).expect("parse");
    let sr = doc.get_child("SR_brass").expect("SR_brass");

    // Check the base_color input
    let children = sr.borrow().get_children().to_vec();
    let base_color = children
        .iter()
        .find(|c| c.borrow().get_name() == "base_color")
        .expect("base_color input");
    assert_eq!(base_color.borrow().get_attribute("type"), Some("color3"));
    assert_eq!(
        base_color.borrow().get_attribute("value"),
        Some("0.944, 0.776, 0.373")
    );
}

#[test]
fn xml_read_same_string_twice() {
    // Reading the same content twice should not crash
    let doc1 = read_from_xml_str(MINIMAL_MTLX).expect("first parse");
    let doc2 = read_from_xml_str(MINIMAL_MTLX).expect("second parse");
    assert_eq!(doc1.get_children().len(), doc2.get_children().len());
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — XInclude (ported from C++ XInclude tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_xinclude_import() {
    let root_mtlx = crate_root()
        .join("tests")
        .join("fixtures")
        .join("xinclude")
        .join("root.mtlx");
    let doc = read_from_xml_file_path(&root_mtlx).expect("read with XInclude");

    // nodedef from lib.mtlx should be imported via XInclude
    assert!(
        doc.get_node_def("ND_IncludedTest").is_some(),
        "XInclude should import ND_IncludedTest"
    );
    // Direct content from root.mtlx
    assert!(
        doc.get_material("Main").is_some(),
        "root material should exist"
    );
}

#[test]
fn xml_xinclude_disabled_via_options() {
    let root_mtlx = crate_root()
        .join("tests")
        .join("fixtures")
        .join("xinclude")
        .join("root.mtlx");

    let opts = XmlReadOptions {
        read_xinclude: false,
        search_path: None,
        parent_xincludes: vec![],
        read_comments: false,
        read_newlines: false,
        upgrade_version: true,
    };
    let xml_content = std::fs::read_to_string(&root_mtlx).unwrap();
    let doc = read_from_xml_str_with_options(&xml_content, &opts).expect("read without XInclude");

    // Without XInclude, lib.mtlx content should NOT be present
    assert!(
        doc.get_node_def("ND_IncludedTest").is_none(),
        "With XInclude disabled, ND_IncludedTest should not be imported"
    );
}

#[test]
fn xml_xinclude_with_read_options() {
    // Read with XInclude enabled via options and explicit search path
    let fixture_dir = crate_root().join("tests").join("fixtures").join("xinclude");

    let xml_content = std::fs::read_to_string(fixture_dir.join("root.mtlx")).unwrap();

    let mut sp = FileSearchPath::new();
    sp.append(FilePath::new(&fixture_dir));

    let opts = XmlReadOptions {
        read_xinclude: true,
        search_path: Some(sp),
        parent_xincludes: vec![],
        read_comments: false,
        read_newlines: false,
        upgrade_version: true,
    };
    let doc = read_from_xml_str_with_options(&xml_content, &opts).expect("read with XInclude opts");
    assert!(doc.get_node_def("ND_IncludedTest").is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — Read libraries and validate
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_read_stdlib_validate() {
    let path = libraries_dir().join("stdlib").join("stdlib_defs.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read stdlib_defs");
    assert!(doc.validate(), "stdlib_defs should be valid");
}

#[test]
fn xml_read_pbrlib_validate() {
    let path = libraries_dir().join("pbrlib").join("pbrlib_defs.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read pbrlib_defs");
    assert!(doc.validate(), "pbrlib_defs should be valid");
}

#[test]
fn xml_round_trip_stdlib() {
    let path = libraries_dir().join("stdlib").join("stdlib_defs.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read");
    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");

    assert_eq!(
        doc.get_children().len(),
        doc2.get_children().len(),
        "Round-trip should preserve child count"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — Various document structures
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_nodegraph_round_trip() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodegraph name="NG_test">
    <constant name="c1" type="float">
      <input name="value" type="float" value="0.5" />
    </constant>
    <output name="out" type="float" nodename="c1" />
  </nodegraph>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    assert!(doc.get_node_graph("NG_test").is_some());
    assert!(doc.validate());

    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");
    assert!(doc2.get_node_graph("NG_test").is_some());
}

#[test]
fn xml_nodedef_round_trip() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_mynode" node="mynode" type="color3">
    <input name="in1" type="float" value="1.0" />
    <input name="in2" type="color3" value="0.5, 0.5, 0.5" />
    <output name="out" type="color3" />
  </nodedef>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    assert!(doc.get_node_def("ND_mynode").is_some());

    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");
    assert!(doc2.get_node_def("ND_mynode").is_some());

    // Check attributes survive round-trip
    let nd = doc2.get_node_def("ND_mynode").unwrap();
    assert_eq!(nd.borrow().get_attribute("node"), Some("mynode"));
    assert_eq!(nd.borrow().get_attribute("type"), Some("color3"));
}

#[test]
fn xml_material_round_trip() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <standard_surface name="SR_gold" type="surfaceshader">
    <input name="base_color" type="color3" value="1.0, 0.8, 0.0" />
    <input name="metalness" type="float" value="1.0" />
  </standard_surface>
  <surfacematerial name="Gold" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="SR_gold" />
  </surfacematerial>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    assert!(doc.get_material("Gold").is_some());

    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");
    assert!(doc2.get_material("Gold").is_some());
}

#[test]
fn xml_multiple_nodedefs() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_a" node="a" type="float">
    <output name="out" type="float" />
  </nodedef>
  <nodedef name="ND_b" node="b" type="color3">
    <output name="out" type="color3" />
  </nodedef>
  <nodedef name="ND_c" node="c" type="vector3">
    <output name="out" type="vector3" />
  </nodedef>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    assert!(doc.get_node_def("ND_a").is_some());
    assert!(doc.get_node_def("ND_b").is_some());
    assert!(doc.get_node_def("ND_c").is_some());
    assert_eq!(doc.get_children().len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — import_library
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_import_library() {
    let lib_mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_lib_node" node="lib_node" type="float">
    <input name="x" type="float" value="0" />
    <output name="out" type="float" />
  </nodedef>
</materialx>"#;

    let main_mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <surfacematerial name="Main" type="material" />
</materialx>"#;

    let lib = read_from_xml_str(lib_mtlx).expect("parse lib");
    let mut doc = read_from_xml_str(main_mtlx).expect("parse main");

    assert!(doc.get_node_def("ND_lib_node").is_none());
    doc.import_library(&lib);
    assert!(doc.get_node_def("ND_lib_node").is_some());
}

#[test]
fn xml_import_library_twice_no_duplicate() {
    let lib_mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_unique" node="unique" type="float">
    <output name="out" type="float" />
  </nodedef>
</materialx>"#;

    let lib = read_from_xml_str(lib_mtlx).expect("parse lib");
    let mut doc = mtlx_rs::core::document::create_document();

    doc.import_library(&lib);
    let count1 = doc.get_children().len();

    doc.import_library(&lib);
    let count2 = doc.get_children().len();

    assert_eq!(
        count1, count2,
        "Importing same library twice should skip duplicates"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — validate
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_validate_empty_doc() {
    let doc = mtlx_rs::core::document::create_document();
    assert!(doc.validate(), "Empty document should be valid");
}

#[test]
fn xml_validate_valid_nodegraph() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodegraph name="NG_valid">
    <constant name="c1" type="float">
      <input name="value" type="float" value="1.0" />
    </constant>
    <output name="out" type="float" nodename="c1" />
  </nodegraph>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    assert!(doc.validate());
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — attribute escaping
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_attribute_with_special_chars() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <input name="test_amp" type="string" value="a &amp; b" />
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    let xml_out = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml_out).expect("re-parse");
    assert_eq!(doc.get_children().len(), doc2.get_children().len());
}

// ═══════════════════════════════════════════════════════════════════════════
// Util — load_library / load_libraries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn util_load_library_single_file() {
    let mut doc = mtlx_rs::core::document::create_document();
    let path = libraries_dir().join("stdlib").join("stdlib_defs.mtlx");
    load_library(&mut doc, &path).expect("load library");

    assert!(
        !doc.get_children().is_empty(),
        "load_library should import content"
    );
}

#[test]
fn util_load_libraries_stdlib() {
    let mut doc = mtlx_rs::core::document::create_document();
    let search = vec![libraries_dir()];
    let loaded = load_libraries(&mut doc, &search, &["stdlib"]).expect("load stdlib");

    assert!(!loaded.is_empty(), "Should have loaded .mtlx files");
    assert!(!doc.get_children().is_empty());
}

#[test]
fn util_load_libraries_multiple_folders() {
    let mut doc = mtlx_rs::core::document::create_document();
    let search = vec![libraries_dir()];
    let loaded =
        load_libraries(&mut doc, &search, &["stdlib", "pbrlib"]).expect("load stdlib+pbrlib");

    assert!(loaded.len() >= 2, "Should load from both folders");
}

#[test]
fn util_load_libraries_nonexistent_folder() {
    let mut doc = mtlx_rs::core::document::create_document();
    let search = vec![libraries_dir()];
    let loaded =
        load_libraries(&mut doc, &search, &["__no_such_lib__"]).expect("nonexistent folder");
    assert!(loaded.is_empty());
}

#[test]
fn util_load_libraries_all() {
    let mut doc = mtlx_rs::core::document::create_document();
    let search = vec![libraries_dir()];
    let loaded = load_libraries(&mut doc, &search, &[]).expect("load all");

    assert!(!loaded.is_empty(), "Should load files when scanning all");
}

// ═══════════════════════════════════════════════════════════════════════════
// Environ — environment variable utilities
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn environ_set_get_remove() {
    let key = "MTLX_RS_TEST_VAR_12345";

    assert!(get_environ(key).is_empty());

    set_environ(key, "test_value");
    assert_eq!(get_environ(key), "test_value");

    remove_environ(key);
    assert!(get_environ(key).is_empty());
}

#[test]
fn environ_materialx_search_path() {
    let key = MATERIALX_SEARCH_PATH_ENV_VAR;
    let original = get_environ(key);

    let test_path = libraries_dir().to_string_lossy().to_string();
    set_environ(key, &test_path);

    let env_path = mtlx_rs::format::get_environment_path();
    assert!(!env_path.is_empty());

    // Restore original
    if original.is_empty() {
        remove_environ(key);
    } else {
        set_environ(key, &original);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// read_file utility
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn util_read_file() {
    let fp = FilePath::new(libraries_dir().join("stdlib").join("stdlib_defs.mtlx"));
    let content = read_file(&fp);
    assert!(!content.is_empty());
    assert!(content.contains("materialx"));
}

#[test]
fn util_read_file_nonexistent() {
    let fp = FilePath::new("__no_such_file__.mtlx");
    let content = read_file(&fp);
    assert!(content.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — source URI is set when reading from file
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_source_uri_set_on_file_read() {
    let path = libraries_dir().join("stdlib").join("stdlib_defs.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read");

    let root = doc.get_root();
    let root_ref = root.borrow();
    let uri = root_ref.get_source_uri();
    assert!(
        uri.is_some(),
        "Source URI should be set after reading from file"
    );
    let uri_str = uri.unwrap().to_string();
    assert!(
        uri_str.contains("stdlib_defs"),
        "Source URI should contain the filename"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — complex documents with nested elements
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_nested_nodegraph() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodegraph name="NG_outer">
    <constant name="c1" type="color3">
      <input name="value" type="color3" value="1, 0, 0" />
    </constant>
    <multiply name="m1" type="color3">
      <input name="in1" type="color3" nodename="c1" />
      <input name="in2" type="float" value="0.5" />
    </multiply>
    <output name="out" type="color3" nodename="m1" />
  </nodegraph>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    let ng = doc.get_node_graph("NG_outer").expect("nodegraph");

    // Check that children of the nodegraph are present
    let children = ng.borrow().get_children().to_vec();
    assert_eq!(children.len(), 3, "Should have c1, m1, out");

    // Round-trip
    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");
    let ng2 = doc2
        .get_node_graph("NG_outer")
        .expect("nodegraph after round-trip");
    assert_eq!(ng2.borrow().get_children().len(), 3);
}

#[test]
fn xml_look_and_material_assign() {
    let mtlx = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <surfacematerial name="Mat1" type="material" />
  <look name="MainLook">
    <materialassign name="ma1" material="Mat1" geom="/scene/mesh" />
  </look>
</materialx>"#;

    let doc = read_from_xml_str(mtlx).expect("parse");
    assert!(doc.get_material("Mat1").is_some());
    assert!(doc.get_look("MainLook").is_some());

    let xml = write_to_xml_string(&doc).expect("write");
    let doc2 = read_from_xml_str(&xml).expect("re-parse");
    assert!(doc2.get_look("MainLook").is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — read from xml_file with different options
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_read_file_with_explicit_options() {
    let path = libraries_dir().join("stdlib").join("stdlib_defs.mtlx");
    let sp = {
        let mut s = FileSearchPath::new();
        s.append(FilePath::new(libraries_dir().join("stdlib")));
        s
    };
    let opts = XmlReadOptions {
        read_xinclude: false,
        search_path: None,
        parent_xincludes: vec![],
        read_comments: false,
        read_newlines: false,
        upgrade_version: true,
    };
    let doc = read_from_xml_file(&path, sp, Some(&opts)).expect("read with opts");
    assert!(!doc.get_children().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// FilePath — read_file for real library files
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn read_bxdf_file() {
    let path = libraries_dir().join("bxdf").join("standard_surface.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read bxdf file");
    assert!(!doc.get_children().is_empty());
}

#[test]
fn read_pbrlib_ng_file() {
    let path = libraries_dir().join("pbrlib").join("pbrlib_ng.mtlx");
    let doc = read_from_xml_file_path(&path).expect("read pbrlib_ng");
    assert!(!doc.get_children().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════
// XML I/O — write options default state
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn xml_write_options_default() {
    let opts = XmlWriteOptions::default();
    // Default should have xinclude enabled (matches C++ MaterialX)
    assert!(opts.write_xinclude_enable);
}

#[test]
fn xml_read_options_default() {
    let opts = XmlReadOptions::default();
    assert!(!opts.read_xinclude);
    assert!(opts.search_path.is_none());
    assert!(opts.parent_xincludes.is_empty());
}
