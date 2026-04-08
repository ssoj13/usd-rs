#![allow(dead_code)]
//! Manager for prim and API schema adapters.

use super::data_source_prim::DataSourcePrim;
use super::data_source_stage_globals::DataSourceStageGlobalsHandle;
use super::prim_adapter::PrimAdapterHandle;
use super::types::PropertyInvalidationType;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use usd_core::Prim;
use usd_hd::{HdContainerDataSourceHandle, HdDataSourceLocatorSet};
use usd_tf::Token;

/// Shared reference to API schema adapter.
pub type ApiSchemaAdapterHandle = Arc<dyn ApiSchemaAdapter>;

/// Trait for API schema adapters.
///
/// API schema adapters contribute data to a prim's scene index representation
/// based on applied API schemas (e.g., MaterialBindingAPI, CollectionAPI).
pub trait ApiSchemaAdapter: Send + Sync {
    /// Returns the schema name this adapter handles.
    fn get_schema_name(&self) -> Token;

    /// Returns whether this is a multi-apply schema.
    ///
    /// Multi-apply schemas can be applied multiple times with different
    /// instance names (e.g., CollectionAPI:lightLink, CollectionAPI:shadowLink).
    fn is_multi_apply(&self) -> bool {
        false
    }

    /// Returns imaging subprims contributed by this API schema adapter.
    fn get_imaging_subprims(&self, _prim: &Prim, _applied_instance_name: &Token) -> Vec<Token> {
        Vec::new()
    }

    /// Returns the Hydra type contributed by this API schema adapter for subprim.
    fn get_imaging_subprim_type(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
    ) -> Token {
        Token::new("")
    }

    /// Returns data source contribution for subprim.
    fn get_imaging_subprim_data(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
        _stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        None
    }

    /// Returns data source invalidation for property changes.
    fn invalidate_imaging_subprim(
        &self,
        _prim: &Prim,
        _subprim: &Token,
        _applied_instance_name: &Token,
        _properties: &[Token],
        _invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        HdDataSourceLocatorSet::empty()
    }
}

/// Entry combining adapter with optional instance name.
#[derive(Clone)]
pub struct AdapterEntry {
    /// The API schema adapter (or prim adapter wrapped as API schema adapter)
    pub adapter: ApiSchemaAdapterHandle,

    /// Instance name for multi-apply API schemas.
    ///
    /// For example, CollectionAPI:lightLink would have instance name "lightLink".
    /// Empty for single-apply schemas.
    pub applied_instance_name: Token,
}

impl AdapterEntry {
    /// Create new adapter entry.
    pub fn new(adapter: ApiSchemaAdapterHandle, applied_instance_name: Token) -> Self {
        Self {
            adapter,
            applied_instance_name,
        }
    }

    /// Create adapter entry with no instance name.
    pub fn single_apply(adapter: ApiSchemaAdapterHandle) -> Self {
        Self {
            adapter,
            applied_instance_name: Token::new(""),
        }
    }
}

/// Collection of adapters for a prim.
#[derive(Clone)]
pub struct AdaptersEntry {
    /// All adapters in order, including prim adapter wrapped as API schema adapter.
    pub all_adapters: Vec<AdapterEntry>,

    /// Just the prim adapter for the prim type.
    pub prim_adapter: Option<PrimAdapterHandle>,
}

impl AdaptersEntry {
    /// Create new adapters entry.
    pub fn new(all_adapters: Vec<AdapterEntry>, prim_adapter: Option<PrimAdapterHandle>) -> Self {
        Self {
            all_adapters,
            prim_adapter,
        }
    }

    /// Create empty adapters entry.
    pub fn empty() -> Self {
        Self {
            all_adapters: Vec::new(),
            prim_adapter: None,
        }
    }
}

/// Wrapped prim adapter as API schema adapter.
struct WrappedPrimAdapter {
    prim_adapter: PrimAdapterHandle,
}

impl ApiSchemaAdapter for WrappedPrimAdapter {
    fn get_schema_name(&self) -> Token {
        Token::new("_PrimAdapter")
    }

    fn get_imaging_subprims(&self, prim: &Prim, _applied_instance_name: &Token) -> Vec<Token> {
        self.prim_adapter.get_imaging_subprims(prim)
    }

    fn get_imaging_subprim_type(
        &self,
        prim: &Prim,
        subprim: &Token,
        _applied_instance_name: &Token,
    ) -> Token {
        self.prim_adapter.get_imaging_subprim_type(prim, subprim)
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        _applied_instance_name: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        self.prim_adapter
            .get_imaging_subprim_data(prim, subprim, stage_globals)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        _applied_instance_name: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        self.prim_adapter
            .invalidate_imaging_subprim(prim, subprim, properties, invalidation_type)
    }
}

/// Fallback adapter matching OpenUSD's `_BasePrimAdapterAPISchemaAdapter`.
///
/// When no schema-specific prim adapter exists, this still exposes the generic
/// `DataSourcePrim` container for the empty subprim so xform/imageable state is
/// preserved for plain USD prims such as `Xform`.
struct BasePrimAdapterApiSchemaAdapter;

impl ApiSchemaAdapter for BasePrimAdapterApiSchemaAdapter {
    fn get_schema_name(&self) -> Token {
        Token::new("_BasePrimAdapter")
    }

    fn get_imaging_subprim_data(
        &self,
        prim: &Prim,
        subprim: &Token,
        _applied_instance_name: &Token,
        stage_globals: &DataSourceStageGlobalsHandle,
    ) -> Option<HdContainerDataSourceHandle> {
        if !subprim.as_str().is_empty() {
            return None;
        }

        Some(Arc::new(DataSourcePrim::new(
            prim.clone(),
            prim.get_path().clone(),
            stage_globals.clone(),
        )) as HdContainerDataSourceHandle)
    }

    fn invalidate_imaging_subprim(
        &self,
        prim: &Prim,
        subprim: &Token,
        _applied_instance_name: &Token,
        properties: &[Token],
        invalidation_type: PropertyInvalidationType,
    ) -> HdDataSourceLocatorSet {
        DataSourcePrim::invalidate(prim, subprim, properties, invalidation_type)
    }
}

/// Prim adapter together with API schema wrapper.
#[derive(Clone)]
struct WrappedPrimAdapterEntry {
    prim_adapter: Option<PrimAdapterHandle>,
    api_schema_adapter: ApiSchemaAdapterHandle,
}

impl WrappedPrimAdapterEntry {
    fn with_prim_adapter(prim_adapter: PrimAdapterHandle) -> Self {
        let api_schema_adapter = Arc::new(WrappedPrimAdapter {
            prim_adapter: prim_adapter.clone(),
        });
        Self {
            prim_adapter: Some(prim_adapter),
            api_schema_adapter,
        }
    }

    fn with_base_prim_fallback() -> Self {
        Self {
            prim_adapter: None,
            api_schema_adapter: Arc::new(BasePrimAdapterApiSchemaAdapter),
        }
    }
}

/// Manager for computing adapters needed for a prim.
///
/// The adapter manager maintains caches of:
/// - Prim type to adapter mappings
/// - API schema to adapter mappings
/// - Combined adapters for specific prim type info
///
/// # Thread Safety
///
/// The manager uses concurrent hash maps for thread-safe lookups during
/// parallel scene index population.
pub struct AdapterManager {
    /// Cache of prim type to wrapped adapter
    prim_type_cache: Arc<RwLock<HashMap<Token, WrappedPrimAdapterEntry>>>,

    /// Cache of schema name to API schema adapter
    schema_cache: Arc<RwLock<HashMap<Token, ApiSchemaAdapterHandle>>>,

    /// Registered API schema adapters that survive cache resets.
    schema_registry: Arc<RwLock<HashMap<Token, ApiSchemaAdapterHandle>>>,

    /// Cache of prim type to full adapter set
    /// In C++, this uses UsdPrimTypeInfo* as key, but we use type name Token
    adapters_cache: Arc<RwLock<HashMap<Token, AdaptersEntry>>>,

    /// Keyless API schema adapters (always applied)
    keyless_adapters: Vec<ApiSchemaAdapterHandle>,

    /// Adapter registry for looking up real prim adapters by type name.
    /// Unknown prim types still fall back to the generic base-prim datasource.
    registry: Option<Arc<super::adapter_registry::AdapterRegistry>>,
}

impl AdapterManager {
    fn register_default_api_schema_adapters(&self) {
        self.register_api_schema_adapter(
            Token::new("MaterialBindingAPI"),
            crate::material_binding_api_adapter::create_default(),
        );
        self.register_api_schema_adapter(
            Token::new("SkelBindingAPI"),
            crate::skel::binding_api_adapter::create_default(),
        );
    }

    fn make_cache_key(type_name: &Token, applied_schemas: &[Token]) -> Token {
        if applied_schemas.is_empty() {
            return type_name.clone();
        }
        let mut key = String::from(type_name.as_str());
        key.push('|');
        for (i, schema) in applied_schemas.iter().enumerate() {
            if i > 0 {
                key.push(';');
            }
            key.push_str(schema.as_str());
        }
        Token::new(&key)
    }

    /// Create new adapter manager without a prim adapter registry.
    ///
    /// This still provides the OpenUSD base-prim fallback so generic USD state
    /// remains visible through `DataSourcePrim` even without specialized adapters.
    pub fn new() -> Self {
        Self {
            prim_type_cache: Arc::new(RwLock::new(HashMap::new())),
            schema_cache: Arc::new(RwLock::new(HashMap::new())),
            schema_registry: Arc::new(RwLock::new(HashMap::new())),
            adapters_cache: Arc::new(RwLock::new(HashMap::new())),
            keyless_adapters: Vec::new(),
            registry: None,
        }
    }

    /// Create adapter manager wired to the given registry.
    ///
    /// `compute_wrapped_prim_adapter()` will look up real adapters from
    /// the registry and otherwise use the same base-prim fallback as OpenUSD.
    pub fn new_with_registry(registry: Arc<super::adapter_registry::AdapterRegistry>) -> Self {
        let manager = Self {
            prim_type_cache: Arc::new(RwLock::new(HashMap::new())),
            schema_cache: Arc::new(RwLock::new(HashMap::new())),
            schema_registry: Arc::new(RwLock::new(HashMap::new())),
            adapters_cache: Arc::new(RwLock::new(HashMap::new())),
            keyless_adapters: Vec::new(),
            registry: Some(registry),
        };
        manager.register_default_api_schema_adapters();
        manager
    }

    /// Reset all caches.
    ///
    /// This should be called when the adapter registry changes or
    /// when switching to a different stage.
    pub fn reset(&self) {
        self.prim_type_cache.write().clear();
        self.schema_cache.write().clear();
        self.adapters_cache.write().clear();
    }

    /// Look up all adapters needed to serve a prim.
    ///
    /// This returns both the prim adapter and all applicable API schema
    /// adapters based on the prim's type and applied schemas.
    ///
    /// # Arguments
    ///
    /// * `prim` - The USD prim to look up adapters for
    ///
    /// # Returns
    ///
    /// Entry containing prim adapter and all API schema adapters
    pub fn lookup_adapters(&self, prim: &Prim) -> AdaptersEntry {
        let type_name = prim.get_type_name();
        let applied_schemas = prim.get_applied_schemas();
        let cache_key = Self::make_cache_key(&type_name, &applied_schemas);

        {
            let cache = self.adapters_cache.read();
            if let Some(entry) = cache.get(&cache_key) {
                return entry.clone();
            }
        }

        let entry = self.compute_adapters_for_prim(&type_name, &applied_schemas);

        {
            let mut cache = self.adapters_cache.write();
            cache.insert(cache_key, entry.clone());
        }

        entry
    }

    /// Look up adapters by prim type name.
    ///
    /// # Arguments
    ///
    /// * `type_name` - USD prim type name (e.g., "Mesh", "Camera")
    ///
    /// # Returns
    ///
    /// Entry containing adapters for this type
    pub fn lookup_adapters_by_type(&self, type_name: &Token) -> AdaptersEntry {
        // Check cache first
        {
            let cache = self.adapters_cache.read();
            if let Some(entry) = cache.get(type_name) {
                return entry.clone();
            }
        }

        // Compute adapters for this type
        let entry = self.compute_adapters(type_name);

        // Cache result
        {
            let mut cache = self.adapters_cache.write();
            cache.insert(type_name.clone(), entry.clone());
        }

        entry
    }

    /// Compute adapters for a prim type.
    fn compute_adapters(&self, type_name: &Token) -> AdaptersEntry {
        self.compute_adapters_for_prim(type_name, &[])
    }

    /// Compute adapters for a prim type + applied API schemas.
    fn compute_adapters_for_prim(
        &self,
        type_name: &Token,
        applied_schemas: &[Token],
    ) -> AdaptersEntry {
        let mut all_adapters = Vec::new();

        // Get wrapped prim adapter
        let wrapped = self.lookup_wrapped_prim_adapter(type_name);

        // Add prim adapter as first entry
        all_adapters.push(AdapterEntry::single_apply(
            wrapped.api_schema_adapter.clone(),
        ));

        // Add applied API schema adapters from apiSchemas metadata.
        for applied in applied_schemas {
            let (schema_name, instance_name) = match applied.as_str().split_once(':') {
                Some((base, instance)) => (base, instance),
                None => (applied.as_str(), ""),
            };
            let schema_token = Token::new(schema_name);
            if let Some(adapter) = self.lookup_api_schema_adapter(&schema_token) {
                all_adapters.push(AdapterEntry::new(adapter, Token::new(instance_name)));
            }
        }

        // Add keyless adapters (always applied)
        for adapter in &self.keyless_adapters {
            all_adapters.push(AdapterEntry::single_apply(adapter.clone()));
        }

        AdaptersEntry::new(all_adapters, wrapped.prim_adapter.clone())
    }

    /// Look up wrapped prim adapter for a type.
    fn lookup_wrapped_prim_adapter(&self, prim_type: &Token) -> WrappedPrimAdapterEntry {
        // Check cache first
        {
            let cache = self.prim_type_cache.read();
            if let Some(entry) = cache.get(prim_type) {
                return entry.clone();
            }
        }

        // Compute wrapped adapter
        let entry = self.compute_wrapped_prim_adapter(prim_type);

        // Cache result
        {
            let mut cache = self.prim_type_cache.write();
            cache.insert(prim_type.clone(), entry.clone());
        }

        entry
    }

    /// Compute wrapped prim adapter for a type.
    /// Uses a schema-specific adapter when available, otherwise the generic
    /// base-prim fallback that exposes `DataSourcePrim`.
    fn compute_wrapped_prim_adapter(&self, prim_type: &Token) -> WrappedPrimAdapterEntry {
        // Try registry first if wired
        if let Some(ref registry) = self.registry {
            if let Some(adapter) = registry.find(prim_type) {
                return WrappedPrimAdapterEntry::with_prim_adapter(adapter);
            }
        }
        WrappedPrimAdapterEntry::with_base_prim_fallback()
    }

    /// Look up API schema adapter by schema name.
    ///
    /// # Arguments
    ///
    /// * `schema_name` - Name of the API schema (e.g., "MaterialBindingAPI")
    ///
    /// # Returns
    ///
    /// The adapter if registered, None otherwise
    pub fn lookup_api_schema_adapter(&self, schema_name: &Token) -> Option<ApiSchemaAdapterHandle> {
        if let Some(adapter) = self.schema_cache.read().get(schema_name).cloned() {
            return Some(adapter);
        }

        let adapter = self.schema_registry.read().get(schema_name).cloned();
        if let Some(adapter) = adapter.clone() {
            self.schema_cache
                .write()
                .insert(schema_name.clone(), adapter.clone());
        }
        adapter
    }

    /// Register API schema adapter.
    ///
    /// # Arguments
    ///
    /// * `schema_name` - Name of the API schema
    /// * `adapter` - The adapter instance
    pub fn register_api_schema_adapter(&self, schema_name: Token, adapter: ApiSchemaAdapterHandle) {
        self.schema_registry
            .write()
            .insert(schema_name.clone(), adapter.clone());
        self.schema_cache.write().insert(schema_name, adapter);
    }

    /// Get number of cached prim type adapters.
    pub fn prim_adapter_count(&self) -> usize {
        self.prim_type_cache.read().len()
    }

    /// Get number of cached API schema adapters.
    pub fn api_schema_adapter_count(&self) -> usize {
        self.schema_registry.read().len()
    }
}

impl Default for AdapterManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_source_stage_globals::NoOpStageGlobals;
    use usd_core::Stage;

    struct TestApiSchemaAdapter {
        schema_name: Token,
    }

    impl ApiSchemaAdapter for TestApiSchemaAdapter {
        fn get_schema_name(&self) -> Token {
            self.schema_name.clone()
        }
    }

    #[test]
    fn test_new_manager() {
        let manager = AdapterManager::new();
        assert_eq!(manager.prim_adapter_count(), 0);
        assert_eq!(manager.api_schema_adapter_count(), 0);
    }

    #[test]
    fn test_lookup_adapters_by_type() {
        let manager = AdapterManager::new();
        let mesh_type = Token::new("Mesh");

        let entry = manager.lookup_adapters_by_type(&mesh_type);
        assert!(entry.prim_adapter.is_none());
        assert!(!entry.all_adapters.is_empty());
    }

    #[test]
    fn test_lookup_caching() {
        let manager = AdapterManager::new();
        let mesh_type = Token::new("Mesh");

        // First lookup computes
        let _entry1 = manager.lookup_adapters_by_type(&mesh_type);
        assert_eq!(manager.prim_adapter_count(), 1);

        // Second lookup uses cache
        let _entry2 = manager.lookup_adapters_by_type(&mesh_type);
        assert_eq!(manager.prim_adapter_count(), 1);
    }

    #[test]
    fn test_reset() {
        let manager = AdapterManager::new();
        let mesh_type = Token::new("Mesh");

        manager.lookup_adapters_by_type(&mesh_type);
        assert_eq!(manager.prim_adapter_count(), 1);

        manager.reset();
        assert_eq!(manager.prim_adapter_count(), 0);
    }

    #[test]
    fn test_register_api_schema_adapter() {
        let manager = AdapterManager::new();
        let schema_name = Token::new("MaterialBindingAPI");
        let adapter = Arc::new(TestApiSchemaAdapter {
            schema_name: schema_name.clone(),
        });

        manager.register_api_schema_adapter(schema_name.clone(), adapter);
        assert_eq!(manager.api_schema_adapter_count(), 1);

        let found = manager.lookup_api_schema_adapter(&schema_name);
        assert!(found.is_some());
    }

    #[test]
    fn test_lookup_adapters_from_prim() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        stage.define_prim("/Mesh", "Mesh").expect("define prim");
        let prim = stage
            .get_prim_at_path(&usd_sdf::Path::from_string("/Mesh").unwrap())
            .expect("prim exists");

        let manager = AdapterManager::new();
        let entry = manager.lookup_adapters(&prim);

        assert!(entry.prim_adapter.is_none());
        assert!(!entry.all_adapters.is_empty());
    }

    #[test]
    fn test_lookup_adapters_with_registry_keeps_real_prim_adapter() {
        let manager = AdapterManager::new_with_registry(Arc::new(
            super::super::adapter_registry::AdapterRegistry::new_with_defaults(),
        ));
        let mesh_type = Token::new("Mesh");

        let entry = manager.lookup_adapters_by_type(&mesh_type);
        assert!(entry.prim_adapter.is_some());
        assert!(!entry.all_adapters.is_empty());
    }

    #[test]
    fn test_base_prim_fallback_provides_xform_data_for_plain_xform_prims() {
        let stage = Stage::create_in_memory(usd_core::common::InitialLoadSet::LoadAll)
            .expect("create stage");
        let prim = stage.define_prim("/Xf", "Xform").expect("define prim");
        let translate_op = usd_geom::xformable::Xformable::new(prim.clone()).add_translate_op(
            usd_geom::XformOpPrecision::Double,
            None,
            false,
        );
        translate_op.set(
            usd_gf::Vec3d::new(1.0, 2.0, 3.0),
            usd_vt::TimeCode::default(),
        );

        let manager = AdapterManager::new_with_registry(Arc::new(
            super::super::adapter_registry::AdapterRegistry::new_with_defaults(),
        ));
        let entry = manager.lookup_adapters(&prim);
        let globals: DataSourceStageGlobalsHandle = Arc::new(NoOpStageGlobals::default());

        assert!(entry.prim_adapter.is_none());
        let data = entry.all_adapters[0]
            .adapter
            .get_imaging_subprim_data(&prim, &Token::new(""), &Token::new(""), &globals)
            .expect("base prim fallback datasource");
        assert!(data.get(&Token::new("xform")).is_some());
    }

    #[test]
    fn test_adapter_entry() {
        let schema_name = Token::new("TestAPI");
        let adapter = Arc::new(TestApiSchemaAdapter {
            schema_name: schema_name.clone(),
        });

        let entry = AdapterEntry::new(adapter.clone(), Token::new("instance1"));
        assert_eq!(entry.applied_instance_name.as_str(), "instance1");

        let single = AdapterEntry::single_apply(adapter);
        assert_eq!(single.applied_instance_name.as_str(), "");
    }

    #[test]
    fn test_adapters_entry() {
        let entry = AdaptersEntry::empty();
        assert!(entry.all_adapters.is_empty());
        assert!(entry.prim_adapter.is_none());
    }
}
