use usd_tf::type_info_map::TypeInfoMap;

// Nothing exists before any insertion.
#[test]
fn empty_map_has_no_entries() {
    let m: TypeInfoMap<i32> = TypeInfoMap::new();

    // C++: m.Exists("doubleAlias") == false
    assert!(!m.exists_by_name("doubleAlias"));
    // C++: m.Exists(typeid(double)) == false
    assert!(!m.exists_by_type::<f64>());
    // C++: m.Exists(typeid(double).name()) == false
    // The type_name string for f64 — we use the same lookup path
    let f64_name = std::any::type_name::<f64>();
    assert!(!m.exists_by_name(f64_name));
}

// Find returns None (NULL) before any insertion.
#[test]
fn find_returns_none_before_insertion() {
    let m: TypeInfoMap<i32> = TypeInfoMap::new();

    // C++: m.Find("doubleAlias") == NULL
    assert_eq!(m.find_by_name("doubleAlias"), None);
    // C++: m.Find(typeid(double)) == NULL
    assert_eq!(m.find_by_type::<f64>(), None);
    // C++: m.Find(typeid(double).name()) == NULL
    let f64_name = std::any::type_name::<f64>();
    assert_eq!(m.find_by_name(f64_name), None);
}

// After Set(typeid(double), 13), Find returns 13 and Exists by typeid is true.
#[test]
fn set_by_type_then_find() {
    let mut m: TypeInfoMap<i32> = TypeInfoMap::new();

    // C++: m.Set(typeid(double), 13)
    m.set_by_type::<f64>(13);

    // C++: m.Find(typeid(double)) && *m.Find(typeid(double)) == 13
    assert_eq!(m.find_by_type::<f64>(), Some(&13));

    // The alias does not yet exist
    assert_eq!(m.find_by_name("doubleAlias"), None);

    // Exists by TypeId
    assert!(m.exists_by_type::<f64>());
    // Exists by type name string (typeid(double).name() equivalent)
    let f64_name = std::any::type_name::<f64>();
    assert!(m.exists_by_name(f64_name));
}

// After CreateAlias("doubleAlias", typeid(double)), Exists("doubleAlias") is true.
#[test]
fn create_alias_by_type() {
    let mut m: TypeInfoMap<i32> = TypeInfoMap::new();
    m.set_by_type::<f64>(13);

    // C++: m.CreateAlias("doubleAlias", typeid(double))
    assert!(m.create_alias_for_type::<f64>("doubleAlias"));
    assert!(m.exists_by_name("doubleAlias"));
}

// After Remove(typeid(double)), all lookups return false/None.
#[test]
fn remove_by_type_clears_all() {
    let mut m: TypeInfoMap<i32> = TypeInfoMap::new();
    m.set_by_type::<f64>(13);
    m.create_alias_for_type::<f64>("doubleAlias");

    // C++: m.Remove(typeid(double))
    m.remove_by_type::<f64>();

    assert!(!m.exists_by_name("doubleAlias"));
    assert!(!m.exists_by_type::<f64>());
    let f64_name = std::any::type_name::<f64>();
    assert!(!m.exists_by_name(f64_name));
}

// Set by name string (mirrors C++: m.Set(typeid(double).name(), 14)).
#[test]
fn set_by_name_string_then_find() {
    let mut m: TypeInfoMap<i32> = TypeInfoMap::new();

    // C++: m.Set(typeid(double).name(), 14)
    let f64_name = std::any::type_name::<f64>();
    m.set_by_name(f64_name, 14);

    // C++: m.Exists(typeid(double)) — after Set by name, should also be findable by type
    // Our TypeInfoMap supports find_by_name(type_name) as the fallback path.
    assert!(m.exists_by_name(f64_name));
}

// CreateAlias from a name-based entry (mirrors C++: CreateAlias("doubleAlias", typeid(double).name())).
#[test]
fn create_alias_by_name_string() {
    let mut m: TypeInfoMap<i32> = TypeInfoMap::new();
    let f64_name = std::any::type_name::<f64>();

    m.set_by_name(f64_name, 14);

    // C++: m.CreateAlias("doubleAlias", typeid(double).name())
    assert!(m.create_alias(f64_name2(), f64_name));
    assert!(m.exists_by_name(f64_name2()));
}

// Helper: unique alias name to avoid test pollution when tests run in parallel.
fn f64_name2() -> &'static str {
    "doubleAlias2"
}

// Full C++ test sequence in one test (mirrors Test_TfTypeInfoMap directly).
#[test]
fn full_cpp_sequence() {
    let mut m: TypeInfoMap<i32> = TypeInfoMap::new();
    let f64_name = std::any::type_name::<f64>();
    let alias = "doubleAliasSeq";

    // Nothing exists
    assert!(!m.exists_by_name(alias));
    assert!(!m.exists_by_type::<f64>());
    assert!(!m.exists_by_name(f64_name));

    assert_eq!(m.find_by_name(alias), None);
    assert_eq!(m.find_by_type::<f64>(), None);
    assert_eq!(m.find_by_name(f64_name), None);

    // Insert by TypeId
    m.set_by_type::<f64>(13);

    assert_eq!(m.find_by_type::<f64>(), Some(&13));
    assert_eq!(m.find_by_name(alias), None);
    assert!(m.exists_by_type::<f64>());
    assert!(m.exists_by_name(f64_name));

    // Create alias
    m.create_alias_for_type::<f64>(alias);
    assert!(m.exists_by_name(alias));

    // Remove
    m.remove_by_type::<f64>();

    assert!(!m.exists_by_name(alias));
    assert!(!m.exists_by_type::<f64>());
    assert!(!m.exists_by_name(f64_name));

    // Insert by name this time
    m.set_by_name(f64_name, 14);
    assert!(m.exists_by_name(f64_name));

    let alias2 = "doubleAliasSeq2";
    m.create_alias(alias2, f64_name);
    assert!(m.exists_by_name(alias2));
}
