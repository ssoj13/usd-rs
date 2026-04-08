//! # opensubdiv-rs
//!
//! Pure Rust port of Pixar OpenSubdiv 3.7.0.
//!
//! Subdivision surface evaluation library supporting Catmull-Clark, Loop,
//! and Bilinear schemes with feature-adaptive refinement.

pub mod bfr;
pub mod far;
pub mod osd;
pub mod sdc;
pub mod vtr;

/// OpenSubdiv version
pub const VERSION: (u32, u32, u32) = (3, 7, 0);
