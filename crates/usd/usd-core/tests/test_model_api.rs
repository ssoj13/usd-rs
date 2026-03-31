//! Tests for UsdModelAPI (kind, model/group hierarchy).
//!
//! Ported from:
//!   - pxr/usd/usd/testenv/testUsdModel.py (core subset)

mod common;

use usd_core::model_api::KindValidation;
use usd_core::{InitialLoadSet, ModelAPI, Stage};
use usd_tf::Token;

// ============================================================================
// GetKind / SetKind
// ============================================================================

#[test]
fn model_get_set_kind() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/MyModel", "").expect("define prim");

    // Initially no kind
    assert_eq!(prim.get_kind(), None, "kind should be unset initially");

    // Set kind to "component"
    let component = Token::new("component");
    assert!(prim.set_kind(&component));
    assert_eq!(
        prim.get_kind().as_ref().map(|t| t.as_str()),
        Some("component")
    );

    // Set kind to "group"
    let group = Token::new("group");
    assert!(prim.set_kind(&group));
    assert_eq!(prim.get_kind().as_ref().map(|t| t.as_str()), Some("group"));

    // Set kind to "assembly"
    let assembly = Token::new("assembly");
    assert!(prim.set_kind(&assembly));
    assert_eq!(
        prim.get_kind().as_ref().map(|t| t.as_str()),
        Some("assembly")
    );
}

// ============================================================================
// IsModel / IsGroup
// ============================================================================

#[test]
fn model_is_model_group() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");

    // Component is a model but not a group
    let comp = stage
        .define_prim("/Component", "")
        .expect("define component");
    comp.set_kind(&Token::new("component"));
    assert!(comp.is_model(), "component should be a model");
    assert!(!comp.is_group(), "component should NOT be a group");

    // Group is both a model and a group
    let grp = stage.define_prim("/Group", "").expect("define group");
    grp.set_kind(&Token::new("group"));
    assert!(grp.is_model(), "group should be a model");
    assert!(grp.is_group(), "group should be a group");

    // Assembly is both a model and a group
    let asm = stage.define_prim("/Assembly", "").expect("define assembly");
    asm.set_kind(&Token::new("assembly"));
    assert!(asm.is_model(), "assembly should be a model");
    assert!(asm.is_group(), "assembly should be a group");

    // No kind — not a model, not a group
    let plain = stage.define_prim("/Plain", "").expect("define plain");
    assert!(!plain.is_model(), "prim without kind should NOT be a model");
    assert!(!plain.is_group(), "prim without kind should NOT be a group");
}

// ============================================================================
// Kind via metadata
// ============================================================================

#[test]
fn model_kind_metadata() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/Test", "").expect("define /Test");

    // Set kind via metadata
    let kind_key = Token::new("kind");
    assert!(prim.set_metadata(&kind_key, "component"));

    // Read back via API
    assert_eq!(
        prim.get_kind().as_ref().map(|t| t.as_str()),
        Some("component")
    );
}

// ============================================================================
// test_ModelHierarchy — model hierarchy validation with parent-child kind rules
// Ported from testUsdModel.py::test_ModelHierarchy
// ============================================================================

#[test]
fn model_hierarchy() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let x = stage.define_prim("/X", "Scope").expect("define /X");
    let y = stage.define_prim("/X/Y", "Scope").expect("define /X/Y");
    let z = stage.define_prim("/X/Y/Z", "Scope").expect("define /X/Y/Z");

    assert!(!x.is_model());
    assert!(!y.is_model());
    assert!(!z.is_model());

    let xm = ModelAPI::new(x.clone());
    let ym = ModelAPI::new(y.clone());
    let zm = ModelAPI::new(z.clone());

    let component = Token::new("component");
    let model_tok = Token::new("model");
    let group = Token::new("group");
    let assembly = Token::new("assembly");
    let subcomponent = Token::new("subcomponent");

    // X is not a model, so Y can't be a model even with kind=component
    ym.set_kind(&component);
    assert!(!ym.is_model());
    assert!(!ym.is_kind(&component, KindValidation::ModelHierarchy));
    assert!(!ym.is_kind(&model_tok, KindValidation::ModelHierarchy));
    // But without hierarchy validation, it IS a component
    assert!(ym.is_kind(&component, KindValidation::None));
    assert!(ym.is_kind(&model_tok, KindValidation::None));

    // Setting X to component: X is a model, Y still NOT (component under component)
    xm.set_kind(&component);
    assert!(xm.is_model());
    assert!(!ym.is_model());
    assert!(xm.is_kind(&component, KindValidation::ModelHierarchy));
    assert!(xm.is_kind(&model_tok, KindValidation::ModelHierarchy));
    assert!(!ym.is_kind(&component, KindValidation::ModelHierarchy));
    assert!(!ym.is_kind(&model_tok, KindValidation::ModelHierarchy));
    assert!(ym.is_kind(&component, KindValidation::None));
    assert!(ym.is_kind(&model_tok, KindValidation::None));

    // Setting X to assembly: X is group, Y is component model
    xm.set_kind(&assembly);
    assert!(xm.is_model());
    assert!(xm.is_group());
    assert!(ym.is_model());
    assert!(!ym.is_group());
    assert!(xm.is_kind(&assembly, KindValidation::ModelHierarchy));
    assert!(xm.is_kind(&group, KindValidation::ModelHierarchy));
    assert!(ym.is_kind(&component, KindValidation::ModelHierarchy));
    assert!(ym.is_kind(&model_tok, KindValidation::ModelHierarchy));

    // Z under component Y: component below component violates hierarchy
    zm.set_kind(&component);
    assert!(!zm.is_model());

    // Subcomponent is not a model
    zm.set_kind(&subcomponent);
    assert!(!zm.is_model());
    assert!(zm.is_kind(&subcomponent, KindValidation::ModelHierarchy));
    assert!(zm.is_kind(&subcomponent, KindValidation::None));
}

// ============================================================================
// test_AssetInfo — asset metadata through ModelAPI
// Ported from testUsdModel.py::test_AssetInfo
// ============================================================================

#[test]
fn model_asset_info() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/World", "Xform").expect("define /World");
    let model = ModelAPI::new(prim);

    // Initially empty asset info
    let info = model.get_asset_info();
    assert!(
        info.is_none() || info.as_ref().map_or(false, |m| m.is_empty()),
        "asset info should be empty initially"
    );

    // Set and read back asset name
    model.set_asset_name("PaperCup");
    assert_eq!(model.get_asset_name().as_deref(), Some("PaperCup"));

    // Set and read back asset version
    model.set_asset_version("10a");
    assert_eq!(model.get_asset_version().as_deref(), Some("10a"));

    // Set and read back asset identifier
    let asset_id = usd_sdf::AssetPath::new("PaperCup/usd/PaperCup.usd");
    model.set_asset_identifier(&asset_id);
    let got_id = model.get_asset_identifier();
    assert!(got_id.is_some(), "asset identifier should be set");

    // Asset info should now contain all the fields
    let info = model.get_asset_info();
    assert!(
        info.is_some(),
        "asset info should be non-empty after setting fields"
    );
    let info = info.expect("asset info");
    assert!(info.contains_key("name"), "asset info should have 'name'");
    assert!(
        info.contains_key("version"),
        "asset info should have 'version'"
    );
    assert!(
        info.contains_key("identifier"),
        "asset info should have 'identifier'"
    );
}

// ============================================================================
// test_ModelAPI — basic ModelAPI construction and kind from another schema
// Ported from testUsdModel.py::test_ModelAPI
// ============================================================================

#[test]
fn model_api_basic() {
    common::setup();

    let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).expect("create stage");
    let prim = stage.define_prim("/World", "Xform").expect("define /World");
    let model = ModelAPI::new(prim.clone());

    // No kind initially
    assert_eq!(model.get_kind(), None);
    assert!(!model.is_model());
    assert!(!model.is_group());

    // Set kind to group
    model.set_kind(&Token::new("group"));

    // Create new ModelAPI from same prim — should see same kind
    let model2 = ModelAPI::new(prim);
    assert_eq!(model.get_kind(), model2.get_kind());
}
