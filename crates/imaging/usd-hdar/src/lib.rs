//! Hydra Asset Resolution (hdar) - Asset resolver integration for scene indices.
//!
//! Port of pxr/imaging/hdar
//!
//! This module provides integration between Hydra's scene index system and
//! the AR (Asset Resolver) system. It allows scene indices to carry asset
//! resolution context data that can be queried when resolving asset paths.
//!
//! # Overview
//!
//! The hdar module consists of:
//!
//! - [`HdarSystemSchema`] - Schema for accessing asset resolution data in scene indices
//! - [`HdarSystemSchemaBuilder`] - Builder for constructing asset resolution containers
//! - [`HdarSystemSchemaTokens`] - Token constants for schema fields
//!
//! # Asset Resolution in Scene Indices
//!
//! Asset resolution context can be stored in the scene index hierarchy under
//! the "system.assetResolution" locator. This allows different parts of the
//! scene to use different resolution contexts (e.g., different search paths).
//!
//! The context is inherited hierarchically - when resolving assets for a prim,
//! the system walks up the namespace looking for the nearest asset resolution
//! context.
//!
//! # Key Concepts
//!
//! - **System Data**: Scene-level metadata stored at "system" locator
//! - **Asset Resolution**: Context for resolving asset paths to physical locations
//! - **Resolver Context**: ArResolverContext containing search paths and settings
//! - **Hierarchical Lookup**: Context is found by walking up the scene hierarchy
//!
//! # Schema Structure
//!
//! ```text
//! prim.dataSource
//!   └── system                      (HdSystemSchema)
//!       └── assetResolution         (HdarSystemSchema)
//!           └── resolverContext     (HdResolverContextDataSource)
//! ```
//!
//! # Usage Example
//!
//! ```rust
//! use usd_hdar::{HdarSystemSchema, HdarSystemSchemaBuilder};
//! use usd_hd::scene_index::HdRetainedSceneIndex;
//! use usd_ar::ResolverContext;
//! use usd_sdf::Path;
//!
//! // Query asset resolution context from scene
//! // let scene_index = HdRetainedSceneIndex::new();
//! // let path = Path::from_str("/World/Characters").unwrap();
//! //
//! // let (container, found_at) = HdarSystemSchema::get_from_path(
//! //     &scene_index,
//! //     &path,
//! // );
//! //
//! // if let Some(container) = container {
//! //     let schema = HdarSystemSchema::new(container);
//! //     if let Some(resolver_ctx_ds) = schema.get_resolver_context() {
//! //         // Use the resolver context for asset resolution
//! //         let ctx = resolver_ctx_ds.get_typed_value(0.0);
//! //         println!("Found context at: {:?}", found_at);
//! //     }
//! // }
//! ```
//!
//! # Building Asset Resolution Data
//!
//! ```rust
//! use usd_hdar::HdarSystemSchemaBuilder;
//! use usd_hd::data_source::HdRetainedTypedSampledDataSource;
//! use usd_ar::ResolverContext;
//!
//! // Create a resolver context
//! // let ctx = ResolverContext::new();
//! // let ctx_ds = HdRetainedTypedSampledDataSource::new(ctx);
//! //
//! // // Build asset resolution container
//! // let container = HdarSystemSchemaBuilder::new()
//! //     .set_resolver_context(std::sync::Arc::new(ctx_ds))
//! //     .build();
//! //
//! // // Add to scene index under system.assetResolution
//! ```
//!
//! # Integration with HdSystemSchema
//!
//! HdarSystemSchema extends HdSystemSchema by providing typed access to the
//! "assetResolution" field in system containers. It uses HdSystemSchema's
//! hierarchy walking functionality to find resolution contexts.
//!
//! # Performance Considerations
//!
//! - Context lookup is O(depth) where depth is the prim's depth in the hierarchy
//! - Contexts are typically cached by scene indices to avoid repeated lookups
//! - Use sparingly at higher levels of the hierarchy for broad coverage
//!
//! # Reference
//!
//! This is a port of OpenUSD's pxr/imaging/hdar module. See the original
//! C++ implementation for additional details:
//! - https://github.com/PixarAnimationStudios/OpenUSD

pub mod system_schema;

#[cfg(test)]
mod tests;

// Re-export main types
pub use system_schema::{
    ASSET_RESOLUTION, HdarSystemSchema, HdarSystemSchemaBuilder, HdarSystemSchemaTokens,
    RESOLVER_CONTEXT,
};
