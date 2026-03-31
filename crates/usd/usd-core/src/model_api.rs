//! UsdModelAPI - model API schema.
//!
//! Port of pxr/usd/usd/modelAPI.h/cpp
//!
//! API schema for model qualities (kind, assetInfo).

use crate::schema_base::APISchemaBase;
use crate::{Prim, Stage};
use std::collections::HashMap;
use usd_kind::{is_a, kind_tokens};
use usd_sdf::AssetPath;
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{Array, Value};

// ============================================================================
// KindValidation
// ============================================================================

/// Option for validating queries to a prim's kind metadata.
///
/// Matches C++ `UsdModelAPI::KindValidation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindValidation {
    /// No additional validation.
    None,
    /// Ensure model hierarchy rules are followed.
    ModelHierarchy,
}

// ============================================================================
// ModelAPI
// ============================================================================

/// API schema for model qualities (kind, assetInfo).
///
/// Matches C++ `UsdModelAPI`.
///
/// This is a NonAppliedAPI schema - it doesn't need to be applied to prims.
#[derive(Debug, Clone)]
pub struct ModelAPI {
    /// Base API schema.
    base: APISchemaBase,
}

impl ModelAPI {
    /// Constructs a ModelAPI from a prim.
    ///
    /// Matches C++ `UsdModelAPI(UsdPrim)`.
    pub fn new(prim: Prim) -> Self {
        Self {
            base: APISchemaBase::new(prim),
        }
    }

    /// Constructs an invalid ModelAPI.
    ///
    /// Matches C++ default constructor.
    pub fn invalid() -> Self {
        Self {
            base: APISchemaBase::invalid(),
        }
    }

    /// Returns true if this schema is valid.
    pub fn is_valid(&self) -> bool {
        self.base.is_valid()
    }

    /// Returns the wrapped prim.
    pub fn prim(&self) -> &Prim {
        self.base.prim()
    }

    /// Returns the schema type name.
    pub fn schema_type_name() -> Token {
        Token::new("ModelAPI")
    }

    /// Gets a ModelAPI from a stage and path.
    ///
    /// Matches C++ `UsdModelAPI::Get(const UsdStagePtr &stage, const SdfPath &path)`.
    pub fn get(stage: &Stage, path: &Path) -> Self {
        if let Some(prim) = stage.get_prim_at_path(path) {
            Self::new(prim)
        } else {
            Self::invalid()
        }
    }

    /// Gets a ModelAPI from a prim.
    ///
    /// Matches C++ `UsdModelAPI(UsdPrim)`.
    pub fn get_from_prim(prim: &Prim) -> Self {
        Self::new(prim.clone())
    }

    /// Return a vector of names of all pre-declared attributes for this schema
    /// class and all its ancestor classes.
    ///
    /// Matches C++ `GetSchemaAttributeNames(bool includeInherited)`.
    pub fn get_schema_attribute_names(include_inherited: bool) -> Vec<Token> {
        let local_names = Vec::new(); // ModelAPI doesn't add attributes

        if include_inherited {
            // APISchemaBase doesn't add attributes either, so just return local
            local_names
        } else {
            local_names
        }
    }

    // ========================================================================
    // Kind and Model-ness
    // ========================================================================

    /// Retrieve the authored kind for this prim.
    ///
    /// Matches C++ `GetKind(TfToken* kind)`.
    /// Returns true if there was an authored kind that was successfully read.
    pub fn get_kind(&self) -> Option<Token> {
        self.prim().get_kind()
    }

    /// Author a kind for this prim, at the current UsdEditTarget.
    ///
    /// Matches C++ `SetKind(const TfToken& kind)`.
    /// Returns true if kind was successfully authored.
    pub fn set_kind(&self, kind: &Token) -> bool {
        self.prim().set_kind(kind)
    }

    /// Return true if the prim's kind metadata is or inherits from baseKind.
    ///
    /// Matches C++ `IsKind(const TfToken& baseKind, KindValidation validation)`.
    pub fn is_kind(&self, base_kind: &Token, validation: KindValidation) -> bool {
        if validation == KindValidation::ModelHierarchy
            && is_a(base_kind, kind_tokens().model.as_token())
            && !self.is_model()
        {
            return false;
        }

        if let Some(prim_kind) = self.get_kind() {
            is_a(&prim_kind, base_kind)
        } else {
            false
        }
    }

    /// Return true if this prim represents a model, based on its kind metadata.
    ///
    /// Matches C++ `IsModel()`.
    pub fn is_model(&self) -> bool {
        self.prim().is_model()
    }

    /// Return true if this prim represents a model group, based on its kind metadata.
    ///
    /// Matches C++ `IsGroup()`.
    pub fn is_group(&self) -> bool {
        self.prim().is_group()
    }

    // ========================================================================
    // Model Asset Info API
    // ========================================================================

    /// Returns the model's asset identifier as authored in the composed assetInfo dictionary.
    ///
    /// Matches C++ `GetAssetIdentifier(SdfAssetPath *identifier)`.
    pub fn get_asset_identifier(&self) -> Option<AssetPath> {
        if let Some(value) = self.prim().get_asset_info_by_key(&Token::new("identifier")) {
            value.downcast_clone::<AssetPath>()
        } else {
            None
        }
    }

    /// Sets the model's asset identifier to the given asset path.
    ///
    /// Matches C++ `SetAssetIdentifier(const SdfAssetPath &identifier)`.
    pub fn set_asset_identifier(&self, identifier: &AssetPath) {
        self.prim()
            .set_asset_info_by_key(&Token::new("identifier"), Value::from(identifier.clone()));
    }

    /// Returns the model's asset name from the composed assetInfo dictionary.
    ///
    /// Matches C++ `GetAssetName(std::string *assetName)`.
    pub fn get_asset_name(&self) -> Option<String> {
        if let Some(value) = self.prim().get_asset_info_by_key(&Token::new("name")) {
            value.downcast_clone::<String>()
        } else {
            None
        }
    }

    /// Sets the model's asset name.
    ///
    /// Matches C++ `SetAssetName(const std::string &assetName)`.
    pub fn set_asset_name(&self, asset_name: &str) {
        self.prim()
            .set_asset_info_by_key(&Token::new("name"), Value::from(asset_name.to_string()));
    }

    /// Returns the model's resolved asset version.
    ///
    /// Matches C++ `GetAssetVersion(std::string *version)`.
    pub fn get_asset_version(&self) -> Option<String> {
        if let Some(value) = self.prim().get_asset_info_by_key(&Token::new("version")) {
            value.downcast_clone::<String>()
        } else {
            None
        }
    }

    /// Sets the model's asset version string.
    ///
    /// Matches C++ `SetAssetVersion(const std::string &version)`.
    pub fn set_asset_version(&self, version: &str) {
        self.prim()
            .set_asset_info_by_key(&Token::new("version"), Value::from(version.to_string()));
    }

    /// Returns the list of asset dependencies referenced inside the payload of the model.
    ///
    /// Matches C++ `GetPayloadAssetDependencies(VtArray<SdfAssetPath> *assetDeps)`.
    pub fn get_payload_asset_dependencies(&self) -> Option<Array<AssetPath>> {
        if let Some(value) = self
            .prim()
            .get_asset_info_by_key(&Token::new("payloadAssetDependencies"))
        {
            value.downcast_clone::<Array<AssetPath>>()
        } else {
            None
        }
    }

    /// Sets the list of external asset dependencies referenced inside the payload of a model.
    ///
    /// Matches C++ `SetPayloadAssetDependencies(const VtArray<SdfAssetPath> &assetDeps)`.
    pub fn set_payload_asset_dependencies(&self, asset_deps: &Array<AssetPath>) {
        self.prim().set_asset_info_by_key(
            &Token::new("payloadAssetDependencies"),
            Value::from(asset_deps.clone()),
        );
    }

    /// Returns the model's composed assetInfo dictionary.
    ///
    /// Matches C++ `GetAssetInfo(VtDictionary *info)`.
    pub fn get_asset_info(&self) -> Option<HashMap<String, Value>> {
        let asset_info = self.prim().get_asset_info();
        if asset_info.is_empty() {
            None
        } else {
            Some(asset_info)
        }
    }

    /// Sets the model's assetInfo dictionary to info in the current edit target.
    ///
    /// Matches C++ `SetAssetInfo(const VtDictionary &info)`.
    pub fn set_asset_info(&self, info: HashMap<String, Value>) {
        self.prim().set_asset_info(info);
    }
}

impl PartialEq for ModelAPI {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base
    }
}

impl Eq for ModelAPI {}
