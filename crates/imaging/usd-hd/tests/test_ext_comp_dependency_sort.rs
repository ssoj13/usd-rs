// Port of pxr/imaging/hd/testenv/testHdExtCompDependencySort.cpp

use usd_hd::ext_computation_utils::{ComputationDesc, get_computation_order};
use usd_sdf::Path;
use usd_tf::Token;

fn comp(path: &str) -> ComputationDesc {
    ComputationDesc {
        path: Path::from(path),
        inputs: Vec::new(),
        outputs: Vec::new(),
        is_gpu: false,
    }
}

fn comp_with_deps(path: &str, deps: &[&str]) -> ComputationDesc {
    ComputationDesc {
        path: Path::from(path),
        inputs: deps
            .iter()
            .map(|d| (Path::from(*d), Token::new("out"), Token::new("in")))
            .collect(),
        outputs: Vec::new(),
        is_gpu: false,
    }
}

fn occurs_before(order: &[Path], a: &str, b: &str) -> bool {
    let pos_a = order.iter().position(|p| p == &Path::from(a));
    let pos_b = order.iter().position(|p| p == &Path::from(b));
    match (pos_a, pos_b) {
        (Some(a), Some(b)) => a < b,
        _ => false,
    }
}

#[test]
fn test_linear_chain_dependency() {
    // A <-- B <-- C
    let computations = vec![
        comp_with_deps("A", &["B"]),
        comp_with_deps("B", &["C"]),
        comp("C"),
    ];

    let order = get_computation_order(&computations);
    assert!(order.is_some(), "Sort should succeed for linear chain");
    let order = order.unwrap();

    assert_eq!(
        order,
        vec![Path::from("C"), Path::from("B"), Path::from("A")]
    );
}

#[test]
fn test_tree_chain_dependency() {
    // A <-- B <-- C
    // ^     ^
    // |     '-- D <-- E
    // '-- F
    let computations = vec![
        comp_with_deps("A", &["B", "F"]),
        comp_with_deps("B", &["C", "D"]),
        comp_with_deps("D", &["E"]),
        comp("C"),
        comp("E"),
        comp("F"),
    ];

    let order = get_computation_order(&computations);
    assert!(order.is_some(), "Sort should succeed for tree chain");
    let order = order.unwrap();

    assert!(occurs_before(&order, "F", "A"));
    assert!(occurs_before(&order, "C", "B"));
    assert!(occurs_before(&order, "E", "B"));
    assert!(occurs_before(&order, "B", "A"));
}

#[test]
fn test_cycle_dependency() {
    // B --> C --> D --> B (cycle)
    let computations = vec![
        comp_with_deps("A", &["B", "F"]),
        comp_with_deps("B", &["D"]),
        comp_with_deps("C", &["B"]),
        comp_with_deps("D", &["C", "E"]),
        comp("E"),
        comp("F"),
    ];

    let order = get_computation_order(&computations);
    assert!(order.is_none(), "Sort should fail for cycle");
}
