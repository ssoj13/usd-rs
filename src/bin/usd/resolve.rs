//! usdresolve - Test USD asset resolution
//!
//! Port of pxr/usd/bin/usdresolve/usdresolve.py
//!
//! Resolves an asset path using the configured USD Asset Resolver.

/// Run the resolve command
pub fn run(args: &[String]) -> i32 {
    let result = run_impl(args);
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

fn run_impl(args: &[String]) -> Result<(), String> {
    let mut input_path: Option<String> = None;
    let mut anchor_path: Option<String> = None;
    let mut context_asset: Option<String> = None;
    let mut context_strings: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "--anchorPath" => {
                i += 1;
                if i >= args.len() {
                    return Err("--anchorPath requires an argument".to_string());
                }
                anchor_path = Some(args[i].clone());
            }
            "--createContextForAsset" => {
                i += 1;
                if i >= args.len() {
                    return Err("--createContextForAsset requires an argument".to_string());
                }
                context_asset = Some(args[i].clone());
            }
            "--createContextFromString" => {
                i += 1;
                if i >= args.len() {
                    return Err("--createContextFromString requires an argument".to_string());
                }
                context_strings.push(args[i].clone());
            }
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                if input_path.is_some() {
                    return Err("Only one input path expected".to_string());
                }
                input_path = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let input = input_path.ok_or("No input path specified")?;

    // Get resolver
    use usd::ar::{DefaultResolver, Resolver};

    let resolver = DefaultResolver::new();

    // Create context
    let context = if let Some(asset) = context_asset {
        resolver.create_default_context_for_asset(&asset)
    } else if !context_strings.is_empty() {
        // Parse context strings (format: [scheme:]config)
        let configs: Vec<(String, String)> = context_strings
            .iter()
            .map(|s| {
                if let Some(idx) = s.find(':') {
                    (s[..idx].to_string(), s[idx + 1..].to_string())
                } else {
                    (String::new(), s.clone())
                }
            })
            .collect();
        resolver.create_context_from_strings(&configs)
    } else {
        resolver.create_default_context_for_asset(&input)
    };

    // Apply anchor path if specified
    let path_to_resolve = if let Some(anchor) = anchor_path {
        let resolved_anchor = usd::ar::ResolvedPath::new(&anchor);
        resolver.create_identifier(&input, Some(&resolved_anchor))
    } else {
        input.clone()
    };

    // Resolve
    let _binder = usd::ar::ResolverContextBinder::new(context);
    let resolved = resolver.resolve(&path_to_resolve);

    if resolved.is_empty() {
        Err(format!("Failed to resolve '{}'", input))
    } else {
        println!("{}", resolved);
        Ok(())
    }
}

fn print_help() {
    println!(
        r#"usdresolve - Resolve an asset path using the USD Asset Resolver

USAGE:
    usd resolve [options] <inputPath>

ARGUMENTS:
    <inputPath>    An asset path to be resolved

OPTIONS:
    -h, --help                          Show this help
    --anchorPath <path>                 Create identifier anchored to this path
    --createContextForAsset <asset>     Create context for this asset
    --createContextFromString <str>     Create context from string ([scheme:]config)

EXAMPLES:
    # Resolve a relative path
    usd resolve ./textures/diffuse.png

    # Resolve with anchor
    usd resolve --anchorPath /assets/model.usd material.usda

    # Resolve with custom context
    usd resolve --createContextForAsset /project/shot.usd asset.usd
"#
    );
}
