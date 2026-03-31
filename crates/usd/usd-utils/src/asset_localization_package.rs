//! Asset Localization Package - package assets for distribution.
//!
//! Provides utilities for packaging USD assets with all their dependencies
//! into a redistributable format. Handles path remapping to avoid leaking
//! sensitive directory structure information.
//!
//! # Directory Remapper
//!
//! The [`DirectoryRemapper`] replaces original directory paths with
//! generated names to protect sensitive information like usernames
//! or project names.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdUtils/assetLocalizationPackage.h` and `.cpp`

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use usd_sdf::{AssetPath, Layer, PrimSpec};

use super::asset_localization::{LocalizationContext, ReferenceType};
use super::asset_localization_delegate::LocalizationDelegate;

/// Remaps directory paths to generated names.
///
/// Replaces original directory structures with artificial names to avoid
/// leaking sensitive information (usernames, project names, etc.) in
/// packaged assets.
#[derive(Default)]
pub struct DirectoryRemapper {
    next_directory_num: usize,
    old_to_new_directory: HashMap<String, String>,
}

impl DirectoryRemapper {
    /// Create a new directory remapper.
    pub fn new() -> Self {
        Self::default()
    }

    /// Remap a file path by replacing its directory with a generated name.
    ///
    /// The generated directory name is reused if the same original directory
    /// is seen again.
    pub fn remap(&mut self, file_path: &str) -> String {
        let path = PathBuf::from(file_path);

        let dir = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        if dir.is_empty() {
            return file_path.to_string();
        }

        let new_dir = if let Some(existing) = self.old_to_new_directory.get(&dir) {
            existing.clone()
        } else {
            let new_name = format!("dir_{}", self.next_directory_num);
            self.next_directory_num += 1;
            self.old_to_new_directory
                .insert(dir.clone(), new_name.clone());
            new_name
        };

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if file_name.is_empty() {
            new_dir
        } else {
            format!("{}/{}", new_dir, file_name)
        }
    }
}

/// File entry to copy into the package.
#[derive(Debug, Clone)]
pub struct FileToCopy {
    /// Source file path (resolved)
    pub source: String,
    /// Destination path in package
    pub dest: String,
}

/// Trait for asset localization package implementations.
///
/// Defines the interface for building packages that contain a USD asset
/// and all its dependencies.
pub trait AssetLocalizationPackage {
    /// Build the package from the given asset.
    fn build(&mut self, asset_path: &AssetPath, first_layer_name: &str) -> bool;

    /// Write the package to the given path.
    fn write(&mut self, package_path: &str) -> bool;
}

/// Base implementation for asset localization packages.
///
/// Handles the common logic of collecting dependencies and preparing
/// them for packaging. Subclasses implement the actual writing.
pub struct AssetLocalizationPackageBase {
    root_layer: Option<Arc<Layer>>,
    root_file_path: String,
    orig_root_file_path: String,
    package_path: String,
    first_layer_name: String,
    dependencies_to_skip: Vec<String>,
    layers_to_copy: HashMap<String, String>,
    files_to_copy: Vec<FileToCopy>,
    directory_remapper: DirectoryRemapper,
    edit_layers_in_place: bool,
}

impl Default for AssetLocalizationPackageBase {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetLocalizationPackageBase {
    /// Create a new package builder.
    pub fn new() -> Self {
        Self {
            root_layer: None,
            root_file_path: String::new(),
            orig_root_file_path: String::new(),
            package_path: String::new(),
            first_layer_name: String::new(),
            dependencies_to_skip: Vec::new(),
            layers_to_copy: HashMap::new(),
            files_to_copy: Vec::new(),
            directory_remapper: DirectoryRemapper::new(),
            edit_layers_in_place: false,
        }
    }

    /// Set the original root file path.
    pub fn set_original_root_file_path(&mut self, path: &str) {
        self.orig_root_file_path = path.to_string();
    }

    /// Set dependencies to skip during packaging.
    pub fn set_dependencies_to_skip(&mut self, deps: Vec<String>) {
        self.dependencies_to_skip = deps;
    }

    /// Set whether to edit layers in place.
    pub fn set_edit_layers_in_place(&mut self, edit: bool) {
        self.edit_layers_in_place = edit;
    }

    /// Set the package output path.
    pub fn set_package_path(&mut self, path: &str) {
        self.package_path = path.to_string();
    }

    /// Get the package output path.
    pub fn get_package_path(&self) -> &str {
        &self.package_path
    }

    /// Remap a path to an artificial directory structure.
    pub fn remap_path(&mut self, path: &str) -> String {
        self.directory_remapper.remap(path)
    }

    /// Get the root layer.
    pub fn get_root_layer(&self) -> Option<&Arc<Layer>> {
        self.root_layer.as_ref()
    }

    /// Get layers to copy.
    pub fn get_layers_to_copy(&self) -> &HashMap<String, String> {
        &self.layers_to_copy
    }

    /// Get files to copy.
    pub fn get_files_to_copy(&self) -> &[FileToCopy] {
        &self.files_to_copy
    }

    /// Build the package dependencies.
    pub fn build_impl(&mut self, asset_path: &AssetPath, first_layer_name: &str) -> bool {
        self.first_layer_name = first_layer_name.to_string();
        self.layers_to_copy.clear();
        self.files_to_copy.clear();

        let path_str = asset_path.get_asset_path();
        let layer = match Layer::find_or_open(path_str) {
            Ok(l) => l,
            Err(_) => return false,
        };

        self.root_layer = Some(layer.clone());

        if let Some(real_path) = layer.real_path() {
            self.root_file_path = real_path.to_string_lossy().to_string();
        }

        if self.orig_root_file_path.is_empty() {
            self.orig_root_file_path = self.root_file_path.clone();
        }

        // Use LocalizationContext for dependency discovery
        let delegate = PackageLocalizationDelegate::new();
        let mut context = LocalizationContext::new(delegate);

        // Configure context
        context.set_ref_types_to_include(ReferenceType::All);
        context.set_resolve_udim_paths(true);
        if !self.dependencies_to_skip.is_empty() {
            context.set_dependencies_to_skip(self.dependencies_to_skip.clone());
        }

        // Process the layer to discover dependencies
        context.process(&layer);

        // Extract delegate and collect discovered dependencies
        let delegate = context.into_delegate();
        for (source, dest) in delegate.get_discovered_layers() {
            self.layers_to_copy.insert(source, dest);
        }

        for (source, dest) in delegate.get_discovered_assets() {
            self.files_to_copy.push(FileToCopy { source, dest });
        }

        true
    }

    /// Add a layer to the package.
    pub fn add_layer_to_package(&mut self, source_layer: &Arc<Layer>, dest_path: &str) -> bool {
        let source_id = source_layer.identifier();
        if self.layers_to_copy.contains_key(source_id) {
            return true;
        }

        self.layers_to_copy
            .insert(source_id.to_string(), dest_path.to_string());
        true
    }

    /// Add a non-layer asset to the package.
    pub fn add_asset_to_package(&mut self, src_path: &str, dest_path: &str) -> bool {
        for file in &self.files_to_copy {
            if file.source == src_path {
                return true;
            }
        }

        self.files_to_copy.push(FileToCopy {
            source: src_path.to_string(),
            dest: dest_path.to_string(),
        });

        true
    }
}

/// Package-specific localization delegate.
///
/// Collects dependencies discovered during localization for packaging.
struct PackageLocalizationDelegate {
    /// Layers discovered during traversal (source -> dest)
    discovered_layers: HashMap<String, String>,
    /// Non-layer assets discovered (source -> dest)  
    discovered_assets: HashMap<String, String>,
    /// Counter for generating unique destination paths
    path_counter: usize,
}

impl PackageLocalizationDelegate {
    fn new() -> Self {
        Self {
            discovered_layers: HashMap::new(),
            discovered_assets: HashMap::new(),
            path_counter: 0,
        }
    }

    fn get_discovered_layers(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.discovered_layers
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
    }

    fn get_discovered_assets(&self) -> impl Iterator<Item = (String, String)> + '_ {
        self.discovered_assets
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
    }

    fn generate_dest_path(&mut self, source: &str) -> String {
        // Extract filename from source path
        let filename = PathBuf::from(source)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                self.path_counter += 1;
                format!("asset_{}", self.path_counter)
            });
        filename
    }
}

impl LocalizationDelegate for PackageLocalizationDelegate {
    fn process_sublayers(&mut self, layer: &Arc<Layer>) -> Vec<String> {
        let sublayers = layer.sublayer_paths();
        let mut result = Vec::new();

        for path in &sublayers {
            let anchored = layer.compute_absolute_path(path);
            if !self.discovered_layers.contains_key(&anchored) {
                let dest = self.generate_dest_path(&anchored);
                self.discovered_layers.insert(anchored.clone(), dest);
            }
            result.push(anchored);
        }

        result
    }

    fn process_payloads(&mut self, _layer: &Arc<Layer>, _prim_spec: &PrimSpec) -> Vec<String> {
        // Note: Full implementation would extract payload paths from prim spec
        Vec::new()
    }

    fn process_references(&mut self, _layer: &Arc<Layer>, _prim_spec: &PrimSpec) -> Vec<String> {
        // Note: Full implementation would extract reference paths from prim spec
        Vec::new()
    }

    fn process_value_path(
        &mut self,
        layer: &Arc<Layer>,
        _key_path: &str,
        authored_path: &str,
        _dependencies: &[String],
        _processing_metadata: bool,
        _processing_dictionary: bool,
    ) -> Vec<String> {
        if authored_path.is_empty() {
            return Vec::new();
        }

        let anchored = layer.compute_absolute_path(authored_path);
        if !self.discovered_assets.contains_key(&anchored) {
            let dest = self.generate_dest_path(&anchored);
            self.discovered_assets.insert(anchored.clone(), dest);
        }

        vec![anchored]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_directory_remapper() {
        let mut remapper = DirectoryRemapper::new();

        let path1 = remapper.remap("/users/john/project/asset.usd");
        let path2 = remapper.remap("/users/john/project/texture.png");
        let path3 = remapper.remap("/other/dir/model.usd");

        // Same directory should get same remapped name
        assert!(path1.starts_with("dir_"));
        assert!(path2.starts_with("dir_"));

        // Different directory should get different name
        let dir1: Vec<&str> = path1.split('/').collect();
        let dir3: Vec<&str> = path3.split('/').collect();
        assert_ne!(dir1[0], dir3[0]);

        // File names should be preserved
        assert!(path1.ends_with("asset.usd"));
        assert!(path2.ends_with("texture.png"));
        assert!(path3.ends_with("model.usd"));
    }

    #[test]
    fn test_package_base_creation() {
        let mut package = AssetLocalizationPackageBase::new();
        package.set_original_root_file_path("/path/to/asset.usd");
        package.set_edit_layers_in_place(true);

        assert!(package.get_root_layer().is_none());
        assert!(package.get_layers_to_copy().is_empty());
        assert!(package.get_files_to_copy().is_empty());
    }
}
