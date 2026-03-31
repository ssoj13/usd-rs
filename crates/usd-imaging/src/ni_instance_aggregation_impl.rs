//! Internal implementation of NiInstanceAggregationSceneIndex.
//!
//! Port of UsdImaging_NiInstanceAggregationSceneIndex_Impl namespace.

use crate::usd_prim_info_schema::UsdPrimInfoSchema;
use std::sync::Arc;
use usd_hd::HdTypedSampledDataSource;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdRetainedContainerDataSource, HdSampledDataSource, hd_container_get, hd_data_source_hash,
};
use usd_hd::schema::{HdInstancedBySchema, HdPrimvarsSchema};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Scope for propagated prototypes. Port of _tokens::propagatedPrototypesScope.
const PROPAGATED_PROTOTYPES_SCOPE: &str = "UsdNiPropagatedPrototypes";

/// Instance information for aggregation grouping.
/// Port of _InstanceInfo.
#[derive(Clone, Debug)]
pub(super) struct InstanceInfo {
    pub enclosing_prototype_root: SdfPath,
    pub binding_hash: TfToken,
    pub prototype_name: TfToken,
}

impl InstanceInfo {
    pub fn is_instance(&self) -> bool {
        !self.prototype_name.is_empty()
    }

    /// Path like /X/Y/UsdNiPropagatedPrototypes/BindingHash
    pub fn get_binding_prim_path(&self) -> SdfPath {
        self.enclosing_prototype_root
            .append_child(PROPAGATED_PROTOTYPES_SCOPE)
            .and_then(|p| p.append_child(self.binding_hash.as_str()))
            .unwrap_or_else(SdfPath::absolute_root)
    }

    /// Path like .../BindingHash/__Prototype_1
    pub fn get_propagated_prototype_base(&self) -> SdfPath {
        self.get_binding_prim_path()
            .append_child(self.prototype_name.as_str())
            .unwrap_or_else(SdfPath::absolute_root)
    }

    /// Path like .../__Prototype_1/UsdNiInstancer
    pub fn get_instancer_path(&self) -> SdfPath {
        self.get_propagated_prototype_base()
            .append_child("UsdNiInstancer")
            .unwrap_or_else(SdfPath::absolute_root)
    }

    /// Path like .../UsdNiInstancer/UsdNiPrototype
    pub fn get_prototype_path(&self) -> SdfPath {
        self.get_instancer_path()
            .append_child("UsdNiPrototype")
            .unwrap_or_else(SdfPath::absolute_root)
    }
}

/// Get primvars schema from prim at path.
pub(super) fn get_primvars_schema(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    prim_path: &SdfPath,
) -> HdPrimvarsSchema {
    let prim = scene.get_prim(prim_path);
    if let Some(ref container) = prim.data_source {
        HdPrimvarsSchema::get_from_parent(container)
    } else {
        let empty: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        HdPrimvarsSchema::get_from_parent(&empty)
    }
}

/// Get primvar (primvars/name) container from prim.
pub(super) fn get_primvar_schema(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    prim_path: &SdfPath,
    primvar_name: &TfToken,
) -> Option<HdContainerDataSourceHandle> {
    let primvars = get_primvars_schema(scene, prim_path);
    primvars.get_primvar(primvar_name)
}

/// Check if primvar has constant interpolation.
pub(super) fn is_constant_primvar(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    prim_path: &SdfPath,
    primvar_name: &TfToken,
) -> bool {
    let primvar = match get_primvar_schema(scene, prim_path, primvar_name) {
        Some(p) => p,
        None => return false,
    };
    let interp_ds = match primvar.get(&TfToken::new("interpolation")) {
        Some(ds) => ds,
        None => return false,
    };
    let any = &interp_ds as &dyn std::any::Any;
    let token_ds = match any.downcast_ref::<Arc<dyn HdTypedSampledDataSource<TfToken>>>() {
        Some(ds) => ds,
        None => return false,
    };
    let interp = token_ds.get_typed_value(0.0);
    interp == "constant"
}

/// Get names of constant primvars on prim.
pub(super) fn get_constant_primvar_names(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    prim_path: &SdfPath,
) -> Vec<TfToken> {
    let primvars = get_primvars_schema(scene, prim_path);
    let mut result = Vec::new();
    for name in primvars.get_primvar_names() {
        if is_constant_primvar(scene, prim_path, &name) {
            result.push(name);
        }
    }
    result
}

/// Get primvar value as VtValue (generic). Returns None if not found.
pub(super) fn get_primvar_value(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    prim_path: &SdfPath,
    primvar_name: &TfToken,
) -> Option<usd_vt::Value> {
    let primvar = get_primvar_schema(scene, prim_path, primvar_name)?;
    let value_ds = primvar.get(&TfToken::new("primvarValue"))?;
    let any = &value_ds as &dyn std::any::Any;
    if let Some(sampled) = any.downcast_ref::<Arc<dyn HdSampledDataSource>>() {
        return Some(sampled.get_value(0.0));
    }
    None
}

/// Get typed primvar value. Handles both T and Vec<T>.
pub(super) fn get_typed_primvar_value<T: Clone + Default>(
    scene: &dyn usd_hd::scene_index::HdSceneIndexBase,
    prim_path: &SdfPath,
    primvar_name: &TfToken,
) -> T
where
    T: 'static,
{
    let value = get_primvar_value(scene, prim_path, primvar_name);
    if let Some(ref v) = value {
        if let Some(t) = v.get::<T>() {
            return t.clone();
        }
        if let Some(arr) = v.get::<Vec<T>>() {
            if let Some(first) = arr.first() {
                return first.clone();
            }
        }
    }
    T::default()
}

/// Compute hash of constant primvars (names + roles) for binding grouping.
/// Port of _ComputeConstantPrimvarsRoleHash.
/// Only primvars with interpolation == constant are included; role may be empty.
pub(super) fn compute_constant_primvars_role_hash(primvars: &HdPrimvarsSchema) -> String {
    use std::collections::BTreeMap;
    let mut name_to_role: BTreeMap<String, String> = BTreeMap::new();
    for name in primvars.get_primvar_names() {
        if let Some(primvar) = primvars.get_primvar(&name) {
            // C++: only add if interpolation == constant
            let is_constant = primvar
                .get(&TfToken::new("interpolation"))
                .and_then(|ds| {
                    let base = ds.as_ref() as &dyn HdDataSourceBase;
                    base.sample_at_zero()
                })
                .and_then(|v| v.get::<TfToken>().map(|t| t == "constant"))
                .unwrap_or(false);
            if !is_constant {
                continue;
            }
            let role = primvar
                .get(&TfToken::new("role"))
                .and_then(|ds| {
                    let base = ds.as_ref() as &dyn HdDataSourceBase;
                    base.sample_at_zero()
                })
                .and_then(|v| v.get::<TfToken>().map(|t| t.as_str().to_string()))
                .unwrap_or_default();
            name_to_role.insert(name.as_str().to_string(), role);
        }
    }
    if name_to_role.is_empty() {
        return "NoPrimvars".to_string();
    }
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for (k, v) in &name_to_role {
        k.hash(&mut hasher);
        v.hash(&mut hasher);
    }
    format!("Primvars{:016x}", hasher.finish())
}

/// Compute binding hash for instance grouping.
/// Port of _ComputeBindingHash.
pub(super) fn compute_binding_hash(
    prim_source: &HdContainerDataSourceHandle,
    instance_data_source_names: &[TfToken],
) -> TfToken {
    let primvars = HdPrimvarsSchema::get_from_parent(prim_source);
    let mut result = compute_constant_primvars_role_hash(&primvars);

    for name in instance_data_source_names {
        if let Some(ds) = prim_source.get(name) {
            let hash = hd_data_source_hash(&ds, 0.0, 0.0);
            result.push_str(&format!("_{}{:016x}", name.as_str(), hash));
        }
    }

    TfToken::new(&result)
}

/// Get niPrototypePath from UsdPrimInfoSchema.
pub(super) fn get_usd_prototype_path(prim_source: &HdContainerDataSourceHandle) -> SdfPath {
    let locator = UsdPrimInfoSchema::get_ni_prototype_path_locator();
    if let Some(path_ds) = hd_container_get(prim_source.clone(), &locator) {
        if let Some(sampled) = path_ds
            .as_any()
            .downcast_ref::<Arc<dyn HdSampledDataSource>>()
        {
            let v = sampled.get_value(0.0);
            if let Some(path) = v.get::<SdfPath>() {
                return path.clone();
            }
        }
    }
    SdfPath::default()
}

/// Get prototype name from niPrototypePath.
pub(super) fn get_usd_prototype_name(prim_source: &HdContainerDataSourceHandle) -> TfToken {
    let path = get_usd_prototype_path(prim_source);
    if path.is_empty() {
        return TfToken::default();
    }
    path.get_name_token()
}

/// Get prototype root from InstancedBySchema.prototypeRoots.
pub(super) fn get_prototype_root(prim_source: &HdContainerDataSourceHandle) -> SdfPath {
    let schema = HdInstancedBySchema::get_from_parent(prim_source);
    if !schema.is_defined() {
        return SdfPath::default();
    }
    if let Some(ds) = schema.get_prototype_roots() {
        let paths = ds.get_typed_value(0.0);
        if let Some(first) = paths.first() {
            return first.clone();
        }
    }
    SdfPath::default()
}

/// Compute locators that force re-aggregation when dirtied.
/// Port of _ComputeResyncLocators.
pub(super) fn compute_resync_locators(
    instance_data_source_names: &[TfToken],
) -> usd_hd::data_source::HdDataSourceLocatorSet {
    use usd_hd::data_source::HdDataSourceLocatorSet;
    use usd_hd::schema::HdInstancedBySchema;

    let mut result = HdDataSourceLocatorSet::new();
    result
        .insert(HdInstancedBySchema::get_default_locator().append(&TfToken::new("prototypeRoots")));
    for name in instance_data_source_names {
        result.insert(usd_hd::data_source::HdDataSourceLocator::from_token(
            name.clone(),
        ));
    }
    result
}

/// Make binding copy - partial copy of prim data source using instanceDataSourceNames.
/// Each data source is statically copied (HdMakeStaticCopy) to snapshot values.
/// Port of _MakeBindingCopy.
pub(super) fn make_binding_copy(
    prim_source: &HdContainerDataSourceHandle,
    instance_data_source_names: &[TfToken],
) -> HdContainerDataSourceHandle {
    use usd_hd::data_source::hd_make_static_copy;

    let mut names = Vec::new();
    let mut data_sources = Vec::new();

    for name in instance_data_source_names {
        if let Some(ds) = prim_source.get(name) {
            let copy = hd_make_static_copy(&ds).unwrap_or_else(|| ds.clone());
            names.push(name.clone());
            data_sources.push(copy);
        }
    }

    if names.is_empty() {
        return HdRetainedContainerDataSource::new_empty();
    }

    let entries: Vec<(TfToken, HdDataSourceBaseHandle)> = names
        .into_iter()
        .zip(data_sources.into_iter())
        .map(|(n, d)| (n, d))
        .collect();

    HdRetainedContainerDataSource::from_entries(&entries)
}
