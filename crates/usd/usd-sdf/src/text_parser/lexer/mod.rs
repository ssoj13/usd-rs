//! Lexer for the USDA text file format.
//!
//! This module provides tokenization of USD text files. The lexer converts
//! raw source text into a stream of tokens that can be consumed by the parser.
//!
//! # Architecture
//!
//! The lexer is split into several components:
//! - `Lexer` - Main lexer struct that tracks position and produces tokens
//! - `chars` - Character classification utilities (identifiers, whitespace, etc.)
//! - `tokenizer` - Token scanning logic for each token type
//!
//! # C++ Parity
//!
//! This implementation mirrors the PEGTL character classes and token rules
//! from `textFileFormatParser.h`. Key considerations:
//! - UTF-8 identifiers using Unicode XID properties
//! - Multiple string quote styles (single, double, triple)
//! - Asset references with @ and @@@ delimiters
//! - Python-style (#) and C++-style (//, /* */) comments
//!
//! # Usage
//!
//! ```rust,ignore
//! use usd_sdf::text_parser::lexer::Lexer;
//!
//! let mut lexer = Lexer::new(source);
//! while let Some(token) = lexer.next_token() {
//!     println!("{:?}", token);
//! }
//! ```

pub mod chars;
pub mod tokenizer;

use super::error::{ParseError, ParseErrorKind, SourceLocation, SourceSpan};
use super::tokens::{Token, TokenKind};

// Performance counters for parse diagnostics
use std::cell::Cell;
thread_local! {
    static TOKEN_COUNT: Cell<u64> = const { Cell::new(0) };
    static WHITESPACE_BYTES: Cell<u64> = const { Cell::new(0) };
}

/// Resets and returns (token_count, whitespace_bytes) from last parse.
pub fn take_perf_counters() -> (u64, u64) {
    let tokens = TOKEN_COUNT.with(|c| c.replace(0));
    let ws = WHITESPACE_BYTES.with(|c| c.replace(0));
    (tokens, ws)
}

// ============================================================================
// Lexer
// ============================================================================

/// The lexer for USD text files.
///
/// Converts source text into a stream of tokens. Tracks position information
/// for error reporting.
///
/// # Thread Safety
///
/// The lexer is not thread-safe. Each thread should have its own lexer instance.
#[derive(Debug, Clone)]
pub struct Lexer<'a> {
    /// Source text being lexed.
    source: &'a str,
    /// Source as bytes for efficient access.
    bytes: &'a [u8],
    /// Current byte position in source.
    position: usize,
    /// Start position of current token.
    token_start: usize,
    /// Whether we've reached end of file.
    at_eof: bool,
    /// File path for error reporting (optional).
    file_path: Option<String>,
}

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given source text.
    ///
    /// # Arguments
    ///
    /// * `source` - The USD text content to tokenize
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let lexer = Lexer::new("#usda 1.0\ndef Xform \"World\" {}");
    /// ```
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            position: 0,
            token_start: 0,
            at_eof: source.is_empty(),
            file_path: None,
        }
    }

    /// Sets the file path for error reporting.
    #[must_use]
    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Returns the current source location (deferred line/column, computed on demand).
    #[inline]
    #[must_use]
    pub fn current_location(&self) -> SourceLocation {
        SourceLocation::from_offset(self.position)
    }

    /// Returns the location where the current token started.
    #[inline]
    #[must_use]
    pub fn token_start_location(&self) -> SourceLocation {
        SourceLocation::from_offset(self.token_start)
    }

    /// Computes line/column from a byte offset by scanning source text.
    /// Only called on error paths.
    pub fn resolve_location(&self, offset: usize) -> SourceLocation {
        let mut line = 1usize;
        let mut col = 1usize;
        for &b in &self.bytes[..offset.min(self.bytes.len())] {
            if b == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        SourceLocation::new(line, col, offset)
    }

    /// Returns true if we've reached the end of the source.
    #[inline]
    #[must_use]
    pub fn is_at_end(&self) -> bool {
        self.position >= self.bytes.len()
    }

    /// Returns the current byte without advancing.
    #[inline]
    #[must_use]
    pub fn peek(&self) -> Option<u8> {
        self.bytes.get(self.position).copied()
    }

    /// Returns the byte at offset from current position.
    #[inline]
    #[must_use]
    pub fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.position + offset).copied()
    }

    /// Returns the current character (UTF-8 aware).
    #[must_use]
    pub fn peek_char(&self) -> Option<char> {
        if self.is_at_end() {
            return None;
        }
        self.source[self.position..].chars().next()
    }

    /// Advances by one byte and returns it.
    #[inline]
    pub fn advance(&mut self) -> Option<u8> {
        if self.is_at_end() {
            return None;
        }
        let byte = self.bytes[self.position];
        self.position += 1;
        Some(byte)
    }

    /// Advances by `n` bytes.
    #[inline]
    pub fn advance_bytes_no_newline(&mut self, n: usize) {
        self.position += n;
    }

    /// Fast advance while predicate holds.
    #[inline]
    pub fn advance_while_no_newline(&mut self, predicate: impl Fn(u8) -> bool) {
        while self.position < self.bytes.len() && predicate(self.bytes[self.position]) {
            self.position += 1;
        }
    }

    /// Advances by one character (UTF-8 aware) and returns it.
    pub fn advance_char(&mut self) -> Option<char> {
        if self.is_at_end() {
            return None;
        }
        let ch = self.source[self.position..].chars().next()?;
        self.position += ch.len_utf8();
        Some(ch)
    }

    /// Advances if current byte matches expected.
    pub fn advance_if(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Advances if current byte matches predicate.
    #[inline]
    pub fn advance_while(&mut self, predicate: impl Fn(u8) -> bool) {
        while let Some(b) = self.peek() {
            if predicate(b) {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Advances while predicate is true for characters (UTF-8 aware).
    pub fn advance_while_char(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(ch) = self.peek_char() {
            if predicate(ch) {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    /// Marks the start of a new token.
    #[inline]
    pub fn mark_token_start(&mut self) {
        self.token_start = self.position;
    }

    /// Returns the text of the current token (from mark to current position).
    #[must_use]
    pub fn token_text(&self) -> &'a str {
        &self.source[self.token_start..self.position]
    }

    /// Returns the span of the current token.
    #[must_use]
    pub fn token_span(&self) -> SourceSpan {
        SourceSpan::new(self.token_start_location(), self.current_location())
    }

    /// Creates a token with the current span.
    #[must_use]
    pub fn make_token(&self, kind: TokenKind) -> Token {
        Token::with_lexeme(kind, self.token_span(), self.token_text())
    }

    /// Creates an error token.
    #[must_use]
    pub fn error_token(&self, message: impl Into<String>) -> Token {
        Token::new(TokenKind::Error(message.into()), self.token_span())
    }

    /// Creates a parse error at current location.
    #[must_use]
    pub fn make_error(&self, kind: ParseErrorKind) -> ParseError {
        let mut err = ParseError::new(kind, self.token_start_location());
        if let Some(ref path) = self.file_path {
            err = err.with_file(path.clone());
        }
        err
    }

    /// Scans the next token.
    ///
    /// Returns `None` at end of file.
    pub fn next_token(&mut self) -> Option<Token> {
        self.skip_whitespace_and_comments();

        if self.is_at_end() {
            if !self.at_eof {
                self.at_eof = true;
                self.mark_token_start();
                return Some(self.make_token(TokenKind::Eof));
            }
            return None;
        }

        self.mark_token_start();
        TOKEN_COUNT.with(|c| c.set(c.get() + 1));
        tokenizer::scan_token(self)
    }

    /// Skips whitespace and comments — batch-optimized for large files.
    fn skip_whitespace_and_comments(&mut self) {
        let ws_pos_start = self.position;
        loop {
            // Batch-skip all ASCII whitespace
            while self.position < self.bytes.len() {
                match self.bytes[self.position] {
                    b' ' | b'\t' | b'\r' | b'\n' => self.position += 1,
                    _ => break,
                }
            }

            match self.peek() {
                // Python-style comment: # ...
                Some(b'#') => {
                    if self.position == 0 {
                        break;
                    }
                    self.skip_line_comment();
                }
                // C++-style comment: // or /* */
                Some(b'/') => match self.peek_at(1) {
                    Some(b'/') => self.skip_line_comment(),
                    Some(b'*') => self.skip_block_comment(),
                    _ => break,
                },
                _ => break,
            }
        }
        WHITESPACE_BYTES.with(|c| c.set(c.get() + (self.position - ws_pos_start) as u64));
    }

    /// Skips a single-line comment (# or //) — uses memchr for fast newline search.
    fn skip_line_comment(&mut self) {
        if let Some(nl_offset) = memchr::memchr(b'\n', &self.bytes[self.position..]) {
            self.position += nl_offset;
        } else {
            self.position = self.bytes.len();
        }
    }

    /// Skips a block comment (/* */).
    fn skip_block_comment(&mut self) {
        self.position += 2; // skip /*
        let mut depth = 1;
        while depth > 0 && self.position < self.bytes.len() {
            if self.position + 1 < self.bytes.len() {
                let b0 = self.bytes[self.position];
                let b1 = self.bytes[self.position + 1];
                if b0 == b'*' && b1 == b'/' {
                    self.position += 2;
                    depth -= 1;
                    continue;
                }
                if b0 == b'/' && b1 == b'*' {
                    self.position += 2;
                    depth += 1;
                    continue;
                }
            }
            self.position += 1;
        }
    }

    /// Peeks at the remaining source from current position.
    #[must_use]
    pub fn remaining(&self) -> &'a str {
        &self.source[self.position..]
    }

    /// Returns the source text.
    #[must_use]
    pub fn source(&self) -> &'a str {
        self.source
    }
}

/// Iterator adapter for lexer.
impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_token()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_creation() {
        let lexer = Lexer::new("test");
        assert!(!lexer.is_at_end());
        assert_eq!(lexer.current_location().offset(), 0);
    }

    #[test]
    fn test_lexer_peek_advance() {
        let mut lexer = Lexer::new("abc");
        assert_eq!(lexer.peek(), Some(b'a'));
        assert_eq!(lexer.advance(), Some(b'a'));
        assert_eq!(lexer.peek(), Some(b'b'));
        assert_eq!(lexer.position, 1);
    }

    #[test]
    fn test_lexer_line_tracking() {
        // Line/column now computed lazily via resolve_location
        let lexer = Lexer::new("a\nb\nc");
        let loc = lexer.resolve_location(2); // after 'a\n'
        assert_eq!(loc.line(), 2);
        assert_eq!(loc.column(), 1);
        let loc2 = lexer.resolve_location(3); // after 'a\nb'
        assert_eq!(loc2.line(), 2);
        assert_eq!(loc2.column(), 2);
    }

    #[test]
    fn test_lexer_utf8() {
        let mut lexer = Lexer::new("abc");
        assert_eq!(lexer.peek_char(), Some('a'));
        assert_eq!(lexer.advance_char(), Some('a'));
    }

    #[test]
    fn test_skip_comments() {
        let mut lexer = Lexer::new("  // comment\nabc");
        lexer.skip_whitespace_and_comments();
        assert_eq!(lexer.peek(), Some(b'a'));
    }

    #[test]
    fn test_skip_block_comment() {
        let mut lexer = Lexer::new("/* comment */ abc");
        lexer.skip_whitespace_and_comments();
        assert_eq!(lexer.peek(), Some(b'a'));
    }
}
