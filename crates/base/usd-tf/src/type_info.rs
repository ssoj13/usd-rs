//! Runtime type system.
//!
//! This module provides `TfType`, a dynamic runtime type representation
//! that supports type registration, hierarchy tracking, and lookups.
//!
//! # Examples
//!
//! ```
//! use usd_tf::TfType;
//!
//! // Get the type for a concrete Rust type
//! let int_type = TfType::find::<i32>();
//! assert!(!int_type.is_unknown());
//!
//! // Unknown type for unregistered types
//! struct Unregistered;
//! let unknown = TfType::find::<Unregistered>();
//! assert!(unknown.is_unknown());
//! ```

use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// Base trait for type factories.
///
/// A factory can instantiate types. Implementations should provide
/// methods to create instances given various arguments.
pub trait FactoryBase: Send + Sync + Any {
    /// Returns this factory as Any for downcasting.
    fn as_any(&self) -> &dyn Any;
}

/// Callback invoked when a declared type needs to be defined.
pub type DefinitionCallback = fn(TfType);

/// Internal key identifying a registered type: either a real Rust TypeId or a
/// sequential plugin-type id (allocated at runtime for name-only plugin types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TypeKey {
    Real(TypeId),
    Plugin(u64),
}

impl PartialOrd for TypeKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TypeKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Real TypeIds come before plugin ids; within each variant compare by value.
        match (self, other) {
            (TypeKey::Real(a), TypeKey::Real(b)) => a.cmp(b),
            (TypeKey::Plugin(a), TypeKey::Plugin(b)) => a.cmp(b),
            (TypeKey::Real(_), TypeKey::Plugin(_)) => std::cmp::Ordering::Less,
            (TypeKey::Plugin(_), TypeKey::Real(_)) => std::cmp::Ordering::Greater,
        }
    }
}

/// Internal type information storage.
struct TypeInfo {
    /// The type name (usually from std::any::type_name)
    type_name: String,
    /// Base types (parent classes)
    base_types: Vec<TypeKey>,
    /// Derived types (child classes)
    derived_types: HashSet<TypeKey>,
    /// Whether this is an enum type
    is_enum: bool,
    /// Whether this is a plain old data type
    is_pod: bool,
    /// Size of the type
    sizeof_type: usize,
    /// Aliases for this type
    aliases: Vec<String>,
    /// Factory for this type
    factory: Option<Arc<dyn FactoryBase>>,
    /// Definition callback (called when type needs to be defined)
    definition_callback: Option<DefinitionCallback>,
}

impl Clone for TypeInfo {
    fn clone(&self) -> Self {
        Self {
            type_name: self.type_name.clone(),
            base_types: self.base_types.clone(),
            derived_types: self.derived_types.clone(),
            is_enum: self.is_enum,
            is_pod: self.is_pod,
            sizeof_type: self.sizeof_type,
            aliases: self.aliases.clone(),
            factory: self.factory.clone(),
            definition_callback: self.definition_callback,
        }
    }
}

/// Global type registry.
struct TypeRegistry {
    /// Map from TypeKey to TypeInfo
    types: HashMap<TypeKey, TypeInfo>,
    /// Map from type name to TypeKey
    names: HashMap<String, TypeKey>,
    /// Map from alias (base TypeKey, alias name) to TypeKey
    aliases: HashMap<(TypeKey, String), TypeKey>,
}

/// Global counter for allocating plugin type ids.
static NEXT_PLUGIN_ID: AtomicU64 = AtomicU64::new(0);

impl TypeRegistry {
    fn new() -> Self {
        let mut registry = Self {
            types: HashMap::new(),
            names: HashMap::new(),
            aliases: HashMap::new(),
        };

        // Register some built-in types
        registry.register_builtin::<()>("void", 0, true);
        registry.register_builtin::<bool>("bool", 1, true);
        registry.register_builtin::<i8>("int8", 1, true);
        registry.register_builtin::<i16>("int16", 2, true);
        registry.register_builtin::<i32>("int32", 4, true);
        registry.register_builtin::<i64>("int64", 8, true);
        registry.register_builtin::<u8>("uint8", 1, true);
        registry.register_builtin::<u16>("uint16", 2, true);
        registry.register_builtin::<u32>("uint32", 4, true);
        registry.register_builtin::<u64>("uint64", 8, true);
        registry.register_builtin::<f32>("float", 4, true);
        registry.register_builtin::<f64>("double", 8, true);
        registry.register_builtin::<String>("string", std::mem::size_of::<String>(), false);

        // NOTE: Gf types are registered by usd_gf::register_gf_types()
        // to avoid circular dependency between tf and gf.

        registry
    }

    /// Allocate a unique TypeKey for a name-only plugin type.
    fn alloc_plugin_type_key() -> TypeKey {
        let id = NEXT_PLUGIN_ID.fetch_add(1, Ordering::Relaxed);
        TypeKey::Plugin(id)
    }

    /// Register a builtin type with the type registry.
    /// Used by downstream crates (e.g. gf) to register their types with tf.
    pub fn register_builtin<T: 'static>(&mut self, name: &str, size: usize, is_pod: bool) {
        let key = TypeKey::Real(TypeId::of::<T>());
        let info = TypeInfo {
            type_name: name.to_string(),
            base_types: Vec::new(),
            derived_types: HashSet::new(),
            is_enum: false,
            is_pod,
            sizeof_type: size,
            aliases: Vec::new(),
            factory: None,
            definition_callback: None,
        };
        self.types.insert(key, info);
        self.names.insert(name.to_string(), key);
    }
}

static REGISTRY: RwLock<Option<TypeRegistry>> = RwLock::new(None);

fn with_registry<F, R>(f: F) -> R
where
    F: FnOnce(&TypeRegistry) -> R,
{
    let guard = REGISTRY.read().expect("type registry lock poisoned");
    if let Some(ref reg) = *guard {
        f(reg)
    } else {
        drop(guard);
        let mut guard = REGISTRY.write().expect("type registry lock poisoned");
        if guard.is_none() {
            *guard = Some(TypeRegistry::new());
        }
        drop(guard);
        let guard = REGISTRY.read().expect("type registry lock poisoned");
        f(guard.as_ref().expect("registry just initialized"))
    }
}

fn with_registry_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut TypeRegistry) -> R,
{
    let mut guard = REGISTRY.write().expect("type registry lock poisoned");
    if guard.is_none() {
        *guard = Some(TypeRegistry::new());
    }
    f(guard.as_mut().expect("registry just initialized"))
}

/// Register an external builtin type with the Tf type system.
/// Used by downstream crates (e.g. gf) to register their types with tf,
/// breaking circular dependencies. Matches C++ TF_REGISTRY_FUNCTION(TfType).
pub fn register_type<T: 'static>(name: &str, size: usize, is_pod: bool) {
    with_registry_mut(|reg| reg.register_builtin::<T>(name, size, is_pod));
}

/// Declare a name-only plugin type without a backing Rust type.
///
/// Used by PlugRegistry when registering types from plugInfo.json.
/// Idempotent: returns the existing TfType if the name was already declared.
/// Matches the C++ path in `PlugPlugin::_DeclareTypes`.
pub fn declare_by_name(name: &str) -> TfType {
    declare_by_name_with_bases(name, &[])
}

/// Declare a name-only plugin type with explicit base-type names.
///
/// Each base name that does not yet exist is created as a placeholder.
/// Matches the C++ path in `PlugPlugin::_DeclareTypes`.
pub fn declare_by_name_with_bases(name: &str, base_names: &[&str]) -> TfType {
    // Resolve or lazily-create each base by name, allocating plugin TypeKeys
    // for any base that hasn't been seen yet.
    let base_keys: Vec<TypeKey> = base_names
        .iter()
        .map(|&base_name| {
            with_registry_mut(|reg| {
                if let Some(&key) = reg.names.get(base_name) {
                    return key;
                }
                let key = TypeRegistry::alloc_plugin_type_key();
                let info = TypeInfo {
                    type_name: base_name.to_string(),
                    base_types: Vec::new(),
                    derived_types: HashSet::new(),
                    is_enum: false,
                    is_pod: false,
                    sizeof_type: 0,
                    aliases: Vec::new(),
                    factory: None,
                    definition_callback: None,
                };
                reg.types.insert(key, info);
                reg.names.insert(base_name.to_string(), key);
                key
            })
        })
        .collect();

    with_registry_mut(|reg| {
        if let Some(&existing_key) = reg.names.get(name) {
            // Already declared — add any new bases.
            let new_bases: Vec<TypeKey> = base_keys
                .iter()
                .copied()
                .filter(|&key| {
                    reg.types
                        .get(&existing_key)
                        .map(|info| !info.base_types.contains(&key))
                        .unwrap_or(false)
                })
                .collect();
            if let Some(info) = reg.types.get_mut(&existing_key) {
                info.base_types.extend(new_bases.iter().copied());
            }
            for base_key in new_bases {
                if let Some(base_info) = reg.types.get_mut(&base_key) {
                    base_info.derived_types.insert(existing_key);
                }
            }
            return TfType {
                type_key: Some(existing_key),
            };
        }

        let key = TypeRegistry::alloc_plugin_type_key();
        let info = TypeInfo {
            type_name: name.to_string(),
            base_types: base_keys.clone(),
            derived_types: HashSet::new(),
            is_enum: false,
            is_pod: false,
            sizeof_type: 0,
            aliases: Vec::new(),
            factory: None,
            definition_callback: None,
        };
        reg.types.insert(key, info);
        reg.names.insert(name.to_string(), key);
        for base_key in &base_keys {
            if let Some(base_info) = reg.types.get_mut(base_key) {
                base_info.derived_types.insert(key);
            }
        }
        TfType {
            type_key: Some(key),
        }
    })
}

/// Legacy flags for type properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyFlags {
    /// Abstract type (cannot be instantiated).
    Abstract = 0x01,
    /// Concrete type.
    Concrete = 0x02,
    /// Manufacturable type (implies concrete).
    Manufacturable = 0x08,
}

/// A runtime type representation.
///
/// `TfType` provides a lightweight handle to type information registered
/// with the type system. It supports:
///
/// - Type lookup by Rust type, TypeId, or name
/// - Type hierarchy queries (base types, derived types)
/// - Type comparison and ordering
/// - Factory registration for type instantiation
///
/// Unknown types (types not registered with the system) are represented
/// by a special "unknown" TfType.
#[derive(Clone, Copy)]
pub struct TfType {
    type_key: Option<TypeKey>,
}

impl TfType {
    /// Returns the unknown type.
    ///
    /// The unknown type represents types that have not been registered
    /// with the type system.
    #[must_use]
    pub fn unknown() -> Self {
        Self { type_key: None }
    }

    /// Returns an empty TfType, representing the unknown type.
    /// Equivalent to `TfType::unknown()`.
    #[must_use]
    pub fn get_unknown_type() -> Self {
        Self::unknown()
    }

    /// Returns the root type of the type hierarchy.
    ///
    /// All registered types implicitly derive from the root type.
    #[must_use]
    pub fn root() -> Self {
        Self {
            type_key: Some(TypeKey::Real(TypeId::of::<()>())),
        }
    }

    /// Returns the root type of the type hierarchy.
    /// Alias for `root()`.
    #[must_use]
    pub fn get_root() -> Self {
        Self::root()
    }

    /// Finds the TfType for a Rust type T.
    ///
    /// Returns the unknown type if T has not been registered.
    #[must_use]
    pub fn find<T: 'static>() -> Self {
        Self::find_by_typeid(TypeId::of::<T>())
    }

    /// Finds the TfType for a given TypeId.
    ///
    /// Returns the unknown type if the TypeId has not been registered.
    #[must_use]
    pub fn find_by_typeid(type_id: TypeId) -> Self {
        let key = TypeKey::Real(type_id);
        with_registry(|reg| {
            if reg.types.contains_key(&key) {
                Self {
                    type_key: Some(key),
                }
            } else {
                Self::unknown()
            }
        })
    }

    /// Finds the TfType with the given name.
    ///
    /// Returns the unknown type if no type with that name exists.
    #[must_use]
    pub fn find_by_name(name: &str) -> Self {
        with_registry(|reg| {
            if let Some(&key) = reg.names.get(name) {
                Self {
                    type_key: Some(key),
                }
            } else {
                Self::unknown()
            }
        })
    }

    /// Declares a new type with the given name.
    ///
    /// If the type is already declared, this is a no-op.
    pub fn declare<T: 'static>(name: &str) {
        Self::declare_with_bases::<T>(name, &[]);
    }

    /// Declares a new type with the given name and base types.
    pub fn declare_with_bases<T: 'static>(name: &str, bases: &[TypeId]) {
        let self_key = TypeKey::Real(TypeId::of::<T>());
        let root_key = TypeKey::Real(TypeId::of::<()>());
        // Convert caller-supplied TypeId bases to TypeKeys.
        let base_keys: Vec<TypeKey> = bases.iter().map(|&id| TypeKey::Real(id)).collect();

        with_registry_mut(|reg| {
            if reg.types.contains_key(&self_key) {
                // Type already declared, collect new bases to add.
                let mut new_bases = Vec::new();
                if let Some(info) = reg.types.get(&self_key) {
                    for &base_key in &base_keys {
                        if !info.base_types.contains(&base_key) {
                            new_bases.push(base_key);
                        }
                    }
                }
                if let Some(info) = reg.types.get_mut(&self_key) {
                    info.base_types.extend(new_bases.iter().copied());
                }
                for base_key in new_bases {
                    if let Some(base_info) = reg.types.get_mut(&base_key) {
                        base_info.derived_types.insert(self_key);
                    }
                }
                return;
            }

            // When no explicit bases given, implicitly derive from root (like C++ TfType).
            // Exception: root itself and Unknown have no implicit parent.
            let effective_bases = if base_keys.is_empty() && self_key != root_key {
                vec![root_key]
            } else {
                base_keys.clone()
            };

            let info = TypeInfo {
                type_name: name.to_string(),
                base_types: effective_bases.clone(),
                derived_types: HashSet::new(),
                is_enum: false,
                is_pod: false,
                sizeof_type: std::mem::size_of::<T>(),
                aliases: Vec::new(),
                factory: None,
                definition_callback: None,
            };

            reg.types.insert(self_key, info);
            reg.names.insert(name.to_string(), self_key);

            for base_key in &effective_bases {
                if let Some(base_info) = reg.types.get_mut(base_key) {
                    base_info.derived_types.insert(self_key);
                }
            }
        });
    }

    /// Declares a new type with a definition callback.
    pub fn declare_with_callback<T: 'static>(
        name: &str,
        bases: &[TypeId],
        callback: DefinitionCallback,
    ) {
        let self_key = TypeKey::Real(TypeId::of::<T>());
        let root_key = TypeKey::Real(TypeId::of::<()>());
        let base_keys: Vec<TypeKey> = bases.iter().map(|&id| TypeKey::Real(id)).collect();

        with_registry_mut(|reg| {
            if reg.types.contains_key(&self_key) {
                return;
            }

            let effective_bases = if base_keys.is_empty() && self_key != root_key {
                vec![root_key]
            } else {
                base_keys.clone()
            };

            let info = TypeInfo {
                type_name: name.to_string(),
                base_types: effective_bases.clone(),
                derived_types: HashSet::new(),
                is_enum: false,
                is_pod: false,
                sizeof_type: std::mem::size_of::<T>(),
                aliases: Vec::new(),
                factory: None,
                definition_callback: Some(callback),
            };

            reg.types.insert(self_key, info);
            reg.names.insert(name.to_string(), self_key);

            for base_key in &effective_bases {
                if let Some(base_info) = reg.types.get_mut(base_key) {
                    base_info.derived_types.insert(self_key);
                }
            }
        });
    }

    /// Declares an enum type.
    pub fn declare_enum<T: 'static>(name: &str) {
        let self_key = TypeKey::Real(TypeId::of::<T>());
        let root_key = TypeKey::Real(TypeId::of::<()>());

        with_registry_mut(|reg| {
            if reg.types.contains_key(&self_key) {
                return;
            }

            // Enums also derive from root implicitly.
            let effective_bases = if self_key != root_key {
                vec![root_key]
            } else {
                Vec::new()
            };

            let info = TypeInfo {
                type_name: name.to_string(),
                base_types: effective_bases.clone(),
                derived_types: HashSet::new(),
                is_enum: true,
                is_pod: true,
                sizeof_type: std::mem::size_of::<T>(),
                aliases: Vec::new(),
                factory: None,
                definition_callback: None,
            };

            reg.types.insert(self_key, info);
            reg.names.insert(name.to_string(), self_key);

            for base_key in &effective_bases {
                if let Some(base_info) = reg.types.get_mut(base_key) {
                    base_info.derived_types.insert(self_key);
                }
            }
        });
    }

    /// Adds an alias for this type under a base type.
    pub fn add_alias(&self, base: TfType, alias: &str) {
        if let (Some(self_key), Some(base_key)) = (self.type_key, base.type_key) {
            with_registry_mut(|reg| {
                if let Some(info) = reg.types.get_mut(&self_key) {
                    if !info.aliases.contains(&alias.to_string()) {
                        info.aliases.push(alias.to_string());
                    }
                }
                reg.aliases.insert((base_key, alias.to_string()), self_key);
            });
        }
    }

    /// Finds a derived type by name under this base type (transitive).
    ///
    /// Walks the full base_types chain to check if the named type
    /// transitively derives from this type, matching C++ TfType behavior.
    #[must_use]
    pub fn find_derived_by_name(&self, name: &str) -> Self {
        if let Some(base_key) = self.type_key {
            with_registry(|reg| {
                // First check aliases
                if let Some(&key) = reg.aliases.get(&(base_key, name.to_string())) {
                    return Self {
                        type_key: Some(key),
                    };
                }
                // Then check type names with transitive derivation
                if let Some(&key) = reg.names.get(name) {
                    if is_derived_from(reg, key, base_key) {
                        return Self {
                            type_key: Some(key),
                        };
                    }
                }
                Self::unknown()
            })
        } else {
            Self::unknown()
        }
    }

    /// Returns the type name.
    #[must_use]
    pub fn type_name(&self) -> String {
        if let Some(key) = self.type_key {
            with_registry(|reg| {
                reg.types
                    .get(&key)
                    .map(|info| info.type_name.clone())
                    .unwrap_or_default()
            })
        } else {
            String::new()
        }
    }

    /// Returns the Rust TypeId for this type, or None for plugin (name-only) types.
    #[must_use]
    pub fn get_typeid(&self) -> Option<TypeId> {
        match self.type_key {
            Some(TypeKey::Real(id)) => Some(id),
            _ => None,
        }
    }

    /// Returns the base types of this type.
    #[must_use]
    pub fn base_types(&self) -> Vec<TfType> {
        if let Some(key) = self.type_key {
            with_registry(|reg| {
                reg.types
                    .get(&key)
                    .map(|info| {
                        info.base_types
                            .iter()
                            .map(|&k| TfType { type_key: Some(k) })
                            .collect()
                    })
                    .unwrap_or_default()
            })
        } else {
            Vec::new()
        }
    }

    /// Returns the directly derived types (immediate children).
    #[must_use]
    pub fn get_directly_derived_types(&self) -> Vec<TfType> {
        if let Some(key) = self.type_key {
            with_registry(|reg| {
                reg.types
                    .get(&key)
                    .map(|info| {
                        info.derived_types
                            .iter()
                            .map(|&k| TfType { type_key: Some(k) })
                            .collect()
                    })
                    .unwrap_or_default()
            })
        } else {
            Vec::new()
        }
    }

    /// Returns all derived types (recursive).
    #[must_use]
    pub fn get_all_derived_types(&self) -> HashSet<TfType> {
        let mut result = HashSet::new();
        self.collect_all_derived(&mut result);
        result
    }

    fn collect_all_derived(&self, result: &mut HashSet<TfType>) {
        for derived in self.get_directly_derived_types() {
            if result.insert(derived) {
                derived.collect_all_derived(result);
            }
        }
    }

    /// Returns all ancestor types in C3 linearization order (same algorithm as C++ TfType).
    ///
    /// The type itself is included as the first element. For single inheritance
    /// the result is just the linear chain. For multiple inheritance, the C3
    /// algorithm (as used by Python's MRO and C++ TfType) is applied to
    /// produce a consistent, monotonic ordering.
    ///
    /// Returns an empty Vec if called on the unknown type.
    #[must_use]
    pub fn get_all_ancestor_types(&self) -> Vec<TfType> {
        if self.is_unknown() {
            return Vec::new();
        }
        let mut result = Vec::new();
        c3_linearize(*self, &mut result);
        result
    }

    /// Returns the aliases registered for this type.
    #[must_use]
    pub fn aliases(&self) -> Vec<String> {
        if let Some(key) = self.type_key {
            with_registry(|reg| {
                reg.types
                    .get(&key)
                    .map(|info| info.aliases.clone())
                    .unwrap_or_default()
            })
        } else {
            Vec::new()
        }
    }

    /// Returns the aliases registered for a derived type under this base type.
    ///
    /// # Arguments
    ///
    /// * `derived_type` - The derived type to get aliases for
    #[must_use]
    pub fn get_aliases_for_derived(&self, derived_type: TfType) -> Vec<String> {
        if let (Some(base_key), Some(derived_key)) = (self.type_key, derived_type.type_key) {
            with_registry(|reg| {
                let mut result = Vec::new();
                for ((alias_base_key, alias_name), &alias_type_key) in &reg.aliases {
                    if *alias_base_key == base_key && alias_type_key == derived_key {
                        result.push(alias_name.clone());
                    }
                }
                result
            })
        } else {
            Vec::new()
        }
    }

    /// Returns the canonical type for this type.
    ///
    /// For most types, this returns the type itself. This is used for types
    /// that may have multiple representations (e.g., typedefs) to return
    /// the canonical underlying type.
    #[must_use]
    pub fn get_canonical_type(&self) -> TfType {
        // In Rust, types are already canonical (no typedef aliasing like C++)
        *self
    }

    /// Copies the first N base types to the output slice.
    ///
    /// Returns the total number of base types for this type.
    ///
    /// This is more efficient than `base_types()` when you only need the first few bases.
    ///
    /// # Arguments
    ///
    /// * `out` - Mutable slice to write base types into
    pub fn get_n_base_types(&self, out: &mut [TfType]) -> usize {
        if let Some(key) = self.type_key {
            with_registry(|reg| {
                if let Some(info) = reg.types.get(&key) {
                    let total = info.base_types.len();
                    let count = out.len().min(total);
                    for (i, &base_key) in info.base_types.iter().take(count).enumerate() {
                        out[i] = TfType {
                            type_key: Some(base_key),
                        };
                    }
                    total
                } else {
                    0
                }
            })
        } else {
            0
        }
    }

    /// Returns true if this type is the same as or derived from T.
    /// Shorthand for `self.is_a(TfType::find::<T>())`.
    #[must_use]
    pub fn is_a_type<T: 'static>(&self) -> bool {
        self.is_a(Self::find::<T>())
    }

    /// Returns true if this type is the same as or derived from `query_type`.
    #[must_use]
    pub fn is_a(&self, query_type: TfType) -> bool {
        if self.is_unknown() || query_type.is_unknown() {
            return false;
        }

        if self.type_key == query_type.type_key {
            return true;
        }

        for base in self.base_types() {
            if base.is_a(query_type) {
                return true;
            }
        }

        false
    }

    /// Returns true if this is the unknown type.
    #[must_use]
    pub fn is_unknown(&self) -> bool {
        self.type_key.is_none()
    }

    /// Returns true if this is the root type.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.type_key == Some(TypeKey::Real(TypeId::of::<()>()))
    }

    /// Returns true if this is an enum type.
    #[must_use]
    pub fn is_enum(&self) -> bool {
        if let Some(key) = self.type_key {
            with_registry(|reg| reg.types.get(&key).map(|i| i.is_enum).unwrap_or(false))
        } else {
            false
        }
    }

    /// Returns true if this is a plain old data type.
    #[must_use]
    pub fn is_plain_old_data_type(&self) -> bool {
        if let Some(key) = self.type_key {
            with_registry(|reg| reg.types.get(&key).map(|i| i.is_pod).unwrap_or(false))
        } else {
            false
        }
    }

    /// Returns the size of this type (like C++ sizeof).
    #[must_use]
    pub fn get_sizeof(&self) -> usize {
        if let Some(key) = self.type_key {
            with_registry(|reg| reg.types.get(&key).map(|i| i.sizeof_type).unwrap_or(0))
        } else {
            0
        }
    }

    /// Sets the factory for this type.
    ///
    /// The factory cannot be changed once set.
    pub fn set_factory(&self, factory: Arc<dyn FactoryBase>) {
        if self.is_unknown() || self.is_root() {
            eprintln!("TfType::set_factory: cannot set factory on unknown or root type");
            return;
        }
        if let Some(key) = self.type_key {
            with_registry_mut(|reg| {
                if let Some(info) = reg.types.get_mut(&key) {
                    if info.factory.is_none() {
                        info.factory = Some(factory);
                    }
                }
            });
        }
    }

    /// Gets the factory for this type, firing the definition callback first if set.
    ///
    /// Matches C++ `_ExecuteDefinitionCallback()` — the callback runs once on
    /// first factory access, allowing lazy type setup.
    #[must_use]
    pub fn get_factory(&self) -> Option<Arc<dyn FactoryBase>> {
        let key = self.type_key?;

        // Check whether a definition callback needs to be fired.
        // Extract it under the write lock so it runs exactly once.
        let callback = with_registry_mut(|reg| {
            reg.types
                .get_mut(&key)
                .and_then(|info| info.definition_callback.take())
        });

        // Fire the callback outside the lock to avoid re-entrant deadlock.
        if let Some(cb) = callback {
            cb(*self);
        }

        with_registry(|reg| reg.types.get(&key).and_then(|info| info.factory.clone()))
    }

    /// Returns true if this type has a factory set.
    #[must_use]
    pub fn has_factory(&self) -> bool {
        self.get_factory().is_some()
    }

    /// Gets the factory for this type, downcasted to the concrete factory type.
    ///
    /// Matches C++ `TfType::GetFactory<T>()`. Returns `None` if no factory is
    /// set or if the factory cannot be downcasted to `T`.
    ///
    /// # Example
    /// ```ignore
    /// struct MyFactory;
    /// impl FactoryBase for MyFactory {
    ///     fn as_any(&self) -> &dyn std::any::Any { self }
    /// }
    ///
    /// let ty = TfType::find::<MyType>();
    /// ty.set_factory(Arc::new(MyFactory));
    /// let factory: Option<&MyFactory> = ty.get_factory_as::<MyFactory>();
    /// ```
    #[must_use]
    pub fn get_factory_as<T: FactoryBase + 'static>(&self) -> Option<Arc<dyn FactoryBase>> {
        let factory = self.get_factory()?;
        // Verify the factory is of the requested type
        if factory.as_any().is::<T>() {
            Some(factory)
        } else {
            None
        }
    }

    /// Returns the canonical type name for a Rust type.
    #[must_use]
    pub fn canonical_type_name<T: 'static>() -> String {
        std::any::type_name::<T>().to_string()
    }
}

/// Merge step of the C3 linearization algorithm.
///
/// Iterates over the sequences and picks the first element that does not
/// appear in the tail of any other sequence (a valid "head"). Mirrors the
/// C++ `_MergeAncestors` helper in pxr/base/tf/type.cpp.
///
/// Returns `false` when sequences are not empty but no valid head can be
/// found (inconsistent inheritance hierarchy).
fn c3_merge(seqs: &mut Vec<Vec<TfType>>, result: &mut Vec<TfType>) -> bool {
    loop {
        // Remove empty sequences upfront to simplify iteration.
        seqs.retain(|s| !s.is_empty());
        if seqs.is_empty() {
            return true;
        }

        // Find a candidate: head of some sequence that is not in any tail.
        let cand = 'outer: {
            for i in 0..seqs.len() {
                let head = seqs[i][0];
                // Check that `head` does not appear in the tail of any seq.
                let in_tail = seqs.iter().any(|s| s.len() > 1 && s[1..].contains(&head));
                if !in_tail {
                    break 'outer Some(head);
                }
            }
            None
        };

        match cand {
            None => {
                // No valid candidate — inconsistent hierarchy.
                return false;
            }
            Some(c) => {
                result.push(c);
                // Remove the chosen candidate from the front of any sequence
                // that starts with it.
                for seq in seqs.iter_mut() {
                    if seq.first() == Some(&c) {
                        seq.remove(0);
                    }
                }
            }
        }
    }
}

/// Compute the C3 linearization of `ty` and append it to `result`.
///
/// Mirrors C++ `TfType::GetAllAncestorTypes` exactly:
/// - Single / no inheritance: push self then recurse into the one base.
/// - Multiple inheritance: build the three input sequences
///   `[self]`, `[direct bases...]`, `[MRO(base1), MRO(base2), ...]`
///   and merge them with `c3_merge`.
fn c3_linearize(ty: TfType, result: &mut Vec<TfType>) {
    let bases = ty.base_types();
    let n = bases.len();

    // Simple case: 0 or 1 base (no ambiguity).
    if n <= 1 {
        result.push(ty);
        if n == 1 {
            c3_linearize(bases[0], result);
        }
        return;
    }

    // Multiple inheritance: run C3 merge.
    // seqs[0] = [ty]  (the type itself)
    // seqs[1] = direct bases in declaration order
    // seqs[2..] = MRO of each direct base
    let mut seqs: Vec<Vec<TfType>> = Vec::with_capacity(2 + n);

    seqs.push(vec![ty]);
    seqs.push(bases.clone());

    for base in &bases {
        let mut base_mro = Vec::new();
        c3_linearize(*base, &mut base_mro);
        seqs.push(base_mro);
    }

    let ok = c3_merge(&mut seqs, result);
    if !ok {
        // Inconsistent hierarchy — fall back to DFS order so callers still
        // get a usable (though unspecified) ordering, matching C++ behavior
        // of logging an error but continuing.
        // We don't have TF_CODING_ERROR here, but at minimum push self.
        if result.is_empty() || result[0] != ty {
            result.insert(0, ty);
        }
    }
}

/// Recursively check if `type_key` derives from `base_key` by walking
/// the base_types chain. Must be called inside with_registry().
fn is_derived_from(reg: &TypeRegistry, type_key: TypeKey, base_key: TypeKey) -> bool {
    if type_key == base_key {
        return true;
    }
    if let Some(info) = reg.types.get(&type_key) {
        for &parent_key in &info.base_types {
            if is_derived_from(reg, parent_key, base_key) {
                return true;
            }
        }
    }
    false
}

impl Default for TfType {
    fn default() -> Self {
        Self::unknown()
    }
}

impl PartialEq for TfType {
    fn eq(&self, other: &Self) -> bool {
        self.type_key == other.type_key
    }
}

impl Eq for TfType {}

impl PartialOrd for TfType {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TfType {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.type_key.cmp(&other.type_key)
    }
}

impl Hash for TfType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.type_key.hash(state);
    }
}

impl fmt::Debug for TfType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unknown() {
            write!(f, "TfType(unknown)")
        } else {
            write!(f, "TfType({})", self.type_name())
        }
    }
}

impl fmt::Display for TfType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_unknown() {
            write!(f, "unknown")
        } else {
            write!(f, "{}", self.type_name())
        }
    }
}

impl From<TfType> for bool {
    fn from(t: TfType) -> bool {
        !t.is_unknown()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unknown_type() {
        let unknown = TfType::unknown();
        assert!(unknown.is_unknown());
        assert_eq!(unknown.type_name(), "");
    }

    #[test]
    fn test_root_type() {
        let root = TfType::root();
        assert!(!root.is_unknown());
        assert!(root.is_root());
    }

    #[test]
    fn test_find_builtin() {
        let int_type = TfType::find::<i32>();
        assert!(!int_type.is_unknown());
        assert_eq!(int_type.type_name(), "int32");

        let float_type = TfType::find::<f32>();
        assert!(!float_type.is_unknown());
        assert_eq!(float_type.type_name(), "float");
    }

    #[test]
    fn test_find_by_name() {
        let double_type = TfType::find_by_name("double");
        assert!(!double_type.is_unknown());
        assert_eq!(double_type.type_name(), "double");
    }

    // test_find_gf_half moved to usd-gf crate (avoids tf→gf cycle)

    #[test]
    fn test_declare_custom_type() {
        struct CustomType;
        TfType::declare::<CustomType>("CustomType");

        let t = TfType::find::<CustomType>();
        assert!(!t.is_unknown());
        assert_eq!(t.type_name(), "CustomType");
    }

    #[test]
    fn test_declare_with_bases() {
        struct BaseType;
        struct DerivedType;

        TfType::declare::<BaseType>("BaseType");
        TfType::declare_with_bases::<DerivedType>("DerivedType", &[TypeId::of::<BaseType>()]);

        let base = TfType::find::<BaseType>();
        let derived = TfType::find::<DerivedType>();

        assert!(derived.is_a(base));
        assert!(derived.is_a(derived));
        assert!(!base.is_a(derived));
    }

    #[test]
    fn test_derived_types() {
        struct BaseForDerived;
        struct Derived1;
        struct Derived2;

        TfType::declare::<BaseForDerived>("BaseForDerived");
        TfType::declare_with_bases::<Derived1>("Derived1", &[TypeId::of::<BaseForDerived>()]);
        TfType::declare_with_bases::<Derived2>("Derived2", &[TypeId::of::<BaseForDerived>()]);

        let base = TfType::find::<BaseForDerived>();
        let directly_derived = base.get_directly_derived_types();

        assert!(directly_derived.len() >= 2);
        assert!(directly_derived.contains(&TfType::find::<Derived1>()));
        assert!(directly_derived.contains(&TfType::find::<Derived2>()));
    }

    #[test]
    fn test_all_ancestor_types() {
        struct GrandBase;
        struct ParentBase;
        struct ChildType;

        TfType::declare::<GrandBase>("GrandBase");
        TfType::declare_with_bases::<ParentBase>("ParentBase", &[TypeId::of::<GrandBase>()]);
        TfType::declare_with_bases::<ChildType>("ChildType", &[TypeId::of::<ParentBase>()]);

        let child = TfType::find::<ChildType>();
        let ancestors = child.get_all_ancestor_types();

        assert!(ancestors.len() >= 3);
        assert_eq!(ancestors[0], child);
    }

    /// Verify C3 MRO for the classic diamond:
    ///
    ///        O
    ///       / \
    ///      A   B
    ///       \ /
    ///        C
    ///
    /// Python gives C -> A -> B -> O; C++ TfType must give the same.
    #[test]
    fn test_c3_mro_diamond() {
        // Use unique marker types to avoid cross-test pollution.
        struct MroO;
        struct MroA;
        struct MroB;
        struct MroC;

        TfType::declare::<MroO>("MroO");
        TfType::declare_with_bases::<MroA>("MroA", &[TypeId::of::<MroO>()]);
        TfType::declare_with_bases::<MroB>("MroB", &[TypeId::of::<MroO>()]);
        // C inherits A first, then B (declaration order matters for C3).
        TfType::declare_with_bases::<MroC>("MroC", &[TypeId::of::<MroA>(), TypeId::of::<MroB>()]);

        let c = TfType::find::<MroC>();
        let a = TfType::find::<MroA>();
        let b = TfType::find::<MroB>();
        let o = TfType::find::<MroO>();

        let root = TfType::root();
        let mro = c.get_all_ancestor_types();
        // C3 result: [C, A, B, O, root] — O now implicitly derives from root.
        assert_eq!(mro.len(), 5, "expected 5 ancestors, got {:?}", mro);
        assert_eq!(mro[0], c);
        assert_eq!(mro[1], a);
        assert_eq!(mro[2], b);
        assert_eq!(mro[3], o);
        assert_eq!(mro[4], root);
    }

    /// Unknown type must return empty Vec, not panic.
    #[test]
    fn test_ancestor_unknown_type() {
        let unknown = TfType::unknown();
        let ancestors = unknown.get_all_ancestor_types();
        assert!(ancestors.is_empty());
    }

    #[test]
    fn test_is_a_type() {
        struct IsABase;
        struct IsADerived;
        struct IsAUnrelated;

        TfType::declare::<IsABase>("IsABase");
        TfType::declare_with_bases::<IsADerived>("IsADerived", &[TypeId::of::<IsABase>()]);
        TfType::declare::<IsAUnrelated>("IsAUnrelated");

        let derived = TfType::find::<IsADerived>();

        // derived is_a IsADerived (self)
        assert!(derived.is_a_type::<IsADerived>());
        // derived is_a IsABase (parent)
        assert!(derived.is_a_type::<IsABase>());
        // derived is NOT is_a unrelated type
        assert!(!derived.is_a_type::<IsAUnrelated>());
        // base is NOT is_a derived
        let base = TfType::find::<IsABase>();
        assert!(!base.is_a_type::<IsADerived>());
    }

    #[test]
    fn test_is_a_unknown() {
        let unknown = TfType::unknown();
        let known = TfType::find::<i32>();

        assert!(!unknown.is_a(known));
        assert!(!known.is_a(unknown));
        assert!(!unknown.is_a(unknown));
    }

    #[test]
    fn test_equality() {
        let t1 = TfType::find::<i32>();
        let t2 = TfType::find::<i32>();
        let t3 = TfType::find::<f64>();

        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_alias() {
        struct AliasedType;
        TfType::declare::<AliasedType>("AliasedType");

        let t = TfType::find::<AliasedType>();
        t.add_alias(TfType::root(), "MyAlias");

        let aliases = t.aliases();
        assert!(aliases.contains(&"MyAlias".to_string()));
    }

    #[test]
    fn test_enum_type() {
        #[allow(dead_code)]
        #[derive(Clone, Copy)]
        enum TestEnum {
            A,
            B,
        }

        TfType::declare_enum::<TestEnum>("TestEnum");

        let t = TfType::find::<TestEnum>();
        assert!(t.is_enum());
    }

    #[test]
    fn test_sizeof() {
        let int_type = TfType::find::<i32>();
        assert_eq!(int_type.get_sizeof(), 4);

        let double_type = TfType::find::<f64>();
        assert_eq!(double_type.get_sizeof(), 8);
    }

    #[test]
    fn test_is_pod() {
        let int_type = TfType::find::<i32>();
        assert!(int_type.is_plain_old_data_type());

        let string_type = TfType::find::<String>();
        assert!(!string_type.is_plain_old_data_type());
    }

    #[test]
    fn test_factory() {
        struct TestFactory;
        impl FactoryBase for TestFactory {
            fn as_any(&self) -> &dyn Any {
                self
            }
        }

        struct FactoryTestType;
        TfType::declare::<FactoryTestType>("FactoryTestType");

        let t = TfType::find::<FactoryTestType>();
        assert!(!t.has_factory());

        t.set_factory(Arc::new(TestFactory));
        assert!(t.has_factory());

        let factory = t.get_factory();
        assert!(factory.is_some());
    }

    #[test]
    fn test_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TfType::find::<i32>());
        set.insert(TfType::find::<f64>());
        set.insert(TfType::find::<i32>());

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_display() {
        let t = TfType::find::<i32>();
        assert_eq!(format!("{}", t), "int32");

        let unknown = TfType::unknown();
        assert_eq!(format!("{}", unknown), "unknown");
    }

    #[test]
    fn test_bool_conversion() {
        let known: bool = TfType::find::<i32>().into();
        let unknown: bool = TfType::unknown().into();

        assert!(known);
        assert!(!unknown);
    }

    #[test]
    fn test_get_canonical_type() {
        let t = TfType::find::<i32>();
        let canonical = t.get_canonical_type();
        assert_eq!(t, canonical);
    }

    #[test]
    fn test_get_n_base_types() {
        struct BaseN;
        struct DerivedN;

        TfType::declare::<BaseN>("BaseN");
        TfType::declare_with_bases::<DerivedN>("DerivedN", &[TypeId::of::<BaseN>()]);

        let derived = TfType::find::<DerivedN>();

        // Get first base into a 1-element slice
        let mut out = [TfType::unknown()];
        let total = derived.get_n_base_types(&mut out);

        assert_eq!(total, 1);
        assert_eq!(out[0], TfType::find::<BaseN>());
    }

    #[test]
    fn test_get_n_base_types_empty() {
        struct NoBasesType;
        TfType::declare::<NoBasesType>("NoBasesType");

        let t = TfType::find::<NoBasesType>();
        let mut out = [TfType::unknown()];
        let total = t.get_n_base_types(&mut out);

        // Types declared without explicit bases implicitly derive from root.
        assert_eq!(total, 1);
        assert_eq!(out[0], TfType::root());
    }

    #[test]
    fn test_get_aliases_for_derived() {
        struct BaseForAliases;
        struct DerivedWithAliases;

        TfType::declare::<BaseForAliases>("BaseForAliases");
        TfType::declare_with_bases::<DerivedWithAliases>(
            "DerivedWithAliases",
            &[TypeId::of::<BaseForAliases>()],
        );

        let base = TfType::find::<BaseForAliases>();
        let derived = TfType::find::<DerivedWithAliases>();

        // Add alias
        derived.add_alias(base, "DerivedAlias");

        // Get aliases for derived under base
        let aliases = base.get_aliases_for_derived(derived);
        assert!(aliases.contains(&"DerivedAlias".to_string()));
    }
}
