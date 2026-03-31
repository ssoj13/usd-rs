//! usddiff - Compare USD files
//!
//! Port of pxr/usd/bin/usddiff/usddiff.py
//!
//! Compares two USD files and displays differences.

use std::path::Path;

/// Exit codes matching Python implementation
const NO_DIFF_FOUND: i32 = 0;
const DIFF_FOUND: i32 = 1;
const ERROR_EXIT: i32 = 2;

/// Run the diff command
pub fn run(args: &[String]) -> i32 {
    match run_impl(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("Error: {}", e);
            ERROR_EXIT
        }
    }
}

fn run_impl(args: &[String]) -> Result<i32, String> {
    let mut files: Vec<String> = Vec::new();
    let mut noeffect = false;
    let mut flatten = false;
    let mut brief = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(NO_DIFF_FOUND);
            }
            "-n" | "--noeffect" => noeffect = true,
            "-f" | "--flatten" => flatten = true,
            "-q" | "--brief" => brief = true,
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                files.push(args[i].clone());
            }
        }
        i += 1;
    }

    if files.len() != 2 {
        return Err("Exactly two files required for comparison".to_string());
    }

    let baseline = &files[0];
    let comparison = &files[1];

    // Check files exist
    if !Path::new(baseline).exists() {
        return Err(format!("File not found: {}", baseline));
    }
    if !Path::new(comparison).exists() {
        return Err(format!("File not found: {}", comparison));
    }

    run_diff(baseline, comparison, flatten, noeffect, brief)
}

fn run_diff(
    baseline: &str,
    comparison: &str,
    flatten: bool,
    _noeffect: bool,
    brief: bool,
) -> Result<i32, String> {
    use usd::sdf::Layer;
    use usd::usd::{InitialLoadSet, Stage};

    // Create temp files for comparison
    let baseline_content = if flatten {
        // Flatten stage and export
        let stage = Stage::open(baseline, InitialLoadSet::LoadAll)
            .map_err(|e| format!("Failed to open {}: {}", baseline, e))?;
        let flattened = stage
            .flatten(false)
            .map_err(|e| format!("Failed to flatten {}: {}", baseline, e))?;
        layer_to_string(&flattened)?
    } else {
        // Just read the layer
        let layer = Layer::find_or_open(baseline)
            .map_err(|e| format!("Failed to open {}: {}", baseline, e))?;
        layer_to_string(&layer)?
    };

    let comparison_content = if flatten {
        let stage = Stage::open(comparison, InitialLoadSet::LoadAll)
            .map_err(|e| format!("Failed to open {}: {}", comparison, e))?;
        let flattened = stage
            .flatten(false)
            .map_err(|e| format!("Failed to flatten {}: {}", comparison, e))?;
        layer_to_string(&flattened)?
    } else {
        let layer = Layer::find_or_open(comparison)
            .map_err(|e| format!("Failed to open {}: {}", comparison, e))?;
        layer_to_string(&layer)?
    };

    // Compare
    let baseline_lines: Vec<&str> = baseline_content.lines().collect();
    let comparison_lines: Vec<&str> = comparison_content.lines().collect();

    if baseline_lines == comparison_lines {
        return Ok(NO_DIFF_FOUND);
    }

    if brief {
        println!("Files {} and {} differ", baseline, comparison);
    } else {
        // Generate unified diff
        print_unified_diff(baseline, comparison, &baseline_lines, &comparison_lines);
    }

    Ok(DIFF_FOUND)
}

fn layer_to_string(layer: &usd::sdf::Layer) -> Result<String, String> {
    // Try to export to string directly
    layer
        .export_to_string()
        .map_err(|e| format!("Failed to export layer: {}", e))
}

fn print_unified_diff(
    baseline_name: &str,
    comparison_name: &str,
    baseline_lines: &[&str],
    comparison_lines: &[&str],
) {
    // Simple unified diff implementation
    println!("--- {}", baseline_name);
    println!("+++ {}", comparison_name);

    // Find differences using simple line-by-line comparison
    let max_len = baseline_lines.len().max(comparison_lines.len());
    let mut in_hunk = false;
    let mut hunk_start = 0;
    let mut hunk_lines: Vec<String> = Vec::new();

    for i in 0..max_len {
        let base_line = baseline_lines.get(i).copied();
        let comp_line = comparison_lines.get(i).copied();

        match (base_line, comp_line) {
            (Some(b), Some(c)) if b == c => {
                if in_hunk {
                    // Context line in hunk
                    hunk_lines.push(format!(" {}", b));
                }
            }
            (Some(b), Some(c)) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_start = i;
                    hunk_lines.clear();
                }
                hunk_lines.push(format!("-{}", b));
                hunk_lines.push(format!("+{}", c));
            }
            (Some(b), None) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_start = i;
                    hunk_lines.clear();
                }
                hunk_lines.push(format!("-{}", b));
            }
            (None, Some(c)) => {
                if !in_hunk {
                    in_hunk = true;
                    hunk_start = i;
                    hunk_lines.clear();
                }
                hunk_lines.push(format!("+{}", c));
            }
            (None, None) => break,
        }

        // Flush hunk if we've accumulated enough context
        if in_hunk && hunk_lines.len() > 6 {
            let context_count = hunk_lines
                .iter()
                .rev()
                .take(3)
                .filter(|l| l.starts_with(' '))
                .count();
            if context_count == 3 {
                // Print hunk
                println!(
                    "@@ -{},{} +{},{} @@",
                    hunk_start + 1,
                    hunk_lines.iter().filter(|l| !l.starts_with('+')).count(),
                    hunk_start + 1,
                    hunk_lines.iter().filter(|l| !l.starts_with('-')).count()
                );
                for line in &hunk_lines[..hunk_lines.len() - 3] {
                    println!("{}", line);
                }
                in_hunk = false;
                hunk_lines.clear();
            }
        }
    }

    // Print remaining hunk
    if in_hunk && !hunk_lines.is_empty() {
        println!(
            "@@ -{},{} +{},{} @@",
            hunk_start + 1,
            hunk_lines.iter().filter(|l| !l.starts_with('+')).count(),
            hunk_start + 1,
            hunk_lines.iter().filter(|l| !l.starts_with('-')).count()
        );
        for line in &hunk_lines {
            println!("{}", line);
        }
    }
}

fn print_help() {
    println!(
        r#"usddiff - Compare USD files

USAGE:
    usd diff [options] <file1> <file2>

ARGUMENTS:
    <file1>    First (baseline) file to compare
    <file2>    Second (comparison) file to compare

OPTIONS:
    -h, --help       Show this help
    -n, --noeffect   Do not edit either file (read-only diff)
    -f, --flatten    Flatten both files as stages before comparing
    -q, --brief      Only report if files differ, no details

ENVIRONMENT:
    USD_DIFF         External diff program to use
    DIFF             Fallback external diff program

EXAMPLES:
    # Compare two USD files
    usd diff model_v1.usda model_v2.usda

    # Compare flattened compositions
    usd diff --flatten scene_a.usd scene_b.usd

    # Quick check if files differ
    usd diff --brief old.usda new.usda
"#
    );
}
