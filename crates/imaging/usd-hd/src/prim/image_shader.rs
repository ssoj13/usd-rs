
//! HdImageShader - Image shader state primitive.
//!
//! Fullscreen image shader for post-process effects (OIT resolve, outline, etc).
//! See pxr/imaging/hd/imageShader.h for C++ reference.

use super::{HdRenderParam, HdSceneDelegate, HdSprim};
use crate::types::HdDirtyBits;
use std::collections::HashMap;
use std::sync::LazyLock;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::Dictionary;

// Token constants matching C++ HdImageShaderTokens
static TOK_ENABLED: LazyLock<Token> = LazyLock::new(|| Token::new("enabled"));
static TOK_PRIORITY: LazyLock<Token> = LazyLock::new(|| Token::new("priority"));
static TOK_FILE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("filePath"));
static TOK_CONSTANTS: LazyLock<Token> = LazyLock::new(|| Token::new("constants"));

/// Dirty bits matching C++ HdImageShader::DirtyBits enum.
const DIRTY_ENABLED: HdDirtyBits = 1 << 0; // DirtyEnabled
const DIRTY_PRIORITY: HdDirtyBits = 1 << 1; // DirtyPriority
const DIRTY_FILE_PATH: HdDirtyBits = 1 << 2; // DirtyFilePath
const DIRTY_CONSTANTS: HdDirtyBits = 1 << 3; // DirtyConstants
#[allow(dead_code)]
const DIRTY_MATERIAL_NETWORK: HdDirtyBits = 1 << 4; // DirtyMaterialNetwork
/// All dirty bits (matches C++ AllDirty = 0x1F).
const ALL_DIRTY_BITS: HdDirtyBits =
    DIRTY_ENABLED | DIRTY_PRIORITY | DIRTY_FILE_PATH | DIRTY_CONSTANTS | DIRTY_MATERIAL_NETWORK;

/// Image shader state primitive.
///
/// Represents a fullscreen image shader for post-processing effects.
#[derive(Debug)]
pub struct HdImageShader {
    /// Prim path
    id: SdfPath,

    /// Dirty bits
    dirty_bits: HdDirtyBits,

    /// Whether shader is enabled
    enabled: bool,

    /// Priority for ordering (higher = later in pipeline)
    priority: i32,

    /// Shader file path (.glslfx)
    file_path: String,

    /// Constant parameters
    constants: HashMap<String, usd_vt::Value>,
}

impl HdImageShader {
    /// Create a new image shader.
    ///
    /// Initial state: disabled, priority 0, empty path/constants.
    /// Matches C++ `HdImageShader(id)` which sets `_enabled(false)`.
    pub fn new(id: SdfPath) -> Self {
        Self {
            id,
            dirty_bits: ALL_DIRTY_BITS,
            enabled: false,
            priority: 0,
            file_path: String::new(),
            constants: HashMap::new(),
        }
    }

    /// Whether the shader is enabled.
    pub fn get_enabled(&self) -> bool {
        self.enabled
    }

    /// Get priority for pipeline ordering.
    pub fn get_priority(&self) -> i32 {
        self.priority
    }

    /// Get shader file path.
    pub fn get_file_path(&self) -> &str {
        &self.file_path
    }

    /// Get constants.
    pub fn get_constants(&self) -> &HashMap<String, usd_vt::Value> {
        &self.constants
    }
}

impl HdSprim for HdImageShader {
    fn get_id(&self) -> &SdfPath {
        &self.id
    }

    fn get_dirty_bits(&self) -> HdDirtyBits {
        self.dirty_bits
    }

    fn set_dirty_bits(&mut self, bits: HdDirtyBits) {
        self.dirty_bits = bits;
    }

    fn sync(
        &mut self,
        delegate: &dyn HdSceneDelegate,
        _render_param: Option<&dyn HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        let id = self.id.clone();
        let bits = *dirty_bits;

        // Read enabled flag.
        if bits & DIRTY_ENABLED != 0 {
            let val = delegate.get(&id, &TOK_ENABLED);
            if !val.is_empty() {
                if let Some(&v) = val.get::<bool>() {
                    self.enabled = v;
                }
            }
        }

        // Read priority.
        if bits & DIRTY_PRIORITY != 0 {
            let val = delegate.get(&id, &TOK_PRIORITY);
            if !val.is_empty() {
                if let Some(&v) = val.get::<i32>() {
                    self.priority = v;
                }
            }
        }

        // Read shader file path.
        if bits & DIRTY_FILE_PATH != 0 {
            let val = delegate.get(&id, &TOK_FILE_PATH);
            if !val.is_empty() {
                if let Some(v) = val.get::<String>() {
                    self.file_path = v.clone();
                }
            }
        }

        // Read constants dictionary (VtDictionary -> HashMap<String, Value>).
        if bits & DIRTY_CONSTANTS != 0 {
            let val = delegate.get(&id, &TOK_CONSTANTS);
            if !val.is_empty() {
                if let Some(dict) = val.get::<Dictionary>() {
                    self.constants = dict.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                }
            }
        }

        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}
