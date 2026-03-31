//! GenContext — context for shader generation.

use crate::core::Document;
use crate::format::{FilePath, FileSearchPath};

use super::color_management::{ColorManagementSystem, DefaultColorManagementSystem};
use super::gen_user_data::GenUserData;
use super::shader_node::{ShaderInput, ShaderNode, ShaderOutput, ShaderPort};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::TypeSystem;
use super::gen_options::GenOptions;
use super::resource_binding_context::ResourceBindingContext;
use super::shader_metadata_registry::ShaderMetadataRegistry;
use super::type_desc::TypeDesc;
use super::unit_system::{DefaultUnitSystem, UnitSystem};

/// Context interface for ShaderNodeImpl — provides type system and file resolution.
pub trait ShaderImplContext {
    fn resolve_source_file(
        &self,
        filename: &str,
        local_path: Option<&FilePath>,
    ) -> Option<FilePath>;
    fn get_type_system(&self) -> &TypeSystem;
    /// Optional graph for resolving upstream connections during emit.
    fn get_graph(&self) -> Option<&super::shader_graph::ShaderGraph> {
        None
    }
    /// Access generation options (C++ GenContext::getOptions).
    /// Default returns GenOptions::default() for impls that don't carry options.
    fn get_gen_options(&self) -> &GenOptions {
        // Leak a static default so we can return a reference.
        // This is fine because GenOptions is small and this only allocates once.
        use std::sync::OnceLock;
        static DEFAULT: OnceLock<GenOptions> = OnceLock::new();
        DEFAULT.get_or_init(GenOptions::default)
    }
    /// Access bound light shaders for HW light dispatch (C++ getUserData<HwLightShaders>).
    /// Default: None (no light shaders bound).
    fn get_light_shaders(&self) -> Option<&crate::gen_hw::HwLightShaders> {
        None
    }
    /// Format filename/Texture arg for function call. WGSL uses "var_texture, var_sampler" split.
    fn format_filename_arg(&self, var: &str) -> String {
        var.to_string()
    }
    /// Type name and default value for emit_output_variables. None = use GLSL-style.
    fn get_type_name_for_emit(&self, type_name: &str) -> Option<(&'static str, &'static str)> {
        let _ = type_name;
        None
    }
    /// Type name for the active target syntax.
    fn get_type_name(&self, type_name: &str) -> Option<String> {
        if let Some(context) = self.as_graph_create_context() {
            let type_desc = context.get_type_desc(type_name);
            return context
                .get_syntax()
                .get_type_name(&type_desc)
                .map(str::to_string);
        }
        self.get_type_name_for_emit(type_name)
            .map(|(name, _)| name.to_string())
    }
    /// Reserved words for the target syntax, if the context carries syntax information.
    fn get_reserved_words(&self) -> Option<&HashSet<String>> {
        None
    }
    /// Make an identifier valid for the active target syntax.
    fn make_valid_name(&self, _name: &mut String) {}
    /// Constant qualifier for inline temporary declarations.
    fn get_constant_qualifier(&self) -> &str {
        ""
    }
    /// Closure-data argument for HW source-code nodes, if required by the target.
    fn get_closure_data_argument(&self, _node: &ShaderNode) -> Option<String> {
        None
    }
    /// Closure-data parameter for function signatures, if required by the target.
    fn get_closure_data_parameter(&self, node: &ShaderNode) -> Option<String> {
        self.get_closure_data_argument(node)
            .map(|arg| format!("{} {}", crate::gen_hw::hw_lighting::CLOSURE_DATA_TYPE, arg))
    }
    /// Access the graph-create context when the implementation context also carries it.
    fn as_graph_create_context(&self) -> Option<&dyn super::ShaderGraphCreateContext> {
        None
    }
    /// Extra substitution tokens for inline sourcecode (e.g. {{MDL_VERSION_SUFFIX}} → "1_10").
    fn get_substitution_tokens(&self) -> Vec<(String, String)> {
        vec![]
    }
    /// Token substitutions for `#include` filename resolution (C++ getTokenSubstitutions).
    /// Maps `$fileTransformUv` -> `mx_transform_uv.glsl` etc.
    fn get_include_token_substitutions(&self) -> Vec<(String, String)> {
        let file_uv = if self.get_file_texture_vertical_flip() {
            "mx_transform_uv_vflip.glsl"
        } else {
            "mx_transform_uv.glsl"
        };
        vec![("$fileTransformUv".to_string(), file_uv.to_string())]
    }
    /// Whether file textures should flip V coordinate. Default: false.
    fn get_file_texture_vertical_flip(&self) -> bool {
        false
    }
    /// MDL version suffix for source code markers (e.g. "1_10"). Default: "1_10".
    fn get_mdl_version_suffix(&self) -> &str {
        "1_10"
    }
    /// Re-emit a node's function call within the current scope.
    /// Used by HwSurfaceNode to invoke BSDF/EDF nodes inside closure scopes.
    /// C++ equivalent: shadergen.emitFunctionCall(*node, context, stage).
    /// Default is a no-op; GLSL-family contexts override via doc + ShaderGraphCreateContext.
    fn emit_node_function_call(&self, _node_name: &str, _stage: &mut super::shader::ShaderStage) {}

    /// Default value for emit. Override for target-specific (e.g. MDL material()).
    /// Uses GLSL-compatible vec2/vec3/vec4 constructors as default (most targets).
    fn get_default_value(&self, type_name: &str, as_uniform: bool) -> String {
        let _ = as_uniform;
        match type_name {
            "float" => "0.0".to_string(),
            "int" | "integer" | "boolean" => "0".to_string(),
            "float2" | "vector2" => "vec2(0.0)".to_string(),
            "float3" | "vector3" | "color3" => "vec3(0.0)".to_string(),
            "float4" | "vector4" | "color4" => "vec4(0.0)".to_string(),
            "BSDF" | "VDF" => "BSDF(vec3(0.0),vec3(1.0))".to_string(),
            "EDF" => "EDF(0.0)".to_string(),
            "surfaceshader" | "material" => "surfaceshader(vec3(0.0),vec3(0.0))".to_string(),
            "volumeshader" => "volumeshader(vec3(0.0),vec3(0.0))".to_string(),
            "displacementshader" => "displacementshader(vec3(0.0),1.0)".to_string(),
            "lightshader" => "lightshader(vec3(0.0),vec3(0.0))".to_string(),
            _ => "0.0".to_string(),
        }
    }
}

/// Shader generator interface (trait for GLSL, MSL, etc.)
pub trait ShaderGenerator {
    fn get_type_system(&self) -> &TypeSystem;
    /// The target identifier for this generator (e.g. "genglsl", "genmsl", "genosl").
    /// Used by CMS/UnitSystem to look up correct color transforms.
    fn target(&self) -> &str;

    /// Emit a typed value to a stage (C++ ShaderGenerator::emitValue<T>).
    /// Appends the Display representation to the stage source code.
    fn emit_value<T: std::fmt::Display>(&self, value: &T, stage: &mut super::shader::ShaderStage) {
        stage.add_value(value);
    }

    /// Create a ShaderNodeImpl for a NodeGraph element (C++ createShaderNodeImplForNodeGraph).
    /// Returns a CompoundNode wrapped in Box<dyn ShaderNodeImpl>.
    fn create_impl_for_node_graph(&self) -> Box<dyn super::shader_node_impl::ShaderNodeImpl> {
        super::compound_node::CompoundNode::create()
    }

    /// Create a ShaderNodeImpl for an Implementation element (C++ createShaderNodeImplForImplementation).
    /// Returns a SourceCodeNode wrapped in Box<dyn ShaderNodeImpl>.
    fn create_impl_for_implementation(&self) -> Box<dyn super::shader_node_impl::ShaderNodeImpl> {
        super::source_code_node::SourceCodeNode::create()
    }

    /// Register TypeDef struct definitions from a document into the type system.
    /// Matches C++ ShaderGenerator::registerTypeDefs.
    /// Note: in the Rust design the type system is not mutably accessible via the immutable
    /// generator reference, so this is a convenience wrapper on TypeSystem directly.
    /// Call doc.register_type_defs_into(type_system_mut) for the mutable path.
    fn register_type_defs(&self, _doc: &Document) {
        // Default no-op: concrete generators own their TypeSystem and must override,
        // or callers use TypeSystem::register_type_defs_from_document directly.
    }

    /// Return true if the given node requires additional ClosureData passed through
    /// function arguments.  HwShaderGenerator overrides this to return true for
    /// BSDF/EDF closure nodes; the base implementation always returns false.
    /// Mirrors C++ `ShaderGenerator::nodeNeedsClosureData`.
    fn node_needs_closure_data(&self, _node: &ShaderNode) -> bool {
        false
    }

    /// Emit the ClosureData argument in a function *call* for a node that needs it.
    /// Default is a no-op; HwShaderGenerator overrides to append the closure data
    /// variable to the argument list.
    /// Mirrors C++ `ShaderGenerator::emitClosureDataArg`.
    fn emit_closure_data_arg(&self, _node: &ShaderNode, _stage: &mut super::shader::ShaderStage) {}

    /// Emit the ClosureData *parameter* in a function *definition* for a node that needs it.
    /// Default is a no-op; HwShaderGenerator overrides to append the closure data
    /// parameter declaration to the parameter list.
    /// Mirrors C++ `ShaderGenerator::emitClosureDataParameter`.
    fn emit_closure_data_parameter(
        &self,
        _node: &ShaderNode,
        _stage: &mut super::shader::ShaderStage,
    ) {
    }

    /// Generate a shader from the given element name and graph.
    ///
    /// C++ flow (ShaderGenerator::generate):
    ///   1. Create a Shader with a name derived from the element.
    ///   2. Create stage(s): HW generators add VERTEX + PIXEL; single-stage (OSL) adds one stage.
    ///   3. Call `createVariables` for every node in the graph (uniforms, inputs, outputs).
    ///   4. Call `emitFunctionDefinitions` for each node — forward-declares helper functions.
    ///   5. Call `emitFunctionCalls` for each node — body of the main entry point.
    ///   6. Apply token substitutions and return the completed Shader.
    ///
    /// Returns `None` if generation is not supported by this generator.
    /// Concrete generators (GlslGenerator, OslGenerator, …) override this.
    fn generate(
        &self,
        name: &str,
        graph: super::shader_graph::ShaderGraph,
        _stage_names: &[&str],
    ) -> Option<super::shader::Shader> {
        // Default: create a Shader with the requested stages but emit no code.
        // Concrete generator implementations (GlslGenerator, OslGenerator, etc.) override
        // this with full createVariables / emitFunctionDefinitions / emitFunctionCalls logic.
        // The base implementation only creates the stage scaffolding; actual code emission
        // is target-specific and handled by the overriding generator.
        let mut shader = super::shader::Shader::new(name, graph);
        for &stage_name in _stage_names {
            shader.create_stage(stage_name);
        }
        Some(shader)
    }
}

/// GenContext — thread-local/instance context for generation
pub struct GenContext<G: ShaderGenerator> {
    pub generator: G,
    pub options: GenOptions,
    pub source_code_search_path: FileSearchPath,
    pub color_management_system: Option<Box<dyn ColorManagementSystem>>,
    pub unit_system: Option<Box<dyn UnitSystem>>,
    /// Resource binding context for layout(binding=N) emission (Rc for shared mutable access during emit)
    pub resource_binding_context: Option<Rc<RefCell<Box<dyn ResourceBindingContext>>>>,
    /// Shader metadata registry for OSL/MDL etc. (по рефу GenContext user data)
    pub shader_metadata_registry: Option<ShaderMetadataRegistry>,
    /// Cache of resolved nodedef -> implementation name mappings (по рефу GenContext _nodeImplMap).
    node_impl_cache: HashMap<String, String>,
    /// Reserved words that should not be used as identifiers during codegen.
    reserved_words: HashSet<String>,
    /// Stack of parent nodes for graph traversal.
    parent_nodes: Vec<String>,
    /// Input suffix map: input ptr identity -> suffix string.
    input_suffix: HashMap<*const ShaderInput, String>,
    /// Output suffix map: output ptr identity -> suffix string.
    output_suffix: HashMap<*const ShaderOutput, String>,
    /// Handler for application variables.
    application_variable_handler: Option<Box<dyn Fn(&ShaderNode, &mut Self)>>,
    /// Named stacks of user-supplied data (C++ GenContext::_userData).
    user_data: HashMap<String, Vec<Box<dyn GenUserData>>>,
}

impl<G: ShaderGenerator> GenContext<G> {
    pub fn new(generator: G) -> Self {
        Self {
            generator,
            options: GenOptions::default(),
            source_code_search_path: FileSearchPath::new(),
            color_management_system: None,
            unit_system: None,
            resource_binding_context: None,
            shader_metadata_registry: None,
            node_impl_cache: HashMap::new(),
            reserved_words: HashSet::new(),
            parent_nodes: Vec::new(),
            input_suffix: HashMap::new(),
            output_suffix: HashMap::new(),
            application_variable_handler: None,
            user_data: HashMap::new(),
        }
    }

    /// Set resource binding context for Vulkan/GL layout bindings.
    pub fn set_resource_binding_context(&mut self, ctx: Box<dyn ResourceBindingContext>) {
        self.resource_binding_context = Some(Rc::new(RefCell::new(ctx)));
    }

    /// Get reference to resource binding context (for initialize/emit via borrow_mut).
    pub fn get_resource_binding_context(
        &self,
    ) -> Option<Rc<RefCell<Box<dyn ResourceBindingContext>>>> {
        self.resource_binding_context.clone()
    }

    pub fn get_shader_generator(&self) -> &G {
        &self.generator
    }

    pub fn get_options(&self) -> &GenOptions {
        &self.options
    }

    pub fn get_options_mut(&mut self) -> &mut GenOptions {
        &mut self.options
    }

    pub fn get_type_desc(&self, name: &str) -> TypeDesc {
        self.generator.get_type_system().get_type(name)
    }

    pub fn register_source_code_search_path(&mut self, path: impl Into<FilePath>) {
        self.source_code_search_path.append(path);
    }

    pub fn resolve_source_file(
        &self,
        filename: impl AsRef<str>,
        local_path: Option<&FilePath>,
    ) -> Option<FilePath> {
        resolve_source_file_impl(self, filename.as_ref(), local_path)
    }

    /// Set color management system (по рефу ShaderGenerator::setColorManagementSystem).
    pub fn set_color_management_system(&mut self, cms: Box<dyn ColorManagementSystem>) {
        self.color_management_system = Some(cms);
    }

    /// Get color management system.
    pub fn get_color_management_system(&self) -> Option<&dyn ColorManagementSystem> {
        self.color_management_system.as_deref()
    }

    /// Load document into CMS if present.
    pub fn load_cms_library(&mut self, doc: Document) {
        if let Some(cms) = self.color_management_system.as_deref_mut() {
            cms.load_library(doc);
        }
    }

    /// Set unit system (по рефу ShaderGenerator::setUnitSystem).
    pub fn set_unit_system(&mut self, unit_sys: Box<dyn UnitSystem>) {
        self.unit_system = Some(unit_sys);
    }

    /// Get unit system.
    pub fn get_unit_system(&self) -> Option<&dyn UnitSystem> {
        self.unit_system.as_deref()
    }

    /// Load document into UnitSystem if present.
    pub fn load_unit_library(&mut self, doc: Document) {
        if let Some(us) = self.unit_system.as_deref_mut() {
            us.load_library(doc);
        }
    }

    /// Configure default CMS and UnitSystem from document (load libraries).
    pub fn load_libraries_from_document(&mut self, doc: &Document) {
        self.load_cms_library(doc.clone());
        self.load_unit_library(doc.clone());
    }

    // ----- Node implementation cache (H-GS8, по рефу GenContext _nodeImplMap) -----

    /// Get cached implementation name for a nodedef, if previously resolved.
    pub fn get_cached_impl(&self, nodedef_name: &str) -> Option<&str> {
        self.node_impl_cache.get(nodedef_name).map(|s| s.as_str())
    }

    /// Store a resolved nodedef -> implementation name mapping in the cache.
    pub fn cache_impl(&mut self, nodedef_name: &str, impl_name: &str) {
        self.node_impl_cache
            .insert(nodedef_name.to_string(), impl_name.to_string());
    }

    /// Clear the node implementation cache.
    /// MDL generator calls this before createShader because it edits subgraphs per context.
    pub fn clear_impl_cache(&mut self) {
        self.node_impl_cache.clear();
    }

    /// Clear cached node implementations (по рефу GenContext::clearNodeImplementations).
    /// Delegates to clear_impl_cache for full compatibility.
    pub fn clear_node_implementations(&mut self) {
        self.clear_impl_cache();
    }

    // ----- Reserved words (C++ GenContext::addReservedWords / getReservedWords) -----

    /// Add reserved words that should not be used as identifiers during codegen.
    pub fn add_reserved_words(&mut self, names: &HashSet<String>) {
        self.reserved_words.extend(names.iter().cloned());
    }

    /// Return the set of reserved words.
    pub fn get_reserved_words(&self) -> &HashSet<String> {
        &self.reserved_words
    }

    // ----- Parent node stack (C++ GenContext::pushParentNode / popParentNode / getParentNodes) -----

    /// Push a parent node name onto the stack.
    pub fn push_parent_node(&mut self, node_name: impl Into<String>) {
        self.parent_nodes.push(node_name.into());
    }

    /// Pop the current parent node from the stack.
    pub fn pop_parent_node(&mut self) {
        self.parent_nodes.pop();
    }

    /// Return the current stack of parent node names.
    pub fn get_parent_nodes(&self) -> &[String] {
        &self.parent_nodes
    }

    // ----- User data (C++ GenContext::pushUserData / popUserData / clearUserData / getUserData) -----

    /// Push user data under the given name. Multiple values may be stacked per name.
    pub fn push_user_data(&mut self, name: impl Into<String>, data: Box<dyn GenUserData>) {
        self.user_data.entry(name.into()).or_default().push(data);
    }

    /// Pop the most recently pushed user data for the given name.
    pub fn pop_user_data(&mut self, name: &str) {
        if let Some(stack) = self.user_data.get_mut(name) {
            stack.pop();
        }
    }

    /// Clear all user data from the context.
    pub fn clear_user_data(&mut self) {
        self.user_data.clear();
    }

    /// Return the top user data for the given name, downcast to `T`.
    /// Returns `None` if the name is absent, the stack is empty, or the type doesn't match.
    pub fn get_user_data<T: GenUserData + 'static>(&self, name: &str) -> Option<&T> {
        self.user_data
            .get(name)
            .and_then(|stack| stack.last())
            .and_then(|boxed| boxed.as_any().downcast_ref::<T>())
    }

    /// Return the top user data for the given name as a mutable reference, downcast to `T`.
    pub fn get_user_data_mut<T: GenUserData + 'static>(&mut self, name: &str) -> Option<&mut T> {
        self.user_data
            .get_mut(name)
            .and_then(|stack| stack.last_mut())
            .and_then(|boxed| boxed.as_any_mut().downcast_mut::<T>())
    }

    // ----- Input suffix (C++ GenContext::addInputSuffix / removeInputSuffix / getInputSuffix) -----

    /// Add an input suffix to be used for the input in this context.
    pub fn add_input_suffix(&mut self, input: &ShaderInput, suffix: impl Into<String>) {
        self.input_suffix
            .insert(input as *const ShaderInput, suffix.into());
    }

    /// Remove an input suffix.
    pub fn remove_input_suffix(&mut self, input: &ShaderInput) {
        self.input_suffix.remove(&(input as *const ShaderInput));
    }

    /// Get an input suffix. Returns empty string if not found.
    pub fn get_input_suffix(&self, input: &ShaderInput) -> &str {
        self.input_suffix
            .get(&(input as *const ShaderInput))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    // ----- Output suffix (C++ GenContext::addOutputSuffix / removeOutputSuffix / getOutputSuffix) -----

    /// Add an output suffix to be used for the output in this context.
    pub fn add_output_suffix(&mut self, output: &ShaderOutput, suffix: impl Into<String>) {
        self.output_suffix
            .insert(output as *const ShaderOutput, suffix.into());
    }

    /// Remove an output suffix.
    pub fn remove_output_suffix(&mut self, output: &ShaderOutput) {
        self.output_suffix.remove(&(output as *const ShaderOutput));
    }

    /// Get an output suffix. Returns empty string if not found.
    pub fn get_output_suffix(&self, output: &ShaderOutput) -> &str {
        self.output_suffix
            .get(&(output as *const ShaderOutput))
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    // ----- Application variable handler (C++ GenContext::setApplicationVariableHandler) -----

    /// Set handler for application variables.
    pub fn set_application_variable_handler(
        &mut self,
        handler: Box<dyn Fn(&ShaderNode, &mut Self)>,
    ) {
        self.application_variable_handler = Some(handler);
    }

    /// Get handler for application variables.
    pub fn get_application_variable_handler(&self) -> Option<&dyn Fn(&ShaderNode, &mut Self)> {
        self.application_variable_handler.as_deref()
    }

    /// Ensure default CMS and UnitSystem are set if not present.
    /// Uses the generator's actual target string, not hardcoded "genglsl".
    pub fn ensure_default_color_and_unit_systems(&mut self) {
        let tgt = self.generator.target().to_string();
        if self.color_management_system.is_none() {
            self.set_color_management_system(DefaultColorManagementSystem::create(&tgt));
        }
        if self.unit_system.is_none() {
            self.set_unit_system(DefaultUnitSystem::create(&tgt));
        }
        // Add default library root for source file resolution when path is empty.
        // Base is crate root; genglsl for Implementation files like mx_image_float.glsl.
        if self.source_code_search_path.is_empty() {
            let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let lib_subdir = crate_root.join("libraries");
            if lib_subdir.exists() {
                self.register_source_code_search_path(FilePath::new(crate_root));
                let genglsl = lib_subdir.join("stdlib/genglsl");
                if genglsl.exists() {
                    self.register_source_code_search_path(FilePath::new(genglsl));
                }
                // pbrlib GLSL implementations (BSDF/EDF/VDF nodes)
                let pbrlib_genglsl = lib_subdir.join("pbrlib/genglsl");
                if pbrlib_genglsl.exists() {
                    self.register_source_code_search_path(FilePath::new(pbrlib_genglsl));
                }
                let genmsl = lib_subdir.join("stdlib/genmsl");
                if genmsl.exists() {
                    self.register_source_code_search_path(FilePath::new(genmsl));
                }
                let genosl = lib_subdir.join("stdlib/genosl");
                if genosl.exists() {
                    self.register_source_code_search_path(FilePath::new(&genosl));
                    let genosl_include = genosl.join("include");
                    if genosl_include.exists() {
                        self.register_source_code_search_path(FilePath::new(genosl_include));
                    }
                }
                let genslang = lib_subdir.join("stdlib/genslang");
                if genslang.exists() {
                    self.register_source_code_search_path(FilePath::new(genslang));
                }
            }
        }
    }
}

fn resolve_source_file_impl<G: ShaderGenerator>(
    ctx: &GenContext<G>,
    filename: &str,
    local_path: Option<&FilePath>,
) -> Option<FilePath> {
    let mut sp = ctx.source_code_search_path.clone();
    if let Some(local) = local_path {
        if !local.as_str().is_empty() {
            sp.prepend(local.clone());
        }
    }
    sp.find(filename)
}

impl<G: ShaderGenerator> ShaderImplContext for GenContext<G> {
    fn resolve_source_file(
        &self,
        filename: &str,
        local_path: Option<&FilePath>,
    ) -> Option<FilePath> {
        resolve_source_file_impl(self, filename, local_path)
    }

    fn get_type_system(&self) -> &TypeSystem {
        self.generator.get_type_system()
    }
}

// ---------------------------------------------------------------------------
// ScopedSetVariableName -- RAII for temporarily overriding a ShaderPort variable name.
// Mirrors C++ ScopedSetVariableName (GenContext.h).
// ---------------------------------------------------------------------------

/// RAII guard that sets `port.variable` to a new name and restores the old one on drop.
pub struct ScopedSetVariableName<'a> {
    pub port: &'a mut ShaderPort,
    old_name: String,
}

impl<'a> ScopedSetVariableName<'a> {
    /// Set `port.variable` to `name`, saving the old value for restoration.
    pub fn new(name: impl Into<String>, port: &'a mut ShaderPort) -> Self {
        let old_name = port.variable.clone();
        port.variable = name.into();
        Self { port, old_name }
    }
}

impl Drop for ScopedSetVariableName<'_> {
    fn drop(&mut self) {
        self.port.variable = self.old_name.clone();
    }
}

// ---------------------------------------------------------------------------
// Tests for user data and ScopedSetVariableName
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::gen_user_data::GenUserData;
    use std::any::Any;

    // Minimal concrete generator for testing
    struct DummyGen;
    impl ShaderGenerator for DummyGen {
        fn get_type_system(&self) -> &TypeSystem {
            unimplemented!("test only")
        }
        fn target(&self) -> &str {
            "test"
        }
    }

    // Concrete user data type
    #[derive(Debug, PartialEq)]
    struct MyData {
        pub x: i32,
    }
    impl GenUserData for MyData {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[test]
    fn user_data_push_get_pop() {
        let mut ctx: GenContext<DummyGen> = GenContext::new(DummyGen);

        ctx.push_user_data("key", Box::new(MyData { x: 42 }));
        assert_eq!(ctx.get_user_data::<MyData>("key").map(|d| d.x), Some(42));

        ctx.push_user_data("key", Box::new(MyData { x: 99 }));
        // Top of stack is 99
        assert_eq!(ctx.get_user_data::<MyData>("key").map(|d| d.x), Some(99));

        ctx.pop_user_data("key");
        // Back to 42
        assert_eq!(ctx.get_user_data::<MyData>("key").map(|d| d.x), Some(42));
    }

    #[test]
    fn user_data_missing_key_returns_none() {
        let ctx: GenContext<DummyGen> = GenContext::new(DummyGen);
        assert!(ctx.get_user_data::<MyData>("missing").is_none());
    }

    #[test]
    fn user_data_clear() {
        let mut ctx: GenContext<DummyGen> = GenContext::new(DummyGen);
        ctx.push_user_data("a", Box::new(MyData { x: 1 }));
        ctx.push_user_data("b", Box::new(MyData { x: 2 }));
        ctx.clear_user_data();
        assert!(ctx.get_user_data::<MyData>("a").is_none());
        assert!(ctx.get_user_data::<MyData>("b").is_none());
    }

    #[test]
    fn scoped_set_variable_name() {
        use crate::gen_shader::shader_node::ShaderPort;
        use crate::gen_shader::type_desc::TypeSystem;
        let ts = TypeSystem::new();
        let td = ts.get_type("float");
        let mut port = ShaderPort::new(td, "myPort");
        port.variable = "original".to_string();

        // Apply scoped override, drop guard, then verify restoration.
        // We cannot read port.variable while the guard holds &mut port.
        ScopedSetVariableName::new("temp_name", &mut port);
        // Guard is immediately dropped (not bound), so original is restored.
        assert_eq!(port.variable, "original");

        // Verify it actually changes while alive by doing work through the guard.
        {
            let guard = ScopedSetVariableName::new("live", &mut port);
            // Access the port only via the guard's internal reference.
            assert_eq!(guard.port.variable, "live");
        }
        assert_eq!(port.variable, "original");
    }

    // ----- Tests for ShaderGenerator closure data methods -----

    #[test]
    fn node_needs_closure_data_default_false() {
        use crate::gen_shader::ShaderNode;

        let generator = DummyGen;
        let node = ShaderNode::new("test");
        // Base trait always returns false.
        assert!(!generator.node_needs_closure_data(&node));
    }

    #[test]
    fn emit_closure_data_noop_does_not_panic() {
        use crate::gen_shader::{ShaderNode, ShaderStage};

        let generator = DummyGen;
        let node = ShaderNode::new("test");
        let mut stage = ShaderStage::new("pixel");

        // Both default no-ops must not panic.
        generator.emit_closure_data_arg(&node, &mut stage);
        generator.emit_closure_data_parameter(&node, &mut stage);

        // Stage code should be empty (no-ops added nothing).
        assert_eq!(stage.get_source_code(), "");
    }
}
