//! Journal system — thread-safe buffered message output.
//!
//! Port of `journal.h` / `journal.cpp` from OSL.
//! Buffers printf, warning, error, and info messages during shading
//! for deferred, thread-safe output.

use std::sync::Mutex;

/// Message severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Print,
    Warning,
    Error,
    Severe,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Print => "print",
            Severity::Warning => "warning",
            Severity::Error => "error",
            Severity::Severe => "severe",
        }
    }
}

/// A single journal entry.
#[derive(Debug, Clone)]
pub struct JournalEntry {
    pub severity: Severity,
    pub message: String,
    /// Source file (if available).
    pub sourcefile: Option<String>,
    /// Source line (if available).
    pub sourceline: Option<u32>,
    /// Shader name (if available).
    pub shader_name: Option<String>,
    /// Layer name (if available).
    pub layer_name: Option<String>,
}

/// A journal writer that buffers messages during execution.
#[derive(Debug, Default)]
pub struct JournalWriter {
    entries: Vec<JournalEntry>,
    max_warnings: u32,
    max_errors: u32,
    warning_count: u32,
    error_count: u32,
}

impl JournalWriter {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_warnings: 100,
            max_errors: 100,
            warning_count: 0,
            error_count: 0,
        }
    }

    /// Set maximum number of warnings before suppression.
    pub fn set_max_warnings(&mut self, max: u32) {
        self.max_warnings = max;
    }

    /// Set maximum number of errors before suppression.
    pub fn set_max_errors(&mut self, max: u32) {
        self.max_errors = max;
    }

    /// Record a printf message.
    pub fn print(&mut self, msg: String) {
        self.entries.push(JournalEntry {
            severity: Severity::Print,
            message: msg,
            sourcefile: None,
            sourceline: None,
            shader_name: None,
            layer_name: None,
        });
    }

    /// Record a warning.
    pub fn warning(&mut self, msg: String) {
        self.warning_count += 1;
        if self.warning_count <= self.max_warnings {
            self.entries.push(JournalEntry {
                severity: Severity::Warning,
                message: msg,
                sourcefile: None,
                sourceline: None,
                shader_name: None,
                layer_name: None,
            });
        }
    }

    /// Record an error.
    pub fn error(&mut self, msg: String) {
        self.error_count += 1;
        if self.error_count <= self.max_errors {
            self.entries.push(JournalEntry {
                severity: Severity::Error,
                message: msg,
                sourcefile: None,
                sourceline: None,
                shader_name: None,
                layer_name: None,
            });
        }
    }

    /// Record an info message.
    pub fn info(&mut self, msg: String) {
        self.entries.push(JournalEntry {
            severity: Severity::Info,
            message: msg,
            sourcefile: None,
            sourceline: None,
            shader_name: None,
            layer_name: None,
        });
    }

    /// Record a severe error.
    pub fn severe(&mut self, msg: String) {
        self.entries.push(JournalEntry {
            severity: Severity::Severe,
            message: msg,
            sourcefile: None,
            sourceline: None,
            shader_name: None,
            layer_name: None,
        });
    }

    /// Record a message with full context.
    pub fn record(
        &mut self,
        severity: Severity,
        msg: String,
        shader: Option<&str>,
        layer: Option<&str>,
        file: Option<&str>,
        line: Option<u32>,
    ) {
        match severity {
            Severity::Warning => self.warning_count += 1,
            Severity::Error | Severity::Severe => self.error_count += 1,
            _ => {}
        }

        let suppress = match severity {
            Severity::Warning => self.warning_count > self.max_warnings,
            Severity::Error => self.error_count > self.max_errors,
            _ => false,
        };

        if !suppress {
            self.entries.push(JournalEntry {
                severity,
                message: msg,
                sourcefile: file.map(|s| s.to_string()),
                sourceline: line,
                shader_name: shader.map(|s| s.to_string()),
                layer_name: layer.map(|s| s.to_string()),
            });
        }
    }

    /// Total entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total warnings.
    pub fn warning_count(&self) -> u32 {
        self.warning_count
    }

    /// Total errors.
    pub fn error_count(&self) -> u32 {
        self.error_count
    }

    /// Get all entries.
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Drain all entries.
    pub fn drain(&mut self) -> Vec<JournalEntry> {
        std::mem::take(&mut self.entries)
    }

    /// Clear all entries and reset counts.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.warning_count = 0;
        self.error_count = 0;
    }

    /// Has any errors occurred?
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }
}

/// A journal reader — reads back buffered messages.
pub struct JournalReader<'a> {
    entries: &'a [JournalEntry],
    pos: usize,
}

impl<'a> JournalReader<'a> {
    pub fn new(entries: &'a [JournalEntry]) -> Self {
        Self { entries, pos: 0 }
    }

    /// Read the next entry (does not implement [`Iterator`] — use this instead of a `next` inherent method).
    pub fn read_next(&mut self) -> Option<&'a JournalEntry> {
        if self.pos < self.entries.len() {
            let entry = &self.entries[self.pos];
            self.pos += 1;
            Some(entry)
        } else {
            None
        }
    }

    /// Remaining entries.
    pub fn remaining(&self) -> usize {
        self.entries.len() - self.pos
    }
}

/// Thread-safe journal — wraps a JournalWriter in a Mutex.
#[derive(Default)]
pub struct ThreadSafeJournal {
    inner: Mutex<JournalWriter>,
}

impl ThreadSafeJournal {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(JournalWriter::new()),
        }
    }

    pub fn print(&self, msg: String) {
        self.inner.lock().unwrap().print(msg);
    }

    pub fn warning(&self, msg: String) {
        self.inner.lock().unwrap().warning(msg);
    }

    pub fn error(&self, msg: String) {
        self.inner.lock().unwrap().error(msg);
    }

    pub fn info(&self, msg: String) {
        self.inner.lock().unwrap().info(msg);
    }

    pub fn drain(&self) -> Vec<JournalEntry> {
        self.inner.lock().unwrap().drain()
    }

    pub fn has_errors(&self) -> bool {
        self.inner.lock().unwrap().has_errors()
    }

    pub fn warning_count(&self) -> u32 {
        self.inner.lock().unwrap().warning_count()
    }

    pub fn error_count(&self) -> u32 {
        self.inner.lock().unwrap().error_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writer() {
        let mut w = JournalWriter::new();
        w.print("hello".into());
        w.warning("warn1".into());
        w.error("err1".into());
        assert_eq!(w.len(), 3);
        assert_eq!(w.warning_count(), 1);
        assert_eq!(w.error_count(), 1);
        assert!(w.has_errors());
    }

    #[test]
    fn test_max_warnings() {
        let mut w = JournalWriter::new();
        w.set_max_warnings(2);
        for i in 0..5 {
            w.warning(format!("warn {i}"));
        }
        assert_eq!(w.warning_count(), 5);
        // Only 2 should be stored
        assert_eq!(w.entries().len(), 2);
    }

    #[test]
    fn test_reader() {
        let mut w = JournalWriter::new();
        w.print("a".into());
        w.print("b".into());
        w.print("c".into());

        let mut r = JournalReader::new(w.entries());
        assert_eq!(r.remaining(), 3);
        assert_eq!(r.read_next().unwrap().message, "a");
        assert_eq!(r.read_next().unwrap().message, "b");
        assert_eq!(r.remaining(), 1);
    }

    #[test]
    fn test_drain() {
        let mut w = JournalWriter::new();
        w.print("test".into());
        let entries = w.drain();
        assert_eq!(entries.len(), 1);
        assert!(w.is_empty());
    }

    #[test]
    fn test_thread_safe() {
        let journal = ThreadSafeJournal::new();
        journal.print("msg1".into());
        journal.warning("warn1".into());
        assert_eq!(journal.warning_count(), 1);
        let entries = journal.drain();
        assert_eq!(entries.len(), 2);
    }
}
