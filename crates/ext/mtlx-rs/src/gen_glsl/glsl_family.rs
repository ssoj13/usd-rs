//! GlslFamilyBase — shared base for all GLSL-family generators (GLSL, Vk, ESSL, WGSL).
//!
//! Provides:
//! - `GlslFamilyBase` struct (syntax + impl_factory) shared by all four variants
//! - `register_hw_impls()` — the identical 20+ impl registrations in one place
//! - `resolve_nodedef_impl()` — shared `get_implementation_for_nodedef` logic
//! - `def_glsl_graph_context!` — macro to define the XxxShaderGraphContext struct
//! - `impl_glsl_context_traits!` — macro to impl all context traits for a given generator

use std::sync::Arc;

use crate::core::{Document, element::category};
use crate::gen_hw::{
    HwBitangentNode, HwFrameNode, HwGeomColorNode, HwGeomPropValueNode,
    HwGeomPropValueNodeAsUniform, HwImageNode, HwLightNode, HwLightShaderNode, HwNormalNode,
    HwPositionNode, HwSurfaceNode, HwTangentNode, HwTexCoordNode, HwTimeNode,
    HwTransformNormalNode, HwTransformPointNode, HwTransformVectorNode, HwViewDirectionNode,
};
use crate::gen_shader::{
    CompoundNode, GenContext, ImplementationFactory, MaterialNode, ShaderGenerator, ShaderNodeImpl,
    TypeSystem,
};

use super::glsl_syntax::GlslSyntax;

/// Common field layout for all GLSL-family generators.
/// Each generator owns one of these directly (not behind a pointer).
pub struct GlslFamilyBase {
    pub syntax: GlslSyntax,
    pub impl_factory: ImplementationFactory,
}

impl GlslFamilyBase {
    pub fn new(syntax: GlslSyntax) -> Self {
        Self {
            syntax,
            impl_factory: ImplementationFactory::new(),
        }
    }

    /// Register all standard hw node implementations under `impl_target`.
    /// All four generators call this with their respective impl_target during construction.
    pub fn register_hw_impls(&mut self, impl_target: &str) {
        let t = impl_target;
        self.impl_factory.register(
            &format!("IM_surfacematerial_{}", t),
            Arc::new(|| MaterialNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_position_vector3_{}", t),
            Arc::new(|| HwPositionNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_normal_vector3_{}", t),
            Arc::new(|| HwNormalNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_tangent_vector3_{}", t),
            Arc::new(|| HwTangentNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_bitangent_vector3_{}", t),
            Arc::new(|| HwBitangentNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_viewdirection_vector3_{}", t),
            Arc::new(|| HwViewDirectionNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_transformpoint_vector3_{}", t),
            Arc::new(|| HwTransformPointNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_transformvector_vector3_{}", t),
            Arc::new(|| HwTransformVectorNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_transformnormal_vector3_{}", t),
            Arc::new(|| HwTransformNormalNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_surface_{}", t),
            Arc::new(|| HwSurfaceNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_light_{}", t),
            Arc::new(|| HwLightNode::create()),
        );
        for ty in ["float", "color3", "color4"] {
            self.impl_factory.register(
                &format!("IM_geomcolor_{}_{}", ty, t),
                Arc::new(|| HwGeomColorNode::create()),
            );
        }
        self.impl_factory.register_multi(
            [
                format!("IM_texcoord_vector2_{}", t),
                format!("IM_texcoord_vector3_{}", t),
            ],
            Arc::new(|| HwTexCoordNode::create()),
        );
        for ty in [
            "integer", "float", "color3", "color4", "vector2", "vector3", "vector4",
        ] {
            self.impl_factory.register(
                &format!("IM_geompropvalue_{}_{}", ty, t),
                Arc::new(|| HwGeomPropValueNode::create()),
            );
        }
        self.impl_factory.register(
            &format!("IM_geompropvalue_boolean_{}", t),
            Arc::new(|| HwGeomPropValueNodeAsUniform::create()),
        );
        self.impl_factory.register(
            &format!("IM_geompropvalue_string_{}", t),
            Arc::new(|| HwGeomPropValueNodeAsUniform::create()),
        );
        self.impl_factory.register(
            &format!("IM_geompropvalue_filename_{}", t),
            Arc::new(|| HwGeomPropValueNodeAsUniform::create()),
        );
        self.impl_factory.register(
            &format!("IM_frame_float_{}", t),
            Arc::new(|| HwFrameNode::create()),
        );
        self.impl_factory.register(
            &format!("IM_time_float_{}", t),
            Arc::new(|| HwTimeNode::create()),
        );

        // <!-- <point_light> -->
        self.impl_factory.register(
            &format!("IM_point_light_{}", t),
            Arc::new(|| HwLightShaderNode::create()),
        );
        // <!-- <directional_light> -->
        self.impl_factory.register(
            &format!("IM_directional_light_{}", t),
            Arc::new(|| HwLightShaderNode::create()),
        );
        // <!-- <spot_light> -->
        self.impl_factory.register(
            &format!("IM_spot_light_{}", t),
            Arc::new(|| HwLightShaderNode::create()),
        );

        // <!-- <image> -->
        for ty in ["float", "color3", "color4", "vector2", "vector3", "vector4"] {
            self.impl_factory.register(
                &format!("IM_image_{}_{}", ty, t),
                Arc::new(|| HwImageNode::create()),
            );
        }
    }
}

/// Shared `get_implementation_for_nodedef` body for all four GLSL-family graph contexts.
/// Called from each XxxShaderGraphContext's `ShaderGraphCreateContext` impl.
///
/// Mirrors C++ ShaderGenerator::getImplementation (Definition.cpp:60-120):
/// 1. Collects matching implementations (qualified + unqualified name)
/// 2. Resolves Implementation elements that have `nodegraph` attr to their NodeGraph
/// 3. Selects best match by target (target-specific first, then generic)
pub fn resolve_nodedef_impl<G>(
    ctx: &GenContext<G>,
    impl_ctx: &dyn crate::gen_shader::ShaderImplContext,
    doc: &Document,
    node_def_name: &str,
    target: &str,
) -> Option<Box<dyn ShaderNodeImpl>>
where
    G: ShaderGenerator + HasImplFactory,
{
    // Collect impls by qualified + unqualified name (C++ Definition.cpp:62-64)
    let mut impls = doc.get_matching_implementations(node_def_name);
    // Also try unqualified name for namespaced nodedefs
    if node_def_name.contains(':') {
        let unqualified = node_def_name.rsplit(':').next().unwrap_or(node_def_name);
        let secondary = doc.get_matching_implementations(unqualified);
        impls.extend(secondary);
    }

    // Resolve Implementation elements with `nodegraph` attr to actual NodeGraph
    // (C++ Definition.cpp:67-80: resolveNodeGraph=true by default)
    for i in 0..impls.len() {
        let cat = impls[i].borrow().get_category().to_string();
        if cat == category::IMPLEMENTATION {
            let ng_attr = impls[i]
                .borrow()
                .get_attribute("nodegraph")
                .map(|s| s.to_string());
            if let Some(ng_name) = ng_attr {
                if !ng_name.is_empty() {
                    if let Some(ng) = doc.get_node_graph(&ng_name) {
                        impls[i] = ng;
                    }
                }
            }
        }
    }

    // Target-specific match first (C++ Definition.cpp:94-105)
    let selected = if !target.is_empty() {
        // First: target-specific match
        impls
            .iter()
            .find(|e| {
                let t = e
                    .borrow()
                    .get_attribute("target")
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                !t.is_empty() && crate::core::element::target_strings_match(&t, target)
            })
            .cloned()
            // Then: generic match (no target attribute)
            .or_else(|| {
                impls
                    .iter()
                    .find(|e| {
                        e.borrow()
                            .get_attribute("target")
                            .map(|s| s.to_string())
                            .unwrap_or_default()
                            .is_empty()
                    })
                    .cloned()
            })
    } else {
        impls.into_iter().next()
    };

    let impl_elem = selected?;
    let impl_name = impl_elem.borrow().get_name().to_string();
    let cat = impl_elem.borrow().get_category().to_string();

    if cat == category::NODE_GRAPH {
        let mut compound = CompoundNode::create();
        compound.initialize(&impl_elem, impl_ctx);
        Some(compound)
    } else if cat == category::IMPLEMENTATION {
        let mut impl_ = ctx
            .get_shader_generator()
            .get_impl_factory()
            .create(&impl_name)
            .or_else(|| Some(crate::gen_shader::SourceCodeNode::create()))?;
        impl_.initialize(&impl_elem, impl_ctx);
        Some(impl_)
    } else {
        None
    }
}

/// Marker trait: all GLSL-family generators expose their `ImplementationFactory`.
/// Needed by `resolve_nodedef_impl` to stay generic.
pub trait HasImplFactory {
    fn get_impl_factory(&self) -> &ImplementationFactory;
    fn get_syntax_base(&self) -> &GlslSyntax;
    fn get_type_system_ref(&self) -> &TypeSystem;
}

// ─── Context struct + constructor macro ─────────────────────────────────────

/// Define `pub struct XxxShaderGraphContext` with `new`, `with_graph`, and `with_graph_and_doc`
/// constructors.  The `doc` field enables `emit_node_function_call` inside closure scopes
/// (C++ `shadergen.emitFunctionCall(*node, context, stage)`).
/// Usage: `def_glsl_graph_context!(MyContext, MyGenerator);`
#[macro_export]
macro_rules! def_glsl_graph_context {
    ($ctx:ident, $gen:ty) => {
        pub struct $ctx<'a> {
            pub ctx: &'a $crate::gen_shader::GenContext<$gen>,
            graph: Option<&'a $crate::gen_shader::ShaderGraph>,
            /// Document reference for impl resolution in emit_node_function_call.
            doc: Option<&'a $crate::core::Document>,
        }

        impl $ctx<'_> {
            pub fn new(ctx: &$crate::gen_shader::GenContext<$gen>) -> $ctx<'_> {
                $ctx {
                    ctx,
                    graph: None,
                    doc: None,
                }
            }
            pub fn with_graph<'b>(
                ctx: &'b $crate::gen_shader::GenContext<$gen>,
                graph: &'b $crate::gen_shader::ShaderGraph,
            ) -> $ctx<'b> {
                $ctx {
                    ctx,
                    graph: Some(graph),
                    doc: None,
                }
            }
            /// Preferred constructor for emit phase: carries doc so that
            /// `emit_node_function_call` can resolve BSDF/EDF impls inside closure scopes.
            pub fn with_graph_and_doc<'b>(
                ctx: &'b $crate::gen_shader::GenContext<$gen>,
                graph: &'b $crate::gen_shader::ShaderGraph,
                doc: &'b $crate::core::Document,
            ) -> $ctx<'b> {
                $ctx {
                    ctx,
                    graph: Some(graph),
                    doc: Some(doc),
                }
            }
        }
    };
}

// ─── ShaderImplContext macro ─────────────────────────────────────────────────

/// Implement `ShaderImplContext` for a GLSL-family context struct.
#[macro_export]
macro_rules! impl_glsl_impl_context {
    ($ctx:ident, $gen:ty) => {
        impl $crate::gen_shader::ShaderImplContext for $ctx<'_> {
            fn resolve_source_file(
                &self,
                filename: &str,
                local_path: Option<&$crate::format::FilePath>,
            ) -> Option<$crate::format::FilePath> {
                self.ctx.resolve_source_file(filename, local_path)
            }
            fn get_graph(&self) -> Option<&$crate::gen_shader::ShaderGraph> {
                self.graph
            }
            fn get_type_system(&self) -> &$crate::gen_shader::TypeSystem {
                self.ctx.get_type_system()
            }
            fn get_default_value(&self, type_name: &str, as_uniform: bool) -> String {
                let td = self.ctx.get_type_desc(type_name);
                self.ctx
                    .get_shader_generator()
                    .get_syntax()
                    .get_syntax()
                    .get_default_value(&td, as_uniform)
            }
            fn make_valid_name(&self, name: &mut String) {
                self.ctx
                    .get_shader_generator()
                    .get_syntax()
                    .get_syntax()
                    .make_valid_name(name);
            }
            fn get_constant_qualifier(&self) -> &str {
                self.ctx
                    .get_shader_generator()
                    .get_syntax()
                    .get_syntax()
                    .get_constant_qualifier()
            }
            fn get_closure_data_argument(
                &self,
                node: &$crate::gen_shader::ShaderNode,
            ) -> Option<String> {
                if <$gen as $crate::gen_shader::ShaderGenerator>::node_needs_closure_data(
                    self.ctx.get_shader_generator(),
                    node,
                ) {
                    Some($crate::gen_hw::hw_lighting::CLOSURE_DATA_ARG.to_string())
                } else {
                    None
                }
            }
            fn get_gen_options(&self) -> &$crate::gen_shader::GenOptions {
                self.ctx.get_options()
            }
            fn as_graph_create_context(
                &self,
            ) -> Option<&dyn $crate::gen_shader::ShaderGraphCreateContext> {
                Some(self)
            }
            /// Re-emit a node's function call: resolves impl via doc+ShaderGraphCreateContext,
            /// then calls emit_function_call. C++ shadergen.emitFunctionCall(*node, ctx, stage).
            fn emit_node_function_call(
                &self,
                node_name: &str,
                stage: &mut $crate::gen_shader::ShaderStage,
            ) {
                let graph = match self.graph {
                    Some(g) => g,
                    None => return,
                };
                let doc = match self.doc {
                    Some(d) => d,
                    None => return,
                };
                let node = match graph.get_node(node_name) {
                    Some(n) => n,
                    None => return,
                };
                let node_def_name = match graph.get_node_def(node_name) {
                    Some(nd) => nd,
                    None => return,
                };
                // get_implementation_target/get_implementation_for_nodedef are on
                // ShaderGraphCreateContext, not ShaderImplContext -- go via as_graph_create_context.
                let gc = match self.as_graph_create_context() {
                    Some(g) => g,
                    None => return,
                };
                let target = gc.get_implementation_target();
                let impl_opt = gc.get_implementation_for_nodedef(doc, node_def_name, target);
                let impl_ = match impl_opt {
                    Some(i) => i,
                    None => return,
                };
                impl_.emit_function_call(node, self, stage);
            }
        }
    };
}

// ─── ShaderGraphCreateContext macro ──────────────────────────────────────────

/// Implement `ShaderGraphCreateContext` for a GLSL-family context.
/// `$target` and `$impl_target` are string literals or const refs.
#[macro_export]
macro_rules! impl_glsl_graph_ctx {
    ($ctx:ident, $gen:ty, $target:expr, $impl_target:expr) => {
        impl $crate::gen_shader::ShaderGraphCreateContext for $ctx<'_> {
            fn get_syntax(&self) -> &$crate::gen_shader::Syntax {
                self.ctx.get_shader_generator().get_syntax().get_syntax()
            }
            fn get_options(&self) -> &$crate::gen_shader::GenOptions {
                self.ctx.get_options()
            }
            fn get_target(&self) -> &str {
                $target
            }
            fn get_implementation_target(&self) -> &str {
                $impl_target
            }
            fn get_color_management_system(
                &self,
            ) -> Option<&dyn $crate::gen_shader::ColorManagementSystem> {
                self.ctx.get_color_management_system()
            }
            fn get_unit_system(&self) -> Option<&dyn $crate::gen_shader::UnitSystem> {
                self.ctx.get_unit_system()
            }
            fn get_implementation_for_nodedef(
                &self,
                doc: &$crate::core::Document,
                node_def_name: &str,
                target: &str,
            ) -> Option<Box<dyn $crate::gen_shader::ShaderNodeImpl>> {
                $crate::gen_glsl::glsl_family::resolve_nodedef_impl(
                    self.ctx,
                    self,
                    doc,
                    node_def_name,
                    target,
                )
            }
            fn get_light_data_type_var_string(&self) -> &str {
                use $crate::gen_hw::HwShaderGenerator;
                self.ctx
                    .get_shader_generator()
                    .get_light_data_type_var_string()
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_glsl::glsl_syntax::GlslSyntax;
    use crate::gen_shader::TypeSystem;

    fn make_base(target: &str) -> GlslFamilyBase {
        let syntax = GlslSyntax::create(TypeSystem::new());
        let mut base = GlslFamilyBase::new(syntax);
        base.register_hw_impls(target);
        base
    }

    // -- GlslFamilyBase construction --

    #[test]
    fn glsl_family_base_creates_with_syntax() {
        let base = make_base("genglsl");
        // Syntax should be usable (has type system)
        let td = base.syntax.get_syntax().get_type("float");
        assert_eq!(base.syntax.get_syntax().get_type_name(&td), Some("float"));
    }

    // -- HW impl registration --

    #[test]
    fn glsl_family_registers_surfacematerial() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_surfacematerial_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_position() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_position_vector3_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_normal() {
        let base = make_base("genglsl");
        assert!(base.impl_factory.is_registered("IM_normal_vector3_genglsl"));
    }

    #[test]
    fn glsl_family_registers_tangent_bitangent() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_tangent_vector3_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_bitangent_vector3_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_viewdirection() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_viewdirection_vector3_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_transforms() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_transformpoint_vector3_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_transformvector_vector3_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_transformnormal_vector3_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_surface_and_light() {
        let base = make_base("genglsl");
        assert!(base.impl_factory.is_registered("IM_surface_genglsl"));
        assert!(base.impl_factory.is_registered("IM_light_genglsl"));
    }

    #[test]
    fn glsl_family_registers_geomcolor_variants() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_geomcolor_float_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_geomcolor_color3_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_geomcolor_color4_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_texcoord_variants() {
        let base = make_base("genglsl");
        assert!(
            base.impl_factory
                .is_registered("IM_texcoord_vector2_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_texcoord_vector3_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_geompropvalue_variants() {
        let base = make_base("genglsl");
        for ty in [
            "integer", "float", "color3", "color4", "vector2", "vector3", "vector4",
        ] {
            assert!(
                base.impl_factory
                    .is_registered(&format!("IM_geompropvalue_{}_genglsl", ty)),
                "should register geompropvalue_{}",
                ty,
            );
        }
        // boolean/string/filename use AsUniform variant
        assert!(
            base.impl_factory
                .is_registered("IM_geompropvalue_boolean_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_geompropvalue_string_genglsl")
        );
        assert!(
            base.impl_factory
                .is_registered("IM_geompropvalue_filename_genglsl")
        );
    }

    #[test]
    fn glsl_family_registers_frame_time() {
        let base = make_base("genglsl");
        assert!(base.impl_factory.is_registered("IM_frame_float_genglsl"));
        assert!(base.impl_factory.is_registered("IM_time_float_genglsl"));
    }

    #[test]
    fn glsl_family_can_create_impls() {
        let base = make_base("genglsl");
        // Should be able to create each registered impl
        let surface = base.impl_factory.create("IM_surface_genglsl");
        assert!(surface.is_some(), "should create surface impl");
        let light = base.impl_factory.create("IM_light_genglsl");
        assert!(light.is_some(), "should create light impl");
        let mat = base.impl_factory.create("IM_surfacematerial_genglsl");
        assert!(mat.is_some(), "should create material impl");
    }

    // -- Different targets --

    #[test]
    fn glsl_family_registers_for_different_target() {
        let base = make_base("essl");
        // ESSL still uses genglsl impls (essl inherits genglsl), but register_hw_impls
        // with a different target would register under that target
        assert!(base.impl_factory.is_registered("IM_surfacematerial_essl"));
        assert!(base.impl_factory.is_registered("IM_surface_essl"));
        assert!(base.impl_factory.is_registered("IM_light_essl"));
        // genglsl ones should NOT be registered
        assert!(!base.impl_factory.is_registered("IM_surface_genglsl"));
    }
}
