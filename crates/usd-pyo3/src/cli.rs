//! Python bindings for all `usd` CLI subcommands.
//!
//! Exposed as the `pxr.Cli` module. Every function maps 1:1 to a subcommand
//! of the `usd` binary and builds the same args vector the CLI parses.
//!
//! # Design
//! Functions that write to stdout (`cat`, `tree`, `dump`, `meshdump`,
//! `dumpcrate`, `diff`) accept an optional `capture: bool` keyword argument.
//! When `capture=True` the subprocess stdout is captured and returned as
//! `str`; when `capture=False` (the default) output goes directly to the
//! calling process's stdout and the function returns the exit code.
//!
//! All other commands (those that write files or launch editors) always
//! return the exit code as `int`.
//!
//! # Finding the binary
//! The `usd` binary is located by inspecting the directory that contains the
//! currently running Python extension (`.pyd` / `.so`). If it is not found
//! there, `PATH` is searched as a fallback. This mirrors how Pixar's
//! `usdcat` script locates `usd`.
//!
//! # Examples (Python)
//! ```python
//! from pxr import Cli
//!
//! # Print a USD file (stdout passthrough, returns exit code)
//! code = Cli.cat("model.usda")
//!
//! # Capture layer text
//! text = Cli.cat("model.usda", capture=True)
//!
//! # Convert to binary
//! code = Cli.cat("model.usda", out="model.usdc")
//!
//! # Flatten and capture
//! text = Cli.cat("model.usda", flatten=True, capture=True)
//!
//! # Show prim tree
//! code = Cli.tree("model.usda", attributes=True, metadata=True)
//!
//! # Diff two files, capture output
//! text = Cli.diff("v1.usda", "v2.usda", capture=True)
//!
//! # Create USDZ package
//! code = Cli.zip("model.usda", output="model.usdz")
//!
//! # List USDZ contents
//! text = Cli.zip("package.usdz", list=True, capture=True)
//! ```

use pyo3::prelude::*;
use std::path::PathBuf;
use std::process::{Command, Stdio};

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Locate the `usd` (or `usd.exe`) binary.
///
/// Search order:
/// 1. Same directory as the loaded extension module (sibling of the `.pyd`).
/// 2. System PATH.
fn find_usd_binary() -> Option<PathBuf> {
    let exe_name = if cfg!(windows) { "usd.exe" } else { "usd" };

    // Try sibling of current exe first (works when running from build tree)
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Fallback: rely on PATH
    Some(PathBuf::from(exe_name))
}

/// Shared execution logic: build a [`Command`], run it, and either pass
/// through I/O or capture stdout depending on `capture`.
///
/// Returns `(exit_code, captured_stdout)` where `captured_stdout` is `Some`
/// only when `capture == true`.
fn exec(subcmd: &str, args: &[String], capture: bool) -> PyResult<(i32, Option<String>)> {
    let bin = find_usd_binary().ok_or_else(|| {
        pyo3::exceptions::PyRuntimeError::new_err(
            "usd binary not found — ensure it is in PATH or the same directory as the extension",
        )
    })?;

    let mut cmd = Command::new(&bin);
    cmd.arg(subcmd);
    for a in args {
        cmd.arg(a);
    }

    if capture {
        cmd.stdout(Stdio::piped()).stderr(Stdio::inherit());
        let output = cmd.output().map_err(|e| {
            pyo3::exceptions::PyOSError::new_err(format!("Failed to run usd {subcmd}: {e}"))
        })?;
        let code = output.status.code().unwrap_or(1);
        let text = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok((code, Some(text)))
    } else {
        // Pass stdout/stderr straight through to the terminal
        cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        let status = cmd.status().map_err(|e| {
            pyo3::exceptions::PyOSError::new_err(format!("Failed to run usd {subcmd}: {e}"))
        })?;
        let code = status.code().unwrap_or(1);
        Ok((code, None))
    }
}

/// Return value used by output-producing commands:
/// - `capture=False` → `int` (exit code)
/// - `capture=True`  → `str` (captured stdout; raises on non-zero exit)
fn output_result(py: Python<'_>, code: i32, text: Option<String>, capture: bool) -> PyResult<PyObject> {
    if capture {
        let s = text.unwrap_or_default();
        if code != 0 {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "command exited with code {code}"
            )));
        }
        s.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
    } else {
        code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// cat
// ---------------------------------------------------------------------------

/// Print or convert USD files.
///
/// Args:
///     *files: One or more input USD files.
///     out (str): Write output to this file instead of stdout.
///     usd_format (str): Output format for .usd files: ``"usda"`` or ``"usdc"``.
///     load_only (bool): Only test whether each file can be loaded.
///     flatten (bool): Flatten composed stage before printing.
///     flatten_layer_stack (bool): Flatten layer stack only.
///     mask (str): Comma-separated prim paths (requires ``flatten=True``).
///     layer_metadata (bool): Load and print only layer metadata.
///     skip_source_comment (bool): Omit the source-file comment in flattened output.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (*files, out=None, usd_format=None, load_only=false, flatten=false,
                    flatten_layer_stack=false, mask=None, layer_metadata=false,
                    skip_source_comment=false, capture=false))]
#[allow(clippy::too_many_arguments)]
fn cat(
    py: Python<'_>,
    files: Vec<String>,
    out: Option<String>,
    usd_format: Option<String>,
    load_only: bool,
    flatten: bool,
    flatten_layer_stack: bool,
    mask: Option<String>,
    layer_metadata: bool,
    skip_source_comment: bool,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if let Some(o) = out {
        args.push("-o".into());
        args.push(o);
    }
    if let Some(fmt) = usd_format {
        args.push("--usdFormat".into());
        args.push(fmt);
    }
    if load_only {
        args.push("--loadOnly".into());
    }
    if flatten {
        args.push("--flatten".into());
    }
    if flatten_layer_stack {
        args.push("--flattenLayerStack".into());
    }
    if let Some(m) = mask {
        args.push("--mask".into());
        args.push(m);
    }
    if layer_metadata {
        args.push("--layerMetadata".into());
    }
    if skip_source_comment {
        args.push("--skipSourceFileComment".into());
    }
    for f in files {
        args.push(f);
    }

    let (code, text) = exec("cat", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// tree
// ---------------------------------------------------------------------------

/// Display USD prim hierarchy as ASCII tree.
///
/// Args:
///     file (str): Input USD file.
///     unloaded (bool): Do not load payloads.
///     attributes (bool): Show authored attributes.
///     metadata (bool): Show authored metadata.
///     simple (bool): Show prim names only.
///     flatten (bool): Compose stage and show flattened tree.
///     flatten_layer_stack (bool): Flatten layer stack only.
///     mask (str): Comma-separated prim paths (requires ``flatten=True``).
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (file, unloaded=false, attributes=false, metadata=false, simple=false,
                    flatten=false, flatten_layer_stack=false, mask=None, capture=false))]
#[allow(clippy::too_many_arguments)]
fn tree(
    py: Python<'_>,
    file: String,
    unloaded: bool,
    attributes: bool,
    metadata: bool,
    simple: bool,
    flatten: bool,
    flatten_layer_stack: bool,
    mask: Option<String>,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if unloaded {
        args.push("--unloaded".into());
    }
    if attributes {
        args.push("--attributes".into());
    }
    if metadata {
        args.push("--metadata".into());
    }
    if simple {
        args.push("--simple".into());
    }
    if flatten {
        args.push("--flatten".into());
    }
    if flatten_layer_stack {
        args.push("--flattenLayerStack".into());
    }
    if let Some(m) = mask {
        args.push("--mask".into());
        args.push(m);
    }
    args.push(file);

    let (code, text) = exec("tree", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// dump
// ---------------------------------------------------------------------------

/// Dump raw SDF layer data.
///
/// Args:
///     *files: One or more USD files.
///     summary (bool): Show high-level statistics only.
///     validate (bool): Read all values to check validity.
///     path (str): Regex — report only paths matching this pattern.
///     field (str): Regex — report only fields matching this pattern.
///     sort_by (str): Group output by ``"path"`` or ``"field"`` (default ``"path"``).
///     no_values (bool): Do not print field values.
///     full_arrays (bool): Print full array contents.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (*files, summary=false, validate=false, path=None, field=None,
                    sort_by=None, no_values=false, full_arrays=false, capture=false))]
#[allow(clippy::too_many_arguments)]
fn dump(
    py: Python<'_>,
    files: Vec<String>,
    summary: bool,
    validate: bool,
    path: Option<String>,
    field: Option<String>,
    sort_by: Option<String>,
    no_values: bool,
    full_arrays: bool,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if summary {
        args.push("--summary".into());
    }
    if validate {
        args.push("--validate".into());
    }
    if let Some(p) = path {
        args.push("--path".into());
        args.push(p);
    }
    if let Some(f) = field {
        args.push("--field".into());
        args.push(f);
    }
    if let Some(s) = sort_by {
        args.push("--sortBy".into());
        args.push(s);
    }
    if no_values {
        args.push("--noValues".into());
    }
    if full_arrays {
        args.push("--fullArrays".into());
    }
    for f in files {
        args.push(f);
    }

    let (code, text) = exec("dump", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// meshdump
// ---------------------------------------------------------------------------

/// Dump composed mesh/prim details (xform, bounds, topology).
///
/// Args:
///     file (str): Input USD file.
///     prim_path (str): SDF path of the prim to inspect (e.g. ``"/World/Car"``).
///     time (float | None): Sample time. Uses default time when ``None``.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (file, prim_path, time=None, capture=false))]
fn meshdump(
    py: Python<'_>,
    file: String,
    prim_path: String,
    time: Option<f64>,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if let Some(t) = time {
        args.push("--time".into());
        args.push(t.to_string());
    }
    args.push(file);
    args.push(prim_path);

    let (code, text) = exec("meshdump", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// filter
// ---------------------------------------------------------------------------

/// Filter and display SDF layer data.
///
/// Args:
///     *files: One or more USD files.
///     out (str): Write output to this file.
///     path (str): Regex — report only paths matching this pattern.
///     field (str): Regex — report only fields matching this pattern.
///     output_type (str): ``"validity"`` | ``"summary"`` | ``"outline"`` |
///         ``"pseudoLayer"`` | ``"layer"`` (default ``"outline"``).
///     output_format (str): Format for ``"layer"`` output (``"usda"``, ``"usdc"``).
///     sort_by (str): ``"path"`` or ``"field"`` (default ``"path"``).
///     no_values (bool): Do not print field values.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (*files, out=None, path=None, field=None, output_type=None,
                    output_format=None, sort_by=None, no_values=false, capture=false))]
#[allow(clippy::too_many_arguments)]
fn filter(
    py: Python<'_>,
    files: Vec<String>,
    out: Option<String>,
    path: Option<String>,
    field: Option<String>,
    output_type: Option<String>,
    output_format: Option<String>,
    sort_by: Option<String>,
    no_values: bool,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if let Some(o) = out {
        args.push("-o".into());
        args.push(o);
    }
    if let Some(p) = path {
        args.push("--path".into());
        args.push(p);
    }
    if let Some(f) = field {
        args.push("--field".into());
        args.push(f);
    }
    if let Some(ot) = output_type {
        args.push("--outputType".into());
        args.push(ot);
    }
    if let Some(of_) = output_format {
        args.push("--outputFormat".into());
        args.push(of_);
    }
    if let Some(s) = sort_by {
        args.push("--sortBy".into());
        args.push(s);
    }
    if no_values {
        args.push("--noValues".into());
    }
    for f in files {
        args.push(f);
    }

    let (code, text) = exec("filter", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

/// Compare two USD files and print a unified diff.
///
/// Args:
///     file1 (str): Baseline file.
///     file2 (str): Comparison file.
///     flatten (bool): Flatten both files as stages before comparing.
///     brief (bool): Only report whether the files differ (no details).
///     noeffect (bool): Read-only mode — do not edit either file.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code (0 = identical, 1 = differs, 2 = error),
///     or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (file1, file2, flatten=false, brief=false, noeffect=false, capture=false))]
fn diff(
    py: Python<'_>,
    file1: String,
    file2: String,
    flatten: bool,
    brief: bool,
    noeffect: bool,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if flatten {
        args.push("--flatten".into());
    }
    if brief {
        args.push("--brief".into());
    }
    if noeffect {
        args.push("--noeffect".into());
    }
    args.push(file1);
    args.push(file2);

    // diff exit code 1 means "files differ" — that is not an error, so we
    // do not raise even when capture=True.
    let (code, text) = exec("diff", &args, capture)?;
    if capture {
        text.unwrap_or_default().into_pyobject(py).map(|o| o.into()).map_err(Into::into)
    } else {
        code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
    }
}

// ---------------------------------------------------------------------------
// resolve
// ---------------------------------------------------------------------------

/// Resolve an asset path using the configured USD Asset Resolver.
///
/// Args:
///     path (str): The asset path to resolve.
///     anchor_path (str): Anchor relative path resolution to this path.
///     context_for_asset (str): Create resolver context for this asset.
///     context_from_string (str | list[str]): Create context from string(s)
///         in ``"[scheme:]config"`` format.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` (resolved path) when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (path, anchor_path=None, context_for_asset=None,
                    context_from_string=None, capture=false))]
fn resolve(
    py: Python<'_>,
    path: String,
    anchor_path: Option<String>,
    context_for_asset: Option<String>,
    context_from_string: Option<Vec<String>>,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if let Some(a) = anchor_path {
        args.push("--anchorPath".into());
        args.push(a);
    }
    if let Some(c) = context_for_asset {
        args.push("--createContextForAsset".into());
        args.push(c);
    }
    if let Some(cs) = context_from_string {
        for s in cs {
            args.push("--createContextFromString".into());
            args.push(s);
        }
    }
    args.push(path);

    let (code, text) = exec("resolve", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------

/// Open a USD file in a text editor, save changes back on exit.
///
/// The file is exported to a temporary ``.usda`` location, opened in the
/// editor selected by ``USD_EDITOR`` / ``EDITOR`` env vars, then saved back
/// to the original format.
///
/// Args:
///     file (str): Input USD file.
///     noeffect (bool): Read-only — open file but do not save changes.
///     forcewrite (bool): Override file-system read-only flag.
///     prefix (str): Prefix for the temporary file name.
///
/// Returns:
///     ``int`` exit code.
#[pyfunction]
#[pyo3(signature = (file, noeffect=false, forcewrite=false, prefix=None))]
fn edit(
    py: Python<'_>,
    file: String,
    noeffect: bool,
    forcewrite: bool,
    prefix: Option<String>,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if noeffect {
        args.push("--noeffect".into());
    }
    if forcewrite {
        args.push("--forcewrite".into());
    }
    if let Some(p) = prefix {
        args.push("--prefix".into());
        args.push(p);
    }
    args.push(file);

    let (code, _) = exec("edit", &args, false)?;
    code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
}

// ---------------------------------------------------------------------------
// stitch
// ---------------------------------------------------------------------------

/// Combine multiple USD layers into one output file.
///
/// Opinion strength follows input order — the first file has the strongest
/// opinions.
///
/// Args:
///     *files: Input USD files (at least one required).
///     out (str): Output file path (required).
///
/// Returns:
///     ``int`` exit code.
#[pyfunction]
#[pyo3(signature = (*files, out))]
fn stitch(py: Python<'_>, files: Vec<String>, out: String) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    for f in files {
        args.push(f);
    }
    args.push("-o".into());
    args.push(out);

    let (code, _) = exec("stitch", &args, false)?;
    code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
}

// ---------------------------------------------------------------------------
// dumpcrate
// ---------------------------------------------------------------------------

/// Dump diagnostic information about USD crate (``.usdc``) files.
///
/// Args:
///     *files: One or more ``.usdc`` files.
///     summary (bool): Report only a short summary.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (*files, summary=false, capture=false))]
fn dumpcrate(
    py: Python<'_>,
    files: Vec<String>,
    summary: bool,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if summary {
        args.push("--summary".into());
    }
    for f in files {
        args.push(f);
    }

    let (code, text) = exec("dumpcrate", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// stitchclips
// ---------------------------------------------------------------------------

/// Stitch USD files using the value clips composition mechanism.
///
/// Args:
///     *files: Input clip files.
///     out (str): Output file path (required).
///     clip_path (str): Prim path for clips, e.g. ``"/Model"`` (required).
///     start_time (float | None): Start time code.
///     end_time (float | None): End time code.
///     stride (float | None): Time stride for template-based clips.
///     template_path (str | None): Template string for clip asset paths,
///         e.g. ``"./clip.###.usd"``.
///     clip_set (str | None): Clip set name (default ``"default"``).
///     no_comment (bool): Do not add a comment to the output file.
///     interpolate_missing (bool): Interpolate values for clips without
///         time samples.
///
/// Returns:
///     ``int`` exit code.
#[pyfunction]
#[pyo3(signature = (*files, out, clip_path, start_time=None, end_time=None,
                    stride=None, template_path=None, clip_set=None,
                    no_comment=false, interpolate_missing=false))]
#[allow(clippy::too_many_arguments)]
fn stitchclips(
    py: Python<'_>,
    files: Vec<String>,
    out: String,
    clip_path: String,
    start_time: Option<f64>,
    end_time: Option<f64>,
    stride: Option<f64>,
    template_path: Option<String>,
    clip_set: Option<String>,
    no_comment: bool,
    interpolate_missing: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    args.push("-o".into());
    args.push(out);
    args.push("-c".into());
    args.push(clip_path);

    if let Some(s) = start_time {
        args.push("-s".into());
        args.push(s.to_string());
    }
    if let Some(e) = end_time {
        args.push("-e".into());
        args.push(e.to_string());
    }
    if let Some(st) = stride {
        args.push("--stride".into());
        args.push(st.to_string());
    }
    if let Some(tp) = template_path {
        args.push("--templatePath".into());
        args.push(tp);
    }
    if let Some(cs) = clip_set {
        args.push("--clipSet".into());
        args.push(cs);
    }
    if no_comment {
        args.push("--noComment".into());
    }
    if interpolate_missing {
        args.push("--interpolateMissingClipValues".into());
    }
    for f in files {
        args.push(f);
    }

    let (code, _) = exec("stitchclips", &args, false)?;
    code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
}

// ---------------------------------------------------------------------------
// zip
// ---------------------------------------------------------------------------

/// Create or inspect a USDZ package.
///
/// When called without ``list`` or ``dump``, creates a new USDZ archive.
/// When called with ``list=True`` or ``dump=True``, inspects an existing one.
///
/// Args:
///     file (str): Input USD file (when creating) or USDZ file (when listing).
///     output (str | None): Output ``.usdz`` file path.
///     list (bool): List file names contained in the USDZ archive.
///     dump (bool): Dump detailed entry information (offsets, sizes, names).
///     recurse (bool): Include referenced dependency files (default ``True``).
///     norecurse (bool): Do not include dependencies.
///     verbose (bool): Verbose progress output.
///     skip (str | list[str]): Pattern(s) to exclude from the package.
///     capture (bool): Capture stdout and return it as ``str`` (default ``False``).
///
/// Returns:
///     ``int`` exit code, or ``str`` when ``capture=True``.
#[pyfunction]
#[pyo3(signature = (file, output=None, list=false, dump=false, recurse=true,
                    norecurse=false, verbose=false, skip=None, capture=false))]
#[allow(clippy::too_many_arguments)]
fn zip(
    py: Python<'_>,
    file: String,
    output: Option<String>,
    list: bool,
    dump: bool,
    recurse: bool,
    norecurse: bool,
    verbose: bool,
    skip: Option<Vec<String>>,
    capture: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if list {
        args.push("--list".into());
    }
    if dump {
        args.push("--dump".into());
    }
    if norecurse {
        args.push("--norecurse".into());
    } else if recurse {
        args.push("--recurse".into());
    }
    if verbose {
        args.push("--verbose".into());
    }
    if let Some(patterns) = skip {
        for p in patterns {
            args.push("--skip".into());
            args.push(p);
        }
    }
    if let Some(o) = output {
        args.push("-o".into());
        args.push(o);
    }
    args.push(file);

    let (code, text) = exec("zip", &args, capture)?;
    output_result(py, code, text, capture)
}

// ---------------------------------------------------------------------------
// compress
// ---------------------------------------------------------------------------

/// Compress USD meshes with Draco compression.
///
/// Produces a modified USD file referencing ``.drc`` compressed mesh files
/// in a sibling ``<output>.draco/`` directory.
///
/// Args:
///     file (str): Input USD file.
///     out (str): Output USD file (required).
///     verbose (bool): Enable verbose output.
///     qp (int): Quantization bits for positions (0–30, default 14).
///     qt (int): Quantization bits for texture coordinates (0–30, default 12).
///     qn (int): Quantization bits for normals (0–30, default 10).
///     cl (int): Compression level 0–10, best = 10 (default 10).
///     preserve_polygons (bool | None): Preserve polygon structure.
///     discard_subdivision (bool | None): Discard subdivision data.
///     ignore_opinion_errors (bool): Continue even when opinions cannot be cleared.
///
/// Returns:
///     ``int`` exit code.
#[pyfunction]
#[pyo3(signature = (file, out, verbose=false, qp=14, qt=12, qn=10, cl=10,
                    preserve_polygons=None, discard_subdivision=None,
                    ignore_opinion_errors=false))]
#[allow(clippy::too_many_arguments)]
fn compress(
    py: Python<'_>,
    file: String,
    out: String,
    verbose: bool,
    qp: i32,
    qt: i32,
    qn: i32,
    cl: i32,
    preserve_polygons: Option<bool>,
    discard_subdivision: Option<bool>,
    ignore_opinion_errors: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    args.push("-o".into());
    args.push(out);

    if verbose {
        args.push("--verbose".into());
    }
    args.push("-qp".into());
    args.push(qp.to_string());
    args.push("-qt".into());
    args.push(qt.to_string());
    args.push("-qn".into());
    args.push(qn.to_string());
    args.push("-cl".into());
    args.push(cl.to_string());

    if let Some(pp) = preserve_polygons {
        args.push("--preserve_polygons".into());
        args.push(if pp { "1" } else { "0" }.into());
    }
    if let Some(ds) = discard_subdivision {
        args.push("--discard_subdivision".into());
        args.push(if ds { "1" } else { "0" }.into());
    }
    if ignore_opinion_errors {
        args.push("--ignore_opinion_errors".into());
    }
    args.push(file);

    let (code, _) = exec("compress", &args, false)?;
    code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
}

// ---------------------------------------------------------------------------
// fixbrokenpixarschemas
// ---------------------------------------------------------------------------

/// Apply schema migration fixes to a USD file in-place.
///
/// Fixes include: adding ``MaterialBindingAPI``/``SkelBindingAPI`` applied
/// schemas where required, setting ``upAxis`` when missing, etc.
///
/// Args:
///     file (str): Input USD file (``.usd``, ``.usda``, or ``.usdc``).
///     backup (str | None): Backup file path (default: ``<stem>.bck.<ext>``).
///     verbose (bool): Print progress messages.
///
/// Returns:
///     ``int`` exit code.
#[pyfunction]
#[pyo3(signature = (file, backup=None, verbose=false))]
fn fixbrokenpixarschemas(
    py: Python<'_>,
    file: String,
    backup: Option<String>,
    verbose: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if let Some(b) = backup {
        args.push("--backup".into());
        args.push(b);
    }
    if verbose {
        args.push("--verbose".into());
    }
    args.push(file);

    let (code, _) = exec("fixbrokenpixarschemas", &args, false)?;
    code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
}

// ---------------------------------------------------------------------------
// genschemafromsdr
// ---------------------------------------------------------------------------

/// Generate USD schema files from SDR shader node definitions.
///
/// Args:
///     config (str): JSON config file (default ``"./schemaConfig.json"``).
///     output_dir (str): Target directory containing ``schema.usda``
///         (default ``"./"``).
///     noreadme (bool): Do not generate ``README.md``.
///     validate (bool): Verify generated files are unchanged.
///
/// Returns:
///     ``int`` exit code.
#[pyfunction]
#[pyo3(signature = (config=None, output_dir=None, noreadme=false, validate=false))]
fn genschemafromsdr(
    py: Python<'_>,
    config: Option<String>,
    output_dir: Option<String>,
    noreadme: bool,
    validate: bool,
) -> PyResult<PyObject> {
    let mut args: Vec<String> = Vec::new();

    if noreadme {
        args.push("--noreadme".into());
    }
    if validate {
        args.push("--validate".into());
    }
    // Positional args (config then output_dir)
    if let Some(c) = config {
        args.push(c);
    }
    if let Some(d) = output_dir {
        args.push(d);
    }

    let (code, _) = exec("genschemafromsdr", &args, false)?;
    code.into_pyobject(py).map(|o| o.into()).map_err(Into::into)
}

// ---------------------------------------------------------------------------
// Module registration
// ---------------------------------------------------------------------------

/// Register all CLI functions into the `pxr.Cli` sub-module.
pub fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(cat, m)?)?;
    m.add_function(wrap_pyfunction!(tree, m)?)?;
    m.add_function(wrap_pyfunction!(dump, m)?)?;
    m.add_function(wrap_pyfunction!(meshdump, m)?)?;
    m.add_function(wrap_pyfunction!(filter, m)?)?;
    m.add_function(wrap_pyfunction!(diff, m)?)?;
    m.add_function(wrap_pyfunction!(resolve, m)?)?;
    m.add_function(wrap_pyfunction!(edit, m)?)?;
    m.add_function(wrap_pyfunction!(stitch, m)?)?;
    m.add_function(wrap_pyfunction!(dumpcrate, m)?)?;
    m.add_function(wrap_pyfunction!(stitchclips, m)?)?;
    m.add_function(wrap_pyfunction!(zip, m)?)?;
    m.add_function(wrap_pyfunction!(compress, m)?)?;
    m.add_function(wrap_pyfunction!(fixbrokenpixarschemas, m)?)?;
    m.add_function(wrap_pyfunction!(genschemafromsdr, m)?)?;
    Ok(())
}
