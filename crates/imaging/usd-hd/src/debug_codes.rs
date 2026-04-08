//! Debug codes for Hydra TF_DEBUG logging.
//!
//! In the original C++ USD implementation, these are used with the TF_DEBUG
//! macro system for conditional debug output. In Rust, use the `log` or
//! `tracing` crates for similar functionality.
//!
//! # Example
//!
//! ```rust,ignore
//! use log::debug;
//!
//! debug!(target: "hd::rprim", "Rprim added: {:?}", id);
//! ```

use usd_tf::Token;

/// Debug codes for Hydra subsystems.
///
/// These correspond to TF_DEBUG codes in the C++ implementation.
/// Use with the `log` or `tracing` crates for debug output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HdDebugCode {
    // Bprim (buffer prim) operations
    /// Buffer primitive added to scene index.
    /// Tracks when new buffer primitives (lights, cameras, materials) are added.
    BprimAdded,

    /// Buffer primitive removed from scene index.
    /// Tracks when buffer primitives are removed from the scene.
    BprimRemoved,

    // Buffer management
    /// Buffer array allocation and layout information.
    /// Dumps detailed information about buffer array structure and memory layout.
    BufferArrayInfo,

    /// Buffer array range cleanup operations.
    /// Tracks when buffer array ranges are deallocated and cleaned up.
    BufferArrayRangeCleaned,

    // Cache statistics
    /// Cache hit events.
    /// Logs when requested data is found in cache, useful for performance analysis.
    CacheHits,

    /// Cache miss events.
    /// Logs when requested data is not in cache, indicating cache thrashing or cold starts.
    CacheMisses,

    // Performance counters
    /// Performance counter value changes.
    /// Tracks changes to Hydra performance counters and statistics.
    CounterChanged,

    // Collections
    /// All collections marked dirty.
    /// Logs when all render collections are invalidated and need reprocessing.
    DirtyAllCollections,

    /// Dirty list processing.
    /// Tracks which prims are marked dirty and need synchronization.
    DirtyList,

    // Threading
    /// Disable multithreaded rprim synchronization.
    /// Forces single-threaded sync for debugging race conditions.
    DisableMultithreadedRprimSync,

    // Culling
    /// Draw items culled by frustum or other mechanisms.
    /// Reports how many draw items were culled and why.
    DrawItemsCulled,

    // Engine
    /// Rendering engine phase information.
    /// Logs detailed information about each rendering phase execution.
    EnginePhaseInfo,

    // Ext computations
    /// External computation primitive added.
    /// Tracks when new ext computation prims are added to scene.
    ExtComputationAdded,

    /// External computation primitive removed.
    /// Tracks when ext computation prims are removed from scene.
    ExtComputationRemoved,

    /// External computation primitive updated.
    /// Logs updates to ext computation prim parameters or connections.
    ExtComputationUpdated,

    /// External computation execution.
    /// Tracks execution of ext computation kernels and their results.
    ExtComputationExecution,

    // Culling frustum
    /// Freeze the culling frustum for debugging.
    /// Prevents frustum updates to debug culling issues.
    FreezeCullFrustum,

    // Instancer operations
    /// Instancer primitive added to scene.
    /// Tracks when new instancers are added for debugging instance management.
    InstancerAdded,

    /// Instancer primitive cleaned up.
    /// Logs when instancer resources are cleaned and released.
    InstancerCleaned,

    /// Instancer primitive removed from scene.
    /// Tracks when instancers are removed from the render index.
    InstancerRemoved,

    /// Instancer primitive updated.
    /// Logs updates to instancer parameters, transforms, or instance arrays.
    InstancerUpdated,

    // Render settings
    /// Render settings and configuration changes.
    /// Tracks changes to render settings prims and their parameters.
    RenderSettings,

    // Renderer plugin
    /// Renderer plugin loading and initialization.
    /// Logs plugin discovery, loading, and initialization steps.
    RendererPlugin,

    // Rprim (renderable prim) operations
    /// Renderable primitive added to scene index.
    /// Tracks when new meshes, curves, points, volumes are added.
    RprimAdded,

    /// Renderable primitive cleaned up.
    /// Logs when rprim resources are cleaned and GPU memory is released.
    RprimCleaned,

    /// Renderable primitive removed from scene index.
    /// Tracks when rprims are removed from the render index.
    RprimRemoved,

    /// Renderable primitive updated.
    /// Logs updates to rprim topology, transforms, materials, or primvars.
    RprimUpdated,

    // Safe mode
    /// Safe mode operation flags.
    /// Enables additional validation and error checking at performance cost.
    SafeMode,

    // Selection
    /// Selection state updates.
    /// Tracks changes to prim selection state for picking and highlighting.
    SelectionUpdate,

    // Shared ext computation data
    /// Shared external computation data management.
    /// Tracks sharing and caching of ext computation results across prims.
    SharedExtComputationData,

    // Sprim (state prim) operations
    /// State primitive added to scene index.
    /// Tracks when new state prims (lights, cameras, materials) are added.
    SprimAdded,

    /// State primitive removed from scene index.
    /// Tracks when state prims are removed from the render index.
    SprimRemoved,

    // Sync
    /// Synchronize all prims regardless of dirty state.
    /// Forces full scene sync, bypassing dirty tracking optimization.
    SyncAll,

    // Task operations
    /// Render task added to task graph.
    /// Tracks when new tasks are added to the execution graph.
    TaskAdded,

    /// Render task removed from task graph.
    /// Tracks when tasks are removed from the execution graph.
    TaskRemoved,

    // Varying state
    /// Varying state changes per-frame.
    /// Tracks state that changes every frame like time-varying transforms.
    VaryingState,
}

impl HdDebugCode {
    /// Get the debug code as a token for use with TF_DEBUG.
    pub fn as_token(&self) -> Token {
        Token::new(self.as_str())
    }

    /// Get the debug code as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BprimAdded => "HD_BPRIM_ADDED",
            Self::BprimRemoved => "HD_BPRIM_REMOVED",
            Self::BufferArrayInfo => "HD_BUFFER_ARRAY_INFO",
            Self::BufferArrayRangeCleaned => "HD_BUFFER_ARRAY_RANGE_CLEANED",
            Self::CacheHits => "HD_CACHE_HITS",
            Self::CacheMisses => "HD_CACHE_MISSES",
            Self::CounterChanged => "HD_COUNTER_CHANGED",
            Self::DirtyAllCollections => "HD_DIRTY_ALL_COLLECTIONS",
            Self::DirtyList => "HD_DIRTY_LIST",
            Self::DisableMultithreadedRprimSync => "HD_DISABLE_MULTITHREADED_RPRIM_SYNC",
            Self::DrawItemsCulled => "HD_DRAWITEMS_CULLED",
            Self::EnginePhaseInfo => "HD_ENGINE_PHASE_INFO",
            Self::ExtComputationAdded => "HD_EXT_COMPUTATION_ADDED",
            Self::ExtComputationRemoved => "HD_EXT_COMPUTATION_REMOVED",
            Self::ExtComputationUpdated => "HD_EXT_COMPUTATION_UPDATED",
            Self::ExtComputationExecution => "HD_EXT_COMPUTATION_EXECUTION",
            Self::FreezeCullFrustum => "HD_FREEZE_CULL_FRUSTUM",
            Self::InstancerAdded => "HD_INSTANCER_ADDED",
            Self::InstancerCleaned => "HD_INSTANCER_CLEANED",
            Self::InstancerRemoved => "HD_INSTANCER_REMOVED",
            Self::InstancerUpdated => "HD_INSTANCER_UPDATED",
            Self::RenderSettings => "HD_RENDER_SETTINGS",
            Self::RendererPlugin => "HD_RENDERER_PLUGIN",
            Self::RprimAdded => "HD_RPRIM_ADDED",
            Self::RprimCleaned => "HD_RPRIM_CLEANED",
            Self::RprimRemoved => "HD_RPRIM_REMOVED",
            Self::RprimUpdated => "HD_RPRIM_UPDATED",
            Self::SafeMode => "HD_SAFE_MODE",
            Self::SelectionUpdate => "HD_SELECTION_UPDATE",
            Self::SharedExtComputationData => "HD_SHARED_EXT_COMPUTATION_DATA",
            Self::SprimAdded => "HD_SPRIM_ADDED",
            Self::SprimRemoved => "HD_SPRIM_REMOVED",
            Self::SyncAll => "HD_SYNC_ALL",
            Self::TaskAdded => "HD_TASK_ADDED",
            Self::TaskRemoved => "HD_TASK_REMOVED",
            Self::VaryingState => "HD_VARYING_STATE",
        }
    }

    /// Get the log target string for use with the `log` crate.
    pub fn log_target(&self) -> &'static str {
        match self {
            Self::BprimAdded | Self::BprimRemoved => "hd::bprim",
            Self::BufferArrayInfo | Self::BufferArrayRangeCleaned => "hd::buffer",
            Self::CacheHits | Self::CacheMisses => "hd::cache",
            Self::CounterChanged => "hd::counter",
            Self::DirtyAllCollections | Self::DirtyList => "hd::dirty",
            Self::DisableMultithreadedRprimSync => "hd::threading",
            Self::DrawItemsCulled | Self::FreezeCullFrustum => "hd::culling",
            Self::EnginePhaseInfo => "hd::engine",
            Self::ExtComputationAdded
            | Self::ExtComputationRemoved
            | Self::ExtComputationUpdated
            | Self::ExtComputationExecution
            | Self::SharedExtComputationData => "hd::extcomp",
            Self::InstancerAdded
            | Self::InstancerCleaned
            | Self::InstancerRemoved
            | Self::InstancerUpdated => "hd::instancer",
            Self::RenderSettings => "hd::render_settings",
            Self::RendererPlugin => "hd::plugin",
            Self::RprimAdded | Self::RprimCleaned | Self::RprimRemoved | Self::RprimUpdated => {
                "hd::rprim"
            }
            Self::SafeMode => "hd::safe",
            Self::SelectionUpdate => "hd::selection",
            Self::SprimAdded | Self::SprimRemoved => "hd::sprim",
            Self::SyncAll => "hd::sync",
            Self::TaskAdded | Self::TaskRemoved => "hd::task",
            Self::VaryingState => "hd::state",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_code_strings() {
        let code = HdDebugCode::RprimAdded;
        assert_eq!(code.as_str(), "HD_RPRIM_ADDED");
        assert_eq!(code.log_target(), "hd::rprim");
    }

    #[test]
    fn test_all_debug_codes() {
        // Verify all codes have unique strings
        let codes = vec![
            HdDebugCode::BprimAdded,
            HdDebugCode::BprimRemoved,
            HdDebugCode::RprimAdded,
            HdDebugCode::SprimAdded,
        ];

        for code in codes {
            assert!(!code.as_str().is_empty());
            assert!(!code.log_target().is_empty());
        }
    }
}
