// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 vtr/types.h

use super::array::{Array, ConstArray};

/// Integer type used to index mesh components (vertices, faces, edges).
/// -1 is the "invalid" sentinel, matching C++ Vtr::Index.
pub type Index = i32;

/// A smaller index type for local per-component indices.
/// Matches C++ `unsigned short` LocalIndex.
pub type LocalIndex = u16;

/// Sentinel value for an invalid index.
pub const INDEX_INVALID: Index = -1;

/// Maximum vertex valence (matches `(1 << 16) - 1`).
pub const VALENCE_LIMIT: i32 = 65535;

/// Returns `true` if the index is not the sentinel value.
#[inline]
pub fn index_is_valid(index: Index) -> bool {
    index != INDEX_INVALID
}

/// Mutable slice view of `Index` values (mirrors C++ `Vtr::IndexArray`).
pub type IndexArray<'a>      = Array<'a, Index>;

/// Immutable slice view of `Index` values (mirrors C++ `Vtr::ConstIndexArray`).
pub type ConstIndexArray<'a> = ConstArray<'a, Index>;

/// Mutable slice view of `LocalIndex` values.
pub type LocalIndexArray<'a>      = Array<'a, LocalIndex>;

/// Immutable slice view of `LocalIndex` values.
pub type ConstLocalIndexArray<'a> = ConstArray<'a, LocalIndex>;

/// Owned vector of indices — convenience alias (mirrors C++ `IndexVector`).
pub type IndexVector = Vec<Index>;
