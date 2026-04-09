//! OSL Lexer -- logos-based tokenizer for OpenShadingLanguage source code.
//!
//! Direct translation of `osllex.l`. Produces tokens consumed by the
//! LALRPOP-generated parser.

use logos::Logos;
use std::fmt;

// --- Source location ----------------------------------------------------

/// Source location in the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceLoc {
    pub line: u32,
    pub col: u32,
}

impl fmt::Display for SourceLoc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// Convert a byte offset in source to a `SourceLoc` (line, col).
pub fn offset_to_loc(source: &str, offset: usize) -> SourceLoc {
    let mut line = 1u32;
    let mut col = 1u32;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    SourceLoc { line, col }
}

// --- Token type ---------------------------------------------------------

/// Token type for the OSL lexer.
///
/// LALRPOP expects `(Loc, Tok, Loc)` triples. We use `usize` byte
/// offsets as locations and convert to `SourceLoc` only when building
/// AST nodes / error messages.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n]+")] // whitespace
#[logos(skip(r"//[^\n]*", allow_greedy = true))] // line comments
#[logos(skip r"/\*([^*]|\*[^/])*\*/")] // block comments
pub enum Tok {
    // -- Keywords --------------------------------------------------------
    #[token("shader")]
    Shader,
    #[token("surface")]
    Surface,
    #[token("displacement")]
    Displacement,
    #[token("volume")]
    Volume,
    #[token("struct")]
    Struct,
    #[token("closure")]
    Closure,
    #[token("output")]
    Output,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("for")]
    For,
    #[token("while")]
    While,
    #[token("do")]
    Do,
    #[token("return")]
    Return,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,

    // C++ or/and/not keyword aliases (converted to operator tokens in OslLexer)
    #[token("or")]
    OrKeyword,
    #[token("and")]
    AndKeyword,
    #[token("not")]
    NotKeyword,

    // -- Type keywords ---------------------------------------------------
    #[token("int")]
    IntType,
    #[token("float")]
    FloatType,
    #[token("string")]
    StringType,
    #[token("color")]
    ColorType,
    #[token("point")]
    PointType,
    #[token("vector")]
    VectorType,
    #[token("normal")]
    NormalType,
    #[token("matrix")]
    MatrixType,
    #[token("void")]
    VoidType,

    // -- Metadata attribute brackets -------------------------------------
    #[token("[[")]
    MetadataBegin,
    // NOTE: No MetadataEnd token -- C++ uses two separate ']' tokens
    // for metadata close. This avoids mislex of nested array access like a[b[0]].

    // -- Compound operators (must come before single-char) ----------------
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    LessEq,
    #[token(">=")]
    GreaterEq,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("<<=")]
    ShiftLeftAssign,
    #[token(">>=")]
    ShiftRightAssign,
    #[token("<<")]
    ShiftLeft,
    #[token(">>")]
    ShiftRight,
    #[token("+=")]
    PlusAssign,
    #[token("-=")]
    MinusAssign,
    #[token("*=")]
    StarAssign,
    #[token("/=")]
    SlashAssign,
    #[token("&=")]
    AmpAssign,
    #[token("|=")]
    PipeAssign,
    #[token("^=")]
    CaretAssign,
    #[token("++")]
    PlusPlus,
    #[token("--")]
    MinusMinus,

    // -- Single-char operators & punctuation ------------------------------
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("~")]
    Tilde,
    #[token("!")]
    Not,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token(":")]
    Colon,
    #[token("?")]
    Question,
    #[token("=")]
    Eq,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,

    // -- Literals --------------------------------------------------------
    #[regex(r"0[xX][0-9a-fA-F]+", parse_hex_int)]
    HexLiteral(i32),

    #[regex(r"0[0-7]+", priority = 3, callback = parse_octal)]
    OctalLiteral(i32),

    #[regex(r"[0-9]+", priority = 2, callback = parse_decimal_int)]
    IntLiteral(i32),

    #[regex(r"[0-9]*\.[0-9]+([eE][+-]?[0-9]+)?[fF]?|[0-9]+\.[0-9]*([eE][+-]?[0-9]+)?[fF]?|[0-9]+[eE][+-]?[0-9]+[fF]?|[0-9]+[fF]", priority = 3, callback = parse_float)]
    FloatLiteral(f32),

    #[regex(r#""([^"\\]|\\.)*""#, parse_string)]
    StringLiteral(String),

    // -- Identifier (must come after all keyword tokens) ------------------
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),

    // -- Preprocessor directives (pass through) ---------------------------
    #[regex(r"#[ \t]*pragma[ \t]+[^\n]*", allow_greedy = true, callback = |lex| lex.slice().to_string())]
    Pragma(String),

    #[regex(r"#[ \t]*line[ \t]+[^\n]*", allow_greedy = true, callback = |lex| lex.slice().to_string())]
    LineDirective(String),

    #[regex(r"#[^\n]*", allow_greedy = true, callback = |lex| lex.slice().to_string())]
    PreprocessorOther(String),
}

thread_local! {
    static LEX_INT_OVERFLOW: std::cell::RefCell<Option<(usize, String)>> = const { std::cell::RefCell::new(None) };
}

/// Parse decimal integer with overflow checking (C++ parity: osllex.l:195–207).
/// INT_MAX+1 is allowed (for negation to INT_MIN). Values > INT_MAX+1 report error but still return truncated value.
fn parse_decimal_int(lex: &mut logos::Lexer<Tok>) -> Option<i32> {
    let s = lex.slice();
    let span_start = lex.span().start;
    let v = s.parse::<i64>().unwrap_or(i64::MAX);
    if v > (i32::MAX as i64) + 1 {
        LEX_INT_OVERFLOW.with(|c| {
            *c.borrow_mut() = Some((
                span_start,
                format!(
                    "integer overflow, value must be between {} and {}.",
                    i32::MIN,
                    i32::MAX
                ),
            ));
        });
    }
    Some(v as i32)
}

/// Parse hex integer with overflow checking (C++ parity: osllex.l:208–221).
/// UINT_MAX+1 is allowed. Values > UINT_MAX+1 report error but still return truncated value.
fn parse_hex_int(lex: &mut logos::Lexer<Tok>) -> Option<i32> {
    let s = &lex.slice()[2..]; // skip 0x/0X
    let span_start = lex.span().start;
    let v = u64::from_str_radix(s, 16).unwrap_or(u64::MAX);
    if v > (u32::MAX as u64) + 1 {
        LEX_INT_OVERFLOW.with(|c| {
            *c.borrow_mut() = Some((
                span_start,
                format!(
                    "integer overflow, value must be between {} and {}.",
                    i32::MIN,
                    i32::MAX
                ),
            ));
        });
    }
    Some(v as i32)
}

fn parse_octal(lex: &mut logos::Lexer<Tok>) -> Option<i32> {
    // Skip leading '0', parse rest as octal
    let s = lex.slice();
    u32::from_str_radix(&s[1..], 8).ok().map(|v| v as i32)
}

fn parse_float(lex: &mut logos::Lexer<Tok>) -> Option<f32> {
    let s = lex.slice().trim_end_matches(['f', 'F']);
    s.parse::<f32>().ok()
}

fn parse_string(lex: &mut logos::Lexer<Tok>) -> Option<String> {
    let raw = lex.slice();
    let inner = &raw[1..raw.len() - 1];
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some('0') => result.push('\0'),
                Some('r') => result.push('\r'),
                Some('a') => result.push('\x07'),
                Some('b') => result.push('\x08'),
                Some('f') => result.push('\x0C'),
                // \xNN hex escape: 2 hex digits -> byte value
                Some('x') => {
                    let hi = chars.next().and_then(|c| c.to_digit(16));
                    let lo = chars.next().and_then(|c| c.to_digit(16));
                    if let (Some(h), Some(l)) = (hi, lo) {
                        result.push(((h << 4) | l) as u8 as char);
                    }
                    // Silently drop malformed \x sequences
                }
                Some(other) => {
                    result.push('\\');
                    result.push(other);
                }
                None => break,
            }
        } else {
            result.push(c);
        }
    }
    Some(result)
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Shader => write!(f, "shader"),
            Tok::Identifier(s) => write!(f, "{}", s),
            Tok::IntLiteral(v) | Tok::OctalLiteral(v) | Tok::HexLiteral(v) => write!(f, "{}", v),
            Tok::FloatLiteral(v) => write!(f, "{}", v),
            Tok::StringLiteral(s) => write!(f, "\"{}\"", s),
            other => write!(f, "{:?}", other),
        }
    }
}

// --- Lex error ----------------------------------------------------------

#[derive(Debug, Clone)]
pub struct LexError {
    pub loc: usize,
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unrecognized token at byte {}", self.loc)
    }
}

// --- Reserved words (C++ parity: osllex.l warns on these) ---------------

/// Reserved words that emit an error if used as identifiers.
/// Includes C++ RESERVED rule words (osllex.l lines 172–184) plus OSL keywords
/// that are not supported in the current grammar: `light`, `public`,
/// `illuminate`, `illuminance`.
const RESERVED_WORDS: &[&str] = &[
    "bool",
    "case",
    "char",
    "class",
    "const",
    "default",
    "double",
    "enum",
    "extern",
    "false",
    "friend",
    "inline",
    "long",
    "private",
    "protected",
    "short",
    "signed",
    "sizeof",
    "static",
    "switch",
    "template",
    "this",
    "true",
    "typedef",
    "uniform",
    "union",
    "unsigned",
    "varying",
    "virtual",
    // Not supported in OSL grammar (RSL/RenderMan legacy)
    "light",
    "public",
    "illuminate",
    "illuminance",
];

/// Check if a word is a reserved keyword.
pub fn is_reserved_word(word: &str) -> bool {
    RESERVED_WORDS.contains(&word)
}

// --- Adapter: logos lexer -> lalrpop token stream ------------------------

/// Iterator adapter that converts `logos::Lexer<Tok>` into
/// `(usize, Tok, usize)` triples that LALRPOP expects.
pub struct OslLexer<'input> {
    inner: logos::Lexer<'input, Tok>,
    /// Source text for error messages.
    source: &'input str,
    /// Accumulated errors (reserved word, integer overflow — C++ errorfmt parity).
    pub errors: Vec<(SourceLoc, String)>,
}

impl<'input> OslLexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            inner: Tok::lexer(input),
            source: input,
            errors: Vec::new(),
        }
    }
}

impl<'input> Iterator for OslLexer<'input> {
    type Item = Result<(usize, Tok, usize), LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        let tok = self.inner.next()?;
        let span = self.inner.span();
        match tok {
            Ok(t) => {
                // Skip preprocessor directives
                match &t {
                    Tok::Pragma(_) | Tok::LineDirective(_) | Tok::PreprocessorOther(_) => {
                        return self.next();
                    }
                    _ => {}
                }

                // Convert or/and/not keywords to their operator equivalents
                let t = match t {
                    Tok::OrKeyword => Tok::OrOr,
                    Tok::AndKeyword => Tok::AndAnd,
                    Tok::NotKeyword => Tok::Not,
                    other => other,
                };

                // Error on reserved word usage (C++ parity: osllex.l RESERVED rule, errorfmt)
                if let Tok::Identifier(ref id) = t
                    && is_reserved_word(id)
                {
                    let loc = offset_to_loc(self.source, span.start);
                    self.errors
                        .push((loc, format!("'{}' is a reserved word", id)));
                }

                // Check for integer overflow from parse_decimal_int/parse_hex_int
                if matches!(t, Tok::IntLiteral(_) | Tok::HexLiteral(_)) {
                    LEX_INT_OVERFLOW.with(|c| {
                        if let Some((off, msg)) = c.borrow_mut().take() {
                            let loc = offset_to_loc(self.source, off);
                            self.errors.push((loc, msg));
                        }
                    });
                }

                Some(Ok((span.start, t, span.end)))
            }
            Err(()) => Some(Err(LexError { loc: span.start })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect all tokens from source (ignoring errors).
    fn lex_tokens(src: &str) -> Vec<Tok> {
        OslLexer::new(src)
            .filter_map(|r| r.ok().map(|(_, t, _)| t))
            .collect()
    }

    #[test]
    fn test_metadata_brackets() {
        // C++ uses [[ as one token, but ]] as two separate ] tokens
        let tokens = lex_tokens("[[ ]]");
        assert_eq!(
            tokens,
            vec![Tok::MetadataBegin, Tok::RBracket, Tok::RBracket]
        );
    }

    #[test]
    fn test_or_and_not_keywords() {
        // or/and/not should be converted to their operator equivalents
        let tokens = lex_tokens("or and not");
        assert_eq!(tokens, vec![Tok::OrOr, Tok::AndAnd, Tok::Not]);
    }

    #[test]
    fn test_reserved_word_error() {
        let mut lexer = OslLexer::new("bool x");
        let toks: Vec<_> = lexer
            .by_ref()
            .filter_map(|r| r.ok().map(|(_, t, _)| t))
            .collect();
        assert_eq!(toks.len(), 2);
        // "bool" is reserved -- should produce an error (C++ errorfmt parity)
        assert_eq!(lexer.errors.len(), 1);
        assert!(lexer.errors[0].1.contains("bool"));
        assert!(lexer.errors[0].1.contains("reserved word"));
    }

    #[test]
    fn test_reserved_word_not_identifier() {
        // Non-reserved identifiers should NOT produce errors
        let mut lexer = OslLexer::new("myvar");
        let _: Vec<_> = lexer.by_ref().filter_map(|r| r.ok()).collect();
        assert!(lexer.errors.is_empty());
    }

    #[test]
    fn test_int_overflow_clamps() {
        // 2147483647 == i32::MAX, should parse fine
        let tokens = lex_tokens("2147483647");
        assert_eq!(tokens, vec![Tok::IntLiteral(i32::MAX)]);
    }

    #[test]
    fn test_int_overflow_max_plus_one() {
        // 2147483648 == INT_MAX+1, allowed in C++ (for negation to INT_MIN)
        let tokens = lex_tokens("2147483648");
        assert_eq!(tokens, vec![Tok::IntLiteral(i32::MIN)]);
    }

    #[test]
    fn test_hex_literal() {
        let tokens = lex_tokens("0xFF");
        assert_eq!(tokens, vec![Tok::HexLiteral(255)]);
    }

    #[test]
    fn test_hex_overflow() {
        // 0x100000001 > UINT_MAX+1, triggers error (C++ parity)
        let mut lexer = OslLexer::new("0x100000001");
        let toks: Vec<_> = lexer
            .by_ref()
            .filter_map(|r| r.ok().map(|(_, t, _)| t))
            .collect();
        assert_eq!(toks, vec![Tok::HexLiteral(1)]); // truncated
        assert_eq!(lexer.errors.len(), 1);
        assert!(lexer.errors[0].1.contains("overflow"));
    }

    #[test]
    fn test_string_literal() {
        let tokens = lex_tokens(r#""hello""#);
        assert_eq!(tokens, vec![Tok::StringLiteral("hello".to_string())]);
    }

    #[test]
    fn test_string_escape() {
        let tokens = lex_tokens(r#""a\nb""#);
        assert_eq!(tokens, vec![Tok::StringLiteral("a\nb".to_string())]);
    }

    #[test]
    fn test_reserved_uniform_varying() {
        // "uniform" and "varying" are reserved RSL keywords (C++ parity)
        let mut lexer = OslLexer::new("uniform");
        let _: Vec<_> = lexer.by_ref().filter_map(|r| r.ok()).collect();
        assert_eq!(lexer.errors.len(), 1, "uniform should error");
        assert!(lexer.errors[0].1.contains("uniform"));

        let mut lexer2 = OslLexer::new("varying");
        let _: Vec<_> = lexer2.by_ref().filter_map(|r| r.ok()).collect();
        assert_eq!(lexer2.errors.len(), 1, "varying should error");
        assert!(lexer2.errors[0].1.contains("varying"));
    }
}
