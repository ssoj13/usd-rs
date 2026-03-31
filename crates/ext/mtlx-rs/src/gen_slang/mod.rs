//! MaterialXGenSlang — Slang shader generation (по рефу MaterialXGenSlang).

mod slang_emit;
mod slang_shader_generator;
mod slang_syntax;

pub use slang_shader_generator::{
    SlangShaderGenerator, SlangShaderGraphContext, TARGET, VERSION, create_slang_shader,
    generate_slang_shader,
};
pub use slang_syntax::{SOURCE_FILE_EXTENSION, SlangSyntax};
