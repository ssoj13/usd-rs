//! Architecture-specific abstractions.
//!
//! This module provides platform-independent abstractions for:
//! - Memory alignment
//! - Compiler attributes and function annotations
//! - Hash functions (SpookyHash)
//! - High-resolution timing
//! - System information
//! - Environment variables
//! - Thread utilities
//! - File system operations
//! - Error reporting and diagnostics
//! - Virtual memory management
//! - Stack trace capture and formatting
//! - Symbol demangling (C++ and Rust)
//! - Background process (daemon) creation
//! - Malloc hooks and allocator instrumentation
//!
//! # Platform Detection
//!
//! Use platform detection functions for compile-time platform detection:
//! ```rust
//! use usd_arch::{os_name, arch_name};
//!
//! println!("Running on {} / {}", os_name(), arch_name());
//! ```

mod align;
mod assumptions;
pub mod attributes;
mod daemon;
mod debugger;
mod defines;
mod demangle;
mod env;
mod errno;
mod error;
mod file_system;
mod function;
mod function_lite;
pub mod hash;
mod hints;
mod init_config;
mod inttypes;
mod library;
pub mod malloc_hook;
mod math;
mod pragmas;
mod regex;
mod stack_trace;
mod symbols;
mod system_info;
mod threads;
pub mod timing;
mod virtual_memory;
mod vsnprintf;

pub use align::*;
pub use assumptions::*;
pub use attributes::*;
pub use daemon::*;
pub use debugger::*;
pub use defines::*;
pub use demangle::*;
pub use env::*;
pub use errno::*;
pub use error::*;
#[allow(ambiguous_glob_reexports)]
pub use file_system::*;
pub use function::*;
pub use hash::*;
pub use hints::*;
pub use init_config::*;
pub use inttypes::*;
pub use library::*;
pub use malloc_hook::*;
pub use math::*;
pub use pragmas::*;
pub use regex::*;
pub use stack_trace::*;
pub use symbols::*;
pub use system_info::*;
pub use threads::*;
pub use timing::*;
pub use virtual_memory::*;
pub use vsnprintf::*;

#[cfg(test)]
mod tests;
