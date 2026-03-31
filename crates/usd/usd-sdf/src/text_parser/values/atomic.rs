//! Atomic value parsing for USDA format.
//!
//! Atomic values are the basic building blocks:
//! - Numbers: integers, floats, inf, nan
//! - Strings: quoted text
//! - Identifiers: names and tokens
//! - Asset paths: @path@ references
//! - SDF paths: <path> references
//!
//! # C++ Parity
//!
//! Matches the atomic value rules from `textFileFormatParser.h`:
//! ```text
//! AtomicValue = NumberValue | IdentifierValue | StringValue | AssetRefValue
//! NumberValue = Number
//! StringValue = String
//! IdentifierValue = Identifier
//! AssetRefValue = AssetRef
//! PathRefValue = PathRef
//! ```

use crate::text_parser::error::{ParseErrorKind, ParseResult};
use crate::text_parser::tokens::{Keyword, TokenKind};
use crate::text_parser::value_context::Value;
use usd_tf::Token;

use super::ValueParser;

impl<'a> ValueParser<'a> {
    /// Parses a number value (integer or float).
    ///
    /// Handles:
    /// - Integers: `42`, `-7`
    /// - Floats: `3.14`, `-0.5e10`
    /// - Special: `inf`, `-inf`, `nan`
    pub fn parse_number(&mut self) -> ParseResult<Value> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedNumber))?;

        match token.kind {
            TokenKind::Integer(n) => Ok(Value::Int64(n)),
            TokenKind::Float(n) => Ok(Value::Double(n)),
            TokenKind::Inf => Ok(Value::Double(f64::INFINITY)),
            TokenKind::NegInf => Ok(Value::Double(f64::NEG_INFINITY)),
            TokenKind::Nan => Ok(Value::Double(f64::NAN)),
            _ => Err(self.error(ParseErrorKind::ExpectedNumber)),
        }
    }

    /// Parses a string value.
    ///
    /// The lexer already handles escape sequences, so we just extract
    /// the content from the token.
    pub fn parse_string(&mut self) -> ParseResult<Value> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedString))?;

        match token.kind {
            TokenKind::String(s) => Ok(Value::String(s)),
            _ => Err(self.error(ParseErrorKind::ExpectedString)),
        }
    }

    /// Parses an identifier as a value.
    ///
    /// Identifiers become `TfToken` values. Special identifiers
    /// like `true`, `false` are handled here.
    pub fn parse_identifier_value(&mut self) -> ParseResult<Value> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedIdentifier))?;

        match token.kind {
            TokenKind::Identifier(s) => {
                // Check for boolean keywords
                match s.as_str() {
                    // C++ stores bool tokens as int64 (1/0), matching _GetImpl<bool> via TfToken.
                    "true" | "True" | "TRUE" => Ok(Value::Int64(1)),
                    "false" | "False" | "FALSE" => Ok(Value::Int64(0)),
                    _ => Ok(Value::Token(Token::new(&s))),
                }
            }
            TokenKind::Keyword(kw) => {
                // Some keywords can appear as values
                match kw {
                    Keyword::None | Keyword::NoneLowercase => Ok(Value::None),
                    _ => Ok(Value::Token(Token::new(kw.as_str()))),
                }
            }
            _ => Err(self.error(ParseErrorKind::ExpectedIdentifier)),
        }
    }

    /// Parses an asset path reference.
    ///
    /// Asset paths are `@path@` or `@@@path@@@` in the source.
    /// The lexer extracts the path content.
    pub fn parse_asset_ref(&mut self) -> ParseResult<Value> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::UnexpectedEof))?;

        match token.kind {
            TokenKind::AssetRef(path) => Ok(Value::AssetPath(path)),
            _ => Err(self.error(ParseErrorKind::UnexpectedToken(
                "expected asset reference".to_string(),
            ))),
        }
    }

    /// Parses an SDF path reference.
    ///
    /// Path references are `<path>` in the source.
    pub fn parse_path_ref(&mut self) -> ParseResult<Value> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedPathRef))?;

        match token.kind {
            TokenKind::PathRef(path) => Ok(Value::Path(path)),
            _ => Err(self.error(ParseErrorKind::ExpectedPathRef)),
        }
    }

    /// Parses a plain identifier (for names, not values).
    ///
    /// Returns the identifier string.
    pub fn parse_identifier(&mut self) -> ParseResult<String> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedIdentifier))?;

        match token.kind {
            TokenKind::Identifier(s) => Ok(s),
            _ => Err(self.error(ParseErrorKind::ExpectedIdentifier)),
        }
    }

    /// Parses a namespaced identifier (e.g., `xformOp:translate`, `xformOp:scale`).
    ///
    /// The lexer produces separate tokens for identifier, colon, identifier/keyword.
    /// Components after the colon may be keywords (e.g. "scale", "offset") which
    /// are valid as identifier parts in this context per USD spec.
    pub fn parse_namespaced_identifier(&mut self) -> ParseResult<String> {
        let mut result = self.parse_identifier_or_keyword()?;

        // Check for namespace separator
        while self.check(&TokenKind::Colon) {
            self.advance(); // consume :
            let next = self.parse_identifier_or_keyword()?;
            result.push(':');
            result.push_str(&next);
        }

        Ok(result)
    }

    /// Parses an identifier or keyword, returning the string.
    ///
    /// In namespaced identifiers and similar contexts, keywords like "scale",
    /// "offset", "translate" are valid as identifier components.
    fn parse_identifier_or_keyword(&mut self) -> ParseResult<String> {
        let token = self
            .advance()
            .ok_or_else(|| self.error(ParseErrorKind::ExpectedIdentifier))?;

        match &token.kind {
            TokenKind::Identifier(s) => Ok(s.clone()),
            TokenKind::Keyword(kw) => Ok(kw.as_str().to_string()),
            _ => Err(self.error(ParseErrorKind::ExpectedIdentifier)),
        }
    }

    /// Parses a C++ namespaced identifier (e.g., `Foo::Bar`).
    pub fn parse_cxx_namespaced_identifier(&mut self) -> ParseResult<String> {
        let mut result = self.parse_identifier()?;

        // Check for C++ namespace separator
        while self.check(&TokenKind::DoubleColon) {
            self.advance(); // consume ::
            let next = self.parse_identifier()?;
            result.push_str("::");
            result.push_str(&next);
        }

        Ok(result)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_integer() {
        let mut parser = ValueParser::new("42");
        let value = parser.parse_number().unwrap();
        assert!(matches!(value, Value::Int64(42)));
    }

    #[test]
    fn test_parse_negative_integer() {
        let mut parser = ValueParser::new("-123");
        let value = parser.parse_number().unwrap();
        assert!(matches!(value, Value::Int64(-123)));
    }

    #[test]
    fn test_parse_float() {
        let mut parser = ValueParser::new("3.14159");
        let value = parser.parse_number().unwrap();
        match value {
            Value::Double(f) => assert!((f - 3.14159).abs() < 0.00001),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn test_parse_float_exponent() {
        let mut parser = ValueParser::new("1.5e10");
        let value = parser.parse_number().unwrap();
        match value {
            Value::Double(f) => assert!((f - 1.5e10).abs() < 1e5),
            _ => panic!("expected float"),
        }
    }

    #[test]
    fn test_parse_inf() {
        let mut parser = ValueParser::new("inf");
        let value = parser.parse_number().unwrap();
        match value {
            Value::Double(f) => assert!(f.is_infinite() && f > 0.0),
            _ => panic!("expected inf"),
        }
    }

    #[test]
    fn test_parse_neg_inf() {
        let mut parser = ValueParser::new("-inf");
        let value = parser.parse_number().unwrap();
        match value {
            Value::Double(f) => assert!(f.is_infinite() && f < 0.0),
            _ => panic!("expected -inf"),
        }
    }

    #[test]
    fn test_parse_nan() {
        let mut parser = ValueParser::new("nan");
        let value = parser.parse_number().unwrap();
        match value {
            Value::Double(f) => assert!(f.is_nan()),
            _ => panic!("expected nan"),
        }
    }

    #[test]
    fn test_parse_string_double_quote() {
        let mut parser = ValueParser::new(r#""hello world""#);
        let value = parser.parse_string().unwrap();
        assert!(matches!(value, Value::String(s) if s == "hello world"));
    }

    #[test]
    fn test_parse_string_single_quote() {
        let mut parser = ValueParser::new("'hello'");
        let value = parser.parse_string().unwrap();
        assert!(matches!(value, Value::String(s) if s == "hello"));
    }

    #[test]
    fn test_parse_identifier() {
        let mut parser = ValueParser::new("myIdentifier");
        let value = parser.parse_identifier_value().unwrap();
        match value {
            Value::Token(t) => assert_eq!(t.as_str(), "myIdentifier"),
            _ => panic!("expected token"),
        }
    }

    #[test]
    fn test_parse_bool_true() {
        let mut parser = ValueParser::new("true");
        let value = parser.parse_identifier_value().unwrap();
        assert!(matches!(value, Value::Int64(1)));
    }

    #[test]
    fn test_parse_bool_false() {
        let mut parser = ValueParser::new("false");
        let value = parser.parse_identifier_value().unwrap();
        assert!(matches!(value, Value::Int64(0)));
    }

    #[test]
    fn test_parse_asset_ref() {
        let mut parser = ValueParser::new("@./path/to/asset.usd@");
        let value = parser.parse_asset_ref().unwrap();
        assert!(matches!(value, Value::AssetPath(p) if p == "./path/to/asset.usd"));
    }

    #[test]
    fn test_parse_path_ref() {
        let mut parser = ValueParser::new("</World/Cube>");
        let value = parser.parse_path_ref().unwrap();
        assert!(matches!(value, Value::Path(p) if p == "/World/Cube"));
    }

    #[test]
    fn test_parse_namespaced_identifier() {
        let mut parser = ValueParser::new("xformOp:translate");
        let result = parser.parse_namespaced_identifier().unwrap();
        assert_eq!(result, "xformOp:translate");
    }

    /// Regression: "scale" is a USD keyword but valid in namespaced names (e.g. xformOp:scale).
    /// Reference: textFileFormatParser.h uses BaseIdentifier (not KeywordlessIdentifier)
    /// for NamespacedIdentifier.
    #[test]
    fn test_parse_namespaced_identifier_with_keyword_component() {
        let mut parser = ValueParser::new("xformOp:scale");
        let result = parser.parse_namespaced_identifier().unwrap();
        assert_eq!(result, "xformOp:scale");
    }

    #[test]
    fn test_parse_cxx_namespaced_identifier() {
        let mut parser = ValueParser::new("Foo::Bar::Baz");
        let result = parser.parse_cxx_namespaced_identifier().unwrap();
        assert_eq!(result, "Foo::Bar::Baz");
    }
}
