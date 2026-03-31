// Port of pxr/imaging/hd/testenv/testHdDataSource.cpp

use usd_hd::data_source::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdRetainedContainerDataSource,
    HdRetainedSmallVectorDataSource, HdRetainedTypedSampledDataSource, HdTypedSampledDataSource,
    HdVectorDataSource, cast_to_container,
};
use usd_hd::schema::{
    HdMaterialInterfaceMappingSchema, HdMaterialInterfaceParameterContainerSchema,
    HdMaterialInterfaceParameterSchema, HdMeshSchema, HdMeshTopologySchema, HdPrimvarSchemaBuilder,
    HdPrimvarsSchema, HdXformSchema, PRIMVAR_FACE_VARYING, PRIMVAR_VARYING, ROLE_COLOR, ROLE_POINT,
};
use usd_tf::Token;
use usd_vt::Array;

fn t(s: &str) -> Token {
    Token::new(s)
}

#[test]
fn test_retained_typed_sampled_data_source() {
    let input_value = 5.0_f32;
    let ds = HdRetainedTypedSampledDataSource::new(input_value);

    let output = ds.get_typed_value(0.0);
    assert_eq!(output, input_value);
}

#[test]
fn test_retained_container_data_source_sizes() {
    let leaf: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(0_i32);

    // 1-entry container
    let c1 = HdRetainedContainerDataSource::new_1(t("a"), leaf.clone());
    let c1_container = cast_to_container(&(c1 as HdDataSourceBaseHandle));
    assert!(c1_container.is_some());
    assert_eq!(c1_container.unwrap().get_names().len(), 1);

    // 2-entry container
    let c2 = HdRetainedContainerDataSource::new_2(t("a"), leaf.clone(), t("b"), leaf.clone());
    let c2_container = cast_to_container(&(c2 as HdDataSourceBaseHandle));
    assert!(c2_container.is_some());
    assert_eq!(c2_container.unwrap().get_names().len(), 2);

    // 3-entry container
    let c3 = HdRetainedContainerDataSource::new_3(
        t("a"),
        leaf.clone(),
        t("b"),
        leaf.clone(),
        t("c"),
        leaf.clone(),
    );
    let c3_container = cast_to_container(&(c3 as HdDataSourceBaseHandle));
    assert!(c3_container.is_some());
    assert_eq!(c3_container.unwrap().get_names().len(), 3);

    // From entries
    let c6 = HdRetainedContainerDataSource::from_entries(&[
        (t("a"), leaf.clone()),
        (t("b"), leaf.clone()),
        (t("c"), leaf.clone()),
        (t("d"), leaf.clone()),
        (t("e"), leaf.clone()),
        (t("f"), leaf.clone()),
    ]);
    let c6_container = cast_to_container(&(c6 as HdDataSourceBaseHandle));
    assert!(c6_container.is_some());
    assert_eq!(c6_container.unwrap().get_names().len(), 6);
}

#[test]
fn test_nested_containers() {
    let leaf: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(0_i32);

    // Build nested: root -> a -> b -> leaf
    let inner = HdRetainedContainerDataSource::new_1(t("b"), leaf.clone());
    let middle = HdRetainedContainerDataSource::new_1(t("a"), inner as HdDataSourceBaseHandle);

    let root_container = cast_to_container(&(middle.clone() as HdDataSourceBaseHandle)).unwrap();
    assert_eq!(root_container.get_names().len(), 1);

    // Navigate into "a"
    let a_ds = root_container.get(&t("a"));
    assert!(a_ds.is_some());

    let a_container = cast_to_container(&a_ds.unwrap());
    assert!(a_container.is_some());
    assert_eq!(a_container.unwrap().get_names().len(), 1);
}

#[test]
fn test_retained_small_vector_data_source() {
    let values: Vec<HdDataSourceBaseHandle> = vec![
        HdRetainedTypedSampledDataSource::new(1_i32),
        HdRetainedTypedSampledDataSource::new(10_i32),
        HdRetainedTypedSampledDataSource::new(20_i32),
    ];

    let vector_ds = HdRetainedSmallVectorDataSource::new(&values);
    assert_eq!(vector_ds.get_num_elements(), 3);

    assert!(vector_ds.get_element(0).is_some());
    assert!(vector_ds.get_element(1).is_some());
    assert!(vector_ds.get_element(2).is_some());

    // Out of bounds
    assert!(vector_ds.get_element(3).is_none());
}

#[test]
fn test_bool_data_source_values() {
    let t1 = HdRetainedTypedSampledDataSource::new(true);
    let t2 = HdRetainedTypedSampledDataSource::new(true);
    let f1 = HdRetainedTypedSampledDataSource::new(false);
    let f2 = HdRetainedTypedSampledDataSource::new(false);

    assert!(t1.get_typed_value(0.0));
    assert!(t2.get_typed_value(0.0));
    assert!(!f1.get_typed_value(0.0));
    assert!(!f2.get_typed_value(0.0));
}

// ---------------------------------------------------------------------------
// Helper: builds the same mesh prim data source used in C++ _GetMeshPrimDataSource()
// Contains primvars (points, displayColor, displayOpacity), mesh topology, and xform.
// ---------------------------------------------------------------------------
fn get_mesh_prim_data_source() -> usd_hd::data_source::HdContainerDataSourceHandle {
    use usd_gf::{Matrix4d, Vec3f};

    // Points primvar: 7 vertices, interpolation=varying, role=point.
    // Uses Vec<Vec3f> because HdSampledDataSource is only implemented for Vec<Vec3f>,
    // not Array<Vec3f>.
    let points_ds = HdPrimvarSchemaBuilder::new()
        .set_primvar_value(HdRetainedTypedSampledDataSource::new(vec![
            Vec3f::new(0.5, -0.5, -0.5),
            Vec3f::new(0.5, -0.5, 0.5),
            Vec3f::new(-0.5, -0.5, 0.5),
            Vec3f::new(-0.5, 0.5, -0.5),
            Vec3f::new(0.5, 0.5, -0.5),
            Vec3f::new(0.5, 0.5, 0.5),
            Vec3f::new(-0.5, 0.5, 0.5),
        ]))
        .set_interpolation(HdRetainedTypedSampledDataSource::new(
            PRIMVAR_VARYING.clone(),
        ))
        .set_role(HdRetainedTypedSampledDataSource::new(ROLE_POINT.clone()))
        .build();

    // displayColor: indexed, 4 color values, faceVarying interpolation, role=color
    let display_color_ds = HdPrimvarSchemaBuilder::new()
        .set_indexed_primvar_value(HdRetainedTypedSampledDataSource::new(vec![
            Vec3f::new(1.0, 0.0, 0.0),
            Vec3f::new(0.0, 1.0, 0.0),
            Vec3f::new(0.0, 0.0, 1.0),
            Vec3f::new(1.0, 1.0, 1.0),
        ]))
        .set_indices(HdRetainedTypedSampledDataSource::new(Array::from(vec![
            3, 3, 3, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3_i32,
        ])))
        .set_interpolation(HdRetainedTypedSampledDataSource::new(
            PRIMVAR_FACE_VARYING.clone(),
        ))
        .set_role(HdRetainedTypedSampledDataSource::new(ROLE_COLOR.clone()))
        .build();

    // displayOpacity: non-indexed, 24 float values, faceVarying
    let display_opacity_ds = HdPrimvarSchemaBuilder::new()
        .set_primvar_value(HdRetainedTypedSampledDataSource::new(vec![
            0.6_f32, 0.6, 0.6, 0.6, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.6, 0.6, 0.6, 0.6,
            1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0,
        ]))
        .set_interpolation(HdRetainedTypedSampledDataSource::new(
            PRIMVAR_FACE_VARYING.clone(),
        ))
        .build();

    // Primvars container
    let primvars = HdPrimvarsSchema::build_retained(
        &[t("points"), t("displayColor"), t("displayOpacity")],
        &[
            points_ds as HdDataSourceBaseHandle,
            display_color_ds as HdDataSourceBaseHandle,
            display_opacity_ds as HdDataSourceBaseHandle,
        ],
    );

    // Mesh topology: cube with 6 quads (leftHanded)
    let topology_container = HdMeshTopologySchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(Array::from(vec![
            4_i32, 4, 4, 4, 4, 4,
        ]))),
        Some(HdRetainedTypedSampledDataSource::new(Array::from(vec![
            1_i32, 5, 4, 0, 2, 6, 5, 1, 3, 7, 6, 2, 0, 4, 7, 3, 2, 1, 0, 3, 5, 6, 7, 4,
        ]))),
        None,
        Some(HdRetainedTypedSampledDataSource::new(t("leftHanded"))),
    );

    // Mesh container wrapping topology
    let mesh_container = HdMeshSchema::build_retained(Some(topology_container), None, None, None);

    // Xform: translation at (10, 20, 30)
    let xform_container = HdXformSchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(Matrix4d::new(
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 30.0, 1.0,
        ))),
        None,
    );

    // Assemble the prim container: primvars + mesh + xform
    HdRetainedContainerDataSource::from_entries(&[
        (t("primvars"), primvars as HdDataSourceBaseHandle),
        (t("mesh"), mesh_container as HdDataSourceBaseHandle),
        (t("xform"), xform_container as HdDataSourceBaseHandle),
    ])
}

// Port of C++ TestMeshTopologySchema
#[test]
fn test_mesh_topology_schema() {
    let prim_ds = get_mesh_prim_data_source();

    // Retrieve mesh schema from prim
    let mesh = HdMeshSchema::get_from_parent(&prim_ds);
    assert!(mesh.is_defined(), "mesh schema must be defined");

    // Retrieve topology schema from mesh container
    let topology = mesh.get_topology();
    assert!(topology.is_some(), "mesh topology must be present");
    let topology = topology.unwrap();

    // faceVertexCounts: 6 quads
    let counts = topology.get_face_vertex_counts();
    assert!(counts.is_some(), "faceVertexCounts must be present");
    let counts_arr = counts.unwrap().get_typed_value(0.0);
    assert_eq!(counts_arr.as_slice(), &[4_i32, 4, 4, 4, 4, 4]);

    // faceVertexIndices: 24 indices for a cube
    let indices = topology.get_face_vertex_indices();
    assert!(indices.is_some(), "faceVertexIndices must be present");
    let indices_arr = indices.unwrap().get_typed_value(0.0);
    assert_eq!(
        indices_arr.as_slice(),
        &[
            1_i32, 5, 4, 0, 2, 6, 5, 1, 3, 7, 6, 2, 0, 4, 7, 3, 2, 1, 0, 3, 5, 6, 7, 4
        ]
    );

    // Orientation: leftHanded
    let orientation = topology.get_orientation();
    assert!(orientation.is_some(), "orientation must be present");
    assert_eq!(orientation.unwrap().get_typed_value(0.0), t("leftHanded"));
}

// Port of C++ TestXformSchema
#[test]
fn test_xform_schema() {
    use usd_gf::Matrix4d;

    let prim_ds = get_mesh_prim_data_source();

    // Retrieve xform schema directly from the prim container
    let xform = HdXformSchema::get_from_parent(&prim_ds);
    assert!(xform.is_defined(), "xform schema must be defined");

    let matrix_ds = xform.get_matrix();
    assert!(matrix_ds.is_some(), "matrix data source must be present");

    let matrix = matrix_ds.unwrap().get_typed_value(0.0);

    // Identity upper 3x3, translation in row 3 = (10, 20, 30)
    let expected = Matrix4d::new(
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 30.0, 1.0,
    );
    assert_eq!(matrix, expected);
}

// Port of C++ TestPrimvarSchema
// Verifies non-indexed (displayOpacity) and indexed (displayColor) primvars.
#[test]
fn test_primvar_schema() {
    use usd_gf::Vec3f;

    let prim_ds = get_mesh_prim_data_source();

    let primvars = HdPrimvarsSchema::get_from_parent(&prim_ds);
    assert!(primvars.is_defined(), "primvars schema must be defined");

    // Check all three primvar names are present
    let names = primvars.get_primvar_names();
    assert!(names.contains(&t("points")), "points primvar must exist");
    assert!(
        names.contains(&t("displayColor")),
        "displayColor primvar must exist"
    );
    assert!(
        names.contains(&t("displayOpacity")),
        "displayOpacity primvar must exist"
    );

    // --- displayOpacity: non-indexed ---
    let opacity_schema = primvars.get_primvar_schema(&t("displayOpacity"));
    assert!(
        !opacity_schema.is_indexed(),
        "displayOpacity must not be indexed"
    );

    let opacity_ds = opacity_schema.get_primvar_value();
    assert!(
        opacity_ds.is_some(),
        "displayOpacity primvarValue must be present"
    );
    // Bind the base handle before borrowing as_sampled() to avoid temporary drop
    let opacity_base = opacity_ds.unwrap();
    let sampled = opacity_base
        .as_sampled()
        .expect("opacity data source must be sampled");
    let v = sampled.get_value(0.0);
    // The value is Vec<f32> (HdSampledDataSource is implemented for Vec<f32> but not Array<f32>)
    let arr: &Vec<f32> = v.downcast().expect("displayOpacity must be Vec<f32>");
    assert_eq!(arr.len(), 24);
    // Spot-check a few values
    assert!((arr[0] - 0.6).abs() < 1e-6);
    assert!((arr[4] - 1.0).abs() < 1e-6);
    assert!((arr[20] - 0.0).abs() < 1e-6);

    // --- displayColor: indexed ---
    let color_schema = primvars.get_primvar_schema(&t("displayColor"));
    assert!(color_schema.is_indexed(), "displayColor must be indexed");

    // Indexed primvar value: 4 colors
    let indexed_val_ds = color_schema.get_indexed_primvar_value();
    assert!(
        indexed_val_ds.is_some(),
        "displayColor indexedPrimvarValue must be present"
    );
    // Bind to a local so the HdDataSourceBaseHandle lives long enough for as_sampled() borrow
    let indexed_base = indexed_val_ds.unwrap();
    let indexed_sampled = indexed_base
        .as_sampled()
        .expect("indexed primvar value must be sampled");
    let indexed_v = indexed_sampled.get_value(0.0);
    // Vec<Vec3f> matches what was stored (HdSampledDataSource impl is for Vec<Vec3f>)
    let indexed_arr: &Vec<Vec3f> = indexed_v
        .downcast()
        .expect("displayColor indexed value must be Vec<Vec3f>");
    assert_eq!(indexed_arr.len(), 4);
    assert_eq!(indexed_arr[0], Vec3f::new(1.0, 0.0, 0.0));
    assert_eq!(indexed_arr[3], Vec3f::new(1.0, 1.0, 1.0));

    // Indices: 24 elements
    let indices_ds = color_schema.get_indices();
    assert!(indices_ds.is_some(), "displayColor indices must be present");
    let indices = indices_ds.unwrap().get_typed_value(0.0);
    assert_eq!(indices.len(), 24);
    let expected_indices = [
        3_i32, 3, 3, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3,
    ];
    assert_eq!(indices.as_slice(), &expected_indices);
}

// Port of C++ TestContainerSchemas.
// Tests HdMaterialInterfaceParameterContainerSchema with the builder pattern.
// Also verifies that concrete type filtering via as_any() downcast works correctly,
// matching the C++ HdContainerOfTypedSampledDataSourcesSchema::Get() Cast() semantics.
#[test]
fn test_container_schemas() {
    use usd_hd::data_source::HdRetainedTypedSampledDataSource as Rtsd;

    // Build a mixed-type container: two ints and one float.
    // This mirrors C++ TestContainerSchemas creating mixed int/float containers.
    let c1 = HdRetainedContainerDataSource::from_entries(&[
        (
            t("a"),
            HdRetainedTypedSampledDataSource::new(1_i32) as HdDataSourceBaseHandle,
        ),
        (
            t("b"),
            HdRetainedTypedSampledDataSource::new(2_i32) as HdDataSourceBaseHandle,
        ),
        (
            t("c"),
            HdRetainedTypedSampledDataSource::new(3.0_f32) as HdDataSourceBaseHandle,
        ),
    ]);

    // Type-filtering: int children can be downcast to i32 retained ds, float cannot.
    // This mirrors C++ HdContainerOfTypedSampledDataSourcesSchema<HdIntDataSource>::Get() behavior.
    let ds_a = c1.get(&t("a")).unwrap();
    assert!(
        ds_a.as_any().downcast_ref::<Rtsd<i32>>().is_some(),
        "'a' (i32) must downcast to Rtsd<i32>"
    );
    assert_eq!(
        ds_a.as_any()
            .downcast_ref::<Rtsd<i32>>()
            .unwrap()
            .get_typed_value(0.0),
        1_i32
    );

    let ds_b = c1.get(&t("b")).unwrap();
    assert_eq!(
        ds_b.as_any()
            .downcast_ref::<Rtsd<i32>>()
            .unwrap()
            .get_typed_value(0.0),
        2_i32
    );

    // 'c' is f32 - must NOT downcast to i32 retained ds
    let ds_c = c1.get(&t("c")).unwrap();
    assert!(
        ds_c.as_any().downcast_ref::<Rtsd<i32>>().is_none(),
        "float 'c' must not downcast to Rtsd<i32>"
    );
    assert!(
        ds_c.as_any().downcast_ref::<Rtsd<f32>>().is_some(),
        "float 'c' must downcast to Rtsd<f32>"
    );

    // --- HdMaterialInterfaceParameterSchema via builder ---
    // Build two mappings: (A, x) and (B, y)
    let mapping0 = HdMaterialInterfaceMappingSchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(t("A"))),
        Some(HdRetainedTypedSampledDataSource::new(t("x"))),
    );
    let mapping1 = HdMaterialInterfaceMappingSchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(t("B"))),
        Some(HdRetainedTypedSampledDataSource::new(t("y"))),
    );

    let mappings_vec = HdRetainedSmallVectorDataSource::new(&[
        mapping0 as HdDataSourceBaseHandle,
        mapping1 as HdDataSourceBaseHandle,
    ]);

    // Build interface parameter with displayGroup="Foo", displayName="Bar"
    let param = HdMaterialInterfaceParameterSchema::build_retained(
        Some(HdRetainedTypedSampledDataSource::new(t("Foo"))),
        Some(HdRetainedTypedSampledDataSource::new(t("Bar"))),
        Some(mappings_vec),
    );

    // Wrap it in a named container (parameter name = "Q")
    let params_container =
        HdRetainedContainerDataSource::from_entries(&[(t("Q"), param as HdDataSourceBaseHandle)]);
    let parameters = HdMaterialInterfaceParameterContainerSchema::new(params_container);

    // displayGroup must be "Foo"
    let display_group_ds = parameters.get(&t("Q")).get_display_group();
    assert!(
        display_group_ds.is_some(),
        "displayGroup data source must be present"
    );
    assert_eq!(
        display_group_ds.unwrap().get_typed_value(0.0),
        t("Foo"),
        "displayGroup value must be 'Foo'"
    );

    // displayName must be "Bar"
    let display_name_ds = parameters.get(&t("Q")).get_display_name();
    assert!(
        display_name_ds.is_some(),
        "displayName data source must be present"
    );
    assert_eq!(
        display_name_ds.unwrap().get_typed_value(0.0),
        t("Bar"),
        "displayName value must be 'Bar'"
    );

    // Mappings vector must be present with 2 elements
    let param_schema = parameters.get(&t("Q"));
    let mappings = param_schema.get_mappings();
    assert!(mappings.is_defined(), "mappings vector must be defined");
    assert_eq!(mappings.get_num_elements(), 2);

    // Second mapping (index 1): nodePath="B"
    let second_mapping = param_schema.get_mapping_element(1);
    let node_path_ds = second_mapping.get_node_path();
    assert!(
        node_path_ds.is_some(),
        "second mapping nodePath must be present"
    );
    assert_eq!(
        node_path_ds.unwrap().get_typed_value(0.0),
        t("B"),
        "second mapping nodePath must be 'B'"
    );
}
