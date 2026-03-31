//! VkSyntax -- Vulkan GLSL syntax.
//! By ref MaterialX VkSyntax.h/.cpp -- extends GlslSyntax, overrides input qualifier.

use crate::gen_shader::TypeSystem;

use super::glsl_syntax::GlslSyntax;

/// Vulkan uses "in" as the input qualifier (same as base GLSL).
/// The C++ override exists for API symmetry; in practice the value is identical.
#[allow(dead_code)]
pub const VK_INPUT_QUALIFIER: &str = "in";

/// Create Vulkan GLSL syntax (identical to GlslSyntax; no behavioral changes needed).
pub fn create_vk_syntax(type_system: TypeSystem) -> GlslSyntax {
    GlslSyntax::create(type_system)
}
