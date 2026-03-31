//! # usd-rs
//!
//! Rust port of OpenUSD (Universal Scene Description).
//!
//! This library provides a pure Rust implementation of Pixar's OpenUSD,
//! offering improved safety, ergonomics, and modularity over the C++ original.
//!
//! The root crate is a thin facade that re-exports all sub-crates.

// ---- Base crates (pxr/base) ----

/// Architecture abstractions (OS, CPU, memory).
pub use usd_arch as arch;
/// Graphics Foundation — math, vectors, matrices, geometry.
pub use usd_gf as gf;
/// JSON utilities.
pub use usd_js as js;
/// Tools Foundation — tokens, diagnostics, type system.
pub use usd_tf as tf;
/// Tracing and profiling.
pub use usd_trace as trace;
/// Spline/animation curves.
pub use usd_ts as ts;
/// Value types — type-erased container for scene data.
pub use usd_vt as vt;
/// Work dispatcher — parallel task execution.
pub use usd_work as work;

// ---- USD core crates (pxr/usd) ----

/// Asset Resolution — file/asset path resolution.
pub use usd_ar as ar;
/// Kind — model hierarchy classification.
pub use usd_kind as kind;
/// PCP — Prim Cache Population (composition engine).
pub use usd_pcp as pcp;
/// Scene Description Foundation — layers, paths, specs.
pub use usd_sdf as sdf;
/// Shader Definition Registry.
pub use usd_sdr as sdr;

/// Core USD — Stage, Prim, Attribute, etc.
///
/// Note: accessed as `usd::usd::Stage` etc., or via re-exports below.
pub mod usd {
    pub use usd_core::*;
}

// ---- USD schema crates ----

/// Geometry schemas (Mesh, Points, BasisCurves, etc.).
pub use usd_geom;
/// Hydra interop schemas.
pub use usd_hydra;
/// Lighting schemas.
pub use usd_lux;
/// Media schemas.
pub use usd_media;
/// MaterialX schemas.
pub use usd_mtlx;
/// Physics schemas.
pub use usd_physics;
/// Procedural schemas.
pub use usd_proc;
/// Render schemas.
pub use usd_render;
/// RenderMan schemas.
pub use usd_ri;
/// Semantic label schemas.
pub use usd_semantics;
/// Shading schemas (Material, Shader, etc.).
pub use usd_shade;
/// Skeleton and skinning schemas.
pub use usd_skel;
/// UI schemas.
pub use usd_ui;
/// USD utilities (flattening, stitching, etc.).
pub use usd_utils;
/// Volume schemas.
pub use usd_vol;

// ---- Imaging crates (pxr/imaging) ----

/// Imaging facade — sub-crate re-exports.
pub mod imaging {
    /// USD Application utilities.
    pub use usd_app_utils as app_utils;
    /// Camera utilities.
    pub use usd_camera_util as camera_util;
    /// Geometry utilities (tessellation, subdivision helpers).
    pub use usd_geom_util as geom_util;
    /// GL Foundation — OpenGL utilities.
    pub use usd_glf as glf;
    /// Hydra core — render delegate, scene index, etc.
    pub use usd_hd as hd;
    /// Hydra generative procedurals.
    pub use usd_hd_gp as hd_gp;
    /// Hydra MaterialX integration.
    pub use usd_hd_mtlx as hd_mtlx;
    /// Hydra Storm renderer.
    pub use usd_hd_st as hd_st;
    /// Hydra Asset Resolution adapter.
    pub use usd_hdar as hdar;
    /// Hydra Scene Index plugins.
    pub use usd_hdsi as hdsi;
    /// Hydra extension utilities.
    pub use usd_hdx as hdx;
    /// Hydra Foundation — plugin system.
    pub use usd_hf as hf;
    /// Hydra Graphics Interface — GPU abstraction.
    pub use usd_hgi as hgi;
    /// HGI interop (GL-Vulkan).
    pub use usd_hgi_interop as hgi_interop;
    /// HGI Metal backend.
    pub use usd_hgi_metal as hgi_metal;
    /// HGI Vulkan backend.
    pub use usd_hgi_vulkan as hgi_vulkan;
    /// Hydra image I/O.
    pub use usd_hio as hio;
    /// OpenSubdiv wrapper.
    pub use usd_px_osd as px_osd;
}

/// USD Imaging — scene adapters connecting USD to Hydra.
pub use usd_imaging;

/// USD Validation framework.
pub use usd_validation;

/// glTF 2.0 loader — re-exported from gltf-rs.
pub mod gltf {
    pub use gltf_crate::*;
}

/// Test utilities for timeout and deadlock detection.
#[cfg(test)]
pub mod test_utils;

// ---- Convenience re-exports ----

// Base types at root
pub use sdf::Path;
pub use usd_core::{Attribute, InitialLoadSet, Prim, Stage, TimeCode};
