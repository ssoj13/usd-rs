//! USD CLI - Unified command-line interface for USD operations
//!
//! Subcommands: cat, tree, dump, filter, diff, resolve, edit, stitch,
//! dumpcrate, stitchclips, genschemafromsdr, compress, fixbrokenpixarschemas, zip.

mod cat;
mod compress;
mod diff;
mod dump;
mod dumpcrate;
mod edit;
mod filter;
mod fixbrokenpixarschemas;
mod genschemafromsdr;
mod meshdump;
mod resolve;
mod stitch;
mod stitchclips;
mod tree;
mod view;
mod zip;

use std::sync::Once;
use tracing_subscriber::EnvFilter;

static INIT_LOGGING: Once = Once::new();

/// Initialize the USD library (file formats, etc.)
fn init_usd() {
    usd::sdf::init();
}

/// Initialize logging via tracing-subscriber (unified for all subcommands).
/// The `view` subcommand calls its own init with file output support.
fn init_logging(verbose: bool) {
    INIT_LOGGING.call_once(|| {
        let level = std::env::var("USD_LOG")
            .or_else(|_| std::env::var("RUST_LOG"))
            .unwrap_or_else(|_| {
                if verbose {
                    "info".into()
                } else {
                    "warn".into()
                }
            });

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::new(&level))
            .with_target(false)
            .without_time()
            .init();
    });
}

/// Run the appropriate subcommand based on arguments
fn run(args: &[String]) -> i32 {
    // Initialize USD library (file formats, etc.)
    init_usd();

    // Check for global verbose flag
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");

    // 'view' has its own logging init (file output, -vv/-vvv levels)
    let is_view = args
        .get(1)
        .map(|s| s == "view" || s == "v")
        .unwrap_or(false);
    if !is_view {
        init_logging(verbose);
    }

    if args.len() < 2 {
        print_help(&args[0]);
        return 0;
    }

    let command = &args[1];

    // Filter out global flags from sub_args (but keep -v for view — it has its own levels)
    let sub_args: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|a| is_view || (*a != "-v" && *a != "--verbose"))
        .cloned()
        .collect();

    match command.as_str() {
        "cat" => cat::run(&sub_args),
        "tree" => tree::run(&sub_args),
        "dump" => dump::run(&sub_args),
        "meshdump" => meshdump::run(&sub_args),
        "filter" => filter::run(&sub_args),
        "diff" => diff::run(&sub_args),
        "resolve" => resolve::run(&sub_args),
        "edit" => edit::run(&sub_args),
        "stitch" => stitch::run(&sub_args),
        "dumpcrate" => dumpcrate::run(&sub_args),
        "stitchclips" => stitchclips::run(&sub_args),
        "genschemafromsdr" => genschemafromsdr::run(&sub_args),
        "compress" => compress::run(&sub_args),
        "fixbrokenpixarschemas" => fixbrokenpixarschemas::run(&sub_args),
        "view" | "v" => view::run(&sub_args),
        "zip" => zip::run(&sub_args),
        "-h" | "--help" | "help" => {
            print_help(&args[0]);
            0
        }
        "-V" | "--version" | "version" => {
            print_version();
            0
        }
        "-v" | "--verbose" => {
            // Just verbose flag without command
            print_help(&args[0]);
            0
        }
        _ => {
            eprintln!("usd: unknown command '{}'", command);
            eprintln!();
            print_usage(&args[0]);
            1
        }
    }
}

fn print_usage(_prog: &str) {
    eprintln!("Usage: usd <command> [options] [args...]");
    eprintln!("       usd -h, --help");
    eprintln!("       usd -V, --version");
    eprintln!();
    eprintln!("Commands: cat, tree, dump, meshdump, filter, diff, resolve, edit, stitch,");
    eprintln!("          dumpcrate, stitchclips, genschemafromsdr, compress,");
    eprintln!("          fixbrokenpixarschemas, zip, view");
}

fn print_help(_prog: &str) {
    let prog = "usd";
    println!(
        r#"usd - Universal Scene Description command-line tools

USAGE:
    {0} <command> [options] [args...]
    {0} [options]

COMMANDS:
    cat              Print/convert USD files to stdout or file
    tree             Display prim hierarchy as ASCII tree
    dump             Dump raw SDF layer data with filtering
    meshdump         Dump one composed prim/mesh with xform and bounds details
    filter           Filter and transform SDF content
    diff             Compare two USD files
    resolve          Test asset resolution
    edit             Edit USD file in text editor
    stitch           Combine multiple USD layers
    dumpcrate        Dump .usdc binary file info
    stitchclips      Stitch USD files using value clips
    genschemafromsdr Generate schemas from SDR nodes
    compress         Draco compression (not implemented)
    fixbrokenpixarschemas  Fix broken Pixar schemas (MaterialBindingAPI, etc.)
    zip              Create/manage USDZ packages
    view (v)         Open USD scene viewer (GUI)

GLOBAL OPTIONS:
    -h, --help       Show this help message
    -V, --version    Show version information
    -v, --verbose    Enable verbose output (or set USD_LOG=info)

LOGGING:
    Set USD_LOG environment variable: error|warn|info|debug|trace

COMMAND OPTIONS:

  cat [options] <file> [file...]
    -o, --out <file>           Write to file instead of stdout
    -f, --flatten              Flatten composed stage
    --flattenLayerStack        Flatten layer stack only
    --usdFormat <usda|usdc>    Output format for .usd files
    --mask <paths>             Limit to prim paths (with --flatten)

  tree [options] <file>
    -a, --attributes           Show authored attributes
    -m, --metadata             Show authored metadata
    -s, --simple               Show prim names only
    -f, --flatten              Show composed stage tree
    --mask <paths>             Limit to prim paths (with --flatten)

  dump [options] <file> [file...]
    -s, --summary              Show high-level statistics only
    -p, --path <regex>         Filter paths by regex
    -f, --field <regex>        Filter fields by regex
    --sortBy <path|field>      Group output by path or field

  meshdump [options] <file> <primPath>
    -t, --time <value>         Sample time (default: default time)

  filter [options] <file> [file...]
    -o, --out <file>           Write to file
    -p, --path <regex>         Filter paths by regex
    -f, --field <regex>        Filter fields by regex
    --outputType <type>        Output type (validity|summary|layer)

  diff [options] <file1> <file2>
    -f, --flatten              Flatten before comparing
    -q, --brief                Only report if files differ
    -n, --noeffect             Do not edit files

  resolve [options] <path>
    --anchorPath <path>        Anchor relative path resolution
    --createContextForAsset    Create context for specific asset

  edit [options] <file>
    -n, --noeffect             Read-only mode
    -f, --forcewrite           Force write even if read-only
    -p, --prefix <str>         Temp file prefix

  stitch [options] <files...> -o <output>
    -o, --out <file>           Output file (required)

  dumpcrate [options] <file> [file...]
    -s, --summary              Report only short summary

  stitchclips [options] <files...> -o <output> -c <clipPath>
    -o, --out <file>           Output file (required)
    -c, --clipPath <path>      Prim path for clips (required)
    -s, --startTimeCode <t>    Start time code
    -e, --endTimeCode <t>      End time code
    --stride <n>               Time stride between clips
    --templatePath <path>      Template for clip paths
    --clipSet <name>           Clip set name

  genschemafromsdr [config] [outputDir]
    Generates USD schema from SDR shader definitions

  compress [options] <input> -o <output>
    (Not implemented - requires Draco library)

  zip [options] <input> [output.usdz]
    -l, --list                 List contents of USDZ
    -d, --dump                 Dump detailed file info
    -r, --recurse              Include dependencies (default)
    --norecurse                Don't include dependencies
    -o, --output <file>        Output file path

EXAMPLES:
    # Print a USD file
    {0} cat model.usda

    # Convert to binary format
    {0} cat model.usda -o model.usdc

    # Flatten and save
    {0} cat --flatten scene.usd -o flat.usda

    # Show prim tree with attributes
    {0} tree -a model.usda

    # Get layer statistics
    {0} dump -s model.usda

    # Compare two files
    {0} diff model_v1.usda model_v2.usda

    # Test asset resolution
    {0} resolve ./textures/diffuse.png

    # Edit a USD file
    {0} edit model.usda

    # Combine layers
    {0} stitch base.usd overlay.usd -o merged.usd

    # Verbose mode with debug logging
    USD_LOG=debug {0} cat model.usda

    # Create USDZ package
    {0} zip model.usda -o model.usdz

    # List USDZ contents
    {0} zip -l package.usdz

For command-specific help: {0} <command> --help
"#,
        prog
    );
}

fn print_version() {
    println!("usd {} (usd-rs)", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Rust implementation of USD command-line tools.");
    println!("https://github.com/ssoj13/usd-rs");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exit_code = run(&args);
    std::process::exit(exit_code);
}
