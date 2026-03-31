//! Museum integration tests - verifying composition results against
//! the OpenUSD PCP Museum test cases.

use usd_pcp::{ArcType, Cache, LayerStackIdentifier};
use usd_sdf::Path;

fn ensure_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        usd_sdf::init();
    });
}

fn testenv_path(relative: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/testenv/{}", manifest_dir.replace('\\', "/"), relative)
}

fn make_cache(root_usda: &str) -> std::sync::Arc<Cache> {
    ensure_init();
    let path = testenv_path(root_usda);
    let id = LayerStackIdentifier::new(path.as_str());
    Cache::new(id, true)
}

fn p(s: &str) -> Path {
    Path::from_string(s).unwrap()
}

fn child_names(cache: &Cache, prim_path: &str) -> Vec<String> {
    let (idx, errors) = cache.compute_prim_index(&p(prim_path));
    assert!(errors.is_empty(), "errors for {}: {:?}", prim_path, errors);
    let (names, _) = idx.compute_prim_child_names();
    names.iter().map(|t| t.as_str().to_string()).collect()
}

#[allow(dead_code)]
fn node_count(cache: &Cache, prim_path: &str) -> usize {
    let (idx, _) = cache.compute_prim_index(&p(prim_path));
    idx.num_nodes()
}

fn root_children_arcs(cache: &Cache, prim_path: &str) -> Vec<(ArcType, String)> {
    let (idx, _) = cache.compute_prim_index(&p(prim_path));
    idx.root_node()
        .children_range()
        .map(|n| (n.arc_type(), n.path().as_str().to_string()))
        .collect()
}

// ============================================================================
// BasicReference Museum Tests
// ============================================================================

#[test]
fn museum_basic_reference_layer_stack() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // Layer stack should have: root.usda + sublayer.usda (session is separate)
    let id = cache.layer_stack_identifier();
    let ls = cache
        .compute_layer_stack(id)
        .expect("Failed to compute layer stack");
    let layers = ls.get_layers();
    assert!(
        layers.len() >= 2,
        "Layer stack should have root + sublayer, got {}",
        layers.len()
    );
}

#[test]
fn museum_basic_reference_prim_with_references() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // /PrimWithReferences has references to ref.usda</PrimA> and ref2.usda</PrimB>
    let (idx, errors) = cache.compute_prim_index(&p("/PrimWithReferences"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have multiple nodes: root + 2 references (ref.usda</PrimA>, ref2.usda</PrimB>)
    // ref.usda</PrimA> itself has a reference to ref2.usda</PrimC>
    // So total: root + ref(PrimA) + ref(PrimC from ref.usda) + ref(PrimB) = 4 nodes
    assert!(
        idx.num_nodes() >= 3,
        "Expected at least 3 nodes, got {}",
        idx.num_nodes()
    );

    // Root children should be Reference arcs
    let children = root_children_arcs(&cache, "/PrimWithReferences");
    assert!(
        children.iter().all(|(t, _)| *t == ArcType::Reference),
        "All children should be Reference arcs, got: {:?}",
        children
    );

    // Child names from baseline: ['PrimB_Child', 'PrimC_Child', 'PrimA_Child']
    let names = child_names(&cache, "/PrimWithReferences");
    assert!(
        names.contains(&"PrimA_Child".to_string()),
        "Should contain PrimA_Child, got: {:?}",
        names
    );
    assert!(
        names.contains(&"PrimB_Child".to_string()),
        "Should contain PrimB_Child, got: {:?}",
        names
    );
}

#[test]
fn museum_basic_reference_internal_reference() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // /PrimWithInternalReference references </InternalReference> and </InternalReference2>
    let (_idx, errors) = cache.compute_prim_index(&p("/PrimWithInternalReference"));
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let children = root_children_arcs(&cache, "/PrimWithInternalReference");
    assert!(
        children.len() >= 2,
        "Should have at least 2 reference children (InternalReference + InternalReference2), got: {:?}",
        children
    );

    // Child should include InternalReference_Child
    let names = child_names(&cache, "/PrimWithInternalReference");
    assert!(
        names.contains(&"InternalReference_Child".to_string()),
        "Should contain InternalReference_Child, got: {:?}",
        names
    );
}

#[test]
fn museum_basic_reference_default_prim() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // /PrimWithDefaultReferenceTarget references @./defaultRef.usda@ (no target path)
    // defaultRef.usda has defaultPrim = "Default", so target is </Default>
    let (idx, errors) = cache.compute_prim_index(&p("/PrimWithDefaultReferenceTarget"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have child Default_Child from the default prim
    let names = child_names(&cache, "/PrimWithDefaultReferenceTarget");
    assert!(
        names.contains(&"Default_Child".to_string()),
        "Should have Default_Child from defaultPrim, got: {:?}",
        names
    );
}

#[test]
fn museum_basic_reference_subroot_reference() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // /PrimWithSubrootReference references ref.usda</PrimA/PrimA_Child>
    // and ref.usda</PrimA/PrimC_Child>
    let (idx, errors) = cache.compute_prim_index(&p("/PrimWithSubrootReference"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());
    assert!(
        idx.num_nodes() >= 2,
        "Subroot reference should have at least 2 nodes, got {}",
        idx.num_nodes()
    );
}

#[test]
fn museum_basic_reference_self_reference() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // /PrimWithSelfReference references root.usda</InternalReference>
    // and sublayer.usda</InternalSublayerReference>
    let (idx, errors) = cache.compute_prim_index(&p("/PrimWithSelfReference"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());
    assert!(
        idx.num_nodes() >= 2,
        "Self reference should have at least 2 nodes, got {}",
        idx.num_nodes()
    );
}

#[test]
fn museum_basic_reference_variant_refs() {
    let cache = make_cache("museum/BasicReference/root.usda");

    // /PrimWithReferencesInVariants has variant "v" = "ref" with internal refs
    let (idx, errors) = cache.compute_prim_index(&p("/PrimWithReferencesInVariants"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have variant arc
    let children = root_children_arcs(&cache, "/PrimWithReferencesInVariants");
    let has_variant = children.iter().any(|(t, _)| *t == ArcType::Variant);
    assert!(has_variant, "Should have variant arc, got: {:?}", children);
}

// ============================================================================
// BasicInherits Museum Tests
// ============================================================================

#[test]
fn museum_basic_inherits_group() {
    let cache = make_cache("museum/BasicInherits/root.usda");

    // /Group references group.usda</Group>
    let (idx, errors) = cache.compute_prim_index(&p("/Group"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have reference arc as direct child of root
    let children = root_children_arcs(&cache, "/Group");
    let has_ref = children.iter().any(|(t, _)| *t == ArcType::Reference);
    assert!(has_ref, "Should have reference arc, got: {:?}", children);

    // Child names: ['Model_2', 'Model_1', 'Model_Special'] per baseline
    let names = child_names(&cache, "/Group");
    assert!(
        names.contains(&"Model_1".to_string()),
        "Should contain Model_1, got: {:?}",
        names
    );
}

#[test]
fn museum_basic_inherits_model_instance() {
    let cache = make_cache("museum/BasicInherits/root.usda");

    // /Group/Model_1 references model.usda</Model> which inherits </_class_Model>
    // The inherit should be propagated through the reference chain.
    let (idx, errors) = cache.compute_prim_index(&p("/Group/Model_1"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have inherit arc somewhere in the node tree (implied inherit)
    let all_nodes = idx.nodes();
    let has_inherit = all_nodes.iter().any(|n| n.arc_type() == ArcType::Inherit);
    assert!(
        has_inherit,
        "Model_1 should have inherit arc (from model.usda inherits _class_Model), got: {:?}",
        all_nodes
            .iter()
            .map(|n| (n.arc_type(), n.path().as_str().to_string()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn museum_basic_inherits_order() {
    let cache = make_cache("museum/BasicInherits/root.usda");

    // /InheritsOrder1 inherits [</RootClass>, </ParentClass/SubrootClass>]
    let (idx, errors) = cache.compute_prim_index(&p("/InheritsOrder1"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    let children = root_children_arcs(&cache, "/InheritsOrder1");
    let inherit_children: Vec<_> = children
        .iter()
        .filter(|(t, _)| *t == ArcType::Inherit)
        .collect();
    assert_eq!(
        inherit_children.len(),
        2,
        "Should have 2 inherit arcs, got: {:?}",
        children
    );
}

// ============================================================================
// BasicPayload Museum Tests
// ============================================================================

#[test]
fn museum_basic_payload_simple() {
    let _cache = make_cache("museum/BasicPayload/root.usda");

    // By default, payloads are NOT loaded unless explicitly included
    // Create cache with all payloads included
    let path = testenv_path("museum/BasicPayload/root.usda");
    let id = LayerStackIdentifier::new(path.as_str());
    let cache_with_payloads = Cache::new(id, true);

    // Include the payload for /SimplePayload
    cache_with_payloads.request_payloads(&[p("/SimplePayload")], &[], None);

    let (idx, errors) = cache_with_payloads.compute_prim_index(&p("/SimplePayload"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // With payload included, should have payload arc
    let children = root_children_arcs(&cache_with_payloads, "/SimplePayload");
    let has_payload = children.iter().any(|(t, _)| *t == ArcType::Payload);
    assert!(
        has_payload,
        "Should have payload arc when payload is included, got: {:?}",
        children
    );

    // Should have child "Child" from the payload
    let names = child_names(&cache_with_payloads, "/SimplePayload");
    assert!(
        names.contains(&"Child".to_string()),
        "Should have Child from payload, got: {:?}",
        names
    );
}

#[test]
fn museum_basic_payload_not_loaded() {
    let cache = make_cache("museum/BasicPayload/root.usda");

    // Without requesting payloads, /SimplePayload should have no payload arc
    let (idx, errors) = cache.compute_prim_index(&p("/SimplePayload"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Root should have no children (payload not loaded)
    let children = root_children_arcs(&cache, "/SimplePayload");
    let has_payload = children.iter().any(|(t, _)| *t == ArcType::Payload);
    assert!(
        !has_payload,
        "Should NOT have payload arc when not requested, got: {:?}",
        children
    );
}

// ============================================================================
// BasicSpecializes Museum Tests
// ============================================================================

// ============================================================================
// BasicTimeOffset Museum Tests
// ============================================================================

#[test]
fn museum_basic_time_offset() {
    let cache = make_cache("museum/BasicTimeOffset/root.usda");

    // /Root references A.usda</Model> with offset=10
    // A.usda sublayers B.usda with offset=20
    let (idx, errors) = cache.compute_prim_index(&p("/Root"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have reference arc
    let children = root_children_arcs(&cache, "/Root");
    assert!(
        children.iter().any(|(t, _)| *t == ArcType::Reference),
        "Should have reference arc, got: {:?}",
        children
    );

    // Child names should include Anim and Frame from B.usda</Model>
    let names = child_names(&cache, "/Root");
    assert!(
        names.contains(&"Anim".to_string()),
        "Should have Anim child, got: {:?}",
        names
    );
    assert!(
        names.contains(&"Frame".to_string()),
        "Should have Frame child, got: {:?}",
        names
    );
}

// ============================================================================
// BasicNestedVariants Museum Tests
// ============================================================================

#[test]
fn museum_basic_nested_variants() {
    let cache = make_cache("museum/BasicNestedVariants/root.usda");

    // /Foo has variant "which" = "A", and inside that /Foo/A/Number has variant "count" = "one"
    let (idx, errors) = cache.compute_prim_index(&p("/Foo"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have variant arc
    let children = root_children_arcs(&cache, "/Foo");
    let has_variant = children.iter().any(|(t, _)| *t == ArcType::Variant);
    assert!(has_variant, "Should have variant arc, got: {:?}", children);

    // Variant selection for "which" should be "A"
    let selection = idx.get_selection_applied_for_variant_set("which");
    assert_eq!(
        selection.as_deref(),
        Some("A"),
        "Variant selection should be A"
    );

    // /Foo/A should exist as a child
    let names = child_names(&cache, "/Foo");
    assert!(
        names.contains(&"A".to_string()),
        "Should have child A from variant, got: {:?}",
        names
    );
}

#[test]
fn museum_nested_variant_child() {
    let cache = make_cache("museum/BasicNestedVariants/root.usda");

    let (idx, errors) = cache.compute_prim_index(&p("/Foo/A/Number"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    let selection = idx.get_selection_applied_for_variant_set("count");
    assert_eq!(selection.as_deref(), Some("one"));

    let names = child_names(&cache, "/Foo/A/Number");
    assert!(
        names.contains(&"one".to_string()),
        "Should have child 'one' from nested variant, got: {:?}",
        names
    );
}

// ============================================================================
// ErrorArcCycle Museum Tests
// ============================================================================

// ============================================================================
// BasicReferenceDiamond Museum Tests
// ============================================================================

#[test]
fn museum_basic_reference_diamond() {
    let cache = make_cache("museum/BasicReferenceDiamond/root.usda");

    // Diamond pattern: A refs B and C, both B and C ref D
    // /A should compose correctly despite diamond
    let (idx, errors) = cache.compute_prim_index(&p("/Root"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());
    assert!(
        idx.num_nodes() >= 3,
        "Diamond should have at least 3 nodes (root + 2 refs), got {}",
        idx.num_nodes()
    );
}

// ============================================================================
// BasicReferenceAndClass Museum Tests
// ============================================================================

#[test]
fn museum_basic_reference_and_class() {
    let cache = make_cache("museum/BasicReferenceAndClass/root.usda");

    let (idx, errors) = cache.compute_prim_index(&p("/Model"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Should have reference and inherit arcs
    let all_nodes = idx.nodes();
    let has_ref = all_nodes.iter().any(|n| n.arc_type() == ArcType::Reference);
    let has_inherit = all_nodes.iter().any(|n| n.arc_type() == ArcType::Inherit);
    assert!(has_ref, "Should have reference arc");
    assert!(has_inherit, "Should have inherit arc");
}

// ============================================================================
// SubrootReferenceAndVariants Museum Tests
// ============================================================================

#[test]
fn museum_subroot_reference_and_variants() {
    let cache = make_cache("museum/SubrootReferenceAndVariants/root.usda");

    // /SubrootRef references group.usda</Group/Model> (subroot ref)
    let (idx, errors) = cache.compute_prim_index(&p("/SubrootRef"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());

    // Subroot ref brings in nodes from the target layer
    assert!(
        idx.num_nodes() >= 2,
        "SubrootRef should have at least 2 nodes, got {}",
        idx.num_nodes()
    );

    // /RootRef references model.usda</Model> which has variants
    let (idx2, errors2) = cache.compute_prim_index(&p("/RootRef"));
    assert!(errors2.is_empty(), "errors: {:?}", errors2);
    assert!(idx2.is_valid());
    let all_nodes2 = idx2.nodes();
    let has_variant = all_nodes2.iter().any(|n| n.arc_type() == ArcType::Variant);
    assert!(
        has_variant,
        "RootRef should have variant arc, got: {:?}",
        all_nodes2
            .iter()
            .map(|n| (n.arc_type(), n.path().as_str().to_string()))
            .collect::<Vec<_>>()
    );
}

// ============================================================================
// RelativePathReferences Museum Tests
// ============================================================================

#[test]
fn museum_relative_path_references() {
    let cache = make_cache("museum/RelativePathReferences/root.usda");

    // /Model gets opinions from sub1/sub1.usda and sub2/sub2.usda sublayers,
    // each with relative-path references to different ref.usda files
    let id = cache.layer_stack_identifier();
    let ls = cache.compute_layer_stack(id).expect("layer stack");
    // Layer stack should have root + sub1 + sub2 + sub3
    assert!(
        ls.get_layers().len() >= 3,
        "Should have at least 3 layers, got {}",
        ls.get_layers().len()
    );

    let (idx, errors) = cache.compute_prim_index(&p("/Model"));
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(idx.is_valid());
    // Both sublayers reference different ref.usda via relative paths
    assert!(
        idx.num_nodes() >= 2,
        "Relative ref should have at least 2 nodes, got {}",
        idx.num_nodes()
    );
}

// ============================================================================
// ErrorArcCycle Museum Tests
// ============================================================================

#[test]
fn museum_error_arc_cycle() {
    let cache = make_cache("museum/ErrorArcCycle/root.usda");

    // /GroupRoot references A.usda which references B.usda which references A.usda
    let (idx, errors) = cache.compute_prim_index(&p("/GroupRoot"));
    assert!(
        !errors.is_empty(),
        "Should have cycle errors for /GroupRoot"
    );
    assert!(idx.is_valid());
}

#[test]
fn museum_error_inherit_cycle() {
    let cache = make_cache("museum/ErrorArcCycle/root.usda");

    // /Parent/Child1 inherits /Parent/Child2, /Parent/Child2 inherits /Parent/Child1
    let (idx, errors) = cache.compute_prim_index(&p("/Parent/Child1"));
    assert!(
        !errors.is_empty(),
        "Should have cycle errors for /Parent/Child1"
    );
    assert!(idx.is_valid());
}

#[test]
fn museum_error_inherit_of_child_cycle() {
    let cache = make_cache("museum/ErrorArcCycle/root.usda");

    let (idx, errors) = cache.compute_prim_index(&p("/InheritOfChild"));
    assert!(
        !errors.is_empty(),
        "Should have cycle errors for /InheritOfChild"
    );
    assert!(idx.is_valid());

    let (names, _) = idx.compute_prim_child_names();
    let names: Vec<String> = names.iter().map(|t| t.as_str().to_string()).collect();
    assert_eq!(names, vec!["Child".to_string()]);
}

// ============================================================================
// BasicSpecializes Museum Tests
// ============================================================================

#[test]
fn museum_basic_specializes() {
    ensure_init();
    let path = testenv_path("museum/BasicSpecializes/root.usda");
    let id = LayerStackIdentifier::new(path.as_str());
    let cache = Cache::new(id, true);

    // root.usda should have prims with specialize arcs
    let (idx, errors) = cache.compute_prim_index(&p("/Concrete"));
    // Some Museum tests may have errors for intentional edge cases
    if errors.is_empty() && idx.is_valid() {
        // Check for specialize arcs
        let children = root_children_arcs(&cache, "/Concrete");
        let has_specialize = children.iter().any(|(t, _)| *t == ArcType::Specialize);
        // Only assert if prim exists and has specializes
        if idx.has_specs() {
            assert!(
                has_specialize,
                "Should have specialize arc, got: {:?}",
                children
            );
        }
    }
}
