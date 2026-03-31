//! USD Cat command - Print/convert USD files
//!
//! Equivalent to standalone `usdcat` tool.

/// Run the cat command with given arguments
pub fn run(args: &[String]) -> i32 {
    if args.len() < 2 {
        print_usage();
        return 1;
    }

    let mut input_files: Vec<String> = Vec::new();
    let mut output_file: Option<String> = None;
    let mut usd_format: Option<String> = None;
    let mut load_only = false;
    let mut flatten = false;
    let mut flatten_layer_stack = false;
    let mut mask: Option<String> = None;
    let mut layer_metadata = false;
    let mut skip_source_comment = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "-o" | "--out" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --out requires a file argument");
                    return 1;
                }
                output_file = Some(args[i].clone());
            }
            "--usdFormat" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --usdFormat requires an argument (usda or usdc)");
                    return 1;
                }
                let fmt = args[i].to_lowercase();
                if fmt != "usda" && fmt != "usdc" {
                    eprintln!("error: --usdFormat must be 'usda' or 'usdc'");
                    return 1;
                }
                usd_format = Some(fmt);
            }
            "-l" | "--loadOnly" => load_only = true,
            "-f" | "--flatten" => flatten = true,
            "--flattenLayerStack" => flatten_layer_stack = true,
            "--mask" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --mask requires a path argument");
                    return 1;
                }
                mask = Some(args[i].clone());
            }
            "--layerMetadata" => layer_metadata = true,
            "--skipSourceFileComment" => skip_source_comment = true,
            arg if arg.starts_with('-') => {
                eprintln!("error: unknown option: {}", arg);
                return 1;
            }
            _ => {
                input_files.push(args[i].clone());
            }
        }
        i += 1;
    }

    if input_files.is_empty() {
        eprintln!("error: no input files specified");
        return 1;
    }

    // Validate options
    if output_file.is_some() && input_files.len() != 1 {
        eprintln!("error: must supply exactly one input file when writing to an output file");
        return 1;
    }

    if mask.is_some() && !flatten {
        eprintln!("error: --mask requires --flatten");
        return 1;
    }

    if layer_metadata && (flatten || flatten_layer_stack) {
        eprintln!("error: --layerMetadata cannot be used with --flatten or --flattenLayerStack");
        return 1;
    }

    if output_file.is_none() && usd_format.as_ref().is_some_and(|f| f != "usda") {
        eprintln!("error: --usdFormat must be 'usda' when writing to stdout");
        return 1;
    }

    // Process files
    let mut exit_code = 0;

    for input in &input_files {
        match process_file(
            input,
            output_file.as_deref(),
            usd_format.as_deref(),
            load_only,
            flatten,
            flatten_layer_stack,
            mask.as_deref(),
            layer_metadata,
            skip_source_comment,
        ) {
            Ok(()) => {
                if load_only {
                    println!("OK  {}", input);
                }
            }
            Err(e) => {
                if load_only {
                    println!("ERR {}", input);
                    println!("\t{}", e);
                } else {
                    eprintln!("error: failed to process '{}': {}", input, e);
                }
                exit_code = 1;
            }
        }
    }

    exit_code
}

fn print_usage() {
    eprintln!("Usage: usd cat [options] <inputFile> [inputFile ...]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -o, --out <file>     Write output to file instead of stdout");
    eprintln!("  --usdFormat <fmt>    Output format: usda or usdc (for .usd files)");
    eprintln!("  -l, --loadOnly       Only test if files can be loaded");
    eprintln!("  -f, --flatten        Flatten composed stage");
    eprintln!("  --flattenLayerStack  Flatten layer stack only");
    eprintln!("  --mask <paths>       Limit to these prim paths (requires --flatten)");
    eprintln!("  --layerMetadata      Load only layer metadata");
    eprintln!("  --skipSourceFileComment  Skip source comment in flattened output");
    eprintln!("  -h, --help           Show this help");
}

fn print_help() {
    println!("usd cat - Print/convert USD files");
    println!();
    print_usage();
}

#[allow(clippy::too_many_arguments)]
fn process_file(
    input: &str,
    output: Option<&str>,
    _usd_format: Option<&str>,
    load_only: bool,
    flatten: bool,
    flatten_layer_stack: bool,
    _mask: Option<&str>,
    layer_metadata: bool,
    skip_source_comment: bool,
) -> Result<(), String> {
    use usd::sdf::Layer;
    use usd::usd::{InitialLoadSet, Stage};

    if flatten {
        let stage = Stage::open(input, InitialLoadSet::LoadAll)
            .map_err(|e| format!("failed to open stage: {}", e))?;

        if load_only {
            return Ok(());
        }

        if let Some(out_path) = output {
            stage
                .export(out_path, !skip_source_comment)
                .map_err(|e| format!("failed to export: {}", e))?;
        } else {
            let text = stage
                .export_to_string(!skip_source_comment)
                .map_err(|e| format!("failed to export: {}", e))?;
            print!("{}", text);
        }
    } else if flatten_layer_stack {
        let stage = Stage::open(input, InitialLoadSet::LoadNone)
            .map_err(|e| format!("failed to open stage: {}", e))?;

        if load_only {
            return Ok(());
        }

        let flattened = stage
            .flatten(!skip_source_comment)
            .map_err(|e| format!("failed to flatten layer stack: {}", e))?;

        if let Some(out_path) = output {
            flattened
                .export(out_path)
                .map_err(|e| format!("failed to export: {}", e))?;
        } else {
            let text = flattened
                .export_to_string()
                .map_err(|e| format!("failed to export: {}", e))?;
            print!("{}", text);
        }
    } else if layer_metadata {
        let layer =
            Layer::open_as_anonymous(input).map_err(|e| format!("failed to open layer: {}", e))?;

        if load_only {
            return Ok(());
        }

        if let Some(out_path) = output {
            layer
                .export(out_path)
                .map_err(|e| format!("failed to export: {}", e))?;
        } else {
            let text = layer
                .export_to_string()
                .map_err(|e| format!("failed to export: {}", e))?;
            print!("{}", text);
        }
    } else {
        let layer =
            Layer::find_or_open(input).map_err(|e| format!("failed to open layer: {}", e))?;

        if load_only {
            return Ok(());
        }

        if let Some(out_path) = output {
            layer
                .export(out_path)
                .map_err(|e| format!("failed to export: {}", e))?;
        } else {
            let text = layer
                .export_to_string()
                .map_err(|e| format!("failed to export: {}", e))?;
            print!("{}", text);
        }
    }

    Ok(())
}
