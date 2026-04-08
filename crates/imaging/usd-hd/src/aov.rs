//! AOV (Arbitrary Output Variable) descriptors and bindings.
//!
//! Corresponds to pxr/imaging/hd/aov.h.
//! Describes display channels for render output (color, depth, custom AOVs).

use crate::types::HdFormat;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use usd_gf::Vec3i;
use usd_tf::Token;
use usd_vt::Value;

/// Map of AOV settings (e.g. pixel filtering options).
///
/// Corresponds to C++ `HdAovSettingsMap` = TfHashMap<TfToken, VtValue>.
pub type HdAovSettingsMap = HashMap<Token, Value>;

/// Descriptor for an AOV display channel.
///
/// Corresponds to C++ `HdAovDescriptor`.
/// Bundles state for render buffer format and render pass binding.
#[derive(Debug, Clone)]
pub struct HdAovDescriptor {
    /// Output format for the render buffer.
    pub format: HdFormat,
    /// Whether the render buffer should be multisampled.
    pub multi_sampled: bool,
    /// Clear value to apply before rendering. Type should match format.
    pub clear_value: Value,
    /// Extra settings (e.g. pixel filtering).
    pub aov_settings: HdAovSettingsMap,
}

impl Default for HdAovDescriptor {
    fn default() -> Self {
        Self {
            format: HdFormat::Invalid,
            multi_sampled: false,
            clear_value: Value::default(),
            aov_settings: HdAovSettingsMap::new(),
        }
    }
}

impl HdAovDescriptor {
    /// Create a new AOV descriptor.
    pub fn new(format: HdFormat, multi_sampled: bool, clear_value: Value) -> Self {
        Self {
            format,
            multi_sampled,
            clear_value,
            aov_settings: HdAovSettingsMap::new(),
        }
    }
}

/// List of AOV descriptors.
pub type HdAovDescriptorList = Vec<HdAovDescriptor>;

/// Describes the allocation structure of a render buffer bprim.
///
/// Corresponds to C++ `HdRenderBufferDescriptor`.
#[derive(Debug, Clone, PartialEq)]
pub struct HdRenderBufferDescriptor {
    /// Width, height, and depth of the allocated render buffer.
    pub dimensions: Vec3i,
    /// Data format of the render buffer.
    pub format: HdFormat,
    /// Whether the render buffer should be multisampled.
    pub multi_sampled: bool,
}

impl Default for HdRenderBufferDescriptor {
    fn default() -> Self {
        Self {
            dimensions: Vec3i::new(0, 0, 0),
            format: HdFormat::Invalid,
            multi_sampled: false,
        }
    }
}

impl HdRenderBufferDescriptor {
    /// Create a new render buffer descriptor.
    pub fn new(dimensions: Vec3i, format: HdFormat, multi_sampled: bool) -> Self {
        Self {
            dimensions,
            format,
            multi_sampled,
        }
    }
}

/// Render pass AOV binding - binds renderer output to render buffer.
///
/// Corresponds to C++ `HdRenderPassAovBinding`.
#[derive(Debug, Clone)]
pub struct HdRenderPassAovBinding {
    /// AOV name (e.g., "color", "depth", "primId").
    pub aov_name: Token,
    /// Path to render buffer in render index (when not using raw pointer).
    pub render_buffer_id: usd_sdf::Path,
    /// Optional pointer to an already-allocated render buffer.
    /// When non-null, this is used preferentially over render_buffer_id.
    pub render_buffer: Option<*mut ()>,
    /// Clear value before rendering (empty = no clear).
    pub clear_value: Value,
    /// Extra AOV settings.
    pub aov_settings: HdAovSettingsMap,
}

// SAFETY: Mirrors C++ semantics where HdRenderBuffer* is freely shared.
// The raw pointer is only stored/compared, not dereferenced without external sync.
// Actual buffer access is protected by the render index's synchronization.
#[allow(unsafe_code)]
unsafe impl Send for HdRenderPassAovBinding {}
#[allow(unsafe_code)]
unsafe impl Sync for HdRenderPassAovBinding {}

impl PartialEq for HdRenderPassAovBinding {
    fn eq(&self, other: &Self) -> bool {
        self.aov_name == other.aov_name
            && self.render_buffer_id == other.render_buffer_id
            && self.render_buffer == other.render_buffer
            && self.clear_value == other.clear_value
            && self.aov_settings == other.aov_settings
    }
}

impl Default for HdRenderPassAovBinding {
    fn default() -> Self {
        Self {
            aov_name: Token::new(""),
            render_buffer_id: usd_sdf::Path::default(),
            render_buffer: None,
            clear_value: Value::default(),
            aov_settings: HdAovSettingsMap::new(),
        }
    }
}

impl HdRenderPassAovBinding {
    /// Create new binding.
    pub fn new(
        aov_name: Token,
        render_buffer_id: usd_sdf::Path,
        clear_value: Value,
        aov_settings: HdAovSettingsMap,
    ) -> Self {
        Self {
            aov_name,
            render_buffer_id,
            render_buffer: None,
            clear_value,
            aov_settings,
        }
    }
}

/// Hash for HdRenderPassAovBinding.
///
/// Uses render_buffer_id path hash, matching C++ `hash_value(HdRenderPassAovBinding)`.
impl Hash for HdRenderPassAovBinding {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.render_buffer_id.hash(state);
    }
}

/// Vector of render pass AOV bindings.
pub type HdRenderPassAovBindingVector = Vec<HdRenderPassAovBinding>;

/// AOV prefix tokens (parsed from aov name).
/// AOV prefix tokens (parsed from aov name).
pub mod aov_prefix {
    /// "primvars:" prefix for primvar AOVs.
    pub fn primvars() -> &'static str {
        "primvars:"
    }
    /// "lpe:" prefix for light path expression AOVs.
    pub fn lpe() -> &'static str {
        "lpe:"
    }
    /// "shader:" prefix for shader signal AOVs.
    pub fn shader() -> &'static str {
        "shader:"
    }
}

/// Parsed AOV token with prefix (primvars:/lpe:/shader:).
///
/// Corresponds to C++ `HdParsedAovToken`.
#[derive(Debug, Clone)]
pub struct HdParsedAovToken {
    /// Token name (suffix after prefix, or full name if no prefix).
    pub name: Token,
    /// True if aov name started with "primvars:".
    pub is_primvar: bool,
    /// True if aov name started with "lpe:".
    pub is_lpe: bool,
    /// True if aov name started with "shader:".
    pub is_shader: bool,
}

impl Default for HdParsedAovToken {
    fn default() -> Self {
        Self {
            name: Token::new(""),
            is_primvar: false,
            is_lpe: false,
            is_shader: false,
        }
    }
}

impl HdParsedAovToken {
    /// Parse AOV token from name (extract prefix: primvars:/lpe:/shader:).
    pub fn new(aov_name: &Token) -> Self {
        let aov = aov_name.as_str();
        let primvars = aov_prefix::primvars();
        let lpe = aov_prefix::lpe();
        let shader = aov_prefix::shader();

        if aov.len() > primvars.len() && aov.starts_with(primvars) {
            Self {
                name: Token::new(&aov[primvars.len()..]),
                is_primvar: true,
                is_lpe: false,
                is_shader: false,
            }
        } else if aov.len() > lpe.len() && aov.starts_with(lpe) {
            Self {
                name: Token::new(&aov[lpe.len()..]),
                is_primvar: false,
                is_lpe: true,
                is_shader: false,
            }
        } else if aov.len() > shader.len() && aov.starts_with(shader) {
            Self {
                name: Token::new(&aov[shader.len()..]),
                is_primvar: false,
                is_lpe: false,
                is_shader: true,
            }
        } else {
            Self {
                name: aov_name.clone(),
                is_primvar: false,
                is_lpe: false,
                is_shader: false,
            }
        }
    }
}

/// Vector of parsed AOV tokens.
pub type HdParsedAovTokenVector = Vec<HdParsedAovToken>;

/// Returns true if the AOV name indicates depth semantics.
/// Case-insensitive check for "depth" suffix, matching C++ behavior.
///
/// Corresponds to C++ `HdAovHasDepthSemantic`.
pub fn hd_aov_has_depth_semantic(aov_name: &Token) -> bool {
    aov_name.as_str().to_ascii_lowercase().ends_with("depth")
}

/// Returns true if the AOV name indicates depth-stencil semantics.
/// Case-insensitive check for "depthstencil" suffix, matching C++ behavior.
///
/// Corresponds to C++ `HdAovHasDepthStencilSemantic`.
pub fn hd_aov_has_depth_stencil_semantic(aov_name: &Token) -> bool {
    aov_name
        .as_str()
        .to_ascii_lowercase()
        .ends_with("depthstencil")
}

/// Create AOV token for primvar output: "primvars:" + primvar_name.
///
/// Corresponds to C++ `HdAovTokensMakePrimvar`.
pub fn hd_aov_tokens_make_primvar(primvar: &Token) -> Token {
    let s = format!("{}{}", aov_prefix::primvars(), primvar.as_str());
    Token::new(&s)
}

/// Create AOV token for light path expression: "lpe:" + lpe.
///
/// Corresponds to C++ `HdAovTokensMakeLpe`.
pub fn hd_aov_tokens_make_lpe(lpe: &Token) -> Token {
    let s = format!("{}{}", aov_prefix::lpe(), lpe.as_str());
    Token::new(&s)
}

/// Create AOV token for shader signal: "shader:" + shader.
///
/// Corresponds to C++ `HdAovTokensMakeShader`.
pub fn hd_aov_tokens_make_shader(shader: &Token) -> Token {
    let s = format!("{}{}", aov_prefix::shader(), shader.as_str());
    Token::new(&s)
}
