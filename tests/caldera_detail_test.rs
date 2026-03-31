//! Detailed Caldera payload investigation.
use std::path::PathBuf;

use usd_core::{InitialLoadSet, Stage};
use usd_sdf::Layer;
use usd_sdf::path::Path;
use usd_tf::Token;

fn caldera_path(relative: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("USD_RS_CALDERA_ROOT") {
        roots.push(PathBuf::from(root));
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("_ref").join("caldera"));

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
fn test_caldera_prefab_layer_fields() {
    usd_sdf::init();
    let Some(path) =
        require_caldera_path("map_source/prefabs/misc_models/ee_pillar_concrete_pipe_support_base.usd")
    else {
        return;
    };
    let layer = Layer::find_or_open(path.to_string_lossy().as_ref()).expect("open layer");
    let data = layer.data();

    // Check known prim paths for payload/reference fields
    let prim_paths = [
        "/world",
        "/world/ee_pillar_concrete_pipe_support_base",
        "/world/ee_pillar_concrete_pipe_support_base/misc_model_189",
        "/world/ee_pillar_concrete_pipe_support_base/worldspawn",
        "/world/ee_pillar_concrete_pipe_support_base/worldspawn/brush_1_390",
    ];

    for path_str in &prim_paths {
        let p = Path::from(*path_str);
        let fields = data.list_fields(&p);
        eprintln!(
            "{}: fields={:?}",
            path_str,
            fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );

        // Check payload specifically
        let payload_token = Token::new("payload");
        if data.has_field(&p, &payload_token) {
            let val = data.get_field(&p, &payload_token);
            eprintln!("  PAYLOAD: {:?}", val);
        }

        // Check references
        let ref_token = Token::new("references");
        if data.has_field(&p, &ref_token) {
            let val = data.get_field(&p, &ref_token);
            eprintln!("  REFERENCES: {:?}", val);
        }
    }
}

#[test]
fn test_caldera_prim_composition() {
    usd_sdf::init();
    let Some(path) =
        require_caldera_path("map_source/prefabs/misc_models/ee_pillar_concrete_pipe_support_base.usd")
    else {
        return;
    };
    let stage = Stage::open(path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
        .expect("open stage");

    for prim in stage.traverse() {
        let p = prim.get_path();
        let has_payload = prim.has_payload();
        let is_loaded = prim.is_loaded();
        let children = prim.get_children();
        let attrs = prim.get_authored_properties();

        if has_payload || !attrs.is_empty() {
            eprintln!(
                "  {} type={} payload={} loaded={} children={} attrs={}",
                p,
                prim.get_type_name(),
                has_payload,
                is_loaded,
                children.len(),
                attrs.len()
            );
        }
    }
}

#[test]
fn test_caldera_geo_references() {
    usd_sdf::init();
    let Some(path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    let layer = Layer::find_or_open(path.to_string_lossy().as_ref()).expect("open layer");
    let data = layer.data();

    // Check some prim paths
    let prim_paths = ["/world", "/world/mp_wz_island_geo"];
    for path_str in &prim_paths {
        let p = Path::from(*path_str);
        let fields = data.list_fields(&p);
        eprintln!(
            "{}: fields={:?}",
            path_str,
            fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );
    }

    // Check for sub-layer composition
    eprintln!("sublayers: {:?}", layer.get_sublayer_paths());

    // Check the stage traverse to see what got composed
    let stage = Stage::open(
        path.to_string_lossy().as_ref(),
        InitialLoadSet::LoadAll,
    )
    .expect("open stage");

    let mut reference_count = 0;
    for prim in stage.traverse() {
        if prim.has_payload() {
            reference_count += 1;
            eprintln!("  payload: {}", prim.get_path());
        }
    }
    eprintln!("total prims with payloads: {}", reference_count);
}

#[test]
fn test_caldera_geo_district_fields() {
    usd_sdf::init();
    let Some(path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    let layer = Layer::find_or_open(path.to_string_lossy().as_ref()).expect("open layer");
    let data = layer.data();

    // Check fields on district prims
    let district_paths = [
        "/world/mp_wz_island_geo/map_phosphate_mine",
        "/world/mp_wz_island_geo/map_tile_p",
        "/world/mp_wz_island_geo/volume_vista_20",
        "/world/mp_wz_island_geo/volume_vista_20/brush_6_4",
    ];
    for path_str in &district_paths {
        let p = Path::from(*path_str);
        let fields = data.list_fields(&p);
        let field_names: Vec<_> = fields.iter().map(|t| t.as_str()).collect();
        eprintln!("{}: fields={:?}", path_str, field_names);

        for field_name in &[
            "references",
            "payload",
            "variantSetNames",
            "variantSelection",
            "inherits",
            "specializes",
        ] {
            let tok = Token::new(field_name);
            if data.has_field(&p, &tok) {
                let val = data.get_field(&p, &tok);
                eprintln!("  {}: {:?}", field_name, val);
            }
        }
    }

    // Check a generated_proxies asset via stage
    if let Some(proxy_path) = caldera_path("assets/xmodel/generated_proxies/map_phosphate_mine.usd")
    {
        if proxy_path.exists() {
        eprintln!("\nProxy asset exists, opening as stage...");
            if let Ok(proxy_stage) =
                Stage::open(proxy_path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
            {
            let mut count = 0;
            for prim in proxy_stage.traverse() {
                count += 1;
                if count <= 15 {
                    eprintln!(
                        "  proxy prim: {} ({})",
                        prim.get_path(),
                        prim.get_type_name()
                    );
                }
            }
            eprintln!("  proxy total: {} prims", count);
        }
    }
    }
}

#[test]
fn test_caldera_variant_content() {
    usd_sdf::init();
    let Some(path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    let layer = Layer::find_or_open(path.to_string_lossy().as_ref()).expect("open layer");
    let data = layer.data();

    // Check variant set content for map_phosphate_mine
    let variant_paths = [
        "/world/mp_wz_island_geo/map_phosphate_mine{districtLod=proxy}",
        "/world/mp_wz_island_geo/map_phosphate_mine{districtLod=full}",
    ];
    for path_str in &variant_paths {
        let p = Path::from(*path_str);
        let fields = data.list_fields(&p);
        let field_names: Vec<_> = fields.iter().map(|t| t.as_str()).collect();
        eprintln!("{}: fields={:?}", path_str, field_names);

        // Check for references inside variant
        for field_name in &["references", "payload", "primChildren"] {
            let tok = Token::new(field_name);
            if data.has_field(&p, &tok) {
                let val = data.get_field(&p, &tok);
                eprintln!("  {}: {:?}", field_name, val);
            }
        }
    }

    // Now open as stage and check if variant selection is active
    let stage = Stage::open(
        path.to_string_lossy().as_ref(),
        InitialLoadSet::LoadAll,
    )
    .expect("open stage");

    if let Some(prim) =
        stage.get_prim_at_path(&Path::from("/world/mp_wz_island_geo/map_phosphate_mine"))
    {
        eprintln!("\nmap_phosphate_mine on stage (geo.usd only):");
        eprintln!("  type: {}", prim.get_type_name());
        eprintln!("  children: {}", prim.get_children().len());
        eprintln!("  has_variant_sets: {}", prim.has_variant_sets());
        let vs = prim.get_variant_sets();
        for name in vs.get_names() {
            eprintln!(
                "  variantSet '{}': selection='{}'",
                name,
                vs.get_variant_selection(&name)
            );
        }
        for child in prim.get_children() {
            eprintln!("  child: {} ({})", child.get_path(), child.get_type_name());
        }
    } else {
        eprintln!("map_phosphate_mine NOT FOUND on stage!");
    }
}

#[test]
fn test_caldera_root_stage() {
    usd_sdf::init();
    // Open caldera.usda (root file) — has variant selections in over prims
    let Some(path) = require_caldera_path("caldera.usda") else {
        return;
    };
    let stage = Stage::open(path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
        .expect("open stage");

    // Check a district prim — prim path goes through sublayer chain:
    // caldera.usda → mp_wz_island.usd → mp_wz_island_paths → mp_wz_island_geo
    let test_paths = [
        "/world/mp_wz_island/mp_wz_island_paths/mp_wz_island_geo/map_phosphate_mine",
        "/world/mp_wz_island/mp_wz_island_paths/mp_wz_island_geo/map_tile_p",
    ];
    for prim_path in &test_paths {
        if let Some(prim) = stage.get_prim_at_path(&Path::from(*prim_path)) {
            eprintln!("\n{}:", prim_path);
            eprintln!("  type: {}", prim.get_type_name());
            eprintln!("  has_variant_sets: {}", prim.has_variant_sets());
            let vs = prim.get_variant_sets();
            let names = vs.get_names();
            eprintln!("  variant set names: {:?}", names);
            for name in &names {
                eprintln!(
                    "  variantSet '{}': selection='{}'",
                    name,
                    vs.get_variant_selection(name)
                );
            }
            let children = prim.get_children();
            eprintln!("  children: {}", children.len());
            for child in children.iter().take(5) {
                eprintln!(
                    "    child: {} ({})",
                    child.get_path(),
                    child.get_type_name()
                );
            }
        } else {
            eprintln!("{}: NOT FOUND", prim_path);
        }
    }

    // Count total traversable prims
    let mut total = 0;
    let mut with_mesh = 0;
    for prim in stage.traverse() {
        total += 1;
        if prim.get_type_name().as_str() == "Mesh" {
            with_mesh += 1;
        }
    }
    eprintln!("\nTotal prims: {}, Meshes: {}", total, with_mesh);

    // Check sublayer composition: list sublayer paths
    let root = stage.get_root_layer();
    eprintln!("\nRoot layer sublayers: {:?}", root.get_sublayer_paths());

    // Check if sublayer files actually exist and can be opened
    for sublayer_path in root.get_sublayer_paths() {
        let anchored =
            usd_sdf::layer_utils::compute_asset_path_relative_to_layer(&root, &sublayer_path);
        eprintln!("  sublayer '{}' -> anchored '{}'", sublayer_path, anchored);
        match Layer::find_or_open(&anchored) {
            Ok(sub) => {
                eprintln!("    OPENED: {}", sub.identifier());
                // Check mp_wz_island.usd sublayers too
                let sub_sublayers = sub.get_sublayer_paths();
                if !sub_sublayers.is_empty() {
                    eprintln!("    sub-sublayers: {:?}", sub_sublayers);
                }
            }
            Err(e) => eprintln!("    FAILED: {}", e),
        }
    }

    // Check the /world prim — does it exist? what specifier?
    if let Some(prim) = stage.get_prim_at_path(&Path::from("/world")) {
        eprintln!("\n/world prim: type='{}'", prim.get_type_name());
        eprintln!("  children: {}", prim.get_children().len());
        for child in prim.get_children().iter().take(10) {
            eprintln!(
                "    child: {} type='{}'",
                child.get_path(),
                child.get_type_name()
            );
        }
    } else {
        eprintln!("/world NOT FOUND");
    }

    // Open mp_wz_island.usd as Layer to check sublayers and prim children
    let Some(sub_path) = require_caldera_path("map_source/mp_wz_island.usd") else {
        return;
    };
    let sub_layer = Layer::find_or_open(sub_path.to_string_lossy().as_ref()).expect("open sublayer");
    eprintln!(
        "\nmp_wz_island.usd sublayers: {:?}",
        sub_layer.get_sublayer_paths()
    );
    // List root children
    let sub_root = Path::from("/");
    let data = sub_layer.data();
    let root_fields = data.list_fields(&sub_root);
    eprintln!(
        "mp_wz_island.usd root fields: {:?}",
        root_fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );

    // List children of /world
    let world_path = Path::from("/world");
    let world_fields = data.list_fields(&world_path);
    eprintln!(
        "mp_wz_island.usd /world fields: {:?}",
        world_fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );
    // List children of /world/mp_wz_island
    let island_path = Path::from("/world/mp_wz_island");
    let island_fields = data.list_fields(&island_path);
    eprintln!(
        "mp_wz_island.usd /world/mp_wz_island fields: {:?}",
        island_fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );
    // Check primChildren to see the child names
    let prim_children_tok = Token::new("primChildren");
    if data.has_field(&island_path, &prim_children_tok) {
        let val = data.get_field(&island_path, &prim_children_tok);
        eprintln!("  primChildren: {:?}", val);
    }

    // Check fields on mp_wz_island_paths prim (should have payload/reference to mp_wz_island_paths.usd)
    let paths_prim_path = Path::from("/world/mp_wz_island/mp_wz_island_paths");
    let paths_fields = data.list_fields(&paths_prim_path);
    eprintln!(
        "\nmp_wz_island.usd /world/mp_wz_island/mp_wz_island_paths fields: {:?}",
        paths_fields.iter().map(|t| t.as_str()).collect::<Vec<_>>()
    );
    for field_name in &[
        "references",
        "payload",
        "primChildren",
        "specifier",
        "typeName",
    ] {
        let tok = Token::new(field_name);
        if data.has_field(&paths_prim_path, &tok) {
            let val = data.get_field(&paths_prim_path, &tok);
            eprintln!("  {}: {:?}", field_name, val);
        }
    }

    // Check mp_wz_island_lighting children too (these are working - 103 prims come from here)
    let lighting_path = Path::from("/world/mp_wz_island/mp_wz_island_lighting");
    let lighting_fields = data.list_fields(&lighting_path);
    eprintln!(
        "\nmp_wz_island.usd /world/mp_wz_island/mp_wz_island_lighting fields: {:?}",
        lighting_fields
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
    );
    for field_name in &[
        "references",
        "payload",
        "primChildren",
        "specifier",
        "typeName",
    ] {
        let tok = Token::new(field_name);
        if data.has_field(&lighting_path, &tok) {
            let val = data.get_field(&lighting_path, &tok);
            eprintln!("  {}: {:?}", field_name, val);
        }
    }

    // Open as stage — count children per branch under /world/mp_wz_island
    let sub_stage = Stage::open(sub_path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
        .expect("open sublayer stage");
    let mut sub_count = 0;
    if let Some(island_prim) = sub_stage.get_prim_at_path(&Path::from("/world/mp_wz_island")) {
        eprintln!("\nmp_wz_island.usd /world/mp_wz_island children:");
        for child in island_prim.get_children() {
            let child_count = sub_stage
                .traverse()
                .into_iter()
                .filter(|p| {
                    p.get_path()
                        .get_as_token()
                        .as_str()
                        .starts_with(child.get_path().get_as_token().as_str())
                })
                .count();
            eprintln!(
                "  {} type='{}' subtree={}",
                child.get_path(),
                child.get_type_name(),
                child_count
            );
            // If mp_wz_island_paths, show its children
            if child
                .get_path()
                .get_as_token()
                .as_str()
                .contains("mp_wz_island_paths")
            {
                eprintln!(
                    "    mp_wz_island_paths children: {}",
                    child.get_children().len()
                );
                for c2 in child.get_children().iter().take(5) {
                    eprintln!("      child: {} type={}", c2.get_path(), c2.get_type_name());
                }
            }
        }
    }
    for _prim in sub_stage.traverse() {
        sub_count += 1;
    }
    eprintln!("mp_wz_island.usd total: {} prims", sub_count);

    // Check if mp_wz_island_paths.usd and mp_wz_island_geo.usd exist and their prim structure
    let Some(paths_path) = require_caldera_path("map_source/mp_wz_island_paths.usd") else {
        return;
    };
    let Some(geo_path) = require_caldera_path("map_source/mp_wz_island_geo.usd") else {
        return;
    };
    eprintln!(
        "mp_wz_island_paths.usd exists: {}",
        paths_path.exists()
    );
    eprintln!(
        "mp_wz_island_geo.usd exists: {}",
        geo_path.exists()
    );

    // Check mp_wz_island_paths.usd structure (does it have /world/mp_wz_island_paths?)
    if let Ok(paths_layer) = Layer::find_or_open(paths_path.to_string_lossy().as_ref()) {
        let pdata = paths_layer.data();
        let root_f = pdata.list_fields(&Path::from("/"));
        eprintln!(
            "\nmp_wz_island_paths.usd root fields: {:?}",
            root_f.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );
        // Check for the target prim path
        let target = Path::from("/world/mp_wz_island_paths");
        let tf = pdata.list_fields(&target);
        eprintln!(
            "  /world/mp_wz_island_paths fields: {:?}",
            tf.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );
        for field_name in &[
            "references",
            "payload",
            "primChildren",
            "specifier",
            "typeName",
        ] {
            let tok = Token::new(field_name);
            if pdata.has_field(&target, &tok) {
                let val = pdata.get_field(&target, &tok);
                eprintln!("    {}: {:?}", field_name, val);
            }
        }
        // Check /world too
        let w = Path::from("/world");
        let wf = pdata.list_fields(&w);
        eprintln!(
            "  /world fields: {:?}",
            wf.iter().map(|t| t.as_str()).collect::<Vec<_>>()
        );
        // Check sublayers
        eprintln!("  sublayers: {:?}", paths_layer.get_sublayer_paths());
    }

    // Open mp_wz_island_paths.usd as stage to see if it works standalone
    if let Ok(paths_stage) =
        Stage::open(paths_path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
    {
        let mut pc = 0;
        for prim in paths_stage.traverse() {
            pc += 1;
            if pc <= 5 {
                eprintln!(
                    "  paths prim: {} type={}",
                    prim.get_path(),
                    prim.get_type_name()
                );
            }
        }
        eprintln!("mp_wz_island_paths.usd total: {} prims", pc);
    } else {
        eprintln!("FAILED to open mp_wz_island_paths.usd as stage!");
    }

    // Open mp_wz_island_geo.usd as stage
    if geo_path.exists() {
        let geo_stage =
            Stage::open(geo_path.to_string_lossy().as_ref(), InitialLoadSet::LoadAll)
                .expect("open geo");
        let mut geo_count = 0;
        let mut geo_mesh = 0;
        for prim in geo_stage.traverse() {
            geo_count += 1;
            if prim.get_type_name().as_str() == "Mesh" {
                geo_mesh += 1;
            }
            if geo_count <= 10 {
                eprintln!(
                    "  geo prim: {} type={}",
                    prim.get_path(),
                    prim.get_type_name()
                );
            }
        }
        eprintln!(
            "mp_wz_island_geo.usd total: {} prims, {} meshes",
            geo_count, geo_mesh
        );
    }
}
