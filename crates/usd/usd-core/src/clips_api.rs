//! UsdClipsAPI - API schema for value clips.
//!
//! Port of pxr/usd/usd/clipsAPI.h/cpp
//!
//! API schema that provides an interface to a prim's clip metadata.
//! Clips are a "value resolution" feature that allows one to specify a sequence
//! of USD files (clips) to be consulted, over time, as a source of varying
//! overrides for the prims at and beneath this prim in namespace.

use super::clip_set::{ClipSet, generate_clip_manifest_with_active_times};
use super::clip_set_definition::compute_clip_set_definitions_for_prim_index;
use crate::schema_base::APISchemaBase;
use crate::{Prim, Stage};
use std::sync::Arc;
use usd_sdf::spec::VtDictionary;
use usd_sdf::{AssetPath, Path, StringListOp};
use usd_tf::Token;
use usd_vt::{Array, Value, Vec2dArray};

// ============================================================================
// ClipsAPIInfoKeys
// ============================================================================

/// Tokens for clip info keys in the clips dictionary.
///
/// Matches C++ `UsdClipsAPIInfoKeys`.
pub struct ClipsAPIInfoKeys;

impl ClipsAPIInfoKeys {
    /// Returns the "active" clip info key token.
    pub fn active() -> Token {
        Token::new("active")
    }

    /// Returns the "assetPaths" clip info key token.
    pub fn asset_paths() -> Token {
        Token::new("assetPaths")
    }

    /// Returns the "interpolateMissingClipValues" clip info key token.
    pub fn interpolate_missing_clip_values() -> Token {
        Token::new("interpolateMissingClipValues")
    }

    /// Returns the "manifestAssetPath" clip info key token.
    pub fn manifest_asset_path() -> Token {
        Token::new("manifestAssetPath")
    }

    /// Returns the "primPath" clip info key token.
    pub fn prim_path() -> Token {
        Token::new("primPath")
    }

    /// Returns the "templateAssetPath" clip info key token.
    pub fn template_asset_path() -> Token {
        Token::new("templateAssetPath")
    }

    /// Returns the "templateEndTime" clip info key token.
    pub fn template_end_time() -> Token {
        Token::new("templateEndTime")
    }

    /// Returns the "templateStartTime" clip info key token.
    pub fn template_start_time() -> Token {
        Token::new("templateStartTime")
    }

    /// Returns the "templateStride" clip info key token.
    pub fn template_stride() -> Token {
        Token::new("templateStride")
    }

    /// Returns the "templateActiveOffset" clip info key token.
    pub fn template_active_offset() -> Token {
        Token::new("templateActiveOffset")
    }

    /// Returns the "times" clip info key token.
    pub fn times() -> Token {
        Token::new("times")
    }
}

// ============================================================================
// ClipsAPISetNames
// ============================================================================

/// Tokens for pre-defined clip set names.
///
/// Matches C++ `UsdClipsAPISetNames`.
pub struct ClipsAPISetNames;

impl ClipsAPISetNames {
    /// The default clip set used for API where no clip set is specified.
    pub fn default_() -> Token {
        Token::new("default")
    }
}

// ============================================================================
// ClipsAPI
// ============================================================================

/// API schema for value clips.
///
/// Matches C++ `UsdClipsAPI`.
///
/// This is a NonAppliedAPI schema - it doesn't need to be applied to prims.
#[derive(Debug, Clone)]
pub struct ClipsAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl ClipsAPI {
    /// Constructs a ClipsAPI from a prim.
    ///
    /// Matches C++ `UsdClipsAPI(UsdPrim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs an invalid ClipsAPI.
    ///
    /// Matches C++ default constructor.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.base.prim()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.prim().path()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("ClipsAPI")
    }

    /// Gets a ClipsAPI from a stage and path.
    ///
    /// Matches C++ `UsdClipsAPI::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Gets a ClipsAPI from a prim.
    ///
    /// Matches C++ `UsdClipsAPI(UsdPrim)`.
    pub fn get_from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // ClipsAPI doesn't add any attributes itself
        Vec::new()
    }

    // ========================================================================
    // Clip Info API
    // ========================================================================

    /// Dictionary that contains the definition of the clip sets on this prim.
    ///
    /// Matches C++ `GetClips(VtDictionary* clips)`.
    pub fn get_clips(&self) -> Option<VtDictionary> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        self.prim().get_metadata(&Token::new("clips"))
    }

    /// Set the clips dictionary for this prim.
    ///
    /// Matches C++ `SetClips(const VtDictionary& clips)`.
    pub fn set_clips(&self, clips: VtDictionary) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        self.prim()
            .set_metadata(&Token::new("clips"), Value::from(clips))
    }

    /// ListOp that may be used to affect how opinions from clip sets are applied.
    ///
    /// Matches C++ `GetClipSets(SdfStringListOp* clipSets)`.
    pub fn get_clip_sets(&self) -> Option<StringListOp> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        self.prim().get_metadata(&Token::new("clipSets"))
    }

    /// Set the clip sets list op for this prim.
    ///
    /// Matches C++ `SetClipSets(const SdfStringListOp& clipSets)`.
    pub fn set_clip_sets(&self, clip_sets: StringListOp) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        self.prim()
            .set_metadata(&Token::new("clipSets"), Value::from_no_hash(clip_sets))
    }

    /// Computes and resolves the list of clip asset paths used by the clip set.
    ///
    /// Matches C++ `ComputeClipAssetPaths(const std::string& clipSet)`.
    ///
    /// NOTE: This requires internal clip set definition computation which is not
    /// yet fully implemented. Returns empty array for now.
    pub fn compute_clip_asset_paths(&self, clip_set: &str) -> Array<AssetPath> {
        if self.base.path().is_absolute_root_path() {
            return Array::new(); // Special-case to pre-empt coding errors
        }
        // Compute clip set definition for this prim
        let prim = self.base.get_prim();
        if let Some(prim_index) = prim.prim_index() {
            let mut clip_set_definitions = Vec::new();
            let mut clip_set_names = Vec::new();
            compute_clip_set_definitions_for_prim_index(
                &prim_index,
                &mut clip_set_definitions,
                &mut clip_set_names,
            );

            // Find the clip set definition matching the requested clip set name
            let clip_set_name = if clip_set.is_empty() {
                ClipsAPISetNames::default_().get_text().to_string()
            } else {
                clip_set.to_string()
            };

            if let Some(index) = clip_set_names
                .iter()
                .position(|name| name == &clip_set_name)
            {
                if index < clip_set_definitions.len() {
                    let def = &clip_set_definitions[index];
                    if let Some(ref asset_paths) = def.clip_asset_paths {
                        return asset_paths.clone();
                    }
                }
            }
        }
        Array::new()
    }

    /// Computes and resolves the list of clip asset paths for the default clip set.
    ///
    /// Matches C++ `ComputeClipAssetPaths()` overload.
    pub fn compute_clip_asset_paths_default(&self) -> Array<AssetPath> {
        self.compute_clip_asset_paths(ClipsAPISetNames::default_().get_text())
    }

    /// List of asset paths to the clips in the clip set.
    ///
    /// Matches C++ `GetClipAssetPaths(VtArray<SdfAssetPath>* assetPaths, const std::string& clipSet)`.
    pub fn get_clip_asset_paths(&self, clip_set: &str) -> Option<Array<AssetPath>> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::asset_paths()),
            )
            .and_then(|v| v.downcast_clone::<Array<AssetPath>>())
    }

    /// List of asset paths to the clips in the default clip set.
    ///
    /// Matches C++ `GetClipAssetPaths(VtArray<SdfAssetPath>* assetPaths)` overload.
    pub fn get_clip_asset_paths_default(&self) -> Option<Array<AssetPath>> {
        self.get_clip_asset_paths(ClipsAPISetNames::default_().get_text())
    }

    /// Set the clip asset paths for the clip set.
    ///
    /// Matches C++ `SetClipAssetPaths(const VtArray<SdfAssetPath>& assetPaths, const std::string& clipSet)`.
    pub fn set_clip_asset_paths(&self, asset_paths: &Array<AssetPath>, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::asset_paths()),
            Value::from(asset_paths.clone()),
        )
    }

    /// Set the clip asset paths for the default clip set.
    ///
    /// Matches C++ `SetClipAssetPaths(const VtArray<SdfAssetPath>& assetPaths)` overload.
    pub fn set_clip_asset_paths_default(&self, asset_paths: &Array<AssetPath>) -> bool {
        self.set_clip_asset_paths(asset_paths, ClipsAPISetNames::default_().get_text())
    }

    /// Path to the prim in the clips from which time samples will be read.
    ///
    /// Matches C++ `GetClipPrimPath(std::string* primPath, const std::string& clipSet)`.
    pub fn get_clip_prim_path(&self, clip_set: &str) -> Option<String> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::prim_path()),
            )
            .and_then(|v| v.downcast_clone::<String>())
    }

    /// Path to the prim in the clips for the default clip set.
    ///
    /// Matches C++ `GetClipPrimPath(std::string* primPath)` overload.
    pub fn get_clip_prim_path_default(&self) -> Option<String> {
        self.get_clip_prim_path(ClipsAPISetNames::default_().get_text())
    }

    /// Set the clip prim path for the clip set.
    ///
    /// Matches C++ `SetClipPrimPath(const std::string& primPath, const std::string& clipSet)`.
    pub fn set_clip_prim_path(&self, prim_path: &str, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::prim_path()),
            Value::from(prim_path.to_string()),
        )
    }

    /// Set the clip prim path for the default clip set.
    ///
    /// Matches C++ `SetClipPrimPath(const std::string& primPath)` overload.
    pub fn set_clip_prim_path_default(&self, prim_path: &str) -> bool {
        self.set_clip_prim_path(prim_path, ClipsAPISetNames::default_().get_text())
    }

    /// List of pairs (time, clip index) indicating when each clip is active.
    ///
    /// Matches C++ `GetClipActive(VtVec2dArray* activeClips, const std::string& clipSet)`.
    pub fn get_clip_active(&self, clip_set: &str) -> Option<Vec2dArray> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::active()),
            )
            .and_then(|v| v.downcast_clone::<Vec2dArray>())
    }

    /// List of pairs (time, clip index) for the default clip set.
    ///
    /// Matches C++ `GetClipActive(VtVec2dArray* activeClips)` overload.
    pub fn get_clip_active_default(&self) -> Option<Vec2dArray> {
        self.get_clip_active(ClipsAPISetNames::default_().get_text())
    }

    /// Set the active clip metadata for the clip set.
    ///
    /// Matches C++ `SetClipActive(const VtVec2dArray& activeClips, const std::string& clipSet)`.
    pub fn set_clip_active(&self, active_clips: &Vec2dArray, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::active()),
            Value::from_no_hash(active_clips.clone()),
        )
    }

    /// Set the active clip metadata for the default clip set.
    ///
    /// Matches C++ `SetClipActive(const VtVec2dArray& activeClips)` overload.
    pub fn set_clip_active_default(&self, active_clips: &Vec2dArray) -> bool {
        self.set_clip_active(active_clips, ClipsAPISetNames::default_().get_text())
    }

    /// List of pairs (stage time, clip time) for time mapping.
    ///
    /// Matches C++ `GetClipTimes(VtVec2dArray* clipTimes, const std::string& clipSet)`.
    pub fn get_clip_times(&self, clip_set: &str) -> Option<Vec2dArray> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::times()),
            )
            .and_then(|v| v.downcast_clone::<Vec2dArray>())
    }

    /// List of pairs (stage time, clip time) for the default clip set.
    ///
    /// Matches C++ `GetClipTimes(VtVec2dArray* clipTimes)` overload.
    pub fn get_clip_times_default(&self) -> Option<Vec2dArray> {
        self.get_clip_times(ClipsAPISetNames::default_().get_text())
    }

    /// Set the clip times metadata for the clip set.
    ///
    /// Matches C++ `SetClipTimes(const VtVec2dArray& clipTimes, const std::string& clipSet)`.
    pub fn set_clip_times(&self, clip_times: &Vec2dArray, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::times()),
            Value::from_no_hash(clip_times.clone()),
        )
    }

    /// Set the clip times metadata for the default clip set.
    ///
    /// Matches C++ `SetClipTimes(const VtVec2dArray& clipTimes)` overload.
    pub fn set_clip_times_default(&self, clip_times: &Vec2dArray) -> bool {
        self.set_clip_times(clip_times, ClipsAPISetNames::default_().get_text())
    }

    /// Asset path for the clip manifest for the clip set.
    ///
    /// Matches C++ `GetClipManifestAssetPath(SdfAssetPath* manifestAssetPath, const std::string& clipSet)`.
    pub fn get_clip_manifest_asset_path(&self, clip_set: &str) -> Option<AssetPath> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::manifest_asset_path()),
            )
            .and_then(|v| v.downcast_clone::<AssetPath>())
    }

    /// Asset path for the clip manifest for the default clip set.
    ///
    /// Matches C++ `GetClipManifestAssetPath(SdfAssetPath* manifestAssetPath)` overload.
    pub fn get_clip_manifest_asset_path_default(&self) -> Option<AssetPath> {
        self.get_clip_manifest_asset_path(ClipsAPISetNames::default_().get_text())
    }

    /// Set the clip manifest asset path for the clip set.
    ///
    /// Matches C++ `SetClipManifestAssetPath(const SdfAssetPath& manifestAssetPath, const std::string& clipSet)`.
    pub fn set_clip_manifest_asset_path(
        &self,
        manifest_asset_path: &AssetPath,
        clip_set: &str,
    ) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::manifest_asset_path()),
            Value::from_no_hash(manifest_asset_path.clone()),
        )
    }

    /// Set the clip manifest asset path for the default clip set.
    ///
    /// Matches C++ `SetClipManifestAssetPath(const SdfAssetPath& manifestAssetPath)` overload.
    pub fn set_clip_manifest_asset_path_default(&self, manifest_asset_path: &AssetPath) -> bool {
        self.set_clip_manifest_asset_path(
            manifest_asset_path,
            ClipsAPISetNames::default_().get_text(),
        )
    }

    /// Create a clip manifest containing entries for all attributes in the value clips.
    ///
    /// Matches C++ `GenerateClipManifest(const std::string& clipSet, bool writeBlocksForClipsWithMissingValues)`.
    ///
    /// NOTE: This requires internal clip set computation.
    pub fn generate_clip_manifest(
        &self,
        clip_set: &str,
        write_blocks_for_clips_with_missing_values: bool,
    ) -> Option<Arc<usd_sdf::Layer>> {
        if self.base.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }

        let prim = self.base.get_prim();
        if let Some(prim_index) = prim.prim_index() {
            let mut clip_set_definitions = Vec::new();
            let mut clip_set_names = Vec::new();
            compute_clip_set_definitions_for_prim_index(
                &prim_index,
                &mut clip_set_definitions,
                &mut clip_set_names,
            );

            // Find the clip set definition matching the requested clip set name
            let clip_set_name = if clip_set.is_empty() {
                ClipsAPISetNames::default_().get_text().to_string()
            } else {
                clip_set.to_string()
            };

            if let Some(index) = clip_set_names
                .iter()
                .position(|name| name == &clip_set_name)
            {
                if index < clip_set_definitions.len() {
                    let def = &clip_set_definitions[index];

                    // Create clip set from definition using the public API
                    let mut status = None;
                    if let Some(clip_set_obj) =
                        ClipSet::new(clip_set_name.clone(), def, &mut status)
                    {
                        // Get clip prim path
                        let clip_prim_path = def
                            .clip_prim_path
                            .as_ref()
                            .and_then(|s| Path::from_string(s))
                            .unwrap_or_else(|| prim.path().clone());

                        // Generate manifest using clip set's clips
                        // value_clips is ClipRefPtrVector where ClipRefPtr = Arc<Clip>
                        // Need to get Layer from each Clip
                        let clip_layers: Vec<Arc<usd_sdf::Layer>> = clip_set_obj
                            .value_clips
                            .iter()
                            .filter_map(|clip| clip.get_layer_for_clip())
                            .collect();

                        if !clip_layers.is_empty() {
                            // Extract active times if needed
                            let active_times_opt = if write_blocks_for_clips_with_missing_values {
                                def.clip_active.as_ref().and_then(|active| {
                                    let times: Vec<f64> = active.iter().map(|v| v[0]).collect();
                                    if times.is_empty() { None } else { Some(times) }
                                })
                            } else {
                                None
                            };
                            let active_times_slice = active_times_opt.as_deref();

                            return generate_clip_manifest_with_active_times(
                                &clip_layers,
                                &clip_prim_path,
                                "", // tag
                                active_times_slice,
                            );
                        }
                    }
                }
            }
        }
        None
    }

    /// Create a clip manifest for the default clip set.
    ///
    /// Matches C++ `GenerateClipManifest(bool writeBlocksForClipsWithMissingValues)` overload.
    pub fn generate_clip_manifest_default(
        &self,
        write_blocks_for_clips_with_missing_values: bool,
    ) -> Option<Arc<usd_sdf::Layer>> {
        self.generate_clip_manifest(
            ClipsAPISetNames::default_().get_text(),
            write_blocks_for_clips_with_missing_values,
        )
    }

    /// Create a clip manifest from given clip layers.
    ///
    /// Matches C++ `GenerateClipManifestFromLayers(const SdfLayerHandleVector& clipLayers, const SdfPath& clipPrimPath)`.
    ///
    /// NOTE: This requires internal clip manifest generation.
    pub fn generate_clip_manifest_from_layers(
        clip_layers: &[Arc<usd_sdf::Layer>],
        clip_prim_path: &Path,
    ) -> Option<Arc<usd_sdf::Layer>> {
        // Generate manifest using the internal function
        generate_clip_manifest_with_active_times(
            clip_layers,
            clip_prim_path,
            "",   // tag
            None, // clip_active - not provided in this API
        )
    }

    /// Flag indicating whether values for a clip that does not contain authored time samples are interpolated.
    ///
    /// Matches C++ `GetInterpolateMissingClipValues(bool* interpolate, const std::string& clipSet)`.
    pub fn get_interpolate_missing_clip_values(&self, clip_set: &str) -> Option<bool> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(
                    clip_set,
                    &ClipsAPIInfoKeys::interpolate_missing_clip_values(),
                ),
            )
            .and_then(|v| v.downcast_clone::<bool>())
    }

    /// Flag for the default clip set.
    ///
    /// Matches C++ `GetInterpolateMissingClipValues(bool* interpolate)` overload.
    pub fn get_interpolate_missing_clip_values_default(&self) -> Option<bool> {
        self.get_interpolate_missing_clip_values(ClipsAPISetNames::default_().get_text())
    }

    /// Set whether missing clip values are interpolated from surrounding clips.
    ///
    /// Matches C++ `SetInterpolateMissingClipValues(bool interpolate, const std::string& clipSet)`.
    pub fn set_interpolate_missing_clip_values(&self, interpolate: bool, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(
                clip_set,
                &ClipsAPIInfoKeys::interpolate_missing_clip_values(),
            ),
            Value::from(interpolate),
        )
    }

    /// Set whether missing clip values are interpolated for the default clip set.
    ///
    /// Matches C++ `SetInterpolateMissingClipValues(bool interpolate)` overload.
    pub fn set_interpolate_missing_clip_values_default(&self, interpolate: bool) -> bool {
        self.set_interpolate_missing_clip_values(
            interpolate,
            ClipsAPISetNames::default_().get_text(),
        )
    }

    // ========================================================================
    // Template Clip API
    // ========================================================================

    /// A template string representing a set of assets to be used as clips.
    ///
    /// Matches C++ `GetClipTemplateAssetPath(std::string* clipTemplateAssetPath, const std::string& clipSet)`.
    pub fn get_clip_template_asset_path(&self, clip_set: &str) -> Option<String> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::template_asset_path()),
            )
            .and_then(|v| v.downcast_clone::<String>())
    }

    /// Template asset path for the default clip set.
    ///
    /// Matches C++ `GetClipTemplateAssetPath(std::string* clipTemplateAssetPath)` overload.
    pub fn get_clip_template_asset_path_default(&self) -> Option<String> {
        self.get_clip_template_asset_path(ClipsAPISetNames::default_().get_text())
    }

    /// Set the clip template asset path for the clip set.
    ///
    /// Matches C++ `SetClipTemplateAssetPath(const std::string& clipTemplateAssetPath, const std::string& clipSet)`.
    pub fn set_clip_template_asset_path(
        &self,
        clip_template_asset_path: &str,
        clip_set: &str,
    ) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::template_asset_path()),
            Value::from(clip_template_asset_path.to_string()),
        )
    }

    /// Set the clip template asset path for the default clip set.
    ///
    /// Matches C++ `SetClipTemplateAssetPath(const std::string& clipTemplateAssetPath)` overload.
    pub fn set_clip_template_asset_path_default(&self, clip_template_asset_path: &str) -> bool {
        self.set_clip_template_asset_path(
            clip_template_asset_path,
            ClipsAPISetNames::default_().get_text(),
        )
    }

    /// A double representing the increment value USD will use when searching for asset paths.
    ///
    /// Matches C++ `GetClipTemplateStride(double* clipTemplateStride, const std::string& clipSet)`.
    pub fn get_clip_template_stride(&self, clip_set: &str) -> Option<f64> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::template_stride()),
            )
            .and_then(|v| v.downcast_clone::<f64>())
    }

    /// Template stride for the default clip set.
    ///
    /// Matches C++ `GetClipTemplateStride(double* clipTemplateStride)` overload.
    pub fn get_clip_template_stride_default(&self) -> Option<f64> {
        self.get_clip_template_stride(ClipsAPISetNames::default_().get_text())
    }

    /// Set the template stride for the clip set.
    ///
    /// Matches C++ `SetClipTemplateStride(const double clipTemplateStride, const std::string& clipSet)`.
    pub fn set_clip_template_stride(&self, clip_template_stride: f64, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_template_stride <= 0.0 {
            return false; // Invalid clipTemplateStride - must be greater than 0
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::template_stride()),
            Value::from(clip_template_stride),
        )
    }

    /// Set the template stride for the default clip set.
    ///
    /// Matches C++ `SetClipTemplateStride(const double clipTemplateStride)` overload.
    pub fn set_clip_template_stride_default(&self, clip_template_stride: f64) -> bool {
        self.set_clip_template_stride(
            clip_template_stride,
            ClipsAPISetNames::default_().get_text(),
        )
    }

    /// A double representing the offset value used when determining the active period for each clip.
    ///
    /// Matches C++ `GetClipTemplateActiveOffset(double* clipTemplateActiveOffset, const std::string& clipSet)`.
    pub fn get_clip_template_active_offset(&self, clip_set: &str) -> Option<f64> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::template_active_offset()),
            )
            .and_then(|v| v.downcast_clone::<f64>())
    }

    /// Template active offset for the default clip set.
    ///
    /// Matches C++ `GetClipTemplateActiveOffset(double* clipTemplateActiveOffset)` overload.
    pub fn get_clip_template_active_offset_default(&self) -> Option<f64> {
        self.get_clip_template_active_offset(ClipsAPISetNames::default_().get_text())
    }

    /// Set the clip template active offset for the clip set.
    ///
    /// Matches C++ `SetClipTemplateActiveOffset(const double clipTemplateActiveOffset, const std::string& clipSet)`.
    pub fn set_clip_template_active_offset(
        &self,
        clip_template_active_offset: f64,
        clip_set: &str,
    ) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::template_active_offset()),
            Value::from(clip_template_active_offset),
        )
    }

    /// Set the clip template active offset for the default clip set.
    ///
    /// Matches C++ `SetClipTemplateActiveOffset(const double clipTemplateActiveOffset)` overload.
    pub fn set_clip_template_active_offset_default(
        &self,
        clip_template_active_offset: f64,
    ) -> bool {
        self.set_clip_template_active_offset(
            clip_template_active_offset,
            ClipsAPISetNames::default_().get_text(),
        )
    }

    /// A double which indicates the start of the range USD will use to search for asset paths.
    ///
    /// Matches C++ `GetClipTemplateStartTime(double* clipTemplateStartTime, const std::string& clipSet)`.
    pub fn get_clip_template_start_time(&self, clip_set: &str) -> Option<f64> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::template_start_time()),
            )
            .and_then(|v| v.downcast_clone::<f64>())
    }

    /// Template start time for the default clip set.
    ///
    /// Matches C++ `GetClipTemplateStartTime(double* clipTemplateStartTime)` overload.
    pub fn get_clip_template_start_time_default(&self) -> Option<f64> {
        self.get_clip_template_start_time(ClipsAPISetNames::default_().get_text())
    }

    /// Set the template start time for the clip set.
    ///
    /// Matches C++ `SetClipTemplateStartTime(const double clipTemplateStartTime, const std::string& clipSet)`.
    pub fn set_clip_template_start_time(
        &self,
        clip_template_start_time: f64,
        clip_set: &str,
    ) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::template_start_time()),
            Value::from(clip_template_start_time),
        )
    }

    /// Set the template start time for the default clip set.
    ///
    /// Matches C++ `SetClipTemplateStartTime(const double clipTemplateStartTime)` overload.
    pub fn set_clip_template_start_time_default(&self, clip_template_start_time: f64) -> bool {
        self.set_clip_template_start_time(
            clip_template_start_time,
            ClipsAPISetNames::default_().get_text(),
        )
    }

    /// A double which indicates the end of the range USD will use to search for asset paths.
    ///
    /// Matches C++ `GetClipTemplateEndTime(double* clipTemplateEndTime, const std::string& clipSet)`.
    pub fn get_clip_template_end_time(&self, clip_set: &str) -> Option<f64> {
        if self.path().is_absolute_root_path() {
            return None; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return None; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return None; // Clip set name must be a valid identifier
        }
        self.prim()
            .get_metadata_by_dict_key(
                &Token::new("clips"),
                &make_key_path(clip_set, &ClipsAPIInfoKeys::template_end_time()),
            )
            .and_then(|v| v.downcast_clone::<f64>())
    }

    /// Template end time for the default clip set.
    ///
    /// Matches C++ `GetClipTemplateEndTime(double* clipTemplateEndTime)` overload.
    pub fn get_clip_template_end_time_default(&self) -> Option<f64> {
        self.get_clip_template_end_time(ClipsAPISetNames::default_().get_text())
    }

    /// Set the template end time for the clip set.
    ///
    /// Matches C++ `SetClipTemplateEndTime(const double clipTemplateEndTime, const std::string& clipSet)`.
    pub fn set_clip_template_end_time(&self, clip_template_end_time: f64, clip_set: &str) -> bool {
        if self.path().is_absolute_root_path() {
            return false; // Special-case to pre-empt coding errors
        }
        if clip_set.is_empty() {
            return false; // Empty clip set name not allowed
        }
        if !is_valid_identifier(clip_set) {
            return false; // Clip set name must be a valid identifier
        }
        self.prim().set_metadata_by_dict_key(
            &Token::new("clips"),
            &make_key_path(clip_set, &ClipsAPIInfoKeys::template_end_time()),
            Value::from(clip_template_end_time),
        )
    }

    /// Set the template end time for the default clip set.
    ///
    /// Matches C++ `SetClipTemplateEndTime(const double clipTemplateEndTime)` overload.
    pub fn set_clip_template_end_time_default(&self, clip_template_end_time: f64) -> bool {
        self.set_clip_template_end_time(
            clip_template_end_time,
            ClipsAPISetNames::default_().get_text(),
        )
    }
}

impl PartialEq for ClipsAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for ClipsAPI {}

// ============================================================================
// Helper Functions
// ============================================================================

/// Makes a key path for clip set metadata.
///
/// Matches C++ `_MakeKeyPath(const std::string& clipSet, const TfToken& clipInfoKey)`.
fn make_key_path(clip_set: &str, clip_info_key: &Token) -> Token {
    Token::new(&format!("{}:{}", clip_set, clip_info_key.get_text()))
}

/// Checks if a string is a valid identifier.
///
/// Matches C++ `TfIsValidIdentifier()`.
fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    // First character must be a letter or underscore
    let first = s.chars().next().expect("not empty");
    if !first.is_alphabetic() && first != '_' {
        return false;
    }

    // Remaining characters must be alphanumeric or underscore
    s.chars().skip(1).all(|c| c.is_alphanumeric() || c == '_')
}
