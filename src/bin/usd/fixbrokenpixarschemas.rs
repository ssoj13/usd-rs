//! usdfixbrokenpixarschemas - Fix broken Pixar schemas in USD files.
//!
//! Port of pxr/bin/usdfixbrokenpixarschemas/usdfixbrokenpixarschemas.py
//!
//! Applies schema migration fixes (MaterialBindingAPI, SkelBindingAPI, upAxis, etc.)
//! to usd/usda/usdc files.

use std::path::Path;

/// Runs the usdfixbrokenpixarschemas command.
pub fn run(args: &[String]) -> i32 {
    let mut input_file: Option<String> = None;
    let mut backup_file: Option<String> = None;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "-v" | "--verbose" => verbose = true,
            "--backup" => {
                i += 1;
                if i < args.len() {
                    backup_file = Some(args[i].clone());
                } else {
                    eprintln!("Error: --backup requires a file path");
                    return 1;
                }
            }
            _ if !arg.starts_with('-') => {
                if input_file.is_none() {
                    input_file = Some(arg.clone());
                }
            }
            _ => {
                eprintln!("Unknown option: {}", arg);
                return 1;
            }
        }
        i += 1;
    }

    let input = match input_file {
        Some(f) => f,
        None => {
            eprintln!("Error: No input file specified");
            print_help();
            return 1;
        }
    };

    let ext = Path::new(&input).extension().and_then(|e| e.to_str());
    let valid_ext = matches!(ext, Some("usd") | Some("usda") | Some("usdc"));
    if !valid_ext {
        eprintln!(
            "Error: Invalid input extension. Expected .usd, .usda, or .usdc, got {:?}",
            ext
        );
        return 1;
    }

    if !Path::new(&input).exists() {
        eprintln!("Error: Input file '{}' does not exist.", input);
        return 1;
    }

    let backup = backup_file.unwrap_or_else(|| {
        let p = Path::new(&input);
        let parent = p.parent().unwrap_or(Path::new("."));
        let stem = p.file_stem().unwrap_or_default();
        let ext = ext.unwrap_or("usda");
        parent
            .join(format!("{}.bck.{}", stem.to_string_lossy(), ext))
            .to_string_lossy()
            .to_string()
    });

    let backup_ext = Path::new(&backup).extension().and_then(|e| e.to_str());
    if !matches!(backup_ext, Some("usd") | Some("usda") | Some("usdc")) {
        eprintln!(
            "Error: Invalid backup extension. Expected .usd, .usda, or .usdc, got {:?}",
            backup_ext
        );
        return 1;
    }

    usd::sdf::init();

    if let Err(e) = std::fs::copy(&input, &backup) {
        eprintln!("Error: Failed to create backup '{}': {}", backup, e);
        return 1;
    }

    let layer = match usd::sdf::Layer::find_or_open(&input) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Error: Failed to open '{}': {}", input, e);
            std::fs::remove_file(&backup).ok();
            return 1;
        }
    };

    let handle = usd::sdf::LayerHandle::from_layer(&layer);
    let fixer = usd::usd_utils::fix_broken_pixar_schemas::FixBrokenPixarSchemas::new(handle);
    fixer.apply_all();

    if fixer.is_layer_updated() {
        if verbose {
            eprintln!("Fixes applied to '{}', saving...", input);
        }
        if let Err(e) = layer.save() {
            eprintln!("Error: Failed to save '{}': {}", input, e);
            if std::fs::copy(&backup, &input).is_err() {
                eprintln!("Warning: Failed to restore from backup");
            }
            return 1;
        }
        if verbose {
            eprintln!("Saved. Backup at '{}'", backup);
        }
    } else {
        if verbose {
            eprintln!("No fixes required for '{}'", input);
        }
    }

    0
}

fn print_help() {
    println!(
        r#"usdfixbrokenpixarschemas - Fix broken Pixar schemas in USD files

USAGE:
    usd fixbrokenpixarschemas [options] <inputFile>

OPTIONS:
    -h, --help       Show this help
    --backup <file>  Backup file path (default: <input>.bck.<ext>)
    -v, --verbose    Verbose output

Applies schema migration fixes:
  - MaterialBindingAPI: add to prims with material:binding
  - SkelBindingAPI: add to prims with skel properties
  - upAxis: set if missing on layer

Supported formats: .usd, .usda, .usdc
"#
    );
}
