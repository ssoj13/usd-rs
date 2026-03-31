//! DataSourcePrimvars - Primvars data source for Hydra.
//!
//! Port of pxr/usdImaging/usdImaging/dataSourcePrimvars.h/cpp
//!
//! Data source representing USD primvars (primitive variables).
//! Enumerates all "primvars:" namespace attributes on a USD prim and
//! exposes each as a DataSourcePrimvar container with value, interpolation,
//! role, and optional indices.

use crate::data_source_attribute::DataSourceAttribute;
use crate::data_source_stage_globals::DataSourceStageGlobalsHandle;
use crate::primvar_utils::{usd_to_hd_interpolation_token, usd_to_hd_role};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use usd_core::Prim;
use usd_core::attribute::Attribute;
use usd_gf::{Vec2f, Vec3f};
use usd_hd::{
    HdContainerDataSource, HdDataSourceBaseHandle, HdDataSourceLocator, HdDataSourceLocatorSet,
    HdRetainedTypedSampledDataSource,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::{Array, Value};

// Token constants
mod tokens {
    use std::sync::LazyLock;
    use usd_tf::Token;

    pub static PRIMVARS: LazyLock<Token> = LazyLock::new(|| Token::new("primvars"));
    pub static PRIMVAR_VALUE: LazyLock<Token> = LazyLock::new(|| Token::new("primvarValue"));
    pub static INDEXED_PRIMVAR_VALUE: LazyLock<Token> =
        LazyLock::new(|| Token::new("indexedPrimvarValue"));
    pub static INDICES: LazyLock<Token> = LazyLock::new(|| Token::new("indices"));
    pub static INTERPOLATION: LazyLock<Token> = LazyLock::new(|| Token::new("interpolation"));
    pub static ROLE: LazyLock<Token> = LazyLock::new(|| Token::new("role"));
    pub static VERTEX: LazyLock<Token> = LazyLock::new(|| Token::new("vertex"));
    #[allow(dead_code)] // C++ interpolation token kept for token parity
    pub static CONSTANT: LazyLock<Token> = LazyLock::new(|| Token::new("constant"));
}

/// Primvar names rejected from the "primvars:" namespace because they are
/// handled as custom primvar mappings (points, velocities, accelerations).
const REJECTED_PRIMVARS: &[&str] = &["points", "velocities", "accelerations"];

/// Returns true if this primvar name should be rejected from enumeration.
/// Matches C++ `_RejectPrimvar()`.
fn reject_primvar(name: &str) -> bool {
    REJECTED_PRIMVARS.contains(&name)
}

/// Read interpolation metadata from a primvar attribute.
/// Re-implements UsdGeomPrimvar::GetInterpolation() with "vertex" as default
/// (matching C++ behavior in dataSourcePrimvars.cpp).
fn get_interpolation(attr: &Attribute) -> Token {
    if let Some(val) = attr.get_metadata(&tokens::INTERPOLATION) {
        if let Some(s) = val.get::<String>() {
            return usd_to_hd_interpolation_token(&Token::new(s));
        }
    }
    // Default to "vertex" (matching C++ _GetInterpolation)
    tokens::VERTEX.clone()
}

/// Check if a primvar has an indices attribute with an authored value.
fn has_indices(prim: &Prim, primvar_name: &str) -> bool {
    let indices_name = format!("primvars:{}:indices", primvar_name);
    if let Some(indices_attr) = prim.get_attribute(&indices_name) {
        return indices_attr.has_authored_value();
    }
    false
}

fn make_primvar_value_data_source(
    attr: &Attribute,
    stage_globals: &DataSourceStageGlobalsHandle,
    scene_index_path: &Path,
) -> HdDataSourceBaseHandle {
    let type_name = attr.get_type_name();
    let scalar_type = type_name.scalar_type();
    let type_token = if scalar_type.is_valid() {
        scalar_type.as_token()
    } else {
        attr.type_name()
    };

    match type_token.as_str() {
        "int" | "int[]" => DataSourceAttribute::<Array<i32>>::new(
            attr.clone(),
            stage_globals.clone(),
            scene_index_path.clone(),
        ) as HdDataSourceBaseHandle,
        "float" | "float[]" => DataSourceAttribute::<Array<f32>>::new(
            attr.clone(),
            stage_globals.clone(),
            scene_index_path.clone(),
        ) as HdDataSourceBaseHandle,
        "double" | "double[]" => DataSourceAttribute::<Array<f64>>::new(
            attr.clone(),
            stage_globals.clone(),
            scene_index_path.clone(),
        ) as HdDataSourceBaseHandle,
        "token" | "token[]" => DataSourceAttribute::<Array<Token>>::new(
            attr.clone(),
            stage_globals.clone(),
            scene_index_path.clone(),
        ) as HdDataSourceBaseHandle,
        "float2" | "float2[]" | "texCoord2f" | "texCoord2f[]" => DataSourceAttribute::<Vec<Vec2f>>::new(
            attr.clone(),
            stage_globals.clone(),
            scene_index_path.clone(),
        ) as HdDataSourceBaseHandle,
        "point3f" | "point3f[]" | "normal3f" | "normal3f[]" | "vector3f" | "vector3f[]"
        | "color3f" | "color3f[]" => {
            DataSourceAttribute::<Vec<Vec3f>>::new(
                attr.clone(),
                stage_globals.clone(),
                scene_index_path.clone(),
            ) as HdDataSourceBaseHandle
        }
        _ => DataSourceAttribute::<Value>::new(
            attr.clone(),
            stage_globals.clone(),
            scene_index_path.clone(),
        ) as HdDataSourceBaseHandle,
    }
}

// ============================================================================
// PrimvarMapping
// ============================================================================

/// Mapping from primvar name to USD attribute name.
///
/// Used by DataSourceCustomPrimvars to map non-primvar attributes
/// to primvars (e.g., "points", "normals").
#[derive(Debug, Clone)]
pub struct PrimvarMapping {
    /// The primvar name in Hydra
    pub primvar_name: Token,
    /// The USD attribute name
    pub usd_attr_name: Token,
    /// Optional interpolation override
    pub interpolation: Option<Token>,
}

impl PrimvarMapping {
    /// Create a new primvar mapping.
    pub fn new(primvar_name: Token, usd_attr_name: Token) -> Self {
        Self {
            primvar_name,
            usd_attr_name,
            interpolation: None,
        }
    }

    /// Create a new primvar mapping with interpolation.
    pub fn with_interpolation(
        primvar_name: Token,
        usd_attr_name: Token,
        interpolation: Token,
    ) -> Self {
        Self {
            primvar_name,
            usd_attr_name,
            interpolation: Some(interpolation),
        }
    }
}

// ============================================================================
// DataSourcePrimvars
// ============================================================================

/// Container data source representing USD primvars.
///
/// Enumerates all "primvars:" attributes on a prim (excluding rejected ones
/// like points/velocities/accelerations) and returns a DataSourcePrimvar
/// container for each.
#[derive(Clone)]
struct CachedPrimvarEntry {
    name: Token,
    value_attr: Attribute,
    indices_attr: Option<Attribute>,
    interpolation: Token,
    role: Token,
    indexed: bool,
}

/// Lazily built authored primvar catalog for one USD prim.
///
/// The live `flo.usdz` profile shows that re-enumerating the `primvars:`
/// namespace and then re-querying each attribute dominates first-load mesh
/// sync. A per-instance catalog lets `get_names()` and `get(name)` share the
/// same namespace scan and metadata reads without changing observable Hydra
/// semantics.
#[derive(Clone)]
pub struct DataSourcePrimvars {
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
    catalog: Arc<OnceLock<Box<[CachedPrimvarEntry]>>>,
}

impl DataSourcePrimvars {
    /// Create a new primvars data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
            catalog: Arc::new(OnceLock::new()),
        }
    }

    /// Build the authored primvar catalog once from the USD namespace.
    fn build_catalog(&self) -> Box<[CachedPrimvarEntry]> {
        if !self.prim.is_valid() {
            return Vec::new().into_boxed_slice();
        }

        let props = self
            .prim
            .get_authored_properties_in_namespace(&Token::new("primvars:"));

        let mut result = Vec::new();
        for prop in &props {
            let full_name = prop.name();
            let full_str = full_name.as_str();

            let Some(name) = full_str.strip_prefix("primvars:") else {
                continue;
            };
            if name.ends_with(":indices") || reject_primvar(name) {
                continue;
            }

            let Some(attr) = self.prim.get_attribute(full_str) else {
                continue;
            };
            if !attr.has_authored_value() {
                continue;
            }

            let interpolation = get_interpolation(&attr);
            let role = usd_to_hd_role(&attr.get_role_name());
            let indexed = has_indices(&self.prim, name);
            let indices_attr = if indexed {
                self.prim.get_attribute(&format!("primvars:{}:indices", name))
            } else {
                None
            };

            result.push(CachedPrimvarEntry {
                name: Token::new(name),
                value_attr: attr,
                indices_attr,
                interpolation,
                role,
                indexed,
            });
        }

        result.into_boxed_slice()
    }

    /// Return the cached authored primvar catalog for this prim.
    fn catalog(&self) -> &[CachedPrimvarEntry] {
        self.catalog.get_or_init(|| self.build_catalog()).as_ref()
    }

    /// Get prefixed primvar name ("primvars:" + name).
    pub fn get_prefixed_name(name: &Token) -> Token {
        Token::new(&format!("primvars:{}", name.as_str()))
    }

    /// Compute invalidation locators for primvar property changes.
    ///
    /// For each changed property that starts with "primvars:", returns a
    /// specific sub-locator (primvars > name) rather than the entire primvars
    /// locator. This allows fine-grained dirtying.
    pub fn invalidate(properties: &[Token]) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::new();

        for prop in properties {
            let s = prop.as_str();
            if let Some(name) = s.strip_prefix("primvars:") {
                // Skip indices sub-attributes (primvars:foo:indices)
                if name.contains(":indices") {
                    // Still dirty the parent primvar
                    if let Some(base) = name.strip_suffix(":indices") {
                        locators.insert(HdDataSourceLocator::from_tokens_2(
                            tokens::PRIMVARS.clone(),
                            Token::new(base),
                        ));
                    }
                } else {
                    locators.insert(HdDataSourceLocator::from_tokens_2(
                        tokens::PRIMVARS.clone(),
                        Token::new(name),
                    ));
                }
            }
        }

        locators
    }
}

impl std::fmt::Debug for DataSourcePrimvars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourcePrimvars")
    }
}

impl usd_hd::HdDataSourceBase for DataSourcePrimvars {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourcePrimvars {
    fn get_names(&self) -> Vec<Token> {
        self.catalog()
            .iter()
            .map(|entry| entry.name.clone())
            .collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let entry = self.catalog().iter().find(|entry| &entry.name == name)?;

        Some(Arc::new(DataSourcePrimvar::new(
            self.scene_index_path.clone(),
            entry.name.clone(),
            entry.value_attr.clone(),
            entry.indices_attr.clone(),
            self.stage_globals.clone(),
            entry.interpolation.clone(),
            entry.role.clone(),
            entry.indexed,
        )))
    }
}

/// Handle type for DataSourcePrimvars.
pub type DataSourcePrimvarsHandle = Arc<DataSourcePrimvars>;

// ============================================================================
// DataSourceCustomPrimvars
// ============================================================================

/// Container data source for custom primvar mappings.
///
/// Maps non-"primvars:" attributes to primvars (e.g., "points", "normals").
/// Used by DataSourceGprim to expose PointBased attributes as primvars.
#[derive(Clone)]
pub struct DataSourceCustomPrimvars {
    scene_index_path: Path,
    prim: Prim,
    stage_globals: DataSourceStageGlobalsHandle,
    mappings: Vec<PrimvarMapping>,
}

impl DataSourceCustomPrimvars {
    /// Create a new custom primvars data source.
    pub fn new(
        scene_index_path: Path,
        prim: Prim,
        mappings: Vec<PrimvarMapping>,
        stage_globals: DataSourceStageGlobalsHandle,
    ) -> Self {
        Self {
            scene_index_path,
            prim,
            stage_globals,
            mappings,
        }
    }

    /// Compute invalidation locators for property changes.
    ///
    /// Returns specific primvar sub-locators for each changed property
    /// that matches a mapping, rather than invalidating all primvars.
    pub fn invalidate(properties: &[Token], mappings: &[PrimvarMapping]) -> HdDataSourceLocatorSet {
        let mut locators = HdDataSourceLocatorSet::new();

        // Build reverse lookup: usd attr name -> primvar name
        let name_map: HashMap<&Token, &Token> = mappings
            .iter()
            .map(|m| (&m.usd_attr_name, &m.primvar_name))
            .collect();

        for prop in properties {
            if let Some(primvar_name) = name_map.get(prop) {
                locators.insert(HdDataSourceLocator::from_tokens_2(
                    tokens::PRIMVARS.clone(),
                    (*primvar_name).clone(),
                ));
            }
        }

        locators
    }
}

impl std::fmt::Debug for DataSourceCustomPrimvars {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourceCustomPrimvars")
    }
}

impl usd_hd::HdDataSourceBase for DataSourceCustomPrimvars {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourceCustomPrimvars {
    fn get_names(&self) -> Vec<Token> {
        self.mappings
            .iter()
            .map(|m| m.primvar_name.clone())
            .collect()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if !self.prim.is_valid() {
            return None;
        }

        // Find the mapping for this primvar name
        let mapping = self.mappings.iter().find(|m| &m.primvar_name == name)?;

        let attr = self.prim.get_attribute(mapping.usd_attr_name.as_str())?;

        if !attr.has_authored_value() {
            return None;
        }

        let role = usd_to_hd_role(&attr.get_role_name());

        // Use mapping interpolation override, or read from attribute metadata
        let interpolation = match &mapping.interpolation {
            Some(interp) if !interp.is_empty() => interp.clone(),
            _ => get_interpolation(&attr),
        };

        // Custom primvars are never indexed (no indices attribute query)
        Some(Arc::new(DataSourcePrimvar::new(
            self.scene_index_path.clone(),
            name.clone(),
            attr,
            None,
            self.stage_globals.clone(),
            interpolation,
            role,
            false,
        )))
    }
}

/// Handle type for DataSourceCustomPrimvars.
pub type DataSourceCustomPrimvarsHandle = Arc<DataSourceCustomPrimvars>;

// ============================================================================
// DataSourcePrimvar
// ============================================================================

/// Container data source representing a single primvar.
///
/// Contains value, interpolation, and role. Value can be a flat primvarValue
/// or an indexedPrimvarValue+indices pair for indexed primvars.
///
/// Schema keys:
/// - "primvarValue" - flat (non-indexed) sampled value
/// - "indexedPrimvarValue" - indexed sampled value (raw, before index lookup)
/// - "indices" - index array for indexed primvars
/// - "interpolation" - token (constant/uniform/varying/vertex/faceVarying)
/// - "role" - token (color/normal/point/vector/textureCoordinate/none)
#[derive(Clone)]
pub struct DataSourcePrimvar {
    /// Value attribute (the primvar itself)
    value_attr: Attribute,
    /// Optional indices attribute for indexed primvars
    indices_attr: Option<Attribute>,
    /// Stage globals for time evaluation
    stage_globals: DataSourceStageGlobalsHandle,
    /// Scene index path (for diagnostics / time-varying flagging)
    scene_index_path: Path,
    /// Primvar name (for locator construction)
    #[allow(dead_code)] // C++ keeps the authored primvar name for locator/diagnostic parity
    name: Token,
    /// Cached interpolation token
    interpolation: Token,
    /// Cached role token
    role: Token,
    /// Whether this primvar is indexed
    has_indices: bool,
}

impl DataSourcePrimvar {
    /// Create a new primvar data source.
    pub fn new(
        scene_index_path: Path,
        name: Token,
        value_attr: Attribute,
        indices_attr: Option<Attribute>,
        stage_globals: DataSourceStageGlobalsHandle,
        interpolation: Token,
        role: Token,
        has_indices: bool,
    ) -> Self {
        if has_indices {
            if value_attr.get_time_samples().len() > 1 {
                stage_globals.flag_as_time_varying(
                    &scene_index_path,
                    &HdDataSourceLocator::new(&[
                        tokens::PRIMVARS.clone(),
                        name.clone(),
                        tokens::INDEXED_PRIMVAR_VALUE.clone(),
                    ]),
                );
            }
            if let Some(indices_attr) = indices_attr.as_ref() {
                if indices_attr.get_time_samples().len() > 1 {
                    stage_globals.flag_as_time_varying(
                        &scene_index_path,
                        &HdDataSourceLocator::new(&[
                            tokens::PRIMVARS.clone(),
                            name.clone(),
                            tokens::INDICES.clone(),
                        ]),
                    );
                }
            }
        } else if value_attr.get_time_samples().len() > 1 {
            stage_globals.flag_as_time_varying(
                &scene_index_path,
                &HdDataSourceLocator::new(&[
                    tokens::PRIMVARS.clone(),
                    name.clone(),
                    tokens::PRIMVAR_VALUE.clone(),
                ]),
            );
        }

        Self {
            value_attr,
            indices_attr,
            stage_globals,
            scene_index_path,
            name,
            interpolation,
            role,
            has_indices,
        }
    }

    /// Create a minimal primvar for testing (no attribute backing).
    #[cfg(test)]
    pub fn new_minimal(
        stage_globals: DataSourceStageGlobalsHandle,
        interpolation: Option<Token>,
        role: Option<Token>,
        has_indices: bool,
    ) -> Self {
        Self {
            value_attr: Attribute::invalid(),
            indices_attr: None,
            stage_globals,
            scene_index_path: Path::absolute_root(),
            name: Token::new("test"),
            interpolation: interpolation.unwrap_or_else(|| tokens::VERTEX.clone()),
            role: role.unwrap_or_else(|| Token::new("")),
            has_indices,
        }
    }

}

impl std::fmt::Debug for DataSourcePrimvar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DataSourcePrimvar")
    }
}

impl usd_hd::HdDataSourceBase for DataSourcePrimvar {
    fn clone_box(&self) -> usd_hd::HdDataSourceBaseHandle {
        std::sync::Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for DataSourcePrimvar {
    fn get_names(&self) -> Vec<Token> {
        let mut names = vec![tokens::INTERPOLATION.clone(), tokens::ROLE.clone()];

        if self.has_indices {
            names.push(tokens::INDEXED_PRIMVAR_VALUE.clone());
            names.push(tokens::INDICES.clone());
        } else {
            names.push(tokens::PRIMVAR_VALUE.clone());
        }

        names
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if self.has_indices {
            if name == &*tokens::INDEXED_PRIMVAR_VALUE {
                // Return sampled data source wrapping the value attribute
                return Some(make_primvar_value_data_source(
                    &self.value_attr,
                    &self.stage_globals,
                    &self.scene_index_path,
                ));
            }
            if name == &*tokens::INDICES {
                // Return sampled data source wrapping the indices attribute
                if let Some(ref indices) = self.indices_attr {
                    return Some(make_primvar_value_data_source(
                        indices,
                        &self.stage_globals,
                        &self.scene_index_path,
                    ));
                }
                return None;
            }
        } else if name == &*tokens::PRIMVAR_VALUE {
            // Return sampled data source wrapping the value attribute
            return Some(make_primvar_value_data_source(
                &self.value_attr,
                &self.stage_globals,
                &self.scene_index_path,
            ));
        }

        if name == &*tokens::INTERPOLATION {
            return Some(
                HdRetainedTypedSampledDataSource::new(self.interpolation.clone())
                    as HdDataSourceBaseHandle,
            );
        }

        if name == &*tokens::ROLE {
            return Some(
                HdRetainedTypedSampledDataSource::new(self.role.clone()) as HdDataSourceBaseHandle
            );
        }

        None
    }
}

/// Handle type for DataSourcePrimvar.
pub type DataSourcePrimvarHandle = Arc<DataSourcePrimvar>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    fn create_test_globals() -> DataSourceStageGlobalsHandle {
        Arc::new(NoOpStageGlobals::default())
    }

    #[test]
    fn test_prefixed_name() {
        let name = Token::new("foo");
        let prefixed = DataSourcePrimvars::get_prefixed_name(&name);
        assert_eq!(prefixed.as_str(), "primvars:foo");
    }

    #[test]
    fn test_primvar_mapping() {
        let mapping = PrimvarMapping::new(Token::new("points"), Token::new("points"));
        assert_eq!(mapping.primvar_name.as_str(), "points");
        assert!(mapping.interpolation.is_none());
    }

    #[test]
    fn test_primvar_mapping_with_interp() {
        let mapping = PrimvarMapping::with_interpolation(
            Token::new("normals"),
            Token::new("normals"),
            Token::new("vertex"),
        );
        assert_eq!(mapping.interpolation.as_ref().unwrap().as_str(), "vertex");
    }

    #[test]
    fn test_primvar_names_flat() {
        let globals = create_test_globals();
        let ds = DataSourcePrimvar::new_minimal(globals, None, None, false);
        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "primvarValue"));
        assert!(!names.iter().any(|n| n == "indices"));
    }

    #[test]
    fn test_primvar_names_indexed() {
        let globals = create_test_globals();
        let ds = DataSourcePrimvar::new_minimal(globals, None, None, true);
        let names = ds.get_names();
        assert!(names.iter().any(|n| n == "indexedPrimvarValue"));
        assert!(names.iter().any(|n| n == "indices"));
    }

    #[test]
    fn test_primvar_get_interpolation() {
        let globals = create_test_globals();
        let ds =
            DataSourcePrimvar::new_minimal(globals, Some(Token::new("faceVarying")), None, false);
        let interp = ds.get(&Token::new("interpolation"));
        assert!(interp.is_some());
    }

    #[test]
    fn test_primvar_get_role() {
        let globals = create_test_globals();
        let ds = DataSourcePrimvar::new_minimal(globals, None, Some(Token::new("color")), false);
        let role = ds.get(&Token::new("role"));
        assert!(role.is_some());
    }

    #[test]
    fn test_data_source_primvars_empty_prim() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let ds = DataSourcePrimvars::new(Path::absolute_root(), prim, globals);
        // Pseudo root has no primvars
        assert!(ds.get_names().is_empty());
    }

    #[test]
    fn test_reject_primvar() {
        assert!(reject_primvar("points"));
        assert!(reject_primvar("velocities"));
        assert!(reject_primvar("accelerations"));
        assert!(!reject_primvar("displayColor"));
        assert!(!reject_primvar("st"));
    }

    #[test]
    fn test_invalidate_specific_locators() {
        let props = vec![
            Token::new("primvars:displayColor"),
            Token::new("primvars:st"),
        ];
        let locators = DataSourcePrimvars::invalidate(&props);
        assert!(!locators.is_empty());
    }

    #[test]
    fn test_invalidate_indices() {
        let props = vec![Token::new("primvars:uv:indices")];
        let locators = DataSourcePrimvars::invalidate(&props);
        // Should dirty the parent primvar "uv"
        assert!(!locators.is_empty());
    }

    #[test]
    fn test_custom_primvars_invalidate() {
        let mappings = vec![
            PrimvarMapping::new(Token::new("points"), Token::new("points")),
            PrimvarMapping::new(Token::new("normals"), Token::new("normals")),
        ];
        let props = vec![Token::new("points")];
        let locators = DataSourceCustomPrimvars::invalidate(&props, &mappings);
        assert!(!locators.is_empty());
    }

    #[test]
    fn test_custom_primvars_invalidate_no_match() {
        let mappings = vec![PrimvarMapping::new(
            Token::new("points"),
            Token::new("points"),
        )];
        let props = vec![Token::new("faceVertexCounts")];
        let locators = DataSourceCustomPrimvars::invalidate(&props, &mappings);
        assert!(locators.is_empty());
    }

    #[test]
    fn test_custom_primvars_get_names() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.get_pseudo_root();
        let globals = create_test_globals();

        let mappings = vec![
            PrimvarMapping::new(Token::new("points"), Token::new("points")),
            PrimvarMapping::new(Token::new("normals"), Token::new("normals")),
        ];

        let ds = DataSourceCustomPrimvars::new(Path::absolute_root(), prim, mappings, globals);
        let names = ds.get_names();
        assert_eq!(names.len(), 2);
        assert!(names.iter().any(|n| n == "points"));
        assert!(names.iter().any(|n| n == "normals"));
    }
}
