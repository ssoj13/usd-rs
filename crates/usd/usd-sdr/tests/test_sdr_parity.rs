//! Parity tests for usd-sdr, ported from C++ reference tests.
//!
//! Sources:
//! - testSdrVersion.py
//! - testSdrFilesystemDiscovery.py  
//! - testSdrParseValue.cpp
//! - testSdrRegistry.py (partial - what doesn't need plugin infrastructure)
//! - testSdrShaderNodeQuery.py (partial)

use usd_sdr::*;
use usd_tf::Token;
use usd_vt::Value;

// ============================================================================
// testSdrVersion.py port
// ============================================================================

#[cfg(test)]
mod test_version {
    use super::*;

    fn relational_tests_equal(lhs: &SdrVersion, rhs: &SdrVersion) {
        assert_eq!(lhs, rhs);
        assert_eq!(rhs, lhs);
        assert!(!(lhs != rhs));
        assert!(!(rhs != lhs));
        assert!(!(lhs < rhs));
        assert!(lhs <= rhs);
        assert!(!(lhs > rhs));
        assert!(lhs >= rhs);
    }

    fn relational_tests_less(lhs: &SdrVersion, rhs: &SdrVersion) {
        assert_ne!(lhs, rhs);
        assert_ne!(rhs, lhs);
        assert!(lhs != rhs);
        assert!(rhs != lhs);
        assert!(lhs < rhs);
        assert!(lhs <= rhs);
        assert!(!(lhs > rhs));
        assert!(!(lhs >= rhs));
    }

    #[test]
    fn test_invalid_version() {
        let v = SdrVersion::invalid();
        assert!(!v.is_valid());
        assert!(!v.is_default());
        relational_tests_equal(&v, &SdrVersion::invalid());
        assert_eq!(v.get_string(), "<invalid version>");
    }

    #[test]
    fn test_invalid_default_version() {
        let v = SdrVersion::invalid();
        let u = v.as_default();
        assert!(!u.is_valid());
        assert!(u.is_default());
        relational_tests_equal(&u, &SdrVersion::invalid());
        assert_eq!(u.get_string(), "<invalid version>");
    }

    #[test]
    fn test_valid_versions() {
        let v1 = SdrVersion::new(1, 0);
        let v1_0 = SdrVersion::new(1, 0).as_default();
        let v1_1 = SdrVersion::new(1, 1);
        let v2_0 = SdrVersion::new(2, 0);

        assert!(v1.is_valid());
        assert!(v1_0.is_valid());
        assert!(v1_1.is_valid());
        assert!(v2_0.is_valid());

        assert!(!v1.is_default());
        assert!(v1_0.is_default());
        assert!(!v1_1.is_default());
        assert!(!v2_0.is_default());
    }

    #[test]
    fn test_version_strings() {
        let v1 = SdrVersion::new(1, 0);
        let v1_0 = SdrVersion::new(1, 0).as_default();
        let v1_1 = SdrVersion::new(1, 1);
        let v2_0 = SdrVersion::new(2, 0);

        assert_eq!(v1.get_string(), "1");
        assert_eq!(v1_0.get_string(), "1");
        assert_eq!(v1_1.get_string(), "1.1");
        assert_eq!(v2_0.get_string(), "2");
    }

    #[test]
    fn test_version_string_suffix() {
        let v1 = SdrVersion::new(1, 0);
        let v1_0 = SdrVersion::new(1, 0).as_default();
        let v1_1 = SdrVersion::new(1, 1);
        let v2_0 = SdrVersion::new(2, 0);

        assert_eq!(v1.get_string_suffix(), "_1");
        assert_eq!(v1_0.get_string_suffix(), "");
        assert_eq!(v1_1.get_string_suffix(), "_1.1");
        assert_eq!(v2_0.get_string_suffix(), "_2");
    }

    #[test]
    fn test_version_relational_equal() {
        let v1 = SdrVersion::new(1, 0);
        let v1_0 = SdrVersion::new(1, 0).as_default();
        let v1_1 = SdrVersion::new(1, 1);
        let v2_0 = SdrVersion::new(2, 0);

        relational_tests_equal(&v1, &v1);
        relational_tests_equal(&v1_0, &v1_0);
        // v1 and v1_0 should be equal (default flag not compared)
        relational_tests_equal(&v1, &v1_0);
        relational_tests_equal(&v1_1, &v1_1);
        relational_tests_equal(&v2_0, &v2_0);
    }

    #[test]
    fn test_version_relational_less() {
        let v1 = SdrVersion::new(1, 0);
        let v1_0 = SdrVersion::new(1, 0).as_default();
        let v1_1 = SdrVersion::new(1, 1);
        let v2_0 = SdrVersion::new(2, 0);

        relational_tests_less(&v1, &v1_1);
        relational_tests_less(&v1_0, &v1_1);
        relational_tests_less(&v1_0, &v2_0);
        relational_tests_less(&v1_1, &v2_0);
    }
}

// ============================================================================
// testSdrFilesystemDiscovery.py port - split_shader_identifier
// ============================================================================

#[cfg(test)]
mod test_split_shader_identifier {
    use super::*;

    fn check_split(
        id_str: &str,
        expected_family: &str,
        expected_name: &str,
        expected_version: SdrVersion,
    ) {
        let identifier = Token::new(id_str);
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();

        let result = split_shader_identifier(&identifier, &mut family, &mut name, &mut version);
        assert!(result, "split_shader_identifier failed for '{}'", id_str);
        assert_eq!(
            family.as_str(),
            expected_family,
            "family mismatch for '{}'",
            id_str
        );
        assert_eq!(
            name.as_str(),
            expected_name,
            "name mismatch for '{}'",
            id_str
        );
        assert_eq!(
            version, expected_version,
            "version mismatch for '{}'",
            id_str
        );
    }

    #[test]
    fn test_simple_identifier() {
        // 'Primvar' -> family='Primvar', name='Primvar', version=invalid
        check_split("Primvar", "Primvar", "Primvar", SdrVersion::invalid());
    }

    #[test]
    fn test_identifier_with_type() {
        // 'Primvar_float2' -> family='Primvar', name='Primvar_float2', version=invalid
        check_split(
            "Primvar_float2",
            "Primvar",
            "Primvar_float2",
            SdrVersion::invalid(),
        );
    }

    #[test]
    fn test_identifier_with_major_version() {
        // 'Primvar_float2_3' -> family='Primvar', name='Primvar_float2', version=3.0
        check_split(
            "Primvar_float2_3",
            "Primvar",
            "Primvar_float2",
            SdrVersion::new(3, 0),
        );
    }

    #[test]
    fn test_identifier_with_major_minor_version() {
        // 'Primvar_float_3_4' -> family='Primvar', name='Primvar_float', version=3.4
        check_split(
            "Primvar_float_3_4",
            "Primvar",
            "Primvar_float",
            SdrVersion::new(3, 4),
        );
    }

    #[test]
    fn test_invalid_penultimate_number_last_not() {
        // 'Primvar_float2_3_nonNumber' -> should fail
        let identifier = Token::new("Primvar_float2_3_nonNumber");
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();
        let result = split_shader_identifier(&identifier, &mut family, &mut name, &mut version);
        assert!(!result, "should fail for 'Primvar_float2_3_nonNumber'");
    }

    #[test]
    fn test_invalid_number_then_non_number() {
        // 'Primvar_4_nonNumber' -> should fail
        let identifier = Token::new("Primvar_4_nonNumber");
        let mut family = Token::default();
        let mut name = Token::default();
        let mut version = SdrVersion::default();
        let result = split_shader_identifier(&identifier, &mut family, &mut name, &mut version);
        assert!(!result, "should fail for 'Primvar_4_nonNumber'");
    }
}

// ============================================================================
// testSdrParseValue.cpp port (partial - tests that don't need SDF parsing)
// ============================================================================

#[cfg(test)]
mod test_sdf_type_indicator {
    use super::*;

    #[test]
    fn test_equality_ignores_has_sdf_type() {
        // C++ operator== only compares sdf_type and sdr_type, NOT has_sdf_type_mapping
        let a = SdrSdfTypeIndicator::with_types(
            usd_sdf::ValueTypeName::default(),
            Token::new("float"),
            true,
        );
        let b = SdrSdfTypeIndicator::with_types(
            usd_sdf::ValueTypeName::default(),
            Token::new("float"),
            false,
        );
        // These should be EQUAL despite different has_sdf_type_mapping
        assert_eq!(
            a, b,
            "PartialEq must ignore has_sdf_type_mapping (C++ parity)"
        );
    }

    #[test]
    fn test_inequality_by_sdr_type() {
        let a = SdrSdfTypeIndicator::with_types(
            usd_sdf::ValueTypeName::default(),
            Token::new("float"),
            true,
        );
        let b = SdrSdfTypeIndicator::with_types(
            usd_sdf::ValueTypeName::default(),
            Token::new("int"),
            true,
        );
        assert_ne!(a, b);
    }
}

// ============================================================================
// Shader node tests (from testSdrRegistry.py patterns)
// ============================================================================

#[cfg(test)]
mod test_shader_node {
    use super::*;

    fn make_node(
        id: &str,
        name: &str,
        family: &str,
        source_type: &str,
        context: &str,
        props: Vec<Box<SdrShaderProperty>>,
    ) -> SdrShaderNode {
        SdrShaderNode::new(
            Token::new(id),
            SdrVersion::new(1, 0),
            name.to_string(),
            Token::new(family),
            Token::new(context),
            Token::new(source_type),
            String::new(),
            String::new(),
            props,
            SdrShaderNodeMetadata::new(),
            String::new(),
        )
    }

    fn make_input(name: &str, type_name: &Token) -> Box<SdrShaderProperty> {
        Box::new(SdrShaderProperty::new(
            Token::new(name),
            type_name.clone(),
            Value::default(),
            false,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        ))
    }

    fn make_output(name: &str, type_name: &Token) -> Box<SdrShaderProperty> {
        Box::new(SdrShaderProperty::new(
            Token::new(name),
            type_name.clone(),
            Value::default(),
            true,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        ))
    }

    #[test]
    fn test_node_validity() {
        // Node with properties is valid
        let node = make_node(
            "test",
            "test",
            "test",
            "OSL",
            "pattern",
            vec![make_output("out", &tokens().property_types.color)],
        );
        assert!(node.is_valid());

        // Node without properties is invalid
        let empty_node = make_node("empty", "empty", "", "OSL", "pattern", vec![]);
        assert!(!empty_node.is_valid());
    }

    #[test]
    fn test_node_basic_getters() {
        let node = make_node(
            "mix_float_2_1",
            "mix_float",
            "mix",
            "OSL",
            "pattern",
            vec![
                make_input("a", &tokens().property_types.float),
                make_input("b", &tokens().property_types.float),
                make_output("out", &tokens().property_types.float),
            ],
        );

        assert_eq!(node.get_identifier().as_str(), "mix_float_2_1");
        assert_eq!(node.get_name(), "mix_float");
        assert_eq!(node.get_family().as_str(), "mix");
        assert_eq!(node.get_source_type().as_str(), "OSL");
        assert_eq!(node.get_context().as_str(), "pattern");
    }

    #[test]
    fn test_node_inputs_outputs() {
        let node = make_node(
            "shader",
            "shader",
            "",
            "OSL",
            "pattern",
            vec![
                make_input("diffuseColor", &tokens().property_types.color),
                make_input("opacity", &tokens().property_types.float),
                make_output("out", &tokens().property_types.color),
            ],
        );

        assert_eq!(node.get_shader_input_names().len(), 2);
        assert_eq!(node.get_shader_output_names().len(), 1);

        assert!(node.get_shader_input(&Token::new("diffuseColor")).is_some());
        assert!(node.get_shader_input(&Token::new("opacity")).is_some());
        assert!(node.get_shader_input(&Token::new("nonexistent")).is_none());
        assert!(node.get_shader_output(&Token::new("out")).is_some());
        assert!(node.get_shader_output(&Token::new("nonexistent")).is_none());
    }

    #[test]
    fn test_node_info_string() {
        let node = make_node(
            "test_shader",
            "test_shader",
            "test",
            "OSL",
            "surface",
            vec![make_output("out", &tokens().property_types.float)],
        );

        let info = node.get_info_string();
        assert!(info.contains("test_shader"));
        assert!(info.contains("context: 'surface'"));
        assert!(info.contains("family: 'test'"));
    }

    #[test]
    fn test_get_data_for_key() {
        let node = make_node(
            "my_node",
            "my_node",
            "my",
            "OSL",
            "pattern",
            vec![make_output("out", &tokens().property_types.float)],
        );

        // C++: GetDataForKey(SdrNodeFieldKey->Identifier) -> VtValue(TfToken)
        let id_val = node.get_data_for_key(&tokens().node_field_key.identifier);
        assert!(!id_val.is_empty());

        // C++: GetDataForKey(SdrNodeFieldKey->Name) -> VtValue(std::string)
        let name_val = node.get_data_for_key(&tokens().node_field_key.name);
        assert!(!name_val.is_empty());

        // C++: GetDataForKey(SdrNodeFieldKey->Family) -> VtValue(TfToken)
        let fam_val = node.get_data_for_key(&tokens().node_field_key.family);
        assert!(!fam_val.is_empty());

        // Unknown key -> empty VtValue
        let unknown = node.get_data_for_key(&Token::new("nonexistent_key"));
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_get_role_fallback() {
        // No role metadata -> falls back to node name
        let node = make_node(
            "my_shader",
            "my_shader",
            "",
            "OSL",
            "pattern",
            vec![make_output("out", &tokens().property_types.float)],
        );
        assert_eq!(node.get_role().as_str(), "my_shader");
    }

    #[test]
    fn test_get_role_from_metadata() {
        let mut meta = SdrShaderNodeMetadata::new();
        meta.set_role(&Token::new("texture"));

        let node = SdrShaderNode::new(
            Token::new("tex_reader"),
            SdrVersion::new(1, 0),
            "tex_reader".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            vec![make_output("out", &tokens().property_types.color)],
            meta,
            String::new(),
        );

        assert_eq!(node.get_role().as_str(), "texture");
    }

    #[test]
    fn test_get_implementation_name() {
        // No implementationName metadata -> falls back to node name
        let node = make_node(
            "my_shader",
            "my_shader",
            "",
            "OSL",
            "pattern",
            vec![make_output("out", &tokens().property_types.float)],
        );
        assert_eq!(node.get_implementation_name(), "my_shader");

        // With implementationName metadata
        let mut meta = SdrShaderNodeMetadata::new();
        meta.set_implementation_name("actual_impl_fn");

        let node2 = SdrShaderNode::new(
            Token::new("display_name"),
            SdrVersion::new(1, 0),
            "display_name".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            vec![make_output("out", &tokens().property_types.float)],
            meta,
            String::new(),
        );
        assert_eq!(node2.get_implementation_name(), "actual_impl_fn");
    }

    #[test]
    fn test_pages() {
        let mut meta_a = SdrShaderPropertyMetadata::new();
        meta_a.set_page(&Token::new("Advanced"));
        let mut meta_b = SdrShaderPropertyMetadata::new();
        meta_b.set_page(&Token::new("Basic"));
        let mut meta_c = SdrShaderPropertyMetadata::new();
        meta_c.set_page(&Token::new("Advanced")); // duplicate

        let node = SdrShaderNode::new(
            Token::new("pages_node"),
            SdrVersion::new(1, 0),
            "pages_node".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            vec![
                Box::new(SdrShaderProperty::new(
                    Token::new("a"),
                    tokens().property_types.float.clone(),
                    Value::default(),
                    false,
                    0,
                    meta_a,
                    SdrTokenMap::new(),
                    Vec::new(),
                )),
                Box::new(SdrShaderProperty::new(
                    Token::new("b"),
                    tokens().property_types.float.clone(),
                    Value::default(),
                    false,
                    0,
                    meta_b,
                    SdrTokenMap::new(),
                    Vec::new(),
                )),
                Box::new(SdrShaderProperty::new(
                    Token::new("c"),
                    tokens().property_types.float.clone(),
                    Value::default(),
                    true,
                    0,
                    meta_c,
                    SdrTokenMap::new(),
                    Vec::new(),
                )),
            ],
            SdrShaderNodeMetadata::new(),
            String::new(),
        );

        let pages = node.get_pages();
        // Should have exactly 2 unique pages (Advanced, Basic)
        assert_eq!(pages.len(), 2);
        let page_strs: Vec<&str> = pages.iter().map(|p| p.as_str()).collect();
        assert!(page_strs.contains(&"Advanced"));
        assert!(page_strs.contains(&"Basic"));
    }

    #[test]
    fn test_property_names_for_page() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_page(&Token::new("Advanced"));

        let node = SdrShaderNode::new(
            Token::new("test"),
            SdrVersion::new(1, 0),
            "test".to_string(),
            Token::default(),
            Token::new("pattern"),
            Token::new("OSL"),
            String::new(),
            String::new(),
            vec![
                Box::new(SdrShaderProperty::new(
                    Token::new("adv_prop"),
                    tokens().property_types.float.clone(),
                    Value::default(),
                    false,
                    0,
                    meta,
                    SdrTokenMap::new(),
                    Vec::new(),
                )),
                Box::new(SdrShaderProperty::new(
                    Token::new("no_page_prop"),
                    tokens().property_types.float.clone(),
                    Value::default(),
                    false,
                    0,
                    SdrShaderPropertyMetadata::new(),
                    SdrTokenMap::new(),
                    Vec::new(),
                )),
                make_output("out", &tokens().property_types.float),
            ],
            SdrShaderNodeMetadata::new(),
            String::new(),
        );

        let adv_props = node.get_property_names_for_page("Advanced");
        assert_eq!(adv_props.len(), 1);
        assert_eq!(adv_props[0].as_str(), "adv_prop");

        // Empty page name gets properties with no page
        let no_page_props = node.get_property_names_for_page("");
        assert!(no_page_props.len() >= 1);
    }

    #[test]
    fn test_compliance_type_mismatch() {
        let node_a = make_node(
            "A",
            "A",
            "",
            "OSL",
            "pattern",
            vec![make_input("color", &tokens().property_types.color)],
        );
        let node_b = make_node(
            "B",
            "B",
            "",
            "OSL",
            "pattern",
            vec![make_input("color", &tokens().property_types.float)],
        );

        let results = check_property_compliance(&[&node_a, &node_b]);
        assert!(!results.is_empty());
        assert!(results.contains_key(&Token::new("color")));
        assert!(results[&Token::new("color")].contains(&Token::new("B")));
    }

    #[test]
    fn test_compliance_identical() {
        let node_a = make_node(
            "A",
            "A",
            "",
            "OSL",
            "pattern",
            vec![make_input("x", &tokens().property_types.float)],
        );
        let node_b = make_node(
            "B",
            "B",
            "",
            "OSL",
            "pattern",
            vec![make_input("x", &tokens().property_types.float)],
        );

        let results = check_property_compliance(&[&node_a, &node_b]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_compliance_empty() {
        let results = check_property_compliance(&[]);
        assert!(results.is_empty());
    }
}

// ============================================================================
// Shader property tests
// ============================================================================

#[cfg(test)]
mod test_shader_property {
    use super::*;

    fn make_prop(name: &str, type_name: &Token, is_output: bool) -> SdrShaderProperty {
        SdrShaderProperty::new(
            Token::new(name),
            type_name.clone(),
            Value::default(),
            is_output,
            0,
            SdrShaderPropertyMetadata::new(),
            SdrTokenMap::new(),
            Vec::new(),
        )
    }

    #[test]
    fn test_basic_property() {
        let prop = make_prop("diffuseColor", &tokens().property_types.color, false);
        assert_eq!(prop.get_name().as_str(), "diffuseColor");
        assert_eq!(prop.get_type(), &tokens().property_types.color);
        assert!(!prop.is_output());
        assert!(!prop.is_array());
    }

    #[test]
    fn test_output_property() {
        let prop = make_prop("out", &tokens().property_types.color, true);
        assert!(prop.is_output());
        // Outputs are always connectable
        assert!(prop.is_connectable());
    }

    #[test]
    fn test_connectable_default() {
        // Properties are connectable by default
        let prop = make_prop("input", &tokens().property_types.float, false);
        assert!(prop.is_connectable());
    }

    #[test]
    fn test_vstruct_member() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_item(
            tokens().property_metadata.vstruct_member_of.clone(),
            Value::from("myVstruct".to_string()),
        );
        meta.set_item(
            tokens().property_metadata.vstruct_member_name.clone(),
            Value::from("r".to_string()),
        );

        let prop = SdrShaderProperty::new(
            Token::new("vsChannel"),
            tokens().property_types.float.clone(),
            Value::default(),
            false,
            0,
            meta,
            SdrTokenMap::new(),
            Vec::new(),
        );

        assert!(prop.is_vstruct_member());
        assert_eq!(prop.get_vstruct_member_of().as_str(), "myVstruct");
        assert_eq!(prop.get_vstruct_member_name().as_str(), "r");
        assert!(!prop.is_vstruct());
    }

    #[test]
    fn test_convert_to_vstruct() {
        let mut prop = make_prop("myVs", &tokens().property_types.float, false);
        prop.convert_to_vstruct();
        assert!(prop.is_vstruct());
        assert!(!prop.is_vstruct_member());
    }

    #[test]
    fn test_info_string() {
        let prop = make_prop("diffuseColor", &tokens().property_types.color, false);
        let info = prop.get_info_string();
        assert!(info.contains("diffuseColor"));
        assert!(info.contains("color"));
        assert!(info.contains("input"));
    }

    #[test]
    fn test_default_widget() {
        // C++: if no widget metadata, default widget is "default"
        let prop = make_prop("test", &tokens().property_types.float, false);
        assert_eq!(prop.get_widget().as_str(), "default");
    }

    #[test]
    fn test_implementation_name_fallback() {
        let prop = make_prop("myProp", &tokens().property_types.float, false);
        // No implementationName metadata -> falls back to property name
        assert_eq!(prop.get_implementation_name(), "myProp");
    }
}

// ============================================================================
// Registry tests
// ============================================================================

#[cfg(test)]
mod test_registry {
    use super::*;

    fn make_discovery_result(
        id: &str,
        name: &str,
        family: &str,
        disc_type: &str,
        src_type: &str,
        version: SdrVersion,
    ) -> SdrShaderNodeDiscoveryResult {
        SdrShaderNodeDiscoveryResult::new(
            Token::new(id),
            version,
            name.to_string(),
            Token::new(family),
            Token::new(disc_type),
            Token::new(src_type),
            format!("/path/to/{}.{}", id, disc_type),
            format!("/path/to/{}.{}", id, disc_type),
            String::new(),
            SdrTokenMap::new(),
            String::new(),
            Token::default(),
        )
    }

    #[test]
    fn test_add_and_get_identifiers() {
        let registry = SdrRegistry::new_isolated();

        registry.add_discovery_result(make_discovery_result(
            "shader_a",
            "shader_a",
            "",
            "osl",
            "OSL",
            SdrVersion::new(1, 0).as_default(),
        ));
        registry.add_discovery_result(make_discovery_result(
            "shader_b",
            "shader_b",
            "",
            "osl",
            "OSL",
            SdrVersion::new(1, 0).as_default(),
        ));

        let ids = registry.get_shader_node_identifiers(None, SdrVersionFilter::DefaultOnly);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_get_names_dedup() {
        let registry = SdrRegistry::new_isolated();

        // Two discovery results with same name but different identifiers
        registry.add_discovery_result(make_discovery_result(
            "shader_v1",
            "shader",
            "",
            "osl",
            "OSL",
            SdrVersion::new(1, 0).as_default(),
        ));
        registry.add_discovery_result(make_discovery_result(
            "shader_v2",
            "shader",
            "",
            "osl",
            "OSL",
            SdrVersion::new(2, 0),
        ));

        let names = registry.get_shader_node_names(None);
        // Should have only 1 unique name
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"shader".to_string()));
    }

    #[test]
    fn test_source_types_no_duplicates() {
        let registry = SdrRegistry::new_isolated();

        registry.add_discovery_result(make_discovery_result(
            "a",
            "a",
            "",
            "osl",
            "OSL",
            SdrVersion::new(1, 0).as_default(),
        ));
        registry.add_discovery_result(make_discovery_result(
            "b",
            "b",
            "",
            "args",
            "OSL", // same source type
            SdrVersion::new(1, 0).as_default(),
        ));
        registry.add_discovery_result(make_discovery_result(
            "c",
            "c",
            "",
            "glslfx",
            "glslfx",
            SdrVersion::new(1, 0).as_default(),
        ));

        let types = registry.get_all_shader_node_source_types();
        // Should have exactly 2 unique source types (OSL, glslfx), sorted
        assert_eq!(types.len(), 2);
    }

    #[test]
    fn test_family_filter() {
        let registry = SdrRegistry::new_isolated();

        registry.add_discovery_result(make_discovery_result(
            "mix_float",
            "mix_float",
            "mix",
            "osl",
            "OSL",
            SdrVersion::new(1, 0).as_default(),
        ));
        registry.add_discovery_result(make_discovery_result(
            "noise_perlin",
            "noise_perlin",
            "noise",
            "osl",
            "OSL",
            SdrVersion::new(1, 0).as_default(),
        ));

        let mix_ids = registry
            .get_shader_node_identifiers(Some(&Token::new("mix")), SdrVersionFilter::DefaultOnly);
        assert_eq!(mix_ids.len(), 1);
        assert_eq!(mix_ids[0].as_str(), "mix_float");
    }
}

// ============================================================================
// Metadata tests
// ============================================================================

#[cfg(test)]
mod test_metadata {
    use super::*;

    #[test]
    fn test_node_metadata_has_set_get() {
        let mut meta = SdrShaderNodeMetadata::new();

        assert!(!meta.has_label());
        meta.set_label(&Token::new("My Label"));
        assert!(meta.has_label());
        assert_eq!(meta.get_label().as_str(), "My Label");
    }

    #[test]
    fn test_node_metadata_departments() {
        let mut meta = SdrShaderNodeMetadata::new();
        meta.set_departments(&vec![Token::new("lighting"), Token::new("fx")]);
        assert!(meta.has_departments());
        let deps = meta.get_departments();
        assert_eq!(deps.len(), 2);
    }

    #[test]
    fn test_property_metadata_has_set_get() {
        let mut meta = SdrShaderPropertyMetadata::new();

        assert!(!meta.has_label());
        meta.set_label(&Token::new("Diffuse Color"));
        assert!(meta.has_label());
        assert_eq!(meta.get_label().as_str(), "Diffuse Color");
    }

    #[test]
    fn test_property_metadata_connectable() {
        let mut meta = SdrShaderPropertyMetadata::new();
        meta.set_connectable(false);
        assert!(meta.has_connectable());
        assert!(!meta.get_connectable());
    }

    #[test]
    fn test_node_metadata_from_token_map() {
        let mut map = SdrTokenMap::new();
        map.insert(Token::new("label"), "Test Node".to_string());
        map.insert(Token::new("category"), "shading".to_string());
        map.insert(Token::new("help"), "A test node".to_string());

        let meta = SdrShaderNodeMetadata::from_token_map(&map);
        assert_eq!(meta.get_label().as_str(), "Test Node");
        assert_eq!(meta.get_category().as_str(), "shading");
        assert_eq!(meta.get_help(), "A test node");
    }

    #[test]
    fn test_node_metadata_encode_legacy() {
        let mut meta = SdrShaderNodeMetadata::new();
        meta.set_label(&Token::new("My Label"));
        meta.set_help("Help text");

        let legacy = meta.encode_legacy_metadata();
        assert_eq!(legacy.get(&Token::new("label")).unwrap(), "My Label");
        assert_eq!(legacy.get(&Token::new("help")).unwrap(), "Help text");
    }
}
