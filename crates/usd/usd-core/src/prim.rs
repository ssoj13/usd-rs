//! UsdPrim - composed prim on a stage.

use super::attribute::Attribute;
use super::object::{Object, Stage};
use super::prim_data::PrimData;
use super::prim_flags::{PrimFlags, PrimFlagsPredicate};
use super::relationship::Relationship;
use std::sync::{Arc, Weak};
use usd_sdf::{Path, Specifier};
use usd_tf::string_utils::dictionary_less_than;
use usd_tf::Token;

// ============================================================================
// Prim
// ============================================================================

/// A composed prim on a stage.
///
/// UsdPrim provides access to a prim's composed data on a stage. It represents
/// the result of composing all opinions from all layers in the layer stack.
///
/// # Examples
///
/// ```rust,ignore
/// use usd_core::UsdStage;
///
/// let stage = UsdStage::open("scene.usda")?;
/// let prim = stage.get_prim_at_path("/World/Cube")?;
///
/// // Check prim type
/// println!("Type: {}", prim.type_name());
///
/// // Iterate children
/// for child in prim.children() {
///     println!("Child: {}", child.name());
/// }
///
/// // Get an attribute
/// if let Some(attr) = prim.get_attribute("size") {
///     let size: f64 = attr.get(TimeCode::default())?;
/// }
/// ```
#[derive(Clone)]
pub struct Prim {
    /// Base object data.
    inner: Object,
    /// Cached prim data (may be None for lazy-created prims).
    prim_data: Option<Arc<PrimData>>,
    /// Proxy prim path for instance proxies (empty for non-proxy prims).
    proxy_prim_path: Path,
}

impl std::fmt::Debug for Prim {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Prim")
            .field("path", &self.inner.path())
            .field("type_name", &self.type_name())
            .field("valid", &self.is_valid())
            .finish()
    }
}

impl Prim {
    /// Creates a new prim (lazy - will query stage for data).
    pub(crate) fn new(stage: Weak<Stage>, path: Path) -> Self {
        Self {
            inner: Object::new(stage, path),
            prim_data: None,
            proxy_prim_path: Path::empty(),
        }
    }

    /// Creates a new prim with type (for authoring).
    /// Used in tests to construct a typed prim without a stage.
    #[allow(dead_code)]
    pub(crate) fn new_with_type(stage: Weak<Stage>, path: Path, type_name: Token) -> Self {
        // Create temporary PrimData for authored prims
        let data = Arc::new(PrimData::new(
            path.clone(),
            stage.clone(),
            type_name,
            Specifier::Def,
        ));
        Self {
            inner: Object::new(stage, path),
            prim_data: Some(data),
            proxy_prim_path: Path::empty(),
        }
    }

    /// Creates a prim from PrimData.
    pub(crate) fn from_data(stage: Weak<Stage>, data: Arc<PrimData>) -> Self {
        Self {
            inner: Object::new(stage, data.path().clone()),
            prim_data: Some(data),
            proxy_prim_path: Path::empty(),
        }
    }

    /// Creates a prim from PrimData with proxy path (for instance proxies).
    pub(crate) fn from_data_with_proxy(
        stage: Weak<Stage>,
        data: Arc<PrimData>,
        proxy_path: Path,
    ) -> Self {
        Self {
            inner: Object::new(stage, data.path().clone()),
            prim_data: Some(data),
            proxy_prim_path: proxy_path,
        }
    }

    /// Creates an invalid prim.
    pub fn invalid() -> Self {
        Self {
            inner: Object::invalid(),
            prim_data: None,
            proxy_prim_path: Path::empty(),
        }
    }

    /// Returns the proxy prim path (for instance proxies).
    ///
    /// Matches C++ `_ProxyPrimPath()`.
    pub(crate) fn proxy_prim_path(&self) -> &Path {
        &self.proxy_prim_path
    }

    /// Gets the PrimData, querying stage if necessary.
    pub fn data(&self) -> Option<Arc<PrimData>> {
        if let Some(stage) = self.inner.stage() {
            if let Some(data) = stage.get_prim_data(self.inner.path()) {
                return Some(data);
            }
        }

        self.prim_data.clone()
    }

    /// Returns true if this prim is valid.
    ///
    /// Matches C++ `UsdObject::IsValid()`: requires a non-null, non-dead
    /// `Usd_PrimDataHandle`. A prim is valid only if it has `PrimData` that
    /// is itself valid (not dead and its stage is still alive).
    /// The blanket `stage().is_some()` fallback is intentionally removed
    /// because a live stage alone does not imply the prim path exists.
    pub fn is_valid(&self) -> bool {
        if !self.inner.is_valid() {
            return false;
        }
        // Primary source: PrimData obtained from the stage's prim map.
        if let Some(d) = self.data() {
            return d.is_valid();
        }
        // Secondary source: locally cached PrimData (e.g. prims built via
        // `from_data()` or `new_with_type()` without a stage round-trip).
        self.prim_data
            .as_ref()
            .map(|d| d.is_valid())
            .unwrap_or(false)
    }

    /// Returns the path to this prim.
    /// For instance proxies, returns the proxy path (instance child path)
    /// instead of the prototype path. Matches C++ UsdObject::GetPath().
    pub fn path(&self) -> &Path {
        if !self.proxy_prim_path.is_empty() {
            return &self.proxy_prim_path;
        }
        self.inner.path()
    }

    /// Returns the path to this prim (alias for path()).
    ///
    /// Matches C++ `GetPath()` naming convention.
    #[inline]
    pub fn get_path(&self) -> &Path {
        self.path()
    }

    /// Returns the prim index for this prim.
    ///
    /// Matches C++ `Usd_PrimData::GetPrimIndex()`: for prototype ROOT prims
    /// returns None (C++ returns empty dummy), for all other prims (including
    /// prototype children and instance proxies) returns the source prim index.
    pub fn prim_index(&self) -> Option<usd_pcp::PrimIndex> {
        let data = self.data()?;
        // C++ GetPrimIndex(): prototype root returns empty dummy index.
        if data.is_prototype() {
            return None;
        }
        self.source_prim_index()
    }

    /// Returns the source prim index as `Arc<PrimIndex>` (cheap reference-counted handle).
    ///
    /// Unlike `prim_index()` which deep-clones the PrimIndex from PcpCache on every call,
    /// this returns a shared handle. Used by hot paths like `get_resolve_info` that only
    /// need read access to the composition graph.
    pub fn prim_index_arc(&self) -> Option<std::sync::Arc<usd_pcp::PrimIndex>> {
        let data = self.data()?;
        if data.is_prototype() {
            return None;
        }
        let stage = self.inner.stage()?;
        let pcp_cache = stage.pcp_cache()?;
        let source_path = data.source_prim_index_path();
        pcp_cache.get_or_compute_prim_index_arc(&source_path)
    }

    /// Returns the source prim index for this prim.
    ///
    /// Matches C++ `Usd_PrimData::GetSourcePrimIndex()`: always returns the
    /// stored prim index, even for prototype root prims. Uses the
    /// `source_prim_index_path` stored on PrimData during composition.
    pub fn source_prim_index(&self) -> Option<usd_pcp::PrimIndex> {
        let Some(stage) = self.inner.stage() else {
            return None;
        };
        let Some(pcp_cache) = stage.pcp_cache() else {
            return None;
        };
        // Use source_prim_index_path from PrimData (set during composition).
        // For normal prims this equals the prim path.
        // For prototype prims/children this is the source prim's path.
        let source_path = self.data()?.source_prim_index_path();
        let (prim_index, _errors) = pcp_cache.compute_prim_index(&source_path);
        Some(prim_index)
    }

    /// Maps a source-space path from a prototype-owned property back into the
    /// current prototype subtree.
    ///
    /// Matches C++ `UsdPrim::_GetProtoToInstancePathMap()` +
    /// `_ProtoToInstancePathMap::MapProtoToInstance()`.
    pub(crate) fn map_prototype_source_path_to_current(&self, source_path: &Path) -> Path {
        if !self.is_in_prototype() && !self.is_instance_proxy() {
            return source_path.clone();
        }

        let mut prim = if self.is_instance() {
            self.parent()
        } else {
            self.clone()
        };

        let mut best_match: Option<(Path, Path)> = None;
        while prim.is_valid() {
            let prototype = if prim.is_instance() {
                prim.get_prototype()
            } else if prim.is_prototype() {
                prim.clone()
            } else {
                Prim::invalid()
            };

            if prototype.is_valid() {
                if let Some(source_index) = prototype.source_prim_index() {
                    let source_prefix = source_index.path().clone();
                    if source_path.has_prefix(&source_prefix) {
                        let replace = match &best_match {
                            Some((best_prefix, _)) => {
                                source_prefix.as_str().len() > best_prefix.as_str().len()
                            }
                            None => true,
                        };
                        if replace {
                            best_match = Some((source_prefix, prim.path().clone()));
                        }
                    }
                }
            }

            prim = prim.parent();
        }

        if let Some((source_prefix, current_prefix)) = best_match {
            if source_path.is_property_path() {
                let mapped_prim = source_path
                    .get_prim_path()
                    .replace_prefix(&source_prefix, &current_prefix)
                    .unwrap_or_else(|| source_path.get_prim_path());
                return mapped_prim
                    .append_property(source_path.get_name())
                    .unwrap_or_else(|| source_path.clone());
            }
            return source_path
                .replace_prefix(&source_prefix, &current_prefix)
                .unwrap_or_else(|| source_path.clone());
        }

        source_path.clone()
    }

    /// Returns the stage that owns this prim.
    pub fn stage(&self) -> Option<Arc<Stage>> {
        self.inner.stage()
    }

    /// Returns the name of this prim.
    pub fn name(&self) -> Token {
        Token::new(self.inner.name())
    }

    /// Alias for name() matching C++ GetName().
    #[inline]
    pub fn get_name(&self) -> Token {
        self.name()
    }

    /// Returns the type name of this prim.
    pub fn type_name(&self) -> Token {
        self.data()
            .map(|d| d.type_name().clone())
            .unwrap_or_else(|| Token::new(""))
    }

    /// Returns the type name of this prim (alias for type_name()).
    ///
    /// Matches C++ `GetTypeName()` naming convention.
    #[inline]
    pub fn get_type_name(&self) -> Token {
        self.type_name()
    }

    /// Returns the full type information for this prim.
    ///
    /// The prim's type info provides a more complete picture of the prim's
    /// type including any applied API schemas. This value is cached and
    /// efficient to query.
    pub fn get_prim_type_info(&self) -> super::prim_data::PrimTypeInfo {
        self.data()
            .map(|d| d.prim_type_info().clone())
            .unwrap_or_default()
    }

    /// Returns true if this prim has the given type or a derived type.
    ///
    /// Checks the schema hierarchy via SchemaRegistry to determine if the
    /// prim's type derives from the given schema type.
    pub fn is_a(&self, type_name: &Token) -> bool {
        let prim_type = self.type_name();
        // Exact match
        if &prim_type == type_name {
            return true;
        }
        // Empty type name means untyped prim - matches nothing
        if prim_type.is_empty() {
            return false;
        }
        use super::schema_registry::SchemaRegistry;
        SchemaRegistry::prim_type_is_a_query(&prim_type, type_name.as_str())
    }

    /// Returns true if this prim has the given API schema applied.
    pub fn has_api(&self, api_name: &Token) -> bool {
        self.data()
            .map(|d| d.prim_type_info().applied_api_schemas().contains(api_name))
            .unwrap_or(false)
    }

    // ========================================================================
    // Flags
    // ========================================================================

    /// Returns the prim flags.
    pub fn flags(&self) -> PrimFlags {
        self.data().map(|d| d.flags()).unwrap_or(PrimFlags::empty())
    }

    /// Returns true if this prim is active.
    pub fn is_active(&self) -> bool {
        self.data().map(|d| d.is_active()).unwrap_or(false)
    }

    /// Returns true if this prim is loaded.
    pub fn is_loaded(&self) -> bool {
        self.data().map(|d| d.is_loaded()).unwrap_or(false)
    }

    /// Returns true if this prim is a model.
    pub fn is_model(&self) -> bool {
        self.data().map(|d| d.is_model()).unwrap_or(false)
    }

    /// Returns true if this prim is a group.
    pub fn is_group(&self) -> bool {
        self.data().map(|d| d.is_group()).unwrap_or(false)
    }

    /// Returns true if this prim is abstract.
    pub fn is_abstract(&self) -> bool {
        self.data().map(|d| d.is_abstract()).unwrap_or(false)
    }

    /// Returns true if this prim is defined (has a def, not just over).
    pub fn is_defined(&self) -> bool {
        self.data().map(|d| d.is_defined()).unwrap_or(false)
    }

    /// Returns true if this prim has a payload.
    pub fn has_payload(&self) -> bool {
        self.data().map(|d| d.has_payload()).unwrap_or(false)
    }

    // ========================================================================
    // Instancing
    // ========================================================================

    /// Returns true if this prim has been marked as instanceable.
    ///
    /// Note that this is not the same as IsInstance(). A prim may return
    /// true for IsInstanceable() and false for IsInstance() if this prim
    /// is not active or if it is marked as instanceable but contains no
    /// instanceable data.
    ///
    /// Matches C++ `IsInstanceable()`.
    pub fn is_instanceable(&self) -> bool {
        self.get_metadata::<bool>(&Token::new("instanceable"))
            .unwrap_or(false)
    }

    /// Author 'instanceable' metadata for this prim at the current EditTarget.
    ///
    /// Matches C++ `SetInstanceable(bool instanceable)`.
    pub fn set_instanceable(&self, instanceable: bool) -> bool {
        self.set_metadata(
            &Token::new("instanceable"),
            usd_vt::Value::from(instanceable),
        )
    }

    /// Remove the authored 'instanceable' opinion at the current EditTarget.
    ///
    /// Matches C++ `ClearInstanceable()`.
    pub fn clear_instanceable(&self) -> bool {
        self.clear_metadata(&Token::new("instanceable"))
    }

    /// Return true if this prim has an authored opinion for 'instanceable'.
    ///
    /// Matches C++ `HasAuthoredInstanceable()`.
    pub fn has_authored_instanceable(&self) -> bool {
        self.has_authored_metadata(&Token::new("instanceable"))
    }

    /// Returns true if this prim is an instance.
    ///
    /// Matches C++ `IsInstance()`.
    pub fn is_instance(&self) -> bool {
        self.data().map(|d| d.is_instance()).unwrap_or(false)
    }

    /// Returns true if this prim is an instance proxy.
    ///
    /// Instance proxies are virtual prims that represent the contents of
    /// a prototype as seen through a specific instance.
    ///
    /// Matches C++ `IsInstanceProxy()`.
    pub fn is_instance_proxy(&self) -> bool {
        // Instance proxy detection: check if prim has a proxy path
        // In C++, this uses Usd_IsInstanceProxy(_Prim(), _ProxyPrimPath())
        !self.proxy_prim_path.is_empty()
    }

    /// Returns true if the given path identifies a prototype prim.
    ///
    /// Matches C++ static `IsPrototypePath(const SdfPath& path)`.
    pub fn is_prototype_path(path: &Path) -> bool {
        super::instance_cache::InstanceCache::is_prototype_path(path)
    }

    /// Returns true if the given path identifies a prototype prim or
    /// a prim or property descendant of a prototype prim.
    ///
    /// Matches C++ static `IsPathInPrototype(const SdfPath& path)`.
    pub fn is_path_in_prototype(path: &Path) -> bool {
        super::instance_cache::InstanceCache::is_path_in_prototype(path)
    }

    /// Returns true if this prim is an instancing prototype prim.
    ///
    /// Matches C++ `IsPrototype()`.
    pub fn is_prototype(&self) -> bool {
        self.data().map(|d| d.is_prototype()).unwrap_or(false)
    }

    /// Returns true if this prim is a prototype prim or a descendant
    /// of a prototype prim.
    ///
    /// Matches C++ `IsInPrototype()`.
    pub fn is_in_prototype(&self) -> bool {
        if self.is_instance_proxy() {
            Self::is_path_in_prototype(self.path())
        } else {
            self.data().map(|d| d.is_in_prototype()).unwrap_or(false)
        }
    }

    /// If this prim is an instance, return the UsdPrim for the corresponding
    /// prototype. Otherwise, return an invalid UsdPrim.
    ///
    /// Matches C++ `GetPrototype()`.
    pub fn get_prototype(&self) -> Prim {
        if let Some(stage) = self.inner.stage() {
            if let Some(prim_data) = self.data() {
                if let Some(prototype_data) = stage.get_prototype_for_instance(&prim_data) {
                    return Prim::from_data(self.inner.stage.clone(), prototype_data);
                }
            }
        }
        Prim::invalid()
    }

    /// If this prim is an instance proxy, return the UsdPrim for the
    /// corresponding prim in the instance's prototype. Otherwise, return an
    /// invalid UsdPrim.
    ///
    /// Matches C++ `GetPrimInPrototype()`.
    pub fn get_prim_in_prototype(&self) -> Prim {
        if self.is_instance_proxy() {
            // C++ returns UsdPrim(_Prim(), SdfPath()) -- same PrimData but empty proxy path.
            // Construct a Prim from the underlying data without the proxy path.
            if let Some(data) = &self.prim_data {
                return Prim::from_data(self.inner.stage.clone(), data.clone());
            }
        }
        Prim::invalid()
    }

    /// If this prim is a prototype prim, returns all prims that are instances
    /// of this prototype. Otherwise, returns an empty vector.
    ///
    /// Matches C++ `GetInstances()`.
    pub fn get_instances(&self) -> Vec<Prim> {
        if let Some(stage) = self.inner.stage() {
            return stage.get_instances_for_prototype(self);
        }
        Vec::new()
    }

    /// Returns true if this prim is the pseudo-root.
    pub fn is_pseudo_root(&self) -> bool {
        self.data().map(|d| d.is_pseudo_root()).unwrap_or(false)
    }

    // ========================================================================
    // Hierarchy
    // ========================================================================

    /// Returns the parent prim.
    pub fn parent(&self) -> Prim {
        if self.is_instance_proxy() {
            let parent_path = self.path().get_parent_path();
            if parent_path.is_empty() {
                return Prim::invalid();
            }
            if let Some(stage) = self.inner.stage() {
                if let Some(parent) = stage.get_prim_at_path(&parent_path) {
                    return parent;
                }
            }
            return Prim::new(self.inner.stage.clone(), parent_path);
        }

        if let Some(data) = self.data() {
            if let Some(parent_data) = data.parent() {
                let stage = self.inner.stage.clone();
                return Prim::from_data(stage, parent_data);
            }
        }

        // Fallback to path-based parent
        let parent_path = self.inner.path().get_parent_path();
        if parent_path.is_empty() {
            Prim::invalid()
        } else {
            Prim::new(self.inner.stage.clone(), parent_path)
        }
    }

    /// Returns the children of this prim.
    /// Returns all children of this prim (including inactive and unloaded).
    ///
    /// Matches C++ `GetAllChildren()`.
    /// Returns true if this prim has any children (lightweight, no allocation).
    /// For instance prims, checks the prototype's children.
    /// Matches C++ Usd_MoveToChild: if src->IsInstance(), src = src->GetPrototype().
    pub fn has_children(&self) -> bool {
        let Some(data) = self.data() else {
            return false;
        };
        // Instance prims have no direct children — check prototype.
        if data.is_instance() {
            if let Some(stage) = self.inner.stage() {
                if let Some(proto_data) = stage.get_prototype_for_instance(&data) {
                    return proto_data.first_child().is_some();
                }
            }
            return false;
        }
        data.first_child().is_some()
    }

    pub fn get_all_children(&self) -> Vec<Prim> {
        let Some(data) = self.data() else {
            return Vec::new();
        };
        let stage_weak = self.inner.stage.clone();

        // OpenUSD destroys composed descendants when a prim is deactivated, so
        // child enumeration must not leak stale cached links through older
        // UsdPrim handles after SetActive(false).
        if !data.is_active() {
            return Vec::new();
        }

        // For instance prims: children come from the prototype, returned as
        // instance proxy prims with paths under the instance.
        // Matches C++ Usd_MoveToChild (primData.h:593-603).
        if data.is_instance() {
            if let Some(stage) = stage_weak.upgrade() {
                if let Some(proto_data) = stage.get_prototype_for_instance(&data) {
                    let my_path = self.path().clone();
                    return proto_data
                        .children()
                        .into_iter()
                        .filter_map(|child_data| {
                            let child_name = child_data.name();
                            let proxy_path = my_path.append_child(child_name.as_str())?;
                            Some(Prim::from_data_with_proxy(
                                stage_weak.clone(),
                                child_data,
                                proxy_path,
                            ))
                        })
                        .collect();
                }
            }
            return Vec::new();
        }

        // For instance proxy prims: children also come from prototype data,
        // with proxy paths extended under the instance.
        if self.is_instance_proxy() {
            let proxy_path = self.proxy_prim_path.clone();
            return data
                .children()
                .into_iter()
                .filter_map(|child_data| {
                    let child_name = child_data.name();
                    let child_proxy = proxy_path.append_child(child_name.as_str())?;
                    Some(Prim::from_data_with_proxy(
                        stage_weak.clone(),
                        child_data,
                        child_proxy,
                    ))
                })
                .collect();
        }

        data.children()
            .into_iter()
            .map(|child_data| Prim::from_data(stage_weak.clone(), child_data))
            .collect()
    }

    /// Returns the direct children of this prim matching the default predicate
    /// (active, defined, loaded, not abstract).
    ///
    /// Matches C++ `GetChildren()` which applies `UsdPrimDefaultPredicate`.
    pub fn children(&self) -> Vec<Prim> {
        let pred = super::prim_flags::default_predicate().into_predicate();
        self.get_all_children()
            .into_iter()
            .filter(|p| pred.matches(p.flags()))
            .collect()
    }

    /// Returns the direct children of this prim (alias for children()).
    ///
    /// Matches C++ `GetChildren()` naming convention.
    #[inline]
    pub fn get_children(&self) -> Vec<Prim> {
        self.children()
    }

    /// Returns the children matching the given predicate.
    pub fn children_filtered(&self, predicate: PrimFlagsPredicate) -> Vec<Prim> {
        self.get_all_children()
            .into_iter()
            .filter(|p| predicate.matches(p.flags()))
            .collect()
    }

    /// Returns default-filtered descendant prims (active, loaded, defined, non-abstract).
    ///
    /// Matches C++ `GetDescendants()` which uses `UsdPrimDefaultPredicate`.
    pub fn descendants(&self) -> Vec<Prim> {
        let mut result = Vec::new();
        self.collect_descendants(&mut result);
        result
    }

    fn collect_descendants(&self, result: &mut Vec<Prim>) {
        for child in self.children() {
            result.push(child.clone());
            child.collect_descendants(result);
        }
    }

    /// Returns default-filtered descendants (alias for `descendants()`).
    ///
    /// Matches C++ `GetDescendants()` naming convention.
    #[inline]
    pub fn get_descendants(&self) -> Vec<Prim> {
        self.descendants()
    }

    /// Returns all descendant prims (unfiltered).
    ///
    /// Matches C++ `GetAllDescendants()` which uses `UsdPrimAllPrimsPredicate`.
    pub fn get_all_descendants(&self) -> Vec<Prim> {
        let mut result = Vec::new();
        self.collect_all_descendants(&mut result);
        result
    }

    fn collect_all_descendants(&self, result: &mut Vec<Prim>) {
        for child in self.get_all_children() {
            result.push(child.clone());
            child.collect_all_descendants(result);
        }
    }

    /// Returns the number of children.
    /// For instance prims, counts the prototype's children.
    pub fn child_count(&self) -> usize {
        let Some(data) = self.data() else {
            return 0;
        };
        if data.is_instance() {
            if let Some(stage) = self.inner.stage() {
                if let Some(proto_data) = stage.get_prototype_for_instance(&data) {
                    return proto_data.children().len();
                }
            }
            return 0;
        }
        data.children().len()
    }

    /// Returns filtered children matching the given predicate.
    ///
    /// Matches C++ `GetFilteredChildren(const Usd_PrimFlagsPredicate &predicate)`.
    /// Filters from all children (not just default-predicate children).
    pub fn get_filtered_children(&self, predicate: PrimFlagsPredicate) -> Vec<Prim> {
        self.get_all_children()
            .into_iter()
            .filter(|p| predicate.matches(p.flags()))
            .collect()
    }

    /// Returns all filtered descendants matching the given predicate.
    ///
    /// Matches C++ `GetFilteredDescendants(const Usd_PrimFlagsPredicate &predicate)`.
    pub fn get_filtered_descendants(&self, predicate: PrimFlagsPredicate) -> Vec<Prim> {
        let mut result = Vec::new();
        self.collect_filtered_descendants(predicate, &mut result);
        result
    }

    fn collect_filtered_descendants(&self, predicate: PrimFlagsPredicate, result: &mut Vec<Prim>) {
        // Predicate controls both which prims are included AND which subtrees
        // are traversed - matching C++ _MakeDescendantsRange behavior.
        for child in self.get_filtered_children(predicate) {
            result.push(child.clone());
            child.collect_filtered_descendants(predicate, result);
        }
    }

    /// Returns the next sibling of this prim.
    ///
    /// Matches C++ `GetNextSibling()`.
    pub fn get_next_sibling(&self) -> Prim {
        let parent = self.parent();
        if !parent.is_valid() {
            return Prim::invalid();
        }

        let children = parent.children();
        let my_path = self.path();

        // Find this prim in parent's children and return next
        for (i, child) in children.iter().enumerate() {
            if child.path() == my_path && i + 1 < children.len() {
                return children[i + 1].clone();
            }
        }

        Prim::invalid()
    }

    /// Returns the prim stack for this prim.
    ///
    /// The prim stack is the list of prim specs that contribute to this prim's composition,
    /// ordered from strongest to weakest opinion.
    ///
    /// Matches C++ `GetPrimStack()`.
    pub fn get_prim_stack(&self) -> Vec<usd_sdf::PrimSpec> {
        let mut stack = Vec::new();

        // Use PrimIndex-based Resolver (C++ UsdStage::_GetPrimStack):
        // walks nodes in strong-to-weak order, uses local path per node.
        if let Some(prim_index) = self.prim_index().map(std::sync::Arc::new) {
            let mut resolver = super::resolver::Resolver::new(&prim_index, false);
            while resolver.is_valid() {
                if let (Some(layer), Some(local_path)) =
                    (resolver.get_layer(), resolver.get_local_path())
                {
                    if let Some(spec) = layer.get_prim_at_path(&local_path) {
                        stack.push(spec);
                    }
                }
                resolver.next_layer();
            }
        } else if let Some(stage) = self.inner.stage() {
            // Fallback: root layer stack (in-memory stages)
            for layer in stage.layer_stack() {
                if let Some(spec) = layer.get_prim_at_path(self.path()) {
                    stack.push(spec);
                }
            }
        }

        stack
    }

    /// Returns the property order for this prim.
    ///
    /// Matches C++ `GetPropertyOrder()`.
    pub fn get_property_order(&self) -> Vec<Token> {
        // Get propertyOrder metadata field
        self.get_metadata::<Vec<Token>>(&Token::new("propertyOrder"))
            .unwrap_or_default()
    }

    /// Sets the property order for this prim.
    ///
    /// Matches C++ `SetPropertyOrder(const TfTokenVector &order)`.
    pub fn set_property_order(&self, order: Vec<Token>) -> bool {
        self.set_metadata(&Token::new("propertyOrder"), usd_vt::Value::from(order))
    }

    /// Clears the property order for this prim (matches C++ ClearPropertyOrder).
    pub fn clear_property_order(&self) -> bool {
        self.clear_metadata(&Token::new("propertyOrder"))
    }

    // ========================================================================
    // Properties
    // ========================================================================

    /// Gets an attribute by name.
    ///
    /// Returns an attribute handle when the resolved property exists, or for
    /// namespaced properties where callers commonly need a path-preserving handle
    /// even before authoring a spec (for example `primvars:*`, `inputs:*`, etc.).
    pub fn get_attribute_handle(&self, name: &str) -> Option<Attribute> {
        let attr_path = self.path().append_property(name)?;
        Some(Attribute::new(self.inner.stage.clone(), attr_path))
    }

    pub fn get_attribute(&self, name: &str) -> Option<Attribute> {
        let attr = self.get_attribute_handle(name)?;
        let spec_type = self
            .inner
            .stage()
            .map(|stage| stage.get_defining_spec_type(self.path(), name))
            .unwrap_or(usd_sdf::SpecType::Unknown);
        if spec_type == usd_sdf::SpecType::Relationship {
            return None;
        }
        if spec_type == usd_sdf::SpecType::Attribute {
            return Some(attr);
        }
        if attr.is_valid() {
            return Some(attr);
        }
        if spec_type == usd_sdf::SpecType::Unknown {
            if name.contains(':') {
                return Some(attr);
            }
            let tn = self.get_type_name();
            if !tn.is_empty()
                && super::schema_registry::get_schema_property_names(&tn)
                    .iter()
                    .any(|t| t.as_str() == name)
                && !super::schema_registry::schema_builtin_relationship_property_name(name)
            {
                return Some(attr);
            }
        }
        None
    }

    /// Returns true if this prim has an authored attribute spec in the root layer.
    ///
    /// Unlike `has_attribute()` (which checks the composed property cache),
    /// this checks the actual layer spec — useful in schema create_*_attr
    /// helpers to avoid skipping create_attribute() calls.
    pub fn has_authored_attribute(&self, name: &str) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let Some(attr_path) = self.path().append_property(name) else {
            return false;
        };
        stage
            .root_layer()
            .get_attribute_at_path(&attr_path)
            .is_some()
    }

    /// Gets a relationship by name.
    pub fn get_relationship(&self, name: &str) -> Option<Relationship> {
        let rel_path = self.path().append_property(name)?;
        let rel = Relationship::new(self.inner.stage.clone(), rel_path);
        let spec_type = self
            .inner
            .stage()
            .map(|stage| stage.get_defining_spec_type(self.path(), name))
            .unwrap_or(usd_sdf::SpecType::Unknown);
        if spec_type == usd_sdf::SpecType::Attribute {
            return None;
        }
        if rel.is_valid() || (spec_type == usd_sdf::SpecType::Unknown && name.contains(':')) {
            Some(rel)
        } else {
            None
        }
    }

    /// Returns true if the prim has an attribute with the given name.
    ///
    /// Uses PrimIndex-based resolver walk to find specs across all composed
    /// layers (including payloads, references). Matches C++ parity.
    pub fn has_attribute(&self, name: &str) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let prim_path = if self.is_instance_proxy() {
            self.data()
                .map(|data| data.path().clone())
                .unwrap_or_else(|| self.path().clone())
        } else {
            self.path().clone()
        };
        stage.get_defining_spec_type(&prim_path, name) == usd_sdf::SpecType::Attribute
    }

    /// Returns true if the prim has a relationship with the given name.
    ///
    /// Uses PrimIndex-based resolver walk to find specs across all composed
    /// layers (including payloads, references). Matches C++ parity.
    pub fn has_relationship(&self, name: &str) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let prim_path = if self.is_instance_proxy() {
            self.data()
                .map(|data| data.path().clone())
                .unwrap_or_else(|| self.path().clone())
        } else {
            self.path().clone()
        };
        stage.get_defining_spec_type(&prim_path, name) == usd_sdf::SpecType::Relationship
    }

    /// Returns all attribute names.
    ///
    /// Uses PrimIndex-based resolver walk to detect spec type across all
    /// composed layers (C++ parity via `_GetDefiningSpecType`).
    pub fn get_attribute_names(&self) -> Vec<Token> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };
        let type_name = self.get_type_name();
        let schema_props: std::collections::HashSet<Token> = if type_name.is_empty() {
            std::collections::HashSet::new()
        } else {
            super::schema_registry::get_schema_property_names(&type_name)
                .into_iter()
                .collect()
        };

        self.get_property_names()
            .into_iter()
            .filter(|name| {
                let st = stage.get_defining_spec_type(self.path(), name.get_text());
                if st == usd_sdf::SpecType::Relationship {
                    return false;
                }
                if st == usd_sdf::SpecType::Attribute {
                    return true;
                }
                st == usd_sdf::SpecType::Unknown
                    && schema_props.contains(name)
                    && !super::schema_registry::schema_builtin_relationship_property_name(name.as_str())
            })
            .collect()
    }

    /// Returns all relationship names.
    ///
    /// Uses PrimIndex-based resolver walk to detect spec type across all
    /// composed layers (C++ parity via `_GetDefiningSpecType`).
    pub fn get_relationship_names(&self) -> Vec<Token> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };
        let type_name = self.get_type_name();
        let schema_props: std::collections::HashSet<Token> = if type_name.is_empty() {
            std::collections::HashSet::new()
        } else {
            super::schema_registry::get_schema_property_names(&type_name)
                .into_iter()
                .collect()
        };

        self.get_property_names()
            .into_iter()
            .filter(|name| {
                let st = stage.get_defining_spec_type(self.path(), name.get_text());
                if st == usd_sdf::SpecType::Relationship {
                    return true;
                }
                st == usd_sdf::SpecType::Unknown
                    && schema_props.contains(name)
                    && super::schema_registry::schema_builtin_relationship_property_name(name.as_str())
            })
            .collect()
    }

    /// Returns all attributes as Attribute objects.
    pub fn get_attributes(&self) -> Vec<Attribute> {
        self.get_attribute_names()
            .into_iter()
            .filter_map(|name| self.get_attribute(name.get_text()))
            .collect()
    }

    /// Returns all relationships as Relationship objects.
    pub fn get_relationships(&self) -> Vec<Relationship> {
        self.get_relationship_names()
            .into_iter()
            .filter_map(|name| self.get_relationship(name.get_text()))
            .collect()
    }

    /// Returns all property names (attributes and relationships).
    pub fn get_property_names(&self) -> Vec<Token> {
        let Some(stage) = self.inner.stage() else {
            return self.data().map(|d| d.property_names()).unwrap_or_default();
        };

        let mut names = Vec::new();
        let mut seen = std::collections::HashSet::new();

        if let Some(prim_index) = self.prim_index() {
            for name in prim_index.compute_prim_property_names() {
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
        } else if let Some(data) = self.data() {
            for name in data.property_names() {
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
        }

        // Builtin schema properties (PrimDefinition), matching C++ `UsdPrim::GetProperties`.
        let type_name = self.get_type_name();
        if !type_name.is_empty() {
            for name in super::schema_registry::get_schema_property_names(&type_name) {
                if seen.insert(name.clone()) {
                    names.push(name);
                }
            }
        }

        names.sort_by(|a, b| {
            let a_str = a.as_str();
            let b_str = b.as_str();
            if dictionary_less_than(a_str, b_str) {
                std::cmp::Ordering::Less
            } else if dictionary_less_than(b_str, a_str) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });

        Self::apply_name_order(&self.get_property_order(), &mut names);

        let schema_names: std::collections::HashSet<Token> = if type_name.is_empty() {
            std::collections::HashSet::new()
        } else {
            super::schema_registry::get_schema_property_names(&type_name)
                .into_iter()
                .collect()
        };

        names.retain(|name| {
            schema_names.contains(name)
                || stage.get_defining_spec_type(self.path(), name.get_text())
                    != usd_sdf::SpecType::Unknown
        });
        names
    }

    fn apply_name_order(order: &[Token], names: &mut Vec<Token>) {
        if order.is_empty() || names.is_empty() {
            return;
        }

        let mut names_rest = 0usize;
        for ordered_name in order {
            if let Some(found_index) = names[names_rest..]
                .iter()
                .position(|name| name == ordered_name)
                .map(|idx| names_rest + idx)
            {
                names[names_rest..=found_index].rotate_right(1);
                names_rest += 1;
            }
        }
    }

    // ========================================================================
    // Authoring
    // ========================================================================

    /// Creates an attribute on this prim.
    ///
    /// Authors an attribute spec to the current edit target layer.
    ///
    /// Matches C++ `CreateAttribute(const TfToken& name, const SdfValueTypeName &typeName, bool custom, SdfVariability variability)`.
    pub fn create_attribute(
        &self,
        name: &str,
        type_name: &usd_sdf::ValueTypeName,
        custom: bool,
        variability: Option<crate::attribute::Variability>,
    ) -> Option<Attribute> {
        let attr_path = self.path().append_property(name)?;
        let stage = self.inner.stage()?;
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return None;
        };
        let spec_prim_path = edit_target.map_to_spec_path(self.path());
        let spec_attr_path = edit_target.map_to_spec_path(&attr_path);

        if layer.get_prim_at_path(&spec_prim_path).is_none() {
            let _ = layer.create_prim_spec(&spec_prim_path, usd_sdf::Specifier::Over, "");
        }

        // Create attribute spec in layer
        if !layer.create_spec(&spec_attr_path, usd_sdf::SpecType::Attribute) {
            return None;
        }

        // Set type name
        if type_name.is_valid() {
            let type_token = type_name.as_token();
            layer.set_field(
                &spec_attr_path,
                &Token::new("typeName"),
                usd_vt::Value::from(type_token.get_text().to_string()),
            );
        }

        // Author custom explicitly even when false so required/authored metadata
        // matches USD behavior for freshly created properties.
        layer.set_field(
            &spec_attr_path,
            &Token::new("custom"),
            usd_vt::Value::from(custom),
        );

        // Set variability (defaults to Varying if not specified)
        let var = variability.unwrap_or(crate::attribute::Variability::Varying);
        let var_str = match var {
            crate::attribute::Variability::Varying => "varying",
            crate::attribute::Variability::Uniform => "uniform",
        };
        layer.set_field(
            &spec_attr_path,
            &Token::new("variability"),
            usd_vt::Value::from(var_str),
        );

        // Add to prim's property list (in-memory cache for traverse/flatten)
        // AND to the layer's "properties" field so PrimSpec::properties() finds it.
        {
            let name_token = Token::new(name);
            if let Some(data) = self.data() {
                let mut names = data.property_names();
                if !names.iter().any(|n| n.get_text() == name) {
                    names.push(name_token.clone());
                    data.set_property_names(names);
                }
            }
            // Update layer's "properties" children field
            let prim_path = spec_prim_path.clone();
            let props_token = Token::new("properties");
            let mut prop_names: Vec<Token> = layer
                .get_field(&prim_path, &props_token)
                .and_then(|v| v.as_vec_clone::<Token>())
                .unwrap_or_default();
            if !prop_names.iter().any(|t| t == &name_token) {
                prop_names.push(name_token);
                layer.set_field(&prim_path, &props_token, usd_vt::Value::new(prop_names));
            }
        }

        Some(Attribute::new(self.inner.stage.clone(), attr_path))
    }

    /// Creates a relationship on this prim.
    ///
    /// Authors a relationship spec to the current edit target layer.
    pub fn create_relationship(&self, name: &str, custom: bool) -> Option<Relationship> {
        let rel_path = self.path().append_property(name)?;
        let stage = self.inner.stage()?;
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return None;
        };
        let spec_prim_path = edit_target.map_to_spec_path(self.path());
        let spec_rel_path = edit_target.map_to_spec_path(&rel_path);

        if layer.get_prim_at_path(&spec_prim_path).is_none() {
            let _ = layer.create_prim_spec(&spec_prim_path, usd_sdf::Specifier::Over, "");
        }

        // Create relationship spec in layer
        if !layer.create_spec(&spec_rel_path, usd_sdf::SpecType::Relationship) {
            return None;
        }

        // Author custom explicitly even when false.
        layer.set_field(
            &spec_rel_path,
            &Token::new("custom"),
            usd_vt::Value::from(custom),
        );

        // Add to prim's property list AND layer's "properties" field
        {
            let name_token = Token::new(name);
            if let Some(data) = self.data() {
                let mut names = data.property_names();
                if !names.iter().any(|n| n.get_text() == name) {
                    names.push(name_token.clone());
                    data.set_property_names(names);
                }
            }
            let prim_path = spec_prim_path.clone();
            let props_token = Token::new("properties");
            let mut prop_names: Vec<Token> = layer
                .get_field(&prim_path, &props_token)
                .and_then(|v| v.as_vec_clone::<Token>())
                .unwrap_or_default();
            if !prop_names.iter().any(|t| t == &name_token) {
                prop_names.push(name_token);
                layer.set_field(&prim_path, &props_token, usd_vt::Value::new(prop_names));
            }
        }

        Some(Relationship::new(self.inner.stage.clone(), rel_path))
    }

    /// Removes a property from this prim.
    pub fn remove_property(&self, name: &str) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };

        let Some(prop_path) = self.path().append_property(name) else {
            return false;
        };
        let spec_prop_path = edit_target.map_to_spec_path(&prop_path);

        // Delete spec from layer
        if !layer.delete_spec(&spec_prop_path) {
            return false;
        }

        // Remove from prim's property list
        if let Some(data) = self.data() {
            let names: Vec<Token> = data
                .property_names()
                .into_iter()
                .filter(|n| n.get_text() != name)
                .collect();
            data.set_property_names(names);
        }

        true
    }

    /// Returns all properties in the given namespace.
    ///
    /// Matches C++ `GetPropertiesInNamespace()`. Uses ':' delimiter to avoid
    /// false prefix matches (e.g. prefix "foo" won't match "foobar").
    ///
    /// Unlike `get_authored_properties_in_namespace`, this also includes
    /// schema-defined (built-in) property names from the SchemaPropertyRegistry,
    /// matching C++ `_GetPropertyNames(onlyAuthored=false)` which queries the
    /// PrimDefinition for builtin properties.
    pub fn get_properties_in_namespace(
        &self,
        namespace_prefix: &Token,
    ) -> Vec<super::property::Property> {
        // Collect all property names: authored + schema-defined builtins.
        let mut all_names = std::collections::HashSet::new();

        // Authored properties from prim data
        if let Some(data) = self.data() {
            for prop_name in data.property_names() {
                all_names.insert(prop_name);
            }
            // Schema-defined builtin properties (C++: PrimDefinition::GetPropertyNames)
            let type_name = data.type_name().clone();
            if !type_name.is_empty() {
                for name in super::schema_registry::get_schema_property_names(&type_name) {
                    all_names.insert(name);
                }
            }
        }

        let prefix_str = namespace_prefix.as_str();
        let stage = self.inner.stage.clone();

        // Empty prefix = return all properties (matches C++)
        if prefix_str.is_empty() {
            let mut result = Vec::new();
            for prop_name in &all_names {
                if let Some(prop_path) = self.path().append_property(prop_name.as_str()) {
                    result.push(super::property::Property::new(stage.clone(), prop_path));
                }
            }
            return result;
        }

        // C++: terminator = namespaces.size() - (last_char == delim)
        // Then checks: s.size() > terminator && starts_with && s[terminator] == ':'
        let delim = ':';
        let terminator = if prefix_str.ends_with(delim) {
            prefix_str.len() - 1
        } else {
            prefix_str.len()
        };

        let mut result = Vec::new();
        let prefix_base = prefix_str.trim_end_matches(delim);
        for prop_name in &all_names {
            let s = prop_name.as_str();
            if s.len() > terminator
                && s.starts_with(prefix_base)
                && s.as_bytes().get(terminator) == Some(&(delim as u8))
            {
                if let Some(prop_path) = self.path().append_property(s) {
                    result.push(super::property::Property::new(stage.clone(), prop_path));
                }
            }
        }
        result
    }

    /// Returns only authored properties in the given namespace.
    ///
    /// Matches C++ `GetAuthoredPropertiesInNamespace()`. Unlike `get_properties_in_namespace`,
    /// this only returns properties that have specs authored in at least one layer.
    pub fn get_authored_properties_in_namespace(
        &self,
        namespace_prefix: &Token,
    ) -> Vec<super::property::Property> {
        let Some(stage) = self.inner.stage() else {
            return Vec::new();
        };

        let prefix_str = namespace_prefix.as_str();

        // Collect authored property names from ALL layers in the stack (C++ parity).
        // C++ _GetPropertyNames(onlyAuthored=true) walks the composed prim index.
        let authored_names: std::collections::HashSet<String> = {
            let mut names = std::collections::HashSet::new();
            for layer in stage.layer_stack() {
                if let Some(prim_spec) = layer.get_prim_at_path(self.path()) {
                    for p in prim_spec.properties() {
                        names.insert(p.name().get_text().to_string());
                    }
                }
            }
            names
        };

        if authored_names.is_empty() {
            return Vec::new();
        }

        // Empty prefix = return all authored properties
        if prefix_str.is_empty() {
            let mut result = Vec::new();
            let weak = self.inner.stage.clone();
            for name in &authored_names {
                if let Some(prop_path) = self.path().append_property(name) {
                    result.push(super::property::Property::new(weak.clone(), prop_path));
                }
            }
            return result;
        }

        // Namespace delimiter filtering (same logic as get_properties_in_namespace)
        let delim = ':';
        let terminator = if prefix_str.ends_with(delim) {
            prefix_str.len() - 1
        } else {
            prefix_str.len()
        };
        let prefix_base = prefix_str.trim_end_matches(delim);

        let mut result = Vec::new();
        let weak = self.inner.stage.clone();
        for name in &authored_names {
            if name.len() > terminator
                && name.starts_with(prefix_base)
                && name.as_bytes().get(terminator) == Some(&(delim as u8))
            {
                if let Some(prop_path) = self.path().append_property(name) {
                    result.push(super::property::Property::new(weak.clone(), prop_path));
                }
            }
        }
        result
    }

    // ========================================================================
    // Metadata
    // ========================================================================

    /// Gets metadata value by key.
    /// Routes through stage layer stack for proper composition (matches C++ UsdObject::GetMetadata).
    pub fn get_metadata<T: Clone + 'static>(&self, key: &Token) -> Option<T> {
        // First check stage layers (authoritative source)
        if let Some(stage) = self.stage() {
            let value = stage.get_metadata_for_object(self.path(), key)?;
            return value
                .downcast_clone::<T>()
                .or_else(|| value.cast::<T>().and_then(|casted| casted.get::<T>().cloned()));
        }
        // Fallback to PrimData cache if no stage
        let data = self.data()?;
        let value = data.get_metadata(key)?;
        value
            .downcast_clone::<T>()
            .or_else(|| value.cast::<T>().and_then(|casted| casted.get::<T>().cloned()))
    }

    /// Sets metadata value.
    /// Routes through stage edit target layer (matches C++ UsdObject::SetMetadata).
    pub fn set_metadata(&self, key: &Token, value: impl Into<usd_vt::Value>) -> bool {
        let value = value.into();
        // Write to edit target layer via stage
        if let Some(stage) = self.stage() {
            if stage.set_metadata_for_object(self.path(), key, value.clone()) {
                // Also update PrimData cache for consistency
                if let Some(data) = self.data() {
                    data.set_metadata(key.clone(), value);
                }
                return true;
            }
            return false;
        }
        // Fallback: write to PrimData cache only (no stage)
        if let Some(data) = self.data() {
            data.set_metadata(key.clone(), value);
            return true;
        }
        false
    }

    /// Clears metadata value.
    /// Routes through stage edit target layer (matches C++ UsdObject::ClearMetadata).
    pub fn clear_metadata(&self, key: &Token) -> bool {
        if let Some(stage) = self.stage() {
            if stage.clear_metadata_for_object(self.path(), key) {
                // Also clear PrimData cache
                if let Some(data) = self.data() {
                    data.clear_metadata(key);
                }
                return true;
            }
            return false;
        }
        self.data().map(|d| d.clear_metadata(key)).unwrap_or(false)
    }

    /// Returns true if metadata is authored.
    /// Checks stage layers (matches C++ UsdObject::HasAuthoredMetadata).
    pub fn has_authored_metadata(&self, key: &Token) -> bool {
        if let Some(stage) = self.stage() {
            return stage.has_authored_metadata_for_object(self.path(), key);
        }
        self.data().map(|d| d.has_metadata(key)).unwrap_or(false)
    }

    /// Gets this prim's documentation metadata.
    pub fn get_documentation(&self) -> String {
        self.get_metadata::<String>(&Token::new("documentation"))
            .unwrap_or_default()
    }

    /// Sets this prim's documentation metadata.
    pub fn set_documentation(&self, doc: &str) -> bool {
        self.set_metadata(
            &Token::new("documentation"),
            usd_vt::Value::from(doc.to_string()),
        )
    }

    /// Returns true if this prim has authored references.
    pub fn has_authored_references(&self) -> bool {
        let refs_token = Token::new("references");
        self.has_authored_metadata(&refs_token)
    }

    /// Returns the kind of this prim.
    ///
    /// Matches C++ `UsdPrim::GetKind(TfToken* kind)`.
    /// Returns true if kind was successfully read, false otherwise.
    pub fn get_kind(&self) -> Option<Token> {
        if self.is_pseudo_root() {
            return None; // Pseudo-root cannot have kind
        }
        self.get_metadata::<String>(&Token::new("kind"))
            .map(|s| Token::new(&s))
    }

    /// Returns the kind of this prim (convenience method returning Token directly).
    ///
    /// Returns empty token if no kind is authored.
    pub fn kind(&self) -> Token {
        self.get_kind().unwrap_or_else(|| Token::new(""))
    }

    /// Sets the kind of this prim.
    ///
    /// Matches C++ `UsdPrim::SetKind(const TfToken &kind)`.
    /// Returns true if kind was successfully authored, false otherwise.
    pub fn set_kind(&self, kind: &Token) -> bool {
        if self.is_pseudo_root() {
            return false; // Pseudo-root cannot have kind
        }
        self.set_metadata(
            &Token::new("kind"),
            usd_vt::Value::from(kind.get_text().to_string()),
        )
    }

    /// Returns true if this prim is a component model based on its kind metadata.
    ///
    /// Matches C++ `UsdPrim::IsComponent()`.
    pub fn is_component(&self) -> bool {
        // In C++, this calls _Prim()->IsComponent() which uses KindRegistry
        // to check if the prim's kind is "component" or inherits from it
        let kind = self.kind();
        if kind.is_empty() {
            return false;
        }
        // Use KindRegistry to check if kind is "component" or inherits from it
        usd_kind::is_component_kind(&kind)
    }

    /// Returns the composed assetInfo dictionary.
    ///
    /// Matches C++ `UsdObject::GetAssetInfo()`.
    pub fn get_asset_info(&self) -> std::collections::HashMap<String, usd_vt::Value> {
        self.get_metadata::<std::collections::HashMap<String, usd_vt::Value>>(&Token::new(
            "assetInfo",
        ))
        .unwrap_or_default()
    }

    /// Returns the element identified by keyPath in this prim's composed assetInfo dictionary.
    ///
    /// Matches C++ `UsdObject::GetAssetInfoByKey(const TfToken &keyPath)`.
    pub fn get_asset_info_by_key(&self, key_path: &Token) -> Option<usd_vt::Value> {
        // Use GetMetadataByDictKey with "assetInfo" key
        // In C++, this calls GetMetadataByDictKey(TfToken("assetInfo"), keyPath)
        let asset_info_token = Token::new("assetInfo");
        self.get_metadata_by_dict_key(&asset_info_token, key_path)
    }

    /// Authors the element identified by keyPath in this prim's assetInfo dictionary.
    ///
    /// Matches C++ `UsdObject::SetAssetInfoByKey(const TfToken &keyPath, const VtValue &value)`.
    pub fn set_asset_info_by_key(&self, key_path: &Token, value: impl Into<usd_vt::Value>) -> bool {
        // Use SetMetadataByDictKey with "assetInfo" key
        // In C++, this calls SetMetadataByDictKey(TfToken("assetInfo"), keyPath, value)
        let asset_info_token = Token::new("assetInfo");
        self.set_metadata_by_dict_key(&asset_info_token, key_path, value)
    }

    /// Authors this prim's assetInfo dictionary.
    ///
    /// Matches C++ `UsdObject::SetAssetInfo(const VtDictionary &assetInfo)`.
    pub fn set_asset_info(
        &self,
        asset_info: std::collections::HashMap<String, usd_vt::Value>,
    ) -> bool {
        self.set_metadata(&Token::new("assetInfo"), usd_vt::Value::from(asset_info))
    }

    /// Gets metadata by dictionary key (matches C++ GetMetadataByDictKey).
    ///
    /// Gets a value from a dictionary metadata field using a colon-separated key path.
    /// E.g. key_path="previews:thumbnails:default" navigates nested dictionaries:
    /// dict["previews"]["thumbnails"]["default"]
    pub fn get_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> Option<usd_vt::Value> {
        let dict: std::collections::HashMap<String, usd_vt::Value> = self.get_metadata(key)?;
        Self::dict_get_at_path(&dict, key_path.get_text())
    }

    /// Sets metadata by dictionary key (matches C++ SetMetadataByDictKey).
    ///
    /// Sets a value in a dictionary metadata field using a colon-separated key path.
    /// E.g. key_path="previews:thumbnails:default" creates/navigates nested dictionaries.
    pub fn set_metadata_by_dict_key(
        &self,
        key: &Token,
        key_path: &Token,
        value: impl Into<usd_vt::Value>,
    ) -> bool {
        let mut dict: std::collections::HashMap<String, usd_vt::Value> =
            self.get_metadata(key).unwrap_or_default();
        Self::dict_set_at_path(&mut dict, key_path.get_text(), value.into());
        self.set_metadata(key, usd_vt::Value::from(dict))
    }

    /// Clears metadata by dictionary key (matches C++ ClearMetadataByDictKey).
    ///
    /// Removes a key from a dictionary-valued metadata field using a colon-separated key path.
    pub fn clear_metadata_by_dict_key(&self, key: &Token, key_path: &Token) -> bool {
        let mut dict: std::collections::HashMap<String, usd_vt::Value> =
            match self.get_metadata(key) {
                Some(d) => d,
                None => return false,
            };
        if Self::dict_remove_at_path(&mut dict, key_path.get_text()) {
            self.set_metadata(key, usd_vt::Value::from(dict))
        } else {
            false
        }
    }

    /// Navigate nested HashMap via colon-separated path and get value.
    fn dict_get_at_path(
        dict: &std::collections::HashMap<String, usd_vt::Value>,
        path: &str,
    ) -> Option<usd_vt::Value> {
        let parts: Vec<&str> = path.splitn(2, ':').collect();
        let value = dict.get(parts[0])?;
        if parts.len() == 1 {
            return Some(value.clone());
        }
        // Navigate into nested dictionary
        let nested: std::collections::HashMap<String, usd_vt::Value> = value.downcast_clone()?;
        Self::dict_get_at_path(&nested, parts[1])
    }

    /// Navigate nested HashMap via colon-separated path and set value,
    /// creating intermediate dictionaries as needed.
    fn dict_set_at_path(
        dict: &mut std::collections::HashMap<String, usd_vt::Value>,
        path: &str,
        value: usd_vt::Value,
    ) {
        let parts: Vec<&str> = path.splitn(2, ':').collect();
        if parts.len() == 1 {
            dict.insert(parts[0].to_string(), value);
            return;
        }
        // Get or create nested dictionary
        let nested = dict.entry(parts[0].to_string()).or_insert_with(|| {
            usd_vt::Value::from(std::collections::HashMap::<String, usd_vt::Value>::new())
        });
        let mut nested_dict: std::collections::HashMap<String, usd_vt::Value> =
            nested.downcast_clone().unwrap_or_default();
        Self::dict_set_at_path(&mut nested_dict, parts[1], value);
        *nested = usd_vt::Value::from(nested_dict);
    }

    /// Navigate nested HashMap via colon-separated path and remove value.
    fn dict_remove_at_path(
        dict: &mut std::collections::HashMap<String, usd_vt::Value>,
        path: &str,
    ) -> bool {
        let parts: Vec<&str> = path.splitn(2, ':').collect();
        if parts.len() == 1 {
            return dict.remove(parts[0]).is_some();
        }
        // Navigate into nested dictionary
        let Some(nested_val) = dict.get_mut(parts[0]) else {
            return false;
        };
        let mut nested_dict: std::collections::HashMap<String, usd_vt::Value> =
            match nested_val.downcast_clone() {
                Some(d) => d,
                None => return false,
            };
        let removed = Self::dict_remove_at_path(&mut nested_dict, parts[1]);
        if removed {
            *nested_val = usd_vt::Value::from(nested_dict);
        }
        removed
    }

    /// Clears the element identified by `key_path` in this prim's assetInfo dictionary.
    ///
    /// Matches C++ `UsdObject::ClearAssetInfoByKey(const TfToken &keyPath)`.
    pub fn clear_asset_info_by_key(&self, key_path: &Token) -> bool {
        let asset_info_token = Token::new("assetInfo");
        self.clear_metadata_by_dict_key(&asset_info_token, key_path)
    }

    // ========================================================================
    // Misc
    // ========================================================================

    /// Returns a description of this prim.
    pub fn description(&self) -> String {
        if self.is_valid() {
            format!(
                "Prim '{}' at {} (type: {})",
                self.name().get_text(),
                self.path().get_string(),
                self.type_name().get_text()
            )
        } else {
            "Invalid prim".to_string()
        }
    }

    /// Returns the prim's specifier.
    pub fn specifier(&self) -> Specifier {
        // C++ defaults to SdfSpecifierOver when no opinion exists
        self.data()
            .map(|d| d.specifier())
            .unwrap_or(Specifier::Over)
    }

    /// Sets the prim's active state.
    ///
    /// Matches C++ reference: authors 'active' at the Sdf level via
    /// `Sdf.CreatePrimInLayer` on the edit target layer, then updates
    /// composed PrimData flags (since we lack Tf::Notice recomposition).
    pub fn set_active(&self, active: bool) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.get_edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        // Sdf-level authoring (C++: Sdf.CreatePrimInLayer + sdfPrim.active = active)
        let handle = usd_sdf::LayerHandle::from_layer(layer);
        let Some(mut spec) = usd_sdf::create_prim_in_layer(&handle, self.path()) else {
            return false;
        };
        spec.set_active(active);
        if let Some(pcp_cache) = stage.pcp_cache() {
            pcp_cache.invalidate_prim_index(self.path());
        }
        if let Some(data) = self.data() {
            if active {
                data.add_flags(PrimFlags::ACTIVE);
            } else {
                data.remove_flags(PrimFlags::ACTIVE);
            }
        }
        stage.handle_local_change(self.path());
        true
    }

    /// Clears the authored 'active' metadata (matches C++ ClearActive).
    pub fn clear_active(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.get_edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        if let Some(mut spec) = layer.get_prim_at_path(self.path()) {
            spec.clear_active();
        }
        if let Some(pcp_cache) = stage.pcp_cache() {
            pcp_cache.invalidate_prim_index(self.path());
        }
        if let Some(data) = self.data() {
            data.add_flags(PrimFlags::ACTIVE);
        }
        stage.handle_local_change(self.path());
        true
    }

    /// Returns true if 'active' metadata is authored (matches C++ HasAuthoredActive).
    pub fn has_authored_active(&self) -> bool {
        self.has_authored_metadata(&Token::new("active"))
    }

    /// Sets the prim's type name (matches C++ SetTypeName).
    ///
    /// Authors the typeName to the current edit target layer's prim spec.
    pub fn set_type_name(&self, type_name: &str) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.get_edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        if let Some(mut spec) = layer.get_prim_at_path(self.path()) {
            spec.set_type_name(type_name);
            // Also update runtime PrimData cache (C++ would recompose)
            if let Some(data) = self.data() {
                data.set_type_name(Token::new(type_name));
            }
            true
        } else {
            false
        }
    }

    /// Clears the type name (matches C++ ClearTypeName).
    pub fn clear_type_name(&self) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.get_edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        layer.erase_field(self.path(), &Token::new("typeName"));
        if let Some(data) = self.data() {
            data.set_type_name(Token::empty());
        }
        if let Some(pcp_cache) = stage.pcp_cache() {
            pcp_cache.invalidate_prim_index(self.path());
        }
        stage.handle_local_change(self.path());
        true
    }

    /// Returns true if the type name has been authored (matches C++ HasAuthoredTypeName).
    pub fn has_authored_type_name(&self) -> bool {
        self.has_authored_metadata(&Token::new("typeName"))
    }

    /// Sets the prim's specifier (matches C++ SetSpecifier).
    ///
    /// Authors the specifier to the current edit target layer's prim spec.
    pub fn set_specifier(&self, specifier: Specifier) -> bool {
        let Some(stage) = self.inner.stage() else {
            return false;
        };
        let edit_target = stage.get_edit_target();
        let Some(layer) = edit_target.layer() else {
            return false;
        };
        if let Some(mut spec) = layer.get_prim_at_path(self.path()) {
            spec.set_specifier(specifier);
            true
        } else {
            false
        }
    }

    /// Returns the applied API schemas.
    pub fn get_applied_schemas(&self) -> Vec<Token> {
        self.data()
            .map(|d| d.prim_type_info().applied_api_schemas().to_vec())
            .unwrap_or_default()
    }

    // ========================================================================
    // API Schema Application
    // ========================================================================

    /// Adds an applied schema name to the prim's apiSchemas metadata.
    ///
    /// Matches C++ `UsdPrim::AddAppliedSchema(const TfToken &appliedSchemaName)`.
    ///
    /// This method edits the `apiSchemas` metadata (a TokenListOp) to add the
    /// schema name. If the list op is explicit, it appends to the explicit list.
    /// Otherwise, it adds to the prepended items.
    pub fn add_applied_schema(&self, applied_schema_name: &Token) -> bool {
        if !self.is_valid() {
            return false;
        }

        // Get or create the apiSchemas metadata as a TokenListOp
        let api_schemas_token = super::tokens::usd_tokens().api_schemas.clone();

        // Get current apiSchemas metadata
        let mut list_op: usd_sdf::list_op::TokenListOp =
            self.get_metadata(&api_schemas_token).unwrap_or_default();

        // Check if schema is already present
        if list_op.has_item(applied_schema_name) {
            return true; // Already present
        }

        // Add to the appropriate list
        if list_op.is_explicit() {
            // Append to explicit items
            let mut explicit_items = list_op.get_explicit_items().to_vec();
            explicit_items.push(applied_schema_name.clone());
            if list_op.set_explicit_items(explicit_items).is_err() {
                return false;
            }
        } else {
            // Add to prepended items (at the end)
            let mut prepended_items = list_op.get_prepended_items().to_vec();
            prepended_items.push(applied_schema_name.clone());
            if list_op.set_prepended_items(prepended_items).is_err() {
                return false;
            }
        }

        // Set the updated metadata
        let ok = self.set_metadata(&api_schemas_token, usd_vt::Value::from(list_op));

        // Sync PrimData's applied_api_schemas so has_api() reflects the change
        if ok {
            if let Some(data) = self.data() {
                let mut current = data.prim_type_info().applied_api_schemas().to_vec();
                if !current.contains(applied_schema_name) {
                    current.push(applied_schema_name.clone());
                    data.set_applied_api_schemas(current);
                }
            }
        }

        ok
    }

    /// Removes an applied schema name from the prim's apiSchemas metadata.
    ///
    /// Matches C++ `UsdPrim::RemoveAppliedSchema(const TfToken &appliedSchemaName)`.
    pub fn remove_applied_schema(&self, applied_schema_name: &Token) -> bool {
        if !self.is_valid() {
            return false;
        }

        let api_schemas_token = super::tokens::usd_tokens().api_schemas.clone();

        // Get current apiSchemas metadata
        let list_op: usd_sdf::list_op::TokenListOp =
            self.get_metadata(&api_schemas_token).unwrap_or_default();

        // Check if schema is present
        if !list_op.has_item(applied_schema_name) {
            return true; // Not present, nothing to remove
        }

        // Create an edit list op that deletes the schema name
        let mut edit_list_op = usd_sdf::list_op::TokenListOp::new();
        if edit_list_op
            .set_deleted_items(vec![applied_schema_name.clone()])
            .is_err()
        {
            return false;
        }

        // Apply the edit to the current list op
        let current_items = list_op.get_applied_items();
        let mut result_items = current_items;
        edit_list_op.apply_operations(
            &mut result_items,
            None::<fn(usd_sdf::list_op::ListOpType, &Token) -> Option<Token>>,
        );

        // Rebuild the list op from the result
        let mut new_list_op = usd_sdf::list_op::TokenListOp::new();
        if list_op.is_explicit() {
            if new_list_op.set_explicit_items(result_items).is_err() {
                return false;
            }
        } else {
            // Try to preserve structure - put remaining items in prepended
            if new_list_op.set_prepended_items(result_items).is_err() {
                return false;
            }
        }

        // Set the updated metadata
        // Store the TokenListOp directly - Value should support it
        let ok = self.set_metadata(&api_schemas_token, usd_vt::Value::from_no_hash(new_list_op));

        // Sync PrimData's applied_api_schemas cache
        if ok {
            if let Some(data) = self.data() {
                let mut current = data.prim_type_info().applied_api_schemas().to_vec();
                current.retain(|s| s != applied_schema_name);
                data.set_applied_api_schemas(current);
            }
        }

        ok
    }

    /// Checks if an API schema can be applied to this prim.
    ///
    /// Matches C++ `UsdPrim::CanApplyAPI<SchemaType>()`.
    ///
    /// In C++, this checks:
    /// 1. Prim is valid
    /// 2. Schema is registered and is an API schema
    /// 3. Schema's canOnlyApplyToTypeNames (if any) includes this prim's type
    pub fn can_apply_api(&self, schema_type_name: &Token) -> bool {
        if !self.is_valid() {
            return false;
        }

        // Find schema info by identifier (static method)
        if let Some(schema_info) =
            super::schema_registry::SchemaRegistry::find_schema_info(schema_type_name)
        {
            // Check if it's an API schema
            if !matches!(
                schema_info.kind,
                super::common::SchemaKind::SingleApplyAPI
                    | super::common::SchemaKind::MultipleApplyAPI
            ) {
                return false;
            }

            // Check if schema has canOnlyApplyToTypeNames restriction
            let can_only_apply_to =
                super::schema_registry::SchemaRegistry::get_api_schema_can_only_apply_to_type_names(
                    schema_type_name,
                    &Token::new(""),
                );
            if !can_only_apply_to.is_empty() {
                // Check if this prim's type is in the allowed list
                let prim_type = self.type_name();
                return can_only_apply_to.contains(&prim_type);
            }

            // No restrictions, can apply
            return true;
        }

        // Schema not found
        false
    }

    /// Checks if an API schema can be applied, returning the reason if not.
    ///
    /// Matches C++ `UsdPrim::CanApplyAPI()` with whyNot parameter.
    pub fn can_apply_api_with_reason(&self, schema_type_name: &Token) -> Result<(), String> {
        if !self.is_valid() {
            return Err("Prim is not valid".to_string());
        }

        let Some(schema_info) =
            super::schema_registry::SchemaRegistry::find_schema_info(schema_type_name)
        else {
            return Err(format!(
                "Schema '{}' is not registered",
                schema_type_name.get_text()
            ));
        };

        if !matches!(
            schema_info.kind,
            super::common::SchemaKind::SingleApplyAPI | super::common::SchemaKind::MultipleApplyAPI
        ) {
            return Err(format!(
                "Schema '{}' is not an API schema",
                schema_type_name.get_text()
            ));
        }

        let can_only_apply_to =
            super::schema_registry::SchemaRegistry::get_api_schema_can_only_apply_to_type_names(
                schema_type_name,
                &Token::new(""),
            );
        if !can_only_apply_to.is_empty() {
            let prim_type = self.type_name();
            if !can_only_apply_to.contains(&prim_type) {
                return Err(format!(
                    "Schema '{}' can only apply to types {:?}, not '{}'",
                    schema_type_name.get_text(),
                    can_only_apply_to
                        .iter()
                        .map(|t| t.get_text().to_string())
                        .collect::<Vec<_>>(),
                    prim_type.get_text()
                ));
            }
        }

        Ok(())
    }

    /// Applies an API schema to this prim by adding it to apiSchemas metadata.
    ///
    /// Matches C++ `UsdPrim::ApplyAPI<SchemaType>()`.
    ///
    /// This adds the schema type name to the prim's apiSchemas metadata.
    pub fn apply_api(&self, schema_type_name: &Token) -> bool {
        if !self.can_apply_api(schema_type_name) {
            return false;
        }
        self.add_applied_schema(schema_type_name)
    }

    /// Removes an API schema from this prim.
    ///
    /// Matches C++ `UsdPrim::RemoveAPI<SchemaType>()`.
    pub fn remove_api(&self, schema_type_name: &Token) -> bool {
        self.remove_applied_schema(schema_type_name)
    }

    // ========================================================================
    // Multi-apply API schema (with instance name)
    // ========================================================================

    /// Builds the full schema identifier for a multi-apply API schema.
    /// e.g. "CollectionAPI:lightLink" from ("CollectionAPI", Some("lightLink")).
    fn make_api_schema_id(schema_type_name: &Token, instance_name: Option<&Token>) -> Token {
        match instance_name {
            Some(inst) if !inst.is_empty() => Token::new(&format!(
                "{}:{}",
                schema_type_name.get_text(),
                inst.get_text()
            )),
            _ => schema_type_name.clone(),
        }
    }

    /// Checks if a multi-apply API schema instance is applied.
    ///
    /// Matches C++ `UsdPrim::HasAPI<SchemaType>(const TfToken &instanceName)`.
    pub fn has_api_instance(&self, schema_type_name: &Token, instance_name: &Token) -> bool {
        let full = Self::make_api_schema_id(schema_type_name, Some(instance_name));
        self.has_api(&full)
    }

    /// Checks if a multi-apply API schema instance can be applied.
    ///
    /// Matches C++ `UsdPrim::CanApplyAPI<SchemaType>(const TfToken &instanceName)`.
    pub fn can_apply_api_instance(&self, schema_type_name: &Token, instance_name: &Token) -> bool {
        if instance_name.is_empty() {
            return false; // Multi-apply requires an instance name
        }
        self.can_apply_api(schema_type_name)
    }

    /// Applies a multi-apply API schema instance.
    ///
    /// Matches C++ `UsdPrim::ApplyAPI<SchemaType>(const TfToken &instanceName)`.
    pub fn apply_api_instance(&self, schema_type_name: &Token, instance_name: &Token) -> bool {
        if !self.can_apply_api_instance(schema_type_name, instance_name) {
            return false;
        }
        let full = Self::make_api_schema_id(schema_type_name, Some(instance_name));
        self.add_applied_schema(&full)
    }

    /// Removes a multi-apply API schema instance.
    ///
    /// Matches C++ `UsdPrim::RemoveAPI<SchemaType>(const TfToken &instanceName)`.
    pub fn remove_api_instance(&self, schema_type_name: &Token, instance_name: &Token) -> bool {
        let full = Self::make_api_schema_id(schema_type_name, Some(instance_name));
        self.remove_applied_schema(&full)
    }

    // ========================================================================
    // Composition Queries
    // ========================================================================

    /// Returns a UsdReferences object that allows one to add, remove, or mutate
    /// references at the currently set UsdEditTarget.
    ///
    /// Matches C++ `UsdPrim::GetReferences()`.
    pub fn get_references(&self) -> super::references::References {
        super::references::References::new(self.clone())
    }

    /// Returns all reference arcs for this prim (composition query version).
    ///
    /// Matches C++ `UsdPrimCompositionQuery::GetDirectReferences()`.
    /// Returns a list of composition arcs representing references.
    pub fn get_reference_arcs(
        &self,
    ) -> Vec<super::prim_composition_query::PrimCompositionQueryArc> {
        if !self.is_valid() {
            return Vec::new();
        }
        let query = super::prim_composition_query::PrimCompositionQuery::get_direct_references(
            self.clone(),
        );
        query.get_composition_arcs()
    }

    /// Returns a UsdInherits object that allows one to add, remove, or mutate
    /// inherits at the currently set UsdEditTarget.
    ///
    /// Matches C++ `UsdPrim::GetInherits()`.
    pub fn get_inherits(&self) -> super::inherits::Inherits {
        super::inherits::Inherits::new(self.clone())
    }

    /// Returns all inherit arcs for this prim (composition query version).
    ///
    /// Matches C++ `UsdPrimCompositionQuery::GetDirectInherits()`.
    /// Returns a list of composition arcs representing inherits.
    pub fn get_inherit_arcs(&self) -> Vec<super::prim_composition_query::PrimCompositionQueryArc> {
        if !self.is_valid() {
            return Vec::new();
        }
        let query =
            super::prim_composition_query::PrimCompositionQuery::get_direct_inherits(self.clone());
        query.get_composition_arcs()
    }

    /// Returns a UsdSpecializes object that allows one to add, remove, or mutate
    /// specializes at the currently set UsdEditTarget.
    ///
    /// Matches C++ `UsdPrim::GetSpecializes()`.
    pub fn get_specializes(&self) -> super::specializes::Specializes {
        super::specializes::Specializes::new(self.clone())
    }

    /// Returns all specialize arcs for this prim (composition query version).
    ///
    /// Matches C++ `UsdPrimCompositionQuery::GetDirectSpecializes()`.
    /// Returns a list of composition arcs representing specializes.
    pub fn get_specialize_arcs(
        &self,
    ) -> Vec<super::prim_composition_query::PrimCompositionQueryArc> {
        if !self.is_valid() {
            return Vec::new();
        }
        let mut filter = super::prim_composition_query::Filter::default();
        filter.arc_type_filter = super::prim_composition_query::ArcTypeFilter::Specialize;
        filter.dependency_type_filter = super::prim_composition_query::DependencyTypeFilter::Direct;
        let query = super::prim_composition_query::PrimCompositionQuery::new(self.clone(), filter);
        query.get_composition_arcs()
    }

    /// Returns a UsdPayloads object that allows one to add, remove, or mutate
    /// payloads at the currently set UsdEditTarget.
    ///
    /// Matches C++ `UsdPrim::GetPayloads()`.
    pub fn get_payloads(&self) -> super::payloads::Payloads {
        super::payloads::Payloads::new(self.clone())
    }

    /// Returns all payload arcs for this prim (composition query version).
    ///
    /// Matches C++ `UsdPrimCompositionQuery::GetDirectPayloads()`.
    /// Returns a list of composition arcs representing payloads.
    pub fn get_payload_arcs(&self) -> Vec<super::prim_composition_query::PrimCompositionQueryArc> {
        if !self.is_valid() {
            return Vec::new();
        }
        let mut filter = super::prim_composition_query::Filter::default();
        filter.arc_type_filter = super::prim_composition_query::ArcTypeFilter::Payload;
        filter.dependency_type_filter = super::prim_composition_query::DependencyTypeFilter::Direct;
        let query = super::prim_composition_query::PrimCompositionQuery::new(self.clone(), filter);
        query.get_composition_arcs()
    }

    // ------------------------------------------------------------------ //
    // Variant Sets API
    // ------------------------------------------------------------------ //

    /// Returns a UsdVariantSets object representing all the variant sets
    /// present on this prim.
    ///
    /// Matches C++ `UsdPrim::GetVariantSets()`.
    pub fn get_variant_sets(&self) -> super::variant_sets::VariantSets {
        super::variant_sets::VariantSets::new(self.clone())
    }

    /// Return a UsdVariantSet object for the variant set named variantSetName.
    ///
    /// Matches C++ `UsdPrim::GetVariantSet(const std::string &variantSetName)`.
    ///
    /// This always succeeds, although the returned VariantSet will be invalid
    /// if this prim is invalid.
    pub fn get_variant_set(&self, variant_set_name: &str) -> super::variant_sets::VariantSet {
        super::variant_sets::VariantSet::new(self.clone(), variant_set_name.to_string())
    }

    /// Return true if this prim has a variant set named variantSetName.
    ///
    /// Matches C++ `UsdPrim::HasVariantSets()`.
    pub fn has_variant_sets(&self) -> bool {
        !self.get_variant_sets().get_names().is_empty()
    }

    // =========================================================================
    // Additional schema and family methods
    // =========================================================================

    /// Returns true if this prim has a defining specifier (Def or Class).
    ///
    /// Matches C++ `UsdPrim::HasDefiningSpecifier()`.
    pub fn has_defining_specifier(&self) -> bool {
        self.flags().contains(PrimFlags::HAS_DEFINING_SPECIFIER)
    }

    /// Returns true if this prim is a subcomponent.
    ///
    /// This is true when the prim's kind is 'subcomponent', meaning it's a
    /// lightweight model component that can be embedded in components.
    ///
    /// Matches C++ `UsdPrim::IsSubComponent()`.
    pub fn is_subcomponent(&self) -> bool {
        if let Some(kind) = self.get_kind() {
            kind.get_text() == "subcomponent"
        } else {
            false
        }
    }

    /// Returns the authored applied schemas for this prim.
    ///
    /// Unlike get_applied_schemas(), this only returns schemas that were
    /// explicitly authored and does not include built-in schemas.
    ///
    /// Matches C++ `UsdPrim::GetAuthoredAppliedSchemas()`.
    pub fn get_authored_applied_schemas(&self) -> Vec<Token> {
        if let Some(stage) = self.stage() {
            let api_schemas_key = Token::new("apiSchemas");
            if let Some(value) = stage.get_metadata_for_object(self.path(), &api_schemas_key) {
                if let Some(list) = value.get::<usd_sdf::TokenListOp>() {
                    // Return explicit items from the list op
                    return list.get_explicit_items().to_vec();
                }
            }
        }
        Vec::new()
    }

    /// Return a direct child prim by name.
    ///
    /// Matches C++ `UsdPrim::GetChild(const TfToken &name)`.
    pub fn get_child(&self, name: &Token) -> Prim {
        let child_path = self.path().append_child(name.get_text());
        if let Some(stage) = self.stage() {
            if let Some(child_path) = child_path {
                if let Some(prim) = stage.get_prim_at_path(&child_path) {
                    return prim;
                }
            }
        }
        Prim::invalid()
    }

    /// Returns all authored property names for this prim.
    ///
    /// Matches C++ `UsdPrim::GetAuthoredPropertyNames()`.
    pub fn get_authored_property_names(&self) -> Vec<Token> {
        let mut names = Vec::new();
        if let Some(stage) = self.stage() {
            let layer = stage.root_layer();
            if let Some(prim_spec) = layer.get_prim_at_path(self.path()) {
                for prop in prim_spec.properties() {
                    names.push(prop.name());
                }
            }
        }
        names
    }

    /// Returns all authored properties for this prim as Property objects.
    ///
    /// Matches C++ `UsdPrim::GetAuthoredProperties()`.
    pub fn get_authored_properties(&self) -> Vec<super::property::Property> {
        let mut props = Vec::new();
        for name in self.get_authored_property_names() {
            // Create Property from path
            let prop_path = self.path().append_property(name.get_text());
            if let Some(prop_path) = prop_path {
                if let Some(stage) = self.stage() {
                    props.push(super::property::Property::new(
                        std::sync::Arc::downgrade(&stage),
                        prop_path,
                    ));
                }
            }
        }
        props
    }

    /// Returns the prim at a relative or absolute path from this prim.
    ///
    /// Matches C++ `UsdPrim::GetPrimAtPath(const SdfPath &path)`.
    pub fn get_prim_at_path(&self, path: &Path) -> Prim {
        let resolved_path = if path.is_absolute_path() {
            Some(path.clone())
        } else {
            path.make_absolute(self.path())
        };

        if let Some(resolved) = resolved_path {
            if let Some(stage) = self.stage() {
                if let Some(prim) = stage.get_prim_at_path(&resolved) {
                    return prim;
                }
            }
        }
        Prim::invalid()
    }

    /// Returns the attribute at a relative or absolute path from this prim.
    ///
    /// Matches C++ `UsdPrim::GetAttributeAtPath(const SdfPath &path)`.
    pub fn get_attribute_at_path(&self, path: &Path) -> Option<Attribute> {
        if !path.is_property_path() {
            return None;
        }
        let resolved_path = if path.is_absolute_path() {
            Some(path.clone())
        } else {
            path.make_absolute(self.path())
        };

        if let Some(resolved) = resolved_path {
            if let Some(stage) = self.stage() {
                return stage.get_attribute_at_path(&resolved);
            }
        }
        None
    }

    /// Returns the relationship at a relative or absolute path from this prim.
    ///
    /// Matches C++ `UsdPrim::GetRelationshipAtPath(const SdfPath &path)`.
    pub fn get_relationship_at_path(&self, path: &Path) -> Option<Relationship> {
        if !path.is_property_path() {
            return None;
        }
        let resolved_path = if path.is_absolute_path() {
            Some(path.clone())
        } else {
            path.make_absolute(self.path())
        };

        if let Some(resolved) = resolved_path {
            if let Some(stage) = self.stage() {
                return stage.get_relationship_at_path(&resolved);
            }
        }
        None
    }

    /// Returns the object (prim, attribute, or relationship) at a relative or absolute path.
    ///
    /// Matches C++ `UsdPrim::GetObjectAtPath(const SdfPath &path)`.
    pub fn get_object_at_path(&self, path: &Path) -> Option<super::object::UsdObject> {
        if path.is_empty() {
            return None;
        }

        let resolved_path = if path.is_absolute_path() {
            Some(path.clone())
        } else {
            path.make_absolute(self.path())
        }?;

        if let Some(stage) = self.stage() {
            if resolved_path.is_property_path() {
                if let Some(attr) = stage.get_attribute_at_path(&resolved_path) {
                    return Some(super::object::UsdObject::Attribute(attr));
                }
                if let Some(rel) = stage.get_relationship_at_path(&resolved_path) {
                    return Some(super::object::UsdObject::Relationship(rel));
                }
                None
            } else {
                let prim = stage.get_prim_at_path(&resolved_path)?;
                Some(super::object::UsdObject::Prim(prim))
            }
        } else {
            None
        }
    }

    /// Returns the property at a relative or absolute path.
    ///
    /// Matches C++ `UsdPrim::GetPropertyAtPath(const SdfPath &path)`.
    pub fn get_property_at_path(&self, path: &Path) -> Option<super::object::UsdObject> {
        if !path.is_property_path() {
            return None;
        }
        self.get_object_at_path(path)
    }

    /// Returns the next sibling prim matching the predicate.
    ///
    /// Matches C++ `UsdPrim::GetFilteredNextSibling(const Usd_PrimFlagsPredicate &predicate)`.
    pub fn get_filtered_next_sibling(&self, predicate: PrimFlagsPredicate) -> Prim {
        let parent = self.parent();
        if !parent.is_valid() {
            return Prim::invalid();
        }

        let children = parent.children();
        let mut found_self = false;
        for child in children {
            if found_self && predicate.matches(child.flags()) {
                return child;
            }
            if child.path() == self.path() {
                found_self = true;
            }
        }
        Prim::invalid()
    }

    /// Returns names of children matching default predicate.
    ///
    /// Matches C++ `UsdPrim::GetChildrenNames()`.
    pub fn get_children_names(&self) -> Vec<Token> {
        self.children().into_iter().map(|p| p.name()).collect()
    }

    /// Returns names of all children (unfiltered).
    ///
    /// Matches C++ `UsdPrim::GetAllChildrenNames()`.
    pub fn get_all_children_names(&self) -> Vec<Token> {
        self.get_all_children()
            .into_iter()
            .map(|p| p.name())
            .collect()
    }

    /// Returns children names matching the predicate.
    ///
    /// Matches C++ `UsdPrim::GetFilteredChildrenNames(const Usd_PrimFlagsPredicate &predicate)`.
    pub fn get_filtered_children_names(&self, predicate: PrimFlagsPredicate) -> Vec<Token> {
        self.get_filtered_children(predicate)
            .into_iter()
            .map(|p| p.name())
            .collect()
    }

    /// Returns children reorder list if authored.
    ///
    /// Matches C++ `UsdPrim::GetChildrenReorder()`.
    pub fn get_children_reorder(&self) -> Option<Vec<Token>> {
        let key = Token::new("primOrder");
        if let Some(value) = self.get_metadata::<usd_sdf::TokenListOp>(&key) {
            return Some(value.get_explicit_items().to_vec());
        }
        None
    }

    /// Sets the children reorder list.
    ///
    /// Matches C++ `UsdPrim::SetChildrenReorder(const TfTokenVector &order)`.
    pub fn set_children_reorder(&self, order: Vec<Token>) -> bool {
        let key = Token::new("primOrder");
        let list_op = usd_sdf::TokenListOp::create_explicit(order);
        self.set_metadata(&key, list_op)
    }

    /// Clears the children reorder metadata.
    ///
    /// Matches C++ `UsdPrim::ClearChildrenReorder()`.
    pub fn clear_children_reorder(&self) -> bool {
        let key = Token::new("primOrder");
        self.clear_metadata(&key)
    }

    /// Returns prim stack with layer offsets.
    ///
    /// Matches C++ `UsdPrim::GetPrimStackWithLayerOffsets()`.
    pub fn get_prim_stack_with_layer_offsets(
        &self,
    ) -> Vec<(usd_sdf::PrimSpec, usd_sdf::LayerOffset)> {
        let mut result = Vec::new();

        // Use PrimIndex-based Resolver (C++ UsdStage::_GetPrimStackWithLayerOffsets):
        // walks nodes in strong-to-weak order, uses local path per node.
        if let Some(prim_index) = self.prim_index().map(std::sync::Arc::new) {
            let mut resolver = super::resolver::Resolver::new(&prim_index, false);
            while resolver.is_valid() {
                if let (Some(layer), Some(local_path)) =
                    (resolver.get_layer(), resolver.get_local_path())
                {
                    if let Some(spec) = layer.get_prim_at_path(&local_path) {
                        let offset = resolver.get_layer_to_stage_offset();
                        result.push((spec, offset));
                    }
                }
                resolver.next_layer();
            }
        } else if let Some(stage) = self.stage() {
            // Fallback: root layer stack (in-memory stages)
            for layer in stage.used_layers() {
                if let Some(spec) = layer.get_prim_at_path(self.path()) {
                    let offset = usd_sdf::LayerOffset::identity();
                    result.push((spec, offset));
                }
            }
        }

        result
    }

    /// Find all relationship target paths in this prim subtree.
    ///
    /// Matches C++ `UsdPrim::FindAllRelationshipTargetPaths()`.
    pub fn find_all_relationship_target_paths(
        &self,
        predicate: Option<PrimFlagsPredicate>,
    ) -> Vec<Path> {
        let mut targets = Vec::new();
        let pred = predicate.unwrap_or_else(|| super::prim_flags::default_predicate().into_predicate());

        self.find_relationship_targets_recursive(&pred, &mut targets);

        // Remove duplicates and sort
        targets.sort();
        targets.dedup();
        targets
    }

    fn find_relationship_targets_recursive(
        &self,
        pred: &PrimFlagsPredicate,
        targets: &mut Vec<Path>,
    ) {
        // Collect relationship targets from this prim
        for rel_name in self.get_relationship_names() {
            if let Some(rel) = self.get_relationship(rel_name.get_text()) {
                targets.extend(rel.get_targets());
            }
        }

        // Recurse into children matching predicate
        for child in self.children_filtered(*pred) {
            child.find_relationship_targets_recursive(pred, targets);
        }
    }

    /// Find all attribute connection paths in this prim subtree.
    ///
    /// Matches C++ `UsdPrim::FindAllAttributeConnectionPaths()`.
    pub fn find_all_attribute_connection_paths(
        &self,
        predicate: Option<PrimFlagsPredicate>,
    ) -> Vec<Path> {
        let mut connections = Vec::new();
        let pred = predicate.unwrap_or_else(|| super::prim_flags::default_predicate().into_predicate());

        self.find_attribute_connections_recursive(&pred, &mut connections);

        // Remove duplicates and sort
        connections.sort();
        connections.dedup();
        connections
    }

    fn find_attribute_connections_recursive(
        &self,
        pred: &PrimFlagsPredicate,
        connections: &mut Vec<Path>,
    ) {
        // Collect attribute connections from this prim
        for attr_name in self.get_attribute_names() {
            if let Some(attr) = self.get_attribute(attr_name.get_text()) {
                connections.extend(attr.get_connections());
            }
        }

        // Recurse into children matching predicate
        for child in self.children_filtered(*pred) {
            child.find_attribute_connections_recursive(pred, connections);
        }
    }

    /// Compute an expanded prim index for this prim.
    ///
    /// This is a more expensive prim index computation that includes all
    /// possible sites, not just those currently contributing opinions.
    ///
    /// Matches C++ `UsdPrim::ComputeExpandedPrimIndex()`.
    pub fn compute_expanded_prim_index(&self) -> Option<usd_pcp::PrimIndex> {
        // For now, just return the regular prim index
        // Full implementation would compute the expanded index
        self.prim_index()
    }

    /// Check if prim is in a schema family (any version).
    ///
    /// Matches C++ `UsdPrim::IsInFamily(const TfToken &schemaFamily)`.
    pub fn is_in_family(&self, schema_family: &Token) -> bool {
        use super::common::VersionPolicy;
        self.is_in_family_versioned(schema_family, 0, VersionPolicy::All)
    }

    /// Check if prim is in a schema family with version filtering.
    ///
    /// Uses the SchemaRegistry to look up schemas in the family, then checks
    /// whether the prim's typed schema or any applied API schema matches.
    ///
    /// Matches C++ `UsdPrim::IsInFamily(schemaFamily, schemaVersion, versionPolicy)`.
    pub fn is_in_family_versioned(
        &self,
        schema_family: &Token,
        schema_version: super::common::SchemaVersion,
        version_policy: super::common::VersionPolicy,
    ) -> bool {
        use super::common::VersionPolicy;
        use super::schema_registry::SchemaRegistry;

        // Query registry for matching schemas in this family
        let infos = match version_policy {
            VersionPolicy::All => SchemaRegistry::find_schema_infos_in_family(schema_family),
            _ => SchemaRegistry::find_schema_infos_in_family_filtered(
                schema_family,
                schema_version,
                version_policy,
            ),
        };

        let type_name = self.type_name();
        let applied = self.get_applied_schemas();

        for info in &infos {
            // Check typed schema match (type_name is String, compare with token text)
            if info.type_name == type_name.get_text() {
                return true;
            }
            // Check identifier match against type name token
            if info.identifier == type_name {
                return true;
            }
            // Check applied schemas
            for schema in &applied {
                if info.identifier.get_text() == schema.get_text() {
                    return true;
                }
            }
        }

        // Hierarchy-aware fallback: check if prim's schema has the family in its base types
        let schema_info = SchemaRegistry::find_schema_info(&type_name);
        if let Some(info) = schema_info {
            if info.base_type_names.iter().any(|b| b == schema_family) {
                return true;
            }
            // Also check if the family matches the schema's own identifier
            if &info.identifier == schema_family {
                return true;
            }
        }

        // Fallback: prefix-based matching when registry has no entries
        if infos.is_empty() {
            let family_text = schema_family.get_text();
            if type_name.get_text().starts_with(family_text) {
                return true;
            }
            for schema in &applied {
                if schema.get_text().starts_with(family_text) {
                    return true;
                }
            }
        }

        false
    }

    /// Check if prim has an API in a schema family (any version).
    ///
    /// Matches C++ `UsdPrim::HasAPIInFamily(const TfToken &schemaFamily)`.
    pub fn has_api_in_family(&self, schema_family: &Token) -> bool {
        use super::common::VersionPolicy;
        self.has_api_in_family_versioned(schema_family, 0, VersionPolicy::All)
    }

    /// Check if prim has an API in a schema family with version filtering.
    ///
    /// Matches C++ `UsdPrim::HasAPIInFamily(schemaFamily, schemaVersion, versionPolicy)`.
    pub fn has_api_in_family_versioned(
        &self,
        schema_family: &Token,
        schema_version: super::common::SchemaVersion,
        version_policy: super::common::VersionPolicy,
    ) -> bool {
        use super::common::VersionPolicy;
        use super::schema_registry::SchemaRegistry;

        let infos = match version_policy {
            VersionPolicy::All => SchemaRegistry::find_schema_infos_in_family(schema_family),
            _ => SchemaRegistry::find_schema_infos_in_family_filtered(
                schema_family,
                schema_version,
                version_policy,
            ),
        };

        let applied = self.get_applied_schemas();

        for info in &infos {
            for schema in &applied {
                if info.identifier.get_text() == schema.get_text() {
                    return true;
                }
            }
        }

        // Fallback: prefix-based matching
        if infos.is_empty() {
            let family_text = schema_family.get_text();
            for schema in &applied {
                if schema.get_text().starts_with(family_text) {
                    return true;
                }
            }
        }

        false
    }
}

impl PartialEq for Prim {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Prim {}

impl std::hash::Hash for Prim {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_prim() {
        let prim = Prim::invalid();
        assert!(!prim.is_valid());
    }

    #[test]
    fn test_prim_path() {
        let path = Path::from_string("/World").unwrap();
        let prim = Prim::new(Weak::new(), path.clone());
        assert_eq!(prim.path(), &path);
    }

    #[test]
    fn test_prim_name() {
        let path = Path::from_string("/World/Cube").unwrap();
        let prim = Prim::new(Weak::new(), path);
        assert_eq!(prim.name().get_text(), "Cube");
    }

    #[test]
    fn test_prim_with_type() {
        let path = Path::from_string("/World").unwrap();
        let prim = Prim::new_with_type(Weak::new(), path, Token::new("Xform"));
        assert_eq!(prim.type_name().get_text(), "Xform");
    }

    #[test]
    fn test_create_attribute() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        // Create attribute
        use usd_sdf::value_type_registry::ValueTypeRegistry;
        let type_name = ValueTypeRegistry::instance().find_type("float");
        let attr = prim.create_attribute("size", &type_name, false, None);
        assert!(attr.is_some());

        // Verify attribute exists
        let attr = attr.unwrap();
        assert!(attr.is_valid());
        assert_eq!(attr.name().get_text(), "size");
    }

    #[test]
    fn test_create_relationship() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Mesh").unwrap();

        // Create relationship
        let rel = prim.create_relationship("material:binding", false);
        assert!(rel.is_some());

        let rel = rel.unwrap();
        assert!(rel.is_valid());
        assert_eq!(rel.name().get_text(), "material:binding");
    }

    // M9: GetPrimStack tests
    #[test]
    fn test_get_prim_stack() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let stack = prim.get_prim_stack();
        // Should have at least the root layer spec
        assert!(
            !stack.is_empty(),
            "Prim stack should not be empty for defined prim"
        );
    }

    #[test]
    fn test_get_prim_stack_with_layer_offsets() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let stack = prim.get_prim_stack_with_layer_offsets();
        // Should match get_prim_stack in length
        assert_eq!(stack.len(), prim.get_prim_stack().len());
    }

    // M11: API schema instance name tests
    #[test]
    fn test_make_api_schema_id() {
        let base = Token::new("CollectionAPI");
        let inst = Token::new("lightLink");
        let full = Prim::make_api_schema_id(&base, Some(&inst));
        assert_eq!(full.get_text(), "CollectionAPI:lightLink");

        // No instance name - returns base
        let full2 = Prim::make_api_schema_id(&base, None);
        assert_eq!(full2.get_text(), "CollectionAPI");

        // Empty instance name - returns base
        let empty = Token::new("");
        let full3 = Prim::make_api_schema_id(&base, Some(&empty));
        assert_eq!(full3.get_text(), "CollectionAPI");
    }

    #[test]
    fn test_has_api_instance() {
        let path = Path::from_string("/World").unwrap();
        let prim = Prim::new(Weak::new(), path);

        // Should not have any API instance on a bare prim
        let schema = Token::new("CollectionAPI");
        let inst = Token::new("lightLink");
        assert!(!prim.has_api_instance(&schema, &inst));
    }

    #[test]
    fn test_can_apply_api_instance_empty_name() {
        let path = Path::from_string("/World").unwrap();
        let prim = Prim::new(Weak::new(), path);

        // Empty instance name must fail
        let schema = Token::new("CollectionAPI");
        let empty = Token::new("");
        assert!(!prim.can_apply_api_instance(&schema, &empty));
    }

    // M2: children() filters by default predicate vs get_all_children()
    #[test]
    // set_active() authors the 'active' metadata but Prim::flags() reads cached
    // PCP index flags that are not rebuilt until the stage recomposes. Until
    // Stage::_HandleLayersDidChange / recompose is wired to update prim flags
    // on metadata edits, the filter won't reflect the authored value.
    #[ignore = "Prim flags cache not updated after set_active; needs stage recompose on metadata change"]
    fn test_children_filters_by_default_predicate() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let _parent = stage.define_prim("/Parent", "Xform").unwrap();
        let _child_a = stage.define_prim("/Parent/A", "Xform").unwrap();
        let child_b = stage.define_prim("/Parent/B", "Xform").unwrap();

        // Deactivate child B so the default predicate filters it out.
        child_b.set_active(false);

        let parent = stage
            .get_prim_at_path(&Path::from_string("/Parent").unwrap())
            .unwrap();

        // get_all_children should include both
        let all = parent.get_all_children();
        assert!(
            all.len() >= 2,
            "get_all_children should return all children"
        );

        // children() should filter out inactive
        let filtered = parent.children();
        // /Parent/B is inactive so should be excluded by default predicate
        let has_b = filtered.iter().any(|p| p.name().get_text() == "B");
        assert!(!has_b, "children() should exclude inactive prims");
    }

    // M11: can_apply_api_with_reason returns detailed errors
    #[test]
    fn test_can_apply_api_with_reason_invalid_prim() {
        let prim = Prim::invalid();
        let schema = Token::new("SomeAPI");
        let result = prim.can_apply_api_with_reason(&schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not valid"));
    }

    #[test]
    fn test_can_apply_api_with_reason_unregistered() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let prim = stage.define_prim("/World", "Xform").unwrap();

        let schema = Token::new("NonExistentAPI");
        let result = prim.can_apply_api_with_reason(&schema);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not registered"));
    }

    // ---- New tests for MEDIUM parity fixes ----

    #[test]
    fn test_get_children_names() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Root", "Xform").unwrap();
        stage.define_prim("/Root/A", "Xform").unwrap();
        stage.define_prim("/Root/B", "Mesh").unwrap();

        let root = stage
            .get_prim_at_path(&Path::from_string("/Root").unwrap())
            .unwrap();

        let names = root.get_children_names();
        assert_eq!(names.len(), 2);
        let name_strs: Vec<&str> = names.iter().map(|t| t.get_text()).collect();
        assert!(name_strs.contains(&"A"));
        assert!(name_strs.contains(&"B"));
    }

    #[test]
    fn test_get_all_children_names() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Root", "Xform").unwrap();
        stage.define_prim("/Root/X", "Xform").unwrap();
        stage.define_prim("/Root/Y", "Mesh").unwrap();

        let root = stage
            .get_prim_at_path(&Path::from_string("/Root").unwrap())
            .unwrap();

        let names = root.get_all_children_names();
        assert_eq!(names.len(), 2);
        let name_strs: Vec<&str> = names.iter().map(|t| t.get_text()).collect();
        assert!(name_strs.contains(&"X"));
        assert!(name_strs.contains(&"Y"));
    }

    #[test]
    fn test_get_descendants_alias() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/A", "Xform").unwrap();
        stage.define_prim("/A/B", "Xform").unwrap();
        stage.define_prim("/A/B/C", "Mesh").unwrap();

        let a = stage
            .get_prim_at_path(&Path::from_string("/A").unwrap())
            .unwrap();

        // get_descendants() should return same as descendants()
        let d1 = a.descendants();
        let d2 = a.get_descendants();
        assert_eq!(d1.len(), d2.len());
        for (p1, p2) in d1.iter().zip(d2.iter()) {
            assert_eq!(p1.path(), p2.path());
        }
    }

    #[test]
    fn test_get_all_descendants() {
        use super::super::common::InitialLoadSet;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/P", "Xform").unwrap();
        stage.define_prim("/P/Q", "Xform").unwrap();
        stage.define_prim("/P/Q/R", "Mesh").unwrap();

        let p = stage
            .get_prim_at_path(&Path::from_string("/P").unwrap())
            .unwrap();

        let all = p.get_all_descendants();
        // Should include Q and R
        assert_eq!(all.len(), 2);
        let paths: Vec<&str> = all.iter().map(|d| d.path().get_string()).collect();
        assert!(paths.contains(&"/P/Q"));
        assert!(paths.contains(&"/P/Q/R"));
    }

    #[test]
    fn test_get_filtered_children_uses_all_children() {
        use super::super::common::InitialLoadSet;
        use super::super::prim_flags;
        use super::super::stage::Stage;

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        stage.define_prim("/Root", "Xform").unwrap();
        stage.define_prim("/Root/A", "Xform").unwrap();
        stage.define_prim("/Root/B", "Mesh").unwrap();

        let root = stage
            .get_prim_at_path(&Path::from_string("/Root").unwrap())
            .unwrap();

        // AllPrimsPredicate should return all children, same as get_all_children
        let all_pred = prim_flags::all_prims_predicate();
        let filtered = root.get_filtered_children(all_pred);
        let all = root.get_all_children();
        assert_eq!(
            filtered.len(),
            all.len(),
            "get_filtered_children with all-prims predicate should match get_all_children"
        );
    }

    // Schema hierarchy tests
    #[test]
    fn test_is_in_family_gprim() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::register_builtin_schemas;
        use super::super::stage::Stage;

        // Ensure schemas are registered before creating prims
        register_builtin_schemas();

        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mesh = stage.define_prim("/Mesh", "Mesh").unwrap();

        // Mesh should be in Gprim family
        assert!(
            mesh.is_in_family(&Token::new("Gprim")),
            "Mesh should be in Gprim family"
        );
        assert!(
            mesh.is_in_family(&Token::new("PointBased")),
            "Mesh should be in PointBased family"
        );
        assert!(
            mesh.is_in_family(&Token::new("Boundable")),
            "Mesh should be in Boundable family"
        );
        assert!(
            mesh.is_in_family(&Token::new("Xformable")),
            "Mesh should be in Xformable family"
        );
        assert!(
            mesh.is_in_family(&Token::new("Imageable")),
            "Mesh should be in Imageable family"
        );
        assert!(
            mesh.is_in_family(&Token::new("Typed")),
            "Mesh should be in Typed family"
        );
    }

    #[test]
    fn test_is_in_family_cube() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::register_builtin_schemas;
        use super::super::stage::Stage;

        register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let cube = stage.define_prim("/Cube", "Cube").unwrap();

        // Cube should be in Gprim and Boundable families
        assert!(
            cube.is_in_family(&Token::new("Gprim")),
            "Cube should be in Gprim family"
        );
        assert!(
            cube.is_in_family(&Token::new("Boundable")),
            "Cube should be in Boundable family"
        );
    }

    #[test]
    fn test_is_in_family_scope() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::register_builtin_schemas;
        use super::super::stage::Stage;

        register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let scope = stage.define_prim("/Scope", "Scope").unwrap();

        // Scope should be in Imageable family but not Gprim
        assert!(
            scope.is_in_family(&Token::new("Imageable")),
            "Scope should be in Imageable family"
        );
        assert!(
            scope.is_in_family(&Token::new("Typed")),
            "Scope should be in Typed family"
        );
        assert!(
            !scope.is_in_family(&Token::new("Gprim")),
            "Scope should NOT be in Gprim family"
        );
    }

    #[test]
    fn test_is_in_family_camera() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::register_builtin_schemas;
        use super::super::stage::Stage;

        register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let camera = stage.define_prim("/Camera", "Camera").unwrap();

        // Camera should be in Xformable family
        assert!(
            camera.is_in_family(&Token::new("Xformable")),
            "Camera should be in Xformable family"
        );
        assert!(
            camera.is_in_family(&Token::new("Imageable")),
            "Camera should be in Imageable family"
        );
        assert!(
            camera.is_in_family(&Token::new("Typed")),
            "Camera should be in Typed family"
        );
        assert!(
            !camera.is_in_family(&Token::new("Boundable")),
            "Camera should NOT be in Boundable family"
        );
    }

    #[test]
    fn test_is_a_gprim() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::register_builtin_schemas;
        use super::super::stage::Stage;

        register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let mesh = stage.define_prim("/Mesh", "Mesh").unwrap();

        // Mesh should be a Gprim
        assert!(
            mesh.is_a(&Token::new("Mesh")),
            "Mesh should be a Mesh (exact match)"
        );
        assert!(
            mesh.is_a(&Token::new("PointBased")),
            "Mesh should be a PointBased"
        );
        assert!(mesh.is_a(&Token::new("Gprim")), "Mesh should be a Gprim");
        assert!(
            mesh.is_a(&Token::new("Boundable")),
            "Mesh should be a Boundable"
        );
        assert!(mesh.is_a(&Token::new("Typed")), "Mesh should be a Typed");
        assert!(
            !mesh.is_a(&Token::new("Camera")),
            "Mesh should NOT be a Camera"
        );
    }

    #[test]
    fn test_is_a_xform() {
        use super::super::common::InitialLoadSet;
        use super::super::schema_registry::register_builtin_schemas;
        use super::super::stage::Stage;

        register_builtin_schemas();
        let stage = Stage::create_in_memory(InitialLoadSet::LoadAll).unwrap();
        let xform = stage.define_prim("/Xform", "Xform").unwrap();

        // Xform should be Xformable
        assert!(
            xform.is_a(&Token::new("Xform")),
            "Xform should be a Xform (exact match)"
        );
        assert!(
            xform.is_a(&Token::new("Xformable")),
            "Xform should be a Xformable"
        );
        assert!(
            xform.is_a(&Token::new("Imageable")),
            "Xform should be an Imageable"
        );
        assert!(xform.is_a(&Token::new("Typed")), "Xform should be a Typed");
        assert!(
            !xform.is_a(&Token::new("Boundable")),
            "Xform should NOT be a Boundable"
        );
    }
}
