//! Rust port of the Draco Maya plugin layer.
//! Reference: `_ref/draco/src/draco/maya`.
//!
//! The C ABI implementation lives in the dedicated `draco-maya` crate (cdylib
//! name `draco_maya`). This module re-exports the same symbols for parity and
//! to keep the Rust-side API surface consistent with the reference.

// Re-export the C ABI entry points and data layout used by Maya/Python.
pub use draco_maya::{
    drc2py_decode, drc2py_encode, drc2py_free, DecodeResult, Drc2PyMesh, EncodeResult,
};
