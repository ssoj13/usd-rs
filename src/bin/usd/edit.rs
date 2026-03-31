//! usdedit - Edit USD files in a text editor
//!
//! Port of pxr/usd/bin/usdedit/usdedit.py
//!
//! Converts a USD file to .usda in a temp location, opens it in
//! an editor, and saves changes back to the original format.

use std::path::Path;
use std::process::Command;

/// Run the edit command
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
    let mut input_file: Option<String> = None;
    let mut read_only = false;
    let mut force_write = false;
    let mut prefix: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-n" | "--noeffect" => read_only = true,
            "-f" | "--forcewrite" => force_write = true,
            "-p" | "--prefix" => {
                i += 1;
                if i >= args.len() {
                    return Err("--prefix requires an argument".to_string());
                }
                prefix = Some(args[i].clone());
            }
            arg if arg.starts_with('-') => {
                return Err(format!("Unknown option: {}", arg));
            }
            _ => {
                if input_file.is_some() {
                    return Err("Only one input file expected".to_string());
                }
                input_file = Some(args[i].clone());
            }
        }
        i += 1;
    }

    let usd_file = input_file.ok_or("No input file specified")?;

    // Validate args
    if read_only && force_write {
        return Err("Cannot set read-only (-n) and force-write (-f) together".to_string());
    }

    // Check file exists
    if !Path::new(&usd_file).exists() {
        return Err(format!("File not found: {}", usd_file));
    }

    // Check writable
    let writable = std::fs::metadata(&usd_file)
        .map(|m| !m.permissions().readonly())
        .unwrap_or(false);

    if !writable && !read_only && !force_write {
        return Err("File isn't writable, use -n (read-only) or -f (force-write)".to_string());
    }

    // Find editor
    let editor = find_editor()?;
    log::info!("Using editor: {}", editor);

    // Generate temp file
    let temp_path = generate_temp_file(&usd_file, read_only, prefix.as_deref())?;
    log::info!("Created temp file: {}", temp_path);

    // Record timestamp before edit
    let before_mtime = std::fs::metadata(&temp_path)
        .and_then(|m| m.modified())
        .ok();

    // Open editor
    log::debug!("Opening editor...");
    let status = Command::new(&editor)
        .arg(&temp_path)
        .status()
        .map_err(|e| format!("Failed to run editor: {}", e))?;

    if !status.success() {
        log::warn!("Editor exited with non-zero status");
    }

    // Check if file was modified
    let after_mtime = std::fs::metadata(&temp_path)
        .and_then(|m| m.modified())
        .ok();

    let file_changed = match (before_mtime, after_mtime) {
        (Some(before), Some(after)) => after != before,
        _ => true, // Assume changed if we can't tell
    };

    // Write back changes if appropriate
    if (!read_only || force_write) && file_changed {
        log::info!("Writing changes back to {}", usd_file);
        write_out_changes(&temp_path, &usd_file)?;
    } else if file_changed {
        log::info!("File modified but read-only mode, not saving changes");
    } else {
        log::info!("No changes detected");
    }

    // Cleanup temp file
    if read_only {
        // Make writable before delete (intentional: need write perms to delete on Windows)
        #[allow(clippy::permissions_set_readonly_false)]
        if let Ok(mut perms) = std::fs::metadata(&temp_path).map(|m| m.permissions()) {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(&temp_path, perms);
        }
    }
    let _ = std::fs::remove_file(&temp_path);

    Ok(())
}

fn find_editor() -> Result<String, String> {
    // Check environment variables
    if let Ok(editor) = std::env::var("USD_EDITOR") {
        return Ok(editor);
    }
    if let Ok(editor) = std::env::var("EDITOR") {
        return Ok(editor);
    }

    // Try common editors
    #[cfg(windows)]
    let candidates = ["code", "notepad++", "notepad"];

    #[cfg(not(windows))]
    let candidates = ["code", "vim", "nano", "emacs"];

    for editor in candidates {
        if which_exists(editor) {
            return Ok(editor.to_string());
        }
    }

    Err("No suitable editor found. Set USD_EDITOR or EDITOR environment variable.".to_string())
}

fn which_exists(cmd: &str) -> bool {
    Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn generate_temp_file(
    usd_file: &str,
    read_only: bool,
    prefix: Option<&str>,
) -> Result<String, String> {
    use usd::sdf::Layer;

    // Open the USD file
    let layer =
        Layer::find_or_open(usd_file).map_err(|e| format!("Failed to open {}: {}", usd_file, e))?;

    // Generate temp file name
    let basename = Path::new(usd_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("usd");

    let full_prefix = prefix
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("{}_tmp", basename));

    let temp_dir = if read_only {
        std::env::temp_dir()
    } else {
        std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir())
    };

    let temp_path = temp_dir.join(format!("{}.usda", full_prefix));
    let temp_str = temp_path.to_string_lossy().to_string();

    // Export to usda
    layer
        .export(&temp_str)
        .map_err(|e| format!("Failed to export to {}: {}", temp_str, e))?;

    // Set read-only if requested
    if read_only {
        if let Ok(mut perms) = std::fs::metadata(&temp_str).map(|m| m.permissions()) {
            perms.set_readonly(true);
            let _ = std::fs::set_permissions(&temp_str, perms);
        }
    }

    Ok(temp_str)
}

fn write_out_changes(temp_file: &str, original_file: &str) -> Result<(), String> {
    use usd::sdf::Layer;

    // Open the edited temp file
    let temp_layer = Layer::find_or_open(temp_file)
        .map_err(|e| format!("Failed to parse edited file: {}", e))?;

    // Open the original file
    let out_layer = Layer::find_or_open(original_file)
        .map_err(|e| format!("Failed to open original file: {}", e))?;

    // Transfer content
    out_layer.transfer_content(&temp_layer);

    // Save
    match out_layer.save() {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!("Failed to save changes to {}", original_file));
        }
        Err(e) => {
            return Err(format!("Error saving changes to {}: {}", original_file, e));
        }
    }

    Ok(())
}

fn print_help() {
    println!(
        r#"usdedit - Edit USD files in a text editor

USAGE:
    usd edit [options] <usdFile>

DESCRIPTION:
    Converts a USD file to .usda text format in a temporary location,
    opens it in a text editor, and saves changes back to the original
    file format when the editor exits.

ARGUMENTS:
    <usdFile>    The USD file to edit

OPTIONS:
    -h, --help          Show this help
    -n, --noeffect      Read-only mode, don't save changes back
    -f, --forcewrite    Override file permissions to allow writing
    -p, --prefix <str>  Prefix for the temporary file name

ENVIRONMENT:
    USD_EDITOR    Preferred text editor
    EDITOR        Fallback text editor

EXAMPLES:
    # Edit a USD file
    usd edit model.usda

    # View a USD file without editing
    usd edit -n scene.usdc

    # Force edit a read-only file
    usd edit -f locked_asset.usd

    # Use custom temp file prefix
    usd edit -p my_edit model.usda
"#
    );
}
