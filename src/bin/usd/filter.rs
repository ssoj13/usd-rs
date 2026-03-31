//! SDF Filter command - Filter and display SDF layer data
//!
//! Equivalent to standalone `sdffilter` tool.

use regex::Regex;
use std::collections::HashMap;

/// Run the filter command with given arguments
pub fn run(args: &[String]) -> i32 {
    if args.len() < 2 {
        print_usage();
        return 1;
    }

    let mut input_files: Vec<String> = Vec::new();
    let mut path_regex: Option<String> = None;
    let mut field_regex: Option<String> = None;
    let mut output_type = OutputType::Outline;
    let mut output_file: Option<String> = None;
    let mut output_format: Option<String> = None;
    let mut sort_by = "path".to_string();
    let mut no_values = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
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
            "-o" | "--out" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --out requires a file argument");
                    return 1;
                }
                output_file = Some(args[i].clone());
            }
            "--outputType" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --outputType requires an argument");
                    return 1;
                }
                output_type = match args[i].to_lowercase().as_str() {
                    "validity" => OutputType::Validity,
                    "summary" => OutputType::Summary,
                    "outline" => OutputType::Outline,
                    "pseudolayer" => OutputType::PseudoLayer,
                    "layer" => OutputType::Layer,
                    _ => {
                        eprintln!("error: unknown outputType '{}'", args[i]);
                        eprintln!("  valid: validity|summary|outline|pseudoLayer|layer");
                        return 1;
                    }
                };
            }
            "--outputFormat" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("error: --outputFormat requires an argument");
                    return 1;
                }
                output_format = Some(args[i].clone());
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

    if output_type == OutputType::Layer && output_file.is_some() && input_files.len() > 1 {
        eprintln!("error: must supply exactly one input file with '--outputType layer' and --out");
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

    let params = FilterParams {
        output_type,
        output_file,
        output_format,
        path_matcher,
        field_matcher,
        sort_by,
        show_values: !no_values,
    };

    let mut exit_code = 0;

    for file in &input_files {
        match filter_layer(file, &params) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("error: failed to process '{}': {}", file, e);
                exit_code = 1;
            }
        }
    }

    exit_code
}

fn print_usage() {
    eprintln!("Usage: usd filter [options] <inputFile> [inputFile ...]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -p, --path <re>         Report only paths matching this regex");
    eprintln!("  -f, --field <re>        Report only fields matching this regex");
    eprintln!("  -o, --out <file>        Direct output to file");
    eprintln!("  --outputType <type>     Output format:");
    eprintln!("                          validity|summary|outline|pseudoLayer|layer");
    eprintln!("  --outputFormat <fmt>    Format for 'layer' output (usda, usdc)");
    eprintln!("  --sortBy <key>          Group outline by path or field (default: path)");
    eprintln!("  --noValues              Do not report field values");
    eprintln!("  -h, --help              Show this help");
}

fn print_help() {
    println!("usd filter - Filter and display SDF layer data");
    println!();
    print_usage();
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputType {
    Validity,
    Summary,
    Outline,
    PseudoLayer,
    Layer,
}

struct FilterParams {
    output_type: OutputType,
    output_file: Option<String>,
    #[allow(dead_code)]
    output_format: Option<String>,
    path_matcher: Option<Regex>,
    field_matcher: Option<Regex>,
    sort_by: String,
    show_values: bool,
}

struct SummaryStats {
    num_specs: usize,
    num_prim_specs: usize,
    num_property_specs: usize,
    num_fields: usize,
}

fn filter_layer(file: &str, params: &FilterParams) -> Result<(), String> {
    use usd::sdf::Layer;

    let layer = Layer::find_or_open(file).map_err(|e| format!("failed to open: {}", e))?;

    match params.output_type {
        OutputType::Validity => match validate_layer(&layer) {
            Ok(()) => println!("@{}@ - OK", layer.identifier()),
            Err(e) => println!("@{}@ - ERROR: {}", layer.identifier(), e),
        },
        OutputType::Summary => {
            let stats = get_summary_stats(&layer);
            println!("@{}@", layer.identifier());
            println!(
                "  {} specs, {} prim specs, {} property specs, {} fields",
                stats.num_specs, stats.num_prim_specs, stats.num_property_specs, stats.num_fields
            );
        }
        OutputType::Outline => {
            println!("@{}@", layer.identifier());
            if params.sort_by == "path" {
                report_by_path(&layer, params);
            } else {
                report_by_field(&layer, params);
            }
        }
        OutputType::PseudoLayer => {
            println!("#sdffilter << from @{}@ >>", layer.identifier());
            output_pseudo_layer(&layer, params);
        }
        OutputType::Layer => {
            if let Some(ref out_path) = params.output_file {
                layer
                    .export(out_path)
                    .map_err(|e| format!("failed to export: {}", e))?;
                println!("Exported to {}", out_path);
            } else {
                let text = layer
                    .export_to_string()
                    .map_err(|e| format!("failed to export: {}", e))?;
                print!("{}", text);
            }
        }
    }

    Ok(())
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

        if !prim.type_name().as_str().is_empty() {
            stats.num_fields += 1;
        }
        stats.num_fields += 1;
        if prim.has_kind() {
            stats.num_fields += 1;
        }
        if prim.has_active() {
            stats.num_fields += 1;
        }

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

fn report_by_path(layer: &usd::sdf::Layer, params: &FilterParams) {
    fn report_prim(prim: &usd::sdf::PrimSpec, path: &str, params: &FilterParams) {
        let prim_path = if path.is_empty() || path == "/" {
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

fn report_by_field(layer: &usd::sdf::Layer, params: &FilterParams) {
    let mut fields_map: HashMap<String, Vec<String>> = HashMap::new();

    fn collect_fields(
        prim: &usd::sdf::PrimSpec,
        path: &str,
        params: &FilterParams,
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

fn output_pseudo_layer(layer: &usd::sdf::Layer, params: &FilterParams) {
    fn output_prim(prim: &usd::sdf::PrimSpec, indent: usize, params: &FilterParams) {
        let ind = "    ".repeat(indent);
        let spec = format!("{:?}", prim.specifier()).to_lowercase();
        let type_name = prim.type_name();

        if type_name.as_str().is_empty() {
            println!("{}{} \"{}\"", ind, spec, prim.name());
        } else {
            println!("{}{} {} \"{}\"", ind, spec, type_name.as_str(), prim.name());
        }
        println!("{}{{", ind);

        if prim.has_kind() && params.show_values {
            println!("{}    kind = \"{}\"", ind, prim.kind().as_str());
        }
        if prim.has_active() && !prim.active() && params.show_values {
            println!("{}    active = false", ind);
        }

        for prop in prim.properties() {
            let prop_name = prop.name();
            if let Some(ref matcher) = params.field_matcher {
                if !matcher.is_match(prop_name.as_str()) {
                    continue;
                }
            }
            println!("{}    custom ... {}", ind, prop_name.as_str());
        }

        for child in prim.name_children() {
            output_prim(&child, indent + 1, params);
        }

        println!("{}}}", ind);
    }

    let root = layer.get_pseudo_root();
    for child in root.name_children() {
        output_prim(&child, 0, params);
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
