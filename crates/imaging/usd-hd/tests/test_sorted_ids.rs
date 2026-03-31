// Port of pxr/imaging/hd/testenv/testHdSortedIds.cpp

use usd_hd::sorted_ids::HdSortedIds;
use usd_sdf::Path;

fn p(s: &str) -> Path {
    Path::from(s)
}

fn populate_paths() -> Vec<Path> {
    let first_level = ['A', 'B', 'Y', 'Z'];
    let mut paths = Vec::new();

    for &fl in &first_level {
        for sl in 'A'..='Z' {
            paths.push(p(&format!("/{}/{}", fl, sl)));
        }
    }
    paths
}

fn populate(sorted_ids: &mut HdSortedIds) -> Vec<Path> {
    let paths = populate_paths();
    for path in &paths {
        sorted_ids.insert(path.clone());
    }
    let _ = sorted_ids.get_ids(); // trigger sort
    paths
}

#[test]
fn test_populate() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    let ids = sorted_ids.get_ids();
    assert_eq!(ids.len(), 4 * 26);

    for i in 1..ids.len() {
        assert!(ids[i - 1] <= ids[i], "Not sorted at index {}", i);
    }
}

#[test]
fn test_single_insert() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    sorted_ids.insert(p("/I/J"));

    let ids = sorted_ids.get_ids();
    assert_eq!(ids.len(), 4 * 26 + 1);

    for i in 1..ids.len() {
        assert!(ids[i - 1] <= ids[i], "Not sorted at index {}", i);
    }

    assert!(ids.contains(&p("/I/J")));
}

#[test]
fn test_multi_insert() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    for c in 'A'..='Z' {
        sorted_ids.insert(p(&format!("/I/{}", c)));
    }

    let ids = sorted_ids.get_ids();
    assert_eq!(ids.len(), 4 * 26 + 26);

    for i in 1..ids.len() {
        assert!(ids[i - 1] <= ids[i], "Not sorted at index {}", i);
    }
}

#[test]
fn test_remove() {
    let mut sorted_ids = HdSortedIds::new();
    let paths = populate(&mut sorted_ids);

    let mut removed = Vec::new();
    for path in paths.iter().skip(10).take(10) {
        sorted_ids.remove(path.clone());
        removed.push(path.clone());
    }

    let ids = sorted_ids.get_ids();

    for i in 1..ids.len() {
        assert!(ids[i - 1] <= ids[i], "Not sorted at index {}", i);
    }

    assert_eq!(ids.len(), paths.len() - 10);

    for removed_id in &removed {
        assert!(
            !ids.contains(removed_id),
            "{} should have been removed",
            removed_id
        );
    }
}

#[test]
fn test_remove_only_element() {
    let mut sorted_ids = HdSortedIds::new();
    let paths = populate_paths();

    sorted_ids.insert(paths[0].clone());
    let _ = sorted_ids.get_ids();
    sorted_ids.remove(paths[0].clone());

    let ids = sorted_ids.get_ids();
    assert!(ids.is_empty());
}

#[test]
fn test_remove_range() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    let ids = sorted_ids.get_ids().clone();

    let range_start = ids.iter().position(|id| *id >= p("/B")).unwrap_or(0);
    let range_end = ids
        .iter()
        .position(|id| *id >= p("/C"))
        .unwrap_or(ids.len())
        - 1;

    let expected_len = ids.len() - (range_end - range_start + 1);

    sorted_ids.remove_range(range_start, range_end);

    let ids_after = sorted_ids.get_ids();
    assert_eq!(ids_after.len(), expected_len);

    for id in ids_after {
        assert!(
            !id.as_str().starts_with("/B/"),
            "/B path still present: {}",
            id
        );
    }
}

#[test]
fn test_remove_batch() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    for c in 'A'..='Z' {
        sorted_ids.remove(p(&format!("/Y/{}", c)));
    }

    let ids = sorted_ids.get_ids();

    for id in ids {
        assert!(
            !id.as_str().starts_with("/Y/"),
            "/Y path still present: {}",
            id
        );
    }
}

#[test]
fn test_remove_last_item() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);
    let paths = sorted_ids.get_ids().clone();

    for path in paths.iter().rev() {
        sorted_ids.remove(path.clone());
    }

    let ids = sorted_ids.get_ids();
    assert!(ids.is_empty());
}

#[test]
fn test_insert_remove_dupes() {
    let mut sorted_ids = HdSortedIds::new();

    sorted_ids.insert(p("/B"));
    sorted_ids.insert(p("/A"));

    assert_eq!(sorted_ids.get_ids().clone(), vec![p("/A"), p("/B")]);

    sorted_ids.insert(p("/B"));
    sorted_ids.insert(p("/A"));
    sorted_ids.insert(p("/B"));
    sorted_ids.insert(p("/A"));

    assert_eq!(
        sorted_ids.get_ids().clone(),
        vec![p("/A"), p("/A"), p("/A"), p("/B"), p("/B"), p("/B")]
    );

    sorted_ids.remove(p("/B"));
    assert_eq!(
        sorted_ids.get_ids().clone(),
        vec![p("/A"), p("/A"), p("/A"), p("/B"), p("/B")]
    );

    sorted_ids.remove(p("/A"));
    sorted_ids.remove(p("/B"));
    assert_eq!(
        sorted_ids.get_ids().clone(),
        vec![p("/A"), p("/A"), p("/B")]
    );

    sorted_ids.remove(p("/A"));
    sorted_ids.remove(p("/B"));
    assert_eq!(sorted_ids.get_ids().clone(), vec![p("/A")]);

    sorted_ids.remove(p("/A"));
    assert!(sorted_ids.get_ids().is_empty());
}

#[test]
fn test_insert_remove_without_sync() {
    let mut sorted_ids = HdSortedIds::new();

    sorted_ids.insert(p("B"));
    sorted_ids.insert(p("B"));
    sorted_ids.remove(p("B"));
    sorted_ids.insert(p("A"));
    sorted_ids.insert(p("B"));
    sorted_ids.insert(p("A"));
    sorted_ids.insert(p("A"));
    sorted_ids.remove(p("B"));
    sorted_ids.remove(p("A"));
    sorted_ids.remove(p("A"));

    assert_eq!(sorted_ids.get_ids().clone(), vec![p("A"), p("B")]);

    sorted_ids.insert(p("C"));
    sorted_ids.remove(p("B"));
    sorted_ids.remove(p("B"));
    sorted_ids.insert(p("C"));
    sorted_ids.insert(p("A"));
    sorted_ids.insert(p("B"));
    sorted_ids.remove(p("C"));
    sorted_ids.insert(p("C"));
    sorted_ids.remove(p("C"));
    sorted_ids.remove(p("A"));

    assert_eq!(sorted_ids.get_ids().clone(), vec![p("A"), p("B"), p("C")]);

    sorted_ids.insert(p("D"));
    sorted_ids.remove(p("D"));
    sorted_ids.remove(p("B"));

    assert_eq!(sorted_ids.get_ids().clone(), vec![p("A"), p("C")]);
}

#[test]
fn test_remove_after_insert_no_sync() {
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    sorted_ids.remove(p("/Z/A"));
    sorted_ids.insert(p("/I/I"));
    sorted_ids.remove(p("/I/I"));

    let ids = sorted_ids.get_ids();
    assert!(!ids.contains(&p("/Z/A")));
    assert!(!ids.contains(&p("/I/I")));
}

#[test]
fn test_remove_sorted() {
    // Port of RemoveSortedTest: remove prims from sorted bucket
    let first_level = ['A', 'B', 'Y', 'Z'];
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    for &c in first_level.iter().rev() {
        sorted_ids.remove(p(&format!("/{}/{}", c, c)));
    }

    let ids = sorted_ids.get_ids();
    for &c in &first_level {
        assert!(
            !ids.contains(&p(&format!("/{}/{}", c, c))),
            "/{}/{} should have been removed",
            c,
            c
        );
    }
    // Verify sorted
    for i in 1..ids.len() {
        assert!(ids[i - 1] <= ids[i], "Not sorted at index {}", i);
    }
}

#[test]
fn test_remove_unsorted() {
    // Port of RemoveUnsortedTest: remove prims from unsorted bucket
    let first_level = ['A', 'B', 'Y', 'Z'];
    let mut sorted_ids = HdSortedIds::new();
    populate(&mut sorted_ids);

    for &c in &first_level {
        sorted_ids.remove(p(&format!("/{}/{}", c, c)));
    }

    let ids = sorted_ids.get_ids();
    for &c in &first_level {
        assert!(
            !ids.contains(&p(&format!("/{}/{}", c, c))),
            "/{}/{} should have been removed",
            c,
            c
        );
    }
    // Verify sorted
    for i in 1..ids.len() {
        assert!(ids[i - 1] <= ids[i], "Not sorted at index {}", i);
    }
}
