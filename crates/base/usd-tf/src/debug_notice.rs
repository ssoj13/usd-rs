//! Debug system notices.
//!
//! Port of pxr/base/tf/debugNotice.h
//!
//! Notices sent when debug symbols are changed.

use crate::notice::Notice;

/// Notice sent when the list of available debug symbol names has changed.
///
/// This notice is broadcast when debug symbols are registered or unregistered.
#[derive(Debug, Clone, Default)]
pub struct DebugSymbolsChangedNotice;

impl DebugSymbolsChangedNotice {
    /// Creates a new notice.
    pub fn new() -> Self {
        Self
    }
}

impl Notice for DebugSymbolsChangedNotice {
    fn notice_type_name() -> &'static str {
        "DebugSymbolsChangedNotice"
    }
}

/// Notice sent when a debug symbol has been enabled or disabled.
///
/// This notice is broadcast when `TfDebug::SetDebugSymbolsByName` or
/// similar functions are called.
#[derive(Debug, Clone, Default)]
pub struct DebugSymbolEnableChangedNotice;

impl DebugSymbolEnableChangedNotice {
    /// Creates a new notice.
    pub fn new() -> Self {
        Self
    }
}

impl Notice for DebugSymbolEnableChangedNotice {
    fn notice_type_name() -> &'static str {
        "DebugSymbolEnableChangedNotice"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbols_changed_notice() {
        let notice = DebugSymbolsChangedNotice::new();
        // Just verify it can be created and cloned
        let _ = notice.clone();
    }

    #[test]
    fn test_enable_changed_notice() {
        let notice = DebugSymbolEnableChangedNotice::new();
        let _ = notice.clone();
    }
}
