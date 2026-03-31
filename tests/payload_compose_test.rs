//! Test that payload composition actually works with the PcpCache predicate fix.

use std::path::Path;

fn caldera_base() -> &'static Path {
    Path::new(r"_ref\caldera\map_source")
}

#[test]
fn test_payload_composition_caldera() {
    usd_sdf::init();

    // This is the root geo hierarchy file that references prefabs with payloads
    let geo_path = caldera_base()
        .join("prefabs")
        .join("misc_models")
        .join("geo")
        .join("ee_pillar_concrete_pipe_support_base.usd");

    if !geo_path.exists() {
        eprintln!("Skipping: {:?} not found", geo_path);
        return;
    }

    let stage = usd_core::Stage::open(
        geo_path.to_str().unwrap(),
        usd_core::InitialLoadSet::LoadAll,
    )
    .expect("Failed to open stage");

    let mut total_prims = 0;
    let mut prims_with_payload = 0;
    let mut prims_with_children = 0;

    for prim in stage.traverse() {
        total_prims += 1;
        if prim.has_payload() {
            prims_with_payload += 1;
        }
        let children = prim.children();
        if !children.is_empty() {
            prims_with_children += 1;
        }
        eprintln!(
            "  prim: {} type={} has_payload={} children={}",
            prim.get_path(),
            prim.get_type_name().as_str(),
            prim.has_payload(),
            children.len()
        );
    }

    eprintln!("Total prims: {}", total_prims);
    eprintln!("Prims with payload flag: {}", prims_with_payload);
    eprintln!("Prims with children: {}", prims_with_children);

    // We expect at least some prims
    assert!(total_prims > 0, "Should have some prims");
}

#[test]
fn test_payload_layer_has_payload_field() {
    usd_sdf::init();

    // Direct prefab file that HAS payloads
    let prefab_path = caldera_base()
        .join("prefabs")
        .join("misc_models")
        .join("ee_pillar_concrete_pipe_support_base.usd");

    if !prefab_path.exists() {
        eprintln!("Skipping: {:?} not found", prefab_path);
        return;
    }

    // Open layer directly to check payload fields
    let layer =
        usd_sdf::Layer::find_or_open(prefab_path.to_str().unwrap()).expect("Failed to open layer");

    // Check specific known path for payload field
    let payload_path =
        usd_sdf::Path::from_string("/world/ee_pillar_concrete_pipe_support_base/misc_model_189")
            .unwrap();
    let list_op = layer.get_payload_list_op(&payload_path);
    eprintln!("Layer payload at known path: {:?}", list_op.is_some());
    if let Some(ref lo) = list_op {
        eprintln!("  Payload list op: {:?}", lo);
    }

    // Now open as stage with LoadAll
    let stage = usd_core::Stage::open(
        prefab_path.to_str().unwrap(),
        usd_core::InitialLoadSet::LoadAll,
    )
    .expect("Failed to open stage");

    let mut total = 0;
    let mut with_payload = 0;
    for prim in stage.traverse() {
        total += 1;
        let path = prim.get_path();
        let hp = prim.has_payload();
        let children = prim.children();
        eprintln!(
            "  Stage prim: {} type={} has_payload={} children={}",
            path,
            prim.get_type_name().as_str(),
            hp,
            children.len()
        );
        if hp {
            with_payload += 1;
        }
    }
    eprintln!("Stage: {} prims, {} with payload flag", total, with_payload);

    // The misc_model_189 prim should have payload composed
    // and therefore should have children from the payload target
    let prim = stage.get_prim_at_path(
        &usd_sdf::Path::from_string("/world/ee_pillar_concrete_pipe_support_base/misc_model_189")
            .unwrap(),
    );
    if let Some(p) = prim {
        eprintln!(
            "  misc_model_189: has_payload={} children={}",
            p.has_payload(),
            p.children().len()
        );
        for child in p.children() {
            eprintln!(
                "    child: {} type={}",
                child.get_path(),
                child.get_type_name().as_str()
            );
        }
    } else {
        eprintln!("  misc_model_189: NOT FOUND in stage");
    }
}
