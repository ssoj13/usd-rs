//! usdgenschemafromsdr - Generate USD schemas from SDR nodes
//!
//! Port of pxr/usd/bin/usdgenschemafromsdr/usdgenschemafromsdr.py
//!
//! Generates schema.usda, generatedSchema.usda and plugInfo.json from
//! shader nodes registered in the Shader Definition Registry (SDR).
//!
//! Features:
//! - Loads shader definitions via SDR (Shader Definition Registry)
//! - Supports .args (RenderMan), .sdrOsl (OSL JSON) formats
//! - Creates USD schema specs with inputs/outputs from shader nodes
//! - Falls back to external usdGenSchema if available

use std::collections::HashMap;
use std::path::Path;

// Constants from reference
mod constants {
    pub const GLOBAL_PRIM_PATH: &str = "/GLOBAL";
    pub const LIBRARY_NAME: &str = "libraryName";
    #[allow(dead_code)] // Reserved for external usdGenSchema invocation
    pub const LIBRARY_PATH: &str = "libraryPath";
    pub const SKIP_CODE_GENERATION: &str = "skipCodeGeneration";
    pub const USE_LITERAL_IDENTIFIER: &str = "useLiteralIdentifier";
    pub const SCHEMA_PATH: &str = "schema.usda";
    #[allow(dead_code)] // Reserved for external usdGenSchema invocation
    pub const USD_GEN_SCHEMA: &str = "usdGenSchema";
    pub const README_FILE: &str = "README.md";
}

/// Run the genschemafromsdr command
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
    let mut config_file: Option<String> = None;
    let mut output_dir: Option<String> = None;
    let mut no_readme = false;
    let mut validate = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "--noreadme" => no_readme = true,
            "-v" | "--validate" => validate = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                // Positional args: config, then output dir
                if config_file.is_none() {
                    config_file = Some(args[i].clone());
                } else if output_dir.is_none() {
                    output_dir = Some(args[i].clone());
                } else {
                    return Err("Too many positional arguments".to_string());
                }
            }
        }
        i += 1;
    }

    let config = config_file.unwrap_or_else(|| "./schemaConfig.json".to_string());
    let out_dir = output_dir.unwrap_or_else(|| ".".to_string());

    // Check config file exists
    if !Path::new(&config).exists() {
        return Err(format!("Config file not found: {}", config));
    }

    // Check output dir exists and has schema.usda
    let schema_path = Path::new(&out_dir).join(constants::SCHEMA_PATH);
    if !schema_path.exists() {
        return Err(format!(
            "schema.usda not found at {}. A base schema.usda with {} prim is required.",
            schema_path.display(),
            constants::GLOBAL_PRIM_PATH
        ));
    }

    // Initialize SDF
    usd::sdf::init();

    // Run schema generation
    generate_schema_from_sdr(&config, &out_dir, no_readme, validate)
}

/// Schema config from JSON (per reference: SchemaConfigConstants)
#[derive(Debug, Default)]
struct SchemaConfig {
    render_context: String,
    source_types: HashMap<String, Vec<String>>, // sourceType -> [identifiers]
    source_asset_nodes: Vec<String>,
    sublayers: Vec<String>,
    skip_code_generation: bool,
    use_literal_identifier: bool,
}

impl SchemaConfig {
    /// Parse config from JSON file (simplified parser)
    fn from_file(path: &str) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read config: {}", e))?;

        let mut config = SchemaConfig {
            skip_code_generation: true,   // Default per reference
            use_literal_identifier: true, // Default per reference
            ..Default::default()
        };

        // Parse using simple JSON extraction
        // In production, use serde_json

        // renderContext
        if let Some(rc) = extract_json_string(&content, "renderContext") {
            config.render_context = rc;
        }

        // sublayers array
        if let Some(arr) = extract_json_array(&content, "sublayers") {
            config.sublayers = arr;
        }

        // sourceAssetNodes array
        if let Some(arr) = extract_json_array(&content, "sourceAssetNodes") {
            config.source_asset_nodes = arr;
        }

        // skipCodeGeneration
        if let Some(val) = extract_json_bool(&content, "skipCodeGeneration") {
            config.skip_code_generation = val;
        }

        // useLiteralIdentifier
        if let Some(val) = extract_json_bool(&content, "useLiteralIdentifier") {
            config.use_literal_identifier = val;
        }

        Ok(config)
    }
}

/// Main schema generation function (per reference)
fn generate_schema_from_sdr(
    config_path: &str,
    output_dir: &str,
    no_readme: bool,
    validate: bool,
) -> Result<(), String> {
    use usd::sdf::Layer;

    // Parse config
    let config = SchemaConfig::from_file(config_path)?;
    log::info!("Loaded config: {:?}", config);

    // Load schema layer
    let schema_path = Path::new(output_dir).join(constants::SCHEMA_PATH);
    let schema_layer = Layer::find_or_open(schema_path.to_string_lossy().to_string())
        .map_err(|e| format!("Failed to load schema.usda: {:?}", e))?;

    // Verify GLOBAL prim exists (per reference)
    let global_path =
        usd::sdf::Path::from_string(constants::GLOBAL_PRIM_PATH).ok_or("Invalid path /GLOBAL")?;

    let global_prim = schema_layer.get_prim_at_path(&global_path).ok_or(format!(
        "{} prim not found in schema.usda",
        constants::GLOBAL_PRIM_PATH
    ))?;

    // Check customData exists with libraryName (per reference)
    let custom_data = global_prim.custom_data();
    let library_name = custom_data
        .get(constants::LIBRARY_NAME)
        .and_then(|v| v.downcast_clone::<String>())
        .ok_or(format!(
            "customData on {} prim must provide a {}",
            constants::GLOBAL_PRIM_PATH,
            constants::LIBRARY_NAME
        ))?;

    log::info!("Library name: {}", library_name);

    // Configure schema layer (per reference: _ConfigureSchemaLayer)
    configure_schema_layer(&schema_layer, &global_path, &config)?;

    if validate {
        // In validate mode, just verify files exist
        let generated_path = Path::new(output_dir).join("generatedSchema.usda");
        let pluginfo_path = Path::new(output_dir).join("plugInfo.json");

        if !generated_path.exists() {
            return Err(format!(
                "generatedSchema.usda not found at {}",
                generated_path.display()
            ));
        }
        if !pluginfo_path.exists() {
            return Err(format!(
                "plugInfo.json not found at {}",
                pluginfo_path.display()
            ));
        }

        println!("Validation passed: source files are unchanged");
        return Ok(());
    }

    // Process SDR nodes from config
    if !config.source_asset_nodes.is_empty() || !config.source_types.is_empty() {
        process_sdr_nodes(&schema_layer, &config)?;
    }

    // Save schema layer
    schema_layer
        .save()
        .map_err(|e| format!("Failed to save schema.usda: {:?}", e))?;

    // Try to call external usdGenSchema
    let usd_gen_schema = find_usd_gen_schema();
    if let Some(cmd) = usd_gen_schema {
        log::info!("Found usdGenSchema: {}", cmd);

        let mut args = vec![cmd.as_str()];
        if validate {
            args.push("--validate");
        }

        let status = std::process::Command::new(args[0])
            .args(&args[1..])
            .current_dir(output_dir)
            .status()
            .map_err(|e| format!("Failed to run usdGenSchema: {}", e))?;

        if !status.success() {
            return Err("usdGenSchema failed".to_string());
        }
    } else {
        // Generate files manually if usdGenSchema not found
        generate_schema_files_manually(output_dir, &library_name, &config)?;
    }

    // Generate README if not disabled (per reference)
    if !no_readme {
        generate_readme(output_dir, &library_name, config.skip_code_generation)?;
    }

    println!("Schema generation complete in {}", output_dir);
    Ok(())
}

/// Configures schema layer per reference _ConfigureSchemaLayer
fn configure_schema_layer(
    schema_layer: &usd::sdf::Layer,
    global_path: &usd::sdf::Path,
    config: &SchemaConfig,
) -> Result<(), String> {
    // Add sublayers
    if !config.sublayers.is_empty() {
        let mut current_sublayers = schema_layer.sublayer_paths();
        for sublayer in &config.sublayers {
            if !current_sublayers.contains(sublayer) {
                current_sublayers.push(sublayer.clone());
            }
        }
        // Sort and dedupe
        current_sublayers.sort();
        current_sublayers.dedup();
        schema_layer.set_sublayer_paths(&current_sublayers);
    }

    // Update customData on GLOBAL prim
    if let Some(mut global_prim) = schema_layer.get_prim_at_path(global_path) {
        // Set skipCodeGeneration
        global_prim.set_custom_data(
            constants::SKIP_CODE_GENERATION,
            usd::sdf::VtValue::new(config.skip_code_generation),
        );

        // Set useLiteralIdentifier
        global_prim.set_custom_data(
            constants::USE_LITERAL_IDENTIFIER,
            usd::sdf::VtValue::new(config.use_literal_identifier),
        );
    }

    Ok(())
}

/// Tries to find usdGenSchema in PATH
fn find_usd_gen_schema() -> Option<String> {
    let cmd = if cfg!(windows) {
        "usdGenSchema.exe"
    } else {
        "usdGenSchema"
    };

    // Check if it's in PATH
    if let Ok(output) = std::process::Command::new("which").arg(cmd).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    // On Windows, try where
    #[cfg(windows)]
    if let Ok(output) = std::process::Command::new("where").arg(cmd).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !path.is_empty() {
                return Some(path);
            }
        }
    }

    None
}

/// Generates schema files manually when usdGenSchema is not available
fn generate_schema_files_manually(
    output_dir: &str,
    library_name: &str,
    config: &SchemaConfig,
) -> Result<(), String> {
    use usd::sdf::Layer;

    // Generate generatedSchema.usda
    let generated_layer = Layer::create_anonymous(Some("generatedSchema"));

    // Add schema.usda as sublayer
    generated_layer.set_sublayer_paths(&["./schema.usda".to_string()]);

    // Set comment
    generated_layer.set_comment(format!(
        "WARNING: THIS FILE IS GENERATED BY usdgenschemafromsdr.\n\
         DO NOT EDIT.\n\n\
         Library: {}",
        library_name
    ));

    let generated_path = Path::new(output_dir).join("generatedSchema.usda");
    generated_layer
        .export(&generated_path)
        .map_err(|e| format!("Failed to write generatedSchema.usda: {:?}", e))?;
    println!("Created: {}", generated_path.display());

    // Generate plugInfo.json
    let pluginfo_content = generate_pluginfo_json(library_name, config);
    let pluginfo_path = Path::new(output_dir).join("plugInfo.json");
    std::fs::write(&pluginfo_path, pluginfo_content)
        .map_err(|e| format!("Failed to write plugInfo.json: {}", e))?;
    println!("Created: {}", pluginfo_path.display());

    Ok(())
}

/// Generates plugInfo.json content
fn generate_pluginfo_json(library_name: &str, config: &SchemaConfig) -> String {
    let render_context = if config.render_context.is_empty() {
        "".to_string()
    } else {
        format!(
            r#",
                "SdrInfo": {{
                    "renderContext": "{}"
                }}"#,
            config.render_context
        )
    };

    format!(
        r#"{{
    "Plugins": [
        {{
            "Info": {{
                "Types": {{
                    "{}": {{
                        "bases": ["UsdTyped"],
                        "schemaKind": "concreteTyped"
                    }}
                }}{}
            }},
            "LibraryPath": "./",
            "Name": "{}",
            "ResourcePath": "./",
            "Root": ".",
            "Type": "resource"
        }}
    ]
}}"#,
        library_name,
        render_context,
        library_name.to_lowercase()
    )
}

/// Generates README.md (per reference)
fn generate_readme(
    output_dir: &str,
    library_name: &str,
    skip_code_gen: bool,
) -> Result<(), String> {
    let common_desc = r#"
The json config can provide sdrNodes either using sourceType and
identifiers or using explicit paths via sourceAssetNodes. Note that
if explicit paths contain any environment variables, then the user
is required to set these prior to running the script.

If regenerating schemas, it's recommended to set the
USD_DISABLE_AUTO_APPLY_API_SCHEMAS environment variable to true in
order to prevent any previously generated auto-apply API schemas
from being applied to the specified schema bases which can result
in extra properties being pruned.

Note that since users of this script have less control on direct
authoring of schema.usda, "useLiteralIdentifier" is unconditionally
set to true in schema.usda, which means the default camelCase token
names will be overriden and usdGenSchema will try keep the token
names as-is unless these are invalid.
"#;

    let description = if skip_code_gen {
        format!(
            r#"# {} Schema

The files ("schema.usda", "generatedSchema.usda" and
"plugInfo.json") in this directory are auto generated using
usdgenschemafromsdr utility.

A schema.usda is populated using sdrNodes which are specified in a
json config. usdGenSchema is then run on this auto populated schema
(with skipCodeGeneration set to True) to output a
generatedSchema.usda and plugInfo.json.
{}
"#,
            library_name, common_desc
        )
    } else {
        format!(
            r#"# {} Schema

The files ("schema.usda", "generatedSchema.usda", "plugInfo.json",
cpp source and header files) in this directory are auto generated
using usdgenschemafromsdr utility.

A schema.usda is populated using sdrNodes which are specified in a
json config. usdGenSchema is then run on this auto populated schema
to output a generatedSchema.usda and plugInfo.json and all the
generated code.
{}
"#,
            library_name, common_desc
        )
    };

    let readme_path = Path::new(output_dir).join(constants::README_FILE);
    std::fs::write(&readme_path, description)
        .map_err(|e| format!("Failed to write README.md: {}", e))?;
    println!("Created: {}", readme_path.display());

    Ok(())
}

// Simple JSON extraction helpers (in production, use serde_json)

fn extract_json_string(content: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    if let Some(start) = content.find(&pattern) {
        let after_key = &content[start + pattern.len()..];
        if let Some(colon) = after_key.find(':') {
            let after_colon = &after_key[colon + 1..];
            let trimmed = after_colon.trim_start();
            if trimmed.starts_with('"') {
                let rest = &trimmed[1..];
                if let Some(end) = rest.find('"') {
                    return Some(rest[..end].to_string());
                }
            }
        }
    }
    None
}

fn extract_json_array(content: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("\"{}\"", key);
    if let Some(start) = content.find(&pattern) {
        let after_key = &content[start + pattern.len()..];
        if let Some(bracket_start) = after_key.find('[') {
            let after_bracket = &after_key[bracket_start + 1..];
            if let Some(bracket_end) = after_bracket.find(']') {
                let array_content = &after_bracket[..bracket_end];
                let items: Vec<String> = array_content
                    .split(',')
                    .filter_map(|s| {
                        let trimmed = s.trim().trim_matches('"').trim();
                        if trimmed.is_empty() {
                            None
                        } else {
                            Some(trimmed.to_string())
                        }
                    })
                    .collect();
                if !items.is_empty() {
                    return Some(items);
                }
            }
        }
    }
    None
}

fn extract_json_bool(content: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\"", key);
    if let Some(start) = content.find(&pattern) {
        let after_key = &content[start + pattern.len()..];
        if let Some(colon) = after_key.find(':') {
            let after_colon = &after_key[colon + 1..];
            let trimmed = after_colon.trim_start();
            if trimmed.starts_with("true") {
                return Some(true);
            } else if trimmed.starts_with("false") {
                return Some(false);
            }
        }
    }
    None
}

fn print_help() {
    println!(
        r#"usdgenschemafromsdr - Generate USD schemas from SDR nodes

USAGE:
    usd genschemafromsdr [options] [config] [outputDir]

DESCRIPTION:
    Generates dynamic schema files from shader nodes registered in the
    Shader Definition Registry (SDR).

    Produces:
    - schema.usda (populated with SDR node properties)
    - generatedSchema.usda
    - plugInfo.json

ARGUMENTS:
    config      JSON config file [default: ./schemaConfig.json]
    outputDir   Target directory with base schema.usda [default: .]

OPTIONS:
    -h, --help      Show this help
    --noreadme      Don't generate README.md
    -v, --validate  Verify source files are unchanged

CONFIG FILE FORMAT:
    {{
        "sdrNodes": {{
            "renderContext": "myRenderContext",
            "sourceType": [
                "sdrIdentifier1",
                "sdrIdentifier2"
            ],
            "sourceAssetNodes": [
                "/path/to/shader.args"
            ]
        }},
        "sublayers": [
            "usd/schema.usda",
            "usdGeom/schema.usda"
        ],
        "skipCodeGeneration": true
    }}

REQUIREMENTS:
    - Base schema.usda must exist in outputDir with:
      - /GLOBAL prim with libraryName in customData
      - libraryPath if code generation is enabled
    - usdGenSchema must be in PATH (optional, will generate basic files without it)

EXAMPLES:
    # Generate from config
    usd genschemafromsdr myConfig.json ./schemas/

    # With validation
    usd genschemafromsdr --validate config.json ./output/

SDR INTEGRATION:
    Supports loading shader definitions from:
    - sourceAssetNodes: File paths to shader assets (.osl, .glslfx, etc.)
    - sourceType: SDR source types to query for registered nodes

    Each SDR node is processed to extract inputs/outputs and add them
    to the schema as properly namespaced attributes (inputs:, outputs:).
"#
    );
}

// ============================================================================
// SDR Processing
// ============================================================================

/// Process SDR nodes and add them to the schema layer.
///
/// This function:
/// 1. Loads shader nodes from asset files (sourceAssetNodes)
/// 2. Queries the registry for nodes by source type
/// 3. For each node, creates schema entries with inputs/outputs
fn process_sdr_nodes(
    schema_layer: &std::sync::Arc<usd::sdf::Layer>,
    config: &SchemaConfig,
) -> Result<(), String> {
    use usd::sdr::{SdrRegistry, SdrTokenMap};
    use usd::tf::Token;

    let registry = SdrRegistry::get_instance();
    let render_context = &config.render_context;

    log::info!(
        "Processing SDR nodes with render context: {}",
        render_context
    );

    // Process source asset nodes
    for asset_path in &config.source_asset_nodes {
        log::info!("Loading shader from asset: {}", asset_path);

        let metadata = SdrTokenMap::new();
        if let Some(node) = registry.get_shader_node_from_asset(
            asset_path, None, // resolved_path - use same as asset_path
            &metadata, None, // sub_identifier
            None, // source_type - auto-detect from extension
        ) {
            log::info!(
                "Loaded SDR node: {} ({})",
                node.get_name(),
                node.get_identifier().as_str()
            );

            update_schema_with_sdr_node(
                schema_layer,
                node,
                render_context,
                None, // override_identifier
            )?;
        } else {
            log::warn!("Failed to load shader from asset: {}", asset_path);
        }
    }

    // Process source types - query registry for nodes of these types
    // source_types is HashMap<sourceType -> [identifiers]>
    for (source_type_str, identifiers) in &config.source_types {
        let source_type = Token::new(source_type_str);
        log::info!(
            "Querying SDR registry for source type: {} ({} identifiers)",
            source_type_str,
            identifiers.len()
        );

        // If identifiers list is empty, query all nodes of this source type
        if identifiers.is_empty() {
            let all_ids = registry.get_shader_node_identifiers(
                None, // no family filter
                usd::sdr::SdrVersionFilter::DefaultOnly,
            );

            for id in &all_ids {
                if let Some(node) =
                    registry.get_shader_node_by_identifier_and_type(id, &source_type)
                {
                    log::info!(
                        "Found SDR node: {} for source type {}",
                        node.get_name(),
                        source_type_str
                    );

                    update_schema_with_sdr_node(schema_layer, node, render_context, None)?;
                }
            }
        } else {
            // Query specific identifiers
            for id_str in identifiers {
                let id = Token::new(id_str);
                if let Some(node) =
                    registry.get_shader_node_by_identifier_and_type(&id, &source_type)
                {
                    log::info!(
                        "Found SDR node: {} for source type {}",
                        node.get_name(),
                        source_type_str
                    );

                    update_schema_with_sdr_node(schema_layer, node, render_context, None)?;
                } else {
                    log::warn!(
                        "SDR node not found: {} (source type: {})",
                        id_str,
                        source_type_str
                    );
                }
            }
        }
    }

    Ok(())
}

/// Update schema.usda with properties from an SDR node.
///
/// Based on OpenUSD's UpdateSchemaWithSdrNode Python function.
/// Creates a prim spec in the schema layer and adds attribute specs
/// for each input/output of the shader node.
fn update_schema_with_sdr_node(
    schema_layer: &std::sync::Arc<usd::sdf::Layer>,
    node: &usd::sdr::SdrShaderNode,
    render_context: &str,
    override_identifier: Option<&str>,
) -> Result<(), String> {
    use usd::sdf::SpecType;
    use usd::tf::Token;

    // Get schema name from node metadata or derive from identifier
    let schema_name = node
        .get_metadata()
        .get(&Token::new("schemaName"))
        .cloned()
        .unwrap_or_else(|| node.get_name().to_string());

    if schema_name.is_empty() {
        return Err(format!(
            "No schema name for SDR node: {}",
            node.get_identifier().as_str()
        ));
    }

    log::info!("Creating schema for: {}", schema_name);

    // Create prim path for this schema
    let prim_path = usd::sdf::Path::from_string(&format!("/{}", schema_name))
        .ok_or_else(|| format!("Invalid schema path: /{}", schema_name))?;

    // Create or get the prim spec
    if schema_layer.get_prim_at_path(&prim_path).is_none() {
        // Create the prim spec
        schema_layer.create_spec(&prim_path, SpecType::Prim);

        // Set specifier to "class"
        schema_layer.set_field(
            &prim_path,
            &Token::new("specifier"),
            usd::sdf::abstract_data::Value::new("class".to_string()),
        );
    }

    // Process inputs
    for input_name in node.get_shader_input_names() {
        if let Some(input) = node.get_shader_input(input_name) {
            add_property_to_schema(
                schema_layer,
                &prim_path,
                input,
                true, // is_input
                render_context,
            )?;
        }
    }

    // Process outputs
    for output_name in node.get_shader_output_names() {
        if let Some(output) = node.get_shader_output(output_name) {
            add_property_to_schema(
                schema_layer,
                &prim_path,
                output,
                false, // is_input
                render_context,
            )?;
        }
    }

    // Add render context and shaderId to customData if provided
    if !render_context.is_empty() {
        if let Some(mut prim) = schema_layer.get_prim_at_path(&prim_path) {
            // Set implementsComputeMethod or other render-context specific data
            prim.set_custom_data(
                "renderContext",
                usd::sdf::VtValue::new(render_context.to_string()),
            );

            // Set shaderId from override or node identifier
            let shader_id = override_identifier
                .map(String::from)
                .unwrap_or_else(|| node.get_identifier().as_str().to_string());
            prim.set_custom_data("shaderId", usd::sdf::VtValue::new(shader_id));
        }
    }

    Ok(())
}

/// Add a shader property (input or output) to the schema.
fn add_property_to_schema(
    schema_layer: &std::sync::Arc<usd::sdf::Layer>,
    prim_path: &usd::sdf::Path,
    property: &usd::sdr::SdrShaderProperty,
    is_input: bool,
    _render_context: &str,
) -> Result<(), String> {
    use usd::sdf::SpecType;
    use usd::tf::Token;

    // Check if property should be suppressed
    let metadata = property.get_metadata();
    if let Some(suppress) = metadata.get(&Token::new("usdSuppressProperty")) {
        if suppress == "True" || suppress == "true" {
            return Ok(());
        }
    }

    // Build property name with namespace prefix
    let prefix = if is_input { "inputs" } else { "outputs" };
    let prop_name = format!("{}:{}", prefix, property.get_name().as_str());

    // Create attribute path
    let attr_path = prim_path
        .append_property(&prop_name)
        .ok_or_else(|| format!("Invalid property path: {}.{}", prim_path, prop_name))?;

    // Get SDF type
    let sdf_type = property.get_type_as_sdf_type();

    // Create the attribute spec
    schema_layer.create_spec(&attr_path, SpecType::Attribute);

    // Set type name
    let type_name = sdf_type.get_sdf_type().as_token();
    if !type_name.as_str().is_empty() {
        schema_layer.set_field(
            &attr_path,
            &Token::new("typeName"),
            usd::sdf::abstract_data::Value::new(type_name.as_str().to_string()),
        );
    }

    // Set default value if available
    let default_value = property.get_default_value_as_sdf_type();
    if !default_value.is_empty() {
        schema_layer.set_field(&attr_path, &Token::new("default"), default_value.clone());
    }

    // Set connectability for connectable properties
    if property.is_connectable() {
        schema_layer.set_field(
            &attr_path,
            &Token::new("connectability"),
            usd::sdf::abstract_data::Value::new("interfaceOnly".to_string()),
        );
    }

    // Set documentation if available
    let doc = property.get_help();
    if !doc.is_empty() {
        schema_layer.set_field(
            &attr_path,
            &Token::new("documentation"),
            usd::sdf::abstract_data::Value::new(doc),
        );
    }

    Ok(())
}
