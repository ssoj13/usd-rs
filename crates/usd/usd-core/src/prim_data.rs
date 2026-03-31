//! Internal prim data structure for cached prim information.
//!
//! Usd_PrimData is the internal structure that stores cached prim information
//! and defines the prim tree on a UsdStage. UsdPrim is a lightweight handle
//! to this data.

use super::prim_flags::PrimFlags;
use std::sync::{Arc, RwLock, Weak};
use usd_sdf::{Path, Specifier};
use usd_tf::Token;

// ============================================================================
// PrimTypeInfo
// ============================================================================

/// Type information for a prim.
///
/// Holds the full type information for a prim including type name,
/// applied API schemas, and optionally a mapped schema type name
/// for fallback types on unrecognized prims.
#[derive(Debug, Clone)]
pub struct PrimTypeInfo {
    /// The composed type name.
    pub(crate) type_name: Token,
    /// Optional mapped schema type name (for fallback types).
    pub(crate) schema_type_name: Token,
    /// Applied API schemas.
    pub(crate) applied_api_schemas: Vec<Token>,
}

impl Default for PrimTypeInfo {
    fn default() -> Self {
        Self {
            type_name: Token::new(""),
            schema_type_name: Token::new(""),
            applied_api_schemas: Vec::new(),
        }
    }
}

impl PrimTypeInfo {
    /// Creates a new PrimTypeInfo with the given type name.
    pub fn new(type_name: Token) -> Self {
        Self {
            type_name: type_name.clone(),
            schema_type_name: type_name,
            applied_api_schemas: Vec::new(),
        }
    }

    /// Creates PrimTypeInfo with type name and API schemas.
    pub fn with_api_schemas(type_name: Token, api_schemas: Vec<Token>) -> Self {
        Self {
            type_name: type_name.clone(),
            schema_type_name: type_name,
            applied_api_schemas: api_schemas,
        }
    }

    /// Creates PrimTypeInfo with full type information including mapped type.
    pub fn full(type_name: Token, schema_type_name: Token, api_schemas: Vec<Token>) -> Self {
        Self {
            type_name,
            schema_type_name,
            applied_api_schemas: api_schemas,
        }
    }

    /// Returns the concrete prim type name.
    pub fn type_name(&self) -> &Token {
        &self.type_name
    }

    /// Returns the schema type name (may differ from type_name for fallback types).
    ///
    /// This is the type name used to look up the schema in the schema registry.
    pub fn schema_type_name(&self) -> &Token {
        &self.schema_type_name
    }

    /// Returns the applied API schemas.
    ///
    /// These are the API schemas directly authored on the prim, NOT including
    /// API schemas that may be defined in the concrete prim type's definition.
    pub fn applied_api_schemas(&self) -> &[Token] {
        &self.applied_api_schemas
    }

    /// Returns true if this is an empty/untyped prim.
    pub fn is_empty(&self) -> bool {
        self.type_name.is_empty()
            && self.schema_type_name.is_empty()
            && self.applied_api_schemas.is_empty()
    }

    /// Returns the prim definition for this type info.
    ///
    /// The prim definition defines all the built-in properties and metadata
    /// of a prim of this type.
    pub fn prim_definition(
        &self,
    ) -> Option<std::sync::Arc<super::prim_definition::PrimDefinition>> {
        if self.schema_type_name.is_empty() {
            return None;
        }
        super::schema_registry::SchemaRegistry::get_instance()
            .find_concrete_prim_definition(&self.schema_type_name)
    }

    /// Returns the empty prim type info.
    pub fn empty() -> &'static Self {
        static EMPTY: std::sync::OnceLock<PrimTypeInfo> = std::sync::OnceLock::new();
        EMPTY.get_or_init(PrimTypeInfo::default)
    }
}

impl PartialEq for PrimTypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name
            && self.schema_type_name == other.schema_type_name
            && self.applied_api_schemas == other.applied_api_schemas
    }
}

impl Eq for PrimTypeInfo {}

impl std::hash::Hash for PrimTypeInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use std::hash::Hash;
        Hash::hash(&self.type_name, state);
        Hash::hash(&self.schema_type_name, state);
        Hash::hash(&self.applied_api_schemas, state);
    }
}

// ============================================================================
// PrimData
// ============================================================================

/// Internal cached prim data.
///
/// This stores the composed/cached information for a prim. The Stage builds
/// and maintains the tree of PrimData objects.
pub struct PrimData {
    /// The composed path for this prim.
    path: Path,
    /// Weak reference to owning stage.
    stage: Weak<super::stage::Stage>,
    /// Cached type information (behind RwLock for apiSchemas composition).
    prim_type_info: RwLock<PrimTypeInfo>,
    /// Cached flags.
    flags: RwLock<PrimFlags>,
    /// Composed specifier.
    specifier: Specifier,
    /// Parent prim (weak to avoid cycles).
    parent: RwLock<Option<Weak<PrimData>>>,
    /// First child.
    first_child: RwLock<Option<Arc<PrimData>>>,
    /// Next sibling (or parent link if last sibling).
    next_sibling: RwLock<Option<Arc<PrimData>>>,
    /// Whether this prim is "dead" (removed from stage).
    dead: RwLock<bool>,
    /// Property names (cached from layer).
    property_names: RwLock<Vec<Token>>,
    /// Custom data / metadata (simplified).
    custom_data: RwLock<std::collections::HashMap<Token, usd_vt::Value>>,
    /// Source prim index path for PCP lookup.
    /// Matches C++ Usd_PrimData::_primIndex — stores the path used to find
    /// the PcpPrimIndex in the PCP cache. For normal prims this equals `path`.
    /// For prototype prims/children this is the source prim's path.
    source_prim_index_path: RwLock<Option<Path>>,
}

impl std::fmt::Debug for PrimData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimData")
            .field("path", &self.path)
            .field("type_name", &self.type_name())
            .field("specifier", &self.specifier)
            .finish()
    }
}

impl PrimData {
    /// Creates new prim data.
    pub fn new(
        path: Path,
        stage: Weak<super::stage::Stage>,
        type_name: Token,
        specifier: Specifier,
    ) -> Self {
        // Newly created prims are active and loaded (no pending payload).
        // DEFINED is only set for defining specifiers (Def, Class), not Over.
        // In C++, _ComposeAndCacheFlags sets these based on layer opinions.
        let mut flags = PrimFlags::ACTIVE | PrimFlags::LOADED;
        if specifier == Specifier::Def || specifier == Specifier::Class {
            flags |= PrimFlags::DEFINED | PrimFlags::HAS_DEFINING_SPECIFIER;
        }
        if specifier == Specifier::Class {
            flags |= PrimFlags::ABSTRACT;
        }

        Self {
            path,
            stage,
            prim_type_info: RwLock::new(PrimTypeInfo::new(type_name)),
            flags: RwLock::new(flags),
            specifier,
            parent: RwLock::new(None),
            first_child: RwLock::new(None),
            next_sibling: RwLock::new(None),
            dead: RwLock::new(false),
            property_names: RwLock::new(Vec::new()),
            custom_data: RwLock::new(std::collections::HashMap::new()),
            source_prim_index_path: RwLock::new(None),
        }
    }

    /// Creates pseudo-root prim data.
    pub fn pseudo_root(stage: Weak<super::stage::Stage>) -> Self {
        let mut data = Self::new(Path::absolute_root(), stage, Token::new(""), Specifier::Def);
        data.flags
            .get_mut()
            .expect("flags accessible")
                    .insert(PrimFlags::PSEUDO_ROOT | PrimFlags::GROUP | PrimFlags::MODEL);
        data
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Returns the composed path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the prim name.
    pub fn name(&self) -> Token {
        Token::new(self.path.get_name())
    }

    /// Returns the stage.
    pub fn stage(&self) -> Option<Arc<super::stage::Stage>> {
        self.stage.upgrade()
    }

    /// Returns the type name.
    pub fn type_name(&self) -> Token {
        self.prim_type_info
            .read()
            .expect("rwlock poisoned")
            .type_name()
            .clone()
    }

    /// Returns a clone of the full type info.
    pub fn prim_type_info(&self) -> PrimTypeInfo {
        self.prim_type_info.read().expect("rwlock poisoned").clone()
    }

    /// Sets the type name on this prim's type info (runtime cache update).
    pub fn set_type_name(&self, type_name: Token) {
        let mut info = self.prim_type_info.write().expect("rwlock poisoned");
        info.type_name = type_name;
    }

    /// Sets the applied API schemas on this prim's type info.
    pub fn set_applied_api_schemas(&self, schemas: Vec<Token>) {
        self.prim_type_info
            .write()
            .expect("rwlock poisoned")
            .applied_api_schemas = schemas;
    }

    /// Returns the specifier.
    pub fn specifier(&self) -> Specifier {
        self.specifier
    }

    // ========================================================================
    // Flags
    // ========================================================================

    /// Returns true if this is the pseudo-root.
    pub fn is_pseudo_root(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::PSEUDO_ROOT)
    }

    /// Returns true if this prim is active.
    pub fn is_active(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::ACTIVE)
    }

    /// Returns true if this prim is loaded.
    pub fn is_loaded(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::LOADED)
    }

    /// Returns true if this prim is a model.
    pub fn is_model(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::MODEL)
    }

    /// Returns true if this prim is a group.
    pub fn is_group(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::GROUP)
    }

    /// Returns true if this prim is abstract.
    pub fn is_abstract(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::ABSTRACT)
    }

    /// Returns true if this prim is defined.
    pub fn is_defined(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::DEFINED)
    }

    /// Returns true if this prim has a defining specifier.
    pub fn has_defining_specifier(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::HAS_DEFINING_SPECIFIER)
    }

    /// Returns true if this prim has a payload.
    pub fn has_payload(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::HAS_PAYLOAD)
    }

    /// Returns true if this prim is an instance.
    pub fn is_instance(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::INSTANCE)
    }

    /// Returns true if this prim is a prototype.
    /// Matches C++ Usd_PrimData::IsPrototype() which checks the flag bit.
    pub fn is_prototype(&self) -> bool {
        self.flags
            .read()
            .expect("rwlock poisoned")
            .contains(PrimFlags::PROTOTYPE)
    }

    /// Returns true if this prim is in a prototype subtree.
    /// Uses path-based check: walks up to root prim and checks /__Prototype_ prefix.
    pub fn is_in_prototype(&self) -> bool {
        super::instance_cache::InstanceCache::is_path_in_prototype(self.path())
    }

    /// Returns the source prim index path used for PCP lookup.
    /// Matches C++ `_primIndex->GetPath()` — for prototype prims/children this
    /// differs from `path()` because the PcpPrimIndex lives at the source prim path.
    pub fn source_prim_index_path(&self) -> Path {
        self.source_prim_index_path
            .read()
            .expect("rwlock poisoned")
            .clone()
            .unwrap_or_else(|| self.path.clone())
    }

    /// Sets the source prim index path.
    /// Called during composition for prototype prims/children.
    /// Matches C++ `prim->_primIndex = cache->FindPrimIndex(primIndexPath)` (stage.cpp:3478).
    pub fn set_source_prim_index_path(&self, path: Path) {
        *self
            .source_prim_index_path
            .write()
            .expect("rwlock poisoned") = Some(path);
    }

    /// Returns the flags.
    pub fn flags(&self) -> PrimFlags {
        *self.flags.read().expect("rwlock poisoned")
    }

    /// Sets flags.
    pub fn set_flags(&self, flags: PrimFlags) {
        *self.flags.write().expect("rwlock poisoned") = flags;
    }

    /// Adds flags.
    pub fn add_flags(&self, flags: PrimFlags) {
        self.flags.write().expect("rwlock poisoned").insert(flags);
    }

    /// Removes flags.
    pub fn remove_flags(&self, flags: PrimFlags) {
        self.flags.write().expect("rwlock poisoned").remove(flags);
    }

    // ========================================================================
    // Tree Structure
    // ========================================================================

    /// Returns the parent prim data.
    pub fn parent(&self) -> Option<Arc<PrimData>> {
        self.parent
            .read()
            .expect("rwlock poisoned")
            .as_ref()
            .and_then(|w| w.upgrade())
    }

    /// Sets the parent.
    pub fn set_parent(&self, parent: Option<Weak<PrimData>>) {
        *self.parent.write().expect("rwlock poisoned") = parent;
    }

    /// Returns the first child.
    pub fn first_child(&self) -> Option<Arc<PrimData>> {
        self.first_child.read().expect("rwlock poisoned").clone()
    }

    /// Sets the first child.
    pub fn set_first_child(&self, child: Option<Arc<PrimData>>) {
        *self.first_child.write().expect("rwlock poisoned") = child;
    }

    /// Returns the next sibling.
    pub fn next_sibling(&self) -> Option<Arc<PrimData>> {
        self.next_sibling.read().expect("rwlock poisoned").clone()
    }

    /// Sets the next sibling.
    pub fn set_next_sibling(&self, sibling: Option<Arc<PrimData>>) {
        *self.next_sibling.write().expect("rwlock poisoned") = sibling;
    }

    /// Returns all children as a vector.
    pub fn children(&self) -> Vec<Arc<PrimData>> {
        let mut result = Vec::new();
        let mut current = self.first_child();
        while let Some(child) = current {
            let next = child.next_sibling();
            result.push(child);
            current = next;
        }
        result
    }

    /// Adds a child prim.
    pub fn add_child(self: &Arc<Self>, child: Arc<PrimData>) {
        child.set_parent(Some(Arc::downgrade(self)));

        let mut first = self.first_child.write().expect("rwlock poisoned");
        if first.is_none() {
            *first = Some(child);
        } else {
            // Find last sibling and add there
            drop(first);
            let mut current = self.first_child();
            while let Some(cur) = current.clone() {
                if cur.next_sibling().is_none() {
                    cur.set_next_sibling(Some(child));
                    break;
                }
                current = cur.next_sibling();
            }
        }
    }

    /// Removes all children from this prim.
    pub fn clear_children(&self) {
        *self.first_child.write().expect("rwlock poisoned") = None;
    }

    /// Removes a child by path from this prim's children list.
    pub fn remove_child(&self, child_path: &Path) {
        let first = self.first_child();
        if first.is_none() {
            return;
        }
        let first = first.unwrap();

        // If first child is the one to remove, replace with its sibling
        if first.path() == child_path {
            let next = first.next_sibling();
            *self.first_child.write().expect("rwlock poisoned") = next;
            first.mark_dead();
            return;
        }

        // Walk the sibling list to find and unlink the child
        let mut prev = first;
        while let Some(cur) = prev.next_sibling() {
            if cur.path() == child_path {
                let next = cur.next_sibling();
                prev.set_next_sibling(next);
                cur.mark_dead();
                return;
            }
            prev = cur;
        }
    }

    // ========================================================================
    // Dead/Live Status
    // ========================================================================

    /// Returns true if this prim data is dead (removed from stage).
    pub fn is_dead(&self) -> bool {
        *self.dead.read().expect("rwlock poisoned")
    }

    /// Marks this prim data as dead.
    pub fn mark_dead(&self) {
        *self.dead.write().expect("rwlock poisoned") = true;
    }

    /// Returns true if this prim data is valid (not dead and has stage).
    pub fn is_valid(&self) -> bool {
        !self.is_dead() && self.stage.upgrade().is_some()
    }

    // ========================================================================
    // Properties
    // ========================================================================

    /// Returns property names.
    pub fn property_names(&self) -> Vec<Token> {
        self.property_names.read().expect("rwlock poisoned").clone()
    }

    /// Sets property names.
    pub fn set_property_names(&self, names: Vec<Token>) {
        *self.property_names.write().expect("rwlock poisoned") = names;
    }

    /// Adds a property name.
    pub fn add_property_name(&self, name: Token) {
        self.property_names
            .write()
            .expect("rwlock poisoned")
            .push(name);
    }

    // ========================================================================
    // Metadata
    // ========================================================================

    /// Gets metadata value.
    pub fn get_metadata(&self, key: &Token) -> Option<usd_vt::Value> {
        self.custom_data
            .read()
            .expect("rwlock poisoned")
            .get(key)
            .cloned()
    }

    /// Sets metadata value.
    pub fn set_metadata(&self, key: Token, value: usd_vt::Value) {
        self.custom_data
            .write()
            .expect("rwlock poisoned")
            .insert(key, value);
    }

    /// Returns true if metadata exists.
    pub fn has_metadata(&self, key: &Token) -> bool {
        self.custom_data
            .read()
            .expect("rwlock poisoned")
            .contains_key(key)
    }

    /// Clears metadata.
    pub fn clear_metadata(&self, key: &Token) -> bool {
        self.custom_data
            .write()
            .expect("rwlock poisoned")
            .remove(key)
            .is_some()
    }
}

// ============================================================================
// PrimDataHandle
// ============================================================================

/// Handle to PrimData that checks validity on access.
#[derive(Clone)]
pub struct PrimDataHandle {
    data: Option<Arc<PrimData>>,
}

impl PrimDataHandle {
    /// Creates a new handle.
    pub fn new(data: Arc<PrimData>) -> Self {
        Self { data: Some(data) }
    }

    /// Creates an invalid handle.
    pub fn invalid() -> Self {
        Self { data: None }
    }

    /// Returns true if the handle is valid.
    pub fn is_valid(&self) -> bool {
        self.data.as_ref().map(|d| d.is_valid()).unwrap_or(false)
    }

    /// Returns the prim data, or None if invalid.
    pub fn get(&self) -> Option<&Arc<PrimData>> {
        self.data.as_ref().filter(|d| d.is_valid())
    }

    /// Returns the prim data without validity check.
    pub fn get_unchecked(&self) -> Option<&Arc<PrimData>> {
        self.data.as_ref()
    }
}

impl std::fmt::Debug for PrimDataHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.data {
            Some(d) if d.is_valid() => write!(f, "PrimDataHandle({:?})", d.path()),
            Some(_) => write!(f, "PrimDataHandle(dead)"),
            None => write!(f, "PrimDataHandle(invalid)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Weak;

    #[test]
    fn test_prim_type_info() {
        let info = PrimTypeInfo::new(Token::new("Mesh"));
        assert_eq!(info.type_name().get_text(), "Mesh");
        assert!(info.applied_api_schemas().is_empty());
    }

    #[test]
    fn test_prim_data_flags() {
        let data = PrimData::new(
            Path::from_string("/World").unwrap(),
            Weak::new(),
            Token::new("Xform"),
            Specifier::Def,
        );
        assert!(data.is_active());
        assert!(data.is_defined());
        assert!(data.has_defining_specifier());
        assert!(!data.is_pseudo_root());
    }

    #[test]
    fn test_pseudo_root() {
        let data = PrimData::pseudo_root(Weak::new());
        assert!(data.is_pseudo_root());
        assert_eq!(data.path(), &Path::absolute_root());
    }

    #[test]
    fn test_prim_data_handle() {
        let handle = PrimDataHandle::invalid();
        assert!(!handle.is_valid());
    }
}
