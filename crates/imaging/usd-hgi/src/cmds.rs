//! GPU command buffer abstraction for HGI
//!
//! This module provides the base command buffer interface for HGI (Hydra Graphics Interface).
//! Command buffers are the primary mechanism for recording GPU work (rendering, compute,
//! transfers) and submitting it for execution.
//!
//! # Architecture
//!
//! The HGI command buffer system uses a two-trait design:
//!
//! - [`HgiCmds`]: Public API for recording commands and debug markers
//! - [`HgiCmdsSubmit`]: Internal API for submitting to GPU (backend-specific)
//!
//! This separation ensures:
//! - Users work with a clean, safe recording API
//! - Backend implementations handle submission details
//! - Command buffers can only be submitted once
//!
//! # Command Buffer Types
//!
//! HGI supports specialized command buffer types:
//! - Graphics commands (HgiGraphicsCmds)
//! - Compute commands (HgiComputeCmds)
//! - Blit/transfer commands (HgiBlitCmds)
//!
//! Each type has a debug color for GPU profiling tools.
//!
//! # Usage Pattern
//!
//! ```ignore
//! // Create and record commands
//! let mut cmds = hgi.create_graphics_cmds();
//! cmds.push_debug_group("Render Pass");
//! // ... record rendering commands ...
//! cmds.pop_debug_group();
//!
//! // Submit to GPU (can only submit once)
//! hgi.submit_cmds(&mut cmds, HgiSubmitWaitType::NoWait);
//! assert!(cmds.is_submitted());
//! ```
//!
//! # OpenUSD Reference
//!
//! This corresponds to the HGI command buffer system in OpenUSD:
//! - `pxr/imaging/hgi/cmds.h`
//! - `pxr/imaging/hgi/graphicsCmds.h`
//! - `pxr/imaging/hgi/computeCmds.h`
//! - `pxr/imaging/hgi/blitCmds.h`

use super::enums::HgiSubmitWaitType;
use usd_gf::Vec4f;

/// Base trait for GPU command buffers
///
/// Graphics commands are recorded in 'cmds' objects which are later submitted to HGI.
/// This trait provides the common interface for all command buffer types.
///
/// # Thread Safety
///
/// Command buffers are `Send + Sync` and can be recorded on background threads.
/// However, submission must happen on the main rendering thread.
///
/// # Debug Markers
///
/// All command buffers support hierarchical debug markers for GPU profilers:
/// - `push_debug_group` / `pop_debug_group` for scoped regions
/// - `insert_debug_marker` for single markers
///
/// # Submission State
///
/// Once a command buffer is submitted via [`HgiCmdsSubmit::submit`],
/// `is_submitted()` returns true and it cannot be submitted again.
///
/// # OpenUSD Reference
///
/// `HgiCmds` base class in `pxr/imaging/hgi/cmds.h`
pub trait HgiCmds: Send + Sync {
    /// Returns true if the command buffer has been submitted to GPU
    ///
    /// Once submitted, a command buffer cannot be submitted again.
    /// This check prevents double-submission bugs.
    ///
    /// # Returns
    ///
    /// `true` if this command buffer has been submitted, `false` otherwise
    fn is_submitted(&self) -> bool;

    /// Push a hierarchical debug marker group
    ///
    /// Opens a named debug region in GPU profilers like RenderDoc, NSight, etc.
    /// Must be balanced with a corresponding [`pop_debug_group`].
    ///
    /// # Arguments
    ///
    /// * `label` - Name of the debug region (e.g., "Shadow Pass")
    ///
    /// # Example
    ///
    /// ```ignore
    /// cmds.push_debug_group("Geometry Pass");
    /// // ... record commands ...
    /// cmds.pop_debug_group();
    /// ```
    ///
    /// [`pop_debug_group`]: HgiCmds::pop_debug_group
    fn push_debug_group(&mut self, label: &str);

    /// Pop the current debug marker group
    ///
    /// Closes the most recently opened debug region.
    /// Must balance a previous [`push_debug_group`] call.
    ///
    /// # Panics
    ///
    /// Backend implementations may panic or error if there's no
    /// matching push (debug builds only).
    ///
    /// [`push_debug_group`]: HgiCmds::push_debug_group
    fn pop_debug_group(&mut self);

    /// Insert a single debug marker at the current command position
    ///
    /// Unlike push/pop, this creates a single marker point rather than
    /// a region. Useful for marking specific draw calls or operations.
    ///
    /// # Arguments
    ///
    /// * `label` - Marker name (e.g., "Draw Terrain Chunk 42")
    ///
    /// # Example
    ///
    /// ```ignore
    /// for (i, chunk) in chunks.iter().enumerate() {
    ///     cmds.insert_debug_marker(&format!("Chunk {}", i));
    ///     // ... draw chunk ...
    /// }
    /// ```
    fn insert_debug_marker(&mut self, label: &str);

    /// Execute/submit the recorded commands to the GPU.
    ///
    /// Called by `Hgi::submit_cmds` before dropping the command buffer.
    /// OpenGL backends execute GL calls here; Vulkan/Metal may no-op (deferred).
    fn execute_submit(&mut self) {}
}

// Debug colors as lazy statics since Vec4f::new is not const
use once_cell::sync::Lazy;

/// Debug color for compute command buffers (red-orange)
///
/// Used to identify compute passes in GPU profiling tools.
/// Color: RGB(0.855, 0.161, 0.11)
pub static COMPUTE_DEBUG_COLOR: Lazy<Vec4f> = Lazy::new(|| Vec4f::new(0.855, 0.161, 0.11, 1.0));

/// Debug color for graphics command buffers (blue)
///
/// Used to identify rendering passes in GPU profiling tools.
/// Color: RGB(0.0, 0.639, 0.878)
pub static GRAPHICS_DEBUG_COLOR: Lazy<Vec4f> = Lazy::new(|| Vec4f::new(0.0, 0.639, 0.878, 1.0));

/// Debug color for blit/transfer command buffers (yellow)
///
/// Used to identify copy/transfer operations in GPU profiling tools.
/// Color: RGB(0.996, 0.875, 0.0)
pub static BLIT_DEBUG_COLOR: Lazy<Vec4f> = Lazy::new(|| Vec4f::new(0.996, 0.875, 0.0, 1.0));

/// Debug color for debug markers (transparent)
///
/// Used for general debug markers without specific type.
/// Color: Transparent black
pub static MARKER_DEBUG_COLOR: Lazy<Vec4f> = Lazy::new(|| Vec4f::new(0.0, 0.0, 0.0, 0.0));

/// Internal trait for command buffer submission
///
/// This is implemented by concrete backend implementations (OpenGL, Vulkan, Metal)
/// and is not part of the public API. It provides the mechanism for HGI to submit
/// recorded commands to the GPU.
///
/// # Design Rationale
///
/// Separating submission from recording ensures:
/// - Clean public API ([`HgiCmds`]) without backend details
/// - Backend-specific submission logic is encapsulated
/// - Single-submission guarantee via state tracking
///
/// # Implementation
///
/// Backend command buffer types should implement both traits:
/// ```ignore
/// impl HgiCmds for HgiGLGraphicsCmds { /* ... */ }
/// impl HgiCmdsSubmit for HgiGLGraphicsCmds { /* ... */ }
/// ```
///
/// # OpenUSD Reference
///
/// Equivalent to protected submission methods in `HgiCmds` subclasses
pub trait HgiCmdsSubmit {
    /// Submit the command buffer to the GPU for execution
    ///
    /// This finalizes command recording and submits all commands to the
    /// GPU command queue. After submission, the command buffer is marked
    /// as submitted and cannot be reused.
    ///
    /// # Arguments
    ///
    /// * `wait` - Whether to wait for GPU completion before returning
    ///
    /// # Returns
    ///
    /// `true` if work was committed, `false` if buffer was empty or already submitted
    ///
    /// # Implementation Notes
    ///
    /// Backends should:
    /// - Finalize command recording
    /// - Submit to GPU queue
    /// - Optionally wait based on `wait` parameter
    /// - Call `set_submitted()` on success
    fn submit(&mut self, wait: HgiSubmitWaitType) -> bool;

    /// Mark the command buffer as submitted
    ///
    /// Sets the internal submitted flag to `true`. This is called by
    /// `submit()` after successful submission to prevent double-submit.
    ///
    /// # Invariant
    ///
    /// After this is called, `is_submitted()` must return `true`.
    fn set_submitted(&mut self);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCmds {
        submitted: bool,
    }

    impl HgiCmds for MockCmds {
        fn is_submitted(&self) -> bool {
            self.submitted
        }

        fn push_debug_group(&mut self, _label: &str) {}
        fn pop_debug_group(&mut self) {}
        fn insert_debug_marker(&mut self, _label: &str) {}
    }

    impl HgiCmdsSubmit for MockCmds {
        fn submit(&mut self, _wait: HgiSubmitWaitType) -> bool {
            true
        }

        fn set_submitted(&mut self) {
            self.submitted = true;
        }
    }

    #[test]
    fn test_cmds_submission() {
        let mut cmds = MockCmds { submitted: false };
        assert!(!cmds.is_submitted());

        cmds.set_submitted();
        assert!(cmds.is_submitted());
    }

    #[test]
    fn test_debug_colors() {
        assert_eq!(COMPUTE_DEBUG_COLOR.x, 0.855);
        assert_eq!(GRAPHICS_DEBUG_COLOR.y, 0.639);
        // Value was rounded from 0.99607843137 to 0.996 to avoid excessive precision clippy warning
        assert_eq!(BLIT_DEBUG_COLOR.x, 0.996);
    }
}
