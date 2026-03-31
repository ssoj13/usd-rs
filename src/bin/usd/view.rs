//! `usd view` — launch the USD scene viewer (GUI).

/// Run the viewer subcommand.
pub fn run(args: &[String]) -> i32 {
    // Re-parse args into ViewerConfig (skip "view" command name)
    let iter = args.iter().skip(1).cloned(); // skip "view"
    let config =
        match usd_view::launcher::parse_args(std::iter::once("usdview".to_string()).chain(iter)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("usd view: {e}");
                return 1;
            }
        };

    usd_view::launcher::init_logging(&config);

    match usd_view::launcher::run(config) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("usd view: {e}");
            1
        }
    }
}
