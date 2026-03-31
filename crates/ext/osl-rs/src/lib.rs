//! # osl-rs — Pure Rust port of OpenShadingLanguage
//!
//! A complete, binary-compatible implementation of
//! [OpenShadingLanguage](https://github.com/AcademySoftwareFoundation/OpenShadingLanguage)
//! in pure Rust.
//!
//! ## Modules
//!
//! **Foundation**: [`typedesc`], [`math`], [`dual`], [`hashes`], [`ustring`], [`strdecls`]
//! **Closures**: [`closure`], [`closure_ops`]
//! **Shader Interface**: [`shaderglobals`], [`renderer`]
//! **Compiler**: [`typespec`], [`symbol`], [`symtab`], [`lexer`], [`ast`], [`parser`], [`typecheck`], [`codegen`], [`builtins`], [`stdosl`], [`preprocess`]
//! **OSO Format**: [`oso`], [`oslquery`]
//! **Runtime**: [`shadingsys`], [`context`], [`interp`], [`optimizer`], [`message`], [`journal`], [`fmt`]
//! **Noise**: [`noise`], [`simplex`], [`gabor`]
//! **Shading**: [`color`], [`opstring`], [`spline`], [`texture`], [`matrix_ops`], [`dict`], [`pointcloud`]
//! **BSDFs**: [`bsdf`], [`bsdf_ext`]
//! **LPE**: [`lpe`], [`accum`]
//! **CLI**: [`oslc`], [`oslinfo`]

pub mod accum;
pub mod ast;
pub mod batched;
pub mod batched_exec;
pub mod bsdf;
pub mod bsdf_ext;
pub mod builtins;
#[cfg(feature = "capi")]
pub mod capi;
pub mod closure;
pub mod closure_ops;
pub mod codegen;
pub mod color;
pub mod context;
pub mod dict;
pub mod dual;
pub mod dual_vec;
pub mod encodedtypes;
pub mod fmt;
pub mod gabor;
pub mod hashes;
pub mod interp;
#[cfg(feature = "jit")]
pub mod jit;
pub mod journal;
pub mod lexer;
pub mod lpe;
pub mod math;
pub mod matrix_ops;
pub mod message;
pub mod noise;
pub mod opstring;
pub mod optimizer;
pub mod oslc;
pub mod oslinfo;
pub mod oslquery;
pub mod oso;
pub mod parser;
pub mod pointcloud;
pub mod preprocess;
pub mod renderer;
pub mod shaderglobals;
pub mod shadingsys;
pub mod simplex;
pub mod spline;
pub mod stdosl;
pub mod strdecls;
pub mod symbol;
pub mod symtab;
pub mod texture;
pub mod typecheck;
pub mod typedesc;
pub mod typespec;
pub mod usd_bridge;
pub mod ustring;

// VFX integration — real texture I/O, OCIO color transforms, and
// production renderer when the `vfx` feature is enabled.
#[cfg(feature = "vfx")]
pub mod color_vfx;
#[cfg(feature = "vfx")]
pub mod texture_vfx;
#[cfg(feature = "vfx")]
pub mod vfx_renderer;

// Re-export primary types at crate root for convenience.
pub use accum::{Accumulator, LPEPathState};
pub use batched::{BatchedRendererServices, BatchedShaderGlobals, Mask, Wide};
pub use batched_exec::BatchedInterpreter;
pub use closure::{
    ClosureColor, ClosureLabels, ClosureNode, ClosureParam, ClosureRef, DirectionKind,
    ScatteringKind,
};
pub use dual::{Dual, Dual2};
pub use lexer::{OslLexer, SourceLoc, Tok};
pub use lpe::{LPEDFA, LPEEvent, compile_lpe};
pub use math::{Color3, Matrix22, Matrix33, Matrix44, Vec2, Vec3};
pub use oslquery::OslQuery;
pub use oso::OsoFile;
pub use parser::ParseError;
pub use renderer::RendererServices;
pub use shaderglobals::ShaderGlobals;
pub use shadingsys::{OSLQueryInfo, ParamInfo, ShadingSystem};
pub use symbol::{Opcode, ShaderType, SymArena, SymType, Symbol};
pub use typedesc::{Aggregate, BaseType, TypeDesc, VecSemantics};
pub use typespec::TypeSpec;
pub use ustring::{UString, UStringHash};

// VFX re-exports at crate root.
// JIT re-exports.
#[cfg(feature = "jit")]
pub use jit::{CompiledShader, CraneliftBackend, JitBackend, JitError};

// VFX re-exports at crate root.
#[cfg(feature = "vfx")]
pub use color_vfx::VfxColorSystem;
#[cfg(feature = "vfx")]
pub use texture_vfx::VfxTextureSystem;
#[cfg(feature = "vfx")]
pub use vfx_renderer::VfxRenderer;

/// The fundamental floating-point type used throughout OSL.
/// OSL operates in single precision by default.
pub type Float = f32;

/// OSL version constants (shared across capi and stdosl).
pub const OSL_VERSION_MAJOR: u32 = 1;
pub const OSL_VERSION_MINOR: u32 = 14;
pub const OSL_VERSION_PATCH: u32 = 0;
/// Encoded version: major * 10000 + minor * 100 + patch.
pub const OSL_VERSION: u32 =
    OSL_VERSION_MAJOR * 10000 + OSL_VERSION_MINOR * 100 + OSL_VERSION_PATCH;

/// OSO file format version (separate from OSL language version).
/// Matches C++ OSO_FILE_VERSION_MAJOR/MINOR from CMakeLists.txt.
pub const OSO_FILE_VERSION_MAJOR: u32 = 1;
pub const OSO_FILE_VERSION_MINOR: u32 = 0;
