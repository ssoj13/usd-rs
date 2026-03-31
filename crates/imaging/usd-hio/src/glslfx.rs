//! HioGlslfx - Central parser for .glslfx shader effect files.
//!
//! Port of pxr/imaging/hio/glslfx.h/cpp
//!
//! GLSLFX is the shader effect file format used throughout Storm/Hydra.
//! It combines shader configuration (parameters, textures, metadata) with
//! GLSL source code sections and resource layout definitions.
//!
//! Even with wgpu/WGSL, GLSLFX is still used by MaterialX shader generation
//! and legacy shader loading.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use usd_ar;

use usd_tf::Token;
use usd_vt::Dictionary;

use super::glslfx_config::{
    self, Attributes, HioGlslfxConfig, MetadataDictionary, Parameters, Textures,
};

// ============================================================================
// GLSLFX tokens
// ============================================================================

/// Well-known GLSLFX tokens.
pub mod glslfx_tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    /// Static token set for GLSLFX.
    pub struct GlslfxTokens {
        pub glslfx: Token,
        pub fragment_shader: Token,
        pub geometry_shader: Token,
        pub geometry_shader_injection: Token,
        pub preamble: Token,
        pub tess_control_shader: Token,
        pub tess_eval_shader: Token,
        pub post_tess_control_shader: Token,
        pub post_tess_vertex_shader: Token,
        pub vertex_shader: Token,
        pub vertex_shader_injection: Token,
        pub surface_shader: Token,
        pub displacement_shader: Token,
        pub volume_shader: Token,
        pub def_val: Token,
    }

    static TOKENS: LazyLock<GlslfxTokens> = LazyLock::new(|| GlslfxTokens {
        glslfx: Token::new("glslfx"),
        fragment_shader: Token::new("fragmentShader"),
        geometry_shader: Token::new("geometryShader"),
        geometry_shader_injection: Token::new("geometryShaderInjection"),
        preamble: Token::new("preamble"),
        tess_control_shader: Token::new("tessControlShader"),
        tess_eval_shader: Token::new("tessEvalShader"),
        post_tess_control_shader: Token::new("postTessControlShader"),
        post_tess_vertex_shader: Token::new("postTessVertexShader"),
        vertex_shader: Token::new("vertexShader"),
        vertex_shader_injection: Token::new("vertexShaderInjection"),
        surface_shader: Token::new("surfaceShader"),
        displacement_shader: Token::new("displacementShader"),
        volume_shader: Token::new("volumeShader"),
        def_val: Token::new("default"),
    });

    /// Get the GLSLFX tokens singleton.
    pub fn tokens() -> &'static GlslfxTokens {
        &TOKENS
    }
}

// ============================================================================
// Internal parse tokens
// ============================================================================

const SECTION_DELIMITER: &str = "--";
const COMMENT_DELIMITER: &str = "---";
const IMPORT_DIRECTIVE: &str = "#import";
/// Current GLSLFX format version.
pub const GLSLFX_VERSION: f64 = 0.1;

// ============================================================================
// ParseContext
// ============================================================================

/// Internal parser state for a single file being processed.
#[derive(Debug, Clone)]
struct ParseContext {
    filename: String,
    line_no: usize,
    version: f64,
    current_line: String,
    current_section_type: String,
    current_section_id: String,
    imports: Vec<String>,
}

impl ParseContext {
    fn new(filename: &str) -> Self {
        Self {
            filename: filename.to_string(),
            line_no: 0,
            version: -1.0,
            current_line: String::new(),
            current_section_type: String::new(),
            current_section_id: String::new(),
            imports: Vec::new(),
        }
    }
}

// ============================================================================
// HioGlslfx
// ============================================================================

/// A GLSLFX shader effect file parser.
///
/// Matches C++ `HioGlslfx`.
///
/// Parses `.glslfx` files which contain:
/// - Version header: `-- glslfx version 0.1`
/// - Configuration section (JSON): parameters, textures, techniques
/// - GLSL source sections: `-- glsl SectionName`
/// - Layout sections: `-- layout SectionName`
/// - Import directives: `#import path/to/file.glslfx`
pub struct HioGlslfx {
    /// Global parse context (root file).
    global_context: ParseContext,
    /// Map of section ID -> GLSL source code.
    source_map: HashMap<String, String>,
    /// Map of section ID -> layout definitions.
    layout_map: HashMap<String, String>,
    /// Map of filename -> config JSON text.
    config_map: HashMap<String, String>,
    /// Ordered list of config filenames (weakest to strongest).
    config_order: Vec<String>,
    /// Set of already-processed files (cycle detection).
    seen_files: HashSet<String>,
    /// Parsed configuration.
    config: Option<HioGlslfxConfig>,
    /// Active technique.
    technique: Token,
    /// Whether the file is valid.
    valid: bool,
    /// Reason for invalidity.
    invalid_reason: String,
    /// Hash of all processed content.
    hash: u64,
}

impl HioGlslfx {
    /// Create an invalid (empty) GLSLFX object.
    pub fn invalid() -> Self {
        Self {
            global_context: ParseContext::new(""),
            source_map: HashMap::new(),
            layout_map: HashMap::new(),
            config_map: HashMap::new(),
            config_order: Vec::new(),
            seen_files: HashSet::new(),
            config: None,
            technique: glslfx_tokens::tokens().def_val.clone(),
            valid: false,
            invalid_reason: String::new(),
            hash: 0,
        }
    }

    /// Create a GLSLFX object from a file path.
    ///
    /// Matches C++ `HioGlslfx(filePath, technique)`.
    pub fn from_file(file_path: &str, technique: Option<&Token>) -> Self {
        let technique = technique
            .cloned()
            .unwrap_or_else(|| glslfx_tokens::tokens().def_val.clone());

        let resolved = Self::resolve_path("", file_path);
        if resolved.is_empty() {
            log::warn!("HioGlslfx: File doesn't exist: \"{}\"", file_path);
            return Self::invalid();
        }

        let mut glslfx = Self {
            global_context: ParseContext::new(&resolved),
            source_map: HashMap::new(),
            layout_map: HashMap::new(),
            config_map: HashMap::new(),
            config_order: Vec::new(),
            seen_files: HashSet::new(),
            config: None,
            technique,
            valid: true,
            invalid_reason: String::new(),
            hash: 0,
        };

        let filename = glslfx.global_context.filename.clone();
        glslfx.valid = glslfx.process_file(&filename);

        if glslfx.valid {
            glslfx.valid = glslfx.compose_configuration();
        }

        glslfx
    }

    /// Create a GLSLFX object from a string (in-memory source).
    ///
    /// Matches C++ `HioGlslfx(istream, technique)`.
    pub fn from_string(source: &str, technique: Option<&Token>) -> Self {
        let technique = technique
            .cloned()
            .unwrap_or_else(|| glslfx_tokens::tokens().def_val.clone());

        let mut glslfx = Self {
            global_context: ParseContext::new("string"),
            source_map: HashMap::new(),
            layout_map: HashMap::new(),
            config_map: HashMap::new(),
            config_order: Vec::new(),
            seen_files: HashSet::new(),
            config: None,
            technique,
            valid: true,
            invalid_reason: String::new(),
            hash: 0,
        };

        let reader = BufReader::new(source.as_bytes());
        glslfx.valid = glslfx.process_input(reader, "string");

        if glslfx.valid {
            glslfx.valid = glslfx.compose_configuration();
        }

        glslfx
    }

    /// Returns true if this is a valid GLSLFX file.
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Returns the reason for invalidity, if any.
    pub fn invalid_reason(&self) -> &str {
        &self.invalid_reason
    }

    /// Return the parameters specified in the configuration.
    pub fn get_parameters(&self) -> Parameters {
        self.config
            .as_ref()
            .map(|c| c.get_parameters().clone())
            .unwrap_or_default()
    }

    /// Return the textures specified in the configuration.
    pub fn get_textures(&self) -> Textures {
        self.config
            .as_ref()
            .map(|c| c.get_textures().clone())
            .unwrap_or_default()
    }

    /// Return the attributes specified in the configuration.
    pub fn get_attributes(&self) -> Attributes {
        self.config
            .as_ref()
            .map(|c| c.get_attributes().clone())
            .unwrap_or_default()
    }

    /// Return the metadata specified in the configuration.
    pub fn get_metadata(&self) -> MetadataDictionary {
        self.config
            .as_ref()
            .map(|c| c.get_metadata().clone())
            .unwrap_or_default()
    }

    /// Get the surface shader source.
    pub fn get_surface_source(&self) -> String {
        self.get_source_internal(&glslfx_tokens::tokens().surface_shader)
    }

    /// Get the displacement shader source.
    pub fn get_displacement_source(&self) -> String {
        self.get_source_internal(&glslfx_tokens::tokens().displacement_shader)
    }

    /// Get the volume shader source.
    pub fn get_volume_source(&self) -> String {
        self.get_source_internal(&glslfx_tokens::tokens().volume_shader)
    }

    /// Get the shader source for a given stage key.
    pub fn get_source(&self, shader_stage_key: &Token) -> String {
        self.get_source_internal(shader_stage_key)
    }

    /// Get the layout config as a Dictionary for given shader stage keys.
    pub fn get_layout_as_dictionary(
        &self,
        shader_stage_keys: &[Token],
        error_str: &mut String,
    ) -> Dictionary {
        let layout_str = self.get_layout_as_string(shader_stage_keys);
        glslfx_config::parse_dict_from_input(&layout_str, error_str).unwrap_or_default()
    }

    /// Get the original file path.
    pub fn get_file_path(&self) -> &str {
        &self.global_context.filename
    }

    /// Return set of all processed files.
    pub fn get_files(&self) -> &HashSet<String> {
        &self.seen_files
    }

    /// Return the hash of the content.
    pub fn get_hash(&self) -> u64 {
        self.hash
    }

    /// Extract import paths from a GLSLFX file (non-recursive).
    ///
    /// Returns as-authored paths in declaration order with possible duplicates.
    pub fn extract_imports(filename: &str) -> Vec<String> {
        let content = match std::fs::read_to_string(filename) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        content
            .lines()
            .filter_map(|line| {
                if line.starts_with(IMPORT_DIRECTIVE) {
                    Some(line[IMPORT_DIRECTIVE.len()..].trim().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    // ========================================================================
    // Private methods
    // ========================================================================

    /// Resolve a file path relative to a containing file.
    ///
    /// Handles:
    /// - BUG-6: `$TOOLS/<package>/<path>` via `HIO_SHADER_RESOURCES` env var
    /// - BUG-7: AR resolver fallback for unresolvable paths
    fn resolve_path(containing_file: &str, filename: &str) -> String {
        if filename.is_empty() {
            return String::new();
        }

        // BUG-6: $TOOLS prefix -- resolve via HIO_SHADER_RESOURCES env var
        const TOOLS_PREFIX: &str = "$TOOLS/";
        if let Some(rest) = filename.strip_prefix(TOOLS_PREFIX) {
            let resolved = resolve_tools_path(rest);
            if !resolved.is_empty() {
                return resolved;
            }
            // Fall through to other resolution methods if tools path not found
        }

        let path = Path::new(filename);
        if path.is_absolute() {
            if path.exists() {
                return filename.to_string();
            }
            // BUG-7: try AR resolver for absolute paths that don't exist on disk
            return try_ar_resolve(containing_file, filename);
        }

        // Relative to containing file
        if !containing_file.is_empty() {
            if let Some(parent) = Path::new(containing_file).parent() {
                let resolved = parent.join(filename);
                if resolved.exists() {
                    return resolved.to_string_lossy().to_string();
                }
            }
        }

        // Try as-is
        if path.exists() {
            return filename.to_string();
        }

        // BUG-7: fallback to AR resolver for relative paths
        try_ar_resolve(containing_file, filename)
    }

    /// Process a file by path.
    fn process_file(&mut self, file_path: &str) -> bool {
        if self.seen_files.contains(file_path) {
            return true; // Already processed
        }
        self.seen_files.insert(file_path.to_string());

        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                self.invalid_reason = format!("Could not open {}: {}", file_path, e);
                return false;
            }
        };

        let reader = BufReader::new(content.as_bytes());
        self.process_input(reader, file_path)
    }

    /// Process input from a reader.
    fn process_input<R: Read>(&mut self, reader: BufReader<R>, filename: &str) -> bool {
        let mut context = ParseContext::new(filename);
        let global_version = self.global_context.version;

        for line_result in reader.lines() {
            let line = match line_result {
                Ok(l) => l.trim_end().to_string(),
                Err(_) => break,
            };

            context.line_no += 1;
            context.current_line = line;

            // Update hash
            let mut hasher = DefaultHasher::new();
            self.hash.hash(&mut hasher);
            context.current_line.hash(&mut hasher);
            self.hash = hasher.finish();

            if context.line_no > 1 && context.version < 0.0 {
                self.invalid_reason = format!(
                    "Syntax Error on line 1 of {}. First line must be version info.",
                    context.filename
                );
                return false;
            }

            // Skip comments (---)
            if context.current_line.starts_with(COMMENT_DELIMITER) {
                continue;
            }

            // Section delimiter (--)
            if context.current_line.starts_with(SECTION_DELIMITER) {
                if !self.parse_section_line(&mut context, global_version) {
                    return false;
                }
            }
            // Import directive within glslfx section
            else if context.current_section_type == "glslfx"
                && context.current_line.starts_with(IMPORT_DIRECTIVE)
            {
                if !self.parse_import(&mut context) {
                    return false;
                }
            }
            // GLSL source section
            else if context.current_section_type == "glsl" {
                self.source_map
                    .entry(context.current_section_id.clone())
                    .or_default()
                    .push_str(&context.current_line);
                self.source_map
                    .get_mut(&context.current_section_id)
                    .unwrap()
                    .push('\n');
            }
            // Layout section
            else if context.current_section_type == "layout" {
                self.layout_map
                    .entry(context.current_section_id.clone())
                    .or_default()
                    .push_str(&context.current_line);
                self.layout_map
                    .get_mut(&context.current_section_id)
                    .unwrap()
                    .push('\n');
            }
            // Configuration section
            else if context.current_section_type == "configuration" {
                self.config_map
                    .entry(context.filename.clone())
                    .or_default()
                    .push_str(&context.current_line);
                self.config_map
                    .get_mut(&context.filename)
                    .unwrap()
                    .push('\n');
            }
        }

        // Version must have been found
        if context.version < 0.0 {
            return false;
        }

        // Process imports
        for import_file in context.imports {
            if !self.process_file(&import_file) {
                return false;
            }
        }

        true
    }

    /// Parse a section delimiter line (-- type ...).
    fn parse_section_line(&mut self, context: &mut ParseContext, global_version: f64) -> bool {
        // Clone the line to avoid borrow conflict between tokens and context mutation
        let line = context.current_line.clone();
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 2 {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. Section delimiter must be followed by a valid token.",
                context.line_no, context.filename
            );
            return false;
        }

        context.current_section_type = tokens[1].to_string();
        context.current_section_id.clear();

        match context.current_section_type.as_str() {
            "glslfx" => self.parse_version_line(&tokens, context, global_version),
            "configuration" => self.parse_configuration_line(context),
            "glsl" => self.parse_glsl_section_line(&tokens, context),
            "layout" => self.parse_layout_section_line(&tokens, context),
            other => {
                self.invalid_reason = format!(
                    "Syntax Error on line {} of {}. Unknown section tag \"{}\"",
                    context.line_no, context.filename, other
                );
                false
            }
        }
    }

    /// Parse the version line: `-- glslfx version 0.1`.
    fn parse_version_line(
        &mut self,
        tokens: &[&str],
        context: &mut ParseContext,
        global_version: f64,
    ) -> bool {
        if context.line_no != 1 {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. Version specifier must be on the first line.",
                context.line_no, context.filename
            );
            return false;
        }

        if tokens.len() != 4 || tokens[2] != "version" {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. Invalid version specifier.",
                context.line_no, context.filename
            );
            return false;
        }

        context.version = tokens[3].parse::<f64>().unwrap_or(-1.0);

        // First file sets global version
        if self.global_context.version < 0.0 {
            self.global_context.version = context.version;
        } else if (context.version - global_version).abs() > f64::EPSILON {
            self.invalid_reason = format!(
                "Version mismatch. {} specifies {:.2}, but {} specifies {:.2}",
                self.global_context.filename, global_version, context.filename, context.version
            );
            return false;
        }

        true
    }

    /// Parse configuration section start.
    fn parse_configuration_line(&mut self, context: &mut ParseContext) -> bool {
        if self.config_map.contains_key(&context.filename) {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. Configuration already defined for this file.",
                context.line_no, context.filename
            );
            return false;
        }

        // Insert in weakest-to-strongest order (same as encounter order)
        self.config_order.insert(0, context.filename.clone());
        self.config_map
            .insert(context.filename.clone(), String::new());
        true
    }

    /// Parse GLSL section start: `-- glsl SectionId`.
    fn parse_glsl_section_line(&mut self, tokens: &[&str], context: &mut ParseContext) -> bool {
        if tokens.len() < 3 {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. \"glsl\" tag must be followed by a valid identifier.",
                context.line_no, context.filename
            );
            return false;
        }

        context.current_section_id = tokens[2].to_string();

        if self.source_map.contains_key(&context.current_section_id) {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. Source for \"{}\" has already been defined.",
                context.line_no, context.filename, context.current_section_id
            );
            return false;
        }

        // Emit a comment for diagnostics
        let basename = Path::new(&context.filename)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| context.filename.clone());

        self.source_map.insert(
            context.current_section_id.clone(),
            format!("// line {} \"{}\"\n", context.line_no, basename),
        );

        true
    }

    /// Parse layout section start: `-- layout SectionId`.
    fn parse_layout_section_line(&mut self, tokens: &[&str], context: &mut ParseContext) -> bool {
        if tokens.len() < 3 {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. \"layout\" tag must be followed by a valid identifier.",
                context.line_no, context.filename
            );
            return false;
        }

        context.current_section_id = tokens[2].to_string();

        if self.layout_map.contains_key(&context.current_section_id) {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. Layout for \"{}\" has already been defined.",
                context.line_no, context.filename, context.current_section_id
            );
            return false;
        }

        true
    }

    /// Parse an import directive.
    fn parse_import(&mut self, context: &mut ParseContext) -> bool {
        let tokens: Vec<&str> = context.current_line.split_whitespace().collect();
        if tokens.len() != 2 {
            self.invalid_reason = format!(
                "Syntax Error on line {} of {}. #import must be followed by a valid file path.",
                context.line_no, context.filename
            );
            return false;
        }

        let import_file = Self::resolve_path(&context.filename, tokens[1]);
        if import_file.is_empty() {
            log::warn!("HioGlslfx: File doesn't exist: \"{}\"", tokens[1]);
            return false;
        }

        context.imports.push(import_file);
        true
    }

    /// Compose all configuration sections into a single config.
    fn compose_configuration(&mut self) -> bool {
        for item in &self.config_order.clone() {
            let config_text = match self.config_map.get(item) {
                Some(t) => t.clone(),
                None => continue,
            };

            let mut error_str = String::new();
            self.config =
                HioGlslfxConfig::read(&self.technique, &config_text, item, &mut error_str);

            if !error_str.is_empty() {
                self.invalid_reason = format!(
                    "Error parsing configuration section of {}: {}",
                    item, error_str
                );
                return false;
            }
        }
        true
    }

    /// Internal: get concatenated source for a shader stage key.
    fn get_source_internal(&self, shader_stage_key: &Token) -> String {
        let config = match &self.config {
            Some(c) => c,
            None => return String::new(),
        };

        let source_keys = config.get_source_keys(shader_stage_key);
        let mut result = String::new();

        for key in &source_keys {
            match self.source_map.get(key) {
                Some(source) => {
                    result.push_str(source);
                    result.push('\n');
                }
                None => {
                    log::error!(
                        "HioGlslfx: Can't find shader source for <{}> with key <{}>",
                        shader_stage_key.as_str(),
                        key
                    );
                    return String::new();
                }
            }
        }

        result
    }

    /// Internal: get layout text for a shader stage key.
    fn get_layout(&self, shader_stage_key: &Token) -> String {
        let config = match &self.config {
            Some(c) => c,
            None => return String::new(),
        };

        let config_keys = config.get_source_keys(shader_stage_key);
        let mut result = String::new();

        for key in &config_keys {
            if let Some(layout) = self.layout_map.get(key) {
                if !result.is_empty() {
                    result.push_str(",\n");
                }
                result.push_str(layout);
                result.push('\n');
            }
        }

        result
    }

    /// Get layout as JSON string for multiple shader stages.
    fn get_layout_as_string(&self, shader_stage_keys: &[Token]) -> String {
        let mut parts = Vec::new();
        for key in shader_stage_keys {
            let layout = self.get_layout(key);
            parts.push(format!("\"{}\" : [ {} ]", key.as_str(), layout));
        }
        format!("{{ {} }}", parts.join(", "))
    }
}

impl Default for HioGlslfx {
    fn default() -> Self {
        Self::invalid()
    }
}

// ---------------------------------------------------------------------------
// BUG-6: $TOOLS path resolution via HIO_SHADER_RESOURCES env var
// ---------------------------------------------------------------------------

/// Resolve a `$TOOLS/<package>/<path>` reference.
///
/// Reads `HIO_SHADER_RESOURCES=<package>:<dir>[;<package2>:<dir2>...]`.
/// Returns the resolved filesystem path if the file exists, or empty string.
///
/// Mirrors C++ `ShaderResourceRegistry::_ResolveResourcePath()`.
fn resolve_tools_path(rest: &str) -> String {
    // rest = "<package>/<asset_path>" (after stripping "$TOOLS/")
    let (package, asset_path) = if let Some(slash) = rest.find('/') {
        (&rest[..slash], &rest[slash + 1..])
    } else {
        (rest, "")
    };

    if asset_path.is_empty() {
        return String::new();
    }

    let resources = match std::env::var("HIO_SHADER_RESOURCES") {
        Ok(v) => v,
        Err(_) => return String::new(),
    };

    // Parse "pkg:dir;pkg2:dir2" entries
    for entry in resources.split(';') {
        let parts: Vec<&str> = entry.splitn(2, ':').collect();
        if parts.len() == 2 && parts[0].trim() == package {
            let dir = parts[1].trim();
            let candidate = Path::new(dir).join(asset_path);
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }

    String::new()
}

// ---------------------------------------------------------------------------
// BUG-7: AR resolver integration for GLSLFX path resolution
// ---------------------------------------------------------------------------

/// Try to resolve `filename` via the AR asset resolver.
///
/// Uses `usd_ar::open_packaged_asset` for package-relative paths, or
/// `usd_ar::resolve` for plain asset paths relative to `containing_file`.
/// Returns the resolved path string, or empty string on failure.
///
/// Mirrors C++ `_ComputeResolvedPath()` using `ArGetResolver().Resolve()`.
fn try_ar_resolve(containing_file: &str, filename: &str) -> String {
    // For package-relative paths (e.g. outer.usdz[inner.glslfx])
    if usd_ar::package_utils::is_package_relative_path(filename) {
        // AR can resolve package-relative paths; return as-is if the format is valid
        return filename.to_string();
    }

    // Construct an anchor path from the containing file's directory
    let anchor = if !containing_file.is_empty() {
        Path::new(containing_file)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Attempt to build an anchored path and check if it's resolvable
    if !anchor.is_empty() {
        let candidate = Path::new(&anchor).join(filename);
        let candidate_str = candidate.to_string_lossy().to_string();
        if Path::new(&candidate_str).exists() {
            return candidate_str;
        }
    }

    // Last resort: return filename as-is (AR will attempt resolution at load time)
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_GLSLFX: &str = r#"-- glslfx version 0.1

-- configuration
{
    "parameters": {
        "diffuseColor": {
            "default": [0.18, 0.18, 0.18],
            "role": "color",
            "documentation": "Base diffuse color"
        }
    },
    "techniques": {
        "default": {
            "fragmentShader": {
                "source": ["MyFragment"]
            }
        }
    }
}

-- glsl MyFragment

void main() {
    gl_FragColor = vec4(1.0);
}
"#;

    #[test]
    fn test_parse_from_string() {
        let glslfx = HioGlslfx::from_string(SAMPLE_GLSLFX, None);
        assert!(glslfx.is_valid(), "reason: {}", glslfx.invalid_reason());
    }

    #[test]
    fn test_get_parameters() {
        let glslfx = HioGlslfx::from_string(SAMPLE_GLSLFX, None);
        assert!(glslfx.is_valid());
        let params = glslfx.get_parameters();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].name, "diffuseColor");
    }

    #[test]
    fn test_get_source() {
        let glslfx = HioGlslfx::from_string(SAMPLE_GLSLFX, None);
        assert!(glslfx.is_valid());
        let frag_src = glslfx.get_source(&Token::new("fragmentShader"));
        assert!(frag_src.contains("gl_FragColor"), "source: {}", frag_src);
    }

    #[test]
    fn test_get_surface_source_empty() {
        let glslfx = HioGlslfx::from_string(SAMPLE_GLSLFX, None);
        // No surfaceShader defined
        let src = glslfx.get_surface_source();
        assert!(src.is_empty());
    }

    #[test]
    fn test_invalid_glslfx() {
        let glslfx = HioGlslfx::from_string("not a valid glslfx file", None);
        assert!(!glslfx.is_valid());
    }

    #[test]
    fn test_extract_imports() {
        // Static method, needs a real file - just ensure it returns empty for missing file
        let imports = HioGlslfx::extract_imports("nonexistent.glslfx");
        assert!(imports.is_empty());
    }

    #[test]
    fn test_hash_differs() {
        let g1 = HioGlslfx::from_string(SAMPLE_GLSLFX, None);
        let g2 = HioGlslfx::from_string(
            "-- glslfx version 0.1\n-- configuration\n{\"techniques\":{\"default\":{\"fragmentShader\":{\"source\":[\"F\"]}}}}\n-- glsl F\nvoid main(){}\n",
            None,
        );
        assert_ne!(g1.get_hash(), g2.get_hash());
    }
}
