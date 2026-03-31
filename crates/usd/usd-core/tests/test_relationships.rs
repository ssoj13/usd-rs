//! Tests for UsdRelationship.
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdRelationships.py

mod common;

use std::collections::HashSet;
use usd_core::Stage;
use usd_core::common::InitialLoadSet;
use usd_sdf::{Layer, Path};
use usd_tf::Token;

// ============================================================================
// Helpers
// ============================================================================

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap_or_else(|| panic!("invalid path: {s}"))
}

fn paths(strs: &[&str]) -> Vec<Path> {
    strs.iter().map(|s| p(s)).collect()
}

fn path_set(strs: &[&str]) -> HashSet<String> {
    strs.iter().map(|s| s.to_string()).collect()
}

fn to_set(paths: &[Path]) -> HashSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}

// ============================================================================
// Stage creation helper (matches Python _CreateStage)
// ============================================================================

fn create_stage() -> std::sync::Arc<Stage> {
    let layer = Layer::create_anonymous(Some("test_relationships.usda"));
    let ok = layer.import_from_string(
        r#"#usda 1.0
        def Scope "Foo"
        {
            custom int someAttr
            add rel testRel = [
                </Qux>,
                </Bar>,
                </Baz>,
                </Foo.someAttr>,
            ]
            add rel testRelBug138452 = </Bug138452>
        }

        def Scope "Bar"
        {
            add rel cycle = </Bar.fwd>
            add rel fwd = [
                </Baz>,
                </Foo.testRel>,
                </Qux>,
                </Bar.cycle>,
            ]
            add rel fwd2 = [
                </Bar.fwd2a>,
                </Bar.fwd2b>,
                </Bar.fwd2c>,
            ]
            add rel fwd2a = </Qux>
            add rel fwd2b = </Baz>
            add rel fwd2c = </Bar>
        }

        def Scope "Baz"
        {
            add rel bogus = </MissingTargetPath>
        }

        def Scope "Qux"
        {
        }

        def Scope "Bug138452"
        {
            custom rel Bug138452
            add rel Bug138452 = </Qux>
        }

        def "Recursive" {
            def "A" { custom rel AtoB = <../B>
            }
            def "B" { custom rel BtoC = <../C>
            }
            def "C" { custom rel CtoD = <../D>
            }
            def "D" { custom rel DtoA = <../A>
                def "A" { custom rel AtoB = <../B>
                }
                def "B" { custom rel BtoC = <../C>
                }
                def "C" { custom rel CtoD = <../D>
                }
                def "D" { custom rel DtoA = <../A>
                }
            }
            over "E" { custom rel EtoF = <../F>
            }
            over "F" { custom rel FtoE = <../E>
            }
        }
        "#,
    );
    assert!(ok, "import_from_string failed");
    Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage")
}

// ============================================================================
// test_Targets — from Python test_Targets
// ============================================================================

#[test]
fn rels_targets() {
    common::setup();
    let stage = create_stage();

    // Simple target list with correct order
    let foo = stage.get_prim_at_path(&p("/Foo")).expect("/Foo");
    let r = foo.get_relationship("testRel").expect("testRel");
    assert_eq!(
        r.get_targets(),
        paths(&["/Qux", "/Bar", "/Baz", "/Foo.someAttr"])
    );

    // Forwarded targets
    let bar = stage.get_prim_at_path(&p("/Bar")).expect("/Bar");
    let r = bar.get_relationship("fwd").expect("fwd");
    assert_eq!(
        r.get_forwarded_targets(),
        paths(&["/Baz", "/Qux", "/Bar", "/Foo.someAttr"])
    );

    // Forwarded targets via fwd2
    let r = bar.get_relationship("fwd2").expect("fwd2");
    assert_eq!(r.get_forwarded_targets(), paths(&["/Qux", "/Baz", "/Bar"]));

    // Forwarded targets, bug 138452
    let r = foo
        .get_relationship("testRelBug138452")
        .expect("testRelBug138452");
    assert_eq!(r.get_forwarded_targets(), paths(&["/Bug138452"]));

    // Cycle detection
    let r = bar.get_relationship("cycle").expect("cycle");
    assert_eq!(
        r.get_forwarded_targets(),
        paths(&["/Baz", "/Qux", "/Bar", "/Foo.someAttr"])
    );

    // Bogus target path
    let baz = stage.get_prim_at_path(&p("/Baz")).expect("/Baz");
    let r = baz.get_relationship("bogus").expect("bogus");
    assert_eq!(r.get_forwarded_targets(), paths(&["/MissingTargetPath"]));

    // Recursive finding
    let recursive = stage
        .get_prim_at_path(&p("/Recursive"))
        .expect("/Recursive");
    let all = recursive.find_all_relationship_target_paths(None);
    assert_eq!(
        to_set(&all),
        path_set(&[
            "/Recursive/A",
            "/Recursive/B",
            "/Recursive/C",
            "/Recursive/D",
            "/Recursive/D/A",
            "/Recursive/D/B",
            "/Recursive/D/C",
            "/Recursive/D/D",
        ])
    );

    // Recursive finding from /Recursive/A
    let recursive_a = stage
        .get_prim_at_path(&p("/Recursive/A"))
        .expect("/Recursive/A");
    let all_a = recursive_a.find_all_relationship_target_paths(None);
    assert_eq!(to_set(&all_a), path_set(&["/Recursive/B"]));
}

// ============================================================================
// test_TargetsInInstances — from Python test_TargetsInInstances
// ============================================================================

#[test]
fn rels_targets_in_instances() {
    common::setup();
    let layer = Layer::create_anonymous(Some("rels_targets_in_instances.usda"));
    let ok = layer.import_from_string(
        r#"#usda 1.0
        def Scope "Ref"
        {
            def Scope "Foo"
            {
                custom int someAttr
                add rel testRel = [
                    </Ref/Qux>,
                    </Ref/Bar>,
                    </Ref/Baz>,
                    </Ref/Foo.someAttr>,
                ]
            }

            def Scope "Bar"
            {
                add rel cycle = </Ref/Bar.fwd>
                add rel fwd = [
                    </Ref/Baz>,
                    </Ref/Foo.testRel>,
                    </Ref/Qux>,
                    </Ref/Bar.cycle>,
                ]
                add rel fwd2 = [
                    </Ref/Bar.fwd2a>,
                    </Ref/Bar.fwd2b>,
                    </Ref/Bar.fwd2c>,
                ]
                add rel fwd2a = </Ref/Qux>
                add rel fwd2b = </Ref/Baz>
                add rel fwd2c = </Ref/Bar>
            }

            def Scope "Baz"
            {
                add rel bogus = </Ref/MissingTargetPath>
                add rel root = </Ref>
            }

            def Scope "Qux"
            {
            }
        }

        def Scope "Root" (
            instanceable = true
            references = </Ref>
        )
        {
        }
        "#,
    );
    assert!(ok, "import_from_string failed");
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");

    let root = stage.get_prim_at_path(&p("/Root")).expect("/Root");
    let prototype = root.get_prototype();
    assert!(prototype.is_valid(), "expected prototype");

    let proto_path = prototype.path().clone();

    // Simple target list with correct order
    let foo = prototype.get_child(&Token::new("Foo"));
    assert!(foo.is_valid(), "Foo child");
    let r = foo.get_relationship("testRel").expect("testRel");
    let expected = vec![
        proto_path.append_child("Qux").unwrap(),
        proto_path.append_child("Bar").unwrap(),
        proto_path.append_child("Baz").unwrap(),
        proto_path.append_path(&p("Foo.someAttr")).unwrap(),
    ];
    assert_eq!(r.get_targets(), expected);

    // Forwarded targets
    let bar = prototype.get_child(&Token::new("Bar"));
    assert!(bar.is_valid(), "Bar child");
    let r = bar.get_relationship("fwd").expect("fwd");
    let expected = vec![
        proto_path.append_child("Baz").unwrap(),
        proto_path.append_child("Qux").unwrap(),
        proto_path.append_child("Bar").unwrap(),
        proto_path.append_path(&p("Foo.someAttr")).unwrap(),
    ];
    assert_eq!(r.get_forwarded_targets(), expected);

    // Forwarded targets via fwd2
    let r = bar.get_relationship("fwd2").expect("fwd2");
    let expected = vec![
        proto_path.append_child("Qux").unwrap(),
        proto_path.append_child("Baz").unwrap(),
        proto_path.append_child("Bar").unwrap(),
    ];
    assert_eq!(r.get_forwarded_targets(), expected);

    // Cycle detection
    let r = bar.get_relationship("cycle").expect("cycle");
    let expected = vec![
        proto_path.append_child("Baz").unwrap(),
        proto_path.append_child("Qux").unwrap(),
        proto_path.append_child("Bar").unwrap(),
        proto_path.append_path(&p("Foo.someAttr")).unwrap(),
    ];
    assert_eq!(r.get_forwarded_targets(), expected);

    // Bogus target path
    let baz = prototype.get_child(&Token::new("Baz"));
    assert!(baz.is_valid(), "Baz child");
    let r = baz.get_relationship("bogus").expect("bogus");
    let expected = vec![proto_path.append_child("MissingTargetPath").unwrap()];
    assert_eq!(r.get_targets(), expected);
    assert_eq!(r.get_forwarded_targets(), expected);

    // Path inside an instance that points to the instance root
    let r = baz.get_relationship("root").expect("root");
    let expected = vec![proto_path.clone()];
    assert_eq!(r.get_targets(), expected);
    assert_eq!(r.get_forwarded_targets(), expected);
}

// ============================================================================
// test_TargetsToObjectsInInstances — from Python test_TargetsToObjectsInInstances
// ============================================================================

#[test]
fn rels_targets_to_instance_objects() {
    common::setup();
    let layer = Layer::create_anonymous(Some("rels_targets_to_instance_objects.usda"));
    let ok = layer.import_from_string(
        r#"#usda 1.0
        def "Instance"
        {
            double attr = 1.0

            def "A"
            {
                double attr = 1.0
                rel rel = [
                    </Instance>,
                    </Instance.attr>,
                    </Instance/A>,
                    </Instance/A.attr>,
                    </Instance/NestedInstance_1>,
                    </Instance/NestedInstance_1.attr>,
                    </Instance/NestedInstance_1/B>,
                    </Instance/NestedInstance_1/B.attr>,
                    </Instance/NestedInstance_2>,
                    </Instance/NestedInstance_2.attr>,
                    </Instance/NestedInstance_2/B>,
                    </Instance/NestedInstance_2/B.attr>
                ]
            }

            def "NestedInstance_1" (
                instanceable = true
                references = </NestedInstance>
            )
            {
            }

            def "NestedInstance_2" (
                instanceable = true
                references = </NestedInstance>
            )
            {
            }
        }

        def "NestedInstance"
        {
            double attr = 1.0
            def "B"
            {
                double attr = 1.0
            }
        }

        def "Root"
        {
            rel fwdRel = [
                </Root/Instance_1/A.rel>,
                </Root/Instance_2/A.rel>
            ]

            rel rel = [
                </Root/Instance_1>,
                </Root/Instance_1.attr>,
                </Root/Instance_1/A>,
                </Root/Instance_1/A.attr>,
                </Root/Instance_1/NestedInstance_1>,
                </Root/Instance_1/NestedInstance_1.attr>,
                </Root/Instance_1/NestedInstance_1/B>,
                </Root/Instance_1/NestedInstance_1/B.attr>,
                </Root/Instance_1/NestedInstance_2>,
                </Root/Instance_1/NestedInstance_2.attr>,
                </Root/Instance_1/NestedInstance_2/B>,
                </Root/Instance_1/NestedInstance_2/B.attr>,
                </Root/Instance_2>,
                </Root/Instance_2.attr>,
                </Root/Instance_2/A>,
                </Root/Instance_2/A.attr>,
                </Root/Instance_2/NestedInstance_1>,
                </Root/Instance_2/NestedInstance_1.attr>,
                </Root/Instance_2/NestedInstance_1/B>,
                </Root/Instance_2/NestedInstance_1/B.attr>,
                </Root/Instance_2/NestedInstance_2>,
                </Root/Instance_2/NestedInstance_2.attr>,
                </Root/Instance_2/NestedInstance_2/B>,
                </Root/Instance_2/NestedInstance_2/B.attr>
            ]

            def "Instance_1" (
                instanceable = true
                references = </Instance>
            )
            {
                rel fwdRel = [
                    </Root/Instance_1/A.rel>,
                    </Root/Instance_2/A.rel>
                ]

                rel rel = [
                    </Root/Instance_1>,
                    </Root/Instance_1.attr>,
                    </Root/Instance_1/A>,
                    </Root/Instance_1/A.attr>,
                    </Root/Instance_1/NestedInstance_1>,
                    </Root/Instance_1/NestedInstance_1.attr>,
                    </Root/Instance_1/NestedInstance_1/B>,
                    </Root/Instance_1/NestedInstance_1/B.attr>,
                    </Root/Instance_1/NestedInstance_2>,
                    </Root/Instance_1/NestedInstance_2.attr>,
                    </Root/Instance_1/NestedInstance_2/B>,
                    </Root/Instance_1/NestedInstance_2/B.attr>,
                    </Root/Instance_2>,
                    </Root/Instance_2.attr>,
                    </Root/Instance_2/A>,
                    </Root/Instance_2/A.attr>,
                    </Root/Instance_2/NestedInstance_1>,
                    </Root/Instance_2/NestedInstance_1.attr>,
                    </Root/Instance_2/NestedInstance_1/B>,
                    </Root/Instance_2/NestedInstance_1/B.attr>,
                    </Root/Instance_2/NestedInstance_2>,
                    </Root/Instance_2/NestedInstance_2.attr>,
                    </Root/Instance_2/NestedInstance_2/B>,
                    </Root/Instance_2/NestedInstance_2/B.attr>
                ]
            }

            def "Instance_2" (
                instanceable = true
                references = </Instance>
            )
            {
            }
        }
        "#,
    );
    assert!(ok, "import_from_string failed");
    let stage = Stage::open_with_root_layer(layer, InitialLoadSet::LoadAll).expect("open stage");

    let expected_targets = paths(&[
        "/Root/Instance_1",
        "/Root/Instance_1.attr",
        "/Root/Instance_1/A",
        "/Root/Instance_1/A.attr",
        "/Root/Instance_1/NestedInstance_1",
        "/Root/Instance_1/NestedInstance_1.attr",
        "/Root/Instance_1/NestedInstance_1/B",
        "/Root/Instance_1/NestedInstance_1/B.attr",
        "/Root/Instance_1/NestedInstance_2",
        "/Root/Instance_1/NestedInstance_2.attr",
        "/Root/Instance_1/NestedInstance_2/B",
        "/Root/Instance_1/NestedInstance_2/B.attr",
        "/Root/Instance_2",
        "/Root/Instance_2.attr",
        "/Root/Instance_2/A",
        "/Root/Instance_2/A.attr",
        "/Root/Instance_2/NestedInstance_1",
        "/Root/Instance_2/NestedInstance_1.attr",
        "/Root/Instance_2/NestedInstance_1/B",
        "/Root/Instance_2/NestedInstance_1/B.attr",
        "/Root/Instance_2/NestedInstance_2",
        "/Root/Instance_2/NestedInstance_2.attr",
        "/Root/Instance_2/NestedInstance_2/B",
        "/Root/Instance_2/NestedInstance_2/B.attr",
    ]);

    // Test /Root.rel
    let root = stage.get_prim_at_path(&p("/Root")).expect("/Root");
    let rel = root.get_relationship("rel").expect("rel");
    assert_eq!(rel.get_targets(), expected_targets);

    // Test /Root/Instance_1.rel (same expected)
    let inst1 = stage
        .get_prim_at_path(&p("/Root/Instance_1"))
        .expect("/Root/Instance_1");
    let rel = inst1.get_relationship("rel").expect("rel");
    assert_eq!(rel.get_targets(), expected_targets);

    // Test relationship in prototype
    let prototype = inst1.get_prototype();
    assert!(prototype.is_valid());
    let proto_path = prototype.path().clone();
    let proto_a = prototype.get_child(&Token::new("A"));
    assert!(proto_a.is_valid(), "A in prototype");
    let rel_in_proto = proto_a.get_relationship("rel").expect("rel in prototype");

    let expected_proto_targets: Vec<Path> = [
        "",
        ".attr",
        "A",
        "A.attr",
        "NestedInstance_1",
        "NestedInstance_1.attr",
        "NestedInstance_1/B",
        "NestedInstance_1/B.attr",
        "NestedInstance_2",
        "NestedInstance_2.attr",
        "NestedInstance_2/B",
        "NestedInstance_2/B.attr",
    ]
    .iter()
    .map(|suffix| {
        if suffix.is_empty() {
            proto_path.clone()
        } else if suffix.contains('.') || suffix.contains('/') {
            proto_path
                .append_path(&Path::from_string(suffix).unwrap())
                .unwrap()
        } else {
            proto_path.append_child(suffix).unwrap()
        }
    })
    .collect();
    assert_eq!(rel_in_proto.get_targets(), expected_proto_targets);

    // Test forwarding: /Root.fwdRel
    let fwd_rel = root.get_relationship("fwdRel").expect("fwdRel");
    assert_eq!(fwd_rel.get_forwarded_targets(), expected_targets);

    // Test forwarding: /Root/Instance_1.fwdRel
    let fwd_rel = inst1.get_relationship("fwdRel").expect("fwdRel on inst1");
    assert_eq!(fwd_rel.get_forwarded_targets(), expected_targets);
}

// ============================================================================
// test_AuthoringTargets — from Python test_AuthoringTargets
// ============================================================================

#[test]
fn rels_authoring_targets() {
    common::setup();
    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    let prim = stage.define_prim("/Test", "").expect("define /Test");
    let _ = stage.define_prim("/Test/A", "").expect("define /Test/A");
    let _ = stage.define_prim("/Test/B", "").expect("define /Test/B");
    let _ = stage.define_prim("/Test/C", "").expect("define /Test/C");
    let _ = stage.define_prim("/Test/D", "").expect("define /Test/D");

    let rel = prim.create_relationship("rel", true).expect("create rel");

    // SetTargets
    rel.set_targets(&paths(&["/Test/A", "/Test/B"]));
    assert_eq!(rel.get_targets(), paths(&["/Test/A", "/Test/B"]));

    // AddTarget appends to explicit list
    rel.add_target(&p("/Test/C"));
    assert_eq!(rel.get_targets(), paths(&["/Test/A", "/Test/B", "/Test/C"]));

    // ClearTargets (keep spec)
    rel.clear_targets_with_spec(false);
    assert!(rel.get_targets().is_empty());

    // AddTarget with front-of-prepend position
    rel.add_target_with_position(
        &p("/Test/A"),
        usd_core::common::ListPosition::FrontOfPrependList,
    );
    assert_eq!(rel.get_targets(), paths(&["/Test/A"]));

    // AddTarget with back-of-prepend position
    rel.add_target_with_position(
        &p("/Test/B"),
        usd_core::common::ListPosition::BackOfPrependList,
    );
    assert_eq!(rel.get_targets(), paths(&["/Test/A", "/Test/B"]));

    // AddTarget with front-of-append position
    rel.add_target_with_position(
        &p("/Test/C"),
        usd_core::common::ListPosition::FrontOfAppendList,
    );
    assert_eq!(rel.get_targets(), paths(&["/Test/A", "/Test/B", "/Test/C"]));

    // AddTarget with back-of-append position
    rel.add_target_with_position(
        &p("/Test/D"),
        usd_core::common::ListPosition::BackOfAppendList,
    );
    assert_eq!(
        rel.get_targets(),
        paths(&["/Test/A", "/Test/B", "/Test/C", "/Test/D"])
    );
}
