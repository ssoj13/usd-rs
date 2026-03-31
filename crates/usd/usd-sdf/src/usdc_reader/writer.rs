//! USDC binary format writer — CrateWriter struct and all write/encode methods.
//!
//! CrateWriter builds token/path/field/spec tables and writes compressed
//! sections to produce a valid USDC binary file.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use usd_gf::{Matrix2d, Matrix3d, Matrix4d};
use usd_tf::Token;
use usd_tf::fast_compression::FastCompression;

use crate::Layer;
use crate::path::Path;
use crate::types::SpecType;

use super::types::{
    BOOTSTRAP_SIZE, Bootstrap, CrateSpec, Field, FieldIndex, FieldSetIndex, Index, PathIndex,
    Section, StringIndex, TableOfContents, TokenIndex, TypeEnum, ValueRep,
};

// ============================================================================
// CrateWriter - Binary writer for USDC files
// ============================================================================

/// Writer for USDC binary format.
/// Handles building tables and writing compressed sections.
pub struct CrateWriter {
    /// Write version
    version: (u8, u8, u8),
    /// Token to index mapping
    token_to_index: std::collections::HashMap<Token, TokenIndex>,
    /// Tokens in order
    tokens: Vec<Token>,
    /// String to index mapping
    string_to_index: std::collections::HashMap<String, StringIndex>,
    /// Strings (stored as token indices)
    strings: Vec<TokenIndex>,
    /// Path to index mapping (pub for test access from parent module)
    pub path_to_index: std::collections::HashMap<Path, PathIndex>,
    /// Paths in order
    paths: Vec<Path>,
    /// Fields
    fields: Vec<Field>,
    /// Field deduplication
    field_to_index: std::collections::HashMap<(u32, u64), FieldIndex>,
    /// Field sets (terminated by invalid index)
    field_sets: Vec<FieldIndex>,
    /// Field set deduplication (hash of field indices -> index)
    field_set_to_index: std::collections::HashMap<Vec<u32>, FieldSetIndex>,
    /// Specs
    specs: Vec<CrateSpec>,
    /// Output buffer
    buffer: Vec<u8>,
    /// Value dedup: hash of written bytes -> (data bytes, ValueRep) for collision safety
    value_dedup: HashMap<u64, Vec<(Vec<u8>, ValueRep)>>,
}

impl CrateWriter {
    /// Creates a new writer with the given version.
    ///
    /// The buffer is pre-seeded with BOOTSTRAP_SIZE zero bytes so that any
    /// value-data written by `populate_from_layer` (e.g. doubles, token-vectors)
    /// receives absolute file offsets that are correct once `write()` fills in
    /// the real bootstrap header at the front and appends all sections after it.
    pub fn new(version: (u8, u8, u8)) -> Self {
        let mut writer = Self {
            version,
            token_to_index: std::collections::HashMap::new(),
            tokens: Vec::new(),
            string_to_index: std::collections::HashMap::new(),
            strings: Vec::new(),
            path_to_index: std::collections::HashMap::new(),
            paths: Vec::new(),
            fields: Vec::new(),
            field_to_index: std::collections::HashMap::new(),
            field_sets: Vec::new(),
            field_set_to_index: std::collections::HashMap::new(),
            specs: Vec::new(),
            buffer: Vec::new(),
            value_dedup: HashMap::new(),
        };
        // Reserve bootstrap placeholder so subsequent value-data gets correct
        // absolute offsets (bootstrap is written last by write()).
        writer.buffer.resize(BOOTSTRAP_SIZE, 0);
        // Add empty path at index 0
        writer.paths.push(Path::empty());
        writer
            .path_to_index
            .insert(Path::empty(), PathIndex::new(0));
        writer
    }

    /// Adds a token and returns its index. Deduplicates.
    ///
    /// USDC stores indices as u32, so we saturate at u32::MAX. In practice
    /// files cannot exceed ~4 billion unique tokens.
    pub fn add_token(&mut self, token: &Token) -> TokenIndex {
        if let Some(&idx) = self.token_to_index.get(token) {
            return idx;
        }
        let raw = u32::try_from(self.tokens.len()).unwrap_or_else(|_| {
            log::error!("USDC token table overflow (>u32::MAX tokens)");
            u32::MAX
        });
        let idx = TokenIndex::new(raw);
        self.tokens.push(token.clone());
        self.token_to_index.insert(token.clone(), idx);
        idx
    }

    /// Adds a string and returns its index. Deduplicates.
    pub fn add_string(&mut self, s: &str) -> StringIndex {
        if let Some(&idx) = self.string_to_index.get(s) {
            return idx;
        }
        // Strings are stored as token indices
        let token = Token::new(s);
        let token_idx = self.add_token(&token);
        let raw = u32::try_from(self.strings.len()).unwrap_or_else(|_| {
            log::error!("USDC string table overflow (>u32::MAX strings)");
            u32::MAX
        });
        let idx = StringIndex::new(raw);
        self.strings.push(token_idx);
        self.string_to_index.insert(s.to_string(), idx);
        idx
    }

    /// Adds a path and returns its index. Deduplicates.
    pub fn add_path(&mut self, path: &Path) -> PathIndex {
        if let Some(&idx) = self.path_to_index.get(path) {
            return idx;
        }
        let raw = u32::try_from(self.paths.len()).unwrap_or_else(|_| {
            log::error!("USDC path table overflow (>u32::MAX paths)");
            u32::MAX
        });
        let idx = PathIndex::new(raw);
        self.paths.push(path.clone());
        self.path_to_index.insert(path.clone(), idx);
        idx
    }

    /// Adds a field and returns its index. Deduplicates.
    pub fn add_field(&mut self, token_idx: TokenIndex, value_rep: ValueRep) -> FieldIndex {
        let key = (token_idx.value(), value_rep.data);
        if let Some(&idx) = self.field_to_index.get(&key) {
            return idx;
        }
        let raw = u32::try_from(self.fields.len()).unwrap_or_else(|_| {
            log::error!("USDC field table overflow (>u32::MAX fields)");
            u32::MAX
        });
        let idx = FieldIndex::new(raw);
        self.fields.push(Field::new(token_idx, value_rep));
        self.field_to_index.insert(key, idx);
        idx
    }

    /// Adds a field set (list of field indices) and returns its index. Deduplicates.
    pub fn add_field_set(&mut self, field_indices: &[FieldIndex]) -> FieldSetIndex {
        let key: Vec<u32> = field_indices.iter().map(|f| f.value()).collect();
        if let Some(&idx) = self.field_set_to_index.get(&key) {
            return idx;
        }
        let raw = u32::try_from(self.field_sets.len()).unwrap_or_else(|_| {
            log::error!("USDC field-set table overflow (>u32::MAX entries)");
            u32::MAX
        });
        let idx = FieldSetIndex::new(raw);
        // Add field indices
        for &fi in field_indices {
            self.field_sets.push(fi);
        }
        // Add terminator (invalid index)
        self.field_sets.push(FieldIndex(Index::invalid()));
        self.field_set_to_index.insert(key, idx);
        idx
    }

    /// Adds a spec.
    pub fn add_spec(
        &mut self,
        path_idx: PathIndex,
        spec_type: SpecType,
        field_set_idx: FieldSetIndex,
    ) {
        self.specs.push(CrateSpec {
            path_index: path_idx,
            field_set_index: field_set_idx,
            spec_type,
        });
    }

    /// Packs a value into a ValueRep.
    /// Returns an inlined or file-stored ValueRep depending on the type.
    pub fn pack_value(&mut self, value: &usd_vt::Value) -> ValueRep {
        use crate::{AssetPath, Permission, Specifier, Variability};

        // Try to unbox common types and create appropriate ValueReps
        // Many simple types can be inlined in the 6-byte payload
        if let Some(&b) = value.get::<bool>() {
            return ValueRep::new_inlined(TypeEnum::Bool, if b { 1 } else { 0 });
        }
        if let Some(&i) = value.get::<i32>() {
            return ValueRep::new_inlined(TypeEnum::Int, i as u32);
        }
        if let Some(&u) = value.get::<u32>() {
            return ValueRep::new_inlined(TypeEnum::UInt, u);
        }
        if let Some(&f) = value.get::<f32>() {
            return ValueRep::new_inlined(TypeEnum::Float, f.to_bits());
        }
        if let Some(&spec) = value.get::<Specifier>() {
            return ValueRep::new_inlined(TypeEnum::Specifier, spec as u32);
        }
        if let Some(&var) = value.get::<Variability>() {
            return ValueRep::new_inlined(TypeEnum::Variability, var as u32);
        }
        if let Some(&perm) = value.get::<Permission>() {
            return ValueRep::new_inlined(TypeEnum::Permission, perm as u32);
        }
        if let Some(token) = value.get::<Token>() {
            let idx = self.add_token(token);
            return ValueRep::new_inlined(TypeEnum::Token, idx.value());
        }
        if let Some(s) = value.get::<String>() {
            let idx = self.add_string(s);
            return ValueRep::new_inlined(TypeEnum::String, idx.value());
        }
        if let Some(ap) = value.get::<AssetPath>() {
            // Asset path: store as token index
            let idx = self.add_token(&Token::new(ap.get_authored_path()));
            return ValueRep::new_inlined(TypeEnum::AssetPath, idx.value());
        }
        // Double: inline if exactly representable as f32 (P2-1),
        // otherwise write with dedup (P2-4)
        if let Some(&d) = value.get::<f64>() {
            let f = d as f32;
            if f as f64 == d {
                // Exactly representable as float — inline as f32 bits
                return ValueRep::new_inlined(TypeEnum::Double, f.to_bits());
            }
            return self.write_deduped(TypeEnum::Double, &d.to_le_bytes());
        }

        // Empty dictionary inlining (P2-3)
        if let Some(dict) = value.get::<usd_vt::Dictionary>() {
            if dict.is_empty() {
                return ValueRep::new_inlined(TypeEnum::Dictionary, 0);
            }
        }

        // ── Scalar vector / matrix / quaternion types ─────────────────────────
        use usd_gf::half::Half;
        use usd_gf::quat::Quath;
        use usd_gf::vec2::Vec2h;
        use usd_gf::vec3::Vec3h;
        use usd_gf::vec4::Vec4h;
        use usd_gf::{Matrix2d, Matrix3d};
        use usd_gf::{
            Matrix4d, Quatd, Quatf, Vec2d, Vec2f, Vec2i, Vec3d, Vec3f, Vec3i, Vec4d, Vec4f, Vec4i,
        };

        if let Some(v) = value.get::<Vec2f>() {
            return self.write_deduped(TypeEnum::Vec2f, bytemuck_pod_2(v.x, v.y).as_ref());
        }
        if let Some(v) = value.get::<Vec3f>() {
            return self.write_deduped(TypeEnum::Vec3f, bytemuck_pod_3(v.x, v.y, v.z).as_ref());
        }
        if let Some(v) = value.get::<Vec4f>() {
            return self
                .write_deduped(TypeEnum::Vec4f, bytemuck_pod_4(v.x, v.y, v.z, v.w).as_ref());
        }
        if let Some(v) = value.get::<Vec2d>() {
            return self.write_deduped(TypeEnum::Vec2d, bytemuck_pod_2(v.x, v.y).as_ref());
        }
        if let Some(v) = value.get::<Vec3d>() {
            return self.write_deduped(TypeEnum::Vec3d, bytemuck_pod_3(v.x, v.y, v.z).as_ref());
        }
        if let Some(v) = value.get::<Vec4d>() {
            return self
                .write_deduped(TypeEnum::Vec4d, bytemuck_pod_4(v.x, v.y, v.z, v.w).as_ref());
        }
        if let Some(v) = value.get::<Vec2i>() {
            return self.write_deduped(TypeEnum::Vec2i, bytemuck_pod_2(v.x, v.y).as_ref());
        }
        if let Some(v) = value.get::<Vec3i>() {
            return self.write_deduped(TypeEnum::Vec3i, bytemuck_pod_3(v.x, v.y, v.z).as_ref());
        }
        if let Some(v) = value.get::<Vec4i>() {
            return self
                .write_deduped(TypeEnum::Vec4i, bytemuck_pod_4(v.x, v.y, v.z, v.w).as_ref());
        }
        if let Some(v) = value.get::<Vec2h>() {
            return self.write_deduped(
                TypeEnum::Vec2h,
                bytemuck_pod_2(v.x.bits(), v.y.bits()).as_ref(),
            );
        }
        if let Some(v) = value.get::<Vec3h>() {
            return self.write_deduped(
                TypeEnum::Vec3h,
                bytemuck_pod_3(v.x.bits(), v.y.bits(), v.z.bits()).as_ref(),
            );
        }
        if let Some(v) = value.get::<Vec4h>() {
            return self.write_deduped(
                TypeEnum::Vec4h,
                bytemuck_pod_4(v.x.bits(), v.y.bits(), v.z.bits(), v.w.bits()).as_ref(),
            );
        }
        if let Some(v) = value.get::<Quatf>() {
            // C++ USDC quaternion layout: imaginary.x, imaginary.y, imaginary.z, real
            let im = v.imaginary();
            return self.write_deduped(
                TypeEnum::Quatf,
                bytemuck_pod_4(im.x, im.y, im.z, v.real()).as_ref(),
            );
        }
        if let Some(v) = value.get::<Quatd>() {
            let im = v.imaginary();
            return self.write_deduped(
                TypeEnum::Quatd,
                bytemuck_pod_4(im.x, im.y, im.z, v.real()).as_ref(),
            );
        }
        if let Some(v) = value.get::<Quath>() {
            let im = v.imaginary();
            return self.write_deduped(
                TypeEnum::Quath,
                bytemuck_pod_4(im.x.bits(), im.y.bits(), im.z.bits(), v.real().bits()).as_ref(),
            );
        }
        if let Some(v) = value.get::<Matrix2d>() {
            let bytes = matrix_to_bytes_2x2(v);
            return self.write_deduped(TypeEnum::Matrix2d, &bytes);
        }
        if let Some(v) = value.get::<Matrix3d>() {
            let bytes = matrix_to_bytes_3x3(v);
            return self.write_deduped(TypeEnum::Matrix3d, &bytes);
        }
        if let Some(v) = value.get::<Matrix4d>() {
            let bytes = matrix_to_bytes_4x4(v);
            return self.write_deduped(TypeEnum::Matrix4d, &bytes);
        }
        if let Some(&h) = value.get::<Half>() {
            return ValueRep::new_inlined(TypeEnum::Half, h.bits() as u32);
        }
        if let Some(&i) = value.get::<i64>() {
            let bytes = i.to_le_bytes();
            return self.write_deduped(TypeEnum::Int64, &bytes);
        }
        if let Some(&u) = value.get::<u64>() {
            let bytes = u.to_le_bytes();
            return self.write_deduped(TypeEnum::UInt64, &bytes);
        }

        // ── Array types ──────────────────────────────────────────────────────
        if let Some(arr) = value.get::<Vec<bool>>() {
            return self.write_raw_array(TypeEnum::Bool, arr.len(), |buf| {
                for &v in arr {
                    buf.push(v as u8);
                }
            });
        }
        if let Some(arr) = value.get::<Vec<i32>>() {
            return self.write_raw_array(TypeEnum::Int, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<u32>>() {
            return self.write_raw_array(TypeEnum::UInt, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<f32>>() {
            return self.write_raw_array(TypeEnum::Float, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<f64>>() {
            return self.write_raw_array(TypeEnum::Double, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<i64>>() {
            return self.write_raw_array(TypeEnum::Int64, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<u64>>() {
            return self.write_raw_array(TypeEnum::UInt64, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Half>>() {
            return self.write_raw_array(TypeEnum::Half, arr.len(), |buf| {
                for &v in arr {
                    buf.extend_from_slice(&v.bits().to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec2f>>() {
            return self.write_raw_array(TypeEnum::Vec2f, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec3f>>() {
            return self.write_raw_array(TypeEnum::Vec3f, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                    buf.extend_from_slice(&v.z.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec4f>>() {
            return self.write_raw_array(TypeEnum::Vec4f, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                    buf.extend_from_slice(&v.z.to_le_bytes());
                    buf.extend_from_slice(&v.w.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec2d>>() {
            return self.write_raw_array(TypeEnum::Vec2d, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec3d>>() {
            return self.write_raw_array(TypeEnum::Vec3d, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                    buf.extend_from_slice(&v.z.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec4d>>() {
            return self.write_raw_array(TypeEnum::Vec4d, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                    buf.extend_from_slice(&v.z.to_le_bytes());
                    buf.extend_from_slice(&v.w.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec2i>>() {
            return self.write_raw_array(TypeEnum::Vec2i, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec3i>>() {
            return self.write_raw_array(TypeEnum::Vec3i, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                    buf.extend_from_slice(&v.z.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec4i>>() {
            return self.write_raw_array(TypeEnum::Vec4i, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.to_le_bytes());
                    buf.extend_from_slice(&v.y.to_le_bytes());
                    buf.extend_from_slice(&v.z.to_le_bytes());
                    buf.extend_from_slice(&v.w.to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec2h>>() {
            return self.write_raw_array(TypeEnum::Vec2h, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.bits().to_le_bytes());
                    buf.extend_from_slice(&v.y.bits().to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec3h>>() {
            return self.write_raw_array(TypeEnum::Vec3h, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.bits().to_le_bytes());
                    buf.extend_from_slice(&v.y.bits().to_le_bytes());
                    buf.extend_from_slice(&v.z.bits().to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Vec4h>>() {
            return self.write_raw_array(TypeEnum::Vec4h, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&v.x.bits().to_le_bytes());
                    buf.extend_from_slice(&v.y.bits().to_le_bytes());
                    buf.extend_from_slice(&v.z.bits().to_le_bytes());
                    buf.extend_from_slice(&v.w.bits().to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Matrix4d>>() {
            return self.write_raw_array(TypeEnum::Matrix4d, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&matrix_to_bytes_4x4(v));
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Matrix2d>>() {
            return self.write_raw_array(TypeEnum::Matrix2d, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&matrix_to_bytes_2x2(v));
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Matrix3d>>() {
            return self.write_raw_array(TypeEnum::Matrix3d, arr.len(), |buf| {
                for v in arr {
                    buf.extend_from_slice(&matrix_to_bytes_3x3(v));
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Quatf>>() {
            return self.write_raw_array(TypeEnum::Quatf, arr.len(), |buf| {
                for v in arr {
                    let im = v.imaginary();
                    buf.extend_from_slice(&im.x.to_le_bytes());
                    buf.extend_from_slice(&im.y.to_le_bytes());
                    buf.extend_from_slice(&im.z.to_le_bytes());
                    buf.extend_from_slice(&v.real().to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Quatd>>() {
            return self.write_raw_array(TypeEnum::Quatd, arr.len(), |buf| {
                for v in arr {
                    let im = v.imaginary();
                    buf.extend_from_slice(&im.x.to_le_bytes());
                    buf.extend_from_slice(&im.y.to_le_bytes());
                    buf.extend_from_slice(&im.z.to_le_bytes());
                    buf.extend_from_slice(&v.real().to_le_bytes());
                }
            });
        }
        if let Some(arr) = value.get::<Vec<Quath>>() {
            return self.write_raw_array(TypeEnum::Quath, arr.len(), |buf| {
                for v in arr {
                    let im = v.imaginary();
                    buf.extend_from_slice(&im.x.bits().to_le_bytes());
                    buf.extend_from_slice(&im.y.bits().to_le_bytes());
                    buf.extend_from_slice(&im.z.bits().to_le_bytes());
                    buf.extend_from_slice(&v.real().bits().to_le_bytes());
                }
            });
        }
        // Token arrays: stored as u32 token indices
        if let Some(arr) = value.get::<Vec<Token>>() {
            let indices: Vec<u32> = arr.iter().map(|t| self.add_token(t).value()).collect();
            return self.write_raw_array(TypeEnum::Token, indices.len(), |buf| {
                for idx in &indices {
                    buf.extend_from_slice(&idx.to_le_bytes());
                }
            });
        }
        // String arrays: stored as u32 string indices
        if let Some(arr) = value.get::<Vec<String>>() {
            let indices: Vec<u32> = arr.iter().map(|s| self.add_string(s).value()).collect();
            return self.write_raw_array(TypeEnum::String, indices.len(), |buf| {
                for idx in &indices {
                    buf.extend_from_slice(&idx.to_le_bytes());
                }
            });
        }
        // AssetPath arrays: stored as u32 token indices
        if let Some(arr) = value.get::<Vec<crate::AssetPath>>() {
            let indices: Vec<u32> = arr
                .iter()
                .map(|ap| self.add_token(&Token::new(ap.get_authored_path())).value())
                .collect();
            return self.write_raw_array(TypeEnum::AssetPath, indices.len(), |buf| {
                for idx in &indices {
                    buf.extend_from_slice(&idx.to_le_bytes());
                }
            });
        }

        // ── ListOps ─────────────────────────────────────────────────────────
        use crate::list_op::{
            Int64ListOp, IntListOp, StringListOp, TokenListOp, UInt64ListOp, UIntListOp,
        };

        if let Some(op) = value.get::<TokenListOp>() {
            return self.pack_list_op_token(op);
        }
        if let Some(op) = value.get::<StringListOp>() {
            return self.pack_list_op_string(op);
        }
        if let Some(op) = value.get::<IntListOp>() {
            return self.pack_list_op_int(op);
        }
        if let Some(op) = value.get::<UIntListOp>() {
            return self.pack_list_op_uint(op);
        }
        if let Some(op) = value.get::<Int64ListOp>() {
            return self.pack_list_op_int64(op);
        }
        if let Some(op) = value.get::<UInt64ListOp>() {
            return self.pack_list_op_uint64(op);
        }
        // PathListOp (relationship targets, connection paths)
        if let Some(op) = value.get::<crate::list_op::PathListOp>() {
            return self.pack_path_list_op(op);
        }

        // ── Non-empty Dictionary ─────────────────────────────────────────────
        if let Some(dict) = value.get::<usd_vt::Dictionary>() {
            return self.pack_dictionary(dict);
        }

        // Unknown / unsupported type — silently skip
        ValueRep { data: 0 }
    }

    /// Packs a double value with inlining (P2-1) and dedup (P2-4).
    /// C++ inlines doubles exactly representable as float (crateValueInliners.h:44-55).
    fn pack_double(&mut self, d: f64) -> ValueRep {
        let f = d as f32;
        if f as f64 == d {
            // Exactly representable as float — inline as f32 bits
            ValueRep::new_inlined(TypeEnum::Double, f.to_bits())
        } else {
            self.write_deduped(TypeEnum::Double, &d.to_le_bytes())
        }
    }

    // =========================================================================
    // Helper: write raw array (count + bytes) and return ValueRep
    // =========================================================================

    /// Writes an array value: align(8), u64 count, then element bytes produced by `write_fn`.
    /// Returns `ValueRep::new_array_at_offset` for non-empty arrays.
    /// For empty arrays returns a zero-payload array rep (matches C++ CrateWriter).
    fn write_raw_array<F>(&mut self, type_enum: TypeEnum, count: usize, write_fn: F) -> ValueRep
    where
        F: FnOnce(&mut Vec<u8>),
    {
        if count == 0 {
            // Empty array: IS_ARRAY set, payload = 0
            return ValueRep::new_array_at_offset(type_enum, 0);
        }
        // Collect element bytes first so we can dedup the whole block
        let mut elem_bytes: Vec<u8> = Vec::new();
        write_fn(&mut elem_bytes);

        // Align to 8, write u64 count header + element bytes
        self.align(8);
        let offset = self.tell();
        self.write_u64(count as u64);
        self.buffer.extend_from_slice(&elem_bytes);
        ValueRep::new_array_at_offset(type_enum, offset)
    }

    // =========================================================================
    // Helper: byte encoders for scalars/matrices/vectors
    // =========================================================================

    // =========================================================================
    // Helper: Dictionary packing
    // =========================================================================

    /// Packs a non-empty Dictionary.
    /// Format: align(8), u64 count, then for each (key, value):
    ///   u32 key_string_index + ValueRep (8 bytes).
    fn pack_dictionary(&mut self, dict: &usd_vt::Dictionary) -> ValueRep {
        if dict.is_empty() {
            return ValueRep::new_inlined(TypeEnum::Dictionary, 0);
        }
        self.align(8);
        let offset = self.tell();
        self.write_u64(dict.len() as u64);
        // Collect pairs to avoid borrow conflict with self
        let pairs: Vec<(String, usd_vt::Value)> =
            dict.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (key, val) in pairs {
            let str_idx = self.add_string(&key);
            self.buffer
                .extend_from_slice(&str_idx.value().to_le_bytes());
            let rep = self.pack_value(&val);
            self.buffer.extend_from_slice(&rep.data.to_le_bytes());
        }
        ValueRep::new_at_offset(TypeEnum::Dictionary, offset)
    }

    // =========================================================================
    // Helper: ListOp packing (TokenListOp, StringListOp, Int*, UInt*, etc.)
    // =========================================================================
    //
    // Binary layout matches C++ CrateWriter::_PackListOp:
    //   u8 header (bitmask of which lists are present, bit 0 = explicit flag)
    //   for each list present (in order: explicit/prepended/appended/deleted/ordered):
    //     write as token/string/int array

    /// Bitmask bits for ListOp header byte.
    const LIST_OP_IS_EXPLICIT: u8 = 0x01;
    const LIST_OP_HAS_EXPLICIT: u8 = 0x02;
    const LIST_OP_HAS_PREPENDED: u8 = 0x04;
    const LIST_OP_HAS_APPENDED: u8 = 0x08;
    const LIST_OP_HAS_DELETED: u8 = 0x10;
    const LIST_OP_HAS_ORDERED: u8 = 0x20;

    fn list_op_header(
        op_is_explicit: bool,
        n_explicit: usize,
        n_prepended: usize,
        n_appended: usize,
        n_deleted: usize,
        n_ordered: usize,
    ) -> u8 {
        let mut h = 0u8;
        if op_is_explicit {
            h |= Self::LIST_OP_IS_EXPLICIT;
        }
        if n_explicit > 0 {
            h |= Self::LIST_OP_HAS_EXPLICIT;
        }
        if n_prepended > 0 {
            h |= Self::LIST_OP_HAS_PREPENDED;
        }
        if n_appended > 0 {
            h |= Self::LIST_OP_HAS_APPENDED;
        }
        if n_deleted > 0 {
            h |= Self::LIST_OP_HAS_DELETED;
        }
        if n_ordered > 0 {
            h |= Self::LIST_OP_HAS_ORDERED;
        }
        h
    }

    fn pack_list_op_token(&mut self, op: &crate::list_op::TokenListOp) -> ValueRep {
        let explicit_indices: Vec<u32> = op
            .get_explicit_items()
            .iter()
            .map(|t| self.add_token(t).value())
            .collect();
        let prepended_indices: Vec<u32> = op
            .get_prepended_items()
            .iter()
            .map(|t| self.add_token(t).value())
            .collect();
        let appended_indices: Vec<u32> = op
            .get_appended_items()
            .iter()
            .map(|t| self.add_token(t).value())
            .collect();
        let deleted_indices: Vec<u32> = op
            .get_deleted_items()
            .iter()
            .map(|t| self.add_token(t).value())
            .collect();
        let ordered_indices: Vec<u32> = op
            .get_ordered_items()
            .iter()
            .map(|t| self.add_token(t).value())
            .collect();

        self.align(8);
        let offset = self.tell();
        let hdr = Self::list_op_header(
            op.is_explicit(),
            explicit_indices.len(),
            prepended_indices.len(),
            appended_indices.len(),
            deleted_indices.len(),
            ordered_indices.len(),
        );
        self.buffer.push(hdr);
        // Pad to 8-byte alignment before first list
        self.align(8);
        for slice in [
            &explicit_indices,
            &prepended_indices,
            &appended_indices,
            &deleted_indices,
            &ordered_indices,
        ] {
            if !slice.is_empty() {
                self.align(8);
                self.write_u64(slice.len() as u64);
                for &idx in slice.iter() {
                    self.buffer.extend_from_slice(&idx.to_le_bytes());
                }
            }
        }
        ValueRep::new_at_offset(TypeEnum::TokenListOp, offset)
    }

    fn pack_list_op_string(&mut self, op: &crate::list_op::StringListOp) -> ValueRep {
        let explicit_idx: Vec<u32> = op
            .get_explicit_items()
            .iter()
            .map(|s| self.add_string(s).value())
            .collect();
        let prepended_idx: Vec<u32> = op
            .get_prepended_items()
            .iter()
            .map(|s| self.add_string(s).value())
            .collect();
        let appended_idx: Vec<u32> = op
            .get_appended_items()
            .iter()
            .map(|s| self.add_string(s).value())
            .collect();
        let deleted_idx: Vec<u32> = op
            .get_deleted_items()
            .iter()
            .map(|s| self.add_string(s).value())
            .collect();
        let ordered_idx: Vec<u32> = op
            .get_ordered_items()
            .iter()
            .map(|s| self.add_string(s).value())
            .collect();

        self.align(8);
        let offset = self.tell();
        let hdr = Self::list_op_header(
            op.is_explicit(),
            explicit_idx.len(),
            prepended_idx.len(),
            appended_idx.len(),
            deleted_idx.len(),
            ordered_idx.len(),
        );
        self.buffer.push(hdr);
        self.align(8);
        for slice in [
            &explicit_idx,
            &prepended_idx,
            &appended_idx,
            &deleted_idx,
            &ordered_idx,
        ] {
            if !slice.is_empty() {
                self.align(8);
                self.write_u64(slice.len() as u64);
                for &idx in slice.iter() {
                    self.buffer.extend_from_slice(&idx.to_le_bytes());
                }
            }
        }
        ValueRep::new_at_offset(TypeEnum::StringListOp, offset)
    }

    /// Generic helper for integer ListOps (i32, u32, i64, u64).
    fn pack_list_op_ints<T, ToBytes>(
        &mut self,
        type_enum: TypeEnum,
        is_explicit: bool,
        explicit: &[T],
        prepended: &[T],
        appended: &[T],
        deleted: &[T],
        ordered: &[T],
        to_bytes: ToBytes,
    ) -> ValueRep
    where
        ToBytes: Fn(&T) -> Vec<u8>,
    {
        self.align(8);
        let offset = self.tell();
        let hdr = Self::list_op_header(
            is_explicit,
            explicit.len(),
            prepended.len(),
            appended.len(),
            deleted.len(),
            ordered.len(),
        );
        self.buffer.push(hdr);
        self.align(8);
        for slice in [explicit, prepended, appended, deleted, ordered] {
            if !slice.is_empty() {
                self.align(8);
                self.write_u64(slice.len() as u64);
                for v in slice {
                    self.buffer.extend_from_slice(&to_bytes(v));
                }
            }
        }
        ValueRep::new_at_offset(type_enum, offset)
    }

    fn pack_list_op_int(&mut self, op: &crate::list_op::IntListOp) -> ValueRep {
        self.pack_list_op_ints(
            TypeEnum::IntListOp,
            op.is_explicit(),
            op.get_explicit_items(),
            op.get_prepended_items(),
            op.get_appended_items(),
            op.get_deleted_items(),
            op.get_ordered_items(),
            |v| v.to_le_bytes().to_vec(),
        )
    }
    fn pack_list_op_uint(&mut self, op: &crate::list_op::UIntListOp) -> ValueRep {
        self.pack_list_op_ints(
            TypeEnum::UIntListOp,
            op.is_explicit(),
            op.get_explicit_items(),
            op.get_prepended_items(),
            op.get_appended_items(),
            op.get_deleted_items(),
            op.get_ordered_items(),
            |v| v.to_le_bytes().to_vec(),
        )
    }
    fn pack_list_op_int64(&mut self, op: &crate::list_op::Int64ListOp) -> ValueRep {
        self.pack_list_op_ints(
            TypeEnum::Int64ListOp,
            op.is_explicit(),
            op.get_explicit_items(),
            op.get_prepended_items(),
            op.get_appended_items(),
            op.get_deleted_items(),
            op.get_ordered_items(),
            |v| v.to_le_bytes().to_vec(),
        )
    }
    fn pack_list_op_uint64(&mut self, op: &crate::list_op::UInt64ListOp) -> ValueRep {
        self.pack_list_op_ints(
            TypeEnum::UInt64ListOp,
            op.is_explicit(),
            op.get_explicit_items(),
            op.get_prepended_items(),
            op.get_appended_items(),
            op.get_deleted_items(),
            op.get_ordered_items(),
            |v| v.to_le_bytes().to_vec(),
        )
    }

    /// Packs a PathListOp: paths are stored as path indices (u32).
    fn pack_path_list_op(&mut self, op: &crate::list_op::PathListOp) -> ValueRep {
        let explicit_idx: Vec<u32> = op
            .get_explicit_items()
            .iter()
            .map(|p| self.add_path(p).value())
            .collect();
        let prepended_idx: Vec<u32> = op
            .get_prepended_items()
            .iter()
            .map(|p| self.add_path(p).value())
            .collect();
        let appended_idx: Vec<u32> = op
            .get_appended_items()
            .iter()
            .map(|p| self.add_path(p).value())
            .collect();
        let deleted_idx: Vec<u32> = op
            .get_deleted_items()
            .iter()
            .map(|p| self.add_path(p).value())
            .collect();
        let ordered_idx: Vec<u32> = op
            .get_ordered_items()
            .iter()
            .map(|p| self.add_path(p).value())
            .collect();

        self.align(8);
        let offset = self.tell();
        let hdr = Self::list_op_header(
            op.is_explicit(),
            explicit_idx.len(),
            prepended_idx.len(),
            appended_idx.len(),
            deleted_idx.len(),
            ordered_idx.len(),
        );
        self.buffer.push(hdr);
        self.align(8);
        for slice in [
            &explicit_idx,
            &prepended_idx,
            &appended_idx,
            &deleted_idx,
            &ordered_idx,
        ] {
            if !slice.is_empty() {
                self.align(8);
                self.write_u64(slice.len() as u64);
                for &idx in slice.iter() {
                    self.buffer.extend_from_slice(&idx.to_le_bytes());
                }
            }
        }
        ValueRep::new_at_offset(TypeEnum::PathListOp, offset)
    }

    // =========================================================================
    // TimeSamples packing — called from add_property_spec
    // =========================================================================

    /// Packs attribute time samples.
    ///
    /// C++ on-disk layout (matches `Write(TimeSamples)` + `_RecursiveWrite`):
    ///   int64  jump1          — forward skip to timesRep (= size of times blob)
    ///   [times blob]          — raw f64 array data pointed to by timesRep.payload
    ///   ValueRep timesRep     — 8-byte ref with absolute offset to times blob
    ///   int64  jump2          — forward skip to numValues (= size of value blobs)
    ///   [value blobs]         — per-sample raw data (each pointed to by sample ValueReps)
    ///   uint64 numValues      — sample count
    ///   ValueRep[numValues]   — per-sample references (payload = absolute file offset)
    pub(crate) fn pack_time_samples(
        &mut self,
        samples: &std::collections::HashMap<crate::attribute_spec::OrderedFloat, usd_vt::Value>,
    ) -> ValueRep {
        // Collect + sort by time
        let mut sorted: Vec<(f64, usd_vt::Value)> = samples
            .iter()
            .map(|(t, v)| (t.value(), v.clone()))
            .collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        self.align(8);
        let start_offset = self.tell();

        // --- Block 1: jump1 placeholder + times blob ---
        let jump1_pos = self.buffer.len();
        self.write_u64(0); // i64 placeholder for jump1

        // Write times array blob (write_raw_array appends to buffer and returns ValueRep ref).
        let times: Vec<f64> = sorted.iter().map(|(t, _)| *t).collect();
        let times_rep = self.write_raw_array(TypeEnum::Double, times.len(), |buf| {
            for &t in &times {
                buf.extend_from_slice(&t.to_le_bytes());
            }
        });

        // Patch jump1: distance from byte immediately after jump1 field to here
        // (i.e. the size of the times blob).
        // C++ convention: jump = end - start_of_jump_field (includes 8-byte field)
        let jump1_dist = (self.buffer.len() - jump1_pos) as i64;
        self.buffer[jump1_pos..jump1_pos + 8].copy_from_slice(&jump1_dist.to_le_bytes());

        // Write timesRep reference (absolute offset; payload set by write_raw_array above).
        self.buffer.extend_from_slice(&times_rep.data.to_le_bytes());

        // --- Block 2: jump2 placeholder + value blobs ---
        let jump2_pos = self.buffer.len();
        self.write_u64(0); // i64 placeholder for jump2

        // pack_value writes each sample blob to the buffer and returns its ValueRep.
        let sample_vals: Vec<usd_vt::Value> = sorted.into_iter().map(|(_, v)| v).collect();
        let sample_reps: Vec<ValueRep> = sample_vals.iter().map(|v| self.pack_value(v)).collect();

        // Patch jump2: distance from byte after jump2 field to here (= size of value blobs).
        // C++ convention: jump = end - start_of_jump_field (includes 8-byte field)
        let jump2_dist = (self.buffer.len() - jump2_pos) as i64;
        self.buffer[jump2_pos..jump2_pos + 8].copy_from_slice(&jump2_dist.to_le_bytes());

        // Write numValues + per-sample ValueRep array.
        self.write_u64(sample_reps.len() as u64);
        for rep in &sample_reps {
            self.buffer.extend_from_slice(&rep.data.to_le_bytes());
        }

        ValueRep::new_at_offset(TypeEnum::TimeSamples, start_offset)
    }

    /// Populates the writer from a Layer.
    pub fn populate_from_layer(&mut self, layer: &Layer) {
        // First pass: collect all paths and add root spec
        let root_path = Path::absolute_root();
        let root_idx = self.add_path(&root_path);

        // Add pseudo-root spec for layer
        let mut root_fields = Vec::new();

        // Add layer metadata fields
        let doc = layer.documentation();
        if !doc.is_empty() {
            let token_idx = self.add_token(&Token::new("documentation"));
            let string_idx = self.add_string(&doc);
            let rep = ValueRep::new_inlined(TypeEnum::String, string_idx.value());
            let field_idx = self.add_field(token_idx, rep);
            root_fields.push(field_idx);
        }

        // Add default prim if set
        let default_prim = layer.default_prim();
        if !default_prim.is_empty() {
            let token_idx = self.add_token(&Token::new("defaultPrim"));
            let prim_token_idx = self.add_token(&default_prim);
            let rep = ValueRep::new_inlined(TypeEnum::Token, prim_token_idx.value());
            let field_idx = self.add_field(token_idx, rep);
            root_fields.push(field_idx);
        }

        // Write startTimeCode if set (P1-12) — uses double inlining + dedup
        if layer.has_start_time_code() {
            let token_idx = self.add_token(&Token::new("startTimeCode"));
            let rep = self.pack_double(layer.start_time_code());
            let field_idx = self.add_field(token_idx, rep);
            root_fields.push(field_idx);
        }

        // Write endTimeCode if set (P1-12) — uses double inlining + dedup
        if layer.has_end_time_code() {
            let token_idx = self.add_token(&Token::new("endTimeCode"));
            let rep = self.pack_double(layer.end_time_code());
            let field_idx = self.add_field(token_idx, rep);
            root_fields.push(field_idx);
        }

        // Write upAxis if set (P1-12) — stored as Token on pseudo-root
        {
            let up_axis_token = Token::new("upAxis");
            if let Some(val) = layer.get_field(&root_path, &up_axis_token) {
                if let Some(tok) = val.get::<Token>() {
                    let token_idx = self.add_token(&Token::new("upAxis"));
                    let val_token_idx = self.add_token(tok);
                    let rep = ValueRep::new_inlined(TypeEnum::Token, val_token_idx.value());
                    let field_idx = self.add_field(token_idx, rep);
                    root_fields.push(field_idx);
                }
            }
        }

        // Write metersPerUnit if set (P1-12) — stored as f64 on pseudo-root
        {
            let mpu_token = Token::new("metersPerUnit");
            if let Some(val) = layer.get_field(&root_path, &mpu_token) {
                if let Some(&d) = val.get::<f64>() {
                    let token_idx = self.add_token(&Token::new("metersPerUnit"));
                    let rep = self.pack_double(d);
                    let field_idx = self.add_field(token_idx, rep);
                    root_fields.push(field_idx);
                }
            }
        }

        // Write subLayers if set (P1-12) — stored as StringVector on pseudo-root
        // C++ format: u64 count + per-string u32 StringIndex (not inline bytes)
        {
            let sublayers = layer.sublayer_paths();
            if !sublayers.is_empty() {
                let token_idx = self.add_token(&Token::new("subLayers"));
                // Pre-register all sublayer strings in the string table
                let str_indices: Vec<u32> = sublayers
                    .iter()
                    .map(|s| self.add_string(s).value())
                    .collect();
                self.align(8); // P2-5: align array data to 8 bytes
                let offset = self.tell();
                self.write_u64(str_indices.len() as u64);
                for idx in &str_indices {
                    self.buffer.extend_from_slice(&idx.to_le_bytes());
                }
                let rep = ValueRep::new_at_offset(TypeEnum::StringVector, offset);
                let field_idx = self.add_field(token_idx, rep);
                root_fields.push(field_idx);
            }
        }

        // Add primChildren for root prims
        let root_prim_specs = layer.root_prims();
        let root_prim_names: Vec<String> = root_prim_specs.iter().map(|ps| ps.name()).collect();
        if !root_prim_names.is_empty() {
            let token_idx = self.add_token(&Token::new("primChildren"));
            // For primChildren, store as token list
            // Write count + token indices to file
            self.align(8); // P2-5: align array data to 8 bytes
            let offset = self.tell();
            self.write_u64(root_prim_names.len() as u64);
            for prim_name in &root_prim_names {
                let prim_token_idx = self.add_token(&Token::new(prim_name));
                self.buffer
                    .extend_from_slice(&prim_token_idx.value().to_le_bytes());
            }
            let rep = ValueRep::new_at_offset(TypeEnum::TokenVector, offset);
            let field_idx = self.add_field(token_idx, rep);
            root_fields.push(field_idx);
        }

        let root_field_set_idx = self.add_field_set(&root_fields);
        self.add_spec(root_idx, SpecType::PseudoRoot, root_field_set_idx);

        // Add prim specs recursively
        for prim_name in &root_prim_names {
            if let Some(prim_path) = Path::from_string(&format!("/{}", prim_name)) {
                self.add_prim_spec_recursive(layer, &prim_path);
            }
        }
    }

    /// Recursively adds a prim spec and its children.
    fn add_prim_spec_recursive(&mut self, layer: &Layer, path: &Path) {
        if let Some(spec) = layer.get_prim_at_path(path) {
            let path_idx = self.add_path(path);
            let mut fields = Vec::new();

            // Specifier field
            let spec_token_idx = self.add_token(&Token::new("specifier"));
            let specifier = spec.specifier();
            let spec_rep = ValueRep::new_inlined(TypeEnum::Specifier, specifier as u32);
            let spec_field_idx = self.add_field(spec_token_idx, spec_rep);
            fields.push(spec_field_idx);

            // TypeName field if present
            let type_name = spec.type_name();
            if !type_name.is_empty() {
                let token_idx = self.add_token(&Token::new("typeName"));
                let type_token_idx = self.add_token(&type_name);
                let rep = ValueRep::new_inlined(TypeEnum::Token, type_token_idx.value());
                let field_idx = self.add_field(token_idx, rep);
                fields.push(field_idx);
            }

            // Prim metadata fields (P2): documentation, kind, active, hidden, customData, apiSchemas
            for field_name in &["documentation", "kind", "active", "hidden", "instanceable"] {
                let field_token = Token::new(field_name);
                if let Some(val) = layer.get_field(path, &field_token) {
                    let token_idx = self.add_token(&field_token);
                    let rep = self.pack_value(&val);
                    if rep.data != 0 {
                        let field_idx = self.add_field(token_idx, rep);
                        fields.push(field_idx);
                    }
                }
            }
            // customData dictionary
            {
                let cd_token = Token::new("customData");
                if let Some(val) = layer.get_field(path, &cd_token) {
                    let token_idx = self.add_token(&cd_token);
                    let rep = self.pack_value(&val);
                    if rep.data != 0 {
                        let field_idx = self.add_field(token_idx, rep);
                        fields.push(field_idx);
                    }
                }
            }
            // apiSchemas (TokenListOp)
            {
                let api_token = Token::new("apiSchemas");
                if let Some(val) = layer.get_field(path, &api_token) {
                    let token_idx = self.add_token(&api_token);
                    let rep = self.pack_value(&val);
                    if rep.data != 0 {
                        let field_idx = self.add_field(token_idx, rep);
                        fields.push(field_idx);
                    }
                }
            }

            // Properties - props is Vec<PropertySpec>
            let props = spec.properties();
            let prop_names: Vec<String> = props
                .iter()
                .map(|p| p.name().get_text().to_string())
                .collect();
            if !prop_names.is_empty() {
                let token_idx = self.add_token(&Token::new("properties"));
                self.align(8); // P2-5: align array data to 8 bytes
                let offset = self.tell();
                self.write_u64(prop_names.len() as u64);
                for prop_name in &prop_names {
                    let prop_token_idx = self.add_token(&Token::new(prop_name));
                    self.buffer
                        .extend_from_slice(&prop_token_idx.value().to_le_bytes());
                }
                let rep = ValueRep::new_at_offset(TypeEnum::TokenVector, offset);
                let field_idx = self.add_field(token_idx, rep);
                fields.push(field_idx);
            }

            // Children - children is Vec<PrimSpec>
            let children = spec.name_children();
            let child_names: Vec<String> = children.iter().map(|c| c.name()).collect();
            if !child_names.is_empty() {
                let token_idx = self.add_token(&Token::new("primChildren"));
                self.align(8); // P2-5: align array data to 8 bytes
                let offset = self.tell();
                self.write_u64(child_names.len() as u64);
                for child_name in &child_names {
                    let child_token_idx = self.add_token(&Token::new(child_name));
                    self.buffer
                        .extend_from_slice(&child_token_idx.value().to_le_bytes());
                }
                let rep = ValueRep::new_at_offset(TypeEnum::TokenVector, offset);
                let field_idx = self.add_field(token_idx, rep);
                fields.push(field_idx);
            }

            let field_set_idx = self.add_field_set(&fields);
            self.add_spec(path_idx, SpecType::Prim, field_set_idx);

            // Add property specs
            for prop_name in &prop_names {
                if let Some(prop_path) = path.append_property(prop_name) {
                    self.add_property_spec(layer, &prop_path);
                }
            }

            // Recursively add children
            for child_name in &child_names {
                if let Some(child_path) = path.append_child(child_name) {
                    self.add_prim_spec_recursive(layer, &child_path);
                }
            }
        }
    }

    /// Adds a property spec.
    fn add_property_spec(&mut self, layer: &Layer, path: &Path) {
        if let Some(attr_spec) = layer.get_attribute_at_path(path) {
            let path_idx = self.add_path(path);
            let mut fields = Vec::new();

            // TypeName for attribute
            let type_name = attr_spec.type_name();
            if !type_name.is_empty() {
                let token_idx = self.add_token(&Token::new("typeName"));
                let type_token_idx = self.add_token(&Token::new(&type_name));
                let rep = ValueRep::new_inlined(TypeEnum::Token, type_token_idx.value());
                let field_idx = self.add_field(token_idx, rep);
                fields.push(field_idx);
            }

            // Variability (P1-10): write when not the default (Varying)
            let variability = attr_spec.variability();
            if variability != crate::Variability::Varying {
                let token_idx = self.add_token(&Token::new("variability"));
                let rep = ValueRep::new_inlined(TypeEnum::Variability, variability as u32);
                let field_idx = self.add_field(token_idx, rep);
                fields.push(field_idx);
            }

            // Interpolation (P1-11): write if the attribute has it set as a field
            {
                let interp_token = Token::new("interpolation");
                if let Some(interp_val) = layer.get_field(path, &interp_token) {
                    if let Some(tok) = interp_val.get::<Token>() {
                        let token_idx = self.add_token(&Token::new("interpolation"));
                        let interp_token_idx = self.add_token(tok);
                        let rep = ValueRep::new_inlined(TypeEnum::Token, interp_token_idx.value());
                        let field_idx = self.add_field(token_idx, rep);
                        fields.push(field_idx);
                    }
                }
            }

            // Default value
            if attr_spec.has_default_value() {
                let default_value = attr_spec.default_value();
                let token_idx = self.add_token(&Token::new("default"));
                let rep = self.pack_value(&default_value);
                if rep.data != 0 {
                    let field_idx = self.add_field(token_idx, rep);
                    fields.push(field_idx);
                }
            }

            // Time samples — check both the spec field (timeSamples stored in-spec)
            // and the layer's direct time_samples storage (written via set_time_sample()).
            let has_ts_in_spec = attr_spec.has_time_samples();
            let layer_times = layer.list_time_samples_for_path(path);
            if has_ts_in_spec || !layer_times.is_empty() {
                let samples: std::collections::HashMap<
                    crate::attribute_spec::OrderedFloat,
                    usd_vt::Value,
                > = if has_ts_in_spec {
                    attr_spec.time_sample_map()
                } else {
                    // Build from the layer's direct time_samples map.
                    layer_times
                        .iter()
                        .filter_map(|&t| {
                            layer
                                .query_time_sample(path, t)
                                .map(|v| (crate::attribute_spec::OrderedFloat::new(t), v))
                        })
                        .collect()
                };
                if !samples.is_empty() {
                    let token_idx = self.add_token(&Token::new("timeSamples"));
                    let rep = self.pack_time_samples(&samples);
                    let field_idx = self.add_field(token_idx, rep);
                    fields.push(field_idx);
                }
            }

            let field_set_idx = self.add_field_set(&fields);
            self.add_spec(path_idx, SpecType::Attribute, field_set_idx);
        } else if let Some(rel_spec) = layer.get_relationship_at_path(path) {
            let path_idx = self.add_path(path);
            let mut fields = Vec::new();

            // Variability — write when not default (Varying)
            let variability = rel_spec.variability();
            if variability != crate::Variability::Varying {
                let token_idx = self.add_token(&Token::new("variability"));
                let rep = ValueRep::new_inlined(TypeEnum::Variability, variability as u32);
                let field_idx = self.add_field(token_idx, rep);
                fields.push(field_idx);
            }

            // Target paths — stored as PathListOp
            if let Some(targets_val) = layer.get_field(path, &Token::new("targetPaths")) {
                if let Some(list_op) = targets_val.get::<crate::list_op::PathListOp>() {
                    let token_idx = self.add_token(&Token::new("targetPaths"));
                    let rep = self.pack_path_list_op(list_op);
                    let field_idx = self.add_field(token_idx, rep);
                    fields.push(field_idx);
                }
            }

            let field_set_idx = self.add_field_set(&fields);
            self.add_spec(path_idx, SpecType::Relationship, field_set_idx);
        }
    }

    /// Writes a u64 to the buffer.
    fn write_u64(&mut self, value: u64) {
        self.buffer.extend_from_slice(&value.to_le_bytes());
    }

    /// Writes bytes to the buffer.
    fn write_bytes(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Returns current write position.
    fn tell(&self) -> u64 {
        self.buffer.len() as u64
    }

    /// Pads buffer to next `alignment`-byte boundary (P2-5).
    fn align(&mut self, alignment: usize) {
        let pos = self.buffer.len();
        let rem = pos % alignment;
        if rem != 0 {
            let pad = alignment - rem;
            self.buffer.extend(std::iter::repeat(0u8).take(pad));
        }
    }

    /// Hashes a byte slice for value dedup (P2-4).
    fn hash_bytes(data: &[u8]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    /// Writes bytes with dedup: returns existing ValueRep if identical bytes
    /// were already written, otherwise writes and caches.
    /// Uses hash bucketing with full data comparison to avoid hash collisions.
    fn write_deduped(&mut self, type_enum: TypeEnum, data: &[u8]) -> ValueRep {
        let h = Self::hash_bytes(data);
        if let Some(bucket) = self.value_dedup.get(&h) {
            for (stored_data, rep) in bucket {
                if rep.get_type() == type_enum && stored_data == data {
                    return *rep;
                }
            }
        }
        let offset = self.tell();
        self.write_bytes(data);
        let rep = ValueRep::new_at_offset(type_enum, offset);
        self.value_dedup
            .entry(h)
            .or_default()
            .push((data.to_vec(), rep));
        rep
    }

    /// Writes tokens section (compressed for version >= 0.4.0).
    fn write_tokens(&mut self) {
        // Number of tokens
        self.write_u64(self.tokens.len() as u64);

        if self.version < (0, 4, 0) {
            // Old uncompressed format
            let mut total_bytes = 0u64;
            for t in &self.tokens {
                total_bytes += t.get_text().len() as u64 + 1;
            }
            self.write_u64(total_bytes);
            for t in &self.tokens {
                self.buffer.extend_from_slice(t.get_text().as_bytes());
                self.buffer.push(0);
            }
        } else {
            // Compressed format (version >= 0.4.0)
            let mut token_data = Vec::new();
            for t in &self.tokens {
                token_data.extend_from_slice(t.get_text().as_bytes());
                token_data.push(0);
            }
            self.write_u64(token_data.len() as u64);

            // Compress with LZ4 TfFastCompression
            match FastCompression::compress(&token_data) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    // Fallback: write uncompressed with 0 compressed size
                    self.write_u64(0);
                    self.write_bytes(&token_data);
                }
            }
        }
    }

    /// Writes strings section (just the string->token index mapping).
    fn write_strings(&mut self) {
        // Strings are stored as array of token indices
        let indices: Vec<u32> = self.strings.iter().map(|s| s.value()).collect();

        if self.version < (0, 4, 0) {
            // Old format: raw u32 array
            for idx in &indices {
                self.buffer.extend_from_slice(&idx.to_le_bytes());
            }
        } else {
            // Compressed format
            self.write_u64(indices.len() as u64);
            match crate::integer_coding::IntegerCompression::compress_u32(&indices) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    // Fallback
                    self.write_u64(0);
                    for idx in &indices {
                        self.buffer.extend_from_slice(&idx.to_le_bytes());
                    }
                }
            }
        }
    }

    /// Writes fields section (compressed for version >= 0.4.0).
    fn write_fields(&mut self) {
        if self.version < (0, 4, 0) {
            // Old format: raw Field array
            for field in &self.fields {
                self.buffer.extend_from_slice(&field.to_bytes());
            }
        } else {
            // Compressed format
            self.write_u64(self.fields.len() as u64);

            // Token indices
            let token_indices: Vec<u32> =
                self.fields.iter().map(|f| f.token_index.value()).collect();
            match crate::integer_coding::IntegerCompression::compress_u32(&token_indices) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    self.write_u64(0);
                    for idx in &token_indices {
                        self.buffer.extend_from_slice(&idx.to_le_bytes());
                    }
                }
            }

            // Value reps (u64 array, LZ4 compressed)
            let mut reps_data = Vec::with_capacity(self.fields.len() * 8);
            for field in &self.fields {
                reps_data.extend_from_slice(&field.value_rep.data.to_le_bytes());
            }
            match FastCompression::compress(&reps_data) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    self.write_u64(0);
                    self.write_bytes(&reps_data);
                }
            }
        }
    }

    /// Writes field sets section (compressed for version >= 0.4.0).
    fn write_field_sets(&mut self) {
        let field_set_vals: Vec<u32> = self.field_sets.iter().map(|f| f.value()).collect();

        if self.version < (0, 4, 0) {
            // Old format
            for val in &field_set_vals {
                self.buffer.extend_from_slice(&val.to_le_bytes());
            }
        } else {
            // Compressed format
            self.write_u64(field_set_vals.len() as u64);
            match crate::integer_coding::IntegerCompression::compress_u32(&field_set_vals) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    self.write_u64(0);
                    for val in &field_set_vals {
                        self.buffer.extend_from_slice(&val.to_le_bytes());
                    }
                }
            }
        }
    }

    /// Writes paths section (compressed for version >= 0.4.0).
    fn write_paths(&mut self) {
        self.write_u64(self.paths.len() as u64);

        if self.version < (0, 4, 0) {
            // Old uncompressed path tree format (not implemented)
            // For now, write as compressed format which is simpler
        }

        // Compressed path format: pathIndexes, elementTokenIndexes, jumps
        // Build sorted path list (excluding empty path)
        let mut path_pairs: Vec<(Path, PathIndex)> = self
            .paths
            .iter()
            .enumerate()
            .filter(|(_, p)| !p.is_empty())
            .map(|(i, p)| {
                // Index is bounded by path count which itself is bounded at add_path time
                let raw = u32::try_from(i).unwrap_or(u32::MAX);
                (p.clone(), PathIndex::new(raw))
            })
            .collect();
        path_pairs.sort_by(|a, b| a.0.cmp(&b.0));

        // Write number of encoded paths
        self.write_u64(path_pairs.len() as u64);

        // Build arrays
        let mut path_indexes = Vec::with_capacity(path_pairs.len());
        let mut element_token_indexes = Vec::with_capacity(path_pairs.len());
        let mut jumps = Vec::with_capacity(path_pairs.len());

        self.build_compressed_path_data(
            &path_pairs,
            &mut path_indexes,
            &mut element_token_indexes,
            &mut jumps,
        );

        // Write pathIndexes
        match crate::integer_coding::IntegerCompression::compress_u32(&path_indexes) {
            Ok(compressed) => {
                self.write_u64(compressed.len() as u64);
                self.write_bytes(&compressed);
            }
            Err(_) => {
                self.write_u64(0);
                for idx in &path_indexes {
                    self.buffer.extend_from_slice(&idx.to_le_bytes());
                }
            }
        }

        // Write elementTokenIndexes
        match crate::integer_coding::IntegerCompression::compress_i32(&element_token_indexes) {
            Ok(compressed) => {
                self.write_u64(compressed.len() as u64);
                self.write_bytes(&compressed);
            }
            Err(_) => {
                self.write_u64(0);
                for idx in &element_token_indexes {
                    self.buffer.extend_from_slice(&idx.to_le_bytes());
                }
            }
        }

        // Write jumps
        match crate::integer_coding::IntegerCompression::compress_i32(&jumps) {
            Ok(compressed) => {
                self.write_u64(compressed.len() as u64);
                self.write_bytes(&compressed);
            }
            Err(_) => {
                self.write_u64(0);
                for j in &jumps {
                    self.buffer.extend_from_slice(&j.to_le_bytes());
                }
            }
        }
    }

    /// Builds compressed path data arrays matching C++ _BuildCompressedPathDataRecursive.
    ///
    /// The algorithm walks sorted (path, index) pairs as a tree using
    /// getNextSubtree to skip past all descendants, exactly like the C++ impl.
    /// All path element tokens must be pre-registered in the token table
    /// (done in write() before TOKENS section is serialized).
    fn build_compressed_path_data(
        &self,
        paths: &[(Path, PathIndex)],
        path_indexes: &mut Vec<u32>,
        element_token_indexes: &mut Vec<i32>,
        jumps: &mut Vec<i32>,
    ) {
        // Pre-allocate arrays to full size (C++ resizes upfront)
        path_indexes.resize(paths.len(), 0);
        element_token_indexes.resize(paths.len(), 0);
        jumps.resize(paths.len(), 0);

        let mut cur_index: usize = 0;
        Self::build_path_tree_recursive(
            &mut cur_index,
            0,
            paths.len(),
            paths,
            path_indexes,
            element_token_indexes,
            jumps,
            &self.token_to_index,
        );
    }

    /// Recursive path tree builder matching C++ _BuildCompressedPathDataRecursive.
    ///
    /// Processes sorted path slice [start..end), filling arrays at cur_index.
    /// Returns the index past the last consumed element.
    fn build_path_tree_recursive(
        cur_index: &mut usize,
        start: usize,
        end: usize,
        paths: &[(Path, PathIndex)],
        path_indexes: &mut [u32],
        element_token_indexes: &mut [i32],
        jumps: &mut [i32],
        token_map: &std::collections::HashMap<Token, TokenIndex>,
    ) -> usize {
        // C++ getNextSubtree: advance past all paths with prefix of start path
        let get_next_subtree = |from: usize, to: usize| -> usize {
            let start_path = &paths[from].0;
            let mut i = from + 1;
            while i < to && paths[i].0.has_prefix(start_path) {
                i += 1;
            }
            i
        };

        let mut cur = start;
        while cur < end {
            let next_subtree = get_next_subtree(cur, end);
            let mut next = cur + 1;

            let has_child = next != next_subtree && paths[next].0.get_parent_path() == paths[cur].0;

            let has_sibling = next_subtree != end
                && paths[next_subtree].0.get_parent_path() == paths[cur].0.get_parent_path();

            let is_property = paths[cur].0.is_property_path();
            let element = paths[cur].0.get_name();
            let token = Token::new(element);
            // Token must already be in the table (pre-collected in write())
            let token_idx = token_map.get(&token).copied().unwrap_or(TokenIndex::new(0));

            let this_index = *cur_index;
            *cur_index += 1;

            path_indexes[this_index] = paths[cur].1.value();
            element_token_indexes[this_index] = if is_property {
                -(token_idx.value() as i32)
            } else {
                token_idx.value() as i32
            };

            // Recurse into children first (C++ order: recurse child, then set jump)
            if has_child {
                next = Self::build_path_tree_recursive(
                    cur_index,
                    next,
                    next_subtree,
                    paths,
                    path_indexes,
                    element_token_indexes,
                    jumps,
                    token_map,
                );
            }

            // Set jump value after child recursion so cur_index reflects child count
            jumps[this_index] = if has_sibling && has_child {
                (*cur_index - this_index) as i32
            } else if has_sibling {
                0
            } else if has_child {
                -1
            } else {
                -2
            };

            if !has_sibling {
                return next;
            }

            cur = next_subtree;
        }
        end
    }

    /// Writes specs section (compressed for version >= 0.4.0).
    fn write_specs(&mut self) {
        if self.version < (0, 4, 0) {
            // Old format
            for spec in &self.specs {
                self.buffer.extend_from_slice(&spec.to_bytes());
            }
        } else {
            // Compressed format
            self.write_u64(self.specs.len() as u64);

            // pathIndexes
            let path_idxs: Vec<u32> = self.specs.iter().map(|s| s.path_index.value()).collect();
            match crate::integer_coding::IntegerCompression::compress_u32(&path_idxs) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    self.write_u64(0);
                    for idx in &path_idxs {
                        self.buffer.extend_from_slice(&idx.to_le_bytes());
                    }
                }
            }

            // fieldSetIndexes
            let fset_idxs: Vec<u32> = self
                .specs
                .iter()
                .map(|s| s.field_set_index.value())
                .collect();
            match crate::integer_coding::IntegerCompression::compress_u32(&fset_idxs) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    self.write_u64(0);
                    for idx in &fset_idxs {
                        self.buffer.extend_from_slice(&idx.to_le_bytes());
                    }
                }
            }

            // specTypes
            let spec_types: Vec<u32> = self.specs.iter().map(|s| s.spec_type as u32).collect();
            match crate::integer_coding::IntegerCompression::compress_u32(&spec_types) {
                Ok(compressed) => {
                    self.write_u64(compressed.len() as u64);
                    self.write_bytes(&compressed);
                }
                Err(_) => {
                    self.write_u64(0);
                    for t in &spec_types {
                        self.buffer.extend_from_slice(&t.to_le_bytes());
                    }
                }
            }
        }
    }

    /// Writes a section and records it in the TOC.
    fn write_section<F>(&mut self, name: &str, toc: &mut TableOfContents, write_fn: F)
    where
        F: FnOnce(&mut Self),
    {
        let start = self.tell();
        write_fn(self);
        let size = self.tell() - start;
        toc.add_section(Section::new(name, start as i64, size as i64));
    }

    /// Writes the complete crate file and returns the bytes.
    ///
    /// The buffer already holds BOOTSTRAP_SIZE bytes of placeholder + any
    /// value-data written during `populate_from_layer`.  We append all sections
    /// after that value-data, build the TOC, then write the real bootstrap at
    /// the very beginning of the buffer (byte 0).
    pub fn write(&mut self) -> Vec<u8> {
        // Do NOT clear the buffer — it already contains the bootstrap placeholder
        // (bytes 0..BOOTSTRAP_SIZE) and any inline value-data written by
        // populate_from_layer.  Sections are appended after that existing content.

        let mut toc = TableOfContents::new();

        // Pre-collect all path element tokens so they are present in the TOKENS
        // section.  build_compressed_path_data() calls add_token() internally,
        // but it runs inside the PATHS write_section closure (after TOKENS is
        // already written).  Collecting here ensures every element token is in
        // the token table before TOKENS is serialised.
        let path_elements: Vec<String> = self
            .paths
            .iter()
            .filter(|p| !p.is_empty())
            .map(|p| {
                if p.is_absolute_root_path() {
                    String::new()
                } else {
                    p.get_name().to_string()
                }
            })
            .collect();
        for elem in path_elements {
            self.add_token(&Token::new(&elem));
        }

        // Write sections
        let tokens_clone = self.tokens.clone();
        self.write_section("TOKENS", &mut toc, |w| {
            w.tokens = tokens_clone;
            w.write_tokens();
        });

        let strings_clone = self.strings.clone();
        self.write_section("STRINGS", &mut toc, |w| {
            w.strings = strings_clone;
            w.write_strings();
        });

        let fields_clone = self.fields.clone();
        self.write_section("FIELDS", &mut toc, |w| {
            w.fields = fields_clone;
            w.write_fields();
        });

        let field_sets_clone = self.field_sets.clone();
        self.write_section("FIELDSETS", &mut toc, |w| {
            w.field_sets = field_sets_clone;
            w.write_field_sets();
        });

        let paths_clone = self.paths.clone();
        self.write_section("PATHS", &mut toc, |w| {
            w.paths = paths_clone;
            w.write_paths();
        });

        let specs_clone = self.specs.clone();
        self.write_section("SPECS", &mut toc, |w| {
            w.specs = specs_clone;
            w.write_specs();
        });

        // Write TOC
        let toc_offset = self.tell();
        self.write_u64(toc.sections.len() as u64);
        for section in &toc.sections {
            self.write_bytes(&section.to_bytes());
        }

        // Write bootstrap at start
        let mut boot = Bootstrap::with_version(self.version.0, self.version.1, self.version.2);
        boot.toc_offset = toc_offset as i64;
        let boot_bytes = boot.to_bytes();
        self.buffer[..BOOTSTRAP_SIZE].copy_from_slice(&boot_bytes);

        std::mem::take(&mut self.buffer)
    }
}

// =============================================================================
// Free helper functions: scalar-to-bytes encoders
// =============================================================================

/// Packs two same-type primitives to a little-endian byte array.
#[inline]
fn bytemuck_pod_2<T: Copy + ToLeBytes>(a: T, b: T) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 * std::mem::size_of::<T>());
    v.extend_from_slice(a.to_le_bytes_dyn().as_ref());
    v.extend_from_slice(b.to_le_bytes_dyn().as_ref());
    v
}

/// Packs three same-type primitives to a little-endian byte array.
#[inline]
fn bytemuck_pod_3<T: Copy + ToLeBytes>(a: T, b: T, c: T) -> Vec<u8> {
    let mut v = Vec::with_capacity(3 * std::mem::size_of::<T>());
    v.extend_from_slice(a.to_le_bytes_dyn().as_ref());
    v.extend_from_slice(b.to_le_bytes_dyn().as_ref());
    v.extend_from_slice(c.to_le_bytes_dyn().as_ref());
    v
}

/// Packs four same-type primitives to a little-endian byte array.
#[inline]
fn bytemuck_pod_4<T: Copy + ToLeBytes>(a: T, b: T, c: T, d: T) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 * std::mem::size_of::<T>());
    v.extend_from_slice(a.to_le_bytes_dyn().as_ref());
    v.extend_from_slice(b.to_le_bytes_dyn().as_ref());
    v.extend_from_slice(c.to_le_bytes_dyn().as_ref());
    v.extend_from_slice(d.to_le_bytes_dyn().as_ref());
    v
}

/// Trait to produce LE bytes dynamically from a scalar.
trait ToLeBytes {
    fn to_le_bytes_dyn(&self) -> Vec<u8>;
}
impl ToLeBytes for f32 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl ToLeBytes for f64 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl ToLeBytes for i32 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl ToLeBytes for u32 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl ToLeBytes for i64 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl ToLeBytes for u64 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}
impl ToLeBytes for u16 {
    fn to_le_bytes_dyn(&self) -> Vec<u8> {
        self.to_le_bytes().to_vec()
    }
}

/// Serializes a 2x2 f64 matrix row-major to 32 bytes.
fn matrix_to_bytes_2x2(m: &Matrix2d) -> Vec<u8> {
    let mut v = Vec::with_capacity(32);
    for row in 0..2usize {
        for col in 0..2usize {
            v.extend_from_slice(&m[row][col].to_le_bytes());
        }
    }
    v
}

/// Serializes a 3x3 f64 matrix row-major to 72 bytes.
fn matrix_to_bytes_3x3(m: &Matrix3d) -> Vec<u8> {
    let mut v = Vec::with_capacity(72);
    for row in 0..3usize {
        for col in 0..3usize {
            v.extend_from_slice(&m[row][col].to_le_bytes());
        }
    }
    v
}

/// Serializes a 4x4 f64 matrix row-major to 128 bytes.
fn matrix_to_bytes_4x4(m: &Matrix4d) -> Vec<u8> {
    let mut v = Vec::with_capacity(128);
    for row in 0..4usize {
        for col in 0..4usize {
            v.extend_from_slice(&m[row][col].to_le_bytes());
        }
    }
    v
}
