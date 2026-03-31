//! Definition helpers — NodeDef, Implementation, TypeDef.
//!
//! Rust-idiomatic free-function wrappers over ElementPtr following the same
//! pattern as interface.rs and node.rs.  No inheritance needed.

use crate::core::element::{
    ElementPtr, NODE_ATTRIBUTE, NODE_DEF_ATTRIBUTE, NODE_GRAPH_ATTRIBUTE, add_child_of_category,
    category,
};
use crate::core::node::{get_inputs, get_outputs};

// ── Attribute name constants (matches C++ Definition.cpp) ─────────────────

/// Implementation: path to the source code file
pub const FILE_ATTRIBUTE: &str = "file";
/// Implementation: function name within the file
pub const FUNCTION_ATTRIBUTE: &str = "function";
/// TypeDef: semantic string (e.g. "color", "shader")
pub const SEMANTIC_ATTRIBUTE: &str = "semantic";
/// TypeDef: context string
pub const CONTEXT_ATTRIBUTE: &str = "context";
/// NodeDef: node group (texture/procedural/geometric/…)
pub const NODE_GROUP_ATTRIBUTE: &str = "nodegroup";

// Node group string constants (NodeDef::*_NODE_GROUP in C++)
pub const TEXTURE_NODE_GROUP: &str = "texture";
pub const PROCEDURAL_NODE_GROUP: &str = "procedural";
pub const GEOMETRIC_NODE_GROUP: &str = "geometric";
pub const ADJUSTMENT_NODE_GROUP: &str = "adjustment";
pub const CONDITIONAL_NODE_GROUP: &str = "conditional";
pub const CHANNEL_NODE_GROUP: &str = "channel";
pub const ORGANIZATION_NODE_GROUP: &str = "organization";
pub const TRANSLATION_NODE_GROUP: &str = "translation";

// TypeDef semantic constants (C++ COLOR_SEMANTIC / SHADER_SEMANTIC)
pub const COLOR_SEMANTIC: &str = "color";
pub const SHADER_SEMANTIC: &str = "shader";

// Member element category
pub const MEMBER_CATEGORY: &str = "member";
// TargetDef element category
pub const TARGET_DEF_CATEGORY: &str = "targetdef";

// ── NodeDef helpers ────────────────────────────────────────────────────────

/// Return true if element is a nodedef.
pub fn is_node_def(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == category::NODEDEF
}

/// NodeDef: get the "node" attribute (the functional category string, e.g. "standard_surface").
/// Already available in interface.rs as get_node_string; re-exported here for symmetry.
pub fn nodedef_get_node_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(NODE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// NodeDef: set "node" attribute.
pub fn nodedef_set_node_string(elem: &ElementPtr, node: impl Into<String>) {
    elem.borrow_mut().set_attribute(NODE_ATTRIBUTE, node.into());
}

/// NodeDef: get "nodegroup" attribute (texture / procedural / …).
pub fn get_node_group(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(NODE_GROUP_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// NodeDef: set "nodegroup" attribute.
pub fn set_node_group(elem: &ElementPtr, group: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(NODE_GROUP_ATTRIBUTE, group.into());
}

/// NodeDef: has "nodegroup" attribute.
pub fn has_node_group(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(NODE_GROUP_ATTRIBUTE)
}

/// NodeDef: derive output type from active outputs (mirrors C++ NodeDef::getType).
/// - 0 outputs  → DEFAULT_TYPE_STRING ("float")
/// - 1 output   → that output's type
/// - 2+ outputs → MULTI_OUTPUT_TYPE_STRING ("multioutput")
pub fn nodedef_get_type(elem: &ElementPtr) -> String {
    let outputs = get_outputs(elem);
    match outputs.len() {
        0 => crate::core::types::DEFAULT_TYPE_STRING.to_string(),
        1 => outputs[0]
            .borrow()
            .get_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::core::types::DEFAULT_TYPE_STRING.to_string()),
        _ => crate::core::types::MULTI_OUTPUT_TYPE_STRING.to_string(),
    }
}

/// NodeDef: get all active inputs (direct children with category "input").
pub fn nodedef_get_inputs(elem: &ElementPtr) -> Vec<ElementPtr> {
    get_inputs(elem)
}

/// NodeDef: get all active outputs.
pub fn nodedef_get_outputs(elem: &ElementPtr) -> Vec<ElementPtr> {
    get_outputs(elem)
}

/// NodeDef: find the first Implementation (or NodeGraph) for this nodedef,
/// optionally filtered by target string.
///
/// Mirrors C++ NodeDef::getImplementation.
/// `doc` must be the Document that owns the nodedef.
/// When `target` is empty the first match is returned.
/// When `resolve_node_graph` is true an Implementation that references a
/// NodeGraph is replaced by that NodeGraph element.
pub fn get_implementation_for_nodedef(
    elem: &ElementPtr,
    doc: &crate::core::Document,
    target: &str,
    resolve_node_graph: bool,
) -> Option<ElementPtr> {
    let nd_name = elem.borrow().get_name().to_string();
    let qualified = elem.borrow().get_qualified_name(&nd_name);

    // Collect all matching implementations/nodegraphs
    let mut candidates: Vec<ElementPtr> = doc.get_matching_implementations(&nd_name);
    if nd_name != qualified {
        candidates.extend(doc.get_matching_implementations(&qualified));
    }

    // Optionally resolve Implementation → NodeGraph
    let candidates: Vec<ElementPtr> = if resolve_node_graph {
        candidates
            .into_iter()
            .map(|c| {
                if c.borrow().get_category() == category::IMPLEMENTATION {
                    // Check if it has a nodegraph attribute pointing to a NodeGraph
                    let ng_name = c
                        .borrow()
                        .get_attribute(NODE_GRAPH_ATTRIBUTE)
                        .map(|s| s.to_string());
                    if let Some(ng_name) = ng_name {
                        if let Some(ng) = doc.get_node_graph(&ng_name) {
                            return ng;
                        }
                    }
                }
                c
            })
            .collect()
    } else {
        candidates
    };

    if target.is_empty() {
        return candidates.into_iter().next();
    }

    // Prefer target-specific match, then fall back to generic (no target attr)
    let target_str = target.to_string();
    for c in &candidates {
        let t = c
            .borrow()
            .get_attribute(crate::core::element::TARGET_ATTRIBUTE)
            .map(|s| s.to_string())
            .unwrap_or_default();
        if !t.is_empty() && t == target_str {
            return Some(c.clone());
        }
    }
    // Generic fallback
    for c in &candidates {
        let t = c
            .borrow()
            .get_attribute(crate::core::element::TARGET_ATTRIBUTE)
            .map(|s| s.to_string())
            .unwrap_or_default();
        if t.is_empty() {
            return Some(c.clone());
        }
    }
    None
}

// ── Implementation helpers ─────────────────────────────────────────────────

/// Return true if element is an implementation.
pub fn is_implementation(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == category::IMPLEMENTATION
}

/// Implementation: get "file" attribute (source code file path).
pub fn impl_get_file(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(FILE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Implementation: set "file" attribute.
pub fn impl_set_file(elem: &ElementPtr, file: impl Into<String>) {
    elem.borrow_mut().set_attribute(FILE_ATTRIBUTE, file.into());
}

/// Implementation: has "file" attribute.
pub fn impl_has_file(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(FILE_ATTRIBUTE)
}

/// Implementation: get "function" attribute (function name in the source file).
pub fn impl_get_function(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(FUNCTION_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Implementation: set "function" attribute.
pub fn impl_set_function(elem: &ElementPtr, func: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(FUNCTION_ATTRIBUTE, func.into());
}

/// Implementation: has "function" attribute.
pub fn impl_has_function(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(FUNCTION_ATTRIBUTE)
}

/// Implementation: get "nodegraph" attribute (NodeGraph that provides this impl).
pub fn impl_get_node_graph(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(NODE_GRAPH_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Implementation: set "nodegraph" attribute.
pub fn impl_set_node_graph(elem: &ElementPtr, ng: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(NODE_GRAPH_ATTRIBUTE, ng.into());
}

/// Implementation: has "nodegraph" attribute.
pub fn impl_has_node_graph(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(NODE_GRAPH_ATTRIBUTE)
}

/// Implementation: get "nodedef" attribute (which NodeDef this implements).
pub fn impl_get_nodedef_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(NODE_DEF_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// Implementation: set "nodedef" attribute.
pub fn impl_set_nodedef_string(elem: &ElementPtr, nd: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(NODE_DEF_ATTRIBUTE, nd.into());
}

/// Implementation: resolve the NodeDef element this implementation refers to.
pub fn impl_get_nodedef(elem: &ElementPtr, doc: &crate::core::Document) -> Option<ElementPtr> {
    let nd_name = impl_get_nodedef_string(elem)?;
    doc.get_node_def(&nd_name)
}

// ── TypeDef helpers ────────────────────────────────────────────────────────

/// Return true if element is a typedef.
pub fn is_type_def(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == category::TYPEDEF
}

/// TypeDef: get "semantic" attribute.
pub fn typedef_get_semantic(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(SEMANTIC_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// TypeDef: set "semantic" attribute.
pub fn typedef_set_semantic(elem: &ElementPtr, semantic: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(SEMANTIC_ATTRIBUTE, semantic.into());
}

/// TypeDef: has "semantic" attribute.
pub fn typedef_has_semantic(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(SEMANTIC_ATTRIBUTE)
}

/// TypeDef: get "context" attribute.
pub fn typedef_get_context(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(CONTEXT_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// TypeDef: set "context" attribute.
pub fn typedef_set_context(elem: &ElementPtr, context: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(CONTEXT_ATTRIBUTE, context.into());
}

/// TypeDef: has "context" attribute.
pub fn typedef_has_context(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(CONTEXT_ATTRIBUTE)
}

/// TypeDef: get all Member child elements.
/// Members describe the named components of a compound type (e.g. vector2 → x,y).
pub fn typedef_get_members(elem: &ElementPtr) -> Vec<ElementPtr> {
    elem.borrow()
        .get_children()
        .iter()
        .filter(|c| c.borrow().get_category() == MEMBER_CATEGORY)
        .cloned()
        .collect()
}

/// TypeDef: add a Member child with the given name.
pub fn typedef_add_member(elem: &ElementPtr, name: &str) -> Result<ElementPtr, String> {
    add_child_of_category(elem, MEMBER_CATEGORY, name)
}

/// TypeDef: remove a Member by name.
pub fn typedef_remove_member(elem: &ElementPtr, name: &str) {
    elem.borrow_mut().remove_child(name);
}

// ── TargetDef helpers ──────────────────────────────────────────────────────

/// Return true if element is a targetdef.
pub fn is_target_def(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == TARGET_DEF_CATEGORY
}

/// TargetDef: collect matching target names walking the inheritance chain.
/// Mirrors C++ TargetDef::getMatchingTargets.
pub fn targetdef_get_matching_targets(elem: &ElementPtr) -> Vec<String> {
    let mut result = vec![elem.borrow().get_name().to_string()];
    // Walk inheritance (inherit attribute) upward
    let mut current = elem.clone();
    loop {
        let inherited_name = current
            .borrow()
            .get_attribute(crate::core::element::INHERIT_ATTRIBUTE)
            .map(|s| s.to_string());
        let Some(inh) = inherited_name else { break };
        if inh.is_empty() {
            break;
        }
        result.push(inh.clone());
        // Try to resolve to a sibling targetdef (requires parent)
        let parent = current.borrow().get_parent();
        let Some(parent) = parent else { break };
        let next = parent.borrow().get_child(&inh).map(|c| c.clone());
        let Some(next) = next else { break };
        current = next;
    }
    result
}

// ── NodeDef validation ─────────────────────────────────────────────────────

/// Validate a NodeDef element. Checks that it does NOT have a direct "type"
/// attribute (outputs should carry the type instead). Also runs base element
/// validation. Mirrors C++ NodeDef::validate.
pub fn validate_node_def(elem: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    // NodeDef should not have a type attribute directly
    if elem.borrow().has_type() {
        valid = false;
        errors.push(format!(
            "Nodedef should not have a type but an explicit output: {}",
            elem.borrow().as_string()
        ));
    }

    // Run base element validation
    let (base_valid, base_errors) = crate::core::element::validate_element_self(elem);
    if !base_valid {
        valid = false;
    }
    errors.extend(base_errors);

    (valid, errors)
}

/// Check if a NodeDef is version-compatible with the given version string.
/// Returns true if the version matches exactly, or if this is the default
/// version and the requested version is empty.
/// Mirrors C++ NodeDef::isVersionCompatible.
pub fn is_version_compatible(elem: &ElementPtr, version: &str) -> bool {
    let b = elem.borrow();
    if b.get_version_string() == version {
        return true;
    }
    if b.get_default_version() && version.is_empty() {
        return true;
    }
    false
}

/// Collect input hints from the NodeDef's active inputs.
/// Returns a map of input_name -> hint_string for inputs that have a hint.
/// Mirrors C++ NodeDef::getInputHints.
pub fn nodedef_get_input_hints(elem: &ElementPtr) -> std::collections::HashMap<String, String> {
    use crate::core::interface::get_active_inputs;
    let mut hints = std::collections::HashMap::new();
    for input in get_active_inputs(elem) {
        let b = input.borrow();
        if let Some(hint) = b.get_attribute(crate::core::element::HINT_ATTRIBUTE) {
            hints.insert(b.get_name().to_string(), hint.to_string());
        }
    }
    hints
}

/// NodeDef: get declaration (returns self as an InterfaceElement).
/// Mirrors C++ NodeDef::getDeclaration.
pub fn nodedef_get_declaration(elem: &ElementPtr) -> Option<ElementPtr> {
    Some(elem.clone())
}

// ── Implementation validation ──────────────────────────────────────────────

/// Validate an Implementation element. Checks that it does NOT have a
/// version string (implementations don't support version). Also runs base
/// element validation. Mirrors C++ Implementation::validate.
pub fn validate_implementation(elem: &ElementPtr) -> (bool, Vec<String>) {
    let mut errors = Vec::new();
    let mut valid = true;

    // Implementation elements do not support version strings
    if elem.borrow().has_version_string() {
        valid = false;
        errors.push(format!(
            "Implementation elements do not support version strings: {}",
            elem.borrow().as_string()
        ));
    }

    // Run base element validation
    let (base_valid, base_errors) = crate::core::element::validate_element_self(elem);
    if !base_valid {
        valid = false;
    }
    errors.extend(base_errors);

    (valid, errors)
}

/// Implementation: get declaration (returns the referenced NodeDef).
/// Mirrors C++ Implementation::getDeclaration.
pub fn impl_get_declaration(elem: &ElementPtr, doc: &crate::core::Document) -> Option<ElementPtr> {
    impl_get_nodedef(elem, doc)
}

// ── UnitTypeDef helpers ────────────────────────────────────────────────────

/// UnitTypeDef category constant
pub const UNIT_TYPE_DEF_CATEGORY: &str = "unittypedef";

/// Return true if element is a unittypedef.
pub fn is_unit_type_def(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == UNIT_TYPE_DEF_CATEGORY
}

/// UnitTypeDef: get all UnitDef elements in the document that match this
/// UnitTypeDef's name. Mirrors C++ UnitTypeDef::getUnitDefs.
pub fn get_unit_defs_for_type(elem: &ElementPtr, doc: &crate::core::Document) -> Vec<ElementPtr> {
    let type_name = elem.borrow().get_name().to_string();
    doc.get_unit_defs()
        .into_iter()
        .filter(|ud| {
            ud.borrow()
                .get_attribute(crate::core::element::UNITTYPE_ATTRIBUTE)
                .map(|s| s == type_name)
                .unwrap_or(false)
        })
        .collect()
}

// ── AttributeDef helpers ───────────────────────────────────────────────────

/// AttributeDef attribute name constants (C++ AttributeDef class)
pub const ATTRNAME_ATTRIBUTE: &str = "attrname";
pub const ATTRIBUTEDEF_VALUE_ATTRIBUTE: &str = "value";
pub const ELEMENTS_ATTRIBUTE: &str = "elements";
pub const EXPORTABLE_ATTRIBUTE: &str = "exportable";

/// Return true if element is an attributedef.
pub fn is_attribute_def(elem: &ElementPtr) -> bool {
    elem.borrow().get_category() == category::ATTRIBUTE_DEF
}

/// AttributeDef: get "attrname" attribute
pub fn attrdef_get_attrname(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(ATTRNAME_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// AttributeDef: set "attrname" attribute
pub fn attrdef_set_attrname(elem: &ElementPtr, name: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(ATTRNAME_ATTRIBUTE, name.into());
}

/// AttributeDef: has "attrname" attribute
pub fn attrdef_has_attrname(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(ATTRNAME_ATTRIBUTE)
}

/// AttributeDef: get "elements" attribute (comma-separated element categories)
pub fn attrdef_get_elements(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(ELEMENTS_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// AttributeDef: set "elements" attribute
pub fn attrdef_set_elements(elem: &ElementPtr, elements: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(ELEMENTS_ATTRIBUTE, elements.into());
}

/// AttributeDef: has "elements" attribute
pub fn attrdef_has_elements(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(ELEMENTS_ATTRIBUTE)
}

/// AttributeDef: get "exportable" attribute as bool
pub fn attrdef_get_exportable(elem: &ElementPtr) -> bool {
    elem.borrow().get_attribute(EXPORTABLE_ATTRIBUTE) == Some("true")
}

/// AttributeDef: set "exportable" attribute
pub fn attrdef_set_exportable(elem: &ElementPtr, exportable: bool) {
    elem.borrow_mut().set_attribute(
        EXPORTABLE_ATTRIBUTE,
        if exportable { "true" } else { "false" },
    );
}

/// AttributeDef: has "exportable" attribute
pub fn attrdef_has_exportable(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(EXPORTABLE_ATTRIBUTE)
}

/// AttributeDef: get value string (uses "value" attribute).
/// Mirrors C++ AttributeDef::getValue which creates Value from string.
pub fn attrdef_get_value_string(elem: &ElementPtr) -> Option<String> {
    elem.borrow()
        .get_attribute(ATTRIBUTEDEF_VALUE_ATTRIBUTE)
        .map(|s| s.to_string())
}

/// AttributeDef: set value string
pub fn attrdef_set_value_string(elem: &ElementPtr, value: impl Into<String>) {
    elem.borrow_mut()
        .set_attribute(ATTRIBUTEDEF_VALUE_ATTRIBUTE, value.into());
}

/// AttributeDef: has value
pub fn attrdef_has_value(elem: &ElementPtr) -> bool {
    elem.borrow().has_attribute(ATTRIBUTEDEF_VALUE_ATTRIBUTE)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::document::create_document;
    use crate::core::element::category;

    #[test]
    fn test_nodedef_node_string() {
        let mut doc = create_document();
        let nd = doc
            .add_child_of_category(category::NODEDEF, "ND_standard_surface")
            .unwrap();
        nodedef_set_node_string(&nd, "standard_surface");
        assert_eq!(
            nodedef_get_node_string(&nd),
            Some("standard_surface".to_string())
        );
        assert!(is_node_def(&nd));
    }

    #[test]
    fn test_nodedef_node_group() {
        let mut doc = create_document();
        let nd = doc
            .add_child_of_category(category::NODEDEF, "ND_noise")
            .unwrap();
        assert!(!has_node_group(&nd));
        set_node_group(&nd, PROCEDURAL_NODE_GROUP);
        assert!(has_node_group(&nd));
        assert_eq!(get_node_group(&nd), Some(PROCEDURAL_NODE_GROUP.to_string()));
    }

    #[test]
    fn test_nodedef_get_type_single_output() {
        let mut doc = create_document();
        let nd = doc
            .add_child_of_category(category::NODEDEF, "ND_foo")
            .unwrap();
        let out = add_child_of_category(&nd, category::OUTPUT, "out").unwrap();
        out.borrow_mut().set_attribute("type", "color3");
        assert_eq!(nodedef_get_type(&nd), "color3");
    }

    #[test]
    fn test_nodedef_get_type_multioutput() {
        let mut doc = create_document();
        let nd = doc
            .add_child_of_category(category::NODEDEF, "ND_multi")
            .unwrap();
        add_child_of_category(&nd, category::OUTPUT, "out1").unwrap();
        add_child_of_category(&nd, category::OUTPUT, "out2").unwrap();
        assert_eq!(
            nodedef_get_type(&nd),
            crate::core::types::MULTI_OUTPUT_TYPE_STRING
        );
    }

    #[test]
    fn test_implementation_attrs() {
        let mut doc = create_document();
        let impl_elem = doc
            .add_child_of_category(category::IMPLEMENTATION, "IM_standard_surface")
            .unwrap();
        assert!(is_implementation(&impl_elem));
        impl_set_file(&impl_elem, "stdlib/genglsl/standard_surface.glsl");
        impl_set_function(&impl_elem, "mx_standard_surface");
        impl_set_nodedef_string(&impl_elem, "ND_standard_surface");
        assert_eq!(
            impl_get_file(&impl_elem),
            Some("stdlib/genglsl/standard_surface.glsl".to_string())
        );
        assert_eq!(
            impl_get_function(&impl_elem),
            Some("mx_standard_surface".to_string())
        );
        assert_eq!(
            impl_get_nodedef_string(&impl_elem),
            Some("ND_standard_surface".to_string())
        );
        assert!(impl_has_file(&impl_elem));
        assert!(impl_has_function(&impl_elem));
    }

    #[test]
    fn test_typedef_semantic_context() {
        let mut doc = create_document();
        let td = doc
            .add_child_of_category(category::TYPEDEF, "color4")
            .unwrap();
        assert!(is_type_def(&td));
        assert!(!typedef_has_semantic(&td));
        typedef_set_semantic(&td, COLOR_SEMANTIC);
        typedef_set_context(&td, "render");
        assert_eq!(typedef_get_semantic(&td), Some(COLOR_SEMANTIC.to_string()));
        assert_eq!(typedef_get_context(&td), Some("render".to_string()));
    }

    #[test]
    fn test_typedef_members() {
        let mut doc = create_document();
        let td = doc
            .add_child_of_category(category::TYPEDEF, "vector3")
            .unwrap();
        typedef_add_member(&td, "x").unwrap();
        typedef_add_member(&td, "y").unwrap();
        typedef_add_member(&td, "z").unwrap();
        let members = typedef_get_members(&td);
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].borrow().get_name(), "x");
        assert_eq!(members[2].borrow().get_name(), "z");
        typedef_remove_member(&td, "y");
        assert_eq!(typedef_get_members(&td).len(), 2);
    }

    #[test]
    fn test_impl_resolve_nodedef() {
        let mut doc = create_document();
        // NodeDef
        doc.add_child_of_category(category::NODEDEF, "ND_foo")
            .unwrap();
        // Implementation linking to it
        let impl_elem = doc
            .add_child_of_category(category::IMPLEMENTATION, "IM_foo")
            .unwrap();
        impl_set_nodedef_string(&impl_elem, "ND_foo");
        let resolved = impl_get_nodedef(&impl_elem, &doc).unwrap();
        assert_eq!(resolved.borrow().get_name(), "ND_foo");
    }
}
