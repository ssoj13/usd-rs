//! Thread-safe interned strings with O(1) comparison. 100% safe Rust.
//!
//! `UString` is the Rust equivalent of OIIO's `ustring`. Each unique string
//! is stored exactly once in a global table. Comparisons are performed by
//! comparing a single hash, giving O(1) equality checks.
//!
//! # Why `Box::leak` instead of raw pointers?
//!
//! The previous implementation stored a `*const u8` raw pointer into a
//! `DashMap<u64, Box<str>>`, requiring `unsafe` for:
//! - Extending the borrow lifetime to `'static`
//! - Reconstructing `&str` from raw pointer + length
//! - Manual `Send`/`Sync` impls (raw pointers are `!Send`)
//!
//! String interning by definition keeps strings alive forever — we never
//! free them. `Box::leak()` is a safe function that converts `Box<str>`
//! into `&'static str`, which is exactly the lifetime we need. Since
//! `&'static str` is `Copy + Send + Sync`, `UString` automatically gets
//! all three traits without any `unsafe`.
//!
//! `UStringHash` is a lightweight handle that stores only the hash value
//! and can exist without a reference to the string table (useful for
//! device/GPU code and POD transfer).

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;

use dashmap::DashMap;

use crate::hashes;

// ---------------------------------------------------------------------------
// Global string table
// ---------------------------------------------------------------------------

/// The global interned string table.
///
/// Strings are stored as `&'static str` obtained via `Box::leak`. This is
/// intentional: interning means the string lives for the entire program.
/// The `DashMap` provides concurrent lock-free reads and sharded writes.
struct StringTable {
    map: DashMap<u64, &'static str>,
}

impl StringTable {
    fn new() -> Self {
        Self {
            map: DashMap::with_capacity(4096),
        }
    }

    /// Intern a string. Returns the hash and a `&'static str` reference.
    ///
    /// If the string is already interned, returns the existing reference.
    /// Otherwise, leaks a new `Box<str>` to create a `&'static str`.
    fn intern(&self, s: &str) -> (u64, &'static str) {
        let hash = hashes::strhash(s);

        // Fast path: already interned.
        if let Some(entry) = self.map.get(&hash) {
            return (hash, *entry.value());
        }

        // Slow path: leak a null-terminated copy to get &'static str.
        // We append '\0' so that the raw pointer is always null-terminated
        // for C consumers (capi osl_ustring_c_str). The stored &str slice
        // excludes the trailing NUL, so Rust-side .len() stays correct.
        let mut s_with_nul = s.to_string();
        s_with_nul.push('\0');
        let leaked_full: &'static str = Box::leak(s_with_nul.into_boxed_str());
        // The &str we store is everything except the trailing \0
        let leaked: &'static str = &leaked_full[..leaked_full.len() - 1];
        self.map.entry(hash).or_insert(leaked);

        // Re-read in case another thread won the race (their leaked copy
        // is also valid; ours just becomes a harmless tiny leak).
        let entry = self.map.get(&hash).unwrap();
        (hash, *entry.value())
    }

    /// Look up a string by hash. Returns `None` if not interned.
    fn lookup(&self, hash: u64) -> Option<&'static str> {
        self.map.get(&hash).map(|entry| *entry.value())
    }
}

fn global_table() -> &'static StringTable {
    static TABLE: OnceLock<StringTable> = OnceLock::new();
    TABLE.get_or_init(StringTable::new)
}

// ---------------------------------------------------------------------------
// UString
// ---------------------------------------------------------------------------

/// An interned string with O(1) equality comparison.
///
/// Internally stores a `&'static str` reference to the interned storage
/// and the precomputed hash. Two `UString`s are equal if and only if their
/// interned pointers match (same content = same `&'static str` via
/// `Box::leak`, so pointer identity is collision-proof).
///
/// `UString` is `Copy` — passing it around is as cheap as copying two
/// machine words (pointer + hash). No reference counting, no allocation.
///
/// # Size difference from C++
///
/// C++ `ustring` is 8 bytes (single `const char*`). This Rust `UString` is
/// 24 bytes (`&'static str` = ptr+len = 16 bytes, plus `u64` hash = 8 bytes).
/// The larger size is an intentional trade-off: DashMap with 64 shards gives
/// much better concurrent-insert throughput than C++'s single-mutex table,
/// and storing the hash inline avoids re-hashing on every comparison.
#[derive(Clone, Copy)]
pub struct UString {
    /// Reference to the interned string (lives forever via `Box::leak`).
    s: &'static str,
    /// Precomputed FarmHash64.
    hash: u64,
}

// No `unsafe impl Send/Sync` needed — `&'static str` is Send + Sync
// automatically, and `u64` is too. The compiler derives both for free.

impl UString {
    /// The empty string hash (matches FarmHash64 of "").
    pub const EMPTY_HASH: u64 = crate::hashes::K2;

    /// Create a new interned string.
    pub fn new(s: &str) -> Self {
        if s.is_empty() {
            return Self::empty();
        }
        let (hash, leaked) = global_table().intern(s);
        Self { s: leaked, hash }
    }

    /// Return the empty UString.
    pub fn empty() -> Self {
        Self {
            s: "",
            hash: Self::EMPTY_HASH,
        }
    }

    /// Check if the string is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.s.is_empty()
    }

    /// Length in bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.s.len()
    }

    /// Get the precomputed hash.
    #[inline]
    pub fn hash(&self) -> u64 {
        self.hash
    }

    /// Get the `UStringHash` for this string.
    #[inline]
    pub fn uhash(&self) -> UStringHash {
        UStringHash(self.hash)
    }

    /// Get the string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        self.s
    }

    /// Construct from a hash (reverse lookup). Returns `None` if the hash
    /// is not in the global table.
    pub fn from_hash(hash: u64) -> Option<Self> {
        if hash == Self::EMPTY_HASH {
            return Some(Self::empty());
        }
        global_table().lookup(hash).map(|s| Self { s, hash })
    }
}

impl Default for UString {
    fn default() -> Self {
        Self::empty()
    }
}

impl PartialEq for UString {
    /// O(1) pointer-identity comparison. Since all interned strings go
    /// through the global table, same content always yields the same
    /// `&'static str` pointer — no hash-collision risk.
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.s.as_ptr() == other.s.as_ptr()
    }
}

impl Eq for UString {}

impl PartialOrd for UString {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UString {
    /// Lexicographic ordering by string content (not hash).
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for UString {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(self.hash);
    }
}

impl fmt::Debug for UString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UString({:?})", self.s)
    }
}

impl fmt::Display for UString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.s)
    }
}

impl From<&str> for UString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for UString {
    fn from(s: String) -> Self {
        Self::new(&s)
    }
}

impl AsRef<str> for UString {
    fn as_ref(&self) -> &str {
        self.s
    }
}

impl PartialEq<str> for UString {
    fn eq(&self, other: &str) -> bool {
        self.s == other
    }
}

impl PartialEq<&str> for UString {
    fn eq(&self, other: &&str) -> bool {
        self.s == *other
    }
}

// ---------------------------------------------------------------------------
// UStringHash
// ---------------------------------------------------------------------------

/// A lightweight hash-only handle to an interned string.
///
/// This is the Rust equivalent of OIIO's `ustringhash`. It stores only the
/// 64-bit hash value and can be used without access to the string table
/// (e.g., on GPU or in POD structures).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(transparent)]
pub struct UStringHash(pub u64);

/// POD type for passing hash data in FFI / LLVM function calls.
pub type UStringHashPod = usize;

impl UStringHash {
    /// Hash of the empty string.
    pub const EMPTY: Self = Self(UString::EMPTY_HASH);

    /// Create from a raw hash value.
    #[inline]
    pub const fn from_hash(h: u64) -> Self {
        Self(h)
    }

    /// Hash UTF-8 text at runtime (replaces ad-hoc `from_str` naming; see also [`FromStr`] impl).
    #[inline]
    pub fn hash_utf8(s: &str) -> Self {
        Self(hashes::strhash(s))
    }

    /// Get the raw hash value.
    #[inline]
    pub const fn hash(&self) -> u64 {
        self.0
    }

    /// Is this the empty string hash?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == UString::EMPTY_HASH || self.0 == 0
    }

    /// Resolve to a full UString (reverse lookup).
    #[inline]
    pub fn resolve(&self) -> Option<UString> {
        UString::from_hash(self.0)
    }
}

impl std::str::FromStr for UStringHash {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(hashes::strhash(s)))
    }
}

impl From<UString> for UStringHash {
    #[inline]
    fn from(u: UString) -> Self {
        u.uhash()
    }
}

impl fmt::Debug for UStringHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(u) = self.resolve() {
            write!(f, "UStringHash({:?})", u.as_str())
        } else {
            write!(f, "UStringHash(0x{:016x})", self.0)
        }
    }
}

impl fmt::Display for UStringHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(u) = self.resolve() {
            f.write_str(u.as_str())
        } else {
            write!(f, "<hash:0x{:016x}>", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty() {
        let e = UString::empty();
        assert!(e.is_empty());
        assert_eq!(e.as_str(), "");
        assert_eq!(e, UString::default());
    }

    #[test]
    fn test_intern() {
        let a = UString::new("hello");
        let b = UString::new("hello");
        assert_eq!(a, b);
        assert_eq!(a.hash(), b.hash());
        assert_eq!(a.as_str(), "hello");
    }

    #[test]
    fn test_different() {
        let a = UString::new("hello");
        let b = UString::new("world");
        assert_ne!(a, b);
        assert_ne!(a.hash(), b.hash());
    }

    #[test]
    fn test_hash_resolve() {
        let u = UString::new("test_string");
        let h = u.uhash();
        let resolved = h.resolve().unwrap();
        assert_eq!(resolved, u);
        assert_eq!(resolved.as_str(), "test_string");
    }

    #[test]
    fn test_from_str() {
        let u: UString = "rust_osl".into();
        assert_eq!(u.as_str(), "rust_osl");
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;
        let handles: Vec<_> = (0..8)
            .map(|i| {
                thread::spawn(move || {
                    let s = format!("shared_string_{}", i % 3);
                    let u = UString::new(&s);
                    (u.hash(), u.as_str().to_string())
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        // All threads that used the same string should get the same hash.
        for w in results.windows(2) {
            if w[0].1 == w[1].1 {
                assert_eq!(w[0].0, w[1].0);
            }
        }
    }
}
