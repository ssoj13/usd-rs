//! OpenGL Foundations (GLF) - GL utility classes.
//!
//! Port of pxr/imaging/glf
//!
//! This module provides OpenGL utility classes for USD imaging:
//!
//! # Components
//!
//! ## Context Management
//! - [`gl_context`] - GL context wrapper with sharing support
//!
//! ## Textures & Targets
//! - [`texture`] - GL texture wrapper
//! - [`draw_target`] - Offscreen render target (FBO)
//!
//! ## Lighting & Materials
//! - [`simple_light`] - Basic light structure
//! - [`simple_material`] - Basic material properties
//! - [`simple_lighting_context`] - Light collection management
//! - [`simple_shadow_array`] - Shadow map array management
//!
//! ## Utilities
//! - [`binding_map`] - Uniform buffer binding management
//! - [`info`] - GL info queries
//!
//! # Status
//!
//! Current implementation provides STUB interfaces. Full GL integration
//! requires:
//! - GL loader (gl, glow, or raw bindings)
//! - Context creation (glutin, winit+raw-window-handle)
//! - Integration with HGI (Hydra Graphics Interface)

pub mod binding_map;
pub mod context_caps;
pub mod debug_codes;
pub mod diagnostic;
pub mod draw_target;
pub mod gl_context;
pub mod gl_context_registry;
pub mod gl_raw_context;
pub mod info;
pub mod simple_light;
pub mod simple_lighting_context;
pub mod simple_material;
pub mod simple_shadow_array;
pub mod texture;
pub mod uniform_block;
pub mod utils;

// Re-exports for convenience
pub use binding_map::GlfBindingMap;
pub use context_caps::GlfContextCaps;
pub use debug_codes::GlfDebugCode;
pub use diagnostic::{
    GlfDebugGroup, GlfGLQueryObject, glf_post_pending_gl_errors, glf_register_debug_callback,
};
pub use draw_target::{GlfDrawTarget, GlfDrawTargetAttachment};
pub use gl_context::{
    GlfAnyGLContextScopeHolder, GlfGLContext, GlfGLContextScopeHolder,
    GlfSharedGLContextScopeHolder,
};
pub use gl_context_registry::{GlfGLContextRegistration, GlfGLContextRegistry};
pub use gl_raw_context::{GlfGLRawContext, GlfGLRawContextScopeHolder, RawContextHandle};
pub use info::glf_has_extensions;
pub use simple_light::{GlfSimpleLight, GlfSimpleLightVector};
pub use simple_lighting_context::GlfSimpleLightingContext;
pub use simple_material::GlfSimpleMaterial;
pub use simple_shadow_array::GlfSimpleShadowArray;
pub use texture::{GlfTexture, GlfTextureBinding, GlfTextureTarget};
pub use uniform_block::GlfUniformBlock;
pub use utils::{
    HioFormat as GlfHioFormat, check_gl_framebuffer_status, get_element_size, get_hio_format,
    get_num_elements,
};

// Type aliases for USD compatibility
pub use usd_sdf::asset_path::AssetPath as SdfAssetPath;
pub use usd_sdf::path::Path as SdfPath;
pub use usd_tf::Token as TfToken;
pub use usd_vt::{Array as VtArray, Dictionary as VtDictionary};
