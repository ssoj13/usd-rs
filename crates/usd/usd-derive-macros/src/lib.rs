//! USD Procedural Macros
//!
//! This crate provides procedural macros for defining USD schemas in Rust.
//!
//! # Example
//!
//! ```ignore
//! use usd_derive_macros::{UsdSchema, UsdTyped};
//!
//! #[derive(UsdSchema)]
//! #[usd_prim_type("Mesh")]
//! pub struct Mesh {
//!     #[usd_attr(type = "point3f[]", interpolation = "vertex")]
//!     pub points: Vec<Vec3f>,
//!
//!     #[usd_attr(type = "normal3f[]", interpolation = "faceVarying")]
//!     pub normals: Option<Vec<Vec3f>>,
//!
//!     #[usd_attr(type = "int[]")]
//!     pub face_vertex_counts: Vec<i32>,
//! }
//! ```
//!
//! # Generated Code
//!
//! The `UsdSchema` derive macro generates:
//! - `impl UsdTyped` - type name and schema info
//! - `impl UsdSchemaBase` - base schema operations
//! - Attribute accessors (get/set/has/clear)
//! - `define()` static method for creating prims
//! - `get()` static method for wrapping existing prims

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Expr, Field, Fields, Ident, Lit, Meta, Type, parse_macro_input,
};

/// Derive macro for USD schema types.
///
/// # Attributes
///
/// ## Struct-level attributes
///
/// - `#[usd_prim_type("TypeName")]` - The USD prim type name (required)
/// - `#[usd_schema_kind("concrete")]` - Schema kind: concrete, abstract, api
/// - `#[usd_schema_base("UsdGeomGprim")]` - Base schema type
/// - `#[usd_doc("Documentation")]` - Schema documentation
///
/// ## Field-level attributes
///
/// - `#[usd_attr(type = "float")]` - USD type name (required)
/// - `#[usd_attr(default = "0.0")]` - Default value
/// - `#[usd_attr(interpolation = "vertex")]` - Interpolation mode
/// - `#[usd_attr(doc = "Help text")]` - Attribute documentation
/// - `#[usd_rel]` - Mark as relationship instead of attribute
///
/// # Example
///
/// ```ignore
/// #[derive(UsdSchema)]
/// #[usd_prim_type("MyCustomPrim")]
/// #[usd_schema_base("UsdTyped")]
/// pub struct MyCustomPrim {
///     #[usd_attr(type = "float", default = "1.0")]
///     pub intensity: f32,
///
///     #[usd_attr(type = "color3f", default = "(1, 1, 1)")]
///     pub color: Vec3f,
///
///     #[usd_rel]
///     pub target: Option<Path>,
/// }
/// ```
#[proc_macro_derive(
    UsdSchema,
    attributes(
        usd_prim_type,
        usd_schema_kind,
        usd_schema_base,
        usd_doc,
        usd_attr,
        usd_rel
    )
)]
pub fn derive_usd_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match impl_usd_schema(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive macro for basic USD typed trait.
///
/// Simpler than UsdSchema - just implements the type name.
///
/// ```ignore
/// #[derive(UsdTyped)]
/// #[usd_prim_type("MyType")]
/// struct MyType;
/// ```
#[proc_macro_derive(UsdTyped, attributes(usd_prim_type))]
pub fn derive_usd_typed(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match impl_usd_typed(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

// ============================================================================
// Implementation
// ============================================================================

fn impl_usd_schema(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;

    // Parse struct-level attributes
    let prim_type = get_prim_type(&input.attrs)?;
    let schema_kind = get_schema_kind(&input.attrs);
    let schema_base = get_schema_base(&input.attrs);
    let doc = get_doc(&input.attrs);

    // Get fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "UsdSchema requires named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "UsdSchema can only be derived for structs",
            ));
        }
    };

    // Validate that struct has a `prim` field (generated code references self.prim)
    let has_prim = fields
        .iter()
        .any(|f| f.ident.as_ref().map(|id| id == "prim").unwrap_or(false));
    if !has_prim {
        return Err(syn::Error::new_spanned(
            input,
            "UsdSchema requires a field named `prim` (e.g. `pub prim: Prim`)",
        ));
    }

    // Generate attribute accessors
    let mut attr_accessors = Vec::new();
    let mut attr_names = Vec::new();

    for field in fields {
        if let Some(accessor) = generate_field_accessor(field)? {
            attr_names.push(accessor.attr_name.clone());
            attr_accessors.push(accessor.methods);
        }
    }

    // Generate the impl blocks
    let typed_impl = generate_typed_impl(name, &prim_type);
    let schema_impl = generate_schema_impl(name, &prim_type, &schema_kind, &schema_base, &doc);
    let define_impl = generate_define_impl(name, &prim_type);
    let accessors_impl = generate_accessors_impl(name, &attr_accessors);
    let attr_names_impl = generate_attr_names_impl(name, &attr_names);

    Ok(quote! {
        #typed_impl
        #schema_impl
        #define_impl
        #accessors_impl
        #attr_names_impl
    })
}

fn impl_usd_typed(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let prim_type = get_prim_type(&input.attrs)?;

    Ok(generate_typed_impl(name, &prim_type))
}

// ============================================================================
// Attribute Parsing
// ============================================================================

fn get_prim_type(attrs: &[Attribute]) -> syn::Result<String> {
    for attr in attrs {
        if attr.path().is_ident("usd_prim_type") {
            let meta = &attr.meta;
            if let Meta::List(list) = meta {
                let tokens: TokenStream2 = list.tokens.clone();
                let lit: Lit = syn::parse2(tokens)?;
                if let Lit::Str(s) = lit {
                    return Ok(s.value());
                }
            }
        }
    }
    // Use call_site span so we don't panic on empty attrs (e.g. struct with only #[derive(...)])
    Err(syn::Error::new(
        Span::call_site(),
        "UsdSchema requires #[usd_prim_type(\"TypeName\")] attribute",
    ))
}

fn get_schema_kind(attrs: &[Attribute]) -> String {
    for attr in attrs {
        if attr.path().is_ident("usd_schema_kind") {
            if let Meta::List(list) = &attr.meta {
                if let Ok(Lit::Str(s)) = syn::parse2(list.tokens.clone()) {
                    return s.value();
                }
            }
        }
    }
    "concreteTyped".to_string()
}

fn get_schema_base(attrs: &[Attribute]) -> String {
    for attr in attrs {
        if attr.path().is_ident("usd_schema_base") {
            if let Meta::List(list) = &attr.meta {
                if let Ok(Lit::Str(s)) = syn::parse2(list.tokens.clone()) {
                    return s.value();
                }
            }
        }
    }
    "UsdTyped".to_string()
}

fn get_doc(attrs: &[Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("usd_doc") {
            if let Meta::List(list) = &attr.meta {
                if let Ok(Lit::Str(s)) = syn::parse2(list.tokens.clone()) {
                    return Some(s.value());
                }
            }
        }
        // Also check standard doc comments
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(s) = &expr_lit.lit {
                        return Some(s.value());
                    }
                }
            }
        }
    }
    None
}

// ============================================================================
// Field Parsing
// ============================================================================

#[allow(dead_code)] // Some fields reserved for future use
struct FieldAccessor {
    field_name: Ident,
    attr_name: String,
    usd_type: String,
    default_value: Option<String>,
    interpolation: Option<String>,
    doc: Option<String>,
    is_relationship: bool,
    rust_type: Type,
    methods: TokenStream2,
}

fn generate_field_accessor(field: &Field) -> syn::Result<Option<FieldAccessor>> {
    let field_name = field
        .ident
        .as_ref()
        .ok_or_else(|| syn::Error::new_spanned(field, "Field must have a name"))?;

    let rust_type = field.ty.clone();

    // Check for usd_rel (relationship)
    let is_relationship = field.attrs.iter().any(|a| a.path().is_ident("usd_rel"));

    // Check for usd_attr
    let mut usd_type = None;
    let mut default_value = None;
    let mut interpolation = None;
    let mut doc = None;

    for attr in &field.attrs {
        if attr.path().is_ident("usd_attr") {
            // Parse usd_attr(type = "...", default = "...", interpolation = "...", doc = "...")
            attr.parse_nested_meta(|meta| {
                // Helper: parse a string literal value or error on non-string
                let parse_str = |meta: &syn::meta::ParseNestedMeta| -> syn::Result<String> {
                    let value: Lit = meta.value()?.parse()?;
                    match value {
                        Lit::Str(s) => Ok(s.value()),
                        _ => Err(meta.error(format!(
                            "#[usd_attr({})] value must be a string literal",
                            meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default()
                        ))),
                    }
                };

                if meta.path.is_ident("type") {
                    usd_type = Some(parse_str(&meta)?);
                } else if meta.path.is_ident("default") {
                    default_value = Some(parse_str(&meta)?);
                } else if meta.path.is_ident("interpolation") {
                    interpolation = Some(parse_str(&meta)?);
                } else if meta.path.is_ident("doc") {
                    doc = Some(parse_str(&meta)?);
                } else {
                    // Unknown key — emit a compile error instead of silently ignoring
                    return Err(meta.error(format!(
                        "unrecognized #[usd_attr] key `{}`. Valid keys: type, default, interpolation, doc",
                        meta.path.get_ident().map(|i| i.to_string()).unwrap_or("?".into())
                    )));
                }
                Ok(())
            })?;
        }
    }

    // Skip fields without usd_attr or usd_rel
    if usd_type.is_none() && !is_relationship {
        return Ok(None);
    }

    let usd_type = usd_type.unwrap_or_else(|| "token".to_string());

    // Convert field name to USD attribute name (snake_case -> camelCase)
    let attr_name = to_usd_attr_name(&field_name.to_string());

    // Generate accessor methods
    let methods = if is_relationship {
        generate_relationship_methods(field_name, &attr_name, &rust_type)
    } else {
        generate_attribute_methods(
            field_name,
            &attr_name,
            &usd_type,
            &rust_type,
            &default_value,
        )
    };

    Ok(Some(FieldAccessor {
        field_name: field_name.clone(),
        attr_name,
        usd_type,
        default_value,
        interpolation,
        doc,
        is_relationship,
        rust_type,
        methods,
    }))
}

// ============================================================================
// Code Generation
// ============================================================================

/// Generate `impl UsdTyped` block.
///
/// # Path note
/// Generated code uses `crate::usd::*` and `crate::sdf::*` paths.
/// This macro is designed for use inside the `usd-rs` workspace only.
/// External consumers must re-export the required traits at those paths.
fn generate_typed_impl(name: &Ident, prim_type: &str) -> TokenStream2 {
    quote! {
        impl crate::usd::UsdTyped for #name {
            fn get_schema_type_name() -> &'static str {
                #prim_type
            }

            fn is_typed() -> bool {
                true
            }
        }
    }
}

fn generate_schema_impl(
    name: &Ident,
    _prim_type: &str,
    schema_kind: &str,
    schema_base: &str,
    doc: &Option<String>,
) -> TokenStream2 {
    let doc_str = doc.as_deref().unwrap_or("");

    quote! {
        impl crate::usd::UsdSchemaBase for #name {
            fn get_schema_kind() -> &'static str {
                #schema_kind
            }

            fn get_schema_base_type() -> &'static str {
                #schema_base
            }

            fn get_documentation() -> &'static str {
                #doc_str
            }

            fn get_prim(&self) -> &crate::usd::Prim {
                &self.prim
            }
        }
    }
}

fn generate_define_impl(name: &Ident, prim_type: &str) -> TokenStream2 {
    quote! {
        impl #name {
            /// Define a new prim of this type at the given path.
            ///
            /// Creates the prim if it doesn't exist, or returns the existing one
            /// if it's compatible with this schema type.
            pub fn define(
                stage: &crate::usd::Stage,
                path: &crate::sdf::Path,
            ) -> Option<Self> {
                let prim = stage.define_prim(path, #prim_type)?;
                Some(Self { prim })
            }

            /// Get an existing prim as this schema type.
            ///
            /// Returns None if the prim doesn't exist or isn't compatible.
            pub fn get(
                stage: &crate::usd::Stage,
                path: &crate::sdf::Path,
            ) -> Option<Self> {
                let prim = stage.get_prim_at_path(path)?;
                if prim.is_a(#prim_type) {
                    Some(Self { prim })
                } else {
                    None
                }
            }

            /// Wrap an existing prim as this schema type.
            ///
            /// Caller must ensure the prim is compatible.
            pub fn from_prim(prim: crate::usd::Prim) -> Self {
                Self { prim }
            }

            /// Get the underlying prim.
            pub fn get_prim(&self) -> &crate::usd::Prim {
                &self.prim
            }
        }
    }
}

fn generate_accessors_impl(name: &Ident, accessors: &[TokenStream2]) -> TokenStream2 {
    quote! {
        impl #name {
            #(#accessors)*
        }
    }
}

fn generate_attr_names_impl(name: &Ident, attr_names: &[String]) -> TokenStream2 {
    quote! {
        impl #name {
            /// Returns the list of all attribute names for this schema.
            pub fn get_schema_attribute_names() -> &'static [&'static str] {
                &[#(#attr_names),*]
            }
        }
    }
}

fn generate_attribute_methods(
    field_name: &Ident,
    attr_name: &str,
    usd_type: &str,
    rust_type: &Type,
    default_value: &Option<String>,
) -> TokenStream2 {
    let get_fn = format_ident!("get_{}", field_name);
    let set_fn = format_ident!("set_{}", field_name);
    let has_fn = format_ident!("has_{}", field_name);
    let clear_fn = format_ident!("clear_{}", field_name);
    let create_fn = format_ident!("create_{}_attr", field_name);

    let default_doc = default_value
        .as_ref()
        .map(|d| format!(" Default: {}", d))
        .unwrap_or_default();

    let get_doc = format!(
        "Get the {} attribute. USD type: {}.{}",
        attr_name, usd_type, default_doc
    );
    let set_doc = format!("Set the {} attribute.", attr_name);
    let has_doc = format!("Check if {} attribute is authored.", attr_name);
    let clear_doc = format!("Clear the {} attribute.", attr_name);
    let create_doc = format!("Create the {} attribute if it doesn't exist.", attr_name);

    quote! {
        #[doc = #get_doc]
        pub fn #get_fn(&self) -> Option<#rust_type> {
            self.prim.get_attribute(#attr_name)
                .and_then(|attr| attr.get::<#rust_type>())
        }

        #[doc = #set_doc]
        pub fn #set_fn(&self, value: #rust_type) -> bool {
            if let Some(attr) = self.prim.get_attribute(#attr_name) {
                attr.set(value)
            } else {
                false
            }
        }

        #[doc = #has_doc]
        pub fn #has_fn(&self) -> bool {
            self.prim.has_attribute(#attr_name)
        }

        #[doc = #clear_doc]
        pub fn #clear_fn(&self) -> bool {
            if let Some(attr) = self.prim.get_attribute(#attr_name) {
                attr.clear()
            } else {
                false
            }
        }

        #[doc = #create_doc]
        pub fn #create_fn(&self) -> Option<crate::usd::Attribute> {
            self.prim.create_attribute(
                #attr_name,
                crate::sdf::ValueTypeName::find(#usd_type),
            )
        }
    }
}

fn generate_relationship_methods(
    field_name: &Ident,
    rel_name: &str,
    _rust_type: &Type,
) -> TokenStream2 {
    let get_fn = format_ident!("get_{}_rel", field_name);
    let get_targets_fn = format_ident!("get_{}_targets", field_name);
    let set_targets_fn = format_ident!("set_{}_targets", field_name);
    let add_target_fn = format_ident!("add_{}_target", field_name);
    let clear_fn = format_ident!("clear_{}_targets", field_name);

    quote! {
        /// Get the relationship.
        pub fn #get_fn(&self) -> Option<crate::usd::Relationship> {
            self.prim.get_relationship(#rel_name)
        }

        /// Get relationship targets.
        pub fn #get_targets_fn(&self) -> Vec<crate::sdf::Path> {
            self.prim.get_relationship(#rel_name)
                .map(|rel| rel.get_targets())
                .unwrap_or_default()
        }

        /// Set relationship targets.
        pub fn #set_targets_fn(&self, targets: &[crate::sdf::Path]) -> bool {
            if let Some(rel) = self.prim.get_relationship(#rel_name) {
                rel.set_targets(targets)
            } else {
                false
            }
        }

        /// Add a target to the relationship.
        pub fn #add_target_fn(&self, target: &crate::sdf::Path) -> bool {
            if let Some(rel) = self.prim.get_relationship(#rel_name) {
                rel.add_target(target)
            } else {
                false
            }
        }

        /// Clear relationship targets.
        pub fn #clear_fn(&self) -> bool {
            if let Some(rel) = self.prim.get_relationship(#rel_name) {
                rel.clear_targets()
            } else {
                false
            }
        }
    }
}

// Reserved for future schema registration
#[allow(dead_code)]
fn generate_attr_registration(accessor: &FieldAccessor) -> TokenStream2 {
    let attr_name = &accessor.attr_name;
    let usd_type = &accessor.usd_type;
    let default_value = accessor.default_value.as_deref().unwrap_or("");
    let interpolation = accessor.interpolation.as_deref().unwrap_or("");
    let doc = accessor.doc.as_deref().unwrap_or("");

    quote! {
        crate::usd::SchemaAttrInfo {
            name: #attr_name,
            usd_type: #usd_type,
            default_value: #default_value,
            interpolation: #interpolation,
            doc: #doc,
        }
    }
}

// ============================================================================
// Utilities
// ============================================================================

/// Convert snake_case to camelCase for USD attribute names.
fn to_usd_attr_name(rust_name: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for c in rust_name.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_usd_attr_name() {
        assert_eq!(to_usd_attr_name("face_vertex_counts"), "faceVertexCounts");
        assert_eq!(to_usd_attr_name("points"), "points");
        assert_eq!(to_usd_attr_name("uv_set"), "uvSet");
    }
}
