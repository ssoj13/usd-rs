//! HGI Interop - Cross-backend texture presentation via wgpu.
//!
//! Port of pxr/imaging/hgiInterop
//!
//! Composites HGI render results (color + optional depth textures) onto a
//! wgpu surface using a fullscreen-triangle blit with proper alpha blending.

pub mod interop;
pub mod wgpu_interop;

pub use interop::HgiInterop;
pub use wgpu_interop::HgiInteropWgpu;
