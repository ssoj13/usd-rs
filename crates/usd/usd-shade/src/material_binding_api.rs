//! USD Shade MaterialBindingAPI - API schema for binding materials to prims.
//!
//! Port of pxr/usd/usdShade/materialBindingAPI.h and materialBindingAPI.cpp
//!
//! UsdShadeMaterialBindingAPI is an API schema that provides an interface
//! for binding materials to prims or collections of prims.

use super::material::Material;
use super::tokens::tokens;
use std::sync::Arc;
use usd_core::collection_api::CollectionAPI;
use usd_core::prim::Prim;
use usd_core::relationship::Relationship;
use usd_core::schema_base::APISchemaBase;
use usd_core::stage::Stage;
use usd_geom::imageable::Imageable;
use usd_geom::subset::Subset;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

// Re-export for convenience
pub use usd_core::collection_membership_query::CollectionMembershipQuery;

// ============================================================================
// DirectBinding
// ============================================================================

/// Represents a direct material binding.
#[derive(Debug, Clone)]
pub struct DirectBinding {
    /// The path to the material that is bound to.
    material_path: Path,
    /// The binding relationship.
    binding_rel: Relationship,
    /// The purpose of the material binding.
    material_purpose: Token,
    /// Store if there was a binding here.
    is_bound: bool,
}

impl DirectBinding {
    /// Default constructor initializes a DirectBinding object with invalid material and bindingRel data members.
    pub fn new() -> Self {
        Self {
            material_path: Path::empty(),
            binding_rel: Relationship::invalid(),
            material_purpose: tokens().all_purpose.clone(),
            is_bound: false,
        }
    }

    /// Constructs a DirectBinding from a binding relationship.
    ///
    /// Matches C++ `DirectBinding(const UsdRelationship &bindingRel)`.
    pub fn from_relationship(binding_rel: Relationship) -> Self {
        let material_purpose = MaterialBindingAPI::_get_material_purpose(&binding_rel);
        let mut material_path = Path::empty();
        let mut is_bound = false;

        let target_paths = binding_rel.get_forwarded_targets();

        if target_paths.len() == 1 && target_paths[0].is_prim_path() {
            material_path = target_paths[0].clone();
            is_bound = true;
        }

        Self {
            material_path,
            binding_rel,
            material_purpose,
            is_bound,
        }
    }

    /// Gets the material object that this direct binding binds to.
    ///
    /// Matches C++ `GetMaterial()`.
    pub fn get_material(&self) -> Material {
        if self.binding_rel.is_valid() && !self.material_path.is_empty() {
            if let Some(stage) = self.binding_rel.stage() {
                if let Some(prim) = stage.get_prim_at_path(&self.material_path) {
                    return Material::new(prim);
                }
            }
        }
        Material::invalid()
    }

    /// Returns the path to the material that is bound to by this direct binding.
    pub fn get_material_path(&self) -> &Path {
        &self.material_path
    }

    /// Returns the binding-relationship that represents this direct binding.
    pub fn get_binding_rel(&self) -> &Relationship {
        &self.binding_rel
    }

    /// Returns the purpose of the direct binding.
    pub fn get_material_purpose(&self) -> &Token {
        &self.material_purpose
    }

    /// Returns true if there is a material bound.
    pub fn is_bound(&self) -> bool {
        self.is_bound
    }
}

impl Default for DirectBinding {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CollectionBinding
// ============================================================================

/// Represents a collection-based material binding.
#[derive(Debug, Clone)]
pub struct CollectionBinding {
    /// The collection being bound.
    collection_path: Path,
    /// The material that is bound to.
    material_path: Path,
    /// The relationship that binds the collection to the material.
    binding_rel: Relationship,
}

impl CollectionBinding {
    /// Default constructor initializes a CollectionBinding object with invalid collection, material and bindingRel data members.
    pub fn new() -> Self {
        Self {
            collection_path: Path::empty(),
            material_path: Path::empty(),
            binding_rel: Relationship::invalid(),
        }
    }

    /// Constructs a CollectionBinding from a collection-binding relationship.
    ///
    /// Matches C++ `CollectionBinding(const UsdRelationship &collBindingRel)`.
    pub fn from_relationship(coll_binding_rel: Relationship) -> Self {
        let mut collection_path = Path::empty();
        let mut material_path = Path::empty();

        let target_paths = coll_binding_rel.get_forwarded_targets();

        // A collection binding relationship must have exactly two targets
        // One of them should target a property path (i.e. the collection path)
        // and the other must target a prim (the bound material).
        if target_paths.len() == 2 {
            let first_is_prim = target_paths[0].is_prim_path();
            let second_is_prim = target_paths[1].is_prim_path();

            if first_is_prim != second_is_prim {
                if first_is_prim {
                    material_path = target_paths[0].clone();
                    collection_path = target_paths[1].clone();
                } else {
                    material_path = target_paths[1].clone();
                    collection_path = target_paths[0].clone();
                }
            }
        }

        Self {
            collection_path,
            material_path,
            binding_rel: coll_binding_rel,
        }
    }

    /// Constructs and returns the material object that this collection-based binding binds to.
    ///
    /// Matches C++ `GetMaterial()`.
    pub fn get_material(&self) -> Material {
        if self.binding_rel.is_valid() && !self.material_path.is_empty() {
            if let Some(stage) = self.binding_rel.stage() {
                if let Some(prim) = stage.get_prim_at_path(&self.material_path) {
                    return Material::new(prim);
                }
            }
        }
        Material::invalid()
    }

    /// Constructs and returns the CollectionAPI object for the collection that is bound by this collection-binding.
    ///
    /// Matches C++ `GetCollection()`.
    pub fn get_collection(&self) -> CollectionAPI {
        if self.binding_rel.is_valid() && !self.collection_path.is_empty() {
            if let Some(stage) = self.binding_rel.stage() {
                return CollectionAPI::get_collection(&stage, &self.collection_path);
            }
        }
        CollectionAPI::invalid()
    }

    /// Checks if the bindingRel identifies a collection binding.
    ///
    /// Matches C++ `IsCollectionBindingRel(const UsdRelationship &bindingRel)`.
    pub fn is_collection_binding_rel(binding_rel: &Relationship) -> bool {
        let name = binding_rel.name();
        let name_str = name.as_str();
        name_str.starts_with("material:binding:collection")
    }

    /// Returns true if the CollectionBinding points to a non-empty material path and collection.
    pub fn is_valid(&self) -> bool {
        Self::is_collection_binding_rel(&self.binding_rel) && !self.get_material_path().is_empty()
    }

    /// Returns the path to the collection that is bound by this binding.
    pub fn get_collection_path(&self) -> &Path {
        &self.collection_path
    }

    /// Returns the path to the material that is bound to by this binding.
    pub fn get_material_path(&self) -> &Path {
        &self.material_path
    }

    /// Returns the binding-relationship that represents this collection-based binding.
    pub fn get_binding_rel(&self) -> &Relationship {
        &self.binding_rel
    }
}

impl Default for CollectionBinding {
    fn default() -> Self {
        Self::new()
    }
}

/// Type alias for a vector of CollectionBinding objects.
pub type CollectionBindingVector = Vec<CollectionBinding>;

// ============================================================================
// MaterialBindingAPI
// ============================================================================

/// UsdShadeMaterialBindingAPI is an API schema that provides an interface
/// for binding materials to prims or collections of prims.
///
/// This is a SingleApplyAPI schema.
#[derive(Debug, Clone)]
pub struct MaterialBindingAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl MaterialBindingAPI {
    /// Constructs a MaterialBindingAPI on the given prim.
    ///
    /// Matches C++ `UsdShadeMaterialBindingAPI(const UsdPrim& prim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs a MaterialBindingAPI from an APISchemaBase.
    ///
    /// Matches C++ `UsdShadeMaterialBindingAPI(const UsdSchemaBase& schemaObj)`.
    pub fn from_schema_base(schema: APISchemaBase) -> Self {
        Self { base: schema }
    }

    /// Creates an invalid MaterialBindingAPI.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Return a MaterialBindingAPI holding the prim at `path` on `stage`.
    ///
    /// Matches C++ `UsdShadeMaterialBindingAPI::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Arc<Stage>, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Returns true if this single-apply API schema can be applied to the given prim.
    ///
    /// Matches C++ `UsdShadeMaterialBindingAPI::CanApply(const UsdPrim &prim, std::string *whyNot)`.
    pub fn can_apply(prim: &Prim, why_not: &mut Option<String>) -> bool {
        if !prim.is_valid() {
            if let Some(reason) = why_not {
                *reason = "Invalid prim".to_string();
            }
            return false;
        }

        let schema_type_name = tokens().material_binding_api.clone();
        if !prim.can_apply_api(&schema_type_name) {
            if let Some(reason) = why_not {
                *reason = format!(
                    "Cannot apply MaterialBindingAPI to prim at path {}",
                    prim.path()
                );
            }
            return false;
        }

        true
    }

    /// Applies this single-apply API schema to the given prim.
    ///
    /// Matches C++ `UsdShadeMaterialBindingAPI::Apply(const UsdPrim &prim)`.
    pub fn apply(prim: &Prim) -> Self {
        if !prim.is_valid() {
            return Self::invalid();
        }

        let schema_type_name = tokens().material_binding_api.clone();
        if prim.apply_api(&schema_type_name) {
            Self::new(prim.clone())
        } else {
            Self::invalid()
        }
    }

    /// Returns true if this MaterialBindingAPI is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Returns the wrapped prim.
    ///
    /// Matches C++ `GetPrim()`.
    pub fn get_prim(&self) -> &Prim {
        self.base.get_prim()
    }

    /// Returns the path to this prim.
    ///
    /// Matches C++ `GetPath()`.
    pub fn path(&self) -> &Path {
        self.base.path()
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(_include_inherited: bool) -> Vec<Token> {
        // MaterialBindingAPI doesn't define any schema attributes
        Vec::new()
    }

    // ========================================================================
    // Helper functions for constructing binding relationship names
    // ========================================================================

    /// Returns the name of the direct binding relationship for the given material purpose.
    fn _get_direct_binding_rel_name(material_purpose: &Token) -> Token {
        if *material_purpose == tokens().all_purpose {
            tokens().material_binding.clone()
        } else if *material_purpose == tokens().preview {
            Token::new("material:binding:preview")
        } else if *material_purpose == tokens().full {
            Token::new("material:binding:full")
        } else {
            // Join "material:binding" with the purpose
            Token::new(&format!("material:binding:{}", material_purpose.as_str()))
        }
    }

    /// Returns the name of the collection binding relationship for the given binding name and material purpose.
    fn _get_collection_binding_rel_name(binding_name: &Token, material_purpose: &Token) -> Token {
        if *material_purpose == tokens().all_purpose {
            Token::new(&format!(
                "material:binding:collection:{}",
                binding_name.as_str()
            ))
        } else if *material_purpose == tokens().preview {
            Token::new(&format!(
                "material:binding:collection:preview:{}",
                binding_name.as_str()
            ))
        } else if *material_purpose == tokens().full {
            Token::new(&format!(
                "material:binding:collection:full:{}",
                binding_name.as_str()
            ))
        } else {
            Token::new(&format!(
                "material:binding:collection:{}:{}",
                material_purpose.as_str(),
                binding_name.as_str()
            ))
        }
    }

    /// Returns the material purpose associated with the given binding relationship.
    fn _get_material_purpose(binding_rel: &Relationship) -> Token {
        let name = binding_rel.name();
        let name_str = name.as_str();
        let parts: Vec<&str> = name_str.split(':').collect();

        match parts.len() {
            // material:binding:collection:purpose:bindingName
            5 => Token::new(parts[3]),
            // material:binding:collection:bindingName (no explicit purpose)
            4 => tokens().all_purpose.clone(),
            // material:binding:purpose
            3 => Token::new(parts[2]),
            // material:binding (all-purpose) or any other count
            _ => tokens().all_purpose.clone(),
        }
    }

    // ========================================================================
    // Schema property and associated data retrieval API
    // ========================================================================

    /// Returns the direct material-binding relationship on this prim for the given material purpose.
    ///
    /// Matches C++ `GetDirectBindingRel(const TfToken &materialPurpose)`.
    pub fn get_direct_binding_rel(&self, material_purpose: &Token) -> Relationship {
        let rel_name = Self::_get_direct_binding_rel_name(material_purpose);
        self.get_prim()
            .get_relationship(rel_name.as_str())
            .unwrap_or_else(Relationship::invalid)
    }

    /// Returns the collection-based material-binding relationship with the given bindingName and materialPurpose on this prim.
    ///
    /// Matches C++ `GetCollectionBindingRel(const TfToken &bindingName, const TfToken &materialPurpose)`.
    pub fn get_collection_binding_rel(
        &self,
        binding_name: &Token,
        material_purpose: &Token,
    ) -> Relationship {
        let rel_name = Self::_get_collection_binding_rel_name(binding_name, material_purpose);
        self.get_prim()
            .get_relationship(rel_name.as_str())
            .unwrap_or_else(Relationship::invalid)
    }

    /// Returns the list of collection-based material binding relationships on this prim for the given material purpose.
    ///
    /// Matches C++ `GetCollectionBindingRels(const TfToken &materialPurpose)`.
    pub fn get_collection_binding_rels(&self, material_purpose: &Token) -> Vec<Relationship> {
        let prefix = Self::_get_collection_binding_rel_name(&Token::new(""), material_purpose);

        let properties = self
            .get_prim()
            .get_authored_properties_in_namespace(&prefix);
        let mut result = Vec::new();

        for prop in properties {
            if let Some(rel) = prop.as_relationship() {
                if Self::_get_material_purpose(&rel) == *material_purpose {
                    result.push(rel);
                }
            }
        }

        result
    }

    /// Computes and returns the direct binding for the given material purpose on this prim.
    ///
    /// Matches C++ `GetDirectBinding(const TfToken &materialPurpose)`.
    pub fn get_direct_binding(&self, material_purpose: &Token) -> DirectBinding {
        let direct_binding_rel = self.get_direct_binding_rel(material_purpose);
        DirectBinding::from_relationship(direct_binding_rel)
    }

    /// Returns all the collection-based bindings on this prim for the given material purpose.
    ///
    /// Matches C++ `GetCollectionBindings(const TfToken &materialPurpose)`.
    pub fn get_collection_bindings(&self, material_purpose: &Token) -> CollectionBindingVector {
        let collection_binding_rels = self.get_collection_binding_rels(material_purpose);
        let mut result = Vec::new();
        result.reserve(collection_binding_rels.len());

        for coll_binding_rel in collection_binding_rels {
            let binding = CollectionBinding::from_relationship(coll_binding_rel);
            if binding.is_valid() {
                result.push(binding);
            }
        }

        result
    }

    /// Helper method for getting collection bindings when the set of all collection binding relationship names is known.
    fn _get_collection_bindings(
        &self,
        coll_binding_property_names: &[Token],
    ) -> CollectionBindingVector {
        let mut result = Vec::new();
        result.reserve(coll_binding_property_names.len());

        for coll_binding_prop_name in coll_binding_property_names {
            if let Some(rel) = self
                .get_prim()
                .get_relationship(coll_binding_prop_name.as_str())
            {
                let binding = CollectionBinding::from_relationship(rel);
                if binding.is_valid() {
                    result.push(binding);
                }
            }
        }

        result
    }

    /// Resolves the 'bindMaterialAs' token-valued metadata on the given binding relationship and returns it.
    ///
    /// Matches C++ `GetMaterialBindingStrength(const UsdRelationship &bindingRel)`.
    /// Uses composed metadata via `Property::get_metadata()`, matching C++ `bindingRel.GetMetadata(...)`.
    pub fn get_material_binding_strength(binding_rel: &Relationship) -> Token {
        // Use composed metadata resolution through the property interface,
        // matching C++: bindingRel.GetMetadata(UsdShadeTokens->bindMaterialAs, &bindingStrength)
        if let Some(value) = binding_rel
            .as_property()
            .get_metadata(&tokens().bind_material_as)
        {
            if let Some(token_value) = value.downcast_clone::<Token>() {
                if !token_value.is_empty() {
                    return token_value;
                }
            }
        }
        // Default binding strength is weakerThanDescendants
        tokens().weaker_than_descendants.clone()
    }

    /// Sets the 'bindMaterialAs' token-valued metadata on the given binding relationship.
    ///
    /// Matches C++ `SetMaterialBindingStrength(const UsdRelationship &bindingRel, const TfToken &bindingStrength)`.
    /// Per C++ materialBindingAPI.cpp:377-395:
    /// - If `fallbackStrength` and metadata exists with non-default value: clear metadata
    ///   so the fallback (weakerThanDescendants) takes effect naturally.
    /// - Otherwise: write the provided strength value.
    pub fn set_material_binding_strength(
        binding_rel: &Relationship,
        binding_strength: &Token,
    ) -> bool {
        if *binding_strength == tokens().fallback_strength {
            // C++: if existing value is non-empty and not weakerThanDescendants, set it to
            // weakerThanDescendants (NOT just clear). This matches:
            //   bindingRel.GetMetadata(bindMaterialAs, &existing);
            //   if (!existing.IsEmpty() && existing != weakerThanDescendants)
            //       return bindingRel.SetMetadata(bindMaterialAs, weakerThanDescendants);
            let existing_strength = Self::get_material_binding_strength(binding_rel);
            if !existing_strength.is_empty()
                && existing_strength != tokens().weaker_than_descendants
            {
                return binding_rel.as_property().set_metadata(
                    &tokens().bind_material_as,
                    Value::from(tokens().weaker_than_descendants.clone()),
                );
            }
            return true;
        }
        // Set metadata via composed property interface matching C++ `bindingRel.SetMetadata(...)`
        binding_rel.as_property().set_metadata(
            &tokens().bind_material_as,
            Value::from(binding_strength.clone()),
        )
    }

    // ========================================================================
    // Binding authoring and clearing API
    // ========================================================================

    /// Creates a direct binding relationship for the given material purpose.
    fn _create_direct_binding_rel(&self, material_purpose: &Token) -> Option<Relationship> {
        let rel_name = Self::_get_direct_binding_rel_name(material_purpose);
        self.get_prim()
            .create_relationship(rel_name.as_str(), false)
    }

    /// Creates a collection binding relationship for the given binding name and material purpose.
    fn _create_collection_binding_rel(
        &self,
        binding_name: &Token,
        material_purpose: &Token,
    ) -> Option<Relationship> {
        let rel_name = Self::_get_collection_binding_rel_name(binding_name, material_purpose);
        self.get_prim()
            .create_relationship(rel_name.as_str(), false)
    }

    /// Authors a direct binding to the given material on this prim.
    ///
    /// Matches C++ `Bind(const UsdShadeMaterial &material, const TfToken &bindingStrength, const TfToken &materialPurpose)`.
    pub fn bind(
        &self,
        material: &Material,
        binding_strength: &Token,
        material_purpose: &Token,
    ) -> bool {
        if let Some(binding_rel) = self._create_direct_binding_rel(material_purpose) {
            Self::set_material_binding_strength(&binding_rel, binding_strength);
            return binding_rel.set_targets(&[material.path().clone()]);
        }
        false
    }

    /// Authors a collection-based binding, which binds the given material to the given collection on this prim.
    ///
    /// Matches C++ `Bind(const UsdCollectionAPI &collection, const UsdShadeMaterial &material, const TfToken &bindingName, const TfToken &bindingStrength, const TfToken &materialPurpose)`.
    pub fn bind_collection(
        &self,
        collection: &CollectionAPI,
        material: &Material,
        binding_name: &Token,
        binding_strength: &Token,
        material_purpose: &Token,
    ) -> bool {
        // BindingName should not contain any namespaces.
        let mut fixed_binding_name = binding_name.clone();

        if binding_name.as_str().is_empty() {
            // Use the collection-name when bindingName is empty
            if let Some(name) = collection.name() {
                // Strip namespace from collection name
                let name_str = name.as_str();
                if let Some(last_colon) = name_str.rfind(':') {
                    fixed_binding_name = Token::new(&name_str[last_colon + 1..]);
                } else {
                    fixed_binding_name = name.clone();
                }
            }
        } else if binding_name.as_str().contains(':') {
            eprintln!(
                "Invalid bindingName '{}', as it contains namespaces. Not binding collection <{}> to material <{}>.",
                binding_name.as_str(),
                collection.get_collection_path(),
                material.path()
            );
            return false;
        }

        if let Some(coll_binding_rel) =
            self._create_collection_binding_rel(&fixed_binding_name, material_purpose)
        {
            Self::set_material_binding_strength(&coll_binding_rel, binding_strength);
            return coll_binding_rel.set_targets(&[
                collection.get_collection_path().clone(),
                material.path().clone(),
            ]);
        }

        false
    }

    /// Unbinds the direct binding for the given material purpose on this prim.
    ///
    /// Matches C++ `UnbindDirectBinding(const TfToken &materialPurpose)`.
    pub fn unbind_direct_binding(&self, material_purpose: &Token) -> bool {
        let rel_name = Self::_get_direct_binding_rel_name(material_purpose);
        if let Some(binding_rel) = self
            .get_prim()
            .create_relationship(rel_name.as_str(), false)
        {
            return binding_rel.set_targets(&[]);
        }
        false
    }

    /// Unbinds the collection-based binding with the given bindingName, for the given materialPurpose on this prim.
    ///
    /// Matches C++ `UnbindCollectionBinding(const TfToken &bindingName, const TfToken &materialPurpose)`.
    pub fn unbind_collection_binding(
        &self,
        binding_name: &Token,
        material_purpose: &Token,
    ) -> bool {
        let rel_name = Self::_get_collection_binding_rel_name(binding_name, material_purpose);
        if let Some(coll_binding_rel) = self
            .get_prim()
            .create_relationship(rel_name.as_str(), false)
        {
            return coll_binding_rel.set_targets(&[]);
        }
        false
    }

    /// Unbinds all direct and collection-based bindings on this prim.
    ///
    /// Matches C++ `UnbindAllBindings()`.
    pub fn unbind_all_bindings(&self) -> bool {
        let all_binding_properties = self
            .get_prim()
            .get_properties_in_namespace(&tokens().material_binding);

        // The relationship named material:binding (Which is the default/all-purpose
        // direct binding relationship) isn't included in the result of GetPropertiesInNamespace.
        // Add it here if it exists.
        let mut all_properties = all_binding_properties;
        if let Some(all_purpose_direct_binding_rel) = self
            .get_prim()
            .get_relationship(tokens().material_binding.as_str())
        {
            all_properties.push(all_purpose_direct_binding_rel.into());
        }

        let mut success = true;
        for prop in all_properties {
            if let Some(binding_rel) = prop.as_relationship() {
                success = binding_rel.set_targets(&[]) && success;
            }
        }

        success
    }

    /// Removes the specified prim from the collection targeted by the binding relationship corresponding to given bindingName and materialPurpose.
    ///
    /// Matches C++ `RemovePrimFromBindingCollection(const UsdPrim &prim, const TfToken &bindingName, const TfToken &materialPurpose)`.
    pub fn remove_prim_from_binding_collection(
        &self,
        prim: &Prim,
        binding_name: &Token,
        material_purpose: &Token,
    ) -> bool {
        let coll_binding_rel = self.get_collection_binding_rel(binding_name, material_purpose);
        if coll_binding_rel.is_valid() {
            let coll_binding = CollectionBinding::from_relationship(coll_binding_rel);
            let collection = coll_binding.get_collection();
            if collection.is_valid() {
                return collection.exclude_path(prim.path());
            }
        }
        true
    }

    /// Adds the specified prim to the collection targeted by the binding relationship corresponding to given bindingName and materialPurpose.
    ///
    /// Matches C++ `AddPrimToBindingCollection(const UsdPrim &prim, const TfToken &bindingName, const TfToken &materialPurpose)`.
    pub fn add_prim_to_binding_collection(
        &self,
        prim: &Prim,
        binding_name: &Token,
        material_purpose: &Token,
    ) -> bool {
        let coll_binding_rel = self.get_collection_binding_rel(binding_name, material_purpose);
        if coll_binding_rel.is_valid() {
            let coll_binding = CollectionBinding::from_relationship(coll_binding_rel);
            let collection = coll_binding.get_collection();
            if collection.is_valid() {
                return collection.include_path(prim.path());
            }
        }
        true
    }

    // ========================================================================
    // Bound Material Resolution
    // ========================================================================

    /// Returns a vector of the possible values for the 'material purpose'.
    ///
    /// Matches C++ `GetMaterialPurposes()`.
    pub fn get_material_purposes() -> Vec<Token> {
        vec![
            tokens().all_purpose.clone(),
            tokens().preview.clone(),
            tokens().full.clone(),
        ]
    }

    /// Returns the path of the resolved target identified by bindingRel.
    ///
    /// Matches C++ `GetResolvedTargetPathFromBindingRel(const UsdRelationship &bindingRel)`.
    pub fn get_resolved_target_path_from_binding_rel(binding_rel: &Relationship) -> Path {
        if !binding_rel.is_valid() {
            return Path::empty();
        }

        let target_paths = binding_rel.get_forwarded_targets();
        if CollectionBinding::is_collection_binding_rel(binding_rel) {
            // For collection bindings, return the material path (second target)
            if target_paths.len() >= 2 {
                return target_paths[1].clone();
            }
        } else {
            // For direct bindings, return the first target
            if !target_paths.is_empty() {
                return target_paths[0].clone();
            }
        }
        Path::empty()
    }

    /// Computes the resolved bound material for this prim, for the given material purpose.
    ///
    /// Matches C++ `ComputeBoundMaterial(const TfToken &materialPurpose, UsdRelationship *bindingRel, bool supportLegacyBindings)`.
    pub fn compute_bound_material(
        &self,
        material_purpose: &Token,
        binding_rel: &mut Option<Relationship>,
        _support_legacy_bindings: bool,
    ) -> Material {
        if !self.get_prim().is_valid() {
            return Material::invalid();
        }

        // Build purpose list: specific purpose first, then allPurpose fallback
        let mut material_purposes = vec![material_purpose.clone()];
        if *material_purpose != tokens().all_purpose {
            material_purposes.push(tokens().all_purpose.clone());
        }

        for purpose in &material_purposes {
            let mut bound_material = Material::invalid();
            let mut has_valid_target = false;
            let mut winning_binding_rel = Relationship::invalid();

            // Walk ancestor chain from this prim up to (but not including) pseudo root
            let mut p = self.get_prim().clone();
            while !p.is_pseudo_root() {
                let binding_api = Self::new(p.clone());

                // Check direct binding at this ancestor
                let direct_binding = binding_api.get_direct_binding(purpose);
                if direct_binding.is_bound() && direct_binding.get_material_purpose() == purpose {
                    let direct_rel = direct_binding.get_binding_rel().clone();
                    // Accept if no binding yet, or if this one is strongerThanDescendants
                    if !has_valid_target
                        || Self::get_material_binding_strength(&direct_rel).as_str()
                            == tokens().stronger_than_descendants.as_str()
                    {
                        has_valid_target = !direct_binding.get_material_path().is_empty();
                        bound_material = direct_binding.get_material();
                        winning_binding_rel = direct_rel;
                    }
                }

                // Check collection bindings at this ancestor.
                // Only accept a collection binding if the target prim is actually
                // a member of that collection (CollectionMembershipQuery evaluation).
                let target_path = self.get_prim().path().clone();
                let coll_bindings = binding_api.get_collection_bindings(purpose);
                for coll_binding in &coll_bindings {
                    let collection = coll_binding.get_collection();
                    if !collection.is_valid() {
                        continue;
                    }
                    // Evaluate membership; skip this binding if prim is not a member.
                    if !Self::_is_prim_in_collection(&collection, &target_path) {
                        continue;
                    }
                    let coll_rel = coll_binding.get_binding_rel().clone();
                    // Collection on the prim itself is stronger than its direct binding,
                    // or accept if no binding yet, or if strongerThanDescendants
                    let coll_on_same_prim = winning_binding_rel.prim_path() == p.get_path().clone();
                    if !has_valid_target
                        || (has_valid_target && coll_on_same_prim)
                        || Self::get_material_binding_strength(&coll_rel).as_str()
                            == tokens().stronger_than_descendants.as_str()
                    {
                        has_valid_target = !coll_binding.get_material_path().is_empty();
                        bound_material = coll_binding.get_material();
                        winning_binding_rel = coll_rel;
                        // First matching collection binding wins at this level
                        break;
                    }
                }

                p = p.parent();
            }

            // First purpose with a valid binding wins
            if has_valid_target {
                if let Some(rel) = binding_rel {
                    *rel = winning_binding_rel;
                }
                // C++ materialBindingAPI.cpp:834-836: verify the resolved prim IsA<UsdShadeMaterial>().
                // Material::is_valid() already uses prim.is_a("Material") for schema hierarchy check.
                if bound_material.is_valid() {
                    return bound_material;
                }
                return Material::invalid();
            }
        }

        Material::invalid()
    }

    /// Cached overload of `compute_bound_material`.
    ///
    /// C++ materialBindingAPI.cpp:701-810: uses `BindingsCache` to avoid
    /// recomputing bindings for shared ancestors when resolving materials
    /// for many prims. The cache is populated on first access per prim path
    /// and reused for subsequent lookups.
    /// Cached overload of `compute_bound_material`.
    ///
    /// C++ materialBindingAPI.cpp:701-810: uses `BindingsCache` to avoid
    /// recomputing bindings for shared ancestors. The cache stores
    /// `BindingsAtPrim` per prim path, populated on first access.
    pub fn compute_bound_material_cached(
        &self,
        bindings_cache: &BindingsCache,
        _collection_query_cache: &CollectionQueryCache,
        material_purpose: &Token,
        binding_rel: &mut Relationship,
        _support_legacy_bindings: bool,
    ) -> Material {
        if !self.get_prim().is_valid() {
            return Material::invalid();
        }

        let all_purpose = tokens().all_purpose.clone();
        let purposes = if *material_purpose != all_purpose {
            vec![material_purpose.clone(), all_purpose.clone()]
        } else {
            vec![all_purpose.clone()]
        };

        let target_path = self.get_prim().path().clone();

        for purpose in &purposes {
            let mut bound_material = Material::invalid();
            let mut has_valid_target = false;
            let mut winning_binding_rel = Relationship::invalid();

            let mut current = self.get_prim().clone();
            while current.is_valid() && !current.is_pseudo_root() {
                let bindings = bindings_cache.get_or_build(&current, material_purpose);

                // Direct binding check
                if bindings.direct.is_bound() && bindings.direct.get_material_purpose() == *purpose
                {
                    let direct_rel = bindings.direct.get_binding_rel().clone();
                    if !has_valid_target
                        || Self::get_material_binding_strength(&direct_rel)
                            == tokens().stronger_than_descendants
                    {
                        has_valid_target = !bindings.direct.get_material_path().is_empty();
                        bound_material = bindings.direct.get_material();
                        winning_binding_rel = direct_rel;
                    }
                }

                // Collection bindings — select vector by purpose
                let coll_bindings = if *purpose == all_purpose {
                    &bindings.all_purpose_coll_bindings
                } else {
                    &bindings.restricted_purpose_coll_bindings
                };

                for coll_binding in coll_bindings {
                    if !coll_binding.is_valid() {
                        continue;
                    }
                    let collection = coll_binding.get_collection();
                    if !collection.is_valid() {
                        continue;
                    }
                    if !Self::_is_prim_in_collection(&collection, &target_path) {
                        continue;
                    }
                    let coll_rel = coll_binding.get_binding_rel().clone();
                    if !has_valid_target
                        || Self::get_material_binding_strength(&coll_rel)
                            == tokens().stronger_than_descendants
                    {
                        has_valid_target = !coll_binding.get_material_path().is_empty();
                        bound_material = coll_binding.get_material();
                        winning_binding_rel = coll_rel;
                        break;
                    }
                }

                current = current.parent();
            }

            if bound_material.is_valid() {
                *binding_rel = winning_binding_rel;
                return bound_material;
            }
        }

        Material::invalid()
    }

    /// Static API for efficiently computing the resolved material bindings for a vector of UsdPrims.
    ///
    /// Matches C++ `ComputeBoundMaterials(const std::vector<UsdPrim> &prims, const TfToken &materialPurpose, std::vector<UsdRelationship> *bindingRels, bool supportLegacyBindings)`.
    /// C++ materialBindingAPI.cpp:860-896: creates shared BindingsCache and
    /// CollectionQueryCache, then calls the cached overload for each prim.
    /// Shared caches amortize repeated ancestor lookups across the batch.
    pub fn compute_bound_materials(
        prims: &[Prim],
        material_purpose: &Token,
        binding_rels: &mut Option<&mut Vec<Relationship>>,
        support_legacy_bindings: bool,
    ) -> Vec<Material> {
        let mut materials = Vec::with_capacity(prims.len());

        if let Some(rels) = binding_rels {
            rels.clear();
            rels.resize(prims.len(), Relationship::invalid());
        }

        // Shared caches for the batch — ancestors resolved once and reused
        let bindings_cache = BindingsCache::new();
        let collection_query_cache = CollectionQueryCache::new();

        for (i, prim) in prims.iter().enumerate() {
            let binding_api = Self::new(prim.clone());
            let mut binding_rel = Relationship::invalid();
            let material = binding_api.compute_bound_material_cached(
                &bindings_cache,
                &collection_query_cache,
                material_purpose,
                &mut binding_rel,
                support_legacy_bindings,
            );
            materials.push(material);

            if let Some(rels) = binding_rels {
                rels[i] = binding_rel;
            }
        }

        materials
    }

    // ========================================================================
    // Binding materials to subsets
    // ========================================================================

    /// Creates a GeomSubset named subsetName with element type, elementType and familyName "materialBind" below this prim.
    ///
    /// Matches C++ `CreateMaterialBindSubset(const TfToken &subsetName, const VtIntArray &indices, const TfToken &elementType)`.
    pub fn create_material_bind_subset(
        &self,
        subset_name: &Token,
        indices: &[i32],
        element_type: &Token,
    ) -> Subset {
        let imageable = Imageable::new(self.get_prim().clone());
        let result = Subset::create_geom_subset(
            &imageable,
            subset_name,
            element_type,
            indices,
            &tokens().material_bind,
            &usd_geom::tokens::usd_geom_tokens().non_overlapping,
        );

        // Check family type and set to nonOverlapping if unset or unrestricted
        let family_type = Subset::get_family_type(&imageable, &tokens().material_bind);
        if family_type.as_str().is_empty()
            || family_type == usd_geom::tokens::usd_geom_tokens().unrestricted
        {
            let _ = self.set_material_bind_subsets_family_type(
                &usd_geom::tokens::usd_geom_tokens().non_overlapping,
            );
        }

        result
    }

    /// Returns all the existing GeomSubsets with familyName=UsdShadeTokens->materialBind below this prim.
    ///
    /// Matches C++ `GetMaterialBindSubsets()`.
    pub fn get_material_bind_subsets(&self) -> Vec<Subset> {
        let imageable = Imageable::new(self.get_prim().clone());
        Subset::get_geom_subsets(&imageable, &Token::new(""), &tokens().material_bind)
    }

    /// Author the familyType of the "materialBind" family of GeomSubsets on this prim.
    ///
    /// Matches C++ `SetMaterialBindSubsetsFamilyType(const TfToken &familyType)`.
    pub fn set_material_bind_subsets_family_type(&self, family_type: &Token) -> bool {
        if *family_type == usd_geom::tokens::usd_geom_tokens().unrestricted {
            eprintln!(
                "Attempted to set invalid familyType 'unrestricted' for the \"materialBind\" family of subsets on <{}>.",
                self.path()
            );
            return false;
        }
        let imageable = Imageable::new(self.get_prim().clone());
        Subset::set_family_type(&imageable, &tokens().material_bind, family_type)
    }

    /// Returns the familyType of the family of "materialBind" GeomSubsets on this prim.
    ///
    /// Matches C++ `GetMaterialBindSubsetsFamilyType()`.
    pub fn get_material_bind_subsets_family_type(&self) -> Token {
        let imageable = Imageable::new(self.get_prim().clone());
        Subset::get_family_type(&imageable, &tokens().material_bind)
    }

    /// Returns all binding information at a specific prim.
    ///
    /// Collects direct and collection-based bindings for all material
    /// purposes (allPurpose, preview, full) on the given prim.
    ///
    /// Matches C++ convenience of gathering all bindings at a prim.
    pub fn bindings_at_prim(prim: &Prim) -> Vec<(Token, DirectBinding, CollectionBindingVector)> {
        let binding_api = Self::new(prim.clone());
        let mut result = Vec::new();

        for purpose in Self::get_material_purposes() {
            let direct = binding_api.get_direct_binding(&purpose);
            let collections = binding_api.get_collection_bindings(&purpose);
            result.push((purpose, direct, collections));
        }

        result
    }

    /// Test whether a given name contains the "material:binding:" prefix.
    ///
    /// Matches C++ `CanContainPropertyName(const TfToken &name)`.
    pub fn can_contain_property_name(name: &Token) -> bool {
        name.as_str()
            .starts_with(tokens().material_binding.as_str())
    }

    /// Returns true if `target_path` is a member of `collection`.
    ///
    /// Evaluates CollectionMembershipQuery, matching C++ collection binding logic.
    fn _is_prim_in_collection(collection: &CollectionAPI, target_path: &Path) -> bool {
        if !collection.is_valid() {
            return false;
        }
        let query = collection.compute_membership_query();
        query.is_path_included(target_path, None)
    }

    /// Returns true if `target_path` is a member of `collection`, using `cache`.
    ///
    /// Matches C++ `ComputeBoundMaterial` overload that takes CollectionQueryCache.
    pub fn _is_prim_in_collection_cached(
        collection: &CollectionAPI,
        target_path: &Path,
        cache: &CollectionQueryCache,
    ) -> bool {
        if !collection.is_valid() {
            return false;
        }
        let Some(stage) = collection.prim().stage() else {
            return false;
        };
        let collection_path = collection.get_collection_path();
        let Some(query) = cache.get_or_compute(&stage, &collection_path) else {
            return false;
        };
        query.is_path_included(target_path, None)
    }
}

impl PartialEq for MaterialBindingAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for MaterialBindingAPI {}

impl std::hash::Hash for MaterialBindingAPI {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.base.hash(state);
    }
}

// ============================================================================
// Caching infrastructure (matches C++ BindingsCache + CollectionQueryCache)
// ============================================================================

use std::collections::HashMap;
use std::sync::RwLock;

/// Holds all binding information for a single prim at a specific purpose.
///
/// Matches C++ `UsdShadeMaterialBindingAPI::BindingsAtPrim` struct.
#[derive(Debug, Clone)]
pub struct BindingsAtPrim {
    /// Direct binding for the specific purpose (or allPurpose fallback).
    pub direct: DirectBinding,
    /// Collection bindings for the specific purpose.
    pub restricted_purpose_coll_bindings: CollectionBindingVector,
    /// Collection bindings for allPurpose (fallback).
    pub all_purpose_coll_bindings: CollectionBindingVector,
}

impl BindingsAtPrim {
    /// Build bindings for `prim` at the given `purpose`.
    ///
    /// C++ materialBindingAPI.cpp:607-658: does a two-pass lookup:
    /// 1. Get direct binding for specific purpose
    /// 2. If no binding (or empty material path), fallback to allPurpose
    /// Also collects collection bindings for both specific and allPurpose.
    pub fn build(prim: &Prim, purpose: &Token) -> Self {
        let api = MaterialBindingAPI::new(prim.clone());
        let all_purpose = tokens().all_purpose.clone();

        // Direct binding: try specific purpose first, fallback to allPurpose
        let specific_direct = api.get_direct_binding(purpose);
        let direct = if specific_direct.is_bound() {
            specific_direct
        } else if *purpose != all_purpose {
            api.get_direct_binding(&all_purpose)
        } else {
            specific_direct
        };

        // C++ maintains separate collection binding vectors per purpose
        let restricted = api.get_collection_bindings(purpose);
        let all_coll = if *purpose != all_purpose {
            api.get_collection_bindings(&all_purpose)
        } else {
            Vec::new()
        };

        Self {
            direct,
            restricted_purpose_coll_bindings: restricted,
            all_purpose_coll_bindings: all_coll,
        }
    }
}

/// Thread-safe cache mapping prim paths to their BindingsAtPrim.
///
/// Matches C++ `tbb::concurrent_unordered_map<SdfPath, unique_ptr<BindingsAtPrim>, ...>`.
#[derive(Debug, Default)]
pub struct BindingsCache {
    inner: RwLock<HashMap<Path, BindingsAtPrim>>,
}

impl BindingsCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return cached bindings for `prim`+`purpose`, computing and storing them on miss.
    pub fn get_or_build(&self, prim: &Prim, purpose: &Token) -> BindingsAtPrim {
        let key = prim.path().clone();
        {
            let guard = self.inner.read().unwrap();
            if let Some(entry) = guard.get(&key) {
                return entry.clone();
            }
        }
        let entry = BindingsAtPrim::build(prim, purpose);
        let mut guard = self.inner.write().unwrap();
        guard.entry(key).or_insert_with(|| entry.clone());
        entry
    }
}

/// Thread-safe cache mapping collection paths to their CollectionMembershipQuery.
///
/// Matches C++ `tbb::concurrent_unordered_map<SdfPath, optional<CollectionMembershipQuery>, ...>`.
#[derive(Debug, Default)]
pub struct CollectionQueryCache {
    inner: RwLock<HashMap<Path, Option<CollectionMembershipQuery>>>,
}

impl CollectionQueryCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return cached query for `collection_path`, computing it on miss.
    ///
    /// Returns `None` if the collection cannot be found on the stage.
    pub fn get_or_compute(
        &self,
        stage: &Arc<Stage>,
        collection_path: &Path,
    ) -> Option<CollectionMembershipQuery> {
        let key = collection_path.clone();
        {
            let guard = self.inner.read().unwrap();
            if let Some(entry) = guard.get(&key) {
                return entry.clone();
            }
        }
        let query = CollectionAPI::get_collection(stage, collection_path)
            .is_valid()
            .then(|| {
                CollectionAPI::get_collection(stage, collection_path).compute_membership_query()
            });
        let mut guard = self.inner.write().unwrap();
        guard.entry(key).or_insert_with(|| query.clone());
        query
    }
}
