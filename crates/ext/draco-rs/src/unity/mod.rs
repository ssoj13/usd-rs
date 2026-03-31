//! Rust port of Draco Unity plugin layer.
//! Reference: `_ref/draco/src/draco/unity`.
//!
//! The C ABI implementation lives in the dedicated `draco-unity` crate (cdylib
//! name `draco_unity`). This module re-exports the same symbols for parity and
//! to keep the Rust-side API surface consistent with the reference.

pub use draco_unity::{
    DecodeDracoMesh, DecodeMeshForUnity, DracoAttribute, DracoData, DracoMesh, DracoToUnityMesh,
    ReleaseDracoAttribute, ReleaseDracoData, ReleaseDracoMesh, ReleaseUnityMesh,
};
