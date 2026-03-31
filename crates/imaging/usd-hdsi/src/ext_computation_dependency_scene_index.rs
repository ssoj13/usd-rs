
//! Ext computation dependency scene index.
//!
//! Overlays __dependencies on every prim to track ext computation inputs,
//! outputs, and primvars driven by computations. Enables proper invalidation
//! when upstream computation data changes.

use once_cell::sync::Lazy;
use std::sync::Arc;
use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSourceHandle, HdDataSourceBaseHandle, HdDataSourceLocator,
    HdOverlayContainerDataSource, HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource,
};
use usd_hd::scene_index::filtering::FilteringSceneIndexObserver;
use usd_hd::scene_index::observer::*;
use usd_hd::scene_index::*;
use usd_hd::schema::{
    HdDependenciesSchema, HdDependencySchemaBuilder, HdExtComputationInputComputationSchema,
    HdExtComputationPrimvarsSchema, HdExtComputationSchema, HdLocatorDataSourceHandle,
    HdPathDataSourceHandle, HdPrimvarsSchema, PRIMVAR_VALUE,
};
use usd_sdf::Path as SdfPath;
use usd_tf::Token as TfToken;

/// Token for "__all__" (dummy locator element).
static ALL: Lazy<TfToken> = Lazy::new(|| TfToken::new("__all__"));
/// Token for "value".
static VALUE: Lazy<TfToken> = Lazy::new(|| TfToken::new("value"));

/// Token prefixes for dependency names.
static PRIMVAR_EXT_COMPUTATION_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("primvarExtComputationDependency_"));
static EXT_COMPUTATION_INPUT_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("extComputationInputDependency_"));
static EXT_COMPUTATION_OUTPUT_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("extComputationOutputDependency_"));

/// Static dependency names.
static EXT_COMPUTATION_INPUT_VALUES_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("extComputationInputValuesDependency"));
static EXT_COMPUTATION_INPUT_COMPUTATIONS_DEPENDENCIES_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("extComputationInputComputationsDependenciesDependency"));
static EXT_COMPUTATION_OUTPUTS_DEPENDENCIES_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("extComputationOutputsDependenciesDependency"));
static EXT_COMPUTATION_PRIMVARS_DEPENDENCIES_DEPENDENCY: Lazy<TfToken> =
    Lazy::new(|| TfToken::new("extComputationPrimvarsDependenciesDependency"));

/// Dummy locator: extComputation/outputs/__all__/value.
/// All outputs depend on this; we use it to funnel many dependencies.
fn all_output_values_locator() -> HdDataSourceLocator {
    HdExtComputationSchema::get_outputs_locator()
        .append(&ALL)
        .append(&VALUE)
}

/// Dependency: inputComputations changes → __dependencies invalidated.
fn ext_computation_input_computations_dependencies_dependency() -> HdDataSourceBaseHandle {
    HdDependencySchemaBuilder::default()
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdExtComputationSchema::get_input_computations_locator(),
        ) as HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdDependenciesSchema::get_default_locator(),
        ) as HdLocatorDataSourceHandle)
        .build() as HdDataSourceBaseHandle
}

/// Dependency: outputs changes → __dependencies invalidated.
fn ext_computation_outputs_dependencies_dependency() -> HdDataSourceBaseHandle {
    HdDependencySchemaBuilder::default()
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdExtComputationSchema::get_outputs_locator(),
        ) as HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdDependenciesSchema::get_default_locator(),
        ) as HdLocatorDataSourceHandle)
        .build() as HdDataSourceBaseHandle
}

/// Dependency: all outputs depend on inputValues (via dummy locator).
fn ext_computation_input_values_dependency() -> HdDataSourceBaseHandle {
    HdDependencySchemaBuilder::default()
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdExtComputationSchema::get_input_values_locator(),
        ) as HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            all_output_values_locator(),
        ) as HdLocatorDataSourceHandle)
        .build() as HdDataSourceBaseHandle
}

/// Dependency: extComputationPrimvars changes → __dependencies invalidated.
fn ext_computation_primvars_dependencies_dependency() -> HdDataSourceBaseHandle {
    HdDependencySchemaBuilder::default()
        .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdExtComputationPrimvarsSchema::get_default_locator(),
        ) as HdLocatorDataSourceHandle)
        .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
            HdDependenciesSchema::get_default_locator(),
        ) as HdLocatorDataSourceHandle)
        .build() as HdDataSourceBaseHandle
}

/// Build __dependencies for an extComputation prim.
fn build_ext_computation_dependencies(
    prim_source: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let comp_schema = HdExtComputationSchema::get_from_parent(prim_source);

    let mut names: Vec<TfToken> = vec![EXT_COMPUTATION_INPUT_VALUES_DEPENDENCY.clone()];
    let mut sources: Vec<HdDataSourceBaseHandle> = vec![ext_computation_input_values_dependency()];

    if let Some(input_computations) = comp_schema.get_input_computations() {
        for input_name in input_computations.get_names() {
            let input_schema = HdExtComputationInputComputationSchema::get_from_parent(
                &input_computations,
                &input_name,
            );
            let source_computation: HdPathDataSourceHandle =
                match input_schema.get_source_computation() {
                    Some(ds) => ds,
                    None => continue,
                };
            let source_output_name: Arc<
                dyn usd_hd::data_source::HdTypedSampledDataSource<TfToken>,
            > = match input_schema.get_source_computation_output_name() {
                Some(ds) => ds,
                None => continue,
            };

            let output_name_token = source_output_name.get_typed_value(0.0);
            let depended_on_locator = HdExtComputationSchema::get_outputs_locator()
                .append(&output_name_token)
                .append(&VALUE);

            let dep_name = TfToken::new(&format!(
                "{}{}",
                EXT_COMPUTATION_INPUT_DEPENDENCY.as_str(),
                input_name.as_str()
            ));
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::new(
                    source_computation.get_typed_value(0.0),
                ) as HdPathDataSourceHandle)
                .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    depended_on_locator,
                ) as HdLocatorDataSourceHandle)
                .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    all_output_values_locator(),
                ) as HdLocatorDataSourceHandle)
                .build();
            names.push(dep_name);
            sources.push(dep as HdDataSourceBaseHandle);
        }
    }

    if let Some(outputs) = comp_schema.get_outputs() {
        for output_name in outputs.get_names() {
            let affected_locator = HdExtComputationSchema::get_outputs_locator()
                .append(&output_name)
                .append(&VALUE);
            let dep_name = TfToken::new(&format!(
                "{}{}",
                EXT_COMPUTATION_OUTPUT_DEPENDENCY.as_str(),
                output_name.as_str()
            ));
            let dep = HdDependencySchemaBuilder::default()
                .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    all_output_values_locator(),
                ) as HdLocatorDataSourceHandle)
                .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                    affected_locator,
                ) as HdLocatorDataSourceHandle)
                .build();
            names.push(dep_name);
            sources.push(dep as HdDataSourceBaseHandle);
        }
    }

    names.push(EXT_COMPUTATION_INPUT_COMPUTATIONS_DEPENDENCIES_DEPENDENCY.clone());
    sources.push(ext_computation_input_computations_dependencies_dependency());
    names.push(EXT_COMPUTATION_OUTPUTS_DEPENDENCIES_DEPENDENCY.clone());
    sources.push(ext_computation_outputs_dependencies_dependency());

    HdDependenciesSchema::build_retained(&names, &sources)
}

/// Build __dependencies for a prim with ext computation primvars.
fn build_primvar_dependencies(
    prim_source: &HdContainerDataSourceHandle,
) -> HdContainerDataSourceHandle {
    let comp_primvars = HdExtComputationPrimvarsSchema::get_from_parent(prim_source);

    if !comp_primvars.is_defined() {
        return HdRetainedContainerDataSource::from_arrays(
            &[EXT_COMPUTATION_PRIMVARS_DEPENDENCIES_DEPENDENCY.clone()],
            &[ext_computation_primvars_dependencies_dependency()],
        );
    }

    let mut names: Vec<TfToken> = Vec::new();
    let mut sources: Vec<HdDataSourceBaseHandle> = Vec::new();

    for pv_name in comp_primvars.get_ext_computation_primvar_names() {
        let comp_primvar = comp_primvars.get_ext_computation_primvar(&pv_name);
        let source_computation: HdPathDataSourceHandle = match comp_primvar.get_source_computation()
        {
            Some(ds) => ds,
            None => continue,
        };
        let source_output_name: Arc<dyn usd_hd::data_source::HdTypedSampledDataSource<TfToken>> =
            match comp_primvar.get_source_computation_output_name() {
                Some(ds) => ds,
                None => continue,
            };

        let output_name_token = source_output_name.get_typed_value(0.0);
        let depended_on_locator = HdExtComputationSchema::get_outputs_locator()
            .append(&output_name_token)
            .append(&VALUE);

        let affected_locator = HdPrimvarsSchema::get_default_locator()
            .append(&pv_name)
            .append(&PRIMVAR_VALUE);

        let dep_name = TfToken::new(&format!(
            "{}{}",
            PRIMVAR_EXT_COMPUTATION_DEPENDENCY.as_str(),
            pv_name.as_str()
        ));
        let dep = HdDependencySchemaBuilder::default()
            .set_depended_on_prim_path(HdRetainedTypedSampledDataSource::new(
                source_computation.get_typed_value(0.0),
            ) as HdPathDataSourceHandle)
            .set_depended_on_data_source_locator(HdRetainedTypedSampledDataSource::new(
                depended_on_locator,
            ) as HdLocatorDataSourceHandle)
            .set_affected_data_source_locator(HdRetainedTypedSampledDataSource::new(
                affected_locator,
            ) as HdLocatorDataSourceHandle)
            .build();
        names.push(dep_name);
        sources.push(dep as HdDataSourceBaseHandle);
    }

    names.push(EXT_COMPUTATION_PRIMVARS_DEPENDENCIES_DEPENDENCY.clone());
    sources.push(ext_computation_primvars_dependencies_dependency());

    HdDependenciesSchema::build_retained(&names, &sources)
}

fn build_dependencies(prim: &HdSceneIndexPrim) -> HdContainerDataSourceHandle {
    if prim.prim_type == *usd_hd::tokens::SPRIM_EXT_COMPUTATION {
        if let Some(ref ds) = prim.data_source {
            return build_ext_computation_dependencies(ds);
        }
    }
    if let Some(ref ds) = prim.data_source {
        return build_primvar_dependencies(ds);
    }
    HdRetainedContainerDataSource::new_empty()
}

/// ExtComputation dependency scene index.
///
/// Overlays __dependencies on every prim so clients can correctly invalidate
/// when ext computation inputs, outputs, or primvars change.
pub struct HdsiExtComputationDependencySceneIndex {
    base: HdSingleInputFilteringSceneIndexBase,
}

impl HdsiExtComputationDependencySceneIndex {
    /// Creates a new ExtComputation dependency tracking scene index.
    pub fn new(input_scene: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        let observer = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input_scene.clone())),
        }));
        let filtering_observer = FilteringSceneIndexObserver::new(
            Arc::downgrade(&observer) as std::sync::Weak<RwLock<dyn FilteringObserverTarget>>
        );
        {
            input_scene.read().add_observer(Arc::new(filtering_observer));
        }
        observer
    }
}

impl HdSceneIndexBase for HdsiExtComputationDependencySceneIndex {
    fn get_prim(&self, prim_path: &SdfPath) -> HdSceneIndexPrim {
        let mut prim = if let Some(input) = self.base.get_input_scene() {
            si_ref(&input).get_prim(prim_path)
        } else {
            HdSceneIndexPrim::default()
        };

        if let Some(ref data_source) = prim.data_source {
            let deps = build_dependencies(&prim);
            let deps_token = (*HdDependenciesSchema::get_schema_token()).clone();
            let overlay = HdRetainedContainerDataSource::from_arrays(
                &[deps_token],
                &[deps as HdDataSourceBaseHandle],
            );
            prim.data_source = Some(HdOverlayContainerDataSource::new_2(
                overlay,
                data_source.clone(),
            ));
        }

        prim
    }

    fn get_child_prim_paths(&self, prim_path: &SdfPath) -> SdfPathVector {
        if let Some(input) = self.base.get_input_scene() {
            return si_ref(&input).get_child_prim_paths(prim_path);
        }
        Vec::new()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn _system_message(&self, _message_type: &TfToken, _args: Option<HdDataSourceBaseHandle>) {}

    fn get_display_name(&self) -> String {
        "HdsiExtComputationDependencySceneIndex".to_string()
    }
}

impl FilteringObserverTarget for HdsiExtComputationDependencySceneIndex {
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
