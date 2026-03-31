//! Parse error types for the USDA text parser.
//!
//! This module provides comprehensive error handling for the text file format
//! parser, including source location tracking for precise error reporting.
//!
//! # Error Categories
//!
//! Errors are categorized by kind:
//! - **Lexical errors**: Invalid characters, unterminated strings
//! - **Syntax errors**: Unexpected tokens, missing delimiters
//! - **Semantic errors**: Invalid values, type mismatches
//!
//! # Location Tracking
//!
//! All errors include precise source location information (line, column, offset)
//! to help users locate and fix issues in their USD files.
//!
//! # Examples
//!
//! ```
//! use usd_sdf::text_parser::{ParseError, ParseErrorKind, SourceLocation};
//!
//! let loc = SourceLocation::new(10, 5, 150);
//! let err = ParseError::new(ParseErrorKind::UnexpectedToken("def".into()), loc);
//! assert_eq!(err.location().line(), 10);
//! ```

use std::fmt;

// ============================================================================
// Source Location
// ============================================================================

/// A location in source code.
///
/// Tracks line number, column number, and byte offset for precise error
/// reporting. Line and column numbers are 1-based for human readability.
///
/// # Thread Safety
///
/// `SourceLocation` is `Copy` and can be safely shared across threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceLocation {
    /// Line number (1-based). May be 0 if deferred (computed lazily on error).
    line: usize,
    /// Column number (1-based). May be 0 if deferred.
    column: usize,
    /// Byte offset from start of source.
    offset: usize,
}

impl SourceLocation {
    /// Creates a deferred location (byte offset only, line/column computed on demand).
    #[inline]
    #[must_use]
    pub const fn from_offset(offset: usize) -> Self {
        Self {
            line: 0,
            column: 0,
            offset,
        }
    }
}

impl SourceLocation {
    /// Creates a new source location.
    ///
    /// # Parameters
    ///
    /// - `line` - Line number (1-based)
    /// - `column` - Column number (1-based)
    /// - `offset` - Byte offset from start
    #[inline]
    #[must_use]
    pub const fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }

    /// Creates an unknown/invalid location.
    #[inline]
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            line: 0,
            column: 0,
            offset: 0,
        }
    }

    /// Returns the line number (1-based).
    #[inline]
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// Returns the column number (1-based).
    #[inline]
    #[must_use]
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Returns the byte offset from start.
    #[inline]
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns true if this is a valid location.
    #[inline]
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.line > 0
    }
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_valid() {
            write!(f, "line {}, column {}", self.line, self.column)
        } else {
            write!(f, "unknown location")
        }
    }
}

// ============================================================================
// Source Span
// ============================================================================

/// A range in source code.
///
/// Represents a contiguous region of source text, useful for highlighting
/// errors or extracting source fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    /// Start location.
    start: SourceLocation,
    /// End location (exclusive).
    end: SourceLocation,
}

impl SourceSpan {
    /// Creates a new source span.
    #[inline]
    #[must_use]
    pub const fn new(start: SourceLocation, end: SourceLocation) -> Self {
        Self { start, end }
    }

    /// Creates a span from a single location (zero-width).
    #[inline]
    #[must_use]
    pub const fn point(loc: SourceLocation) -> Self {
        Self {
            start: loc,
            end: loc,
        }
    }

    /// Returns the start location.
    #[inline]
    #[must_use]
    pub const fn start(&self) -> SourceLocation {
        self.start
    }

    /// Returns the end location.
    #[inline]
    #[must_use]
    pub const fn end(&self) -> SourceLocation {
        self.end
    }

    /// Returns the byte length of the span.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.offset.saturating_sub(self.start.offset)
    }

    /// Returns true if the span is empty.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start.offset >= self.end.offset
    }
}

// ============================================================================
// Parse Error Kind
// ============================================================================

/// The kind of parse error that occurred.
///
/// Categorizes errors for appropriate handling and messaging.
/// Each variant includes relevant context data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    // ========================================================================
    // Lexical Errors
    // ========================================================================
    /// Invalid character encountered.
    InvalidCharacter(char),

    /// Unterminated string literal.
    UnterminatedString,

    /// Unterminated multi-line comment.
    UnterminatedComment,

    /// Unterminated asset reference (@...@).
    UnterminatedAssetRef,

    /// Invalid escape sequence in string.
    InvalidEscapeSequence(String),

    /// Invalid number format.
    InvalidNumber(String),

    /// Invalid UTF-8 sequence.
    InvalidUtf8,

    // ========================================================================
    // Syntax Errors
    // ========================================================================
    /// Unexpected token encountered.
    UnexpectedToken(String),

    /// Expected a specific token.
    ExpectedToken {
        /// The token that was expected.
        expected: String,
        /// The token that was found instead.
        found: String,
    },

    /// Expected an identifier.
    ExpectedIdentifier,

    /// Expected a keyword.
    ExpectedKeyword(String),

    /// Expected a string literal.
    ExpectedString,

    /// Expected a number.
    ExpectedNumber,

    /// Expected a value.
    ExpectedValue,

    /// Expected a path reference.
    ExpectedPathRef,

    /// Missing closing delimiter.
    MissingClosingDelimiter(char),

    /// Missing opening delimiter.
    MissingOpeningDelimiter(char),

    /// Unexpected end of file.
    UnexpectedEof,

    // ========================================================================
    // Header Errors
    // ========================================================================
    /// Invalid or missing file header.
    InvalidHeader(String),

    /// Unsupported file format version.
    UnsupportedVersion(String),

    /// Invalid magic identifier (expected #usda or #sdf).
    InvalidMagic(String),

    // ========================================================================
    // Semantic Errors
    // ========================================================================
    /// Invalid path syntax.
    InvalidPath(String),

    /// Invalid prim specifier.
    InvalidSpecifier(String),

    /// Invalid variability.
    InvalidVariability(String),

    /// Invalid permission.
    InvalidPermission(String),

    /// Invalid type name.
    InvalidTypeName(String),

    /// Invalid metadata key.
    InvalidMetadataKey(String),

    /// Invalid metadata value.
    InvalidMetadataValue(String),

    /// Duplicate metadata key.
    DuplicateMetadataKey(String),

    /// Invalid list operation.
    InvalidListOp(String),

    /// Invalid time sample.
    InvalidTimeSample(String),

    /// Invalid layer offset.
    InvalidLayerOffset(String),

    /// Invalid reference.
    InvalidReference(String),

    /// Invalid payload.
    InvalidPayload(String),

    /// Invalid variant.
    InvalidVariant(String),

    // ========================================================================
    // Structural Errors
    // ========================================================================
    /// Nested structure too deep.
    NestingTooDeep {
        /// Maximum allowed nesting depth.
        max_depth: usize,
    },

    /// Invalid nesting (e.g., property inside property).
    InvalidNesting(String),

    /// Duplicate definition.
    DuplicateDefinition(String),

    // ========================================================================
    // I/O Errors
    // ========================================================================
    /// File read error.
    IoError(String),

    // ========================================================================
    // Internal Errors
    // ========================================================================
    /// Internal parser error (bug).
    Internal(String),

    /// Custom error with message.
    Custom(String),
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Lexical
            Self::InvalidCharacter(c) => write!(f, "invalid character '{}'", c.escape_default()),
            Self::UnterminatedString => write!(f, "unterminated string literal"),
            Self::UnterminatedComment => write!(f, "unterminated comment"),
            Self::UnterminatedAssetRef => write!(f, "unterminated asset reference"),
            Self::InvalidEscapeSequence(s) => write!(f, "invalid escape sequence: {}", s),
            Self::InvalidNumber(s) => write!(f, "invalid number: {}", s),
            Self::InvalidUtf8 => write!(f, "invalid UTF-8 sequence"),

            // Syntax
            Self::UnexpectedToken(t) => write!(f, "unexpected token: {}", t),
            Self::ExpectedToken { expected, found } => {
                write!(f, "expected {}, found {}", expected, found)
            }
            Self::ExpectedIdentifier => write!(f, "expected identifier"),
            Self::ExpectedKeyword(kw) => write!(f, "expected keyword: {}", kw),
            Self::ExpectedString => write!(f, "expected string"),
            Self::ExpectedNumber => write!(f, "expected number"),
            Self::ExpectedValue => write!(f, "expected value"),
            Self::ExpectedPathRef => write!(f, "expected path reference"),
            Self::MissingClosingDelimiter(c) => write!(f, "missing closing '{}'", c),
            Self::MissingOpeningDelimiter(c) => write!(f, "missing opening '{}'", c),
            Self::UnexpectedEof => write!(f, "unexpected end of file"),

            // Header
            Self::InvalidHeader(s) => write!(f, "invalid header: {}", s),
            Self::UnsupportedVersion(v) => write!(f, "unsupported version: {}", v),
            Self::InvalidMagic(s) => write!(f, "invalid magic identifier: {}", s),

            // Semantic
            Self::InvalidPath(s) => write!(f, "invalid path: {}", s),
            Self::InvalidSpecifier(s) => write!(f, "invalid specifier: {}", s),
            Self::InvalidVariability(s) => write!(f, "invalid variability: {}", s),
            Self::InvalidPermission(s) => write!(f, "invalid permission: {}", s),
            Self::InvalidTypeName(s) => write!(f, "invalid type name: {}", s),
            Self::InvalidMetadataKey(s) => write!(f, "invalid metadata key: {}", s),
            Self::InvalidMetadataValue(s) => write!(f, "invalid metadata value: {}", s),
            Self::DuplicateMetadataKey(s) => write!(f, "duplicate metadata key: {}", s),
            Self::InvalidListOp(s) => write!(f, "invalid list operation: {}", s),
            Self::InvalidTimeSample(s) => write!(f, "invalid time sample: {}", s),
            Self::InvalidLayerOffset(s) => write!(f, "invalid layer offset: {}", s),
            Self::InvalidReference(s) => write!(f, "invalid reference: {}", s),
            Self::InvalidPayload(s) => write!(f, "invalid payload: {}", s),
            Self::InvalidVariant(s) => write!(f, "invalid variant: {}", s),

            // Structural
            Self::NestingTooDeep { max_depth } => {
                write!(f, "nesting too deep (max {})", max_depth)
            }
            Self::InvalidNesting(s) => write!(f, "invalid nesting: {}", s),
            Self::DuplicateDefinition(s) => write!(f, "duplicate definition: {}", s),

            // I/O
            Self::IoError(s) => write!(f, "I/O error: {}", s),

            // Internal
            Self::Internal(s) => write!(f, "internal error: {}", s),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

// ============================================================================
// Parse Error
// ============================================================================

/// A parse error with location information.
///
/// Combines an error kind with source location for precise error reporting.
/// Optionally includes the file context (path) where the error occurred.
///
/// # Examples
///
/// ```ignore
/// use usd_sdf::text_parser::{ParseError, ParseErrorKind, SourceLocation};
///
/// let err = ParseError::new(
///     ParseErrorKind::UnexpectedEof,
///     SourceLocation::new(42, 1, 1000)
/// );
///
/// println!("{}", err); // "line 42, column 1: unexpected end of file"
/// ```
#[derive(Debug, Clone)]
pub struct ParseError {
    /// The kind of error.
    kind: ParseErrorKind,
    /// Location in source where error occurred.
    location: SourceLocation,
    /// Optional file context (path).
    file_context: Option<String>,
    /// Optional additional context message.
    context: Option<String>,
}

impl ParseError {
    /// Creates a new parse error.
    #[inline]
    #[must_use]
    pub fn new(kind: ParseErrorKind, location: SourceLocation) -> Self {
        Self {
            kind,
            location,
            file_context: None,
            context: None,
        }
    }

    /// Creates an error at an unknown location.
    #[inline]
    #[must_use]
    pub fn at_unknown(kind: ParseErrorKind) -> Self {
        Self::new(kind, SourceLocation::unknown())
    }

    /// Adds file context to the error.
    #[inline]
    #[must_use]
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file_context = Some(file.into());
        self
    }

    /// Adds context message to the error.
    #[inline]
    #[must_use]
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Returns the error kind.
    #[inline]
    #[must_use]
    pub fn kind(&self) -> &ParseErrorKind {
        &self.kind
    }

    /// Returns the source location.
    #[inline]
    #[must_use]
    pub fn location(&self) -> SourceLocation {
        self.location
    }

    /// Returns the file context if set.
    #[inline]
    #[must_use]
    pub fn file_context(&self) -> Option<&str> {
        self.file_context.as_deref()
    }

    /// Returns the additional context if set.
    #[inline]
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        self.context.as_deref()
    }

    /// Returns true if this is a lexical error.
    #[must_use]
    pub fn is_lexical(&self) -> bool {
        matches!(
            self.kind,
            ParseErrorKind::InvalidCharacter(_)
                | ParseErrorKind::UnterminatedString
                | ParseErrorKind::UnterminatedComment
                | ParseErrorKind::UnterminatedAssetRef
                | ParseErrorKind::InvalidEscapeSequence(_)
                | ParseErrorKind::InvalidNumber(_)
                | ParseErrorKind::InvalidUtf8
        )
    }

    /// Returns true if this is a syntax error.
    #[must_use]
    pub fn is_syntax(&self) -> bool {
        matches!(
            self.kind,
            ParseErrorKind::UnexpectedToken(_)
                | ParseErrorKind::ExpectedToken { .. }
                | ParseErrorKind::ExpectedIdentifier
                | ParseErrorKind::ExpectedString
                | ParseErrorKind::ExpectedNumber
                | ParseErrorKind::ExpectedValue
                | ParseErrorKind::ExpectedPathRef
                | ParseErrorKind::MissingClosingDelimiter(_)
                | ParseErrorKind::MissingOpeningDelimiter(_)
                | ParseErrorKind::UnexpectedEof
        )
    }

    /// Returns true if this is a semantic error.
    #[must_use]
    pub fn is_semantic(&self) -> bool {
        matches!(
            self.kind,
            ParseErrorKind::InvalidPath(_)
                | ParseErrorKind::InvalidSpecifier(_)
                | ParseErrorKind::InvalidVariability(_)
                | ParseErrorKind::InvalidPermission(_)
                | ParseErrorKind::InvalidTypeName(_)
                | ParseErrorKind::InvalidMetadataKey(_)
                | ParseErrorKind::InvalidMetadataValue(_)
                | ParseErrorKind::DuplicateMetadataKey(_)
                | ParseErrorKind::InvalidListOp(_)
                | ParseErrorKind::InvalidTimeSample(_)
                | ParseErrorKind::InvalidLayerOffset(_)
                | ParseErrorKind::InvalidReference(_)
                | ParseErrorKind::InvalidPayload(_)
                | ParseErrorKind::InvalidVariant(_)
        )
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // File context
        if let Some(file) = &self.file_context {
            write!(f, "{}:", file)?;
        }

        // Location
        if self.location.is_valid() {
            write!(f, "{}: ", self.location)?;
        }

        // Error kind
        write!(f, "{}", self.kind)?;

        // Additional context
        if let Some(ctx) = &self.context {
            write!(f, " ({})", ctx)?;
        }

        Ok(())
    }
}

impl std::error::Error for ParseError {}

// ============================================================================
// Parse Result
// ============================================================================

/// Result type for parsing operations.
pub type ParseResult<T> = Result<T, ParseError>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_location() {
        let loc = SourceLocation::new(10, 5, 100);
        assert_eq!(loc.line(), 10);
        assert_eq!(loc.column(), 5);
        assert_eq!(loc.offset(), 100);
        assert!(loc.is_valid());
    }

    #[test]
    fn test_unknown_location() {
        let loc = SourceLocation::unknown();
        assert!(!loc.is_valid());
        assert_eq!(loc.line(), 0);
    }

    #[test]
    fn test_source_span() {
        let start = SourceLocation::new(1, 1, 0);
        let end = SourceLocation::new(1, 10, 9);
        let span = SourceSpan::new(start, end);

        assert_eq!(span.len(), 9);
        assert!(!span.is_empty());
    }

    #[test]
    fn test_parse_error_display() {
        let err = ParseError::new(
            ParseErrorKind::UnexpectedToken("foo".into()),
            SourceLocation::new(5, 3, 42),
        );

        let msg = format!("{}", err);
        assert!(msg.contains("line 5"));
        assert!(msg.contains("column 3"));
        assert!(msg.contains("unexpected token"));
        assert!(msg.contains("foo"));
    }

    #[test]
    fn test_parse_error_with_file() {
        let err = ParseError::new(ParseErrorKind::UnexpectedEof, SourceLocation::new(1, 1, 0))
            .with_file("test.usda");

        let msg = format!("{}", err);
        assert!(msg.contains("test.usda"));
    }

    #[test]
    fn test_error_categories() {
        let lexical = ParseError::at_unknown(ParseErrorKind::InvalidCharacter('$'));
        assert!(lexical.is_lexical());
        assert!(!lexical.is_syntax());

        let syntax = ParseError::at_unknown(ParseErrorKind::UnexpectedEof);
        assert!(syntax.is_syntax());
        assert!(!syntax.is_lexical());

        let semantic = ParseError::at_unknown(ParseErrorKind::InvalidPath("/bad".into()));
        assert!(semantic.is_semantic());
    }
}
