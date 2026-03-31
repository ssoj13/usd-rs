//! UsdProc - Procedural generation schemas for USD.
//!
//! This module provides schemas for procedural content generation:
//!
//! - **GenerativeProcedural** - Abstract procedural prim definition
//!
//! # Procedural System
//!
//! The GenerativeProcedural prim delivers parameters via primvars namespace
//! properties. The `proceduralSystem` attribute indicates which system
//! should interpret the procedural definition.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdProc/` module.

mod generative_procedural;
mod tokens;

// Public re-exports
pub use generative_procedural::GenerativeProcedural;
pub use tokens::{USD_PROC_TOKENS, UsdProcTokensType};
