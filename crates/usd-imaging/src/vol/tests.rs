//! Tests for usd_imaging::vol module.
//!
//! These tests verify the basic functionality of volume imaging adapters.

use super::*;
use usd_tf::Token;

#[test]
fn test_tokens_exist() {
    // Verify tokens are accessible
    let field3d = &USD_VOL_IMAGING_TOKENS.field3d_asset;
    let openvdb = &USD_VOL_IMAGING_TOKENS.openvdb_asset;

    assert_eq!(field3d.as_str(), "field3dAsset");
    assert_eq!(openvdb.as_str(), "openvdbAsset");
}

#[test]
fn test_field3d_adapter_creation() {
    let adapter = Field3DAssetAdapter::new();
    let prim_type = adapter.get_prim_type_token();

    assert_eq!(prim_type.as_str(), "Field3DAsset");
}

#[test]
fn test_openvdb_adapter_creation() {
    let adapter = OpenVDBAssetAdapter::new();
    let prim_type = adapter.get_prim_type_token();

    assert_eq!(prim_type.as_str(), "OpenVDBAsset");
}

#[test]
fn test_field3d_adapter_default() {
    let adapter = Field3DAssetAdapter::default();
    let prim_type = adapter.get_prim_type_token();

    assert_eq!(prim_type.as_str(), "Field3DAsset");
}

#[test]
fn test_openvdb_adapter_default() {
    let adapter = OpenVDBAssetAdapter::default();
    let prim_type = adapter.get_prim_type_token();

    assert_eq!(prim_type.as_str(), "OpenVDBAsset");
}

#[test]
fn test_field3d_subprim_type() {
    use usd_core::Prim;

    let adapter = Field3DAssetAdapter::new();
    let prim = Prim::invalid();
    let empty_token = Token::new("");

    let subprim_type = adapter.get_imaging_subprim_type(&prim, &empty_token);
    assert!(subprim_type.is_some());
    assert_eq!(subprim_type.expect("has subprim").as_str(), "field3dAsset");
}

#[test]
fn test_openvdb_subprim_type() {
    use usd_core::Prim;

    let adapter = OpenVDBAssetAdapter::new();
    let prim = Prim::invalid();
    let empty_token = Token::new("");

    let subprim_type = adapter.get_imaging_subprim_type(&prim, &empty_token);
    assert!(subprim_type.is_some());
    assert_eq!(subprim_type.expect("has subprim").as_str(), "openvdbAsset");
}

#[test]
fn test_field3d_imaging_subprims() {
    use usd_core::Prim;

    let adapter = Field3DAssetAdapter::new();
    let prim = Prim::invalid();

    let subprims = adapter.get_imaging_subprims(&prim);
    assert_eq!(subprims.len(), 1);
    assert!(subprims[0].is_empty());
}

#[test]
fn test_openvdb_imaging_subprims() {
    use usd_core::Prim;

    let adapter = OpenVDBAssetAdapter::new();
    let prim = Prim::invalid();

    let subprims = adapter.get_imaging_subprims(&prim);
    assert_eq!(subprims.len(), 1);
    assert!(subprims[0].is_empty());
}

#[test]
fn test_property_invalidation_type() {
    use crate::types::PropertyInvalidationType;
    let resync = PropertyInvalidationType::Resync;
    let changed = PropertyInvalidationType::PropertyChanged;

    assert_ne!(resync, changed);
    assert_eq!(resync, PropertyInvalidationType::Resync);
}
