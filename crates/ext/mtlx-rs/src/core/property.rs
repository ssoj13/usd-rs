//! Property, PropertyAssign, PropertySet — property elements.

use crate::core::element::{ElementPtr, PROPERTY_ATTRIBUTE, add_child_of_category, category};

// Attribute keys matching C++ Property.h / Geom.h constants
const GEOM_ATTRIBUTE: &str = "geom";
const COLLECTION_ATTRIBUTE: &str = "collection";
const TARGET_ATTRIBUTE: &str = "target";
const VALUE_ATTRIBUTE: &str = "value";
const VALUE_STRING_ATTRIBUTE: &str = "valuestring";
/// PropertySetAssign: which PropertySet this assigns to (C++ PROPERTY_SET_ATTRIBUTE)
const PROPERTY_SET_ATTRIBUTE: &str = "propertyset";

// ---------------------------------------------------------------------------
// PropertyAssign helpers
// ---------------------------------------------------------------------------

/// Get property attribute (PropertyAssign: which property this assigns to)
pub fn get_property_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(PROPERTY_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set property attribute
pub fn set_property_string(elem: &ElementPtr, property: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(PROPERTY_ATTRIBUTE, property.into());
}

/// Has property attribute
pub fn has_property_string(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(PROPERTY_ATTRIBUTE)
}

/// Get geom string on PropertyAssign (which geometry this assignment targets)
pub fn get_property_assign_geom(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(GEOM_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set geom string on PropertyAssign
pub fn set_property_assign_geom(elem: &ElementPtr, geom: impl Into<String>) {
    elem.borrow_mut().set_attribute(GEOM_ATTRIBUTE, geom.into());
}

/// Get collection string on PropertyAssign
pub fn get_property_assign_collection(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(COLLECTION_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set collection string on PropertyAssign
pub fn set_property_assign_collection(elem: &ElementPtr, collection: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(COLLECTION_ATTRIBUTE, collection.into());
}

// ---------------------------------------------------------------------------
// Property element helpers (C++ Property extends ValueElement)
// ---------------------------------------------------------------------------

/// Get target attribute on a Property (which render target it applies to)
pub fn get_property_target(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(TARGET_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set target attribute on a Property
pub fn set_property_target(elem: &ElementPtr, target: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(TARGET_ATTRIBUTE, target.into());
}

/// Get value string from a Property (C++ ValueElement::getValueString)
pub fn get_property_value_string(elem: &ElementPtr) -> Option<String> {
    // Prefer "value" attribute, fall back to "valuestring" — each borrow is separate
    let b = elem.borrow();
    if let Some(v) = b.get_attribute(VALUE_ATTRIBUTE) {
        return Some(v.to_string());
    }
    b.get_attribute(VALUE_STRING_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set value string on a Property (C++ ValueElement::setValueString)
pub fn set_property_value_string(elem: &ElementPtr, value: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(VALUE_ATTRIBUTE, value.into());
}

// ---------------------------------------------------------------------------
// PropertySet CRUD helpers
// ---------------------------------------------------------------------------

/// Get Property children of a PropertySet (C++ PropertySet::getProperties)
pub fn get_properties(property_set: &ElementPtr) -> Vec<ElementPtr> {
    property_set
        .borrow()
        .get_children()
        .iter()
        .filter(|c| c.borrow().get_category() == category::PROPERTY)
        .cloned()
        .collect()
}

/// Add a Property child to a PropertySet (C++ PropertySet::addProperty).
/// Returns the new child, or the existing child if a Property with that name already exists.
pub fn add_property(property_set: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    // Re-use existing child if present (idempotent, matching C++ behaviour)
    if let Some(existing) = property_set
        .borrow()
        .get_child_of_category(name, category::PROPERTY)
    {
        return Ok(existing);
    }
    add_child_of_category(property_set, category::PROPERTY, name)
}

/// Remove a Property child from a PropertySet by name (C++ PropertySet::removeProperty).
pub fn remove_property(property_set: &ElementPtr, name: &str) {
    property_set
        .borrow_mut()
        .remove_child_of_category(name, category::PROPERTY);
}

/// Get a single Property child by name from a PropertySet (C++ PropertySet::getProperty).
pub fn get_property(property_set: &ElementPtr, name: &str) -> Option<ElementPtr> {
    property_set
        .borrow()
        .get_child_of_category(name, category::PROPERTY)
}

// ---------------------------------------------------------------------------
// PropertySet typed value helpers (C++ PropertySet::setPropertyValue/getPropertyValue)
// ---------------------------------------------------------------------------

/// Set a typed property value on a PropertySet. Creates the Property child if
/// it doesn't exist, then sets its type and value attributes.
/// Mirrors C++ PropertySet::setPropertyValue.
pub fn set_property_set_value(
    property_set: &ElementPtr,
    name: &str,
    value: &str,
    type_str: &str,
) -> Result<ElementPtr, String> {
    let prop = add_property(property_set, name)?;
    if !type_str.is_empty() {
        prop.borrow_mut().set_attribute("type", type_str);
    }
    prop.borrow_mut().set_attribute(VALUE_ATTRIBUTE, value);
    Ok(prop)
}

/// Get a property value string from a PropertySet by property name.
/// Returns None if the property doesn't exist or has no value.
/// Mirrors C++ PropertySet::getPropertyValue.
pub fn get_property_set_value(property_set: &ElementPtr, name: &str) -> Option<String> {
    let prop = get_property(property_set, name)?;
    prop.borrow()
        .get_attribute(VALUE_ATTRIBUTE)
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// PropertySetAssign helpers (C++ PropertySetAssign)
// ---------------------------------------------------------------------------

/// PropertySetAssign: set "propertyset" attribute (which PropertySet this assigns).
pub fn set_property_set_ref_string(elem: &ElementPtr, name: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(PROPERTY_SET_ATTRIBUTE, name.into());
}

/// PropertySetAssign: get "propertyset" attribute.
pub fn get_property_set_ref_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(PROPERTY_SET_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// PropertySetAssign: resolve propertyset to an element (looks up by name in root).
pub fn get_property_set_ref(elem: &ElementPtr) -> Option<ElementPtr> {
    let name = get_property_set_ref_string(elem)?;
    crate::core::element::resolve_name_reference(elem, &name)
}

/// PropertySetAssign: set propertyset from element reference (or clear with None).
pub fn set_property_set_ref(elem: &ElementPtr, property_set: Option<&ElementPtr>) {
    match property_set {
        Some(ps) => {
            let name = ps.borrow().get_name().to_string();
            set_property_set_ref_string(elem, name);
        }
        None => elem.borrow_mut().remove_attribute(PROPERTY_SET_ATTRIBUTE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::add_child_of_category;

    fn make_property_set() -> ElementPtr {
        let doc = create_document();
        add_child_of_category(&doc.get_root(), category::PROPERTY_SET, "ps1").unwrap()
    }

    #[test]
    fn add_and_get_property() {
        let ps = make_property_set();
        let prop = add_property(&ps, "roughness").unwrap();
        assert_eq!(prop.borrow().get_name(), "roughness");

        let found = get_property(&ps, "roughness");
        assert!(found.is_some());
        assert_eq!(found.unwrap().borrow().get_name(), "roughness");
    }

    #[test]
    fn add_property_idempotent() {
        let ps = make_property_set();
        let p1 = add_property(&ps, "base").unwrap();
        let p2 = add_property(&ps, "base").unwrap();
        // Same underlying Arc pointer
        assert!(p1.ptr_eq(&p2));
        // Still only one child
        assert_eq!(get_properties(&ps).len(), 1);
    }

    #[test]
    fn test_remove_property() {
        let ps = make_property_set();
        add_property(&ps, "metalness").unwrap();
        assert_eq!(get_properties(&ps).len(), 1);
        remove_property(&ps, "metalness");
        assert_eq!(get_properties(&ps).len(), 0);
    }

    #[test]
    fn property_target_value_string() {
        let ps = make_property_set();
        let prop = add_property(&ps, "roughness").unwrap();

        set_property_target(&prop, "glsl");
        assert_eq!(get_property_target(&prop), Some("glsl".to_string()));

        set_property_value_string(&prop, "0.5");
        assert_eq!(get_property_value_string(&prop), Some("0.5".to_string()));
    }

    #[test]
    fn property_assign_helpers() {
        let doc = create_document();
        let pa = add_child_of_category(&doc.get_root(), category::PROPERTY_ASSIGN, "pa1").unwrap();

        set_property_string(&pa, "roughness");
        assert_eq!(get_property_string(&pa), Some("roughness".to_string()));
        assert!(has_property_string(&pa));

        set_property_assign_geom(&pa, "/geo/mesh");
        assert_eq!(get_property_assign_geom(&pa), Some("/geo/mesh".to_string()));

        set_property_assign_collection(&pa, "myCollection");
        assert_eq!(
            get_property_assign_collection(&pa),
            Some("myCollection".to_string())
        );
    }
}
