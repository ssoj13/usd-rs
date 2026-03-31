//! Draco compression facade.
//!
//! Core compression options live in `draco-core`. Bitstream encode/decode
//! is implemented in `draco-bitstream` and re-exported here.

pub use draco_core::compression::*;
pub use draco_bitstream::compression::*;
