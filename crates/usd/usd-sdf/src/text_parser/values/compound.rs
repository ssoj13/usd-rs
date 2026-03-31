//! Compound value parsing for USDA format.
//!
//! Compound values are structured containers:
//! - **Tuples**: `(a, b, c)` - fixed-size, ordered collections
//! - **Lists**: `[a, b, c]` - variable-size arrays
//! - **Dictionaries**: `{ type key = value; }` - key-value mappings
//!
//! # C++ Parity
//!
//! Matches the compound value rules from `textFileFormatParser.h`:
//! ```text
//! TupleValue = ( TupleValueItem, ... )        // lines 444-467
//! ListValue = [ ListValueItem, ... ]          // lines 469-493
//! EmptyListValue = [ ]                        // lines 495-502
//! DictionaryValue = { DictionaryItem; ... }   // lines 504-547
//! ```
//!
//! # Grammar
//!
//! ```text
//! TupleValueItem = NumberValue | IdentifierValue | StringValue |
//!                  AssetRefValue | TupleValue
//!
//! ListValueItem = NumberValue | IdentifierValue | StringValue |
//!                 AssetRefValue | ListValue | TupleValue
//!
//! DictionaryKey = String | Identifier
//! DictionaryType = Identifier [ '[' ']' ]?
//! DictionaryValueItem = 'dictionary' Key '=' DictionaryValue |
//!                       Type Key '=' TypedValue
//! ```

use crate::text_parser::error::{ParseErrorKind, ParseResult};
use crate::text_parser::tokens::{Keyword, TokenKind};
use crate::text_parser::value_context::Value;

use super::ValueParser;

impl<'a> ValueParser<'a> {
    /// Parses a tuple value: `(a, b, c)`.
    ///
    /// Tuples are fixed-size ordered collections. In USD, tuples are used
    /// for vectors, matrices, and other fixed-size compound types.
    ///
    /// # Grammar
    ///
    /// ```text
    /// TupleValue = '(' TupleValueItem (',' TupleValueItem)* ')'
    /// TupleValueItem = NumberValue | IdentifierValue | StringValue |
    ///                  AssetRefValue | TupleValue
    /// ```
    ///
    /// # Examples
    ///
    /// - `(1, 2, 3)` - Vec3i
    /// - `(1.0, 2.0, 3.0)` - Vec3f
    /// - `((1, 0, 0), (0, 1, 0), (0, 0, 1))` - Matrix3d rows
    pub fn parse_tuple(&mut self) -> ParseResult<Value> {
        // Expect opening paren
        self.expect(&TokenKind::LeftParen)?;

        // Notify context we're starting a tuple
        self.context.begin_tuple();

        let mut elements = Vec::new();

        // Check for empty tuple
        if self.check(&TokenKind::RightParen) {
            self.advance();
            self.context.end_tuple();
            return Ok(Value::Tuple(elements));
        }

        // Parse first element
        let elem = self.parse_tuple_item()?;
        elements.push(elem);

        // Parse remaining elements
        while self.check(&TokenKind::Comma) {
            self.advance(); // consume comma

            // Allow trailing comma
            if self.check(&TokenKind::RightParen) {
                break;
            }

            let elem = self.parse_tuple_item()?;
            elements.push(elem);
        }

        // Expect closing paren
        self.expect(&TokenKind::RightParen)?;
        self.context.end_tuple();

        Ok(Value::Tuple(elements))
    }

    /// Parses a single tuple item.
    ///
    /// Tuple items can be atomic values or nested tuples.
    fn parse_tuple_item(&mut self) -> ParseResult<Value> {
        match self.peek_kind() {
            // Numbers
            Some(TokenKind::Integer(_))
            | Some(TokenKind::Float(_))
            | Some(TokenKind::Inf)
            | Some(TokenKind::NegInf)
            | Some(TokenKind::Nan) => self.parse_number(),

            // Identifier
            Some(TokenKind::Identifier(_)) => self.parse_identifier_value(),

            // Keyword as identifier (None, etc.)
            Some(TokenKind::Keyword(_)) => self.parse_identifier_value(),

            // String
            Some(TokenKind::String(_)) => self.parse_string(),

            // Asset reference
            Some(TokenKind::AssetRef(_)) => self.parse_asset_ref(),

            // Nested tuple
            Some(TokenKind::LeftParen) => self.parse_tuple(),

            // Unexpected token
            Some(kind) => Err(self.error(ParseErrorKind::UnexpectedToken(format!(
                "expected tuple item, found {}",
                kind
            )))),

            None => Err(self.error(ParseErrorKind::UnexpectedEof)),
        }
    }

    /// Parses a list value: `[a, b, c]`.
    ///
    /// Lists are variable-size arrays. They can contain any value type,
    /// including nested lists and tuples.
    ///
    /// # Grammar
    ///
    /// ```text
    /// ListValue = '[' ListValueItem (',' ListValueItem)* ']'
    /// ListValueItem = NumberValue | IdentifierValue | StringValue |
    ///                 AssetRefValue | ListValue | TupleValue
    /// EmptyListValue = '[' ']'
    /// ```
    ///
    /// # Examples
    ///
    /// - `[]` - empty list
    /// - `[1, 2, 3]` - integer list
    /// - `["a", "b"]` - string list
    /// - `[(1, 2), (3, 4)]` - list of tuples
    pub fn parse_list(&mut self) -> ParseResult<Value> {
        // Expect opening bracket
        self.expect(&TokenKind::LeftBracket)?;

        // Notify context we're starting a list
        self.context.begin_list();

        // Check for empty list
        if self.check(&TokenKind::RightBracket) {
            self.advance();
            self.context.end_list();
            return Ok(Value::List(Vec::new()));
        }

        // Pre-allocate based on heuristic: estimate element count from remaining source.
        // For numeric arrays each element is ~10-20 bytes of text.
        let capacity = self.estimate_list_capacity();
        let mut elements = Vec::with_capacity(capacity);

        // Parse first element
        let elem = self.parse_list_item()?;
        elements.push(elem);

        // Parse remaining elements
        while self.check(&TokenKind::Comma) {
            self.advance(); // consume comma

            // Allow trailing comma
            if self.check(&TokenKind::RightBracket) {
                break;
            }

            let elem = self.parse_list_item()?;
            elements.push(elem);
        }

        // Expect closing bracket
        self.expect(&TokenKind::RightBracket)?;
        self.context.end_list();

        Ok(Value::List(elements))
    }

    /// Estimates list element count from remaining source text.
    /// Scans forward for ']' and divides by average element width.
    fn estimate_list_capacity(&self) -> usize {
        let remaining = self.lexer.remaining().as_bytes();
        // Find closing bracket (approximate — ignores nesting)
        let bracket_dist = memchr::memchr(b']', remaining).unwrap_or(remaining.len());
        // Heuristic: numeric tuples like (1.0, 2.0, 3.0) average ~25 bytes each
        let estimate = bracket_dist / 20;
        estimate.clamp(4, 100_000)
    }

    /// Parses a single list item.
    ///
    /// List items can be atomic values, tuples, or nested lists.
    fn parse_list_item(&mut self) -> ParseResult<Value> {
        match self.peek_kind() {
            // Numbers
            Some(TokenKind::Integer(_))
            | Some(TokenKind::Float(_))
            | Some(TokenKind::Inf)
            | Some(TokenKind::NegInf)
            | Some(TokenKind::Nan) => self.parse_number(),

            // Identifier
            Some(TokenKind::Identifier(_)) => self.parse_identifier_value(),

            // Keyword as identifier
            Some(TokenKind::Keyword(_)) => self.parse_identifier_value(),

            // String
            Some(TokenKind::String(_)) => self.parse_string(),

            // Asset reference
            Some(TokenKind::AssetRef(_)) => self.parse_asset_ref(),

            // Path reference
            Some(TokenKind::PathRef(_)) => self.parse_path_ref(),

            // Nested list
            Some(TokenKind::LeftBracket) => self.parse_list(),

            // Tuple
            Some(TokenKind::LeftParen) => self.parse_tuple(),

            // Unexpected token
            Some(kind) => Err(self.error(ParseErrorKind::UnexpectedToken(format!(
                "expected list item, found {}",
                kind
            )))),

            None => Err(self.error(ParseErrorKind::UnexpectedEof)),
        }
    }

    /// Parses a dictionary value: `{ type key = value; ... }`.
    ///
    /// Dictionaries are typed key-value maps. Each entry has a type
    /// specifier and the key is a string or identifier.
    ///
    /// # Grammar
    ///
    /// ```text
    /// DictionaryValue = '{' DictionaryItem* '}'
    /// DictionaryItem = DictionaryType DictionaryKey '=' TypedValue ';'
    ///                | 'dictionary' DictionaryKey '=' DictionaryValue ';'
    /// DictionaryKey = String | Identifier
    /// DictionaryType = Identifier ('[' ']')?
    /// ```
    ///
    /// # Examples
    ///
    /// ```text
    /// {
    ///     string name = "Cube"
    ///     double3 size = (1, 1, 1)
    ///     dictionary nested = {
    ///         int value = 42
    ///     }
    /// }
    /// ```
    pub fn parse_dictionary_value(&mut self) -> ParseResult<Value> {
        // Expect opening brace
        self.expect(&TokenKind::LeftBrace)?;

        let mut entries = Vec::new();

        // Parse entries until closing brace
        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            let entry = self.parse_dictionary_item()?;
            entries.push(entry);

            // Entries can be separated by semicolons or newlines
            // The lexer handles whitespace, so we just look for optional semicolon
            self.match_kind(&TokenKind::Semicolon);
        }

        // Expect closing brace
        self.expect(&TokenKind::RightBrace)?;

        Ok(Value::Dictionary(entries))
    }

    /// Parses a single dictionary item.
    ///
    /// Dictionary items have the form: `type key = value`
    /// or `dictionary key = { ... }` for nested dictionaries.
    fn parse_dictionary_item(&mut self) -> ParseResult<(String, String, Value)> {
        // Check for nested dictionary: `dictionary key = { ... }`
        if self.check_keyword(Keyword::Dictionary) {
            self.advance(); // consume 'dictionary'

            let key = self.parse_dictionary_key()?;
            self.expect(&TokenKind::Equals)?;

            let value = self.parse_dictionary_value()?;

            return Ok(("dictionary".to_string(), key, value));
        }

        // Regular typed item: `type key = value`
        let type_name = self.parse_dictionary_type()?;
        let key = self.parse_dictionary_key()?;
        self.expect(&TokenKind::Equals)?;

        // Parse value with type context
        let value = self.parse_typed_value(&type_name)?;

        Ok((type_name, key, value))
    }

    /// Parses a dictionary type specifier.
    ///
    /// Types can be simple identifiers or array types:
    /// - `int`
    /// - `double3`
    /// - `string[]`
    fn parse_dictionary_type(&mut self) -> ParseResult<String> {
        let mut type_name = self.parse_identifier()?;

        // Check for array type: `[]`
        if self.check(&TokenKind::LeftBracket) {
            self.advance();
            self.expect(&TokenKind::RightBracket)?;
            type_name.push_str("[]");
        }

        Ok(type_name)
    }

    /// Parses a dictionary key.
    ///
    /// Keys can be quoted strings, identifiers, or keywords (e.g. `doc`,
    /// `kind`, `type` are valid keys inside customData dictionaries).
    fn parse_dictionary_key(&mut self) -> ParseResult<String> {
        match self.peek_kind() {
            Some(TokenKind::String(_)) => {
                let token = self.advance().expect("token after peek");
                match token.kind {
                    TokenKind::String(s) => Ok(s),
                    _ => unreachable!(),
                }
            }
            Some(TokenKind::Identifier(_)) => self.parse_identifier(),
            // Keywords like `doc`, `kind`, `type` are valid dictionary keys
            Some(TokenKind::Keyword(_)) => {
                let token = self.advance().expect("token after peek");
                match token.kind {
                    TokenKind::Keyword(kw) => Ok(kw.as_str().to_string()),
                    _ => unreachable!(),
                }
            }
            Some(kind) => Err(self.error(ParseErrorKind::UnexpectedToken(format!(
                "expected dictionary key, found {}",
                kind
            )))),
            None => Err(self.error(ParseErrorKind::UnexpectedEof)),
        }
    }

    /// Parses a string-to-string map: `{ "key": "value", ... }`.
    ///
    /// Used for `prefixSubstitutions` and `suffixSubstitutions` metadata.
    /// Grammar (C++ `StringDictionary`):
    /// ```text
    /// StringDictionary = '{' (String ':' String (',' String ':' String)* ','?)? '}'
    /// ```
    pub fn parse_string_map_value(&mut self) -> ParseResult<Value> {
        self.expect(&TokenKind::LeftBrace)?;

        let mut pairs = Vec::new();

        while !self.check(&TokenKind::RightBrace) && !self.is_at_end() {
            // Key must be a quoted string
            let key = match self.peek_kind() {
                Some(TokenKind::String(_)) => {
                    let tok = self.advance().expect("token after peek");
                    match tok.kind {
                        TokenKind::String(s) => s,
                        _ => unreachable!(),
                    }
                }
                Some(kind) => {
                    return Err(self.error(ParseErrorKind::UnexpectedToken(format!(
                        "expected string key in string map, found {}",
                        kind
                    ))));
                }
                None => return Err(self.error(ParseErrorKind::UnexpectedEof)),
            };

            // Colon separator
            self.expect(&TokenKind::Colon)?;

            // Value must be a quoted string
            let val = match self.peek_kind() {
                Some(TokenKind::String(_)) => {
                    let tok = self.advance().expect("token after peek");
                    match tok.kind {
                        TokenKind::String(s) => s,
                        _ => unreachable!(),
                    }
                }
                Some(kind) => {
                    return Err(self.error(ParseErrorKind::UnexpectedToken(format!(
                        "expected string value in string map, found {}",
                        kind
                    ))));
                }
                None => return Err(self.error(ParseErrorKind::UnexpectedEof)),
            };

            pairs.push((key, val));

            // Optional trailing comma
            self.match_kind(&TokenKind::Comma);
        }

        self.expect(&TokenKind::RightBrace)?;

        // Store as typed Dictionary entries so downstream
        // convert_parser_value_to_abstract_value produces a real VtDictionary,
        // not the empty () that RelocatesMap yields.
        let entries: Vec<(String, String, Value)> = pairs
            .into_iter()
            .map(|(k, v)| ("string".to_string(), k, Value::String(v)))
            .collect();
        Ok(Value::Dictionary(entries))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Tuple Tests
    // ========================================================================

    #[test]
    fn test_parse_empty_tuple() {
        let mut parser = ValueParser::new("()");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(elements) => assert!(elements.is_empty()),
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn test_parse_single_element_tuple() {
        let mut parser = ValueParser::new("(42)");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(elements) => {
                assert_eq!(elements.len(), 1);
                assert!(matches!(elements[0], Value::Int64(42)));
            }
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn test_parse_int_tuple() {
        let mut parser = ValueParser::new("(1, 2, 3)");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(elements) => {
                assert_eq!(elements.len(), 3);
                assert!(matches!(elements[0], Value::Int64(1)));
                assert!(matches!(elements[1], Value::Int64(2)));
                assert!(matches!(elements[2], Value::Int64(3)));
            }
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn test_parse_float_tuple() {
        let mut parser = ValueParser::new("(1.0, 2.5, 3.14)");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(elements) => {
                assert_eq!(elements.len(), 3);
                assert!(matches!(elements[0], Value::Double(_)));
                assert!(matches!(elements[1], Value::Double(_)));
                assert!(matches!(elements[2], Value::Double(_)));
            }
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn test_parse_mixed_tuple() {
        let mut parser = ValueParser::new("(42, 3.14, \"hello\")");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(elements) => {
                assert_eq!(elements.len(), 3);
                assert!(matches!(elements[0], Value::Int64(42)));
                assert!(matches!(elements[1], Value::Double(_)));
                assert!(matches!(elements[2], Value::String(_)));
            }
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn test_parse_nested_tuple() {
        let mut parser = ValueParser::new("((1, 2), (3, 4))");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(outer) => {
                assert_eq!(outer.len(), 2);
                match &outer[0] {
                    Value::Tuple(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert!(matches!(inner[0], Value::Int64(1)));
                        assert!(matches!(inner[1], Value::Int64(2)));
                    }
                    _ => panic!("expected inner tuple"),
                }
            }
            _ => panic!("expected tuple"),
        }
    }

    #[test]
    fn test_parse_tuple_with_trailing_comma() {
        let mut parser = ValueParser::new("(1, 2, 3,)");
        let value = parser.parse_tuple().unwrap();
        match value {
            Value::Tuple(elements) => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("expected tuple"),
        }
    }

    // ========================================================================
    // List Tests
    // ========================================================================

    #[test]
    fn test_parse_empty_list() {
        let mut parser = ValueParser::new("[]");
        let value = parser.parse_list().unwrap();
        match value {
            Value::List(elements) => assert!(elements.is_empty()),
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_parse_int_list() {
        let mut parser = ValueParser::new("[1, 2, 3, 4, 5]");
        let value = parser.parse_list().unwrap();
        match value {
            Value::List(elements) => {
                assert_eq!(elements.len(), 5);
                assert!(matches!(elements[0], Value::Int64(1)));
                assert!(matches!(elements[4], Value::Int64(5)));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_parse_string_list() {
        let mut parser = ValueParser::new(r#"["a", "b", "c"]"#);
        let value = parser.parse_list().unwrap();
        match value {
            Value::List(elements) => {
                assert_eq!(elements.len(), 3);
                assert!(matches!(&elements[0], Value::String(s) if s == "a"));
                assert!(matches!(&elements[1], Value::String(s) if s == "b"));
                assert!(matches!(&elements[2], Value::String(s) if s == "c"));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_parse_nested_list() {
        let mut parser = ValueParser::new("[[1, 2], [3, 4]]");
        let value = parser.parse_list().unwrap();
        match value {
            Value::List(outer) => {
                assert_eq!(outer.len(), 2);
                match &outer[0] {
                    Value::List(inner) => {
                        assert_eq!(inner.len(), 2);
                        assert!(matches!(inner[0], Value::Int64(1)));
                    }
                    _ => panic!("expected inner list"),
                }
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_parse_list_of_tuples() {
        let mut parser = ValueParser::new("[(1, 2, 3), (4, 5, 6)]");
        let value = parser.parse_list().unwrap();
        match value {
            Value::List(elements) => {
                assert_eq!(elements.len(), 2);
                assert!(matches!(elements[0], Value::Tuple(_)));
                assert!(matches!(elements[1], Value::Tuple(_)));
            }
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn test_parse_list_with_trailing_comma() {
        let mut parser = ValueParser::new("[1, 2, 3,]");
        let value = parser.parse_list().unwrap();
        match value {
            Value::List(elements) => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("expected list"),
        }
    }

    // ========================================================================
    // Dictionary Tests
    // ========================================================================

    #[test]
    fn test_parse_empty_dictionary() {
        let mut parser = ValueParser::new("{}");
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => assert!(entries.is_empty()),
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_simple_dictionary() {
        let mut parser = ValueParser::new("{ int value = 42 }");
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                let (type_name, key, val) = &entries[0];
                assert_eq!(type_name, "int");
                assert_eq!(key, "value");
                assert!(matches!(val, Value::Int64(42)));
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_dictionary_multiple_entries() {
        let mut parser = ValueParser::new(
            r#"{
            string name = "Test"
            int count = 5
            double value = 3.14
        }"#,
        );
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 3);

                assert_eq!(entries[0].0, "string");
                assert_eq!(entries[0].1, "name");

                assert_eq!(entries[1].0, "int");
                assert_eq!(entries[1].1, "count");

                assert_eq!(entries[2].0, "double");
                assert_eq!(entries[2].1, "value");
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_dictionary_with_array_type() {
        let mut parser = ValueParser::new("{ int[] values = [1, 2, 3] }");
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                let (type_name, key, _) = &entries[0];
                assert_eq!(type_name, "int[]");
                assert_eq!(key, "values");
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_dictionary_quoted_key() {
        let mut parser = ValueParser::new(r#"{ string "my key" = "value" }"#);
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].1, "my key");
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_nested_dictionary() {
        let mut parser = ValueParser::new(
            r#"{
            dictionary inner = {
                int value = 42
            }
        }"#,
        );
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                let (type_name, key, val) = &entries[0];
                assert_eq!(type_name, "dictionary");
                assert_eq!(key, "inner");
                assert!(matches!(val, Value::Dictionary(_)));
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_dictionary_with_tuple() {
        let mut parser = ValueParser::new("{ double3 position = (1.0, 2.0, 3.0) }");
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                let (type_name, key, val) = &entries[0];
                assert_eq!(type_name, "double3");
                assert_eq!(key, "position");
                assert!(matches!(val, Value::Tuple(_)));
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_parse_dictionary_with_semicolons() {
        let mut parser = ValueParser::new("{ int a = 1; int b = 2; }");
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 2);
            }
            _ => panic!("expected dictionary"),
        }
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[test]
    fn test_parse_complex_nested_structure() {
        let mut parser = ValueParser::new(
            r#"{
            string[] names = ["a", "b", "c"]
            double3[] points = [(0, 0, 0), (1, 1, 1)]
            dictionary metadata = {
                int version = 1
            }
        }"#,
        );
        let value = parser.parse_dictionary_value().unwrap();
        assert!(matches!(value, Value::Dictionary(_)));
    }

    #[test]
    fn test_parse_value_tuple() {
        let mut parser = ValueParser::new("(1, 2, 3)");
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::Tuple(_)));
    }

    #[test]
    fn test_parse_value_list() {
        let mut parser = ValueParser::new("[1, 2, 3]");
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::List(_)));
    }

    #[test]
    fn test_parse_value_dictionary() {
        let mut parser = ValueParser::new("{ int x = 1 }");
        let value = parser.parse_value().unwrap();
        assert!(matches!(value, Value::Dictionary(_)));
    }

    #[test]
    fn test_dictionary_keyword_as_key() {
        // `doc` is a keyword but valid as a dictionary key (issue 112)
        let mut parser = ValueParser::new(r#"{ string doc = "test" }"#);
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].0, "string");
                assert_eq!(entries[0].1, "doc");
                assert!(matches!(&entries[0].2, Value::String(s) if s == "test"));
            }
            _ => panic!("expected dictionary"),
        }
    }

    #[test]
    fn test_dictionary_multiple_keyword_keys() {
        // Various keywords that can appear as dictionary keys
        let mut parser = ValueParser::new(
            r#"{
            string doc = "desc"
            string kind = "component"
        }"#,
        );
        let value = parser.parse_dictionary_value().unwrap();
        match value {
            Value::Dictionary(entries) => {
                assert_eq!(entries.len(), 2);
                assert_eq!(entries[0].1, "doc");
                assert_eq!(entries[1].1, "kind");
            }
            _ => panic!("expected dictionary"),
        }
    }
}
