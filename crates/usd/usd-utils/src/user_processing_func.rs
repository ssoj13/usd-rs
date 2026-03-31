//! User processing function types for asset localization.
//!
//! Provides [`DependencyInfo`] and [`ProcessingFunc`] for customizing
//! how dependencies are processed during asset localization.

use std::sync::Arc;
use usd_sdf::layer::Layer;

/// Information about a processed dependency.
///
/// A `DependencyInfo` object is passed into the user processing function
/// and contains relevant asset path and dependency information.
/// It is also returned from the processing function to communicate any
/// changes made during processing.
///
/// # Examples
///
/// ```ignore
/// use usd_core::usd_utils::DependencyInfo;
///
/// let info = DependencyInfo::new("textures/diffuse.png");
///
/// // With dependencies (e.g., UDIM tiles)
/// let info = DependencyInfo::with_dependencies(
///     "textures/diffuse.<UDIM>.png",
///     vec![
///         "textures/diffuse.1001.png".to_string(),
///         "textures/diffuse.1002.png".to_string(),
///     ],
/// );
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyInfo {
    /// The asset path for the dependency.
    asset_path: String,
    /// List of dependencies related to the asset path.
    dependencies: Vec<String>,
}

impl DependencyInfo {
    /// Creates a new dependency info with just an asset path.
    pub fn new(asset_path: impl Into<String>) -> Self {
        Self {
            asset_path: asset_path.into(),
            dependencies: Vec::new(),
        }
    }

    /// Creates a new dependency info with an asset path and dependencies.
    ///
    /// The dependencies array contains paths that are related to the asset
    /// path, such as UDIM tiles or clip asset path template expansions.
    pub fn with_dependencies(asset_path: impl Into<String>, dependencies: Vec<String>) -> Self {
        Self {
            asset_path: asset_path.into(),
            dependencies,
        }
    }

    /// Returns the asset path for the dependency.
    ///
    /// When returned from a processing function:
    /// - If the same as input: no special action is taken
    /// - If empty string: the path and dependencies will be ignored
    /// - If different: the system will operate on the updated value
    pub fn get_asset_path(&self) -> &str {
        &self.asset_path
    }

    /// Sets the asset path.
    ///
    /// Use this to modify the asset path during processing.
    pub fn set_asset_path(&mut self, path: impl Into<String>) {
        self.asset_path = path.into();
    }

    /// Returns the list of dependencies related to the asset path.
    ///
    /// Paths are specified relative to their containing layer.
    /// When passed into the processing function, if this array is populated,
    /// then the asset path resolved to one or more values (e.g., UDIM tiles).
    ///
    /// When returned from the processing function, each path will be
    /// processed by the system.
    pub fn get_dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Returns a mutable reference to the dependencies list.
    pub fn get_dependencies_mut(&mut self) -> &mut Vec<String> {
        &mut self.dependencies
    }

    /// Sets the dependencies list.
    pub fn set_dependencies(&mut self, dependencies: Vec<String>) {
        self.dependencies = dependencies;
    }

    /// Adds a dependency to the list.
    pub fn add_dependency(&mut self, dependency: impl Into<String>) {
        self.dependencies.push(dependency.into());
    }

    /// Clears the asset path, marking this dependency to be ignored.
    pub fn ignore(&mut self) {
        self.asset_path.clear();
    }

    /// Returns true if this dependency should be ignored.
    pub fn is_ignored(&self) -> bool {
        self.asset_path.is_empty()
    }
}

impl Default for DependencyInfo {
    fn default() -> Self {
        Self::new("")
    }
}

/// Signature for user-supplied processing function.
///
/// The processing function is invoked on every asset path that is discovered
/// during localization. It receives the layer containing the dependency and
/// the dependency info, and returns modified dependency info.
///
/// # Arguments
///
/// * `layer` - The layer containing this dependency
/// * `dependency_info` - Asset path information for this dependency
///
/// # Returns
///
/// Modified `DependencyInfo` indicating how to handle this dependency:
/// - Return unchanged: process normally
/// - Return with empty asset path: ignore this dependency
/// - Return with modified path: use the new path
/// - Return with added dependencies: process those as well
pub type ProcessingFunc = dyn Fn(&Arc<Layer>, &DependencyInfo) -> DependencyInfo + Send + Sync;

/// A boxed processing function for use in APIs.
pub type BoxedProcessingFunc = Box<ProcessingFunc>;

/// Creates a processing function that passes through all dependencies unchanged.
pub fn identity_processing_func() -> BoxedProcessingFunc {
    Box::new(|_layer, info| info.clone())
}

/// Creates a processing function that ignores all dependencies.
pub fn ignore_all_processing_func() -> BoxedProcessingFunc {
    Box::new(|_layer, info| {
        let mut result = info.clone();
        result.ignore();
        result
    })
}

/// Creates a processing function that filters dependencies by extension.
///
/// Only dependencies with the specified extensions will be kept.
pub fn filter_by_extension_processing_func(extensions: Vec<String>) -> BoxedProcessingFunc {
    Box::new(move |_layer, info| {
        let path = info.get_asset_path();

        // Check if the path ends with any of the allowed extensions
        let has_allowed_ext = extensions
            .iter()
            .any(|ext| path.to_lowercase().ends_with(&ext.to_lowercase()));

        if has_allowed_ext {
            info.clone()
        } else {
            let mut result = info.clone();
            result.ignore();
            result
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_info_new() {
        let info = DependencyInfo::new("textures/diffuse.png");
        assert_eq!(info.get_asset_path(), "textures/diffuse.png");
        assert!(info.get_dependencies().is_empty());
    }

    #[test]
    fn test_dependency_info_with_dependencies() {
        let info = DependencyInfo::with_dependencies(
            "textures/diffuse.<UDIM>.png",
            vec!["1001.png".to_string(), "1002.png".to_string()],
        );
        assert_eq!(info.get_asset_path(), "textures/diffuse.<UDIM>.png");
        assert_eq!(info.get_dependencies().len(), 2);
    }

    #[test]
    fn test_dependency_info_ignore() {
        let mut info = DependencyInfo::new("path");
        assert!(!info.is_ignored());
        info.ignore();
        assert!(info.is_ignored());
    }

    #[test]
    fn test_dependency_info_equality() {
        let a = DependencyInfo::new("path");
        let b = DependencyInfo::new("path");
        let c = DependencyInfo::new("other");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
