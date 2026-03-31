//! Token types and keywords for the USDA text parser.
//!
//! This module defines all token types recognized by the lexer and the
//! complete set of USD keywords. Token types are modeled after the C++
//! PEGTL grammar in `textFileFormatParser.h`.
//!
//! # Token Categories
//!
//! - **Literals**: Numbers, strings, identifiers, asset references
//! - **Keywords**: Reserved words like `def`, `over`, `class`, `custom`
//! - **Operators**: Assignment, separators, brackets
//! - **Special**: Comments, whitespace, EOF
//!
//! # Examples
//!
//! ```
//! use usd_sdf::text_parser::{TokenKind, Keyword};
//!
//! let kind = TokenKind::Keyword(Keyword::Def);
//! assert!(kind.is_keyword());
//! ```

use std::fmt;

use super::error::SourceSpan;

// ============================================================================
// Keywords
// ============================================================================

/// USD keywords recognized by the parser.
///
/// These keywords are reserved and cannot be used as identifiers in certain
/// contexts. The list is derived from the C++ PEGTL grammar.
///
/// # Categories
///
/// - **Specifiers**: `def`, `over`, `class`
/// - **Variability**: `uniform`, `varying`, `config`, `custom`
/// - **List operations**: `add`, `delete`, `append`, `prepend`, `reorder`
/// - **Metadata**: `doc`, `kind`, `permission`, `payload`, `references`, etc.
/// - **Property types**: `rel`, `connect`, `timeSamples`
/// - **Values**: `None`, `true`, `false`
/// - **Splines**: `bezier`, `hermite`, `held`, `linear`, `curve`, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    // Specifiers
    /// `def` - Define a new prim
    Def,
    /// `over` - Override an existing prim
    Over,
    /// `class` - Define an abstract class prim
    Class,

    // Variability
    /// `uniform` - Value doesn't vary over time
    Uniform,
    /// `varying` - Value can vary over time
    Varying,
    /// `config` - Configuration value
    Config,
    /// `custom` - Custom (user-defined) property
    Custom,

    // List operations
    /// `add` - Add to list
    Add,
    /// `delete` - Delete from list
    Delete,
    /// `append` - Append to list
    Append,
    /// `prepend` - Prepend to list
    Prepend,
    /// `reorder` - Reorder list
    Reorder,

    // Ordering
    /// `nameChildren` - Child prim ordering
    NameChildren,
    /// `properties` - Property ordering
    Properties,
    /// `rootPrims` - Root prim ordering
    RootPrims,

    // Composition
    /// `inherits` - Inheritance arcs
    Inherits,
    /// `specializes` - Specialization arcs
    Specializes,
    /// `references` - Reference arcs
    References,
    /// `payload` - Payload arcs
    Payload,
    /// `subLayers` - Sublayer list
    SubLayers,
    /// `relocates` - Path relocations
    Relocates,

    // Variants
    /// `variantSet` - Variant set definition
    VariantSet,
    /// `variantSets` - Variant sets metadata
    VariantSets,
    /// `variants` - Variant selections
    Variants,

    // Properties
    /// `rel` - Relationship property
    Rel,
    /// `connect` - Attribute connection
    Connect,
    /// `timeSamples` - Time-sampled values
    TimeSamples,
    /// `default` - Default value
    Default,

    // Metadata
    /// `doc` - Documentation string
    Doc,
    /// `kind` - Prim kind
    Kind,
    /// `permission` - Access permission
    Permission,
    /// `symmetryFunction` - Symmetry function
    SymmetryFunction,
    /// `symmetryArguments` - Symmetry arguments
    SymmetryArguments,
    /// `customData` - Custom data dictionary
    CustomData,
    /// `displayUnit` - Display unit
    DisplayUnit,
    /// `prefixSubstitutions` - Prefix substitutions
    PrefixSubstitutions,
    /// `suffixSubstitutions` - Suffix substitutions
    SuffixSubstitutions,

    // Values
    /// `None` - Null/empty value
    None,
    /// `none` - Lowercase none (for splines)
    NoneLowercase,
    /// `dictionary` - Dictionary type
    Dictionary,

    // Layer offsets
    /// `offset` - Time offset
    Offset,
    /// `scale` - Time scale
    Scale,

    // Spline keywords
    /// `spline` - Spline value
    Spline,
    /// `bezier` - Bezier interpolation
    Bezier,
    /// `hermite` - Hermite interpolation
    Hermite,
    /// `held` - Held interpolation
    Held,
    /// `linear` - Linear interpolation
    Linear,
    /// `curve` - Curve interpolation
    Curve,
    /// `pre` - Pre-extrapolation
    Pre,
    /// `post` - Post-extrapolation
    Post,
    /// `loop` - Loop extrapolation
    Loop,
    /// `repeat` - Repeat loop mode
    Repeat,
    /// `reset` - Reset loop mode
    Reset,
    /// `oscillate` - Oscillate loop mode
    Oscillate,
    /// `sloped` - Sloped extrapolation
    Sloped,
    /// `autoEase` - Auto-ease tangent
    AutoEase,

    // Array edit keywords
    /// `edit` - Array edit operation
    Edit,
    /// `insert` - Insert into array
    Insert,
    /// `erase` - Erase from array
    Erase,
    /// `write` - Write to array
    Write,
    /// `at` - At position
    At,
    /// `to` - To position
    To,
    /// `fill` - Fill with value
    Fill,
    /// `minsize` - Minimum size
    Minsize,
    /// `maxsize` - Maximum size
    Maxsize,
    /// `resize` - Resize array
    Resize,

    // Animation
    /// `AnimationBlock` - Animation block marker
    AnimationBlock,
}

impl Keyword {
    /// Returns the string representation of the keyword.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            // Specifiers
            Self::Def => "def",
            Self::Over => "over",
            Self::Class => "class",

            // Variability
            Self::Uniform => "uniform",
            Self::Varying => "varying",
            Self::Config => "config",
            Self::Custom => "custom",

            // List operations
            Self::Add => "add",
            Self::Delete => "delete",
            Self::Append => "append",
            Self::Prepend => "prepend",
            Self::Reorder => "reorder",

            // Ordering
            Self::NameChildren => "nameChildren",
            Self::Properties => "properties",
            Self::RootPrims => "rootPrims",

            // Composition
            Self::Inherits => "inherits",
            Self::Specializes => "specializes",
            Self::References => "references",
            Self::Payload => "payload",
            Self::SubLayers => "subLayers",
            Self::Relocates => "relocates",

            // Variants
            Self::VariantSet => "variantSet",
            Self::VariantSets => "variantSets",
            Self::Variants => "variants",

            // Properties
            Self::Rel => "rel",
            Self::Connect => "connect",
            Self::TimeSamples => "timeSamples",
            Self::Default => "default",

            // Metadata
            Self::Doc => "doc",
            Self::Kind => "kind",
            Self::Permission => "permission",
            Self::SymmetryFunction => "symmetryFunction",
            Self::SymmetryArguments => "symmetryArguments",
            Self::CustomData => "customData",
            Self::DisplayUnit => "displayUnit",
            Self::PrefixSubstitutions => "prefixSubstitutions",
            Self::SuffixSubstitutions => "suffixSubstitutions",

            // Values
            Self::None => "None",
            Self::NoneLowercase => "none",
            Self::Dictionary => "dictionary",

            // Layer offsets
            Self::Offset => "offset",
            Self::Scale => "scale",

            // Splines
            Self::Spline => "spline",
            Self::Bezier => "bezier",
            Self::Hermite => "hermite",
            Self::Held => "held",
            Self::Linear => "linear",
            Self::Curve => "curve",
            Self::Pre => "pre",
            Self::Post => "post",
            Self::Loop => "loop",
            Self::Repeat => "repeat",
            Self::Reset => "reset",
            Self::Oscillate => "oscillate",
            Self::Sloped => "sloped",
            Self::AutoEase => "autoEase",

            // Array edit
            Self::Edit => "edit",
            Self::Insert => "insert",
            Self::Erase => "erase",
            Self::Write => "write",
            Self::At => "at",
            Self::To => "to",
            Self::Fill => "fill",
            Self::Minsize => "minsize",
            Self::Maxsize => "maxsize",
            Self::Resize => "resize",

            // Animation
            Self::AnimationBlock => "AnimationBlock",
        }
    }

    /// Looks up a keyword by name.
    ///
    /// Returns `None` if the string is not a keyword.
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            // Specifiers
            "def" => Some(Self::Def),
            "over" => Some(Self::Over),
            "class" => Some(Self::Class),

            // Variability
            "uniform" => Some(Self::Uniform),
            "varying" => Some(Self::Varying),
            "config" => Some(Self::Config),
            "custom" => Some(Self::Custom),

            // List operations
            "add" => Some(Self::Add),
            "delete" => Some(Self::Delete),
            "append" => Some(Self::Append),
            "prepend" => Some(Self::Prepend),
            "reorder" => Some(Self::Reorder),

            // Ordering
            "nameChildren" => Some(Self::NameChildren),
            "properties" => Some(Self::Properties),
            "rootPrims" => Some(Self::RootPrims),

            // Composition
            "inherits" => Some(Self::Inherits),
            "specializes" => Some(Self::Specializes),
            "references" => Some(Self::References),
            "payload" => Some(Self::Payload),
            "subLayers" => Some(Self::SubLayers),
            "relocates" => Some(Self::Relocates),

            // Variants
            "variantSet" => Some(Self::VariantSet),
            "variantSets" => Some(Self::VariantSets),
            "variants" => Some(Self::Variants),

            // Properties
            "rel" => Some(Self::Rel),
            "connect" => Some(Self::Connect),
            "timeSamples" => Some(Self::TimeSamples),
            "default" => Some(Self::Default),

            // Metadata
            "doc" => Some(Self::Doc),
            "kind" => Some(Self::Kind),
            "permission" => Some(Self::Permission),
            "symmetryFunction" => Some(Self::SymmetryFunction),
            "symmetryArguments" => Some(Self::SymmetryArguments),
            "customData" => Some(Self::CustomData),
            "displayUnit" => Some(Self::DisplayUnit),
            "prefixSubstitutions" => Some(Self::PrefixSubstitutions),
            "suffixSubstitutions" => Some(Self::SuffixSubstitutions),

            // Values
            "None" => Some(Self::None),
            "none" => Some(Self::NoneLowercase),
            "dictionary" => Some(Self::Dictionary),

            // Layer offsets
            "offset" => Some(Self::Offset),
            "scale" => Some(Self::Scale),

            // Splines
            "spline" => Some(Self::Spline),
            "bezier" => Some(Self::Bezier),
            "hermite" => Some(Self::Hermite),
            "held" => Some(Self::Held),
            "linear" => Some(Self::Linear),
            "curve" => Some(Self::Curve),
            "pre" => Some(Self::Pre),
            "post" => Some(Self::Post),
            "loop" => Some(Self::Loop),
            "repeat" => Some(Self::Repeat),
            "reset" => Some(Self::Reset),
            "oscillate" => Some(Self::Oscillate),
            "sloped" => Some(Self::Sloped),
            "autoEase" => Some(Self::AutoEase),

            // Array edit
            "edit" => Some(Self::Edit),
            "insert" => Some(Self::Insert),
            "erase" => Some(Self::Erase),
            "write" => Some(Self::Write),
            "at" => Some(Self::At),
            "to" => Some(Self::To),
            "fill" => Some(Self::Fill),
            "minsize" => Some(Self::Minsize),
            "maxsize" => Some(Self::Maxsize),
            "resize" => Some(Self::Resize),

            // Animation
            "AnimationBlock" => Some(Self::AnimationBlock),

            _ => Option::None,
        }
    }

    /// Returns true if this keyword is a specifier (def/over/class).
    #[inline]
    #[must_use]
    pub const fn is_specifier(&self) -> bool {
        matches!(self, Self::Def | Self::Over | Self::Class)
    }

    /// Returns true if this keyword is a list operation.
    #[inline]
    #[must_use]
    pub const fn is_list_op(&self) -> bool {
        matches!(
            self,
            Self::Add | Self::Delete | Self::Append | Self::Prepend | Self::Reorder
        )
    }

    /// Returns true if this keyword is a variability modifier.
    #[inline]
    #[must_use]
    pub const fn is_variability(&self) -> bool {
        matches!(self, Self::Uniform | Self::Varying | Self::Config)
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Token Kind
// ============================================================================

/// The kind of token recognized by the lexer.
///
/// Each variant represents a distinct syntactic element in USD text files.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ========================================================================
    // Magic Identifier
    // ========================================================================
    /// Magic header identifier: `#usda 1.0` or `#sdf 1.4.32`.
    Magic {
        /// Format name ("usda" or "sdf").
        format: String,
        /// Version string (e.g., "1.0").
        version: String,
    },

    // ========================================================================
    // Literals
    // ========================================================================
    /// Integer literal (e.g., `42`, `-7`)
    Integer(i64),

    /// Floating-point literal (e.g., `3.14`, `-0.5e10`)
    Float(f64),

    /// Positive infinity
    Inf,

    /// Negative infinity
    NegInf,

    /// Not-a-number
    Nan,

    /// String literal (content without quotes)
    String(String),

    /// Identifier (e.g., `myPrim`, `xformOp:translate`)
    Identifier(String),

    /// Asset reference path (content without @)
    AssetRef(String),

    /// Path reference (content without <>)
    PathRef(String),

    // ========================================================================
    // Keywords
    // ========================================================================
    /// A reserved keyword
    Keyword(Keyword),

    // ========================================================================
    // Punctuation
    // ========================================================================
    /// `(`
    LeftParen,
    /// `)`
    RightParen,
    /// `[`
    LeftBracket,
    /// `]`
    RightBracket,
    /// `{`
    LeftBrace,
    /// `}`
    RightBrace,
    /// `<`
    LeftAngle,
    /// `>`
    RightAngle,

    /// `=`
    Equals,
    /// `:`
    Colon,
    /// `::`
    DoubleColon,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `.`
    Dot,
    /// `&`
    Ampersand,

    // ========================================================================
    // Special
    // ========================================================================
    /// Newline (significant for statement separation)
    Newline,

    /// End of file
    Eof,

    /// Error token (lexer error)
    Error(String),
}

impl TokenKind {
    /// Returns true if this is a keyword token.
    #[inline]
    #[must_use]
    pub const fn is_keyword(&self) -> bool {
        matches!(self, Self::Keyword(_))
    }

    /// Returns true if this is a literal token.
    #[inline]
    #[must_use]
    pub fn is_literal(&self) -> bool {
        matches!(
            self,
            Self::Integer(_)
                | Self::Float(_)
                | Self::Inf
                | Self::NegInf
                | Self::Nan
                | Self::String(_)
                | Self::Identifier(_)
                | Self::AssetRef(_)
                | Self::PathRef(_)
        )
    }

    /// Returns true if this is a number token.
    #[inline]
    #[must_use]
    pub fn is_number(&self) -> bool {
        matches!(
            self,
            Self::Integer(_) | Self::Float(_) | Self::Inf | Self::NegInf | Self::Nan
        )
    }

    /// Returns true if this is an opening bracket.
    #[inline]
    #[must_use]
    pub const fn is_opening_bracket(&self) -> bool {
        matches!(
            self,
            Self::LeftParen | Self::LeftBracket | Self::LeftBrace | Self::LeftAngle
        )
    }

    /// Returns true if this is a closing bracket.
    #[inline]
    #[must_use]
    pub const fn is_closing_bracket(&self) -> bool {
        matches!(
            self,
            Self::RightParen | Self::RightBracket | Self::RightBrace | Self::RightAngle
        )
    }

    /// Returns the matching closing bracket for an opening bracket.
    #[must_use]
    pub const fn matching_bracket(&self) -> Option<Self> {
        match self {
            Self::LeftParen => Some(Self::RightParen),
            Self::LeftBracket => Some(Self::RightBracket),
            Self::LeftBrace => Some(Self::RightBrace),
            Self::LeftAngle => Some(Self::RightAngle),
            Self::RightParen => Some(Self::LeftParen),
            Self::RightBracket => Some(Self::LeftBracket),
            Self::RightBrace => Some(Self::LeftBrace),
            Self::RightAngle => Some(Self::LeftAngle),
            _ => Option::None,
        }
    }

    /// Returns the closing character for bracket tokens.
    #[must_use]
    pub const fn bracket_char(&self) -> Option<char> {
        match self {
            Self::LeftParen | Self::RightParen => Some(')'),
            Self::LeftBracket | Self::RightBracket => Some(']'),
            Self::LeftBrace | Self::RightBrace => Some('}'),
            Self::LeftAngle | Self::RightAngle => Some('>'),
            _ => Option::None,
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Magic { format, version } => write!(f, "#{} {}", format, version),
            Self::Integer(n) => write!(f, "{}", n),
            Self::Float(n) => write!(f, "{}", n),
            Self::Inf => write!(f, "inf"),
            Self::NegInf => write!(f, "-inf"),
            Self::Nan => write!(f, "nan"),
            Self::String(s) => write!(f, "\"{}\"", s),
            Self::Identifier(s) => write!(f, "{}", s),
            Self::AssetRef(s) => write!(f, "@{}@", s),
            Self::PathRef(s) => write!(f, "<{}>", s),
            Self::Keyword(k) => write!(f, "{}", k),
            Self::LeftParen => write!(f, "("),
            Self::RightParen => write!(f, ")"),
            Self::LeftBracket => write!(f, "["),
            Self::RightBracket => write!(f, "]"),
            Self::LeftBrace => write!(f, "{{"),
            Self::RightBrace => write!(f, "}}"),
            Self::LeftAngle => write!(f, "<"),
            Self::RightAngle => write!(f, ">"),
            Self::Equals => write!(f, "="),
            Self::Colon => write!(f, ":"),
            Self::DoubleColon => write!(f, "::"),
            Self::Comma => write!(f, ","),
            Self::Semicolon => write!(f, ";"),
            Self::Dot => write!(f, "."),
            Self::Ampersand => write!(f, "&"),
            Self::Newline => write!(f, "newline"),
            Self::Eof => write!(f, "end of file"),
            Self::Error(msg) => write!(f, "error: {}", msg),
        }
    }
}

// ============================================================================
// Token
// ============================================================================

/// A token produced by the lexer.
///
/// Contains the token kind and its source span.
#[derive(Debug, Clone)]
pub struct Token {
    /// The kind of token.
    pub kind: TokenKind,
    /// The source span of the token.
    pub span: SourceSpan,
}

impl Token {
    /// Creates a new token.
    #[inline]
    #[must_use]
    pub fn new(kind: TokenKind, span: SourceSpan) -> Self {
        Self { kind, span }
    }

    /// Backwards-compat: creates token (lexeme param ignored for performance).
    #[inline]
    #[must_use]
    pub fn with_lexeme(kind: TokenKind, span: SourceSpan, _lexeme: &str) -> Self {
        Self { kind, span }
    }

    /// Returns true if this is a specific keyword.
    #[inline]
    #[must_use]
    pub fn is_keyword(&self, kw: Keyword) -> bool {
        matches!(&self.kind, TokenKind::Keyword(k) if *k == kw)
    }

    /// Returns the keyword if this is a keyword token.
    #[inline]
    #[must_use]
    pub fn as_keyword(&self) -> Option<Keyword> {
        match &self.kind {
            TokenKind::Keyword(k) => Some(*k),
            _ => None,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_lookup() {
        assert_eq!(Keyword::from_str("def"), Some(Keyword::Def));
        assert_eq!(Keyword::from_str("over"), Some(Keyword::Over));
        assert_eq!(Keyword::from_str("class"), Some(Keyword::Class));
        assert_eq!(Keyword::from_str("custom"), Some(Keyword::Custom));
        assert_eq!(Keyword::from_str("None"), Some(Keyword::None));
        assert_eq!(Keyword::from_str("notakeyword"), None);
    }

    #[test]
    fn test_keyword_roundtrip() {
        let keywords = [
            Keyword::Def,
            Keyword::Over,
            Keyword::Class,
            Keyword::Uniform,
            Keyword::Add,
            Keyword::TimeSamples,
        ];

        for kw in keywords {
            let s = kw.as_str();
            let parsed = Keyword::from_str(s);
            assert_eq!(parsed, Some(kw), "Failed roundtrip for {:?}", kw);
        }
    }

    #[test]
    fn test_keyword_categories() {
        assert!(Keyword::Def.is_specifier());
        assert!(Keyword::Over.is_specifier());
        assert!(Keyword::Class.is_specifier());
        assert!(!Keyword::Custom.is_specifier());

        assert!(Keyword::Add.is_list_op());
        assert!(Keyword::Delete.is_list_op());
        assert!(!Keyword::Def.is_list_op());

        assert!(Keyword::Uniform.is_variability());
        assert!(Keyword::Varying.is_variability());
        assert!(!Keyword::Custom.is_variability());
    }

    #[test]
    fn test_token_kind_categories() {
        assert!(TokenKind::Integer(42).is_literal());
        assert!(TokenKind::Float(3.14).is_number());
        assert!(TokenKind::Inf.is_number());
        assert!(TokenKind::Keyword(Keyword::Def).is_keyword());
    }

    #[test]
    fn test_bracket_matching() {
        assert_eq!(
            TokenKind::LeftParen.matching_bracket(),
            Some(TokenKind::RightParen)
        );
        assert_eq!(
            TokenKind::LeftBrace.matching_bracket(),
            Some(TokenKind::RightBrace)
        );
        assert_eq!(TokenKind::Comma.matching_bracket(), None);
    }

    #[test]
    fn test_token_display() {
        assert_eq!(format!("{}", TokenKind::Integer(42)), "42");
        assert_eq!(format!("{}", TokenKind::Float(3.14)), "3.14");
        assert_eq!(format!("{}", TokenKind::Keyword(Keyword::Def)), "def");
        assert_eq!(format!("{}", TokenKind::LeftParen), "(");
    }
}
