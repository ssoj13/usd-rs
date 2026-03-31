//! Profiling benchmark test for USD stage loading and traversal.
//!
//! Run with: cargo test --test profile_loading -- --ignored --nocapture
//! The test will output timing information and generate a Chrome trace JSON.

use std::time::Instant;

use usd::usd::{InitialLoadSet, Stage};

/// Helper: measure a closure, return (result, elapsed_ms)
fn timed<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    eprintln!("  {:<40} {:>8.2} ms", label, elapsed.as_secs_f64() * 1000.0);
    result
}

/// Profile loading all available test files (USDA, USDC, USDZ).
/// Measures: file open, traverse, attribute access.
#[test]
#[ignore]
fn profile_stage_loading() {
    eprintln!("\n======================================================================");
    eprintln!("  USD-RS PROFILING BENCHMARK");
    eprintln!("======================================================================\n");

    // Register file format plugins (USDA, USDC, USDZ, ABC)
    usd::sdf::init();

    // Initialize profiling
    usd::trace::profiling::init();

    let files = [
        ("bgnd.usda", "data/bgnd.usda"),
        ("bgnd.usdc", "data/bgnd.usdc"),
        ("bmw_x3.usda", "data/bmw_x3.usda"),
        ("bmw_x3.usdc", "data/bmw_x3.usdc"),
        ("audi.usda", "data/audi.usda"),
        ("audi.usdc", "data/audi.usdc"),
    ];

    for (label, path) in &files {
        let full_path = format!("{}/{}", env!("CARGO_MANIFEST_DIR").replace('\\', "/"), path);

        // Check file exists
        if !std::path::Path::new(&full_path).exists() {
            eprintln!("  SKIP {} (not found)", label);
            continue;
        }

        let file_size = std::fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);

        eprintln!(
            "--- {} ({:.1} MB) ---",
            label,
            file_size as f64 / 1_048_576.0
        );

        // Stage::open (includes parsing + composition)
        let stage = timed(&format!("{}: Stage::open", label), || {
            Stage::open(&full_path, InitialLoadSet::LoadAll)
        });

        let stage = match stage {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  FAILED to open {}: {}", label, e);
                continue;
            }
        };

        // Traverse all prims
        let prim_count = timed(&format!("{}: traverse (count prims)", label), || {
            let mut count = 0u64;
            for _prim in stage.traverse_all() {
                count += 1;
            }
            count
        });
        eprintln!("    prims: {}", prim_count);

        // Access attributes on every prim
        let attr_count = timed(&format!("{}: attribute access", label), || {
            let mut count = 0u64;
            for prim in stage.traverse_all() {
                let names = prim.get_attribute_names();
                count += names.len() as u64;
                // Actually read first attribute to trigger resolution
                if let Some(first_name) = names.first() {
                    let _ = prim.get_attribute(first_name.get_text());
                }
            }
            count
        });
        eprintln!("    total attributes: {}", attr_count);

        // Second traverse pass (warm cache)
        let prim_count_2 = timed(&format!("{}: traverse #2 (warm cache)", label), || {
            let mut count = 0u64;
            for _prim in stage.traverse_all() {
                count += 1;
            }
            count
        });
        assert_eq!(prim_count, prim_count_2);

        // Traverse and count root children
        timed(&format!("{}: enumerate root children", label), || {
            let pseudo_root = stage.pseudo_root();
            let children = pseudo_root.children();
            eprintln!("    root children: {}", children.len());
            for child in children.iter().take(10) {
                let child_prefix = child.path().get_as_string();
                let sub_count = stage
                    .traverse_all()
                    .into_iter()
                    .filter(|p| p.path().get_as_string().starts_with(&child_prefix))
                    .count();
                eprintln!("      {}: {} prims", child.name().get_text(), sub_count);
            }
        });

        eprintln!();
    }

    // Shutdown profiling, write trace
    let trace_path = format!(
        "{}/profile_results.json",
        env!("CARGO_MANIFEST_DIR").replace('\\', "/")
    );
    usd::trace::profiling::shutdown(&trace_path);

    eprintln!("\nTrace written to: {}", trace_path);
}

/// Profile in-memory stage operations (create, define, mutate).
#[test]
#[ignore]
fn profile_in_memory_ops() {
    eprintln!("\n--- In-memory stage operations ---\n");

    // Register formats
    usd::sdf::init();

    let n_prims = 1_000; // reduced from 10k — define_prim is slow

    let stage = timed("create_in_memory", || {
        Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage")
    });

    timed(&format!("define {} prims", n_prims), || {
        for i in 0..n_prims {
            let path = format!("/World/Prim_{}", i);
            stage.define_prim(&path, "Mesh").expect("define prim");
        }
    });

    let count = timed("traverse all defined prims", || {
        stage.traverse_all().into_iter().count()
    });
    eprintln!("    prims traversed: {}", count);

    timed("get_attribute_names on all prims", || {
        for prim in stage.traverse_all() {
            let _ = prim.get_attribute_names();
        }
    });
}
