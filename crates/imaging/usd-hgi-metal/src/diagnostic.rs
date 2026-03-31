//! Metal debug/diagnostic utilities. Port of pxr/imaging/hgiMetal/diagnostic

use std::sync::atomic::{AtomicBool, Ordering};

static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Returns whether Metal debug labels/groups are enabled.
/// Mirrors C++ HgiMetalDebugEnabled().
pub fn debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Setup Metal debug facilities.
/// Mirrors C++ HgiMetalSetupMetalDebug().
/// On non-Metal platforms this is a no-op.
pub fn setup_metal_debug() {
    // On macOS, this would check environment variables and enable
    // Metal validation layers. Stub on non-Apple platforms.
    if std::env::var("HGIMETAL_DEBUG").is_ok() {
        DEBUG_ENABLED.store(true, Ordering::Relaxed);
    }
}

/// Posts diagnostic errors for all Metal errors in the current context.
/// Mirrors C++ HgiMetalPostPendingErrors().
pub fn post_pending_errors(_where: &str) {
    // Stub: no Metal context available on non-Apple platforms
}

/// Set a debug label on a Metal object (no-op stub).
/// Mirrors C++ HGIMETAL_DEBUG_LABEL macro.
#[inline]
pub fn debug_label(_label: &str) {
    // Stub: requires Metal object
}

/// Push a debug group (no-op stub).
/// Mirrors C++ HGIMETAL_DEBUG_PUSH_GROUP macro.
#[inline]
pub fn debug_push_group(_label: &str) {
    // Stub: requires Metal encoder
}

/// Pop a debug group (no-op stub).
/// Mirrors C++ HGIMETAL_DEBUG_POP_GROUP macro.
#[inline]
pub fn debug_pop_group() {
    // Stub: requires Metal encoder
}

/// Insert a debug signpost marker (no-op stub).
/// Mirrors C++ HGIMETAL_DEBUG_INSERT_DEBUG_MARKER macro.
#[inline]
pub fn debug_insert_marker(_label: &str) {
    // Stub: requires Metal encoder
}
