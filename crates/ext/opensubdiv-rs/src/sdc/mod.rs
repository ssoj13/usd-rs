//! Subdivision Core — scheme types, options, crease math, and mask computation.

pub mod types;
pub mod options;
pub mod crease;
pub mod scheme;
pub mod bilinear_scheme;
pub mod catmark_scheme;
pub mod loop_scheme;

pub use types::*;
pub use options::*;
pub use crease::{Crease, Rule};
pub use scheme::Scheme;
