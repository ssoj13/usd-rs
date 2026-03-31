//! Baseline comparison tests.
//! Compares our composition results against the C++ OpenUSD baselines.
//!
//! For each Museum scenario, generates a prim stack + child names dump
//! and compares against the reference baseline file.

use std::collections::BTreeMap;
use std::path::PathBuf;
use usd_pcp::{
    Cache, LayerStackIdentifier, PrimIndex, compare_sibling_node_strength, dump_prim_index,
    prim_index_is_instanceable, traverse_instanceable_strong_to_weak,
};
use usd_sdf::Path;

fn ensure_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| usd_sdf::init());
}

fn testenv_path(relative: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/testenv/{}", manifest_dir.replace('\\', "/"), relative)
}

fn ref_baseline_path(scenario: &str) -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join(format!(
        "testenv/baselines/compositionResults_{}.txt",
        scenario
    ))
}

/// Extract just the filename from a layer identifier path.
fn layer_filename(identifier: &str) -> String {
    identifier
        .rsplit('/')
        .next()
        .unwrap_or(identifier)
        .rsplit('\\')
        .next()
        .unwrap_or(identifier)
        .to_string()
}

/// Generate prim stack entries: Vec<(layer_filename, prim_path)>
/// Matches C++ testPcpCompositionResults.py Prim Stack output.
fn get_prim_stack(prim_index: &PrimIndex) -> Vec<(String, String)> {
    let mut stack = Vec::new();
    if !prim_index.is_valid() {
        return stack;
    }

    // Use the prim stack (CompressedSdSite array) which gives
    // (node_index, layer_index) pairs in strength order.
    let prim_stack = prim_index.prim_stack();
    for (i, _site) in prim_stack.iter().enumerate() {
        if let Some(sdf_site) = prim_index.get_site_at_prim_stack_index(i) {
            let filename = if let Some(l) = sdf_site.layer.upgrade() {
                layer_filename(&l.identifier())
            } else {
                "<unknown>".to_string()
            };
            stack.push((filename, sdf_site.path.as_str().to_string()));
        }
    }
    stack
}

/// Generate child names from prim index.
fn get_child_names(prim_index: &PrimIndex) -> Vec<String> {
    let (names, _prohibited) = prim_index.compute_prim_child_names();
    names.iter().map(|t| t.as_str().to_string()).collect()
}

/// Parse a C++ baseline file into structured data.
/// Returns map of prim_path -> (prim_stack, child_names, variant_selections)
fn parse_baseline(content: &str) -> BTreeMap<String, (Vec<(String, String)>, Vec<String>)> {
    let mut result = BTreeMap::new();
    let mut current_prim: Option<String> = None;
    let mut current_stack: Vec<(String, String)> = Vec::new();
    let mut current_children: Vec<String> = Vec::new();
    let mut in_prim_stack = false;
    let mut in_child_names = false;

    for line in content.lines() {
        if line.starts_with("Results for composing <") {
            // Save previous prim
            if let Some(ref prim) = current_prim {
                result.insert(
                    prim.clone(),
                    (current_stack.clone(), current_children.clone()),
                );
            }
            // Parse prim path: "Results for composing </Foo>"
            let path = line
                .trim_start_matches("Results for composing <")
                .trim_end_matches('>');
            current_prim = Some(path.to_string());
            current_stack.clear();
            current_children.clear();
            in_prim_stack = false;
            in_child_names = false;
        } else if line == "Prim Stack:" {
            in_prim_stack = true;
            in_child_names = false;
        } else if line.starts_with("Child names:") {
            in_prim_stack = false;
            in_child_names = true;
        } else if line.starts_with("Variant Selections:")
            || line.starts_with("Time Offsets:")
            || line.starts_with("Property names:")
            || line.starts_with("Property stacks:")
            || line.starts_with("----")
        {
            in_prim_stack = false;
            in_child_names = false;
        } else if in_prim_stack && !line.trim().is_empty() {
            // Parse: "    layer.usda            /PrimPath"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                current_stack.push((parts[0].to_string(), parts[1].to_string()));
            }
        } else if in_child_names && !line.trim().is_empty() {
            // Parse: "     ['Child1', 'Child2']"
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                // Parse Python-style list: ['a', 'b', 'c']
                let inner = trimmed.trim_start_matches('[').trim_end_matches(']');
                for item in inner.split(',') {
                    let name = item.trim().trim_matches('\'').trim_matches('"');
                    if !name.is_empty() {
                        current_children.push(name.to_string());
                    }
                }
            }
        }
    }

    // Save last prim
    if let Some(ref prim) = current_prim {
        result.insert(
            prim.clone(),
            (current_stack.clone(), current_children.clone()),
        );
    }

    result
}

/// Run baseline comparison for a Museum scenario.
/// Returns (total_prims, matching_prims, mismatches_details)
fn compare_baseline(scenario: &str) -> (usize, usize, Vec<String>) {
    ensure_init();

    let baseline_path = ref_baseline_path(scenario);
    let baseline_content = match std::fs::read_to_string(&baseline_path) {
        Ok(c) => c,
        Err(e) => {
            return (0, 0, vec![format!("Cannot read baseline: {:?}", e)]);
        }
    };

    let expected = parse_baseline(&baseline_content);
    if expected.is_empty() {
        return (0, 0, vec!["Empty baseline".into()]);
    }

    let root_path = testenv_path(&format!("museum/{}/root.usda", scenario));
    // C++ baseline loads session.usda if present alongside root.usda
    let session_path = testenv_path(&format!("museum/{}/session.usda", scenario));
    let id = if std::path::Path::new(&session_path).exists() {
        LayerStackIdentifier::with_session(root_path.as_str(), Some(session_path.as_str()))
    } else {
        LayerStackIdentifier::new(root_path.as_str())
    };
    let cache = Cache::new(id.clone(), true);

    // C++ testPcpCompositionResults loads all payloads by default.
    // Set a predicate that includes everything.
    cache.set_include_payload_predicate(Some(std::sync::Arc::new(|_path: &Path| true)));

    // C++ testPcpCompositionResults sets variant fallback: standin -> ["render"]
    // Required for TypicalReferenceToRiggedModel which expects this fallback selection.
    let mut fallbacks = std::collections::HashMap::new();
    fallbacks.insert("standin".to_string(), vec!["render".to_string()]);
    cache.set_variant_fallbacks(fallbacks, None);

    let mut total = 0;
    let mut matching = 0;
    let mut mismatches = Vec::new();

    for (prim_path, (expected_stack, expected_children)) in &expected {
        total += 1;
        let path = Path::from_string(prim_path).unwrap();
        let (prim_index, _errors) = cache.compute_prim_index(&path);

        if !prim_index.is_valid() {
            mismatches.push(format!("{}: invalid prim index", prim_path));
            continue;
        }

        let actual_stack = get_prim_stack(&prim_index);
        let actual_children = get_child_names(&prim_index);

        let mut prim_ok = true;

        // Compare prim stacks
        if actual_stack != *expected_stack {
            prim_ok = false;
            mismatches.push(format!(
                "{}: prim stack mismatch\n  expected: {:?}\n  actual:   {:?}",
                prim_path, expected_stack, actual_stack
            ));
        }

        // Compare child names
        if actual_children != *expected_children {
            prim_ok = false;
            mismatches.push(format!(
                "{}: child names mismatch\n  expected: {:?}\n  actual:   {:?}",
                prim_path, expected_children, actual_children
            ));
        }

        if prim_ok {
            matching += 1;
        }
    }

    (total, matching, mismatches)
}

// ============================================================================
// Baseline comparison tests
// ============================================================================

macro_rules! baseline_test {
    ($test_name:ident, $scenario:expr) => {
        #[test]
        fn $test_name() {
            let (total, matching, mismatches) = compare_baseline($scenario);
            if total == 0 {
                eprintln!("SKIP {}: no baseline data", $scenario);
                return;
            }
            if matching < total {
                eprintln!(
                    "BASELINE {} : {}/{} prims match",
                    $scenario, matching, total
                );
                for m in &mismatches {
                    eprintln!("  {}", m);
                }
            }
            assert_eq!(
                matching,
                total,
                "Baseline {}: {}/{} prims match. {} mismatches.",
                $scenario,
                matching,
                total,
                mismatches.len()
            );
        }
    };
}

// Start with the key scenarios
baseline_test!(baseline_basic_reference, "BasicReference");
baseline_test!(baseline_basic_inherits, "BasicInherits");
baseline_test!(baseline_basic_specializes, "BasicSpecializes");
baseline_test!(baseline_basic_payload, "BasicPayload");
baseline_test!(baseline_basic_time_offset, "BasicTimeOffset");
baseline_test!(baseline_basic_nested_variants, "BasicNestedVariants");
baseline_test!(baseline_basic_list_editing, "BasicListEditing");
baseline_test!(baseline_basic_reference_diamond, "BasicReferenceDiamond");
baseline_test!(baseline_basic_reference_and_class, "BasicReferenceAndClass");
baseline_test!(
    baseline_basic_variant_with_reference,
    "BasicVariantWithReference"
);
baseline_test!(baseline_basic_instancing, "BasicInstancing");
baseline_test!(baseline_basic_owner, "BasicOwner");
baseline_test!(baseline_typical_chargroup, "TypicalReferenceToChargroup");
baseline_test!(
    baseline_typical_rigged_model,
    "TypicalReferenceToRiggedModel"
);

#[test]
#[ignore]
fn debug_basic_instancing_dump() {
    ensure_init();

    let root_path = testenv_path("museum/BasicInstancing/root.usda");
    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id, true);
    cache.set_include_payload_predicate(Some(std::sync::Arc::new(|_path: &Path| true)));

    for prim_path in [
        "/Set_1/InstancedProp",
        "/Set_1/InstancedProp/geom",
        "/SubrootReferences/Instanced",
    ] {
        let path = Path::from_string(prim_path).unwrap();
        let (prim_index, errors) = cache.compute_prim_index(&path);
        let mut manual_has_instanceable_data = false;
        let mut manual_authored_instanceable = None;
        eprintln!("PRIM {prim_path}");
        eprintln!("is_usd={}", prim_index.is_usd());
        eprintln!("instanceable={}", prim_index.is_instanceable());
        eprintln!(
            "computed_instanceable={}",
            prim_index_is_instanceable(&prim_index)
        );
        eprintln!("errors={errors:?}");
        traverse_instanceable_strong_to_weak(&prim_index, |node, node_is_instanceable| {
            let site = node.site();
            let layers: Vec<String> = node
                .layer_stack()
                .map(|ls| {
                    ls.get_layers()
                        .into_iter()
                        .map(|layer| layer_filename(&layer.identifier()))
                        .collect()
                })
                .unwrap_or_default();
            eprintln!(
                "  node arc={:?} path={} site_path={} node_is_instanceable={} direct={} can_specs={} has_specs={} layers={layers:?}",
                node.arc_type(),
                node.path(),
                site.path,
                node_is_instanceable,
                node.has_transitive_direct_dependency(),
                node.can_contribute_specs(),
                node.has_specs(),
            );
            if node_is_instanceable {
                manual_has_instanceable_data = true;
            }
            if let Some(ls) = node.layer_stack() {
                let field = usd_tf::Token::new("instanceable");
                for layer in ls.get_layers() {
                    let mut authored = false;
                    let has_field = layer.has_field_typed(&site.path, &field, Some(&mut authored));
                    if has_field {
                        manual_authored_instanceable.get_or_insert(authored);
                        eprintln!(
                            "    authored instanceable on {}{} = {}",
                            layer_filename(&layer.identifier()),
                            site.path,
                            authored
                        );
                    }
                }
            }
            true
        });
        eprintln!("manual_has_instanceable_data={manual_has_instanceable_data}");
        eprintln!("manual_authored_instanceable={manual_authored_instanceable:?}");
        eprintln!("{}", dump_prim_index(&prim_index, false, false));
    }
}

#[test]
#[ignore]
fn debug_basic_owner_layer_stack() {
    ensure_init();

    let root_path = testenv_path("museum/BasicOwner/root.usda");
    let session_path = testenv_path("museum/BasicOwner/session.usda");
    let id = LayerStackIdentifier::with_session(root_path.as_str(), Some(session_path.as_str()));
    let cache = Cache::new(id, true);

    let layers: Vec<String> = cache
        .layer_stack()
        .expect("layer stack must exist")
        .get_layers()
        .into_iter()
        .map(|layer| layer_filename(&layer.identifier()))
        .collect();
    eprintln!("owner layers={layers:?}");
}

#[test]
#[ignore]
fn debug_basic_list_editing_fields() {
    ensure_init();

    let root_path = testenv_path("museum/BasicListEditing/root.usda");
    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id.clone(), true);
    let layer_stack = cache
        .compute_layer_stack(&id)
        .expect("layer stack must exist");
    let path = Path::from_string("/A").unwrap();
    let children = usd_tf::Token::new("primChildren");
    let prim_order = usd_tf::Token::new("primOrder");
    let name_children_order = usd_tf::Token::new("nameChildrenOrder");

    for layer in layer_stack.get_layers() {
        let layer_name = layer_filename(&layer.identifier());
        let prim_children = layer.get_field_as_token_vector(&path, &children);
        let order = layer.get_field_as_token_vector(&path, &prim_order);
        let name_order = layer.get_field_as_token_vector(&path, &name_children_order);
        eprintln!(
            "{layer_name}: primChildren={prim_children:?} primOrder={order:?} nameChildrenOrder={name_order:?}"
        );
    }

    let (prim_index, errors) = cache.compute_prim_index(&path);
    eprintln!("errors={errors:?}");
    eprintln!("children={:?}", get_child_names(&prim_index));
    eprintln!("{}", dump_prim_index(&prim_index, false, false));
}

#[test]
#[ignore]
fn debug_basic_variant_with_reference_dump() {
    ensure_init();

    let root_path = testenv_path("museum/BasicVariantWithReference/root.usda");
    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id, true);
    cache.set_include_payload_predicate(Some(std::sync::Arc::new(|_path: &Path| true)));
    let model_layer =
        usd_sdf::Layer::find_or_open(testenv_path("museum/BasicVariantWithReference/model.usda"))
            .expect("model layer must open");
    let refs_tk = usd_tf::Token::new("references");
    let inherits_tk = usd_tf::Token::new("inheritPaths");
    for raw_path in [
        "/Model{vset=with_children}_prototype",
        "/Model{vset=with_children}InstanceViaReference",
        "/Model{vset=with_children}InstanceViaClass",
    ] {
        let path = Path::from_string(raw_path).unwrap();
        eprintln!(
            "LAYER {} path={} has_spec={} has_refs={} has_inherits={} refs={:?} inherits={:?}",
            layer_filename(&model_layer.identifier()),
            raw_path,
            model_layer.has_spec(&path),
            model_layer.has_field(&path, &refs_tk),
            model_layer.has_field(&path, &inherits_tk),
            model_layer.get_reference_list_op(&path),
            model_layer.get_inherit_paths_list_op(&path),
        );
    }

    for prim_path in [
        "/ModelRefWithChildren",
        "/ModelRefWithChildren/_prototype",
        "/ModelRefWithChildren/InstanceViaReference",
        "/ModelRefWithChildren/InstanceViaClass",
    ] {
        let path = Path::from_string(prim_path).unwrap();
        let (prim_index, errors) = cache.compute_prim_index(&path);
        eprintln!("PRIM {prim_path}");
        eprintln!("errors={errors:?}");
        eprintln!("stack={:?}", get_prim_stack(&prim_index));
        eprintln!("children={:?}", get_child_names(&prim_index));
        let mut stack_nodes = vec![prim_index.root_node()];
        while let Some(node) = stack_nodes.pop() {
            if !node.is_valid() {
                continue;
            }
            let layers: Vec<String> = node
                .layer_stack()
                .map(|ls| {
                    ls.get_layers()
                        .into_iter()
                        .map(|layer| layer_filename(&layer.identifier()))
                        .collect()
                })
                .unwrap_or_default();
            let refs = node
                .layer_stack()
                .map(|ls| usd_pcp::compose_site_references(&ls, &node.path()).0)
                .unwrap_or_default();
            let inherits = node
                .layer_stack()
                .map(|ls| usd_pcp::compose_site_inherits(&ls, &node.path()))
                .unwrap_or_default();
            eprintln!(
                "  NODE arc={:?} path={} can_specs={} has_specs={} layers={layers:?} refs={refs:?} inherits={inherits:?}",
                node.arc_type(),
                node.path(),
                node.can_contribute_specs(),
                node.has_specs(),
            );
            let children = node.children();
            for child in children.into_iter().rev() {
                stack_nodes.push(child);
            }
        }
        eprintln!("{}", dump_prim_index(&prim_index, false, false));
    }

    let direct_model_id =
        LayerStackIdentifier::new(testenv_path("museum/BasicVariantWithReference/model.usda"));
    let direct_model_cache = Cache::new(direct_model_id, true);
    for prim_path in ["/Model/InstanceViaReference", "/Model/InstanceViaClass"] {
        let path = Path::from_string(prim_path).unwrap();
        let (prim_index, errors) = direct_model_cache.compute_prim_index(&path);
        eprintln!("DIRECT PRIM {prim_path}");
        eprintln!("errors={errors:?}");
        eprintln!("stack={:?}", get_prim_stack(&prim_index));
        eprintln!("{}", dump_prim_index(&prim_index, false, false));
    }
}

#[test]
#[ignore]
fn debug_typical_rigged_model_dump() {
    ensure_init();

    let root_path = testenv_path("museum/TypicalReferenceToRiggedModel/root.usda");
    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id, true);
    cache.set_include_payload_predicate(Some(std::sync::Arc::new(|_path: &Path| true)));

    for prim_path in ["/Model"] {
        let path = Path::from_string(prim_path).unwrap();
        let (prim_index, errors) = cache.compute_prim_index(&path);
        eprintln!("ROOT PRIM {prim_path}");
        eprintln!("errors={errors:?}");
        eprintln!("stack={:?}", get_prim_stack(&prim_index));
        eprintln!("{}", dump_prim_index(&prim_index, false, false));
    }

    for layer_rel in [
        "museum/TypicalReferenceToRiggedModel/mcat.usda",
        "museum/TypicalReferenceToRiggedModel/model_latest.usda",
    ] {
        let layer_id = LayerStackIdentifier::new(testenv_path(layer_rel));
        let layer_cache = Cache::new(layer_id, true);
        layer_cache.set_include_payload_predicate(Some(std::sync::Arc::new(|_path: &Path| true)));
        let path = Path::from_string("/Model").unwrap();
        let (prim_index, errors) = layer_cache.compute_prim_index(&path);
        eprintln!("DIRECT LAYER {layer_rel} /Model");
        eprintln!("errors={errors:?}");
        eprintln!("stack={:?}", get_prim_stack(&prim_index));
        eprintln!("{}", dump_prim_index(&prim_index, false, false));
    }
}

#[test]
#[ignore]
fn debug_basic_specializes_dump() {
    ensure_init();

    let root_path = testenv_path("museum/BasicSpecializes/root.usda");
    let id = LayerStackIdentifier::new(root_path.as_str());
    let cache = Cache::new(id, true);
    cache.set_include_payload_predicate(Some(std::sync::Arc::new(|_path: &Path| true)));

    for prim_path in ["/Root", "/Model/Looks/Brass", "/MultipleRefsAndSpecializes"] {
        let path = Path::from_string(prim_path).unwrap();
        let (prim_index, errors) = cache.compute_prim_index(&path);
        eprintln!("PRIM {prim_path}");
        eprintln!("errors={errors:?}");
        eprintln!("stack={:?}", get_prim_stack(&prim_index));
        let root = prim_index.root_node();
        let children = root.children();
        for (idx, child) in children.iter().enumerate() {
            let child_ls = child
                .layer_stack()
                .map(|ls| {
                    ls.get_layers()
                        .into_iter()
                        .map(|layer| layer_filename(&layer.identifier()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let origin = child.origin_node();
            let origin_ls = origin
                .layer_stack()
                .map(|ls| {
                    ls.get_layers()
                        .into_iter()
                        .map(|layer| layer_filename(&layer.identifier()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            eprintln!(
                "ROOT_CHILD[{idx}] arc={:?} path={} ns={} depth={} sib={} site={} origin={} origin_site={} child_ls={child_ls:?} origin_ls={origin_ls:?}",
                child.arc_type(),
                child.path(),
                child.namespace_depth(),
                child.depth_below_introduction(),
                child.sibling_num_at_origin(),
                child.site().path,
                origin.path(),
                origin.site().path,
            );
        }
        for i in 0..children.len() {
            for j in (i + 1)..children.len() {
                eprintln!(
                    "CMP [{}:{}] vs [{}:{}] = {}",
                    i,
                    children[i].path(),
                    j,
                    children[j].path(),
                    compare_sibling_node_strength(&children[i], &children[j]),
                );
            }
        }
        eprintln!("{}", dump_prim_index(&prim_index, true, false));
        // Dump ALL nodes with inert/culled status
        if let Some(graph) = prim_index.graph() {
            eprintln!("=== ALL NODES ({}) ===", graph.num_nodes());
            for i in 0..graph.num_nodes() {
                let n = usd_pcp::NodeRef::new(graph.clone(), i);
                let ls_name = n
                    .layer_stack()
                    .map(|ls| {
                        ls.get_layers()
                            .into_iter()
                            .map(|l| layer_filename(&l.identifier()))
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default();
                let parent_idx = n.parent_node().node_index();
                let origin_idx = n.origin_node().node_index();
                eprintln!(
                    "  [{i}] arc={:?} path={} ls=[{ls_name}] inert={} culled={} restrict={} can_contrib={} has_specs={} depth_intro={} parent={parent_idx} origin={origin_idx}",
                    n.arc_type(),
                    n.path(),
                    n.is_inert(),
                    n.is_culled(),
                    n.spec_contribution_restricted_depth(),
                    n.can_contribute_specs(),
                    n.has_specs(),
                    n.depth_below_introduction()
                );
            }
        }
    }
}
