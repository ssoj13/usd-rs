//! Token scanning logic for the USDA lexer.
//!
//! This module contains the core tokenization logic that scans individual
//! tokens from the source text. Each `scan_*` function handles a specific
//! token type.
//!
//! # C++ Parity
//!
//! The scanning rules follow the PEGTL grammar in `textFileFormatParser.h`:
//! - Numbers: Integer, Number (float), inf, nan
//! - Strings: Single/double quote, single/multi-line
//! - Asset refs: @path@ and @@@path@@@
//! - Path refs: <path>
//! - Identifiers: UTF-8 identifiers with namespace support
//! - Keywords: All USD reserved words

use super::Lexer;
use super::chars;
use crate::text_parser::tokens::{Keyword, Token, TokenKind};

// ============================================================================
// Main Scanner
// ============================================================================

/// Scans the next token from the lexer.
///
/// This is the main entry point for tokenization. It examines the current
/// character and delegates to the appropriate scanning function.
pub fn scan_token(lexer: &mut Lexer) -> Option<Token> {
    let byte = lexer.peek()?;

    // Dispatch based on first character
    let token = match byte {
        // String literals
        b'"' | b'\'' => scan_string(lexer),

        // Asset reference @path@ or @@@path@@@
        b'@' => scan_asset_ref(lexer),

        // Path reference <path>
        b'<' => scan_path_ref_or_angle(lexer),

        // Number or dot
        b'0'..=b'9' => scan_number(lexer),
        b'-' => scan_number_or_minus(lexer),
        b'.' => scan_dot_or_number(lexer),

        // Identifier or keyword
        b'a'..=b'z' | b'A'..=b'Z' | b'_' => scan_identifier_or_keyword(lexer),

        // Hash - could be magic header or comment
        b'#' => scan_magic_or_comment(lexer),

        // Punctuation
        b'(' => {
            lexer.advance();
            lexer.make_token(TokenKind::LeftParen)
        }
        b')' => {
            lexer.advance();
            lexer.make_token(TokenKind::RightParen)
        }
        b'[' => {
            lexer.advance();
            lexer.make_token(TokenKind::LeftBracket)
        }
        b']' => {
            lexer.advance();
            lexer.make_token(TokenKind::RightBracket)
        }
        b'{' => {
            lexer.advance();
            lexer.make_token(TokenKind::LeftBrace)
        }
        b'}' => {
            lexer.advance();
            lexer.make_token(TokenKind::RightBrace)
        }
        b'>' => {
            lexer.advance();
            lexer.make_token(TokenKind::RightAngle)
        }
        b'=' => {
            lexer.advance();
            lexer.make_token(TokenKind::Equals)
        }
        b':' => scan_colon(lexer),
        b',' => {
            lexer.advance();
            lexer.make_token(TokenKind::Comma)
        }
        b';' => {
            lexer.advance();
            lexer.make_token(TokenKind::Semicolon)
        }
        b'&' => {
            lexer.advance();
            lexer.make_token(TokenKind::Ampersand)
        }

        // UTF-8 identifier start
        _ if byte >= 0x80 => scan_utf8_identifier(lexer),

        // Unknown character
        _ => {
            lexer.advance();
            lexer.error_token(format!("unexpected character '{}'", byte as char))
        }
    };

    Some(token)
}

// ============================================================================
// String Scanning
// ============================================================================

/// Scans a string literal (single or double quoted, single or multi-line).
fn scan_string(lexer: &mut Lexer) -> Token {
    let quote = lexer.advance().expect("quote char");
    let is_double = quote == b'"';

    // Check for triple-quoted string
    let is_triple = lexer.peek() == Some(quote) && lexer.peek_at(1) == Some(quote);
    if is_triple {
        lexer.advance();
        lexer.advance();
        return scan_multiline_string(lexer, quote);
    }

    // Single-line string
    let mut content = String::new();

    loop {
        match lexer.peek() {
            None => {
                return lexer.error_token("unterminated string");
            }
            Some(b'\n') | Some(b'\r') => {
                return lexer.error_token("unterminated string (newline in single-line string)");
            }
            Some(b) if b == quote => {
                lexer.advance();
                break;
            }
            Some(b'\\') => {
                lexer.advance();
                match lexer.peek() {
                    None => {
                        return lexer.error_token("unterminated escape sequence");
                    }
                    Some(b) if b == quote => {
                        lexer.advance();
                        content.push(if is_double { '"' } else { '\'' });
                    }
                    Some(b'\\') => {
                        lexer.advance();
                        content.push('\\');
                    }
                    Some(b'n') => {
                        lexer.advance();
                        content.push('\n');
                    }
                    Some(b'r') => {
                        lexer.advance();
                        content.push('\r');
                    }
                    Some(b't') => {
                        lexer.advance();
                        content.push('\t');
                    }
                    Some(b'0') => {
                        lexer.advance();
                        content.push('\0');
                    }
                    Some(b'x') => {
                        lexer.advance();
                        match scan_hex_escape(lexer) {
                            Some(ch) => content.push(ch),
                            None => return lexer.error_token("invalid hex escape sequence"),
                        }
                    }
                    // \uNNNN - 4-digit Unicode (BMP)
                    Some(b'u') => {
                        lexer.advance();
                        match scan_unicode_escape(lexer, 4) {
                            Some(ch) => content.push(ch),
                            None => return lexer.error_token("invalid \\u escape sequence"),
                        }
                    }
                    // \UNNNNNNNN - 8-digit Unicode (full range)
                    Some(b'U') => {
                        lexer.advance();
                        match scan_unicode_escape(lexer, 8) {
                            Some(ch) => content.push(ch),
                            None => return lexer.error_token("invalid \\U escape sequence"),
                        }
                    }
                    Some(c) => {
                        // Unknown escape, just include literally
                        lexer.advance();
                        content.push('\\');
                        content.push(c as char);
                    }
                }
            }
            Some(_) => {
                if let Some(ch) = lexer.advance_char() {
                    content.push(ch);
                }
            }
        }
    }

    lexer.make_token(TokenKind::String(content))
}

/// Scans a multi-line (triple-quoted) string.
fn scan_multiline_string(lexer: &mut Lexer, quote: u8) -> Token {
    let mut content = String::new();

    loop {
        match lexer.peek() {
            None => {
                return lexer.error_token("unterminated multi-line string");
            }
            Some(b) if b == quote => {
                // Check for closing triple quote
                if lexer.peek_at(1) == Some(quote) && lexer.peek_at(2) == Some(quote) {
                    lexer.advance();
                    lexer.advance();
                    lexer.advance();
                    break;
                }
                // Just a single quote, include it
                lexer.advance();
                content.push(quote as char);
            }
            Some(b'\\') => {
                lexer.advance();
                match lexer.peek() {
                    None => {
                        return lexer.error_token("unterminated escape sequence");
                    }
                    Some(b) if b == quote => {
                        lexer.advance();
                        content.push(quote as char);
                    }
                    Some(b'\\') => {
                        lexer.advance();
                        content.push('\\');
                    }
                    Some(b'n') => {
                        lexer.advance();
                        content.push('\n');
                    }
                    Some(b'r') => {
                        lexer.advance();
                        content.push('\r');
                    }
                    Some(b't') => {
                        lexer.advance();
                        content.push('\t');
                    }
                    // \xNN - hex escape
                    Some(b'x') => {
                        lexer.advance();
                        match scan_hex_escape(lexer) {
                            Some(ch) => content.push(ch),
                            None => return lexer.error_token("invalid hex escape sequence"),
                        }
                    }
                    // \uNNNN - 4-digit Unicode (BMP)
                    Some(b'u') => {
                        lexer.advance();
                        match scan_unicode_escape(lexer, 4) {
                            Some(ch) => content.push(ch),
                            None => return lexer.error_token("invalid \\u escape sequence"),
                        }
                    }
                    // \UNNNNNNNN - 8-digit Unicode (full range)
                    Some(b'U') => {
                        lexer.advance();
                        match scan_unicode_escape(lexer, 8) {
                            Some(ch) => content.push(ch),
                            None => return lexer.error_token("invalid \\U escape sequence"),
                        }
                    }
                    Some(_) => {
                        // Keep the backslash and next char
                        if let Some(ch) = lexer.advance_char() {
                            content.push('\\');
                            content.push(ch);
                        }
                    }
                }
            }
            Some(_) => {
                if let Some(ch) = lexer.advance_char() {
                    content.push(ch);
                }
            }
        }
    }

    lexer.make_token(TokenKind::String(content))
}

/// Scans a hex escape sequence (\xNN).
fn scan_hex_escape(lexer: &mut Lexer) -> Option<char> {
    let mut value = 0u32;

    for _ in 0..2 {
        let byte = lexer.peek()?;
        if !chars::is_hex_digit(byte) {
            return None;
        }
        lexer.advance();

        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        };

        value = value * 16 + digit as u32;
    }

    char::from_u32(value)
}

/// Scans a Unicode escape sequence.
/// - `\uNNNN`     : 4 hex digits  (BMP codepoint)
/// - `\UNNNNNNNN` : 8 hex digits  (full Unicode codepoint)
fn scan_unicode_escape(lexer: &mut Lexer, digits: usize) -> Option<char> {
    let mut value = 0u32;

    for _ in 0..digits {
        let byte = lexer.peek()?;
        if !chars::is_hex_digit(byte) {
            return None;
        }
        lexer.advance();

        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => return None,
        };

        value = value * 16 + digit as u32;
    }

    char::from_u32(value)
}

// ============================================================================
// Asset Reference Scanning
// ============================================================================

/// Scans an asset reference (@path@ or @@@path@@@).
fn scan_asset_ref(lexer: &mut Lexer) -> Token {
    lexer.advance(); // First @

    // Check for triple @
    let is_triple = lexer.peek() == Some(b'@') && lexer.peek_at(1) == Some(b'@');
    if is_triple {
        lexer.advance();
        lexer.advance();
        return scan_triple_asset_ref(lexer);
    }

    // Single @ asset ref
    let mut content = String::new();

    loop {
        match lexer.peek() {
            None => {
                return lexer.error_token("unterminated asset reference");
            }
            Some(b'@') => {
                lexer.advance();
                break;
            }
            Some(b'\n') | Some(b'\r') => {
                return lexer.error_token("newline in asset reference");
            }
            Some(_) => {
                if let Some(ch) = lexer.advance_char() {
                    content.push(ch);
                }
            }
        }
    }

    lexer.make_token(TokenKind::AssetRef(content))
}

/// Scans a triple-quoted asset reference (@@@path@@@).
fn scan_triple_asset_ref(lexer: &mut Lexer) -> Token {
    let mut content = String::new();

    loop {
        match lexer.peek() {
            None => {
                return lexer.error_token("unterminated asset reference");
            }
            Some(b'\\') => {
                // Check for escaped @@@
                if lexer.peek_at(1) == Some(b'@')
                    && lexer.peek_at(2) == Some(b'@')
                    && lexer.peek_at(3) == Some(b'@')
                {
                    lexer.advance(); // Skip backslash
                    content.push('@');
                    content.push('@');
                    content.push('@');
                    lexer.advance();
                    lexer.advance();
                    lexer.advance();
                } else if let Some(ch) = lexer.advance_char() {
                    content.push(ch);
                }
            }
            Some(b'@') => {
                // Check for closing @@@
                if lexer.peek_at(1) == Some(b'@') && lexer.peek_at(2) == Some(b'@') {
                    // Could have trailing @ (up to 2)
                    lexer.advance();
                    lexer.advance();
                    lexer.advance();

                    // Handle trailing @ that are part of path
                    // Per spec: @@@ closes, with 0-2 extra @ being part of path
                    let mut extra_at = 0;
                    while extra_at < 2 && lexer.peek() == Some(b'@') {
                        content.push('@');
                        lexer.advance();
                        extra_at += 1;
                    }
                    break;
                }
                // Single @ in content
                lexer.advance();
                content.push('@');
            }
            Some(_) => {
                if let Some(ch) = lexer.advance_char() {
                    content.push(ch);
                }
            }
        }
    }

    lexer.make_token(TokenKind::AssetRef(content))
}

// ============================================================================
// Path Reference Scanning
// ============================================================================

/// Scans a path reference (<path>) or just a left angle bracket.
fn scan_path_ref_or_angle(lexer: &mut Lexer) -> Token {
    lexer.advance(); // <

    // Check for empty path <>
    if lexer.peek() == Some(b'>') {
        lexer.advance();
        return lexer.make_token(TokenKind::PathRef(String::new()));
    }

    // Check if this looks like a path
    match lexer.peek() {
        Some(b'/') | Some(b'.') | Some(b'a'..=b'z') | Some(b'A'..=b'Z') | Some(b'_') => {
            scan_path_ref_content(lexer)
        }
        _ => {
            // Just a left angle bracket (used in comparisons, etc.)
            lexer.make_token(TokenKind::LeftAngle)
        }
    }
}

/// Scans the content of a path reference after the opening <.
fn scan_path_ref_content(lexer: &mut Lexer) -> Token {
    let mut content = String::new();

    loop {
        match lexer.peek() {
            None => {
                return lexer.error_token("unterminated path reference");
            }
            Some(b'>') => {
                lexer.advance();
                break;
            }
            Some(b'\n') | Some(b'\r') => {
                return lexer.error_token("newline in path reference");
            }
            Some(_) => {
                if let Some(ch) = lexer.advance_char() {
                    content.push(ch);
                }
            }
        }
    }

    lexer.make_token(TokenKind::PathRef(content))
}

// ============================================================================
// Number Scanning
// ============================================================================

/// Scans a number (integer or float).
fn scan_number(lexer: &mut Lexer) -> Token {
    // Digits never contain newlines — use fast path
    lexer.advance_while_no_newline(chars::is_digit);

    let mut is_float = false;

    // Check for decimal point
    if lexer.peek() == Some(b'.') {
        if let Some(next) = lexer.peek_at(1) {
            if chars::is_digit(next) {
                is_float = true;
                lexer.advance_bytes_no_newline(1); // .
                lexer.advance_while_no_newline(chars::is_digit);
            }
        }
    }

    // Check for exponent
    if let Some(b'e' | b'E') = lexer.peek() {
        is_float = true;
        lexer.advance_bytes_no_newline(1); // e/E

        if let Some(b'+' | b'-') = lexer.peek() {
            lexer.advance_bytes_no_newline(1);
        }

        lexer.advance_while_no_newline(chars::is_digit);
    }

    let text = lexer.token_text();

    if is_float {
        match text.parse::<f64>() {
            Ok(value) => lexer.make_token(TokenKind::Float(value)),
            Err(_) => lexer.error_token(format!("invalid float: {}", text)),
        }
    } else {
        match text.parse::<i64>() {
            Ok(value) => lexer.make_token(TokenKind::Integer(value)),
            Err(_) => match text.parse::<f64>() {
                Ok(value) => lexer.make_token(TokenKind::Float(value)),
                Err(_) => lexer.error_token(format!("invalid number: {}", text)),
            },
        }
    }
}

/// Scans a number starting with minus, or just minus.
fn scan_number_or_minus(lexer: &mut Lexer) -> Token {
    lexer.advance(); // -

    match lexer.peek() {
        // -inf
        Some(b'i') if lexer.remaining().starts_with("inf") => {
            lexer.advance(); // i
            lexer.advance(); // n
            lexer.advance(); // f
            lexer.make_token(TokenKind::NegInf)
        }
        // Negative number
        Some(b) if chars::is_digit(b) || b == b'.' => scan_number_after_minus(lexer),
        // Just minus (rare, but handle it)
        _ => lexer.error_token("unexpected minus sign"),
    }
}

/// Scans a number after the minus sign has been consumed.
fn scan_number_after_minus(lexer: &mut Lexer) -> Token {
    lexer.advance_while_no_newline(chars::is_digit);

    let mut is_float = false;

    if lexer.peek() == Some(b'.') {
        if let Some(next) = lexer.peek_at(1) {
            if chars::is_digit(next) {
                is_float = true;
                lexer.advance_bytes_no_newline(1); // .
                lexer.advance_while_no_newline(chars::is_digit);
            }
        }
    }

    if let Some(b'e' | b'E') = lexer.peek() {
        is_float = true;
        lexer.advance_bytes_no_newline(1);

        if let Some(b'+' | b'-') = lexer.peek() {
            lexer.advance_bytes_no_newline(1);
        }

        lexer.advance_while_no_newline(chars::is_digit);
    }

    let text = lexer.token_text();

    if is_float {
        match text.parse::<f64>() {
            Ok(value) => lexer.make_token(TokenKind::Float(value)),
            Err(_) => lexer.error_token(format!("invalid float: {}", text)),
        }
    } else {
        match text.parse::<i64>() {
            Ok(value) => lexer.make_token(TokenKind::Integer(value)),
            Err(_) => lexer.error_token(format!("invalid integer: {}", text)),
        }
    }
}

/// Scans a dot (could be number like .5 or just dot).
fn scan_dot_or_number(lexer: &mut Lexer) -> Token {
    if let Some(next) = lexer.peek_at(1) {
        if chars::is_digit(next) {
            lexer.advance_bytes_no_newline(1); // .
            lexer.advance_while_no_newline(chars::is_digit);

            if let Some(b'e' | b'E') = lexer.peek() {
                lexer.advance_bytes_no_newline(1);
                if let Some(b'+' | b'-') = lexer.peek() {
                    lexer.advance_bytes_no_newline(1);
                }
                lexer.advance_while_no_newline(chars::is_digit);
            }

            let text = lexer.token_text();
            match text.parse::<f64>() {
                Ok(value) => return lexer.make_token(TokenKind::Float(value)),
                Err(_) => return lexer.error_token(format!("invalid float: {}", text)),
            }
        }
    }

    lexer.advance();
    lexer.make_token(TokenKind::Dot)
}

// ============================================================================
// Identifier and Keyword Scanning
// ============================================================================

/// Scans an identifier or keyword.
fn scan_identifier_or_keyword(lexer: &mut Lexer) -> Token {
    // Scan the identifier
    lexer.advance_while_char(chars::is_identifier_continue);

    let text = lexer.token_text();

    // Check for special values
    match text {
        "inf" => return lexer.make_token(TokenKind::Inf),
        "nan" => return lexer.make_token(TokenKind::Nan),
        _ => {}
    }

    // Check if it's a keyword
    if let Some(keyword) = Keyword::from_str(text) {
        return lexer.make_token(TokenKind::Keyword(keyword));
    }

    // It's an identifier
    lexer.make_token(TokenKind::Identifier(text.to_string()))
}

/// Scans a UTF-8 identifier starting with a non-ASCII character.
fn scan_utf8_identifier(lexer: &mut Lexer) -> Token {
    // Check if first char is valid identifier start
    if let Some(ch) = lexer.peek_char() {
        if !chars::is_identifier_start(ch) {
            lexer.advance_char();
            return lexer.error_token(format!("invalid character in identifier: {}", ch));
        }
    }

    // Scan the rest of the identifier
    lexer.advance_while_char(chars::is_identifier_continue);

    let text = lexer.token_text();

    // UTF-8 identifiers can't be keywords (keywords are ASCII)
    lexer.make_token(TokenKind::Identifier(text.to_string()))
}

// ============================================================================
// Colon Scanning
// ============================================================================

/// Scans a colon (could be single : or double ::).
fn scan_colon(lexer: &mut Lexer) -> Token {
    lexer.advance(); // First :

    if lexer.peek() == Some(b':') {
        lexer.advance();
        lexer.make_token(TokenKind::DoubleColon)
    } else {
        lexer.make_token(TokenKind::Colon)
    }
}

// ============================================================================
// Magic Header Scanning
// ============================================================================

/// Scans the magic header (#usda or #sdf) at start of file.
fn scan_magic_or_comment(lexer: &mut Lexer) -> Token {
    lexer.advance(); // #

    // Check for magic header: #usda or #sdf
    if let Some(ch) = lexer.peek_char() {
        if chars::is_identifier_start(ch) {
            let format_start = lexer.token_start + 1; // Skip #
            lexer.advance_while_char(chars::is_identifier_continue);

            let text = lexer.token_text();
            let format = &text[1..]; // Skip #

            // Check if this is a magic identifier (usda or sdf at start of file)
            if (format == "usda" || format == "sdf") && format_start == 1 {
                // Skip whitespace to find version
                lexer.advance_while(|b| b == b' ' || b == b'\t');

                // Read version number
                let version_start = lexer.remaining();
                lexer.advance_while(|b| b.is_ascii_digit() || b == b'.');

                let version_len = version_start.len() - lexer.remaining().len();
                let version = version_start[..version_len].to_string();

                if !version.is_empty() {
                    return lexer.make_token(TokenKind::Magic {
                        format: format.to_string(),
                        version,
                    });
                }
            }

            // Not a magic header, return as identifier
            return lexer.make_token(TokenKind::Identifier(text.to_string()));
        }
    }

    // Not a magic header, skip rest of line as comment
    while let Some(b) = lexer.peek() {
        if b == b'\n' {
            break;
        }
        lexer.advance();
    }

    // Return newline or try again
    if let Some(b'\n') = lexer.peek() {
        lexer.advance();
        lexer.make_token(TokenKind::Newline)
    } else {
        lexer.make_token(TokenKind::Eof)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(source: &str) -> Vec<Token> {
        Lexer::new(source).collect()
    }

    fn token_kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source).into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn test_scan_integer() {
        let tokens = token_kinds("42");
        assert!(matches!(tokens[0], TokenKind::Integer(42)));
    }

    #[test]
    fn test_scan_negative_integer() {
        let tokens = token_kinds("-42");
        assert!(matches!(tokens[0], TokenKind::Integer(-42)));
    }

    #[test]
    fn test_scan_float() {
        let tokens = token_kinds("3.14");
        assert!(matches!(tokens[0], TokenKind::Float(f) if (f - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_scan_float_exponent() {
        let tokens = token_kinds("1e10");
        assert!(matches!(tokens[0], TokenKind::Float(_)));
    }

    #[test]
    fn test_scan_inf() {
        let tokens = token_kinds("inf -inf nan");
        assert!(matches!(tokens[0], TokenKind::Inf));
        assert!(matches!(tokens[1], TokenKind::NegInf));
        assert!(matches!(tokens[2], TokenKind::Nan));
    }

    #[test]
    fn test_scan_string() {
        let tokens = token_kinds(r#""hello""#);
        assert!(matches!(&tokens[0], TokenKind::String(s) if s == "hello"));
    }

    #[test]
    fn test_scan_string_escape() {
        let tokens = token_kinds(r#""hello\nworld""#);
        assert!(matches!(&tokens[0], TokenKind::String(s) if s == "hello\nworld"));
    }

    #[test]
    fn test_scan_triple_string() {
        let tokens = token_kinds(
            r#""""multi
line""""#,
        );
        assert!(matches!(&tokens[0], TokenKind::String(s) if s.contains("multi")));
    }

    #[test]
    fn test_scan_asset_ref() {
        let tokens = token_kinds("@./path/to/file.usd@");
        assert!(matches!(&tokens[0], TokenKind::AssetRef(s) if s == "./path/to/file.usd"));
    }

    #[test]
    fn test_scan_path_ref() {
        let tokens = token_kinds("</World/Cube>");
        assert!(matches!(&tokens[0], TokenKind::PathRef(s) if s == "/World/Cube"));
    }

    #[test]
    fn test_scan_empty_path_ref() {
        let tokens = token_kinds("<>");
        assert!(matches!(&tokens[0], TokenKind::PathRef(s) if s.is_empty()));
    }

    #[test]
    fn test_scan_identifier() {
        let tokens = token_kinds("myIdentifier");
        assert!(matches!(&tokens[0], TokenKind::Identifier(s) if s == "myIdentifier"));
    }

    #[test]
    fn test_scan_keyword() {
        let tokens = token_kinds("def over class");
        assert!(matches!(tokens[0], TokenKind::Keyword(Keyword::Def)));
        assert!(matches!(tokens[1], TokenKind::Keyword(Keyword::Over)));
        assert!(matches!(tokens[2], TokenKind::Keyword(Keyword::Class)));
    }

    #[test]
    fn test_scan_punctuation() {
        let tokens = token_kinds("()[]{}=:,;.");
        assert!(matches!(tokens[0], TokenKind::LeftParen));
        assert!(matches!(tokens[1], TokenKind::RightParen));
        assert!(matches!(tokens[2], TokenKind::LeftBracket));
        assert!(matches!(tokens[3], TokenKind::RightBracket));
        assert!(matches!(tokens[4], TokenKind::LeftBrace));
        assert!(matches!(tokens[5], TokenKind::RightBrace));
        assert!(matches!(tokens[6], TokenKind::Equals));
        assert!(matches!(tokens[7], TokenKind::Colon));
        assert!(matches!(tokens[8], TokenKind::Comma));
        assert!(matches!(tokens[9], TokenKind::Semicolon));
        assert!(matches!(tokens[10], TokenKind::Dot));
    }

    #[test]
    fn test_scan_double_colon() {
        let tokens = token_kinds("Foo::Bar");
        assert!(matches!(&tokens[0], TokenKind::Identifier(s) if s == "Foo"));
        assert!(matches!(tokens[1], TokenKind::DoubleColon));
        assert!(matches!(&tokens[2], TokenKind::Identifier(s) if s == "Bar"));
    }

    #[test]
    fn test_skip_line_comment() {
        let tokens = token_kinds("foo // comment\nbar");
        assert!(matches!(&tokens[0], TokenKind::Identifier(s) if s == "foo"));
        assert!(matches!(&tokens[1], TokenKind::Identifier(s) if s == "bar"));
    }

    #[test]
    fn test_skip_block_comment() {
        let tokens = token_kinds("foo /* comment */ bar");
        assert!(matches!(&tokens[0], TokenKind::Identifier(s) if s == "foo"));
        assert!(matches!(&tokens[1], TokenKind::Identifier(s) if s == "bar"));
    }

    #[test]
    fn test_full_usda_header() {
        let tokens = token_kinds("#usda 1.0");
        // #usda at file start becomes Magic token
        assert!(matches!(&tokens[0], TokenKind::Magic { format, version }
            if format == "usda" && version == "1.0"));
    }

    #[test]
    fn test_sdf_header() {
        let tokens = token_kinds("#sdf 1.4.32");
        assert!(matches!(&tokens[0], TokenKind::Magic { format, version }
            if format == "sdf" && version == "1.4.32"));
    }

    #[test]
    fn test_string_unicode_escape_4digit() {
        // \u0041 = 'A' (USDA string "\u0041" should decode to "A")
        let tokens = token_kinds("\"\\u0041\"");
        assert!(matches!(&tokens[0], TokenKind::String(s) if s == "A"));
    }

    #[test]
    fn test_string_unicode_escape_4digit_snowman() {
        // \u2603 = snowman U+2603
        let tokens = token_kinds("\"\\u2603\"");
        assert!(matches!(&tokens[0], TokenKind::String(s) if s == "\u{2603}"));
    }

    #[test]
    fn test_string_unicode_escape_8digit() {
        // \U00010000 = first char outside BMP (Linear B Syllabary)
        let tokens = token_kinds("\"\\U00010000\"");
        assert!(matches!(&tokens[0], TokenKind::String(s) if s == "\u{10000}"));
    }

    #[test]
    fn test_multiline_string_unicode_escape() {
        // Unicode inside triple-quoted string ("""\u0041""")
        let tokens = token_kinds("\"\"\"\\u0041\"\"\"");
        assert!(matches!(&tokens[0], TokenKind::String(s) if s == "A"));
    }
}
