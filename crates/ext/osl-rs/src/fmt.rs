//! Encoded format/message system matching OSL's `opfmt.cpp`.
//!
//! OSL uses an encoded type system for format string arguments to allow
//! thread-safe, deferred formatting. This module implements the encoding
//! and decoding of typed arguments for printf/fprintf/error/warning/format.

use crate::ustring::UString;

/// Encoded type tag for format arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncodedType {
    /// No more arguments.
    End = 0,
    /// 32-bit integer.
    Int32 = 1,
    /// 32-bit unsigned integer.
    UInt32 = 2,
    /// 64-bit integer.
    Int64 = 3,
    /// 64-bit unsigned integer.
    UInt64 = 4,
    /// 32-bit float.
    Float = 5,
    /// 64-bit double.
    Double = 6,
    /// String (pointer).
    String = 7,
    /// Pointer/address.
    Ptr = 8,
    /// ustringhash (64-bit).
    UStringHash = 9,
}

impl EncodedType {
    /// Convert from u8.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::End),
            1 => Some(Self::Int32),
            2 => Some(Self::UInt32),
            3 => Some(Self::Int64),
            4 => Some(Self::UInt64),
            5 => Some(Self::Float),
            6 => Some(Self::Double),
            7 => Some(Self::String),
            8 => Some(Self::Ptr),
            9 => Some(Self::UStringHash),
            _ => None,
        }
    }

    /// Size in bytes of this encoded type.
    pub fn size(self) -> usize {
        match self {
            Self::End => 0,
            Self::Int32 | Self::UInt32 | Self::Float => 4,
            Self::Int64 | Self::UInt64 | Self::Double | Self::Ptr | Self::UStringHash => 8,
            Self::String => std::mem::size_of::<usize>(),
        }
    }
}

// ---------------------------------------------------------------------------
// Format argument packing
// ---------------------------------------------------------------------------

/// A packed argument buffer for deferred formatting.
#[derive(Debug, Clone, Default)]
pub struct FormatArgs {
    /// Encoded type tags.
    pub types: Vec<EncodedType>,
    /// Packed argument data.
    pub data: Vec<u8>,
}

impl FormatArgs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an i32 argument.
    pub fn push_int(&mut self, v: i32) {
        self.types.push(EncodedType::Int32);
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    /// Push a u32 argument.
    pub fn push_uint(&mut self, v: u32) {
        self.types.push(EncodedType::UInt32);
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    /// Push an i64 argument.
    pub fn push_int64(&mut self, v: i64) {
        self.types.push(EncodedType::Int64);
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    /// Push a u64 argument.
    pub fn push_uint64(&mut self, v: u64) {
        self.types.push(EncodedType::UInt64);
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    /// Push an f32 argument.
    pub fn push_float(&mut self, v: f32) {
        self.types.push(EncodedType::Float);
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    /// Push an f64 argument.
    pub fn push_double(&mut self, v: f64) {
        self.types.push(EncodedType::Double);
        self.data.extend_from_slice(&v.to_le_bytes());
    }

    /// Push a UStringHash argument.
    pub fn push_ustringhash(&mut self, hash: u64) {
        self.types.push(EncodedType::UStringHash);
        self.data.extend_from_slice(&hash.to_le_bytes());
    }

    /// Number of arguments.
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Format argument unpacking / formatting
// ---------------------------------------------------------------------------

/// Parse optional flags, width, precision from a printf-style format spec.
/// Consumes chars after '%' up to (but not including) the conversion char.
/// Returns (flags, width, precision).
fn parse_fmt_spec(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> (String, Option<usize>, Option<usize>) {
    let mut flags = String::new();
    // Flags: '-' '+' ' ' '0' '#'
    while let Some(&c) = chars.peek() {
        if "-+ 0#".contains(c) {
            flags.push(c);
            chars.next();
        } else {
            break;
        }
    }
    // Width
    let mut width = None;
    let mut w = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            w.push(c);
            chars.next();
        } else {
            break;
        }
    }
    if !w.is_empty() {
        width = w.parse().ok();
    }
    // Precision
    let mut prec = None;
    if chars.peek() == Some(&'.') {
        chars.next();
        let mut p = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_ascii_digit() {
                p.push(c);
                chars.next();
            } else {
                break;
            }
        }
        prec = Some(p.parse::<usize>().unwrap_or(0));
    }
    (flags, width, prec)
}

/// Apply width/precision/flags to a formatted value string.
fn apply_fmt_spec(val: &str, flags: &str, width: Option<usize>, _prec: Option<usize>) -> String {
    let w = width.unwrap_or(0);
    if w == 0 || val.len() >= w {
        return val.to_string();
    }
    let pad = w - val.len();
    if flags.contains('-') {
        // Left-align
        format!("{}{}", val, " ".repeat(pad))
    } else if flags.contains('0') && !val.starts_with('-') {
        // Zero-pad (only for numeric)
        format!("{}{}", "0".repeat(pad), val)
    } else {
        format!("{}{}", " ".repeat(pad), val)
    }
}

/// Unpack and format arguments according to a format string.
/// Supports: %[flags][width][.prec]d/u/f/g/e/s/x/o/%%.
pub fn format_encoded(fmt: &str, args: &FormatArgs) -> String {
    let mut result = String::new();
    let mut chars = fmt.chars().peekable();
    let mut arg_idx = 0;
    let mut data_offset = 0;

    while let Some(c) = chars.next() {
        if c == '%' {
            if chars.peek() == Some(&'%') {
                chars.next();
                result.push('%');
                continue;
            }
            let (flags, width, prec) = parse_fmt_spec(&mut chars);
            if let Some(&spec) = chars.peek() {
                chars.next();
                match spec {
                    'd' | 'i' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_i64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let s = val.to_string();
                            result.push_str(&apply_fmt_spec(&s, &flags, width, prec));
                            arg_idx += 1;
                        }
                    }
                    'u' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_u64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let s = val.to_string();
                            result.push_str(&apply_fmt_spec(&s, &flags, width, prec));
                            arg_idx += 1;
                        }
                    }
                    'f' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_f64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let p = prec.unwrap_or(6);
                            let s = format!("{val:.p$}");
                            result.push_str(&apply_fmt_spec(&s, &flags, width, None));
                            arg_idx += 1;
                        }
                    }
                    'e' | 'E' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_f64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let p = prec.unwrap_or(6);
                            let raw = format!("{val:.p$e}");
                            // Convert to C-style exponent: e+02, e-03
                            let s = if let Some(epos) = raw.find('e') {
                                let m = &raw[..epos];
                                let e: i32 = raw[epos + 1..].parse().unwrap_or(0);
                                let ec = if spec == 'E' { 'E' } else { 'e' };
                                if e.abs() < 100 {
                                    format!("{m}{ec}{e:+03}")
                                } else {
                                    format!("{m}{ec}{e:+}")
                                }
                            } else {
                                raw
                            };
                            result.push_str(&apply_fmt_spec(&s, &flags, width, None));
                            arg_idx += 1;
                        }
                    }
                    'g' | 'G' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_f64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let s = fmt_g(val, prec.unwrap_or(6), spec == 'G', flags.contains('#'));
                            result.push_str(&apply_fmt_spec(&s, &flags, width, None));
                            arg_idx += 1;
                        }
                    }
                    'x' | 'X' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_u64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let s = if spec == 'X' {
                                format!("{val:X}")
                            } else {
                                format!("{val:x}")
                            };
                            result.push_str(&apply_fmt_spec(&s, &flags, width, prec));
                            arg_idx += 1;
                        }
                    }
                    'o' => {
                        if arg_idx < args.types.len() {
                            let val =
                                read_as_u64(&args.data, &mut data_offset, args.types[arg_idx]);
                            let s = format!("{val:o}");
                            result.push_str(&apply_fmt_spec(&s, &flags, width, prec));
                            arg_idx += 1;
                        }
                    }
                    's' => {
                        if arg_idx < args.types.len() {
                            match args.types[arg_idx] {
                                EncodedType::UStringHash => {
                                    let hash = read_u64_le(&args.data, &mut data_offset);
                                    let sv = if let Some(u) = UString::from_hash(hash) {
                                        u.as_str().to_string()
                                    } else {
                                        "<unknown>".to_string()
                                    };
                                    // Apply precision as max chars for strings
                                    let sv = if let Some(p) = prec {
                                        sv.chars().take(p).collect()
                                    } else {
                                        sv
                                    };
                                    result.push_str(&apply_fmt_spec(&sv, &flags, width, None));
                                    arg_idx += 1;
                                }
                                _ => {
                                    data_offset += args.types[arg_idx].size();
                                    result.push_str("<?>");
                                    arg_idx += 1;
                                }
                            }
                        }
                    }
                    _ => {
                        // Unknown specifier - pass through
                        result.push('%');
                        result.push(spec);
                    }
                }
            } else {
                result.push('%');
            }
        } else {
            result.push(c);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Data reading helpers
// ---------------------------------------------------------------------------

fn read_i32_le(data: &[u8], offset: &mut usize) -> i32 {
    if *offset + 4 > data.len() {
        return 0;
    }
    let bytes: [u8; 4] = data[*offset..*offset + 4].try_into().unwrap_or([0; 4]);
    *offset += 4;
    i32::from_le_bytes(bytes)
}

fn read_u32_le(data: &[u8], offset: &mut usize) -> u32 {
    if *offset + 4 > data.len() {
        return 0;
    }
    let bytes: [u8; 4] = data[*offset..*offset + 4].try_into().unwrap_or([0; 4]);
    *offset += 4;
    u32::from_le_bytes(bytes)
}

fn read_i64_le(data: &[u8], offset: &mut usize) -> i64 {
    if *offset + 8 > data.len() {
        return 0;
    }
    let bytes: [u8; 8] = data[*offset..*offset + 8].try_into().unwrap_or([0; 8]);
    *offset += 8;
    i64::from_le_bytes(bytes)
}

fn read_u64_le(data: &[u8], offset: &mut usize) -> u64 {
    if *offset + 8 > data.len() {
        return 0;
    }
    let bytes: [u8; 8] = data[*offset..*offset + 8].try_into().unwrap_or([0; 8]);
    *offset += 8;
    u64::from_le_bytes(bytes)
}

fn read_f32_le(data: &[u8], offset: &mut usize) -> f32 {
    if *offset + 4 > data.len() {
        return 0.0;
    }
    let bytes: [u8; 4] = data[*offset..*offset + 4].try_into().unwrap_or([0; 4]);
    *offset += 4;
    f32::from_le_bytes(bytes)
}

fn read_f64_le(data: &[u8], offset: &mut usize) -> f64 {
    if *offset + 8 > data.len() {
        return 0.0;
    }
    let bytes: [u8; 8] = data[*offset..*offset + 8].try_into().unwrap_or([0; 8]);
    *offset += 8;
    f64::from_le_bytes(bytes)
}

fn read_as_i64(data: &[u8], offset: &mut usize, ty: EncodedType) -> i64 {
    match ty {
        EncodedType::Int32 => read_i32_le(data, offset) as i64,
        EncodedType::UInt32 => read_u32_le(data, offset) as i64,
        EncodedType::Int64 => read_i64_le(data, offset),
        EncodedType::UInt64 => read_u64_le(data, offset) as i64,
        EncodedType::Float => read_f32_le(data, offset) as i64,
        EncodedType::Double => read_f64_le(data, offset) as i64,
        _ => {
            *offset += ty.size();
            0
        }
    }
}

fn read_as_u64(data: &[u8], offset: &mut usize, ty: EncodedType) -> u64 {
    match ty {
        EncodedType::Int32 => read_i32_le(data, offset) as u64,
        EncodedType::UInt32 => read_u32_le(data, offset) as u64,
        EncodedType::Int64 => read_i64_le(data, offset) as u64,
        EncodedType::UInt64 => read_u64_le(data, offset),
        EncodedType::Float => read_f32_le(data, offset) as u64,
        EncodedType::Double => read_f64_le(data, offset) as u64,
        _ => {
            *offset += ty.size();
            0
        }
    }
}

fn read_as_f64(data: &[u8], offset: &mut usize, ty: EncodedType) -> f64 {
    match ty {
        EncodedType::Int32 => read_i32_le(data, offset) as f64,
        EncodedType::UInt32 => read_u32_le(data, offset) as f64,
        EncodedType::Int64 => read_i64_le(data, offset) as f64,
        EncodedType::UInt64 => read_u64_le(data, offset) as f64,
        EncodedType::Float => read_f32_le(data, offset) as f64,
        EncodedType::Double => read_f64_le(data, offset),
        _ => {
            *offset += ty.size();
            0.0
        }
    }
}

/// Format a float using C's `%g`/`%G` rules.
///
/// - `sig` = number of significant digits (default 6, minimum 1).
/// - Use `%e` style when exponent < -4 or >= sig; otherwise `%f` style.
/// - Strip trailing zeros (and trailing dot) unless `alt` (`#` flag) is set.
/// - Exponent uses C-style notation: `e+02`, `e-03`.
pub fn fmt_g(val: f64, sig: usize, upper: bool, alt: bool) -> String {
    let sig = sig.max(1);
    if val.is_nan() {
        return "nan".to_string();
    }
    if val.is_infinite() {
        return if val > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if val == 0.0 {
        return if alt {
            let mut s = if val.is_sign_negative() {
                "-0.".to_string()
            } else {
                "0.".to_string()
            };
            for _ in 1..sig {
                s.push('0');
            }
            s
        } else if val.is_sign_negative() {
            "-0".to_string()
        } else {
            "0".to_string()
        };
    }
    let abs = val.abs();
    let exp = abs.log10().floor() as i32;
    if exp < -4 || exp >= sig as i32 {
        // %e style with C-style exponent
        let p = sig - 1;
        let s = format!("{val:.p$e}");
        let (mantissa, exp_val) = if let Some(epos) = s.find('e') {
            let m = &s[..epos];
            let e: i32 = s[epos + 1..].parse().unwrap_or(0);
            (m.to_string(), e)
        } else {
            return s;
        };
        let m = if !alt {
            mantissa
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        } else {
            mantissa
        };
        let e_char = if upper { 'E' } else { 'e' };
        if exp_val.abs() < 100 {
            format!("{m}{e_char}{exp_val:+03}")
        } else {
            format!("{m}{e_char}{exp_val:+}")
        }
    } else {
        // %f style
        let p = if sig as i32 > exp + 1 {
            (sig as i32 - exp - 1) as usize
        } else {
            0
        };
        let s = format!("{val:.p$}");
        if !alt && s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else {
            s
        }
    }
}

// ---------------------------------------------------------------------------
// Message passing -- canonical implementation in message.rs
// ---------------------------------------------------------------------------

// Re-export MessageStore from message module (single source of truth).
pub use crate::message::MessageStore;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_args_int() {
        let mut args = FormatArgs::new();
        args.push_int(42);
        args.push_float(3.14);

        let result = format_encoded("val=%d pi=%f", &args);
        assert!(result.contains("val=42"));
        assert!(result.contains("pi="));
    }

    #[test]
    fn test_format_args_hex() {
        let mut args = FormatArgs::new();
        args.push_uint(255);
        let result = format_encoded("0x%x", &args);
        assert_eq!(result, "0xff");
    }

    #[test]
    fn test_format_percent() {
        let result = format_encoded("100%%", &FormatArgs::new());
        assert_eq!(result, "100%");
    }

    #[test]
    fn test_encoded_type_size() {
        assert_eq!(EncodedType::Int32.size(), 4);
        assert_eq!(EncodedType::Float.size(), 4);
        assert_eq!(EncodedType::Double.size(), 8);
        assert_eq!(EncodedType::Int64.size(), 8);
    }

    #[test]
    fn test_format_width_precision() {
        let mut args = FormatArgs::new();
        args.push_float(3.14159);
        // %8.3f -> 8-wide, 3 decimals
        let result = format_encoded("%8.3f", &args);
        assert_eq!(result, "   3.142");
    }

    #[test]
    fn test_format_left_align() {
        let mut args = FormatArgs::new();
        args.push_int(42);
        // %-10d -> left-aligned, 10-wide
        let result = format_encoded("%-10d", &args);
        assert_eq!(result, "42        ");
    }

    #[test]
    fn test_format_zero_pad() {
        let mut args = FormatArgs::new();
        args.push_int(42);
        // %05d -> zero-padded, 5-wide
        let result = format_encoded("%05d", &args);
        assert_eq!(result, "00042");
    }

    #[test]
    fn test_message_store() {
        use crate::message::MessageValue;

        let mut store = MessageStore::new();
        let name = UString::new("test_msg");
        store.setmessage(name, MessageValue::Float(1.0));
        assert_eq!(store.count(), 1);

        assert!(store.has_message(name));

        // Overwrite
        store.setmessage(name, MessageValue::Int(42));
        assert_eq!(store.count(), 1);

        store.clear();
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_fmt_g_basic() {
        assert_eq!(fmt_g(0.7, 6, false, false), "0.7");
        assert_eq!(fmt_g(0.2, 6, false, false), "0.2");
        assert_eq!(fmt_g(0.9, 6, false, false), "0.9");
        assert_eq!(fmt_g(3.5, 6, false, false), "3.5");
        assert_eq!(fmt_g(0.14, 6, false, false), "0.14");
        assert_eq!(fmt_g(-0.7, 6, false, false), "-0.7");
        assert_eq!(fmt_g(2.85714, 6, false, false), "2.85714");
    }

    #[test]
    fn test_fmt_g_zero() {
        assert_eq!(fmt_g(0.0, 6, false, false), "0");
        assert_eq!(fmt_g(-0.0_f64, 6, false, false), "-0");
    }

    #[test]
    fn test_fmt_g_scientific() {
        assert_eq!(fmt_g(0.00001, 6, false, false), "1e-05");
        assert_eq!(fmt_g(1000000.0, 6, false, false), "1e+06");
        assert_eq!(fmt_g(1.23456e10, 6, false, false), "1.23456e+10");
    }

    #[test]
    fn test_fmt_g_specials() {
        assert_eq!(fmt_g(f64::NAN, 6, false, false), "nan");
        assert_eq!(fmt_g(f64::INFINITY, 6, false, false), "inf");
        assert_eq!(fmt_g(f64::NEG_INFINITY, 6, false, false), "-inf");
    }

    #[test]
    fn test_fmt_g_alt_flag() {
        assert_eq!(fmt_g(1.0, 6, false, true), "1.00000");
    }

    #[test]
    fn test_format_g_via_encoded() {
        let mut args = FormatArgs::new();
        args.push_float(0.7);
        args.push_float(0.2);
        args.push_float(0.7_f32 + 0.2_f32);
        let result = format_encoded("%g + %g = %g", &args);
        assert_eq!(result, "0.7 + 0.2 = 0.9");
    }
}
