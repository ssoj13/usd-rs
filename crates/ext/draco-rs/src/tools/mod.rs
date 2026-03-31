//! Draco tools module (Rust port of `_ref/draco/src/draco/tools`).
//!
//! What: Library helpers for Draco CLI tooling (transcoder library).
//! Why: Mirrors C++ tools APIs for scene transcoding.
//! How: Exposes `DracoTranscoder` and file/option structs.
//! Where used: `draco-cli` combined tools binary.

pub mod transcoder_lib;
