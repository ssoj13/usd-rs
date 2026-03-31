//! # opensubdiv-rs
//!
//! Pure Rust port of Pixar OpenSubdiv 3.7.0.
//!
//! Subdivision surface evaluation library supporting Catmull-Clark, Loop,
//! and Bilinear schemes with feature-adaptive refinement.

pub mod sdc;
pub mod vtr;
pub mod far;
pub mod bfr;
pub mod osd;

/// OpenSubdiv version
pub const VERSION: (u32, u32, u32) = (3, 7, 0);
