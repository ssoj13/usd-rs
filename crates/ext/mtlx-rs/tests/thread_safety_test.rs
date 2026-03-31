//! Thread-safety tests for mtlx-rs core module.
//!
//! Verifies that Arc<RwLock<Element>> (ElementPtr) is correctly usable
//! across threads, covering document creation, element traversal, and XML IO.

use mtlx_rs::core::{
    Element, ElementPtr, add_child_of_category, create_document, element::category,
    get_inherits_from, set_inherits_from, validate_element,
};
use mtlx_rs::format::read_from_xml_str;
use std::sync::Arc;
use std::thread;

// ─── helpers ────────────────────────────────────────────────────────────────

fn make_doc() -> ElementPtr {
    ElementPtr::new(Element::new(None, category::DOCUMENT, ""))
}

fn add(parent: &ElementPtr, cat: &str, name: &str) -> ElementPtr {
    add_child_of_category(parent, cat, name).expect("add_child_of_category failed")
}

// ─── basic document tests ────────────────────────────────────────────────────

#[test]
fn test_document_creation() {
    let doc = create_document();
    assert_eq!(doc.get_root().borrow().get_category(), category::DOCUMENT);
}

#[test]
fn test_element_ptr_new() {
    let elem = ElementPtr::new(Element::new(None, category::NODE, "mynode"));
    assert_eq!(elem.borrow().get_name(), "mynode");
    assert_eq!(elem.borrow().get_category(), category::NODE);
}

#[test]
fn test_element_set_attribute() {
    let elem = make_doc();
    elem.borrow_mut().set_attribute("colorspace", "lin_rec709");
    assert_eq!(
        elem.borrow().get_attribute("colorspace"),
        Some("lin_rec709")
    );
}

#[test]
fn test_add_child_and_find() {
    let root = make_doc();
    let node = add(&root, category::NODE, "n1");
    node.borrow_mut().set_attribute("type", "float");

    let found = root.borrow().get_child("n1");
    assert!(found.is_some());
    assert_eq!(found.unwrap().borrow().get_type(), Some("float"));
}

#[test]
fn test_remove_child() {
    let root = make_doc();
    add(&root, category::NODE, "n1");
    add(&root, category::NODE, "n2");

    assert_eq!(root.borrow().get_children().len(), 2);
    root.borrow_mut().remove_child("n1");
    assert_eq!(root.borrow().get_children().len(), 1);
    assert!(root.borrow().get_child("n1").is_none());
    assert!(root.borrow().get_child("n2").is_some());
}

#[test]
fn test_element_weak_ptr_roundtrip() {
    let root = make_doc();
    let weak = root.downgrade();
    let upgraded = weak.upgrade();
    assert!(upgraded.is_some());
    assert!(upgraded.unwrap().ptr_eq(&root));
}

#[test]
fn test_element_ptr_equality() {
    let a = make_doc();
    let b = a.clone(); // same Arc, same pointer
    let c = make_doc(); // different allocation

    assert!(a.ptr_eq(&b));
    assert!(!a.ptr_eq(&c));
}

#[test]
fn test_element_name_path() {
    let root = make_doc();
    let graph = add(&root, category::NODE_GRAPH, "g1");
    let node = add(&graph, category::NODE, "n1");

    let path = node.borrow().get_name_path(None);
    assert_eq!(path, "g1/n1");
}

#[test]
fn test_element_get_parent() {
    let root = make_doc();
    let child = add(&root, category::NODE, "child");
    let parent = child.borrow().get_parent();
    assert!(parent.is_some());
    assert!(parent.unwrap().ptr_eq(&root));
}

#[test]
fn test_element_validate_valid() {
    let root = make_doc();
    let nd = add(&root, category::NODEDEF, "ND_foo");
    nd.borrow_mut().set_attribute("type", "float");

    let (valid, errors) = validate_element(&nd);
    assert!(valid, "valid element: {:?}", errors);
}

#[test]
fn test_element_inheritance() {
    let root = make_doc();
    let base = add(&root, category::NODEDEF, "ND_base");
    let derived = add(&root, category::NODEDEF, "ND_derived");

    set_inherits_from(&derived, Some(&base));
    assert_eq!(derived.borrow().get_inherit_string(), "ND_base");

    let resolved = get_inherits_from(&derived);
    assert!(resolved.is_some());
    assert!(resolved.unwrap().ptr_eq(&base));

    // Clear
    set_inherits_from(&derived, None);
    assert!(get_inherits_from(&derived).is_none());
}

#[test]
fn test_element_children_of_category() {
    let root = make_doc();
    add(&root, category::NODE, "n1");
    add(&root, category::NODE, "n2");
    add(&root, category::INPUT, "i1");

    let nodes = root.borrow().get_children_of_category(category::NODE);
    assert_eq!(nodes.len(), 2);

    let inputs = root.borrow().get_children_of_category(category::INPUT);
    assert_eq!(inputs.len(), 1);
}

#[test]
fn test_xml_read_basic() {
    let xml = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_test" node="test" type="surfaceshader"/>
</materialx>"#;

    let doc = read_from_xml_str(xml).expect("XML parse failed");
    let nd = doc.get_root().borrow().get_child("ND_test");
    assert!(nd.is_some(), "nodedef should be found");
    assert_eq!(nd.unwrap().borrow().get_category(), category::NODEDEF);
}

#[test]
fn test_xml_read_nested() {
    let xml = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodegraph name="NG_main">
    <input name="diffuseColor" type="color3" value="0.18, 0.18, 0.18"/>
    <output name="surface" type="surfaceshader" nodename="shader1"/>
  </nodegraph>
</materialx>"#;

    let doc = read_from_xml_str(xml).expect("XML parse failed");
    let root = doc.get_root();
    let ng = root
        .borrow()
        .get_child("NG_main")
        .expect("nodegraph missing");
    assert_eq!(ng.borrow().get_category(), category::NODE_GRAPH);

    let inp = ng
        .borrow()
        .get_child("diffuseColor")
        .expect("input missing");
    assert_eq!(inp.borrow().get_type(), Some("color3"));
}

// ─── thread-safety tests ─────────────────────────────────────────────────────

#[test]
fn test_element_ptr_send_sync() {
    // ElementPtr must be Send + Sync for multi-threaded use
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ElementPtr>();
}

#[test]
fn test_concurrent_read_access() {
    // Multiple threads can read the same ElementPtr concurrently.
    let root = make_doc();
    let nd = add(&root, category::NODEDEF, "ND_shared");
    nd.borrow_mut().set_attribute("type", "float");

    let nd_arc = Arc::new(nd);
    let mut handles = vec![];

    for _ in 0..8 {
        let nd_clone = Arc::clone(&nd_arc);
        let handle = thread::spawn(move || {
            // Read-only access from multiple threads simultaneously
            let b = nd_clone.borrow();
            assert_eq!(b.get_category(), category::NODEDEF);
            assert_eq!(b.get_type(), Some("float"));
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("thread panicked");
    }
}

#[test]
fn test_arc_clone_across_threads() {
    // Arc clone and send to another thread.
    let root = make_doc();
    let child = add(&root, category::NODE, "worker_node");
    child.borrow_mut().set_attribute("type", "integer");

    // Clone the Arc (cheap pointer clone) and send to thread
    let child_clone = child.clone();
    let result = thread::spawn(move || child_clone.borrow().get_type().map(|s| s.to_string()))
        .join()
        .expect("thread panicked");

    assert_eq!(result, Some("integer".to_string()));
}

#[test]
fn test_document_create_and_read_threaded() {
    // Create document in main thread, read from worker thread.
    let xml = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <nodedef name="ND_multithreaded" node="test" type="float"/>
</materialx>"#;

    let doc = read_from_xml_str(xml).expect("parse failed");
    let root = doc.get_root().clone();

    // Send root to another thread for reading
    let handle = thread::spawn(move || root.borrow().get_child("ND_multithreaded").is_some());

    assert!(handle.join().expect("thread panicked"));
}

#[test]
fn test_element_attribute_iter() {
    let elem = make_doc();
    elem.borrow_mut().set_attribute("a", "1");
    elem.borrow_mut().set_attribute("b", "2");
    elem.borrow_mut().set_attribute("c", "3");

    let attrs: Vec<(String, String)> = elem
        .borrow()
        .iter_attributes()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    assert_eq!(attrs.len(), 3);
    assert!(attrs.iter().any(|(k, v)| k == "a" && v == "1"));
    assert!(attrs.iter().any(|(k, v)| k == "b" && v == "2"));
    assert!(attrs.iter().any(|(k, v)| k == "c" && v == "3"));
}

#[test]
fn test_document_add_nodedef() {
    let mut doc = create_document();
    let nd = doc
        .add_child_of_category(category::NODEDEF, "ND_mynode")
        .expect("add nodedef");
    nd.borrow_mut().set_attribute("type", "color3");
    nd.borrow_mut().set_attribute("node", "mynode");

    assert_eq!(
        doc.get_root().borrow().get_children().len(),
        1,
        "document should have one child"
    );
    assert_eq!(nd.borrow().get_name(), "ND_mynode");
}
