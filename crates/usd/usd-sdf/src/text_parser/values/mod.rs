//! Value parsing for the USDA text format.
//!
//! This module parses values from the token stream into intermediate
//! representations. Values can be:
//!
//! - **Atomic**: Numbers, strings, identifiers, asset paths, SDF paths
//! - **Compound**: Tuples `(a, b, c)`, lists `[a, b, c]`, dictionaries `{k = v}`
//! - **Typed**: Time samples, splines, array edits
//!
//! # C++ Parity
//!
//! This implements the value parsing portion of `textFileFormatParser.h`:
//! - `AtomicValue` (lines 416-425)
//! - `TupleValue` (lines 444-467)
//! - `ListValue` (lines 469-493)
//! - `DictionaryValue` (lines 504-547)
//! - `TimeSampleMap` (lines 764-781)
//! - `SplineValue` (lines 783-981)
//! - `ArrayEditValue` (lines 549-623)
//!
//! # Architecture
//!
//! The value parser uses a recursive descent approach:
//! 1. `parse_value()` - Main entry, dispatches based on token
//! 2. `parse_typed_value()` - Parses with type context
//! 3. Specialized parsers for each value kind
//!
//! The `ValueContext` from `value_context.rs` tracks state during parsing.

pub mod atomic;
pub mod compound;
pub mod typed;

use crate::text_parser::error::{ParseError, ParseErrorKind, ParseResult};
use crate::text_parser::lexer::Lexer;
use crate::text_parser::tokens::{Keyword, Token, TokenKind};
use crate::text_parser::value_context::{Value, ValueContext};

// Re-exports from typed module
pub use typed::{TimeSample, TimeSampleMap};
// SplineValue and SplineKnot are now imported from usd_vt::spline in typed.rs

// ============================================================================
// Value Parser
// ============================================================================

/// A value parser that consumes tokens and produces parsed values.
///
/// This wraps a lexer and provides high-level value parsing methods.
#[derive(Debug)]
pub struct ValueParser<'a> {
    /// The underlying lexer.
    pub lexer: Lexer<'a>,
    /// Current token (lookahead).
    current: Option<Token>,

    /// Value building context.
    pub context: ValueContext,
}

impl<'a> ValueParser<'a> {
    /// Creates a new value parser.
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        Self {
            lexer,
            current,
            context: ValueContext::new(),
        }
    }

    /// Creates a parser from an existing lexer.
    #[must_use]
    pub fn from_lexer(mut lexer: Lexer<'a>) -> Self {
        let current = lexer.next_token();
        Self {
            lexer,
            current,
            context: ValueContext::new(),
        }
    }

    /// Creates a parser from full state (lexer + current token).
    /// Use with `into_state()` to properly transfer parser state.
    #[must_use]
    pub fn from_state(lexer: Lexer<'a>, current: Option<Token>) -> Self {
        Self {
            lexer,
            current,
            context: ValueContext::new(),
        }
    }

    /// Extracts full parser state (lexer + current token).
    /// Use with `from_state()` to properly transfer parser state.
    pub fn into_state(self) -> (Lexer<'a>, Option<Token>) {
        (self.lexer, self.current)
    }

    /// Saves current parser state for lookahead / backtracking.
    /// Lexer is Clone, so this is a full copy.
    pub fn save_state(&self) -> (Lexer<'a>, Option<Token>) {
        (self.lexer.clone(), self.current.clone())
    }

    /// Restores a previously saved state (backtrack after lookahead).
    pub fn restore_state(&mut self, state: (Lexer<'a>, Option<Token>)) {
        self.lexer = state.0;
        self.current = state.1;
    }

    /// Returns the current token without consuming it.
    #[must_use]
    pub fn peek(&self) -> Option<&Token> {
        self.current.as_ref()
    }

    /// Returns the current token kind.
    #[must_use]
    pub fn peek_kind(&self) -> Option<&TokenKind> {
        self.current.as_ref().map(|t| &t.kind)
    }

    /// Returns true if at end of input.
    #[must_use]
    pub fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), None | Some(TokenKind::Eof))
    }

    /// Advances to the next token, returning the consumed current token.
    pub fn advance(&mut self) -> Option<Token> {
        let consumed = self.current.take();
        self.current = self.lexer.next_token();
        consumed
    }

    /// Checks if current token matches the expected kind.
    #[must_use]
    pub fn check(&self, kind: &TokenKind) -> bool {
        match self.peek_kind() {
            Some(k) => std::mem::discriminant(k) == std::mem::discriminant(kind),
            None => false,
        }
    }

    /// Checks if current token is a specific keyword.
    #[must_use]
    pub fn check_keyword(&self, kw: Keyword) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Keyword(k)) if *k == kw)
    }

    /// Advances if current token matches, otherwise returns None.
    pub fn match_kind(&mut self, kind: &TokenKind) -> Option<Token> {
        if self.check(kind) {
            self.advance()
        } else {
            None
        }
    }

    /// Advances if current token is the expected keyword.
    pub fn match_keyword(&mut self, kw: Keyword) -> Option<Token> {
        if self.check_keyword(kw) {
            self.advance()
        } else {
            None
        }
    }

    /// Expects the current token to match, or returns an error.
    pub fn expect(&mut self, kind: &TokenKind) -> ParseResult<Token> {
        if self.check(kind) {
            Ok(self.advance().expect("checked above"))
        } else {
            let found = self
                .peek_kind()
                .map(|k| k.to_string())
                .unwrap_or_else(|| "end of file".to_string());
            Err(self.error(ParseErrorKind::ExpectedToken {
                expected: format!("{:?}", kind),
                found,
            }))
        }
    }

    /// Expects a specific keyword.
    pub fn expect_keyword(&mut self, kw: Keyword) -> ParseResult<Token> {
        if self.check_keyword(kw) {
            Ok(self.advance().expect("checked above"))
        } else {
            let found = self
                .peek_kind()
                .map(|k| k.to_string())
                .unwrap_or_else(|| "end of file".to_string());
            Err(self.error(ParseErrorKind::ExpectedToken {
                expected: kw.as_str().to_string(),
                found,
            }))
        }
    }

    /// Creates an error at the current position.
    pub fn error(&self, kind: ParseErrorKind) -> ParseError {
        let location = self
            .current
            .as_ref()
            .map(|t| t.span.start())
            .unwrap_or_else(|| self.lexer.current_location());
        ParseError::new(kind, location)
    }

    // ========================================================================
    // Value Parsing
    // ========================================================================

    /// Parses any value (the main entry point).
    ///
    /// This corresponds to `TypedValue` in the C++ grammar:
    /// ```text
    /// TypedValue = AtomicValue | TupleValue | EmptyListValue | ListValue | PathRefValue
    /// ```
    pub fn parse_value(&mut self) -> ParseResult<Value> {
        match self.peek_kind() {
            // Numbers
            Some(TokenKind::Integer(_))
            | Some(TokenKind::Float(_))
            | Some(TokenKind::Inf)
            | Some(TokenKind::NegInf)
            | Some(TokenKind::Nan) => self.parse_number(),

            // String
            Some(TokenKind::String(_)) => self.parse_string(),

            // Identifier (could be keyword like None, true, false)
            Some(TokenKind::Identifier(_)) => self.parse_identifier_value(),

            // Keyword (None, etc.)
            Some(TokenKind::Keyword(Keyword::None)) => {
                self.advance();
                Ok(Value::None)
            }

            // Asset reference
            Some(TokenKind::AssetRef(_)) => self.parse_asset_ref(),

            // Path reference
            Some(TokenKind::PathRef(_)) => self.parse_path_ref(),

            // Tuple
            Some(TokenKind::LeftParen) => self.parse_tuple(),

            // List
            Some(TokenKind::LeftBracket) => self.parse_list(),

            // Dictionary
            Some(TokenKind::LeftBrace) => self.parse_dictionary_value(),

            // Unexpected
            Some(kind) => Err(self.error(ParseErrorKind::UnexpectedToken(kind.to_string()))),

            None => Err(self.error(ParseErrorKind::UnexpectedEof)),
        }
    }

    /// Parses a typed value with known type context.
    ///
    /// Sets up the value context factory and parses the value.
    pub fn parse_typed_value(&mut self, type_name: &str) -> ParseResult<Value> {
        self.context.setup_factory(type_name);
        self.parse_value()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_parser_creation() {
        let parser = ValueParser::new("42");
        assert!(!parser.is_at_end());
    }

    #[test]
    fn test_parse_integer() {
        let mut parser = ValueParser::new("42");
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::Int64(42)));
    }

    #[test]
    fn test_parse_float() {
        let mut parser = ValueParser::new("3.14");
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::Double(f) if (f - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_parse_string() {
        let mut parser = ValueParser::new(r#""hello""#);
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::String(s) if s == "hello"));
    }

    #[test]
    fn test_parse_identifier() {
        let mut parser = ValueParser::new("myIdent");
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::Token(_)));
    }
}
