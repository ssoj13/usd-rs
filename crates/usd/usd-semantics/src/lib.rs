//! UsdSemantics - Semantic tagging schemas for USD.
//!
//! This module provides schemas for semantic labeling:
//!
//! - **LabelsAPI** - Multi-apply API for attaching semantic labels to prims
//! - **LabelsQuery** - Query utility for computing labels with caching
//!
//! # Semantic Labels
//!
//! Labels provide a way to tag prims with semantic information (e.g., "car",
//! "building", "person") for downstream processing like ML training data.
//!
//! # Query Utility
//!
//! LabelsQuery provides efficient querying with caching and inheritance
//! computation. Discard the query when stage state changes.
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdSemantics/` module.

mod labels_api;
mod labels_query;
mod tokens;

// Public re-exports
pub use labels_api::LabelsAPI;
pub use labels_query::{LabelsQuery, QueryTime};
pub use tokens::{USD_SEMANTICS_TOKENS, UsdSemanticsTokensType};
