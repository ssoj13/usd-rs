
//! Material override resolving scene index.
//!
//! Port of HdsiMaterialOverrideResolvingSceneIndex from
//! pxr/imaging/hdsi/materialOverrideResolvingSceneIndex.h/.cpp
//!
//! Applies material overrides in the form of edits to a material's interface
//! or directly to parameters of its shader nodes. When geometry has
//! materialOverride data sources, a copy of the bound material is generated
//! and overrides are applied only to it.

use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, cast_to_container,
    hd_make_static_copy,
};
use usd_hd::scene_index::filtering::{
    FilteringObserverTarget, FilteringSceneIndexObserver, HdSingleInputFilteringSceneIndexBase,
};
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::prim::HdSceneIndexPrim;
use usd_hd::scene_index::{HdSceneIndexBase, HdSceneIndexHandle, SdfPathVector, si_ref};
use usd_hd::schema::{
    HdMaterialBindingsSchema, HdMaterialInterfaceSchema, HdMaterialOverrideSchema,
};
use usd_hd::tokens::hd_prim_type_is_gprim;
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

// ---- private tokens ----

static MATERIAL_OVERRIDE: Lazy<TfToken> = Lazy::new(|| TfToken::new("materialOverride"));
static MATERIAL: Lazy<TfToken> = Lazy::new(|| TfToken::new("material"));
static MATERIAL_BINDINGS: Lazy<TfToken> = Lazy::new(|| TfToken::new("materialBindings"));
static DEPENDENCIES: Lazy<TfToken> = Lazy::new(|| TfToken::new("__dependencies"));
static MATERIAL_OVERRIDE_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("materialOverrideDependency"));
static PARAMETERS: Lazy<TfToken> = Lazy::new(|| TfToken::new("parameters"));
static INTERFACE: Lazy<TfToken> = Lazy::new(|| TfToken::new("interface"));
static NODES: Lazy<TfToken> = Lazy::new(|| TfToken::new("nodes"));
static DEPENDED_ON_PRIM_PATH: Lazy<TfToken> = Lazy::new(|| TfToken::new("dependedOnPrimPath"));
static DEPENDED_ON_DS_LOCATOR: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("dependedOnDataSourceLocator"));
static AFFECTED_DS_LOCATOR: Lazy<TfToken> = Lazy::new(|| TfToken::new("affectedDataSourceLocator"));

/// material prim type token
static MATERIAL_PRIM_TYPE: Lazy<TfToken> = Lazy::new(|| TfToken::new("material"));
/// "full" purpose for material bindings
static FULL: Lazy<TfToken> = Lazy::new(|| TfToken::new("full"));
/// allPurpose
static ALL_PURPOSE: Lazy<TfToken> = Lazy::new(|| TfToken::new("allPurpose"));
/// internal empty-string allPurpose
static ALL_PURPOSE_EMPTY: Lazy<TfToken> = Lazy::new(|| TfToken::new(""));
/// path token
static PATH: Lazy<TfToken> = Lazy::new(|| TfToken::new("path"));
/// interfaceValues token
static INTERFACE_VALUES: Lazy<TfToken> = Lazy::new(|| TfToken::new("interfaceValues"));
/// parameterValues token
static PARAMETER_VALUES: Lazy<TfToken> = Lazy::new(|| TfToken::new("parameterValues"));

// ---- helper type aliases ----

/// nodePath -> (inputName -> publicUIName)
type NestedTokenMap = HashMap<TfToken, HashMap<TfToken, TfToken>>;

// ---- data source implementations ----

/// Wraps a material node's "parameters" container, applying overrides.
///
/// Corresponds to C++ `_ParametersContainerDataSource`.
#[derive(Clone)]
struct ParametersContainerDs {
    /// Original node parameters container
    params: HdContainerDataSourceHandle,
    /// materialOverride data source from the geometry prim
    mat_override_ds: HdContainerDataSourceHandle,
    /// Reversed interface mappings: nodePath -> (inputName -> publicUIName)
    reverse_mappings: Option<Arc<NestedTokenMap>>,
    /// This node's name within the material network
    node_path: TfToken,
}

impl ParametersContainerDs {
    fn new(
        params: HdContainerDataSourceHandle,
        mat_override_ds: HdContainerDataSourceHandle,
        reverse_mappings: Option<Arc<NestedTokenMap>>,
        node_path: TfToken,
    ) -> Arc<Self> {
        Arc::new(Self {
            params,
            mat_override_ds,
            reverse_mappings,
            node_path,
        })
    }

    /// Returns override names for this node's parameters.
    fn get_override_names(&self) -> HashSet<TfToken> {
        let mut names = HashSet::new();

        // 1. Parameter edit overrides: look up parameterValues[nodePath][*]
        if let Some(param_vals_ds) = self.mat_override_ds.get(&*PARAMETER_VALUES) {
            if let Some(param_c) = cast_to_container(&param_vals_ds) {
                if let Some(node_ds) = param_c.get(&self.node_path) {
                    if let Some(node_c) = cast_to_container(&node_ds) {
                        for name in node_c.get_names() {
                            names.insert(name);
                        }
                    }
                }
            }
        }

        // 2. Interface mapping overrides
        let reverse = match &self.reverse_mappings {
            Some(m) => m,
            None => return names,
        };
        let params_map = match reverse.get(&self.node_path) {
            Some(m) => m,
            None => return names,
        };

        if let Some(iface_ds) = self.mat_override_ds.get(&*INTERFACE_VALUES) {
            if let Some(iface_c) = cast_to_container(&iface_ds) {
                for (input_name, public_ui_name) in params_map {
                    if iface_c.get(public_ui_name).is_some() {
                        names.insert(input_name.clone());
                    }
                }
            }
        }

        names
    }

    /// Get the overriding container for a given parameter name (interface or param edit).
    /// Interface overrides take precedence.
    fn get_override_container(&self, name: &TfToken) -> Option<HdContainerDataSourceHandle> {
        // Interface override has higher priority
        if let Some(c) = self.get_public_ui_ds(name) {
            return Some(c);
        }
        self.get_param_edit_ds(name)
    }

    /// Get the interface (publicUI) override container for parameter `name`.
    fn get_public_ui_ds(&self, name: &TfToken) -> Option<HdContainerDataSourceHandle> {
        let reverse = self.reverse_mappings.as_ref()?;
        let params_map = reverse.get(&self.node_path)?;
        let public_ui_name = params_map.get(name)?;

        let iface_ds = self.mat_override_ds.get(&*INTERFACE_VALUES)?;
        let iface_c = cast_to_container(&iface_ds)?;
        let override_ds = iface_c.get(public_ui_name)?;
        cast_to_container(&override_ds)
    }

    /// Get the parameter-edit override container for parameter `name`.
    fn get_param_edit_ds(&self, name: &TfToken) -> Option<HdContainerDataSourceHandle> {
        let param_vals_ds = self.mat_override_ds.get(&*PARAMETER_VALUES)?;
        let param_vals_c = cast_to_container(&param_vals_ds)?;
        let node_ds = param_vals_c.get(&self.node_path)?;
        let node_c = cast_to_container(&node_ds)?;
        let param_ds = node_c.get(name)?;
        cast_to_container(&param_ds)
    }
}

impl std::fmt::Debug for ParametersContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParametersContainerDs")
            .field("node_path", &self.node_path)
            .finish()
    }
}

impl HdDataSourceBase for ParametersContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            params: self.params.clone(),
            mat_override_ds: self.mat_override_ds.clone(),
            reverse_mappings: self.reverse_mappings.clone(),
            node_path: self.node_path.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ParametersContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        let mut names = self.params.get_names();
        for n in self.get_override_names() {
            if !names.contains(&n) {
                names.push(n);
            }
        }
        names
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let original = self.params.get(name);
        let override_ds = self.get_override_container(name);

        match (override_ds, original.and_then(|o| cast_to_container(&o))) {
            (Some(ov), Some(orig)) => Some(HdOverlayContainerDataSource::new_2(ov, orig)),
            (Some(ov), None) => Some(ov as HdDataSourceBaseHandle),
            (None, Some(orig)) => Some(orig as HdDataSourceBaseHandle),
            (None, None) => None,
        }
    }
}

/// Wraps a material node container, intercepting "parameters" child.
///
/// Corresponds to C++ `_MaterialNodeContainerDataSource`.
#[derive(Clone)]
struct MaterialNodeContainerDs {
    node_ds: HdContainerDataSourceHandle,
    mat_override_ds: HdContainerDataSourceHandle,
    reverse_mappings: Option<Arc<NestedTokenMap>>,
    node_path: TfToken,
}

impl MaterialNodeContainerDs {
    fn new(
        node_ds: HdContainerDataSourceHandle,
        mat_override_ds: HdContainerDataSourceHandle,
        reverse_mappings: Option<Arc<NestedTokenMap>>,
        node_path: TfToken,
    ) -> Arc<Self> {
        Arc::new(Self {
            node_ds,
            mat_override_ds,
            reverse_mappings,
            node_path,
        })
    }
}

impl std::fmt::Debug for MaterialNodeContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialNodeContainerDs")
            .field("node_path", &self.node_path)
            .finish()
    }
}

impl HdDataSourceBase for MaterialNodeContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            node_ds: self.node_ds.clone(),
            mat_override_ds: self.mat_override_ds.clone(),
            reverse_mappings: self.reverse_mappings.clone(),
            node_path: self.node_path.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for MaterialNodeContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        self.node_ds.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.node_ds.get(name)?;

        if name != &*PARAMETERS {
            return Some(result);
        }

        let result_container = cast_to_container(&result)?;
        Some(ParametersContainerDs::new(
            result_container,
            self.mat_override_ds.clone(),
            self.reverse_mappings.clone(),
            self.node_path.clone(),
        ) as HdDataSourceBaseHandle)
    }
}

/// Wraps the "nodes" container, intercepting each node to apply overrides.
///
/// Corresponds to C++ `_NodesContainerDataSource`.
#[derive(Clone)]
struct NodesContainerDs {
    nodes_ds: HdContainerDataSourceHandle,
    mat_override_ds: HdContainerDataSourceHandle,
    reverse_mappings: Option<Arc<NestedTokenMap>>,
}

impl NodesContainerDs {
    fn new(
        nodes_ds: HdContainerDataSourceHandle,
        mat_override_ds: HdContainerDataSourceHandle,
        reverse_mappings: Option<Arc<NestedTokenMap>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            nodes_ds,
            mat_override_ds,
            reverse_mappings,
        })
    }
}

impl std::fmt::Debug for NodesContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodesContainerDs").finish()
    }
}

impl HdDataSourceBase for NodesContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            nodes_ds: self.nodes_ds.clone(),
            mat_override_ds: self.mat_override_ds.clone(),
            reverse_mappings: self.reverse_mappings.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for NodesContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        self.nodes_ds.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.nodes_ds.get(name)?;
        let result_container = cast_to_container(&result)?;
        Some(MaterialNodeContainerDs::new(
            result_container,
            self.mat_override_ds.clone(),
            self.reverse_mappings.clone(),
            name.clone(),
        ) as HdDataSourceBaseHandle)
    }
}

/// Wraps a material network container, intercepting "nodes".
///
/// Corresponds to C++ `_MaterialNetworkContainerDataSource`.
#[derive(Clone)]
struct MaterialNetworkContainerDs {
    network_ds: HdContainerDataSourceHandle,
    mat_override_ds: HdContainerDataSourceHandle,
    reverse_mappings: Option<Arc<NestedTokenMap>>,
}

impl MaterialNetworkContainerDs {
    fn new(
        network_ds: HdContainerDataSourceHandle,
        mat_override_ds: HdContainerDataSourceHandle,
        reverse_mappings: Option<Arc<NestedTokenMap>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            network_ds,
            mat_override_ds,
            reverse_mappings,
        })
    }
}

impl std::fmt::Debug for MaterialNetworkContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialNetworkContainerDs").finish()
    }
}

impl HdDataSourceBase for MaterialNetworkContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            network_ds: self.network_ds.clone(),
            mat_override_ds: self.mat_override_ds.clone(),
            reverse_mappings: self.reverse_mappings.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for MaterialNetworkContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        self.network_ds.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.network_ds.get(name)?;

        if name != &*NODES {
            return Some(result);
        }

        let result_container = cast_to_container(&result)?;
        Some(NodesContainerDs::new(
            result_container,
            self.mat_override_ds.clone(),
            self.reverse_mappings.clone(),
        ) as HdDataSourceBaseHandle)
    }
}

/// Wraps the "material" container (per render context), applying overrides.
///
/// Corresponds to C++ `_MaterialContainerDataSource`.
#[derive(Clone)]
struct MaterialContainerDs {
    /// Full prim data source (to read materialOverride from)
    input_ds: HdContainerDataSourceHandle,
    /// The "material" child container
    material_ds: HdContainerDataSourceHandle,
}

impl MaterialContainerDs {
    fn new(
        input_ds: HdContainerDataSourceHandle,
        material_ds: HdContainerDataSourceHandle,
    ) -> Arc<Self> {
        Arc::new(Self {
            input_ds,
            material_ds,
        })
    }
}

impl std::fmt::Debug for MaterialContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialContainerDs").finish()
    }
}

impl HdDataSourceBase for MaterialContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_ds: self.input_ds.clone(),
            material_ds: self.material_ds.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for MaterialContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        self.material_ds.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.material_ds.get(name)?;
        let result_container = cast_to_container(&result)?;

        // Only intercept children that are material networks (have "nodes" or "interface")
        let has_nodes = result_container.get(&*NODES).is_some();
        let has_iface = result_container.get(&*INTERFACE).is_some();
        if !has_nodes && !has_iface {
            return Some(result);
        }

        // Check if we have material overrides on the input prim
        let mat_override_ds = self.input_ds.get(&*MATERIAL_OVERRIDE)?;
        let mat_override_c = cast_to_container(&mat_override_ds)?;

        // Build reverse interface mappings
        let reverse_mappings = if let Some(iface_ds) = result_container.get(&*INTERFACE) {
            if let Some(iface_c) = cast_to_container(&iface_ds) {
                let iface_schema = HdMaterialInterfaceSchema::new(iface_c);
                let map = iface_schema.get_reverse_interface_mappings();
                if !map.is_empty() {
                    Some(Arc::new(map))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        Some(
            MaterialNetworkContainerDs::new(result_container, mat_override_c, reverse_mappings)
                as HdDataSourceBaseHandle,
        )
    }
}

/// Wraps the full prim data source for a material prim.
/// Intercepts "material" and "__dependencies" children.
///
/// Corresponds to C++ `_MaterialPrimContainerDataSource`.
#[derive(Clone)]
struct MaterialPrimContainerDs {
    input_ds: HdContainerDataSourceHandle,
    prim_path: SdfPath,
}

impl MaterialPrimContainerDs {
    fn new(input_ds: HdContainerDataSourceHandle, prim_path: SdfPath) -> Arc<Self> {
        Arc::new(Self {
            input_ds,
            prim_path,
        })
    }
}

impl std::fmt::Debug for MaterialPrimContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialPrimContainerDs")
            .field("prim_path", &self.prim_path)
            .finish()
    }
}

impl HdDataSourceBase for MaterialPrimContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_ds: self.input_ds.clone(),
            prim_path: self.prim_path.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for MaterialPrimContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        let mut names = self.input_ds.get_names();
        if !names.contains(&*DEPENDENCIES) {
            names.push(DEPENDENCIES.clone());
        }
        names
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input_ds.get(name);

        if name == &*MATERIAL {
            let result_container = result.and_then(|r| cast_to_container(&r))?;
            return Some(
                MaterialContainerDs::new(self.input_ds.clone(), result_container)
                    as HdDataSourceBaseHandle,
            );
        }

        if name == &*DEPENDENCIES {
            // Declare: material depends on materialOverride (coarse dependency)
            let mat_override_locator = HdDataSourceLocator::from_token(MATERIAL_OVERRIDE.clone());
            let material_locator = HdDataSourceLocator::from_token(MATERIAL.clone());

            let dep_entry = build_dependency_ds(
                self.prim_path.clone(),
                mat_override_locator,
                material_locator,
            );

            let new_dep_ds = HdRetainedContainerDataSource::new_1(
                MATERIAL_OVERRIDE_DEPENDENCY.clone(),
                dep_entry as HdDataSourceBaseHandle,
            );

            return Some(match result.and_then(|r| cast_to_container(&r)) {
                Some(existing) => HdOverlayContainerDataSource::new_2(new_dep_ds, existing)
                    as HdDataSourceBaseHandle,
                None => new_dep_ds as HdDataSourceBaseHandle,
            });
        }

        result
    }
}

/// Build a retained container representing a HdDependencySchema entry.
fn build_dependency_ds(
    depended_prim_path: SdfPath,
    depended_locator: HdDataSourceLocator,
    affected_locator: HdDataSourceLocator,
) -> HdContainerDataSourceHandle {
    let path_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(depended_prim_path);
    let dep_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(depended_locator);
    let aff_ds: HdDataSourceBaseHandle = HdRetainedTypedSampledDataSource::new(affected_locator);

    HdRetainedContainerDataSource::from_entries(&[
        (DEPENDED_ON_PRIM_PATH.clone(), path_ds),
        (DEPENDED_ON_DS_LOCATOR.clone(), dep_ds),
        (AFFECTED_DS_LOCATOR.clone(), aff_ds),
    ])
}

/// Wraps a geometry prim's materialBindings container, redirecting to generated material.
///
/// Corresponds to C++ `_MaterialBindingsContainerDataSource`.
#[derive(Clone)]
struct MaterialBindingsContainerDs {
    input_ds: HdContainerDataSourceHandle,
    new_binding: SdfPath,
}

impl MaterialBindingsContainerDs {
    fn new(input_ds: HdContainerDataSourceHandle, new_binding: SdfPath) -> Arc<Self> {
        Arc::new(Self {
            input_ds,
            new_binding,
        })
    }
}

impl std::fmt::Debug for MaterialBindingsContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialBindingsContainerDs").finish()
    }
}

impl HdDataSourceBase for MaterialBindingsContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_ds: self.input_ds.clone(),
            new_binding: self.new_binding.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for MaterialBindingsContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        self.input_ds.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input_ds.get(name)?;
        let result_container = cast_to_container(&result)?;

        // Override "full", "", and "allPurpose" purposes
        let is_override_purpose =
            name == &*FULL || name == &*ALL_PURPOSE || name == &*ALL_PURPOSE_EMPTY;

        if is_override_purpose {
            let override_ds = HdRetainedContainerDataSource::new_1(
                PATH.clone(),
                HdRetainedTypedSampledDataSource::new(self.new_binding.clone())
                    as HdDataSourceBaseHandle,
            );
            Some(
                HdOverlayContainerDataSource::new_2(override_ds, result_container)
                    as HdDataSourceBaseHandle,
            )
        } else {
            Some(result_container as HdDataSourceBaseHandle)
        }
    }
}

/// Wraps a geometry prim's full data source, intercepting "materialBindings".
///
/// Corresponds to C++ `_BindablePrimContainerDataSource`.
#[derive(Clone)]
struct BindablePrimContainerDs {
    input_ds: HdContainerDataSourceHandle,
    new_binding: SdfPath,
}

impl BindablePrimContainerDs {
    fn new(input_ds: HdContainerDataSourceHandle, new_binding: SdfPath) -> Arc<Self> {
        Arc::new(Self {
            input_ds,
            new_binding,
        })
    }
}

impl std::fmt::Debug for BindablePrimContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindablePrimContainerDs").finish()
    }
}

impl HdDataSourceBase for BindablePrimContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(Self {
            input_ds: self.input_ds.clone(),
            new_binding: self.new_binding.clone(),
        })
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for BindablePrimContainerDs {
    fn get_names(&self) -> Vec<TfToken> {
        self.input_ds.get_names()
    }

    fn get(&self, name: &TfToken) -> Option<HdDataSourceBaseHandle> {
        let result = self.input_ds.get(name)?;
        let result_container = cast_to_container(&result)?;

        if name == &*MATERIAL_BINDINGS {
            Some(
                MaterialBindingsContainerDs::new(result_container, self.new_binding.clone())
                    as HdDataSourceBaseHandle,
            )
        } else {
            Some(result_container as HdDataSourceBaseHandle)
        }
    }
}

// ---- bookkeeping data structures ----

/// Data about a generated material prim.
#[derive(Clone)]
struct MaterialData {
    /// Original material path this was generated from
    original_material_path: SdfPath,
    /// Geometry prims that use this generated material
    bound_prims: HashSet<SdfPath>,
}

/// Data about a geometry prim using a generated material.
#[derive(Clone)]
struct PrimData {
    /// Path to the generated material bound to this prim
    generated_material_path: SdfPath,
    /// Hash of this prim's material overrides
    material_override_hash: u64,
}

/// Snapshot of PrimData for use during mutable operations.
#[allow(dead_code)]
struct PrimDataSnapshot {
    generated_material_path: SdfPath,
    material_override_hash: u64,
}

// ---- scene index ----

/// Hydra scene index that resolves material override data sources.
///
/// When a geometry prim has `materialOverride` data, this scene index generates
/// a copy of the bound material and applies the overrides to the copy, leaving
/// the original material untouched for other geometry prims.
pub struct HdsiMaterialOverrideResolvingSceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
    state: Mutex<MaterialOverrideState>,
}

#[derive(Default)]
struct MaterialOverrideState {
    /// materialScopePath -> set of generated materials under it
    scope_to_new_material_paths: HashMap<SdfPath, HashSet<SdfPath>>,
    /// originalMaterialPath -> set of generated materials derived from it
    old_to_new_material_paths: HashMap<SdfPath, HashSet<SdfPath>>,
    /// geometryPrimPath -> PrimData
    prim_data: HashMap<SdfPath, PrimData>,
    /// generatedMaterialPath -> MaterialData
    material_data: HashMap<SdfPath, MaterialData>,
    /// originalMaterialPath -> { overrideHash -> generatedMaterialPath }
    material_hash_map: HashMap<SdfPath, HashMap<u64, SdfPath>>,
}

impl HdsiMaterialOverrideResolvingSceneIndex {
    /// Creates a new material override resolving scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer_arc = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
            state: Mutex::new(MaterialOverrideState::default()),
        }));

        let filtering_observer =
            FilteringSceneIndexObserver::new(Arc::downgrade(&observer_arc)
                as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>);
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }

        observer_arc
    }

    // ---- private helpers ----

    /// Get materialOverride schema from the prim at primPath in the input scene.
    fn get_material_overrides(&self, prim_path: &SdfPath) -> Option<HdMaterialOverrideSchema> {
        let input = self.base.get_input_scene()?;
        let prim = si_ref(&input).get_prim(prim_path);
        let ds = prim.data_source?;
        // Check materialOverride key exists
        ds.get(&*MATERIAL_OVERRIDE)?;
        Some(HdMaterialOverrideSchema::get_from_parent(&ds))
    }

    /// Get path of material bound to prim for 'full' or 'allPurpose' purpose.
    fn get_bound_material(&self, prim_path: &SdfPath) -> Option<SdfPath> {
        let input = self.base.get_input_scene()?;
        let prim = si_ref(&input).get_prim(prim_path);
        let ds = prim.data_source?;

        let bindings = HdMaterialBindingsSchema::get_from_parent(&ds);
        if !bindings.is_defined() {
            return None;
        }

        for purpose in &[&*FULL, &*ALL_PURPOSE_EMPTY, &*ALL_PURPOSE] {
            let binding = bindings.get_material_binding(purpose);
            if let Some(path) = binding.get_path() {
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Compute a hash of a materialOverride schema using FNV-1a.
    fn compute_hash(mat_override: &HdMaterialOverrideSchema) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325u64; // FNV-64 offset basis

        fn fnv_str(h: &mut u64, s: &str) {
            for b in s.bytes() {
                *h ^= b as u64;
                *h = h.wrapping_mul(0x100000001b3);
            }
        }

        // Hash interfaceValues names and values
        let iface_vals = mat_override.get_interface_values();
        let iface_names = iface_vals.get_names();
        for name in &iface_names {
            fnv_str(&mut hash, "iv:");
            fnv_str(&mut hash, name.as_str());
            if let Some(child_ds) = iface_vals.get(name) {
                if let Some(sampled) = child_ds.as_sampled() {
                    fnv_str(&mut hash, &format!("{:?}", sampled.get_value(0.0)));
                }
            }
        }

        // Hash parameterValues node/input names and values
        let param_vals = mat_override.get_parameter_values();
        let node_names = param_vals.get_names();
        for node_name in &node_names {
            fnv_str(&mut hash, "pn:");
            fnv_str(&mut hash, node_name.as_str());
            if let Some(node_ds) = param_vals.get(node_name) {
                if let Some(node_c) = cast_to_container(&node_ds) {
                    for input_name in node_c.get_names() {
                        fnv_str(&mut hash, "in:");
                        fnv_str(&mut hash, input_name.as_str());
                        if let Some(input_ds) = node_c.get(&input_name) {
                            if let Some(sampled) = input_ds.as_sampled() {
                                fnv_str(&mut hash, &format!("{:?}", sampled.get_value(0.0)));
                            }
                        }
                    }
                }
            }
        }

        hash
    }

    /// Create or reuse a generated material for a geometry prim.
    /// Returns the generated material path, or None if not needed.
    fn add_generated_material(
        &self,
        prim_type: &TfToken,
        prim_path: &SdfPath,
    ) -> Option<SdfPath> {
        // Only geometry prims get generated materials
        if prim_type == &*MATERIAL_PRIM_TYPE {
            return None;
        }
        if !hd_prim_type_is_gprim(prim_type) {
            return None;
        }

        // Must have materialOverride
        let mat_override = self.get_material_overrides(prim_path)?;

        // Must have a bound material
        let material_path = self.get_bound_material(prim_path)?;

        // Compute override hash
        let mat_over_hash = Self::compute_hash(&mat_override);
        if mat_over_hash == 0 {
            return None;
        }

        let mut state = self.state.lock().expect("Lock poisoned");

        // Check cache
        if let Some(hash_to_mat) = state.material_hash_map.get(&material_path) {
            if let Some(gen_path) = hash_to_mat.get(&mat_over_hash).cloned() {
                if state.material_data.contains_key(&gen_path) {
                    // Cache hit: reuse generated material
                    state.prim_data.insert(
                        prim_path.clone(),
                        PrimData {
                            generated_material_path: gen_path.clone(),
                            material_override_hash: mat_over_hash,
                        },
                    );
                    if let Some(md) = state.material_data.get_mut(&gen_path) {
                        md.bound_prims.insert(prim_path.clone());
                    }
                    return Some(gen_path);
                }
            }
        }

        // Create new generated material: __MOR_<materialName>_<primName>
        let base_name = format!(
            "__MOR_{}_{}",
            material_path.get_name(),
            prim_path.get_name()
        );
        let scope_path = material_path.get_parent_path();

        let mut new_path = scope_path
            .append_child(&base_name)
            .unwrap_or(scope_path.clone());

        // Ensure uniqueness
        let mut suffix = 1u32;
        while state.material_data.contains_key(&new_path) {
            let suffixed = format!("{}{}", base_name, suffix);
            new_path = scope_path
                .append_child(&suffixed)
                .unwrap_or(scope_path.clone());
            suffix += 1;
        }

        // Update bookkeeping
        state
            .scope_to_new_material_paths
            .entry(scope_path.clone())
            .or_default()
            .insert(new_path.clone());

        state
            .old_to_new_material_paths
            .entry(material_path.clone())
            .or_default()
            .insert(new_path.clone());

        let mut bound = HashSet::new();
        bound.insert(prim_path.clone());
        state.material_data.insert(
            new_path.clone(),
            MaterialData {
                original_material_path: material_path.clone(),
                bound_prims: bound,
            },
        );

        state.prim_data.insert(
            prim_path.clone(),
            PrimData {
                generated_material_path: new_path.clone(),
                material_override_hash: mat_over_hash,
            },
        );

        state
            .material_hash_map
            .entry(material_path)
            .or_default()
            .insert(mat_over_hash, new_path.clone());

        Some(new_path)
    }

    /// Process a batch of added prim entries, generating materials where needed.
    fn add_generated_materials(&self, entries: &[AddedPrimEntry]) -> Vec<AddedPrimEntry> {
        let mut new_entries: Vec<AddedPrimEntry> = entries.to_vec();
        for entry in entries {
            if let Some(gen_path) = self.add_generated_material(&entry.prim_type, &entry.prim_path)
            {
                new_entries.push(AddedPrimEntry::new(gen_path, MATERIAL_PRIM_TYPE.clone()));
            }
        }
        new_entries
    }

    /// Get all generated materials located under a given scope path.
    fn get_generated_materials(&self, prim_path: &SdfPath) -> HashSet<SdfPath> {
        self.state
            .lock()
            .expect("Lock poisoned")
            .scope_to_new_material_paths
            .get(prim_path)
            .cloned()
            .unwrap_or_default()
    }

    /// Returns true if primPath is a material generated by this scene index.
    fn is_generated_material(&self, prim_path: &SdfPath) -> bool {
        self.state
            .lock()
            .expect("Lock poisoned")
            .material_data
            .contains_key(prim_path)
    }

    /// Populate the data source of a generated material prim.
    fn create_generated_material_data_source(
        &self,
        prim: &mut HdSceneIndexPrim,
        prim_path: &SdfPath,
    ) {
        let mat_data = match self
            .state
            .lock()
            .expect("Lock poisoned")
            .material_data
            .get(prim_path)
            .cloned()
        {
            Some(d) => d,
            None => return,
        };
        if mat_data.bound_prims.is_empty() {
            return;
        }

        let input = match self.base.get_input_scene() {
            Some(i) => i,
            None => return,
        };
        let guard = input.read();

        // Copy original material data source (static / disconnected)
        let original_prim = guard.get_prim(&mat_data.original_material_path);
        let original_ds = match original_prim.data_source {
            Some(ds) => ds,
            None => return,
        };

        let static_base = hd_make_static_copy(&(original_ds.clone() as HdDataSourceBaseHandle));
        let static_container = match static_base.and_then(|b| cast_to_container(&b)) {
            Some(c) => c,
            None => return,
        };

        // Get materialOverride from the first geom prim using this generated material
        let geom_path = mat_data.bound_prims.iter().next().unwrap();
        let geom_prim = guard.get_prim(geom_path);
        let geom_override: Option<HdContainerDataSourceHandle> =
            geom_prim.data_source.and_then(|ds| {
                ds.get(&*MATERIAL_OVERRIDE)
                    .and_then(|o| cast_to_container(&o))
            });

        // Get materialOverride from the original material (if any)
        let orig_override: Option<HdContainerDataSourceHandle> = original_ds
            .get(&*MATERIAL_OVERRIDE)
            .and_then(|o| cast_to_container(&o));

        // Overlay geom override on top of material override
        let overlayed_override: Option<HdContainerDataSourceHandle> =
            match (geom_override, orig_override) {
                (Some(a), Some(b)) => Some(HdOverlayContainerDataSource::new_2(a, b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };

        if let Some(ov_ds) = overlayed_override {
            let ov_wrapper = HdRetainedContainerDataSource::new_1(
                MATERIAL_OVERRIDE.clone(),
                ov_ds as HdDataSourceBaseHandle,
            );
            prim.data_source = Some(HdOverlayContainerDataSource::new_2(
                ov_wrapper,
                static_container,
            ));
        } else {
            prim.data_source = Some(static_container);
        }

        prim.prim_type = MATERIAL_PRIM_TYPE.clone();
    }

    /// Propagate dirty from a base material to all generated materials derived from it.
    fn dirty_base_material(
        &self,
        _prim_path: &SdfPath,
        generated_materials: &HashSet<SdfPath>,
        dirtied_paths: &mut HashSet<SdfPath>,
    ) {
        for path in generated_materials {
            dirtied_paths.insert(path.clone());
        }
    }

    /// Process a materialOverride locator dirty for a prim that didn't have overrides before.
    fn dirty_material_override_locator(
        &self,
        prim_path: &SdfPath,
        added_paths: &mut HashSet<SdfPath>,
        dirtied_paths: &mut HashSet<SdfPath>,
    ) {
        let prim_type = {
            let input = match self.base.get_input_scene() {
                Some(i) => i,
                None => return,
            };
            si_ref(&input).get_prim(prim_path).prim_type
        };

        if let Some(new_mat_path) = self.add_generated_material(&prim_type, prim_path) {
            dirtied_paths.insert(prim_path.clone());
            added_paths.insert(new_mat_path);
        }
    }

    /// Process dirty for a geometry prim that previously had material overrides.
    fn dirty_geometry(
        &self,
        entry: &DirtiedPrimEntry,
        prim_data_snap: PrimDataSnapshot,
        processed_prims: &mut HashSet<SdfPath>,
        added_paths: &mut HashSet<SdfPath>,
        dirtied_paths: &mut HashSet<SdfPath>,
        removed_paths: &mut HashSet<SdfPath>,
    ) {
        let mat_override_locator = HdDataSourceLocator::from_token(MATERIAL_OVERRIDE.clone());
        if !entry
            .dirty_locators
            .intersects_locator(&mat_override_locator)
        {
            return;
        }

        let prim_type = {
            let input = match self.base.get_input_scene() {
                Some(i) => i,
                None => return,
            };
            let prim = si_ref(&input).get_prim(&entry.prim_path);
            if !hd_prim_type_is_gprim(&prim.prim_type) {
                return;
            }
            prim.prim_type
        };

        let gen_mat = &prim_data_snap.generated_material_path;
        let siblings: Vec<SdfPath> = {
            let state = self.state.lock().expect("Lock poisoned");
            if !state.material_data.contains_key(gen_mat) {
                return;
            }
            state
                .material_data
                .get(gen_mat)
                .map(|md| md.bound_prims.iter().cloned().collect())
                .unwrap_or_default()
        };
        if siblings.is_empty() {
            return;
        }

        let mut prims_to_process: Vec<(TfToken, SdfPath)> =
            vec![(prim_type, entry.prim_path.clone())];

        for sibling in &siblings {
            if sibling == &entry.prim_path || processed_prims.contains(sibling) {
                continue;
            }
            let sibling_type = {
                let input = match self.base.get_input_scene() {
                    Some(i) => i,
                    None => continue,
                };
                si_ref(&input).get_prim(sibling).prim_type
            };
            processed_prims.insert(sibling.clone());
            prims_to_process.push((sibling_type, sibling.clone()));
        }

        // The old generated material is being replaced
        removed_paths.insert(gen_mat.clone());

        // Invalidate stale bookkeeping for the primary prim
        self.invalidate_maps(&entry.prim_path);

        // Generate new materials for each affected prim
        for (pt, path) in prims_to_process {
            if let Some(new_gen) = self.add_generated_material(&pt, &path) {
                added_paths.insert(new_gen);
            }
            dirtied_paths.insert(path);
        }
    }

    /// Invalidate bookkeeping maps for a given geometry prim.
    fn invalidate_maps(&self, prim_path: &SdfPath) {
        let mut state = self.state.lock().expect("Lock poisoned");
        let prim_data = match state.prim_data.remove(prim_path) {
            Some(d) => d,
            None => return,
        };

        let gen_mat = prim_data.generated_material_path;
        let hash = prim_data.material_override_hash;

        let mat_data = match state.material_data.remove(&gen_mat) {
            Some(d) => d,
            None => return,
        };

        let orig_mat = mat_data.original_material_path.clone();

        // Clean prim_data for all prims using this generated material
        for bound_prim in &mat_data.bound_prims {
            state.prim_data.remove(bound_prim);
        }

        // Clean hash map
        if let Some(h2m) = state.material_hash_map.get_mut(&orig_mat) {
            h2m.remove(&hash);
        }

        let scope = orig_mat.get_parent_path();
        if let Some(set) = state.scope_to_new_material_paths.get_mut(&scope) {
            set.remove(&gen_mat);
        }

        if let Some(set) = state.old_to_new_material_paths.get_mut(&orig_mat) {
            set.remove(&gen_mat);
        }
    }

    /// Process a batch of dirtied entries, generating extra entries for generated materials.
    fn dirty_generated_materials(
        &self,
        _sender: &dyn HdSceneIndexBase,
        entries: &[DirtiedPrimEntry],
    ) {
        let mut added_set: HashSet<SdfPath> = HashSet::new();
        let mut removed_set: HashSet<SdfPath> = HashSet::new();
        let mut dirtied_set: HashSet<SdfPath> = HashSet::new();
        let mut processed_set: HashSet<SdfPath> = HashSet::new();

        let mat_override_locator = HdDataSourceLocator::from_token(MATERIAL_OVERRIDE.clone());

        for entry in entries {
            if processed_set.contains(&entry.prim_path) {
                continue;
            }

            let is_base_material = self
                .state
                .lock()
                .expect("Lock poisoned")
                .old_to_new_material_paths
                .contains_key(&entry.prim_path);
            let prim_snap: Option<PrimDataSnapshot> = self
                .state
                .lock()
                .expect("Lock poisoned")
                .prim_data
                    .get(&entry.prim_path)
                    .map(|d| PrimDataSnapshot {
                        generated_material_path: d.generated_material_path.clone(),
                        material_override_hash: d.material_override_hash,
                    });

            if is_base_material {
                processed_set.insert(entry.prim_path.clone());
                let gen_mats = self
                    .state
                    .lock()
                    .expect("Lock poisoned")
                    .old_to_new_material_paths
                    .get(&entry.prim_path)
                    .cloned()
                    .unwrap_or_default();
                self.dirty_base_material(&entry.prim_path, &gen_mats, &mut dirtied_set);
            } else if let Some(snap) = prim_snap {
                processed_set.insert(entry.prim_path.clone());
                self.dirty_geometry(
                    entry,
                    snap,
                    &mut processed_set,
                    &mut added_set,
                    &mut dirtied_set,
                    &mut removed_set,
                );
            } else if entry
                .dirty_locators
                .intersects_locator(&mat_override_locator)
            {
                processed_set.insert(entry.prim_path.clone());
                self.dirty_material_override_locator(
                    &entry.prim_path,
                    &mut added_set,
                    &mut dirtied_set,
                );
            }
        }

        // Send removed
        if !removed_set.is_empty() {
            let removed: Vec<RemovedPrimEntry> =
                removed_set.into_iter().map(RemovedPrimEntry::new).collect();
            self.base.base().send_prims_removed(self, &removed);
        }

        // Send added
        if !added_set.is_empty() {
            let added: Vec<AddedPrimEntry> = added_set
                .into_iter()
                .map(|p| AddedPrimEntry::new(p, MATERIAL_PRIM_TYPE.clone()))
                .collect();
            self.base.base().send_prims_added(self, &added);
        }

        // Build combined dirty entries (original + generated)
        let mut new_entries = entries.to_vec();
        if !dirtied_set.is_empty() {
            let mat_locator = HdDataSourceLocator::from_token(MATERIAL.clone());
            let mat_bind_locator = HdDataSourceLocator::from_token(MATERIAL_BINDINGS.clone());
            let container_locator = HdDataSourceLocator::empty();

            for dirtied_path in dirtied_set {
                let mut locators = HdDataSourceLocatorSet::new();
                locators.insert(container_locator.clone());
                locators.insert(mat_locator.clone());
                locators.insert(mat_bind_locator.clone());
                new_entries.push(DirtiedPrimEntry::new(dirtied_path, locators));
            }
        }

        self.base.base().send_prims_dirtied(self, &new_entries);
    }
}

impl HdSceneIndexBase for HdsiMaterialOverrideResolvingSceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::empty()
        };

        // Populate generated material's data source
        if self.is_generated_material(prim_path) {
            self.create_generated_material_data_source(&mut prim, prim_path);
        }

        let ds = match prim.data_source.clone() {
            Some(ds) => ds,
            None => return prim,
        };

        if prim.prim_type == *MATERIAL_PRIM_TYPE {
            // Wrap material: apply override pipeline
            prim.data_source = Some(MaterialPrimContainerDs::new(ds, prim_path.clone()));
        } else if let Some(prim_data) = self
            .state
            .lock()
            .expect("Lock poisoned")
            .prim_data
            .get(prim_path)
            .cloned()
        {
            // Wrap geometry: redirect material binding to generated material
            prim.data_source = Some(BindablePrimContainerDs::new(
                ds,
                prim_data.generated_material_path.clone(),
            ));
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        let mut children = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_child_prim_paths(prim_path)
        } else {
            Vec::new()
        };

        for gen_path in self.get_generated_materials(prim_path) {
            if !children.contains(&gen_path) {
                children.push(gen_path);
            }
        }

        children
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn get_display_name(&self) -> String {
        "HdsiMaterialOverrideResolvingSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiMaterialOverrideResolvingSceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        let new_entries = self.add_generated_materials(entries);
        self.base.base().send_prims_added(self, &new_entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        let mut generated_materials_to_remove = HashSet::new();
        let mut prims_to_invalidate = HashSet::new();
        {
            let state = self.state.lock().expect("Lock poisoned");
            for entry in entries {
                for (prim_path, prim_data) in &state.prim_data {
                    if prim_path.has_prefix(&entry.prim_path) {
                        generated_materials_to_remove
                            .insert(prim_data.generated_material_path.clone());
                        prims_to_invalidate.insert(prim_path.clone());
                    }
                }
                for (original_material_path, generated_materials) in &state.old_to_new_material_paths {
                    if !original_material_path.has_prefix(&entry.prim_path) {
                        continue;
                    }
                    for generated_material_path in generated_materials {
                        generated_materials_to_remove.insert(generated_material_path.clone());
                        if let Some(material_data) = state.material_data.get(generated_material_path) {
                            prims_to_invalidate.extend(material_data.bound_prims.iter().cloned());
                        }
                    }
                }
            }
        }

        for prim_path in prims_to_invalidate {
            self.invalidate_maps(&prim_path);
        }

        self.base.forward_prims_removed(self, entries);
        if !generated_materials_to_remove.is_empty() {
            let removed_entries: Vec<RemovedPrimEntry> = generated_materials_to_remove
                .into_iter()
                .map(RemovedPrimEntry::new)
                .collect();
            self.base.base().send_prims_removed(self, &removed_entries);
        }
    }

    fn on_prims_dirtied(&self, sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.dirty_generated_materials(sender, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}
