//! GLSL to SPIR-V shader compilation.
//!
//! Port of pxr/imaging/hgiVulkan/shaderCompiler

use shaderc::{CompileOptions, Compiler, EnvVersion, ShaderKind, SpirvVersion, TargetEnv};
use usd_hgi::HgiShaderStage;

/// Maps an `HgiShaderStage` bitflag to a `shaderc::ShaderKind`.
///
/// Because `HgiShaderStage` is a bitflags type, this checks each bit in
/// priority order matching the C++ switch statement.  Unknown or zero
/// stages fall back to `ShaderKind::InferFromSource`.
fn to_shader_kind(stage: HgiShaderStage) -> ShaderKind {
    if stage.contains(HgiShaderStage::VERTEX) {
        ShaderKind::Vertex
    } else if stage.contains(HgiShaderStage::TESSELLATION_CONTROL) {
        ShaderKind::TessControl
    } else if stage.contains(HgiShaderStage::TESSELLATION_EVAL) {
        ShaderKind::TessEvaluation
    } else if stage.contains(HgiShaderStage::GEOMETRY) {
        ShaderKind::Geometry
    } else if stage.contains(HgiShaderStage::FRAGMENT) {
        ShaderKind::Fragment
    } else if stage.contains(HgiShaderStage::COMPUTE) {
        ShaderKind::Compute
    } else {
        log::error!("to_shader_kind: unknown HgiShaderStage {:?}", stage);
        ShaderKind::InferFromSource
    }
}

/// Compiles GLSL shader source into SPIR-V binary words.
///
/// - `name`: label used in error messages (arbitrary debug string)
/// - `shader_codes`: GLSL source fragments concatenated in order
/// - `stage`: pipeline stage that determines the shader kind
///
/// Targets Vulkan 1.3 / SPIR-V 1.6, matching the C++ reference implementation.
/// Returns the SPIR-V words on success, or the compiler error string on failure.
pub fn compile_glsl(
    name: &str,
    shader_codes: &[&str],
    stage: HgiShaderStage,
) -> Result<Vec<u32>, String> {
    if shader_codes.is_empty() {
        return Err(format!("No shader to compile: {name}"));
    }

    let source: String = shader_codes.concat();

    let mut options = CompileOptions::new()
        .map_err(|e| format!("Failed to create shaderc CompileOptions: {e}"))?;

    options.set_target_env(TargetEnv::Vulkan, EnvVersion::Vulkan1_3 as u32);
    options.set_target_spirv(SpirvVersion::V1_6);

    let kind = to_shader_kind(stage);

    let compiler =
        Compiler::new().map_err(|e| format!("Failed to create shaderc Compiler: {e}"))?;

    let result = compiler
        .compile_into_spirv(&source, kind, name, "main", Some(&options))
        .map_err(|e| e.to_string())?;

    Ok(result.as_binary().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_mapping_vertex() {
        assert_eq!(to_shader_kind(HgiShaderStage::VERTEX), ShaderKind::Vertex);
    }

    #[test]
    fn test_stage_mapping_fragment() {
        assert_eq!(
            to_shader_kind(HgiShaderStage::FRAGMENT),
            ShaderKind::Fragment
        );
    }

    #[test]
    fn test_stage_mapping_compute() {
        assert_eq!(to_shader_kind(HgiShaderStage::COMPUTE), ShaderKind::Compute);
    }

    #[test]
    fn test_stage_mapping_tess_control() {
        assert_eq!(
            to_shader_kind(HgiShaderStage::TESSELLATION_CONTROL),
            ShaderKind::TessControl
        );
    }

    #[test]
    fn test_stage_mapping_tess_eval() {
        assert_eq!(
            to_shader_kind(HgiShaderStage::TESSELLATION_EVAL),
            ShaderKind::TessEvaluation
        );
    }

    #[test]
    fn test_stage_mapping_geometry() {
        assert_eq!(
            to_shader_kind(HgiShaderStage::GEOMETRY),
            ShaderKind::Geometry
        );
    }

    #[test]
    fn test_empty_codes_returns_error() {
        let result = compile_glsl("test", &[], HgiShaderStage::VERTEX);
        assert!(result.is_err());
    }
}
