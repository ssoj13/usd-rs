//! UsdHydra - Hydra integration schemas for USD.
//!
//! This module provides schemas for Hydra-specific features:
//!
//! - **HydraGenerativeProceduralAPI** - Configure Hydra generative procedurals
//!
//! # Texture Sampling Tokens
//!
//! This module also provides tokens for texture sampling parameters:
//!
//! - Wrap modes: `black`, `clamp`, `mirror`, `repeat`, `useMetadata`
//! - Filter modes: `nearest`, `linear`, `nearestMipmapNearest`, etc.
//! - Shader inputs: `wrapS`, `wrapT`, `minFilter`, `magFilter`
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdHydra/` module.

mod discovery_plugin;
mod generative_procedural_api;
mod tokens;

// Public re-exports - API schemas
pub use discovery_plugin::{DiscoveryContext, UsdHydraDiscoveryPlugin};
pub use generative_procedural_api::HydraGenerativeProceduralAPI;

// Public re-exports - Tokens
pub use tokens::{USD_HYDRA_TOKENS, UsdHydraTokensType};
