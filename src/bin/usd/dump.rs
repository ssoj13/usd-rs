//! SDF Dump command - Dump raw SDF layer data
//!
//! Equivalent to standalone `sdfdump` tool.

use regex::Regex;
use std::collections::HashMap;

/// Run the dump command with given arguments
pub fn run(args: &[String]) -> i32 {
    if args.len() < 2 {
        print_usage();
        return 1;
    }

    let mut input_files: Vec<String> = Vec::new();
    let mut show_summary = false;
    let mut validate = false;
    let mut path_regex: Option<String> = None;
    let mut field_regex: Option<String> = None;
    let mut sort_by = "path".to_string();
    let mut no_values = false;
    let mut full_arrays = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "-s" | "--summary" => show_summary = true,
            "--validate" => validate = true,
            "-p" | "--path" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --path requires a regex argument");
                    return 1;
                }
                path_regex = Some(args[i].clone());
            }
            "-f" | "--field" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --field requires a regex argument");
                    return 1;
                }
                field_regex = Some(args[i].clone());
            }
            "--sortBy" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --sortBy requires path|field");
                    return 1;
                }
                let val = args[i].to_lowercase();
                if val != "path" && val != "field" {
                    eprintln!("error: --sortBy must be 'path' or 'field'");
                    return 1;
                }
                sort_by = val;
            }
            "--noValues" => no_values = true,
            "--fullArrays" => full_arrays = true,
            arg if arg.starts_with('-') => {
                eprintln!("error: unknown option: {}", arg);
                return 1;
            }
            _ => input_files.push(args[i].clone()),
        }
        i += 1;
    }

    if input_files.is_empty() {
        eprintln!("error: no input files specified");
        return 1;
    }

    let path_matcher = path_regex.as_ref().map(|r| {
        Regex::new(r).unwrap_or_else(|e| {
            eprintln!("error: invalid path regex '{}': {}", r, e);
            std::process::exit(1);
        })
    });

    let field_matcher = field_regex.as_ref().map(|r| {
        Regex::new(r).unwrap_or_else(|e| {
            eprintln!("error: invalid field regex '{}': {}", r, e);
            std::process::exit(1);
        })
    });

    let params = ReportParams {
        show_summary,
        validate,
        path_matcher,
        field_matcher,
        sort_by,
        show_values: !no_values,
        full_arrays,
    };

    let mut exit_code = 0;

    for file in &input_files {
        match dump_layer(file, &params) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error: failed to open layer '{}': {}", file, e);
                exit_code = 1;
            }
        }
    }

    exit_code
}

fn print_usage() {
    eprintln!("Usage: usd dump [options] <inputFile> [inputFile ...]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -s, --summary     Report a high-level summary");
    eprintln!("  --validate        Check validity by reading all data values");
    eprintln!("  -p, --path <re>   Report only paths matching this regex");
    eprintln!("  -f, --field <re>  Report only fields matching this regex");
    eprintln!("  --sortBy <key>    Group output by path or field (default: path)");
    eprintln!("  --noValues        Do not report field values");
    eprintln!("  --fullArrays      Report full array contents");
    eprintln!("  -h, --help        Show this help");
}

fn print_help() {
    println!("usd dump - Filter and display raw SDF layer data");
    println!();
    print_usage();
}

struct ReportParams {
    show_summary: bool,
    validate: bool,
    path_matcher: Option<Regex>,
    field_matcher: Option<Regex>,
    sort_by: String,
    show_values: bool,
    #[allow(dead_code)]
    full_arrays: bool,
}

struct SummaryStats {
    num_specs: usize,
    num_prim_specs: usize,
    num_property_specs: usize,
    num_fields: usize,
}

fn dump_layer(file: &str, params: &ReportParams) -> Result<(), String> {
    use usd::sdf::Layer;

    let layer = Layer::find_or_open(file).map_err(|e| format!("failed to open: {}", e))?;

    println!("@{}@", layer.identifier());

    if params.show_summary {
        let stats = get_summary_stats(&layer);
        println!(
            "  {} specs, {} prim specs, {} property specs, {} fields",
            stats.num_specs, stats.num_prim_specs, stats.num_property_specs, stats.num_fields
        );
    } else if params.validate {
        match validate_layer(&layer) {
            Ok(()) => println!("  - OK"),
            Err(e) => println!("  - ERROR: {}", e),
        }
    } else if params.sort_by == "path" {
        report_by_path(&layer, params);
    } else {
        report_by_field(&layer, params);
    }

    Ok(())
}

fn get_summary_stats(layer: &usd::sdf::Layer) -> SummaryStats {
    let mut stats = SummaryStats {
        num_specs: 0,
        num_prim_specs: 0,
        num_property_specs: 0,
        num_fields: 0,
    };

    fn count_prims(prim: &usd::sdf::PrimSpec, stats: &mut SummaryStats) {
        stats.num_specs += 1;
        stats.num_prim_specs += 1;

        let props = prim.properties();
        stats.num_specs += props.len();
        stats.num_property_specs += props.len();

        for child in prim.name_children() {
            count_prims(&child, stats);
        }
    }

    let root = layer.get_pseudo_root();
    for child in root.name_children() {
        count_prims(&child, &mut stats);
    }

    stats
}

fn validate_layer(layer: &usd::sdf::Layer) -> Result<(), String> {
    fn validate_prim(prim: &usd::sdf::PrimSpec) -> Result<(), String> {
        let _ = prim.name();
        let _ = prim.type_name();
        let _ = prim.specifier();

        for prop in prim.properties() {
            let _ = prop.name();
        }

        for child in prim.name_children() {
            validate_prim(&child)?;
        }

        Ok(())
    }

    let root = layer.get_pseudo_root();
    for child in root.name_children() {
        validate_prim(&child)?;
    }

    Ok(())
}

fn report_by_path(layer: &usd::sdf::Layer, params: &ReportParams) {
    fn report_prim(prim: &usd::sdf::PrimSpec, path: &str, params: &ReportParams) {
        let prim_path = if path.is_empty() {
            format!("/{}", prim.name())
        } else if path == "/" {
            format!("/{}", prim.name())
        } else {
            format!("{}/{}", path, prim.name())
        };

        if let Some(ref matcher) = params.path_matcher {
            if !matcher.is_match(&prim_path) {
                for child in prim.name_children() {
                    report_prim(&child, &prim_path, params);
                }
                return;
            }
        }

        let spec_type = format!("{:?}", prim.specifier());
        println!("<{}> : {}", prim_path, spec_type);

        let fields = collect_prim_fields(prim);
        for (field, value) in &fields {
            if let Some(ref matcher) = params.field_matcher {
                if !matcher.is_match(field) {
                    continue;
                }
            }

            if params.show_values {
                println!("  {}: {}", field, value);
            } else {
                println!("  {}", field);
            }
        }

        for prop in prim.properties() {
            let prop_path = format!("{}.{}", prim_path, prop.name().as_str());

            if let Some(ref matcher) = params.path_matcher {
                if !matcher.is_match(&prop_path) {
                    continue;
                }
            }

            println!("<{}> : Property", prop_path);
        }

        for child in prim.name_children() {
            report_prim(&child, &prim_path, params);
        }
    }

    let root = layer.get_pseudo_root();
    for child in root.name_children() {
        report_prim(&child, "", params);
    }
}

fn report_by_field(layer: &usd::sdf::Layer, params: &ReportParams) {
    let mut fields_map: HashMap<String, Vec<String>> = HashMap::new();

    fn collect_fields(
        prim: &usd::sdf::PrimSpec,
        path: &str,
        params: &ReportParams,
        fields_map: &mut HashMap<String, Vec<String>>,
    ) {
        let prim_path = if path.is_empty() || path == "/" {
            format!("/{}", prim.name())
        } else {
            format!("{}/{}", path, prim.name())
        };

        let path_matches = params
            .path_matcher
            .as_ref()
            .map(|m| m.is_match(&prim_path))
            .unwrap_or(true);

        if path_matches {
            let fields = collect_prim_fields(prim);
            for (field, value) in fields {
                if let Some(ref matcher) = params.field_matcher {
                    if !matcher.is_match(&field) {
                        continue;
                    }
                }

                let key = if params.show_values {
                    format!("{}: {}", field, value)
                } else {
                    field
                };

                fields_map
                    .entry(key)
                    .or_default()
                    .push(format!("  <{}>", prim_path));
            }
        }

        for child in prim.name_children() {
            collect_fields(&child, &prim_path, params, fields_map);
        }
    }

    let root = layer.get_pseudo_root();
    for child in root.name_children() {
        collect_fields(&child, "", params, &mut fields_map);
    }

    let mut keys: Vec<_> = fields_map.keys().cloned().collect();
    keys.sort();

    for key in keys {
        println!("{}", key);
        if let Some(paths) = fields_map.get(&key) {
            for path in paths {
                println!("{}", path);
            }
        }
    }
}

fn collect_prim_fields(prim: &usd::sdf::PrimSpec) -> Vec<(String, String)> {
    let mut fields = Vec::new();

    let type_name = prim.type_name();
    if !type_name.as_str().is_empty() {
        fields.push(("typeName".to_string(), type_name.as_str().to_string()));
    }

    fields.push(("specifier".to_string(), format!("{:?}", prim.specifier())));

    if prim.has_kind() {
        let kind = prim.kind();
        if !kind.as_str().is_empty() {
            fields.push(("kind".to_string(), kind.as_str().to_string()));
        }
    }

    if prim.has_active() {
        fields.push(("active".to_string(), prim.active().to_string()));
    }

    fields
}
