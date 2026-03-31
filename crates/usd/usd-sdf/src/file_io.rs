//! SdfFileIO - common file I/O utilities.
//!
//! Port of pxr/usd/sdf/fileIO.h and fileIO_Common.h
//!
//! Utilities for reading and writing SDF files.

use crate::{Layer, Path, Specifier, SpecType};
use usd_tf::Token;
use usd_vt::Value;
use std::io::{Read, Write};
use std::sync::Arc;

/// File I/O context for reading.
pub struct FileIOReadContext {
    /// Source identifier.
    pub identifier: String,
    /// Whether to resolve asset paths.
    pub resolve_asset_paths: bool,
    /// Metadata only mode.
    pub metadata_only: bool,
}

impl Default for FileIOReadContext {
    fn default() -> Self {
        Self {
            identifier: String::new(),
            resolve_asset_paths: true,
            metadata_only: false,
        }
    }
}

impl FileIOReadContext {
    /// Creates a new read context.
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            ..Default::default()
        }
    }

    /// Sets whether to resolve asset paths.
    pub fn with_resolve_asset_paths(mut self, resolve: bool) -> Self {
        self.resolve_asset_paths = resolve;
        self
    }

    /// Sets metadata only mode.
    pub fn with_metadata_only(mut self, metadata_only: bool) -> Self {
        self.metadata_only = metadata_only;
        self
    }
}

/// File I/O context for writing.
pub struct FileIOWriteContext {
    /// Target identifier.
    pub identifier: String,
    /// Whether to write comments.
    pub write_comments: bool,
    /// Indentation string.
    pub indent: String,
}

impl Default for FileIOWriteContext {
    fn default() -> Self {
        Self {
            identifier: String::new(),
            write_comments: true,
            indent: "    ".to_string(),
        }
    }
}

impl FileIOWriteContext {
    /// Creates a new write context.
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
            ..Default::default()
        }
    }

    /// Sets whether to write comments.
    pub fn with_comments(mut self, write: bool) -> Self {
        self.write_comments = write;
        self
    }

    /// Sets indentation.
    pub fn with_indent(mut self, indent: impl Into<String>) -> Self {
        self.indent = indent.into();
        self
    }
}

/// Spec data for file I/O.
#[derive(Debug, Clone)]
pub struct SpecData {
    /// Path to spec.
    pub path: Path,
    /// Spec type.
    pub spec_type: SpecType,
    /// Fields.
    pub fields: Vec<(Token, Value)>,
}

impl SpecData {
    /// Creates new spec data.
    pub fn new(path: Path, spec_type: SpecType) -> Self {
        Self {
            path,
            spec_type,
            fields: Vec::new(),
        }
    }

    /// Adds a field.
    pub fn add_field(&mut self, key: Token, value: Value) {
        self.fields.push((key, value));
    }

    /// Gets a field value.
    pub fn get_field(&self, key: &Token) -> Option<&Value> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

/// Writer for USDA text format.
pub struct TextWriter<W: Write> {
    /// Output writer.
    writer: W,
    /// Current indentation level.
    indent_level: usize,
    /// Indentation string.
    indent_str: String,
}

impl<W: Write> TextWriter<W> {
    /// Creates a new text writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            indent_level: 0,
            indent_str: "    ".to_string(),
        }
    }

    /// Sets indentation string.
    pub fn set_indent(&mut self, indent: impl Into<String>) {
        self.indent_str = indent.into();
    }

    /// Increases indentation.
    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    /// Decreases indentation.
    pub fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    /// Writes indentation.
    pub fn write_indent(&mut self) -> std::io::Result<()> {
        for _ in 0..self.indent_level {
            self.writer.write_all(self.indent_str.as_bytes())?;
        }
        Ok(())
    }

    /// Writes a line with indentation.
    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        self.write_indent()?;
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }

    /// Writes raw bytes.
    pub fn write_raw(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.writer.write_all(data)
    }

    /// Writes a specifier.
    pub fn write_specifier(&mut self, spec: Specifier) -> std::io::Result<()> {
        let s = match spec {
            Specifier::Def => "def",
            Specifier::Over => "over",
            Specifier::Class => "class",
        };
        self.writer.write_all(s.as_bytes())
    }

    /// Writes a quoted string.
    pub fn write_quoted_string(&mut self, s: &str) -> std::io::Result<()> {
        self.writer.write_all(b"\"")?;
        for c in s.chars() {
            match c {
                '"' => self.writer.write_all(b"\\\"")?,
                '\\' => self.writer.write_all(b"\\\\")?,
                '\n' => self.writer.write_all(b"\\n")?,
                '\r' => self.writer.write_all(b"\\r")?,
                '\t' => self.writer.write_all(b"\\t")?,
                _ => {
                    let mut buf = [0u8; 4];
                    let s = c.encode_utf8(&mut buf);
                    self.writer.write_all(s.as_bytes())?;
                }
            }
        }
        self.writer.write_all(b"\"")
    }

    /// Writes a value.
    pub fn write_value(&mut self, value: &Value) -> std::io::Result<()> {
        if value.is_empty() {
            self.writer.write_all(b"None")?;
        } else if let Some(&b) = value.get::<bool>() {
            self.writer.write_all(if b { b"true" } else { b"false" })?;
        } else if let Some(&i) = value.get::<i32>() {
            write!(self.writer, "{}", i)?;
        } else if let Some(&i) = value.get::<i64>() {
            write!(self.writer, "{}", i)?;
        } else if let Some(&f) = value.get::<f32>() {
            write!(self.writer, "{}", f)?;
        } else if let Some(&f) = value.get::<f64>() {
            write!(self.writer, "{}", f)?;
        } else if let Some(s) = value.get::<String>() {
            self.write_quoted_string(s)?;
        } else if let Some(t) = value.get::<Token>() {
            self.write_quoted_string(t.as_str())?;
        } else {
            // Default representation
            write!(self.writer, "{:?}", value)?;
        }
        Ok(())
    }

    /// Flushes the writer.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }

    /// Consumes and returns the inner writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

/// Reader for USDA text format.
pub struct TextReader<R: Read> {
    /// Input reader.
    reader: R,
    /// Current line number.
    line_number: usize,
    /// Current column.
    column: usize,
}

impl<R: Read> TextReader<R> {
    /// Creates a new text reader.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            line_number: 1,
            column: 1,
        }
    }

    /// Returns current line number.
    pub fn line_number(&self) -> usize {
        self.line_number
    }

    /// Returns current column.
    pub fn column(&self) -> usize {
        self.column
    }

    /// Reads all content as string.
    pub fn read_to_string(&mut self) -> std::io::Result<String> {
        let mut content = String::new();
        self.reader.read_to_string(&mut content)?;
        Ok(content)
    }
}

/// Writes a layer to a file.
pub fn write_layer_to_file<W: Write>(
    layer: &Arc<Layer>,
    writer: W,
    context: &FileIOWriteContext,
) -> std::io::Result<()> {
    let mut text_writer = TextWriter::new(writer);
    text_writer.set_indent(&context.indent);

    // Write header
    text_writer.write_line("#usda 1.0")?;
    text_writer.write_line("")?;

    // Write layer metadata if any
    // ...

    // Write specs
    // This would need to traverse the layer's specs
    // For now, just write an empty layer

    text_writer.flush()?;
    Ok(())
}

/// Reads a layer from a file.
pub fn read_layer_from_file<R: Read>(
    layer: &Arc<Layer>,
    reader: R,
    _context: &FileIOReadContext,
) -> std::io::Result<()> {
    let mut text_reader = TextReader::new(reader);
    let content = text_reader.read_to_string()?;

    // Parse the content and populate the layer
    // This would use the text_parser module
    let _ = (layer, content);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_text_writer() {
        let mut buf = Vec::new();
        {
            let mut writer = TextWriter::new(&mut buf);
            writer.write_line("test").unwrap();
            writer.indent();
            writer.write_line("indented").unwrap();
            writer.dedent();
            writer.write_line("back").unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("test\n"));
        assert!(output.contains("    indented\n"));
        assert!(output.contains("back\n"));
    }

    #[test]
    fn test_quoted_string() {
        let mut buf = Vec::new();
        {
            let mut writer = TextWriter::new(&mut buf);
            writer.write_quoted_string("hello \"world\"").unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "\"hello \\\"world\\\"\"");
    }
}
