
//! Storm debug flags (ported from debugCodes.h).
//!
//! TF_DEBUG codes for controlling Storm diagnostic output.
//! Enable at runtime via TF_DEBUG environment variable.

use std::sync::atomic::{AtomicU32, Ordering};

/// Storm debug flag categories.
///
/// Each flag controls a specific area of diagnostic output.
/// Ported from C++ TF_DEBUG_CODES in hdSt/debugCodes.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum HdStDebugCode {
    /// General draw operations
    Draw = 0,
    /// Draw batch creation and management
    DrawBatch = 1,
    /// Force rebuild of all draw batches
    ForceDrawBatchRebuild = 2,
    /// Draw item gathering from render index
    DrawItemGather = 3,
    /// Draw items cache hits/misses
    DrawItemsCache = 4,
    /// Disable view frustum culling
    DisableFrustumCulling = 5,
    /// Disable multi-threaded culling
    DisableMultithreadedCulling = 6,
    /// Dump GLSLFX config blocks
    DumpGlslfxConfig = 7,
    /// Dump shader source on compilation failure
    DumpFailingShaderSource = 8,
    /// Dump shader source to file on failure
    DumpFailingShaderSourceFile = 9,
    /// Dump all shader source
    DumpShaderSource = 10,
    /// Dump all shader source to file
    DumpShaderSourceFile = 11,
    /// Log compute shader program cache hits
    LogComputeShaderProgramHits = 12,
    /// Log compute shader program cache misses
    LogComputeShaderProgramMisses = 13,
    /// Log drawing shader program cache hits
    LogDrawingShaderProgramHits = 14,
    /// Log drawing shader program cache misses
    LogDrawingShaderProgramMisses = 15,
    /// Log texture fallback usage
    LogTextureFallbacks = 16,
    /// Log skipped primvar processing
    LogSkippedPrimvar = 17,
    /// Material added to scene
    MaterialAdded = 18,
    /// Material removed from scene
    MaterialRemoved = 19,
    /// MaterialX processing
    Mtlx = 20,
    /// MaterialX value inspection
    MtlxValues = 21,
    /// Disable MaterialX anonymization
    MtlxDisableAnonymize = 22,
    /// Dump MaterialX shader source to file
    MtlxDumpShaderSourceFile = 23,
}

/// Bitmask of active debug flags for fast runtime checks.
static ACTIVE_FLAGS: AtomicU32 = AtomicU32::new(0);

impl HdStDebugCode {
    /// Get the string name of this debug code (matches C++ TF_DEBUG symbol).
    pub fn name(&self) -> &'static str {
        match self {
            Self::Draw => "HDST_DRAW",
            Self::DrawBatch => "HDST_DRAW_BATCH",
            Self::ForceDrawBatchRebuild => "HDST_FORCE_DRAW_BATCH_REBUILD",
            Self::DrawItemGather => "HDST_DRAW_ITEM_GATHER",
            Self::DrawItemsCache => "HDST_DRAWITEMS_CACHE",
            Self::DisableFrustumCulling => "HDST_DISABLE_FRUSTUM_CULLING",
            Self::DisableMultithreadedCulling => "HDST_DISABLE_MULTITHREADED_CULLING",
            Self::DumpGlslfxConfig => "HDST_DUMP_GLSLFX_CONFIG",
            Self::DumpFailingShaderSource => "HDST_DUMP_FAILING_SHADER_SOURCE",
            Self::DumpFailingShaderSourceFile => "HDST_DUMP_FAILING_SHADER_SOURCEFILE",
            Self::DumpShaderSource => "HDST_DUMP_SHADER_SOURCE",
            Self::DumpShaderSourceFile => "HDST_DUMP_SHADER_SOURCEFILE",
            Self::LogComputeShaderProgramHits => "HDST_LOG_COMPUTE_SHADER_PROGRAM_HITS",
            Self::LogComputeShaderProgramMisses => "HDST_LOG_COMPUTE_SHADER_PROGRAM_MISSES",
            Self::LogDrawingShaderProgramHits => "HDST_LOG_DRAWING_SHADER_PROGRAM_HITS",
            Self::LogDrawingShaderProgramMisses => "HDST_LOG_DRAWING_SHADER_PROGRAM_MISSES",
            Self::LogTextureFallbacks => "HDST_LOG_TEXTURE_FALLBACKS",
            Self::LogSkippedPrimvar => "HDST_LOG_SKIPPED_PRIMVAR",
            Self::MaterialAdded => "HDST_MATERIAL_ADDED",
            Self::MaterialRemoved => "HDST_MATERIAL_REMOVED",
            Self::Mtlx => "HDST_MTLX",
            Self::MtlxValues => "HDST_MTLX_VALUES",
            Self::MtlxDisableAnonymize => "HDST_MTLX_DISABLE_ANONYMIZE",
            Self::MtlxDumpShaderSourceFile => "HDST_MTLX_DUMP_SHADER_SOURCEFILE",
        }
    }

    /// Check if this debug flag is currently enabled.
    pub fn is_enabled(&self) -> bool {
        let mask = 1u32 << (*self as u32);
        ACTIVE_FLAGS.load(Ordering::Relaxed) & mask != 0
    }

    /// Enable this debug flag.
    pub fn enable(&self) {
        let mask = 1u32 << (*self as u32);
        ACTIVE_FLAGS.fetch_or(mask, Ordering::Relaxed);
    }

    /// Disable this debug flag.
    pub fn disable(&self) {
        let mask = 1u32 << (*self as u32);
        ACTIVE_FLAGS.fetch_and(!mask, Ordering::Relaxed);
    }
}

/// Enable all debug flags.
pub fn enable_all() {
    ACTIVE_FLAGS.store(u32::MAX, Ordering::Relaxed);
}

/// Disable all debug flags.
pub fn disable_all() {
    ACTIVE_FLAGS.store(0, Ordering::Relaxed);
}

/// Check if a debug code is enabled (convenience macro-like function).
#[inline]
pub fn is_debug_enabled(code: HdStDebugCode) -> bool {
    code.is_enabled()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_codes() {
        disable_all();
        assert!(!HdStDebugCode::Draw.is_enabled());

        HdStDebugCode::Draw.enable();
        assert!(HdStDebugCode::Draw.is_enabled());
        assert!(!HdStDebugCode::DrawBatch.is_enabled());

        HdStDebugCode::Draw.disable();
        assert!(!HdStDebugCode::Draw.is_enabled());
    }

    #[test]
    fn test_names() {
        assert_eq!(HdStDebugCode::Draw.name(), "HDST_DRAW");
        assert_eq!(HdStDebugCode::Mtlx.name(), "HDST_MTLX");
    }
}
