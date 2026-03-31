//! Basic integration tests for PCP composition.
//!
//! These tests create actual USDA layers on disk and verify
//! that PcpCache correctly composes prim indexes.

use std::io::Write;
use usd_pcp::{ArcType, Cache, LayerStackIdentifier};
use usd_sdf::Path;

/// Initialize file format registry (must be called before loading layers).
fn ensure_init() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        usd_sdf::init();
    });
}

/// Helper to create a temporary USDA file and return its path.
fn write_temp_usda(name: &str, content: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let file_path = dir.path().join(name);
    let mut file = std::fs::File::create(&file_path).expect("failed to create file");
    file.write_all(content.as_bytes()).expect("failed to write");
    let path_str = file_path.to_string_lossy().replace('\\', "/");
    (dir, path_str)
}

/// Test: trivial single-layer composition with no arcs.
/// Verifies that PcpCache can compute a prim index for a simple prim.
#[test]
fn test_trivial_prim_index() {
    ensure_init();
    let usda = r#"#usda 1.0

def Scope "World"
{
    def Scope "Geometry"
    {
    }
}
"#;
    let (_dir, path) = write_temp_usda("trivial.usda", usda);
    let id = LayerStackIdentifier::new(path.as_str());
    let cache = Cache::new(id, true);

    // Compute prim index for /World
    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/World").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(prim_index.is_valid());

    let root_node = prim_index.root_node();
    assert!(root_node.is_valid());
    assert_eq!(root_node.arc_type(), ArcType::Root);
    assert_eq!(root_node.path(), Path::from_string("/World").unwrap());

    // Root node should have specs
    assert!(
        prim_index.has_specs(),
        "prim index should have specs for /World"
    );

    // Compute prim index for /World/Geometry
    let (geom_index, errors) =
        cache.compute_prim_index(&Path::from_string("/World/Geometry").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(geom_index.is_valid());
    assert_eq!(
        geom_index.root_node().path(),
        Path::from_string("/World/Geometry").unwrap()
    );
}

/// Test: internal reference composition.
/// Verifies that internal references (</SourcePrim>) are resolved correctly.
#[test]
fn test_internal_reference() {
    ensure_init();
    let usda = r#"#usda 1.0

def Scope "Source"
{
    custom string myAttr = "hello"
}

def Scope "Target" (
    references = </Source>
)
{
}
"#;
    let (_dir, path) = write_temp_usda("internal_ref.usda", usda);
    let id = LayerStackIdentifier::new(path.as_str());
    let cache = Cache::new(id, true);

    // Compute prim index for /Target
    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/Target").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(prim_index.is_valid());

    let root_node = prim_index.root_node();
    assert!(root_node.is_valid());
    assert_eq!(root_node.arc_type(), ArcType::Root);

    // /Target should have at least 2 nodes: root + reference
    let num_nodes = prim_index.num_nodes();
    assert!(
        num_nodes >= 2,
        "Expected at least 2 nodes (root + ref), got {}",
        num_nodes
    );

    // Check that we have a reference arc child
    let children: Vec<_> = root_node.children_range().collect();
    assert!(
        !children.is_empty(),
        "Root node should have children (reference arc)"
    );

    // The first child should be a Reference arc pointing to /Source
    let ref_child = &children[0];
    assert_eq!(
        ref_child.arc_type(),
        ArcType::Reference,
        "First child should be a Reference arc"
    );
    assert_eq!(
        ref_child.path(),
        Path::from_string("/Source").unwrap(),
        "Reference should point to /Source"
    );
}

/// Test: inherit arc composition.
/// Verifies that inherits are resolved correctly.
#[test]
fn test_inherit_arc() {
    ensure_init();
    let usda = r#"#usda 1.0

class Scope "_class_Model"
{
    custom string name = "from_class"
}

def Scope "Instance" (
    inherits = </_class_Model>
)
{
}
"#;
    let (_dir, path) = write_temp_usda("inherit.usda", usda);
    let id = LayerStackIdentifier::new(path.as_str());
    let cache = Cache::new(id, true);

    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/Instance").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(prim_index.is_valid());

    // Should have inherit arc
    let children: Vec<_> = prim_index.root_node().children_range().collect();
    assert!(
        !children.is_empty(),
        "Should have at least one child (inherit arc)"
    );

    let has_inherit = children.iter().any(|c| c.arc_type() == ArcType::Inherit);
    assert!(has_inherit, "Should have an inherit arc child");
}

/// Test: variant selection composition.
#[test]
fn test_variant_selection() {
    ensure_init();
    let usda = r#"#usda 1.0

def Scope "Model" (
    add variantSets = "shade"
    variants = {
        string shade = "red"
    }
)
{
    variantSet "shade" = {
        "red" {
            custom string color = "red"
        }
        "blue" {
            custom string color = "blue"
        }
    }
}
"#;
    let (_dir, path) = write_temp_usda("variant.usda", usda);
    let id = LayerStackIdentifier::new(path.as_str());
    let cache = Cache::new(id, true);

    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/Model").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(prim_index.is_valid());

    // Should have a variant arc child
    let children: Vec<_> = prim_index.root_node().children_range().collect();
    let has_variant = children.iter().any(|c| c.arc_type() == ArcType::Variant);
    assert!(has_variant, "Should have a variant arc child");

    // Verify variant selection was applied
    let selection = prim_index.get_selection_applied_for_variant_set("shade");
    assert_eq!(
        selection.as_deref(),
        Some("red"),
        "Variant selection should be 'red'"
    );
}

/// Test: external reference composition (two USDA files).
#[test]
fn test_external_reference() {
    ensure_init();
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Write referenced file
    let ref_path = dir.path().join("ref.usda");
    std::fs::write(
        &ref_path,
        r#"#usda 1.0

def Scope "RefPrim"
{
    custom string refAttr = "from_ref"

    def Scope "Child"
    {
    }
}
"#,
    )
    .unwrap();

    // Write root file referencing it
    let root_path = dir.path().join("root.usda");
    std::fs::write(
        &root_path,
        r#"#usda 1.0

def Scope "MyPrim" (
    references = @./ref.usda@</RefPrim>
)
{
}
"#,
    )
    .unwrap();

    let root_str = root_path.to_string_lossy().replace('\\', "/");
    let id = LayerStackIdentifier::new(root_str.as_str());
    let cache = Cache::new(id, true);

    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/MyPrim").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(prim_index.is_valid());

    // Should have reference arc
    let children: Vec<_> = prim_index.root_node().children_range().collect();
    assert!(!children.is_empty(), "Should have reference arc children");

    let ref_child = &children[0];
    assert_eq!(ref_child.arc_type(), ArcType::Reference);
    assert_eq!(ref_child.path(), Path::from_string("/RefPrim").unwrap());

    // Child prims should be available
    let (child_index, errors) =
        cache.compute_prim_index(&Path::from_string("/MyPrim/Child").unwrap());
    assert!(errors.is_empty(), "child errors: {:?}", errors);
    assert!(child_index.is_valid());
}

/// Test: sublayer composition.
#[test]
fn test_sublayer() {
    ensure_init();
    let dir = tempfile::tempdir().expect("failed to create temp dir");

    // Write sublayer
    let sub_path = dir.path().join("sublayer.usda");
    std::fs::write(
        &sub_path,
        r#"#usda 1.0

def Scope "World"
{
    custom string subAttr = "from_sublayer"
}
"#,
    )
    .unwrap();

    // Write root with sublayer
    let root_path = dir.path().join("root.usda");
    std::fs::write(
        &root_path,
        r#"#usda 1.0
(
    subLayers = [
        @./sublayer.usda@
    ]
)

over "World"
{
    custom string rootAttr = "from_root"
}
"#,
    )
    .unwrap();

    let root_str = root_path.to_string_lossy().replace('\\', "/");
    let id = LayerStackIdentifier::new(root_str.as_str());
    let cache = Cache::new(id.clone(), true);

    // Layer stack should include both layers
    let layer_stack = cache
        .compute_layer_stack(&id)
        .expect("Failed to compute layer stack");
    let layers = layer_stack.get_layers();
    assert!(
        layers.len() >= 2,
        "Layer stack should have at least 2 layers (root + sublayer), got {}",
        layers.len()
    );

    // Prim index for /World should have specs from both layers
    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/World").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);
    assert!(prim_index.is_valid());
    assert!(prim_index.has_specs());
}

/// Test: path translation through reference arc.
#[test]
fn test_path_translation() {
    ensure_init();
    let usda = r#"#usda 1.0

def Scope "Source"
{
    def Scope "Child"
    {
    }
}

def Scope "Target" (
    references = </Source>
)
{
}
"#;
    let (_dir, path) = write_temp_usda("path_xlate.usda", usda);
    let id = LayerStackIdentifier::new(path.as_str());
    let cache = Cache::new(id, true);

    let (prim_index, errors) = cache.compute_prim_index(&Path::from_string("/Target").unwrap());
    assert!(errors.is_empty(), "errors: {:?}", errors);

    let root_node = prim_index.root_node();
    let ref_nodes: Vec<_> = root_node
        .children_range()
        .filter(|n| n.arc_type() == ArcType::Reference)
        .collect();
    assert!(!ref_nodes.is_empty(), "Should have reference node");

    let ref_node = &ref_nodes[0];

    // Translate /Target from root to reference node -> should give /Source
    let (translated, _exact) =
        usd_pcp::translate_path_from_root_to_node(ref_node, &Path::from_string("/Target").unwrap());
    assert_eq!(
        translated,
        Path::from_string("/Source").unwrap(),
        "Path /Target should translate to /Source in reference node"
    );

    // Translate /Source from reference node to root -> should give /Target
    let (back, _exact) =
        usd_pcp::translate_path_from_node_to_root(ref_node, &Path::from_string("/Source").unwrap());
    assert_eq!(
        back,
        Path::from_string("/Target").unwrap(),
        "Path /Source should translate to /Target from reference node"
    );
}
