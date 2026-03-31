//! TfToken - Interned string tokens for efficient comparison and hashing.
//!
//! A `Token` is a handle for a registered string that can be compared,
//! assigned, and hashed in constant time. It is useful when a bounded number
//! of strings are used as fixed symbols.
//!
//! # Examples
//!
//! ```
//! use usd_tf::{Token, token};
//!
//! // Create tokens - string is interned on first use
//! let t1 = Token::new("hello");
//! let t2 = Token::new("hello");
//! let t3 = Token::new("world");
//!
//! // Comparison is O(1) - just pointer comparison
//! assert_eq!(t1, t2);
//! assert_ne!(t1, t3);
//!
//! // Use the token! macro for compile-time tokens
//! let t4 = token!("hello");
//! assert_eq!(t1, t4);
//! ```
//!
//! # Thread Safety
//!
//! Token creation involves a global table lookup and is thread-safe.
//! Once created, tokens can be freely shared between threads.
//!
//! # Sharding
//!
//! The token registry uses 128 shards (matching C++ USD's 128 TBB shards)
//! to reduce lock contention under concurrent access. Each shard has
//! its own RwLock, so threads accessing different shards never block
//! each other.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, RwLock};

/// Number of shards in the token registry (power of 2 for fast masking).
/// Matches C++ TfToken's 128-shard TBB hash_map layout.
const NUM_SHARDS: usize = 128;
const SHARD_MASK: usize = NUM_SHARDS - 1;

/// Global sharded token registry for string interning.
static TOKEN_REGISTRY: std::sync::LazyLock<ShardedTokenRegistry> =
    std::sync::LazyLock::new(ShardedTokenRegistry::new);

/// Internal data for a token entry in a shard.
struct TokenData {
    storage: Arc<str>,
    compare_code: u64,
    immortal: bool,
}

/// A single shard of the token registry.
struct Shard {
    /// Map from string content to token data. Uses String keys so
    /// Arc<str> strong_count accurately reflects external references.
    strings: HashMap<String, TokenData>,
}

/// Sharded token registry - 128 independent shards to reduce lock contention.
/// Mirrors the C++ Tf_TokenRegistry which uses 128 TBB hash_map shards.
struct ShardedTokenRegistry {
    shards: [RwLock<Shard>; NUM_SHARDS],
}

impl ShardedTokenRegistry {
    fn new() -> Self {
        Self {
            shards: std::array::from_fn(|_| {
                RwLock::new(Shard {
                    strings: HashMap::new(),
                })
            }),
        }
    }

    /// Pack the first 8 bytes of a string big-endian into a u64 for ordering.
    /// Matches C++ TfToken's _ComputeCompareCode (token.cpp:154-164).
    fn compute_compare_code(s: &str) -> u64 {
        let bytes = s.as_bytes();
        let len = bytes.len().min(8);
        let mut code: u64 = 0;
        for i in 0..len {
            code = (code << 8) | bytes[i] as u64;
        }
        // Left-justify in 64 bits so shorter strings sort before longer ones
        // with the same prefix.
        code <<= 8 * (8 - len);
        code
    }

    /// Compute shard index from string content.
    #[inline]
    fn shard_index(s: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        hasher.finish() as usize & SHARD_MASK
    }

    /// Get or create a token for the given string.
    ///
    /// Uses a fast-path read-lock to avoid contention when the token already
    /// exists, matching the C++ registry's concurrent_hash_map semantics.
    fn get_or_insert(&self, s: &str, immortal: bool) -> Token {
        let idx = Self::shard_index(s);

        // Fast path: read-lock lookup (avoids write contention for existing tokens)
        if !immortal {
            let shard = self.shards[idx].read().expect("Token shard lock poisoned");
            if let Some(data) = shard.strings.get(s) {
                return Token {
                    data: Some(Arc::clone(&data.storage)),
                };
            }
        }

        // Slow path: write-lock for insertion or immortal upgrade
        let mut shard = self.shards[idx].write().expect("Token shard lock poisoned");

        if let Some(data) = shard.strings.get_mut(s) {
            // Upgrade to immortal if requested
            if immortal && !data.immortal {
                data.immortal = true;
            }
            return Token {
                data: Some(Arc::clone(&data.storage)),
            };
        }

        // Create new token entry
        let storage: Arc<str> = Arc::from(s);
        let compare_code = Self::compute_compare_code(s);

        let data = TokenData {
            storage: Arc::clone(&storage),
            compare_code,
            immortal,
        };

        shard.strings.insert(s.to_string(), data);

        Token {
            data: Some(storage),
        }
    }

    /// Find a token if it exists, without creating it.
    fn find(&self, s: &str) -> Option<Token> {
        let idx = Self::shard_index(s);
        let shard = self.shards[idx].read().expect("Token shard lock poisoned");

        shard.strings.get(s).map(|data| Token {
            data: Some(Arc::clone(&data.storage)),
        })
    }

    /// Get the compare code for a string, looking up only the relevant shard.
    fn get_compare_code(&self, s: &str) -> u64 {
        let idx = Self::shard_index(s);
        let shard = self.shards[idx].read().expect("Token shard lock poisoned");

        shard.strings.get(s).map(|d| d.compare_code).unwrap_or(0)
    }

    /// Check if a token is immortal.
    fn is_immortal(&self, s: &str) -> bool {
        let idx = Self::shard_index(s);
        let shard = self.shards[idx].read().expect("Token shard lock poisoned");

        shard.strings.get(s).map(|d| d.immortal).unwrap_or(false)
    }

    /// Sweep all shards, removing tokens where only the registry holds a
    /// reference (Arc strong_count == 1) and the token is not immortal.
    /// Returns the number of tokens removed.
    pub fn gc_sweep(&self) -> usize {
        let mut removed = 0;
        for shard_lock in &self.shards {
            let mut shard = shard_lock.write().expect("Token shard lock poisoned");
            shard.strings.retain(|_key, data| {
                // Keep immortal tokens unconditionally
                if data.immortal {
                    return true;
                }
                // strong_count == 1 means only the registry holds a reference
                if Arc::strong_count(&data.storage) == 1 {
                    removed += 1;
                    false
                } else {
                    true
                }
            });
        }
        removed
    }

    /// Get the number of tokens in a specific shard (for testing).
    #[cfg(test)]
    fn shard_len(&self, idx: usize) -> usize {
        let shard = self.shards[idx].read().expect("Token shard lock poisoned");
        shard.strings.len()
    }

    /// Get total number of tokens across all shards (for testing).
    #[cfg(test)]
    #[allow(dead_code)]
    fn total_len(&self) -> usize {
        self.shards
            .iter()
            .map(|s| s.read().expect("Token shard lock poisoned").strings.len())
            .sum()
    }
}

/// A token representing an interned string.
///
/// Tokens provide O(1) comparison, assignment, and hashing for strings.
/// The string content is stored in a global table, and the token is just
/// a pointer to that storage.
///
/// # Performance
///
/// - Creation: O(n) where n is string length (hash + possible allocation)
/// - Comparison: O(1) (pointer comparison)
/// - Hashing: O(1) (pointer-based hash)
/// - Clone: O(1) (Arc increment)
#[derive(Clone)]
pub struct Token {
    /// The interned string data, or None for empty token
    data: Option<Arc<str>>,
}

impl Token {
    /// Create the empty token.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self { data: None }
    }

    /// Create a token for the given string.
    ///
    /// If the string has been seen before, returns a token pointing to
    /// the existing interned copy. Otherwise, interns the string.
    ///
    /// # Note
    ///
    /// Token creation locks only the relevant shard of the global registry.
    /// Avoid creating tokens in tight inner loops; create them once and reuse.
    #[must_use]
    pub fn new(s: &str) -> Self {
        if s.is_empty() {
            return Self::empty();
        }

        TOKEN_REGISTRY.get_or_insert(s, false)
    }

    /// Create an immortal token for the given string.
    ///
    /// Immortal tokens are never deallocated by GC, which makes them
    /// persistent but means their memory is never reclaimed.
    #[must_use]
    pub fn new_immortal(s: &str) -> Self {
        if s.is_empty() {
            return Self::empty();
        }

        TOKEN_REGISTRY.get_or_insert(s, true)
    }

    /// Find an existing token for the string, if one exists.
    ///
    /// Returns `None` if no token has been created for this string.
    /// This does not create a new token.
    #[must_use]
    pub fn find(s: &str) -> Option<Self> {
        if s.is_empty() {
            return Some(Self::empty());
        }

        TOKEN_REGISTRY.find(s)
    }

    /// Run garbage collection on the token registry.
    ///
    /// Removes tokens that are no longer referenced by any live `Token`
    /// instance (only the registry holds the last Arc reference).
    /// Immortal tokens are never collected.
    ///
    /// Returns the number of tokens removed.
    pub fn gc() -> usize {
        TOKEN_REGISTRY.gc_sweep()
    }

    /// Returns true if this is the empty token.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_none()
    }

    /// Returns true if this is an immortal token.
    ///
    /// Immortal tokens are never deallocated by GC.
    ///
    /// Note: A return of `false` could be instantly stale if another thread
    /// races to immortalize this token. A return of `true` is always valid
    /// since tokens cannot lose immortality.
    #[must_use]
    pub fn is_immortal(&self) -> bool {
        match &self.data {
            Some(s) => TOKEN_REGISTRY.is_immortal(s),
            // The empty token is always considered immortal (matches C++ TfToken behavior)
            None => true,
        }
    }

    /// Returns the string that this token represents.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        match &self.data {
            Some(s) => s,
            None => "",
        }
    }

    /// Returns the text that this token represents.
    ///
    /// Alias for `as_str()` for compatibility with C++ API.
    #[inline]
    #[must_use]
    pub fn get_text(&self) -> &str {
        self.as_str()
    }

    /// Returns the string that this token represents as an owned `String`.
    ///
    /// Equivalent to C++ `GetString()`. For a borrowed `&str`, use `as_str()`.
    #[inline]
    #[must_use]
    pub fn get_string(&self) -> String {
        self.as_str().to_string()
    }

    /// Synonym for `as_str()`. Matches C++ `data()` naming.
    #[inline]
    #[must_use]
    pub fn data(&self) -> &str {
        self.as_str()
    }

    /// Returns the length of the string.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_str().len()
    }

    /// Returns the size of the string (alias for `len()`).
    ///
    /// Matches C++ `TfToken::size()` naming.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.as_str().len()
    }

    /// Returns a hash value for this token.
    ///
    /// The hash is based on the token's storage identity (pointer),
    /// not the string content, making it O(1).
    #[inline]
    #[must_use]
    pub fn hash(&self) -> u64 {
        match &self.data {
            // Cast fat pointer (*const str) to thin pointer (*const ()) first
            Some(s) => Arc::as_ptr(s) as *const () as u64,
            None => 0,
        }
    }

    /// Swaps this token with another.
    #[inline]
    pub fn swap(&mut self, other: &mut Self) {
        std::mem::swap(&mut self.data, &mut other.data);
    }
}

impl Default for Token {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl PartialEq for Token {
    /// Tokens are equal if they point to the same interned string.
    /// This is O(1) pointer comparison.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match (&self.data, &other.data) {
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl Eq for Token {}

impl PartialOrd for Token {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Token {
    /// Lexicographic comparison of the underlying strings.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Fast path: pointer equality
        if self == other {
            return std::cmp::Ordering::Equal;
        }

        let self_code = self
            .data
            .as_ref()
            .map(|s| TOKEN_REGISTRY.get_compare_code(s))
            .unwrap_or(0);
        let other_code = other
            .data
            .as_ref()
            .map(|s| TOKEN_REGISTRY.get_compare_code(s))
            .unwrap_or(0);

        match self_code.cmp(&other_code) {
            std::cmp::Ordering::Equal => {
                // Compare codes are equal, fall back to string comparison
                self.as_str().cmp(other.as_str())
            }
            ord => ord,
        }
    }
}

impl Hash for Token {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash by pointer identity for O(1) hashing
        self.hash().hash(state);
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Token").field(&self.as_str()).finish()
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl AsRef<str> for Token {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Note: C++ TfToken deliberately has no implicit conversion from `const char*`
/// to prevent accidental token creation. In Rust, `From<&str>` requires an
/// explicit `.into()` or `Token::from()` call, which is equivalent to C++'s
/// explicit `TfToken("str")` constructor. This is kept for ergonomic API use
/// with `impl Into<Token>` parameters.
impl From<&str> for Token {
    fn from(s: &str) -> Self {
        Token::new(s)
    }
}

impl From<String> for Token {
    fn from(s: String) -> Self {
        Token::new(&s)
    }
}

impl From<&String> for Token {
    fn from(s: &String) -> Self {
        Token::new(s)
    }
}

impl PartialEq<str> for Token {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Token {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Token {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Lazy<Token>> for Token {
    fn eq(&self, other: &Lazy<Token>) -> bool {
        *self == **other
    }
}

impl PartialEq<Lazy<Token>> for &Token {
    fn eq(&self, other: &Lazy<Token>) -> bool {
        **self == **other
    }
}

impl PartialEq<Token> for &Token {
    fn eq(&self, other: &Token) -> bool {
        **self == *other
    }
}

impl PartialEq<&Token> for Token {
    fn eq(&self, other: &&Token) -> bool {
        *self == **other
    }
}

impl PartialEq<std::sync::LazyLock<Token>> for Token {
    fn eq(&self, other: &std::sync::LazyLock<Token>) -> bool {
        *self == **other
    }
}

impl PartialEq<std::sync::LazyLock<Token>> for &Token {
    fn eq(&self, other: &std::sync::LazyLock<Token>) -> bool {
        **self == **other
    }
}

/// Fast but non-lexicographical comparison for tokens.
///
/// This compares by pointer value rather than string content,
/// providing arbitrary but consistent ordering. Use only when
/// you need fast ordered access (like in a BTreeSet) but don't
/// care about the actual order.
#[derive(Debug, Clone, Copy)]
pub struct TokenFastArbitraryLess;

impl TokenFastArbitraryLess {
    /// Compare two tokens by pointer value.
    #[inline]
    pub fn less(lhs: &Token, rhs: &Token) -> bool {
        lhs.hash() < rhs.hash()
    }
}

/// Convert a vector of strings to a vector of tokens.
pub fn to_token_vec(strings: &[String]) -> Vec<Token> {
    strings.iter().map(|s| Token::new(s)).collect()
}

/// Convert a vector of string slices to a vector of tokens.
pub fn to_token_vec_from_strs(strings: &[&str]) -> Vec<Token> {
    strings.iter().map(|s| Token::new(s)).collect()
}

/// Convert a vector of tokens to a vector of strings.
pub fn to_string_vec(tokens: &[Token]) -> Vec<String> {
    tokens.iter().map(|t| t.as_str().to_string()).collect()
}

/// A token vector type alias.
pub type TokenVec = Vec<Token>;

/// Macro for creating tokens from string literals.
///
/// This provides a convenient syntax for creating tokens.
/// Note that tokens are still created at runtime; this is
/// just syntactic sugar.
///
/// # Examples
///
/// ```
/// use usd_tf::token;
///
/// let t = token!("hello");
/// assert_eq!(t.as_str(), "hello");
/// ```
#[macro_export]
macro_rules! token {
    ($s:expr) => {
        $crate::Token::new($s)
    };
}

/// Type alias for a vector of tokens.
pub type TokenVector = Vec<Token>;

/// Convert a vector of strings to a vector of tokens.
///
/// # Examples
///
/// ```
/// use usd_tf::{Token, to_token_vector};
///
/// let strings = vec!["hello".to_string(), "world".to_string()];
/// let tokens = to_token_vector(&strings);
/// assert_eq!(tokens.len(), 2);
/// assert_eq!(tokens[0].as_str(), "hello");
/// ```
#[must_use]
pub fn to_token_vector(sv: &[String]) -> Vec<Token> {
    sv.iter().map(|s| Token::new(s)).collect()
}

/// Convert a vector of tokens to a vector of strings.
///
/// # Examples
///
/// ```
/// use usd_tf::{Token, to_string_vector};
///
/// let tokens = vec![Token::new("hello"), Token::new("world")];
/// let strings = to_string_vector(&tokens);
/// assert_eq!(strings, vec!["hello", "world"]);
/// ```
#[must_use]
pub fn to_string_vector(tv: &[Token]) -> Vec<String> {
    tv.iter().map(|t| t.as_str().to_string()).collect()
}

/// Convert a slice of string references to a vector of tokens.
#[must_use]
pub fn to_token_vector_from_strs(sv: &[&str]) -> Vec<Token> {
    sv.iter().map(|s| Token::new(s)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_token() {
        let t = Token::empty();
        assert!(t.is_empty());
        assert_eq!(t.as_str(), "");
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn test_token_creation() {
        let t1 = Token::new("hello");
        let t2 = Token::new("hello");
        let t3 = Token::new("world");

        assert!(!t1.is_empty());
        assert_eq!(t1.as_str(), "hello");
        assert_eq!(t1, t2); // Same string = same token
        assert_ne!(t1, t3); // Different string = different token
    }

    #[test]
    fn test_token_interning() {
        let t1 = Token::new("interned");
        let t2 = Token::new("interned");

        // Should point to same storage
        assert!(Arc::ptr_eq(
            t1.data.as_ref().unwrap(),
            t2.data.as_ref().unwrap()
        ));
    }

    #[test]
    fn test_token_hash() {
        let t1 = Token::new("test");
        let t2 = Token::new("test");
        let t3 = Token::new("other");

        assert_eq!(t1.hash(), t2.hash());
        assert_ne!(t1.hash(), t3.hash());
    }

    #[test]
    fn test_token_find() {
        let name = "find_test_unique_12345";

        // Should not exist yet
        assert!(Token::find(name).is_none());

        // Create it
        let t1 = Token::new(name);

        // Now it should exist
        let t2 = Token::find(name);
        assert!(t2.is_some());
        assert_eq!(t1, t2.unwrap());
    }

    #[test]
    fn test_token_ord() {
        let t1 = Token::new("aaa");
        let t2 = Token::new("bbb");
        let t3 = Token::new("aaa");

        assert!(t1 < t2);
        assert!(t2 > t1);
        assert!(t1 <= t3);
        assert!(t1 >= t3);
    }

    #[test]
    fn test_token_from_string() {
        let s = String::from("from_string");
        let t: Token = s.into();
        assert_eq!(t.as_str(), "from_string");
    }

    #[test]
    fn test_token_display() {
        let t = Token::new("display_test");
        assert_eq!(format!("{}", t), "display_test");
    }

    #[test]
    fn test_token_debug() {
        let t = Token::new("debug_test");
        assert!(format!("{:?}", t).contains("debug_test"));
    }

    #[test]
    fn test_token_eq_str() {
        let t = Token::new("compare");
        assert!(t == "compare");
        assert!(t != "other");
    }

    #[test]
    fn test_to_token_vec() {
        let strings = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        let tokens = to_token_vec(&strings);

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].as_str(), "one");
        assert_eq!(tokens[1].as_str(), "two");
        assert_eq!(tokens[2].as_str(), "three");
    }

    #[test]
    fn test_to_string_vec() {
        let tokens = vec![Token::new("a"), Token::new("b"), Token::new("c")];
        let strings = to_string_vec(&tokens);

        assert_eq!(strings, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_token_in_hashmap() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        let t1 = Token::new("key1");
        let t2 = Token::new("key2");

        map.insert(t1.clone(), 1);
        map.insert(t2.clone(), 2);

        assert_eq!(map.get(&t1), Some(&1));
        assert_eq!(map.get(&t2), Some(&2));
        assert_eq!(map.get(&Token::new("key1")), Some(&1));
    }

    #[test]
    fn test_token_thread_safety() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    let name = format!("thread_token_{}", i);
                    let t = Token::new(&name);
                    assert_eq!(t.as_str(), name);
                    t
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join().unwrap();
        }
    }

    #[test]
    fn test_empty_string_token() {
        let t1 = Token::new("");
        let t2 = Token::empty();

        assert!(t1.is_empty());
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_token_swap() {
        let mut t1 = Token::new("first");
        let mut t2 = Token::new("second");

        t1.swap(&mut t2);

        assert_eq!(t1.as_str(), "second");
        assert_eq!(t2.as_str(), "first");
    }

    #[test]
    fn test_token_default() {
        let t: Token = Default::default();
        assert!(t.is_empty());
    }

    // --- GC tests ---

    #[test]
    fn test_gc_collects_unreferenced_tokens() {
        // Use unique names to avoid collision with parallel tests
        let name_a = "gc_test_collect_a_unique_98765";
        let name_b = "gc_test_collect_b_unique_98765";

        // Create and immediately drop tokens
        drop(Token::new(name_a));
        drop(Token::new(name_b));

        // Verify tokens are in registry
        {
            let found_a = Token::find(name_a);
            let found_b = Token::find(name_b);
            assert!(found_a.is_some(), "Token A should exist before GC");
            assert!(found_b.is_some(), "Token B should exist before GC");
        }
        // found_a and found_b are dropped here

        // GC should collect at least some tokens
        let removed = Token::gc();
        assert!(removed >= 1, "Expected at least 1 removed, got {}", removed);

        // Both tokens should be gone since no external references exist
        assert!(Token::find(name_a).is_none(), "Token A should be collected");
        assert!(Token::find(name_b).is_none(), "Token B should be collected");
    }

    #[test]
    fn test_gc_preserves_immortal_tokens() {
        let name = "gc_test_immortal_unique_54321";

        let t_immortal = Token::new_immortal(name);
        assert!(t_immortal.is_immortal());

        // Drop the local reference
        drop(t_immortal);

        // GC should NOT collect it
        Token::gc();

        // Token should still be findable
        let found = Token::find(name);
        assert!(found.is_some(), "Immortal token should survive GC");
        assert!(found.unwrap().is_immortal());
    }

    #[test]
    fn test_gc_preserves_referenced_tokens() {
        let name = "gc_test_referenced_unique_11111";

        let t = Token::new(name);

        // GC should not collect tokens that are still referenced
        Token::gc();

        assert!(Token::find(name).is_some());
        assert_eq!(t.as_str(), name);
    }

    #[test]
    fn test_sharding_distributes_across_shards() {
        // Create many unique tokens and verify they spread across multiple shards
        let prefix = "shard_dist_test_unique_";
        let mut tokens = Vec::new();
        for i in 0..256 {
            tokens.push(Token::new(&format!("{}{}", prefix, i)));
        }

        // Count how many shards have at least one token
        let mut nonempty_shards = 0;
        for i in 0..NUM_SHARDS {
            if TOKEN_REGISTRY.shard_len(i) > 0 {
                nonempty_shards += 1;
            }
        }

        // With 256 tokens across 128 shards, we should have good distribution
        // At minimum, expect more than half the shards to be populated
        assert!(
            nonempty_shards > NUM_SHARDS / 2,
            "Expected tokens distributed across >64 shards, got {}",
            nonempty_shards
        );

        // Keep tokens alive until after the check
        drop(tokens);
    }

    #[test]
    fn test_concurrent_token_creation() {
        use std::sync::Arc as StdArc;
        use std::thread;

        let barrier = StdArc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|thread_id| {
                let barrier = StdArc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let mut tokens = Vec::new();
                    for i in 0..100 {
                        // Some shared names (all threads create these)
                        let shared = format!("concurrent_shared_{}", i);
                        tokens.push(Token::new(&shared));

                        // Some unique names (only this thread creates these)
                        let unique = format!("concurrent_unique_{}_{}", thread_id, i);
                        tokens.push(Token::new(&unique));
                    }
                    tokens
                })
            })
            .collect();

        let mut all_tokens = Vec::new();
        for handle in handles {
            all_tokens.extend(handle.join().unwrap());
        }

        // Verify shared tokens are actually the same interned string
        for i in 0..100 {
            let name = format!("concurrent_shared_{}", i);
            let t1 = Token::new(&name);
            let t2 = Token::new(&name);
            assert_eq!(t1, t2);
            assert!(Arc::ptr_eq(
                t1.data.as_ref().unwrap(),
                t2.data.as_ref().unwrap()
            ));
        }
    }

    #[test]
    fn test_immortalize_existing_token() {
        let name = "immortalize_test_unique_77777";

        // Create as mortal
        let t1 = Token::new(name);
        assert!(!t1.is_immortal());

        // Re-create as immortal
        let t2 = Token::new_immortal(name);
        assert!(t2.is_immortal());

        // Original should now also be immortal (same registry entry)
        assert!(t1.is_immortal());

        // Same interned string
        assert_eq!(t1, t2);
    }

    #[test]
    fn test_empty_token_is_immortal() {
        // Empty token IS immortal, matching C++ behavior
        let empty = Token::empty();
        assert!(empty.is_immortal());

        let empty2 = Token::new_immortal("");
        assert!(empty2.is_immortal());
        assert_eq!(empty, empty2);
    }

    #[test]
    fn test_immortal_survives_gc() {
        let name = "immortal_gc_survive_unique_99988";
        let t = Token::new_immortal(name);
        assert!(t.is_immortal());

        // Drop our reference but it should survive GC
        drop(t);
        Token::gc();

        // Should still be findable after GC
        let found = Token::find(name);
        assert!(found.is_some(), "immortal token must survive GC");
        assert!(found.unwrap().is_immortal());
    }

    // --- C++ parity method tests ---

    #[test]
    fn test_token_data_method() {
        let t = Token::new("data_method_test");
        assert_eq!(t.data(), "data_method_test");
        assert_eq!(t.data(), t.as_str());

        let empty = Token::empty();
        assert_eq!(empty.data(), "");
    }

    #[test]
    fn test_token_get_string_method() {
        let t = Token::new("get_string_test");
        let s = t.get_string();
        assert_eq!(s, "get_string_test");

        let empty = Token::empty();
        assert_eq!(empty.get_string(), "");
    }

    #[test]
    fn test_token_size_method() {
        let t = Token::new("hello");
        assert_eq!(t.size(), 5);
        assert_eq!(t.size(), t.len());

        let empty = Token::empty();
        assert_eq!(empty.size(), 0);
    }

    #[test]
    fn test_read_lock_fast_path() {
        // Verify that looking up an existing token works via the read-lock
        // fast path (no regression from the read-before-write optimization)
        let name = "read_lock_fast_path_test_42";
        let t1 = Token::new(name);
        // Second lookup should use read-lock fast path
        let t2 = Token::new(name);
        assert_eq!(t1, t2);
        assert!(Arc::ptr_eq(
            t1.data.as_ref().unwrap(),
            t2.data.as_ref().unwrap()
        ));
    }
}
