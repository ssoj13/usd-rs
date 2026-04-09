//! Asset resolver context.
//!
//! Provides additional data to the resolver for use during resolution.
//! Contexts allow clients to customize resolution behavior per-thread.

use std::any::{Any, TypeId};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Marker trait for types that can be used as context objects.
///
/// A context object must be:
/// - `Clone` - for copying contexts
/// - `PartialEq` - for comparing contexts
/// - `Hash` - for hashing contexts
/// - `Send + Sync` - for thread-safety
/// - `'static` - for type erasure
///
/// # Examples
///
/// ```
/// use usd_ar::ContextObject;
///
/// #[derive(Clone, PartialEq, Eq, Hash, Debug)]
/// struct MyContext {
///     search_paths: Vec<String>,
/// }
///
/// impl ContextObject for MyContext {}
/// ```
pub trait ContextObject: Any + Clone + PartialEq + Hash + Send + Sync + 'static {
    /// Returns a debug string representation of this context object.
    ///
    /// The default implementation returns the type name.
    fn debug_string(&self) -> String {
        std::any::type_name::<Self>().to_string()
    }
}

/// Asset resolver context that holds context objects.
///
/// An `ResolverContext` may hold multiple context objects of different types.
/// Each type can only appear once in the context. Context objects provide
/// additional data to the resolver during resolution.
///
/// # Thread Safety
///
/// Context binding is thread-specific. When a context is bound in a thread,
/// it only affects resolution calls in that thread.
///
/// # Examples
///
/// ```
/// use usd_ar::ResolverContext;
///
/// let ctx = ResolverContext::new();
/// assert!(ctx.is_empty());
/// ```
#[derive(Clone, Default)]
pub struct ResolverContext {
    /// Type-erased context objects keyed by TypeId.
    objects: HashMap<TypeId, Arc<dyn ContextObjectHolder>>,
}

impl ResolverContext {
    /// Creates a new empty resolver context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolverContext;
    ///
    /// let ctx = ResolverContext::new();
    /// assert!(ctx.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
        }
    }

    /// Creates a resolver context containing the given context object.
    ///
    /// # Arguments
    ///
    /// * `obj` - The context object to add
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverContext, DefaultResolverContext};
    ///
    /// let search_ctx = DefaultResolverContext::new(vec!["/path/to/assets".into()]);
    /// let ctx = ResolverContext::with_object(search_ctx);
    ///
    /// assert!(!ctx.is_empty());
    /// ```
    pub fn with_object<T: ContextObject>(obj: T) -> Self {
        let mut ctx = Self::new();
        ctx.add(obj);
        ctx
    }

    /// Creates a resolver context from a vector of resolver contexts.
    ///
    /// All context objects from each context in `contexts` will be added
    /// to the constructed `ResolverContext`. If a context object with the
    /// same type is encountered multiple times, the first one encountered
    /// will be kept and subsequent ones will be ignored.
    ///
    /// # Arguments
    ///
    /// * `contexts` - Vector of resolver contexts to combine
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverContext, DefaultResolverContext};
    ///
    /// let ctx1 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path1".into()]));
    /// let ctx2 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path2".into()]));
    /// let combined = ResolverContext::from_contexts(vec![ctx1, ctx2]);
    /// ```
    pub fn from_contexts(contexts: Vec<ResolverContext>) -> Self {
        let mut result = Self::new();
        for ctx in contexts {
            result.merge(&ctx);
        }
        result
    }

    /// Returns `true` if this context contains no context objects.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolverContext;
    ///
    /// let ctx = ResolverContext::new();
    /// assert!(ctx.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Returns the number of context objects in this context.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Adds a context object to this context.
    ///
    /// If a context object of the same type already exists, it is replaced.
    ///
    /// # Arguments
    ///
    /// * `obj` - The context object to add
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverContext, DefaultResolverContext};
    ///
    /// let mut ctx = ResolverContext::new();
    /// let search_ctx = DefaultResolverContext::new(vec!["/path".into()]);
    /// ctx.add(search_ctx);
    ///
    /// assert!(!ctx.is_empty());
    /// ```
    pub fn add<T: ContextObject>(&mut self, obj: T) {
        let holder: Arc<dyn ContextObjectHolder> = Arc::new(TypedHolder { value: obj });
        self.objects.insert(TypeId::of::<T>(), holder);
    }

    /// Returns a reference to the context object of the given type.
    ///
    /// Returns `None` if no context object of that type exists.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverContext, DefaultResolverContext};
    ///
    /// let search_ctx = DefaultResolverContext::new(vec!["/path".into()]);
    /// let ctx = ResolverContext::with_object(search_ctx.clone());
    ///
    /// let retrieved: Option<&DefaultResolverContext> = ctx.get();
    /// assert!(retrieved.is_some());
    /// assert_eq!(retrieved.unwrap(), &search_ctx);
    /// ```
    pub fn get<T: ContextObject>(&self) -> Option<&T> {
        self.objects.get(&TypeId::of::<T>()).map(|holder| {
            holder
                .as_any()
                .downcast_ref::<TypedHolder<T>>()
                .map(|h| &h.value)
                .expect("Type mismatch in context object storage")
        })
    }

    /// Returns `true` if this context contains a context object of the given type.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::{ResolverContext, DefaultResolverContext};
    ///
    /// let ctx = ResolverContext::with_object(DefaultResolverContext::new(vec![]));
    /// assert!(ctx.contains::<DefaultResolverContext>());
    /// ```
    pub fn contains<T: ContextObject>(&self) -> bool {
        self.objects.contains_key(&TypeId::of::<T>())
    }

    /// Removes the context object of the given type.
    ///
    /// Returns `true` if an object was removed, `false` otherwise.
    pub fn remove<T: ContextObject>(&mut self) -> bool {
        self.objects.remove(&TypeId::of::<T>()).is_some()
    }

    /// Merges another context into this one.
    ///
    /// Objects from `other` that don't exist in `self` are added.
    /// Objects that already exist in `self` are kept (not overwritten).
    ///
    /// # Arguments
    ///
    /// * `other` - The context to merge from
    pub fn merge(&mut self, other: &ResolverContext) {
        for (type_id, holder) in &other.objects {
            self.objects
                .entry(*type_id)
                .or_insert_with(|| holder.clone_holder());
        }
    }

    /// Returns a debug string representation of this context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::ResolverContext;
    ///
    /// let ctx = ResolverContext::new();
    /// let debug = ctx.debug_string();
    /// assert!(debug.contains("empty") || debug.contains("ResolverContext"));
    /// ```
    pub fn debug_string(&self) -> String {
        if self.is_empty() {
            return "ResolverContext(empty)".to_string();
        }

        let parts: Vec<String> = self.objects.values().map(|h| h.debug_string()).collect();

        format!("ResolverContext({})", parts.join(", "))
    }
}

impl fmt::Debug for ResolverContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolverContext")
            .field("objects", &self.objects.len())
            .finish()
    }
}

impl fmt::Display for ResolverContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.debug_string())
    }
}

impl PartialEq for ResolverContext {
    fn eq(&self, other: &Self) -> bool {
        if self.objects.len() != other.objects.len() {
            return false;
        }

        for (type_id, holder) in &self.objects {
            match other.objects.get(type_id) {
                Some(other_holder) => {
                    if !holder.eq_holder(other_holder.as_ref()) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }
}

impl Eq for ResolverContext {}

impl PartialOrd for ResolverContext {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ResolverContext {
    /// Total order consistent with [`PartialEq`]: same key set (by [`TypeId`]), then holder
    /// `hash_value`, then `eq_holder` / `debug_string` when hashes collide.
    fn cmp(&self, other: &Self) -> Ordering {
        fn type_id_total_cmp(a: &TypeId, b: &TypeId) -> Ordering {
            if a == b {
                return Ordering::Equal;
            }
            // `TypeId` has no `Ord`; `Debug` is stable enough for a canonical tie-break.
            format!("{a:?}").cmp(&format!("{b:?}"))
        }

        match self.objects.len().cmp(&other.objects.len()) {
            Ordering::Equal => {}
            o => return o,
        }

        let mut a_keys: Vec<TypeId> = self.objects.keys().copied().collect();
        let mut b_keys: Vec<TypeId> = other.objects.keys().copied().collect();
        a_keys.sort_by(type_id_total_cmp);
        b_keys.sort_by(type_id_total_cmp);

        for i in 0..a_keys.len() {
            let ka = a_keys[i];
            let kb = b_keys[i];
            if ka != kb {
                return type_id_total_cmp(&ka, &kb);
            }
            let ha = &self.objects[&ka];
            let hb = &other.objects[&ka];
            match ha.hash_value().cmp(&hb.hash_value()) {
                Ordering::Equal => {
                    if ha.eq_holder(hb.as_ref()) {
                        continue;
                    }
                    return ha.debug_string().cmp(&hb.debug_string());
                }
                o => return o,
            }
        }
        Ordering::Equal
    }
}

impl Hash for ResolverContext {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash the number of objects
        self.objects.len().hash(state);

        // Hash each object (in a deterministic order by TypeId)
        let mut type_ids: Vec<_> = self.objects.keys().collect();
        type_ids.sort();

        for type_id in type_ids {
            type_id.hash(state);
            if let Some(holder) = self.objects.get(type_id) {
                holder.hash_value().hash(state);
            }
        }
    }
}

// Internal trait for type-erased context object storage
trait ContextObjectHolder: Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn clone_holder(&self) -> Arc<dyn ContextObjectHolder>;
    fn eq_holder(&self, other: &dyn ContextObjectHolder) -> bool;
    fn hash_value(&self) -> u64;
    fn debug_string(&self) -> String;
}

struct TypedHolder<T: ContextObject> {
    value: T,
}

impl<T: ContextObject> ContextObjectHolder for TypedHolder<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn clone_holder(&self) -> Arc<dyn ContextObjectHolder> {
        Arc::new(TypedHolder {
            value: self.value.clone(),
        })
    }

    fn eq_holder(&self, other: &dyn ContextObjectHolder) -> bool {
        other
            .as_any()
            .downcast_ref::<TypedHolder<T>>()
            .map(|o| self.value == o.value)
            .unwrap_or(false)
    }

    fn hash_value(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        self.value.hash(&mut hasher);
        hasher.finish()
    }

    fn debug_string(&self) -> String {
        self.value.debug_string()
    }
}

/// Default resolver context with search paths.
///
/// This is the context object used by the default asset resolver.
/// It contains a list of search paths that the resolver uses to
/// find assets.
///
/// # Examples
///
/// ```
/// use usd_ar::DefaultResolverContext;
///
/// let ctx = DefaultResolverContext::new(vec![
///     "/assets/characters".into(),
///     "/assets/props".into(),
/// ]);
///
/// assert_eq!(ctx.search_paths().len(), 2);
/// ```
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct DefaultResolverContext {
    /// Search paths for resolving relative asset paths.
    search_paths: Vec<String>,
}

impl DefaultResolverContext {
    /// Creates a new default resolver context with the given search paths.
    ///
    /// Elements in `search_paths` should be absolute paths. If they are not,
    /// they will be anchored to the current working directory (matches C++ behavior).
    ///
    /// # Arguments
    ///
    /// * `search_paths` - List of directories to search for assets
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_ar::DefaultResolverContext;
    ///
    /// // Note: paths are canonicalized, so results may vary by platform
    /// let ctx = DefaultResolverContext::new(vec!["assets".into()]);
    /// println!("Search paths: {:?}", ctx.search_paths());
    /// ```
    pub fn new(search_paths: Vec<String>) -> Self {
        use std::path::PathBuf;
        let anchored_paths: Vec<String> = search_paths
            .into_iter()
            .map(|path| {
                let path_buf = PathBuf::from(&path);
                // If path is absolute, use as-is
                if path_buf.is_absolute() {
                    path
                } else {
                    // Anchor relative paths to current working directory
                    // Match C++ behavior: TfAbsPath or TfRealPath anchoring
                    if let Ok(cwd) = std::env::current_dir() {
                        let joined = cwd.join(&path_buf);
                        joined
                            .canonicalize()
                            .unwrap_or(joined)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        // If CWD is unavailable, return path as-is
                        path
                    }
                }
            })
            .collect();
        Self {
            search_paths: anchored_paths,
        }
    }

    /// Creates an empty default resolver context.
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::DefaultResolverContext;
    ///
    /// let ctx = DefaultResolverContext::empty();
    /// assert!(ctx.search_paths().is_empty());
    /// ```
    pub fn empty() -> Self {
        Self {
            search_paths: Vec::new(),
        }
    }

    /// Returns the search paths in this context.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use usd_ar::DefaultResolverContext;
    ///
    /// // Note: paths are canonicalized, so results may vary by platform
    /// let ctx = DefaultResolverContext::new(vec!["path1".into(), "path2".into()]);
    /// println!("Paths: {:?}", ctx.search_paths());
    /// ```
    pub fn search_paths(&self) -> &[String] {
        &self.search_paths
    }

    /// Returns a mutable reference to the search paths.
    pub fn search_paths_mut(&mut self) -> &mut Vec<String> {
        &mut self.search_paths
    }

    /// Adds a search path to this context.
    ///
    /// # Arguments
    ///
    /// * `path` - The search path to add
    ///
    /// # Examples
    ///
    /// ```
    /// use usd_ar::DefaultResolverContext;
    ///
    /// let mut ctx = DefaultResolverContext::empty();
    /// ctx.add_search_path("/new/path");
    /// assert_eq!(ctx.search_paths(), &["/new/path"]);
    /// ```
    pub fn add_search_path(&mut self, path: impl Into<String>) {
        self.search_paths.push(path.into());
    }
}

impl Default for DefaultResolverContext {
    fn default() -> Self {
        Self::empty()
    }
}

impl ContextObject for DefaultResolverContext {
    fn debug_string(&self) -> String {
        if self.search_paths.is_empty() {
            "DefaultResolverContext(no search paths)".to_string()
        } else {
            format!("DefaultResolverContext([{}])", self.search_paths.join(", "))
        }
    }
}

impl fmt::Display for DefaultResolverContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.debug_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_context_new() {
        let ctx = ResolverContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);
    }

    #[test]
    fn test_resolver_context_with_object() {
        let search_ctx = DefaultResolverContext::new(vec!["/path".into()]);
        let ctx = ResolverContext::with_object(search_ctx);
        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 1);
    }

    #[test]
    fn test_resolver_context_add_get() {
        let mut ctx = ResolverContext::new();
        let search_ctx = DefaultResolverContext::new(vec!["/path".into()]);
        ctx.add(search_ctx.clone());

        let retrieved: Option<&DefaultResolverContext> = ctx.get();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), &search_ctx);
    }

    #[test]
    fn test_resolver_context_contains() {
        let ctx = ResolverContext::with_object(DefaultResolverContext::empty());
        assert!(ctx.contains::<DefaultResolverContext>());
    }

    #[test]
    fn test_resolver_context_remove() {
        let mut ctx = ResolverContext::with_object(DefaultResolverContext::empty());
        assert!(ctx.remove::<DefaultResolverContext>());
        assert!(ctx.is_empty());
        assert!(!ctx.remove::<DefaultResolverContext>());
    }

    #[test]
    fn test_resolver_context_merge() {
        let temp_dir = std::env::temp_dir();
        let path1 = temp_dir.join("merge_path1").to_string_lossy().to_string();
        let mut ctx1 = ResolverContext::with_object(DefaultResolverContext::new(vec![path1]));

        // Create a second context type for testing
        #[derive(Clone, PartialEq, Eq, Hash)]
        struct OtherContext(i32);
        impl ContextObject for OtherContext {}

        let mut ctx2 = ResolverContext::new();
        ctx2.add(OtherContext(42));
        let temp_dir = std::env::temp_dir();
        let path2 = temp_dir.join("merge_path2").to_string_lossy().to_string();
        ctx2.add(DefaultResolverContext::new(vec![path2]));

        ctx1.merge(&ctx2);

        // Should have both types
        assert_eq!(ctx1.len(), 2);

        // Original DefaultResolverContext should be preserved (not overwritten)
        let search: &DefaultResolverContext = ctx1.get().unwrap();
        assert!(search.search_paths()[0].contains("merge_path1"));

        // OtherContext should be added
        let other: &OtherContext = ctx1.get().unwrap();
        assert_eq!(other.0, 42);
    }

    #[test]
    fn test_resolver_context_equality() {
        let ctx1 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path".into()]));
        let ctx2 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path".into()]));
        let ctx3 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/other".into()]));

        assert_eq!(ctx1, ctx2);
        assert_ne!(ctx1, ctx3);
    }

    #[test]
    fn test_resolver_context_hash() {
        use std::collections::hash_map::DefaultHasher;

        let ctx1 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path".into()]));
        let ctx2 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path".into()]));

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        ctx1.hash(&mut h1);
        ctx2.hash(&mut h2);

        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn test_resolver_context_clone() {
        let ctx1 = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path".into()]));
        let ctx2 = ctx1.clone();

        assert_eq!(ctx1, ctx2);
    }

    #[test]
    fn test_resolver_context_debug_string() {
        let ctx = ResolverContext::new();
        assert!(ctx.debug_string().contains("empty"));

        let ctx = ResolverContext::with_object(DefaultResolverContext::new(vec!["/path".into()]));
        let debug = ctx.debug_string();
        assert!(debug.contains("DefaultResolverContext"));
    }

    #[test]
    fn test_default_resolver_context_new() {
        // Use platform-specific temp paths that exist
        let temp_dir = std::env::temp_dir();
        let path1 = temp_dir.join("test_path1").to_string_lossy().to_string();
        let path2 = temp_dir.join("test_path2").to_string_lossy().to_string();

        let ctx = DefaultResolverContext::new(vec![path1.clone(), path2.clone()]);
        let paths = ctx.search_paths();
        assert_eq!(paths.len(), 2);
        // Paths may be normalized but should contain our test directories
        assert!(paths[0].contains("test_path1"));
        assert!(paths[1].contains("test_path2"));
    }

    #[test]
    fn test_default_resolver_context_empty() {
        let ctx = DefaultResolverContext::empty();
        assert!(ctx.search_paths().is_empty());
    }

    #[test]
    fn test_default_resolver_context_add_search_path() {
        let mut ctx = DefaultResolverContext::empty();
        ctx.add_search_path("/path1");
        ctx.add_search_path("/path2");
        assert_eq!(ctx.search_paths(), &["/path1", "/path2"]);
    }

    #[test]
    fn test_default_resolver_context_equality() {
        let ctx1 = DefaultResolverContext::new(vec!["/path".into()]);
        let ctx2 = DefaultResolverContext::new(vec!["/path".into()]);
        let ctx3 = DefaultResolverContext::new(vec!["/other".into()]);

        assert_eq!(ctx1, ctx2);
        assert_ne!(ctx1, ctx3);
    }

    #[test]
    fn test_default_resolver_context_debug_string() {
        let ctx = DefaultResolverContext::empty();
        assert!(ctx.debug_string().contains("no search paths"));

        let ctx = DefaultResolverContext::new(vec!["/path".into()]);
        assert!(ctx.debug_string().contains("/path"));
    }

    #[test]
    fn test_default_resolver_context_display() {
        let ctx = DefaultResolverContext::new(vec!["/path".into()]);
        let display = format!("{}", ctx);
        assert!(display.contains("DefaultResolverContext"));
        assert!(display.contains("/path"));
    }
}
