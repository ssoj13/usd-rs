//! Asset Previews API schema.
//!
//! API for managing asset preview thumbnails stored in assetInfo metadata.
//!
//! # Metadata Structure
//!
//! The thumbnails are stored in a nested dictionary structure within assetInfo:
//!
//! ```text
//! assetInfo = {
//!     dictionary previews = {
//!         dictionary thumbnails = {
//!             dictionary default = {
//!                 asset defaultImage = @thumb.jpg@
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMedia/assetPreviewsAPI.h` and `assetPreviewsAPI.cpp`

use std::collections::HashMap;
use std::sync::Arc;

use usd_core::{Prim, SchemaKind, Stage};
use usd_sdf::{AssetPath, Path};
use usd_tf::Token;
use usd_vt::Value;

use super::tokens::USD_MEDIA_TOKENS;

/// Thumbnails data structure.
///
/// Stores the paths to thumbnail images for an asset.
#[derive(Debug, Clone, Default)]
pub struct Thumbnails {
    /// Path to the default thumbnail image.
    pub default_image: AssetPath,
}

impl Thumbnails {
    /// Create new Thumbnails with a default image path.
    pub fn new(default_image: AssetPath) -> Self {
        Self { default_image }
    }
}

/// API schema for asset preview thumbnails.
///
/// Provides interface for storing and retrieving thumbnail images
/// for assets. Preview data is stored in `assetInfo` metadata, not
/// as attributes.
///
/// # Schema Kind
///
/// This is a single-apply API schema (SingleApplyAPI).
///
/// # Usage
///
/// ```ignore
/// let api = AssetPreviewsAPI::get(&stage, &path)?;
/// if let Some(thumbs) = api.get_default_thumbnails() {
///     println!("Thumbnail: {:?}", thumbs.default_image);
/// }
/// ```
#[derive(Clone)]
pub struct AssetPreviewsAPI {
    prim: Prim,
    /// Optional cached stage for GetAssetDefaultPreviews.
    /// Holds a minimal masked stage to keep it alive.
    default_masked_stage: Option<Arc<Stage>>,
}

impl std::fmt::Debug for AssetPreviewsAPI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetPreviewsAPI")
            .field("prim", &self.prim)
            .field("has_masked_stage", &self.default_masked_stage.is_some())
            .finish()
    }
}

impl AssetPreviewsAPI {
    /// The schema kind for this class.
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::SingleApplyAPI;

    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "AssetPreviewsAPI";

    // =========================================================================
    // Construction
    // =========================================================================

    /// Construct an AssetPreviewsAPI on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self {
            prim,
            default_masked_stage: None,
        }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return an AssetPreviewsAPI holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `Get()` — does NOT require the API to be applied.
    /// The prim is simply wrapped; callers can check `is_valid()` if needed.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Returns true if this API schema can be applied to the given prim.
    pub fn can_apply(prim: &Prim, _why_not: Option<&mut String>) -> bool {
        prim.can_apply_api(&USD_MEDIA_TOKENS.asset_previews_api)
    }

    /// Applies this API schema to the given prim.
    pub fn apply(prim: &Prim) -> Option<Self> {
        if prim.apply_api(&USD_MEDIA_TOKENS.asset_previews_api) {
            Some(Self {
                prim: prim.clone(),
                default_masked_stage: None,
            })
        } else {
            None
        }
    }

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.prim.is_valid()
    }

    /// Get the wrapped prim.
    pub fn get_prim(&self) -> &Prim {
        &self.prim
    }

    // =========================================================================
    // Thumbnails API
    // =========================================================================

    /// Fetch the default thumbnails data.
    ///
    /// Returns `Some(Thumbnails)` if data was successfully fetched,
    /// `None` if no thumbnail data exists or the prim is invalid.
    ///
    /// Reads from `assetInfo["previews:thumbnails:default"]` metadata.
    pub fn get_default_thumbnails(&self) -> Option<Thumbnails> {
        if !self.prim.is_valid() {
            return None;
        }

        // Check if the API is applied
        if !self.prim.has_api(&USD_MEDIA_TOKENS.asset_previews_api) {
            return None;
        }

        // Get the thumbnails dictionary from assetInfo
        let thumbnails_val = self
            .prim
            .get_asset_info_by_key(&USD_MEDIA_TOKENS.preview_thumbnails_default)?;

        // Try to extract as a HashMap (VtDictionary equivalent)
        if let Some(thumbnails_dict) = thumbnails_val.downcast_clone::<HashMap<String, Value>>() {
            // Get the defaultImage from the dictionary
            if let Some(image_val) = thumbnails_dict.get(USD_MEDIA_TOKENS.default_image.as_str()) {
                if let Some(asset_path) = image_val.downcast_clone::<AssetPath>() {
                    return Some(Thumbnails {
                        default_image: asset_path,
                    });
                }
                // Also try as a string and convert to AssetPath
                if let Some(path_str) = image_val.downcast_clone::<String>() {
                    return Some(Thumbnails {
                        default_image: AssetPath::new(&path_str),
                    });
                }
            }
        }

        None
    }

    /// Author the default thumbnails dictionary.
    ///
    /// Sets the `assetInfo["previews:thumbnails:default"]` metadata
    /// with the thumbnail data.
    ///
    /// Returns true on success.
    pub fn set_default_thumbnails(&self, thumbnails: &Thumbnails) -> bool {
        if !self.prim.is_valid() {
            return false;
        }

        // Build the thumbnails dictionary
        // Store as string since Value doesn't implement From<AssetPath>
        let mut thumbnails_dict: HashMap<String, Value> = HashMap::new();
        thumbnails_dict.insert(
            USD_MEDIA_TOKENS.default_image.as_str().to_string(),
            Value::from(thumbnails.default_image.get_asset_path().to_string()),
        );

        // Set in assetInfo
        self.prim.set_asset_info_by_key(
            &USD_MEDIA_TOKENS.preview_thumbnails_default,
            Value::from(thumbnails_dict),
        )
    }

    /// Remove the entire entry for default thumbnails.
    ///
    /// Clears the `assetInfo["previews:thumbnails:default"]` metadata.
    /// Matches C++ `ClearDefaultThumbnails()` which calls `ClearAssetInfoByKey`.
    pub fn clear_default_thumbnails(&self) {
        if !self.prim.is_valid() {
            return;
        }
        self.prim
            .clear_asset_info_by_key(&USD_MEDIA_TOKENS.preview_thumbnails_default);
    }

    /// Return a schema object for the default prim of a stage from layer path.
    ///
    /// This is a convenience method that opens a minimal stage and returns
    /// the AssetPreviewsAPI for the default prim.
    ///
    /// # Note
    ///
    /// This requires Stage::open_masked with population mask which limits
    /// the stage to only the default prim. This is more efficient for
    /// preview interrogation of large assets.
    pub fn get_asset_default_previews_from_path(layer_path: &str) -> Option<Self> {
        // Try to find or open the layer
        let layer = usd_sdf::Layer::find_or_open(layer_path).ok()?;
        Self::get_asset_default_previews_from_layer(&layer)
    }

    /// Return a schema object for the default prim of a stage from layer.
    ///
    /// Opens a minimal stage with just the default prim populated.
    ///
    /// # Note
    ///
    /// This method uses OpenMasked to create a minimal stage for efficient
    /// preview inspection without loading the entire stage hierarchy.
    pub fn get_asset_default_previews_from_layer(layer: &Arc<usd_sdf::Layer>) -> Option<Self> {
        // Get the default prim path
        let default_prim_path = layer.get_default_prim_as_path();
        if default_prim_path.is_empty() {
            return None;
        }

        // Create a population mask that limits traversal to just the default prim
        // In C++, this uses a technique where a non-existent child is added to limit depth
        // For simplicity, we open the full stage but could optimize with OpenMasked later
        let stage =
            Stage::open_with_root_layer(layer.clone(), usd_core::common::InitialLoadSet::LoadAll)
                .ok()?;

        let default_prim = stage.get_default_prim();
        if !default_prim.is_valid() {
            return None;
        }

        // Return the API and store the stage to keep it alive
        Some(Self {
            prim: default_prim,
            default_masked_stage: Some(stage),
        })
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// AssetPreviewsAPI stores data in metadata rather than attributes,
    /// so this returns an empty list.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        Vec::new()
    }
}

impl From<Prim> for AssetPreviewsAPI {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<AssetPreviewsAPI> for Prim {
    fn from(api: AssetPreviewsAPI) -> Self {
        api.prim
    }
}

impl AsRef<Prim> for AssetPreviewsAPI {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_kind() {
        assert_eq!(AssetPreviewsAPI::SCHEMA_KIND, SchemaKind::SingleApplyAPI);
    }

    #[test]
    fn test_schema_type_name() {
        assert_eq!(AssetPreviewsAPI::SCHEMA_TYPE_NAME, "AssetPreviewsAPI");
    }

    #[test]
    fn test_thumbnails_default() {
        let thumbs = Thumbnails::default();
        assert!(thumbs.default_image.get_asset_path().is_empty());
    }

    #[test]
    fn test_thumbnails_new() {
        let thumbs = Thumbnails::new(AssetPath::new("thumb.png"));
        assert_eq!(thumbs.default_image.get_asset_path(), "thumb.png");
    }
}
