//! Script module loading system with dependency tracking.
//!
//! This module provides a system for registering modules (libraries, plugins, etc.)
//! with their dependencies and loading them in the correct order.
//!
//! # Overview
//!
//! In OpenUSD's C++, this is used to manage Python bindings loading order.
//! In Rust, this provides a general-purpose dependency-aware module loading system
//! that can be used for any kind of dynamically loaded modules or plugins.
//!
//! # Examples
//!
//! ```
//! use usd_tf::script_module_loader::ScriptModuleLoader;
//! use std::sync::atomic::{AtomicBool, Ordering};
//!
//! static LOADED: AtomicBool = AtomicBool::new(false);
//!
//! // Register a loader function for a module
//! ScriptModuleLoader::register_library(
//!     "my_lib",
//!     "my_module",
//!     &[], // no dependencies
//!     |_name| {
//!         LOADED.store(true, Ordering::SeqCst);
//!         Ok(())
//!     },
//! );
//!
//! // Load all registered modules
//! ScriptModuleLoader::load_modules();
//!
//! assert!(LOADED.load(Ordering::SeqCst));
//! ```

use crate::Token;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

/// Type for module loading functions.
///
/// The function receives the module name and returns Ok(()) on success
/// or an error message on failure.
pub type LoaderFn = fn(&str) -> Result<(), String>;

/// Information about a registered library/module.
struct LibInfo {
    /// The module name (e.g., for import).
    module_name: Token,
    /// Libraries that must be loaded first.
    predecessors: Vec<Token>,
    /// Whether this module has been loaded.
    is_loaded: AtomicBool,
    /// The loader function.
    loader: Option<LoaderFn>,
}

impl LibInfo {
    fn new(module_name: Token, predecessors: Vec<Token>, loader: Option<LoaderFn>) -> Self {
        Self {
            module_name,
            predecessors,
            is_loaded: AtomicBool::new(false),
            loader,
        }
    }
}

/// Internal loader data.
#[derive(Default)]
struct LoaderData {
    /// Library info map.
    libs: HashMap<Token, LibInfo>,
}

/// Global loader instance.
static LOADER_DATA: OnceLock<RwLock<LoaderData>> = OnceLock::new();

/// Module loading callback for external handlers.
static MODULE_CALLBACK: OnceLock<Mutex<Option<LoaderFn>>> = OnceLock::new();

fn get_loader_data() -> &'static RwLock<LoaderData> {
    LOADER_DATA.get_or_init(|| RwLock::new(LoaderData::default()))
}

fn get_callback() -> &'static Mutex<Option<LoaderFn>> {
    MODULE_CALLBACK.get_or_init(|| Mutex::new(None))
}

/// Script module loader for dependency-aware module loading.
///
/// This singleton manages registration and loading of modules with dependencies.
/// Modules are loaded in dependency order - prerequisites are always loaded
/// before dependents.
///
/// # Thread Safety
///
/// All operations are thread-safe. The loader uses RwLock for the main data
/// and atomic flags for individual module load states.
///
/// # Examples
///
/// ```
/// use usd_tf::script_module_loader::ScriptModuleLoader;
///
/// // Register libraries with dependencies
/// ScriptModuleLoader::register_library("base", "base_module", &[], |_| Ok(()));
/// ScriptModuleLoader::register_library("derived", "derived_module", &["base"], |_| Ok(()));
///
/// // Loading "derived" will first load "base"
/// ScriptModuleLoader::load_modules_for_library("derived");
/// ```
pub struct ScriptModuleLoader;

impl ScriptModuleLoader {
    /// Registers a library with its module name, dependencies, and loader function.
    ///
    /// # Arguments
    ///
    /// * `lib_name` - The library name (used as key)
    /// * `module_name` - The module name (e.g., Python module name)
    /// * `predecessors` - Libraries that must be loaded first
    /// * `loader` - Function to call when loading this module
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// ScriptModuleLoader::register_library(
    ///     "my_lib",
    ///     "my_module",
    ///     &["dependency_a", "dependency_b"],
    ///     |module_name| {
    ///         println!("Loading module: {}", module_name);
    ///         Ok(())
    ///     },
    /// );
    /// ```
    pub fn register_library(
        lib_name: &str,
        module_name: &str,
        predecessors: &[&str],
        loader: LoaderFn,
    ) {
        let lib_token = Token::new(lib_name);
        let module_token = Token::new(module_name);
        let pred_tokens: Vec<Token> = predecessors.iter().map(|s| Token::new(s)).collect();

        let Ok(mut guard) = get_loader_data().write() else {
            return;
        };

        if guard.libs.contains_key(&lib_token) {
            eprintln!(
                "Library {} (with module '{}') already registered, repeated registration ignored",
                lib_name, module_name
            );
            return;
        }

        guard.libs.insert(
            lib_token,
            LibInfo::new(module_token, pred_tokens, Some(loader)),
        );
    }

    /// Registers a library without a loader function.
    ///
    /// This is useful when the library will be loaded by an external system
    /// (e.g., Python's import system) and we just need to track dependencies.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// // Register for dependency tracking only
    /// ScriptModuleLoader::register_library_info("external_lib", "ext_mod", &["base"]);
    /// ```
    pub fn register_library_info(lib_name: &str, module_name: &str, predecessors: &[&str]) {
        let lib_token = Token::new(lib_name);
        let module_token = Token::new(module_name);
        let pred_tokens: Vec<Token> = predecessors.iter().map(|s| Token::new(s)).collect();

        let Ok(mut guard) = get_loader_data().write() else {
            return;
        };

        if guard.libs.contains_key(&lib_token) {
            return;
        }

        guard
            .libs
            .insert(lib_token, LibInfo::new(module_token, pred_tokens, None));
    }

    /// Sets a global callback for module loading.
    ///
    /// This callback is called for any module that doesn't have its own loader.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// ScriptModuleLoader::set_load_callback(|module_name| {
    ///     println!("External load: {}", module_name);
    ///     Ok(())
    /// });
    /// ```
    pub fn set_load_callback(callback: LoaderFn) {
        if let Ok(mut guard) = get_callback().lock() {
            *guard = Some(callback);
        }
    }

    /// Clears the global load callback.
    pub fn clear_load_callback() {
        if let Ok(mut guard) = get_callback().lock() {
            *guard = None;
        }
    }

    /// Loads all registered modules that haven't been loaded yet.
    ///
    /// Modules are loaded in dependency order.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// // Load all pending modules
    /// ScriptModuleLoader::load_modules();
    /// ```
    pub fn load_modules() {
        let libs_to_load = {
            let Ok(guard) = get_loader_data().read() else {
                return;
            };

            // Collect all unloaded libraries
            let mut to_load: Vec<Token> = guard
                .libs
                .iter()
                .filter(|(_, info)| !info.is_loaded.load(Ordering::Acquire))
                .map(|(name, _)| name.clone())
                .collect();

            // Sort for consistent load order
            to_load.sort();
            to_load
        };

        // Load in dependency order
        let mut loaded = HashSet::new();
        for lib in &libs_to_load {
            Self::load_with_deps(lib, &mut loaded);
        }
    }

    /// Loads modules for a specific library and its dependencies.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// // Load a specific library and its dependencies
    /// ScriptModuleLoader::load_modules_for_library("my_lib");
    /// ```
    pub fn load_modules_for_library(lib_name: &str) {
        if lib_name.is_empty() {
            Self::load_modules();
            return;
        }

        let lib_token = Token::new(lib_name);
        let mut loaded = HashSet::new();
        Self::load_with_deps(&lib_token, &mut loaded);
    }

    /// Recursively loads a library and its dependencies.
    fn load_with_deps(lib: &Token, loaded: &mut HashSet<Token>) {
        // Avoid cycles
        if loaded.contains(lib) {
            return;
        }
        loaded.insert(lib.clone());

        // Get info and dependencies
        let (predecessors, module_name, loader) = {
            let Ok(guard) = get_loader_data().read() else {
                return;
            };

            let Some(info) = guard.libs.get(lib) else {
                return;
            };

            // Already loaded?
            if info.is_loaded.load(Ordering::Acquire) {
                return;
            }

            (
                info.predecessors.clone(),
                info.module_name.clone(),
                info.loader,
            )
        };

        // Load dependencies first
        for pred in &predecessors {
            Self::load_with_deps(pred, loaded);
        }

        // Now load this module
        Self::do_load(lib, &module_name, loader);
    }

    /// Actually loads a module.
    fn do_load(lib: &Token, module_name: &Token, loader: Option<LoaderFn>) {
        let module_str = module_name.as_str();
        if module_str.is_empty() {
            return;
        }

        // Try library-specific loader first
        if let Some(load_fn) = loader {
            if let Err(e) = load_fn(module_str) {
                eprintln!(
                    "Error loading lib {}'s module '{}': {}",
                    lib.as_str(),
                    module_str,
                    e
                );
            }
        } else {
            // Try global callback
            if let Ok(guard) = get_callback().lock() {
                if let Some(callback) = *guard {
                    if let Err(e) = callback(module_str) {
                        eprintln!(
                            "Error loading lib {}'s module '{}': {}",
                            lib.as_str(),
                            module_str,
                            e
                        );
                    }
                }
            }
        }

        // Mark as loaded (even on error to prevent repeated attempts)
        if let Ok(guard) = get_loader_data().read() {
            if let Some(info) = guard.libs.get(lib) {
                info.is_loaded.store(true, Ordering::Release);
            }
        }
    }

    /// Checks if a library is registered.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// ScriptModuleLoader::register_library_info("test_lib", "test_mod", &[]);
    /// assert!(ScriptModuleLoader::is_registered("test_lib"));
    /// assert!(!ScriptModuleLoader::is_registered("nonexistent"));
    /// ```
    pub fn is_registered(lib_name: &str) -> bool {
        let lib_token = Token::new(lib_name);
        if let Ok(guard) = get_loader_data().read() {
            guard.libs.contains_key(&lib_token)
        } else {
            false
        }
    }

    /// Checks if a library's module has been loaded.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// ScriptModuleLoader::register_library("load_check", "mod", &[], |_| Ok(()));
    /// assert!(!ScriptModuleLoader::is_loaded("load_check"));
    ///
    /// ScriptModuleLoader::load_modules_for_library("load_check");
    /// assert!(ScriptModuleLoader::is_loaded("load_check"));
    /// ```
    pub fn is_loaded(lib_name: &str) -> bool {
        let lib_token = Token::new(lib_name);
        if let Ok(guard) = get_loader_data().read() {
            guard
                .libs
                .get(&lib_token)
                .map(|info| info.is_loaded.load(Ordering::Acquire))
                .unwrap_or(false)
        } else {
            false
        }
    }

    /// Returns a list of all registered library names.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// let libs = ScriptModuleLoader::library_names();
    /// // Returns sorted list of library names
    /// ```
    pub fn library_names() -> Vec<String> {
        if let Ok(guard) = get_loader_data().read() {
            let mut names: Vec<String> =
                guard.libs.keys().map(|t| t.as_str().to_string()).collect();
            names.sort();
            names
        } else {
            Vec::new()
        }
    }

    /// Gets the module name for a library.
    pub fn module_name(lib_name: &str) -> Option<String> {
        let lib_token = Token::new(lib_name);
        if let Ok(guard) = get_loader_data().read() {
            guard
                .libs
                .get(&lib_token)
                .map(|info| info.module_name.as_str().to_string())
        } else {
            None
        }
    }

    /// Gets the predecessors (dependencies) for a library.
    pub fn predecessors(lib_name: &str) -> Vec<String> {
        let lib_token = Token::new(lib_name);
        if let Ok(guard) = get_loader_data().read() {
            guard
                .libs
                .get(&lib_token)
                .map(|info| {
                    info.predecessors
                        .iter()
                        .map(|t| t.as_str().to_string())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Writes a Graphviz DOT file of the dependency graph.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use usd_tf::script_module_loader::ScriptModuleLoader;
    ///
    /// ScriptModuleLoader::write_dot_file("deps.dot")?;
    /// // Run: dot -Tpng deps.dot -o deps.png
    /// ```
    pub fn write_dot_file(path: &str) -> std::io::Result<()> {
        use std::fs::File;
        use std::io::Write;

        let mut file = File::create(path)?;
        writeln!(file, "digraph Modules {{")?;

        if let Ok(guard) = get_loader_data().read() {
            for (lib, info) in &guard.libs {
                for pred in &info.predecessors {
                    writeln!(file, "\t{} -> {};", lib.as_str(), pred.as_str())?;
                }
            }
        }

        writeln!(file, "}}")?;
        Ok(())
    }

    /// Returns the number of registered libraries.
    pub fn count() -> usize {
        if let Ok(guard) = get_loader_data().read() {
            guard.libs.len()
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    // Each test uses unique names to avoid parallel test interference

    #[test]
    fn test_register_and_is_registered() {
        ScriptModuleLoader::register_library_info("sml_test_reg_1", "mod1", &[]);
        assert!(ScriptModuleLoader::is_registered("sml_test_reg_1"));
        assert!(!ScriptModuleLoader::is_registered("sml_nonexistent_1"));
    }

    #[test]
    fn test_register_with_loader() {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);

        ScriptModuleLoader::register_library("sml_test_loader_2", "mod2", &[], |_| {
            COUNTER.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });

        let before = COUNTER.load(Ordering::SeqCst);
        ScriptModuleLoader::load_modules_for_library("sml_test_loader_2");
        assert!(COUNTER.load(Ordering::SeqCst) > before);
    }

    #[test]
    fn test_is_loaded() {
        ScriptModuleLoader::register_library("sml_test_loaded_3", "mod3", &[], |_| Ok(()));

        assert!(!ScriptModuleLoader::is_loaded("sml_test_loaded_3"));
        ScriptModuleLoader::load_modules_for_library("sml_test_loaded_3");
        assert!(ScriptModuleLoader::is_loaded("sml_test_loaded_3"));
    }

    #[test]
    fn test_dependencies() {
        static ORDER: Mutex<Vec<String>> = Mutex::new(Vec::new());

        ScriptModuleLoader::register_library("sml_base_4", "base_mod", &[], |name| {
            if let Ok(mut guard) = ORDER.lock() {
                guard.push(name.to_string());
            }
            Ok(())
        });

        ScriptModuleLoader::register_library(
            "sml_derived_4",
            "derived_mod",
            &["sml_base_4"],
            |name| {
                if let Ok(mut guard) = ORDER.lock() {
                    guard.push(name.to_string());
                }
                Ok(())
            },
        );

        ScriptModuleLoader::load_modules_for_library("sml_derived_4");

        if let Ok(guard) = ORDER.lock() {
            // Base should be loaded before derived
            let base_pos = guard.iter().position(|s| s == "base_mod");
            let derived_pos = guard.iter().position(|s| s == "derived_mod");
            if let (Some(b), Some(d)) = (base_pos, derived_pos) {
                assert!(b < d, "base should load before derived");
            }
        }
    }

    #[test]
    fn test_library_names() {
        ScriptModuleLoader::register_library_info("sml_names_a_5", "mod_a", &[]);
        ScriptModuleLoader::register_library_info("sml_names_b_5", "mod_b", &[]);

        let names = ScriptModuleLoader::library_names();
        assert!(names.contains(&"sml_names_a_5".to_string()));
        assert!(names.contains(&"sml_names_b_5".to_string()));
    }

    #[test]
    fn test_module_name() {
        ScriptModuleLoader::register_library_info("sml_modname_6", "the_module", &[]);

        let name = ScriptModuleLoader::module_name("sml_modname_6");
        assert_eq!(name, Some("the_module".to_string()));

        let none = ScriptModuleLoader::module_name("nonexistent_6");
        assert_eq!(none, None);
    }

    #[test]
    fn test_predecessors() {
        ScriptModuleLoader::register_library_info("sml_preds_7", "mod7", &["dep1", "dep2"]);

        let preds = ScriptModuleLoader::predecessors("sml_preds_7");
        assert!(preds.contains(&"dep1".to_string()));
        assert!(preds.contains(&"dep2".to_string()));
    }

    #[test]
    fn test_global_callback() {
        static CALLBACK_CALLED: AtomicBool = AtomicBool::new(false);

        ScriptModuleLoader::set_load_callback(|_| {
            CALLBACK_CALLED.store(true, Ordering::SeqCst);
            Ok(())
        });

        // Register without a loader
        ScriptModuleLoader::register_library_info("sml_callback_8", "callback_mod", &[]);

        // Loading should use global callback
        ScriptModuleLoader::load_modules_for_library("sml_callback_8");
        assert!(CALLBACK_CALLED.load(Ordering::SeqCst));

        ScriptModuleLoader::clear_load_callback();
    }

    #[test]
    fn test_loader_error() {
        ScriptModuleLoader::register_library("sml_error_9", "error_mod", &[], |_| {
            Err("test error".to_string())
        });

        // Should not panic, just print error
        ScriptModuleLoader::load_modules_for_library("sml_error_9");

        // Should still be marked as loaded (to prevent repeated attempts)
        assert!(ScriptModuleLoader::is_loaded("sml_error_9"));
    }

    #[test]
    fn test_count() {
        let initial = ScriptModuleLoader::count();
        ScriptModuleLoader::register_library_info("sml_count_10", "mod10", &[]);
        // Count should increase (may not be exactly +1 due to parallel tests)
        assert!(ScriptModuleLoader::count() > initial);
    }
}
