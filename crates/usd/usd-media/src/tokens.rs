//! UsdMedia tokens for media schemas.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMedia/tokens.h`

use std::sync::LazyLock;

use usd_tf::Token;

/// All tokens for UsdMedia schemas.
pub struct UsdMediaTokensType {
    // Aural mode values
    /// "nonSpatial" - Non-spatial audio
    pub non_spatial: Token,
    /// "spatial" - Spatial 3D audio  
    pub spatial: Token,

    // Playback mode values
    /// "onceFromStart" - Play once from start
    pub once_from_start: Token,
    /// "onceFromStartToEnd" - Play once start to end
    pub once_from_start_to_end: Token,
    /// "loopFromStart" - Loop from start
    pub loop_from_start: Token,
    /// "loopFromStartToEnd" - Loop from start to end
    pub loop_from_start_to_end: Token,
    /// "loopFromStage" - Loop from stage time
    pub loop_from_stage: Token,

    // SpatialAudio attribute names
    /// "auralMode" - Aural mode
    pub aural_mode: Token,
    /// "playbackMode" - Playback mode
    pub playback_mode: Token,
    /// "filePath" - Audio file path
    pub file_path: Token,
    /// "startTime" - Start time
    pub start_time: Token,
    /// "endTime" - End time
    pub end_time: Token,
    /// "mediaOffset" - Media offset
    pub media_offset: Token,
    /// "gain" - Audio gain
    pub gain: Token,

    // AssetPreviewsAPI
    /// "previews" - Previews dictionary key
    pub previews: Token,
    /// "thumbnails" - Thumbnails dictionary key
    pub thumbnails: Token,
    /// "defaultImage" - Default thumbnail key
    pub default_image: Token,
    /// "previews:thumbnails" - Nested path to thumbnails dict
    pub preview_thumbnails: Token,
    /// "previews:thumbnails:default" - Nested path to default thumbnails
    pub preview_thumbnails_default: Token,

    // Schema type names
    /// "SpatialAudio" - Schema identifier
    pub spatial_audio: Token,
    /// "AssetPreviewsAPI" - Schema identifier
    pub asset_previews_api: Token,
}

impl UsdMediaTokensType {
    /// Returns all tokens as a vector.
    /// Matches C++ `UsdMediaTokensType::allTokens`.
    pub fn all_tokens(&self) -> Vec<Token> {
        vec![
            self.aural_mode.clone(),
            self.default_image.clone(),
            self.end_time.clone(),
            self.file_path.clone(),
            self.gain.clone(),
            self.loop_from_stage.clone(),
            self.loop_from_start.clone(),
            self.loop_from_start_to_end.clone(),
            self.media_offset.clone(),
            self.non_spatial.clone(),
            self.once_from_start.clone(),
            self.once_from_start_to_end.clone(),
            self.playback_mode.clone(),
            self.previews.clone(),
            self.preview_thumbnails.clone(),
            self.preview_thumbnails_default.clone(),
            self.spatial.clone(),
            self.start_time.clone(),
            self.thumbnails.clone(),
            self.asset_previews_api.clone(),
            self.spatial_audio.clone(),
        ]
    }
}

impl UsdMediaTokensType {
    fn new() -> Self {
        Self {
            // Aural modes
            non_spatial: Token::new("nonSpatial"),
            spatial: Token::new("spatial"),

            // Playback modes
            once_from_start: Token::new("onceFromStart"),
            once_from_start_to_end: Token::new("onceFromStartToEnd"),
            loop_from_start: Token::new("loopFromStart"),
            loop_from_start_to_end: Token::new("loopFromStartToEnd"),
            loop_from_stage: Token::new("loopFromStage"),

            // SpatialAudio attributes
            aural_mode: Token::new("auralMode"),
            playback_mode: Token::new("playbackMode"),
            file_path: Token::new("filePath"),
            start_time: Token::new("startTime"),
            end_time: Token::new("endTime"),
            media_offset: Token::new("mediaOffset"),
            gain: Token::new("gain"),

            // AssetPreviewsAPI
            previews: Token::new("previews"),
            thumbnails: Token::new("thumbnails"),
            default_image: Token::new("defaultImage"),
            preview_thumbnails: Token::new("previews:thumbnails"),
            preview_thumbnails_default: Token::new("previews:thumbnails:default"),

            // Schema types
            spatial_audio: Token::new("SpatialAudio"),
            asset_previews_api: Token::new("AssetPreviewsAPI"),
        }
    }
}

/// Global tokens instance for UsdMedia schemas.
pub static USD_MEDIA_TOKENS: LazyLock<UsdMediaTokensType> = LazyLock::new(UsdMediaTokensType::new);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokens() {
        assert_eq!(USD_MEDIA_TOKENS.spatial_audio.as_str(), "SpatialAudio");
        assert_eq!(USD_MEDIA_TOKENS.file_path.as_str(), "filePath");
        assert_eq!(USD_MEDIA_TOKENS.non_spatial.as_str(), "nonSpatial");
    }
}
