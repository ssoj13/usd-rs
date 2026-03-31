//! Backdrop — layout element for grouping nodes in a graph.

use crate::core::element::{ElementPtr, category};

// Attribute names (matches C++ Backdrop::CONTAINS_ATTRIBUTE, etc.)
pub const CONTAINS_ATTRIBUTE: &str = "contains";
pub const WIDTH_ATTRIBUTE: &str = "width";
pub const HEIGHT_ATTRIBUTE: &str = "height";

/// Return true if the element is a Backdrop.
pub fn is_backdrop(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == category::BACKDROP
}

/// Set the contains string (comma-separated node names).
pub fn set_contains_string(elem: &ElementPtr, contains: &str) {
    elem.borrow_mut()
        .set_attribute(CONTAINS_ATTRIBUTE, contains);
}

/// Check if backdrop has a contains string.
pub fn has_contains_string(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(CONTAINS_ATTRIBUTE)
}

/// Get the contains string.
pub fn get_contains_string(elem: &ElementPtr) -> String {
    elem.borrow().get_attribute_or_empty(CONTAINS_ATTRIBUTE)
}

/// Set width attribute.
pub fn set_width(elem: &ElementPtr, width: f32) {
    elem.borrow_mut()
        .set_attribute(WIDTH_ATTRIBUTE, width.to_string());
}

/// Check if backdrop has a width attribute.
pub fn has_width(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(WIDTH_ATTRIBUTE)
}

/// Get width attribute (0.0 if missing or unparseable).
pub fn get_width(elem: &ElementPtr) -> f32 {
    elem.borrow()
        .get_attribute(WIDTH_ATTRIBUTE)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0)
}

/// Set height attribute.
pub fn set_height(elem: &ElementPtr, height: f32) {
    elem.borrow_mut()
        .set_attribute(HEIGHT_ATTRIBUTE, height.to_string());
}

/// Check if backdrop has a height attribute.
pub fn has_height(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(HEIGHT_ATTRIBUTE)
}

/// Get height attribute (0.0 if missing or unparseable).
pub fn get_height(elem: &ElementPtr) -> f32 {
    elem.borrow()
        .get_attribute(HEIGHT_ATTRIBUTE)
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.0)
}

/// Set the contained elements by writing a comma-separated list of their names.
pub fn set_contains_elements(elem: &ElementPtr, nodes: &[ElementPtr]) {
    let names: Vec<String> = nodes
        .iter()
        .map(|n| n.borrow().get_name().to_string())
        .collect();
    set_contains_string(elem, &names.join(","));
}

/// Get the contained elements by looking up names in the parent scope.
pub fn get_contains_elements(elem: &ElementPtr) -> Vec<ElementPtr> {
    let contains = get_contains_string(elem);
    if contains.is_empty() {
        return Vec::new();
    }
    let parent = match elem.borrow().get_parent() {
        Some(p) => p,
        None => return Vec::new(),
    };
    contains
        .split(',')
        .filter_map(|name| {
            let name = name.trim();
            if name.is_empty() {
                None
            } else {
                parent.borrow().get_child(name)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::add_child_of_category;

    #[test]
    fn test_backdrop_attrs() {
        let doc = create_document();
        let root = doc.get_root();
        // Add a backdrop via the graph element (Document is a GraphElement)
        let bd = add_child_of_category(&root, category::BACKDROP, "bd1").unwrap();

        assert!(is_backdrop(&bd));
        assert!(!has_contains_string(&bd));
        assert!(!has_width(&bd));
        assert!(!has_height(&bd));

        set_contains_string(&bd, "node1,node2");
        assert!(has_contains_string(&bd));
        assert_eq!(get_contains_string(&bd), "node1,node2");

        set_width(&bd, 400.0);
        set_height(&bd, 300.0);
        assert!(has_width(&bd));
        assert!(has_height(&bd));
        assert!((get_width(&bd) - 400.0).abs() < f32::EPSILON);
        assert!((get_height(&bd) - 300.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_contains_elements() {
        let doc = create_document();
        let root = doc.get_root();
        let n1 = add_child_of_category(&root, category::NODE, "node1").unwrap();
        let n2 = add_child_of_category(&root, category::NODE, "node2").unwrap();
        let bd = add_child_of_category(&root, category::BACKDROP, "bd1").unwrap();

        set_contains_elements(&bd, &[n1.clone(), n2.clone()]);
        let contained = get_contains_elements(&bd);
        assert_eq!(contained.len(), 2);
        assert_eq!(contained[0].borrow().get_name(), "node1");
        assert_eq!(contained[1].borrow().get_name(), "node2");
    }
}
