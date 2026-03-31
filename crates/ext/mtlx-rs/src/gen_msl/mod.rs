//! MaterialXGenMsl — Metal Shading Language (MSL) shader generation.
//! Target "genmsl" for Apple Metal (macOS, iOS).

mod msl_emit;
mod msl_resource_binding_context;
mod msl_shader_generator;
mod msl_syntax;

pub use msl_resource_binding_context::MslResourceBindingContext;
pub use msl_shader_generator::{
    MslShaderGenerator, MslShaderGraphContext, TARGET as MSL_TARGET, VERSION as MSL_VERSION,
};
pub use msl_syntax::MslSyntax;

/// Type alias for OpenUSD compatibility (MaterialX reference: SurfaceNodeMsl.h).
pub type SurfaceNodeMsl = crate::gen_hw::HwSurfaceNode;
