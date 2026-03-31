//! GeomPath, GeomInfo, GeomPropDef, Collection -- geometry binding elements.

use crate::core::element::{
    COLLECTION_ATTRIBUTE, EXCLUDEGEOM_ATTRIBUTE, ElementPtr, GEOM_ATTRIBUTE,
    INCLUDE_COLLECTION_ATTRIBUTE, INCLUDEGEOM_ATTRIBUTE, add_child_of_category, category,
};
use crate::core::util::{create_name_path, split_string};

/// GeomPropDef attribute names (C++ Geom.h)
pub const GEOM_PROP_ATTRIBUTE: &str = "geomprop";
pub const SPACE_ATTRIBUTE: &str = "space";
pub const INDEX_ATTRIBUTE: &str = "index";

pub const GEOM_PATH_SEPARATOR: &str = "/";
pub const UNIVERSAL_GEOM_NAME: &str = "*";
#[allow(dead_code)]
pub const UDIM_SET_PROPERTY: &str = "udimset";

// --- GeomPropDef getters/setters ---

/// Get geomprop attribute from GeomPropDef
pub fn get_geom_prop(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(GEOM_PROP_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set geomprop attribute on GeomPropDef
pub fn set_geom_prop(elem: &ElementPtr, geomprop: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(GEOM_PROP_ATTRIBUTE, geomprop.into());
}

/// Get space attribute from GeomPropDef
pub fn get_space(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(SPACE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set space attribute on GeomPropDef
pub fn set_space(elem: &ElementPtr, space: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(SPACE_ATTRIBUTE, space.into());
}

/// Get index attribute from GeomPropDef
pub fn get_index(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(INDEX_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Set index attribute on GeomPropDef
pub fn set_index(elem: &ElementPtr, index: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(INDEX_ATTRIBUTE, index.into());
}

// --- GeomElement helpers (GeomInfo, MaterialAssign, etc.) ---

/// Set geom string (C++ GeomElement::setGeom)
pub fn set_geom(elem: &ElementPtr, geom: impl Into<String>) {
    elem.borrow_mut().set_attribute(GEOM_ATTRIBUTE, geom.into());
}

/// Has geom string
pub fn has_geom(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(GEOM_ATTRIBUTE)
}

/// Get geom string
pub fn get_geom(elem: &ElementPtr) -> String {
    elem.borrow().get_attribute_or_empty(GEOM_ATTRIBUTE)
}

/// Set collection string (C++ GeomElement::setCollectionString)
pub fn set_collection_string(elem: &ElementPtr, collection: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(COLLECTION_ATTRIBUTE, collection.into());
}

/// Has collection string
pub fn has_collection_string(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(COLLECTION_ATTRIBUTE)
}

/// Get collection string
pub fn get_collection_string(elem: &ElementPtr) -> String {
    elem.borrow().get_attribute_or_empty(COLLECTION_ATTRIBUTE)
}

// --- GeomInfo CRUD for GeomProp children ---

/// Add a GeomProp to a GeomInfo
pub fn add_geom_prop_child(
    geom_info: &ElementPtr,
    name: &str,
    type_str: &str,
) -> Result<ElementPtr, String> {
    let gp = add_child_of_category(geom_info, category::GEOM_PROP, name)?;
    if !type_str.is_empty() {
        gp.borrow_mut().set_attribute("type", type_str);
    }
    Ok(gp)
}

/// Get GeomProp child by name from a GeomInfo
pub fn get_geom_prop_child(geom_info: &ElementPtr, name: &str) -> Option<ElementPtr> {
    geom_info
        .borrow()
        .get_child_of_category(name, category::GEOM_PROP)
}

/// Get all GeomProp children of a GeomInfo
pub fn get_geom_props(geom_info: &ElementPtr) -> Vec<ElementPtr> {
    geom_info
        .borrow()
        .get_children_of_category(category::GEOM_PROP)
}

/// Remove GeomProp child by name from a GeomInfo
pub fn remove_geom_prop(geom_info: &ElementPtr, name: &str) {
    geom_info
        .borrow_mut()
        .remove_child_of_category(name, category::GEOM_PROP);
}

// --- Collection helpers ---

/// Set includegeom on Collection
pub fn set_include_geom(elem: &ElementPtr, geom: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(INCLUDEGEOM_ATTRIBUTE, geom.into());
}
/// Get includegeom from Collection
pub fn get_include_geom(elem: &ElementPtr) -> String {
    elem.borrow().get_attribute_or_empty(INCLUDEGEOM_ATTRIBUTE)
}
/// Has includegeom
pub fn has_include_geom(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(INCLUDEGEOM_ATTRIBUTE)
}

/// Set excludegeom on Collection
pub fn set_exclude_geom(elem: &ElementPtr, geom: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(EXCLUDEGEOM_ATTRIBUTE, geom.into());
}
/// Get excludegeom from Collection
pub fn get_exclude_geom(elem: &ElementPtr) -> String {
    elem.borrow().get_attribute_or_empty(EXCLUDEGEOM_ATTRIBUTE)
}
/// Has excludegeom
pub fn has_exclude_geom(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(EXCLUDEGEOM_ATTRIBUTE)
}

/// Set includecollection on Collection
pub fn set_include_collection(elem: &ElementPtr, collection: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(INCLUDE_COLLECTION_ATTRIBUTE, collection.into());
}
/// Get includecollection from Collection
pub fn get_include_collection(elem: &ElementPtr) -> String {
    elem.borrow()
        .get_attribute_or_empty(INCLUDE_COLLECTION_ATTRIBUTE)
}
/// Has includecollection
pub fn has_include_collection(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(INCLUDE_COLLECTION_ATTRIBUTE)
}

// --- GeomPath ---

/// Geometry path -- hierarchical geometry name
#[derive(Clone, Debug, Default)]
pub struct GeomPath {
    segments: Vec<String>,
    empty: bool,
}

impl GeomPath {
    pub fn new() -> Self {
        Self {
            segments: vec![],
            empty: true,
        }
    }

    pub fn from_string(geom: &str) -> Self {
        let empty = geom.is_empty();
        let segments = if geom.is_empty() {
            vec![]
        } else {
            split_string(geom, GEOM_PATH_SEPARATOR)
        };
        Self { segments, empty }
    }

    pub fn is_empty(&self) -> bool {
        self.empty && self.segments.is_empty()
    }

    pub fn is_universal(&self) -> bool {
        self.segments.len() == 1 && self.segments[0] == UNIVERSAL_GEOM_NAME
    }

    pub fn is_matching(&self, other: &GeomPath) -> bool {
        if self.is_universal() || other.is_universal() {
            return true;
        }
        if self.segments.len() > other.segments.len() {
            return false;
        }
        self.segments
            .iter()
            .zip(other.segments.iter())
            .all(|(a, b)| a == b)
    }
}

impl PartialEq for GeomPath {
    fn eq(&self, other: &Self) -> bool {
        self.segments == other.segments
    }
}

impl From<&str> for GeomPath {
    fn from(s: &str) -> Self {
        Self::from_string(s)
    }
}

impl ToString for GeomPath {
    fn to_string(&self) -> String {
        if self.segments.is_empty() {
            return if self.empty {
                String::new()
            } else {
                UNIVERSAL_GEOM_NAME.to_string()
            };
        }
        GEOM_PATH_SEPARATOR.to_string() + &create_name_path(&self.segments)
    }
}

/// Match two geometry strings with wildcard support (C++ Collection::matchesGeomString).
///
/// Returns true when:
/// - Either string is the universal wildcard "*".
/// - `geom2` is a prefix-path of `geom1` or vice-versa (shared ancestry).
/// - Either string ends with "/*" making it match all descendants.
///
/// An empty geom string matches nothing (returns false).
pub fn geom_strings_match(geom1: &str, geom2: &str) -> bool {
    if geom1.is_empty() || geom2.is_empty() {
        return false;
    }
    // Universal wildcard matches anything
    if geom1 == UNIVERSAL_GEOM_NAME || geom2 == UNIVERSAL_GEOM_NAME {
        return true;
    }
    // Expand comma-separated lists: any pair that matches counts
    let parts1: Vec<&str> = geom1.split(',').map(str::trim).collect();
    let parts2: Vec<&str> = geom2.split(',').map(str::trim).collect();
    for p1 in &parts1 {
        for p2 in &parts2 {
            if geom_single_match(p1, p2) {
                return true;
            }
        }
    }
    false
}

/// Match a single (no comma) geom path pair with optional trailing-wildcard support.
fn geom_single_match(g1: &str, g2: &str) -> bool {
    if g1 == g2 {
        return true;
    }
    // Strip trailing "/*" wildcard to get base path
    let (base1, wild1) = strip_geom_wildcard(g1);
    let (base2, wild2) = strip_geom_wildcard(g2);
    // If g1 is a wildcard prefix, check g2 starts with base1
    if wild1 && g2.starts_with(base1) {
        return true;
    }
    // If g2 is a wildcard prefix, check g1 starts with base2
    if wild2 && g1.starts_with(base2) {
        return true;
    }
    // Path-prefix check (shared ancestry): one path is a prefix of the other at a separator boundary
    path_is_prefix(base1, base2) || path_is_prefix(base2, base1)
}

/// Strip trailing "/*" wildcard from a geom path. Returns (base, is_wildcard).
fn strip_geom_wildcard(g: &str) -> (&str, bool) {
    if let Some(base) = g.strip_suffix("/*") {
        (base, true)
    } else if g == "*" {
        ("", true)
    } else {
        (g, false)
    }
}

/// Return true if `prefix` is an exact path prefix of `path` at a "/" boundary.
fn path_is_prefix(prefix: &str, path: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    if path == prefix {
        return true;
    }
    // Check "path" starts with "prefix/"
    if path.len() > prefix.len() + 1 {
        path.starts_with(prefix) && path.as_bytes()[prefix.len()] == b'/'
    } else {
        false
    }
}

// --- GeomElement resolved Collection helpers ---

/// Set collection on a GeomElement by reference (GeomElement::setCollection).
/// Pass None to clear the collection attribute.
pub fn set_collection(elem: &ElementPtr, collection: Option<&ElementPtr>) {
    match collection {
        Some(c) => {
            let name = c.borrow().get_name().to_string();
            set_collection_string(elem, name);
        }
        None => elem.borrow_mut().remove_attribute(COLLECTION_ATTRIBUTE),
    }
}

/// Resolve collection string to a Collection element (GeomElement::getCollection).
/// Looks up the collection by name in the document root.
pub fn get_collection(elem: &ElementPtr) -> Option<ElementPtr> {
    let col_name = {
        let b = elem.borrow();
        b.get_attribute(COLLECTION_ATTRIBUTE)
            .map(|s| s.to_string())?
    };
    crate::core::element::resolve_name_reference(elem, &col_name)
}

/// Validate a GeomElement (GeomInfo, MaterialAssign, etc.).
/// Checks that collection string resolves if present.
/// Mirrors C++ GeomElement::validate.
pub fn validate_geom_element(elem: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    if has_collection_string(elem) {
        if get_collection(elem).is_none() {
            valid = false;
            errors.push(format!(
                "Invalid collection string: {}",
                elem.borrow().as_string()
            ));
        }
    }

    let (base_valid, base_errors) = crate::core::element::validate_element_self(elem);
    if !base_valid {
        valid = false;
    }
    errors.extend(base_errors);

    (valid, errors)
}

// --- Collection advanced methods ---

/// Resolve includecollection to a single Collection element.
/// For the setter that accepts an element, use set_include_collection_ref.
pub fn set_include_collection_ref(elem: &ElementPtr, collection: Option<&ElementPtr>) {
    match collection {
        Some(c) => {
            let name = c.borrow().get_name().to_string();
            set_include_collection(elem, name);
        }
        None => elem
            .borrow_mut()
            .remove_attribute(INCLUDE_COLLECTION_ATTRIBUTE),
    }
}

/// Set includecollection from a list of Collection elements.
/// Mirrors C++ Collection::setIncludeCollections.
pub fn set_include_collections(elem: &ElementPtr, collections: &[ElementPtr]) {
    if collections.is_empty() {
        elem.borrow_mut()
            .remove_attribute(INCLUDE_COLLECTION_ATTRIBUTE);
    } else {
        let names: Vec<String> = collections
            .iter()
            .map(|c| c.borrow().get_name().to_string())
            .collect();
        elem.borrow_mut()
            .set_attribute(INCLUDE_COLLECTION_ATTRIBUTE, names.join(","));
    }
}

/// Get resolved include collections. Splits the includecollection string
/// by comma and resolves each name to a Collection element.
/// Mirrors C++ Collection::getIncludeCollections.
pub fn get_include_collections(elem: &ElementPtr) -> Vec<ElementPtr> {
    let inc_str = get_include_collection(elem);
    if inc_str.is_empty() {
        return vec![];
    }
    inc_str
        .split(',')
        .filter_map(|name| {
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            crate::core::element::resolve_name_reference(elem, name)
        })
        .collect()
}

/// Get active includegeom: walks inheritance chain.
/// Mirrors C++ Collection::getActiveIncludeGeom.
pub fn get_active_include_geom(elem: &ElementPtr) -> String {
    use crate::core::traversal::traverse_inheritance;
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        if has_include_geom(&ancestor) {
            return get_include_geom(&ancestor);
        }
    }
    String::new()
}

/// Get active excludegeom: walks inheritance chain.
/// Mirrors C++ Collection::getActiveExcludeGeom.
pub fn get_active_exclude_geom(elem: &ElementPtr) -> String {
    use crate::core::traversal::traverse_inheritance;
    for ancestor in traverse_inheritance(elem.clone()).filter_map(|r| r.ok()) {
        if has_exclude_geom(&ancestor) {
            return get_exclude_geom(&ancestor);
        }
    }
    String::new()
}

/// Check if a Collection matches a geometry string.
/// Walks the includecollection chain with cycle detection.
/// Mirrors C++ Collection::matchesGeomString.
pub fn matches_geom_string(elem: &ElementPtr, geom: &str) -> Result<bool, String> {
    // Check excludegeom first (with contains=true semantics)
    let exclude = get_active_exclude_geom(elem);
    if !exclude.is_empty() && geom_strings_match(&exclude, geom) {
        return Ok(false);
    }

    // Check includegeom
    let include = get_active_include_geom(elem);
    if !include.is_empty() && geom_strings_match(&include, geom) {
        return Ok(true);
    }

    // Walk includecollection chain with cycle detection
    let mut included_set = std::collections::HashSet::new();
    let mut included_vec = get_include_collections(elem);
    let mut i = 0;
    while i < included_vec.len() {
        let collection = included_vec[i].clone();
        let col_name = collection.borrow().get_name().to_string();
        if !included_set.insert(col_name.clone()) {
            return Err(format!(
                "Encountered a cycle in collection: {}",
                elem.borrow().get_name()
            ));
        }
        let append = get_include_collections(&collection);
        included_vec.extend(append);
        i += 1;
    }

    // Check each unique included collection
    for col in &included_set {
        // Re-resolve to get the element
        if let Some(c) = crate::core::element::resolve_name_reference(elem, col) {
            let inc = get_active_include_geom(&c);
            let exc = get_active_exclude_geom(&c);
            if !exc.is_empty() && geom_strings_match(&exc, geom) {
                continue;
            }
            if !inc.is_empty() && geom_strings_match(&inc, geom) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Check if the collection has a cycle in its includecollection chain.
/// Mirrors C++ Collection::hasIncludeCycle.
pub fn has_include_cycle(elem: &ElementPtr) -> bool {
    matches!(matches_geom_string(elem, UNIVERSAL_GEOM_NAME), Err(_))
}

/// Validate a Collection element.
/// Checks for cycles in the includecollection chain.
/// Mirrors C++ Collection::validate.
pub fn validate_collection(elem: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    if has_include_cycle(elem) {
        valid = false;
        errors.push(format!(
            "Cycle in collection include chain: {}",
            elem.borrow().as_string()
        ));
    }

    let (base_valid, base_errors) = crate::core::element::validate_element_self(elem);
    if !base_valid {
        valid = false;
    }
    errors.extend(base_errors);

    (valid, errors)
}

// ---- GeomInfo value helpers ----

/// Set a GeomProp value on a GeomInfo element, creating the child if needed.
/// Mirrors C++ GeomInfo::setGeomPropValue.
pub fn set_geom_info_prop_value(
    geom_info: &ElementPtr,
    name: &str,
    value: &str,
    type_str: &str,
) -> Result<ElementPtr, String> {
    let prop = if let Some(existing) = geom_info
        .borrow()
        .get_child_of_category(name, category::GEOM_PROP)
    {
        existing
    } else {
        add_geom_prop_child(geom_info, name, type_str)?
    };
    if !type_str.is_empty() {
        prop.borrow_mut().set_attribute("type", type_str);
    }
    prop.borrow_mut().set_value_string(value);
    Ok(prop)
}

/// Set a Token value on a GeomInfo element, creating the child if needed.
/// Mirrors C++ GeomInfo::setTokenValue.
pub fn set_geom_info_token_value(
    geom_info: &ElementPtr,
    name: &str,
    value: &str,
) -> Result<ElementPtr, String> {
    let tok = if let Some(existing) = geom_info
        .borrow()
        .get_child_of_category(name, category::TOKEN)
    {
        existing
    } else {
        add_child_of_category(geom_info, category::TOKEN, name)?
    };
    tok.borrow_mut().set_value_string(value);
    Ok(tok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_geom_no_match() {
        assert!(!geom_strings_match("", "/geo/mesh"));
        assert!(!geom_strings_match("/geo/mesh", ""));
        assert!(!geom_strings_match("", ""));
    }

    #[test]
    fn universal_wildcard_matches_anything() {
        assert!(geom_strings_match("*", "/geo/mesh"));
        assert!(geom_strings_match("/geo/mesh", "*"));
        assert!(geom_strings_match("*", "*"));
    }

    #[test]
    fn exact_match() {
        assert!(geom_strings_match("/geo/mesh", "/geo/mesh"));
        assert!(!geom_strings_match("/geo/mesh", "/geo/other"));
    }

    #[test]
    fn trailing_wildcard_descendant() {
        assert!(geom_strings_match("/geo/*", "/geo/mesh"));
        assert!(geom_strings_match("/geo/mesh", "/geo/*"));
        assert!(!geom_strings_match("/other/*", "/geo/mesh"));
    }

    #[test]
    fn path_prefix_shared_ancestry() {
        // /geo is ancestor of /geo/mesh
        assert!(geom_strings_match("/geo", "/geo/mesh"));
        assert!(geom_strings_match("/geo/mesh", "/geo"));
        assert!(!geom_strings_match("/other", "/geo/mesh"));
    }

    #[test]
    fn comma_separated_any_pair_matches() {
        assert!(geom_strings_match("/geo/mesh,/other", "/geo/mesh"));
        assert!(geom_strings_match("/a,/b", "/b,/c"));
        assert!(!geom_strings_match("/a,/b", "/c,/d"));
    }
}
