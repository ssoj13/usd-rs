//! Metadata parsing for USDA format.
//!
//! This module parses metadata blocks that appear in various USD constructs:
//! - Layer metadata: `#usda 1.0 ( ... )`
//! - Prim metadata: `def Xform "Name" ( ... )`
//! - Property metadata: `float attr = 1.0 ( ... )`
//!
//! # C++ Parity
//!
//! Matches the metadata rules from `textFileFormatParser.h`:
//! ```text
//! MetadataBlock = '(' MetadataItem* ')'                    // lines 625-636
//! KeyValueMetadata = Key '=' Value                         // lines 646-656
//! DocMetadata = 'doc' '=' String                           // lines 665-669
//! GeneralListOpMetadata = ListOp Key '=' Value             // lines 688-705
//! SharedMetadata = String | KeyValueMetadata | DocMetadata // lines 707-718
//! ```
//!
//! # Metadata Types
//!
//! ## Key-Value Metadata
//! Simple key = value pairs:
//! ```text
//! customData = { ... }
//! kind = "component"
//! ```
//!
//! ## List Operation Metadata
//! List edit operations:
//! ```text
//! prepend apiSchemas = ["GeomModelAPI"]
//! add references = @./other.usda@
//! ```
//!
//! ## Special Metadata
//! - `doc = "documentation string"`
//! - `permission = public/private`
//! - `symmetryFunction = func`

use crate::text_parser::error::{ParseErrorKind, ParseResult};
use crate::text_parser::lexer::Lexer;
use crate::text_parser::tokens::{Keyword, TokenKind};
use crate::text_parser::value_context::{ArrayEditOp, Value};
use crate::text_parser::values::ValueParser;

// ============================================================================
// Metadata Types
// ============================================================================

/// A single metadata entry.
#[derive(Debug, Clone, PartialEq)]
pub enum MetadataEntry {
    /// Documentation string: `doc = "text"`.
    Doc(String),

    /// Key-value pair: `key = value`.
    KeyValue {
        /// The metadata key.
        key: String,
        /// The value.
        value: Value,
    },

    /// List operation: `prepend key = value`.
    ListOp {
        /// The operation type.
        op: ArrayEditOp,
        /// The metadata key.
        key: String,
        /// The value (usually a list).
        value: Value,
    },

    /// Permission metadata: `permission = public`.
    Permission(String),

    /// Symmetry function: `symmetryFunction = func`.
    SymmetryFunction(Option<String>),

    /// Display unit: `displayUnit = meter`.
    DisplayUnit(String),

    /// Comment string (bare string in metadata).
    Comment(String),
}

/// A collection of metadata entries.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    /// The entries in order.
    pub entries: Vec<MetadataEntry>,
}

impl Metadata {
    /// Creates empty metadata.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if no metadata entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Gets the documentation string if present.
    pub fn doc(&self) -> Option<&str> {
        for entry in &self.entries {
            if let MetadataEntry::Doc(s) = entry {
                return Some(s);
            }
        }
        None
    }

    /// Gets a key-value entry by key name.
    pub fn get(&self, key: &str) -> Option<&Value> {
        for entry in &self.entries {
            if let MetadataEntry::KeyValue { key: k, value: v } = entry {
                if k == key {
                    return Some(v);
                }
            }
        }
        None
    }

    /// Gets the customData dictionary if present.
    pub fn custom_data(&self) -> Option<&Value> {
        self.get("customData")
    }

    /// Gets the kind value if present.
    pub fn kind(&self) -> Option<&str> {
        self.get("kind").and_then(|v| v.as_string())
    }
}

// ============================================================================
// Metadata Parser
// ============================================================================

/// A parser for metadata blocks.
pub struct MetadataParser<'a> {
    /// The underlying value parser.
    parser: ValueParser<'a>,
}

impl<'a> MetadataParser<'a> {
    /// Creates a new metadata parser.
    pub fn new(source: &'a str) -> Self {
        Self {
            parser: ValueParser::new(source),
        }
    }

    /// Creates a metadata parser from an existing lexer.
    pub fn from_lexer(lexer: Lexer<'a>) -> Self {
        Self {
            parser: ValueParser::from_lexer(lexer),
        }
    }

    /// Creates a metadata parser from full state (lexer + current token).
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

    /// Parses a metadata block: `( entries... )`.
    ///
    /// # Grammar
    ///
    /// ```text
    /// MetadataBlock = '(' MetadataItem* ')'
    /// MetadataItem = DocMetadata | KeyValueMetadata | ListOpMetadata | String
    /// ```
    pub fn parse_metadata_block(&mut self) -> ParseResult<Metadata> {
        // Expect opening paren
        self.parser.expect(&TokenKind::LeftParen)?;

        let metadata = self.parse_metadata_contents()?;

        // Expect closing paren
        self.parser.expect(&TokenKind::RightParen)?;

        Ok(metadata)
    }

    /// Parses metadata contents (without surrounding parens).
    pub fn parse_metadata_contents(&mut self) -> ParseResult<Metadata> {
        let mut metadata = Metadata::new();

        // Parse entries until closing paren or end
        while !self.parser.check(&TokenKind::RightParen) && !self.parser.is_at_end() {
            let entry = self.parse_metadata_entry()?;
            metadata.entries.push(entry);

            // Entries can be separated by semicolons or newlines
            self.parser.match_kind(&TokenKind::Semicolon);
        }

        Ok(metadata)
    }

    /// Parses a single metadata entry.
    fn parse_metadata_entry(&mut self) -> ParseResult<MetadataEntry> {
        // Check for list operation keywords first
        if self.is_list_op_start() {
            return self.parse_list_op_metadata();
        }

        match self.parser.peek_kind() {
            // doc = "string"
            Some(TokenKind::Keyword(Keyword::Doc)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let token = self
                    .parser
                    .advance()
                    .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;
                match token.kind {
                    TokenKind::String(s) => Ok(MetadataEntry::Doc(s)),
                    _ => Err(self.parser.error(ParseErrorKind::ExpectedString)),
                }
            }

            // permission = identifier
            Some(TokenKind::Keyword(Keyword::Permission)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let ident = self.parser.parse_identifier()?;
                Ok(MetadataEntry::Permission(ident))
            }

            // symmetryFunction = identifier?
            Some(TokenKind::Keyword(Keyword::SymmetryFunction)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let func = if self.parser.check(&TokenKind::Identifier(String::new())) {
                    Some(self.parser.parse_identifier()?)
                } else {
                    None
                };
                Ok(MetadataEntry::SymmetryFunction(func))
            }

            // displayUnit = identifier
            Some(TokenKind::Keyword(Keyword::DisplayUnit)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let unit = self.parser.parse_identifier()?;
                Ok(MetadataEntry::DisplayUnit(unit))
            }

            // Bare string (comment)
            Some(TokenKind::String(_)) => {
                let token = self.parser.advance().expect("token after peek");
                match token.kind {
                    TokenKind::String(s) => Ok(MetadataEntry::Comment(s)),
                    _ => unreachable!(),
                }
            }

            // customData = { ... }
            Some(TokenKind::Keyword(Keyword::CustomData)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parser.parse_dictionary_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "customData".to_string(),
                    value,
                })
            }

            // symmetryArguments = { ... }
            Some(TokenKind::Keyword(Keyword::SymmetryArguments)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parser.parse_dictionary_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "symmetryArguments".to_string(),
                    value,
                })
            }

            // references = ReferenceList
            Some(TokenKind::Keyword(Keyword::References)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_references_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "references".to_string(),
                    value,
                })
            }

            // payload = PayloadList
            Some(TokenKind::Keyword(Keyword::Payload)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_payload_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "payload".to_string(),
                    value,
                })
            }

            // inherits = PathList
            Some(TokenKind::Keyword(Keyword::Inherits)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_path_list_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "inherits".to_string(),
                    value,
                })
            }

            // specializes = PathList
            Some(TokenKind::Keyword(Keyword::Specializes)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_path_list_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "specializes".to_string(),
                    value,
                })
            }

            // relocates = { </Old>: </New>, ... }
            Some(TokenKind::Keyword(Keyword::Relocates)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_relocates_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "relocates".to_string(),
                    value,
                })
            }

            // subLayers = [ @asset@ (offset=N; scale=N), ... ]
            Some(TokenKind::Keyword(Keyword::SubLayers)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_sublayers_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "subLayers".to_string(),
                    value,
                })
            }

            // variantSets = NameList (C++ VariantSetsMetadata rule)
            Some(TokenKind::Keyword(Keyword::VariantSets)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = if self.parser.check_keyword(Keyword::None)
                    || self.parser.check_keyword(Keyword::NoneLowercase)
                {
                    self.parser.advance();
                    Value::None
                } else {
                    self.parse_variant_sets_name_list()?
                };
                Ok(MetadataEntry::KeyValue {
                    key: "variantSets".to_string(),
                    value,
                })
            }

            // key = value (generic key-value)
            Some(TokenKind::Identifier(_)) => {
                let key = self.parser.parse_identifier()?;
                self.parser.expect(&TokenKind::Equals)?;

                // Value can be None, dictionary, or typed value
                let value = if self.parser.check_keyword(Keyword::None)
                    || self.parser.check_keyword(Keyword::NoneLowercase)
                {
                    self.parser.advance();
                    Value::None
                } else if self.parser.check(&TokenKind::LeftBrace) {
                    self.parser.parse_dictionary_value()?
                } else {
                    self.parser.parse_value()?
                };

                Ok(MetadataEntry::KeyValue { key, value })
            }

            // prefixSubstitutions = { "key": "value", ... }
            Some(TokenKind::Keyword(Keyword::PrefixSubstitutions)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parser.parse_string_map_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "prefixSubstitutions".to_string(),
                    value,
                })
            }

            // suffixSubstitutions = { "key": "value", ... }
            Some(TokenKind::Keyword(Keyword::SuffixSubstitutions)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parser.parse_string_map_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "suffixSubstitutions".to_string(),
                    value,
                })
            }

            // Keyword used as key (e.g., kind = "component")
            Some(TokenKind::Keyword(kw)) => {
                let key = kw.as_str().to_string();
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;

                let value = if self.parser.check_keyword(Keyword::None)
                    || self.parser.check_keyword(Keyword::NoneLowercase)
                {
                    self.parser.advance();
                    Value::None
                } else if self.parser.check(&TokenKind::LeftBrace) {
                    self.parser.parse_dictionary_value()?
                } else {
                    self.parser.parse_value()?
                };

                Ok(MetadataEntry::KeyValue { key, value })
            }

            Some(kind) => Err(self.parser.error(ParseErrorKind::UnexpectedToken(format!(
                "expected metadata entry, found {}",
                kind
            )))),

            None => Err(self.parser.error(ParseErrorKind::UnexpectedEof)),
        }
    }

    /// Checks if current token starts a list operation.
    fn is_list_op_start(&self) -> bool {
        matches!(
            self.parser.peek_kind(),
            Some(TokenKind::Keyword(Keyword::Add))
                | Some(TokenKind::Keyword(Keyword::Delete))
                | Some(TokenKind::Keyword(Keyword::Append))
                | Some(TokenKind::Keyword(Keyword::Prepend))
                | Some(TokenKind::Keyword(Keyword::Reorder))
        )
    }

    /// Parses list operation metadata: `prepend key = value`.
    ///
    /// Special handling for `variantSets` key: parses value as NameList
    /// (single string or string array) per C++ VariantSetsMetadata rule.
    fn parse_list_op_metadata(&mut self) -> ParseResult<MetadataEntry> {
        let op_token = self.parser.advance().expect("token after peek");
        let op = match &op_token.kind {
            TokenKind::Keyword(Keyword::Add) => ArrayEditOp::Add,
            TokenKind::Keyword(Keyword::Delete) => ArrayEditOp::Delete,
            TokenKind::Keyword(Keyword::Append) => ArrayEditOp::Append,
            TokenKind::Keyword(Keyword::Prepend) => ArrayEditOp::Prepend,
            TokenKind::Keyword(Keyword::Reorder) => ArrayEditOp::Reorder,
            _ => unreachable!(),
        };

        // Parse key
        let key = if let Some(TokenKind::Keyword(kw)) = self.parser.peek_kind() {
            let s = kw.as_str().to_string();
            self.parser.advance();
            s
        } else {
            self.parser.parse_identifier()?
        };

        self.parser.expect(&TokenKind::Equals)?;

        // Value can be None or a composition arc list
        let value = if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            Value::None
        } else if key == "variantSets" {
            // C++ VariantSetsMetadata: value is NameList (string or [string, ...])
            self.parse_variant_sets_name_list()?
        } else if key == "references" {
            self.parse_references_value()?
        } else if key == "payload" {
            self.parse_payload_value()?
        } else if key == "inherits" || key == "specializes" {
            self.parse_path_list_value()?
        } else if key == "subLayers" {
            self.parse_sublayers_value()?
        } else {
            self.parser.parse_value()?
        };

        Ok(MetadataEntry::ListOp { op, key, value })
    }

    /// Parses a NameList value for variantSets: single string or `["a", "b"]`.
    ///
    /// Returns `Value::List` of strings to match C++ behavior.
    fn parse_variant_sets_name_list(&mut self) -> ParseResult<Value> {
        // Single string: "name" → List([String("name")])
        if let Some(TokenKind::String(_)) = self.parser.peek_kind() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
            if let TokenKind::String(s) = token.kind {
                return Ok(Value::List(vec![Value::String(s)]));
            }
        }

        // Array: ["name1", "name2"]
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut names = Vec::new();
        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;
            match token.kind {
                TokenKind::String(s) => names.push(Value::String(s)),
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedString)),
            }
            self.parser.match_kind(&TokenKind::Comma);
        }
        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Value::List(names))
    }

    // ========================================================================
    // Composition Arc Parsers
    // ========================================================================

    /// Parses layer offset params: `(offset = N; scale = N)`.
    ///
    /// Both offset and scale are optional; defaults are 0.0 and 1.0.
    /// Returns `(offset, scale)`.
    fn parse_layer_offset_params(&mut self) -> ParseResult<(f64, f64)> {
        let mut offset = 0.0f64;
        let mut scale = 1.0f64;

        // Optional parens block: (offset = N; scale = N)
        if !self.parser.check(&TokenKind::LeftParen) {
            return Ok((offset, scale));
        }
        self.parser.advance(); // consume '('

        // Parse semicolon-separated offset/scale fields until ')'
        while !self.parser.check(&TokenKind::RightParen) && !self.parser.is_at_end() {
            let key_token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;

            match &key_token.kind {
                TokenKind::Keyword(Keyword::Offset) => {
                    self.parser.expect(&TokenKind::Equals)?;
                    let val = self.parser.parse_number()?;
                    offset = val.as_f64().unwrap_or(0.0);
                }
                TokenKind::Keyword(Keyword::Scale) => {
                    self.parser.expect(&TokenKind::Equals)?;
                    let val = self.parser.parse_number()?;
                    scale = val.as_f64().unwrap_or(1.0);
                }
                other => {
                    return Err(self.parser.error(ParseErrorKind::UnexpectedToken(format!(
                        "expected 'offset' or 'scale', found {}",
                        other
                    ))));
                }
            }

            // Fields separated by semicolons
            self.parser.match_kind(&TokenKind::Semicolon);
        }

        self.parser.expect(&TokenKind::RightParen)?;
        Ok((offset, scale))
    }

    /// Parses a single reference item.
    ///
    /// Grammar:
    /// ```text
    /// ReferenceItem = (AssetRef PathRef?)? | PathRef
    ///                 followed by optional (offset=N; scale=N; customData={...})
    /// ```
    /// Returns `(asset_path, prim_path, offset, scale)`.
    fn parse_reference_item(&mut self) -> ParseResult<(String, String, f64, f64)> {
        let mut asset_path = String::new();
        let mut prim_path = String::new();

        match self.parser.peek_kind() {
            Some(TokenKind::AssetRef(_)) => {
                // @asset@ optionally followed by </path>
                let tok = self
                    .parser
                    .advance()
                    .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
                if let TokenKind::AssetRef(p) = tok.kind {
                    asset_path = p;
                }
                // Optional prim path after asset ref
                if let Some(TokenKind::PathRef(_)) = self.parser.peek_kind() {
                    let tok = self
                        .parser
                        .advance()
                        .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
                    if let TokenKind::PathRef(p) = tok.kind {
                        prim_path = p;
                    }
                }
            }
            Some(TokenKind::PathRef(_)) => {
                // Internal reference: </path>
                let tok = self
                    .parser
                    .advance()
                    .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
                if let TokenKind::PathRef(p) = tok.kind {
                    prim_path = p;
                }
            }
            _ => {
                return Err(self.parser.error(ParseErrorKind::UnexpectedToken(
                    "expected asset ref @...@ or path ref <...>".to_string(),
                )));
            }
        }

        // Optional layer offset params — skip customData for references too
        let (offset, scale) = self.parse_layer_offset_params_with_custom_data()?;

        Ok((asset_path, prim_path, offset, scale))
    }

    /// Parses layer offset params allowing an optional `customData = {...}` field.
    ///
    /// This is used for references (which permit customData inside the parens).
    /// Returns `(offset, scale)` — customData is parsed but discarded at this level.
    fn parse_layer_offset_params_with_custom_data(&mut self) -> ParseResult<(f64, f64)> {
        let mut offset = 0.0f64;
        let mut scale = 1.0f64;

        if !self.parser.check(&TokenKind::LeftParen) {
            return Ok((offset, scale));
        }
        self.parser.advance(); // consume '('

        while !self.parser.check(&TokenKind::RightParen) && !self.parser.is_at_end() {
            let key_token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;

            match &key_token.kind {
                TokenKind::Keyword(Keyword::Offset) => {
                    self.parser.expect(&TokenKind::Equals)?;
                    let val = self.parser.parse_number()?;
                    offset = val.as_f64().unwrap_or(0.0);
                }
                TokenKind::Keyword(Keyword::Scale) => {
                    self.parser.expect(&TokenKind::Equals)?;
                    let val = self.parser.parse_number()?;
                    scale = val.as_f64().unwrap_or(1.0);
                }
                TokenKind::Keyword(Keyword::CustomData) => {
                    // Parse and discard customData dict at this layer
                    self.parser.expect(&TokenKind::Equals)?;
                    self.parser.parse_dictionary_value()?;
                }
                other => {
                    return Err(self.parser.error(ParseErrorKind::UnexpectedToken(format!(
                        "expected 'offset', 'scale', or 'customData', found {}",
                        other
                    ))));
                }
            }

            self.parser.match_kind(&TokenKind::Semicolon);
        }

        self.parser.expect(&TokenKind::RightParen)?;
        Ok((offset, scale))
    }

    /// Parses a references value.
    ///
    /// Grammar:
    /// ```text
    /// references = None | ReferenceItem | '[' (ReferenceItem (',' ReferenceItem)*)? ']'
    /// ```
    fn parse_references_value(&mut self) -> ParseResult<Value> {
        // None keyword = empty list
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Value::ReferenceList(vec![]));
        }

        // Single item (no brackets)
        if matches!(
            self.parser.peek_kind(),
            Some(TokenKind::AssetRef(_)) | Some(TokenKind::PathRef(_))
        ) {
            let item = self.parse_reference_item()?;
            return Ok(Value::ReferenceList(vec![item]));
        }

        // Bracketed list
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut items = Vec::new();
        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            // Allow trailing commas
            if matches!(self.parser.peek_kind(), Some(TokenKind::RightBracket)) {
                break;
            }
            items.push(self.parse_reference_item()?);
            self.parser.match_kind(&TokenKind::Comma);
        }
        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Value::ReferenceList(items))
    }

    /// Parses a `subLayers` value.
    ///
    /// Grammar:
    /// ```text
    /// SublayerItem = AssetRef (LayerOffsetList)?
    /// subLayers = '[' (SublayerItem (',' SublayerItem)*)? ']'
    /// ```
    /// Returns `Value::SubLayerList` where each entry is `(asset_path, offset, scale)`.
    fn parse_sublayers_value(&mut self) -> ParseResult<Value> {
        // None keyword = empty list
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Value::SubLayerList(vec![]));
        }

        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut items: Vec<(String, f64, f64)> = Vec::new();

        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            // Consume asset ref @path@
            let tok = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
            let asset_path = match tok.kind {
                TokenKind::AssetRef(p) => p,
                _ => {
                    return Err(self.parser.error(ParseErrorKind::UnexpectedToken(
                        "expected asset ref @...@ in subLayers".to_string(),
                    )));
                }
            };

            // Optional (offset = N; scale = N) block
            let (offset, scale) = self.parse_layer_offset_params()?;
            items.push((asset_path, offset, scale));

            // Allow trailing comma
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Value::SubLayerList(items))
    }

    /// Parses a payload value.
    ///
    /// Identical grammar to references but without customData inside layer offset.
    fn parse_payload_value(&mut self) -> ParseResult<Value> {
        // None keyword = empty list
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Value::PayloadList(vec![]));
        }

        // Single item (no brackets)
        if matches!(
            self.parser.peek_kind(),
            Some(TokenKind::AssetRef(_)) | Some(TokenKind::PathRef(_))
        ) {
            let item = self.parse_payload_item()?;
            return Ok(Value::PayloadList(vec![item]));
        }

        // Bracketed list
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut items = Vec::new();
        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            if matches!(self.parser.peek_kind(), Some(TokenKind::RightBracket)) {
                break;
            }
            items.push(self.parse_payload_item()?);
            self.parser.match_kind(&TokenKind::Comma);
        }
        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Value::PayloadList(items))
    }

    /// Parses a single payload item (same as reference but uses plain layer offset).
    fn parse_payload_item(&mut self) -> ParseResult<(String, String, f64, f64)> {
        let mut asset_path = String::new();
        let mut prim_path = String::new();

        match self.parser.peek_kind() {
            Some(TokenKind::AssetRef(_)) => {
                let tok = self
                    .parser
                    .advance()
                    .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
                if let TokenKind::AssetRef(p) = tok.kind {
                    asset_path = p;
                }
                if let Some(TokenKind::PathRef(_)) = self.parser.peek_kind() {
                    let tok = self
                        .parser
                        .advance()
                        .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
                    if let TokenKind::PathRef(p) = tok.kind {
                        prim_path = p;
                    }
                }
            }
            Some(TokenKind::PathRef(_)) => {
                let tok = self
                    .parser
                    .advance()
                    .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
                if let TokenKind::PathRef(p) = tok.kind {
                    prim_path = p;
                }
            }
            _ => {
                return Err(self.parser.error(ParseErrorKind::UnexpectedToken(
                    "expected asset ref @...@ or path ref <...>".to_string(),
                )));
            }
        }

        let (offset, scale) = self.parse_layer_offset_params()?;
        Ok((asset_path, prim_path, offset, scale))
    }

    /// Parses a path list value for inherits/specializes.
    ///
    /// Grammar:
    /// ```text
    /// PathList = None | PathRef | '[' (PathRef (',' PathRef)*)? ']'
    /// ```
    fn parse_path_list_value(&mut self) -> ParseResult<Value> {
        // None keyword = empty list
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Value::PathList(vec![]));
        }

        // Single path (no brackets)
        if let Some(TokenKind::PathRef(_)) = self.parser.peek_kind() {
            let tok = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
            if let TokenKind::PathRef(p) = tok.kind {
                return Ok(Value::PathList(vec![p]));
            }
        }

        // Bracketed list
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut paths = Vec::new();
        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            if matches!(self.parser.peek_kind(), Some(TokenKind::RightBracket)) {
                break;
            }
            let tok = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;
            match tok.kind {
                TokenKind::PathRef(p) => paths.push(p),
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            }
            self.parser.match_kind(&TokenKind::Comma);
        }
        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Value::PathList(paths))
    }

    /// Parses a relocates value: `{ </Old>: </New>, ... }`.
    ///
    /// Grammar:
    /// ```text
    /// Relocates = '{' (PathRef ':' PathRef (',' PathRef ':' PathRef)*)? '}'
    /// ```
    fn parse_relocates_value(&mut self) -> ParseResult<Value> {
        self.parser.expect(&TokenKind::LeftBrace)?;
        let mut pairs: Vec<(String, String)> = Vec::new();

        while !self.parser.check(&TokenKind::RightBrace) && !self.parser.is_at_end() {
            // Source path
            let src_tok = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;
            let src = match src_tok.kind {
                TokenKind::PathRef(p) => p,
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            };

            self.parser.expect(&TokenKind::Colon)?;

            // Destination path
            let dst_tok = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;
            let dst = match dst_tok.kind {
                TokenKind::PathRef(p) => p,
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            };

            pairs.push((src, dst));
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBrace)?;
        Ok(Value::RelocatesMap(pairs))
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_metadata() {
        let mut parser = MetadataParser::new("()");
        let result = parser.parse_metadata_block().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_doc_metadata() {
        let mut parser = MetadataParser::new(r#"(doc = "This is documentation")"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.doc(), Some("This is documentation"));
    }

    #[test]
    fn test_parse_key_value_metadata() {
        let mut parser = MetadataParser::new(r#"(kind = "component")"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result.kind(), Some("component"));
    }

    #[test]
    fn test_parse_custom_data() {
        let mut parser = MetadataParser::new(r#"(customData = { int value = 42 })"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        assert!(result.custom_data().is_some());
    }

    #[test]
    fn test_parse_list_op_metadata() {
        let mut parser = MetadataParser::new(r#"(prepend apiSchemas = ["GeomModelAPI"])"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::ListOp { op, key, value } => {
                assert_eq!(*op, ArrayEditOp::Prepend);
                assert_eq!(key, "apiSchemas");
                assert!(matches!(value, Value::List(_)));
            }
            _ => panic!("expected list op"),
        }
    }

    #[test]
    fn test_parse_permission_metadata() {
        let mut parser = MetadataParser::new("(permission = public)");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::Permission(p) => assert_eq!(p, "public"),
            _ => panic!("expected permission"),
        }
    }

    #[test]
    fn test_parse_multiple_entries() {
        let mut parser = MetadataParser::new(
            r#"(
            doc = "A test prim"
            kind = "component"
            customData = {
                string author = "test"
            }
        )"#,
        );
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result.doc(), Some("A test prim"));
        assert_eq!(result.kind(), Some("component"));
        assert!(result.custom_data().is_some());
    }

    #[test]
    fn test_parse_bare_string_comment() {
        let mut parser = MetadataParser::new(r#"("This is a comment")"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::Comment(s) => assert_eq!(s, "This is a comment"),
            _ => panic!("expected comment"),
        }
    }

    #[test]
    fn test_parse_none_value() {
        let mut parser = MetadataParser::new("(active = None)");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "active");
                assert!(matches!(value, Value::None));
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_add_list_op() {
        let mut parser = MetadataParser::new(r#"(add references = @./other.usda@)"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::ListOp { op, key, .. } => {
                assert_eq!(*op, ArrayEditOp::Add);
                assert_eq!(key, "references");
            }
            _ => panic!("expected list op"),
        }
    }

    #[test]
    fn test_parse_entries_with_semicolons() {
        let mut parser = MetadataParser::new(r#"(kind = "model"; active = true)"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_numeric_value() {
        let mut parser = MetadataParser::new("(instanceable = 1)");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "instanceable");
                assert!(matches!(value, Value::Int64(1)));
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_tuple_value() {
        let mut parser = MetadataParser::new("(extent = [(-1, -1, -1), (1, 1, 1)])");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "extent");
                assert!(matches!(value, Value::List(_)));
            }
            _ => panic!("expected key-value"),
        }
    }

    // ========================================================================
    // variantSets metadata tests
    // ========================================================================

    #[test]
    fn test_parse_variant_sets_single_string() {
        // C++: variantSets = "shapeVariant" (NameList = String)
        let mut parser = MetadataParser::new(r#"(variantSets = "shapeVariant")"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "variantSets");
                // Should be List([String("shapeVariant")])
                if let Value::List(items) = value {
                    assert_eq!(items.len(), 1);
                    assert!(matches!(&items[0], Value::String(s) if s == "shapeVariant"));
                } else {
                    panic!("expected Value::List, got {:?}", value);
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_variant_sets_array() {
        // C++: variantSets = ["shape", "color"] (NameList = '[' String* ']')
        let mut parser = MetadataParser::new(r#"(variantSets = ["shape", "color"])"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "variantSets");
                if let Value::List(items) = value {
                    assert_eq!(items.len(), 2);
                    assert!(matches!(&items[0], Value::String(s) if s == "shape"));
                    assert!(matches!(&items[1], Value::String(s) if s == "color"));
                } else {
                    panic!("expected Value::List, got {:?}", value);
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_add_variant_sets_single() {
        // C++: add variantSets = "shapeVariant"
        let mut parser = MetadataParser::new(r#"(add variantSets = "shapeVariant")"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::ListOp { op, key, value } => {
                assert_eq!(*op, ArrayEditOp::Add);
                assert_eq!(key, "variantSets");
                if let Value::List(items) = value {
                    assert_eq!(items.len(), 1);
                    assert!(matches!(&items[0], Value::String(s) if s == "shapeVariant"));
                } else {
                    panic!("expected Value::List, got {:?}", value);
                }
            }
            _ => panic!("expected list op"),
        }
    }

    #[test]
    fn test_parse_prepend_variant_sets_array() {
        // C++: prepend variantSets = ["a", "b"]
        let mut parser = MetadataParser::new(r#"(prepend variantSets = ["a", "b"])"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::ListOp { op, key, value } => {
                assert_eq!(*op, ArrayEditOp::Prepend);
                assert_eq!(key, "variantSets");
                if let Value::List(items) = value {
                    assert_eq!(items.len(), 2);
                } else {
                    panic!("expected Value::List");
                }
            }
            _ => panic!("expected list op"),
        }
    }

    #[test]
    fn test_parse_variant_sets_none() {
        // C++: variantSets = None
        let mut parser = MetadataParser::new("(variantSets = None)");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "variantSets");
                assert!(matches!(value, Value::None));
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_variant_sets_with_other_metadata() {
        // Combined metadata like real USD files
        let mut parser = MetadataParser::new(
            r#"(
            add variantSets = "shapeVariant"
            kind = "component"
        )"#,
        );
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 2);
        match &result.entries[0] {
            MetadataEntry::ListOp { key, .. } => assert_eq!(key, "variantSets"),
            _ => panic!("expected list op for variantSets"),
        }
        assert_eq!(result.kind(), Some("component"));
    }

    // ========================================================================
    // Composition arc tests
    // ========================================================================

    #[test]
    fn test_parse_references_single_asset() {
        let mut parser = MetadataParser::new("(references = @./other.usda@)");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "references");
                if let Value::ReferenceList(refs) = value {
                    assert_eq!(refs.len(), 1);
                    assert_eq!(refs[0].0, "./other.usda");
                    assert_eq!(refs[0].1, ""); // no prim path
                    assert_eq!(refs[0].2, 0.0); // offset
                    assert_eq!(refs[0].3, 1.0); // scale
                } else {
                    panic!("expected ReferenceList, got {:?}", value);
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_references_asset_with_path() {
        let mut parser = MetadataParser::new("(references = @./other.usda@</Prim>)");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "references");
                if let Value::ReferenceList(refs) = value {
                    assert_eq!(refs.len(), 1);
                    assert_eq!(refs[0].0, "./other.usda");
                    assert_eq!(refs[0].1, "/Prim");
                } else {
                    panic!("expected ReferenceList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_references_with_layer_offset() {
        let mut parser =
            MetadataParser::new("(references = @./other.usda@</Prim> (offset = 10; scale = 2))");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "references");
                if let Value::ReferenceList(refs) = value {
                    assert_eq!(refs.len(), 1);
                    assert_eq!(refs[0].0, "./other.usda");
                    assert_eq!(refs[0].1, "/Prim");
                    assert_eq!(refs[0].2, 10.0);
                    assert_eq!(refs[0].3, 2.0);
                } else {
                    panic!("expected ReferenceList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_references_list() {
        let mut parser = MetadataParser::new("(references = [@a.usda@, @b.usda@</P>])");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "references");
                if let Value::ReferenceList(refs) = value {
                    assert_eq!(refs.len(), 2);
                    assert_eq!(refs[0].0, "a.usda");
                    assert_eq!(refs[0].1, "");
                    assert_eq!(refs[1].0, "b.usda");
                    assert_eq!(refs[1].1, "/P");
                } else {
                    panic!("expected ReferenceList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_references_none() {
        let mut parser = MetadataParser::new("(references = None)");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "references");
                assert!(matches!(value, Value::ReferenceList(v) if v.is_empty()));
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_payload_single() {
        let mut parser = MetadataParser::new("(payload = @model.usda@</Model>)");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "payload");
                if let Value::PayloadList(payloads) = value {
                    assert_eq!(payloads.len(), 1);
                    assert_eq!(payloads[0].0, "model.usda");
                    assert_eq!(payloads[0].1, "/Model");
                } else {
                    panic!("expected PayloadList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_payload_with_offset() {
        let mut parser = MetadataParser::new("(payload = @model.usda@ (offset = 5))");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "payload");
                if let Value::PayloadList(payloads) = value {
                    assert_eq!(payloads.len(), 1);
                    assert_eq!(payloads[0].2, 5.0);
                    assert_eq!(payloads[0].3, 1.0); // scale defaults to 1
                } else {
                    panic!("expected PayloadList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_inherits_single() {
        let mut parser = MetadataParser::new("(inherits = </Base>)");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "inherits");
                if let Value::PathList(paths) = value {
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0], "/Base");
                } else {
                    panic!("expected PathList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_inherits_list() {
        let mut parser = MetadataParser::new("(inherits = [</A>, </B>])");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "inherits");
                if let Value::PathList(paths) = value {
                    assert_eq!(paths.len(), 2);
                    assert_eq!(paths[0], "/A");
                    assert_eq!(paths[1], "/B");
                } else {
                    panic!("expected PathList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_specializes_single() {
        let mut parser = MetadataParser::new("(specializes = </Spec>)");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "specializes");
                if let Value::PathList(paths) = value {
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0], "/Spec");
                } else {
                    panic!("expected PathList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_relocates() {
        let mut parser = MetadataParser::new("(relocates = { </Old>: </New> })");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "relocates");
                if let Value::RelocatesMap(pairs) = value {
                    assert_eq!(pairs.len(), 1);
                    assert_eq!(pairs[0].0, "/Old");
                    assert_eq!(pairs[0].1, "/New");
                } else {
                    panic!("expected RelocatesMap");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_relocates_multiple() {
        let mut parser = MetadataParser::new("(relocates = { </A>: </B>, </C>: </D> })");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "relocates");
                if let Value::RelocatesMap(pairs) = value {
                    assert_eq!(pairs.len(), 2);
                    assert_eq!(pairs[0].0, "/A");
                    assert_eq!(pairs[0].1, "/B");
                    assert_eq!(pairs[1].0, "/C");
                    assert_eq!(pairs[1].1, "/D");
                } else {
                    panic!("expected RelocatesMap");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_prepend_references() {
        let mut parser = MetadataParser::new("(prepend references = @layer.usda@)");
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::ListOp { op, key, value } => {
                assert_eq!(*op, ArrayEditOp::Prepend);
                assert_eq!(key, "references");
                if let Value::ReferenceList(refs) = value {
                    assert_eq!(refs.len(), 1);
                    assert_eq!(refs[0].0, "layer.usda");
                } else {
                    panic!("expected ReferenceList");
                }
            }
            _ => panic!("expected list op"),
        }
    }

    #[test]
    fn test_parse_add_inherits() {
        let mut parser = MetadataParser::new("(add inherits = [</NewBase>])");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::ListOp { op, key, value } => {
                assert_eq!(*op, ArrayEditOp::Add);
                assert_eq!(key, "inherits");
                if let Value::PathList(paths) = value {
                    assert_eq!(paths.len(), 1);
                    assert_eq!(paths[0], "/NewBase");
                } else {
                    panic!("expected PathList");
                }
            }
            _ => panic!("expected list op"),
        }
    }

    #[test]
    fn test_parse_internal_reference() {
        let mut parser = MetadataParser::new("(references = </InternalPrim>)");
        let result = parser.parse_metadata_block().unwrap();
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "references");
                if let Value::ReferenceList(refs) = value {
                    assert_eq!(refs.len(), 1);
                    assert_eq!(refs[0].0, ""); // no asset path for internal refs
                    assert_eq!(refs[0].1, "/InternalPrim");
                } else {
                    panic!("expected ReferenceList");
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    // ========================================================================
    // prefix/suffix substitution tests
    // ========================================================================

    #[test]
    fn test_parse_prefix_suffix_string_metadata() {
        // prefix and suffix are plain identifiers — parsed as generic key-value
        let mut parser = MetadataParser::new(r#"(prefix = "$Left" suffix = "_$NUM")"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 2);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "prefix");
                assert!(matches!(value, Value::String(s) if s == "$Left"));
            }
            _ => panic!("expected key-value for prefix"),
        }
        match &result.entries[1] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "suffix");
                assert!(matches!(value, Value::String(s) if s == "_$NUM"));
            }
            _ => panic!("expected key-value for suffix"),
        }
    }

    #[test]
    fn test_parse_prefix_substitutions() {
        // prefixSubstitutions = { "$Left": "Right", "Left": "Right" }
        let mut parser =
            MetadataParser::new(r#"(prefixSubstitutions = { "$Left": "Right", "Left": "Right" })"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "prefixSubstitutions");
                if let Value::Dictionary(entries) = value {
                    assert_eq!(entries.len(), 2);
                    assert_eq!(entries[0].1, "$Left");
                    assert!(matches!(&entries[0].2, Value::String(s) if s == "Right"));
                    assert_eq!(entries[1].1, "Left");
                    assert!(matches!(&entries[1].2, Value::String(s) if s == "Right"));
                } else {
                    panic!("expected Dictionary, got {:?}", value);
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_suffix_substitutions_trailing_comma() {
        // suffixSubstitutions = { "$NUM": "1", } — trailing comma
        let mut parser = MetadataParser::new(r#"(suffixSubstitutions = { "$NUM": "1", })"#);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 1);
        match &result.entries[0] {
            MetadataEntry::KeyValue { key, value } => {
                assert_eq!(key, "suffixSubstitutions");
                if let Value::Dictionary(entries) = value {
                    assert_eq!(entries.len(), 1);
                    assert_eq!(entries[0].1, "$NUM");
                    assert!(matches!(&entries[0].2, Value::String(s) if s == "1"));
                } else {
                    panic!("expected Dictionary, got {:?}", value);
                }
            }
            _ => panic!("expected key-value"),
        }
    }

    #[test]
    fn test_parse_displayname_metadata_block() {
        // Full prim metadata block from 113_displayName_metadata.usda
        let src = r#"(
    inherits = </Rig>
    prefixSubstitutions = {
        "$Left": "Right",
        "Left": "Right"
    }
    suffixSubstitutions = {
        "$NUM": "1",
    }
)"#;
        let mut parser = MetadataParser::new(src);
        let result = parser.parse_metadata_block().unwrap();
        assert_eq!(result.len(), 3);
    }
}
