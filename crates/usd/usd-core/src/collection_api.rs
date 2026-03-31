//! UsdCollectionAPI - API schema for collections.
//!
//! Port of pxr/usd/usd/collectionAPI.h/cpp
//!
//! A general purpose API schema used to describe a collection of prims
//! and properties within a scene. This API schema can be applied to a prim
//! multiple times with different instance names to define several collections
//! on a single prim.

use crate::object::Object;
use crate::prim_flags::PrimFlagsPredicate;
use crate::schema_base::APISchemaBase;
use crate::{Attribute, Prim, Relationship, Stage};
use std::collections::HashSet;
use usd_sdf::{Path, PathExpression, TimeCode};
use usd_tf::Token;
use usd_vt::Value;

// Re-export for convenience
pub use crate::collection_membership_query::{
    CollectionMembershipQuery, CollectionMembershipQueryBase, PathExpansionRuleMap, SdfPathSet,
};

// ============================================================================
// CollectionAPI
// ============================================================================

/// API schema for collections.
///
/// Matches C++ `UsdCollectionAPI`.
///
/// This is a MultipleApplyAPI schema - it can be applied multiple times
/// to a prim with different instance names.
#[derive(Debug, Clone)]
pub struct CollectionAPI {
    /// Base API schema with instance name.
    base: APISchemaBase,
    /// Schema fallback for includeRoot when attr is not authored.
    /// Default is false per USD spec. LightAPI overrides to true.
    include_root_fallback: bool,
}

impl CollectionAPI {
    /// Constructs a CollectionAPI from a prim and instance name.
    ///
    /// Matches C++ `UsdCollectionAPI(UsdPrim, TfToken)`.
    pub fn new(prim: Prim, name: Token) -> Self {
        Self {
            base: APISchemaBase::new_with_instance(prim, name),
            include_root_fallback: false,
        }
    }

    /// Constructs a CollectionAPI with a custom includeRoot fallback.
    ///
    /// Used by LightAPI/LightFilter where schema declares includeRoot=true.
    pub fn new_with_include_root_fallback(
        prim: Prim,
        name: Token,
        include_root_fallback: bool,
    ) -> Self {
        Self {
            base: APISchemaBase::new_with_instance(prim, name),
            include_root_fallback,
        }
    }

    /// Constructs an invalid CollectionAPI.
    ///
    /// Matches C++ default constructor.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
            include_root_fallback: false,
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid() && self.base.instance_name().is_some()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.base.prim()
    }

    /// Returns the instance name (collection name).
    ///
    /// Matches C++ `GetName()`.
    pub fn name(&self) -> Option<&Token> {
        self.base.instance_name()
    }

    /// Returns the path to this prim.
    pub fn path(&self) -> &Path {
        self.prim().path()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("CollectionAPI")
    }

    /// Gets a CollectionAPI from a stage and path.
    ///
    /// Matches C++ `UsdCollectionAPI::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    /// Path must be of format <path>.collection:name
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if !path.is_property_path() {
            return Self::invalid();
        }

        let mut name = Token::new("");
        if !Self::is_collection_api_path(path, &mut name) {
            return Self::invalid();
        }

        if let Some(prim) = stage.get_prim_at_path(&path.get_prim_path()) {
            Self::new(prim, name)
        } else {
            Self::invalid()
        }
    }

    /// Gets a CollectionAPI from a prim and name.
    ///
    /// Matches C++ `UsdCollectionAPI::Get(const UsdPrim &prim, const TfToken &name)`.
    pub fn get_from_prim(prim: &Prim, name: &Token) -> Self {
        Self::new(prim.clone(), name.clone())
    }

    /// Returns all named instances of CollectionAPI on the given prim.
    ///
    /// Matches C++ `GetAll(const UsdPrim &prim)`.
    pub fn get_all(prim: &Prim) -> Vec<Self> {
        let applied_schemas = prim.get_applied_schemas();
        let mut collections = Vec::new();

        let coll_api_prefix = format!("{}:", Self::schema_type_name().get_text());

        for schema_name in applied_schemas {
            let schema_str = schema_name.get_text();
            if schema_str.starts_with(&coll_api_prefix) {
                let collection_name = schema_str[coll_api_prefix.len()..].to_string();
                collections.push(Self::new(prim.clone(), Token::new(&collection_name)));
            }
        }

        collections
    }

    /// Checks if the given baseName is the base name of a property of CollectionAPI.
    ///
    /// Matches C++ `IsSchemaPropertyBaseName(const TfToken &baseName)`.
    pub fn is_schema_property_base_name(base_name: &Token) -> bool {
        let base_name_str = base_name.get_text();
        matches!(
            base_name_str,
            "expansionRule" | "includeRoot" | "membershipExpression" | "" | "includes" | "excludes"
        )
    }

    /// Checks if the given path is of an API schema of type CollectionAPI.
    ///
    /// Matches C++ `IsCollectionAPIPath(const SdfPath &path, TfToken *name)`.
    pub fn is_collection_api_path(path: &Path, name: &mut Token) -> bool {
        if !path.is_property_path() {
            return false;
        }

        let property_name = path.get_name();
        let tokens: Vec<&str> = property_name.split(':').collect();

        // Check if base name is a schema property (invalid)
        if let Some(base_name) = tokens.last() {
            if Self::is_schema_property_base_name(&Token::new(base_name)) {
                return false;
            }
        }

        // Check if starts with "collection:"
        if tokens.len() >= 2 && tokens[0] == "collection" {
            let collection_name = tokens[1..].join(":");
            *name = Token::new(&collection_name);
            return true;
        }

        false
    }

    /// Returns true if this multiple-apply API schema can be applied.
    ///
    /// Matches C++ `CanApply(const UsdPrim &prim, const TfToken &name, std::string *whyNot)`.
    pub fn can_apply(prim: &Prim, name: &Token, why_not: &mut Option<String>) -> bool {
        // Basic validation - check if name is valid identifier
        if name.get_text().is_empty() {
            if let Some(reason) = why_not {
                *reason = "Collection name cannot be empty".to_string();
            }
            return false;
        }

        // Check if prim can apply API
        use super::schema_registry::SchemaRegistry;
        let schema_type_name = SchemaRegistry::get_api_schema_type_name("CollectionAPI");
        prim.can_apply_api_instance(&schema_type_name, name)
    }

    /// Applies this multiple-apply API schema to the given prim.
    ///
    /// Matches C++ `Apply(const UsdPrim &prim, const TfToken &name)`.
    pub fn apply(prim: &Prim, name: &Token) -> Self {
        use super::schema_registry::SchemaRegistry;
        let schema_type_name = SchemaRegistry::get_api_schema_type_name("CollectionAPI");
        if prim.apply_api_instance(&schema_type_name, name) {
            return Self::new(prim.clone(), name.clone());
        }
        Self::invalid()
    }

    /// Return a vector of names of all pre-declared attributes for this schema.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        vec![
            Token::new("collection:expansionRule"),
            Token::new("collection:includeRoot"),
            Token::new("collection:membershipExpression"),
            Token::new("collection:"),
        ]
    }

    /// Return a vector of names of all pre-declared attributes for a given instance name.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited, const TfToken &instanceName)`.
    pub fn get_schema_attribute_names_for_instance(
        _include_inherited: bool,
        instance_name: &Token,
    ) -> Vec<Token> {
        let prefix = format!("collection:{}:", instance_name.get_text());
        vec![
            Token::new(&format!("{}expansionRule", prefix)),
            Token::new(&format!("{}includeRoot", prefix)),
            Token::new(&format!("{}membershipExpression", prefix)),
            Token::new(&prefix.to_string()),
        ]
    }

    // ========================================================================
    // Property Accessors
    // ========================================================================

    /// Makes a namespaced property name for this collection instance.
    fn make_namespaced_property_name(&self, prop_name: &str) -> String {
        if let Some(name) = self.name() {
            format!("collection:{}:{}", name.get_text(), prop_name)
        } else {
            format!("collection:{}", prop_name)
        }
    }

    /// Gets the expansionRule attribute.
    ///
    /// Matches C++ `GetExpansionRuleAttr()`.
    pub fn get_expansion_rule_attr(&self) -> Attribute {
        let attr_name = self.make_namespaced_property_name("expansionRule");
        self.prim()
            .get_attribute(&attr_name)
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the expansionRule attribute.
    ///
    /// Matches C++ `CreateExpansionRuleAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_expansion_rule_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let attr_name = self.make_namespaced_property_name("expansionRule");
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_or_create_type_name(&Token::new("token"));
        self.prim()
            .create_attribute(
                &attr_name,
                &type_name,
                false,
                Some(crate::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Gets the includeRoot attribute.
    ///
    /// Matches C++ `GetIncludeRootAttr()`.
    pub fn get_include_root_attr(&self) -> Attribute {
        let attr_name = self.make_namespaced_property_name("includeRoot");
        self.prim()
            .get_attribute(&attr_name)
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the includeRoot attribute.
    ///
    /// Matches C++ `CreateIncludeRootAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_include_root_attr(
        &self,
        default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let attr_name = self.make_namespaced_property_name("includeRoot");
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_or_create_type_name(&Token::new("bool"));
        let attr = self
            .prim()
            .create_attribute(
                &attr_name,
                &type_name,
                false,
                Some(crate::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid);
        if let Some(value) = default_value {
            attr.set(value, usd_sdf::TimeCode::default());
        }
        attr
    }

    /// Gets the membershipExpression attribute.
    ///
    /// Matches C++ `GetMembershipExpressionAttr()`.
    pub fn get_membership_expression_attr(&self) -> Attribute {
        let attr_name = self.make_namespaced_property_name("membershipExpression");
        self.prim()
            .get_attribute(&attr_name)
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the membershipExpression attribute.
    ///
    /// Matches C++ `CreateMembershipExpressionAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_membership_expression_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let attr_name = self.make_namespaced_property_name("membershipExpression");
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_or_create_type_name(&Token::new("pathExpression"));
        self.prim()
            .create_attribute(
                &attr_name,
                &type_name,
                false,
                Some(crate::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Gets the collection attribute (opaque property representing the collection).
    ///
    /// Matches C++ `GetCollectionAttr()`.
    pub fn get_collection_attr(&self) -> Attribute {
        let attr_name = if let Some(name) = self.name() {
            format!("collection:{}:", name.get_text())
        } else {
            "collection:".to_string()
        };
        self.prim()
            .get_attribute(&attr_name)
            .unwrap_or_else(Attribute::invalid)
    }

    /// Creates the collection attribute.
    ///
    /// Matches C++ `CreateCollectionAttr(VtValue const &defaultValue, bool writeSparsely)`.
    pub fn create_collection_attr(
        &self,
        _default_value: Option<Value>,
        _write_sparsely: bool,
    ) -> Attribute {
        let attr_name = if let Some(name) = self.name() {
            format!("collection:{}:", name.get_text())
        } else {
            "collection:".to_string()
        };
        let registry = usd_sdf::ValueTypeRegistry::instance();
        let type_name = registry.find_or_create_type_name(&Token::new("opaque"));
        self.prim()
            .create_attribute(
                &attr_name,
                &type_name,
                false,
                Some(crate::attribute::Variability::Uniform),
            )
            .unwrap_or_else(Attribute::invalid)
    }

    /// Gets the includes relationship.
    ///
    /// Matches C++ `GetIncludesRel()`.
    pub fn get_includes_rel(&self) -> Relationship {
        let rel_name = self.make_namespaced_property_name("includes");
        self.prim()
            .get_relationship(&rel_name)
            .unwrap_or_else(Relationship::invalid)
    }

    /// Creates the includes relationship.
    ///
    /// Matches C++ `CreateIncludesRel()`.
    pub fn create_includes_rel(&self) -> Relationship {
        let rel_name = self.make_namespaced_property_name("includes");
        self.prim()
            .create_relationship(&rel_name, false)
            .unwrap_or_else(Relationship::invalid)
    }

    /// Gets the excludes relationship.
    ///
    /// Matches C++ `GetExcludesRel()`.
    pub fn get_excludes_rel(&self) -> Relationship {
        let rel_name = self.make_namespaced_property_name("excludes");
        self.prim()
            .get_relationship(&rel_name)
            .unwrap_or_else(Relationship::invalid)
    }

    /// Creates the excludes relationship.
    ///
    /// Matches C++ `CreateExcludesRel()`.
    pub fn create_excludes_rel(&self) -> Relationship {
        let rel_name = self.make_namespaced_property_name("excludes");
        self.prim()
            .create_relationship(&rel_name, false)
            .unwrap_or_else(Relationship::invalid)
    }

    // ========================================================================
    // Collection Path Methods
    // ========================================================================

    /// Returns the collection represented by the given collection path.
    ///
    /// Matches C++ `GetCollection(const UsdStagePtr &stage, const SdfPath &collectionPath)`.
    pub fn get_collection(stage: &Stage, collection_path: &Path) -> Self {
        let mut name = Token::new("");
        if !Self::is_collection_api_path(collection_path, &mut name) {
            return Self::invalid();
        }

        if let Some(prim) = stage.get_prim_at_path(&collection_path.get_prim_path()) {
            Self::new(prim, name)
        } else {
            Self::invalid()
        }
    }

    /// Returns the schema object representing a collection named name on the given prim.
    ///
    /// Matches C++ `GetCollection(const UsdPrim &prim, const TfToken &name)`.
    pub fn get_collection_from_prim(prim: &Prim, name: &Token) -> Self {
        Self::new(prim.clone(), name.clone())
    }

    /// Returns all the named collections on the given USD prim.
    ///
    /// Matches C++ `GetAllCollections(const UsdPrim &prim)`.
    pub fn get_all_collections(prim: &Prim) -> Vec<Self> {
        Self::get_all(prim)
    }

    /// Returns the canonical path that represents this collection.
    ///
    /// Matches C++ `GetCollectionPath()`.
    pub fn get_collection_path(&self) -> Path {
        let attr_name = if let Some(name) = self.name() {
            format!("collection:{}", name.get_text())
        } else {
            "collection".to_string()
        };
        self.path()
            .append_property(&attr_name)
            .unwrap_or_else(Path::empty)
    }

    /// Returns the canonical path to the collection named name on the given prim.
    ///
    /// Matches C++ `GetNamedCollectionPath(const UsdPrim &prim, const TfToken &collectionName)`.
    pub fn get_named_collection_path(prim: &Prim, collection_name: &Token) -> Path {
        let attr_name = format!("collection:{}", collection_name.get_text());
        prim.path()
            .append_property(&attr_name)
            .unwrap_or_else(Path::empty)
    }

    // ========================================================================
    // Membership Query Methods
    // ========================================================================

    /// Obtains a complete SdfPathExpression from this collection's membershipExpression.
    ///
    /// Resolves the complete membership expression by recursively resolving all references.
    ///
    /// Matches C++ `ResolveCompleteMembershipExpression()`.
    ///
    /// This method resolves all expression references (e.g., `%:collectionName`) in the
    /// membership expression by recursively following references to other collections.
    /// Circular dependencies are detected and result in empty expressions with warnings.
    ///
    /// # Returns
    ///
    /// A `PathExpression` with all references resolved, or an empty expression if
    /// circular dependencies are detected or referenced collections are not found.
    pub fn resolve_complete_membership_expression(&self) -> PathExpression {
        let mut visited = std::collections::HashSet::new();
        self.resolve_complete_membership_expression_impl(self, &mut visited)
    }

    /// Internal helper for recursive resolution.
    ///
    /// Matches C++ `_ResolveCompleteMembershipExpressionImpl`.
    fn resolve_complete_membership_expression_impl(
        &self,
        original_coll: &CollectionAPI,
        visited: &mut std::collections::HashSet<(Path, Token)>,
    ) -> PathExpression {
        // Track visited collections to avoid infinite recursion
        let instance_name = self
            .base
            .instance_name()
            .cloned()
            .unwrap_or_else(|| Token::new(""));
        let coll_key = (self.prim().path().clone(), instance_name.clone());
        if !visited.insert(coll_key.clone()) {
            // Circular dependency detected - return empty expression
            return PathExpression::new();
        }

        let attr = self.get_membership_expression_attr();
        let expr = if attr.is_valid() {
            if let Some(value) = attr.get(usd_sdf::TimeCode::default()) {
                if let Some(path_expr) = value.downcast_clone::<PathExpression>() {
                    path_expr
                } else {
                    PathExpression::new()
                }
            } else {
                PathExpression::new()
            }
        } else {
            PathExpression::new()
        };

        // Resolve references in the expression using PathExpression::resolve_references
        let this_prim = self.prim();
        let this_prim_path = this_prim.path().clone();
        let this_stage = this_prim.stage();
        use std::cell::RefCell;
        use std::rc::Rc;
        let visited_refcell = Rc::new(RefCell::new(std::mem::take(visited)));
        let visited_refcell_clone = visited_refcell.clone();
        let resolved_expr = expr.resolve_references(move |ref_expr| {
            // Extract collection name from reference
            let ref_collection_name = Token::new(&ref_expr.name);

            // Get the referenced prim
            // If ref.path is empty, use the current prim's path
            let ref_prim_path = if ref_expr.path.is_empty() {
                this_prim_path.clone()
            } else {
                ref_expr.path.clone()
            };

            // Get the prim from the stage
            let ref_collection_prim = if ref_prim_path == this_prim_path {
                Some(this_prim.clone())
            } else if let Some(ref stage) = this_stage {
                stage.get_prim_at_path(&ref_prim_path)
            } else {
                None
            };

            let ref_collection_prim = ref_collection_prim.unwrap_or_else(Prim::invalid);
            let ref_collection_prim_path = ref_collection_prim.path().clone();

            // Create CollectionAPI from referenced prim and name
            let ref_collection = if ref_collection_prim.is_valid() {
                CollectionAPI::new(ref_collection_prim, ref_collection_name.clone())
            } else {
                CollectionAPI::invalid()
            };

            // If we can't find a collection, return empty expression
            if !ref_collection.is_valid() {
                usd_tf::tf_warn!(
                    "Collection '{}' not found at path '{}'",
                    ref_collection_name.as_str(),
                    ref_prim_path
                );
                return PathExpression::new();
            }

            // Check for circular dependency
            let ref_coll_key = (ref_collection_prim_path, ref_collection_name.clone());
            if visited_refcell_clone.borrow().contains(&ref_coll_key) {
                // Circular dependency detected - return empty expression
                usd_tf::tf_warn!(
                    "Circular dependency detected in collection '{}' at path '{}'",
                    ref_collection_name.as_str(),
                    ref_prim_path
                );
                return PathExpression::new();
            }

            // Recursively resolve the referenced collection's expression
            let mut visited_borrowed = visited_refcell_clone.borrow_mut();
            let ret = ref_collection
                .resolve_complete_membership_expression_impl(original_coll, &mut visited_borrowed);
            drop(visited_borrowed);
            ret
        });

        // Restore visited set
        *visited = Rc::try_unwrap(visited_refcell)
            .expect("single owner")
            .into_inner();
        // Remove from visited after processing
        visited.remove(&coll_key);

        resolved_expr
    }

    /// Computes and returns a CollectionMembershipQuery object.
    ///
    /// Matches C++ `ComputeMembershipQuery()`.
    pub fn compute_membership_query(&self) -> CollectionMembershipQuery {
        let mut query = CollectionMembershipQuery::new();
        self.compute_membership_query_into(&mut query);
        query
    }

    /// Populates the CollectionMembershipQuery object with data from this collection.
    ///
    /// Matches C++ `ComputeMembershipQuery(UsdCollectionMembershipQuery *query)`.
    pub fn compute_membership_query_into(&self, query: &mut CollectionMembershipQuery) {
        use std::collections::HashSet;

        let mut chained_collection_paths = HashSet::new();
        self.compute_membership_query_impl(query, &mut chained_collection_paths);
    }

    /// Internal implementation of membership query computation.
    ///
    /// Matches C++ `_ComputeMembershipQueryImpl`.
    pub(crate) fn compute_membership_query_impl(
        &self,
        query: &mut CollectionMembershipQuery,
        chained_collection_paths: &mut HashSet<Path>,
    ) {
        use usd_sdf::TimeCode;

        // Check for circular dependencies
        let collection_path = self.get_collection_path();
        if !chained_collection_paths.insert(collection_path.clone()) {
            // Circular dependency - return empty query
            return;
        }

        // Get expansion rule
        let expansion_rule_token = {
            let attr = self.get_expansion_rule_attr();
            if attr.is_valid() {
                if let Some(value) = attr.get(TimeCode::default()) {
                    if let Some(token) = value.downcast_clone::<Token>() {
                        token
                    } else {
                        Token::new("expandPrims")
                    }
                } else {
                    Token::new("expandPrims")
                }
            } else {
                Token::new("expandPrims")
            }
        };

        let expansion_rule_str = expansion_rule_token.as_str();
        let expansion_rule = if expansion_rule_str == "explicitOnly" {
            Token::new("explicitOnly")
        } else if expansion_rule_str == "expandPrims" {
            Token::new("expandPrims")
        } else if expansion_rule_str == "expandPrimsAndProperties" {
            Token::new("expandPrimsAndProperties")
        } else {
            Token::new("expandPrims") // default
        };

        // Get include root — use schema fallback when attr not authored
        let include_root = {
            let attr = self.get_include_root_attr();
            if attr.is_valid() {
                if let Some(value) = attr.get(TimeCode::default()) {
                    value
                        .downcast_clone::<bool>()
                        .unwrap_or(self.include_root_fallback)
                } else {
                    self.include_root_fallback
                }
            } else {
                self.include_root_fallback
            }
        };

        // Get membership expression attribute for later use
        let _membership_expr_attr = self.get_membership_expression_attr();
        let membership_expr = self.resolve_complete_membership_expression();

        // Get includes and excludes relationships
        let includes_rel = self.get_includes_rel();
        let excludes_rel = self.get_excludes_rel();
        let mut includes = includes_rel.get_targets();
        let excludes = excludes_rel.get_targets();

        // Consult includeRoot and include </> if requested
        // includeRoot is not meaningful in combination with explicitOnly
        if expansion_rule != "explicitOnly" && include_root {
            includes.push(Path::absolute_root());
        }

        // Get stage for resolving nested collections
        let stage = self.prim().stage();

        // Build path expansion rule map
        let mut rule_map = query.get_as_path_expansion_rule_map().clone();
        let mut included_collections = query.get_included_collections().clone();

        // Process includes
        for included_path in includes {
            // Check if the included path is a collection
            let mut collection_name = Token::new("");
            if Self::is_collection_api_path(&included_path, &mut collection_name) {
                // Check for circular dependency
                if chained_collection_paths.contains(&included_path) {
                    // Circular dependency detected - skip
                    continue;
                }

                // Get the prim for the included collection
                let included_prim_path = included_path.get_prim_path();
                if let Some(included_prim) = stage
                    .as_ref()
                    .and_then(|s| s.get_prim_at_path(&included_prim_path))
                {
                    let included_collection = CollectionAPI::new(included_prim, collection_name);

                    // Recursively compute the included collection's membership map
                    let mut seen_collection_paths = chained_collection_paths.clone();
                    seen_collection_paths.insert(included_path.clone());
                    let mut included_query = CollectionMembershipQuery::new();
                    included_collection.compute_membership_query_impl(
                        &mut included_query,
                        &mut seen_collection_paths,
                    );

                    // Merge path expansion rule maps
                    let included_map = included_query.get_as_path_expansion_rule_map();
                    for (path, rule) in included_map {
                        rule_map.insert(path.clone(), rule.clone());
                    }

                    // Merge included collections
                    included_collections.insert(included_path.clone());
                    for coll_path in included_query.get_included_collections() {
                        included_collections.insert(coll_path.clone());
                    }
                }
            } else {
                // Regular path - add to rule map
                rule_map.insert(included_path, expansion_rule.clone());
            }
        }

        // Process excludes after includes
        for exclude_path in excludes {
            rule_map.insert(exclude_path, Token::new("exclude"));
        }

        // Update query with rule map and top expansion rule
        *query = CollectionMembershipQuery::new_with_map_and_rule(
            rule_map,
            included_collections,
            expansion_rule,
        );

        // Set expression evaluator if we have a membership expression
        if !membership_expr.is_empty() {
            use super::collection_membership_query::ObjectCollectionExpressionEvaluator;
            if let Some(stage_arc) = stage {
                let stage_weak = std::sync::Arc::downgrade(&stage_arc);
                query.set_expression_evaluator(
                    ObjectCollectionExpressionEvaluator::new_with_stage(
                        stage_weak,
                        membership_expr,
                    ),
                );
            }
        }

        // Remove from chained paths after processing
        chained_collection_paths.remove(&collection_path);
    }

    /// Returns true if the collection cannot possibly include anything.
    ///
    /// Matches C++ `HasNoIncludedPaths()`.
    pub fn has_no_included_paths(&self) -> bool {
        let includes_rel = self.get_includes_rel();
        let excludes_rel = self.get_excludes_rel();
        let includes = includes_rel.get_targets();
        let excludes = excludes_rel.get_targets();

        let mut include_root = false;
        if let Some(attr) = self
            .prim()
            .get_attribute(&self.make_namespaced_property_name("includeRoot"))
        {
            if let Some(value) = attr.get(TimeCode::default()) {
                if let Some(bool_val) = value.downcast_clone::<bool>() {
                    include_root = bool_val;
                }
            }
        }

        let membership_expr_attr = self.get_membership_expression_attr();
        let has_membership_expr =
            membership_expr_attr.is_valid() && membership_expr_attr.has_authored_value();

        includes.is_empty() && !include_root && (excludes.is_empty() || !has_membership_expr)
    }

    /// Returns true if this collection is in relationships-mode.
    ///
    /// Matches C++ `IsInRelationshipsMode()`.
    pub fn is_in_relationships_mode(&self) -> bool {
        let includes_rel = self.get_includes_rel();
        let excludes_rel = self.get_excludes_rel();
        let includes = includes_rel.get_targets();
        let excludes = excludes_rel.get_targets();

        let mut include_root = false;
        if let Some(attr) = self
            .prim()
            .get_attribute(&self.make_namespaced_property_name("includeRoot"))
        {
            if let Some(value) = attr.get(TimeCode::default()) {
                if let Some(bool_val) = value.downcast_clone::<bool>() {
                    include_root = bool_val;
                }
            }
        }

        !includes.is_empty() || !excludes.is_empty() || include_root
    }

    /// Returns true if this collection is in expression-mode.
    ///
    /// Matches C++ `IsInExpressionMode()`.
    pub fn is_in_expression_mode(&self) -> bool {
        !self.is_in_relationships_mode()
    }

    // ========================================================================
    // Authoring API
    // ========================================================================

    /// Includes or adds the given path in the collection.
    ///
    /// Matches C++ `IncludePath(const SdfPath &pathToInclude)`.
    pub fn include_path(&self, path_to_include: &Path) -> bool {
        // Check if already included
        let query = self.compute_membership_query();
        if query.is_path_included(path_to_include, None) {
            return true;
        }

        if path_to_include.is_absolute_root_path() {
            let attr = self.create_include_root_attr(Some(Value::from(true)), false);
            return attr.set(Value::from(true), usd_sdf::TimeCode::default());
        }

        // Remove from excludes if present
        let excludes_rel = self.get_excludes_rel();
        let excludes = excludes_rel.get_targets();
        if excludes.contains(path_to_include) {
            excludes_rel.remove_target(path_to_include);
        }

        // Add to includes
        let includes_rel = self.create_includes_rel();
        includes_rel.add_target(path_to_include)
    }

    /// Excludes or removes the given path from the collection.
    ///
    /// Matches C++ `ExcludePath(const SdfPath &pathToExclude)`.
    pub fn exclude_path(&self, path_to_exclude: &Path) -> bool {
        let query = self.compute_membership_query();
        let map = query.get_as_path_expansion_rule_map();

        if !map.is_empty() && !query.is_path_included(path_to_exclude, None) {
            return true;
        }

        if path_to_exclude.is_absolute_root_path() {
            let attr = self.create_include_root_attr(Some(Value::from(false)), false);
            return attr.set(Value::from(false), usd_sdf::TimeCode::default());
        }

        // Remove from includes if present
        let includes_rel = self.get_includes_rel();
        let includes = includes_rel.get_targets();
        if includes.contains(path_to_exclude) {
            includes_rel.remove_target(path_to_exclude);
        }

        // Add to excludes
        let excludes_rel = self.create_excludes_rel();
        excludes_rel.add_target(path_to_exclude)
    }

    /// Validates the collection.
    ///
    /// Matches C++ `Validate(std::string *reason)`.
    pub fn validate(&self, reason: &mut Option<String>) -> bool {
        use usd_sdf::TimeCode;

        // Check expansion rule is valid
        let attr = self.get_expansion_rule_attr();
        if attr.is_valid() {
            if let Some(value) = attr.get(TimeCode::default()) {
                if let Some(token) = value.downcast_clone::<Token>() {
                    let token_str = token.as_str();
                    if token_str != "explicitOnly"
                        && token_str != "expandPrims"
                        && token_str != "expandPrimsAndProperties"
                    {
                        *reason = Some(format!("Invalid expansion rule: {}", token_str));
                        return false;
                    }
                }
            }
        }

        // Check for circular dependencies by attempting to resolve expression
        let mut visited = std::collections::HashSet::new();
        let _ = self.resolve_complete_membership_expression_impl(self, &mut visited);
        // If we detected a cycle, visited would have been modified

        // Check for conflicting includes/excludes
        let includes = self.get_includes_rel().get_targets();
        let excludes = self.get_excludes_rel().get_targets();

        for include_path in &includes {
            if excludes.contains(include_path) {
                *reason = Some(format!(
                    "Path {} is both included and excluded",
                    include_path
                ));
                return false;
            }
        }

        true
    }

    /// Resets the collection by clearing both includes and excludes targets.
    ///
    /// Matches C++ `ResetCollection()`.
    pub fn reset_collection(&self) -> bool {
        let includes_rel = self.get_includes_rel();
        let excludes_rel = self.get_excludes_rel();

        let includes = includes_rel.get_targets();
        let excludes = excludes_rel.get_targets();

        let mut success = true;
        for path in includes {
            if !includes_rel.remove_target(&path) {
                success = false;
            }
        }
        for path in excludes {
            if !excludes_rel.remove_target(&path) {
                success = false;
            }
        }

        success
    }

    /// Blocks the targets of the includes and excludes relationships.
    ///
    /// Matches C++ `BlockCollection()`.
    pub fn block_collection(&self) -> bool {
        // Block relationships by setting empty target lists
        // This explicitly blocks weaker opinions
        let includes_rel = self.get_includes_rel();
        let excludes_rel = self.get_excludes_rel();
        let membership_expr_attr = self.get_membership_expression_attr();

        let mut success = true;

        // Block includes relationship
        if !includes_rel.set_targets(&[]) {
            success = false;
        }

        // Block excludes relationship
        if !excludes_rel.set_targets(&[]) {
            success = false;
        }

        // Block membership expression attribute
        if membership_expr_attr.is_valid() && !membership_expr_attr.block() {
            success = false;
        }

        success
    }

    /// Test whether a given name contains the "collection:" prefix.
    ///
    /// Matches C++ `CanContainPropertyName(const TfToken &name)`.
    pub fn can_contain_property_name(name: &Token) -> bool {
        name.get_text().starts_with("collection:")
    }

    // ========================================================================
    // Static Helper Methods
    // ========================================================================

    /// Returns all the usd objects that satisfy the predicate in the collection.
    ///
    /// Matches C++ `ComputeIncludedObjects(const UsdCollectionMembershipQuery &query, const UsdStageWeakPtr &stage, const Usd_PrimFlagsPredicate &pred)`.
    pub fn compute_included_objects(
        query: &CollectionMembershipQuery,
        stage: &Stage,
        pred: PrimFlagsPredicate,
    ) -> HashSet<Object> {
        let mut result = HashSet::new();

        // Traverse all prims on the stage
        let root_prim = stage.get_pseudo_root();
        for prim in root_prim.descendants() {
            if pred.matches(prim.flags()) {
                let prim_path = prim.path();
                if query.is_path_included(prim_path, None) {
                    result.insert(Object::from_prim(prim));
                }
            }
        }

        result
    }

    /// Returns all the paths that satisfy the predicate in the collection.
    ///
    /// Matches C++ `ComputeIncludedPaths(const UsdCollectionMembershipQuery &query, const UsdStageWeakPtr &stage, const Usd_PrimFlagsPredicate &pred)`.
    pub fn compute_included_paths(
        query: &CollectionMembershipQuery,
        stage: &Stage,
        pred: PrimFlagsPredicate,
    ) -> std::collections::BTreeSet<Path> {
        let mut result = std::collections::BTreeSet::new();

        // Traverse all prims on the stage
        let root_prim = stage.get_pseudo_root();
        for prim in root_prim.descendants() {
            if pred.matches(prim.flags()) {
                let prim_path = prim.path();
                if query.is_path_included(prim_path, None) {
                    result.insert(prim_path.clone());
                }
            }
        }

        result
    }
}

impl PartialEq for CollectionAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for CollectionAPI {}
