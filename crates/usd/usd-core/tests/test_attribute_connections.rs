//! Tests for attribute connections.
//!
//! Ported from pxr/usd/usd/testenv/testUsdAttributeConnections.py

mod common;

use std::collections::HashSet;
use usd_core::common::ListPosition;
use usd_sdf::Path;

fn create_stage() -> std::sync::Arc<usd_core::stage::Stage> {
    common::setup();

    // Import USDA into an anonymous layer, then open a stage from it.
    // This ensures proper composition (import_from_string on an existing
    // stage does not recompose prim_data).
    let layer = usd_sdf::Layer::create_anonymous(Some("connections_test.usda"));
    layer.import_from_string(
        r#"#usda 1.0
        def Scope "Foo"
        {
            custom int someAttr
            add int testAttr.connect = [
                </Qux>,
                </Bar>,
                </Baz>,
                </Foo.someAttr>,
            ]
        }

        def Scope "Baz"
        {
            add int bogus.connect = </MissingConnectionPath>
        }

        def "Recursive" {
            def "A" { add int AtoB.connect = <../B>
            }
            def "B" { add int BtoC.connect = <../C>
            }
            def "C" { add int CtoD.connect = <../D>
            }
            def "D" { add int DtoA.connect = <../A>
                def "A" { add int AtoB.connect = <../B>
                }
                def "B" { add int BtoC.connect = <../C>
                }
                def "C" { add int CtoD.connect = <../D>
                }
                def "D" { add int DtoA.connect = <../A>
                }
            }
            over "E" { add int EtoF.connect = <../F>
            }
            over "F" { add int FtoE.connect = <../E>
            }
        }
        "#,
    );

    usd_core::stage::Stage::open_with_root_layer(
        layer,
        usd_core::common::InitialLoadSet::LoadAll,
    )
    .expect("open stage from layer")
}

/// test_Connections: simple connect list with correct order.
#[test]
fn attr_connections_basic_list() {
    let stage = create_stage();

    let foo = stage.get_prim_at_path(&Path::from("/Foo")).expect("Foo");
    let attr = foo.get_attribute("testAttr").expect("testAttr");
    let connections = attr.get_connections();

    let expected = vec![
        Path::from("/Qux"),
        Path::from("/Bar"),
        Path::from("/Baz"),
        Path::from("/Foo.someAttr"),
    ];
    assert_eq!(connections, expected);
}

/// test_Connections: recursive finding on /Recursive (default predicate).
#[test]
fn attr_connections_recursive_find() {
    let stage = create_stage();

    let recursive = stage
        .get_prim_at_path(&Path::from("/Recursive"))
        .expect("Recursive");
    let paths = recursive.find_all_attribute_connection_paths(None);
    let path_set: HashSet<Path> = paths.into_iter().collect();

    let expected: HashSet<Path> = [
        "/Recursive/A",
        "/Recursive/B",
        "/Recursive/C",
        "/Recursive/D",
        "/Recursive/D/A",
        "/Recursive/D/B",
        "/Recursive/D/C",
        "/Recursive/D/D",
    ]
    .iter()
    .map(|s| Path::from(*s))
    .collect();

    assert_eq!(path_set, expected);
}

/// test_Connections: recursive finding on /Recursive/A (single prim).
#[test]
fn attr_connections_recursive_find_single() {
    let stage = create_stage();

    let recursive_a = stage
        .get_prim_at_path(&Path::from("/Recursive/A"))
        .expect("/Recursive/A");
    let paths = recursive_a.find_all_attribute_connection_paths(None);
    let path_set: HashSet<Path> = paths.into_iter().collect();

    let expected: HashSet<Path> = ["/Recursive/B"].iter().map(|s| Path::from(*s)).collect();

    assert_eq!(path_set, expected);
}

/// test_Connections: recursive find with AllPrimsPredicate (includes over prims E/F).
#[test]
fn attr_connections_recursive_all_prims() {
    let stage = create_stage();

    let recursive = stage
        .get_prim_at_path(&Path::from("/Recursive"))
        .expect("Recursive");

    let all_prims_pred = usd_core::prim_flags::all_prims_predicate();
    let paths = recursive.find_all_attribute_connection_paths(Some(all_prims_pred));
    let path_set: HashSet<Path> = paths.into_iter().collect();

    let expected: HashSet<Path> = [
        "/Recursive/A",
        "/Recursive/B",
        "/Recursive/C",
        "/Recursive/D",
        "/Recursive/E",
        "/Recursive/F",
        "/Recursive/D/A",
        "/Recursive/D/B",
        "/Recursive/D/C",
        "/Recursive/D/D",
    ]
    .iter()
    .map(|s| Path::from(*s))
    .collect();

    assert_eq!(path_set, expected);
}

/// test_ConnectionsInInstances: connections within instance prototypes.
/// C++ ref: testUsdAttributeConnections.py line 140
#[test]
fn attr_connections_in_instances() {
    common::setup();

    let layer = usd_sdf::Layer::create_anonymous(Some("connections_instances.usda"));
    layer.import_from_string(
        r#"#usda 1.0
        def Scope "Ref"
        {
            def Scope "Foo"
            {
                custom int someAttr
                add int testAttr.connect = [
                    </Ref/Qux>,
                    </Ref/Bar>,
                    </Ref/Baz>,
                    </Ref/Foo.someAttr>,
                ]
            }

            def Scope "Baz"
            {
                add int bogus.connect = </Ref/MissingConnectionPath>
                add int root.connect = </Ref>
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

    let stage = usd_core::stage::Stage::open_with_root_layer(
        layer,
        usd_core::common::InitialLoadSet::LoadAll,
    )
    .expect("open stage");

    let root_prim = stage.get_prim_at_path(&Path::from("/Root")).expect("/Root");
    eprintln!("Root is_instance: {}", root_prim.is_instance());
    eprintln!("Root is_instanceable: {}", root_prim.is_instanceable());
    let prototype = root_prim.get_prototype();
    eprintln!("prototype path: {}, is_pseudo_root: {}", prototype.path(), prototype.is_pseudo_root());
    eprintln!("prototype children: {:?}", prototype.get_children().iter().map(|c| c.path().to_string()).collect::<Vec<_>>());
    assert!(!prototype.is_pseudo_root(), "prototype should exist");
    let proto_path = prototype.path().clone();

    // Simple source list with correct order
    let foo = prototype.get_child(&usd_tf::Token::new("Foo"));
    eprintln!("foo path: {}, is_valid: {}", foo.path(), !foo.is_pseudo_root());
    let attr = foo.get_attribute("testAttr").expect("testAttr");
    let expected = vec![
        proto_path.append_child("Qux").unwrap(),
        proto_path.append_child("Bar").unwrap(),
        proto_path.append_child("Baz").unwrap(),
        proto_path.append_path(&Path::from("Foo.someAttr")).unwrap(),
    ];
    assert_eq!(attr.get_connections(), expected);

    // Bogus source path
    let baz = prototype.get_child(&usd_tf::Token::new("Baz"));
    let bogus = baz.get_attribute("bogus").expect("bogus attr");
    let expected_bogus = vec![proto_path.append_child("MissingConnectionPath").unwrap()];
    assert_eq!(bogus.get_connections(), expected_bogus);

    // Path inside an instance that points to the instance root
    let root_attr = baz.get_attribute("root").expect("root attr");
    assert_eq!(root_attr.get_connections(), vec![proto_path.clone()]);
}

/// test_ConnectionsToObjectsInInstances: connections pointing into instances.
/// C++ ref: testUsdAttributeConnections.py line 194
#[test]
fn attr_connections_to_objects_in_instances() {
    common::setup();

    let layer = usd_sdf::Layer::create_anonymous(Some("connections_to_instances.usda"));
    layer.import_from_string(
        r#"#usda 1.0
        def "Instance"
        {
            double attr = 1.0

            def "A"
            {
                double attr = 1.0
                int cattr.connect = [
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
            int cattr.connect = [
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
                int cattr.connect = [
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

    let stage = usd_core::stage::Stage::open_with_root_layer(
        layer,
        usd_core::common::InitialLoadSet::LoadAll,
    )
    .expect("open stage");

    let expected_connections = vec![
        Path::from("/Root/Instance_1"),
        Path::from("/Root/Instance_1.attr"),
        Path::from("/Root/Instance_1/A"),
        Path::from("/Root/Instance_1/A.attr"),
        Path::from("/Root/Instance_1/NestedInstance_1"),
        Path::from("/Root/Instance_1/NestedInstance_1.attr"),
        Path::from("/Root/Instance_1/NestedInstance_1/B"),
        Path::from("/Root/Instance_1/NestedInstance_1/B.attr"),
        Path::from("/Root/Instance_1/NestedInstance_2"),
        Path::from("/Root/Instance_1/NestedInstance_2.attr"),
        Path::from("/Root/Instance_1/NestedInstance_2/B"),
        Path::from("/Root/Instance_1/NestedInstance_2/B.attr"),
        Path::from("/Root/Instance_2"),
        Path::from("/Root/Instance_2.attr"),
        Path::from("/Root/Instance_2/A"),
        Path::from("/Root/Instance_2/A.attr"),
        Path::from("/Root/Instance_2/NestedInstance_1"),
        Path::from("/Root/Instance_2/NestedInstance_1.attr"),
        Path::from("/Root/Instance_2/NestedInstance_1/B"),
        Path::from("/Root/Instance_2/NestedInstance_1/B.attr"),
        Path::from("/Root/Instance_2/NestedInstance_2"),
        Path::from("/Root/Instance_2/NestedInstance_2.attr"),
        Path::from("/Root/Instance_2/NestedInstance_2/B"),
        Path::from("/Root/Instance_2/NestedInstance_2/B.attr"),
    ];

    // Test from /Root
    let root_attr = stage
        .get_prim_at_path(&Path::from("/Root"))
        .expect("/Root")
        .get_attribute("cattr")
        .expect("cattr");
    assert_eq!(root_attr.get_connections(), expected_connections);

    // Test from /Root/Instance_1
    let inst1_attr = stage
        .get_prim_at_path(&Path::from("/Root/Instance_1"))
        .expect("/Root/Instance_1")
        .get_attribute("cattr")
        .expect("cattr");
    assert_eq!(inst1_attr.get_connections(), expected_connections);

    // Test connections in prototype
    let prototype = stage
        .get_prim_at_path(&Path::from("/Root/Instance_1"))
        .expect("/Root/Instance_1")
        .get_prototype();
    assert!(!prototype.is_pseudo_root(), "prototype should exist");
    let proto_path = prototype.path().clone();

    let proto_attr = prototype
        .get_child(&usd_tf::Token::new("A"))
        .get_attribute("cattr")
        .expect("cattr");
    let proto_expected = vec![
        proto_path.clone(),
        proto_path.append_property("attr").unwrap(),
        proto_path.append_child("A").unwrap(),
        proto_path.append_path(&Path::from("A.attr")).unwrap(),
        proto_path.append_child("NestedInstance_1").unwrap(),
        proto_path.append_path(&Path::from("NestedInstance_1.attr")).unwrap(),
        proto_path.append_path(&Path::from("NestedInstance_1/B")).unwrap(),
        proto_path.append_path(&Path::from("NestedInstance_1/B.attr")).unwrap(),
        proto_path.append_child("NestedInstance_2").unwrap(),
        proto_path.append_path(&Path::from("NestedInstance_2.attr")).unwrap(),
        proto_path.append_path(&Path::from("NestedInstance_2/B")).unwrap(),
        proto_path.append_path(&Path::from("NestedInstance_2/B.attr")).unwrap(),
    ];
    assert_eq!(proto_attr.get_connections(), proto_expected);
}

/// test_AuthoringConnections: SetConnections, AddConnection, ClearConnections
/// with various ListPositions.
#[test]
fn attr_connections_authoring() {
    common::setup();

    let stage = usd_core::stage::Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
        .expect("create stage");

    let prim = stage.define_prim("/Test", "").expect("define /Test");
    let attr = prim
        .create_attribute("attr", &common::vtn("int"), false, None)
        .expect("create attr");

    // SetConnections (explicit list)
    attr.set_connections(vec![Path::from("/Test.A"), Path::from("/Test.B")]);
    assert_eq!(
        attr.get_connections(),
        vec![Path::from("/Test.A"), Path::from("/Test.B")]
    );

    // AddConnection (default position: BackOfAppendList on explicit list)
    attr.add_connection(&Path::from("/Test.C"));
    assert_eq!(
        attr.get_connections(),
        vec![
            Path::from("/Test.A"),
            Path::from("/Test.B"),
            Path::from("/Test.C")
        ]
    );

    // ClearConnections
    attr.clear_connections();
    assert!(attr.get_connections().is_empty());

    // AddConnection with FrontOfPrependList
    attr.add_connection_with_position(&Path::from("/Test.A"), ListPosition::FrontOfPrependList);
    assert_eq!(attr.get_connections(), vec![Path::from("/Test.A")]);

    // AddConnection with BackOfPrependList
    attr.add_connection_with_position(&Path::from("/Test.B"), ListPosition::BackOfPrependList);
    assert_eq!(
        attr.get_connections(),
        vec![Path::from("/Test.A"), Path::from("/Test.B")]
    );

    // AddConnection with FrontOfAppendList
    attr.add_connection_with_position(&Path::from("/Test.C"), ListPosition::FrontOfAppendList);
    assert_eq!(
        attr.get_connections(),
        vec![
            Path::from("/Test.A"),
            Path::from("/Test.B"),
            Path::from("/Test.C")
        ]
    );

    // AddConnection with BackOfAppendList
    attr.add_connection_with_position(&Path::from("/Test.D"), ListPosition::BackOfAppendList);
    assert_eq!(
        attr.get_connections(),
        vec![
            Path::from("/Test.A"),
            Path::from("/Test.B"),
            Path::from("/Test.C"),
            Path::from("/Test.D")
        ]
    );
}

/// test_ConnectionsWithInconsistentSpecs: connections across references
/// with inconsistent spec types (double vs string).
#[test]
fn attr_connections_inconsistent_specs() {
    common::setup();

    let layer = usd_sdf::Layer::create_anonymous(Some("inconsistent_specs.usda"));
    layer.import_from_string(
        r#"#usda 1.0
                def "A"
                {
                    double a = 1.0
                    double attr = 1.0
                    prepend double attr.connect = </A.a>
                }

                def "B" (
                    references = </A>
                )
                {
                    string b = "foo"
                    uniform string attr = "foo"
                    prepend uniform string attr.connect = </B.b>
                }
            "#,
    );

    let stage = usd_core::stage::Stage::open_with_root_layer(
        layer,
        usd_core::common::InitialLoadSet::LoadAll,
    )
    .expect("open stage");

    let attr = stage
        .get_prim_at_path(&Path::from("/B"))
        .expect("/B")
        .get_attribute("attr")
        .expect("attr");
    let connections = attr.get_connections();
    assert_eq!(connections, vec![Path::from("/B.b"), Path::from("/B.a")]);
}
