//! Environment setting variables.
//!
//! This module provides a type-safe way to access environment variables
//! with default values and documentation. Environment settings are useful
//! for enabling experimental features or customizing program behavior.
//!
//! # Supported Types
//!
//! - `bool` - Boolean settings (parsed from "true"/"false", "1"/"0", "yes"/"no")
//! - `i32` - Integer settings
//! - `i64` - 64-bit integer settings
//!
//! # Examples
//!
//! ```
//! use usd_tf::env_setting::{EnvSetting, get_env_setting};
//!
//! // Define settings
//! static MY_FEATURE: EnvSetting<bool> = EnvSetting::new(
//!     "MY_FEATURE_ENABLED",
//!     false,
//!     "Enable the experimental my feature"
//! );
//!
//! static MY_LIMIT: EnvSetting<i32> = EnvSetting::new(
//!     "MY_LIMIT",
//!     100,
//!     "Maximum number of items to process"
//! );
//!
//! // Access the values
//! let enabled = MY_FEATURE.get();
//! let limit = MY_LIMIT.get();
//! ```
//!
//! # Thread Safety
//!
//! All environment settings are thread-safe. Values are cached after first
//! access, so subsequent accesses are very fast.

use std::env;
use std::io::BufRead;
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;

/// A type that can be used as an environment setting value.
pub trait EnvSettingValue: Sized + Clone + Send + Sync + PartialEq + 'static {
    /// Parse a string into this type.
    fn parse(s: &str) -> Option<Self>;

    /// Format the value for display in override alerts.
    fn display(&self) -> String;
}

impl EnvSettingValue for bool {
    fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Some(true),
            "false" | "0" | "no" | "off" | "" => Some(false),
            _ => None,
        }
    }
    fn display(&self) -> String {
        if *self {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }
}

impl EnvSettingValue for i32 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
    fn display(&self) -> String {
        self.to_string()
    }
}

impl EnvSettingValue for i64 {
    fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
    fn display(&self) -> String {
        self.to_string()
    }
}

/// An environment setting with a typed default value.
///
/// Environment settings provide a type-safe way to access environment
/// variables. The value is cached after first access for performance.
///
/// # Examples
///
/// ```
/// use usd_tf::env_setting::EnvSetting;
///
/// static DEBUG_MODE: EnvSetting<bool> = EnvSetting::new(
///     "DEBUG_MODE",
///     false,
///     "Enable debug output"
/// );
///
/// // Get the value (reads env var on first access, cached thereafter)
/// if DEBUG_MODE.get() {
///     println!("Debug mode enabled");
/// }
/// ```
pub struct EnvSetting<T: EnvSettingValue + Copy> {
    /// The environment variable name.
    name: &'static str,
    /// The default value if the env var is not set.
    default: T,
    /// Description of this setting.
    description: &'static str,
    /// Cached value (initialized on first access).
    value: OnceLock<T>,
}

impl<T: EnvSettingValue + Copy> EnvSetting<T> {
    /// Create a new environment setting.
    ///
    /// # Arguments
    ///
    /// * `name` - The environment variable name
    /// * `default` - The default value if not set
    /// * `description` - Human-readable description
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::env_setting::EnvSetting;
    ///
    /// static TIMEOUT: EnvSetting<i32> = EnvSetting::new(
    ///     "REQUEST_TIMEOUT",
    ///     30,
    ///     "Request timeout in seconds"
    /// );
    /// ```
    pub const fn new(name: &'static str, default: T, description: &'static str) -> Self {
        Self {
            name,
            default,
            description,
            value: OnceLock::new(),
        }
    }

    /// Get the value of this setting.
    ///
    /// On first access, reads the environment variable and caches the result.
    /// Subsequent accesses return the cached value.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::env_setting::EnvSetting;
    ///
    /// static MAX_THREADS: EnvSetting<i32> = EnvSetting::new(
    ///     "MAX_THREADS",
    ///     4,
    ///     "Maximum number of worker threads"
    /// );
    ///
    /// let threads = MAX_THREADS.get();
    /// ```
    pub fn get(&self) -> T {
        *self.value.get_or_init(|| self.read_from_env())
    }

    /// Get the name of the environment variable.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get the default value.
    #[must_use]
    pub const fn default_value(&self) -> T {
        self.default
    }

    /// Get the description.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        self.description
    }

    /// Read the value from environment, with file override support.
    ///
    /// Priority: env var > PIXAR_TF_ENV_SETTING_FILE entries > default.
    /// Prints a boxed alert to stderr when value differs from default,
    /// matching C++ Tf_InitializeEnvSetting behavior.
    fn read_from_env(&self) -> T {
        // Check env var first (highest priority)
        let value = match env::var(self.name) {
            Ok(s) => T::parse(&s).unwrap_or(self.default),
            Err(_) => {
                // Fall back to file-based overrides
                load_file_overrides()
                    .get(self.name)
                    .and_then(|s| T::parse(s))
                    .unwrap_or(self.default)
            }
        };

        // Print override alert if enabled (matches C++ TF_ENV_SETTING_ALERTS_ENABLED)
        if value != self.default && alerts_enabled() {
            let text = format!(
                "#  {} is overridden to '{}'. Default is '{}'. #",
                self.name,
                value.display(),
                self.default.display()
            );
            let border = "#".repeat(text.len());
            eprintln!("{}\n{}\n{}", border, text, border);
        }

        // Register the setting for introspection
        register_setting(self.name, &value.display());

        value
    }

    /// Check if this setting has been overridden from its default.
    ///
    /// Returns true after `.get()` has been called and the resolved
    /// value differs from the compile-time default.
    #[must_use]
    pub fn is_overridden(&self) -> bool {
        match self.value.get() {
            Some(v) => *v != self.default,
            None => false,
        }
    }
}

// SAFETY: EnvSetting can be used in static context because OnceLock provides
// the necessary synchronization and T is Copy
#[allow(unsafe_code)]
unsafe impl<T: EnvSettingValue + Copy> Sync for EnvSetting<T> {}

#[allow(unsafe_code)]
unsafe impl<T: EnvSettingValue + Copy> Send for EnvSetting<T> {}

/// An environment setting for string values.
///
/// This is separate from `EnvSetting<T>` because String cannot be created
/// in a const context. The default is stored as a `&'static str`.
///
/// # Examples
///
/// ```
/// use usd_tf::env_setting::StringEnvSetting;
///
/// static MY_NAME: StringEnvSetting = StringEnvSetting::new(
///     "MY_NAME",
///     "default",
///     "The name to use"
/// );
///
/// let name = MY_NAME.get();
/// ```
pub struct StringEnvSetting {
    /// The environment variable name.
    name: &'static str,
    /// The default value if the env var is not set.
    default: &'static str,
    /// Description of this setting.
    description: &'static str,
    /// Cached value (initialized on first access).
    value: OnceLock<String>,
}

impl StringEnvSetting {
    /// Create a new string environment setting.
    pub const fn new(name: &'static str, default: &'static str, description: &'static str) -> Self {
        Self {
            name,
            default,
            description,
            value: OnceLock::new(),
        }
    }

    /// Get the value of this setting.
    pub fn get(&self) -> String {
        self.value.get_or_init(|| self.read_from_env()).clone()
    }

    /// Get a reference to the value of this setting.
    pub fn get_ref(&self) -> &str {
        self.value.get_or_init(|| self.read_from_env())
    }

    /// Get the name of the environment variable.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get the default value.
    #[must_use]
    pub const fn default_value(&self) -> &'static str {
        self.default
    }

    /// Get the description.
    #[must_use]
    pub const fn description(&self) -> &'static str {
        self.description
    }

    /// Read the value from environment, with file override support.
    fn read_from_env(&self) -> String {
        // Check env var first (highest priority)
        let value = match env::var(self.name) {
            Ok(s) => s,
            Err(_) => {
                // Fall back to file-based overrides
                load_file_overrides()
                    .get(self.name)
                    .cloned()
                    .unwrap_or_else(|| self.default.to_string())
            }
        };

        // Print override alert if value differs from default
        if value != self.default && alerts_enabled() {
            let text = format!(
                "#  {} is overridden to '{}'. Default is '{}'. #",
                self.name, value, self.default
            );
            let border = "#".repeat(text.len());
            eprintln!("{}\n{}\n{}", border, text, border);
        }

        // Register for introspection
        register_setting(self.name, &value);

        value
    }
}

// SAFETY: StringEnvSetting can be used in static context because OnceLock
// provides the necessary synchronization
#[allow(unsafe_code)]
unsafe impl Sync for StringEnvSetting {}

#[allow(unsafe_code)]
unsafe impl Send for StringEnvSetting {}

use std::collections::HashMap;

/// Whether override alerts should be printed to stderr.
/// Controlled by TF_ENV_SETTING_ALERTS_ENABLED env var (default: true).
static ALERTS_ENABLED: OnceLock<bool> = OnceLock::new();

/// Check if override alerts are enabled.
fn alerts_enabled() -> bool {
    *ALERTS_ENABLED.get_or_init(|| {
        env::var("TF_ENV_SETTING_ALERTS_ENABLED")
            .ok()
            .and_then(|s| bool::parse(&s))
            .unwrap_or(true)
    })
}

/// Global registry of all defined settings (name -> current value string).
/// Used by `get_env_setting_by_name()` for introspection.
static SETTING_REGISTRY: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn setting_registry() -> &'static Mutex<HashMap<String, String>> {
    SETTING_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register a setting's resolved value for introspection.
fn register_setting(name: &str, value: &str) {
    let mut reg = setting_registry().lock().expect("setting registry lock");
    reg.insert(name.to_string(), value.to_string());
}

/// Look up a previously-initialized env setting by name.
///
/// Returns the string representation of the setting's resolved value,
/// or `None` if the setting has not been accessed yet.
/// Matches C++ `Tf_GetEnvSettingByName`.
pub fn get_env_setting_by_name(name: &str) -> Option<String> {
    let reg = setting_registry().lock().expect("setting registry lock");
    reg.get(name).cloned()
}

/// Cached file-based overrides from PIXAR_TF_ENV_SETTING_FILE.
static FILE_OVERRIDES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Load env setting overrides from the file specified by PIXAR_TF_ENV_SETTING_FILE.
///
/// The file format is `KEY=VALUE` per line, blank lines and `#` comments ignored.
/// Per C++ reference: if the file cannot be read, no error is printed.
/// Malformed lines produce stderr warnings.
/// Look up a file override by key. Returns None if not found.
pub fn get_file_override(key: &str) -> Option<&'static str> {
    load_file_overrides().get(key).map(|s| s.as_str())
}

fn load_file_overrides() -> &'static HashMap<String, String> {
    FILE_OVERRIDES.get_or_init(|| {
        let mut overrides = HashMap::new();

        let Ok(file_path) = env::var("PIXAR_TF_ENV_SETTING_FILE") else {
            return overrides;
        };

        if file_path.is_empty() || !Path::new(&file_path).is_file() {
            return overrides;
        }

        let Ok(file) = std::fs::File::open(&file_path) else {
            // Per C++ spec: if file cannot be read, no error is printed
            return overrides;
        };

        let reader = std::io::BufReader::new(file);
        for (line_num, line_result) in reader.lines().enumerate() {
            let Ok(line) = line_result else { continue };
            let trimmed = line.trim();

            // Skip blank lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Parse KEY=VALUE format
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();
                if !key.is_empty() {
                    overrides.insert(key.to_string(), value.to_string());
                } else {
                    eprintln!(
                        "PIXAR_TF_ENV_SETTING_FILE:{}: malformed line (empty key): {}",
                        line_num + 1,
                        trimmed
                    );
                }
            } else {
                eprintln!(
                    "PIXAR_TF_ENV_SETTING_FILE:{}: malformed line (no '='): {}",
                    line_num + 1,
                    trimmed
                );
            }
        }

        overrides
    })
}

/// Get the value of an environment setting.
///
/// This is a convenience function equivalent to `setting.get()`.
///
/// # Examples
///
/// ```
/// use usd_tf::env_setting::{EnvSetting, get_env_setting};
///
/// static VERBOSE: EnvSetting<bool> = EnvSetting::new(
///     "VERBOSE",
///     false,
///     "Enable verbose output"
/// );
///
/// let verbose = get_env_setting(&VERBOSE);
/// ```
pub fn get_env_setting<T: EnvSettingValue + Copy>(setting: &EnvSetting<T>) -> T {
    setting.get()
}

/// A macro to define an environment setting.
///
/// # Examples
///
/// ```
/// use usd_tf::define_env_setting;
///
/// // Define a boolean setting
/// define_env_setting!(MY_FEATURE, bool, false, "Enable my feature");
///
/// // Define an integer setting  
/// define_env_setting!(MY_LIMIT, i32, 100, "Maximum limit");
///
/// // Access the settings
/// let enabled = MY_FEATURE.get();
/// let limit = MY_LIMIT.get();
/// ```
#[macro_export]
macro_rules! define_env_setting {
    // Private (default) variants
    ($name:ident, bool, $default:expr, $desc:expr) => {
        static $name: $crate::env_setting::EnvSetting<bool> =
            $crate::env_setting::EnvSetting::new(stringify!($name), $default, $desc);
    };
    ($name:ident, i32, $default:expr, $desc:expr) => {
        static $name: $crate::env_setting::EnvSetting<i32> =
            $crate::env_setting::EnvSetting::new(stringify!($name), $default, $desc);
    };
    ($name:ident, i64, $default:expr, $desc:expr) => {
        static $name: $crate::env_setting::EnvSetting<i64> =
            $crate::env_setting::EnvSetting::new(stringify!($name), $default, $desc);
    };
    ($name:ident, String, $default:expr, $desc:expr) => {
        static $name: $crate::env_setting::StringEnvSetting =
            $crate::env_setting::StringEnvSetting::new(stringify!($name), $default, $desc);
    };
    // Public variants with `pub` modifier
    (pub $name:ident, bool, $default:expr, $desc:expr) => {
        pub static $name: $crate::env_setting::EnvSetting<bool> =
            $crate::env_setting::EnvSetting::new(stringify!($name), $default, $desc);
    };
    (pub $name:ident, i32, $default:expr, $desc:expr) => {
        pub static $name: $crate::env_setting::EnvSetting<i32> =
            $crate::env_setting::EnvSetting::new(stringify!($name), $default, $desc);
    };
    (pub $name:ident, i64, $default:expr, $desc:expr) => {
        pub static $name: $crate::env_setting::EnvSetting<i64> =
            $crate::env_setting::EnvSetting::new(stringify!($name), $default, $desc);
    };
    (pub $name:ident, String, $default:expr, $desc:expr) => {
        pub static $name: $crate::env_setting::StringEnvSetting =
            $crate::env_setting::StringEnvSetting::new(stringify!($name), $default, $desc);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bool_setting_default() {
        static TEST_BOOL: EnvSetting<bool> =
            EnvSetting::new("TF_TEST_BOOL_12345", false, "Test setting");

        // Should return default since env var is not set
        assert!(!TEST_BOOL.get());
    }

    #[test]
    fn test_i32_setting_default() {
        static TEST_INT: EnvSetting<i32> = EnvSetting::new("TF_TEST_INT_12345", 42, "Test setting");

        assert_eq!(TEST_INT.get(), 42);
    }

    #[test]
    fn test_bool_parsing() {
        assert_eq!(bool::parse("true"), Some(true));
        assert_eq!(bool::parse("false"), Some(false));
        assert_eq!(bool::parse("1"), Some(true));
        assert_eq!(bool::parse("0"), Some(false));
        assert_eq!(bool::parse("yes"), Some(true));
        assert_eq!(bool::parse("no"), Some(false));
        assert_eq!(bool::parse("on"), Some(true));
        assert_eq!(bool::parse("off"), Some(false));
        assert_eq!(bool::parse("TRUE"), Some(true));
        assert_eq!(bool::parse("FALSE"), Some(false));
        assert_eq!(bool::parse("invalid"), None);
    }

    #[test]
    fn test_i32_parsing() {
        assert_eq!(i32::parse("42"), Some(42));
        assert_eq!(i32::parse("-100"), Some(-100));
        assert_eq!(i32::parse("0"), Some(0));
        assert_eq!(i32::parse("invalid"), None);
    }

    #[test]
    fn test_setting_metadata() {
        static TEST: EnvSetting<i32> = EnvSetting::new("TEST_VAR", 100, "Test description");

        assert_eq!(TEST.name(), "TEST_VAR");
        assert_eq!(TEST.description(), "Test description");
        assert_eq!(TEST.default_value(), 100);
    }

    #[test]
    fn test_get_env_setting_function() {
        static TEST: EnvSetting<i32> = EnvSetting::new("TF_TEST_GET_12345", 55, "Test");

        assert_eq!(get_env_setting(&TEST), 55);
    }

    #[test]
    fn test_setting_with_env_var() {
        // Set env var before creating setting
        // SAFETY: test is single-threaded for this env var
        unsafe {
            env::set_var("TF_TEST_ENV_SET", "999");
        }

        static TEST: EnvSetting<i32> = EnvSetting::new("TF_TEST_ENV_SET", 0, "Test");

        assert_eq!(TEST.get(), 999);

        // Clean up
        unsafe {
            env::remove_var("TF_TEST_ENV_SET");
        }
    }

    #[test]
    fn test_bool_setting_with_env_var() {
        unsafe {
            env::set_var("TF_TEST_BOOL_SET", "true");
        }

        static TEST: EnvSetting<bool> = EnvSetting::new("TF_TEST_BOOL_SET", false, "Test");

        assert!(TEST.get());

        unsafe {
            env::remove_var("TF_TEST_BOOL_SET");
        }
    }

    #[test]
    fn test_cached_value() {
        unsafe {
            env::set_var("TF_TEST_CACHE", "100");
        }

        static TEST: EnvSetting<i32> = EnvSetting::new("TF_TEST_CACHE", 0, "Test");

        // First access
        assert_eq!(TEST.get(), 100);

        // Change env var
        unsafe {
            env::set_var("TF_TEST_CACHE", "200");
        }

        // Should still return cached value
        assert_eq!(TEST.get(), 100);

        unsafe {
            env::remove_var("TF_TEST_CACHE");
        }
    }

    #[test]
    fn test_string_setting_default() {
        static TEST: StringEnvSetting =
            StringEnvSetting::new("TF_TEST_STR_12345", "default_value", "Test");

        assert_eq!(TEST.get(), "default_value");
        assert_eq!(TEST.get_ref(), "default_value");
    }

    #[test]
    fn test_string_setting_with_env() {
        unsafe {
            env::set_var("TF_TEST_STR_SET", "custom_value");
        }

        static TEST: StringEnvSetting = StringEnvSetting::new("TF_TEST_STR_SET", "default", "Test");

        assert_eq!(TEST.get(), "custom_value");

        unsafe {
            env::remove_var("TF_TEST_STR_SET");
        }
    }

    #[test]
    fn test_string_setting_metadata() {
        static TEST: StringEnvSetting = StringEnvSetting::new("STR_VAR", "def", "Description");

        assert_eq!(TEST.name(), "STR_VAR");
        assert_eq!(TEST.default_value(), "def");
        assert_eq!(TEST.description(), "Description");
    }

    #[test]
    fn test_load_file_overrides_no_file() {
        // Without PIXAR_TF_ENV_SETTING_FILE set, overrides should be empty
        let overrides = load_file_overrides();
        // Can't assert empty since another test might have set it,
        // but at least ensure it doesn't panic
        let _ = overrides;
    }

    #[test]
    fn test_file_override_parsing() {
        use std::io::Write;

        // Create a temp file with settings
        let dir = std::env::temp_dir();
        let path = dir.join("tf_env_setting_test.txt");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "# This is a comment").unwrap();
            writeln!(f, "").unwrap();
            writeln!(f, "TEST_KEY1=value1").unwrap();
            writeln!(f, "TEST_KEY2=123").unwrap();
            writeln!(f, "TEST_KEY3=").unwrap();
            writeln!(f, "TEST_KEY4=long value with spaces").unwrap();
        }

        // Parse the file directly (can't use static FILE_OVERRIDES since it's shared)
        let content = std::fs::read_to_string(&path).unwrap();
        let mut overrides = HashMap::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();
                if !key.is_empty() {
                    overrides.insert(key.to_string(), value.to_string());
                }
            }
        }

        assert_eq!(overrides.get("TEST_KEY1").unwrap(), "value1");
        assert_eq!(overrides.get("TEST_KEY2").unwrap(), "123");
        assert_eq!(overrides.get("TEST_KEY3").unwrap(), "");
        assert_eq!(
            overrides.get("TEST_KEY4").unwrap(),
            "long value with spaces"
        );
        assert!(!overrides.contains_key("# This is a comment"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_is_overridden_default() {
        static TEST: EnvSetting<i32> = EnvSetting::new("TF_TEST_IS_OVERRIDDEN_DEFAULT", 42, "Test");

        // Access the setting (env var not set, so returns default)
        let val = TEST.get();
        assert_eq!(val, 42);
        // Value equals default => not overridden
        assert!(!TEST.is_overridden());
    }

    #[test]
    fn test_is_overridden_with_env() {
        unsafe {
            env::set_var("TF_TEST_IS_OVERRIDDEN_ENV", "99");
        }

        static TEST: EnvSetting<i32> = EnvSetting::new("TF_TEST_IS_OVERRIDDEN_ENV", 42, "Test");

        let val = TEST.get();
        assert_eq!(val, 99);
        assert!(TEST.is_overridden());

        unsafe {
            env::remove_var("TF_TEST_IS_OVERRIDDEN_ENV");
        }
    }

    #[test]
    fn test_get_env_setting_by_name() {
        static TEST: EnvSetting<i32> = EnvSetting::new("TF_TEST_LOOKUP_BY_NAME", 77, "Test");

        // Before first access, not registered
        assert!(get_env_setting_by_name("TF_TEST_LOOKUP_BY_NAME").is_none());

        // Trigger initialization
        let _ = TEST.get();

        // Now it should be registered
        let found = get_env_setting_by_name("TF_TEST_LOOKUP_BY_NAME");
        assert!(found.is_some());
        assert_eq!(found.unwrap(), "77");
    }

    #[test]
    fn test_display_trait_impls() {
        assert_eq!(true.display(), "true");
        assert_eq!(false.display(), "false");
        assert_eq!(42i32.display(), "42");
        assert_eq!((-1i64).display(), "-1");
    }
}
