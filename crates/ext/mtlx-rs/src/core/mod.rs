//! MaterialXCore -- core elements, document, nodes, types, traversal.

pub mod backdrop;
mod definition;
pub mod document;
pub mod element;
mod geom;
mod graph;
mod interface;
mod material;
mod node;
mod property;
mod traversal;
mod types;
mod unit;
pub mod util;
mod value;
mod variant;

pub use definition::{
    ADJUSTMENT_NODE_GROUP,
    ATTRNAME_ATTRIBUTE,
    CHANNEL_NODE_GROUP,
    // Semantic constants
    COLOR_SEMANTIC,
    CONDITIONAL_NODE_GROUP,
    CONTEXT_ATTRIBUTE,
    ELEMENTS_ATTRIBUTE,
    EXPORTABLE_ATTRIBUTE,
    // Attribute name constants
    FILE_ATTRIBUTE,
    FUNCTION_ATTRIBUTE,
    GEOMETRIC_NODE_GROUP,
    // Category strings
    MEMBER_CATEGORY,
    NODE_GROUP_ATTRIBUTE,
    ORGANIZATION_NODE_GROUP,
    PROCEDURAL_NODE_GROUP,
    SEMANTIC_ATTRIBUTE,
    SHADER_SEMANTIC,
    TARGET_DEF_CATEGORY,
    // Node group strings
    TEXTURE_NODE_GROUP,
    TRANSLATION_NODE_GROUP,
    UNIT_TYPE_DEF_CATEGORY,
    attrdef_get_attrname,
    attrdef_get_elements,
    attrdef_get_exportable,
    attrdef_get_value_string,
    attrdef_has_attrname,
    attrdef_has_elements,
    attrdef_has_exportable,
    attrdef_has_value,
    attrdef_set_attrname,
    attrdef_set_elements,
    attrdef_set_exportable,
    attrdef_set_value_string,
    get_implementation_for_nodedef,
    get_node_group,
    get_unit_defs_for_type,
    has_node_group,
    impl_get_declaration,
    impl_get_file,
    impl_get_function,
    impl_get_node_graph,
    impl_get_nodedef,
    impl_get_nodedef_string,
    impl_has_file,
    impl_has_function,
    impl_has_node_graph,
    impl_set_file,
    impl_set_function,
    impl_set_node_graph,
    impl_set_nodedef_string,
    // AttributeDef helpers
    is_attribute_def,
    // Implementation helpers
    is_implementation,
    // NodeDef helpers
    is_node_def,
    // TargetDef helpers
    is_target_def,
    // TypeDef helpers
    is_type_def,
    // UnitTypeDef helpers
    is_unit_type_def,
    is_version_compatible,
    nodedef_get_declaration,
    nodedef_get_input_hints,
    nodedef_get_inputs,
    nodedef_get_node_string,
    nodedef_get_outputs,
    nodedef_get_type,
    nodedef_set_node_string,
    set_node_group,
    targetdef_get_matching_targets,
    typedef_add_member,
    typedef_get_context,
    typedef_get_members,
    typedef_get_semantic,
    typedef_has_context,
    typedef_has_semantic,
    typedef_remove_member,
    typedef_set_context,
    typedef_set_semantic,
    // Implementation validation
    validate_implementation,
    // NodeDef validation & versioning
    validate_node_def,
};
pub use document::{Document, create_document};
pub use element::{
    Element,
    // Equivalence
    ElementEquivalenceOptions,
    ElementPtr,
    FloatFormat,
    TREE_ORDER_ATTRIBUTE,
    add_child_of_category,
    // Newline factory
    add_newline,
    // Category change
    change_child_category,
    copy_content_from_element,
    // String resolver factory
    create_string_resolver,
    // Active prefix helpers
    get_active_file_prefix,
    get_active_geom_prefix,
    get_active_unit,
    get_default_value_string,
    // Inheritance
    get_inherits_from,
    // ValueElement resolved/default/unit
    get_resolved_value_string,
    get_root,
    get_tree_order,
    has_inheritance_cycle,
    has_inherited_base,
    is_attribute_equivalent,
    is_equivalent,
    pretty_print,
    // Name reference resolution
    resolve_name_reference,
    set_inherits_from,
    // Tree order
    set_tree_order,
    target_strings_match,
    // Validation
    validate_element,
    validate_element_self,
};
pub use geom::{
    GEOM_PROP_ATTRIBUTE,
    GeomPath,
    INDEX_ATTRIBUTE,
    SPACE_ATTRIBUTE,
    add_geom_prop_child,
    geom_strings_match,
    get_active_exclude_geom,
    get_active_include_geom,
    get_collection,
    get_collection_string,
    get_exclude_geom,
    get_geom,
    get_geom_prop,
    get_geom_prop_child,
    get_geom_props,
    get_include_collection,
    get_include_collections,
    get_include_geom,
    get_index,
    get_space,
    has_collection_string,
    has_exclude_geom,
    has_geom,
    has_include_collection,
    has_include_cycle,
    has_include_geom,
    matches_geom_string,
    remove_geom_prop,
    // Collection resolved helpers
    set_collection,
    set_collection_string,
    set_exclude_geom,
    set_geom,
    // GeomInfo value helpers
    set_geom_info_prop_value,
    set_geom_info_token_value,
    set_geom_prop,
    set_include_collection,
    set_include_collection_ref,
    set_include_collections,
    set_include_geom,
    set_index,
    set_space,
    validate_collection,
    validate_geom_element,
};
pub use graph::{as_string_dot, flatten_subgraphs, topological_sort};
pub use interface::{
    add_input,
    add_output,
    add_token,
    clear_interface_content,
    get_active_input,
    get_active_inputs,
    get_active_output,
    get_active_outputs,
    get_active_tokens,
    get_active_value_element,
    get_active_value_elements,
    get_connected_interface_name,
    get_declaration,
    get_default_geom_prop,
    get_hint,
    get_input_value,
    get_node_def_string,
    get_node_string,
    get_token,
    get_token_value,
    get_tokens,
    get_version_integers,
    has_exact_input_match,
    has_hint,
    has_node_def_string,
    has_node_string,
    // Output-specific
    has_upstream_cycle,
    is_interface_element,
    remove_input,
    remove_output,
    remove_token,
    // Input-specific
    set_connected_interface_name,
    set_connected_output,
    set_hint,
    set_input_value,
    set_node_def_string,
    set_node_string,
    set_token_value,
    set_version_integers,
    // Validation
    validate_port,
};
pub use material::{
    add_material_assign,
    add_property_assign,
    add_property_set_assign,
    // VariantAssign on MaterialAssign
    add_variant_assign_to_material,
    add_visibility,
    get_active_material_assigns,
    get_active_property_assigns,
    get_active_property_set_assigns,
    // Look active variant assigns
    get_active_variant_assigns,
    get_active_variant_assigns_of_material,
    get_active_visibilities,
    get_connected_outputs,
    get_exclusive,
    get_geometry_bindings,
    get_look_group_active,
    get_look_group_looks,
    get_look_inherit_string,
    get_material_assign,
    get_material_assigns,
    // MaterialAssign material outputs
    get_material_outputs_for_assign,
    get_material_string,
    get_property_assign,
    get_property_assigns,
    get_property_set_assign,
    get_property_set_assigns,
    get_referenced_material,
    // Material processing
    get_shader_nodes,
    get_shader_refs,
    get_surface_shader_input,
    get_variant_assign_of_material,
    get_variant_assigns_of_material,
    get_viewer_collection,
    get_viewer_geom,
    get_visibilities,
    get_visibility,
    get_visibility_type,
    get_visible,
    has_material_string,
    remove_material_assign,
    remove_property_assign,
    remove_property_set_assign,
    remove_variant_assign_from_material,
    remove_visibility,
    set_exclusive,
    set_look_group_active,
    set_look_group_looks,
    set_look_inherit_string,
    set_material_string,
    set_viewer_collection,
    set_viewer_geom,
    set_visibility_type,
    set_visible,
};
pub use node::{
    add_geom_node,
    add_inputs_from_node_def,
    get_active_color_space,
    get_connected_node,
    get_connected_output,
    get_default_geom_prop_string,
    get_downstream_ports,
    get_input,
    get_inputs,
    get_interface_input,
    get_interface_name,
    get_interface_name_raw,
    get_node_def,
    // NodeDef output lookup
    get_node_def_output,
    // GraphElement methods
    get_nodes_of_type,
    get_output,
    get_outputs,
    has_default_geom_prop_string,
    has_interface_name,
    nodegraph_add_interface_name,
    nodegraph_get_downstream_ports,
    nodegraph_get_implementation,
    nodegraph_get_material_outputs,
    nodegraph_get_node_def_name,
    nodegraph_modify_interface_name,
    nodegraph_remove_interface_name,
    nodegraph_resolve_node_def,
    nodegraph_set_name_global,
    // NodeGraph methods
    nodegraph_set_node_def,
    set_connected_node_name,
    // Node rename
    set_name_global,
    // Validation
    validate_node,
    validate_node_graph,
};
pub use property::{
    add_property,
    get_properties,
    get_property,
    get_property_assign_collection,
    get_property_assign_geom,
    get_property_set_ref,
    get_property_set_ref_string,
    get_property_set_value,
    get_property_string,
    get_property_target,
    get_property_value_string,
    has_property_string,
    remove_property,
    set_property_assign_collection,
    set_property_assign_geom,
    set_property_set_ref,
    // PropertySetAssign helpers
    set_property_set_ref_string,
    // PropertySet value helpers
    set_property_set_value,
    set_property_string,
    set_property_target,
    set_property_value_string,
};
pub use traversal::{
    CycleError, Edge, GraphIterator, InheritanceIterator, TreeIterator, get_upstream_edge,
    get_upstream_edge_count, traverse_graph, traverse_graph_iter, traverse_inheritance,
    traverse_tree,
};
pub use types::*;
pub use unit::{
    LinearUnitConverter, UnitConverter, UnitConverterRegistry, UnitScale, get_unit_scales,
    get_unit_scales_from_typedef,
};
pub use util::*;
pub use value::{
    AggregateValue, ScopedFloatFormatting, Value, format_float, get_float_format,
    get_float_precision, parse_struct_value_string, set_float_format, set_float_precision,
};
// Value's FloatFormat is accessible as value::FloatFormat to avoid conflict with element::FloatFormat.
pub use value::FloatFormat as ValueFloatFormat;
pub use variant::{
    add_variant, add_variant_assign, get_variant, get_variant_assign, get_variant_assigns,
    get_variant_names, get_variant_set_string, get_variant_string, get_variants,
    has_variant_set_string, has_variant_string, remove_variant, remove_variant_assign,
    set_variant_set_string, set_variant_string,
};
