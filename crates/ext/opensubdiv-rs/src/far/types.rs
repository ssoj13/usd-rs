// Copyright 2014 DreamWorks Animation LLC.
// Ported to Rust from OpenSubdiv 3.7.0 far/types.h
//
// Far index types are re-exported directly from VTR.

pub use crate::vtr::types::{
    Index,
    LocalIndex,
    IndexVector,
    INDEX_INVALID,
    VALENCE_LIMIT,
    index_is_valid,
};

pub use crate::vtr::array::{Array, ConstArray};

/// Mutable slice view of `Index` values (mirrors C++ `Far::IndexArray`).
pub type IndexArray<'a>      = Array<'a, Index>;

/// Immutable slice view of `Index` values (mirrors C++ `Far::ConstIndexArray`).
pub type ConstIndexArray<'a> = ConstArray<'a, Index>;

/// Mutable slice view of `LocalIndex` values.
pub type LocalIndexArray<'a>      = Array<'a, LocalIndex>;

/// Immutable slice view of `LocalIndex` values.
pub type ConstLocalIndexArray<'a> = ConstArray<'a, LocalIndex>;
