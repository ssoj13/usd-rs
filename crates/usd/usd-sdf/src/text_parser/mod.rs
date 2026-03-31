//! Text file format parser for USDA files.
//!
//! This module implements parsing of USD text file format (.usda files),
//! which is the human-readable representation of USD layers.
//!
//! # Architecture
//!
//! The parser is organized into several components:
//!
//! - **Lexer** (`lexer/`): Tokenizes input into a stream of tokens
//! - **Context** (`context.rs`): Parser state matching C++ `Sdf_TextParserContext`
//! - **ValueContext** (`value_context.rs`): Value building matching C++ `Sdf_ParserValueContext`
//! - **Tokens** (`tokens.rs`): Token types and keywords
//! - **Error** (`error.rs`): Error types with source locations
//!
//! # C++ Parity
//!
//! This implementation mirrors the C++ PEGTL-based parser from OpenUSD:
//! - `textFileFormatParser.h` - Grammar rules (~1725 lines)
//! - `textParserContext.h` - Parser state (~238 lines)
//! - `parserValueContext.h` - Value context (~127 lines)
//! - `parserHelpers.h` - Helper functions (~372 lines)
//!
//! # Usage
//!
//! ```rust,ignore
//! use usd_sdf::text_parser::{parse_layer, TextParserContext};
//!
//! let content = r#"#usda 1.0
//! def Xform "World" {
//!     def Cube "MyCube" {}
//! }
//! "#;
//!
//! let layer = parse_layer(content)?;
//! ```

// Sub-modules - Foundation (Phase 5A)
pub mod context;
pub mod error;
pub mod tokens;
pub mod value_context;

// Lexer module (Phase 5B)
pub mod lexer;

// Values parsing module (Phase 5C)
pub mod values;

// Metadata parsing module (Phase 5D)
pub mod metadata;

// Specs parsing module (Phase 5E)
pub mod specs;

// Grammar module (Phase 5F)
pub mod grammar;

// Re-exports for convenience
pub use context::{ParsedValue, ParsingContext, TextParserContext};
pub use error::{ParseError, ParseErrorKind, ParseResult, SourceLocation, SourceSpan};
pub use lexer::Lexer;
pub use metadata::{Metadata, MetadataEntry, MetadataParser};
pub use tokens::{Keyword, Token, TokenKind};
pub use value_context::{
    ArrayEdit, ArrayEditOp, ProducedValue, TupleDimensions, Value, ValueContext, ValueFactory,
};
pub use values::{TimeSample, TimeSampleMap, ValueParser};
// SplineValue is now available from usd_vt::spline - import directly when needed
pub use grammar::{
    LayerHeader, LayerParser, ParsedLayer, parse_layer_header_and_metadata, parse_layer_text,
};
pub use specs::{
    ParsedAttributeSpec, ParsedPrimSpec, ParsedPropertySpec, ParsedRelationshipSpec, Specifier,
    SpecsParser, Variability,
};

// ============================================================================
// Parser Trait
// ============================================================================

/// Trait for parsing USD text content.
///
/// This trait defines the interface for text file parsing, allowing for
/// different parser implementations (e.g., streaming vs. in-memory).
pub trait TextParser {
    /// Parses the given content and returns a parsed layer structure.
    ///
    /// # Errors
    ///
    /// Returns a `ParseError` if the content is not valid USD text format.
    fn parse(&mut self, content: &str) -> ParseResult<ParsedLayer>;

    /// Parses only the metadata portion of the content.
    ///
    /// This is useful for quickly reading layer hints without parsing
    /// the full structure.
    ///
    /// # Errors
    ///
    /// Returns a `ParseError` if the header/metadata is invalid.
    fn parse_metadata_only(&mut self, content: &str) -> ParseResult<()>;
}

// ============================================================================
// Public API
// ============================================================================

/// Parses a complete layer from USD text format.
///
/// This is the main entry point for parsing .usda files.
///
/// # Arguments
///
/// * `content` - The text content to parse
///
/// # Returns
///
/// A `Layer` containing all the parsed data, or a `ParseError` if parsing fails.
///
/// # Examples
///
/// ```rust,ignore
/// let layer = parse_layer(r#"#usda 1.0
/// def Xform "World" {}
/// "#)?;
/// ```
///
/// # Errors
///
/// Returns `ParseError` for:
/// - Invalid header (missing #usda magic)
/// - Syntax errors (unexpected tokens, missing delimiters)
/// - Semantic errors (invalid paths, type mismatches)
pub fn parse_layer(content: &str) -> ParseResult<ParsedLayer> {
    // Parse the text content using the grammar module
    // Returns ParsedLayer which can be used to build a Layer via UsdaFileFormat
    grammar::parse_layer_text(content)
}

/// Parses a layer from text, setting it as the content of an existing layer.
///
/// Unlike `parse_layer`, this modifies an existing layer in place.
///
/// # Arguments
///
/// * `layer` - The layer to populate with parsed content
/// * `content` - The text content to parse
/// * `file_path` - Optional file path for error reporting
///
/// # Errors
///
/// Returns `ParseError` if parsing fails.
pub fn parse_layer_from_string(content: &str, file_path: Option<&str>) -> ParseResult<ParsedLayer> {
    // Parse the text content with optional file path for error reporting
    let mut parser = if let Some(path) = file_path {
        grammar::LayerParser::with_file_path(content, path)
    } else {
        grammar::LayerParser::new(content)
    };

    parser.parse()
}

/// Parses only the layer metadata (header and layer-level metadata).
///
/// This is a fast path for reading layer hints without parsing the full
/// prim hierarchy. Useful for layer sniffing and dependency resolution.
///
/// # Arguments
///
/// * `content` - The text content to parse
///
/// # Returns
///
/// A `TextParserContext` containing the parsed metadata, or error.
///
/// # Examples
///
/// ```rust,ignore
/// let ctx = parse_layer_metadata_only(content)?;
/// println!("Version: {}", ctx.version_string);
/// println!("Sublayers: {:?}", ctx.sublayer_paths);
/// ```
pub fn parse_layer_metadata_only(content: &str) -> ParseResult<TextParserContext> {
    // Use the fast path that only parses header and metadata
    let (header, metadata) = grammar::parse_layer_header_and_metadata(content)?;

    // Build a TextParserContext from the parsed data
    let mut ctx = TextParserContext::default();
    ctx.version_string = format!("{} {}", header.format, header.version);

    if let Some(meta) = metadata {
        // Extract sublayer paths from both plain KeyValue and ListOp forms
        for entry in &meta.entries {
            let (key, value) = match entry {
                MetadataEntry::KeyValue { key, value } => (key.as_str(), value),
                MetadataEntry::ListOp { key, value, .. } => (key.as_str(), value),
                _ => continue,
            };
            if key == "subLayers" {
                match value {
                    Value::SubLayerList(items) => {
                        for (path, _, _) in items {
                            ctx.sublayer_paths.push(path.clone());
                        }
                    }
                    Value::List(items) => {
                        for item in items {
                            if let Value::AssetPath(path) = item {
                                ctx.sublayer_paths.push(path.clone());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // Extract other commonly needed metadata
        if let Some(v) = meta.get("defaultPrim").and_then(|v| v.as_string()) {
            ctx.default_prim = Some(v.to_string());
        }
    }

    Ok(ctx)
}

/// Checks if a string looks like it could be valid USD text format.
///
/// Performs a quick header check without full parsing.
/// Returns true if the content starts with a valid USD magic identifier.
///
/// # Arguments
///
/// * `content` - The content to check
///
/// # Examples
///
/// ```rust,ignore
/// assert!(is_usda_content("#usda 1.0\n..."));
/// assert!(!is_usda_content("not a usd file"));
/// ```
#[must_use]
pub fn is_usda_content(content: &str) -> bool {
    let trimmed = content.trim_start();
    trimmed.starts_with("#usda") || trimmed.starts_with("#sdf")
}

/// Validates USD text content without creating a layer.
///
/// Parses the content and reports any errors, but doesn't build
/// the full layer structure. Useful for syntax checking.
///
/// # Returns
///
/// `Ok(())` if the content is valid, or `Err(ParseError)` with details.
pub fn validate_usda_content(content: &str) -> ParseResult<()> {
    // Use the grammar module's validation function
    grammar::validate_layer_text(content)
}

// ============================================================================
// Helper Functions (matching parserHelpers.h)
// ============================================================================

/// Evaluates a quoted string, handling escape sequences.
///
/// Matches C++ `Sdf_EvalQuotedString`.
///
/// # Arguments
///
/// * `s` - The raw string content (without quotes)
/// * `trim_both_sides` - Number of quote characters to trim from each side
///
/// # Returns
///
/// The unescaped string content.
pub fn eval_quoted_string(s: &str, trim_both_sides: usize) -> String {
    let trimmed = if trim_both_sides > 0 && s.len() >= 2 * trim_both_sides {
        &s[trim_both_sides..s.len() - trim_both_sides]
    } else {
        s
    };

    let mut result = String::with_capacity(trimmed.len());
    let mut chars = trimmed.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('r') => result.push('\r'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('\'') => result.push('\''),
                Some('0') => result.push('\0'),
                Some('x') => {
                    // Hex escape \xNN
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&c) = chars.peek() {
                            if c.is_ascii_hexdigit() {
                                hex.push(chars.next().expect("peeked"));
                            }
                        }
                    }
                    if let Ok(code) = u8::from_str_radix(&hex, 16) {
                        result.push(code as char);
                    }
                }
                Some(c) => {
                    result.push('\\');
                    result.push(c);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Evaluates an asset path string.
///
/// Matches C++ `Sdf_EvalAssetPath`.
///
/// # Arguments
///
/// * `s` - The raw asset path content
/// * `triple_delimited` - Whether the path uses @@@ delimiters
pub fn eval_asset_path(s: &str, triple_delimited: bool) -> String {
    let delim_len = if triple_delimited { 3 } else { 1 };

    if s.len() >= 2 * delim_len {
        let trimmed = &s[delim_len..s.len() - delim_len];

        if triple_delimited {
            // Handle escaped @@@ in triple-delimited paths
            trimmed.replace("\\@@@", "@@@")
        } else {
            trimmed.to_string()
        }
    } else {
        s.to_string()
    }
}

/// Converts a string to a bool.
///
/// Matches C++ `Sdf_BoolFromString`.
/// Accepts case insensitive: "yes", "no", "false", "true", "0", "1", "on", "off".
///
/// # Returns
///
/// `Some(bool)` if recognized, `None` otherwise.
#[must_use]
pub fn bool_from_string(s: &str) -> Option<bool> {
    match s.to_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_usda_content() {
        assert!(is_usda_content("#usda 1.0\n"));
        assert!(is_usda_content("  #usda 1.0\n"));
        assert!(is_usda_content("#sdf 1.4.32\n"));
        assert!(!is_usda_content("not usd"));
        assert!(!is_usda_content(""));
        assert!(!is_usda_content("# comment"));
    }

    #[test]
    fn test_context_reexport() {
        // Verify re-exports work
        let _ctx = TextParserContext::new();
        let _loc = SourceLocation::new(1, 1, 0);
        let _kw = Keyword::Def;
    }

    #[test]
    fn test_parse_layer() {
        // Parse minimal valid layer
        let result = parse_layer("#usda 1.0\n");
        assert!(result.is_ok());
        let parsed = result.unwrap();
        assert_eq!(parsed.header.format, "usda");
        assert_eq!(parsed.header.version, "1.0");
    }

    #[test]
    fn test_eval_quoted_string() {
        assert_eq!(eval_quoted_string("hello", 0), "hello");
        assert_eq!(eval_quoted_string("hello\\nworld", 0), "hello\nworld");
        assert_eq!(eval_quoted_string("tab\\there", 0), "tab\there");
        assert_eq!(eval_quoted_string("quote\\\"here", 0), "quote\"here");
    }

    #[test]
    fn test_eval_asset_path() {
        assert_eq!(eval_asset_path("@./path@", false), "./path");
        assert_eq!(eval_asset_path("@@@./path@@@", true), "./path");
    }

    #[test]
    fn test_bool_from_string() {
        assert_eq!(bool_from_string("true"), Some(true));
        assert_eq!(bool_from_string("TRUE"), Some(true));
        assert_eq!(bool_from_string("yes"), Some(true));
        assert_eq!(bool_from_string("1"), Some(true));
        assert_eq!(bool_from_string("false"), Some(false));
        assert_eq!(bool_from_string("no"), Some(false));
        assert_eq!(bool_from_string("0"), Some(false));
        assert_eq!(bool_from_string("maybe"), None);
    }

    #[test]
    fn test_lexer_available() {
        let mut lexer = Lexer::new("def Xform \"test\" {}");
        let token = lexer.next_token();
        assert!(token.is_some());
    }

    #[test]
    fn test_value_context_available() {
        let mut ctx = ValueContext::new();
        ctx.setup_factory("float");
        assert!(ctx.value_type_is_valid);
    }
}
