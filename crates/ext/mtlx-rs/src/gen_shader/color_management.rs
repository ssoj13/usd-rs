//! ColorManagementSystem -- color space transforms (ref: MaterialX ColorManagementSystem).

use crate::core::{Document, ElementPtr};

use super::ShaderGraphCreateContext;
use super::shader_node::ShaderNode;
use super::shader_node_impl::ShaderNodeImpl;
use super::type_desc::TypeDesc;

/// Color space transform: source space, target space, type.
/// Matches C++ ColorSpaceTransform.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorSpaceTransform {
    pub source_space: String,
    pub target_space: String,
    pub type_desc: TypeDesc,
}

impl ColorSpaceTransform {
    pub fn new(
        source_space: impl Into<String>,
        target_space: impl Into<String>,
        type_desc: TypeDesc,
    ) -> Self {
        Self {
            source_space: source_space.into(),
            target_space: target_space.into(),
            type_desc,
        }
    }
}

/// Color management system -- transforms between color spaces.
/// Matches C++ ColorManagementSystem base class.
pub trait ColorManagementSystem {
    /// Return the CMS name.
    fn get_name(&self) -> &str;

    /// Load implementations from document (replacing previous).
    fn load_library(&mut self, _document: Document) {}

    /// Return true if this CMS supports the given transform.
    fn supports_transform(&self, transform: &ColorSpaceTransform) -> bool;

    /// Create color transform node (ref: ColorManagementSystem::createNode).
    fn create_node(
        &self,
        transform: &ColorSpaceTransform,
        name: &str,
        doc: &Document,
        context: &dyn ShaderGraphCreateContext,
    ) -> Option<ShaderNode>;

    /// Return true if CMS can create a ShaderNodeImpl for a locally managed transform.
    fn has_implementation(&self, _impl_name: &str) -> bool {
        false
    }

    /// Create CMS node implementation for a locally managed transform.
    fn create_implementation(&self, _impl_name: &str) -> Option<Box<dyn ShaderNodeImpl>> {
        None
    }
}

// ---------------------------------------------------------------------------
// Legacy color space name remap (ref: DefaultColorManagementSystem.cpp)
// ---------------------------------------------------------------------------

/// Remap legacy color space names to their ACES 1.2 / ASWF ColorInterop equivalents.
fn remap_color_space(name: &str) -> &str {
    match name {
        // Legacy short names
        "gamma18" => "g18_rec709",
        "gamma22" => "g22_rec709",
        "gamma24" => "rec709_display",
        "lin_ap1" => "acescg",

        // ASWF recommended ColorInterop -> nanocolor spaces (MaterialX 1.39+)
        "lin_ap1_scene" => "acescg",
        "lin_rec709_scene" => "lin_rec709",
        "lin_p3d65_scene" => "lin_displayp3",
        "lin_adobergb_scene" => "lin_adobergb",
        "srgb_rec709_scene" => "srgb_texture",
        "g22_rec709_scene" => "g22_rec709",
        "g18_rec709_scene" => "g18_rec709",
        "g22_ap1_scene" => "g22_ap1",
        "srgb_p3d65_scene" => "srgb_displayp3",
        "g22_adobergb_scene" => "adobergb",

        other => other,
    }
}

/// Default CMS implementation. Matches C++ DefaultColorManagementSystem.
#[derive(Debug)]
pub struct DefaultColorManagementSystem {
    #[allow(dead_code)] // Shader target name -- used by emit code to filter color transforms
    target: String,
    document: Option<Document>,
}

const CMS_NAME: &str = "default_cms";

impl DefaultColorManagementSystem {
    pub fn new(target: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            document: None,
        }
    }

    pub fn create(target: impl Into<String>) -> Box<dyn ColorManagementSystem> {
        Box::new(Self::new(target))
    }

    /// Look up NodeDef for transform (ref: DefaultColorManagementSystem::getNodeDef).
    /// Node name pattern: `<source>_to_<target>`, then find matching nodedef with correct output type.
    fn get_node_def(&self, transform: &ColorSpaceTransform) -> Option<ElementPtr> {
        let doc = self.document.as_ref()?;

        // Remap legacy color space names
        let source = remap_color_space(&transform.source_space);
        let target = remap_color_space(&transform.target_space);
        let node_name = format!("{}_to_{}", source, target);
        let type_name = transform.type_desc.get_name();

        // Walk document nodedefs matching this node name + output type
        for child in doc.get_root().borrow().get_children() {
            let b = child.borrow();
            let cat = b.get_category();
            if cat != crate::core::element::category::NODEDEF {
                continue;
            }
            // Check if nodedef's "node" attribute matches our transform node name
            let nd_node = b.get_attribute("node").unwrap_or_default();
            if nd_node != node_name {
                // Fallback: check if nodedef name contains the node name (for ND_xxx_color3 patterns)
                let nd_name = b.get_name();
                if !(nd_name.contains(&node_name) && nd_name.ends_with(type_name)) {
                    continue;
                }
            }
            // Check output type matches
            for output in b.get_children() {
                let ob = output.borrow();
                if ob.get_category() != "output" {
                    continue;
                }
                let out_type = ob.get_attribute("type").unwrap_or_default();
                if out_type == type_name {
                    drop(ob);
                    drop(b);
                    return Some(child.clone());
                }
            }
        }
        None
    }
}

impl ColorManagementSystem for DefaultColorManagementSystem {
    fn get_name(&self) -> &str {
        CMS_NAME
    }

    fn load_library(&mut self, document: Document) {
        self.document = Some(document);
    }

    fn supports_transform(&self, transform: &ColorSpaceTransform) -> bool {
        self.get_node_def(transform).is_some()
    }

    fn create_node(
        &self,
        transform: &ColorSpaceTransform,
        name: &str,
        doc: &Document,
        context: &dyn ShaderGraphCreateContext,
    ) -> Option<ShaderNode> {
        let node_def = self.get_node_def(transform)?;
        super::shader_node_factory::create_node_from_nodedef(name, &node_def, doc, context).ok()
    }
}

// ---------------------------------------------------------------------------
// OCIO Color Management System
// ---------------------------------------------------------------------------

#[cfg(feature = "ocio")]
#[allow(dead_code)] // Public API -- items used by downstream crates, not internally
pub mod ocio_impl {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use vfx_ocio::{Config, GpuLanguage, GpuProcessor};

    use crate::core::{Document, ElementPtr, element::add_child_of_category};
    use crate::gen_shader::ShaderGraphCreateContext;
    use crate::gen_shader::gen_context::ShaderImplContext;
    use crate::gen_shader::shader::ShaderStage;
    use crate::gen_shader::shader_node::ShaderNode;
    use crate::gen_shader::shader_node_impl::ShaderNodeImpl;

    use super::{ColorManagementSystem, ColorSpaceTransform, DefaultColorManagementSystem};

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    /// Prefix common to all OCIO implementation names.
    pub const IMPL_PREFIX: &str = "IMPL_MXOCIO_";
    /// Prefix common to all OCIO node def names.
    #[allow(dead_code)]
    const ND_PREFIX: &str = "ND_MXOCIO_";
    /// Source URI attached to OCIO-generated NodeDefs / Implementations.
    pub const OCIO_SOURCE_URI: &str = "materialx://OcioColorManagementSystem.cpp";
    /// Display name for this CMS.
    const OCIO_CMS_NAME: &str = "OpenColorIO";
    /// Default function name emitted by vfx-ocio generate_shader().
    const OCIO_DEFAULT_FN: &str = "ocio_transform";

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// OCIO-specific legacy color space name remap to ACES 1.3 display names.
    fn ocio_remap(name: &str) -> Option<&'static str> {
        match name {
            "gamma18" => Some("Gamma 1.8 Rec.709 - Texture"),
            "gamma22" => Some("Gamma 2.2 Rec.709 - Texture"),
            "gamma24" => Some("Gamma 2.4 Rec.709 - Texture"),
            _ => None,
        }
    }

    /// Map shader target to GpuLanguage.
    fn target_to_gpu_lang(target: &str) -> GpuLanguage {
        match target {
            "genglsl" => GpuLanguage::Glsl400,
            "genmsl" => GpuLanguage::Metal,
            // vfx-ocio falls back to GLSL for OSL currently
            _ => GpuLanguage::Glsl400,
        }
    }

    /// Hash a string to u64.
    fn hash_str(s: &str) -> u64 {
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }

    /// Sanitize a string to a valid MaterialX identifier component.
    /// Replaces non-alphanumeric chars with '_' and prepends 'c' if starts with digit.
    fn sanitize_id(s: &str) -> String {
        let mut result: String = s
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if result.starts_with(|c: char| c.is_ascii_digit()) {
            result.insert(0, 'c');
        }
        result
    }

    /// Validate that the target is one supported by OCIO.
    fn validate_target(target: &str) -> Result<(), String> {
        match target {
            "genglsl" | "genmsl" | "genosl" => Ok(()),
            other => Err(format!(
                "OCIO does not support target '{other}' (expected genglsl/genmsl/genosl)"
            )),
        }
    }

    /// Derive OCIO function name from impl name.
    /// Strips IMPL_PREFIX and the trailing _color3 / _color4 type suffix.
    pub fn fn_name_from_impl(impl_name: &str) -> String {
        let s = impl_name.strip_prefix(IMPL_PREFIX).unwrap_or(impl_name);
        let s = s
            .strip_suffix("_color3")
            .or_else(|| s.strip_suffix("_color4"))
            .unwrap_or(s);
        s.to_string()
    }

    // -----------------------------------------------------------------------
    // Cache entry
    // -----------------------------------------------------------------------

    struct CacheEntry {
        gpu_processor: GpuProcessor,
    }

    // -----------------------------------------------------------------------
    // OcioColorManagementSystem
    // -----------------------------------------------------------------------

    /// OCIO-backed color management system.
    ///
    /// Delegates standard library transforms to `DefaultColorManagementSystem`,
    /// and dynamically creates OCIO-backed NodeDefs for transforms not found there.
    pub struct OcioColorManagementSystem {
        default_cms: DefaultColorManagementSystem,
        config: Config,
        target: String,
        /// Loaded library document for dynamic nodedef insertion.
        /// Uses RefCell so get_node_def can mutate even through &self.
        document: RefCell<Option<Document>>,
        /// impl_name -> cached GpuProcessor (filled by get_ocio_node_def).
        cache: RefCell<HashMap<String, CacheEntry>>,
    }

    impl OcioColorManagementSystem {
        // --- Factory methods ---

        /// Create from OCIO environment variable (OCIO=<path>).
        pub fn create_from_env(
            target: impl Into<String>,
        ) -> Result<Box<dyn ColorManagementSystem>, String> {
            let target = target.into();
            validate_target(&target)?;
            let path = std::env::var("OCIO")
                .map_err(|_| "OCIO environment variable not set".to_string())?;
            let config =
                Config::from_file(path).map_err(|e| format!("OCIO config from env: {e}"))?;
            Ok(Box::new(Self::new(config, target)))
        }

        /// Create from explicit config file path.
        pub fn create_from_file(
            filename: impl AsRef<std::path::Path>,
            target: impl Into<String>,
        ) -> Result<Box<dyn ColorManagementSystem>, String> {
            let target = target.into();
            validate_target(&target)?;
            let config =
                Config::from_file(filename).map_err(|e| format!("OCIO config from file: {e}"))?;
            Ok(Box::new(Self::new(config, target)))
        }

        /// Create from a named built-in OCIO config.
        /// Supported names: "aces_1_3", "studio-config-latest", "ocio://studio-config-latest".
        pub fn create_from_builtin(
            config_name: &str,
            target: impl Into<String>,
        ) -> Result<Box<dyn ColorManagementSystem>, String> {
            let target = target.into();
            validate_target(&target)?;
            let config = match config_name {
                "aces_1_3" | "studio-config-latest" | "ocio://studio-config-latest" => {
                    vfx_ocio::builtin::aces_1_3()
                }
                other => {
                    return Err(format!("Unknown built-in OCIO config: '{other}'"));
                }
            };
            Ok(Box::new(Self::new(config, target)))
        }

        fn new(config: Config, target: String) -> Self {
            Self {
                default_cms: DefaultColorManagementSystem::new(target.clone()),
                config,
                target,
                document: RefCell::new(None),
                cache: RefCell::new(HashMap::new()),
            }
        }

        // --- Internal helpers ---

        /// Return Some(name_in_config) if the color space is known to our config.
        /// Tries: direct lookup -> OCIO legacy remap -> ACES builtin check.
        fn get_supported_cs_name(&self, name: &str) -> Option<String> {
            // 1. Direct config lookup
            if self.config.colorspace(name).is_some() {
                return Some(name.to_string());
            }

            // 2. OCIO-specific legacy name remap (different from DefaultCMS remap)
            if let Some(remapped) = ocio_remap(name) {
                if self.config.colorspace(remapped).is_some() {
                    return Some(remapped.to_string());
                }
            }

            // 3. ACES 1.3 builtin fallback: check if the name is known in builtin ACES config
            //    and simultaneously exists in our config (by role or alias).
            let aces = vfx_ocio::builtin::aces_1_3();
            if aces.colorspace(name).is_some() && self.config.colorspace(name).is_some() {
                return Some(name.to_string());
            }

            None
        }

        /// Get or create an OCIO NodeDef for the given transform.
        /// Mutates the loaded document to add the nodedef + implementation element if needed.
        /// Returns None if the transform cannot be handled by OCIO.
        fn get_ocio_node_def(&self, transform: &ColorSpaceTransform) -> Option<ElementPtr> {
            // Need a loaded document to insert nodedefs into
            let mut doc_borrow = self.document.borrow_mut();
            let doc = doc_borrow.as_mut()?;

            let src_cs = self.get_supported_cs_name(&transform.source_space)?;
            let dst_cs = self.get_supported_cs_name(&transform.target_space)?;

            // Get processor from OCIO config
            let processor = self.config.processor(&src_cs, &dst_cs).ok()?;

            let type_name = transform.type_desc.get_name();

            // No-op (identity) -> use standard ND_dot_<type> nodedef if it exists
            if processor.is_no_op() {
                let nd_name = format!("ND_dot_{}", type_name);
                return doc.get_child(&nd_name);
            }

            // Build GPU processor to inspect texture requirements
            let gpu_proc = GpuProcessor::from_processor(&processor).ok()?;

            // Reject transforms that require LUT textures (not yet supported)
            if gpu_proc.num_textures() > 0 || gpu_proc.num_3d_textures() > 0 {
                return None;
            }

            // Build a stable identifier from the processor cache_id
            let raw_id = processor.cache_id();
            let safe_id = sanitize_id(&raw_id);
            let fn_base = format!("ocio_color_conversion_{}", safe_id);

            let impl_name = format!("{IMPL_PREFIX}{fn_base}_{type_name}");
            let nd_name = format!("{ND_PREFIX}{fn_base}_{type_name}");

            // If nodedef already in document, just ensure GPU processor is cached
            if let Some(existing) = doc.get_child(&nd_name) {
                if !self.cache.borrow().contains_key(&impl_name) {
                    self.cache.borrow_mut().insert(
                        impl_name,
                        CacheEntry {
                            gpu_processor: gpu_proc,
                        },
                    );
                }
                return Some(existing);
            }

            // Dynamically create NodeDef in document
            let nd = doc.add_child_of_category("nodedef", &nd_name).ok()?;
            {
                let mut ndb = nd.borrow_mut();
                ndb.set_attribute("node", fn_base.clone());
                ndb.set_attribute("nodegroup", "colortransform");
                ndb.set_attribute("sourceUri", OCIO_SOURCE_URI);
            }
            // Add input/output children via add_child_of_category on the nodedef ptr
            add_child_of_category(&nd, "input", "in")
                .ok()?
                .borrow_mut()
                .set_attribute("type", type_name.to_string());
            add_child_of_category(&nd, "output", "out")
                .ok()?
                .borrow_mut()
                .set_attribute("type", type_name.to_string());

            // Create Implementation element in document
            if let Ok(imp) = doc.add_child_of_category("implementation", &impl_name) {
                let mut impb = imp.borrow_mut();
                impb.set_attribute("target", self.target.clone());
                impb.set_attribute("nodedef", nd_name);
                impb.set_attribute("sourceUri", OCIO_SOURCE_URI);
            }

            // Cache GPU processor for later code generation
            self.cache.borrow_mut().insert(
                impl_name,
                CacheEntry {
                    gpu_processor: gpu_proc,
                },
            );

            Some(nd)
        }

        /// Generate shader code for a cached impl, replacing "ocio_transform" with `function_name`.
        pub fn get_gpu_processor_code(&self, impl_name: &str, function_name: &str) -> String {
            let cache = self.cache.borrow();
            let entry = match cache.get(impl_name) {
                Some(e) => e,
                None => return String::new(),
            };
            let language = target_to_gpu_lang(&self.target);
            let shader = entry.gpu_processor.generate_shader(language);
            // Replace the default function name with the desired one
            shader
                .fragment_code()
                .replace(OCIO_DEFAULT_FN, function_name)
        }
    }

    impl ColorManagementSystem for OcioColorManagementSystem {
        fn get_name(&self) -> &str {
            OCIO_CMS_NAME
        }

        fn load_library(&mut self, document: Document) {
            // Feed same document to default CMS for standard library node lookup
            self.default_cms.load_library(document.clone());
            *self.document.borrow_mut() = Some(document);
        }

        fn supports_transform(&self, transform: &ColorSpaceTransform) -> bool {
            // Try default CMS first (uses library nodedef docs)
            if self.default_cms.supports_transform(transform) {
                return true;
            }
            // Then OCIO dynamic path
            self.get_ocio_node_def(transform).is_some()
        }

        fn create_node(
            &self,
            transform: &ColorSpaceTransform,
            name: &str,
            doc: &Document,
            context: &dyn ShaderGraphCreateContext,
        ) -> Option<ShaderNode> {
            // Default CMS first
            if let Some(node) = self.default_cms.create_node(transform, name, doc, context) {
                return Some(node);
            }
            // OCIO-generated nodedef
            let node_def = self.get_ocio_node_def(transform)?;
            super::super::shader_node_factory::create_node_from_nodedef(
                name, &node_def, doc, context,
            )
            .ok()
        }

        fn has_implementation(&self, impl_name: &str) -> bool {
            self.cache.borrow().contains_key(impl_name)
        }

        fn create_implementation(&self, impl_name: &str) -> Option<Box<dyn ShaderNodeImpl>> {
            if !self.cache.borrow().contains_key(impl_name) {
                return None;
            }
            let fn_name = fn_name_from_impl(impl_name);
            let gpu_code = self.get_gpu_processor_code(impl_name, &fn_name);
            Some(OcioNode::create(
                impl_name,
                hash_str(&fn_name),
                gpu_code,
                self.target.clone(),
            ))
        }
    }

    // -----------------------------------------------------------------------
    // OcioNode
    // -----------------------------------------------------------------------

    /// OCIO shader node implementation.
    ///
    /// Holds pre-generated GPU shader code and injects it into the pixel stage.
    /// Color3 and color4 variants share the same function definition (same hash).
    pub struct OcioNode {
        /// Implementation name (e.g. "IMPL_MXOCIO_ocio_color_conversion_<id>_color3").
        name: String,
        /// Hash based on function name only -- color3/color4 variants share the same definition.
        hash: u64,
        /// Pre-generated shader code with "ocio_transform" replaced by the actual function name.
        gpu_code: String,
        /// Target identifier ("genglsl", "genmsl", "genosl").
        target: String,
    }

    impl OcioNode {
        pub fn create(
            name: &str,
            hash: u64,
            gpu_code: String,
            target: String,
        ) -> Box<dyn ShaderNodeImpl> {
            Box::new(Self {
                name: name.to_string(),
                hash,
                gpu_code,
                target,
            })
        }

        /// True if this node processes color3 (wraps input in vec4, returns .rgb).
        fn is_color3(&self) -> bool {
            self.name.ends_with("_color3")
        }

        /// Derive the OCIO function name from this node's impl name.
        fn get_fn_name(&self) -> String {
            fn_name_from_impl(&self.name)
        }
    }

    impl ShaderNodeImpl for OcioNode {
        fn get_name(&self) -> &str {
            &self.name
        }

        fn get_hash(&self) -> u64 {
            self.hash
        }

        fn initialize(&mut self, element: &ElementPtr, _context: &dyn ShaderImplContext) {
            // Sync name from element (in case node was created generically)
            let elem_name = element.borrow().get_name().to_string();
            if !elem_name.is_empty() {
                self.name = elem_name;
            }
            // Recompute hash from function name so color3/color4 share one definition
            self.hash = hash_str(&self.get_fn_name());
        }

        fn emit_function_definition(
            &self,
            _node: &ShaderNode,
            _context: &dyn ShaderImplContext,
            stage: &mut ShaderStage,
        ) {
            // Deduplicate: color3 and color4 nodes emit the same OCIO function once
            let fn_name = self.get_fn_name();
            if stage.has_function_definition(&fn_name) {
                return;
            }
            if !self.gpu_code.is_empty() {
                stage.append_source_code(&self.gpu_code);
                stage.append_line("");
                stage.add_function_definition(fn_name);
            }
        }

        fn emit_function_call(
            &self,
            node: &ShaderNode,
            _context: &dyn ShaderImplContext,
            stage: &mut ShaderStage,
        ) {
            let fn_name = self.get_fn_name();
            let is_color3 = self.is_color3();

            let output = match node.get_output_at(0) {
                Some(o) => o,
                None => return,
            };
            let input = match node.get_input_at(0) {
                Some(i) => i,
                None => return,
            };

            let out_var = output.port.get_variable();
            let in_var = input.port.get_variable();

            // GLSL / OSL fallback use vec3/vec4 ; MSL uses float3/float4
            let (scalar3, scalar4, ctor4) = if self.target == "genmsl" {
                ("float3", "float4", "float4")
            } else {
                ("vec3", "vec4", "vec4")
            };

            let line = if is_color3 {
                // vec3 out = func(vec4(in, 1.0)).rgb;
                format!("{scalar3} {out_var} = {fn_name}({ctor4}({in_var}, 1.0)).rgb;")
            } else {
                // vec4 out = func(in);
                format!("{scalar4} {out_var} = {fn_name}({in_var});")
            };
            stage.append_line(&line);
        }
    }
}

// Re-export public OCIO types at the module level when feature is enabled.
#[cfg(feature = "ocio")]
#[allow(unused_imports)]
pub use ocio_impl::{IMPL_PREFIX, OCIO_SOURCE_URI, OcioColorManagementSystem, OcioNode};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen_shader::type_desc::types;

    #[test]
    fn test_color_space_transform_eq() {
        let a = ColorSpaceTransform::new("srgb_texture", "lin_rec709", types::color3());
        let b = ColorSpaceTransform::new("srgb_texture", "lin_rec709", types::color3());
        let c = ColorSpaceTransform::new("acescg", "lin_rec709", types::color3());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_remap_legacy_spaces() {
        assert_eq!(remap_color_space("gamma18"), "g18_rec709");
        assert_eq!(remap_color_space("gamma22"), "g22_rec709");
        assert_eq!(remap_color_space("gamma24"), "rec709_display");
        assert_eq!(remap_color_space("lin_ap1"), "acescg");
        assert_eq!(remap_color_space("lin_ap1_scene"), "acescg");
        assert_eq!(remap_color_space("srgb_rec709_scene"), "srgb_texture");
        // Pass-through for unknown names
        assert_eq!(remap_color_space("custom_space"), "custom_space");
    }

    #[test]
    fn test_default_cms_name() {
        let cms = DefaultColorManagementSystem::new("genslang");
        assert_eq!(cms.get_name(), "default_cms");
    }

    #[test]
    fn test_default_cms_no_library() {
        let cms = DefaultColorManagementSystem::new("genslang");
        let transform = ColorSpaceTransform::new("srgb_texture", "lin_rec709", types::color3());
        // No library loaded -- should return false
        assert!(!cms.supports_transform(&transform));
    }

    #[test]
    fn test_default_cms_trait_defaults() {
        let cms = DefaultColorManagementSystem::new("genslang");
        assert!(!cms.has_implementation("foo"));
        assert!(cms.create_implementation("foo").is_none());
    }

    // OCIO-specific tests -- only compiled when the "ocio" feature is enabled.
    #[cfg(feature = "ocio")]
    mod ocio_tests {
        use crate::gen_shader::color_management::ColorSpaceTransform;
        use crate::gen_shader::color_management::ocio_impl::{
            OcioColorManagementSystem, fn_name_from_impl,
        };
        use crate::gen_shader::type_desc::types;

        #[test]
        fn test_ocio_cms_name() {
            let cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            assert_eq!(cms.get_name(), "OpenColorIO");
        }

        #[test]
        fn test_ocio_invalid_target_rejected() {
            let result = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genvulkan");
            assert!(result.is_err(), "should reject unknown target");
        }

        #[test]
        fn test_ocio_unknown_builtin_rejected() {
            let result =
                OcioColorManagementSystem::create_from_builtin("nonexistent_config", "genglsl");
            assert!(result.is_err(), "should reject unknown built-in config");
        }

        #[test]
        fn test_ocio_has_implementation_empty_initially() {
            let cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            // Nothing cached yet
            assert!(!cms.has_implementation("IMPL_MXOCIO_ocio_color_conversion_abc_color3"));
        }

        #[test]
        fn test_ocio_create_implementation_uncached_returns_none() {
            let cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            let result = cms.create_implementation("IMPL_MXOCIO_ocio_color_conversion_abc_color3");
            assert!(result.is_none(), "uncached impl should return None");
        }

        #[test]
        fn test_ocio_has_implementation_unknown() {
            let cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            // Not in cache -> false
            assert!(!cms.has_implementation("IMPL_MXOCIO_unknown_color3"));
        }

        #[test]
        fn test_fn_name_from_impl_strips_prefix_and_suffix() {
            assert_eq!(
                fn_name_from_impl("IMPL_MXOCIO_ocio_color_conversion_abc_color3"),
                "ocio_color_conversion_abc"
            );
            assert_eq!(
                fn_name_from_impl("IMPL_MXOCIO_ocio_color_conversion_xyz_color4"),
                "ocio_color_conversion_xyz"
            );
            // No prefix -- strip suffix only from remainder
            assert_eq!(
                fn_name_from_impl("ocio_color_conversion_abc_color3"),
                "ocio_color_conversion_abc"
            );
        }

        #[test]
        fn test_ocio_supports_aces_transform_no_doc() {
            let cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            let transform = ColorSpaceTransform::new("ACEScg", "sRGB", types::color3());
            // Without loaded document, get_ocio_node_def returns None -> supports_transform false
            assert!(!cms.supports_transform(&transform));
        }

        #[test]
        fn test_ocio_supports_aces_transform_with_doc() {
            let mut cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            // Load an empty document to enable dynamic nodedef insertion
            cms.load_library(crate::core::Document::new());

            let transform = ColorSpaceTransform::new("ACEScg", "sRGB", types::color3());
            // ACEScg and sRGB are both in ACES 1.3 -- should be supported
            assert!(cms.supports_transform(&transform));
        }

        #[test]
        fn test_ocio_gpu_processor_code_generated() {
            let mut cms = OcioColorManagementSystem::create_from_builtin("aces_1_3", "genglsl")
                .expect("create_from_builtin");
            cms.load_library(crate::core::Document::new());

            let transform = ColorSpaceTransform::new("ACEScg", "sRGB", types::color3());
            // Trigger dynamic nodedef creation
            assert!(cms.supports_transform(&transform));
            // supports_transform should still be true on second call (cached)
            assert!(cms.supports_transform(&transform));
        }
    }
}
