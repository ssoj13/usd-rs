// Port of pxr/imaging/hd/testenv/testHdDataSourceLocator.cpp

use usd_hd::data_source::{HdDataSourceLocator, HdDataSourceLocatorSet};
use usd_tf::Token;

fn parse(input: &str) -> HdDataSourceLocator {
    let tokens: Vec<Token> = input
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| Token::new(s))
        .collect();
    HdDataSourceLocator::new(&tokens)
}

fn t(s: &str) -> Token {
    Token::new(s)
}

// --- TestConstructors ---

#[test]
fn test_constructors_0_element() {
    assert_eq!(HdDataSourceLocator::empty().to_string(), "");
}

#[test]
fn test_constructors_1_element() {
    let loc = HdDataSourceLocator::from_token(t("a"));
    assert_eq!(loc.to_string(), "a");
}

#[test]
fn test_constructors_2_element() {
    let loc = HdDataSourceLocator::from_tokens_2(t("a"), t("b"));
    assert_eq!(loc.to_string(), "a/b");
}

#[test]
fn test_constructors_3_element() {
    let loc = HdDataSourceLocator::from_tokens_3(t("a"), t("b"), t("c"));
    assert_eq!(loc.to_string(), "a/b/c");
}

#[test]
fn test_constructors_n_elements() {
    let tokens = vec![t("a"), t("b"), t("c"), t("d"), t("e"), t("f")];
    let loc = HdDataSourceLocator::new(&tokens);
    assert_eq!(loc.to_string(), "a/b/c/d/e/f");
}

#[test]
fn test_constructors_parsing() {
    assert_eq!(parse("a/b").to_string(), "a/b");
    assert_eq!(parse("/a/b").to_string(), "a/b");
}

// --- TestEqualityAndHashing ---

#[test]
fn test_equality() {
    assert_eq!(parse("a/b"), parse("a/b"));
    assert_ne!(parse("a/b"), parse("a/c"));
}

#[test]
fn test_hashing() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(parse("a/b"));
    set.insert(parse("a/b/c"));

    assert_eq!(set.len(), 2);
    assert!(set.contains(&parse("a/b")));
    assert!(set.contains(&parse("a/b/c")));
    assert!(!set.contains(&parse("a/b/d")));
}

// --- TestAccessors ---

#[test]
fn test_is_empty() {
    assert!(HdDataSourceLocator::empty().is_empty());
    assert!(!parse("a/b/c").is_empty());
}

#[test]
fn test_element_count() {
    assert_eq!(parse("a/b/c").len(), 3);
}

#[test]
fn test_get_element() {
    let loc = parse("a/b/c");
    assert_eq!(loc.get_element(0), Some(&t("a")));
    assert_eq!(loc.get_element(1), Some(&t("b")));
    assert_eq!(loc.get_element(2), Some(&t("c")));
    assert_eq!(loc.last_element(), Some(&t("c")));
}

#[test]
fn test_remove_last_element() {
    let loc = parse("a/b/c");
    assert_eq!(loc.remove_last().to_string(), "a/b");
}

#[test]
fn test_has_prefix_empty() {
    let loc = parse("a/b/c");
    assert!(loc.has_prefix(&HdDataSourceLocator::empty()));
}

#[test]
fn test_has_prefix_parent() {
    let loc = parse("a/b/c");
    assert!(loc.has_prefix(&loc.remove_last()));
}

#[test]
fn test_has_prefix_shallow_ancestor() {
    let loc = parse("a/b/c");
    assert!(loc.has_prefix(&HdDataSourceLocator::from_token(t("a"))));
}

#[test]
fn test_has_prefix_unrelated() {
    let loc = parse("a/b/c");
    assert!(!loc.has_prefix(&parse("a/e")));
}

#[test]
fn test_common_prefix() {
    let loc = parse("a/b/c");
    assert_eq!(
        loc.common_prefix(&parse("a/e")),
        HdDataSourceLocator::from_token(t("a"))
    );
    assert_eq!(
        loc.common_prefix(&parse("e/f")),
        HdDataSourceLocator::empty()
    );
}

// --- TestAppendsAndReplaces ---

#[test]
fn test_replace_last_element() {
    let loc = parse("a/b/c");
    assert_eq!(loc.replace_last(t("z")).to_string(), "a/b/z");
}

#[test]
fn test_append_token() {
    let loc = parse("a/b/c");
    assert_eq!(loc.append(&t("z")).to_string(), "a/b/c/z");
}

#[test]
fn test_append_locator() {
    let loc = parse("a/b/c");
    assert_eq!(loc.append_locator(&loc).to_string(), "a/b/c/a/b/c");
}

#[test]
fn test_replace_prefix() {
    let loc = parse("a/b/c");
    assert_eq!(
        loc.replace_prefix(&parse("a"), &parse("X/Y")).to_string(),
        "X/Y/b/c"
    );
}

#[test]
fn test_replace_prefix_with_empty() {
    let loc = parse("a/b/c");
    assert_eq!(
        loc.replace_prefix(&parse("a/b"), &HdDataSourceLocator::empty())
            .to_string(),
        "c"
    );
}

#[test]
fn test_replace_prefix_unrelated() {
    let loc = parse("a/b/c");
    assert_eq!(
        loc.replace_prefix(&parse("X/Y"), &HdDataSourceLocator::empty())
            .to_string(),
        "a/b/c"
    );
}

// --- TestIntersection ---

#[test]
fn test_intersect_against_empty() {
    assert!(HdDataSourceLocator::from_token(t("a")).intersects(&HdDataSourceLocator::empty()));
}

#[test]
fn test_intersect_equal() {
    assert!(
        HdDataSourceLocator::from_token(t("a"))
            .intersects(&HdDataSourceLocator::from_token(t("a")))
    );
}

#[test]
fn test_intersect_nested() {
    assert!(parse("a/b/c").intersects(&parse("a")));
}

#[test]
fn test_intersect_unrelated() {
    assert!(!parse("a/b/c").intersects(&parse("d/e")));
}

#[test]
fn test_intersect_siblings() {
    assert!(!parse("a/b/c").intersects(&parse("a/b/d")));
}

// --- TestLocatorSet ---

#[test]
fn test_set_insert_non_intersecting() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/b"));

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("a/b"));
    baseline.insert(parse("c/b"));

    assert_eq!(locators, baseline);
}

#[test]
fn test_set_insert_intersecting() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("a/b/c")); // child of a/b, should be subsumed
    locators.insert(parse("f"));
    locators.insert(parse("a/b/d")); // child of a/b, should be subsumed

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("a/b"));
    baseline.insert(parse("c/d"));
    baseline.insert(parse("f"));

    assert_eq!(locators, baseline);
}

#[test]
fn test_set_insert_empty_locator() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("a/b/c"));
    locators.insert(parse("q/e/d"));
    locators.insert(HdDataSourceLocator::empty()); // universal - subsumes all

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(HdDataSourceLocator::empty());

    assert_eq!(locators, baseline);
}

// --- TestLocatorSetIntersects ---

#[test]
fn test_set_intersects_single_parent() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    assert!(locators.intersects_locator(&parse("a")));
}

#[test]
fn test_set_intersects_single_child() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    assert!(locators.intersects_locator(&parse("a/b/e")));
}

#[test]
fn test_set_intersects_single_sibling() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    assert!(!locators.intersects_locator(&parse("a/c")));
}

#[test]
fn test_set_intersects_single_equal() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    assert!(locators.intersects_locator(&parse("f")));
}

#[test]
fn test_set_intersects_single_unrelated() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    assert!(!locators.intersects_locator(&parse("x/y/z")));
}

#[test]
fn test_set_intersects_single_empty_locator() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    assert!(locators.intersects_locator(&HdDataSourceLocator::empty()));
}

#[test]
fn test_set_intersects_set_empty() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    let test1 = HdDataSourceLocatorSet::new();
    assert!(!locators.intersects(&test1));
}

#[test]
fn test_set_intersects_set_empty_locator() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    let test2 = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());
    assert!(locators.intersects(&test2));
}

#[test]
fn test_set_intersects_set_unrelated() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    let mut test3 = HdDataSourceLocatorSet::new();
    test3.insert(parse("g/h/i"));
    test3.insert(parse("q/r/s"));
    assert!(!locators.intersects(&test3));
}

#[test]
fn test_set_intersects_set_child() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    let mut test4 = HdDataSourceLocatorSet::new();
    test4.insert(parse("a/b/z"));
    test4.insert(parse("f/g/h"));
    assert!(locators.intersects(&test4));
}

#[test]
fn test_set_intersects_set_parent() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    let mut test5 = HdDataSourceLocatorSet::new();
    test5.insert(parse("a"));
    test5.insert(parse("z"));
    assert!(locators.intersects(&test5));
}

#[test]
fn test_set_intersects_set_sibling() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));

    let mut test6 = HdDataSourceLocatorSet::new();
    test6.insert(parse("a/c"));
    test6.insert(parse("z"));
    assert!(!locators.intersects(&test6));
}

#[test]
fn test_set_intersects_empty_sets() {
    let test1 = HdDataSourceLocatorSet::new();
    let test2 = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());

    assert!(!test1.intersects(&test2));
    assert!(!test1.intersects(&test1));
    assert!(test2.intersects(&test2));
    assert!(!test2.intersects(&test1));
}

// --- TestLocatorSetContains ---

#[test]
fn test_set_contains_empty_set() {
    let locators = HdDataSourceLocatorSet::new();
    assert!(!locators.contains(&parse("")));
    assert!(!locators.contains(&parse("c")));
    assert!(!locators.contains(&parse("c/d")));
}

#[test]
fn test_set_contains_universal() {
    let locators = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());
    assert!(locators.contains(&parse("")));
    assert!(locators.contains(&parse("c")));
    assert!(locators.contains(&parse("c/d")));
}

#[test]
fn test_set_contains_membership() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("c"));
    locators.insert(parse("f/g"));

    assert!(!locators.contains(&parse("")));
    assert!(!locators.contains(&parse("b")));
    assert!(!locators.contains(&parse("b/c")));
    assert!(locators.contains(&parse("c")));
    assert!(locators.contains(&parse("c/d")));
    assert!(!locators.contains(&parse("d")));
    assert!(!locators.contains(&parse("f")));
    assert!(locators.contains(&parse("f/g")));
    assert!(locators.contains(&parse("f/g/h")));
    assert!(!locators.contains(&parse("g")));
}

// --- TestLocatorSetReplaces ---

#[test]
fn test_set_replace_empty_set() {
    let locators = HdDataSourceLocatorSet::new();
    let result = locators.replace_prefix(&HdDataSourceLocator::empty(), &parse("foo"));
    assert_eq!(result, locators);
}

#[test]
fn test_set_replace_universal() {
    let locators = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());
    let result = locators.replace_prefix(&HdDataSourceLocator::empty(), &parse("foo"));

    let baseline = HdDataSourceLocatorSet::from_locator(parse("foo"));
    assert_eq!(result, baseline);
}

// --- TestLocatorSet (final intersection checks from C++) ---

#[test]
fn test_set_intersection_basic() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c"));

    assert!(locators.intersects_locator(&parse("c/d")));
    assert!(!locators.intersects_locator(&parse("e/f")));
}

// --- Large set tests (exercise binary search paths) ---

#[test]
fn test_set_intersects_large_set_parent() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));
    locators.insert(parse("g/a"));
    locators.insert(parse("g/b"));
    locators.insert(parse("g/c"));
    locators.insert(parse("g/d"));
    locators.insert(parse("g/e"));
    locators.insert(parse("g/f"));
    locators.insert(parse("g/g"));

    assert!(locators.intersects_locator(&parse("a")));
    assert!(locators.intersects_locator(&parse("a/b/e")));
    assert!(!locators.intersects_locator(&parse("a/c")));
    assert!(locators.intersects_locator(&parse("f")));
    assert!(!locators.intersects_locator(&parse("x/y/z")));
    assert!(locators.intersects_locator(&HdDataSourceLocator::empty()));
}

#[test]
fn test_set_intersects_large_set_sets() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("c/d"));
    locators.insert(parse("f"));
    locators.insert(parse("g/a"));
    locators.insert(parse("g/b"));
    locators.insert(parse("g/c"));
    locators.insert(parse("g/d"));
    locators.insert(parse("g/e"));
    locators.insert(parse("g/f"));
    locators.insert(parse("g/g"));

    let test2 = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());
    assert!(locators.intersects(&test2));

    let mut test3 = HdDataSourceLocatorSet::new();
    test3.insert(parse("g/h/i"));
    test3.insert(parse("q/r/s"));
    assert!(!locators.intersects(&test3));

    let mut test4 = HdDataSourceLocatorSet::new();
    test4.insert(parse("a/b/z"));
    test4.insert(parse("f/g/h"));
    assert!(locators.intersects(&test4));

    let mut test5 = HdDataSourceLocatorSet::new();
    test5.insert(parse("a"));
    test5.insert(parse("z"));
    assert!(locators.intersects(&test5));

    let mut test6 = HdDataSourceLocatorSet::new();
    test6.insert(parse("a/c"));
    test6.insert(parse("z"));
    assert!(!locators.intersects(&test6));
}

// --- Large set Contains ---

#[test]
fn test_set_contains_large_set() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("c"));
    locators.insert(parse("e/a"));
    locators.insert(parse("e/b"));
    locators.insert(parse("e/c"));
    locators.insert(parse("e/d"));
    locators.insert(parse("e/e"));
    locators.insert(parse("e/f"));
    locators.insert(parse("e/g"));
    locators.insert(parse("e/h"));
    locators.insert(parse("e/i"));
    locators.insert(parse("e/j"));
    locators.insert(parse("e/k"));
    locators.insert(parse("e/l"));
    locators.insert(parse("f/g"));

    assert!(!locators.contains(&parse("")));
    assert!(!locators.contains(&parse("b")));
    assert!(!locators.contains(&parse("b/c")));
    assert!(locators.contains(&parse("c")));
    assert!(locators.contains(&parse("c/d")));
    assert!(!locators.contains(&parse("d")));
    assert!(!locators.contains(&parse("f")));
    assert!(locators.contains(&parse("f/g")));
    assert!(locators.contains(&parse("f/g/h")));
    assert!(!locators.contains(&parse("g")));
}

// --- Missing ReplacePrefix variations ---

#[test]
fn test_set_replace_with_uniquify() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/a/c"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e"));
    locators.insert(parse("a/d/e"));

    // Replace a/c -> a/d causes collision with existing a/d/e
    let result = locators.replace_prefix(&parse("a/c"), &parse("a/d"));

    // After replace: a/a/c stays, a/c/d->a/d/d, a/c/e->a/d/e (merged with existing a/d/e)
    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("a/a/c"));
    baseline.insert(parse("a/d/d"));
    baseline.insert(parse("a/d/e"));

    assert_eq!(result, baseline);
}

#[test]
fn test_set_replace_prefix_to_empty() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/a/c"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e"));
    locators.insert(parse("a/d/e"));

    let result = locators.replace_prefix(&parse("a"), &HdDataSourceLocator::empty());

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("a/c"));
    baseline.insert(parse("c/d"));
    baseline.insert(parse("c/e"));
    baseline.insert(parse("d/e"));

    assert_eq!(result, baseline);
}

#[test]
fn test_set_replace_full_prefix_match() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/a/c"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e"));
    locators.insert(parse("a/d/e"));

    let result = locators.replace_prefix(&parse("a/c/d"), &parse("b"));

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("a/a/c"));
    baseline.insert(parse("b"));
    baseline.insert(parse("a/c/e"));
    baseline.insert(parse("a/d/e"));

    assert_eq!(result, baseline);
}

// --- Large set ReplacePrefix ---

#[test]
fn test_set_replace_large_set() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/b"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e/f"));
    locators.insert(parse("a/c/e/g"));
    locators.insert(parse("g/a"));
    locators.insert(parse("g/b"));
    locators.insert(parse("g/c/c"));
    locators.insert(parse("g/d/b"));

    // Not matched
    let r1 = locators.replace_prefix(&parse("a/d"), &parse("a/c"));
    assert_eq!(r1, locators);

    // Replace a/c -> X/Y
    let r2 = locators.replace_prefix(&parse("a/c"), &parse("X/Y"));
    let mut b2 = HdDataSourceLocatorSet::new();
    b2.insert(parse("a/b"));
    b2.insert(parse("X/Y/d"));
    b2.insert(parse("X/Y/e/f"));
    b2.insert(parse("X/Y/e/g"));
    b2.insert(parse("g/a"));
    b2.insert(parse("g/b"));
    b2.insert(parse("g/c/c"));
    b2.insert(parse("g/d/b"));
    assert_eq!(r2, b2);

    // Empty prefix match
    let r4 = locators.replace_prefix(&HdDataSourceLocator::empty(), &parse("X/Y"));
    let mut b4 = HdDataSourceLocatorSet::new();
    b4.insert(parse("X/Y/a/b"));
    b4.insert(parse("X/Y/a/c/d"));
    b4.insert(parse("X/Y/a/c/e/f"));
    b4.insert(parse("X/Y/a/c/e/g"));
    b4.insert(parse("X/Y/g/a"));
    b4.insert(parse("X/Y/g/b"));
    b4.insert(parse("X/Y/g/c/c"));
    b4.insert(parse("X/Y/g/d/b"));
    assert_eq!(r4, b4);
}

#[test]
fn test_set_replace_prefix_not_matched() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/a/c"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e"));
    locators.insert(parse("a/d/e"));

    let result = locators.replace_prefix(&parse("a/b"), &parse("a/d"));
    assert_eq!(result, locators);
}

// --- TestLocatorSetIntersection ---

// Helper: collect intersection into a sorted vec of string representations.
fn intersection_strs(set: &HdDataSourceLocatorSet, locator: &HdDataSourceLocator) -> Vec<String> {
    set.intersection(locator)
        .into_iter()
        .map(|l| l.to_string())
        .collect()
}

#[test]
fn test_locator_set_intersection_empty_set() {
    // Empty set: intersection with anything is empty.
    let locators = HdDataSourceLocatorSet::new();
    let empty = HdDataSourceLocator::empty();
    let primvars = HdDataSourceLocator::from_token(t("primvars"));

    assert_eq!(intersection_strs(&locators, &empty), Vec::<String>::new());
    assert_eq!(
        intersection_strs(&locators, &primvars),
        Vec::<String>::new()
    );
}

#[test]
fn test_locator_set_intersection_universal_set() {
    // Universal set (contains empty locator): intersection yields the query.
    let locators = HdDataSourceLocatorSet::from_locator(HdDataSourceLocator::empty());
    let empty = HdDataSourceLocator::empty();
    let primvars = HdDataSourceLocator::from_token(t("primvars"));

    // Intersection with empty locator yields empty locator itself.
    assert_eq!(intersection_strs(&locators, &empty), vec![""]);
    // Intersection with primvars yields primvars (set ancestor covers it).
    assert_eq!(intersection_strs(&locators, &primvars), vec!["primvars"]);
}

#[test]
fn test_locator_set_intersection_two_element_set() {
    // Set {mesh, primvars}.
    let mesh = HdDataSourceLocator::from_token(t("mesh"));
    let primvars = HdDataSourceLocator::from_token(t("primvars"));
    let primvars_color = primvars.append(&t("color"));
    let empty = HdDataSourceLocator::empty();

    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(mesh.clone());
    locators.insert(primvars.clone());

    // Query empty: both set elements are descendants of empty, yielded as-is.
    assert_eq!(
        intersection_strs(&locators, &empty),
        vec!["mesh", "primvars"]
    );
    // Query mesh: exact match, yields mesh.
    assert_eq!(intersection_strs(&locators, &mesh), vec!["mesh"]);
    // Query primvars: exact match, yields primvars.
    assert_eq!(intersection_strs(&locators, &primvars), vec!["primvars"]);
    // Query primvars/color: set element primvars is ancestor, yields the query.
    assert_eq!(
        intersection_strs(&locators, &primvars_color),
        vec!["primvars/color"]
    );
}

#[test]
fn test_locator_set_intersection_three_element_set() {
    // Set {mesh, primvars/color/interpolation, primvars/opacity}.
    let mesh = HdDataSourceLocator::from_token(t("mesh"));
    let primvars = HdDataSourceLocator::from_token(t("primvars"));
    let primvars_color = primvars.append(&t("color"));
    let primvars_color_interp = primvars_color.append(&t("interpolation"));
    let primvars_opacity = primvars.append(&t("opacity"));

    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(mesh.clone());
    locators.insert(primvars_color_interp.clone());
    locators.insert(primvars_opacity.clone());

    // Query primvars: both primvars/color/interpolation and primvars/opacity
    // are descendants, both yielded.
    assert_eq!(
        intersection_strs(&locators, &primvars),
        vec!["primvars/color/interpolation", "primvars/opacity"]
    );
    // Query primvars/color: only primvars/color/interpolation is a descendant.
    assert_eq!(
        intersection_strs(&locators, &primvars_color),
        vec!["primvars/color/interpolation"]
    );
}

#[test]
fn test_locator_set_intersection_large_set() {
    // Same as three-element test but with extra entries to trigger binary-search
    // code path in _FirstIntersection (size >= 5).
    let mesh = HdDataSourceLocator::from_token(t("mesh"));
    let primvars = HdDataSourceLocator::from_token(t("primvars"));
    let primvars_color = primvars.append(&t("color"));
    let primvars_color_interp = primvars_color.append(&t("interpolation"));
    let primvars_opacity = primvars.append(&t("opacity"));

    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(mesh.clone());
    locators.insert(primvars_color_interp.clone());
    locators.insert(primvars_opacity.clone());
    // Padding entries so the set exceeds the binary-search cutoff of 5.
    for suffix in &["za", "zb", "zc", "zd", "ze", "zf", "zg", "zh", "zi", "zj"] {
        locators.insert(HdDataSourceLocator::from_token(t(suffix)));
    }

    // Same expected results as the small set.
    assert_eq!(
        intersection_strs(&locators, &primvars),
        vec!["primvars/color/interpolation", "primvars/opacity"]
    );
    assert_eq!(
        intersection_strs(&locators, &primvars_color),
        vec!["primvars/color/interpolation"]
    );
}

#[test]
fn test_locator_set_intersection_iterator_last_element() {
    // Verify that elements yielded by intersection() have the correct last
    // token, mirroring the C++ IntersectionIterator::operator-> test.
    let primvars = HdDataSourceLocator::from_token(t("primvars"));
    let primvars_color = primvars.append(&t("color"));
    let primvars_opacity = primvars.append(&t("opacity"));
    let mesh = HdDataSourceLocator::from_token(t("mesh"));

    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(mesh.clone());
    locators.insert(primvars_color.clone());
    locators.insert(primvars_opacity.clone());

    let last_elements: Vec<String> = locators
        .intersection(&primvars)
        .into_iter()
        .map(|l| l.last_element().unwrap().as_str().to_string())
        .collect();

    assert_eq!(
        last_elements,
        vec![
            primvars_color.last_element().unwrap().as_str(),
            primvars_opacity.last_element().unwrap().as_str(),
        ]
    );
}

#[test]
fn test_set_replace_prefix_matched() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/a/c"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e"));
    locators.insert(parse("a/d/e"));

    let result = locators.replace_prefix(&parse("a/c"), &parse("X/Y"));

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("a/a/c"));
    baseline.insert(parse("X/Y/d"));
    baseline.insert(parse("X/Y/e"));
    baseline.insert(parse("a/d/e"));

    assert_eq!(result, baseline);
}

#[test]
fn test_set_replace_empty_prefix_match() {
    let mut locators = HdDataSourceLocatorSet::new();
    locators.insert(parse("a/a/c"));
    locators.insert(parse("a/c/d"));
    locators.insert(parse("a/c/e"));
    locators.insert(parse("a/d/e"));

    let result = locators.replace_prefix(&HdDataSourceLocator::empty(), &parse("X/Y"));

    let mut baseline = HdDataSourceLocatorSet::new();
    baseline.insert(parse("X/Y/a/a/c"));
    baseline.insert(parse("X/Y/a/c/d"));
    baseline.insert(parse("X/Y/a/c/e"));
    baseline.insert(parse("X/Y/a/d/e"));

    assert_eq!(result, baseline);
}
