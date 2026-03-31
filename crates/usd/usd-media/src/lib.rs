//! UsdMedia - Media schemas for USD.
//!
//! This module provides schemas for media assets:
//!
//! - **SpatialAudio** - Spatial audio playback primitive
//! - **AssetPreviewsAPI** - API for asset thumbnail previews
//!
//! # Spatial Audio
//!
//! The SpatialAudio prim supports both spatial 3D audio and non-spatial
//! mono/stereo audio playback within a USD stage.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdMedia/` module.

mod asset_previews_api;
mod spatial_audio;
mod tokens;

// Public re-exports
pub use asset_previews_api::{AssetPreviewsAPI, Thumbnails};
pub use spatial_audio::SpatialAudio;
pub use tokens::{USD_MEDIA_TOKENS, UsdMediaTokensType};
