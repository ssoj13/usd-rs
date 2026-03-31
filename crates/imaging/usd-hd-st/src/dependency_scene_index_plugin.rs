
//! HdSt_DependencySceneIndexPlugin — Storm dependency declaration plugin.
//!
//! Adds a filtering scene index that overlays `__dependencies` onto mesh and
//! volume prims so the dependency-forwarding scene index can propagate the
//! right invalidation signals downstream.
//!
//! Two dependency groups are declared:
//!
//! 1. **Mesh prims** — `materialBindings` depends on the bound material's
//!    `material` locator.  This ensures that when the material changes (e.g.
//!    a ptex texture is added/removed) the mesh re-evaluates whether it needs
//!    quadrangulation.
//!
//!    Two entries are written:
//!    - `storm_materialToMaterialBindings` — prim P's `materialBindings`
//!      is invalidated when `material` on the bound material prim changes.
//!    - `storm_materialBindingsToDependency` — the dependency entry itself
//!      (`__dependencies/storm_materialToMaterialBindings`) is invalidated
//!      when `materialBindings` on the same prim changes (so we re-compute
//!      the dependency if the binding target changes).
//!
//! 2. **Volume prims** — for every field listed in `volumeFieldBinding`,
//!    `volumeFieldBinding` is invalidated when `volumeField` on the target
//!    field prim changes (e.g. filePath update → texture rebind).
//!
//!    Additionally a self-referential dependency keeps `__dependencies` fresh
//!    whenever `volumeFieldBinding` itself changes.
//!
//! Insertion phase 100 — runs before HdSt_DependencyForwardingSceneIndexPlugin
//! (phase 1000).
//!
//! Port of C++ `HdSt_DependencySceneIndexPlugin`.

use std::sync::Arc;
use parking_lot::RwLock;

use once_cell::sync::Lazy;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdLocatorDataSourceHandle, HdMapContainerDataSource, HdOverlayContainerDataSource,
    HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource, cast_to_container,
};
use usd_hd::scene_index::{
    AddedPrimEntry, DirtiedPrimEntry, FilteringObserverTarget, HdSceneIndexBase,
    HdSceneIndexHandle, HdSceneIndexObserverHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, RemovedPrimEntry, RenamedPrimEntry, SdfPathVector,
    si_ref,
};
// Schemas are re-exported flat from usd_hd::schema — sub-modules are private.
use usd_hd::schema::{
    HdDependenciesSchema, HdDependencySchemaBuilder, HdMaterialBindingsSchema, HdMaterialSchema,
    HdVolumeFieldBindingSchema, HdVolumeFieldSchema, MATERIAL_BINDING_ALL_PURPOSE,
};
use usd_hd::tokens::{RPRIM_MESH, RPRIM_VOLUME};
use usd_sdf::Path as SdfPath;
use usd_tf::Token;

// ---------------------------------------------------------------------------
// Plugin constants
// ---------------------------------------------------------------------------

/// Insertion phase: before dependency forwarding (phase 1000).
pub const INSERTION_PHASE: u32 = 100;

/// Storm plugin display name (renderer identifier).
pub const PLUGIN_DISPLAY_NAME: &str = "GL";

// ---------------------------------------------------------------------------
// Private tokens — mirror C++ TF_DEFINE_PRIVATE_TOKENS
// ---------------------------------------------------------------------------

/// Key for the mesh material→materialBindings dependency entry.
static STORM_MATERIAL_TO_MATERIAL_BINDINGS: Lazy<Token> =
    Lazy::new(|| Token::new("storm_materialToMaterialBindings"));

/// Key for the self-referential materialBindings→dependency entry.
static STORM_MATERIAL_BINDINGS_TO_DEPENDENCY: Lazy<Token> =
    Lazy::new(|| Token::new("storm_materialBindingsToDependency"));

/// Key for the volume volumeFieldBinding→dependency entry.
static STORM_VOLUME_FIELD_BINDING_TO_DEPENDENCY: Lazy<Token> =
    Lazy::new(|| Token::new("storm_volumeFieldBindingToDependency"));

// ---------------------------------------------------------------------------
// Static retained locator data sources
// (mirrors C++ static locals — created once, reused on every call)
// ---------------------------------------------------------------------------

/// Retained locator DS for `material` (the whole material data source).
static MATERIAL_LOC_DS: Lazy<HdLocatorDataSourceHandle> =
    Lazy::new(|| HdRetainedTypedSampledDataSource::new(HdMaterialSchema::get_default_locator()));

/// Retained locator DS for `materialBindings`.
static MATERIAL_BINDINGS_LOC_DS: Lazy<HdLocatorDataSourceHandle> = Lazy::new(|| {
    HdRetainedTypedSampledDataSource::new(HdMaterialBindingsSchema::get_default_locator())
});

/// Retained locator DS for `volumeField` (whole volumeField data source).
static VOLUME_FIELD_LOC_DS: Lazy<HdLocatorDataSourceHandle> =
    Lazy::new(|| HdRetainedTypedSampledDataSource::new(HdVolumeFieldSchema::get_default_locator()));

/// Retained locator DS for `volumeFieldBinding`.
static VOLUME_FIELD_BINDING_LOC_DS: Lazy<HdLocatorDataSourceHandle> = Lazy::new(|| {
    HdRetainedTypedSampledDataSource::new(HdVolumeFieldBindingSchema::get_default_locator())
});

// ---------------------------------------------------------------------------
// Helper: retrieve existing __dependencies container from a prim data source
// ---------------------------------------------------------------------------

/// Extract the `__dependencies` container from a prim's data source if present.
///
/// Mirrors the C++ pattern of `HdDependenciesSchema::GetFromParent(ds).GetContainer()`.
/// Because `HdDependenciesSchema::get_container()` is not exposed in our Rust API
/// (the inner `schema` field is private), we retrieve the raw child directly.
fn get_existing_dependencies(
    prim_ds: &HdContainerDataSourceHandle,
) -> Option<HdContainerDataSourceHandle> {
    let schema_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
    let child = prim_ds.get(&schema_token)?;
    cast_to_container(&child)
}

// ---------------------------------------------------------------------------
// compute_material_bindings_dependency
// ---------------------------------------------------------------------------

/// Build the `__dependencies` sub-container for a mesh prim.
///
/// Mirrors C++ `_ComputeMaterialBindingsDependency`.
///
/// Returns up to two dependency entries:
/// - `storm_materialToMaterialBindings` (only when the prim has a non-empty
///   bound material path): the bound material prim's `material` locator
///   invalidates this prim's `materialBindings`.
/// - `storm_materialBindingsToDependency`: this prim's `materialBindings`
///   invalidates `__dependencies/storm_materialToMaterialBindings` (keeps
///   the dep entry fresh when the binding target changes).
fn compute_material_bindings_dependency(
    prim_ds: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let material_bindings = HdMaterialBindingsSchema::get_from_parent(prim_ds);

    let mut names: Vec<Token> = Vec::with_capacity(2);
    let mut data_sources: Vec<HdDataSourceBaseHandle> = Vec::with_capacity(2);

    // Entry 1: materialBindings depends on the bound material's `material`
    // data source. HdsiMaterialBindingResolvingSceneIndex has already run, so
    // get_material_binding(allPurpose) returns the resolved all-purpose path.
    if let Some(bound_path) = material_bindings
        .get_material_binding(&MATERIAL_BINDING_ALL_PURPOSE)
        .get_path()
    {
        if !bound_path.is_empty() {
            let path_ds: Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<SdfPath>> =
                HdRetainedTypedSampledDataSource::new(bound_path);

            let dep_ds = HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(path_ds)
                .set_depended_on_data_source_locator(MATERIAL_LOC_DS.clone())
                .set_affected_data_source_locator(MATERIAL_BINDINGS_LOC_DS.clone())
                .build();

            names.push(STORM_MATERIAL_TO_MATERIAL_BINDINGS.clone());
            data_sources.push(dep_ds);
        }
    }

    // Entry 2: `__dependencies/storm_materialToMaterialBindings` is itself
    // invalidated when `materialBindings` on this prim changes, so the
    // dependency is re-evaluated if the bound material target changes.
    {
        let dep_entry_locator = HdDependenciesSchema::get_default_locator()
            .append(&STORM_MATERIAL_TO_MATERIAL_BINDINGS);

        let dep_entry_loc_ds: HdLocatorDataSourceHandle =
            HdRetainedTypedSampledDataSource::new(dep_entry_locator);

        let dep_ds = HdDependencySchemaBuilder::default()
            // No depended_on_prim_path → depends on this same prim.
            .set_depended_on_data_source_locator(MATERIAL_BINDINGS_LOC_DS.clone())
            .set_affected_data_source_locator(dep_entry_loc_ds)
            .build();

        names.push(STORM_MATERIAL_BINDINGS_TO_DEPENDENCY.clone());
        data_sources.push(dep_ds);
    }

    HdRetainedContainerDataSource::from_arrays(&names, &data_sources)
}

// ---------------------------------------------------------------------------
// compute_volume_field_dependency / compute_volume_field_binding_dependencies
// ---------------------------------------------------------------------------

/// Build a single dependency DS for one volume field binding entry.
///
/// `path_ds` is the raw base data source stored under a field name in the
/// `volumeFieldBinding` container — it is an `HdPathDataSource` (typed on
/// `SdfPath`).  The resulting dependency says: `volumeFieldBinding` on this
/// volume prim is invalidated when `volumeField` on the *target* field prim
/// changes (e.g. its filePath changes → texture rebind needed).
///
/// Mirrors C++ `_ComputeVolumeFieldDependency`.
fn compute_volume_field_dependency(path_ds: &HdDataSourceBaseHandle) -> HdDataSourceBaseHandle {
    // Try to recover the typed path DS so we can pass it to the builder.
    let typed_path_ds: Option<Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<SdfPath>>> =
        path_ds
            .as_any()
            .downcast_ref::<HdRetainedTypedSampledDataSource<SdfPath>>()
            .map(|ds| {
                let arc: Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<SdfPath>> =
                    Arc::new(ds.clone());
                arc
            });

    let dep_ds = if let Some(p) = typed_path_ds {
        HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(p)
            .set_depended_on_data_source_locator(VOLUME_FIELD_LOC_DS.clone())
            .set_affected_data_source_locator(VOLUME_FIELD_BINDING_LOC_DS.clone())
            .build()
    } else {
        // Path DS type unknown — emit dependency without prim path (graceful).
        HdDependencySchemaBuilder::default()
            .set_depended_on_data_source_locator(VOLUME_FIELD_LOC_DS.clone())
            .set_affected_data_source_locator(VOLUME_FIELD_BINDING_LOC_DS.clone())
            .build()
    };

    dep_ds as HdDataSourceBaseHandle
}

/// Map `compute_volume_field_dependency` over every child of the
/// `volumeFieldBinding` container, producing a container keyed by field name
/// where each value is the corresponding dependency DS.
///
/// Mirrors C++ `_ComputeVolumeFieldBindingDependencies`.
fn compute_volume_field_binding_dependencies(
    prim_ds: &HdContainerDataSourceHandle,
) -> Option<HdContainerDataSourceHandle> {
    let binding_schema = HdVolumeFieldBindingSchema::get_from_parent(prim_ds);
    let binding_container = binding_schema.get_container()?.clone();

    // HdMapContainerDataSource applies the function to every child.
    Some(HdMapContainerDataSource::new(
        compute_volume_field_dependency,
        binding_container,
    ))
}

/// Build the self-referential dependency that keeps `__dependencies` fresh
/// whenever `volumeFieldBinding` itself changes (e.g. a field is added or
/// removed from the volume prim).
///
/// Mirrors C++ `_ComputeVolumeFieldBindingDependencyDependencies`.
fn compute_volume_field_binding_dependency_dependencies() -> HdContainerDataSourceHandle {
    // `__dependencies` (affected) depends on `volumeFieldBinding` (depended-on),
    // both on this same prim.
    let deps_loc_ds: HdLocatorDataSourceHandle =
        HdRetainedTypedSampledDataSource::new(HdDependenciesSchema::get_default_locator());

    let dep_ds = HdDependencySchemaBuilder::default()
        .set_depended_on_data_source_locator(VOLUME_FIELD_BINDING_LOC_DS.clone())
        .set_affected_data_source_locator(deps_loc_ds)
        .build();

    HdRetainedContainerDataSource::new_1(STORM_VOLUME_FIELD_BINDING_TO_DEPENDENCY.clone(), dep_ds)
}

// ---------------------------------------------------------------------------
// PrimDataSource
// ---------------------------------------------------------------------------

/// Wraps an input prim data source and overlays the `__dependencies` key.
///
/// Mirrors C++ `_PrimDataSource`: intercepts `get(__dependencies)` and
/// `get_names()` to inject the Storm dependency declarations, passing all
/// other keys through to the upstream data source unchanged.
#[derive(Clone)]
struct PrimDataSource {
    /// The upstream prim data source.
    input: HdContainerDataSourceHandle,
    /// Prim type — determines which dependency group to emit.
    prim_type: Token,
}

impl std::fmt::Debug for PrimDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDataSource")
            .field("prim_type", &self.prim_type)
            .finish()
    }
}

impl PrimDataSource {
    /// Wrap a prim's data source. Returns None if the prim has no data source.
    fn new(prim: &HdSceneIndexPrim) -> Option<Arc<Self>> {
        let ds = prim.data_source.clone()?;
        Some(Arc::new(Self {
            input: ds,
            prim_type: prim.prim_type.clone(),
        }))
    }

    /// Compute and return the merged `__dependencies` container, or None.
    ///
    /// Mirrors C++ `_PrimDataSource::_GetDependencies`:
    /// - Start with existing `__dependencies` from upstream (if any).
    /// - For mesh prims: overlay material-binding dependencies.
    /// - For volume prims: overlay per-field + self-referential dependencies.
    /// - Merge all sources via HdOverlayContainerDataSource.
    fn get_dependencies(&self) -> Option<HdContainerDataSourceHandle> {
        let mut sources: Vec<HdContainerDataSourceHandle> = Vec::with_capacity(3);

        // Pass through any existing __dependencies from the upstream source.
        if let Some(existing) = get_existing_dependencies(&self.input) {
            sources.push(existing);
        }

        // Mesh: add materialBindings↔material dependency declarations.
        if self.prim_type == *RPRIM_MESH {
            sources.push(compute_material_bindings_dependency(&self.input));
        }

        // Volume: per-field binding deps + self-referential dep on the dep container.
        if self.prim_type == *RPRIM_VOLUME {
            if let Some(field_deps) = compute_volume_field_binding_dependencies(&self.input) {
                sources.push(field_deps);
            }
            sources.push(compute_volume_field_binding_dependency_dependencies());
        }

        match sources.len() {
            0 => None,
            1 => sources.into_iter().next(),
            _ => Some(HdOverlayContainerDataSource::new(sources)),
        }
    }
}

impl HdDataSourceBase for PrimDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(PrimDataSource {
            input: self.input.clone(),
            prim_type: self.prim_type.clone(),
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<usd_hd::HdContainerDataSourceHandle> {
        Some(std::sync::Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimDataSource {
    /// Union of upstream names plus `__dependencies` when needed.
    ///
    /// Mirrors C++ `_PrimDataSource::GetNames`: adds `__dependencies` only
    /// when the prim is a volume or has materialBindings.
    fn get_names(&self) -> Vec<Token> {
        let mut names = self.input.get_names();

        // Inject __dependencies for volumes and for prims with materialBindings.
        let needs_deps = self.prim_type == *RPRIM_VOLUME
            || HdMaterialBindingsSchema::get_from_parent(&self.input).is_defined();

        if needs_deps {
            let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
            if !names.contains(&dep_token) {
                names.push(dep_token);
            }
        }

        names
    }

    /// Intercept `__dependencies`; forward all other keys to the input.
    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if name == &**HdDependenciesSchema::get_schema_token() {
            // Return the merged dependency container (None if nothing to contribute).
            let deps = self.get_dependencies()?;
            Some(deps as HdDataSourceBaseHandle)
        } else {
            self.input.get(name)
        }
    }
}

// ---------------------------------------------------------------------------
// HdStDependencySceneIndex
// ---------------------------------------------------------------------------

/// Filtering scene index that overlays Storm dependency declarations.
///
/// Wraps every prim that has a data source in a `PrimDataSource` which
/// transparently adds `__dependencies` for mesh and volume prims.
///
/// Mirrors C++ `_SceneIndex`.
pub struct HdStDependencySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdStDependencySceneIndex {
    /// Create a new dependency scene index wrapping the given input.
    pub fn new(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(input_scene),
        }))
    }
}

impl HdSceneIndexBase for HdStDependencySceneIndex {
    /// Wrap the upstream prim's data source with PrimDataSource.
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::empty()
        };

        // Overlay PrimDataSource only when upstream has a data source.
        if prim.data_source.is_some() {
            if let Some(wrapped) = PrimDataSource::new(&prim) {
                prim.data_source = Some(wrapped);
            }
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            { let lock = input.read();
                return lock.get_child_prim_paths(prim_path);
            }
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn get_display_name(&self) -> String {
        // Match C++ SetDisplayName("HdSt: declare Storm dependencies").
        "HdSt: declare Storm dependencies".to_string()
    }

    fn get_input_scenes_for_system_message(&self) -> Vec<HdSceneIndexHandle> {
        self.base.get_input_scene().cloned().into_iter().collect()
    }
}

impl FilteringObserverTarget for HdStDependencySceneIndex {
    fn on_prims_added(&self, _sender: &dyn HdSceneIndexBase, entries: &[AddedPrimEntry]) {
        self.base.forward_prims_added(self, entries);
    }

    fn on_prims_removed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RemovedPrimEntry]) {
        self.base.forward_prims_removed(self, entries);
    }

    fn on_prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, entries: &[DirtiedPrimEntry]) {
        self.base.forward_prims_dirtied(self, entries);
    }

    fn on_prims_renamed(&self, _sender: &dyn HdSceneIndexBase, entries: &[RenamedPrimEntry]) {
        self.base.forward_prims_renamed(self, entries);
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/// Create the Storm dependency scene index wrapping `input_scene`.
pub fn create(input_scene: Option<HdSceneIndexHandle>) -> Arc<RwLock<HdStDependencySceneIndex>> {
    HdStDependencySceneIndex::new(input_scene)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use usd_hd::data_source::{HdRetainedContainerDataSource, cast_to_container};
    use usd_hd::schema::HdDependenciesSchema;

    // Minimal stub scene index that returns a single hard-wired prim.
    struct StubSceneIndex {
        prim: HdSceneIndexPrim,
    }

    impl HdSceneIndexBase for StubSceneIndex {
        fn get_prim(&self, _path: &SdfPath) -> HdSceneIndexPrim {
            self.prim.clone()
        }
        fn get_child_prim_paths(&self, _path: &SdfPath) -> SdfPathVector {
            Vec::new()
        }
        fn add_observer(&self, _o: HdSceneIndexObserverHandle) {}
        fn remove_observer(&self, _o: &HdSceneIndexObserverHandle) {}
        fn get_display_name(&self) -> String {
            "StubSceneIndex".to_string()
        }
    }

    /// Build an input scene index returning an empty-DS prim of the given type.
    fn make_input(prim_type: &str) -> Option<HdSceneIndexHandle> {
        let ds: HdContainerDataSourceHandle = HdRetainedContainerDataSource::new_empty();
        let prim = HdSceneIndexPrim::new(Token::new(prim_type), Some(ds));
        Some(Arc::new(RwLock::new(StubSceneIndex { prim })))
    }

    /// Build an input mesh prim that has a `materialBindings` container in its
    /// data source, so `HdMaterialBindingsSchema::is_defined()` returns true.
    /// This matches the condition C++ `GetNames()` checks to inject __dependencies.
    fn make_mesh_with_material_bindings() -> Option<HdSceneIndexHandle> {
        use usd_hd::schema::MATERIAL_BINDINGS;
        let mat_bindings_ds: HdDataSourceBaseHandle = HdRetainedContainerDataSource::new_empty();
        let ds = HdRetainedContainerDataSource::new_1(MATERIAL_BINDINGS.clone(), mat_bindings_ds);
        let prim = HdSceneIndexPrim::new(Token::new("mesh"), Some(ds));
        Some(Arc::new(RwLock::new(StubSceneIndex { prim })))
    }

    #[test]
    fn test_constants() {
        assert_eq!(INSERTION_PHASE, 100);
        assert_eq!(PLUGIN_DISPLAY_NAME, "GL");
    }

    #[test]
    fn test_display_name() {
        let si = create(None);
        let lock = si.read();
        assert_eq!(lock.get_display_name(), "HdSt: declare Storm dependencies");
    }

    #[test]
    fn test_mesh_prim_gets_dependencies_key() {
        // C++ GetNames() adds __dependencies only when materialBindings is present.
        // Use a mesh DS that contains the materialBindings child.
        let si = create(make_mesh_with_material_bindings());
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        assert!(prim.is_defined(), "mesh prim must be defined");

        let ds = prim.data_source.unwrap();
        let names = ds.get_names();
        let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
        assert!(
            names.contains(&dep_token),
            "__dependencies must appear in mesh names when materialBindings is present"
        );
    }

    #[test]
    fn test_mesh_without_material_bindings_no_dep_in_names() {
        // A plain mesh with no materialBindings in its DS must NOT get
        // __dependencies injected into get_names() — matches C++ GetNames().
        let si = create(make_input("mesh"));
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        assert!(prim.is_defined());

        let ds = prim.data_source.unwrap();
        let names = ds.get_names();
        let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
        assert!(
            !names.contains(&dep_token),
            "mesh without materialBindings must not expose __dependencies in get_names()"
        );
    }

    #[test]
    fn test_volume_prim_gets_dependencies_key() {
        let si = create(make_input("volume"));
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        assert!(prim.is_defined(), "volume prim must be defined");

        let ds = prim.data_source.unwrap();
        let names = ds.get_names();
        let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
        assert!(
            names.contains(&dep_token),
            "__dependencies must appear in volume prim names"
        );
    }

    #[test]
    fn test_generic_prim_no_extra_dependencies() {
        // A points prim has no materialBindings and no volumeFieldBinding —
        // it must not gain __dependencies unless upstream already had them.
        let si = create(make_input("points"));
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        assert!(prim.is_defined());

        let ds = prim.data_source.unwrap();
        let names = ds.get_names();
        let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
        assert!(
            !names.contains(&dep_token),
            "points prim must not gain __dependencies"
        );
    }

    #[test]
    fn test_empty_prim_passed_through() {
        // get_prim returning an undefined prim must not panic.
        let si = create(None); // NoOp fallback
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        assert!(!prim.is_defined());
    }

    #[test]
    fn test_volume_dependencies_content() {
        // Volume with empty volumeFieldBinding: __dependencies must contain
        // storm_volumeFieldBindingToDependency at minimum.
        let si = create(make_input("volume"));
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        let ds = prim.data_source.unwrap();

        let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
        let deps_base = ds.get(&dep_token).expect("__dependencies must be present");

        if let Some(deps_container) = cast_to_container(&deps_base) {
            let dep_names = deps_container.get_names();
            assert!(
                dep_names.contains(&*STORM_VOLUME_FIELD_BINDING_TO_DEPENDENCY),
                "storm_volumeFieldBindingToDependency must be present"
            );
        }
    }

    #[test]
    fn test_mesh_dependencies_content() {
        // Mesh with empty DS (no materialBindings) still gets the
        // storm_materialBindingsToDependency self-referential entry.
        let si = create(make_input("mesh"));
        let lock = si.read();
        let prim = lock.get_prim(&SdfPath::absolute_root());
        let ds = prim.data_source.unwrap();

        let dep_token: Token = (**HdDependenciesSchema::get_schema_token()).clone();
        let deps_base = ds.get(&dep_token).expect("__dependencies must be present");

        if let Some(deps_container) = cast_to_container(&deps_base) {
            let dep_names = deps_container.get_names();
            assert!(
                dep_names.contains(&*STORM_MATERIAL_BINDINGS_TO_DEPENDENCY),
                "storm_materialBindingsToDependency must be present for mesh"
            );
        }
    }
}
