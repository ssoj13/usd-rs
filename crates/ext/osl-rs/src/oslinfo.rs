//! oslinfo — CLI utility for inspecting compiled .oso shader files.
//!
//! Port of `oslinfo.cpp`. Reads an `.oso` file and prints shader
//! parameter information, metadata, and structure.

use std::path::Path;

use crate::oslquery::OslQuery;

/// Output format for oslinfo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output.
    Text,
    /// JSON output.
    Json,
}

/// Options for oslinfo output.
#[derive(Debug, Clone)]
pub struct OslInfoOptions {
    pub format: OutputFormat,
    pub verbose: bool,
    pub print_params: bool,
    pub print_metadata: bool,
}

impl Default for OslInfoOptions {
    fn default() -> Self {
        Self {
            format: OutputFormat::Text,
            verbose: false,
            print_params: true,
            print_metadata: true,
        }
    }
}

/// Get shader info as a formatted string.
pub fn shader_info_string(oso_text: &str, opts: &OslInfoOptions) -> Result<String, String> {
    let mut query = OslQuery::new();
    if !query.open_bytecode(oso_text) {
        return Err("Failed to parse OSO bytecode".to_string());
    }

    match opts.format {
        OutputFormat::Text => format_text(&query, opts),
        OutputFormat::Json => format_json(&query, opts),
    }
}

/// Get shader info from a file.
pub fn shader_info_file(path: &Path, opts: &OslInfoOptions) -> Result<String, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read '{}': {}", path.display(), e))?;
    shader_info_string(&contents, opts)
}

fn format_text(query: &OslQuery, opts: &OslInfoOptions) -> Result<String, String> {
    let mut out = String::new();

    out.push_str(&format!(
        "shader \"{}\" (type: {})\n",
        query.shader_name().as_str(),
        query.shader_type_name().as_str(),
    ));
    out.push_str(&format!("  {} parameters\n", query.nparams()));

    if opts.print_params {
        for (i, p) in query.iter().enumerate() {
            let dir = if p.is_output { "output" } else { "" };
            out.push_str(&format!(
                "  [{:2}] {} {} {}\n",
                i,
                dir,
                p.type_desc,
                p.name.as_str()
            ));

            if opts.verbose {
                if !p.idefault.is_empty() {
                    out.push_str(&format!("       default int: {:?}\n", p.idefault));
                }
                if !p.fdefault.is_empty() {
                    out.push_str(&format!("       default float: {:?}\n", p.fdefault));
                }
                if !p.sdefault.is_empty() {
                    let strs: Vec<_> = p.sdefault.iter().map(|s| s.as_str()).collect();
                    out.push_str(&format!("       default string: {:?}\n", strs));
                }
                if p.valid_default {
                    out.push_str("       has valid default\n");
                }
                if p.is_closure {
                    out.push_str("       closure\n");
                }
                if p.is_struct {
                    out.push_str(&format!("       struct: {}\n", p.structname.as_str()));
                }

                if opts.print_metadata && !p.metadata.is_empty() {
                    out.push_str("       metadata:\n");
                    for m in &p.metadata {
                        out.push_str(&format!("         {} {}", m.type_desc, m.name.as_str()));
                        if !m.sdefault.is_empty() {
                            let strs: Vec<_> = m.sdefault.iter().map(|s| s.as_str()).collect();
                            out.push_str(&format!(" = {:?}", strs));
                        }
                        out.push('\n');
                    }
                }
            }
        }
    }

    Ok(out)
}

fn format_json(query: &OslQuery, _opts: &OslInfoOptions) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!(
        "  \"name\": \"{}\",\n",
        query.shader_name().as_str()
    ));
    out.push_str(&format!(
        "  \"type\": \"{}\",\n",
        query.shader_type_name().as_str()
    ));
    out.push_str(&format!("  \"nparams\": {},\n", query.nparams()));
    out.push_str("  \"parameters\": [\n");

    let nparams = query.nparams();
    for (i, p) in query.iter().enumerate() {
        out.push_str("    {\n");
        out.push_str(&format!("      \"name\": \"{}\",\n", p.name.as_str()));
        out.push_str(&format!("      \"type\": \"{}\",\n", p.type_desc));
        out.push_str(&format!("      \"isoutput\": {},\n", p.is_output));
        out.push_str(&format!("      \"validdefault\": {},\n", p.valid_default));
        out.push_str(&format!("      \"isclosure\": {}\n", p.is_closure));
        out.push_str("    }");
        if i + 1 < nparams {
            out.push(',');
        }
        out.push('\n');
    }

    out.push_str("  ]\n");
    out.push_str("}\n");
    Ok(out)
}

/// Parse command-line arguments for oslinfo.
pub fn parse_args(args: &[String]) -> Result<(String, OslInfoOptions), String> {
    let mut opts = OslInfoOptions::default();
    let mut input = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-v" | "--verbose" => opts.verbose = true,
            "--json" => opts.format = OutputFormat::Json,
            "--no-params" => opts.print_params = false,
            "--no-metadata" => opts.print_metadata = false,
            s if !s.starts_with('-') => {
                input = Some(s.to_string());
            }
            other => {
                return Err(format!("Unknown option: {}", other));
            }
        }
        i += 1;
    }

    match input {
        Some(path) => Ok((path, opts)),
        None => Err("No input file specified".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_basic() {
        let args: Vec<String> = vec!["test.oso", "-v"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let (path, opts) = parse_args(&args).unwrap();
        assert_eq!(path, "test.oso");
        assert!(opts.verbose);
    }

    #[test]
    fn test_parse_args_json() {
        let args: Vec<String> = vec!["test.oso", "--json"]
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        let (_, opts) = parse_args(&args).unwrap();
        assert_eq!(opts.format, OutputFormat::Json);
    }

    #[test]
    fn test_parse_args_no_input() {
        let args: Vec<String> = vec!["-v"].into_iter().map(|s| s.to_string()).collect();
        assert!(parse_args(&args).is_err());
    }
}
