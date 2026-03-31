//! usdview — USD Scene Viewer (Rust/egui)
//!
//! Alternative to usdview (usdviewq). Open USD files, inspect prims, attributes, layers.

fn main() -> eframe::Result<()> {
    usd_sdf::init();

    let config = usd_view::launcher::parse_args(std::env::args()).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    });

    // Enable profiling if --profile flag is set
    let profiling = config.profile;
    if profiling {
        usd_trace::profiling::init();
    }

    usd_view::launcher::init_logging(&config);
    let result = usd_view::launcher::run(config);

    // Write trace output on exit
    if profiling {
        usd_trace::profiling::shutdown("trace.json");
    }

    result
}
