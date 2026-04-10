//! `pxr.Sdr.shaderParserTestUtils` — OpenUSD shader parser test helpers.
//!
//! Pure Rust implementation (parity with `pxr/usd/sdr/shaderParserTestUtils.py`).
//! No embedded Python source or `compile`/`exec` — production policy is a single native
//! extension plus minimal `pxr` package stubs.

use pyo3::exceptions::PyAssertionError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::collections::{HashMap, HashSet};

use usd_gf::{Vec2f, Vec2i, Vec3f, Vec3i, Vec4f, Vec4i};
use usd_sdf::ValueTypeName;
use usd_sdf::types::get_type_for_value_type_name;
use usd_sdr::tokens::tokens;
use usd_tf::Token;
use usd_vt::Value;

use crate::sdr::{PyShaderNode, PyShaderProperty};
use crate::tf::PyType;

type SdrNode<'a> = &'a usd_sdr::shader_node::SdrShaderNode;
type SdrProp<'a> = &'a usd_sdr::shader_property::SdrShaderProperty;

fn err(msg: impl Into<String>) -> PyErr {
    PyAssertionError::new_err(msg.into())
}

fn py_assert(cond: bool, msg: impl Into<String>) -> PyResult<()> {
    if cond { Ok(()) } else { Err(err(msg)) }
}

fn vtn(token_str: &str) -> ValueTypeName {
    get_type_for_value_type_name(&Token::new(token_str))
}

/// Map `SdfValueTypeName::GetCppTypeName()`-style strings to names accepted by
/// `Tf.Type.FindByName` in OpenUSD (Python tests use `sdfValueTypeName.type`, not raw C++ names).
fn sdf_cpp_name_for_tf_lookup(cpp: &str) -> String {
    match cpp {
        "std::string" => "string".to_string(),
        "f32" => "float".to_string(),
        "i32" => "int".to_string(),
        "f64" => "double".to_string(),
        _ if cpp.starts_with("VtArray<") && cpp.ends_with('>') => {
            let inner = &cpp["VtArray<".len()..cpp.len() - 1];
            let inner = match inner {
                "std::string" => "string",
                "f32" => "float",
                "i32" => "int",
                "f64" => "double",
                other => other,
            };
            format!("VtArray<{inner}>")
        }
        _ => cpp.to_string(),
    }
}

fn tf_cpp(prop: SdrProp<'_>) -> usd_tf::TfType {
    let ind = prop.get_type_as_sdf_type();
    let cpp = ind.get_sdf_type().cpp_type_name();
    usd_tf::TfType::find_by_name(&sdf_cpp_name_for_tf_lookup(cpp))
}

fn validate_on_node(node: SdrNode<'_>, prop: SdrProp<'_>) -> bool {
    let name = prop.get_name();
    node.get_shader_input(name).is_some() || node.get_shader_output(name).is_some()
}

fn input<'a>(
    node: SdrNode<'a>,
    name: &str,
) -> PyResult<&'a usd_sdr::shader_property::SdrShaderProperty> {
    let t = Token::new(name);
    node.get_shader_input(&t)
        .ok_or_else(|| err(format!("missing shader input '{name}'")))
}

fn output<'a>(
    node: SdrNode<'a>,
    name: &str,
) -> PyResult<&'a usd_sdr::shader_property::SdrShaderProperty> {
    let t = Token::new(name);
    node.get_shader_output(&t)
        .ok_or_else(|| err(format!("missing shader output '{name}'")))
}

fn assert_token_maps_eq(
    got: &HashMap<Token, String>,
    expected: &HashMap<Token, String>,
) -> PyResult<()> {
    py_assert(got.len() == expected.len(), "metadata map length mismatch")?;
    for (k, v) in expected {
        match got.get(k) {
            Some(gv) if gv == v => {}
            _ => {
                return Err(err(format!(
                    "metadata mismatch at key {}: expected {:?}",
                    k.as_str(),
                    v
                )));
            }
        }
    }
    Ok(())
}

fn value_f32_components(v: &Value) -> Option<Vec<f32>> {
    if let Some(a) = v.as_vec_clone::<f32>() {
        return Some(a);
    }
    if let Some(u) = v.downcast_clone::<Vec2f>() {
        return Some(vec![u.x, u.y]);
    }
    if let Some(u) = v.downcast_clone::<Vec3f>() {
        return Some(vec![u.x, u.y, u.z]);
    }
    if let Some(u) = v.downcast_clone::<Vec4f>() {
        return Some(vec![u.x, u.y, u.z, u.w]);
    }
    None
}

fn value_i32_components(v: &Value) -> Option<Vec<i32>> {
    if let Some(a) = v.as_vec_clone::<i32>() {
        return Some(a);
    }
    if let Some(u) = v.downcast_clone::<Vec2i>() {
        return Some(vec![u.x, u.y]);
    }
    if let Some(u) = v.downcast_clone::<Vec3i>() {
        return Some(vec![u.x, u.y, u.z]);
    }
    if let Some(u) = v.downcast_clone::<Vec4i>() {
        return Some(vec![u.x, u.y, u.z, u.w]);
    }
    None
}

fn default_values_equivalent(a: &Value, b: &Value) -> bool {
    if let (Some(ga), Some(gb)) = (value_f32_components(a), value_f32_components(b)) {
        return ga == gb;
    }
    if let (Some(ga), Some(gb)) = (value_i32_components(a), value_i32_components(b)) {
        return ga == gb;
    }
    false
}

fn assert_value_eq(got: &Value, expected: &Value, ctx: &str) -> PyResult<()> {
    py_assert(
        got == expected || default_values_equivalent(got, expected),
        format!("{ctx}: value mismatch: got {got:?} expected {expected:?}"),
    )
}

fn assert_tf_eq(prop: SdrProp<'_>, expected_cpp: &str, ctx: &str) -> PyResult<()> {
    let got = tf_cpp(prop);
    let exp = usd_tf::TfType::find_by_name(expected_cpp);
    py_assert(
        got.type_name() == exp.type_name() && got.is_unknown() == exp.is_unknown(),
        format!(
            "{ctx}: Tf.Type expected {expected_cpp}, got {}",
            got.type_name()
        ),
    )
}

fn props_basic<'a>(
    node: SdrNode<'a>,
) -> PyResult<HashMap<&'static str, &'a usd_sdr::shader_property::SdrShaderProperty>> {
    let mut m = HashMap::new();
    m.insert("inputA", input(node, "inputA")?);
    m.insert("inputB", input(node, "inputB")?);
    m.insert("inputC", input(node, "inputC")?);
    m.insert("inputD", input(node, "inputD")?);
    m.insert("inputF2", input(node, "inputF2")?);
    m.insert("inputStrArray", input(node, "inputStrArray")?);
    m.insert("inputArrayWithTuples", input(node, "inputArrayWithTuples")?);
    m.insert("resultF", output(node, "resultF")?);
    m.insert("resultI", output(node, "resultI")?);
    Ok(m)
}

fn props_shading<'a>(
    node: SdrNode<'a>,
) -> PyResult<HashMap<&'static str, &'a usd_sdr::shader_property::SdrShaderProperty>> {
    let mut m = HashMap::new();
    m.insert("inputA", input(node, "inputA")?);
    m.insert("inputB", input(node, "inputB")?);
    m.insert("inputC", input(node, "inputC")?);
    m.insert("inputD", input(node, "inputD")?);
    m.insert("inputF2", input(node, "inputF2")?);
    m.insert("inputF3", input(node, "inputF3")?);
    m.insert("inputF4", input(node, "inputF4")?);
    m.insert("inputF5", input(node, "inputF5")?);
    m.insert("inputInterp", input(node, "inputInterp")?);
    m.insert("inputOptions", input(node, "inputOptions")?);
    m.insert("inputPoint", input(node, "inputPoint")?);
    m.insert("inputNormal", input(node, "inputNormal")?);
    m.insert("inputStruct", input(node, "inputStruct")?);
    m.insert("inputAssetIdentifier", input(node, "inputAssetIdentifier")?);
    m.insert("resultF", output(node, "resultF")?);
    m.insert("resultF2", output(node, "resultF2")?);
    m.insert("resultF3", output(node, "resultF3")?);
    m.insert("resultI", output(node, "resultI")?);
    // ARGS/USD TestNode exposes vstruct outputs; compiled OSL TestNodeOSL does not.
    if let Some(p) = node.get_shader_output(&Token::new("vstruct1")) {
        m.insert("vstruct1", p);
    }
    if let Some(p) = node.get_shader_output(&Token::new("vstruct1_bump")) {
        m.insert("vstruct1_bump", p);
    }
    m.insert("outputPoint", output(node, "outputPoint")?);
    m.insert("outputNormal", output(node, "outputNormal")?);
    m.insert("outputColor", output(node, "outputColor")?);
    m.insert("outputVector", output(node, "outputVector")?);
    Ok(m)
}

fn option_set(prop: SdrProp<'_>) -> HashSet<(String, String)> {
    prop.get_options()
        .iter()
        .map(|(a, b)| (a.as_str().to_string(), b.as_str().to_string()))
        .collect()
}

/// Determines if the given node has an OSL source type.
#[pyfunction]
#[pyo3(name = "IsNodeOSL")]
fn is_node_osl(node: &PyShaderNode) -> bool {
    node.inner.get_source_type().as_str() == "OSL"
}

/// Given a property (`SdrShaderProperty`), return the `Tf.Type` for its SDF value type
/// (parity with Python: `property.GetTypeAsSdfType().GetSdfType().type`).
#[pyfunction]
#[pyo3(name = "GetType")]
fn get_type(py: Python<'_>, prop: &PyShaderProperty) -> PyResult<Py<PyType>> {
    let ind = prop.inner.get_type_as_sdf_type();
    let cpp = ind.get_sdf_type().cpp_type_name();
    Py::new(
        py,
        PyType::from_tf_type_for_bindings(usd_tf::TfType::find_by_name(cpp)),
    )
}

#[pyfunction]
#[pyo3(name = "TestBasicProperties")]
fn test_basic_properties(node: &PyShaderNode) -> PyResult<()> {
    let n = &node.inner;
    let is_osl = is_node_osl(node);
    let p = props_basic(n)?;
    let t = tokens();

    let mut metadata: HashMap<Token, String> = HashMap::from([
        (Token::new("widget"), "number".to_string()),
        (Token::new("label"), "inputA label".to_string()),
        (Token::new("page"), "inputs1".to_string()),
        (Token::new("open"), "1".to_string()),
        (Token::new("help"), "inputA help message".to_string()),
        (Token::new("uncategorized"), "1".to_string()),
    ]);
    if !is_osl {
        metadata.insert(Token::new("name"), "inputA".to_string());
        metadata.insert(Token::new("default"), "0.0".to_string());
        metadata.insert(Token::new("type"), "float".to_string());
        metadata.remove(&Token::new("open"));
    }

    let a = p["inputA"];
    py_assert(a.get_name().as_str() == "inputA", "inputA name")?;
    py_assert(a.get_type() == &t.property_types.float, "inputA type")?;
    assert_value_eq(
        a.get_default_value(),
        &Value::from_f32(0.0),
        "inputA default",
    )?;
    py_assert(!a.is_output(), "inputA is_output")?;
    py_assert(!a.is_array(), "inputA is_array")?;
    py_assert(!a.is_dynamic_array(), "inputA is_dynamic_array")?;
    py_assert(a.get_array_size() == 0, "inputA array size")?;
    py_assert(a.get_tuple_size() == 0, "inputA tuple size")?;
    py_assert(
        a.get_info_string() == "inputA (type: 'float'); input",
        "inputA info string",
    )?;
    py_assert(a.is_connectable(), "inputA connectable")?;
    py_assert(a.can_connect_to(p["resultF"]), "inputA can connect resultF")?;
    py_assert(
        !a.can_connect_to(p["resultI"]),
        "inputA cannot connect resultI",
    )?;
    assert_token_maps_eq(a.get_metadata(), &metadata)?;

    let input_d = p["inputD"];
    py_assert(input_d.is_dynamic_array(), "inputD dynamic")?;
    py_assert(
        input_d.get_array_size() == if is_osl { 1 } else { -1 },
        "inputD array size",
    )?;
    py_assert(input_d.get_tuple_size() == 0, "inputD tuple size")?;
    py_assert(input_d.is_array(), "inputD is_array")?;
    assert_value_eq(
        input_d.get_default_value(),
        &Value::new(vec![1i32]),
        "inputD default",
    )?;

    let input_f2 = p["inputF2"];
    py_assert(!input_f2.is_dynamic_array(), "inputF2 dynamic")?;
    py_assert(input_f2.get_array_size() == 2, "inputF2 array size")?;
    py_assert(input_d.get_tuple_size() == 0, "tuple size ref inputD")?;
    py_assert(input_f2.is_array(), "inputF2 is_array")?;
    py_assert(!input_f2.is_connectable(), "inputF2 connectable")?;
    assert_value_eq(
        input_f2.get_default_value(),
        &Value::from(vec![1.0f32, 2.0f32]),
        "inputF2 default",
    )?;

    let input_str_array = p["inputStrArray"];
    py_assert(input_str_array.get_array_size() == 4, "inputStrArray size")?;
    py_assert(input_str_array.get_tuple_size() == 0, "inputStrArray tuple")?;
    assert_value_eq(
        input_str_array.get_default_value(),
        &Value::from(vec![
            "test".to_string(),
            "string".to_string(),
            "array".to_string(),
            "values".to_string(),
        ]),
        "inputStrArray default",
    )?;

    let input_array_tuples = p["inputArrayWithTuples"];
    py_assert(
        input_array_tuples.get_array_size() == 8,
        "inputArrayWithTuples size",
    )?;
    py_assert(
        input_array_tuples.get_tuple_size() == 2,
        "inputArrayWithTuples tuple",
    )?;
    py_assert(
        input_array_tuples.is_array(),
        "inputArrayWithTuples is_array",
    )?;

    Ok(())
}

#[pyfunction]
#[pyo3(name = "TestShadingProperties")]
fn test_shading_properties(node: &PyShaderNode) -> PyResult<()> {
    let n = &node.inner;
    let is_osl = is_node_osl(node);
    let p = props_shading(n)?;
    let t = tokens();

    let hints: HashMap<Token, String> = if is_osl {
        HashMap::from([
            (Token::new("uncategorized"), "1".to_string()),
            (Token::new("open"), "1".to_string()),
        ])
    } else {
        HashMap::from([(Token::new("uncategorized"), "1".to_string())])
    };

    let ia = p["inputA"];
    py_assert(ia.get_label().as_str() == "inputA label", "label")?;
    py_assert(ia.get_help() == "inputA help message", "help")?;
    py_assert(ia.get_page().as_str() == "inputs1", "page")?;
    py_assert(ia.get_widget().as_str() == "number", "widget")?;
    assert_token_maps_eq(ia.get_hints(), &hints)?;
    py_assert(ia.get_options().is_empty(), "options empty")?;
    py_assert(ia.get_vstruct_member_of().is_empty(), "vstruct member of")?;
    py_assert(
        ia.get_vstruct_member_name().is_empty(),
        "vstruct member name",
    )?;
    py_assert(!ia.is_vstruct_member(), "vstruct member")?;
    py_assert(!ia.is_vstruct(), "vstruct")?;
    py_assert(ia.is_connectable(), "connectable")?;
    py_assert(
        ia.get_valid_connection_types().is_empty(),
        "valid conn types",
    )?;
    py_assert(ia.can_connect_to(p["resultF"]), "can connect resultF")?;
    py_assert(!ia.can_connect_to(p["resultI"]), "cannot resultI")?;

    let opts_expected: HashSet<_> = [
        ("opt1".to_string(), "opt1val".to_string()),
        ("opt2".to_string(), "opt2val".to_string()),
    ]
    .into_iter()
    .collect();
    py_assert(
        option_set(p["inputOptions"]) == opts_expected,
        "inputOptions",
    )?;

    let interp_expected: HashSet<_> = [
        ("linear".to_string(), "".to_string()),
        ("catmull-rom".to_string(), "".to_string()),
        ("bspline".to_string(), "".to_string()),
        ("constant".to_string(), "".to_string()),
    ]
    .into_iter()
    .collect();
    py_assert(
        option_set(p["inputInterp"]) == interp_expected,
        "inputInterp",
    )?;

    py_assert(
        p["inputPoint"].can_connect_to(p["outputNormal"]),
        "point->normal",
    )?;
    py_assert(
        p["inputNormal"].can_connect_to(p["outputPoint"]),
        "normal->point",
    )?;
    py_assert(
        p["inputNormal"].can_connect_to(p["outputColor"]),
        "normal->color",
    )?;
    py_assert(
        p["inputNormal"].can_connect_to(p["outputVector"]),
        "normal->vector",
    )?;
    py_assert(
        p["inputNormal"].can_connect_to(p["resultF3"]),
        "normal->resultF3",
    )?;
    py_assert(p["inputF2"].can_connect_to(p["resultF2"]), "F2->F2")?;
    py_assert(p["inputD"].can_connect_to(p["resultI"]), "D->I")?;
    py_assert(
        !p["inputNormal"].can_connect_to(p["resultF2"]),
        "!normal->F2",
    )?;
    py_assert(!p["inputF4"].can_connect_to(p["resultF2"]), "!F4->F2")?;
    py_assert(!p["inputF2"].can_connect_to(p["resultF3"]), "!F2->F3")?;

    let expected_mappings = [
        ("inputB", vtn("int"), &t.property_types.int),
        ("inputF2", vtn("float2"), &t.property_types.float),
        ("inputF3", vtn("float3"), &t.property_types.float),
        ("inputF4", vtn("float4"), &t.property_types.float),
        ("inputF5", vtn("float[]"), &t.property_types.float),
        ("inputStruct", vtn("token"), &t.property_types.struct_type),
    ];
    for (name, ref sdf_exp, sdr_exp) in expected_mappings {
        let prop = p[name];
        let ind = prop.get_type_as_sdf_type();
        py_assert(ind.get_sdf_type() == sdf_exp, format!("sdf type {name}"))?;
        py_assert(ind.get_sdr_type() == sdr_exp, format!("sdr type {name}"))?;
    }

    py_assert(
        p["inputAssetIdentifier"].is_asset_identifier(),
        "asset identifier",
    )?;
    py_assert(!p["inputOptions"].is_asset_identifier(), "!options asset")?;
    let asset_ind = p["inputAssetIdentifier"].get_type_as_sdf_type();
    let asset_sdf = vtn("asset");
    py_assert(asset_ind.get_sdf_type() == &asset_sdf, "asset sdf")?;
    py_assert(
        asset_ind.get_sdr_type() == &t.property_types.string,
        "asset sdr",
    )?;

    if !is_osl {
        py_assert(
            p["vstruct1"].get_page().as_str() == "VStructs:Nested",
            "vstruct1 page",
        )?;
        py_assert(
            p["vstruct1_bump"].get_page().as_str() == "VStructs:Nested:More",
            "vstruct1_bump page",
        )?;

        py_assert(p["vstruct1"].is_vstruct(), "vstruct1 is vstruct")?;
        py_assert(
            p["vstruct1"].get_vstruct_member_of().is_empty(),
            "vstruct1 member of",
        )?;
        py_assert(
            p["vstruct1"].get_vstruct_member_name().is_empty(),
            "vstruct1 member name",
        )?;
        py_assert(!p["vstruct1"].is_vstruct_member(), "vstruct1 member")?;

        py_assert(!p["vstruct1_bump"].is_vstruct(), "bump vstruct")?;
        py_assert(
            p["vstruct1_bump"].get_vstruct_member_of().as_str() == "vstruct1",
            "bump member of",
        )?;
        py_assert(
            p["vstruct1_bump"].get_vstruct_member_name().as_str() == "bump",
            "bump member name",
        )?;
        py_assert(p["vstruct1_bump"].is_vstruct_member(), "bump is member")?;
    }

    Ok(())
}

#[pyfunction]
#[pyo3(name = "TestBasicNode")]
fn test_basic_node(
    node: &PyShaderNode,
    node_source_type: String,
    node_definition_uri: String,
    node_implementation_uri: String,
) -> PyResult<()> {
    let n = &node.inner;
    let is_osl = is_node_osl(node);
    let node_context = if is_osl { "OSL" } else { "pattern" };
    let node_name = if is_osl {
        "TestNodeOSL"
    } else {
        "TestNodeARGS"
    };
    let num_outputs = if is_osl { 8 } else { 10 };
    let mut output_names: HashSet<&'static str> = [
        "resultF",
        "resultF2",
        "resultF3",
        "resultI",
        "outputPoint",
        "outputNormal",
        "outputColor",
        "outputVector",
    ]
    .into_iter()
    .collect();
    let mut metadata: HashMap<Token, String> = HashMap::from([
        (Token::new("category"), "testing".to_string()),
        (Token::new("departments"), "testDept".to_string()),
        (Token::new("help"), "This is the test node".to_string()),
        (Token::new("label"), "TestNodeLabel".to_string()),
        (
            Token::new("primvars"),
            "primvar1|primvar2|primvar3|$primvarNamingProperty|$invalidPrimvarNamingProperty"
                .to_string(),
        ),
        (
            Token::new("uncategorizedMetadata"),
            "uncategorized".to_string(),
        ),
    ]);
    if !is_osl {
        metadata.remove(&Token::new("category"));
        metadata.remove(&Token::new("label"));
        metadata.remove(&Token::new("uncategorizedMetadata"));
        output_names.insert("vstruct1");
        output_names.insert("vstruct1_bump");
    }

    py_assert(n.get_name() == node_name, "GetName")?;
    py_assert(n.get_context().as_str() == node_context, "GetContext")?;
    py_assert(
        n.get_source_type().as_str() == node_source_type.as_str(),
        "GetSourceType",
    )?;
    // C++ `GetFunction` is `_function` (discovery `function`); not `GetImplementationName`.
    py_assert(n.get_family().as_str().is_empty(), "GetFunction")?;
    py_assert(
        n.get_resolved_definition_uri() == node_definition_uri.as_str(),
        "definition URI",
    )?;
    py_assert(
        n.get_resolved_implementation_uri() == node_implementation_uri.as_str(),
        "impl URI",
    )?;
    py_assert(n.is_valid(), "IsValid")?;

    let inputs: HashMap<_, _> = n
        .get_shader_input_names()
        .iter()
        .map(|t| {
            let name = t.as_str().to_string();
            input(n, t.as_str()).map(|p| (name, p))
        })
        .collect::<Result<_, _>>()?;
    let outputs: HashMap<_, _> = n
        .get_shader_output_names()
        .iter()
        .map(|t| {
            let name = t.as_str().to_string();
            output(n, t.as_str()).map(|p| (name, p))
        })
        .collect::<Result<_, _>>()?;

    py_assert(inputs.len() == 18, "input count")?;
    py_assert(outputs.len() == num_outputs, "output count")?;
    for name in [
        "inputA",
        "inputB",
        "inputC",
        "inputD",
        "inputF2",
        "inputF3",
        "inputF4",
        "inputF5",
        "inputInterp",
        "inputOptions",
        "inputPoint",
        "inputNormal",
        "inputArrayWithTuples",
    ] {
        py_assert(inputs.contains_key(name), format!("has input {name}"))?;
    }
    for name in [
        "resultF2",
        "resultI",
        "outputPoint",
        "outputNormal",
        "outputColor",
        "outputVector",
    ] {
        py_assert(outputs.contains_key(name), format!("has output {name}"))?;
    }

    let expected_inputs: HashSet<_> = [
        "inputA",
        "inputB",
        "inputC",
        "inputD",
        "inputF2",
        "inputF3",
        "inputF4",
        "inputF5",
        "inputInterp",
        "inputOptions",
        "inputPoint",
        "inputNormal",
        "inputStruct",
        "inputAssetIdentifier",
        "primvarNamingProperty",
        "invalidPrimvarNamingProperty",
        "inputStrArray",
        "inputArrayWithTuples",
    ]
    .into_iter()
    .collect();
    let got_inputs: HashSet<_> = n
        .get_shader_input_names()
        .iter()
        .map(|t| t.as_str())
        .collect();
    py_assert(got_inputs == expected_inputs, "input names")?;

    let got_outputs: HashSet<_> = n
        .get_shader_output_names()
        .iter()
        .map(|t| t.as_str())
        .collect();
    py_assert(got_outputs == output_names, "output names")?;

    let node_meta = n.get_metadata();
    for (k, v) in &metadata {
        py_assert(
            node_meta.get(k).map(String::as_str) == Some(v.as_str()),
            format!("metadata {} ", k.as_str()),
        )?;
    }

    test_basic_properties(node)
}

#[pyfunction]
#[pyo3(name = "TestShaderSpecificNode")]
fn test_shader_specific_node(node: &PyShaderNode) -> PyResult<()> {
    let n = &node.inner;
    let is_osl = is_node_osl(node);
    let num_outputs = if is_osl { 8 } else { 10 };
    let label = if is_osl { "TestNodeLabel" } else { "" };
    let category = if is_osl { "testing" } else { "" };
    let vstruct_names: Vec<&str> = if is_osl { vec![] } else { vec!["vstruct1"] };
    let pages: HashSet<&str> = if is_osl {
        ["", "inputs1", "inputs2", "results"].into_iter().collect()
    } else {
        [
            "",
            "inputs1",
            "inputs2",
            "results",
            "VStructs:Nested",
            "VStructs:Nested:More",
        ]
        .into_iter()
        .collect()
    };
    let open_pages: HashSet<&str> = if is_osl {
        ["inputs1", "results"].into_iter().collect()
    } else {
        ["inputs1", "VStructs", "VStructs:Nested"]
            .into_iter()
            .collect()
    };

    let shader_inputs: HashMap<_, _> = n
        .get_shader_input_names()
        .iter()
        .map(|t| {
            let name = t.as_str().to_string();
            input(n, t.as_str()).map(|p| (name, p))
        })
        .collect::<Result<_, _>>()?;
    let shader_outputs: HashMap<_, _> = n
        .get_shader_output_names()
        .iter()
        .map(|t| {
            let name = t.as_str().to_string();
            output(n, t.as_str()).map(|p| (name, p))
        })
        .collect::<Result<_, _>>()?;

    py_assert(shader_inputs.len() == 18, "shader input len")?;
    py_assert(shader_outputs.len() == num_outputs, "shader output len")?;
    for name in [
        "inputA",
        "inputB",
        "inputC",
        "inputD",
        "inputF2",
        "inputF3",
        "inputF4",
        "inputF5",
        "inputInterp",
        "inputOptions",
        "inputPoint",
        "inputNormal",
        "inputArrayWithTuples",
    ] {
        py_assert(
            shader_inputs.contains_key(name),
            format!("shader in {name}"),
        )?;
    }
    for name in [
        "resultF",
        "resultF2",
        "resultF3",
        "resultI",
        "outputPoint",
        "outputNormal",
        "outputColor",
        "outputVector",
    ] {
        py_assert(
            shader_outputs.contains_key(name),
            format!("shader out {name}"),
        )?;
    }
    py_assert(n.get_label().as_str() == label, "GetLabel")?;
    py_assert(n.get_category().as_str() == category, "GetCategory")?;
    py_assert(n.get_help() == "This is the test node", "GetHelp")?;
    py_assert(
        n.get_departments()
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            == vec!["testDept"],
        "departments",
    )?;
    let got_pages: HashSet<_> = n.get_pages().iter().map(|t| t.as_str()).collect();
    py_assert(got_pages == pages, "pages")?;
    let got_open: HashSet<_> = n.get_open_pages().iter().map(|t| t.as_str()).collect();
    py_assert(got_open == open_pages, "open pages")?;
    let got_prim: HashSet<_> = n.get_primvars().iter().map(|t| t.as_str()).collect();
    py_assert(
        got_prim == HashSet::from(["primvar1", "primvar2", "primvar3"]),
        "primvars",
    )?;
    let got_add: HashSet<_> = n
        .get_additional_primvar_properties()
        .iter()
        .map(|t| t.as_str())
        .collect();
    py_assert(
        got_add == HashSet::from(["primvarNamingProperty"]),
        "add primvars",
    )?;

    let results_page: HashSet<_> = ["resultF", "resultF2", "resultF3", "resultI"]
        .into_iter()
        .collect();
    py_assert(
        n.get_property_names_for_page("results")
            .iter()
            .map(|t| t.as_str())
            .collect::<HashSet<_>>()
            == results_page,
        "page results",
    )?;
    let empty_page: HashSet<_> = ["outputPoint", "outputNormal", "outputColor", "outputVector"]
        .into_iter()
        .collect();
    py_assert(
        n.get_property_names_for_page("")
            .iter()
            .map(|t| t.as_str())
            .collect::<HashSet<_>>()
            == empty_page,
        "page empty",
    )?;
    let inputs1: HashSet<_> = ["inputA"].into_iter().collect();
    py_assert(
        n.get_property_names_for_page("inputs1")
            .iter()
            .map(|t| t.as_str())
            .collect::<HashSet<_>>()
            == inputs1,
        "page inputs1",
    )?;
    let inputs2: HashSet<_> = [
        "inputB",
        "inputC",
        "inputD",
        "inputF2",
        "inputF3",
        "inputF4",
        "inputF5",
        "inputInterp",
        "inputOptions",
        "inputPoint",
        "inputNormal",
        "inputStruct",
        "inputAssetIdentifier",
        "primvarNamingProperty",
        "invalidPrimvarNamingProperty",
        "inputStrArray",
        "inputArrayWithTuples",
    ]
    .into_iter()
    .collect();
    py_assert(
        n.get_property_names_for_page("inputs2")
            .iter()
            .map(|t| t.as_str())
            .collect::<HashSet<_>>()
            == inputs2,
        "page inputs2",
    )?;
    py_assert(
        n.get_all_vstruct_names()
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            == vstruct_names,
        "vstruct names",
    )?;

    test_shading_properties(node)
}

#[pyfunction]
#[pyo3(name = "TestShaderPropertiesNode")]
fn test_shader_properties_node(node: &PyShaderNode) -> PyResult<()> {
    let n = &node.inner;
    let t = tokens();
    let allowed = [
        "TestShaderPropertiesNodeOSL",
        "TestShaderPropertiesNodeARGS",
        "TestShaderPropertiesNodeUSD",
    ];
    py_assert(allowed.contains(&n.get_name()), "allowed node name")?;

    match n.get_name() {
        "TestShaderPropertiesNodeOSL" => {
            py_assert(n.get_source_type().as_str() == "OSL", "source OSL")?;
        }
        "TestShaderPropertiesNodeARGS" => {
            py_assert(n.get_source_type().as_str() == "RmanCpp", "source RmanCpp")?;
        }
        "TestShaderPropertiesNodeUSD" => {
            py_assert(n.get_source_type().as_str() == "glslfx", "source glslfx")?;
        }
        _ => {}
    }

    let check = |prop: SdrProp<'_>, ptype: &Token, cpp: &str| -> PyResult<()> {
        let name = prop.get_name().as_str();
        py_assert(
            prop.get_type() == ptype,
            format!(
                "property Sdr type ({name}): got '{}' expected '{}'",
                prop.get_type().as_str(),
                ptype.as_str(),
            ),
        )?;
        assert_tf_eq(prop, cpp, &format!("GetType Tf ({name})"))?;
        py_assert(validate_on_node(n, prop), "_ValidateProperty")?;
        Ok(())
    };

    check(input(n, "inputInt")?, &t.property_types.int, "int")?;
    check(input(n, "inputString")?, &t.property_types.string, "string")?;
    check(input(n, "inputFloat")?, &t.property_types.float, "float")?;
    check(input(n, "inputColor")?, &t.property_types.color, "GfVec3f")?;
    check(input(n, "inputPoint")?, &t.property_types.point, "GfVec3f")?;
    check(
        input(n, "inputNormal")?,
        &t.property_types.normal,
        "GfVec3f",
    )?;
    check(
        input(n, "inputVector")?,
        &t.property_types.vector,
        "GfVec3f",
    )?;
    check(
        input(n, "inputMatrix")?,
        &t.property_types.matrix,
        "GfMatrix4d",
    )?;

    if n.get_name() != "TestShaderPropertiesNodeUSD" {
        check(
            input(n, "inputStruct")?,
            &t.property_types.struct_type,
            "TfToken",
        )?;
        check(
            input(n, "inputVstruct")?,
            &t.property_types.vstruct,
            "TfToken",
        )?;
    }

    check(
        input(n, "inputIntArray")?,
        &t.property_types.int,
        "VtArray<int>",
    )?;
    check(
        input(n, "inputStringArray")?,
        &t.property_types.string,
        "VtArray<string>",
    )?;
    check(
        input(n, "inputFloatArray")?,
        &t.property_types.float,
        "VtArray<float>",
    )?;
    check(
        input(n, "inputColorArray")?,
        &t.property_types.color,
        "VtArray<GfVec3f>",
    )?;
    check(
        input(n, "inputPointArray")?,
        &t.property_types.point,
        "VtArray<GfVec3f>",
    )?;
    check(
        input(n, "inputNormalArray")?,
        &t.property_types.normal,
        "VtArray<GfVec3f>",
    )?;
    check(
        input(n, "inputVectorArray")?,
        &t.property_types.vector,
        "VtArray<GfVec3f>",
    )?;
    check(
        input(n, "inputMatrixArray")?,
        &t.property_types.matrix,
        "VtArray<GfMatrix4d>",
    )?;
    check(input(n, "inputFloat2")?, &t.property_types.float, "GfVec2f")?;
    check(input(n, "inputFloat3")?, &t.property_types.float, "GfVec3f")?;
    check(input(n, "inputFloat4")?, &t.property_types.float, "GfVec4f")?;
    check(
        input(n, "inputAsset")?,
        &t.property_types.string,
        "SdfAssetPath",
    )?;
    check(
        input(n, "inputAssetArray")?,
        &t.property_types.string,
        "VtArray<SdfAssetPath>",
    )?;
    check(
        input(n, "inputColorRoleNone")?,
        &t.property_types.float,
        "GfVec3f",
    )?;
    check(
        input(n, "inputPointRoleNone")?,
        &t.property_types.float,
        "GfVec3f",
    )?;
    check(
        input(n, "inputNormalRoleNone")?,
        &t.property_types.float,
        "GfVec3f",
    )?;
    check(
        input(n, "inputVectorRoleNone")?,
        &t.property_types.float,
        "GfVec3f",
    )?;

    check(
        output(n, "outputSurface")?,
        &t.property_types.terminal,
        "TfToken",
    )?;

    let normal_in = input(n, "normal")?;
    py_assert(
        normal_in.get_implementation_name() == "aliasedNormalInput",
        "implementationName",
    )?;
    py_assert(validate_on_node(n, normal_in), "validate normal")?;

    if n.get_name() != "TestShaderPropertiesNodeOSL"
        && n.get_name() != "TestShaderPropertiesNodeARGS"
    {
        check(
            input(n, "inputColor4")?,
            &t.property_types.color4,
            "GfVec4f",
        )?;
        check(
            input(n, "inputColor4Array")?,
            &t.property_types.color4,
            "VtArray<GfVec4f>",
        )?;
        check(
            input(n, "inputColor4RoleNone")?,
            &t.property_types.float,
            "GfVec4f",
        )?;
    }

    Ok(())
}

/// Registers `pxr.Sdr.shaderParserTestUtils` (native test helpers, OpenUSD parity).
pub fn register(py: Python<'_>, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(py, "shaderParserTestUtils")?;
    m.add_function(wrap_pyfunction!(is_node_osl, &m)?)?;
    m.add_function(wrap_pyfunction!(get_type, &m)?)?;
    m.add_function(wrap_pyfunction!(test_basic_properties, &m)?)?;
    m.add_function(wrap_pyfunction!(test_shading_properties, &m)?)?;
    m.add_function(wrap_pyfunction!(test_basic_node, &m)?)?;
    m.add_function(wrap_pyfunction!(test_shader_specific_node, &m)?)?;
    m.add_function(wrap_pyfunction!(test_shader_properties_node, &m)?)?;
    parent.add_submodule(&m)?;
    py.import("sys")?
        .getattr("modules")?
        .set_item("pxr.Sdr.shaderParserTestUtils", &m)?;
    Ok(())
}
