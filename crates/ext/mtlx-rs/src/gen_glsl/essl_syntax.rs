//! EsslSyntax -- ESSL (OpenGL ES Shading Language) syntax.
//! By ref MaterialX EsslSyntax.h/.cpp -- trivially extends GlslSyntax with no overrides.

use crate::gen_shader::TypeSystem;

use super::glsl_syntax::GlslSyntax;

/// Create ESSL syntax (identical to GLSL syntax; no overrides needed for ES).
pub fn create_essl_syntax(type_system: TypeSystem) -> GlslSyntax {
    GlslSyntax::create(type_system)
}
