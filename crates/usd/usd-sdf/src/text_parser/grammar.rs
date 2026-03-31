//! Grammar parsing for USDA format.
//!
//! This module implements the top-level grammar for USD text files:
//! - Layer header: `#usda 1.0`
//! - Layer metadata: `( ... )`
//! - Root prims: `def Xform "World" { ... }`
//!
//! # C++ Parity
//!
//! Matches the layer rules from `textFileFormatParser.h`:
//! ```text
//! LayerHeader = '#' until end of line                      // lines 1625-1627
//! LayerMetadata = '(' LayerMetadataItem* ')'               // lines 1619-1623
//! LayerSpec = LayerHeader LayerMetadata? PrimSpec*         // lines 1634-1654
//! LayerMetadataOnly = LayerHeader LayerMetadata?           // lines 1656-1661
//! ```
//!
//! # File Structure
//!
//! ```text
//! #usda 1.0
//! (
//!     defaultPrim = "World"
//!     metersPerUnit = 0.01
//!     upAxis = "Y"
//! )
//!
//! def Xform "World" {
//!     def Mesh "Cube" {
//!         ...
//!     }
//! }
//! ```

use crate::text_parser::error::{ParseErrorKind, ParseResult};
use crate::text_parser::lexer::Lexer;
use crate::text_parser::metadata::{Metadata, MetadataEntry};
use crate::text_parser::specs::{ParsedPrimItem, ParsedPrimWithContents, SpecsParser};
use crate::text_parser::tokens::{Keyword, TokenKind};
use crate::text_parser::value_context::{ArrayEditOp, Value};
use crate::text_parser::values::ValueParser;

// ============================================================================
// Layer Header
// ============================================================================

/// Parsed layer header information.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerHeader {
    /// The format identifier ("usda" or "sdf").
    pub format: String,
    /// The version string (e.g., "1.0").
    pub version: String,
}

impl Default for LayerHeader {
    fn default() -> Self {
        Self {
            format: "usda".to_string(),
            version: "1.0".to_string(),
        }
    }
}

// ============================================================================
// Parsed Layer
// ============================================================================

/// A fully parsed layer from USD text format.
#[derive(Debug, Clone, Default)]
pub struct ParsedLayer {
    /// Layer header information.
    pub header: LayerHeader,
    /// Layer-level metadata.
    pub metadata: Option<Metadata>,
    /// Root prim ordering (if reorder rootPrims specified).
    pub root_prim_order: Option<Vec<String>>,
    /// Root prim specs.
    pub prims: Vec<ParsedPrimWithContents>,
}

impl ParsedLayer {
    /// Returns the default prim name if specified in metadata.
    pub fn default_prim(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("defaultPrim")?.as_string()
    }

    /// Returns the meters per unit if specified.
    pub fn meters_per_unit(&self) -> Option<f64> {
        self.metadata.as_ref()?.get("metersPerUnit")?.as_f64()
    }

    /// Returns the up axis if specified.
    pub fn up_axis(&self) -> Option<&str> {
        self.metadata.as_ref()?.get("upAxis")?.as_string()
    }

    /// Returns sublayer paths if specified.
    ///
    /// Sublayers can be specified as:
    /// - `subLayers = [@./layer1.usda@, @./layer2.usda@]`
    /// - `prepend subLayers = [@./layer.usda@]`
    /// - `append subLayers = [@./layer.usda@]`
    pub fn sublayers(&self) -> Option<Vec<String>> {
        let meta = self.metadata.as_ref()?;

        // Collect all sublayers from both KeyValue and ListOp entries
        let mut result = Vec::new();

        for entry in &meta.entries {
            match entry {
                MetadataEntry::KeyValue { key, value } if key == "subLayers" => {
                    Self::collect_asset_paths(value, &mut result);
                }
                MetadataEntry::ListOp { key, value, .. } if key == "subLayers" => {
                    Self::collect_asset_paths(value, &mut result);
                }
                _ => {}
            }
        }

        if result.is_empty() {
            None
        } else {
            Some(result)
        }
    }

    /// Helper to extract asset path strings from a Value.
    fn collect_asset_paths(value: &Value, result: &mut Vec<String>) {
        match value {
            Value::AssetPath(path) => {
                result.push(path.clone());
            }
            Value::SubLayerList(items) => {
                // Each item is (path, offset, scale) — extract just the path
                for (path, _, _) in items {
                    result.push(path.clone());
                }
            }
            Value::List(items) => {
                for item in items {
                    Self::collect_asset_paths(item, result);
                }
            }
            Value::String(s) if !s.is_empty() => {
                // Sometimes asset paths may be parsed as strings
                result.push(s.clone());
            }
            _ => {}
        }
    }
}

// ============================================================================
// Layer Parser
// ============================================================================

/// A parser for complete USD text layers.
pub struct LayerParser<'a> {
    /// The underlying value parser.
    parser: ValueParser<'a>,
}

impl<'a> LayerParser<'a> {
    /// Creates a new layer parser.
    pub fn new(source: &'a str) -> Self {
        Self {
            parser: ValueParser::new(source),
        }
    }

    /// Creates a layer parser with a file path for error reporting.
    pub fn with_file_path(source: &'a str, path: impl Into<String>) -> Self {
        Self {
            parser: ValueParser::from_lexer(Lexer::new(source).with_file_path(path)),
        }
    }

    /// Parses the complete layer.
    ///
    /// # Grammar
    ///
    /// ```text
    /// LayerSpec = LayerHeader LayerMetadata? (reorder rootPrims = NameList)?
    ///             PrimSpec*
    /// ```
    pub fn parse(&mut self) -> ParseResult<ParsedLayer> {
        // Parse header
        let header = self.parse_header()?;

        // Parse optional metadata
        let metadata = self.parse_layer_metadata()?;

        // Parse optional root prim ordering
        let root_prim_order = self.parse_root_prim_order()?;

        // Parse root prims
        let mut prims = Vec::new();
        while !self.parser.is_at_end() {
            // Check if we have a prim specifier
            match self.parser.peek_kind() {
                Some(TokenKind::Keyword(Keyword::Def))
                | Some(TokenKind::Keyword(Keyword::Over))
                | Some(TokenKind::Keyword(Keyword::Class)) => {
                    let prim = self.parse_prim_with_contents()?;
                    prims.push(prim);
                }
                Some(TokenKind::Eof) | None => break,
                _ => {
                    // Skip unexpected tokens (shouldn't happen in valid files)
                    self.parser.advance();
                }
            }
        }

        Ok(ParsedLayer {
            header,
            metadata,
            root_prim_order,
            prims,
        })
    }

    /// Parses only the layer header and metadata (fast path).
    ///
    /// This is useful for sniffing layer information without parsing
    /// the full prim hierarchy.
    pub fn parse_metadata_only(&mut self) -> ParseResult<(LayerHeader, Option<Metadata>)> {
        let header = self.parse_header()?;
        let metadata = self.parse_layer_metadata()?;
        Ok((header, metadata))
    }

    /// Parses the layer header: `#usda 1.0` or `#sdf 1.4.32`.
    fn parse_header(&mut self) -> ParseResult<LayerHeader> {
        // Expect magic identifier token
        match self.parser.peek_kind() {
            Some(TokenKind::Magic { .. }) => {
                let token = self.parser.advance().expect("token after peek");
                match token.kind {
                    TokenKind::Magic { format, version } => Ok(LayerHeader { format, version }),
                    _ => unreachable!(),
                }
            }
            Some(kind) => Err(self.parser.error(ParseErrorKind::InvalidHeader(format!(
                "expected #usda or #sdf header, found {}",
                kind
            )))),
            None => Err(self.parser.error(ParseErrorKind::InvalidHeader(
                "expected layer header".to_string(),
            ))),
        }
    }

    /// Parses optional layer metadata.
    fn parse_layer_metadata(&mut self) -> ParseResult<Option<Metadata>> {
        // Check for opening paren
        if !matches!(self.parser.peek_kind(), Some(TokenKind::LeftParen)) {
            return Ok(None);
        }

        // Parse metadata block directly using ValueParser's methods
        // Expect opening paren
        self.parser.expect(&TokenKind::LeftParen)?;

        let mut metadata = Metadata::new();

        // Parse entries until closing paren
        while !self.parser.check(&TokenKind::RightParen) && !self.parser.is_at_end() {
            let entry = self.parse_metadata_entry()?;
            metadata.entries.push(entry);

            // Entries can be separated by semicolons or newlines
            self.parser.match_kind(&TokenKind::Semicolon);
        }

        // Expect closing paren
        self.parser.expect(&TokenKind::RightParen)?;

        Ok(Some(metadata))
    }

    /// Parses a single metadata entry.
    fn parse_metadata_entry(&mut self) -> ParseResult<MetadataEntry> {
        // Check for list operation keywords first
        if self.is_list_op_start() {
            return self.parse_list_op_metadata();
        }

        match self.parser.peek_kind().cloned() {
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

            // relocates = { <src>: <dst>, ... }  -- must use path-aware parser
            Some(TokenKind::Keyword(Keyword::Relocates)) => {
                self.parser.advance();
                self.parser.expect(&TokenKind::Equals)?;
                let value = self.parse_relocates_value()?;
                Ok(MetadataEntry::KeyValue {
                    key: "relocates".to_string(),
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

    /// Parses a relocates value: `{ <src>: <dst>, ... }`.
    ///
    /// Keys and values are `PathRef` tokens (including relative and empty).
    /// Mirrors `MetadataParser::parse_relocates_value` but lives in
    /// `GrammarParser` so it is available during layer-level metadata parsing.
    fn parse_relocates_value(&mut self) -> ParseResult<Value> {
        self.parser.expect(&TokenKind::LeftBrace)?;
        let mut pairs: Vec<(String, String)> = Vec::new();

        while !self.parser.check(&TokenKind::RightBrace) && !self.parser.is_at_end() {
            let src_tok = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedPathRef))?;
            let src = match src_tok.kind {
                TokenKind::PathRef(p) => p,
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedPathRef)),
            };

            self.parser.expect(&TokenKind::Colon)?;

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

        // Value can be None, or a subLayers list, or a generic list
        let value = if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            Value::None
        } else if key == "subLayers" {
            self.parse_sublayers_value()?
        } else {
            self.parser.parse_value()?
        };

        Ok(MetadataEntry::ListOp { op, key, value })
    }

    /// Parses layer offset params: `(offset = N; scale = N)`.
    ///
    /// Both fields are optional; defaults are offset=0.0, scale=1.0.
    fn parse_layer_offset_params(&mut self) -> ParseResult<(f64, f64)> {
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
                other => {
                    return Err(self.parser.error(ParseErrorKind::UnexpectedToken(format!(
                        "expected 'offset' or 'scale', found {}",
                        other
                    ))));
                }
            }

            self.parser.match_kind(&TokenKind::Semicolon);
        }

        self.parser.expect(&TokenKind::RightParen)?;
        Ok((offset, scale))
    }

    /// Parses a `subLayers` value: `[ @asset@ (offset=N; scale=N), ... ]`.
    ///
    /// Returns `Value::SubLayerList` with `(asset_path, offset, scale)` per entry.
    fn parse_sublayers_value(&mut self) -> ParseResult<Value> {
        if self.parser.check_keyword(Keyword::None)
            || self.parser.check_keyword(Keyword::NoneLowercase)
        {
            self.parser.advance();
            return Ok(Value::SubLayerList(vec![]));
        }

        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut items: Vec<(String, f64, f64)> = Vec::new();

        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
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

            let (offset, scale) = self.parse_layer_offset_params()?;
            items.push((asset_path, offset, scale));
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Value::SubLayerList(items))
    }

    // ========================================================================
    // Root Prim Ordering
    // ========================================================================

    /// Parses optional root prim ordering: `reorder rootPrims = [...]`.
    fn parse_root_prim_order(&mut self) -> ParseResult<Option<Vec<String>>> {
        // Check for reorder keyword
        if !self.parser.check_keyword(Keyword::Reorder) {
            return Ok(None);
        }

        self.parser.advance();
        self.parser.expect_keyword(Keyword::RootPrims)?;
        self.parser.expect(&TokenKind::Equals)?;

        // Parse list of names
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut names = Vec::new();

        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;

            match token.kind {
                TokenKind::String(s) => names.push(s),
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedString)),
            }

            // Optional comma
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(Some(names))
    }

    // ========================================================================
    // Prim Parsing
    // ========================================================================

    /// Parses a prim with full contents: `def Type "Name" (metadata) { ... }`.
    ///
    /// # Grammar
    ///
    /// ```text
    /// PrimSpec = Specifier TypeName? String MetadataBlock? '{' PrimItem* '}'
    /// PrimItem = PropertySpec | PrimSpec | VariantSet | OrderingStatement
    /// ```
    pub fn parse_prim_with_contents(&mut self) -> ParseResult<ParsedPrimWithContents> {
        usd_trace::trace_scope!("usda_parse_prim");
        // Parse prim header
        let header = self.parse_prim_spec()?;

        // Parse contents if braces present
        let items = if self.parser.check(&TokenKind::LeftBrace) {
            self.parser.advance();
            let items = self.parse_prim_items()?;
            self.parser.expect(&TokenKind::RightBrace)?;
            items
        } else {
            Vec::new()
        };

        Ok(ParsedPrimWithContents { header, items })
    }

    /// Parses a prim spec header (without contents).
    fn parse_prim_spec(&mut self) -> ParseResult<crate::text_parser::specs::ParsedPrimSpec> {
        use crate::text_parser::specs::{ParsedPrimSpec, Specifier};

        // Parse specifier
        let specifier = match self.parser.peek_kind() {
            Some(TokenKind::Keyword(Keyword::Def)) => {
                self.parser.advance();
                Specifier::Def
            }
            Some(TokenKind::Keyword(Keyword::Over)) => {
                self.parser.advance();
                Specifier::Over
            }
            Some(TokenKind::Keyword(Keyword::Class)) => {
                self.parser.advance();
                Specifier::Class
            }
            _ => {
                return Err(self.parser.error(ParseErrorKind::ExpectedKeyword(
                    "def, over, or class".to_string(),
                )));
            }
        };

        // Parse optional type name
        // Type name can be a simple identifier or dotted (e.g., "UsdGeom.Mesh")
        // But we need to distinguish it from the prim name (which is a quoted string)
        let type_name = if let Some(TokenKind::Identifier(_)) = self.parser.peek_kind() {
            // Look ahead to see if next token is a string (prim name) or a dot/identifier
            let ident = self.parser.parse_identifier()?;

            // Check for dotted type name (e.g., UsdGeom.Mesh)
            let mut full_type = ident.clone();
            while self.parser.check(&TokenKind::Dot) {
                // Make sure next is an identifier, not something else
                self.parser.advance();
                if let Some(TokenKind::Identifier(_)) = self.parser.peek_kind() {
                    let next = self.parser.parse_identifier()?;
                    full_type.push('.');
                    full_type.push_str(&next);
                } else {
                    // Put the dot back somehow... actually we can't, so this is an error
                    return Err(self.parser.error(ParseErrorKind::ExpectedIdentifier));
                }
            }

            // Check if this could actually be the prim name without a type
            // If the next token is a string, then ident is the type name
            // If the next token is not a string, then ident might be the prim name
            // But prim names are always strings, so if we have an identifier,
            // it must be the type name
            Some(full_type)
        } else {
            None
        };

        // Parse prim name (quoted string)
        let name_token = self
            .parser
            .advance()
            .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;

        let name = match name_token.kind {
            TokenKind::String(s) => s,
            _ => return Err(self.parser.error(ParseErrorKind::ExpectedString)),
        };

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

    /// Parses a metadata block (opening and closing parens).
    fn parse_metadata_block(&mut self) -> ParseResult<Metadata> {
        use crate::text_parser::metadata::MetadataParser;

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

    /// Parses prim items (properties, children, variants, ordering).
    fn parse_prim_items(&mut self) -> ParseResult<Vec<ParsedPrimItem>> {
        let mut items = Vec::new();

        while !self.parser.check(&TokenKind::RightBrace) && !self.parser.is_at_end() {
            // Check what kind of item this is
            match self.parser.peek_kind() {
                // Specifier keywords indicate a child prim
                Some(TokenKind::Keyword(Keyword::Def))
                | Some(TokenKind::Keyword(Keyword::Over))
                | Some(TokenKind::Keyword(Keyword::Class)) => {
                    let child_prim = self.parse_prim_with_contents()?;
                    items.push(ParsedPrimItem::Prim(Box::new(child_prim)));
                }

                // Variant set
                Some(TokenKind::Keyword(Keyword::VariantSet)) => {
                    let variant_set = self.parse_variant_set()?;
                    items.push(ParsedPrimItem::VariantSet(variant_set));
                }

                // Ordering statements
                Some(TokenKind::Keyword(Keyword::Reorder)) => {
                    self.parser.advance();

                    match self.parser.peek_kind() {
                        Some(TokenKind::Keyword(Keyword::NameChildren)) => {
                            self.parser.advance();
                            self.parser.expect(&TokenKind::Equals)?;
                            let names = self.parse_name_list()?;
                            items.push(ParsedPrimItem::ChildOrder(names));
                        }
                        Some(TokenKind::Keyword(Keyword::Properties)) => {
                            self.parser.advance();
                            self.parser.expect(&TokenKind::Equals)?;
                            let names = self.parse_name_list()?;
                            items.push(ParsedPrimItem::PropertyOrder(names));
                        }
                        _ => {
                            // `reorder` is also a valid list-op prefix for property specs,
                            // e.g. `reorder double foo.connect = [...]`.
                            // The keyword was already consumed above, so delegate directly.
                            let property = self.parse_property_spec()?;
                            items.push(ParsedPrimItem::PropertyListOp(
                                crate::text_parser::specs::PropertyListOp {
                                    op: ArrayEditOp::Reorder,
                                    property,
                                },
                            ));
                        }
                    }
                }

                // Property specs (attributes or relationships)
                Some(TokenKind::Keyword(Keyword::Custom))
                | Some(TokenKind::Keyword(Keyword::Uniform))
                | Some(TokenKind::Keyword(Keyword::Varying))
                | Some(TokenKind::Keyword(Keyword::Config))
                | Some(TokenKind::Keyword(Keyword::Rel))
                | Some(TokenKind::Identifier(_)) => {
                    let property = self.parse_property_spec()?;
                    items.push(ParsedPrimItem::Property(property));
                }

                // List operations on properties
                Some(TokenKind::Keyword(Keyword::Add))
                | Some(TokenKind::Keyword(Keyword::Delete))
                | Some(TokenKind::Keyword(Keyword::Append))
                | Some(TokenKind::Keyword(Keyword::Prepend)) => {
                    // Parse list operation
                    let op_token = self.parser.advance().expect("token after peek");
                    let op = match &op_token.kind {
                        TokenKind::Keyword(Keyword::Add) => ArrayEditOp::Add,
                        TokenKind::Keyword(Keyword::Delete) => ArrayEditOp::Delete,
                        TokenKind::Keyword(Keyword::Append) => ArrayEditOp::Append,
                        TokenKind::Keyword(Keyword::Prepend) => ArrayEditOp::Prepend,
                        _ => unreachable!(),
                    };

                    let property = self.parse_property_spec()?;
                    items.push(ParsedPrimItem::PropertyListOp(
                        crate::text_parser::specs::PropertyListOp { op, property },
                    ));
                }

                _ => {
                    return Err(self.parser.error(ParseErrorKind::UnexpectedToken(format!(
                        "expected prim item, found {:?}",
                        self.parser.peek_kind()
                    ))));
                }
            }

            // Optional semicolon
            self.parser.match_kind(&TokenKind::Semicolon);
        }

        Ok(items)
    }

    /// Parses a property spec using SpecsParser.
    fn parse_property_spec(
        &mut self,
    ) -> ParseResult<crate::text_parser::specs::ParsedPropertySpec> {
        // Extract full state from current parser
        let (lexer, current) =
            std::mem::replace(&mut self.parser, ValueParser::new("")).into_state();

        // Create specs parser with full state
        let mut specs_parser = SpecsParser::from_state(lexer, current);
        let result = specs_parser.parse_property_spec()?;

        // Restore full state back
        let (lexer, current) = specs_parser.into_state();
        self.parser = ValueParser::from_state(lexer, current);
        Ok(result)
    }

    /// Parses a variant set.
    fn parse_variant_set(&mut self) -> ParseResult<crate::text_parser::specs::ParsedVariantSet> {
        use crate::text_parser::specs::{ParsedVariant, ParsedVariantSet};

        self.parser.expect_keyword(Keyword::VariantSet)?;

        // Parse variant set name (string)
        let name_token = self
            .parser
            .advance()
            .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;

        let name = match name_token.kind {
            TokenKind::String(s) => s,
            _ => return Err(self.parser.error(ParseErrorKind::ExpectedString)),
        };

        // Parse '=' between name and opening brace (C++: variantSet "name" = { ... })
        self.parser.expect(&TokenKind::Equals)?;

        // Parse opening brace
        self.parser.expect(&TokenKind::LeftBrace)?;

        // Parse variants
        let mut variants = Vec::new();
        while !self.parser.check(&TokenKind::RightBrace) && !self.parser.is_at_end() {
            // Parse variant name (string)
            let variant_name_token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;

            let variant_name = match variant_name_token.kind {
                TokenKind::String(s) => s,
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedString)),
            };

            // Parse optional metadata
            let metadata = if self.parser.check(&TokenKind::LeftParen) {
                Some(self.parse_metadata_block()?)
            } else {
                None
            };

            // Parse variant contents
            self.parser.expect(&TokenKind::LeftBrace)?;
            let contents = self.parse_prim_items()?;
            self.parser.expect(&TokenKind::RightBrace)?;

            variants.push(ParsedVariant {
                name: variant_name,
                metadata,
                contents,
            });
        }

        // Parse closing brace
        self.parser.expect(&TokenKind::RightBrace)?;

        Ok(ParsedVariantSet { name, variants })
    }

    /// Parses a NameList: single string `"name"` or array `["name1", "name2", ...]`.
    ///
    /// Matches C++ grammar: `NameList = String | '[' String (',' String)* ']'`
    fn parse_name_list(&mut self) -> ParseResult<Vec<String>> {
        // Single string case: NameList = String
        if let Some(TokenKind::String(_)) = self.parser.peek_kind() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::UnexpectedEof))?;
            if let TokenKind::String(s) = token.kind {
                return Ok(vec![s]);
            }
        }

        // Array case: NameList = '[' String (',' String)* ']'
        self.parser.expect(&TokenKind::LeftBracket)?;
        let mut names = Vec::new();

        while !self.parser.check(&TokenKind::RightBracket) && !self.parser.is_at_end() {
            let token = self
                .parser
                .advance()
                .ok_or_else(|| self.parser.error(ParseErrorKind::ExpectedString))?;

            match token.kind {
                TokenKind::String(s) => names.push(s),
                _ => return Err(self.parser.error(ParseErrorKind::ExpectedString)),
            }

            // Optional comma
            self.parser.match_kind(&TokenKind::Comma);
        }

        self.parser.expect(&TokenKind::RightBracket)?;
        Ok(names)
    }
}

// ============================================================================
// Public API Functions
// ============================================================================

/// Parses a complete layer from USD text format.
///
/// # Arguments
///
/// * `content` - The text content to parse
///
/// # Returns
///
/// A `ParsedLayer` containing all parsed data.
///
/// # Errors
///
/// Returns `ParseError` for invalid content.
pub fn parse_layer_text(content: &str) -> ParseResult<ParsedLayer> {
    usd_trace::trace_scope!("usda_parse_layer_text");
    let mut parser = LayerParser::new(content);
    parser.parse()
}

/// Parses only the layer header and metadata (fast path).
///
/// # Arguments
///
/// * `content` - The text content to parse
///
/// # Returns
///
/// A tuple of (LayerHeader, Option<Metadata>).
pub fn parse_layer_header_and_metadata(
    content: &str,
) -> ParseResult<(LayerHeader, Option<Metadata>)> {
    let mut parser = LayerParser::new(content);
    parser.parse_metadata_only()
}

/// Validates that content is valid USD text format.
///
/// # Arguments
///
/// * `content` - The content to validate
///
/// # Returns
///
/// `Ok(())` if valid, `Err(ParseError)` otherwise.
pub fn validate_layer_text(content: &str) -> ParseResult<()> {
    let mut parser = LayerParser::new(content);
    parser.parse()?;
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_layer() {
        let content = "#usda 1.0\n";
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.header.format, "usda");
        assert_eq!(result.header.version, "1.0");
        assert!(result.metadata.is_none());
        assert!(result.prims.is_empty());
    }

    #[test]
    fn test_parse_layer_with_metadata() {
        let content = r#"#usda 1.0
(
    defaultPrim = "World"
    metersPerUnit = 0.01
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert!(result.metadata.is_some());
        assert_eq!(result.default_prim(), Some("World"));
    }

    #[test]
    fn test_parse_sdf_header() {
        let content = "#sdf 1.4.32\n";
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.header.format, "sdf");
        assert_eq!(result.header.version, "1.4.32");
    }

    #[test]
    fn test_parse_header_and_metadata_only() {
        let content = r#"#usda 1.0
(
    upAxis = "Y"
)
def Xform "World" {
    # lots of content that we skip
}
"#;
        let (header, metadata) = parse_layer_header_and_metadata(content).unwrap();
        assert_eq!(header.format, "usda");
        assert!(metadata.is_some());
    }

    #[test]
    fn test_validate_valid_layer() {
        let content = "#usda 1.0\n";
        assert!(validate_layer_text(content).is_ok());
    }

    #[test]
    fn test_validate_invalid_layer() {
        let content = "not a valid layer";
        assert!(validate_layer_text(content).is_err());
    }

    #[test]
    fn test_layer_header_default() {
        let header = LayerHeader::default();
        assert_eq!(header.format, "usda");
        assert_eq!(header.version, "1.0");
    }

    #[test]
    fn test_parsed_layer_helpers() {
        let content = r#"#usda 1.0
(
    defaultPrim = "Root"
    metersPerUnit = 0.01
    upAxis = "Z"
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.default_prim(), Some("Root"));
        assert_eq!(result.meters_per_unit(), Some(0.01));
        assert_eq!(result.up_axis(), Some("Z"));
    }

    #[test]
    fn test_parse_complex_metadata() {
        let content = r#"#usda 1.0
(
    doc = "A test layer"
    defaultPrim = "Model"
    metersPerUnit = 0.01
    upAxis = "Y"
    startTimeCode = 1
    endTimeCode = 100
    timeCodesPerSecond = 24
    framesPerSecond = 24
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert!(result.metadata.is_some());
        let meta = result.metadata.as_ref().unwrap();
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_parse_metadata_with_custom_data() {
        let content = r#"#usda 1.0
(
    customData = {
        string author = "test"
        int version = 1
    }
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert!(result.metadata.is_some());
    }

    #[test]
    fn test_parse_metadata_with_list_ops() {
        let content = r#"#usda 1.0
(
    prepend subLayers = [
        @./sublayer1.usda@,
        @./sublayer2.usda@
    ]
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert!(result.metadata.is_some());

        // Test sublayers() method
        let sublayers = result.sublayers().expect("should have sublayers");
        assert_eq!(sublayers.len(), 2);
        assert_eq!(sublayers[0], "./sublayer1.usda");
        assert_eq!(sublayers[1], "./sublayer2.usda");
    }

    #[test]
    fn test_sublayers_none() {
        let content = r#"#usda 1.0
(
    defaultPrim = "World"
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert!(result.sublayers().is_none());
    }

    #[test]
    fn test_parse_real_world_layer_header() {
        // Based on simpleShading.usda structure
        let content = r#"#usda 1.0
(
    upAxis = "Y"
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.header.format, "usda");
        assert_eq!(result.header.version, "1.0");
        assert_eq!(result.up_axis(), Some("Y"));
    }

    #[test]
    fn test_parse_empty_metadata_block() {
        let content = "#usda 1.0\n()\n";
        let result = parse_layer_text(content).unwrap();
        assert!(result.metadata.is_some());
        assert!(result.metadata.as_ref().unwrap().is_empty());
    }

    #[test]
    fn test_parse_metadata_with_semicolons() {
        let content = r#"#usda 1.0
(
    defaultPrim = "World"; upAxis = "Z"
)
"#;
        let result = parse_layer_text(content).unwrap();
        assert!(result.metadata.is_some());
        let meta = result.metadata.unwrap();
        assert_eq!(meta.len(), 2);
    }

    #[test]
    fn test_parse_simple_prim() {
        let content = r#"#usda 1.0

def Xform "World"
{
}
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.prims.len(), 1);
        let prim = &result.prims[0];
        assert_eq!(prim.header.name, "World");
        assert_eq!(prim.header.type_name, Some("Xform".to_string()));
        assert!(prim.items.is_empty());
    }

    #[test]
    fn test_parse_prim_with_properties() {
        let content = r#"#usda 1.0

def Xform "World"
{
    float3 xformOp:translate = (0, 0, 0)
    uniform token[] xformOpOrder = ["xformOp:translate"]
}
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.prims.len(), 1);
        let prim = &result.prims[0];
        assert_eq!(prim.header.name, "World");
        assert_eq!(prim.items.len(), 2);
    }

    #[test]
    fn test_parse_nested_prims() {
        let content = r#"#usda 1.0

def Xform "World"
{
    def Mesh "Cube"
    {
        float3[] extent = [(-1, -1, -1), (1, 1, 1)]
    }
}
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.prims.len(), 1);
        let prim = &result.prims[0];
        assert_eq!(prim.header.name, "World");
        assert_eq!(prim.items.len(), 1);

        // Check nested prim
        match &prim.items[0] {
            crate::text_parser::specs::ParsedPrimItem::Prim(child) => {
                assert_eq!(child.header.name, "Cube");
                assert_eq!(child.header.type_name, Some("Mesh".to_string()));
                assert_eq!(child.items.len(), 1);
            }
            _ => panic!("Expected child prim"),
        }
    }

    #[test]
    fn test_parse_real_world_file() {
        let content = r#"#usda 1.0
(
    upAxis = "Y"
)

def Xform "TexModel" (
    kind = "component"
)
{
    def Mesh "card"
    {
        float3[] extent = [(-430, -145, 0), (430, 145, 0)]
        int[] faceVertexCounts = [4]
        int[] faceVertexIndices = [0, 1, 2, 3]
        point3f[] points = [(-430, -145, 0), (430, -145, 0), (430, 145, 0), (-430, 145, 0)]
    }
}
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.header.format, "usda");
        assert!(result.metadata.is_some());
        assert_eq!(result.prims.len(), 1);

        let root = &result.prims[0];
        assert_eq!(root.header.name, "TexModel");
        assert_eq!(root.header.type_name, Some("Xform".to_string()));
        assert!(root.header.metadata.is_some());
        assert_eq!(root.items.len(), 1);
    }

    // ========================================================================
    // variantSet parsing tests
    // ========================================================================

    #[test]
    fn test_parse_variant_set_block() {
        // C++: variantSet "name" = { "v1" { ... } "v2" { ... } }
        let content = r#"#usda 1.0

def Sphere "MySphere" (
    add variantSets = "shapeVariant"
)
{
    variantSet "shapeVariant" = {
        "sphere" {
        }
        "cube" {
        }
    }
}
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(result.prims.len(), 1);
        let prim = &result.prims[0];
        assert_eq!(prim.header.name, "MySphere");

        // Should have variant set as a prim item
        let vs_count = prim
            .items
            .iter()
            .filter(|i| matches!(i, ParsedPrimItem::VariantSet(_)))
            .count();
        assert_eq!(vs_count, 1, "should have one variant set");

        // Verify variant set details
        if let Some(ParsedPrimItem::VariantSet(vs)) = prim
            .items
            .iter()
            .find(|i| matches!(i, ParsedPrimItem::VariantSet(_)))
        {
            assert_eq!(vs.name, "shapeVariant");
            assert_eq!(vs.variants.len(), 2);
            assert_eq!(vs.variants[0].name, "sphere");
            assert_eq!(vs.variants[1].name, "cube");
        }
    }

    #[test]
    fn test_parse_variant_set_with_content() {
        // Variant set with actual prim content inside variants
        let content = r#"#usda 1.0

def Xform "Model" (
    add variantSets = "look"
)
{
    variantSet "look" = {
        "red" {
            def Material "Mat" {
            }
        }
        "blue" {
        }
    }
}
"#;
        let result = parse_layer_text(content).unwrap();
        let prim = &result.prims[0];

        if let Some(ParsedPrimItem::VariantSet(vs)) = prim
            .items
            .iter()
            .find(|i| matches!(i, ParsedPrimItem::VariantSet(_)))
        {
            assert_eq!(vs.name, "look");
            assert_eq!(vs.variants.len(), 2);
            // "red" variant should have a child prim
            assert!(
                !vs.variants[0].contents.is_empty(),
                "red should have contents"
            );
        } else {
            panic!("no variant set found");
        }
    }

    #[test]
    fn test_parse_variant_sets_metadata_single_string() {
        // Metadata: add variantSets = "name" (single string NameList)
        let content = r#"#usda 1.0

def Sphere "S" (
    add variantSets = "myVariant"
)
{
}
"#;
        let result = parse_layer_text(content).unwrap();
        let prim = &result.prims[0];
        let meta = prim.header.metadata.as_ref().unwrap();
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_parse_variant_sets_metadata_array() {
        // Metadata: add variantSets = ["a", "b"]
        let content = r#"#usda 1.0

def Sphere "S" (
    add variantSets = ["a", "b"]
)
{
}
"#;
        let result = parse_layer_text(content).unwrap();
        let prim = &result.prims[0];
        let meta = prim.header.metadata.as_ref().unwrap();
        assert!(!meta.is_empty());
    }

    #[test]
    fn test_parse_complexity_scene_pattern() {
        // Matches the structure of testUsdviewComplexity/test.usda
        let content = r#"#usda 1.0
(
    defaultPrim = "frontSphere"
    endTimeCode = 1
    startTimeCode = 1
    upAxis = "Z"
)

def Sphere "frontSphere" (
    add variantSets = "shapeVariant"
)
{
    double3 xformOp:translate = (2, 2, 2)
    uniform token[] xformOpOrder = ["xformOp:translate"]

    variantSet "shapeVariant" = {
        "Capsule" {
        }
        "Cone" {
        }
        "Cube" {
        }
        "Cylinder" {
        }
        "Sphere" {
        }
    }
}

def Sphere "backSphere" (
    add variantSets = "shapeVariant"
)
{
    variantSet "shapeVariant" = {
        "Capsule" {
        }
        "Sphere" {
        }
    }
}
"#;
        let result = parse_layer_text(content).unwrap();
        assert_eq!(
            result.prims.len(),
            2,
            "should have frontSphere and backSphere"
        );
        assert_eq!(result.prims[0].header.name, "frontSphere");
        assert_eq!(result.prims[1].header.name, "backSphere");

        // frontSphere should have variant set with 5 variants
        if let Some(ParsedPrimItem::VariantSet(vs)) = result.prims[0]
            .items
            .iter()
            .find(|i| matches!(i, ParsedPrimItem::VariantSet(_)))
        {
            assert_eq!(vs.name, "shapeVariant");
            assert_eq!(vs.variants.len(), 5);
        } else {
            panic!("frontSphere should have variant set");
        }

        // backSphere should have variant set with 2 variants
        if let Some(ParsedPrimItem::VariantSet(vs)) = result.prims[1]
            .items
            .iter()
            .find(|i| matches!(i, ParsedPrimItem::VariantSet(_)))
        {
            assert_eq!(vs.name, "shapeVariant");
            assert_eq!(vs.variants.len(), 2);
        } else {
            panic!("backSphere should have variant set");
        }
    }

    #[test]
    fn test_reorder_property_list_op_connect() {
        // `reorder` as a list-op on a connection attribute
        let content = r#"#usda 1.0
def "Prim" {
    reorder double foo:bargle.connect = </Prim.foo:argle>
}
"#;
        let result = parse_layer_text(content).unwrap();
        let items = &result.prims[0].items;
        assert_eq!(items.len(), 1);
        match &items[0] {
            ParsedPrimItem::PropertyListOp(list_op) => {
                use crate::text_parser::value_context::ArrayEditOp;
                assert_eq!(list_op.op, ArrayEditOp::Reorder);
            }
            other => panic!("expected PropertyListOp, got {other:?}"),
        }
    }

    #[test]
    fn test_reorder_property_list_op_rel() {
        // `reorder` as a list-op on a relationship
        let content = r#"#usda 1.0
def "Prim" {
    reorder varying rel a:b:d = [</Prim>, </Prim/Child>]
}
"#;
        let result = parse_layer_text(content).unwrap();
        let items = &result.prims[0].items;
        assert_eq!(items.len(), 1);
        match &items[0] {
            ParsedPrimItem::PropertyListOp(list_op) => {
                use crate::text_parser::value_context::ArrayEditOp;
                assert_eq!(list_op.op, ArrayEditOp::Reorder);
                // Confirm it resolved to a Relationship
                assert!(matches!(
                    list_op.property,
                    crate::text_parser::specs::ParsedPropertySpec::Relationship(_)
                ));
            }
            other => panic!("expected PropertyListOp, got {other:?}"),
        }
    }

    #[test]
    fn test_uniform_attr_list_ops() {
        // C++ ref: 104_uniformAttributes.usda
        // `delete/add/reorder uniform double foo.connect` must all parse as PropertyListOp
        let content = r#"#usda 1.0
def MfScope "bool_tests"
{
    custom uniform double foo = 0
    delete uniform double foo.connect = </bool_tests/Foo/Blah.blah>
    add uniform double foo.connect = </bool_tests/Foo/Blah.blah>
    reorder uniform double foo.connect = [
        </bool_tests/Foo/Blah.blah>,
        </bool_tests/Foo/Bar.blah>,
    ]
}
"#;
        let result = parse_layer_text(content).unwrap();
        let items = &result.prims[0].items;
        assert_eq!(items.len(), 4);
        // First item: plain property (custom uniform double foo = 0)
        assert!(matches!(&items[0], ParsedPrimItem::Property(_)));
        // Next three: list-op operations on foo.connect
        use crate::text_parser::value_context::ArrayEditOp;
        match &items[1] {
            ParsedPrimItem::PropertyListOp(lop) => {
                assert_eq!(lop.op, ArrayEditOp::Delete);
                assert!(matches!(
                    &lop.property,
                    crate::text_parser::specs::ParsedPropertySpec::Attribute(a)
                        if a.name == "foo" && matches!(a.variability, crate::text_parser::specs::Variability::Uniform)
                ));
            }
            other => panic!("expected PropertyListOp(Delete), got {other:?}"),
        }
        match &items[2] {
            ParsedPrimItem::PropertyListOp(lop) => assert_eq!(lop.op, ArrayEditOp::Add),
            other => panic!("expected PropertyListOp(Add), got {other:?}"),
        }
        match &items[3] {
            ParsedPrimItem::PropertyListOp(lop) => assert_eq!(lop.op, ArrayEditOp::Reorder),
            other => panic!("expected PropertyListOp(Reorder), got {other:?}"),
        }
    }

    #[test]
    fn test_reorder_namechildren_and_properties_still_work() {
        // Existing ordering statements must not regress
        let content = r#"#usda 1.0
def "Prim" {
    reorder nameChildren = ["Child2", "Child1"]
    reorder properties = ["b", "a"]
}
"#;
        let result = parse_layer_text(content).unwrap();
        let items = &result.prims[0].items;
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], ParsedPrimItem::ChildOrder(v) if v == &["Child2", "Child1"]));
        assert!(matches!(&items[1], ParsedPrimItem::PropertyOrder(v) if v == &["b", "a"]));
    }
}
