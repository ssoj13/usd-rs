//! usddumpcrate - Dump crate (.usdc) file information
//!
//! Port of pxr/usd/bin/usddumpcrate/usddumpcrate.py
//!
//! Displays diagnostic information about binary USD (.usdc) files.

use usd::sdf::CrateInfo;

/// Run the dumpcrate command
pub fn run(args: &[String]) -> i32 {
    match run_impl(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

fn run_impl(args: &[String]) -> Result<(), String> {
    let mut input_files: Vec<String> = Vec::new();
    let mut summary_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-s" | "--summary" => summary_only = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                input_files.push(args[i].clone());
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        return Err("No input files specified".to_string());
    }

    // Print software version header
    let software_version = CrateInfo::get_software_version();
    println!("Usd crate software version {}", software_version.as_str());

    // Process each file
    for fname in &input_files {
        let info = CrateInfo::open(fname);
        if !info.is_valid() {
            eprintln!("Error: Failed to read {}", fname);
            continue;
        }
        print_report(fname, &info, summary_only);
    }

    Ok(())
}

fn print_report(fname: &str, info: &CrateInfo, summary_only: bool) {
    // File header with version
    println!("@{}@ file version {}", fname, info.file_version());

    // Summary stats
    let stats = info.get_summary_stats();
    println!(
        "  {} specs, {} paths, {} tokens, {} strings, {} fields, {} field sets",
        stats.num_specs(),
        stats.num_unique_paths(),
        stats.num_unique_tokens(),
        stats.num_unique_strings(),
        stats.num_unique_fields(),
        stats.num_unique_field_sets()
    );

    if summary_only {
        return;
    }

    // Structural sections
    println!("  Structural Sections:");
    for section in info.get_sections() {
        println!(
            "    {:>16} {:>16} bytes at offset 0x{:X}",
            section.name(),
            section.size(),
            section.start()
        );
    }
    println!();
}

fn print_help() {
    println!(
        r#"usddumpcrate - Dump information about a USD crate (.usdc) file

USAGE:
    usd dumpcrate [options] <file> [file...]

ARGUMENTS:
    <files...>    Input .usdc files to inspect

OPTIONS:
    -h, --help       Show this help
    -s, --summary    Report only a short summary

OUTPUT:
    For each file, displays:
    - File version (crate format version)
    - Summary stats (specs, paths, tokens, strings, fields, field sets)
    - Structural sections with byte offsets and sizes (unless -s)

EXAMPLES:
    # Dump full info for a crate file
    usd dumpcrate model.usdc

    # Get summary for multiple files
    usd dumpcrate -s *.usdc

    # Debug binary format
    usd dumpcrate scene.usdc | grep TOKENS
"#
    );
}
