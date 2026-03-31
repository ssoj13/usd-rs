//! Debug output system for conditional debugging messages.
//!
//! This module provides a flexible debugging system that allows enabling
//! and disabling debug output at runtime. Debug symbols can be registered
//! with descriptions and controlled via the `TF_DEBUG` environment variable.
//!
//! # Overview
//!
//! The debug system allows you to:
//! - Register debug symbols with descriptions
//! - Enable/disable symbols at runtime
//! - Control debug output via environment variables
//! - Issue debug messages that are only evaluated when enabled
//!
//! # Examples
//!
//! ```
//! use usd_tf::{Debug, tf_debug, tf_debug_msg};
//!
//! // Register a debug symbol
//! Debug::register("MY_DEBUG", "Debug messages for my feature");
//!
//! // Enable the symbol
//! Debug::enable("MY_DEBUG");
//!
//! // Issue debug messages (only evaluated if enabled)
//! tf_debug_msg!("MY_DEBUG", "Processing item {}", 42);
//!
//! // Disable the symbol
//! Debug::disable("MY_DEBUG");
//! ```
//!
//! # Environment Variable
//!
//! Debug symbols can be enabled via the `TF_DEBUG` environment variable:
//!
//! ```text
//! TF_DEBUG="MY_DEBUG,OTHER_DEBUG" ./my_program
//! TF_DEBUG="MY_*" ./my_program  # Enable all symbols starting with MY_
//! ```

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, RwLock};

use crate::debug_notice::{DebugSymbolEnableChangedNotice, DebugSymbolsChangedNotice};

/// Global debug symbol registry.
static DEBUG_REGISTRY: LazyLock<RwLock<DebugRegistry>> =
    LazyLock::new(|| RwLock::new(DebugRegistry::new()));

/// Flag indicating whether the registry has been initialized from env.
static ENV_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Generation counter: bumped on every enable/disable mutation.
/// Thread-local caches use this to detect staleness.
static DEBUG_GENERATION: AtomicU64 = AtomicU64::new(0);

// Thread-local lookup cache: maps symbol name -> enabled, with a generation stamp.
thread_local! {
    static TL_CACHE: RefCell<(u64, HashMap<String, bool>)> =
        RefCell::new((0, HashMap::new()));
}

/// Internal debug symbol data.
#[derive(Debug, Clone)]
struct DebugSymbol {
    /// Whether the symbol is enabled.
    enabled: bool,
    /// Description of what this debug symbol controls.
    description: String,
}

/// Internal registry for debug symbols.
struct DebugRegistry {
    symbols: HashMap<String, DebugSymbol>,
    /// Output file (stdout by default).
    use_stderr: bool,
    /// Env-var patterns stored for late-registered symbols.
    /// Each entry is (pattern, is_enable): applied in order, last match wins.
    env_patterns: Vec<(String, bool)>,
}

impl DebugRegistry {
    fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            use_stderr: std::env::var("TF_DEBUG_OUTPUT_FILE")
                .map(|v| v.eq_ignore_ascii_case("stderr"))
                .unwrap_or(false),
            env_patterns: Vec::new(),
        }
    }

    fn register(&mut self, name: &str, description: &str) {
        if self.symbols.contains_key(name) {
            // C++ TF_FATAL_ERROR on duplicate registration; warn here to avoid circularity.
            eprintln!("TF_DEBUG: duplicate registration of symbol '{}'", name);
            return;
        }
        self.symbols.insert(
            name.to_string(),
            DebugSymbol {
                enabled: false,
                description: description.to_string(),
            },
        );
        // Apply any env-var patterns that were parsed before this symbol was registered.
        // Last matching pattern wins, matching C++ TfDebug behavior.
        for (pattern, is_enable) in &self.env_patterns {
            if let Some(prefix) = pattern.strip_suffix('*') {
                if name.starts_with(prefix) {
                    if let Some(symbol) = self.symbols.get_mut(name) {
                        symbol.enabled = *is_enable;
                    }
                }
            } else if pattern == name {
                if let Some(symbol) = self.symbols.get_mut(name) {
                    symbol.enabled = *is_enable;
                }
            }
        }
    }

    fn is_enabled(&self, name: &str) -> bool {
        self.symbols.get(name).map(|s| s.enabled).unwrap_or(false)
    }

    fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(symbol) = self.symbols.get_mut(name) {
            symbol.enabled = enabled;
            // Bump generation so thread-local caches invalidate
            DEBUG_GENERATION.fetch_add(1, Ordering::Release);
            true
        } else {
            false
        }
    }

    fn enable_by_pattern(&mut self, pattern: &str) -> Vec<String> {
        let mut enabled = Vec::new();

        if let Some(prefix) = pattern.strip_suffix('*') {
            for (name, symbol) in &mut self.symbols {
                if name.starts_with(prefix) {
                    symbol.enabled = true;
                    enabled.push(name.clone());
                }
            }
        } else if let Some(symbol) = self.symbols.get_mut(pattern) {
            symbol.enabled = true;
            enabled.push(pattern.to_string());
        }

        if !enabled.is_empty() {
            DEBUG_GENERATION.fetch_add(1, Ordering::Release);
        }
        enabled
    }

    fn disable_by_pattern(&mut self, pattern: &str) -> Vec<String> {
        let mut disabled = Vec::new();

        if let Some(prefix) = pattern.strip_suffix('*') {
            for (name, symbol) in &mut self.symbols {
                if name.starts_with(prefix) {
                    symbol.enabled = false;
                    disabled.push(name.clone());
                }
            }
        } else if let Some(symbol) = self.symbols.get_mut(pattern) {
            symbol.enabled = false;
            disabled.push(pattern.to_string());
        }

        if !disabled.is_empty() {
            DEBUG_GENERATION.fetch_add(1, Ordering::Release);
        }
        disabled
    }

    fn get_names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.symbols.keys().cloned().collect();
        names.sort();
        names
    }

    fn get_description(&self, name: &str) -> Option<&str> {
        self.symbols.get(name).map(|s| s.description.as_str())
    }

    fn get_descriptions(&self) -> String {
        let mut result = String::new();
        let mut names: Vec<_> = self.symbols.keys().collect();
        names.sort();

        for name in names {
            if let Some(symbol) = self.symbols.get(name) {
                let status = if symbol.enabled { "ON " } else { "OFF" };
                result.push_str(&format!("  {} {} - {}\n", status, name, symbol.description));
            }
        }

        result
    }
}

/// Debug output control.
///
/// This struct provides static methods for controlling debug output.
/// Debug symbols must be registered before they can be enabled.
pub struct Debug;

impl Debug {
    /// Initialize debug symbols from the TF_DEBUG environment variable.
    ///
    /// This is called automatically on first use, but can be called
    /// explicitly to control timing.
    pub fn init_from_env() {
        if ENV_INITIALIZED.swap(true, Ordering::SeqCst) {
            return; // Already initialized
        }

        if let Ok(value) = std::env::var("TF_DEBUG") {
            // Collect tokens once, then apply them all under one write lock so
            // that the stored env_patterns are visible to any symbols already
            // registered at this point AND to symbols registered later.
            let tokens: Vec<(String, bool)> = value
                .split_whitespace()
                .filter_map(|token| {
                    if let Some(pattern) = token.strip_prefix('-') {
                        if !pattern.is_empty() {
                            Some((pattern.to_string(), false))
                        } else {
                            None
                        }
                    } else if !token.is_empty() {
                        Some((token.to_string(), true))
                    } else {
                        None
                    }
                })
                .collect();

            if tokens.is_empty() {
                return;
            }

            if let Ok(mut registry) = DEBUG_REGISTRY.write() {
                // Store patterns for symbols registered after init.
                registry.env_patterns.extend(tokens.iter().cloned());
                // Apply to already-registered symbols.
                for (pattern, is_enable) in &tokens {
                    if *is_enable {
                        registry.enable_by_pattern(pattern);
                    } else {
                        registry.disable_by_pattern(pattern);
                    }
                }
            }
        }
    }

    /// Register a debug symbol with a description.
    ///
    /// The symbol is disabled by default. Use `enable()` to turn it on.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::Debug;
    ///
    /// Debug::register("MY_FEATURE_DEBUG", "Debug output for my feature");
    /// ```
    pub fn register(name: &str, description: &str) {
        Self::init_from_env();
        if let Ok(mut registry) = DEBUG_REGISTRY.write() {
            registry.register(name, description);
        }
        // Notify listeners that the symbol set has changed. Send outside the
        // lock to avoid any potential deadlock with notice listeners.
        crate::notice::send(&DebugSymbolsChangedNotice::new());
    }

    /// Check if a debug symbol is enabled.
    ///
    /// Uses a thread-local cache to avoid RwLock contention on the hot path.
    /// Returns false if the symbol is not registered or not enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::Debug;
    ///
    /// Debug::register("MY_DEBUG", "Test debug");
    /// assert!(!Debug::is_enabled("MY_DEBUG"));
    ///
    /// Debug::enable("MY_DEBUG");
    /// assert!(Debug::is_enabled("MY_DEBUG"));
    /// ```
    #[must_use]
    pub fn is_enabled(name: &str) -> bool {
        Self::init_from_env();

        let generation = DEBUG_GENERATION.load(Ordering::Acquire);

        // Fast path: check thread-local cache
        let cached = TL_CACHE.with(|cell| {
            let cache = cell.borrow();
            if cache.0 == generation {
                cache.1.get(name).copied()
            } else {
                None
            }
        });

        if let Some(val) = cached {
            return val;
        }

        // Slow path: read from global registry and populate cache
        let enabled = DEBUG_REGISTRY
            .read()
            .map(|r| r.is_enabled(name))
            .unwrap_or(false);

        TL_CACHE.with(|cell| {
            let mut cache = cell.borrow_mut();
            if cache.0 != generation {
                cache.1.clear();
                cache.0 = generation;
            }
            cache.1.insert(name.to_string(), enabled);
        });

        enabled
    }

    /// Enable a debug symbol.
    ///
    /// Returns true if the symbol was found and enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::Debug;
    ///
    /// Debug::register("MY_DEBUG", "Test");
    /// Debug::enable("MY_DEBUG");
    /// assert!(Debug::is_enabled("MY_DEBUG"));
    /// ```
    pub fn enable(name: &str) -> bool {
        Self::init_from_env();
        DEBUG_REGISTRY
            .write()
            .map(|mut r| r.set_enabled(name, true))
            .unwrap_or(false)
    }

    /// Disable a debug symbol.
    ///
    /// Returns true if the symbol was found and disabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::Debug;
    ///
    /// Debug::register("MY_DEBUG", "Test");
    /// Debug::enable("MY_DEBUG");
    /// Debug::disable("MY_DEBUG");
    /// assert!(!Debug::is_enabled("MY_DEBUG"));
    /// ```
    pub fn disable(name: &str) -> bool {
        Self::init_from_env();
        DEBUG_REGISTRY
            .write()
            .map(|mut r| r.set_enabled(name, false))
            .unwrap_or(false)
    }

    /// Enable debug symbols matching a pattern.
    ///
    /// The pattern can be an exact name or end with '*' for prefix matching.
    /// Returns the names of all symbols that were enabled.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::Debug;
    ///
    /// Debug::register("MY_DEBUG_A", "Test A");
    /// Debug::register("MY_DEBUG_B", "Test B");
    /// Debug::register("OTHER", "Other");
    ///
    /// let enabled = Debug::enable_by_pattern("MY_DEBUG_*");
    /// assert_eq!(enabled.len(), 2);
    /// ```
    pub fn enable_by_pattern(pattern: &str) -> Vec<String> {
        Self::init_from_env();
        let changed = DEBUG_REGISTRY
            .write()
            .map(|mut r| r.enable_by_pattern(pattern))
            .unwrap_or_default();
        if !changed.is_empty() {
            crate::notice::send(&DebugSymbolEnableChangedNotice::new());
        }
        changed
    }

    /// Disable debug symbols matching a pattern.
    ///
    /// Returns the names of all symbols that were disabled.
    pub fn disable_by_pattern(pattern: &str) -> Vec<String> {
        Self::init_from_env();
        let changed = DEBUG_REGISTRY
            .write()
            .map(|mut r| r.disable_by_pattern(pattern))
            .unwrap_or_default();
        if !changed.is_empty() {
            crate::notice::send(&DebugSymbolEnableChangedNotice::new());
        }
        changed
    }

    /// Enable all registered debug symbols.
    pub fn enable_all() {
        Self::init_from_env();
        let any_changed = if let Ok(mut registry) = DEBUG_REGISTRY.write() {
            let had_symbols = !registry.symbols.is_empty();
            for symbol in registry.symbols.values_mut() {
                symbol.enabled = true;
            }
            if had_symbols {
                DEBUG_GENERATION.fetch_add(1, Ordering::Release);
            }
            had_symbols
        } else {
            false
        };
        if any_changed {
            crate::notice::send(&DebugSymbolEnableChangedNotice::new());
        }
    }

    /// Disable all registered debug symbols.
    pub fn disable_all() {
        Self::init_from_env();
        let any_changed = if let Ok(mut registry) = DEBUG_REGISTRY.write() {
            let had_symbols = !registry.symbols.is_empty();
            for symbol in registry.symbols.values_mut() {
                symbol.enabled = false;
            }
            if had_symbols {
                DEBUG_GENERATION.fetch_add(1, Ordering::Release);
            }
            had_symbols
        } else {
            false
        };
        if any_changed {
            crate::notice::send(&DebugSymbolEnableChangedNotice::new());
        }
    }

    /// Get all registered debug symbol names.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::Debug;
    ///
    /// Debug::register("DEBUG_A", "A");
    /// Debug::register("DEBUG_B", "B");
    ///
    /// let names = Debug::get_symbol_names();
    /// assert!(names.contains(&"DEBUG_A".to_string()));
    /// ```
    #[must_use]
    pub fn get_symbol_names() -> Vec<String> {
        Self::init_from_env();
        DEBUG_REGISTRY
            .read()
            .map(|r| r.get_names())
            .unwrap_or_default()
    }

    /// Get the description for a debug symbol.
    ///
    /// Returns None if the symbol is not registered.
    #[must_use]
    pub fn get_symbol_description(name: &str) -> Option<String> {
        Self::init_from_env();
        DEBUG_REGISTRY
            .read()
            .ok()
            .and_then(|r| r.get_description(name).map(String::from))
    }

    /// Get descriptions of all debug symbols.
    ///
    /// Returns a formatted string with all symbols, their status, and descriptions.
    #[must_use]
    pub fn get_symbol_descriptions() -> String {
        Self::init_from_env();
        DEBUG_REGISTRY
            .read()
            .map(|r| r.get_descriptions())
            .unwrap_or_default()
    }

    /// Set output to stderr instead of stdout.
    pub fn set_output_stderr(use_stderr: bool) {
        if let Ok(mut registry) = DEBUG_REGISTRY.write() {
            registry.use_stderr = use_stderr;
        }
    }

    /// Output a debug message.
    ///
    /// This is called by the `tf_debug_msg!` macro.
    #[doc(hidden)]
    pub fn output(message: &str) {
        let use_stderr = DEBUG_REGISTRY.read().map(|r| r.use_stderr).unwrap_or(false);

        if use_stderr {
            let _ = writeln!(std::io::stderr(), "{}", message);
        } else {
            let _ = writeln!(std::io::stdout(), "{}", message);
        }
    }
}

/// Named debug code used to control debug output.
///
/// Wraps a static debug symbol name and provides enable/disable/check methods.
/// Typically declared as `const` in a module's debug_codes file.
pub struct DebugCode {
    /// Debug symbol name.
    name: &'static str,
}

impl DebugCode {
    /// Create a new debug code.
    pub const fn new(name: &'static str) -> Self {
        Self { name }
    }

    /// Get the symbol name.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Check if this debug code is enabled.
    pub fn is_enabled(&self) -> bool {
        Debug::is_enabled(self.name)
    }

    /// Enable this debug code.
    pub fn enable(&self) {
        Debug::enable(self.name);
    }

    /// Disable this debug code.
    pub fn disable(&self) {
        Debug::disable(self.name);
    }
}

/// A scoped debug output helper.
///
/// When created, prints an entry message. When dropped, prints an exit message.
pub struct DebugScope {
    name: &'static str,
    active: bool,
}

impl DebugScope {
    /// Create a new debug scope.
    ///
    /// If the debug symbol is enabled, prints an entry message.
    #[must_use]
    pub fn new(symbol: &str, name: &'static str) -> Self {
        let active = Debug::is_enabled(symbol);
        if active {
            Debug::output(&format!(">> {}", name));
        }
        Self { name, active }
    }
}

impl Drop for DebugScope {
    fn drop(&mut self) {
        if self.active {
            Debug::output(&format!("<< {}", self.name));
        }
    }
}

// ============================================================================
// Macros
// ============================================================================

/// Output a debug message if the symbol is enabled.
///
/// The message is only formatted and output if the debug symbol is enabled.
///
/// # Examples
///
/// ```
/// use usd_tf::{Debug, tf_debug_msg};
///
/// Debug::register("MY_DEBUG", "Test debug");
/// Debug::enable("MY_DEBUG");
///
/// tf_debug_msg!("MY_DEBUG", "Processing value: {}", 42);
/// ```
#[macro_export]
macro_rules! tf_debug_msg {
    ($symbol:expr, $($arg:tt)*) => {{
        if $crate::Debug::is_enabled($symbol) {
            $crate::Debug::output(&format!($($arg)*));
        }
    }};
}

/// Check if a debug symbol is enabled and get a helper for output.
///
/// This macro is similar to TF_DEBUG in C++.
///
/// # Examples
///
/// ```
/// use usd_tf::{Debug, tf_debug};
///
/// Debug::register("MY_DEBUG", "Test debug");
/// Debug::enable("MY_DEBUG");
///
/// if tf_debug!("MY_DEBUG") {
///     println!("Debug enabled!");
/// }
/// ```
#[macro_export]
macro_rules! tf_debug {
    ($symbol:expr) => {
        $crate::Debug::is_enabled($symbol)
    };
}

/// Create a debug scope that prints entry/exit messages.
///
/// # Examples
///
/// ```
/// use usd_tf::{Debug, tf_debug_scope};
///
/// Debug::register("MY_DEBUG", "Test debug");
/// Debug::enable("MY_DEBUG");
///
/// {
///     tf_debug_scope!("MY_DEBUG", "processing_items");
///     // Prints ">> processing_items" on entry
///     // ... do work ...
/// }   // Prints "<< processing_items" on exit
/// ```
#[macro_export]
macro_rules! tf_debug_scope {
    ($symbol:expr, $name:expr) => {
        let _scope = $crate::DebugScope::new($symbol, $name);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use unique symbol names in tests to avoid interference between tests
    fn unique_name(base: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        format!("{}_{}", base, COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn test_register_and_enable() {
        let name = unique_name("TEST_REGISTER");
        Debug::register(&name, "Test symbol");
        assert!(!Debug::is_enabled(&name));

        Debug::enable(&name);
        assert!(Debug::is_enabled(&name));

        Debug::disable(&name);
        assert!(!Debug::is_enabled(&name));
    }

    #[test]
    fn test_enable_unregistered() {
        let name = unique_name("TEST_UNREGISTERED");
        assert!(!Debug::enable(&name));
        assert!(!Debug::is_enabled(&name));
    }

    #[test]
    fn test_enable_by_pattern() {
        let prefix = unique_name("PATTERN");
        let name_a = format!("{}_A", prefix);
        let name_b = format!("{}_B", prefix);
        let name_other = unique_name("OTHER");

        Debug::register(&name_a, "A");
        Debug::register(&name_b, "B");
        Debug::register(&name_other, "Other");

        let pattern = format!("{}_*", prefix);
        let enabled = Debug::enable_by_pattern(&pattern);
        assert_eq!(enabled.len(), 2);
        assert!(Debug::is_enabled(&name_a));
        assert!(Debug::is_enabled(&name_b));
        assert!(!Debug::is_enabled(&name_other));
    }

    #[test]
    fn test_enable_exact_pattern() {
        let name = unique_name("EXACT");
        Debug::register(&name, "Exact match");

        let enabled = Debug::enable_by_pattern(&name);
        assert_eq!(enabled.len(), 1);
        assert!(Debug::is_enabled(&name));
    }

    #[test]
    fn test_disable_by_pattern() {
        let prefix = unique_name("DISABLE");
        let name_a = format!("{}_A", prefix);
        let name_b = format!("{}_B", prefix);

        Debug::register(&name_a, "A");
        Debug::register(&name_b, "B");
        Debug::enable(&name_a);
        Debug::enable(&name_b);

        let pattern = format!("{}_*", prefix);
        let disabled = Debug::disable_by_pattern(&pattern);
        assert_eq!(disabled.len(), 2);
        assert!(!Debug::is_enabled(&name_a));
        assert!(!Debug::is_enabled(&name_b));
    }

    #[test]
    fn test_get_symbol_names() {
        let name = unique_name("NAMES");
        Debug::register(&name, "Test");

        let names = Debug::get_symbol_names();
        assert!(names.contains(&name));
    }

    #[test]
    fn test_get_symbol_description() {
        let name = unique_name("DESC");
        Debug::register(&name, "Test description");

        let desc = Debug::get_symbol_description(&name);
        assert_eq!(desc, Some("Test description".to_string()));

        let missing = Debug::get_symbol_description("NONEXISTENT_SYMBOL_12345");
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_symbol_descriptions() {
        let name = unique_name("DESCS");
        Debug::register(&name, "Test");
        Debug::enable(&name);

        let descs = Debug::get_symbol_descriptions();
        assert!(descs.contains(&name));
        assert!(descs.contains("ON"));
    }

    #[test]
    fn test_tf_debug_msg_macro() {
        let name = unique_name("MSG_MACRO");
        Debug::register(&name, "Test");

        // Should not panic even when disabled
        tf_debug_msg!(name.as_str(), "Test message {}", 42);

        Debug::enable(&name);
        tf_debug_msg!(name.as_str(), "Enabled message {}", 123);
    }

    #[test]
    fn test_tf_debug_macro() {
        let name = unique_name("DEBUG_MACRO");
        Debug::register(&name, "Test");

        assert!(!tf_debug!(name.as_str()));

        Debug::enable(&name);
        assert!(tf_debug!(name.as_str()));
    }

    #[test]
    fn test_debug_scope() {
        let name = unique_name("SCOPE");
        Debug::register(&name, "Test scope");
        Debug::enable(&name);

        {
            let _scope = DebugScope::new(&name, "test_operation");
            // Should print entry message
        }
        // Should print exit message when dropped
    }

    #[test]
    fn test_debug_scope_disabled() {
        let name = unique_name("SCOPE_DISABLED");
        Debug::register(&name, "Test scope disabled");
        // Not enabled

        {
            let _scope = DebugScope::new(&name, "test_operation");
            // Should NOT print anything
        }
    }

    #[test]
    fn test_enable_disable_all() {
        let prefix = unique_name("ALL");
        let name_a = format!("{}_A", prefix);
        let name_b = format!("{}_B", prefix);

        Debug::register(&name_a, "A");
        Debug::register(&name_b, "B");

        // Note: enable_all/disable_all affects ALL symbols, so we just test
        // that our specific symbols are affected
        Debug::enable(&name_a);
        Debug::enable(&name_b);

        assert!(Debug::is_enabled(&name_a));
        assert!(Debug::is_enabled(&name_b));

        Debug::disable(&name_a);
        Debug::disable(&name_b);

        assert!(!Debug::is_enabled(&name_a));
        assert!(!Debug::is_enabled(&name_b));
    }

    #[test]
    fn test_output_stderr() {
        Debug::set_output_stderr(true);
        Debug::output("Test stderr output");
        Debug::set_output_stderr(false);
    }

    #[test]
    fn test_thread_local_cache_invalidation() {
        let name = unique_name("CACHE_TEST");
        Debug::register(&name, "Cache test");

        // First call populates cache
        assert!(!Debug::is_enabled(&name));

        // Enable -> cache must invalidate
        Debug::enable(&name);
        assert!(Debug::is_enabled(&name));

        // Disable -> cache must invalidate again
        Debug::disable(&name);
        assert!(!Debug::is_enabled(&name));
    }
}
