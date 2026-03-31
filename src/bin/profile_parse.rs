//! Minimal profiling binary for USDA/USDC loading.
//! Run: cargo build --release --bin profile_parse
//! Profile: samply record target/release/profile_parse.exe data/audi.usda

use std::time::Instant;

fn main() {
    usd::sdf::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: profile_parse <file.usd> [iterations]");
        std::process::exit(1);
    }

    let file_path = &args[1];
    let iterations: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1);

    let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);

    eprintln!(
        "Profiling: {} ({:.1} MB), {} iteration(s)",
        file_path,
        file_size as f64 / 1_048_576.0,
        iterations
    );

    for i in 0..iterations {
        let t0 = Instant::now();

        let stage = usd::usd::Stage::open(file_path, usd::usd::InitialLoadSet::LoadAll)
            .unwrap_or_else(|e| {
                eprintln!("Failed to open: {}", e);
                std::process::exit(1);
            });

        let open_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = Instant::now();
        let prim_count = stage.traverse_all().into_iter().count();
        let traverse_ms = t1.elapsed().as_secs_f64() * 1000.0;

        // Simulate collect_stage_time_samples from the viewer
        let t2 = Instant::now();
        let mut attr_count = 0usize;
        let mut ts_attr_count = 0usize;
        for prim in stage.traverse() {
            for attr_name in prim.get_attribute_names() {
                attr_count += 1;
                if let Some(attr) = prim.get_attribute(attr_name.get_text()) {
                    if attr.get_num_time_samples() > 0 {
                        ts_attr_count += 1;
                        let _ = attr.get_time_samples();
                    }
                }
            }
        }
        let collect_ms = t2.elapsed().as_secs_f64() * 1000.0;

        eprintln!(
            "  [{}] open={:.1}ms traverse={:.1}ms collect_ts={:.1}ms prims={} attrs={} ts_attrs={}",
            i, open_ms, traverse_ms, collect_ms, prim_count, attr_count, ts_attr_count
        );
    }
}
