//! usdzip - Create and manage USDZ packages.
//!
//! USDZ is a zero-compression, unencrypted zip archive containing USD files.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Runs the usdzip command.
pub fn run(args: &[String]) -> i32 {
    let mut input_file: Option<String> = None;
    let mut output_file: Option<String> = None;
    let mut list_contents = false;
    let mut dump_contents = false;
    let mut recurse = true;
    let mut verbose = false;
    let mut skip_patterns: Vec<String> = Vec::new();

    // Parse args (skip first which is the command name "zip")
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return 0;
            }
            "-l" | "--list" => {
                list_contents = true;
            }
            "-d" | "--dump" => {
                dump_contents = true;
            }
            "-r" | "--recurse" => {
                recurse = true;
            }
            "--norecurse" => {
                recurse = false;
            }
            "-v" | "--verbose" => {
                verbose = true;
            }
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output_file = Some(args[i].clone());
                } else {
                    eprintln!("Error: --output requires a file path");
                    return 1;
                }
            }
            "--skip" => {
                i += 1;
                if i < args.len() {
                    skip_patterns.push(args[i].clone());
                } else {
                    eprintln!("Error: --skip requires a pattern");
                    return 1;
                }
            }
            _ if !arg.starts_with('-') => {
                if input_file.is_none() {
                    input_file = Some(arg.clone());
                } else if output_file.is_none() {
                    output_file = Some(arg.clone());
                }
            }
            _ => {
                eprintln!("Unknown option: {}", arg);
                return 1;
            }
        }
        i += 1;
    }

    // Check for input file
    let input = match input_file {
        Some(f) => f,
        None => {
            eprintln!("Error: No input file specified");
            print_help();
            return 1;
        }
    };

    // Handle list/dump for existing USDZ
    if list_contents || dump_contents {
        return handle_inspect(&input, list_contents, dump_contents);
    }

    // Create USDZ package
    let output = output_file.unwrap_or_else(|| {
        // Default output: replace extension with .usdz
        let p = Path::new(&input);
        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
        format!("{}.usdz", stem)
    });

    if verbose {
        eprintln!("Creating USDZ package: {}", output);
        eprintln!("  Input: {}", input);
        eprintln!("  Recurse: {}", recurse);
        if !skip_patterns.is_empty() {
            eprintln!("  Skip patterns: {:?}", skip_patterns);
        }
    }

    match create_usdz(&input, &output, recurse, verbose, &skip_patterns) {
        Ok(()) => {
            println!("Created: {}", output);
            0
        }
        Err(e) => {
            eprintln!("Error creating USDZ: {}", e);
            1
        }
    }
}

/// Prints help message.
fn print_help() {
    println!(
        r#"usdzip - Create and manage USDZ packages

USAGE:
    usd zip [options] <input> [output.usdz]
    usd zip -l <file.usdz>

OPTIONS:
    -h, --help          Show this help
    -l, --list          List contents of USDZ file
    -d, --dump          Dump detailed contents info
    -r, --recurse       Include referenced files (default)
    --norecurse         Don't include referenced files
    --skip <pattern>    Skip dependencies matching pattern (can be used multiple times)
    -v, --verbose       Verbose output
    -o, --output <file> Output file path

EXAMPLES:
    # Create USDZ from USD file
    usd zip model.usda

    # Create USDZ with explicit output
    usd zip model.usda -o package.usdz

    # List contents of USDZ
    usd zip -l package.usdz

    # Dump detailed info
    usd zip -d package.usdz
"#
    );
}

/// Inspects a USDZ file (list or dump).
fn handle_inspect(path: &str, list: bool, dump: bool) -> i32 {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening {}: {}", path, e);
            return 1;
        }
    };

    match read_usdz_contents(file) {
        Ok(entries) => {
            if dump {
                println!("    Offset\t      Comp\t    Uncomp\tName");
                println!("    ------\t      ----\t    ------\t----");
            }

            for entry in &entries {
                if dump {
                    println!(
                        "{:>10}\t{:>10}\t{:>10}\t{}",
                        entry.offset, entry.compressed_size, entry.uncompressed_size, entry.name
                    );
                } else if list {
                    println!("{}", entry.name);
                }
            }

            if dump {
                println!("----------");
                println!("{} files total", entries.len());
            }

            0
        }
        Err(e) => {
            eprintln!("Error reading USDZ: {}", e);
            1
        }
    }
}

/// USDZ entry info.
#[derive(Debug)]
struct UsdzEntry {
    name: String,
    offset: u64,
    compressed_size: u64,
    uncompressed_size: u64,
}

/// Reads USDZ zip central directory to get file list.
fn read_usdz_contents(mut file: File) -> Result<Vec<UsdzEntry>, String> {
    // USDZ is uncompressed ZIP, read end-of-central-directory
    let file_size = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;

    // Look for EOCD signature (0x06054b50) near end of file
    let search_start = file_size.saturating_sub(65536);
    file.seek(SeekFrom::Start(search_start))
        .map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; (file_size - search_start) as usize];
    file.read_exact(&mut buf).map_err(|e| e.to_string())?;

    // Find EOCD signature
    let eocd_pos = buf
        .windows(4)
        .rposition(|w| w == [0x50, 0x4b, 0x05, 0x06])
        .ok_or("Not a valid ZIP/USDZ file (no EOCD found)")?;

    let eocd = &buf[eocd_pos..];
    if eocd.len() < 22 {
        return Err("Invalid EOCD record".to_string());
    }

    // Parse EOCD
    let cd_size = u32::from_le_bytes([eocd[12], eocd[13], eocd[14], eocd[15]]) as u64;
    let cd_offset = u32::from_le_bytes([eocd[16], eocd[17], eocd[18], eocd[19]]) as u64;

    // Read central directory
    file.seek(SeekFrom::Start(cd_offset))
        .map_err(|e| e.to_string())?;
    let mut cd_buf = vec![0u8; cd_size as usize];
    file.read_exact(&mut cd_buf).map_err(|e| e.to_string())?;

    // Parse central directory entries
    let mut entries = Vec::new();
    let mut pos = 0;

    while pos + 46 <= cd_buf.len() {
        // Check signature
        if cd_buf[pos..pos + 4] != [0x50, 0x4b, 0x01, 0x02] {
            break;
        }

        let compressed = u32::from_le_bytes([
            cd_buf[pos + 20],
            cd_buf[pos + 21],
            cd_buf[pos + 22],
            cd_buf[pos + 23],
        ]) as u64;
        let uncompressed = u32::from_le_bytes([
            cd_buf[pos + 24],
            cd_buf[pos + 25],
            cd_buf[pos + 26],
            cd_buf[pos + 27],
        ]) as u64;
        let name_len = u16::from_le_bytes([cd_buf[pos + 28], cd_buf[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([cd_buf[pos + 30], cd_buf[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([cd_buf[pos + 32], cd_buf[pos + 33]]) as usize;
        let offset = u32::from_le_bytes([
            cd_buf[pos + 42],
            cd_buf[pos + 43],
            cd_buf[pos + 44],
            cd_buf[pos + 45],
        ]) as u64;

        let name_start = pos + 46;
        let name_end = name_start + name_len;
        if name_end > cd_buf.len() {
            break;
        }

        let name = String::from_utf8_lossy(&cd_buf[name_start..name_end]).to_string();

        entries.push(UsdzEntry {
            name,
            offset,
            compressed_size: compressed,
            uncompressed_size: uncompressed,
        });

        pos = name_end + extra_len + comment_len;
    }

    Ok(entries)
}

/// Collects all dependencies from a layer recursively.
fn collect_dependencies(
    input_path: &str,
    base_dir: &Path,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
    verbose: bool,
) -> Result<(), String> {
    // Normalize path
    let abs_path = if Path::new(input_path).is_absolute() {
        input_path.to_string()
    } else {
        base_dir.join(input_path).to_string_lossy().to_string()
    };

    // Skip if already visited
    if visited.contains(&abs_path) {
        return Ok(());
    }
    visited.insert(abs_path.clone());

    // Try to open the layer
    let layer = match usd::sdf::Layer::find_or_open(&abs_path) {
        Ok(l) => l,
        Err(e) => {
            if verbose {
                eprintln!("  Warning: Could not open {}: {:?}", abs_path, e);
            }
            return Ok(());
        }
    };

    // Add this file
    deps.push(abs_path.clone());

    // Get layer's directory for relative path resolution
    let layer_dir = Path::new(&abs_path).parent().unwrap_or(base_dir);

    // Collect sublayers
    let sublayers = layer.sublayer_paths();
    for sublayer_path in sublayers {
        if verbose {
            eprintln!("  Found sublayer: {}", sublayer_path);
        }
        collect_dependencies(&sublayer_path, layer_dir, visited, deps, verbose)?;
    }

    // Collect composition dependencies (references, payloads, etc.)
    let comp_deps = layer.get_composition_asset_dependencies();
    for dep_path in comp_deps {
        if !dep_path.is_empty() && dep_path != abs_path {
            if verbose {
                eprintln!("  Found composition dep: {}", dep_path);
            }
            collect_dependencies(&dep_path, layer_dir, visited, deps, verbose)?;
        }
    }

    // Collect asset paths from attribute values (textures, etc.)
    collect_asset_paths_from_layer(&layer, layer_dir, visited, deps, verbose)?;

    Ok(())
}

/// Collects asset paths from attribute values in a layer.
///
/// Traverses all prims and attributes, extracting SdfAssetPath values
/// (textures, audio files, etc.) and recursively processing them.
fn collect_asset_paths_from_layer(
    layer: &usd::sdf::Layer,
    base_dir: &Path,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
    verbose: bool,
) -> Result<(), String> {
    // Traverse all root prims and their descendants
    for root_prim in layer.root_prims() {
        collect_asset_paths_from_prim(&root_prim, base_dir, visited, deps, verbose)?;
    }

    Ok(())
}

/// Recursively collects asset paths from a prim and its descendants.
fn collect_asset_paths_from_prim(
    prim: &usd::sdf::PrimSpec,
    base_dir: &Path,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
    verbose: bool,
) -> Result<(), String> {
    // Check attributes for asset path values
    for prop in prim.properties() {
        if let Some(attr) = prop.as_attribute() {
            // Check default value
            let default_val = attr.default_value();
            extract_asset_paths_from_value(&default_val, base_dir, visited, deps, verbose)?;

            // Check time samples
            let samples = attr.time_sample_map();
            for (_, val) in samples.iter() {
                extract_asset_paths_from_value(val, base_dir, visited, deps, verbose)?;
            }
        }
    }

    // Recurse into child prims
    for child in prim.name_children() {
        collect_asset_paths_from_prim(&child, base_dir, visited, deps, verbose)?;
    }

    Ok(())
}

/// Extracts asset paths from a Value (handles single AssetPath and Vec<AssetPath>).
fn extract_asset_paths_from_value(
    value: &usd::sdf::abstract_data::Value,
    base_dir: &Path,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
    verbose: bool,
) -> Result<(), String> {
    use usd::sdf::AssetPath;

    // Single AssetPath
    if let Some(ap) = value.get::<AssetPath>() {
        let path = ap.get_asset_path();
        if !path.is_empty() {
            add_asset_dependency(path, base_dir, visited, deps, verbose)?;
        }
    }

    // Array of AssetPaths
    if let Some(arr) = value.get::<Vec<AssetPath>>() {
        for ap in arr {
            let path = ap.get_asset_path();
            if !path.is_empty() {
                add_asset_dependency(path, base_dir, visited, deps, verbose)?;
            }
        }
    }

    Ok(())
}

/// Adds an asset path as a dependency if the file exists.
fn add_asset_dependency(
    asset_path: &str,
    base_dir: &Path,
    visited: &mut std::collections::HashSet<String>,
    deps: &mut Vec<String>,
    verbose: bool,
) -> Result<(), String> {
    // Resolve relative to base_dir
    let full_path = if Path::new(asset_path).is_absolute() {
        asset_path.to_string()
    } else {
        base_dir.join(asset_path).to_string_lossy().to_string()
    };

    // Skip if already visited
    if visited.contains(&full_path) {
        return Ok(());
    }

    // Check if file exists
    if Path::new(&full_path).exists() {
        visited.insert(full_path.clone());
        deps.push(full_path.clone());
        if verbose {
            eprintln!("  Found asset: {}", asset_path);
        }
    } else if verbose {
        eprintln!(
            "  Warning: Asset not found: {} (resolved: {})",
            asset_path, full_path
        );
    }

    Ok(())
}

/// Creates a USDZ package from a USD file.
fn create_usdz(
    input: &str,
    output: &str,
    recurse: bool,
    verbose: bool,
    skip_patterns: &[String],
) -> Result<(), String> {
    use std::io::BufWriter;

    // Open input layer to validate and collect dependencies
    usd::sdf::init();

    let layer = usd::sdf::Layer::find_or_open(input)
        .map_err(|e| format!("Failed to open {}: {:?}", input, e))?;

    // Collect files to include
    let base_dir = Path::new(input).parent().unwrap_or(Path::new("."));
    let mut files_to_add = Vec::new();

    if recurse {
        let mut visited = std::collections::HashSet::new();
        collect_dependencies(input, base_dir, &mut visited, &mut files_to_add, verbose)?;

        if verbose {
            eprintln!("  Collected {} files", files_to_add.len());
        }
    } else {
        files_to_add.push(input.to_string());
    }

    // Filter out skipped dependencies
    if !skip_patterns.is_empty() {
        let original_count = files_to_add.len();
        files_to_add.retain(|path| {
            let should_skip = skip_patterns.iter().any(|pattern| {
                path.contains(pattern)
                    || Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().contains(pattern))
                        .unwrap_or(false)
            });
            if should_skip && verbose {
                eprintln!("  Skipping: {}", path);
            }
            !should_skip
        });
        if verbose && files_to_add.len() < original_count {
            eprintln!("  After skip filter: {} files", files_to_add.len());
        }
    }

    // Create output file
    let out_file =
        File::create(output).map_err(|e| format!("Failed to create {}: {}", output, e))?;
    let mut writer = BufWriter::new(out_file);

    // Write ZIP entries (uncompressed, as per USDZ spec)
    let mut local_headers: Vec<(String, u64, u64, u32)> = Vec::new(); // (name, offset, size, crc)

    for file_path in &files_to_add {
        let offset = writer.stream_position().map_err(|e| e.to_string())?;

        // Read file content
        let content =
            std::fs::read(file_path).map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

        // Get just the filename for storage
        let name = Path::new(file_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Compute CRC-32
        let crc = compute_crc32(&content);

        // Write local file header
        write_local_header(&mut writer, &name, content.len() as u64, crc)?;

        // Write file data (uncompressed)
        writer.write_all(&content).map_err(|e| e.to_string())?;

        local_headers.push((name, offset, content.len() as u64, crc));

        if verbose {
            eprintln!("  Added: {} ({} bytes)", file_path, content.len());
        }
    }

    // Write central directory
    let cd_offset = writer.stream_position().map_err(|e| e.to_string())?;
    let mut cd_size = 0u64;

    for (name, offset, size, crc) in &local_headers {
        cd_size += write_central_header(&mut writer, name, *offset, *size, *crc)?;
    }

    // Write end of central directory
    write_eocd(
        &mut writer,
        local_headers.len() as u16,
        cd_size as u32,
        cd_offset as u32,
    )?;

    writer.flush().map_err(|e| e.to_string())?;

    drop(layer); // Release layer

    Ok(())
}

/// Computes CRC-32 for ZIP (polynomial 0xEDB88320).
fn compute_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Writes a ZIP local file header (uncompressed).
fn write_local_header<W: Write>(w: &mut W, name: &str, size: u64, crc: u32) -> Result<(), String> {
    let name_bytes = name.as_bytes();

    // Signature
    w.write_all(&[0x50, 0x4b, 0x03, 0x04])
        .map_err(|e| e.to_string())?;
    // Version needed
    w.write_all(&[0x14, 0x00]).map_err(|e| e.to_string())?;
    // Flags
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Compression (0 = store)
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Mod time/date (zero)
    w.write_all(&[0x00, 0x00, 0x00, 0x00])
        .map_err(|e| e.to_string())?;
    // CRC-32
    w.write_all(&crc.to_le_bytes()).map_err(|e| e.to_string())?;
    // Compressed size
    w.write_all(&(size as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Uncompressed size
    w.write_all(&(size as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Name length
    w.write_all(&(name_bytes.len() as u16).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Extra field length
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Name
    w.write_all(name_bytes).map_err(|e| e.to_string())?;

    Ok(())
}

/// Writes a ZIP central directory header.
fn write_central_header<W: Write + Seek>(
    w: &mut W,
    name: &str,
    offset: u64,
    size: u64,
    crc: u32,
) -> Result<u64, String> {
    let name_bytes = name.as_bytes();
    let header_size = 46 + name_bytes.len();

    // Signature
    w.write_all(&[0x50, 0x4b, 0x01, 0x02])
        .map_err(|e| e.to_string())?;
    // Version made by
    w.write_all(&[0x14, 0x00]).map_err(|e| e.to_string())?;
    // Version needed
    w.write_all(&[0x14, 0x00]).map_err(|e| e.to_string())?;
    // Flags
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Compression (0 = store)
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Mod time/date
    w.write_all(&[0x00, 0x00, 0x00, 0x00])
        .map_err(|e| e.to_string())?;
    // CRC-32
    w.write_all(&crc.to_le_bytes()).map_err(|e| e.to_string())?;
    // Compressed size
    w.write_all(&(size as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Uncompressed size
    w.write_all(&(size as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Name length
    w.write_all(&(name_bytes.len() as u16).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Extra field length
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Comment length
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Disk number start
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Internal attributes
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // External attributes
    w.write_all(&[0x00, 0x00, 0x00, 0x00])
        .map_err(|e| e.to_string())?;
    // Offset of local header
    w.write_all(&(offset as u32).to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Name
    w.write_all(name_bytes).map_err(|e| e.to_string())?;

    Ok(header_size as u64)
}

/// Writes the ZIP end-of-central-directory record.
fn write_eocd<W: Write>(
    w: &mut W,
    entry_count: u16,
    cd_size: u32,
    cd_offset: u32,
) -> Result<(), String> {
    // Signature
    w.write_all(&[0x50, 0x4b, 0x05, 0x06])
        .map_err(|e| e.to_string())?;
    // Disk number
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Disk with CD
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;
    // Entries on disk
    w.write_all(&entry_count.to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Total entries
    w.write_all(&entry_count.to_le_bytes())
        .map_err(|e| e.to_string())?;
    // CD size
    w.write_all(&cd_size.to_le_bytes())
        .map_err(|e| e.to_string())?;
    // CD offset
    w.write_all(&cd_offset.to_le_bytes())
        .map_err(|e| e.to_string())?;
    // Comment length
    w.write_all(&[0x00, 0x00]).map_err(|e| e.to_string())?;

    Ok(())
}
