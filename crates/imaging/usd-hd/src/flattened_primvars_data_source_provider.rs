//! HdFlattenedPrimvarsDataSourceProvider - Flattens primvars with constant inheritance.
//!
//! Corresponds to pxr/imaging/hd/flattenedPrimvarsDataSourceProvider.h
//!
//! Inherits constant primvars from parent prims when a child prim doesn't define them.

use crate::data_source::{
    HdContainerDataSource, HdContainerDataSourceHandle, HdDataSourceBase, HdDataSourceBaseHandle,
    HdDataSourceLocator, HdDataSourceLocatorSet, HdInvalidatableContainerDataSource,
    cast_to_container,
};
use crate::flattened_data_source_provider::{
    HdFlattenedDataSourceProvider, HdFlattenedDataSourceProviderContext,
};
use crate::schema::HdPrimvarSchema;
use parking_lot::RwLock;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use usd_tf::Token;

fn is_constant_primvar(primvar_container: Option<&HdContainerDataSourceHandle>) -> bool {
    let Some(container) = primvar_container else {
        return false;
    };
    let primvar_schema = HdPrimvarSchema::new(container.clone());
    let Some(interpolation_ds) = primvar_schema.get_interpolation() else {
        return false;
    };
    let interpolation: Token = interpolation_ds.get_typed_value(0.0f32);
    interpolation == "constant"
}

fn does_locator_intersect_interpolation(locator: &HdDataSourceLocator) -> bool {
    if locator.len() < 2 {
        return true;
    }
    locator
        .get_element(1)
        .map_or(false, |t: &Token| t == "interpolation")
}

/// Container data source that merges prim's primvars with parent's constant primvars.
///
/// When a prim doesn't have a primvar, checks the parent's flattened primvars
/// and uses it if the interpolation is constant.
#[derive(Debug)]
struct FlattenedPrimvarsDataSourceInner {
    primvars_container: Option<HdContainerDataSourceHandle>,
    parent_flattened: Option<HdContainerDataSourceHandle>,
    cached_constant_primvar_names: RwLock<Option<BTreeSet<Token>>>,
    cached_primvars: RwLock<HashMap<Token, Option<HdContainerDataSourceHandle>>>,
}

#[derive(Clone)]
struct FlattenedPrimvarsDataSource {
    inner: Arc<FlattenedPrimvarsDataSourceInner>,
}

impl FlattenedPrimvarsDataSource {
    fn new(
        primvars_container: Option<HdContainerDataSourceHandle>,
        parent_flattened: Option<HdContainerDataSourceHandle>,
    ) -> Arc<Self> {
        Arc::new(Self {
            inner: Arc::new(FlattenedPrimvarsDataSourceInner {
                primvars_container,
                parent_flattened,
                cached_constant_primvar_names: RwLock::new(None),
                cached_primvars: RwLock::new(HashMap::new()),
            }),
        })
    }

    fn get_constant_primvar_names(&self) -> BTreeSet<Token> {
        if let Some(cached) = self.inner.cached_constant_primvar_names.read().clone() {
            return cached;
        }

        let computed = self.get_constant_primvar_names_uncached();
        *self.inner.cached_constant_primvar_names.write() = Some(computed.clone());
        computed
    }

    fn get_constant_primvar_names_uncached(&self) -> BTreeSet<Token> {
        let mut result = BTreeSet::new();

        if let Some(ref parent) = self.inner.parent_flattened {
            for name in parent.get_names() {
                if let Some(child) = parent.get(&name) {
                    if let Some(child_container) = cast_to_container(&child) {
                        if is_constant_primvar(Some(&child_container)) {
                            result.insert(name);
                        }
                    }
                }
            }
        }

        if let Some(ref container) = self.inner.primvars_container {
            for name in container.get_names() {
                if let Some(child) = container.get(&name) {
                    if let Some(child_container) = cast_to_container(&child) {
                        if is_constant_primvar(Some(&child_container)) {
                            result.insert(name);
                        }
                    }
                }
            }
        }

        result
    }

    fn get_uncached(&self, name: &Token) -> Option<HdContainerDataSourceHandle> {
        if let Some(ref container) = self.inner.primvars_container {
            if let Some(child) = container.get(name) {
                if let Some(result) = cast_to_container(&child) {
                    return Some(result);
                }
            }
        }

        if let Some(ref parent) = self.inner.parent_flattened {
            if let Some(child) = parent.get(name) {
                if let Some(container) = cast_to_container(&child) {
                    if is_constant_primvar(Some(&container)) {
                        return Some(container);
                    }
                }
            }
        }

        None
    }
}

impl HdDataSourceBase for FlattenedPrimvarsDataSource {
    fn clone_box(&self) -> HdDataSourceBaseHandle {
        Arc::new(self.clone()) as HdDataSourceBaseHandle
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_container(&self) -> Option<HdContainerDataSourceHandle> {
        Some(Arc::new(self.clone()))
    }

    fn as_invalidatable_container(&self) -> Option<&dyn HdInvalidatableContainerDataSource> {
        Some(self)
    }
}

impl HdContainerDataSource for FlattenedPrimvarsDataSource {
    fn get_names(&self) -> Vec<Token> {
        let mut result = Vec::new();

        if let Some(ref container) = self.inner.primvars_container {
            result = container.get_names();
        }

        if self.inner.parent_flattened.is_some() {
            let mut constant_names = self.get_constant_primvar_names();
            for name in &result {
                constant_names.remove(name);
            }
            result.extend(constant_names);
        }

        result
    }

    fn get(&self, name: &Token) -> Option<HdDataSourceBaseHandle> {
        if let Some(cached) = self.inner.cached_primvars.read().get(name).cloned() {
            return cached.map(|c| c as HdDataSourceBaseHandle);
        }

        let result = self.get_uncached(name);
        self.inner
            .cached_primvars
            .write()
            .insert(name.clone(), result.clone());
        result.map(|c| c as HdDataSourceBaseHandle)
    }
}

impl HdInvalidatableContainerDataSource for FlattenedPrimvarsDataSource {
    fn invalidate(&self, dirty_locators: &HdDataSourceLocatorSet) -> bool {
        let mut any_dirtied = false;

        for locator in dirty_locators.iter() {
            if does_locator_intersect_interpolation(locator) {
                let mut cached_primvars = self.inner.cached_primvars.write();
                let mut cached_names = self.inner.cached_constant_primvar_names.write();
                if !cached_primvars.is_empty() || cached_names.is_some() {
                    any_dirtied = true;
                }
                cached_primvars.clear();
                *cached_names = None;
                break;
            }

            if let Some(primvar_name) = locator.first_element() {
                if self
                    .inner
                    .cached_primvars
                    .write()
                    .remove(primvar_name)
                    .is_some()
                {
                    any_dirtied = true;
                }
            }
        }

        any_dirtied
    }
}

impl std::fmt::Debug for FlattenedPrimvarsDataSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FlattenedPrimvarsDataSource")
            .field("has_primvars", &self.inner.primvars_container.is_some())
            .field("has_parent", &self.inner.parent_flattened.is_some())
            .finish()
    }
}

/// Provider that flattens primvars, inheriting constant primvars from parents.
///
/// Corresponds to C++ HdFlattenedPrimvarsDataSourceProvider.
#[derive(Debug, Default)]
pub struct HdFlattenedPrimvarsDataSourceProvider;

impl HdFlattenedPrimvarsDataSourceProvider {
    /// Create a new provider.
    pub fn new() -> Self {
        Self
    }
}

impl HdFlattenedDataSourceProvider for HdFlattenedPrimvarsDataSourceProvider {
    fn get_flattened_data_source(
        &self,
        ctx: &HdFlattenedDataSourceProviderContext<'_>,
    ) -> Option<HdContainerDataSourceHandle> {
        let input = ctx.get_input_data_source();
        let parent = ctx.get_flattened_data_source_from_parent_prim();

        Some(FlattenedPrimvarsDataSource::new(input, parent) as HdContainerDataSourceHandle)
    }

    fn compute_dirty_locators_for_descendants(&self, locators: &mut HdDataSourceLocatorSet) {
        let to_check: Vec<_> = locators.iter().cloned().collect();
        for locator in &to_check {
            if does_locator_intersect_interpolation(locator) {
                *locators = HdDataSourceLocatorSet::universal();
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source::{HdRetainedContainerDataSource, HdRetainedTypedSampledDataSource};

    fn tok(s: &str) -> Token {
        Token::new(s)
    }

    fn constant_primvar() -> HdContainerDataSourceHandle {
        HdPrimvarSchema::build_retained(
            None,
            None,
            None,
            Some(HdRetainedTypedSampledDataSource::new(tok("constant"))),
            None,
            None,
            None,
        )
    }

    #[test]
    fn invalidate_specific_primvar_preserves_other_cached_entries() {
        let parent = HdRetainedContainerDataSource::from_entries(&[
            (tok("color"), constant_primvar() as HdDataSourceBaseHandle),
            (tok("opacity"), constant_primvar() as HdDataSourceBaseHandle),
        ]);
        let ds = FlattenedPrimvarsDataSource::new(None, Some(parent));

        assert!(ds.get(&tok("color")).is_some());
        assert!(ds.get(&tok("opacity")).is_some());
        let names = ds.get_names();
        assert!(names.contains(&tok("color")));
        assert!(names.contains(&tok("opacity")));

        assert!(ds.inner.cached_primvars.read().contains_key(&tok("color")));
        assert!(
            ds.inner
                .cached_primvars
                .read()
                .contains_key(&tok("opacity"))
        );
        assert!(ds.inner.cached_constant_primvar_names.read().is_some());

        let mut locators = HdDataSourceLocatorSet::new();
        locators.insert(HdDataSourceLocator::from_tokens_2(
            tok("color"),
            tok("primvarValue"),
        ));

        assert!(ds.invalidate(&locators));
        assert!(!ds.inner.cached_primvars.read().contains_key(&tok("color")));
        assert!(
            ds.inner
                .cached_primvars
                .read()
                .contains_key(&tok("opacity"))
        );
        assert!(ds.inner.cached_constant_primvar_names.read().is_some());
    }

    #[test]
    fn invalidate_interpolation_clears_all_primvar_caches() {
        let parent = HdRetainedContainerDataSource::from_entries(&[
            (tok("color"), constant_primvar() as HdDataSourceBaseHandle),
            (tok("opacity"), constant_primvar() as HdDataSourceBaseHandle),
        ]);
        let ds = FlattenedPrimvarsDataSource::new(None, Some(parent));

        assert!(ds.get(&tok("color")).is_some());
        assert!(ds.get(&tok("opacity")).is_some());
        let _ = ds.get_names();

        let mut locators = HdDataSourceLocatorSet::new();
        locators.insert(HdDataSourceLocator::from_tokens_2(
            tok("color"),
            tok("interpolation"),
        ));

        assert!(ds.invalidate(&locators));
        assert!(ds.inner.cached_primvars.read().is_empty());
        assert!(ds.inner.cached_constant_primvar_names.read().is_none());
    }
}
