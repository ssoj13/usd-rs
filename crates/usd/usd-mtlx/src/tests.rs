//! Ported tests from C++ testenv:
//!   - testUsdMtlxFileFormat.py   -> file_format_tests (parsing level)
//!   - testUsdMtlxDiscovery.py    -> discovery_tests
//!   - testUsdMtlxParser.py       -> parser_tests (type mapping)
//!
//! Note: Tests that go through backdoor::test_string/test_file (full reader
//! pipeline) are in reader_tests and require --test-threads=1 due to global
//! Stage state.  Pure parsing/utils tests are safe to run in parallel.

#[cfg(test)]
mod file_format_tests {
    //! Port of testUsdMtlxFileFormat.py — parsing level.
    //! Verifies XML parsing succeeds/fails for various inputs without
    //! going through the full reader pipeline.

    use crate::xml_io::{read_from_xml_file, read_from_xml_string};
    use std::path::PathBuf;

    fn testenv_path(name: &str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("testenv");
        path.push("file_format");
        path.push(name);
        path.to_string_lossy().to_string()
    }

    // === Error cases ===

    #[test]
    fn test_empty_string() {
        let result = read_from_xml_string("");
        assert!(result.is_err(), "Empty string should fail");
    }

    #[test]
    fn test_missing_file() {
        let result = read_from_xml_file("non-existent-file.xml");
        assert!(result.is_err(), "Missing file should fail");
    }

    #[test]
    fn test_invalid_xml() {
        let result = read_from_xml_string("<invalid xml");
        assert!(result.is_err(), "Invalid XML should fail");
    }

    #[test]
    fn test_empty_materialx_document() {
        let doc = read_from_xml_string(
            r#"<?xml version="1.0" ?>
               <materialx version="1.35">
               </materialx>"#,
        );
        assert!(doc.is_ok(), "Empty materialx element should succeed");
        let doc = doc.expect("doc");
        assert_eq!(doc.get_root().category(), "materialx");
        assert_eq!(doc.get_root().get_attribute("version"), "1.35");
    }

    // === .mtlx file parsing ===

    #[test]
    fn test_parse_node_graphs_mtlx() {
        let path = testenv_path("NodeGraphs.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "NodeGraphs.mtlx should parse: {:?}", doc.err());
        let doc = doc.expect("doc");
        let nodegraphs = doc.get_node_graphs();
        assert_eq!(nodegraphs.len(), 3, "Should have 3 nodegraphs");
    }

    #[test]
    fn test_parse_graphless_nodes_mtlx() {
        let path = testenv_path("GraphlessNodes.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(
            doc.is_ok(),
            "GraphlessNodes.mtlx should parse: {:?}",
            doc.err()
        );
    }

    #[test]
    fn test_parse_looks_mtlx() {
        // Looks.mtlx includes SimpleSrf.mtlx via xi:include
        let path = testenv_path("Looks.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "Looks.mtlx should parse: {:?}", doc.err());
        let doc = doc.expect("doc");
        let looks = doc.get_looks();
        assert!(
            looks.len() >= 3,
            "Should have at least 3 looks, got {}",
            looks.len()
        );
    }

    #[test]
    fn test_parse_multi_bind_inputs_mtlx() {
        let path = testenv_path("MultiBindInputs.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "MultiBindInputs.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_output_sources_mtlx() {
        let path = testenv_path("OutputSources.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "OutputSources.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_multi_output_node_mtlx() {
        let path = testenv_path("MultiOutputNode.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "MultiOutputNode.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_local_custom_nodes_mtlx() {
        let path = testenv_path("LocalCustomNodes.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "LocalCustomNodes.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_nested_local_custom_nodes_mtlx() {
        let path = testenv_path("NestedLocalCustomNodes.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "NestedLocalCustomNodes.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_node_graph_inputs_mtlx() {
        let path = testenv_path("NodeGraphInputs.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "NodeGraphInputs.mtlx: {:?}", doc.err());
        let doc = doc.expect("doc");

        // Verify nodegraph has interfacename connections
        let ngs = doc.get_node_graphs();
        assert_eq!(ngs.len(), 1);
        assert_eq!(ngs[0].0.name(), "test_nodegraph");

        // Nodegraph should have an input named "scale"
        let inputs = ngs[0].get_inputs();
        assert!(
            inputs.iter().any(|i| i.0.name() == "scale"),
            "Nodegraph should have 'scale' input"
        );

        // mult1 node should have interfacename="scale" on its in2 input
        // Nodes like <multiply name="mult1"> have category "multiply", not "node"
        let all_children = ngs[0].0.get_children();
        let mult1 = all_children.iter().find(|c| c.name() == "mult1");
        assert!(mult1.is_some(), "Should have mult1 child");
        let mult1_inputs: Vec<_> = mult1
            .expect("mult1")
            .get_children_of_type("input")
            .into_iter()
            .collect();
        let in2 = mult1_inputs.iter().find(|i| i.name() == "in2");
        assert!(in2.is_some(), "mult1 should have in2 input");
        assert_eq!(
            in2.expect("in2").get_attribute("interfacename"),
            "scale",
            "in2 should reference interfacename=scale"
        );
    }

    #[test]
    fn test_parse_xinclude() {
        let path = testenv_path("include/Include.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "include/Include.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_expand_file_prefix_mtlx() {
        let path = testenv_path("ExpandFilePrefix.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "ExpandFilePrefix.mtlx: {:?}", doc.err());
        let doc = doc.expect("doc");

        // Verify nodegraph has fileprefix attribute
        let ngs = doc.get_node_graphs();
        assert_eq!(ngs.len(), 1);
        assert_eq!(
            ngs[0].0.get_attribute("fileprefix"),
            "outer_scope/textures/"
        );

        // image_spec has its own fileprefix
        // <image name="image_spec"> has category "image", not "node"
        let all_children = ngs[0].0.get_children();
        let image_spec = all_children.iter().find(|c| c.name() == "image_spec");
        assert!(image_spec.is_some(), "Should have image_spec child");
        let spec_inputs: Vec<_> = image_spec
            .expect("spec")
            .get_children_of_type("input")
            .into_iter()
            .collect();
        let file_input = spec_inputs.iter().find(|i| i.name() == "file");
        assert!(file_input.is_some());
        assert_eq!(
            file_input.expect("file").get_attribute("fileprefix"),
            "inner_scope/textures/"
        );
    }

    #[test]
    fn test_parse_usd_preview_surface_gold_mtlx() {
        let path = testenv_path("usd_preview_surface_gold.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(
            doc.is_ok(),
            "usd_preview_surface_gold.mtlx: {:?}",
            doc.err()
        );
    }

    #[test]
    fn test_parse_simple_srf_mtlx() {
        let path = testenv_path("SimpleSrf.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(doc.is_ok(), "SimpleSrf.mtlx: {:?}", doc.err());
    }

    #[test]
    fn test_parse_checker_surface_shader_mtlx() {
        let path = testenv_path("CheckerSurfaceShaderNodeDef.mtlx");
        let doc = read_from_xml_file(&path);
        assert!(
            doc.is_ok(),
            "CheckerSurfaceShaderNodeDef.mtlx: {:?}",
            doc.err()
        );
    }

    // === File format capabilities ===

    #[test]
    fn test_capabilities() {
        use usd_sdf::file_format::FileFormat;
        let format = crate::file_format::MtlxFileFormat::new();

        assert!(format.can_read("test.mtlx"));
        assert!(!format.supports_writing());
    }
}

#[cfg(test)]
mod discovery_tests {
    //! Port of testUsdMtlxDiscovery.py — parsing level.

    use crate::read_from_xml_string;
    use std::path::PathBuf;

    fn testenv_discovery_path(name: &str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("testenv");
        path.push("discovery");
        path.push(name);
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_parse_discovery_test_mtlx() {
        let path = testenv_discovery_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read test.mtlx");
        let doc = read_from_xml_string(&content).expect("parse test.mtlx");

        let nodedefs = doc.get_node_defs();
        assert!(!nodedefs.is_empty());

        let names: Vec<_> = nodedefs.iter().map(|nd| nd.0.name().to_string()).collect();
        for expected in &[
            "pxr_nd_boolean",
            "pxr_nd_float",
            "pxr_nd_integer",
            "pxr_nd_matrix33",
            "pxr_nd_string",
            "pxr_nd_vector",
            "pxr_nd_vector_2",
            "pxr_nd_vector_2_1",
            "pxr_nd_vector_noversion",
            "pxr_nd_booleanDefaults",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "Missing nodedef: {}",
                expected
            );
        }
    }

    #[test]
    fn test_version_extraction() {
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="nd_v1" type="float" node="test" version="1.0"/>
                <nodedef name="nd_v2" type="float" node="test" version="2.0" isdefaultversion="true"/>
                <nodedef name="nd_v2_1" type="float" node="test" version="2.1" inherit="nd_v2"/>
                <nodedef name="nd_nover" type="float" node="test"/>
            </materialx>"#;

        let doc = read_from_xml_string(xml).expect("parse");
        for nd in doc.get_node_defs() {
            let (version, implicit_default) = crate::utils::get_version(&nd);
            match nd.0.name() {
                "nd_v1" => {
                    assert!(version.is_valid(), "nd_v1 should be valid");
                    assert!(implicit_default, "nd_v1 has no isdefaultversion");
                }
                "nd_v2" => {
                    assert!(version.is_valid(), "nd_v2 should be valid");
                    assert!(!implicit_default, "nd_v2 has isdefaultversion=true");
                    assert!(version.is_default(), "nd_v2 is default");
                }
                "nd_v2_1" => {
                    assert!(version.is_valid(), "nd_v2_1 should be valid");
                }
                "nd_nover" => {
                    assert!(!version.is_valid(), "nd_nover has no version");
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_name_mapping_inheritance() {
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="mix_float" type="float" node="mix"/>
                <nodedef name="mix_float_200" type="float" node="mix" inherit="mix_float" version="2.0"/>
                <nodedef name="mix_float_210" type="float" node="mix" inherit="mix_float_200" version="2.1"/>
            </materialx>"#;

        let doc = read_from_xml_string(xml).expect("parse");
        let mapping = crate::discovery_plugin::tests::compute_name_mapping_for_test(&doc);

        assert_eq!(mapping.get("mix_float"), Some(&"mix_float".to_string()));
        assert_eq!(mapping.get("mix_float_200"), Some(&"mix_float".to_string()));
        assert_eq!(mapping.get("mix_float_210"), Some(&"mix_float".to_string()));
    }

    #[test]
    fn test_discovery_boolean_defaults() {
        let path = testenv_discovery_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read test.mtlx");
        let doc = read_from_xml_string(&content).expect("parse test.mtlx");

        let nd = doc
            .get_node_def("pxr_nd_booleanDefaults")
            .expect("pxr_nd_booleanDefaults");
        let inputs = nd.get_inputs();

        let true_input = inputs
            .iter()
            .find(|i| i.0.name() == "inTrue")
            .expect("inTrue");
        assert_eq!(true_input.get_value_string(), "true");

        let false_input = inputs
            .iter()
            .find(|i| i.0.name() == "inFalse")
            .expect("inFalse");
        assert_eq!(false_input.get_value_string(), "false");
    }

    #[test]
    fn test_discovery_matrix33_default() {
        let path = testenv_discovery_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read test.mtlx");
        let doc = read_from_xml_string(&content).expect("parse test.mtlx");

        let nd = doc
            .get_node_def("pxr_nd_matrix33")
            .expect("pxr_nd_matrix33");
        let inputs = nd.get_inputs();
        let mat_input = inputs
            .iter()
            .find(|i| i.0.name() == "in")
            .expect("'in' input");
        assert_eq!(
            mat_input.get_value_string(),
            "1.0,2.0,3.0, 4.0,5.0,6.0, 7.0,8.0,9.0"
        );
    }
}

#[cfg(test)]
mod parser_tests {
    //! Port of testUsdMtlxParser.py — type mapping and metadata checks.

    use crate::read_from_xml_string;
    use std::path::PathBuf;

    fn testenv_parser_path(name: &str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("testenv");
        path.push("parser");
        path.push(name);
        path.to_string_lossy().to_string()
    }

    #[test]
    fn test_parse_parser_test_mtlx() {
        let path = testenv_parser_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read test.mtlx");
        let doc = read_from_xml_string(&content).expect("parse test.mtlx");

        let nodedefs = doc.get_node_defs();
        let names: Vec<_> = nodedefs.iter().map(|nd| nd.0.name().to_string()).collect();

        for expected in &[
            "nd_integer",
            "nd_float",
            "nd_string",
            "nd_vector",
            "nd_color3",
            "nd_color4",
            "nd_boolean",
            "nd_customtype",
            "nd_surface",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "Missing: {}",
                expected
            );
        }
    }

    #[test]
    fn test_type_conversion_mapping() {
        let all_exact = [
            "boolean",
            "color3",
            "color4",
            "float",
            "integer",
            "string",
            "vector2",
            "vector3",
            "vector4",
            "matrix33",
            "matrix44",
            "filename",
            "surfaceshader",
        ];

        for mtlx_type in &all_exact {
            let type_info = crate::utils::get_usd_type(mtlx_type);
            assert!(
                type_info.value_type_name.is_valid(),
                "'{}' should be valid",
                mtlx_type
            );
            assert!(
                type_info.value_type_name_is_exact,
                "'{}' should be exact",
                mtlx_type
            );
        }

        let unknown = crate::utils::get_usd_type("unknown_type");
        assert!(!unknown.value_type_name.is_valid());
    }

    #[test]
    fn test_vector_doc_and_ui_metadata() {
        let path = testenv_parser_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read");
        let doc = read_from_xml_string(&content).expect("parse");

        let nd = doc.get_node_def("nd_vector").expect("nd_vector");
        assert_eq!(nd.0.get_attribute("doc"), "Vector help");

        let inputs = nd.get_inputs();
        let in_input = inputs.iter().find(|i| i.0.name() == "in").expect("'in'");
        assert_eq!(in_input.0.get_attribute("doc"), "Property help");
        assert_eq!(in_input.0.get_attribute("uiname"), "UI Vector");
        assert_eq!(in_input.0.get_attribute("uifolder"), "UI Page");
        assert_eq!(
            in_input.0.get_attribute("enum"),
            "X Label, Y Label, Z Label"
        );
        assert_eq!(
            in_input.0.get_attribute("enumvalues"),
            "1,0,0, 0,1,0, 0,0,1"
        );
    }

    #[test]
    fn test_float_unit_metadata() {
        let path = testenv_parser_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read");
        let doc = read_from_xml_string(&content).expect("parse");

        let nd = doc.get_node_def("nd_float").expect("nd_float");
        let inputs = nd.get_inputs();
        let in_input = inputs.iter().find(|i| i.0.name() == "in").expect("'in'");

        assert_eq!(in_input.0.get_attribute("uimin"), "-360.0");
        assert_eq!(in_input.0.get_attribute("uimax"), "360.0");
        assert_eq!(in_input.0.get_attribute("uisoftmin"), "0.0");
        assert_eq!(in_input.0.get_attribute("uisoftmax"), "180.0");
        assert_eq!(in_input.0.get_attribute("uistep"), "1.0");
        assert_eq!(in_input.0.get_attribute("unittype"), "angle");
        assert_eq!(in_input.0.get_attribute("unit"), "degree");
    }

    #[test]
    fn test_common_nodedef_info() {
        let path = testenv_parser_path("test.mtlx");
        let content = std::fs::read_to_string(&path).expect("read");
        let doc = read_from_xml_string(&content).expect("parse");

        assert_eq!(
            doc.get_root().get_attribute("namespace"),
            "UsdMtlxTestNamespace"
        );

        for nd in doc.get_node_defs() {
            assert_eq!(nd.get_node_string(), "UsdMtlxTestNode");

            let input_names: Vec<_> = nd
                .get_inputs()
                .iter()
                .map(|i| i.0.name().to_string())
                .collect();
            assert!(
                input_names.contains(&"in".to_string()),
                "{}: missing 'in'",
                nd.0.name()
            );
            assert!(
                input_names.contains(&"note".to_string()),
                "{}: missing 'note'",
                nd.0.name()
            );

            let output_names: Vec<_> = nd
                .get_outputs()
                .iter()
                .map(|o| o.0.name().to_string())
                .collect();
            assert!(
                output_names.contains(&"out".to_string()),
                "{}: missing 'out'",
                nd.0.name()
            );
        }
    }
}

#[cfg(test)]
mod utils_tests {
    use crate::read_from_xml_string;
    use crate::utils::{get_usd_type, get_usd_value, split_string_array};

    #[test]
    fn test_split_string_array() {
        let result = split_string_array("foo, bar , baz");
        assert_eq!(result, vec!["foo", "bar", "baz"]);
        assert!(split_string_array("").is_empty());
        assert_eq!(split_string_array("single"), vec!["single"]);
    }

    #[test]
    fn test_get_usd_value_float() {
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" node="test">
                    <input name="amount" type="float" value="0.5"/>
                </nodedef>
            </materialx>"#;

        let doc = read_from_xml_string(xml).expect("parse");
        let nd = doc.get_node_def("ND_test").expect("ND_test");
        let input = &nd.get_inputs()[0];
        let value = get_usd_value(&input.0, false);
        assert!(!value.is_empty());
    }

    #[test]
    fn test_get_usd_value_color3() {
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" node="test">
                    <input name="color" type="color3" value="1.0, 0.5, 0.25"/>
                </nodedef>
            </materialx>"#;

        let doc = read_from_xml_string(xml).expect("parse");
        let nd = doc.get_node_def("ND_test").expect("ND_test");
        let value = get_usd_value(&nd.get_inputs()[0].0, false);
        assert!(!value.is_empty());
    }

    #[test]
    fn test_get_usd_value_empty() {
        let xml = r#"<?xml version="1.0"?>
            <materialx version="1.38">
                <nodedef name="ND_test" node="test">
                    <input name="amount" type="float"/>
                </nodedef>
            </materialx>"#;

        let doc = read_from_xml_string(xml).expect("parse");
        let value = get_usd_value(
            &doc.get_node_def("ND_test").expect("nd").get_inputs()[0].0,
            false,
        );
        assert!(value.is_empty());
    }

    #[test]
    fn test_get_usd_type_array_sizes() {
        assert_eq!(get_usd_type("vector2").array_size, 2);
        assert_eq!(get_usd_type("vector3").array_size, 3);
        assert_eq!(get_usd_type("vector4").array_size, 4);
        assert_eq!(get_usd_type("float").array_size, 0);
        assert_eq!(get_usd_type("color3").array_size, 0);
    }

    #[test]
    fn test_get_usd_type_shader_property_types() {
        assert_eq!(
            get_usd_type("color3").shader_property_type.as_str(),
            "Color"
        );
        assert_eq!(
            get_usd_type("color4").shader_property_type.as_str(),
            "Color4"
        );
        assert_eq!(get_usd_type("float").shader_property_type.as_str(), "Float");
        assert_eq!(get_usd_type("integer").shader_property_type.as_str(), "Int");
        assert_eq!(
            get_usd_type("string").shader_property_type.as_str(),
            "String"
        );
        assert_eq!(
            get_usd_type("matrix44").shader_property_type.as_str(),
            "Matrix"
        );
        assert_eq!(
            get_usd_type("surfaceshader").shader_property_type.as_str(),
            "Terminal"
        );
        assert_eq!(
            get_usd_type("filename").shader_property_type.as_str(),
            "String"
        );
        assert_eq!(
            get_usd_type("vector2").shader_property_type.as_str(),
            "Float"
        );
        assert_eq!(
            get_usd_type("vector3").shader_property_type.as_str(),
            "Float"
        );
        assert_eq!(
            get_usd_type("vector4").shader_property_type.as_str(),
            "Float"
        );
        assert_eq!(get_usd_type("boolean").shader_property_type.as_str(), "");
        assert_eq!(get_usd_type("matrix33").shader_property_type.as_str(), "");
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::*;

    #[test]
    fn test_full_materialx_document() {
        let xml = r#"
<?xml version="1.0"?>
<materialx version="1.38" colorspace="lin_rec709">
  <typedef name="customtype" semantic="color" context="shader"/>
  <nodedef name="ND_standard_surface" node="standard_surface" type="surfaceshader">
    <input name="base_color" type="color3" value="0.8, 0.8, 0.8"/>
    <input name="metalness" type="float" value="0.0"/>
    <input name="roughness" type="float" value="0.5"/>
    <output name="out" type="surfaceshader"/>
  </nodedef>
  <nodegraph name="NG_marble" nodedef="ND_marble">
    <input name="scale" type="float" value="1.0"/>
    <output name="out" type="float"/>
  </nodegraph>
  <look name="hero">
    <materialassign name="MA1" material="M_gold" geom="/mesh1"/>
  </look>
</materialx>"#;

        let doc = read_from_xml_string(xml).expect("parse");
        assert_eq!(doc.get_root().get_attribute("colorspace"), "lin_rec709");

        let typedefs = doc.get_type_defs();
        assert_eq!(typedefs.len(), 1);
        assert_eq!(typedefs[0].0.name(), "customtype");

        let nodedefs = doc.get_node_defs();
        assert_eq!(nodedefs.len(), 1);
        assert_eq!(nodedefs[0].get_node_string(), "standard_surface");
        assert_eq!(nodedefs[0].get_inputs().len(), 3);

        let nodegraphs = doc.get_node_graphs();
        assert_eq!(nodegraphs.len(), 1);

        let looks = doc.get_looks();
        assert_eq!(looks.len(), 1);
        assert_eq!(looks[0].get_material_assigns()[0].get_material(), "M_gold");
    }
}
