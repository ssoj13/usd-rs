//! Subdivision Core — scheme types, options, crease math, and mask computation.

pub mod bilinear_scheme;
pub mod catmark_scheme;
pub mod crease;
pub mod loop_scheme;
pub mod options;
pub mod scheme;
pub mod types;

pub use crease::{Crease, Rule};
pub use options::*;
pub use scheme::Scheme;
pub use types::*;
