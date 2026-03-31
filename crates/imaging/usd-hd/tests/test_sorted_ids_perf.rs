// Port of pxr/imaging/hd/testenv/testHdSortedIdsPerf.cpp
// Performance benchmark for HdSortedIds operations.

use std::time::Instant;
use usd_hd::sorted_ids::HdSortedIds;
use usd_sdf::Path;

fn get_init_paths() -> Vec<Path> {
    let first_level = ['A', 'B', 'Y', 'Z'];
    let mut paths = Vec::new();
    let mut name = [b'/'; 8];
    name[2] = b'/';
    name[4] = b'/';
    name[6] = b'/';

    for &fl in &first_level {
        name[1] = fl as u8;
        for sl in b'A'..=b'Z' {
            name[3] = sl;
            for tl in b'A'..=b'Z' {
                name[5] = tl;
                for fl2 in b'A'..=b'Z' {
                    name[7] = fl2;
                    let s = std::str::from_utf8(&name).expect("valid utf8");
                    paths.push(Path::from(s));
                }
            }
        }
    }

    // Shuffle with fixed seed for reproducibility
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut indices: Vec<usize> = (0..paths.len()).collect();
    // Simple deterministic shuffle (not cryptographic, matches C++ seed=9223000)
    for i in (1..indices.len()).rev() {
        let mut h = DefaultHasher::new();
        (i as u64 ^ 9223000u64).hash(&mut h);
        let j = (h.finish() as usize) % (i + 1);
        indices.swap(i, j);
    }
    let shuffled: Vec<Path> = indices.into_iter().map(|i| paths[i].clone()).collect();
    shuffled
}

fn get_populated_ids() -> HdSortedIds {
    let mut ids = HdSortedIds::new();
    for p in &get_init_paths() {
        ids.insert(p.clone());
    }
    let _ = ids.get_ids();
    ids
}

#[test]
fn perf_populate() {
    let paths = get_init_paths();
    println!("Using {} initial paths", paths.len());

    let start = Instant::now();
    let mut result = HdSortedIds::new();
    for p in &paths {
        result.insert(p.clone());
    }
    let _ = result.get_ids();
    let elapsed = start.elapsed();
    println!("populate: {:?} ({} paths)", elapsed, paths.len());
}

#[test]
fn perf_single_remove_insert() {
    let test_paths = vec![
        Path::from("/A/A/A/A"),
        Path::from("/B/Y/O/B"),
        Path::from("/Y/M/M/V"),
        Path::from("/Z/Z/Z/Z"),
    ];

    let mut ids = get_populated_ids();

    for path in &test_paths {
        let start = Instant::now();
        ids.remove(path.clone());
        let _ = ids.get_ids();
        ids.insert(path.clone());
        let _ = ids.get_ids();
        let elapsed = start.elapsed();
        println!("add_del_{}: {:?}", path, elapsed);
    }
}

#[test]
fn perf_multi_remove_insert() {
    let test_paths = vec![
        Path::from("/A/A/A/A"),
        Path::from("/B/Y/O/B"),
        Path::from("/Y/M/M/V"),
        Path::from("/Z/Z/Z/Z"),
    ];

    let mut ids = get_populated_ids();

    let start = Instant::now();
    for path in &test_paths {
        ids.remove(path.clone());
    }
    let _ = ids.get_ids();
    for path in &test_paths {
        ids.insert(path.clone());
    }
    let _ = ids.get_ids();
    let elapsed = start.elapsed();
    println!("add_del_multi: {:?}", elapsed);
}

#[test]
fn perf_subtree_remove_insert() {
    let prefixes = vec![
        Path::from("/A/A/A"),
        Path::from("/B/Y/O"),
        Path::from("/Y/M/M"),
        Path::from("/Z/Z/Z"),
    ];

    let init = get_init_paths();
    let mut ids = get_populated_ids();

    for prefix in &prefixes {
        let subtree: Vec<Path> = init
            .iter()
            .filter(|p| p.has_prefix(prefix))
            .cloned()
            .collect();

        let start = Instant::now();
        for p in &subtree {
            ids.remove(p.clone());
        }
        let _ = ids.get_ids();
        for p in &subtree {
            ids.insert(p.clone());
        }
        let _ = ids.get_ids();
        let elapsed = start.elapsed();
        println!(
            "add_del_subtree_{}: {:?} ({} paths)",
            prefix,
            elapsed,
            subtree.len()
        );
    }
}

#[test]
fn perf_spread_remove_insert() {
    let mut ids = get_populated_ids();
    let sorted = ids.get_ids().clone();
    let num_elts = 100;

    let paths: Vec<Path> = (0..num_elts)
        .map(|x| {
            let idx = (sorted.len() * (x + 1)) / (num_elts + 1);
            sorted[idx].clone()
        })
        .collect();

    let start = Instant::now();
    for p in &paths {
        ids.remove(p.clone());
    }
    let _ = ids.get_ids();
    for p in &paths {
        ids.insert(p.clone());
    }
    let _ = ids.get_ids();
    let elapsed = start.elapsed();
    println!("add_del_{}_spread: {:?}", num_elts, elapsed);
}
