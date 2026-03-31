
//! HdStField - Storm field (volume) buffer primitive.
//!
//! Represents OpenVDB/Field3D volume data. See pxr/imaging/hdSt/field.h for C++ reference.

use crate::texture_identifier::{HdStTextureIdentifier, SubtextureIdentifier};
use std::sync::LazyLock;
use usd_hd::prim::{HdBprim, HdSceneDelegate};
use usd_hd::types::HdDirtyBits;
use usd_sdf::Path as SdfPath;
use usd_tf::Token;
use usd_vt::AssetPath;

/// Token for openvdbAsset field type
static OPENVDB_ASSET: LazyLock<Token> = LazyLock::new(|| Token::new("openvdbAsset"));
/// Token for field3dAsset field type
static FIELD3D_ASSET: LazyLock<Token> = LazyLock::new(|| Token::new("field3dAsset"));

/// Supported bprim types for HdStField
pub fn get_supported_bprim_types() -> Vec<Token> {
    vec![OPENVDB_ASSET.clone(), FIELD3D_ASSET.clone()]
}

/// Check if bprim type is supported by HdStField
pub fn is_supported_bprim_type(bprim_type: &Token) -> bool {
    bprim_type == "openvdbAsset" || bprim_type == "field3dAsset"
}

/// Storm field implementation (OpenVDB, Field3D).
#[derive(Debug)]
pub struct HdStField {
    /// Prim path
    id: SdfPath,

    /// Dirty bits
    dirty_bits: HdDirtyBits,

    /// Field type token (openvdbAsset, field3dAsset)
    field_type: Token,

    /// Texture identifier for loading
    texture_id: HdStTextureIdentifier,

    /// Texture memory estimate in bytes
    texture_memory: usize,
}

impl HdStField {
    /// Create a new Storm field.
    pub fn new(id: SdfPath, field_type: Token) -> Self {
        Self {
            id,
            dirty_bits: Self::ALL_DIRTY,
            field_type,
            texture_id: HdStTextureIdentifier::default(),
            texture_memory: 0,
        }
    }

    /// Get texture identifier.
    pub fn get_texture_identifier(&self) -> &HdStTextureIdentifier {
        &self.texture_id
    }

    /// Get texture memory estimate in bytes.
    pub fn get_texture_memory(&self) -> usize {
        self.texture_memory
    }

    /// Get field type.
    pub fn get_field_type(&self) -> &Token {
        &self.field_type
    }
}

impl HdBprim for HdStField {
    /// HdStField uses HdField dirty bit layout
    const DIRTY_PARAMS: HdDirtyBits = 1 << 1;
    const ALL_DIRTY: HdDirtyBits = (1 << 0) | (1 << 1); // DirtyTransform | DirtyParams

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
        _render_param: Option<&dyn usd_hd::prim::HdRenderParam>,
        dirty_bits: &mut HdDirtyBits,
    ) {
        // Token keys matching C++ HdFieldTokens and private _tokens
        static TOK_FILE_PATH: LazyLock<Token> = LazyLock::new(|| Token::new("filePath"));
        static TOK_FIELD_NAME: LazyLock<Token> = LazyLock::new(|| Token::new("fieldName"));
        static TOK_FIELD_INDEX: LazyLock<Token> = LazyLock::new(|| Token::new("fieldIndex"));
        static TOK_FIELD_PURPOSE: LazyLock<Token> = LazyLock::new(|| Token::new("fieldPurpose"));
        static TOK_TEXTURE_MEMORY: LazyLock<Token> = LazyLock::new(|| Token::new("textureMemory"));

        if *dirty_bits & Self::DIRTY_PARAMS != 0 {
            let id = self.id.clone();

            // Read asset path — delegate returns SdfAssetPath stored as VtValue.
            // Prefer resolved path when available, fall back to authored path.
            let file_path_val = delegate.get(&id, &TOK_FILE_PATH);
            let resolved_path = if let Some(ap) = file_path_val.get::<AssetPath>() {
                let r = ap.get_resolved_path();
                if r.is_empty() {
                    ap.get_asset_path().to_owned()
                } else {
                    r.to_owned()
                }
            } else {
                String::new()
            };
            let resolved_token = Token::new(&resolved_path);

            // Field name.
            let field_name_val = delegate.get(&id, &TOK_FIELD_NAME);
            let field_name = field_name_val.get::<Token>().cloned().unwrap_or_default();

            // Field index (int).
            let field_index_val = delegate.get(&id, &TOK_FIELD_INDEX);
            let _field_index = field_index_val.get::<i32>().copied().unwrap_or(0);

            // Build texture identifier — SubtextureIdentifier::Field carries the grid name.
            // For field3d we additionally read fieldPurpose, but our identifier
            // type only stores the name for now (no separate purpose/index slots).
            let sub = if self.field_type == *OPENVDB_ASSET {
                SubtextureIdentifier::Field(field_name)
            } else {
                // field3dAsset: read optional fieldPurpose.
                let _purpose_val = delegate.get(&id, &TOK_FIELD_PURPOSE);
                SubtextureIdentifier::Field(field_name)
            };

            let file_asset_path = usd_sdf::AssetPath::new(resolved_token.as_str());
            self.texture_id = HdStTextureIdentifier::with_subtexture(file_asset_path, sub);

            // Texture memory in bytes: delegate returns float MB.
            let mem_val = delegate.get(&id, &TOK_TEXTURE_MEMORY);
            let mb = mem_val.get::<f32>().copied().unwrap_or(0.0_f32);
            self.texture_memory = (1_048_576.0 * mb) as usize;
        }

        *dirty_bits = Self::CLEAN;
        self.dirty_bits = Self::CLEAN;
    }
}
