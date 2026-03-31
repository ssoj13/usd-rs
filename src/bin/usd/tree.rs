//! USD Tree command - Display prim hierarchy
//!
//! Equivalent to standalone `usdtree` tool.

/// Run the tree command with given arguments
pub fn run(args: &[String]) -> i32 {
    if args.len() < 2 {
        print_usage();
        return 1;
    }

    let mut input_path: Option<String> = None;
    let mut unloaded = false;
    let mut attributes = false;
    let mut metadata = false;
    let mut simple = false;
    let mut flatten = false;
    let mut flatten_layer_stack = false;
    let mut mask: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "--unloaded" => unloaded = true,
            "-a" | "--attributes" => attributes = true,
            "-m" | "--metadata" => metadata = true,
            "-s" | "--simple" => simple = true,
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
            arg if arg.starts_with('-') => {
                eprintln!("error: unknown option: {}", arg);
                return 1;
            }
            _ => {
                if input_path.is_some() {
                    eprintln!("error: only one input file expected");
                    return 1;
                }
                input_path = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input = match input_path {
        Some(p) => p,
        None => {
            eprintln!("error: no input file specified");
            return 1;
        }
    };

    if mask.is_some() && !flatten {
        eprintln!("error: --mask requires --flatten");
        return 1;
    }

    let opts = TreeOptions {
        unloaded,
        attributes,
        metadata,
        simple,
    };

    match run_tree(&input, &opts, flatten, flatten_layer_stack, mask.as_deref()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Failed to process '{}' - {}", input, e);
            1
        }
    }
}

fn print_usage() {
    eprintln!("Usage: usd tree [options] <inputFile>");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --unloaded           Do not load payloads");
    eprintln!("  -a, --attributes     Display authored attributes");
    eprintln!("  -m, --metadata       Display authored metadata");
    eprintln!("  -s, --simple         Only display prim names");
    eprintln!("  -f, --flatten        Compose and display flattened stage tree");
    eprintln!("  --flattenLayerStack  Flatten layer stack only");
    eprintln!("  --mask <paths>       Limit to these prim paths (requires --flatten)");
    eprintln!("  -h, --help           Show this help");
}

fn print_help() {
    println!("usd tree - Display USD file prim hierarchy as tree");
    println!();
    print_usage();
}

struct TreeOptions {
    #[allow(dead_code)]
    unloaded: bool,
    attributes: bool,
    metadata: bool,
    simple: bool,
}

fn run_tree(
    input: &str,
    opts: &TreeOptions,
    flatten: bool,
    flatten_layer_stack: bool,
    mask: Option<&str>,
) -> Result<(), String> {
    use usd::sdf::{Layer, Path};
    use usd::usd::{InitialLoadSet, Stage, StagePopulationMask};

    if flatten {
        let load_set = if opts.unloaded {
            InitialLoadSet::LoadNone
        } else {
            InitialLoadSet::LoadAll
        };

        let stage = if let Some(mask_str) = mask {
            let paths: Vec<Path> = mask_str
                .split(&[',', ' '][..])
                .filter(|s| !s.is_empty())
                .filter_map(Path::from_string)
                .collect();
            let population_mask = StagePopulationMask::from_paths(paths);
            Stage::open_masked(input, population_mask, load_set)
                .map_err(|e| format!("failed to open stage: {}", e))?
        } else {
            Stage::open(input, load_set).map_err(|e| format!("failed to open stage: {}", e))?
        };

        println!("/");
        let pseudo_root = stage.get_pseudo_root();
        print_stage_children(opts, &pseudo_root, "");
    } else if flatten_layer_stack {
        let stage = Stage::open(input, InitialLoadSet::LoadNone)
            .map_err(|e| format!("failed to open stage: {}", e))?;

        let layer = stage
            .flatten(false)
            .map_err(|e| format!("failed to flatten: {}", e))?;

        println!("/");
        print_layer_children(opts, &layer, "");
    } else {
        let layer =
            Layer::find_or_open(input).map_err(|e| format!("failed to open layer: {}", e))?;

        println!("/");
        print_layer_children(opts, &layer, "");
    }

    Ok(())
}

fn print_stage_children(opts: &TreeOptions, prim: &usd::usd::Prim, prefix: &str) {
    let children = prim.get_all_children();
    let count = children.len();

    for (i, child) in children.iter().enumerate() {
        let is_last = i == count - 1;
        print_stage_prim(opts, child, prefix, is_last);

        let new_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{} |  ", prefix)
        };
        print_stage_children(opts, child, &new_prefix);
    }
}

fn print_stage_prim(opts: &TreeOptions, prim: &usd::usd::Prim, prefix: &str, is_last: bool) {
    let has_children = !prim.get_all_children().is_empty();

    let (last_step, attr_step) = if !is_last {
        (" |--", if has_children { " |   |" } else { " |    " })
    } else {
        (" `--", if has_children { "     |" } else { "      " })
    };

    let label = if opts.simple {
        prim.name().as_str().to_string()
    } else {
        get_stage_prim_label(prim)
    };

    println!("{}{}{}", prefix, last_step, label);

    let mut attrs = Vec::new();

    if opts.metadata {
        if let Some(kind) = prim.get_kind() {
            if !kind.as_str().is_empty() {
                attrs.push(format!("(kind = {})", kind.as_str()));
            }
        }
    }

    if opts.attributes {
        let mut prop_names = prim.get_authored_property_names();
        prop_names.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        for prop_name in prop_names {
            attrs.push(format!(".{}", prop_name.as_str()));
        }
    }

    for (i, attr) in attrs.iter().enumerate() {
        let connector = if i < attrs.len() - 1 { " :--" } else { " `--" };
        println!("{}{}{}{}", prefix, attr_step, connector, attr);
    }
}

fn get_stage_prim_label(prim: &usd::usd::Prim) -> String {
    let spec = format!("{:?}", prim.specifier()).to_lowercase();
    let type_name = prim.type_name();
    let type_str = type_name.as_str();

    let definition = if type_str.is_empty() {
        spec
    } else {
        format!("{} {}", spec, type_str)
    };

    let mut label = format!("{} [{}]", prim.name().as_str(), definition);

    let mut short_meta = Vec::new();

    if !prim.is_active() {
        short_meta.push("active = false".to_string());
    }

    if let Some(kind) = prim.get_kind() {
        let kind_str = kind.as_str();
        if !kind_str.is_empty() {
            short_meta.push(format!("kind = {}", kind_str));
        }
    }

    if !short_meta.is_empty() {
        label.push_str(&format!(" ({})", short_meta.join(", ")));
    }

    label
}

fn print_layer_children(opts: &TreeOptions, layer: &usd::sdf::Layer, prefix: &str) {
    let root = layer.get_pseudo_root();
    print_prim_spec_children(opts, &root, prefix);
}

fn print_prim_spec_children(opts: &TreeOptions, prim: &usd::sdf::PrimSpec, prefix: &str) {
    let children = prim.name_children();
    let count = children.len();

    for (i, child) in children.iter().enumerate() {
        let is_last = i == count - 1;
        print_prim_spec(opts, child, prefix, is_last);

        let new_prefix = if is_last {
            format!("{}    ", prefix)
        } else {
            format!("{} |  ", prefix)
        };
        print_prim_spec_children(opts, child, &new_prefix);
    }
}

fn print_prim_spec(opts: &TreeOptions, prim: &usd::sdf::PrimSpec, prefix: &str, is_last: bool) {
    let has_children = !prim.name_children().is_empty();

    let (last_step, attr_step) = if !is_last {
        (" |--", if has_children { " |   |" } else { " |    " })
    } else {
        (" `--", if has_children { "     |" } else { "      " })
    };

    let label = if opts.simple {
        prim.name()
    } else {
        get_prim_spec_label(prim)
    };

    println!("{}{}{}", prefix, last_step, label);

    let mut attrs = Vec::new();

    if opts.metadata {
        if prim.has_kind() {
            let kind = prim.kind();
            if !kind.as_str().is_empty() {
                attrs.push(format!("(kind = {})", kind.as_str()));
            }
        }
    }

    if opts.attributes {
        let props = prim.properties();
        for prop in props {
            attrs.push(format!(".{}", prop.name().as_str()));
        }
    }

    for (i, attr) in attrs.iter().enumerate() {
        let connector = if i < attrs.len() - 1 { " :--" } else { " `--" };
        println!("{}{}{}{}", prefix, attr_step, connector, attr);
    }
}

fn get_prim_spec_label(prim: &usd::sdf::PrimSpec) -> String {
    let spec = format!("{:?}", prim.specifier()).to_lowercase();
    let type_name = prim.type_name();
    let type_str = type_name.as_str();

    let definition = if type_str.is_empty() {
        spec
    } else {
        format!("{} {}", spec, type_str)
    };

    let mut label = format!("{} [{}]", prim.name(), definition);

    let mut short_meta = Vec::new();

    if prim.has_active() && !prim.active() {
        short_meta.push("active = false".to_string());
    }

    if prim.has_kind() {
        let kind = prim.kind();
        let kind_str = kind.as_str();
        if !kind_str.is_empty() {
            short_meta.push(format!("kind = {}", kind_str));
        }
    }

    if !short_meta.is_empty() {
        label.push_str(&format!(" ({})", short_meta.join(", ")));
    }

    label
}
