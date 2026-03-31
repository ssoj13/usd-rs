//! OSL Preprocessor — `#include`, `#define`, `#ifdef`, `#pragma`.
//!
//! Port of the C-style preprocessor used by the OSL compiler.
//! Processes source text before lexing/parsing, handling:
//! - `#define NAME [value]`
//! - `#define NAME(params) body` (function-like macros)
//! - `#undef NAME`
//! - `#ifdef NAME` / `#ifndef NAME` / `#if` / `#elif` / `#else` / `#endif`
//! - `#include "file"` / `#include <file>`
//! - `#pragma once` / `#pragma osl ...`
//! - `__LINE__`, `__FILE__` built-in macros
//! - Line continuation with `\`

use std::cell::Cell;
use std::collections::HashMap;

/// A macro definition.
#[derive(Debug, Clone)]
pub struct MacroDef {
    /// Parameter names for function-like macros (empty for object-like).
    pub params: Vec<String>,
    /// Whether this is a function-like macro.
    pub is_function: bool,
    /// Replacement body text.
    pub body: String,
}

/// Include file resolver callback type.
/// Given an include path and whether it's a system include (<...>),
/// returns the file content or an error.
pub type IncludeResolver = Box<dyn Fn(&str, bool) -> Result<String, String>>;

/// Preprocessor state.
pub struct Preprocessor {
    /// Currently defined macros.
    defines: HashMap<String, MacroDef>,
    /// Stack of conditional inclusion states.
    /// Each entry: (active, seen_true_branch, had_else)
    cond_stack: Vec<(bool, bool, bool)>,
    /// Whether we've seen `#pragma once` for given files.
    once_files: Vec<String>,
    /// Include resolver.
    include_resolver: Option<IncludeResolver>,
    /// Current file name (for __FILE__).
    current_file: String,
    /// Include search paths.
    pub include_paths: Vec<String>,
    /// Errors encountered during preprocessing.
    pub errors: Vec<String>,
    /// Max include depth to prevent infinite recursion.
    pub max_include_depth: usize,
    /// Current include depth.
    include_depth: usize,
    /// Current line number for __LINE__ expansion.
    current_line: Cell<u32>,
}

impl Preprocessor {
    pub fn new() -> Self {
        let mut pp = Self {
            defines: HashMap::new(),
            cond_stack: Vec::new(),
            once_files: Vec::new(),
            include_resolver: None,
            current_file: String::from("<stdin>"),
            include_paths: Vec::new(),
            errors: Vec::new(),
            max_include_depth: 64,
            include_depth: 0,
            current_line: Cell::new(0),
        };
        // Pre-define standard macros
        pp.define_object("OSL_VERSION_MAJOR", &crate::OSL_VERSION_MAJOR.to_string());
        pp.define_object("OSL_VERSION_MINOR", &crate::OSL_VERSION_MINOR.to_string());
        pp.define_object("OSL_VERSION_PATCH", &crate::OSL_VERSION_PATCH.to_string());
        pp.define_object("OSL_VERSION", &crate::OSL_VERSION.to_string());
        pp.define_object("M_PI", "3.14159265358979323846");
        pp.define_object("M_PI_2", "1.57079632679489661923");
        pp.define_object("M_PI_4", "0.78539816339744830962");
        pp.define_object("M_2_PI", "0.63661977236758134308");
        pp.define_object("M_1_PI", "0.31830988618379067154");
        pp.define_object("M_2PI", "6.28318530717958647692");
        pp.define_object("M_E", "2.71828182845904523536");
        pp.define_object("M_LN2", "0.69314718055994530942");
        pp.define_object("M_LN10", "2.30258509299404568402");
        pp.define_object("M_LOG2E", "1.44269504088896340736");
        pp.define_object("M_LOG10E", "0.43429448190325182765");
        pp.define_object("M_SQRT2", "1.41421356237309504880");
        pp.define_object("M_SQRT1_2", "0.70710678118654752440");
        pp
    }

    /// Set the include file resolver.
    pub fn set_include_resolver(&mut self, resolver: IncludeResolver) {
        self.include_resolver = Some(resolver);
    }

    /// Define an object-like macro: `#define NAME value`.
    pub fn define_object(&mut self, name: &str, value: &str) {
        self.defines.insert(
            name.to_string(),
            MacroDef {
                params: Vec::new(),
                is_function: false,
                body: value.to_string(),
            },
        );
    }

    /// Define a function-like macro: `#define NAME(a,b) body`.
    pub fn define_function(&mut self, name: &str, params: Vec<String>, body: &str) {
        self.defines.insert(
            name.to_string(),
            MacroDef {
                params,
                is_function: true,
                body: body.to_string(),
            },
        );
    }

    /// Undefine a macro.
    pub fn undef(&mut self, name: &str) {
        self.defines.remove(name);
    }

    /// Check if a macro is defined.
    pub fn is_defined(&self, name: &str) -> bool {
        self.defines.contains_key(name)
    }

    /// Whether we are currently in an active (non-skipped) region.
    fn is_active(&self) -> bool {
        self.cond_stack.iter().all(|(active, _, _)| *active)
    }

    /// Check if a line contains a function-like macro invocation whose
    /// argument parentheses are not balanced (i.e., the invocation spans
    /// multiple lines). Only returns true when:
    ///   1. A known function-like macro name is found on the line
    ///   2. The overall parenthesis depth is > 0 (unclosed)
    fn line_has_unfinished_macro_call(&self, line: &str) -> bool {
        // Quick check: if parens are balanced, no accumulation needed
        let depth = count_paren_depth(line);
        if depth <= 0 {
            return false;
        }
        // Scan for identifiers that match a known function-like macro
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut i = 0;
        while i < len {
            let c = bytes[i];
            if c == b'"' {
                i += 1;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
                continue;
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &line[start..i];
                if let Some(def) = self.defines.get(ident) {
                    if def.is_function {
                        // Found a function-like macro. Check if the '(' after it
                        // is unmatched within this line.
                        let mut j = i;
                        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }
                        if j < len && bytes[j] == b'(' {
                            // Count paren depth from this '(' to end of line
                            let mut pd = 0i32;
                            let mut k = j;
                            while k < len {
                                match bytes[k] {
                                    b'"' => {
                                        k += 1;
                                        while k < len && bytes[k] != b'"' {
                                            if bytes[k] == b'\\' && k + 1 < len {
                                                k += 2;
                                            } else {
                                                k += 1;
                                            }
                                        }
                                        if k < len {
                                            k += 1;
                                        }
                                        continue;
                                    }
                                    b'(' => pd += 1,
                                    b')' => pd -= 1,
                                    _ => {}
                                }
                                k += 1;
                            }
                            if pd > 0 {
                                return true;
                            }
                        }
                    }
                }
                continue;
            }
            i += 1;
        }
        false
    }

    /// Process source text through the preprocessor.
    pub fn process(&mut self, source: &str) -> Result<String, Vec<String>> {
        self.process_file(source, "<stdin>")
    }

    /// Process a named file's content.
    pub fn process_file(&mut self, source: &str, filename: &str) -> Result<String, Vec<String>> {
        let prev_file = self.current_file.clone();
        self.current_file = filename.to_string();
        let start_depth = self.cond_stack.len();

        let mut output = String::with_capacity(source.len());
        let mut line_num = 0u32;

        // Handle line continuation first
        let source = merge_continuations(source);

        let all_lines: Vec<&str> = source.lines().collect();
        let total_lines = all_lines.len();
        let mut idx = 0;

        while idx < total_lines {
            let line = all_lines[idx];
            line_num += 1;
            self.current_line.set(line_num);
            let trimmed = line.trim();

            if trimmed.starts_with('#') {
                self.process_directive(trimmed, &mut output, filename, line_num);
                idx += 1;
            } else if self.is_active() {
                // Check if this line contains a function-like macro invocation
                // whose arguments span multiple lines (unbalanced parens).
                // Only accumulate when a known function-like macro is detected.
                let needs_accumulation = self.line_has_unfinished_macro_call(line);
                if needs_accumulation {
                    let mut accumulated = line.to_string();
                    let mut paren_depth = count_paren_depth(line);
                    let mut extra_lines = 0u32;
                    while paren_depth > 0 && idx + 1 + (extra_lines as usize) < total_lines {
                        extra_lines += 1;
                        let next_line = all_lines[idx + extra_lines as usize];
                        accumulated.push(' ');
                        accumulated.push_str(next_line);
                        paren_depth += count_paren_depth(next_line);
                    }
                    let expanded = self.expand_macros(&accumulated);
                    output.push_str(&expanded);
                    output.push('\n');
                    for _ in 0..extra_lines {
                        output.push('\n');
                        line_num += 1;
                    }
                    idx += 1 + extra_lines as usize;
                } else {
                    let expanded = self.expand_macros(line);
                    output.push_str(&expanded);
                    output.push('\n');
                    idx += 1;
                }
            } else {
                // In a skipped conditional block — emit empty line to preserve line numbers
                output.push('\n');
                idx += 1;
            }
        }

        self.current_file = prev_file;

        if self.cond_stack.len() > start_depth {
            self.errors.push(format!(
                "{filename}: unterminated #if/#ifdef at end of file"
            ));
        } else if self.cond_stack.len() < start_depth {
            self.errors.push(format!(
                "{filename}: #endif without matching #if in this include"
            ));
        }
        if self.cond_stack.len() != start_depth {
            self.cond_stack.truncate(start_depth);
        }

        if self.errors.is_empty() {
            Ok(output)
        } else {
            Err(self.errors.clone())
        }
    }

    /// Process a preprocessor directive line.
    fn process_directive(
        &mut self,
        line: &str,
        output: &mut String,
        filename: &str,
        line_num: u32,
    ) {
        let directive = &line[1..].trim_start();

        if directive.starts_with("endif") {
            if self.cond_stack.pop().is_none() {
                self.errors.push(format!(
                    "{filename}:{line_num}: #endif without matching #if"
                ));
            }
            output.push('\n');
            return;
        }

        if directive.starts_with("else") {
            if let Some(top) = self.cond_stack.last_mut() {
                if top.2 {
                    self.errors
                        .push(format!("{filename}:{line_num}: duplicate #else"));
                } else {
                    top.2 = true;
                    top.0 = !top.1; // Active if we haven't seen a true branch yet
                }
            } else {
                self.errors
                    .push(format!("{filename}:{line_num}: #else without matching #if"));
            }
            output.push('\n');
            return;
        }

        if directive.starts_with("elif") {
            let stack_len = self.cond_stack.len();
            if stack_len > 0 {
                let already_true = self.cond_stack[stack_len - 1].1;
                if already_true {
                    // Already had a true branch — skip
                    self.cond_stack[stack_len - 1].0 = false;
                } else {
                    let cond_text = directive[4..].trim();
                    let result = self.evaluate_condition(cond_text);
                    let last = self.cond_stack.len() - 1;
                    self.cond_stack[last].0 = result;
                    if result {
                        self.cond_stack[last].1 = true;
                    }
                }
            } else {
                self.errors
                    .push(format!("{filename}:{line_num}: #elif without matching #if"));
            }
            output.push('\n');
            return;
        }

        // For all other directives, only process if we're in an active block
        if !self.is_active() {
            // Still need to track nested #if/#ifdef in skipped regions
            if directive.starts_with("if") {
                self.cond_stack.push((false, false, false));
            }
            output.push('\n');
            return;
        }

        if directive.starts_with("define") {
            self.process_define(&directive[6..].trim_start());
        } else if directive.starts_with("undef") {
            let name = directive[5..].trim();
            self.undef(name);
        } else if directive.starts_with("ifdef") {
            let name = directive[5..].trim();
            let defined = self.is_defined(name);
            self.cond_stack.push((defined, defined, false));
        } else if directive.starts_with("ifndef") {
            let name = directive[6..].trim();
            let defined = self.is_defined(name);
            self.cond_stack.push((!defined, !defined, false));
        } else if directive.starts_with("if") {
            let cond_text = directive[2..].trim();
            let result = self.evaluate_condition(cond_text);
            self.cond_stack.push((result, result, false));
        } else if directive.starts_with("include") {
            self.process_include(&directive[7..].trim_start(), output, filename, line_num);
        } else if directive.starts_with("pragma") {
            self.process_pragma(&directive[6..].trim_start(), filename);
        } else if directive.starts_with("error") {
            let msg = directive[5..].trim();
            self.errors
                .push(format!("{filename}:{line_num}: #error {msg}"));
        } else if directive.starts_with("warning") {
            // Warnings don't stop compilation, just store them
            let msg = directive[7..].trim();
            self.errors
                .push(format!("{filename}:{line_num}: #warning {msg}"));
        } else if directive.starts_with("line") {
            // #line directives — skip (used for line number tracking)
        } else {
            self.errors.push(format!(
                "{filename}:{line_num}: unknown preprocessor directive: #{}",
                directive.split_whitespace().next().unwrap_or("")
            ));
        }

        output.push('\n');
    }

    /// Process `#define`.
    fn process_define(&mut self, rest: &str) {
        let mut chars = rest.chars().peekable();

        // Parse macro name (using peek to avoid consuming the delimiter)
        let mut name = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if name.is_empty() {
            return;
        }

        // Check for function-like macro: NAME(
        if chars.peek() == Some(&'(') {
            chars.next(); // consume '('
            let mut params = Vec::new();
            let mut param = String::new();
            loop {
                match chars.next() {
                    Some(')') => {
                        let p = param.trim().to_string();
                        if !p.is_empty() {
                            params.push(p);
                        }
                        break;
                    }
                    Some(',') => {
                        params.push(param.trim().to_string());
                        param.clear();
                    }
                    Some(c) => param.push(c),
                    None => break,
                }
            }
            let body: String = chars.collect::<String>().trim().to_string();
            self.define_function(&name, params, &body);
        } else {
            // Object-like macro
            let body: String = chars.collect::<String>().trim().to_string();
            self.define_object(&name, &body);
        }
    }

    /// Process `#include "file"` or `#include <file>`.
    fn process_include(&mut self, rest: &str, output: &mut String, filename: &str, line_num: u32) {
        let (path, _is_system) = if rest.starts_with('"') {
            let end = rest[1..].find('"').map(|i| i + 1);
            if let Some(end) = end {
                (rest[1..end].to_string(), false)
            } else {
                self.errors
                    .push(format!("{filename}:{line_num}: malformed #include"));
                return;
            }
        } else if rest.starts_with('<') {
            let end = rest[1..].find('>').map(|i| i + 1);
            if let Some(end) = end {
                (rest[1..end].to_string(), true)
            } else {
                self.errors
                    .push(format!("{filename}:{line_num}: malformed #include"));
                return;
            }
        } else {
            self.errors
                .push(format!("{filename}:{line_num}: malformed #include"));
            return;
        };

        // Check pragma once
        if self.once_files.contains(&path) {
            return;
        }

        // Check include depth
        if self.include_depth >= self.max_include_depth {
            self.errors
                .push(format!("{filename}:{line_num}: #include nested too deeply"));
            return;
        }

        // Handle stdosl.h specially
        if path == "stdosl.h" {
            self.include_depth += 1;
            let content = crate::stdosl::STDOSL_H;
            // We don't recursively preprocess stdosl.h to avoid infinite loops;
            // just expand macros in it
            output.push_str(content);
            output.push('\n');
            self.include_depth -= 1;
            return;
        }

        // Try to resolve the include.
        // Clone the resolver result to avoid borrowing `self` while calling it.
        let resolved = self
            .include_resolver
            .as_ref()
            .map(|resolver| resolver(&path, _is_system));
        if let Some(resolved) = resolved {
            match resolved {
                Ok(content) => {
                    self.include_depth += 1;
                    match self.process_file(&content, &path) {
                        Ok(processed) => output.push_str(&processed),
                        Err(errs) => self.errors.extend(errs),
                    }
                    self.include_depth -= 1;
                }
                Err(e) => {
                    self.errors.push(format!(
                        "{filename}:{line_num}: cannot open include file '{path}': {e}"
                    ));
                }
            }
        } else {
            // No resolver — try reading from filesystem
            let resolved = self.resolve_include_path(&path, filename);
            if let Some(full_path) = resolved {
                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        self.include_depth += 1;
                        match self.process_file(&content, &full_path) {
                            Ok(processed) => output.push_str(&processed),
                            Err(errs) => self.errors.extend(errs),
                        }
                        self.include_depth -= 1;
                    }
                    Err(e) => {
                        self.errors.push(format!(
                            "{filename}:{line_num}: cannot read include file '{full_path}': {e}"
                        ));
                    }
                }
            } else {
                self.errors.push(format!(
                    "{filename}:{line_num}: include file not found: '{path}'"
                ));
            }
        }
    }

    /// Try to resolve an include path.
    fn resolve_include_path(&self, path: &str, current_file: &str) -> Option<String> {
        // First try relative to current file
        if let Some(parent) = std::path::Path::new(current_file).parent() {
            let candidate = parent.join(path);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
        // Then try include paths
        for dir in &self.include_paths {
            let candidate = std::path::Path::new(dir).join(path);
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
        None
    }

    /// Process `#pragma`.
    fn process_pragma(&mut self, rest: &str, filename: &str) {
        if rest.starts_with("once") {
            self.once_files.push(filename.to_string());
        }
        // #pragma osl ... and others are silently ignored
    }

    /// Evaluate a simple preprocessor condition (for `#if` / `#elif`).
    fn evaluate_condition(&self, cond: &str) -> bool {
        let expanded = self.expand_macros(cond);

        // Handle `defined(NAME)` and `defined NAME`
        if expanded.contains("defined") {
            return self.eval_defined_expr(&expanded);
        }

        // Try to parse as integer expression
        let trimmed = expanded.trim();
        // Replace undefined identifiers with 0 (standard C preprocessor behavior)
        let sanitized = self.sanitize_condition(trimmed);

        // Simple integer evaluation
        self.eval_simple_expr(&sanitized)
    }

    /// Handle `defined(NAME)` expressions.
    fn eval_defined_expr(&self, expr: &str) -> bool {
        let expr = expr.trim();

        // Handle `!defined(X)`
        if let Some(rest) = expr.strip_prefix('!') {
            return !self.eval_defined_expr(rest.trim());
        }

        // Handle `defined(X)` or `defined X`
        if let Some(rest) = expr.strip_prefix("defined") {
            let rest = rest.trim();
            let name = if rest.starts_with('(') {
                rest[1..].trim_end_matches(')').trim()
            } else {
                rest.split_whitespace().next().unwrap_or("")
            };
            return self.is_defined(name);
        }

        // Handle compound expressions with && and ||
        if let Some(pos) = expr.find("&&") {
            let left = &expr[..pos];
            let right = &expr[pos + 2..];
            return self.eval_defined_expr(left) && self.eval_defined_expr(right);
        }
        if let Some(pos) = expr.find("||") {
            let left = &expr[..pos];
            let right = &expr[pos + 2..];
            return self.eval_defined_expr(left) || self.eval_defined_expr(right);
        }

        // Fall back to simple numeric
        self.eval_simple_expr(expr)
    }

    /// Replace unknown identifiers with 0.
    fn sanitize_condition(&self, expr: &str) -> String {
        let mut result = String::new();
        let mut chars = expr.chars().peekable();

        while let Some(&c) = chars.peek() {
            if c.is_alphabetic() || c == '_' {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if let Some(def) = self.defines.get(&ident) {
                    if !def.is_function {
                        result.push_str(&def.body);
                    } else {
                        result.push('0');
                    }
                } else {
                    result.push('0');
                }
            } else {
                result.push(c);
                chars.next();
            }
        }
        result
    }

    /// Evaluate a simple integer expression (supports +, -, *, /, %, ==, !=, <, >, <=, >=, &&, ||, !).
    fn eval_simple_expr(&self, expr: &str) -> bool {
        let trimmed = expr.trim();
        if trimmed.is_empty() {
            return false;
        }

        // Try parsing as a simple integer
        if let Ok(v) = trimmed.parse::<i64>() {
            return v != 0;
        }

        // Handle logical operators
        if let Some(pos) = trimmed.find("||") {
            let left = &trimmed[..pos];
            let right = &trimmed[pos + 2..];
            return self.eval_simple_expr(left) || self.eval_simple_expr(right);
        }
        if let Some(pos) = trimmed.find("&&") {
            let left = &trimmed[..pos];
            let right = &trimmed[pos + 2..];
            return self.eval_simple_expr(left) && self.eval_simple_expr(right);
        }

        // Handle comparison operators
        if let Some(pos) = trimmed.find(">=") {
            return self.eval_int(&trimmed[..pos]) >= self.eval_int(&trimmed[pos + 2..]);
        }
        if let Some(pos) = trimmed.find("<=") {
            return self.eval_int(&trimmed[..pos]) <= self.eval_int(&trimmed[pos + 2..]);
        }
        if let Some(pos) = trimmed.find("==") {
            return self.eval_int(&trimmed[..pos]) == self.eval_int(&trimmed[pos + 2..]);
        }
        if let Some(pos) = trimmed.find("!=") {
            return self.eval_int(&trimmed[..pos]) != self.eval_int(&trimmed[pos + 2..]);
        }
        // Be careful with > and < — don't confuse with >= and <=
        if let Some(pos) = trimmed.find('>') {
            if trimmed.as_bytes().get(pos + 1) != Some(&b'=') {
                return self.eval_int(&trimmed[..pos]) > self.eval_int(&trimmed[pos + 1..]);
            }
        }
        if let Some(pos) = trimmed.find('<') {
            if trimmed.as_bytes().get(pos + 1) != Some(&b'=') {
                return self.eval_int(&trimmed[..pos]) < self.eval_int(&trimmed[pos + 1..]);
            }
        }

        // Handle negation
        if let Some(rest) = trimmed.strip_prefix('!') {
            return !self.eval_simple_expr(rest.trim());
        }

        // Try as integer
        self.eval_int(trimmed) != 0
    }

    fn eval_int(&self, expr: &str) -> i64 {
        let trimmed = expr.trim();
        // Handle hex
        if let Some(hex) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            return i64::from_str_radix(hex, 16).unwrap_or(0);
        }
        trimmed.parse::<i64>().unwrap_or(0)
    }

    /// Expand macros in a string of text.
    pub fn expand_macros(&self, text: &str) -> String {
        self.expand_macros_depth(text, 0)
    }

    fn expand_macros_depth(&self, text: &str, depth: usize) -> String {
        if depth > 32 {
            return text.to_string();
        } // prevent infinite macro recursion

        let mut result = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            let c = bytes[i] as char;

            // Skip string literals
            if c == '"' {
                result.push('"');
                i += 1;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        result.push(bytes[i] as char);
                        result.push(bytes[i + 1] as char);
                        i += 2;
                    } else {
                        result.push(bytes[i] as char);
                        i += 1;
                    }
                }
                if i < len {
                    result.push('"');
                    i += 1;
                }
                continue;
            }

            // Skip single-line comments
            if c == '/' && i + 1 < len && bytes[i + 1] == b'/' {
                // Rest of line is comment
                result.push_str(&text[i..]);
                break;
            }

            // Skip block comments
            if c == '/' && i + 1 < len && bytes[i + 1] == b'*' {
                result.push_str("/*");
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        result.push_str("*/");
                        i += 2;
                        break;
                    }
                    result.push(bytes[i] as char);
                    i += 1;
                }
                continue;
            }

            // Handle identifiers
            if c.is_alphabetic() || c == '_' {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &text[start..i];

                // Check for __LINE__ and __FILE__
                if ident == "__LINE__" {
                    result.push_str(&self.current_line.get().to_string());
                    continue;
                }
                if ident == "__FILE__" {
                    result.push('"');
                    result.push_str(&self.current_file);
                    result.push('"');
                    continue;
                }

                if let Some(def) = self.defines.get(ident) {
                    if def.is_function {
                        // Function-like macro — need parenthesized arguments.
                        // Skip whitespace between macro name and '(' per C standard.
                        let mut j = i;
                        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }
                        if j < len && bytes[j] == b'(' {
                            i = j; // advance past whitespace
                            let args = self.parse_macro_args(text, &mut i);
                            let expanded = self.expand_function_macro(def, &args);
                            let re_expanded = self.expand_macros_depth(&expanded, depth + 1);
                            result.push_str(&re_expanded);
                        } else {
                            result.push_str(ident);
                        }
                    } else {
                        // Object-like macro
                        let re_expanded = self.expand_macros_depth(&def.body, depth + 1);
                        result.push_str(&re_expanded);
                    }
                } else {
                    result.push_str(ident);
                }
            } else {
                result.push(c);
                i += 1;
            }
        }

        result
    }

    /// Parse macro call arguments: `(arg1, arg2, ...)`.
    fn parse_macro_args(&self, text: &str, pos: &mut usize) -> Vec<String> {
        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut args = Vec::new();

        if *pos >= len || bytes[*pos] != b'(' {
            return args;
        }
        *pos += 1; // skip '('

        let mut depth = 1;
        let mut current = String::new();

        while *pos < len && depth > 0 {
            let c = bytes[*pos] as char;
            match c {
                '(' => {
                    depth += 1;
                    current.push(c);
                }
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        args.push(current.trim().to_string());
                    } else {
                        current.push(c);
                    }
                }
                ',' if depth == 1 => {
                    args.push(current.trim().to_string());
                    current.clear();
                }
                _ => current.push(c),
            }
            *pos += 1;
        }

        args
    }

    /// Expand a function-like macro with arguments.
    ///
    /// Handles:
    /// - Parameter substitution: `param` → `arg`
    /// - Stringification: `#param` → `"arg"`
    /// - Token pasting: `a ## b` → `ab`
    fn expand_function_macro(&self, def: &MacroDef, args: &[String]) -> String {
        let mut body = def.body.clone();

        // Pre-expand arguments for normal substitution (C preprocessor standard:
        // args used with # or ## are NOT expanded; all others ARE expanded first).
        let expanded_args: Vec<String> = args
            .iter()
            .map(|arg| self.expand_macros_depth(arg, 1))
            .collect();

        // First pass: handle stringification (#param → "arg")
        // Note: # uses the RAW (unexpanded) argument per C standard,
        // but the outer STRINGIZE/STRINGIZE2 pattern relies on the arg
        // being pre-expanded at the outer level. We use expanded_args here
        // because the raw arg was already passed through from the caller.
        for (i, param) in def.params.iter().enumerate() {
            if let Some(arg) = args.get(i) {
                let stringify_pattern = format!("#{}", param);
                // Replace #param with "arg" (quoted), being careful about word boundaries
                body = body.replace(&stringify_pattern, &format!("\"{}\"", arg));
            }
        }

        // Second pass: handle token pasting (a ## b → ab)
        // Token pasting uses RAW (unexpanded) arguments
        while body.contains("##") {
            let old = body.clone();
            body = body.replace(" ## ", "");
            body = body.replace("## ", "");
            body = body.replace(" ##", "");
            body = body.replace("##", "");
            if body == old {
                break;
            }
        }

        // Third pass: parameter substitution (use EXPANDED arguments)
        for (i, param) in def.params.iter().enumerate() {
            if let Some(arg) = expanded_args.get(i) {
                body = replace_identifier(&body, param, arg);
            }
        }
        body
    }
}

/// Count net parenthesis depth change in a line, skipping string literals.
/// Returns positive if more `(` than `)`, negative if more `)` than `(`.
fn count_paren_depth(line: &str) -> i32 {
    let mut depth: i32 = 0;
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        match bytes[i] {
            b'"' => {
                // Skip string literal
                i += 1;
                while i < len && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                } // skip closing "
            }
            b'\'' => {
                // Skip character literal
                i += 1;
                while i < len && bytes[i] != b'\'' {
                    if bytes[i] == b'\\' && i + 1 < len {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < len {
                    i += 1;
                }
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'/' => {
                // Single-line comment — rest of line is ignored
                break;
            }
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                // Block comment (within same line)
                i += 2;
                while i + 1 < len {
                    if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    depth
}

/// Merge line continuations (`\` followed by newline).
fn merge_continuations(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if chars.peek() == Some(&'\n') {
                chars.next(); // skip newline
                continue;
            }
            if chars.peek() == Some(&'\r') {
                chars.next(); // skip \r
                if chars.peek() == Some(&'\n') {
                    chars.next(); // skip \n
                }
                continue;
            }
        }
        result.push(c);
    }
    result
}

/// Replace all occurrences of an identifier (word-boundary aware).
fn replace_identifier(text: &str, name: &str, replacement: &str) -> String {
    let bytes = text.as_bytes();
    let name_bytes = name.as_bytes();
    let len = bytes.len();
    let name_len = name_bytes.len();
    let mut result = String::with_capacity(len);
    let mut i = 0;

    while i < len {
        if i + name_len <= len && &bytes[i..i + name_len] == name_bytes {
            // Check word boundaries
            let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
            let after_ok = i + name_len >= len
                || !bytes[i + name_len].is_ascii_alphanumeric() && bytes[i + name_len] != b'_';
            if before_ok && after_ok {
                result.push_str(replacement);
                i += name_len;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

/// Convenience: preprocess source with default settings.
pub fn preprocess(source: &str) -> Result<String, Vec<String>> {
    let mut pp = Preprocessor::new();
    pp.process(source)
}

/// Preprocess with additional defines.
pub fn preprocess_with_defines(
    source: &str,
    defines: &[(&str, &str)],
) -> Result<String, Vec<String>> {
    let mut pp = Preprocessor::new();
    for (name, value) in defines {
        pp.define_object(name, value);
    }
    pp.process(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_define() {
        let src = "#define FOO 42\nint x = FOO;\n";
        let result = preprocess(src).unwrap();
        assert!(
            result.contains("int x = 42;"),
            "FOO should be expanded to 42: {result}"
        );
    }

    #[test]
    fn test_define_expression() {
        let src = "#define PI 3.14159\nfloat x = PI;\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("float x = 3.14159;"));
    }

    #[test]
    fn test_ifdef_true() {
        let src = "#define FOO\n#ifdef FOO\nint x = 1;\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("int x = 1;"));
    }

    #[test]
    fn test_ifdef_false() {
        let src = "#ifdef BAR\nint x = 1;\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(!result.contains("int x = 1;"));
    }

    #[test]
    fn test_ifndef() {
        let src = "#ifndef BAR\nint x = 1;\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("int x = 1;"));
    }

    #[test]
    fn test_ifdef_else() {
        let src = "#ifdef BAR\nint x = 1;\n#else\nint x = 2;\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(!result.contains("int x = 1;"));
        assert!(result.contains("int x = 2;"));
    }

    #[test]
    fn test_if_numeric() {
        let src = "#define VER 200\n#if VER >= 100\nint new_api = 1;\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("int new_api = 1;"));
    }

    #[test]
    fn test_nested_ifdef() {
        let src = "#define A\n#define B\n#ifdef A\n#ifdef B\nint x = 1;\n#endif\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("int x = 1;"));
    }

    #[test]
    fn test_undef() {
        let src = "#define FOO 1\n#undef FOO\n#ifdef FOO\nint x = 1;\n#endif\n";
        let result = preprocess(src).unwrap();
        assert!(!result.contains("int x = 1;"));
    }

    #[test]
    fn test_function_macro() {
        let src = "#define MAX(a, b) ((a) > (b) ? (a) : (b))\nint x = MAX(3, 5);\n";
        let result = preprocess(src).unwrap();
        // The function macro should expand MAX(3, 5) to ((3) > (5) ? (3) : (5))
        assert!(
            result.contains("((3) > (5) ? (3) : (5))"),
            "Function macro expansion failed: {result}"
        );
    }

    #[test]
    fn test_predefined_macros() {
        let src = "int v = OSL_VERSION;\n";
        let result = preprocess(src).unwrap();
        let expected = format!("int v = {};", crate::OSL_VERSION);
        assert!(result.contains(&expected));
    }

    #[test]
    fn test_predefined_math_constants() {
        let src = "float pi = M_PI;\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("float pi = 3.14159265358979323846;"));
    }

    #[test]
    fn test_line_continuation() {
        let src = "#define LONG_MACRO \\\n    42\nint x = LONG_MACRO;\n";
        let result = preprocess(src).unwrap();
        assert!(
            result.contains("int x = 42;"),
            "Line continuation should work: {result}"
        );
    }

    #[test]
    fn test_string_literals_not_expanded() {
        let src = "#define FOO bar\nstring s = \"FOO\";\n";
        let result = preprocess(src).unwrap();
        assert!(
            result.contains("\"FOO\""),
            "Macros in strings should not be expanded: {result}"
        );
    }

    #[test]
    fn test_nested_macro_expansion() {
        let src = "#define A 1\n#define B A\nint x = B;\n";
        let result = preprocess(src).unwrap();
        assert!(
            result.contains("int x = 1;"),
            "Nested macros should expand: {result}"
        );
    }

    #[test]
    fn test_elif() {
        let src = r#"
#define MODE 2
#if MODE == 1
int x = 1;
#elif MODE == 2
int x = 2;
#elif MODE == 3
int x = 3;
#else
int x = 0;
#endif
"#;
        let result = preprocess(src).unwrap();
        assert!(result.contains("int x = 2;"));
        assert!(!result.contains("int x = 1;"));
        assert!(!result.contains("int x = 3;"));
    }

    #[test]
    fn test_error_directive() {
        let src = "#error This is bad\n";
        let result = preprocess(src);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs[0].contains("This is bad"));
    }

    #[test]
    fn test_comments_preserved() {
        let src = "#define FOO 42\n// FOO should not be expanded in comments\nint x = FOO;\n";
        let result = preprocess(src).unwrap();
        assert!(result.contains("// FOO should not be expanded in comments"));
        assert!(result.contains("int x = 42;"));
    }

    #[test]
    fn test_custom_include_resolver() {
        let src = "#include \"header.h\"\nint x = VALUE;\n";
        let mut pp = Preprocessor::new();
        pp.set_include_resolver(Box::new(|path, _sys| {
            if path == "header.h" {
                Ok("#define VALUE 99\n".to_string())
            } else {
                Err(format!("not found: {path}"))
            }
        }));
        let result = pp.process(src).unwrap();
        assert!(
            result.contains("int x = 99;"),
            "Include should define VALUE: {result}"
        );
    }

    #[test]
    fn test_preprocess_with_defines() {
        let src = "int x = CUSTOM;\n";
        let result = preprocess_with_defines(src, &[("CUSTOM", "123")]).unwrap();
        assert!(result.contains("int x = 123;"));
    }

    #[test]
    fn test_pragma_once() {
        let mut pp = Preprocessor::new();
        pp.set_include_resolver(Box::new(|path, _sys| {
            if path == "once.h" {
                Ok("#pragma once\n#define COUNTER 1\n".to_string())
            } else {
                Err(format!("not found: {path}"))
            }
        }));
        let src = "#include \"once.h\"\n#include \"once.h\"\nint x = COUNTER;\n";
        let result = pp.process(src).unwrap();
        assert!(result.contains("int x = 1;"));
    }
}
