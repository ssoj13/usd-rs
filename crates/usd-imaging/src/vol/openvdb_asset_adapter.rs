//! Adapter for OpenVDBAsset field primitives.
//!
//! Handles imaging translation for USD OpenVDBAsset prims into Hydra
//! volume field representations.
//!
//! # C++ Reference
//!
//! Port of `pxr/usdImaging/usdVolImaging/openvdbAssetAdapter.h`

use std::sync::Arc;

use crate::data_source_stage_globals::NoOpStageGlobals;
use usd_core::Prim;
use usd_hd::data_source::HdContainerDataSourceHandle;
use usd_sdf::{Path, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

use super::data_source_field_asset::DataSourceFieldAssetPrim;
use super::field_adapter::{DataSourceLocatorSet, FieldAdapter, PropertyInvalidationType};
use super::tokens::USD_VOL_IMAGING_TOKENS;

/// Adapter for OpenVDBAsset field primitives.
///
/// Translates UsdVolOpenVDBAsset prims into Hydra imaging primitives,
/// providing access to OpenVDB volumetric data files.
pub struct OpenVDBAssetAdapter;

impl OpenVDBAssetAdapter {
    /// Creates a new OpenVDBAsset adapter.
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenVDBAssetAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldAdapter for OpenVDBAssetAdapter {
    fn get_imaging_subprims(&self, _prim: &Prim) -> Vec<Token> {
        // Return single empty token for the prim itself
        vec![Token::new("")]
    }

    fn get_imaging_subprim_type(&self, _prim: &Prim, subprim: &Token) -> Option<Token> {
        if subprim.is_empty() {
            Some(USD_VOL_IMAGING_TOKENS.openvdb_asset.clone())
        } else {
            None
        }
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        scene_index_path: &Path,
    ) -> Option<HdContainerDataSourceHandle> {
        if subprim.is_empty() {
            // Create default stage globals (in real use, this would come from scene index)
            let stage_globals = Arc::new(NoOpStageGlobals::default());
            Some(DataSourceFieldAssetPrim::new(
                scene_index_path.clone(),
                prim.clone(),
                stage_globals,
            ) as HdContainerDataSourceHandle)
        } else {
            None
        }
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> DataSourceLocatorSet {
        if subprim.is_empty() {
            DataSourceFieldAssetPrim::invalidate(prim, subprim, properties, invalidation_type)
        } else {
            Vec::new()
        }
    }

    fn get(&self, prim: &Prim, _cache_path: &Path, key: &Token, time: TimeCode) -> Option<Value> {
        // Handle OpenVDBAsset specific attributes
        match key.as_str() {
            "filePath" => {
                if let Some(attr) = prim.get_attribute("filePath") {
                    if let Some(value) = attr.get(time) {
                        return Some(value);
                    }
                }
                // Return default empty asset path
                Some(Value::from("".to_string())) // Empty asset path as string
            }
            "fieldName" => {
                if let Some(attr) = prim.get_attribute("fieldName") {
                    if let Some(value) = attr.get(time) {
                        return Some(value);
                    }
                }
                Some(Value::from(Token::new("")))
            }
            "fieldIndex" => {
                if let Some(attr) = prim.get_attribute("fieldIndex") {
                    if let Some(value) = attr.get(time) {
                        return Some(value);
                    }
                }
                Some(Value::from(0i32))
            }
            "fieldDataType" => {
                if let Some(attr) = prim.get_attribute("fieldDataType") {
                    if let Some(value) = attr.get(time) {
                        return Some(value);
                    }
                }
                Some(Value::from(Token::new("")))
            }
            "fieldClass" => {
                // OpenVDB specific - GRID_FOG_VOLUME, GRID_LEVEL_SET, etc.
                if let Some(attr) = prim.get_attribute("fieldClass") {
                    if let Some(value) = attr.get(time) {
                        return Some(value);
                    }
                }
                Some(Value::from(Token::new("unknown")))
            }
            "vectorDataRoleHint" => {
                if let Some(attr) = prim.get_attribute("vectorDataRoleHint") {
                    if let Some(value) = attr.get(time) {
                        return Some(value);
                    }
                }
                Some(Value::from(Token::new("None")))
            }
            _ => None,
        }
    }

    fn get_prim_type_token(&self) -> Token {
        Token::new("OpenVDBAsset")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_creation() {
        let adapter = OpenVDBAssetAdapter::new();
        assert_eq!(adapter.get_prim_type_token().as_str(), "OpenVDBAsset");
    }

    #[test]
    fn test_subprim_type() {
        let adapter = OpenVDBAssetAdapter::new();
        let prim = Prim::invalid();
        let subprim = Token::new("");

        if let Some(prim_type) = adapter.get_imaging_subprim_type(&prim, &subprim) {
            assert_eq!(prim_type.as_str(), "openvdbAsset");
        }
    }
}
