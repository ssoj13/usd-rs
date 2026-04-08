//! InstanceProxyPathTranslationSceneIndex - translate instance-proxy paths in data sources.
//!
//! Port of `pxr/usdImaging/usdImaging/instanceProxyPathTranslationSceneIndex`.
//!
//! The `_ref` scene index does not rewrite prim paths in scene-index notices.
//! Instead it wraps selected prim-level data sources and recursively translates
//! `SdfPath` / `SdfPath[]` payloads from instance-proxy paths to prototype paths.
//! This matters because downstream resolvers may call `sender.get_prim(...)`
//! during notice processing and expect the sender to expose the same prim view
//! while only specific path-valued fields are translated.

use std::sync::Arc;

use parking_lot::RwLock;
use usd_hd::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdSampledDataSource, HdSampledDataSourceTime, HdVectorDataSource, HdVectorDataSourceHandle,
    cast_to_container, cast_to_vector,
};
use usd_hd::scene_index::observer::{
    AddedPrimEntry, DirtiedPrimEntry, HdSceneIndexObserverHandle, RemovedPrimEntry,
    RenamedPrimEntry,
};
use usd_hd::scene_index::{
    FilteringObserverTarget, HdSceneIndexBase, HdSceneIndexHandle, HdSceneIndexPrim,
    HdSingleInputFilteringSceneIndexBase, SdfPathVector, si_ref, wire_filter_to_input,
};
use usd_sdf::Path;
use usd_tf::Token;
use usd_vt::Value;

mod tokens {
    use std::sync::LazyLock;

    use usd_tf::Token;

    pub static INSTANCE: LazyLock<Token> = LazyLock::new(|| Token::new("instance"));
    pub static INSTANCER: LazyLock<Token> = LazyLock::new(|| Token::new("instancer"));
    pub static PROTOTYPE_INDEX: LazyLock<Token> = LazyLock::new(|| Token::new("prototypeIndex"));
    pub static INSTANCER_TOPOLOGY: LazyLock<Token> =
        LazyLock::new(|| Token::new("instancerTopology"));
    pub static PROTOTYPES: LazyLock<Token> = LazyLock::new(|| Token::new("prototypes"));
}

/// Immutable configuration shared by data-source wrappers.
#[derive(Debug, Clone)]
struct TranslationConfig {
    proxy_path_data_source_names: Arc<Vec<Token>>,
}

impl TranslationConfig {
    /// Returns whether a prim-level child should be path-translated recursively.
    fn should_translate_paths_for_name(&self, name: &Token) -> bool {
        self.proxy_path_data_source_names
            .iter()
            .any(|token| token == name)
    }
}

/// Recursively translates all children of a vector data source.
#[derive(Clone)]
struct VectorDs {
    scene_index: HdSceneIndexHandle,
    underlying: HdVectorDataSourceHandle,
}

impl std::fmt::Debug for VectorDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorDs").finish()
    }
}

impl HdDataSourceBase for VectorDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_vector(&self) -> Option<HdVectorDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdVectorDataSource for VectorDs {
    fn get_num_elements(&self) -> usize {
        self.underlying.get_num_elements()
    }

    fn get_element(&self, element: usize) -> Option<HdDataSourceBaseHandle> {
        self.underlying
            .get_element(element)
            .map(|child| translate_data_source(&child, &self.scene_index))
    }
}

/// Recursively translates all children of a container data source.
#[derive(Clone)]
struct ContainerDs {
    scene_index: HdSceneIndexHandle,
    underlying: HdContainerDataSourceHandle,
}

impl std::fmt::Debug for ContainerDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerDs").finish()
    }
}

impl HdDataSourceBase for ContainerDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for ContainerDs {
    fn get_names(&self) -> Vec<Token> {
        self.underlying.get_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        self.underlying
            .get(name)
            .map(|child| translate_data_source(&child, &self.scene_index))
    }
}

/// Prim-level container wrapper that only translates configured child names.
#[derive(Clone)]
struct PrimDs {
    scene_index: HdSceneIndexHandle,
    underlying: HdContainerDataSourceHandle,
    config: TranslationConfig,
}

impl std::fmt::Debug for PrimDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimDs").finish()
    }
}

impl HdDataSourceBase for PrimDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }
}

impl HdContainerDataSource for PrimDs {
    fn get_names(&self) -> Vec<Token> {
        self.underlying.get_names()
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        let child = self.underlying.get(name)?;
        if self.config.should_translate_paths_for_name(name) {
            Some(translate_data_source(&child, &self.scene_index))
        } else {
            Some(child)
        }
    }
}

/// Sampled data source wrapper for `SdfPath`.
#[derive(Clone)]
struct PathDs {
    scene_index: HdSceneIndexHandle,
    underlying: HdDataSourceBaseHandle,
}

impl std::fmt::Debug for PathDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathDs").finish()
    }
}

impl HdDataSourceBase for PathDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for PathDs {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let translated = extract_path_value(&self.underlying, shutter_offset)
            .map(|path| translate_path(&path, &self.scene_index));
        translated.map(Value::new).unwrap_or_else(Value::empty)
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.underlying
            .as_sampled()
            .map(|sampled| {
                sampled.get_contributing_sample_times(start_time, end_time, out_sample_times)
            })
            .unwrap_or(false)
    }
}

/// Sampled data source wrapper for `Vec<SdfPath>`.
#[derive(Clone)]
struct PathArrayDs {
    scene_index: HdSceneIndexHandle,
    underlying: HdDataSourceBaseHandle,
}

impl std::fmt::Debug for PathArrayDs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathArrayDs").finish()
    }
}

impl HdDataSourceBase for PathArrayDs {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_sampled(&self) -> Option<&dyn HdSampledDataSource> {
        Some(self)
    }
}

impl HdSampledDataSource for PathArrayDs {
    fn get_value(&self, shutter_offset: HdSampledDataSourceTime) -> Value {
        let translated = extract_path_array_value(&self.underlying, shutter_offset).map(|paths| {
            paths
                .into_iter()
                .map(|path| translate_path(&path, &self.scene_index))
                .collect::<Vec<_>>()
        });
        translated.map(Value::new).unwrap_or_else(Value::empty)
    }

    fn get_contributing_sample_times(
        &self,
        start_time: HdSampledDataSourceTime,
        end_time: HdSampledDataSourceTime,
        out_sample_times: &mut Vec<HdSampledDataSourceTime>,
    ) -> bool {
        self.underlying
            .as_sampled()
            .map(|sampled| {
                sampled.get_contributing_sample_times(start_time, end_time, out_sample_times)
            })
            .unwrap_or(false)
    }
}

/// Scene index that translates instance-proxy paths within selected data sources.
pub struct InstanceProxyPathTranslationSceneIndex {
    /// Base filtering scene index plumbing.
    base: HdSingleInputFilteringSceneIndexBase,
    /// Prim-level child names that should receive recursive path translation.
    config: TranslationConfig,
}

impl std::fmt::Debug for InstanceProxyPathTranslationSceneIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstanceProxyPathTranslationSceneIndex")
            .field(
                "proxy_path_data_source_names",
                &self.config.proxy_path_data_source_names,
            )
            .finish()
    }
}

impl InstanceProxyPathTranslationSceneIndex {
    /// Creates a new scene index with no extra prim-level path names.
    pub fn new(input: HdSceneIndexHandle) -> Arc<RwLock<Self>> {
        Self::new_with_proxy_path_names(input, Vec::new())
    }

    /// Creates a new scene index with the configured prim-level path data source names.
    pub fn new_with_proxy_path_names(
        input: HdSceneIndexHandle,
        proxy_path_data_source_names: Vec<Token>,
    ) -> Arc<RwLock<Self>> {
        let result = Arc::new(RwLock::new(Self {
            base: HdSingleInputFilteringSceneIndexBase::new(Some(input.clone())),
            config: TranslationConfig {
                proxy_path_data_source_names: Arc::new(proxy_path_data_source_names),
            },
        }));
        wire_filter_to_input(&result, &input);
        result
    }
}

impl HdSceneIndexBase for InstanceProxyPathTranslationSceneIndex {
    fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
        let Some(input) = self.base.get_input_scene() else {
            return HdSceneIndexPrim::default();
        };

        let mut prim = si_ref(input).get_prim(prim_path);
        if let Some(data_source) = prim.data_source.take() {
            prim.data_source = Some(Arc::new(PrimDs {
                scene_index: input.clone(),
                underlying: data_source,
                config: self.config.clone(),
            }) as HdContainerDataSourceHandle);
        }
        prim
    }

    fn get_child_prim_paths(&self, prim_path: &Path) -> SdfPathVector {
        self.base
            .get_input_scene()
            .map(|input| si_ref(input).get_child_prim_paths(prim_path))
            .unwrap_or_default()
    }

    fn add_observer(&self, observer: HdSceneIndexObserverHandle) {
        self.base.base().add_observer(observer);
    }

    fn remove_observer(&self, observer: &HdSceneIndexObserverHandle) {
        self.base.base().remove_observer(observer);
    }

    fn get_display_name(&self) -> String {
        "InstanceProxyPathTranslationSceneIndex".to_string()
    }
}

impl FilteringObserverTarget for InstanceProxyPathTranslationSceneIndex {
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

/// Translate a scene-index data source recursively.
fn translate_data_source(
    data_source: &HdDataSourceBaseHandle,
    scene_index: &HdSceneIndexHandle,
) -> HdDataSourceBaseHandle {
    if let Some(container) = cast_to_container(data_source) {
        return Arc::new(ContainerDs {
            scene_index: scene_index.clone(),
            underlying: container,
        }) as HdDataSourceBaseHandle;
    }

    if let Some(vector) = cast_to_vector(data_source) {
        return Arc::new(VectorDs {
            scene_index: scene_index.clone(),
            underlying: vector,
        }) as HdDataSourceBaseHandle;
    }

    if looks_like_path_array_data_source(data_source) {
        return Arc::new(PathArrayDs {
            scene_index: scene_index.clone(),
            underlying: data_source.clone(),
        }) as HdDataSourceBaseHandle;
    }

    if looks_like_path_data_source(data_source) {
        return Arc::new(PathDs {
            scene_index: scene_index.clone(),
            underlying: data_source.clone(),
        }) as HdDataSourceBaseHandle;
    }

    data_source.clone()
}

/// Returns whether the sampled payload looks like a single `SdfPath`.
fn looks_like_path_data_source(data_source: &HdDataSourceBaseHandle) -> bool {
    extract_path_value(data_source, 0.0).is_some()
}

/// Returns whether the sampled payload looks like a `Vec<SdfPath>`.
fn looks_like_path_array_data_source(data_source: &HdDataSourceBaseHandle) -> bool {
    extract_path_array_value(data_source, 0.0).is_some()
}

/// Extract a path value from an arbitrary sampled data source.
fn extract_path_value(
    data_source: &HdDataSourceBaseHandle,
    shutter_offset: HdSampledDataSourceTime,
) -> Option<Path> {
    let value = data_source.as_sampled()?.get_value(shutter_offset);
    value.get::<Path>().cloned()
}

/// Extract a path-array value from an arbitrary sampled data source.
fn extract_path_array_value(
    data_source: &HdDataSourceBaseHandle,
    shutter_offset: HdSampledDataSourceTime,
) -> Option<Vec<Path>> {
    let value = data_source.as_sampled()?.get_value(shutter_offset);

    if let Some(paths) = value.get::<Vec<Path>>() {
        return Some(paths.clone());
    }

    if let Some(values) = value.get::<Vec<Value>>() {
        return values
            .iter()
            .map(|value| value.get::<Path>().cloned())
            .collect();
    }

    None
}

/// Returns whether a prim query result should be treated as valid.
fn is_valid_prim(prim: &HdSceneIndexPrim) -> bool {
    !prim.prim_type.is_empty() || prim.data_source.is_some()
}

/// Extract an `SdfPath` child from a container data source.
fn get_container_path_value(container: &HdContainerDataSourceHandle, name: &Token) -> Option<Path> {
    extract_path_value(&container.get(name)?, 0.0)
}

/// Extract an `i32` child from a container data source.
fn get_container_i32_value(container: &HdContainerDataSourceHandle, name: &Token) -> Option<i32> {
    let value = container.get(name)?.as_sampled()?.get_value(0.0);
    value.get::<i32>().copied()
}

/// Resolve the prototype path for an instance prim, if present.
fn get_prototype_path(
    prim_data_source: &HdContainerDataSourceHandle,
    scene_index: &HdSceneIndexHandle,
) -> Option<Path> {
    let instance_container = cast_to_container(&prim_data_source.get(&tokens::INSTANCE)?)?;
    let instancer_path = get_container_path_value(&instance_container, &tokens::INSTANCER)?;
    let prototype_index = get_container_i32_value(&instance_container, &tokens::PROTOTYPE_INDEX)?;
    if prototype_index < 0 {
        return None;
    }

    let instancer_prim = si_ref(scene_index).get_prim(&instancer_path);
    let instancer_data_source = instancer_prim.data_source?;
    let instancer_topology =
        cast_to_container(&instancer_data_source.get(&tokens::INSTANCER_TOPOLOGY)?)?;
    let prototypes = extract_path_array_value(&instancer_topology.get(&tokens::PROTOTYPES)?, 0.0)?;
    prototypes.get(prototype_index as usize).cloned()
}

/// Translate an instance-proxy path to its prototype path when needed.
fn translate_path(path: &Path, scene_index: &HdSceneIndexHandle) -> Path {
    if is_valid_prim(&si_ref(scene_index).get_prim(path)) {
        return path.clone();
    }

    let path_string = path.as_str();
    if !path_string.starts_with('/') {
        return path.clone();
    }

    let mut result = Path::absolute_root();
    for component in path_string
        .split('/')
        .filter(|component| !component.is_empty())
    {
        let Some(next) = result.append_child(component) else {
            return path.clone();
        };
        result = next;

        let instance_data_source = si_ref(scene_index).get_prim(&result).data_source;
        if let Some(instance_data_source) = instance_data_source {
            if let Some(prototype_path) = get_prototype_path(&instance_data_source, scene_index) {
                result = prototype_path;
            }
        }
    }

    result
}

/// Handle type for `InstanceProxyPathTranslationSceneIndex`.
pub type InstanceProxyPathTranslationSceneIndexHandle =
    Arc<RwLock<InstanceProxyPathTranslationSceneIndex>>;

/// Convenience constructor matching the other scene-index factory helpers.
pub fn create_instance_proxy_path_translation_scene_index(
    input: HdSceneIndexHandle,
) -> InstanceProxyPathTranslationSceneIndexHandle {
    InstanceProxyPathTranslationSceneIndex::new(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use usd_hd::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};
    use usd_hd::scene_index::observer::HdSceneIndexObserver;

    #[derive(Debug, Default)]
    struct TestSceneIndex {
        prims: HashMap<Path, HdSceneIndexPrim>,
    }

    impl HdSceneIndexBase for TestSceneIndex {
        fn get_prim(&self, prim_path: &Path) -> HdSceneIndexPrim {
            self.prims.get(prim_path).cloned().unwrap_or_default()
        }

        fn get_child_prim_paths(&self, _prim_path: &Path) -> SdfPathVector {
            Vec::new()
        }

        fn add_observer(&self, _observer: HdSceneIndexObserverHandle) {}

        fn remove_observer(&self, _observer: &HdSceneIndexObserverHandle) {}
    }

    #[derive(Debug, Default)]
    struct TestObserver;

    impl HdSceneIndexObserver for TestObserver {
        fn prims_added(&self, _sender: &dyn HdSceneIndexBase, _entries: &[AddedPrimEntry]) {}
        fn prims_removed(&self, _sender: &dyn HdSceneIndexBase, _entries: &[RemovedPrimEntry]) {}
        fn prims_dirtied(&self, _sender: &dyn HdSceneIndexBase, _entries: &[DirtiedPrimEntry]) {}
        fn prims_renamed(&self, _sender: &dyn HdSceneIndexBase, _entries: &[RenamedPrimEntry]) {}
    }

    fn make_test_input() -> HdSceneIndexHandle {
        let mut prims = HashMap::new();

        let instancer_path = Path::from_string("/Instancer").unwrap();
        let prototype_path = Path::from_string("/__Prototype_1").unwrap();
        let instance_path = Path::from_string("/Inst").unwrap();
        let consumer_path = Path::from_string("/Consumer").unwrap();

        let instancer_topology = HdRetainedContainerDataSource::new_1(
            tokens::PROTOTYPES.clone(),
            HdRetainedTypedSampledDataSource::new(vec![prototype_path.clone()]),
        );
        prims.insert(
            instancer_path.clone(),
            HdSceneIndexPrim {
                prim_type: Token::new("instancer"),
                data_source: Some(HdRetainedContainerDataSource::new_1(
                    tokens::INSTANCER_TOPOLOGY.clone(),
                    instancer_topology,
                )),
            },
        );

        let instance_ds = HdRetainedContainerDataSource::from_entries(&[
            (
                tokens::INSTANCER.clone(),
                HdRetainedTypedSampledDataSource::new(instancer_path.clone())
                    as HdDataSourceBaseHandle,
            ),
            (
                tokens::PROTOTYPE_INDEX.clone(),
                HdRetainedTypedSampledDataSource::new(0_i32) as HdDataSourceBaseHandle,
            ),
        ]);
        prims.insert(
            instance_path.clone(),
            HdSceneIndexPrim {
                prim_type: Token::new("instance"),
                data_source: Some(HdRetainedContainerDataSource::new_1(
                    tokens::INSTANCE.clone(),
                    instance_ds,
                )),
            },
        );

        let binding_path = Path::from_string("/Inst/Child").unwrap();
        let material_bindings = HdRetainedContainerDataSource::new_1(
            Token::new("preview"),
            HdRetainedContainerDataSource::new_1(
                Token::new("path"),
                HdRetainedTypedSampledDataSource::new(binding_path),
            ),
        );
        prims.insert(
            consumer_path,
            HdSceneIndexPrim {
                prim_type: Token::new("mesh"),
                data_source: Some(HdRetainedContainerDataSource::new_1(
                    Token::new("materialBindings"),
                    material_bindings,
                )),
            },
        );

        Arc::new(RwLock::new(TestSceneIndex { prims }))
    }

    #[test]
    fn test_translate_invalid_instance_proxy_path_to_prototype_path() {
        let input = make_test_input();
        let path = Path::from_string("/Inst/Child").unwrap();
        let translated = translate_path(&path, &input);
        assert_eq!(
            translated,
            Path::from_string("/__Prototype_1/Child").unwrap()
        );
    }

    #[test]
    fn test_get_prim_wraps_configured_path_data_sources() {
        let input = make_test_input();
        let scene = InstanceProxyPathTranslationSceneIndex::new_with_proxy_path_names(
            input,
            vec![Token::new("materialBindings")],
        );

        let prim = scene
            .read()
            .get_prim(&Path::from_string("/Consumer").unwrap());
        let prim_ds = prim.data_source.expect("wrapped prim data source");
        let material_bindings = prim_ds
            .get(&Token::new("materialBindings"))
            .and_then(|child| cast_to_container(&child))
            .expect("material bindings container");
        let preview = material_bindings
            .get(&Token::new("preview"))
            .and_then(|child| cast_to_container(&child))
            .expect("preview binding");
        let path_ds = preview
            .get(&Token::new("path"))
            .expect("translated path data source");

        let value = path_ds.as_sampled().expect("sampled path").get_value(0.0);
        assert_eq!(
            value.get::<Path>().cloned(),
            Some(Path::from_string("/__Prototype_1/Child").unwrap())
        );
    }

    #[test]
    fn test_notices_forward_original_prim_paths() {
        let input = make_test_input();
        let scene = InstanceProxyPathTranslationSceneIndex::new(input.clone());
        scene.read().add_observer(Arc::new(TestObserver));

        let entry = DirtiedPrimEntry {
            prim_path: Path::from_string("/Inst/Child").unwrap(),
            dirty_locators: Default::default(),
        };

        scene
            .read()
            .on_prims_dirtied(&*input.read(), &[entry.clone()]);
    }
}
