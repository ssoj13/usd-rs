//! UsdRender - Render settings schemas for USD.
//!
//! This module provides schemas for configuring render processes, including:
//!
//! - **RenderSettings** - Global render configuration (resolution, camera, etc.)
//! - **RenderProduct** - Output file/buffer specification
//! - **RenderVar** - Custom data variables (AOVs) to produce
//! - **RenderPass** - Multi-pass render configuration
//!
//! # C++ Reference
//!
//! Port of `pxr/usd/usdRender/` module.

mod pass;
mod product;
mod settings;
mod settings_base;
mod spec;
mod tokens;
mod var;

// Public re-exports - Typed schemas
pub use pass::RenderPass;
pub use product::RenderProduct;
pub use settings::RenderSettings;
pub use settings_base::RenderSettingsBase;
pub use var::RenderVar;

// Public re-exports - Utilities
pub use spec::{Product, RenderSpec, RenderVarSpec, compute_namespaced_settings, compute_spec};
pub use tokens::{USD_RENDER_TOKENS, UsdRenderTokensType};
