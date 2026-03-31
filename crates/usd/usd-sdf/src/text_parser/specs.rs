//! Spec parsing for USDA format.
//!
//! This module parses spec definitions for prims, attributes, and relationships:
//! - **PrimSpec**: `def Xform "Name" { ... }`
//! - **AttributeSpec**: `float attr = 1.0`
//! - **RelationshipSpec**: `rel target = </path>`
//!
//! # C++ Parity
//!
//! Matches the spec rules from `textFileFormatParser.h`:
//! ```text
//! PrimSpec = def/over/class TypeName "Name" (metadata) { contents }  // lines 1562-1586
//! AttributeSpec = [custom] Type Name [= value] (metadata)             // lines 1058-1082
//! RelationshipSpec = rel Name [= targets] (metadata)                  // lines 1160-1182
//! PropertySpec = AttributeSpec | RelationshipSpec                     // lines 1475-1492
//! ```
//!
//! # Spec Types
//!
//! ## Prim Specs
//! ```text
//! def Xform "World" { ... }
//! over "Existing" { ... }
//! class "_MyClass" { ... }
//! ```
//!
//! ## Attribute Specs
//! ```text
//! float attr = 1.0
//! custom double3 myAttr = (1, 2, 3)
//! uniform token visibility = "invisible"
//! float attr.timeSamples = { 0: 1.0, 1: 2.0 }
//! float attr.connect = </Path.attr>
//! ```
//!
//! ## Relationship Specs
//! ```text
//! rel target = </Path/To/Target>
//! rel targets = [</A>, </B>, </C>]
//! custom rel myRel
//! ```

use crate::text_parser::error::{ParseErrorKind, ParseResult};
use crate::text_parser::lexer::Lexer;
use crate::text_parser::metadata::{Metadata, MetadataParser};
use crate::text_parser::tokens::{Keyword, TokenKind};
use crate::text_parser::value_context::{ArrayEditOp, Value};
use crate::text_parser::values::{TimeSampleMap, ValueParser};
use usd_vt::spline::SplineValue;

// Re-export canonical Specifier from types.rs (P2-1: deduplicated)
pub use crate::types::Specifier;

/// Attribute variability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variability {
    /// Varying (default) - can change over time.
    #[default]
    Varying,
    /// Uniform - constant over time.
    Uniform,
    /// Config - configuration value.
    Config,
}

/// A parsed prim spec.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPrimSpec {
    /// Specifier (def/over/class).
    pub specifier: Specifier,
    /// Type name (e.g., "Xform", "Mesh").
    pub type_name: Option<String>,
    /// Prim name.
    pub name: String,
    /// Metadata.
    pub metadata: Option<Metadata>,
    /// Whether this is a custom prim type.
    pub is_custom: bool,
}

/// A parsed attribute spec.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAttributeSpec {
    /// Whether this is a custom attribute.
    pub is_custom: bool,
    /// Variability (uniform/config/varying).
    pub variability: Variability,
    /// Value type name (e.g., "float", "double3").
    pub type_name: String,
    /// Whether it's an array type.
    pub is_array: bool,
    /// Attribute name.
    pub name: String,
    /// Default value.
    pub default_value: Option<Value>,
    /// Time samples.
    pub time_samples: Option<TimeSampleMap>,
    /// Connection targets.
    pub connections: Option<Vec<String>>,
    /// Spline value.
    pub spline: Option<SplineValue>,
    /// Metadata.
    pub metadata: Option<Metadata>,
}

/// A parsed relationship spec.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRelationshipSpec {
    /// Whether this is a custom relationship.
    pub is_custom: bool,
    /// Whether it's varying.
    pub is_varying: bool,
    /// Relationship name.
    pub name: String,
    /// Target paths.
    pub targets: Option<Vec<String>>,
    /// Default target.
    pub default_target: Option<String>,
    /// Time samples for targets.
    pub time_samples: Option<TimeSampleMap>,
    /// Metadata.
    pub metadata: Option<Metadata>,
}

/// A parsed property spec (attribute or relationship).
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedPropertySpec {
    /// An attribute.
    Attribute(ParsedAttributeSpec),
    /// A relationship.
    Relationship(ParsedRelationshipSpec),
}

/// List operation on a property.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyListOp {
    /// The operation.
    pub op: ArrayEditOp,
    /// The property spec.
    pub property: ParsedPropertySpec,
}

/// Child or property ordering statement.
#[derive(Debug, Clone, PartialEq)]
pub enum OrderingStatement {
    /// Reorder children: `reorder nameChildren = [...]`.
    ChildOrder(Vec<String>),
    /// Reorder properties: `reorder properties = [...]`.
    PropertyOrder(Vec<String>),
}

/// A variant statement within a variant set.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVariant {
    /// Variant name.
    pub name: String,
    /// Metadata.
    pub metadata: Option<Metadata>,
    /// Contents (prim items).
    pub contents: Vec<ParsedPrimItem>,
}

/// A variant set statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedVariantSet {
    /// Variant set name.
    pub name: String,
    /// Variants.
    pub variants: Vec<ParsedVariant>,
}

/// A prim item (property, child prim, variant set, or ordering).
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedPrimItem {
    /// A property (attribute or relationship).
    Property(ParsedPropertySpec),
    /// A property list operation.
    PropertyListOp(PropertyListOp),
    /// A child prim.
    Prim(Box<ParsedPrimWithContents>),
    /// A variant set.
    VariantSet(ParsedVariantSet),
    /// Child ordering.
    ChildOrder(Vec<String>),
    /// Property ordering.
    PropertyOrder(Vec<String>),
}

/// A parsed prim with full contents (header + items).
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPrimWithContents {
    /// The prim header (specifier, type_name, name, metadata).
    pub header: ParsedPrimSpec,
    /// The prim contents (children, properties, variants, ordering).
    pub items: Vec<ParsedPrimItem>,
}

// ============================================================================
// Specs Parser
// ============================================================================

/// A parser for spec definitions.
pub struct SpecsParser<'a> {
    /// The underlying value parser.
    parser: ValueParser<'a>,
}

impl<'a> SpecsParser<'a> {
    /// Creates a new specs parser.
    pub fn new(source: &'a str) -> Self {
        Self {
            parser: ValueParser::new(source),
        }
    }

    /// Creates a specs parser from an existing lexer.
    pub fn from_lexer(lexer: Lexer<'a>) -> Self {
        Self {
            parser: ValueParser::from_lexer(lexer),
        }
    }

    /// Creates a specs parser from full state (lexer + current token).
    pub fn from_state(
        lexer: Lexer<'a>,
        current: Option<crate::text_parser::tokens::Token>,
    ) -> Self {
        Self {
            parser: ValueParser::from_state(lexer, current),
        }
    }

    /// Extracts full parser state.
    pub fn into_state(self) -> (Lexer<'a>, Option<crate::text_parser::tokens::Token>) {
        self.parser.into_state()
    }

    // ========================================================================
    // Prim Spec Parsing
    // ========================================================================

    /// Parses a prim spec: `def/over/class Type "Name" (metadata) { ... }`.
    pub fn parse_prim_spec(&mut self) -> ParseResult<ParsedPrimSpec> {
        // Parse specifier
        let specifier = self.parse_specifier()?;

        // Parse optional type name
        let type_name = self.parse_optional_type_name()?;

        // Parse prim name (quoted string)
        let name = self.parse_prim_name()?;

        // Parse optional metadata
        let metadata = if self.parser.check(&TokenKind::LeftParen) {
            Some(self.parse_metadata_block()?)
        } else {
            None
        };

        Ok(ParsedPrimSpec {
            specifier,
            type_name,
            name,
            metadata,
            is_custom: false,
        })
    }

    /// Parses a specifier keyword.
    fn parse_specifier(&mut self) -> ParseResult<Specifier> {
        match self.parser.peek_kind() {
            Some(TokenKind::Keyword(Keyword::Def)) => {
                self.parser.advance();
                Ok(Specifier::Def)
            }
            Some(TokenKind::Keyword(Keyword::Over)) => {
                self.parser.advance();
                Ok(Specifier::Over)
            }
            Some(TokenKind::Keyword(Keyword::Class)) => {
                self.parser.advance();
                Ok(Specifier::Class)
            }
            _ => Err(self.parser.error(ParseErrorKind::ExpectedKeyword(
                "def, over, or class".to_string(),
            ))),
        }
    }

    /// Parses an optional type name (identifier with possible dots).
    fn parse_optional_type_name(&mut self) -> ParseResult<Option<String>> {
        // Check if next is an identifier (type name) followed by another identifier or string
        if let Some(TokenKind::Identifier(_)) = self.parser.peek_kind() {
            // Could be type name or prim name - we need to look ahead
            // For now, simple heuristic: if next-next is a string, current is type
            let ident = self.parser.parse_identifier()?;

            // Check for dotted type name (e.g., UsdGeom.Mesh)
            let mut type_name = ident;
            while self.parser.check(&TokenKind::Dot) {
                self.parser.advance();
                let next = self.parser.parse_identifier()?;
                type_name.push('.');
                type_name.push_str(&next);
            }

            Ok(Some(type_name))
        } else {
            Ok(None)
        }
    }

    /// Parses a prim name (quoted string).
    fn parse_prim_name(&mut self) -> ParseResult<String> {
        let token = self
            .parser
            .advance()
            .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;

        match token.kind {
            TokenKind::String(s) => Ok(s),
            _ => Err(self.parser.error(ParseErrorKind::ExpectedString)),
        }
    }

    // ========================================================================
    // Attribute Spec Parsing
    // ========================================================================

    /// Parses an attribute spec.
    pub fn parse_attribute_spec(&mut self) -> ParseResult<ParsedAttributeSpec> {
        // Parse optional 'custom'
        let is_custom = if self.parser.check_keyword(Keyword::Custom) {
            self.parser.advance();
            true
        } else {
            false
        };

        // Parse optional variability
        let variability = self.parse_variability();

        // Parse type name, supporting C++ namespaced forms like `Some::EnumValue`
        let type_name = self.parser.parse_cxx_namespaced_identifier()?;

        // Parse optional array brackets
        let is_array = if self.parser.check(&TokenKind::LeftBracket) {
            self.parser.advance();
            self.parser.expect(&TokenKind::RightBracket)?;
            true
        } else {
            false
        };

        // Parse attribute name (possibly namespaced)
        let name = self.parser.parse_namespaced_identifier()?;

        // Parse assignment or property accessor
        let (default_value, time_samples, connections, spline) =
            self.parse_attribute_value_or_accessor()?;

        // Parse optional metadata
        let metadata = if self.parser.check(&TokenKind::LeftParen) {
            Some(self.parse_metadata_block()?)
        } else {
            None
        };

        Ok(ParsedAttributeSpec {
            is_custom,
            variability,
            type_name,
            is_array,
            name,
            default_value,
            time_samples,
            connections,
            spline,
            metadata,
        })
    }

    /// Parses variability (uniform/config).
    fn parse_variability(&mut self) -> Variability {
        match self.parser.peek_kind() {
            Some(TokenKind::Keyword(Keyword::Uniform)) => {
                self.parser.advance();
                Variability::Uniform
            }
            Some(TokenKind::Keyword(Keyword::Config)) => {
                self.parser.advance();
                Variability::Config
            }
            _ => Variability::Varying,
        }
    }

    /// Parses attribute value or accessor (.timeSamples, .connect, .spline).
    fn parse_attribute_value_or_accessor(
        &mut self,
    ) -> ParseResult<(
        Option<Value>,
        Option<TimeSampleMap>,
        Option<Vec<String>>,
        Option<SplineValue>,
    )> {
        // Check for dot accessor
        if self.parser.check(&TokenKind::Dot) {
            self.parser.advance();

            match self.parser.peek_kind() {
                // .timeSamples = { ... }
                Some(TokenKind::Keyword(Keyword::TimeSamples)) => {
                    self.parser.advance();
                    self.parser.expect(&TokenKind::Equals)?;
                    let samples = self.parser.parse_time_sample_map()?;
                    return Ok((None, Some(samples), None, None));
                }
                // .connect = path or [paths]
                Some(TokenKind::Keyword(Keyword::Connect)) => {
                    self.parser.advance();
                    self.parser.expect(&TokenKind::Equals)?;
                    let connections = self.parse_connection_targets()?;
                    return Ok((None, None, Some(connections), None));
                }
                // .spline = { ... }
                Some(TokenKind::Keyword(Keyword::Spline)) => {
                    self.parser.advance();
                    self.parser.expect(&TokenKind::Equals)?;
                    let spline = self.parser.parse_spline_value()?;
                    return Ok((None, None, None, Some(spline)));
                }
                // Note: '.default' is NOT a valid attribute accessor per C++ grammar;
                // only relationships support '.default'. Attributes use '= value' directly.
                _ => {
                    return Err(self.parser.error(ParseErrorKind::UnexpectedToken(
                        "expected timeSamples, connect, or spline".to_string(),
                    )));
                }
            }
        }

        // Check for assignment
        if self.parser.check(&TokenKind::Equals) {
            self.parser.advance();

            // Check for None
            if self.parser.check_keyword(Keyword::None)
                || self.parser.check_keyword(Keyword::NoneLowercase)
            {
                self.parser.advance();
                return Ok((None, None, None, None));
            }

            // AnimationBlock is a dedicated attribute-value sentinel, not a generic value.
            if self.parser.check_keyword(Keyword::AnimationBlock) {
                self.parser.advance();
                return Ok((Some(Value::AnimationBlock), None, None, None));
            }

            // Parse value
            let value = self.parser.parse_value()?;
            return Ok((Some(value), None, None, None));
        }

        // No value
        Ok((None, None, None, None))
    }

    /// Parses connection targets (path or list of paths).
    fn parse_connection_targets(&mut self) -> ParseResult<Vec<String>> {
        // Check for None
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Vec::new());
        }

        // Check for single path
        if let Some(TokenKind::PathRef(_)) = self.parser.peek_kind() {
            let token = self.parser.advance().expect("token after peek");
            match token.kind {
                TokenKind::PathRef(path) => return Ok(vec![path]),
                _ => unreachable!(),
            }
        }

        // Parse list of paths
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut paths = Vec::new();

        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;

            match token.kind {
                TokenKind::PathRef(path) => paths.push(path),
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            }

            // Optional comma
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(paths)
    }

    // ========================================================================
    // Relationship Spec Parsing
    // ========================================================================

    /// Parses a relationship spec.
    pub fn parse_relationship_spec(&mut self) -> ParseResult<ParsedRelationshipSpec> {
        // Parse optional 'custom'
        let is_custom = if self.parser.check_keyword(Keyword::Custom) {
            self.parser.advance();
            true
        } else {
            false
        };

        // Parse optional 'varying'
        let is_varying = if self.parser.check_keyword(Keyword::Varying) {
            self.parser.advance();
            true
        } else {
            false
        };

        // Expect 'rel'
        self.parser.expect_keyword(Keyword::Rel)?;

        // Parse relationship name (possibly namespaced)
        let name = self.parser.parse_namespaced_identifier()?;

        // Parse optional assignment or accessor
        let (targets, default_target, time_samples) =
            self.parse_relationship_value_or_accessor()?;

        // Parse optional metadata
        let metadata = if self.parser.check(&TokenKind::LeftParen) {
            Some(self.parse_metadata_block()?)
        } else {
            None
        };

        Ok(ParsedRelationshipSpec {
            is_custom,
            is_varying,
            name,
            targets,
            default_target,
            time_samples,
            metadata,
        })
    }

    /// Parses relationship value or accessor.
    ///
    /// Handles:
    /// - `.default = </path>` — default target
    /// - `.timeSamples = { 0: </p>, ... }` — animated targets
    /// - `[</path>]` — target accessor (bracket syntax)
    /// - `= </path>` or `= [...]` — target assignment
    fn parse_relationship_value_or_accessor(
        &mut self,
    ) -> ParseResult<(Option<Vec<String>>, Option<String>, Option<TimeSampleMap>)> {
        // Check for dot accessor (.default or .timeSamples)
        if self.parser.check(&TokenKind::Dot) {
            self.parser.advance();

            // .default = </path>
            if self.parser.check_keyword(Keyword::Default) {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;

                let token = self
                    .parser
                    .advance()
                    .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;

                match token.kind {
                    TokenKind::PathRef(path) => return Ok((None, Some(path), None)),
                    _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
                }
            }

            // .timeSamples = { time: targets, ... }
            if self.parser.check_keyword(Keyword::TimeSamples) {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let time_samples = self.parse_relationship_time_samples()?;
                return Ok((None, None, Some(time_samples)));
            }

            return Err(self.parser.error(ParseErrorKind::UnexpectedToken(
                "expected 'default' or 'timeSamples'".to_string(),
            )));
        }

        // [</path>] — target accessor bracket syntax
        if self.parser.check(&TokenKind::LeftBracket) {
            self.parser.advance();
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;
            let target_path = match token.kind {
                TokenKind::PathRef(p) => p,
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            };
            self.parser.expect(&TokenKind::RightBracket)?;
            return Ok((Some(vec![target_path]), None, None));
        }

        // = </path> or = [</p1>, </p2>] or = None
        if self.parser.check(&TokenKind::Equals) {
            self.parser.advance();
            let targets = self.parse_relationship_targets()?;
            return Ok((Some(targets), None, None));
        }

        // No value
        Ok((None, None, None))
    }

    /// Parses relationship time samples: `{ time: targets, ... }`.
    ///
    /// Each sample maps a time code to either `None` (blocked), a single path,
    /// or a list of paths. Reuses `ValueParser::parse_time_sample_map()` which
    /// already handles `Value::Path` via `parse_value()`.
    fn parse_relationship_time_samples(&mut self) -> ParseResult<TimeSampleMap> {
        // Delegate to the existing time sample map parser in ValueParser.
        // parse_value() handles PathRef tokens as Value::Path, so this works
        // for relationship targets (paths) as well as attribute values.
        self.parser.parse_time_sample_map()
    }

    /// Parses relationship targets.
    fn parse_relationship_targets(&mut self) -> ParseResult<Vec<String>> {
        // Check for None
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Vec::new());
        }

        // Check for single path
        if let Some(TokenKind::PathRef(_)) = self.parser.peek_kind() {
            let token = self.parser.advance().expect("token after peek");
            match token.kind {
                TokenKind::PathRef(path) => return Ok(vec![path]),
                _ => unreachable!(),
            }
        }

        // Parse list of paths
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut paths = Vec::new();

        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;

            match token.kind {
                TokenKind::PathRef(path) => paths.push(path),
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            }

            // Optional comma
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(paths)
    }

    // ========================================================================
    // Property Spec Parsing
    // ========================================================================

    /// Parses a property spec (attribute or relationship).
    ///
    /// C++ grammar (textFileFormatParser.h ~line 1475):
    ///   RelationshipType = 'rel' | 'custom' 'rel' | 'custom' 'varying' 'rel' | 'varying' 'rel'
    ///
    /// Strategy: greedy lookahead — consume optional 'custom'/'varying' prefixes
    /// then check if 'rel' follows. Backtrack and dispatch accordingly.
    pub fn parse_property_spec(&mut self) -> ParseResult<ParsedPropertySpec> {
        // Save state so we can backtrack after lookahead
        let saved = self.parser.save_state();

        // Consume optional 'custom'
        if self.parser.check_keyword(Keyword::Custom) {
            self.parser.advance();
        }

        // Consume optional 'varying'
        if self.parser.check_keyword(Keyword::Varying) {
            self.parser.advance();
        }

        // After skipping prefixes, is next token 'rel'?
        let is_rel = self.parser.check_keyword(Keyword::Rel);

        // Always restore — let the dedicated parsers consume their own tokens
        self.parser.restore_state(saved);

        if is_rel {
            Ok(ParsedPropertySpec::Relationship(
                self.parse_relationship_spec()?,
            ))
        } else {
            Ok(ParsedPropertySpec::Attribute(self.parse_attribute_spec()?))
        }
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Parses a metadata block.
    fn parse_metadata_block(&mut self) -> ParseResult<Metadata> {
        // Extract full state from current parser
        let (lexer, current) =
            std::mem::replace(&mut self.parser, ValueParser::new("")).into_state();

        // Create metadata parser with full state
        let mut meta_parser = MetadataParser::from_state(lexer, current);
        let result = meta_parser.parse_metadata_block()?;

        // Restore full state back
        let (lexer, current) = meta_parser.into_state();
        self.parser = ValueParser::from_state(lexer, current);
        Ok(result)
    }

    /// Returns a reference to the underlying parser.
    pub fn parser(&self) -> &ValueParser<'a> {
        &self.parser
    }

    /// Returns a mutable reference to the underlying parser.
    pub fn parser_mut(&mut self) -> &mut ValueParser<'a> {
        &mut self.parser
    }

    /// Consumes this parser and returns the underlying value parser.
    pub fn into_parser(self) -> ValueParser<'a> {
        self.parser
    }
}

impl<'a> ValueParser<'a> {
    /// Consumes this parser and returns the underlying lexer.
    pub fn into_inner(self) -> Lexer<'a> {
        self.lexer
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Prim Spec Tests
    // ========================================================================

    #[test]
    fn test_parse_def_prim() {
        let mut parser = SpecsParser::new(r#"def "World""#);
        let result = parser.parse_prim_spec().unwrap();
        assert_eq!(result.specifier, Specifier::Def);
        assert_eq!(result.type_name, None);
        assert_eq!(result.name, "World");
    }

    #[test]
    fn test_parse_def_with_type() {
        let mut parser = SpecsParser::new(r#"def Xform "World""#);
        let result = parser.parse_prim_spec().unwrap();
        assert_eq!(result.specifier, Specifier::Def);
        assert_eq!(result.type_name, Some("Xform".to_string()));
        assert_eq!(result.name, "World");
    }

    #[test]
    fn test_parse_over_prim() {
        let mut parser = SpecsParser::new(r#"over "Existing""#);
        let result = parser.parse_prim_spec().unwrap();
        assert_eq!(result.specifier, Specifier::Over);
        assert_eq!(result.name, "Existing");
    }

    #[test]
    fn test_parse_class_prim() {
        let mut parser = SpecsParser::new(r#"class "_MyClass""#);
        let result = parser.parse_prim_spec().unwrap();
        assert_eq!(result.specifier, Specifier::Class);
        assert_eq!(result.name, "_MyClass");
    }

    // ========================================================================
    // Attribute Spec Tests
    // ========================================================================

    #[test]
    fn test_parse_simple_attribute() {
        let mut parser = SpecsParser::new("float value = 1.0");
        let result = parser.parse_attribute_spec().unwrap();
        assert!(!result.is_custom);
        assert_eq!(result.variability, Variability::Varying);
        assert_eq!(result.type_name, "float");
        assert!(!result.is_array);
        assert_eq!(result.name, "value");
        assert!(result.default_value.is_some());
    }

    #[test]
    fn test_parse_custom_attribute() {
        let mut parser = SpecsParser::new("custom double3 myAttr = (1, 2, 3)");
        let result = parser.parse_attribute_spec().unwrap();
        assert!(result.is_custom);
        assert_eq!(result.type_name, "double3");
        assert_eq!(result.name, "myAttr");
    }

    #[test]
    fn test_parse_uniform_attribute() {
        let mut parser = SpecsParser::new(r#"uniform token visibility = "invisible""#);
        let result = parser.parse_attribute_spec().unwrap();
        assert_eq!(result.variability, Variability::Uniform);
        assert_eq!(result.type_name, "token");
        assert_eq!(result.name, "visibility");
    }

    #[test]
    fn test_parse_array_attribute() {
        let mut parser = SpecsParser::new("int[] values = [1, 2, 3]");
        let result = parser.parse_attribute_spec().unwrap();
        assert!(result.is_array);
        assert_eq!(result.type_name, "int");
    }

    #[test]
    fn test_parse_namespaced_attribute() {
        let mut parser = SpecsParser::new("float3 xformOp:translate = (0, 0, 0)");
        let result = parser.parse_attribute_spec().unwrap();
        assert_eq!(result.name, "xformOp:translate");
    }

    #[test]
    fn test_parse_attribute_no_value() {
        let mut parser = SpecsParser::new("float myAttr");
        let result = parser.parse_attribute_spec().unwrap();
        assert_eq!(result.name, "myAttr");
        assert!(result.default_value.is_none());
    }

    #[test]
    fn test_parse_attribute_animation_block() {
        let mut parser = SpecsParser::new("int a = AnimationBlock");
        let result = parser.parse_attribute_spec().unwrap();
        assert_eq!(result.name, "a");
        assert!(matches!(result.default_value, Some(Value::AnimationBlock)));
    }

    // ========================================================================
    // Relationship Spec Tests
    // ========================================================================

    #[test]
    fn test_parse_simple_relationship() {
        let mut parser = SpecsParser::new("rel target = </Path/To/Target>");
        let result = parser.parse_relationship_spec().unwrap();
        assert!(!result.is_custom);
        assert_eq!(result.name, "target");
        assert_eq!(result.targets, Some(vec!["/Path/To/Target".to_string()]));
    }

    #[test]
    fn test_parse_relationship_list() {
        let mut parser = SpecsParser::new("rel targets = [</A>, </B>, </C>]");
        let result = parser.parse_relationship_spec().unwrap();
        assert_eq!(result.name, "targets");
        let targets = result.targets.unwrap();
        assert_eq!(targets.len(), 3);
        assert_eq!(targets[0], "/A");
        assert_eq!(targets[1], "/B");
        assert_eq!(targets[2], "/C");
    }

    #[test]
    fn test_parse_custom_relationship() {
        let mut parser = SpecsParser::new("custom rel myRel");
        let result = parser.parse_relationship_spec().unwrap();
        assert!(result.is_custom);
        assert_eq!(result.name, "myRel");
        assert!(result.targets.is_none());
    }

    #[test]
    fn test_parse_relationship_none() {
        let mut parser = SpecsParser::new("rel target = None");
        let result = parser.parse_relationship_spec().unwrap();
        assert_eq!(result.targets, Some(Vec::new()));
    }

    #[test]
    fn test_parse_relationship_time_samples() {
        // rel foo.timeSamples = { 0: </Target1>, 10: </Target2> }
        let mut parser =
            SpecsParser::new("rel foo.timeSamples = { 0: </Target1>, 10: </Target2> }");
        let result = parser.parse_relationship_spec().unwrap();
        assert_eq!(result.name, "foo");
        assert!(result.targets.is_none());
        assert!(result.default_target.is_none());
        let ts = result.time_samples.unwrap();
        assert_eq!(ts.samples.len(), 2);
        assert_eq!(ts.samples[0].time, 0.0);
        assert_eq!(ts.samples[1].time, 10.0);
        // Values are Path variants
        use crate::text_parser::value_context::Value;
        assert!(matches!(&ts.samples[0].value, Some(Value::Path(p)) if p == "/Target1"));
        assert!(matches!(&ts.samples[1].value, Some(Value::Path(p)) if p == "/Target2"));
    }

    #[test]
    fn test_parse_relationship_time_samples_with_list_and_none() {
        // rel foo.timeSamples = { 0: [</T1>, </T2>], 5: None }
        let mut parser = SpecsParser::new("rel foo.timeSamples = { 0: [</T1>, </T2>], 5: None }");
        let result = parser.parse_relationship_spec().unwrap();
        let ts = result.time_samples.unwrap();
        assert_eq!(ts.samples.len(), 2);
        // First sample: list of paths
        assert_eq!(ts.samples[0].time, 0.0);
        use crate::text_parser::value_context::Value;
        if let Some(Value::List(items)) = &ts.samples[0].value {
            assert_eq!(items.len(), 2);
            assert!(matches!(&items[0], Value::Path(p) if p == "/T1"));
            assert!(matches!(&items[1], Value::Path(p) if p == "/T2"));
        } else {
            panic!("expected Value::List for sample 0");
        }
        // Second sample: blocked (None)
        assert_eq!(ts.samples[1].time, 5.0);
        assert!(ts.samples[1].value.is_none());
    }

    #[test]
    fn test_parse_relationship_bracket_accessor() {
        // rel foo[</TargetPath>]  — bracket accessor syntax
        let mut parser = SpecsParser::new("rel foo[</TargetPath>]");
        let result = parser.parse_relationship_spec().unwrap();
        assert_eq!(result.name, "foo");
        assert_eq!(result.targets, Some(vec!["/TargetPath".to_string()]));
        assert!(result.default_target.is_none());
        assert!(result.time_samples.is_none());
    }

    // ========================================================================
    // Specifier Tests
    // ========================================================================

    #[test]
    fn test_specifier_as_str() {
        assert_eq!(Specifier::Def.as_str(), "def");
        assert_eq!(Specifier::Over.as_str(), "over");
        assert_eq!(Specifier::Class.as_str(), "class");
    }

    // ========================================================================
    // Property Spec Lookahead Tests (Bug 1 fix)
    // ========================================================================

    #[test]
    fn test_parse_property_spec_plain_rel() {
        // Bare 'rel' with no prefixes
        let mut parser = SpecsParser::new("rel foo = </path>");
        let result = parser.parse_property_spec().unwrap();
        assert!(matches!(result, ParsedPropertySpec::Relationship(_)));
        if let ParsedPropertySpec::Relationship(rel) = result {
            assert_eq!(rel.name, "foo");
            assert!(!rel.is_custom);
        }
    }

    #[test]
    fn test_parse_property_spec_custom_rel() {
        // 'custom rel foo' must be recognized as a relationship, not an attribute
        let mut parser = SpecsParser::new("custom rel foo = </path>");
        let result = parser.parse_property_spec().unwrap();
        assert!(matches!(result, ParsedPropertySpec::Relationship(_)));
        if let ParsedPropertySpec::Relationship(rel) = result {
            assert_eq!(rel.name, "foo");
            assert!(rel.is_custom);
        }
    }

    #[test]
    fn test_parse_property_spec_varying_rel() {
        // 'varying rel foo' must be recognized as a relationship
        let mut parser = SpecsParser::new("varying rel foo = </path>");
        let result = parser.parse_property_spec().unwrap();
        assert!(matches!(result, ParsedPropertySpec::Relationship(_)));
        if let ParsedPropertySpec::Relationship(rel) = result {
            assert_eq!(rel.name, "foo");
        }
    }

    #[test]
    fn test_parse_property_spec_custom_varying_rel() {
        // 'custom varying rel foo' — all three prefixes
        let mut parser = SpecsParser::new("custom varying rel foo");
        let result = parser.parse_property_spec().unwrap();
        assert!(matches!(result, ParsedPropertySpec::Relationship(_)));
        if let ParsedPropertySpec::Relationship(rel) = result {
            assert_eq!(rel.name, "foo");
            assert!(rel.is_custom);
        }
    }

    #[test]
    fn test_parse_property_spec_custom_attr() {
        // 'custom float foo' must still be parsed as an attribute
        let mut parser = SpecsParser::new("custom float foo = 1.0");
        let result = parser.parse_property_spec().unwrap();
        assert!(matches!(result, ParsedPropertySpec::Attribute(_)));
        if let ParsedPropertySpec::Attribute(attr) = result {
            assert_eq!(attr.name, "foo");
            assert!(attr.is_custom);
        }
    }
}
