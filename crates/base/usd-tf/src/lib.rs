#![allow(unsafe_code)]
//! Tools Foundation (tf) - Core utilities for USD.
//!
//! This module provides foundational types and utilities used throughout USD:
//!
//! - [`Token`] - Interned strings for efficient comparison and hashing
//! - [`CallContext`] - Source location capture for diagnostics
//! - [`Debug`] - Conditional debugging output system
//! - Diagnostic system (errors, warnings, status messages)
//! - Reference counting and smart pointers
//! - Type system utilities
//!
//! # Tokens
//!
//! Tokens are the most commonly used type in USD. They provide O(1)
//! comparison and hashing for strings that are used as identifiers.
//!
//! ```
//! use usd_tf::Token;
//!
//! let t1 = Token::new("myAttribute");
//! let t2 = Token::new("myAttribute");
//! assert_eq!(t1, t2); // O(1) comparison
//! ```
//!
//! # Diagnostics
//!
//! The diagnostic system provides macros for error reporting:
//!
//! ```
//! use usd_tf::{tf_warn, tf_status};
//!
//! tf_warn!("Something might be wrong");
//! tf_status!("Processing complete");
//! ```
//!
//! # Debug Output
//!
//! The debug system allows conditional debug messages:
//!
//! ```
//! use usd_tf::{Debug, tf_debug_msg};
//!
//! Debug::register("MY_DEBUG", "Debug for my feature");
//! Debug::enable("MY_DEBUG");
//! tf_debug_msg!("MY_DEBUG", "Processing item {}", 42);
//! ```

pub mod any_unique_ptr;
pub mod any_weak_ptr;
pub mod atomic_ofstream_wrapper;
pub mod atomic_rename;
pub mod big_rw_mutex;
pub mod bit_utils;
pub mod bits;
mod call_context;
pub mod compressed_bits;
pub mod cxx_cast;
pub mod debug;
pub mod debug_codes;
pub mod debug_notice;
pub mod declare_ptrs;
pub mod delegated_count_ptr;
pub mod dense_hashmap;
pub mod dense_hashset;
mod diagnostic;
pub mod diagnostic_base;
pub mod diagnostic_helper;
pub mod diagnostic_lite;
mod diagnostic_mgr;
pub mod dl;
mod enum_type;
pub mod env_setting;
mod error;
mod error_mark;
mod error_transport;
pub mod exception;
pub mod expiry_notifier;
pub mod fast_compression;
pub mod file_utils;
pub mod function_ref;
pub mod function_traits;
pub mod getenv;
pub mod hash;
pub mod hashmap;
pub mod hashset;
pub mod iterator;
pub mod malloc_tag;
pub mod meta;
pub mod notice;
pub mod notice_registry;
pub mod null_ptr;
pub mod ostream_methods;
pub mod path_utils;
pub mod pattern_matcher;
pub mod pointer_and_bits;
pub mod preprocessor_utils_lite;
pub mod ref_base;
mod ref_ptr;
pub mod ref_ptr_tracker;
pub mod reg_test;
pub mod registry_manager;
pub mod safe_output_file;
pub mod safe_type_compare;
pub mod scope_description;
pub mod scope_description_private;
pub mod scoped;
pub mod script_module_loader;
pub mod setenv;
pub mod singleton;
pub mod small_vector;
pub mod span;
pub mod spin_mutex;
pub mod spin_rw_mutex;
pub mod stack_trace;
pub mod stacked;
pub mod static_data;
pub mod static_tokens;
mod status;
pub mod stl;
pub mod stopwatch;
pub mod string_utils;
mod token;
pub mod type_functions;
mod type_info;
pub mod type_info_map;
pub mod type_notice;
pub mod unicode_character_classes;
pub mod unicode_utils;
mod warning;
pub mod weak_base;
mod weak_ptr;
pub mod weak_ptr_facade;

pub use call_context::*;
pub use debug::*;
pub use debug_codes::TfDebugCode;
pub use debug_notice::{DebugSymbolEnableChangedNotice, DebugSymbolsChangedNotice};
pub use diagnostic::*;
pub use diagnostic_base::*;
pub use diagnostic_lite::DiagnosticLiteHelper;
pub use diagnostic_mgr::*;
pub use enum_type::*;
pub use error::TfError;
pub use error_mark::*;
pub use error_transport::*;
pub use ref_base::*;
pub use ref_ptr::*;
pub use ref_ptr_tracker::{
    OwnerTraces, RefPtrTrackable, RefPtrTracker, RefPtrTrackerUtil, Trace, TraceType,
    WatchedCounts, watch_all, watch_none,
};
pub use status::TfStatus;
pub use token::*;
pub use type_info::*;
pub use warning::TfWarning;
pub use weak_base::*;
pub use weak_ptr::*;

// NOTE: #[macro_export] macros are automatically at crate root.
// Do NOT re-export them with `pub use crate::macro_name` — it causes
// E0255 (defined multiple times) in standalone crate builds.
