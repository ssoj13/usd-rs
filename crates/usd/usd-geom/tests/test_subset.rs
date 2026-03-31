//! Tests for UsdGeomSubset, ported from testUsdGeomSubset.py

use std::path::PathBuf;
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_geom::tokens::usd_geom_tokens;
use usd_geom::{Imageable, Subset};
use usd_sdf::TimeCode;
use usd_tf::Token;

// ============================================================================
// Helpers
// ============================================================================

fn testenv_path(file: &str) -> String {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("testenv");
    path.push("testUsdGeomSubset");
    path.push(file);
    path.to_string_lossy().to_string()
}

fn open_stage(file: &str) -> Arc<Stage> {
    usd_sdf::init();
    Stage::open(testenv_path(file), InitialLoadSet::LoadAll).expect("Failed to open test stage")
}

/// Helper to get string value from an attribute at default time.
fn attr_get_string(attr: &usd_core::Attribute) -> String {
    attr.get(TimeCode::default_time())
        .and_then(|v| v.downcast_clone::<String>())
        .unwrap_or_default()
}

/// Helper to get indices from an attribute at default time (handles both Vec and Array storage).
fn attr_get_indices(attr: &usd_core::Attribute) -> Vec<i32> {
    attr.get_typed_vec::<i32>(TimeCode::default_time())
        .unwrap_or_default()
}

/// Validates a family and checks that the result matches expectations.
///
/// Ported from `_ValidateFamily` in the Python test.
fn validate_family(
    geom: &Imageable,
    element_type: &Token,
    family_name: &str,
    expected_is_valid: bool,
    expected_reasons: &[&str],
) {
    let (valid, reason) = Subset::validate_family(geom, element_type, &Token::new(family_name));
    if expected_is_valid {
        assert!(
            valid,
            "Subset family '{}' was found to be invalid: {}",
            family_name, reason
        );
        assert_eq!(
            reason.len(),
            0,
            "Valid family '{}' should have empty reason, got: {}",
            family_name,
            reason
        );
    } else {
        assert!(!valid, "Subset family '{}' should be invalid", family_name);
        assert!(
            !reason.is_empty(),
            "Invalid family '{}' should have a reason",
            family_name
        );
        for expected_reason in expected_reasons {
            assert!(
                reason.contains(expected_reason),
                "Family '{}' reason should contain '{}', got: '{}'",
                family_name,
                expected_reason,
                reason
            );
        }
    }
}

/// Ported from `_TestSubsetRetrieval`.
fn test_subset_retrieval(geom: &Imageable, element_type: &Token, family_name: &str) {
    let prefix = format!("{}_{}", element_type.as_str(), family_name);

    let material_bind_subsets = Subset::get_geom_subsets(geom, element_type, &Token::new(&prefix));
    assert_eq!(
        material_bind_subsets.len(),
        3,
        "Expected 3 subsets for family '{}', got {}",
        prefix,
        material_bind_subsets.len()
    );

    assert_eq!(
        usd_geom_tokens().partition,
        Subset::get_family_type(geom, &Token::new(&prefix))
    );

    validate_family(geom, element_type, &prefix, true, &[]);
}

/// Ported from `_TestSubsetValidity`.
///
/// Checks valid and invalid families across the given geom prims.
fn test_subset_validity(
    geom: &Imageable,
    varying_geom: &Imageable,
    null_geom: &Imageable,
    element_type: &Token,
) {
    let prefix = format!("{}_", element_type.as_str());

    // Valid families
    let valid_families = [
        "validPartition",
        "validNonOverlapping",
        "validUnrestricted",
        "emptyIndicesSomeTimes",
    ];
    for family_name in &valid_families {
        let full_name = format!("{}{}", prefix, family_name);
        validate_family(geom, element_type, &full_name, true, &[]);
    }

    // Invalid families depend on element type
    let invalid_families: Vec<(&str, Vec<&str>)> = if *element_type == usd_geom_tokens().edge {
        vec![
            (
                "invalidIndices",
                vec![
                    "does not exist on the parent prim",
                    "Indices attribute has an odd number of elements",
                    "Found one or more indices that are less than 0",
                ],
            ),
            (
                "badPartition1",
                vec![
                    "does not match the element count",
                    "does not exist on the parent prim",
                ],
            ),
            ("badPartition2", vec!["does not match the element count"]),
            ("badPartition3", vec!["Found duplicate edge"]),
            ("invalidNonOverlapping", vec!["Found duplicate edge"]),
            (
                "invalidUnrestricted",
                vec![
                    "does not exist on the parent prim",
                    "Found one or more indices that are less than 0",
                ],
            ),
            (
                "onlyNegativeIndices",
                vec![
                    "Found one or more indices that are less than 0",
                    "does not exist on the parent prim",
                ],
            ),
            (
                "emptyIndicesAtAllTimes",
                vec!["No indices in family at any time"],
            ),
        ]
    } else if *element_type == usd_geom_tokens().segment {
        vec![
            (
                "invalidIndices",
                vec![
                    "greater than the curve vertex count",
                    "greater than the segment count",
                    "Indices attribute has an odd number of elements",
                    "Found one or more indices that are less than 0",
                ],
            ),
            (
                "badPartition1",
                vec!["Found one or more indices that are greater than the curve vertex count"],
            ),
            ("badPartition2", vec!["does not match the element count"]),
            ("badPartition3", vec!["Found duplicate segment"]),
            ("invalidNonOverlapping", vec!["Found duplicate segment"]),
            (
                "invalidUnrestricted",
                vec![
                    "Found one or more indices that are greater than the segment count",
                    "Found one or more indices that are less than 0",
                ],
            ),
            (
                "onlyNegativeIndices",
                vec!["Found one or more indices that are less than 0"],
            ),
            (
                "emptyIndicesAtAllTimes",
                vec!["No indices in family at any time"],
            ),
        ]
    } else {
        // face, point, tetrahedron
        vec![
            (
                "invalidIndices",
                vec![
                    "Found one or more indices that are greater than the element count",
                    "Found one or more indices that are less than 0",
                ],
            ),
            (
                "badPartition1",
                vec![
                    "does not match the element count",
                    "Found one or more indices that are greater than the element count",
                ],
            ),
            ("badPartition2", vec!["does not match the element count"]),
            ("badPartition3", vec!["Found duplicate index"]),
            ("invalidNonOverlapping", vec!["Found duplicate index"]),
            (
                "invalidUnrestricted",
                vec![
                    "Found one or more indices that are greater than the element count",
                    "Found one or more indices that are less than 0",
                ],
            ),
            (
                "onlyNegativeIndices",
                vec!["Found one or more indices that are less than 0"],
            ),
            (
                "emptyIndicesAtAllTimes",
                vec!["No indices in family at any time"],
            ),
        ]
    };

    for (family_name, reasons) in &invalid_families {
        let full_name = format!("{}{}", prefix, family_name);
        let reason_strs: Vec<&str> = reasons.iter().map(|s| *s).collect();
        validate_family(geom, element_type, &full_name, false, &reason_strs);
    }

    // Varying geom: valid families
    let valid_families_varying = ["validPartition"];
    for family_name in &valid_families_varying {
        let full_name = format!("{}{}", prefix, family_name);
        validate_family(varying_geom, element_type, &full_name, true, &[]);
    }

    // Varying geom: invalid families
    let invalid_families_varying: Vec<(&str, Vec<&str>)> =
        vec![("invalidNoDefaultTimeElements", vec!["has no elements"])];
    for (family_name, reasons) in &invalid_families_varying {
        let full_name = format!("{}{}", prefix, family_name);
        let reason_strs: Vec<&str> = reasons.iter().map(|s| *s).collect();
        validate_family(varying_geom, element_type, &full_name, false, &reason_strs);
    }

    // Null geom: invalid families
    let invalid_families_null: Vec<(&str, Vec<&str>)> = vec![
        (
            "emptyIndicesAtAllTimes",
            vec!["No indices in family at any time"],
        ),
        (
            "invalidPartition",
            vec!["Unable to determine element count at earliest time"],
        ),
    ];
    for (family_name, reasons) in &invalid_families_null {
        let full_name = format!("{}{}", prefix, family_name);
        let reason_strs: Vec<&str> = reasons.iter().map(|s| *s).collect();
        validate_family(null_geom, element_type, &full_name, false, &reason_strs);
    }
}

// ============================================================================
// test_SubsetRetrievalAndValidity
// ============================================================================

/// Tests subset retrieval and validation on Mesh (face, point, edge),
/// TetMesh (tetrahedron, face), and BasisCurves (segment).
///
/// Ported from `test_SubsetRetrievalAndValidity`.
#[test]
fn test_subset_retrieval_and_validity() {
    let stage = open_stage("Sphere.usda");

    // --- Mesh: face, point, edge ---
    let sphere = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/pSphere1").expect("path"))
        .expect("prim");
    let geom = Imageable::new(sphere);
    assert!(geom.is_valid());

    let varying_mesh = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/VaryingMesh").expect("path"))
        .expect("prim");
    let varying_geom = Imageable::new(varying_mesh);
    assert!(varying_geom.is_valid());

    let null_mesh = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/NullMesh").expect("path"))
        .expect("prim");
    let null_geom = Imageable::new(null_mesh);
    assert!(null_geom.is_valid());

    test_subset_retrieval(&geom, &usd_geom_tokens().face, "materialBind");
    test_subset_validity(&geom, &varying_geom, &null_geom, &usd_geom_tokens().face);

    test_subset_retrieval(&geom, &usd_geom_tokens().point, "physicsAttachment");
    test_subset_validity(&geom, &varying_geom, &null_geom, &usd_geom_tokens().point);

    test_subset_retrieval(&geom, &usd_geom_tokens().edge, "physicsAttachment");
    test_subset_validity(&geom, &varying_geom, &null_geom, &usd_geom_tokens().edge);

    // --- TetMesh: tetrahedron, face ---
    let tet_prim = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/TetMesh").expect("path"))
        .expect("prim");
    let tet_geom = Imageable::new(tet_prim);
    assert!(tet_geom.is_valid());

    let varying_tet = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/VaryingTetMesh").expect("path"))
        .expect("prim");
    let varying_tet_geom = Imageable::new(varying_tet);
    assert!(varying_tet_geom.is_valid());

    let null_tet = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/NullTetMesh").expect("path"))
        .expect("prim");
    let null_tet_geom = Imageable::new(null_tet);
    assert!(null_tet_geom.is_valid());

    test_subset_retrieval(&tet_geom, &usd_geom_tokens().tetrahedron, "materialBind");
    test_subset_validity(
        &tet_geom,
        &varying_tet_geom,
        &null_tet_geom,
        &usd_geom_tokens().tetrahedron,
    );

    test_subset_retrieval(&tet_geom, &usd_geom_tokens().face, "materialBind");
    test_subset_validity(
        &tet_geom,
        &varying_tet_geom,
        &null_tet_geom,
        &usd_geom_tokens().face,
    );

    // --- BasisCurves: segment ---
    let curves_prim = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/BasisCurves").expect("path"))
        .expect("prim");
    let curves_geom = Imageable::new(curves_prim);
    assert!(curves_geom.is_valid());

    let varying_curves = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/VaryingBasisCurves").expect("path"))
        .expect("prim");
    let varying_curves_geom = Imageable::new(varying_curves);
    assert!(varying_curves_geom.is_valid());

    let null_curves = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/NullBasisCurves").expect("path"))
        .expect("prim");
    let null_curves_geom = Imageable::new(null_curves);
    assert!(null_curves_geom.is_valid());

    test_subset_retrieval(
        &curves_geom,
        &usd_geom_tokens().segment,
        "physicsAttachment",
    );
    test_subset_validity(
        &curves_geom,
        &varying_curves_geom,
        &null_curves_geom,
        &usd_geom_tokens().segment,
    );
}

// ============================================================================
// test_GetUnassignedIndicesForEdges
// ============================================================================

/// Ported from `test_GetUnassignedIndicesForEdges`.
#[test]
fn test_get_unassigned_indices_for_edges() {
    let stage = open_stage("Sphere.usda");
    let sphere = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/SimpleEdges").expect("path"))
        .expect("prim");
    let geom = Imageable::new(sphere);
    assert!(geom.is_valid());

    let tokens = usd_geom_tokens();

    // Create subset with empty indices
    let new_subset = Subset::create_geom_subset(
        &geom,
        &Token::new("testEdge"),
        &tokens.edge,
        &[],
        &Token::new(""),
        &Token::new(""),
    );
    new_subset
        .get_family_name_attr()
        .set("testEdgeFamily", TimeCode::default_time());

    // Indices are empty when unassigned
    let indices = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(indices, vec![] as Vec<i32>);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.edge,
        &Token::new("testEdgeFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![0, 1, 0, 3, 0, 4, 1, 2, 1, 5, 2, 3, 4, 5]);

    // Some indices are assigned
    let indices_to_set: Vec<i32> = vec![0, 1, 5, 4];
    new_subset
        .get_indices_attr()
        .set(indices_to_set.clone(), TimeCode::default_time());
    let got = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(got, vec![0, 1, 5, 4]);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.edge,
        &Token::new("testEdgeFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![0, 3, 0, 4, 1, 2, 1, 5, 2, 3]);

    // All indices are assigned
    let all_indices: Vec<i32> = vec![0, 1, 0, 3, 0, 4, 1, 2, 1, 5, 2, 3, 4, 5];
    new_subset
        .get_indices_attr()
        .set(all_indices.clone(), TimeCode::default_time());
    let got = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(got, all_indices);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.edge,
        &Token::new("testEdgeFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![] as Vec<i32>);

    // GetUnassignedIndices still works with invalid indices
    let invalid_indices: Vec<i32> = vec![0, 1, 0, 3, 0, 4, 1, 2, 2, 3, 4, 5, 7, -1];
    let new_subset2 = Subset::create_geom_subset(
        &geom,
        &Token::new("testEdge"),
        &tokens.edge,
        &invalid_indices,
        &Token::new(""),
        &Token::new(""),
    );
    new_subset2
        .get_family_name_attr()
        .set("testEdgeFamily", TimeCode::default_time());
    let got = attr_get_indices(&new_subset2.get_indices_attr());
    assert_eq!(got, invalid_indices);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.edge,
        &Token::new("testEdgeFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![1, 5]);
}

// ============================================================================
// test_GetUnassignedIndicesForSegments
// ============================================================================

/// Ported from `test_GetUnassignedIndicesForSegments`.
#[test]
fn test_get_unassigned_indices_for_segments() {
    let stage = open_stage("Sphere.usda");
    let sphere = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/SimpleSegments").expect("path"))
        .expect("prim");
    let geom = Imageable::new(sphere);
    assert!(geom.is_valid());

    let tokens = usd_geom_tokens();

    // Create subset with empty indices
    let new_subset = Subset::create_geom_subset(
        &geom,
        &Token::new("testSegment"),
        &tokens.segment,
        &[],
        &Token::new(""),
        &Token::new(""),
    );
    new_subset
        .get_family_name_attr()
        .set("testSegmentFamily", TimeCode::default_time());

    // Indices are empty when unassigned
    let indices = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(indices, vec![] as Vec<i32>);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.segment,
        &Token::new("testSegmentFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![0, 0, 0, 1, 0, 2, 1, 0, 1, 1]);

    // Some indices are assigned
    let indices_to_set: Vec<i32> = vec![0, 0, 1, 0];
    new_subset
        .get_indices_attr()
        .set(indices_to_set.clone(), TimeCode::default_time());
    let got = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(got, vec![0, 0, 1, 0]);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.segment,
        &Token::new("testSegmentFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![0, 1, 0, 2, 1, 1]);

    // All indices are assigned
    let all_indices: Vec<i32> = vec![0, 0, 0, 1, 0, 2, 1, 0, 1, 1];
    new_subset
        .get_indices_attr()
        .set(all_indices.clone(), TimeCode::default_time());
    let got = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(got, all_indices);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.segment,
        &Token::new("testSegmentFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![] as Vec<i32>);

    // GetUnassignedIndices still works with invalid indices
    let invalid_indices: Vec<i32> = vec![0, 0, 1, 0, -1, 0, 5, 2];
    let new_subset2 = Subset::create_geom_subset(
        &geom,
        &Token::new("testSegment"),
        &tokens.segment,
        &invalid_indices,
        &Token::new(""),
        &Token::new(""),
    );
    new_subset2
        .get_family_name_attr()
        .set("testSegmentFamily", TimeCode::default_time());
    let got = attr_get_indices(&new_subset2.get_indices_attr());
    assert_eq!(got, invalid_indices);

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.segment,
        &Token::new("testSegmentFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![0, 1, 0, 2, 1, 1]);
}

// ============================================================================
// test_CreateGeomSubset
// ============================================================================

/// Ported from `test_CreateGeomSubset`.
#[test]
fn test_create_geom_subset() {
    let stage = open_stage("Sphere.usda");
    let sphere = stage
        .get_prim_at_path(&usd_sdf::Path::from_string("/Sphere/pSphere1").expect("path"))
        .expect("prim");
    let geom = Imageable::new(sphere.clone());
    assert!(geom.is_valid());

    let tokens = usd_geom_tokens();

    // Create a new subset with empty indices
    let new_subset = Subset::create_geom_subset(
        &geom,
        &Token::new("testSubset"),
        &tokens.face,
        &[],
        &Token::new(""),
        &Token::new(""),
    );

    // Check elementType
    let element_type = attr_get_string(&new_subset.get_element_type_attr());
    assert_eq!(element_type, "face");

    // Check familyName (default is empty)
    let family_name = attr_get_string(&new_subset.get_family_name_attr());
    assert_eq!(family_name, "");

    // Set familyName
    new_subset
        .get_family_name_attr()
        .set("testFamily", TimeCode::default_time());
    let family_name = attr_get_string(&new_subset.get_family_name_attr());
    assert_eq!(family_name, "testFamily");

    // Indices are empty when unassigned
    let indices = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(indices, vec![] as Vec<i32>);

    // Unassigned indices should be 0..16
    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.face,
        &Token::new("testFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, (0..16).collect::<Vec<i32>>());

    // Set some indices
    let indices_to_set: Vec<i32> = vec![1, 2, 3, 4, 5];
    new_subset
        .get_indices_attr()
        .set(indices_to_set.clone(), TimeCode::default_time());
    let got = attr_get_indices(&new_subset.get_indices_attr());
    assert_eq!(got, indices_to_set);

    // By default, a family of subsets is not tagged as a partition
    assert_eq!(
        Subset::get_family_type(&geom, &Token::new("testFamily")),
        tokens.unrestricted
    );

    // Ensure that there's only one subset belonging to 'testFamily'
    let test_subsets = Subset::get_geom_subsets(&geom, &tokens.face, &Token::new("testFamily"));
    assert_eq!(test_subsets.len(), 1);

    // Calling CreateGeomSubset with the same subsetName updates the existing subset
    let new_indices: Vec<i32> = vec![0, 1, 2];
    let newer_subset = Subset::create_geom_subset(
        &geom,
        &Token::new("testSubset"),
        &tokens.face,
        &new_indices,
        &Token::new("testFamily"),
        &tokens.partition,
    );
    assert_eq!(
        newer_subset.prim().path().get_string(),
        new_subset.prim().path().get_string()
    );

    // Count is still one as no new subset was created
    let test_subsets = Subset::get_geom_subsets(&geom, &tokens.face, &Token::new("testFamily"));
    assert_eq!(test_subsets.len(), 1);

    // Family type should now be "partition"
    let is_tagged_as_partition =
        tokens.partition == Subset::get_family_type(&geom, &Token::new("testFamily"));
    assert!(is_tagged_as_partition);
    assert_eq!(
        Subset::get_family_type(&geom, &Token::new("testFamily")),
        tokens.partition
    );

    // Validate: should be invalid because indices [0,1,2] don't cover all 16 faces
    validate_family(
        &geom,
        &tokens.face,
        "testFamily",
        false,
        &["does not match the element count"],
    );

    // Unassigned indices should be 3..16
    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.face,
        &Token::new("testFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, (3..16).collect::<Vec<i32>>());

    // CreateUniqueGeomSubset with invalid indices 3..20
    let another_subset = Subset::create_unique_geom_subset(
        &geom,
        &Token::new("testSubset"),
        &tokens.face,
        &(3..20).collect::<Vec<i32>>(),
        &Token::new("testFamily"),
        &tokens.partition,
    );

    // CreateUniqueGeomSubset always creates a new subset
    assert_eq!(another_subset.prim().get_name().as_str(), "testSubset_1");
    assert_ne!(
        another_subset.prim().get_name().as_str(),
        new_subset.prim().get_name().as_str()
    );

    let another_indices = attr_get_indices(&another_subset.get_indices_attr());
    assert_eq!(another_indices, (3..20).collect::<Vec<i32>>());

    // GetUnassignedIndices still works if element count < number of assigned indices (USD-5599)
    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.face,
        &Token::new("testFamily"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, vec![] as Vec<i32>);

    // Count is now two after CreateUniqueGeomSubset
    let test_subsets = Subset::get_geom_subsets(&geom, &tokens.face, &Token::new("testFamily"));
    assert_eq!(test_subsets.len(), 2);

    // Update anotherSubset to contain valid indices
    let _another_subset_updated = Subset::create_geom_subset(
        &geom,
        &Token::new("testSubset_1"),
        &tokens.face,
        &(3..16).collect::<Vec<i32>>(),
        &Token::new("testFamily"),
        &tokens.partition,
    );
    let test_subsets = Subset::get_geom_subsets(&geom, &tokens.face, &Token::new("testFamily"));
    assert_eq!(test_subsets.len(), 2);

    // Now the family should be valid
    validate_family(&geom, &tokens.face, "testFamily", true, &[]);

    // Check total count of all GeomSubsets on the sphere
    let all_geom_subsets = Subset::get_all_geom_subsets(&Imageable::new(sphere));
    assert_eq!(all_geom_subsets.len(), 68);

    // Check that invalid negative indices are ignored when getting unassigned indices
    let invalid_indices: Vec<i32> = vec![-3, -2, 0, 1, 2];
    let invalid_subset = Subset::create_unique_geom_subset(
        &geom,
        &Token::new("testSubset"),
        &tokens.face,
        &invalid_indices,
        &Token::new("testInvalid"),
        &tokens.partition,
    );
    invalid_subset
        .get_indices_attr()
        .set(invalid_indices, TimeCode::default_time());
    assert!(invalid_subset.is_valid());

    let unassigned = Subset::get_unassigned_indices_for_family(
        &geom,
        &tokens.face,
        &Token::new("testInvalid"),
        TimeCode::default_time(),
    );
    assert_eq!(unassigned, (3..16).collect::<Vec<i32>>());
}

// ============================================================================
// test_PointInstancer
// ============================================================================

/// Tests gathering of prim's geom subsets when prim's parent tree includes
/// a not-defined parent prim (e.g. PointInstancer with Prototype prim using
/// specifier "over").
///
/// Ported from `test_PointInstancer`.
#[test]
fn test_point_instancer() {
    let stage = open_stage("PointInstancer.usda");
    let sphere = stage
        .get_prim_at_path(
            &usd_sdf::Path::from_string("/Sphere/PointInstancers/Prototypes/pSphere1")
                .expect("path"),
        )
        .expect("prim");
    let geom = Imageable::new(sphere);
    assert!(geom.is_valid());

    let tokens = usd_geom_tokens();

    let material_bind_subsets =
        Subset::get_geom_subsets(&geom, &tokens.face, &Token::new("materialBind"));
    assert_eq!(material_bind_subsets.len(), 3);

    assert_eq!(
        tokens.partition,
        Subset::get_family_type(&geom, &Token::new("materialBind"))
    );

    validate_family(&geom, &tokens.face, "materialBind", true, &[]);
}
