//! UsdProcImaging - Procedural prim adapter for Hydra.
//!
//! Port of `pxr/usdImaging/usdProcImaging`
//!
//! This module provides adapters for translating USD procedural prims
//! (from the UsdProc schema module) into Hydra scene representations.
//! It bridges the gap between USD procedural definitions and the HdGp
//! (Hydra Generative Procedural) rendering system.
//!
//! # Architecture
//!
//! UsdProcImaging sits between USD schema and Hydra rendering:
//!
//! ```text
//! USD Prims (UsdProc)
//!        |
//!        v
//! UsdProcImaging Adapters
//!        |
//!        v
//! Hydra Scene (HdGp)
//! ```
//!
//! # Components
//!
//! - [`GenerativeProceduralAdapter`] - Adapter for UsdProcGenerativeProcedural
//!
//! # Generative Procedurals
//!
//! A generative procedural is a USD prim that defines procedural content
//! generation. The `proceduralSystem` attribute specifies which system
//! should interpret and execute the procedural.
//!
//! Parameters are delivered through the primvars namespace, allowing
//! procedural systems to access shader-like parameter bindings.
//!
//! # Usage
//!
//! ```ignore
//! use usd_imaging::proc::GenerativeProceduralAdapter;
//! use usd_core::Stage;
//! use usd_core::common::InitialLoadSet;
//! use usd_sdf::Path;
//!
//! let stage = Stage::create_in_memory(InitialLoadSet::LoadAll)
//!     .expect("Failed to create stage");
//! let prim = stage.get_pseudo_root();
//!
//! let adapter = GenerativeProceduralAdapter::new();
//!
//! // Query procedural system type
//! let hydra_type = adapter.get_hydra_prim_type(&prim);
//! println!("Hydra type: {}", hydra_type.as_str());
//!
//! // Get imaging subprims
//! let subprims = adapter.get_imaging_subprims(&prim);
//! println!("Subprims: {:?}", subprims);
//! ```
//!
//! # Plugin System
//!
//! In C++ OpenUSD, this module is registered as a plugin that provides
//! prim adapters. The adapter registry maps USD prim types to their
//! corresponding adapters.
//!
//! # C++ Reference
//!
//! Port of:
//! - `pxr/usdImaging/usdProcImaging/generativeProceduralAdapter.h/cpp`
//! - Plugin registration through `plugInfo.json`
//!
//! # See Also
//!
//! - [`crate::usd_proc`] - USD procedural schemas
//! - [`crate::imaging::hd_gp`] - Hydra Generative Procedural (HdGp) module
//! - [`crate::usd_imaging`] - Base USD imaging infrastructure

pub mod generative_procedural_adapter;
pub mod tokens;

#[cfg(test)]
mod tests;

// Public re-exports
pub use generative_procedural_adapter::GenerativeProceduralAdapter;
pub use tokens::UsdProcImagingTokens;
