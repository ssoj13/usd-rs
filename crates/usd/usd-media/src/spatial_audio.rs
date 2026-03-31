//! Spatial Audio schema.
//!
//! Defines properties for encoding playback of audio files within a USD stage.
//! Supports both spatial 3D audio and non-spatial mono/stereo sounds.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMedia/spatialAudio.h`

use std::sync::Arc;

use usd_core::attribute::Variability;
use usd_core::{Attribute, Prim, SchemaKind, Stage};
use usd_geom::xformable::Xformable;
use usd_sdf::{Path, ValueTypeRegistry};
use usd_tf::Token;

use super::tokens::USD_MEDIA_TOKENS;

/// Spatial audio primitive.
///
/// Encodes playback of an audio file or stream. Derives from Xformable
/// to support spatial positioning while also supporting non-spatial audio.
///
/// # Schema Kind
///
/// This is a concrete typed schema (ConcreteTyped).
///
/// # Attributes
///
/// - `filePath` - Path to the audio file
/// - `auralMode` - spatial or nonSpatial
/// - `playbackMode` - Playback behavior (loop, once, etc.)
/// - `startTime` / `endTime` - Playback time range
/// - `mediaOffset` - Offset into media file
/// - `gain` - Volume gain
#[derive(Debug, Clone)]
pub struct SpatialAudio {
    prim: Prim,
}

impl SpatialAudio {
    /// The schema type name.
    pub const SCHEMA_TYPE_NAME: &'static str = "SpatialAudio";

    /// Compile-time constant for this schema's kind.
    /// C++: `static const UsdSchemaKind schemaKind = UsdSchemaKind::ConcreteTyped;`
    pub const SCHEMA_KIND: SchemaKind = SchemaKind::ConcreteTyped;

    /// Returns the schema kind.
    pub fn get_schema_kind(&self) -> SchemaKind {
        Self::SCHEMA_KIND
    }

    /// Construct a SpatialAudio on the given prim.
    pub fn new(prim: Prim) -> Self {
        Self { prim }
    }

    /// Construct from another prim.
    pub fn from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a SpatialAudio holding the prim at `path` on `stage`.
    ///
    /// Note: no type check is performed — matches C++ which simply wraps `stage->GetPrimAtPath(path)`.
    /// An invalid schema object is returned when the path does not exist.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage.get_prim_at_path(path)?;
        Some(Self::new(prim))
    }

    /// Attempt to ensure a prim adhering to this schema at `path` is defined.
    pub fn define(stage: &Arc<Stage>, path: &Path) -> Option<Self> {
        let prim = stage
            .define_prim(path.as_str(), Self::SCHEMA_TYPE_NAME)
            .ok()?;
        Some(Self::new(prim))
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
    // FilePath Attribute
    // =========================================================================

    /// Get the filePath attribute.
    pub fn get_file_path_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(USD_MEDIA_TOKENS.file_path.as_str())
    }

    /// Creates the filePath attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_file_path_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let asset_type = registry.find_type_by_token(&Token::new("asset"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.file_path.as_str(),
                &asset_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // AuralMode Attribute
    // =========================================================================

    /// Get the auralMode attribute (spatial or nonSpatial).
    pub fn get_aural_mode_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_MEDIA_TOKENS.aural_mode.as_str())
    }

    /// Creates the auralMode attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_aural_mode_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.aural_mode.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // PlaybackMode Attribute
    // =========================================================================

    /// Get the playbackMode attribute.
    pub fn get_playback_mode_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_MEDIA_TOKENS.playback_mode.as_str())
    }

    /// Creates the playbackMode attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_playback_mode_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let token_type = registry.find_type_by_token(&Token::new("token"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.playback_mode.as_str(),
                &token_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // StartTime Attribute
    // =========================================================================

    /// Get the startTime attribute.
    ///
    /// Expressed in timeCodesPerSecond of the stage, specifies when
    /// audio playback should start during animation playback.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform timecode startTime = 0` |
    /// | C++ Type | SdfTimeCode |
    pub fn get_start_time_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_MEDIA_TOKENS.start_time.as_str())
    }

    /// Creates the startTime attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_start_time_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let timecode_type = registry.find_type_by_token(&Token::new("timecode"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.start_time.as_str(),
                &timecode_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // EndTime Attribute
    // =========================================================================

    /// Get the endTime attribute.
    ///
    /// Expressed in timeCodesPerSecond, specifies when audio should stop.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform timecode endTime = 0` |
    /// | C++ Type | SdfTimeCode |
    pub fn get_end_time_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(USD_MEDIA_TOKENS.end_time.as_str())
    }

    /// Creates the endTime attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_end_time_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let timecode_type = registry.find_type_by_token(&Token::new("timecode"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.end_time.as_str(),
                &timecode_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // MediaOffset Attribute
    // =========================================================================

    /// Get the mediaOffset attribute.
    ///
    /// Expressed in seconds, specifies offset from audio file's beginning
    /// at which playback should start.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `uniform double mediaOffset = 0` |
    /// | C++ Type | double |
    pub fn get_media_offset_attr(&self) -> Option<Attribute> {
        self.prim
            .get_attribute(USD_MEDIA_TOKENS.media_offset.as_str())
    }

    /// Creates the mediaOffset attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_media_offset_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.media_offset.as_str(),
                &double_type,
                false,
                Some(Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // Gain Attribute
    // =========================================================================

    /// Get the gain attribute.
    ///
    /// Multiplier on the incoming audio signal. 0 mutes, negative clamped to 0.
    ///
    /// | Property | Value |
    /// |----------|-------|
    /// | Declaration | `double gain = 1` |
    /// | C++ Type | double |
    pub fn get_gain_attr(&self) -> Option<Attribute> {
        self.prim.get_attribute(USD_MEDIA_TOKENS.gain.as_str())
    }

    /// Creates the gain attribute.
    ///
    /// # Arguments
    ///
    /// * `default_value` - Optional default value for the attribute
    /// * `write_sparsely` - If true, author sparsely when default_value matches fallback
    pub fn create_gain_attr(
        &self,
        default_value: Option<&usd_vt::Value>,
        write_sparsely: bool,
    ) -> Attribute {
        if !self.prim.is_valid() {
            return Attribute::invalid();
        }

        let registry = ValueTypeRegistry::instance();
        let double_type = registry.find_type_by_token(&Token::new("double"));

        let attr = self
            .prim
            .create_attribute(
                USD_MEDIA_TOKENS.gain.as_str(),
                &double_type,
                false,
                None, // gain is varying, not uniform
            )
            .unwrap_or_else(Attribute::invalid);

        // Set default value if provided
        if let Some(default_val) = default_value {
            if !write_sparsely || !Self::is_value_sparse(default_val, &attr) {
                let _ = attr.set(default_val.clone(), usd_sdf::TimeCode::default_time());
            }
        }

        attr
    }

    // =========================================================================
    // Helper methods
    // =========================================================================

    /// Check if value is sparse (matches fallback).
    ///
    /// Returns true if the value matches the schema's fallback value,
    /// allowing sparse authoring to skip writing redundant data.
    fn is_value_sparse(_value: &usd_vt::Value, _attr: &Attribute) -> bool {
        // For now, return false to always write values
        // A full implementation would compare with schema fallback values
        false
    }

    // =========================================================================
    // Schema attribute names
    // =========================================================================

    /// Returns all pre-declared attributes for this schema class.
    ///
    /// When `include_inherited` is true, includes attributes from ancestor
    /// schemas (UsdGeomXformable and its parents). C++ concatenates with
    /// `UsdGeomXformable::GetSchemaAttributeNames(true)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = vec![
            USD_MEDIA_TOKENS.file_path.clone(),
            USD_MEDIA_TOKENS.aural_mode.clone(),
            USD_MEDIA_TOKENS.playback_mode.clone(),
            USD_MEDIA_TOKENS.start_time.clone(),
            USD_MEDIA_TOKENS.end_time.clone(),
            USD_MEDIA_TOKENS.media_offset.clone(),
            USD_MEDIA_TOKENS.gain.clone(),
        ];

        if include_inherited {
            let mut all_names = Xformable::get_schema_attribute_names(true);
            all_names.extend(local_names);
            all_names
        } else {
            local_names
        }
    }
}

impl From<Prim> for SpatialAudio {
    fn from(prim: Prim) -> Self {
        Self::new(prim)
    }
}

impl From<SpatialAudio> for Prim {
    fn from(audio: SpatialAudio) -> Self {
        audio.prim
    }
}

impl AsRef<Prim> for SpatialAudio {
    fn as_ref(&self) -> &Prim {
        &self.prim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_type_name() {
        assert_eq!(SpatialAudio::SCHEMA_TYPE_NAME, "SpatialAudio");
    }

    #[test]
    fn test_schema_kind() {
        assert_eq!(SpatialAudio::SCHEMA_KIND, SchemaKind::ConcreteTyped);
    }

    #[test]
    fn test_schema_attribute_names() {
        let names = SpatialAudio::get_schema_attribute_names(false);
        assert!(names.iter().any(|n| n == "filePath"));
        assert!(names.iter().any(|n| n == "auralMode"));
        assert!(names.iter().any(|n| n == "gain"));
        assert_eq!(names.len(), 7);
    }

    #[test]
    fn test_schema_attribute_names_inherited() {
        let local = SpatialAudio::get_schema_attribute_names(false);
        let inherited = SpatialAudio::get_schema_attribute_names(true);
        // Inherited includes Xformable attrs (xformOpOrder) + Imageable attrs
        assert!(inherited.len() > local.len());
        // All local attrs must be present in inherited set
        for name in &local {
            assert!(
                inherited.iter().any(|n| n == name),
                "Missing inherited attr: {}",
                name.as_str()
            );
        }
        // Should include xformOpOrder from Xformable
        assert!(inherited.iter().any(|n| n == "xformOpOrder"));
    }
}
