//! Simple regression test support.
//!
//! This module provides a system for registering and running regression tests
//! from a command-line interface. It's useful for standalone test executables
//! that can run individual tests by name.
//!
//! # Overview
//!
//! `RegTest` is a singleton that maintains a registry of test functions.
//! Tests can be registered with or without arguments, and run by name.
//!
//! # Examples
//!
//! ```
//! use usd_tf::reg_test::RegTest;
//!
//! // Register a test without arguments
//! RegTest::register("my_test", || {
//!     println!("Running my_test");
//!     true
//! });
//!
//! // Register a test with arguments
//! RegTest::register_with_args("my_test_args", |args| {
//!     println!("Running my_test_args with {} args", args.len());
//!     true
//! });
//!
//! // List all registered tests
//! let tests = RegTest::test_names();
//! assert!(tests.contains(&"my_test".to_string()));
//! ```
//!
//! # Command-Line Usage
//!
//! ```ignore
//! // In main.rs:
//! use usd_tf::reg_test::RegTest;
//!
//! fn main() {
//!     std::process::exit(RegTest::main());
//! }
//! ```
//!
//! Then run: `./program test_name [args...]`

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Type for test functions without arguments.
pub type RegFunc = fn() -> bool;

/// Type for test functions with arguments.
pub type RegFuncWithArgs = fn(&[String]) -> bool;

/// Internal registry data.
#[derive(Default)]
struct RegTestData {
    /// Functions without arguments.
    functions: HashMap<String, RegFunc>,
    /// Functions with arguments.
    functions_with_args: HashMap<String, RegFuncWithArgs>,
}

/// Global registry instance.
static REGISTRY: OnceLock<Mutex<RegTestData>> = OnceLock::new();

fn get_registry() -> &'static Mutex<RegTestData> {
    REGISTRY.get_or_init(|| Mutex::new(RegTestData::default()))
}

/// Singleton for registering and running regression tests.
///
/// # Examples
///
/// ```
/// use usd_tf::reg_test::RegTest;
///
/// // Register tests
/// RegTest::register("basic", || true);
/// RegTest::register_with_args("with_args", |_| true);
///
/// // Run a test by name
/// let result = RegTest::run("basic", &[]);
/// assert_eq!(result, Some(true));
/// ```
pub struct RegTest;

impl RegTest {
    /// Registers a test function without arguments.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::reg_test::RegTest;
    ///
    /// RegTest::register("test_addition", || {
    ///     2 + 2 == 4
    /// });
    /// ```
    pub fn register(name: &str, func: RegFunc) {
        if let Ok(mut guard) = get_registry().lock() {
            guard.functions.insert(name.to_string(), func);
        }
    }

    /// Registers a test function that takes arguments.
    ///
    /// The function receives command-line arguments (excluding program name
    /// and test name).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::reg_test::RegTest;
    ///
    /// RegTest::register_with_args("test_with_input", |args| {
    ///     if args.is_empty() {
    ///         eprintln!("Need at least one argument");
    ///         return false;
    ///     }
    ///     true
    /// });
    /// ```
    pub fn register_with_args(name: &str, func: RegFuncWithArgs) {
        if let Ok(mut guard) = get_registry().lock() {
            guard.functions_with_args.insert(name.to_string(), func);
        }
    }

    /// Checks if a test is registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::reg_test::RegTest;
    ///
    /// RegTest::register("exists_test", || true);
    /// assert!(RegTest::is_registered("exists_test"));
    /// assert!(!RegTest::is_registered("nonexistent"));
    /// ```
    pub fn is_registered(name: &str) -> bool {
        if let Ok(guard) = get_registry().lock() {
            guard.functions.contains_key(name) || guard.functions_with_args.contains_key(name)
        } else {
            false
        }
    }

    /// Returns a sorted list of all registered test names.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::reg_test::RegTest;
    ///
    /// RegTest::register("test_a", || true);
    /// RegTest::register("test_b", || true);
    ///
    /// let names = RegTest::test_names();
    /// // Names are sorted alphabetically
    /// ```
    pub fn test_names() -> Vec<String> {
        let Ok(guard) = get_registry().lock() else {
            return Vec::new();
        };

        let mut names: Vec<String> = guard
            .functions
            .keys()
            .chain(guard.functions_with_args.keys())
            .cloned()
            .collect();

        names.sort();
        names
    }

    /// Returns the number of registered tests.
    pub fn count() -> usize {
        if let Ok(guard) = get_registry().lock() {
            guard.functions.len() + guard.functions_with_args.len()
        } else {
            0
        }
    }

    /// Runs a test by name.
    ///
    /// Returns `Some(true)` if the test passed, `Some(false)` if it failed,
    /// or `None` if the test was not found.
    ///
    /// # Arguments
    ///
    /// * `name` - The test name
    /// * `args` - Arguments to pass (ignored for tests without args)
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::reg_test::RegTest;
    ///
    /// RegTest::register("run_test", || true);
    ///
    /// let result = RegTest::run("run_test", &[]);
    /// assert_eq!(result, Some(true));
    ///
    /// let result = RegTest::run("nonexistent", &[]);
    /// assert_eq!(result, None);
    /// ```
    pub fn run(name: &str, args: &[String]) -> Option<bool> {
        let guard = get_registry().lock().ok()?;

        // Check no-args functions first
        if let Some(func) = guard.functions.get(name) {
            return Some(func());
        }

        // Check functions with args
        if let Some(func) = guard.functions_with_args.get(name) {
            return Some(func(args));
        }

        None
    }

    /// Main entry point for test executables.
    ///
    /// Parses command-line arguments and runs the specified test.
    ///
    /// # Returns
    ///
    /// * `0` - Test passed
    /// * `1` - Test failed
    /// * `2` - Usage error
    /// * `3` - Unknown test
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // In main.rs:
    /// fn main() {
    ///     std::process::exit(RegTest::main());
    /// }
    /// ```
    pub fn main() -> i32 {
        let args: Vec<String> = std::env::args().collect();
        Self::main_with_args(&args)
    }

    /// Main entry point with explicit arguments (for testing).
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::reg_test::RegTest;
    ///
    /// RegTest::register("cli_test", || true);
    ///
    /// // Simulate: program cli_test
    /// let result = RegTest::main_with_args(&[
    ///     "program".to_string(),
    ///     "cli_test".to_string(),
    /// ]);
    /// assert_eq!(result, 0); // Test passed
    /// ```
    pub fn main_with_args(args: &[String]) -> i32 {
        let prog_name = args.first().map(|s| s.as_str()).unwrap_or("test");

        if args.len() < 2 {
            Self::print_usage(prog_name);
            Self::print_test_names();
            return 2;
        }

        let test_name = &args[1];
        let test_args: Vec<String> = args.iter().skip(2).cloned().collect();

        let Ok(guard) = get_registry().lock() else {
            eprintln!("Failed to access test registry");
            return 1;
        };

        // Test without args
        if let Some(func) = guard.functions.get(test_name) {
            if !test_args.is_empty() {
                eprintln!(
                    "{}: test function '{}' takes no arguments.",
                    prog_name, test_name
                );
                return 2;
            }
            return if func() { 0 } else { 1 };
        }

        // Test with args
        if let Some(func) = guard.functions_with_args.get(test_name) {
            return if func(&test_args) { 0 } else { 1 };
        }

        // Unknown test
        drop(guard);
        eprintln!("{}: unknown test function '{}'.", prog_name, test_name);
        Self::print_test_names();
        3
    }

    /// Prints usage information to stderr.
    fn print_usage(prog_name: &str) {
        eprintln!("Usage: {} testName [args]", prog_name);
    }

    /// Prints all registered test names to stderr.
    fn print_test_names() {
        let names = Self::test_names();
        if names.is_empty() {
            eprintln!("No tests registered.");
        } else {
            eprintln!("Valid tests are:");
            for name in &names {
                eprintln!("    {}", name);
            }
        }
    }
}

/// Macro to add a regression test function.
///
/// The function must be named `test_<name>` and return `bool`.
///
/// # Examples
///
/// ```ignore
/// use usd_tf::tf_add_regtest;
///
/// fn test_my_feature() -> bool {
///     // Test code...
///     true
/// }
///
/// tf_add_regtest!(my_feature);
/// ```
#[macro_export]
macro_rules! tf_add_regtest {
    ($name:ident) => {
        paste::paste! {
            #[allow(non_upper_case_globals)]
            static [<_TF_REGTEST_ $name>]: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

            #[allow(dead_code)]
            fn [<_tf_register_test_ $name>]() {
                [<_TF_REGTEST_ $name>].get_or_init(|| {
                    $crate::reg_test::RegTest::register(
                        stringify!($name),
                        [<test_ $name>]
                    );
                    true
                });
            }
        }
    };
}

pub use tf_add_regtest;

#[cfg(test)]
mod tests {
    use super::*;

    // Each test uses unique names to avoid parallel test interference

    #[test]
    fn test_register_and_run() {
        RegTest::register("unique_test_simple_1", || true);
        assert!(RegTest::is_registered("unique_test_simple_1"));

        let result = RegTest::run("unique_test_simple_1", &[]);
        assert_eq!(result, Some(true));
    }

    #[test]
    fn test_register_with_args() {
        RegTest::register_with_args("unique_test_args_2", |args| args.len() >= 2);

        let result = RegTest::run("unique_test_args_2", &["a".to_string(), "b".to_string()]);
        assert_eq!(result, Some(true));

        let result = RegTest::run("unique_test_args_2", &["a".to_string()]);
        assert_eq!(result, Some(false));
    }

    #[test]
    fn test_unknown_test() {
        let result = RegTest::run("nonexistent_unique_3", &[]);
        assert_eq!(result, None);
    }

    #[test]
    fn test_test_names() {
        // Register with unique prefix
        RegTest::register("names_zebra_4", || true);
        RegTest::register("names_apple_4", || true);
        RegTest::register_with_args("names_banana_4", |_| true);

        let names = RegTest::test_names();
        // Just check our tests are in there
        assert!(names.contains(&"names_zebra_4".to_string()));
        assert!(names.contains(&"names_apple_4".to_string()));
        assert!(names.contains(&"names_banana_4".to_string()));
    }

    #[test]
    fn test_count() {
        let initial = RegTest::count();

        RegTest::register("count_test1_5", || true);
        // Use >= because other parallel tests may also register entries
        assert!(RegTest::count() >= initial + 1);

        RegTest::register_with_args("count_test2_5", |_| true);
        assert!(RegTest::count() >= initial + 2);
    }

    #[test]
    fn test_main_with_args_success() {
        RegTest::register("main_test_6", || true);

        let result = RegTest::main_with_args(&["prog".to_string(), "main_test_6".to_string()]);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_main_with_args_failure() {
        RegTest::register("fail_test_7", || false);

        let result = RegTest::main_with_args(&["prog".to_string(), "fail_test_7".to_string()]);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_main_with_args_unknown() {
        let result = RegTest::main_with_args(&["prog".to_string(), "unknown_unique_8".to_string()]);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_main_no_args() {
        let result = RegTest::main_with_args(&["prog".to_string()]);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_main_no_args_func_with_args() {
        RegTest::register("no_args_test_9", || true);

        // Passing args to a no-args test should fail
        let result = RegTest::main_with_args(&[
            "prog".to_string(),
            "no_args_test_9".to_string(),
            "extra".to_string(),
        ]);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_main_with_args_func() {
        RegTest::register_with_args("args_test_10", |args| args.len() == 2);

        let result = RegTest::main_with_args(&[
            "prog".to_string(),
            "args_test_10".to_string(),
            "arg1".to_string(),
            "arg2".to_string(),
        ]);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_failing_test() {
        RegTest::register("always_fail_11", || false);

        let result = RegTest::run("always_fail_11", &[]);
        assert_eq!(result, Some(false));
    }
}
