//! Parser utilities.
//! Reference: `_ref/draco/src/draco/io/parser_utils.h` + `.cc`.

use crate::core::decoder_buffer::DecoderBuffer;

pub fn skip_characters(buffer: &mut DecoderBuffer, skip_chars: &str) {
    if skip_chars.is_empty() {
        return;
    }
    let skip = skip_chars.as_bytes();
    let mut c: u8 = 0;
    while buffer.peek(&mut c) {
        if skip.iter().any(|s| *s == c) {
            buffer.advance(1);
        } else {
            return;
        }
    }
}

pub fn skip_whitespace(buffer: &mut DecoderBuffer) {
    let mut end_reached = false;
    while peek_whitespace(buffer, &mut end_reached) && !end_reached {
        buffer.advance(1);
    }
}

pub fn peek_whitespace(buffer: &mut DecoderBuffer, end_reached: &mut bool) -> bool {
    let mut c: u8 = 0;
    if !buffer.peek(&mut c) {
        *end_reached = true;
        return false;
    }
    if !(c as char).is_ascii_whitespace() {
        return false;
    }
    true
}

pub fn skip_line(buffer: &mut DecoderBuffer) {
    parse_line(buffer, None);
}

pub fn parse_float(buffer: &mut DecoderBuffer, value: &mut f32) -> bool {
    let mut ch: u8 = 0;
    if !buffer.peek(&mut ch) {
        return false;
    }
    let mut sign = get_sign_value(ch as char);
    if sign != 0 {
        buffer.advance(1);
    } else {
        sign = 1;
    }

    let mut have_digits = false;
    let mut v: f64 = 0.0;
    while buffer.peek(&mut ch) && ch >= b'0' && ch <= b'9' {
        v *= 10.0;
        v += (ch - b'0') as f64;
        buffer.advance(1);
        have_digits = true;
    }
    if ch == b'.' {
        buffer.advance(1);
        let mut fraction = 1.0f64;
        while buffer.peek(&mut ch) && ch >= b'0' && ch <= b'9' {
            fraction *= 0.1;
            v += ((ch - b'0') as f64) * fraction;
            buffer.advance(1);
            have_digits = true;
        }
    }

    if !have_digits {
        let mut text = String::new();
        if !parse_string(buffer, &mut text) {
            return false;
        }
        if text == "inf" || text == "Inf" {
            v = f64::INFINITY;
        } else if text == "nan" || text == "NaN" {
            v = f64::NAN;
        } else {
            return false;
        }
    } else if ch == b'e' || ch == b'E' {
        buffer.advance(1);
        let mut exponent: i32 = 0;
        if !parse_signed_int(buffer, &mut exponent) {
            return false;
        }
        v *= 10.0f64.powi(exponent);
    }

    let signed = if sign < 0 { -v } else { v };
    *value = signed as f32;
    true
}

pub fn parse_signed_int(buffer: &mut DecoderBuffer, value: &mut i32) -> bool {
    let mut ch: u8 = 0;
    if !buffer.peek(&mut ch) {
        return false;
    }
    let sign = get_sign_value(ch as char);
    if sign != 0 {
        buffer.advance(1);
    }
    let mut v: u32 = 0;
    if !parse_unsigned_int(buffer, &mut v) {
        return false;
    }
    *value = if sign < 0 { -(v as i32) } else { v as i32 };
    true
}

pub fn parse_unsigned_int(buffer: &mut DecoderBuffer, value: &mut u32) -> bool {
    let mut v: u32 = 0;
    let mut ch: u8 = 0;
    let mut have_digits = false;
    while buffer.peek(&mut ch) && ch >= b'0' && ch <= b'9' {
        v = v.wrapping_mul(10);
        v = v.wrapping_add((ch - b'0') as u32);
        buffer.advance(1);
        have_digits = true;
    }
    if !have_digits {
        return false;
    }
    *value = v;
    true
}

pub fn get_sign_value(c: char) -> i32 {
    if c == '-' {
        return -1;
    }
    if c == '+' {
        return 1;
    }
    0
}

pub fn parse_string(buffer: &mut DecoderBuffer, out_string: &mut String) -> bool {
    out_string.clear();
    skip_whitespace(buffer);
    let mut end_reached = false;
    while !peek_whitespace(buffer, &mut end_reached) && !end_reached {
        let mut c: u8 = 0;
        if !buffer.decode(&mut c) {
            return false;
        }
        out_string.push(c as char);
    }
    true
}

pub fn parse_line(buffer: &mut DecoderBuffer, out_string: Option<&mut String>) {
    match out_string {
        Some(out) => {
            out.clear();
            let mut c: u8 = 0;
            let mut num_delims = 0;
            let mut last_delim: u8 = 0;
            while buffer.peek(&mut c) {
                let is_delim = c == b'\r' || c == b'\n';
                if is_delim {
                    if num_delims == 0 {
                        last_delim = c;
                    } else if num_delims == 1 {
                        if c == last_delim || c != b'\n' {
                            return;
                        }
                    } else {
                        return;
                    }
                    num_delims += 1;
                }
                if !is_delim && num_delims > 0 {
                    return;
                }
                buffer.advance(1);
                if !is_delim {
                    out.push(c as char);
                }
            }
        }
        None => {
            let mut c: u8 = 0;
            let mut num_delims = 0;
            let mut last_delim: u8 = 0;
            while buffer.peek(&mut c) {
                let is_delim = c == b'\r' || c == b'\n';
                if is_delim {
                    if num_delims == 0 {
                        last_delim = c;
                    } else if num_delims == 1 {
                        if c == last_delim || c != b'\n' {
                            return;
                        }
                    } else {
                        return;
                    }
                    num_delims += 1;
                }
                if !is_delim && num_delims > 0 {
                    return;
                }
                buffer.advance(1);
            }
        }
    }
}

pub fn parse_line_into_decoder_buffer<'a>(buffer: &mut DecoderBuffer<'a>) -> DecoderBuffer<'a> {
    let head = buffer.data_head();
    let mut c: u8 = 0;
    while buffer.peek(&mut c) {
        buffer.advance(1);
        if c == b'\n' {
            break;
        }
        if c == b'\r' {
            continue;
        }
    }
    let remaining = buffer.data_head();
    let consumed = head.len().saturating_sub(remaining.len());
    let mut out = DecoderBuffer::new();
    out.init(&head[..consumed]);
    out
}

pub fn to_lower(input: &str) -> String {
    input.to_ascii_lowercase()
}
