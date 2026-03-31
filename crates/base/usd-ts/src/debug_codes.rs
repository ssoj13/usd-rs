//! Debug codes for the ts (Timecode Splines) module.
//!
//! Port of pxr/base/ts/debugCodes.h
//!
//! These are conditionally compile-time enabled debug codes
//! (disabled by default for performance).

/// Debug code enumeration for the ts module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TsDebugCode {
    /// Debug inner-loop iteration.
    DebugLoops,
    /// Debug spline sampling.
    DebugSample,
}

impl TsDebugCode {
    /// Returns the debug code name as a string.
    pub fn name(&self) -> &'static str {
        match self {
            Self::DebugLoops => "TS_DEBUG_LOOPS",
            Self::DebugSample => "TS_DEBUG_SAMPLE",
        }
    }

    /// Returns all debug codes.
    pub fn all() -> &'static [TsDebugCode] {
        &[Self::DebugLoops, Self::DebugSample]
    }

    /// Whether this debug code is compile-time enabled.
    ///
    /// In C++, these are TF_CONDITIONALLY_COMPILE_TIME_ENABLED_DEBUG_CODES
    /// with `false`, meaning they are compile-time disabled by default.
    pub const fn is_compile_time_enabled(&self) -> bool {
        false
    }
}

impl std::fmt::Display for TsDebugCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_code_name() {
        assert_eq!(TsDebugCode::DebugLoops.name(), "TS_DEBUG_LOOPS");
        assert_eq!(TsDebugCode::DebugSample.name(), "TS_DEBUG_SAMPLE");
    }

    #[test]
    fn test_all_codes() {
        let all = TsDebugCode::all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_display() {
        let code = TsDebugCode::DebugLoops;
        assert_eq!(format!("{}", code), "TS_DEBUG_LOOPS");
    }

    #[test]
    fn test_compile_time_disabled() {
        // These are compile-time disabled by default (matches C++ `false` param)
        assert!(!TsDebugCode::DebugLoops.is_compile_time_enabled());
    }
}
