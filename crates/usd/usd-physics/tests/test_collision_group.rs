/// Port of testUsdPhysicsCollisionGroupAPI.py
use std::sync::Arc;

use usd_core::{InitialLoadSet, Stage};
use usd_physics::CollisionGroup;
use usd_sdf::Path;

fn new_stage() -> Arc<Stage> {
    usd_sdf::init();
    Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create_in_memory")
}

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap()
}

/// Validate symmetry: IsCollisionEnabled(a,b) == IsCollisionEnabled(b,a)
/// and index-based matches path-based.
fn validate_table_symmetry(table: &usd_physics::CollisionGroupTable) {
    let groups = table.get_collision_groups();
    for (idx_a, a) in groups.iter().enumerate() {
        for (idx_b, b) in groups.iter().enumerate() {
            assert_eq!(
                table.is_collision_enabled_by_index(idx_a, idx_b),
                table.is_collision_enabled_by_index(idx_b, idx_a),
                "Index symmetry failed for ({}, {})",
                idx_a,
                idx_b
            );
            assert_eq!(
                table.is_collision_enabled(a, b),
                table.is_collision_enabled(b, a),
                "Path symmetry failed for ({}, {})",
                a,
                b
            );
            assert_eq!(
                table.is_collision_enabled(a, b),
                table.is_collision_enabled_by_index(idx_a, idx_b),
                "Path/index mismatch for ({}, {})",
                a,
                b
            );
        }
    }
}

/// Port of test_collision_group_table
#[test]
fn test_collision_group_table() {
    let stage = new_stage();

    let a = CollisionGroup::define(&stage, &p("/a")).unwrap();
    let b = CollisionGroup::define(&stage, &p("/b")).unwrap();
    let c = CollisionGroup::define(&stage, &p("/c")).unwrap();

    b.create_filtered_groups_rel()
        .unwrap()
        .add_target(&c.get_prim().get_path());
    c.create_filtered_groups_rel()
        .unwrap()
        .add_target(&c.get_prim().get_path());

    let table = CollisionGroup::compute_collision_group_table(&stage);

    // All 3 groups present
    assert_eq!(table.get_collision_groups().len(), 3);
    assert!(
        table
            .get_collision_groups()
            .contains(&a.get_prim().get_path())
    );
    assert!(
        table
            .get_collision_groups()
            .contains(&b.get_prim().get_path())
    );
    assert!(
        table
            .get_collision_groups()
            .contains(&c.get_prim().get_path())
    );

    let pa = a.get_prim().get_path();
    let pb = b.get_prim().get_path();
    let pc = c.get_prim().get_path();

    // A collides with everything
    // B collides with A and B only
    // C collides with A only
    assert!(table.is_collision_enabled(&pa, &pa));
    assert!(table.is_collision_enabled(&pa, &pb));
    assert!(table.is_collision_enabled(&pa, &pc));
    assert!(table.is_collision_enabled(&pb, &pb));
    assert!(!table.is_collision_enabled(&pb, &pc));
    assert!(!table.is_collision_enabled(&pc, &pc));

    validate_table_symmetry(&table);
}

/// Port of test_collision_group_inversion
#[test]
fn test_collision_group_inversion() {
    let stage = new_stage();

    let a = CollisionGroup::define(&stage, &p("/a")).unwrap();
    let b = CollisionGroup::define(&stage, &p("/b")).unwrap();
    let c = CollisionGroup::define(&stage, &p("/c")).unwrap();

    a.create_filtered_groups_rel()
        .unwrap()
        .add_target(&c.get_prim().get_path());
    a.create_invert_filtered_groups_attr(Some(true));

    let table = CollisionGroup::compute_collision_group_table(&stage);

    let pa = a.get_prim().get_path();
    let pb = b.get_prim().get_path();
    let pc = c.get_prim().get_path();

    // A collides with only C
    // B collides with B and C
    // C collides with A, B, C
    assert!(!table.is_collision_enabled(&pa, &pa));
    assert!(!table.is_collision_enabled(&pa, &pb));
    assert!(table.is_collision_enabled(&pa, &pc));
    assert!(table.is_collision_enabled(&pb, &pb));
    assert!(table.is_collision_enabled(&pb, &pc));
    assert!(table.is_collision_enabled(&pc, &pc));

    validate_table_symmetry(&table);

    // Extended inversion scenario: merge groups + inversion interaction
    let all_others = CollisionGroup::define(&stage, &p("/allOthers")).unwrap();

    let grp_x_collider = CollisionGroup::define(&stage, &p("/grpXCollider")).unwrap();
    let grp_x = CollisionGroup::define(&stage, &p("/grpX")).unwrap();
    grp_x
        .create_filtered_groups_rel()
        .unwrap()
        .add_target(&grp_x_collider.get_prim().get_path());
    grp_x.create_invert_filtered_groups_attr(Some(true));

    let table = CollisionGroup::compute_collision_group_table(&stage);
    assert!(table.is_collision_enabled(
        &grp_x.get_prim().get_path(),
        &grp_x_collider.get_prim().get_path()
    ));
    assert!(!table.is_collision_enabled(
        &grp_x.get_prim().get_path(),
        &all_others.get_prim().get_path()
    ));

    // Add grpX to merge group "mergeTest"
    grp_x.create_merge_group_name_attr(Some("mergeTest".to_string()));

    // grpA filters grpXCollider
    let grp_a = CollisionGroup::define(&stage, &p("/grpA")).unwrap();
    grp_a
        .create_filtered_groups_rel()
        .unwrap()
        .add_target(&grp_x_collider.get_prim().get_path());

    let table = CollisionGroup::compute_collision_group_table(&stage);
    assert!(!table.is_collision_enabled(
        &grp_a.get_prim().get_path(),
        &grp_x_collider.get_prim().get_path()
    ));
    // grpX's collision pairs unchanged
    assert!(table.is_collision_enabled(
        &grp_x.get_prim().get_path(),
        &grp_x_collider.get_prim().get_path()
    ));
    assert!(!table.is_collision_enabled(
        &grp_x.get_prim().get_path(),
        &all_others.get_prim().get_path()
    ));

    // grpA joins same merge group -> disables all
    grp_a.create_merge_group_name_attr(Some("mergeTest".to_string()));
    let table = CollisionGroup::compute_collision_group_table(&stage);
    assert!(!table.is_collision_enabled(
        &grp_x.get_prim().get_path(),
        &grp_x_collider.get_prim().get_path()
    ));
    assert!(!table.is_collision_enabled(
        &grp_x.get_prim().get_path(),
        &all_others.get_prim().get_path()
    ));
    assert!(!table.is_collision_enabled(
        &grp_a.get_prim().get_path(),
        &grp_x_collider.get_prim().get_path()
    ));
    assert!(!table.is_collision_enabled(
        &grp_a.get_prim().get_path(),
        &all_others.get_prim().get_path()
    ));
}

/// Port of test_collision_group_simple_merging
#[test]
fn test_collision_group_simple_merging() {
    let stage = new_stage();

    let a = CollisionGroup::define(&stage, &p("/a")).unwrap();
    let b = CollisionGroup::define(&stage, &p("/b")).unwrap();
    let c = CollisionGroup::define(&stage, &p("/c")).unwrap();

    a.create_filtered_groups_rel()
        .unwrap()
        .add_target(&c.get_prim().get_path());
    // A and B in same merge group
    a.create_merge_group_name_attr(Some("mergeTest".to_string()));
    b.create_merge_group_name_attr(Some("mergeTest".to_string()));

    let table = CollisionGroup::compute_collision_group_table(&stage);

    let pa = a.get_prim().get_path();
    let pb = b.get_prim().get_path();
    let pc = c.get_prim().get_path();

    // A collides with A and B only
    // B collides with A and B only
    // C collides with C only
    assert!(table.is_collision_enabled(&pa, &pa));
    assert!(table.is_collision_enabled(&pa, &pb));
    assert!(!table.is_collision_enabled(&pa, &pc));
    assert!(table.is_collision_enabled(&pb, &pb));
    assert!(!table.is_collision_enabled(&pb, &pc));
    assert!(table.is_collision_enabled(&pc, &pc));

    validate_table_symmetry(&table);
}

/// Port of test_collision_group_complex_merging
#[test]
fn test_collision_group_complex_merging() {
    let stage = new_stage();

    let a = CollisionGroup::define(&stage, &p("/a")).unwrap();
    let b = CollisionGroup::define(&stage, &p("/b")).unwrap();
    let c = CollisionGroup::define(&stage, &p("/c")).unwrap();
    let d = CollisionGroup::define(&stage, &p("/d")).unwrap();

    a.create_filtered_groups_rel()
        .unwrap()
        .add_target(&c.get_prim().get_path());
    // A,B in mergeAB; C,D in mergeCD
    a.create_merge_group_name_attr(Some("mergeAB".to_string()));
    b.create_merge_group_name_attr(Some("mergeAB".to_string()));
    c.create_merge_group_name_attr(Some("mergeCD".to_string()));
    d.create_merge_group_name_attr(Some("mergeCD".to_string()));

    let table = CollisionGroup::compute_collision_group_table(&stage);

    let pa = a.get_prim().get_path();
    let pb = b.get_prim().get_path();
    let pc = c.get_prim().get_path();
    let pd = d.get_prim().get_path();

    // A collides with A and B only
    assert!(table.is_collision_enabled(&pa, &pa));
    assert!(table.is_collision_enabled(&pa, &pb));
    assert!(!table.is_collision_enabled(&pa, &pc));
    assert!(!table.is_collision_enabled(&pa, &pd));

    // B collides with A and B only
    assert!(table.is_collision_enabled(&pb, &pb));
    assert!(!table.is_collision_enabled(&pb, &pc));
    assert!(!table.is_collision_enabled(&pb, &pd));

    // C collides with C and D
    assert!(table.is_collision_enabled(&pc, &pc));
    assert!(table.is_collision_enabled(&pc, &pd));

    // D collides with D
    assert!(table.is_collision_enabled(&pd, &pd));

    validate_table_symmetry(&table);
}
