//! USD Utils Validators - validation for package integrity and dependencies.
//!
//! Port of _ref/OpenUSD/pxr/usdValidation/usdUtilsValidators/validators.cpp
//!
//! Provides 5 validators:
//! - PackageEncapsulationValidator: Checks layers/assets in package stay within package path
//! - FileExtensionValidator: Checks USDZ files only contain allowed extensions
//! - MissingReferenceValidator: Checks for unresolvable dependencies
//! - RootPackageValidator: Checks root package for compression/alignment issues
//! - UsdzPackageValidator: Checks ALL referenced packages for compression/alignment

use crate::{
    ErrorSite, ErrorType, ValidateStageTaskFn, ValidationError, ValidationRegistry,
    ValidationTimeRange, ValidatorMetadata,
};
use std::collections::HashSet;
use std::path::Path as StdPath;
use std::sync::{Arc, LazyLock};
use usd_ar::package_utils::{is_package_relative_path, split_package_relative_path_outer};
use usd_core::stage::Stage;
use usd_sdf::Path;
use usd_sdf::asset_path::AssetPath;
use usd_sdf::layer::Layer;
use usd_sdf::zip_file::ZipFile;
use usd_tf::Token;
use usd_utils::dependencies::compute_all_dependencies;

// ============================================================================
// Validator Name Tokens (prefixed with "usdUtilsValidators:")
// ============================================================================

/// Token for PackageEncapsulationValidator.
pub static PACKAGE_ENCAPSULATION_VALIDATOR: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdUtilsValidators:PackageEncapsulationValidator"));

/// Token for FileExtensionValidator.
pub static FILE_EXTENSION_VALIDATOR: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdUtilsValidators:FileExtensionValidator"));

/// Token for MissingReferenceValidator.
pub static MISSING_REFERENCE_VALIDATOR: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdUtilsValidators:MissingReferenceValidator"));

/// Token for RootPackageValidator.
pub static ROOT_PACKAGE_VALIDATOR: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdUtilsValidators:RootPackageValidator"));

/// Token for UsdzPackageValidator.
pub static USDZ_PACKAGE_VALIDATOR: LazyLock<Token> =
    LazyLock::new(|| Token::new("usdUtilsValidators:UsdzPackageValidator"));

// ============================================================================
// Error Name Tokens
// ============================================================================

/// Error: layer not in package.
pub static LAYER_NOT_IN_PACKAGE: LazyLock<Token> =
    LazyLock::new(|| Token::new("LayerNotInPackage"));

/// Error: asset not in package.
pub static ASSET_NOT_IN_PACKAGE: LazyLock<Token> =
    LazyLock::new(|| Token::new("AssetNotInPackage"));

/// Error: invalid layer in package.
pub static INVALID_LAYER_IN_PACKAGE: LazyLock<Token> =
    LazyLock::new(|| Token::new("InvalidLayerInPackage"));

/// Error: unsupported file extension.
pub static UNSUPPORTED_FILE_EXTENSION: LazyLock<Token> =
    LazyLock::new(|| Token::new("UnsupportedFileExtensionInPackage"));

/// Error: unresolvable dependency.
pub static UNRESOLVABLE_DEPENDENCY: LazyLock<Token> =
    LazyLock::new(|| Token::new("UnresolvableDependency"));

/// Error: compression detected.
pub static COMPRESSION_DETECTED: LazyLock<Token> =
    LazyLock::new(|| Token::new("CompressionDetected"));

/// Error: byte misalignment.
pub static BYTE_MISALIGNMENT: LazyLock<Token> = LazyLock::new(|| Token::new("ByteMisalignment"));

// ============================================================================
// Keyword Tokens
// ============================================================================

static USD_UTILS_VALIDATORS: LazyLock<Token> = LazyLock::new(|| Token::new("UsdUtilsValidators"));

static USDZ_VALIDATORS: LazyLock<Token> = LazyLock::new(|| Token::new("UsdzValidators"));

// ============================================================================
// Helper Functions
// ============================================================================

/// Valid file extensions for USDZ packages.
static VALID_USDZ_EXTENSIONS: &[&str] = &[
    "usda", "usdc", "usd", "usdz", "png", "jpg", "jpeg", "exr", "avif", "m4a", "mp3", "wav",
];

/// Gets file extension from a path.
fn get_extension(path: &str) -> Option<String> {
    StdPath::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase())
}

/// Checks if a file extension is valid for USDZ.
fn is_valid_usdz_extension(ext: &str) -> bool {
    VALID_USDZ_EXTENSIONS.contains(&ext)
}

/// Helper to get USDZ package errors (compression and alignment).
fn get_usdz_package_errors(layer: &Arc<Layer>) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let real_path = match layer.real_path() {
        Some(path) => path.to_string_lossy().to_string(),
        None => return errors,
    };

    // Try to open as ZIP file
    let zip_file = match ZipFile::open(&real_path) {
        Ok(zip) => zip,
        Err(_) => return errors,
    };

    // Check each file in the archive
    for file_name in zip_file.iter() {
        if let Some(info) = zip_file.find(file_name) {
            // Check compression (method 0 = stored/uncompressed)
            if info.compression_method != 0 {
                errors.push(ValidationError::new(
                    COMPRESSION_DETECTED.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_layer(layer, Path::empty())],
                    format!(
                        "File '{}' in package '{}' is compressed (method {}). \
                         USDZ files must be uncompressed.",
                        file_name, real_path, info.compression_method
                    ),
                ));
            }

            // Check 64-byte alignment
            if info.data_offset % 64 != 0 {
                errors.push(ValidationError::new(
                    BYTE_MISALIGNMENT.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_layer(layer, Path::empty())],
                    format!(
                        "File '{}' in package '{}' has data offset {} which is not \
                         aligned to 64 bytes. USDZ requires 64-byte alignment.",
                        file_name, real_path, info.data_offset
                    ),
                ));
            }
        }
    }

    errors
}

// ============================================================================
// Validator 1: PackageEncapsulationValidator
// ============================================================================

/// Validates that layers and assets in a package stay within the package path.
fn package_encapsulation_validator_fn() -> ValidateStageTaskFn {
    Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
        let mut errors = Vec::new();

        let root_layer = stage.get_root_layer();
        let root_identifier = root_layer.identifier();
        let root_real_path = root_layer.real_path();

        // Check if root layer is a package
        let is_package = root_layer
            .get_file_format()
            .map(|ff| ff.is_package())
            .unwrap_or(false)
            || is_package_relative_path(root_identifier);

        if !is_package {
            return errors;
        }

        // Determine package path
        let package_path = if is_package_relative_path(root_identifier) {
            let (outer, _) = split_package_relative_path_outer(root_identifier);
            outer
        } else if let Some(real_path) = root_real_path {
            real_path.to_string_lossy().to_string()
        } else {
            return errors;
        };

        // Compute all dependencies
        let asset_path = AssetPath::new(root_identifier);
        let Some((layers, assets, _unresolved)) = compute_all_dependencies(&asset_path, None)
        else {
            return errors;
        };

        // Check that all referenced layers are within package
        for layer in &layers {
            // Skip the root layer itself
            if Arc::ptr_eq(layer, &root_layer) {
                continue;
            }

            let layer_real_path = match layer.real_path() {
                Some(path) => path.to_string_lossy().to_string(),
                None => {
                    errors.push(ValidationError::new(
                        INVALID_LAYER_IN_PACKAGE.clone(),
                        ErrorType::Error,
                        vec![ErrorSite::from_layer(layer, Path::empty())],
                        format!(
                            "Layer '{}' referenced by package '{}' has no real path \
                             and cannot be validated for encapsulation.",
                            layer.identifier(),
                            package_path
                        ),
                    ));
                    continue;
                }
            };

            if !layer_real_path.starts_with(&package_path) {
                errors.push(ValidationError::new(
                    LAYER_NOT_IN_PACKAGE.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_layer(layer, Path::empty())],
                    format!(
                        "Layer '{}' is outside package '{}'. All referenced layers \
                         must be within the package directory.",
                        layer_real_path, package_path
                    ),
                ));
            }
        }

        // Check that all referenced assets are within package
        for asset in &assets {
            if !asset.starts_with(&package_path) {
                errors.push(ValidationError::new(
                    ASSET_NOT_IN_PACKAGE.clone(),
                    ErrorType::Error,
                    vec![ErrorSite::from_stage(stage, Path::empty(), None)],
                    format!(
                        "Asset '{}' is outside package '{}'. All referenced assets \
                         must be within the package directory.",
                        asset, package_path
                    ),
                ));
            }
        }

        errors
    })
}

// ============================================================================
// Validator 2: FileExtensionValidator
// ============================================================================

/// Validates that USDZ files only contain allowed file extensions.
fn file_extension_validator_fn() -> ValidateStageTaskFn {
    Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
        let mut errors = Vec::new();

        let root_layer = stage.get_root_layer();
        let real_path = match root_layer.real_path() {
            Some(path) => path.to_string_lossy().to_string(),
            None => return errors,
        };

        // Check if this is a USDZ file
        if get_extension(&real_path) != Some("usdz".to_string()) {
            return errors;
        }

        // Open as ZIP file
        let zip_file = match ZipFile::open(&real_path) {
            Ok(zip) => zip,
            Err(_) => return errors,
        };

        // Check each file extension
        for file_name in zip_file.iter() {
            if let Some(ext) = get_extension(file_name) {
                if !is_valid_usdz_extension(&ext) {
                    errors.push(ValidationError::new(
                        UNSUPPORTED_FILE_EXTENSION.clone(),
                        ErrorType::Error,
                        vec![ErrorSite::from_layer(&root_layer, Path::empty())],
                        format!(
                            "File '{}' in USDZ package '{}' has unsupported extension '.{}'. \
                             Allowed extensions: {}",
                            file_name,
                            real_path,
                            ext,
                            VALID_USDZ_EXTENSIONS.join(", ")
                        ),
                    ));
                }
            }
        }

        errors
    })
}

// ============================================================================
// Validator 3: MissingReferenceValidator
// ============================================================================

/// Validates that all dependencies can be resolved.
fn missing_reference_validator_fn() -> ValidateStageTaskFn {
    Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
        let mut errors = Vec::new();

        let root_layer = stage.get_root_layer();
        let asset_path = AssetPath::new(root_layer.identifier());

        // Compute all dependencies
        let Some((_layers, _assets, unresolved_paths)) =
            compute_all_dependencies(&asset_path, None)
        else {
            return errors;
        };

        // Report each unresolved path
        for unresolved_path in unresolved_paths {
            errors.push(ValidationError::new(
                UNRESOLVABLE_DEPENDENCY.clone(),
                ErrorType::Error,
                vec![ErrorSite::from_stage(stage, Path::empty(), None)],
                format!(
                    "Unresolvable dependency '{}' referenced from stage root layer '{}'.",
                    unresolved_path,
                    root_layer.identifier()
                ),
            ));
        }

        errors
    })
}

// ============================================================================
// Validator 4: RootPackageValidator
// ============================================================================

/// Validates the root package for compression and alignment issues.
fn root_package_validator_fn() -> ValidateStageTaskFn {
    Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
        let root_layer = stage.get_root_layer();

        // Check if root layer is a package
        let is_package = root_layer
            .get_file_format()
            .map(|ff| ff.is_package())
            .unwrap_or(false);

        if !is_package {
            return Vec::new();
        }

        // Get USDZ package errors
        get_usdz_package_errors(&root_layer)
    })
}

// ============================================================================
// Validator 5: UsdzPackageValidator
// ============================================================================

/// Validates ALL referenced packages for compression and alignment issues.
fn usdz_package_validator_fn() -> ValidateStageTaskFn {
    Arc::new(|stage: &Arc<Stage>, _time_range: &ValidationTimeRange| {
        let mut errors = Vec::new();

        let root_layer = stage.get_root_layer();
        let asset_path = AssetPath::new(root_layer.identifier());

        // Compute all dependencies
        let Some((layers, _assets, _unresolved)) = compute_all_dependencies(&asset_path, None)
        else {
            return errors;
        };

        // Track validated packages to avoid duplicates
        let mut validated_packages: HashSet<String> = HashSet::new();

        // Check each referenced layer that is a package
        for layer in &layers {
            let is_package = layer
                .get_file_format()
                .map(|ff| ff.is_package())
                .unwrap_or(false);

            if !is_package {
                continue;
            }

            let real_path = match layer.real_path() {
                Some(path) => path.to_string_lossy().to_string(),
                None => continue,
            };

            if validated_packages.contains(&real_path) {
                continue;
            }

            validated_packages.insert(real_path.clone());

            // Get USDZ package errors for this layer
            errors.extend(get_usdz_package_errors(layer));
        }

        errors
    })
}

// ============================================================================
// Registration
// ============================================================================

/// Register all USD Utils validators with the global registry.
///
/// Call this function once during application initialization to make
/// the USD Utils validators available.
pub fn register_utils_validators(registry: &ValidationRegistry) {
    // 1. PackageEncapsulationValidator
    registry.register_stage_validator(
        ValidatorMetadata::new(PACKAGE_ENCAPSULATION_VALIDATOR.clone())
            .with_doc(
                "Validates that layers and assets referenced by a package are \
                 contained within the package's directory structure."
                    .to_string(),
            )
            .with_keywords(vec![USD_UTILS_VALIDATORS.clone()]),
        package_encapsulation_validator_fn(),
        Vec::new(),
    );

    // 2. FileExtensionValidator
    registry.register_stage_validator(
        ValidatorMetadata::new(FILE_EXTENSION_VALIDATOR.clone())
            .with_doc(
                "Validates that USDZ packages only contain files with allowed \
                 extensions (usda, usdc, usd, usdz, png, jpg, jpeg, exr, avif, \
                 m4a, mp3, wav)."
                    .to_string(),
            )
            .with_keywords(vec![USD_UTILS_VALIDATORS.clone(), USDZ_VALIDATORS.clone()]),
        file_extension_validator_fn(),
        Vec::new(),
    );

    // 3. MissingReferenceValidator
    registry.register_stage_validator(
        ValidatorMetadata::new(MISSING_REFERENCE_VALIDATOR.clone())
            .with_doc(
                "Validates that all dependencies (sublayers, references, payloads, \
                 assets) can be resolved."
                    .to_string(),
            )
            .with_keywords(vec![USD_UTILS_VALIDATORS.clone()]),
        missing_reference_validator_fn(),
        Vec::new(),
    );

    // 4. RootPackageValidator
    registry.register_stage_validator(
        ValidatorMetadata::new(ROOT_PACKAGE_VALIDATOR.clone())
            .with_doc(
                "Validates the root package for USDZ compliance: checks that files \
                 are uncompressed and data is aligned to 64-byte boundaries."
                    .to_string(),
            )
            .with_keywords(vec![USD_UTILS_VALIDATORS.clone(), USDZ_VALIDATORS.clone()]),
        root_package_validator_fn(),
        Vec::new(),
    );

    // 5. UsdzPackageValidator
    registry.register_stage_validator(
        ValidatorMetadata::new(USDZ_PACKAGE_VALIDATOR.clone())
            .with_doc(
                "Validates ALL referenced packages for USDZ compliance: checks that \
                 files are uncompressed and data is aligned to 64-byte boundaries."
                    .to_string(),
            )
            .with_keywords(vec![USD_UTILS_VALIDATORS.clone(), USDZ_VALIDATORS.clone()]),
        usdz_package_validator_fn(),
        Vec::new(),
    );
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use usd_core::common::InitialLoadSet;
    use usd_sdf::zip_file::ZipFileWriter;

    // -- Token constants --

    #[test]
    fn test_validator_tokens() {
        assert_eq!(
            PACKAGE_ENCAPSULATION_VALIDATOR.as_str(),
            "usdUtilsValidators:PackageEncapsulationValidator"
        );
        assert_eq!(
            FILE_EXTENSION_VALIDATOR.as_str(),
            "usdUtilsValidators:FileExtensionValidator"
        );
        assert_eq!(
            MISSING_REFERENCE_VALIDATOR.as_str(),
            "usdUtilsValidators:MissingReferenceValidator"
        );
        assert_eq!(
            ROOT_PACKAGE_VALIDATOR.as_str(),
            "usdUtilsValidators:RootPackageValidator"
        );
        assert_eq!(
            USDZ_PACKAGE_VALIDATOR.as_str(),
            "usdUtilsValidators:UsdzPackageValidator"
        );
    }

    #[test]
    fn test_error_tokens() {
        assert_eq!(LAYER_NOT_IN_PACKAGE.as_str(), "LayerNotInPackage");
        assert_eq!(ASSET_NOT_IN_PACKAGE.as_str(), "AssetNotInPackage");
        assert_eq!(INVALID_LAYER_IN_PACKAGE.as_str(), "InvalidLayerInPackage");
        assert_eq!(
            UNSUPPORTED_FILE_EXTENSION.as_str(),
            "UnsupportedFileExtensionInPackage"
        );
        assert_eq!(UNRESOLVABLE_DEPENDENCY.as_str(), "UnresolvableDependency");
        assert_eq!(COMPRESSION_DETECTED.as_str(), "CompressionDetected");
        assert_eq!(BYTE_MISALIGNMENT.as_str(), "ByteMisalignment");
    }

    // -- Helper functions --

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("file.usd"), Some("usd".to_string()));
        assert_eq!(get_extension("file.USDA"), Some("usda".to_string()));
        assert_eq!(get_extension("file.png"), Some("png".to_string()));
        assert_eq!(get_extension("file"), None);
        assert_eq!(get_extension(""), None);
    }

    #[test]
    fn test_is_valid_usdz_extension() {
        assert!(is_valid_usdz_extension("usd"));
        assert!(is_valid_usdz_extension("usda"));
        assert!(is_valid_usdz_extension("usdc"));
        assert!(is_valid_usdz_extension("usdz"));
        assert!(is_valid_usdz_extension("png"));
        assert!(is_valid_usdz_extension("jpg"));
        assert!(is_valid_usdz_extension("jpeg"));
        assert!(is_valid_usdz_extension("exr"));
        assert!(is_valid_usdz_extension("avif"));
        assert!(is_valid_usdz_extension("m4a"));
        assert!(is_valid_usdz_extension("mp3"));
        assert!(is_valid_usdz_extension("wav"));

        assert!(!is_valid_usdz_extension("txt"));
        assert!(!is_valid_usdz_extension("exe"));
        assert!(!is_valid_usdz_extension("usd2"));
    }

    // -- FileExtensionValidator --
    //
    // Note: Full integration tests with .usdz require file format registration.
    // For now, we test the validator logic with mock scenarios.

    #[test]
    fn test_file_extension_validator_non_usdz() {
        // Create a non-USDZ stage
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let validator_fn = file_extension_validator_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        // Should skip validation for non-USDZ files
        assert!(errors.is_empty());
    }

    // -- MissingReferenceValidator --

    #[test]
    fn test_missing_reference_validator_no_missing() {
        // In-memory stage with no external references
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let validator_fn = missing_reference_validator_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        // Should have no errors
        assert!(errors.is_empty());
    }

    // -- RootPackageValidator --

    #[test]
    fn test_root_package_validator_non_package() {
        // In-memory stage (not a package)
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let validator_fn = root_package_validator_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        // Should skip validation for non-packages
        assert!(errors.is_empty());
    }

    // -- UsdzPackageValidator --

    #[test]
    fn test_usdz_package_validator_no_packages() {
        // In-memory stage with no package references
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();

        let validator_fn = usdz_package_validator_fn();
        let errors = validator_fn(&stage, &ValidationTimeRange::default());

        // Should have no errors
        assert!(errors.is_empty());
    }

    // -- Integration with registry --

    #[test]
    fn test_register_utils_validators() {
        let registry = ValidationRegistry::get_instance();
        register_utils_validators(registry);

        assert!(registry.has_validator(&PACKAGE_ENCAPSULATION_VALIDATOR));
        assert!(registry.has_validator(&FILE_EXTENSION_VALIDATOR));
        assert!(registry.has_validator(&MISSING_REFERENCE_VALIDATOR));
        assert!(registry.has_validator(&ROOT_PACKAGE_VALIDATOR));
        assert!(registry.has_validator(&USDZ_PACKAGE_VALIDATOR));
    }

    #[test]
    fn test_validators_have_keywords() {
        let registry = ValidationRegistry::get_instance();
        register_utils_validators(registry);

        let metadata = registry
            .get_validator_metadata(&PACKAGE_ENCAPSULATION_VALIDATOR)
            .unwrap();
        assert!(metadata.keywords.iter().any(|k| k == "UsdUtilsValidators"));

        let metadata = registry
            .get_validator_metadata(&FILE_EXTENSION_VALIDATOR)
            .unwrap();
        assert!(metadata.keywords.iter().any(|k| k == "UsdUtilsValidators"));
        assert!(metadata.keywords.iter().any(|k| k == "UsdzValidators"));
    }

    #[test]
    fn test_get_usdz_package_errors_valid_zip() {
        // Create a well-formed USDZ (uncompressed, aligned)
        let temp_dir = std::env::temp_dir();
        let usdz_path = temp_dir.join("test_good.usdz");
        let usdz_path_str = usdz_path.to_string_lossy().to_string();

        {
            let mut writer = ZipFileWriter::create_new(&usdz_path_str);
            writer.add_file_data("scene.usda", b"#usda 1.0").unwrap();
            writer.save().unwrap();
        }

        // Verify the ZIP was created and is readable
        let zip = ZipFile::open(&usdz_path_str).unwrap();
        assert!(zip.is_valid());
        assert_eq!(zip.len(), 1);

        // Note: Full test would require Layer to support .usdz file format
        // For now, verify ZIP structure is correct
        let file_info = zip.find("scene.usda").unwrap();
        assert_eq!(file_info.compression_method, 0); // Uncompressed
        assert_eq!(file_info.data_offset % 64, 0); // 64-byte aligned

        std::fs::remove_file(&usdz_path_str).ok();
    }
}
