//! Draco compression: entropy submodule.
//! Reference: `_ref/draco/src/draco/compression/entropy`.
//!
//! This module provides rANS/ANS entropy coding utilities used by bit coders,
//! attribute encoders, and mesh/point-cloud compression pipelines.

pub mod ans;
pub mod rans_symbol_coding;
pub mod rans_symbol_decoder;
pub mod rans_symbol_encoder;
pub mod shannon_entropy;
pub mod symbol_decoding;
pub mod symbol_encoding;
