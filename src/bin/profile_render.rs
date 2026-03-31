//! Minimal one-shot imaging profiler for heavy USD scenes.
//!
//! This exists to separate core stage loading from the first Hydra/Storm frame.
//! The viewer can feel "hung on load" even when `Stage::open()` is fast, so this
//! binary times the first imaging steps directly: `prepare_batch()` and
//! `render()`.

use std::collections::BTreeMap;
use std::time::Instant;

use usd::gf::Vec2i;
use usd::usd::{InitialLoadSet, Stage};
use usd::usd_imaging::gl::{DrawMode, Engine, EngineParameters, RenderParams};

fn main() {
    usd::sdf::init();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: profile_render <file.usd>");
        std::process::exit(1);
    }

    let file_path = &args[1];
    let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
    eprintln!(
        "Render profiling: {} ({:.1} MB)",
        file_path,
        file_size as f64 / 1_048_576.0
    );

    let t0 = Instant::now();
    let stage = Stage::open(file_path, InitialLoadSet::LoadAll).unwrap_or_else(|e| {
        eprintln!("Failed to open stage: {e}");
        std::process::exit(1);
    });
    let open_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let mut type_counts: BTreeMap<String, usize> = BTreeMap::new();
    for prim in stage.traverse_all() {
        let ty = prim.type_name().get_text().to_string();
        *type_counts.entry(ty).or_default() += 1;
    }
    eprintln!("Prim types:");
    for (ty, count) in &type_counts {
        eprintln!("  {ty}: {count}");
    }

    let mut engine = Engine::new(EngineParameters::default());
    engine.set_render_buffer_size(Vec2i::new(64, 64));

    let root = stage.pseudo_root();
    let params = RenderParams::new()
        .with_draw_mode(DrawMode::ShadedSmooth)
        .with_lighting(true);

    let t1 = Instant::now();
    engine.prepare_batch(&root, &params);
    let prepare_ms = t1.elapsed().as_secs_f64() * 1000.0;

    let t2 = Instant::now();
    engine.render(&root, &params);
    let render_ms = t2.elapsed().as_secs_f64() * 1000.0;

    let t3 = Instant::now();
    let pixel_len = engine.read_render_pixels().map(|p| p.len()).unwrap_or(0);
    let read_ms = t3.elapsed().as_secs_f64() * 1000.0;

    eprintln!(
        "timings: open={open_ms:.1}ms prepare={prepare_ms:.1}ms render={render_ms:.1}ms read={read_ms:.1}ms pixels={pixel_len}"
    );
}
