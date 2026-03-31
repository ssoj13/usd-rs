use std::any::TypeId;
use std::collections::HashSet;
use usd_tf::TfType;

// ----- Type hierarchy used throughout these tests -----
// Mirrors the C++ class layout in type.cpp:
//
//  root
//   ├── ConcreteClass
//   ├── IAbstractClass
//   │    └── ChildClass (also derives ConcreteClass)
//   │         ├── GrandchildClass
//   │         └── OtherGrandchildClass
//   ├── CountedClass
//   └── SingleClass

struct ConcreteClass;
struct IAbstractClass;
struct ChildClass;
struct GrandchildClass;
struct OtherGrandchildClass;
struct SingleClass;
struct CountedClass;

/// Register the test hierarchy once.  Rust tests can run in any order so we
/// call this helper at the top of every test that needs the hierarchy.
fn register_hierarchy() {
    TfType::declare::<ConcreteClass>("ConcreteClass");
    TfType::declare::<IAbstractClass>("IAbstractClass");
    TfType::declare_with_bases::<ChildClass>(
        "ChildClass",
        &[
            TypeId::of::<ConcreteClass>(),
            TypeId::of::<IAbstractClass>(),
        ],
    );
    TfType::declare_with_bases::<GrandchildClass>("GrandchildClass", &[TypeId::of::<ChildClass>()]);
    TfType::declare_with_bases::<OtherGrandchildClass>(
        "OtherGrandchildClass",
        &[TypeId::of::<ChildClass>()],
    );
    TfType::declare::<CountedClass>("CountedClass");
    TfType::declare::<SingleClass>("SingleClass");
}

// Unknown type is_unknown, known types are not.
#[test]
fn is_unknown() {
    register_hierarchy();

    let t_unknown = TfType::unknown();
    let t_root = TfType::get_root();
    let t_concrete = TfType::find::<ConcreteClass>();
    let t_abstract = TfType::find::<IAbstractClass>();
    let t_child = TfType::find::<ChildClass>();
    let t_grandchild = TfType::find::<GrandchildClass>();
    let t_counted = TfType::find::<CountedClass>();
    let t_single = TfType::find::<SingleClass>();

    assert!(t_unknown.is_unknown());
    assert!(!t_root.is_unknown());
    assert!(!t_concrete.is_unknown());
    assert!(!t_abstract.is_unknown());
    assert!(!t_child.is_unknown());
    assert!(!t_grandchild.is_unknown());
    assert!(!t_counted.is_unknown());
    assert!(!t_single.is_unknown());
}

// All known types are distinct; adding the unknown type adds one more.
#[test]
fn all_types_distinct() {
    register_hierarchy();

    let t_unknown = TfType::unknown();
    let t_root = TfType::get_root();
    let t_concrete = TfType::find::<ConcreteClass>();
    let t_abstract = TfType::find::<IAbstractClass>();
    let t_child = TfType::find::<ChildClass>();
    let t_grandchild = TfType::find::<GrandchildClass>();
    let t_counted = TfType::find::<CountedClass>();
    let t_single = TfType::find::<SingleClass>();

    let mut known_set: HashSet<TfType> = HashSet::new();
    known_set.insert(t_root);
    known_set.insert(t_concrete);
    known_set.insert(t_abstract);
    known_set.insert(t_child);
    known_set.insert(t_grandchild);
    known_set.insert(t_counted);
    known_set.insert(t_single);

    // 7 distinct known types
    assert_eq!(known_set.len(), 7);

    let mut all_set = known_set.clone();
    all_set.insert(t_unknown);
    assert_eq!(all_set.len(), 8);

    // Each type appears exactly once
    assert_eq!(all_set.iter().filter(|&&t| t == t_unknown).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_root).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_concrete).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_abstract).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_child).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_grandchild).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_counted).count(), 1);
    assert_eq!(all_set.iter().filter(|&&t| t == t_single).count(), 1);
}

// All type names are distinct.
#[test]
fn all_type_names_distinct() {
    register_hierarchy();

    let types = [
        TfType::unknown(),
        TfType::get_root(),
        TfType::find::<ConcreteClass>(),
        TfType::find::<IAbstractClass>(),
        TfType::find::<ChildClass>(),
        TfType::find::<GrandchildClass>(),
        TfType::find::<CountedClass>(),
        TfType::find::<SingleClass>(),
    ];

    let names: HashSet<String> = types.iter().map(|t| t.type_name()).collect();
    assert_eq!(names.len(), types.len());
}

// IsA: unknown type always returns false.
#[test]
fn is_a_unknown_returns_false() {
    let t_unknown = TfType::unknown();
    // unknown.is_a(unknown) -> false
    assert!(!t_unknown.is_a(t_unknown));
}

// Every known type IsA root and IsA itself.
#[test]
fn every_known_type_is_a_root_and_self() {
    register_hierarchy();

    let t_root = TfType::get_root();
    let known = [
        TfType::find::<ConcreteClass>(),
        TfType::find::<IAbstractClass>(),
        TfType::find::<ChildClass>(),
        TfType::find::<GrandchildClass>(),
        TfType::find::<CountedClass>(),
        TfType::find::<SingleClass>(),
        t_root,
    ];

    for t in &known {
        assert!(t.is_a(t_root), "{} should be_a root", t.type_name());
        assert!(t.is_a(*t), "{} should be_a itself", t.type_name());
    }
}

// Known types are not IsA unknown.
#[test]
fn known_type_is_not_is_a_unknown() {
    register_hierarchy();

    let t_unknown = TfType::unknown();
    let known = [
        TfType::get_root(),
        TfType::find::<ConcreteClass>(),
        TfType::find::<IAbstractClass>(),
        TfType::find::<ChildClass>(),
        TfType::find::<GrandchildClass>(),
    ];

    for t in &known {
        assert!(
            !t.is_a(t_unknown),
            "{} should not be_a unknown",
            t.type_name()
        );
    }
}

// ChildClass inherits both ConcreteClass and IAbstractClass.
#[test]
fn child_is_a_both_parents() {
    register_hierarchy();

    let t_child = TfType::find::<ChildClass>();
    let t_concrete = TfType::find::<ConcreteClass>();
    let t_abstract = TfType::find::<IAbstractClass>();

    assert!(t_child.is_a(t_concrete));
    assert!(t_child.is_a(t_abstract));
    assert!(t_child.is_a_type::<ConcreteClass>());
    assert!(t_child.is_a_type::<IAbstractClass>());
}

// ConcreteClass is not IsA ChildClass (parent is not derived).
#[test]
fn concrete_not_is_a_child() {
    register_hierarchy();

    assert!(!TfType::find::<ConcreteClass>().is_a_type::<ChildClass>());
}

// IAbstractClass is not IsA ChildClass.
#[test]
fn abstract_not_is_a_child() {
    register_hierarchy();

    assert!(!TfType::find::<IAbstractClass>().is_a_type::<ChildClass>());
}

// GrandchildClass transitively inherits from all ancestors.
#[test]
fn grandchild_is_a_all_ancestors() {
    register_hierarchy();

    let t_gc = TfType::find::<GrandchildClass>();

    assert!(t_gc.is_a_type::<IAbstractClass>());
    assert!(t_gc.is_a_type::<ConcreteClass>());
    assert!(t_gc.is_a_type::<ChildClass>());
}

// find_by_name works for registered types.
#[test]
fn find_by_name() {
    register_hierarchy();

    assert_eq!(
        TfType::find::<IAbstractClass>(),
        TfType::find_by_name("IAbstractClass")
    );
    assert_eq!(
        TfType::find::<ConcreteClass>(),
        TfType::find_by_name("ConcreteClass")
    );
    assert_eq!(
        TfType::find::<ChildClass>(),
        TfType::find_by_name("ChildClass")
    );
}

// root has no base types; derived types list is non-empty.
#[test]
fn root_hierarchy() {
    register_hierarchy();

    let t_root = TfType::get_root();

    assert!(t_root.base_types().is_empty());
    assert!(!t_root.get_directly_derived_types().is_empty());
}

// unknown has no base types and no derived types.
#[test]
fn unknown_has_no_types() {
    let t_unknown = TfType::unknown();

    assert!(t_unknown.base_types().is_empty());
    assert!(t_unknown.get_directly_derived_types().is_empty());
}

// ChildClass has exactly 2 parents (ConcreteClass + IAbstractClass)
// and both GrandchildClass children.
#[test]
fn child_base_and_derived_counts() {
    register_hierarchy();

    let t_child = TfType::find::<ChildClass>();
    let t_concrete = TfType::find::<ConcreteClass>();
    let t_abstract = TfType::find::<IAbstractClass>();
    let t_gc = TfType::find::<GrandchildClass>();

    let child_parents = t_child.base_types();
    assert_eq!(child_parents.len(), 2);
    assert!(
        (child_parents[0] == t_concrete && child_parents[1] == t_abstract)
            || (child_parents[0] == t_abstract && child_parents[1] == t_concrete)
    );

    let child_derived = t_child.get_directly_derived_types();
    // At minimum GrandchildClass and OtherGrandchildClass
    assert!(child_derived.len() >= 2);
    assert!(child_derived.contains(&t_gc));
}

// GrandchildClass has exactly 1 parent (ChildClass) and no direct children.
#[test]
fn grandchild_parents_and_no_children() {
    register_hierarchy();

    let t_child = TfType::find::<ChildClass>();
    let t_gc = TfType::find::<GrandchildClass>();

    let gc_parents = t_gc.base_types();
    assert_eq!(gc_parents.len(), 1);
    assert_eq!(gc_parents[0], t_child);

    assert!(t_gc.get_directly_derived_types().is_empty());
}

// ConcreteClass and IAbstractClass derive directly from root.
#[test]
fn concrete_and_abstract_derive_from_root() {
    register_hierarchy();

    let t_root = TfType::get_root();
    let t_concrete = TfType::find::<ConcreteClass>();
    let t_abstract = TfType::find::<IAbstractClass>();

    assert_eq!(t_concrete.base_types(), vec![t_root]);
    assert_eq!(t_abstract.base_types(), vec![t_root]);
}

// ChildClass and GrandchildClass do NOT appear in root's direct derivatives.
#[test]
fn non_root_children_not_in_root_derived() {
    register_hierarchy();

    let t_root = TfType::get_root();
    let t_child = TfType::find::<ChildClass>();
    let t_gc = TfType::find::<GrandchildClass>();

    let root_derived = t_root.get_directly_derived_types();
    assert!(!root_derived.contains(&t_child));
    assert!(!root_derived.contains(&t_gc));
}

// get_n_base_types: returns total count and fills up to N entries.
#[test]
fn get_n_base_types() {
    register_hierarchy();

    let t_child = TfType::find::<ChildClass>();
    let child_bases = t_child.base_types();

    // Fill 1 slot: still reports 2 total
    let mut out1 = [TfType::unknown()];
    let total1 = t_child.get_n_base_types(&mut out1);
    assert_eq!(total1, 2);
    assert_eq!(out1[0], child_bases[0]);

    // Fill 2 slots
    let mut out2 = [TfType::unknown(); 2];
    let total2 = t_child.get_n_base_types(&mut out2);
    assert_eq!(total2, 2);
    assert_eq!(out2[0], child_bases[0]);
    assert_eq!(out2[1], child_bases[1]);

    // Fill 3 slots (more than available)
    let mut out3 = [TfType::unknown(); 3];
    let total3 = t_child.get_n_base_types(&mut out3);
    assert_eq!(total3, 2);
    assert_eq!(out3[0], child_bases[0]);
    assert_eq!(out3[1], child_bases[1]);
}

// POD types: i32 is POD, String is not.
#[test]
fn plain_old_data_type() {
    assert!(TfType::find::<i32>().is_plain_old_data_type());
    assert!(!TfType::find::<String>().is_plain_old_data_type());
}

// Enum types: a declared enum is_enum; i32 is not.
#[test]
fn enum_type() {
    #[derive(Clone, Copy)]
    #[repr(i32)]
    #[allow(dead_code)]
    enum TestEnumType {
        A,
        B,
        C,
    }

    TfType::declare_enum::<TestEnumType>("TestEnumType");

    assert!(!TfType::find::<TestEnumType>().is_unknown());
    assert!(TfType::find::<TestEnumType>().is_enum());
    assert!(!TfType::find::<i32>().is_enum());
}

// Alias lookup: add_alias then find_derived_by_name.
#[test]
fn alias_lookup() {
    register_hierarchy();

    struct SomeClassA;
    struct SomeClassB;

    TfType::declare_with_bases::<SomeClassA>("SomeClassA", &[TypeId::of::<ConcreteClass>()]);
    TfType::declare_with_bases::<SomeClassB>(
        "SomeClassB2", // unique name to avoid collision
        &[TypeId::of::<IAbstractClass>()],
    );

    let t_concrete = TfType::find::<ConcreteClass>();
    let t_class_a = TfType::find::<SomeClassA>();

    assert!(!t_class_a.is_unknown());

    // Register alias "SomeClassB" for SomeClassA under ConcreteClass
    t_class_a.add_alias(t_concrete, "SomeClassAliasB");

    let found = t_concrete.find_derived_by_name("SomeClassAliasB");
    assert_eq!(found, t_class_a);
}

// Declare by name (plugin-style): a name-only type can be found by name.
#[test]
fn declare_by_name_plugin_type() {
    let t_a = usd_tf::declare_by_name("PluginBaseA");
    let t_b = usd_tf::declare_by_name("PluginBaseB");

    assert!(!bool::from(t_a) == false, "PluginBaseA should be valid");
    assert!(!bool::from(t_b) == false, "PluginBaseB should be valid");
    assert_ne!(t_a, t_b);

    let found = TfType::find_by_name("PluginBaseA");
    assert_eq!(found, t_a);
}
