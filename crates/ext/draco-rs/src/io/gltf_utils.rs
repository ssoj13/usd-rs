//! glTF utilities.
//!
//! What: JSON writer helpers and glTF numeric value wrappers.
//! Why: glTF encoder needs stable, readable JSON output formatting.
//! How: Mirrors Draco `GltfValue`, `Indent`, and `JsonWriter` behavior.
//! Where used: glTF encoder and tests.

use std::any::Any;
use std::fmt::{self, Write as _};

/// glTF value wrapper for integer or floating-point output.
#[derive(Clone, Copy, Debug)]
pub enum GltfValue {
    Int(i64),
    Double(f64),
}

impl GltfValue {
    pub fn from_i8(value: i8) -> Self {
        GltfValue::Int(value as i64)
    }

    pub fn from_u8(value: u8) -> Self {
        GltfValue::Int(value as i64)
    }

    pub fn from_i16(value: i16) -> Self {
        GltfValue::Int(value as i64)
    }

    pub fn from_u16(value: u16) -> Self {
        GltfValue::Int(value as i64)
    }

    pub fn from_u32(value: u32) -> Self {
        GltfValue::Int(value as i64)
    }

    pub fn from_f32(value: f32) -> Self {
        GltfValue::Double(value as f64)
    }
}

impl fmt::Display for GltfValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GltfValue::Int(value) => write!(f, "{}", value),
            GltfValue::Double(value) => write!(f, "{}", format_float_g(*value, 17)),
        }
    }
}

/// Indentation helper for readable JSON output.
#[derive(Clone, Debug, Default)]
pub struct Indent {
    indent: String,
    indent_space_count: usize,
}

impl Indent {
    pub fn new() -> Self {
        Self {
            indent: String::new(),
            indent_space_count: 2,
        }
    }

    pub fn increase(&mut self) {
        self.indent.push_str(&" ".repeat(self.indent_space_count));
    }

    pub fn decrease(&mut self) {
        if self.indent.len() >= self.indent_space_count {
            let new_len = self.indent.len() - self.indent_space_count;
            self.indent.truncate(new_len);
        }
    }
}

impl fmt::Display for Indent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.indent)
    }
}

/// JSON writer state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    Start,
    Begin,
    End,
    Value,
}

/// JSON output mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Readable,
    Compact,
}

/// JSON writer with indentation and escaping helpers.
#[derive(Clone, Debug)]
pub struct JsonWriter {
    output: String,
    indent: Indent,
    last_type: OutputType,
    mode: Mode,
}

impl Default for JsonWriter {
    fn default() -> Self {
        Self {
            output: String::new(),
            indent: Indent::new(),
            last_type: OutputType::Start,
            mode: Mode::Readable,
        }
    }
}

impl JsonWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    pub fn reset(&mut self) {
        self.last_type = OutputType::Start;
        self.output.clear();
    }

    pub fn begin_object(&mut self) {
        self.begin_object_named("");
    }

    pub fn begin_object_named(&mut self, name: &str) {
        self.finish_previous_line(OutputType::Begin);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        if !name.is_empty() {
            let escaped_name = self.escape_json_special_characters(name);
            let _ = write!(
                self.output,
                "\"{}\":{}",
                escaped_name,
                self.separator_string()
            );
        }
        self.output.push('{');
        self.indent.increase();
    }

    pub fn end_object(&mut self) {
        self.finish_previous_line(OutputType::End);
        self.indent.decrease();
        let indent = self.indent_string();
        self.output.push_str(&indent);
        self.output.push('}');
    }

    pub fn begin_array(&mut self) {
        self.finish_previous_line(OutputType::Begin);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        self.output.push('[');
        self.indent.increase();
    }

    pub fn begin_array_named(&mut self, name: &str) {
        self.finish_previous_line(OutputType::Begin);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        let escaped_name = self.escape_json_special_characters(name);
        let _ = write!(
            self.output,
            "\"{}\":{}[",
            escaped_name,
            self.separator_string()
        );
        self.indent.increase();
    }

    pub fn end_array(&mut self) {
        self.finish_previous_line(OutputType::End);
        self.indent.decrease();
        let indent = self.indent_string();
        self.output.push_str(&indent);
        self.output.push(']');
    }

    pub fn output_value<T: fmt::Display + 'static>(&mut self, value: T) {
        self.finish_previous_line(OutputType::Value);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        let value_str = format_json_value(&value, 17);
        self.output.push_str(&value_str);
    }

    pub fn output_bool(&mut self, value: bool) {
        self.finish_previous_line(OutputType::Value);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        self.output.push_str(if value { "true" } else { "false" });
    }

    pub fn output_string(&mut self, value: &str) {
        let escaped = self.escape_json_special_characters(value);
        self.finish_previous_line(OutputType::Value);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        let _ = write!(self.output, "\"{}\"", escaped);
    }

    pub fn output_named_string(&mut self, name: &str, value: &str) {
        let escaped_name = self.escape_json_special_characters(name);
        let escaped_value = self.escape_json_special_characters(value);
        self.finish_previous_line(OutputType::Value);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        let _ = write!(
            self.output,
            "\"{}\":{}\"{}\"",
            escaped_name,
            self.separator_string(),
            escaped_value
        );
    }

    pub fn output_named_value<T: fmt::Display + 'static>(&mut self, name: &str, value: T) {
        let escaped_name = self.escape_json_special_characters(name);
        self.finish_previous_line(OutputType::Value);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        let value_str = format_json_value(&value, 17);
        let _ = write!(
            self.output,
            "\"{}\":{}{}",
            escaped_name,
            self.separator_string(),
            value_str
        );
    }

    pub fn output_named_bool(&mut self, name: &str, value: bool) {
        let escaped_name = self.escape_json_special_characters(name);
        self.finish_previous_line(OutputType::Value);
        let indent = self.indent_string();
        self.output.push_str(&indent);
        let _ = write!(
            self.output,
            "\"{}\":{}{}",
            escaped_name,
            self.separator_string(),
            if value { "true" } else { "false" }
        );
    }

    pub fn move_data(&mut self) -> String {
        let out = self.output.clone();
        self.output.clear();
        out
    }

    fn finish_previous_line(&mut self, curr_type: OutputType) {
        if self.last_type != OutputType::Start {
            if (self.last_type == OutputType::Value && curr_type == OutputType::Value)
                || (self.last_type == OutputType::Value && curr_type == OutputType::Begin)
                || (self.last_type == OutputType::End && curr_type == OutputType::Begin)
                || (self.last_type == OutputType::End && curr_type == OutputType::Value)
            {
                self.output.push(',');
            }
            if self.mode == Mode::Readable {
                self.output.push('\n');
            }
        }
        self.last_type = curr_type;
    }

    fn indent_string(&self) -> String {
        if self.mode == Mode::Readable {
            self.indent.indent.clone()
        } else {
            String::new()
        }
    }

    fn separator_string(&self) -> &'static str {
        if self.mode == Mode::Readable {
            " "
        } else {
            ""
        }
    }

    fn escape_json_special_characters(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '"' => out.push_str("\\\""),
                _ => out.push(ch),
            }
        }
        out
    }
}

fn format_json_value<T: fmt::Display + 'static>(value: &T, precision: usize) -> String {
    if let Some(value) = (value as &dyn Any).downcast_ref::<f32>() {
        return format_float_g(*value as f64, precision);
    }
    if let Some(value) = (value as &dyn Any).downcast_ref::<f64>() {
        return format_float_g(*value, precision);
    }
    format!("{}", value)
}

fn format_float_g(value: f64, precision: usize) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    if !value.is_finite() {
        return value.to_string();
    }

    let abs = value.abs();
    let mut exp = abs.log10().floor() as i32;
    let pow = 10_f64.powi(exp);
    let mantissa = abs / pow;
    if mantissa >= 10.0 {
        exp += 1;
    } else if mantissa < 1.0 {
        exp -= 1;
    }

    if exp < -4 || exp >= precision as i32 {
        let sci = format!("{:.*e}", precision.saturating_sub(1), value);
        let mut parts = sci.split('e');
        let mantissa_raw = parts.next().unwrap_or("0");
        let exp_raw = parts.next().unwrap_or("0");
        let exp_value = exp_raw.parse::<i32>().unwrap_or(0);
        let mantissa_str = trim_trailing_zeros(mantissa_raw.to_string());
        let exp_str = format!("{:+03}", exp_value);
        return format!("{}e{}", mantissa_str, exp_str);
    }

    let digits_after = (precision as i32 - 1 - exp).max(0) as usize;
    let mut out = format!("{:.*}", digits_after, value);
    out = trim_trailing_zeros(out);
    out
}

fn trim_trailing_zeros(mut input: String) -> String {
    if let Some(dot_pos) = input.find('.') {
        while input.ends_with('0') {
            input.pop();
        }
        if input.ends_with('.') && dot_pos == input.len() - 1 {
            input.pop();
        }
    }
    input
}
