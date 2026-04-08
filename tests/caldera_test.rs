//! Smoke test for loading Caldera USD files.
use std::path::PathBuf;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Layer;
use usd_sdf::path::Path;
use usd_tf::Token;

fn ensure_init() {
    usd_sdf::init();
}

fn caldera_path(relative: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("USD_RS_CALDERA_ROOT") {
        roots.push(PathBuf::from(root));
    }
    roots.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("_ref")
            .join("caldera"),
    );

    roots
        .into_iter()
        .find(|root| root.exists())
        .map(|root| root.join(relative))
}

fn require_caldera_path(relative: &str) -> Option<PathBuf> {
    let Some(path) = caldera_path(relative) else {
        eprintln!(
            "Skipping Caldera test: fixture root not found. Set USD_RS_CALDERA_ROOT or populate _ref/caldera."
        );
        return None;
    };

    if path.exists() {
        Some(path)
    } else {
        eprintln!("Skipping Caldera test: missing fixture {}", path.display());
        None
    }
}

#[test]
fn test_caldera_single_prefab() {
    ensure_init();
    let Some(path) = require_caldera_path(
        "map_source/prefabs/misc_models/ee_pillar_concrete_pipe_support_base.usd",
    ) else {
        return;
    };
    let stage = Stage::open(path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
        .expect("Failed to open Caldera prefab");

    let default_prim = stage.get_default_prim();
    eprintln!(
        "default prim: {} (valid={})",
        default_prim.get_path(),
        default_prim.is_valid()
    );
    assert!(default_prim.is_valid(), "Should have a valid default prim");

    let mut count = 0;
    for prim in stage.traverse() {
        count += 1;
        if count <= 30 {
            let type_name = prim.get_type_name();
            let num_props = prim.get_authored_properties().len();
            eprintln!(
                "  prim[{}]: {} ({}) props={}",
                count,
                prim.get_path(),
                type_name,
                num_props
            );
        }
    }
    eprintln!("total prims: {}", count);
    assert!(count > 0, "Should have at least one prim");
}

#[test]
fn test_caldera_geo_hierarchy() {
    ensure_init();
    // The geo file references many prefabs via payloads
    let Some(path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    match Stage::open(path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll) {
        Ok(stage) => {
            let mut count = 0;
            let mut mesh_count = 0;
            let mut xform_count = 0;
            let mut payload_count = 0;
            for prim in stage.traverse() {
                count += 1;
                let tn = prim.get_type_name();
                if tn == "Mesh" {
                    mesh_count += 1;
                }
                if tn == "Xform" {
                    xform_count += 1;
                }
                if prim.has_payload() {
                    payload_count += 1;
                }
                if count <= 20 {
                    eprintln!(
                        "  {}: {} ({}) payload={}",
                        count,
                        prim.get_path(),
                        tn,
                        prim.has_payload()
                    );
                }
            }
            eprintln!(
                "geo total: {} prims ({} Xform, {} Mesh, {} with payloads)",
                count, xform_count, mesh_count, payload_count
            );
        }
        Err(e) => {
            eprintln!("FAILED to open geo: {:?}", e);
            // Don't panic - this may fail due to missing referenced files
            // Just report what happened
        }
    }
}

#[test]
fn test_caldera_prefab_mesh_attrs() {
    ensure_init();
    // Open a single prefab directly — fast, no full scene needed
    let Some(path) = require_caldera_path(
        "map_source/prefabs/misc_models/ee_pillar_concrete_pipe_support_base.usd",
    ) else {
        return;
    };
    let stage =
        Stage::open(path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll).expect("open prefab");

    let mut mesh_count = 0;
    let mut ok_count = 0;
    for prim in stage.traverse() {
        if prim.get_type_name() != "Mesh" {
            continue;
        }
        mesh_count += 1;
        let path_str = prim.get_path().get_string();

        // Check PrimIndex
        if let Some(pi) = prim.prim_index() {
            eprintln!("[MESH] {} — PI nodes={}", path_str, pi.num_nodes());
            if let Some(graph) = pi.graph() {
                for i in 0..pi.num_nodes() {
                    let node = usd_pcp::NodeRef::new(graph.clone(), i);
                    let ls_name = node
                        .layer_stack()
                        .and_then(|ls| ls.get_layers().first().map(|l| l.get_display_name()))
                        .unwrap_or_default();
                    eprintln!(
                        "  node[{}]: {:?} path={} specs={} ls={}",
                        i,
                        node.arc_type(),
                        node.path().get_string(),
                        node.has_specs(),
                        ls_name
                    );
                }
            }
        }

        // Try reading faceVertexCounts
        let fvc_attr = prim.get_attribute("faceVertexCounts");
        let fvi_attr = prim.get_attribute("faceVertexIndices");
        let pts_attr = prim.get_attribute("points");
        eprintln!(
            "  fvc valid={} fvi valid={} pts valid={}",
            fvc_attr.is_some(),
            fvi_attr.is_some(),
            pts_attr.is_some()
        );

        let fvc_val = fvc_attr.and_then(|a| a.get(usd_sdf::TimeCode::default_time()));
        let fvi_val = fvi_attr.and_then(|a| a.get(usd_sdf::TimeCode::default_time()));
        let pts_val = pts_attr.and_then(|a| a.get(usd_sdf::TimeCode::default_time()));
        eprintln!(
            "  fvc={} fvi={} pts={}",
            fvc_val
                .as_ref()
                .map(|v| format!("{:?}", v.type_name()))
                .unwrap_or("None".into()),
            fvi_val
                .as_ref()
                .map(|v| format!("{:?}", v.type_name()))
                .unwrap_or("None".into()),
            pts_val
                .as_ref()
                .map(|v| format!("{:?}", v.type_name()))
                .unwrap_or("None".into())
        );

        if fvc_val.is_some() && fvi_val.is_some() && pts_val.is_some() {
            ok_count += 1;
        }
    }
    eprintln!("[RESULT] {}/{} meshes have topology", ok_count, mesh_count);
    assert!(mesh_count > 0, "should have meshes");
    assert!(ok_count > 0, "at least one mesh should have topology");
}

#[test]
fn test_caldera_root_stage() {
    ensure_init();
    let Some(path) = require_caldera_path("map_source/mp_wz_island.usd") else {
        return;
    };
    match Stage::open(path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll) {
        Ok(stage) => {
            let dp = stage.get_default_prim();
            eprintln!(
                "root default prim: {} (valid={})",
                dp.get_path(),
                dp.is_valid()
            );
            let mut count = 0;
            for prim in stage.traverse() {
                count += 1;
                if count <= 30 {
                    eprintln!(
                        "  {}: {} ({})",
                        count,
                        prim.get_path(),
                        prim.get_type_name()
                    );
                }
            }
            eprintln!("root total: {} prims", count);
        }
        Err(e) => {
            eprintln!("root stage FAILED: {:?}", e);
        }
    }
}

#[test]
fn test_caldera_batch_prefabs() {
    ensure_init();
    let Some(base) = require_caldera_path("map_source/prefabs/misc_models") else {
        return;
    };
    // Read directory and try opening all .usd files
    let entries = std::fs::read_dir(&base);
    if entries.is_err() {
        eprintln!("Cannot read dir: {:?}", entries.err());
        return;
    }
    let mut ok = 0;
    let mut fail = 0;
    let mut total_prims = 0;
    for entry in entries.unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "usd") {
            let path_str = path.to_string_lossy().to_string();
            match Stage::open(&path_str, InitialLoadSet::LoadAll) {
                Ok(stage) => {
                    let count = stage.traverse().into_iter().count();
                    total_prims += count;
                    ok += 1;
                }
                Err(e) => {
                    eprintln!(
                        "  FAIL {}: {:?}",
                        path.file_name().unwrap().to_string_lossy(),
                        e
                    );
                    fail += 1;
                }
            }
        }
    }
    eprintln!(
        "batch: {} ok, {} fail, {} total prims",
        ok, fail, total_prims
    );
    assert!(ok > 0, "Should open at least some files");
}

#[test]
fn test_caldera_variant_diagnostics() {
    ensure_init();

    // 1. Check mp_wz_island_geo.usd layer directly for variant fields
    let Some(geo_path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    let geo_layer =
        Layer::find_or_open(geo_path.to_string_lossy().as_ref()).expect("open geo layer");
    let mine_path = Path::from("/world/mp_wz_island_geo/map_phosphate_mine");

    let vset_names_token = Token::new("variantSetNames");
    let vset_children_token = Token::new("variantChildren");
    let variants_token = Token::new("variantSelection");

    let fields = geo_layer.list_fields(&mine_path);
    eprintln!(
        "[GEO LAYER] map_phosphate_mine fields: {:?}",
        fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );

    if let Some(vset_val) = geo_layer.get_field(&mine_path, &vset_names_token) {
        eprintln!("[GEO LAYER]   variantSetNames: {:?}", vset_val);
    } else {
        eprintln!("[GEO LAYER]   variantSetNames: NONE");
    }

    // Check variant set path: /world/mp_wz_island_geo/map_phosphate_mine{districtLod=}
    let vset_path = mine_path.append_variant_selection("districtLod", "");
    eprintln!(
        "[GEO LAYER]   variant set path: {:?}",
        vset_path.as_ref().map(|p| p.get_string())
    );
    if let Some(ref vsp) = vset_path {
        let vset_fields = geo_layer.list_fields(vsp);
        eprintln!(
            "[GEO LAYER]   variant set fields: {:?}",
            vset_fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );
        if let Some(vc) = geo_layer.get_field(vsp, &vset_children_token) {
            eprintln!("[GEO LAYER]   variantChildren: {:?}", vc);
        }
    }

    // Check variant selection paths
    for sel in ["proxy", "full"] {
        let sel_path = mine_path.append_variant_selection("districtLod", sel);
        if let Some(ref sp) = sel_path {
            let sel_fields = geo_layer.list_fields(sp);
            eprintln!(
                "[GEO LAYER]   {}{{}}: fields={:?}",
                sel,
                sel_fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
            );
            // Check for primChildren, references, payloads under variant
            let pc_token = Token::new("primChildren");
            if let Some(pc) = geo_layer.get_field(sp, &pc_token) {
                eprintln!("[GEO LAYER]     primChildren: {:?}", pc);
            }
            let ref_token = Token::new("references");
            if let Some(refs) = geo_layer.get_field(sp, &ref_token) {
                eprintln!("[GEO LAYER]     references: {:?}", refs);
            }
            let payload_token = Token::new("payload");
            if let Some(pl) = geo_layer.get_field(sp, &payload_token) {
                eprintln!("[GEO LAYER]     payload: {:?}", pl);
            }
        }
    }

    // 2. Check caldera.usda layer for variant selection at the over prim path
    let Some(caldera_path) = require_caldera_path("caldera.usda") else {
        return;
    };
    let caldera_layer =
        Layer::find_or_open(caldera_path.to_string_lossy().as_ref()).expect("open caldera layer");
    let caldera_mine =
        Path::from("/world/mp_wz_island/mp_wz_island_paths/mp_wz_island_geo/map_phosphate_mine");

    let caldera_fields = caldera_layer.list_fields(&caldera_mine);
    eprintln!(
        "\n[CALDERA LAYER] map_phosphate_mine fields: {:?}",
        caldera_fields
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
    );

    if let Some(vs) = caldera_layer.get_field(&caldera_mine, &variants_token) {
        eprintln!("[CALDERA LAYER]   variantSelection: {:?}", vs);
    }
    if let Some(vs) = caldera_layer.get_variant_selections(&caldera_mine) {
        eprintln!("[CALDERA LAYER]   get_variant_selections: {:?}", vs);
    } else {
        eprintln!("[CALDERA LAYER]   get_variant_selections: NONE");
    }

    // 3. Open caldera as stage and check PrimIndex for map_phosphate_mine
    let stage = Stage::open(
        caldera_path.to_string_lossy().as_ref(),
        InitialLoadSet::LoadAll,
    )
    .expect("open caldera stage");

    if let Some(prim) = stage.get_prim_at_path(&caldera_mine) {
        eprintln!("\n[STAGE] map_phosphate_mine:");
        eprintln!("  type: {}", prim.get_type_name());
        eprintln!("  children: {}", prim.get_children().len());
        eprintln!(
            "  variant_sets.get_names: {:?}",
            prim.get_variant_sets().get_names()
        );
    } else {
        eprintln!(
            "\n[STAGE] map_phosphate_mine NOT FOUND at {}",
            caldera_mine.get_string()
        );
    }

    // Check mp_wz_island_geo.usd standalone to see if variants deliver children
    let geo_stage = Stage::open(geo_path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
        .expect("geo stage");
    let geo_mine = Path::from("/world/mp_wz_island_geo/map_phosphate_mine");
    if let Some(geo_prim) = geo_stage.get_prim_at_path(&geo_mine) {
        eprintln!("\n[GEO STAGE] map_phosphate_mine:");
        eprintln!("  type: {}", geo_prim.get_type_name());
        eprintln!("  children: {}", geo_prim.get_children().len());
        eprintln!(
            "  variant_sets: {:?}",
            geo_prim.get_variant_sets().get_names()
        );
        for child in geo_prim.get_children().iter().take(10) {
            eprintln!("  child: {} ({})", child.get_path(), child.get_type_name());
        }
    } else {
        eprintln!("\n[GEO STAGE] map_phosphate_mine NOT FOUND");
    }

    // Count prims with variant sets in geo stage
    let mut vset_count = 0;
    for prim in geo_stage.traverse() {
        if !prim.get_variant_sets().get_names().is_empty() {
            vset_count += 1;
            if vset_count <= 5 {
                eprintln!(
                    "[GEO STAGE] prim with vsets: {} vsets={:?}",
                    prim.get_path(),
                    prim.get_variant_sets().get_names()
                );
            }
        }
    }
    eprintln!("[GEO STAGE] total prims with variant sets: {}", vset_count);
}

#[test]
fn test_caldera_prim_index_graph() {
    ensure_init();
    // Use PcpCache directly to inspect the PrimIndex graph for a variant prim
    let Some(geo_path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    // Ensure layer is loaded
    let _geo_layer =
        Layer::find_or_open(geo_path.to_string_lossy().as_ref()).expect("open geo layer");
    let layer_id = usd_pcp::LayerStackIdentifier::new(geo_path.to_string_lossy().as_ref());
    let cache = usd_pcp::Cache::new(layer_id, true);

    let prim_path = Path::from_string("/world/mp_wz_island_geo/map_phosphate_mine").unwrap();
    let (prim_index, errors) = cache.compute_prim_index(&prim_path);
    eprintln!("\n=== PRIM INDEX for {} ===", prim_path.get_string());
    eprintln!("valid={}, errors={}", prim_index.is_valid(), errors.len());
    for e in &errors {
        eprintln!("  error: {:?}", e);
    }

    let num_nodes = prim_index.num_nodes();
    eprintln!("num_nodes={}", num_nodes);

    if let Some(graph) = prim_index.graph() {
        for i in 0..num_nodes {
            let node = usd_pcp::NodeRef::new(graph.clone(), i);
            let has_specs = node.has_specs();
            let can_contrib = node.can_contribute_specs();
            let arc_type = node.arc_type();
            let site_path = node.path();
            let ls_layers = node
                .layer_stack()
                .map(|ls| {
                    ls.get_layers()
                        .iter()
                        .map(|l| l.get_display_name())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            eprintln!(
                "  node[{}]: arc={:?} path={} has_specs={} can_contrib={} layers={:?}",
                i,
                arc_type,
                site_path.get_string(),
                has_specs,
                can_contrib,
                ls_layers
            );
        }
    }

    // Check child names
    let (children, prohibited) = prim_index.compute_prim_child_names();
    eprintln!(
        "children={:?} prohibited={:?}",
        children.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        prohibited.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );

    // Check variant selections
    let selections = prim_index.compose_authored_variant_selections();
    eprintln!("variant_selections={:?}", selections);

    // Now test with payload predicate (like Stage LoadAll) and force proxy variant
    let layer_id2 = usd_pcp::LayerStackIdentifier::new(geo_path.to_string_lossy().as_ref());
    let cache2 = usd_pcp::Cache::new(layer_id2, true);
    // Set payload predicate: include all
    let pred: std::sync::Arc<dyn Fn(&Path) -> bool + Send + Sync> = std::sync::Arc::new(|_| true);
    cache2.set_include_payload_predicate(Some(pred));
    // Set variant fallbacks to prefer proxy
    let mut fallbacks = std::collections::HashMap::new();
    fallbacks.insert("districtLod".to_string(), vec!["proxy".to_string()]);
    cache2.set_variant_fallbacks(fallbacks, None);

    let (pi2, errs2) = cache2.compute_prim_index(&prim_path);
    eprintln!("\n=== PRIM INDEX with proxy+predicate ===");
    eprintln!("valid={}, errors={}", pi2.is_valid(), errs2.len());
    for e in &errs2 {
        eprintln!("  error: {:?}", e);
    }
    let nn2 = pi2.num_nodes();
    eprintln!("num_nodes={}", nn2);
    if let Some(graph) = pi2.graph() {
        for i in 0..nn2 {
            let node = usd_pcp::NodeRef::new(graph.clone(), i);
            eprintln!(
                "  node[{}]: arc={:?} path={} has_specs={} can_contrib={}",
                i,
                node.arc_type(),
                node.path().get_string(),
                node.has_specs(),
                node.can_contribute_specs()
            );
        }
    }
    let (ch2, _) = pi2.compute_prim_child_names();
    eprintln!("children={}", ch2.len());
    eprintln!(
        "variant_selections={:?}",
        pi2.compose_authored_variant_selections()
    );

    // Now check Stage-level: does Stage::open produce the same children?
    let geo_stage = Stage::open(geo_path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
        .expect("open geo stage");
    let prim = geo_stage.get_prim_at_path(&prim_path);
    if let Some(prim) = prim {
        let stage_children: Vec<_> = prim
            .get_children()
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        eprintln!(
            "\n[STAGE] map_phosphate_mine children count={}",
            stage_children.len()
        );
        for (i, name) in stage_children.iter().enumerate().take(10) {
            eprintln!("  child[{}]: {}", i, name);
        }
        eprintln!("  type_name={}", prim.get_type_name());
    } else {
        eprintln!("\n[STAGE] map_phosphate_mine NOT FOUND in stage!");
        // List all prims containing 'phosphate'
        for p in geo_stage.traverse() {
            if p.get_path().get_string().contains("phosphate") {
                let ch_count = p.get_children().len();
                eprintln!("  found: {} children={}", p.get_path(), ch_count);
            }
        }
    }

    // FINAL: count total prims in stage (are there any variant children at ALL?)
    let mut total = 0;
    let mut with_children = 0;
    for p in geo_stage.traverse() {
        total += 1;
        if !p.get_children().is_empty() {
            with_children += 1;
        }
    }
    eprintln!(
        "\n[STAGE TOTAL] {} prims, {} with children",
        total, with_children
    );
}

#[test]
fn test_caldera_stage_prim_index_debug() {
    ensure_init();
    let Some(caldera_path) = require_caldera_path("caldera.usda") else {
        return;
    };
    let _caldera_layer =
        Layer::find_or_open(caldera_path.to_string_lossy().as_ref()).expect("open caldera layer");
    let layer_id = usd_pcp::LayerStackIdentifier::new(caldera_path.to_string_lossy().as_ref());
    let cache = usd_pcp::Cache::new(layer_id, true);

    // Set payload predicate (LoadAll)
    let pred: std::sync::Arc<dyn Fn(&Path) -> bool + Send + Sync> = std::sync::Arc::new(|_| true);
    cache.set_include_payload_predicate(Some(pred));

    // Check the layer stack first
    if let Some(ls) = cache.layer_stack() {
        eprintln!(
            "[CALDERA CACHE] Layer stack ({} layers):",
            ls.get_layers().len()
        );
        for (i, l) in ls.get_layers().iter().enumerate() {
            eprintln!("  [{}] {}", i, l.get_display_name());
        }
    }

    // Test several paths up the hierarchy
    let test_paths = [
        "/world",
        "/world/mp_wz_island",
        "/world/mp_wz_island/mp_wz_island_paths",
        "/world/mp_wz_island/mp_wz_island_paths/mp_wz_island_geo",
        "/world/mp_wz_island/mp_wz_island_paths/mp_wz_island_geo/map_phosphate_mine",
    ];

    for path_str in &test_paths {
        let path = Path::from_string(path_str).unwrap();
        let (pi, errors) = cache.compute_prim_index(&path);
        let nn = pi.num_nodes();
        let (children, _) = pi.compute_prim_child_names();
        eprintln!(
            "\n[CALDERA PI] {} — valid={} nodes={} children={} errors={}",
            path_str,
            pi.is_valid(),
            nn,
            children.len(),
            errors.len()
        );
        for e in &errors {
            eprintln!("  ERROR: {:?}", e);
        }
        if let Some(graph) = pi.graph() {
            for i in 0..nn {
                let node = usd_pcp::NodeRef::new(graph.clone(), i);
                let ls_info = node
                    .layer_stack()
                    .map(|ls| {
                        format!(
                            "{} layers ({})",
                            ls.get_layers().len(),
                            ls.get_layers()
                                .first()
                                .map(|l| l.get_display_name())
                                .unwrap_or_default()
                        )
                    })
                    .unwrap_or("no-ls".to_string());
                eprintln!(
                    "  node[{}]: arc={:?} path={} specs={} contrib={} ls={}",
                    i,
                    node.arc_type(),
                    node.path().get_string(),
                    node.has_specs(),
                    node.can_contribute_specs(),
                    ls_info
                );
            }
        }
        if children.len() <= 10 {
            eprintln!(
                "  children: {:?}",
                children.iter().map(|t| t.as_str()).collect::<Vec<_>>()
            );
        } else {
            eprintln!(
                "  children (first 10): {:?}",
                children
                    .iter()
                    .take(10)
                    .map(|t| t.as_str())
                    .collect::<Vec<_>>()
            );
        }
    }
}
